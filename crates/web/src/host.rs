use std::{
    cell::RefCell,
    collections::HashMap,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
};

use js_sys::{Function, Uint8Array};
use pc::{
    Machine, RunExit,
    fs::{DosFile, FileSystem, MemFile},
};
use wasm_bindgen::prelude::*;
use web_sys::{Blob, FileReaderSync};

/// Read-only, seekable view of a `Blob` (DUNE.DAT), read synchronously.
struct BlobFile {
    blob: Blob,
    reader: FileReaderSync,
    pos: u64,
    len: u64,
}

impl Read for BlobFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let end = (self.pos + buf.len() as u64).min(self.len);
        if end <= self.pos {
            return Ok(0);
        }
        let slice = self
            .blob
            .slice_with_f64_and_f64(self.pos as f64, end as f64)
            .map_err(|e| io::Error::other(format!("{e:?}")))?;
        let ab = self.reader.read_as_array_buffer(&slice).map_err(|e| io::Error::other(format!("{e:?}")))?;
        let n = (end - self.pos) as usize;
        Uint8Array::new(&ab).copy_to(&mut buf[..n]);
        self.pos = end;
        Ok(n)
    }
}

impl Write for BlobFile {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("read-only"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for BlobFile {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let p = match pos {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::Current(d) => self.pos as i64 + d,
            SeekFrom::End(d) => self.len as i64 + d,
        };
        self.pos = p.max(0) as u64;
        Ok(self.pos)
    }
}

impl DosFile for BlobFile {}

/// DUNE.DAT from the Blob; save games in memory, reported via `on_save`.
struct WebFs {
    dat: Blob,
    saves: Rc<RefCell<HashMap<String, Vec<u8>>>>,
    on_save: Function,
}

impl FileSystem for WebFs {
    fn open(&mut self, name: &str) -> Option<Box<dyn DosFile>> {
        if name == "DUNE.DAT" {
            let reader = FileReaderSync::new().ok()?;
            let len = self.dat.size() as u64;
            return Some(Box::new(BlobFile { blob: self.dat.clone(), reader, pos: 0, len }));
        }
        let data = self.saves.borrow().get(name)?.clone();
        Some(self.mem_file(name, data))
    }

    fn create(&mut self, name: &str) -> Option<Box<dyn DosFile>> {
        self.saves.borrow_mut().insert(name.to_string(), Vec::new());
        Some(self.mem_file(name, Vec::new()))
    }

    fn exists(&mut self, name: &str) -> bool {
        name == "DUNE.DAT" || self.saves.borrow().contains_key(name)
    }
}

impl WebFs {
    fn mem_file(&self, name: &str, data: Vec<u8>) -> Box<dyn DosFile> {
        let saves = self.saves.clone();
        let cb = self.on_save.clone();
        Box::new(MemFile {
            data: Cursor::new(data),
            name: name.to_string(),
            dirty: false,
            on_close: Some(Box::new(move |name: &str, bytes: &[u8]| {
                saves.borrow_mut().insert(name.to_string(), bytes.to_vec());
                let _ = cb.call2(&JsValue::NULL, &name.into(), &Uint8Array::from(bytes).into());
            })),
        })
    }
}

/// HD sprites and text: the reference compositor following the game's
/// drawing, and the last HD screen sent to the page.
struct Hd {
    follower: gfx::compose::Follower,
    /// The HD screen texels as last presented.
    cur: Vec<u8>,
    /// The low-resolution screen they were presented against.
    screen: Vec<u8>,
    /// Pixels falling back to low resolution, per screen row.
    fallback: Vec<u16>,
    /// Present and send the whole screen next time (after enabling or a restore).
    full: bool,
    /// Send HD frames (following continues while hidden, so turning HD on
    /// shows the current scene at once).
    visible: bool,
}

/// An HD art pack: the ZIP Blob and where its entries are.
struct Pack {
    file: BlobFile,
    index: gfx::pack::PackIndex,
}

#[wasm_bindgen]
pub struct Emu {
    m: Machine,
    frame: Vec<u8>,
    saves: Rc<RefCell<HashMap<String, Vec<u8>>>>,
    hd: Option<Rc<RefCell<Hd>>>,
    pack: Option<Rc<RefCell<Pack>>>,
}

#[wasm_bindgen]
impl Emu {
    /// `exe`: DNCDPRG.EXE bytes; `dat`: the DUNE.DAT Blob; `on_save(name, bytes)`
    /// is called when the game closes a file it wrote.
    #[wasm_bindgen(constructor)]
    pub fn new(exe: &[u8], dat: Blob, cmdline: &str, on_save: Function) -> Result<Emu, JsValue> {
        let saves = Rc::new(RefCell::new(HashMap::new()));
        let fs = WebFs { dat, saves: saves.clone(), on_save };
        let mut m = Machine::new(exe, cmdline, Box::new(fs)).map_err(|e| JsValue::from_str(&e))?;
        m.set_speed(20e6);
        Ok(Emu { m, frame: vec![0; 64000 + 768], saves, hd: None, pack: None })
    }

    /// Provide an existing save file before the game starts.
    #[wasm_bindgen(js_name = putFile)]
    pub fn put_file(&mut self, name: &str, data: &[u8]) {
        self.saves.borrow_mut().insert(pc::fs::normalize(name), data.to_vec());
    }

    /// Run for `ms` of virtual time. Returns 0 to continue, 1 if the game
    /// exited, 2 if the CPU got stuck or hit an unimplemented opcode.
    #[wasm_bindgen(js_name = runMs)]
    pub fn run_ms(&mut self, ms: f64) -> u32 {
        let deadline = self.m.now_ns() + (ms * 1e6) as u64;
        match self.m.run_until(deadline) {
            RunExit::Deadline => 0,
            RunExit::Exited(_) => 1,
            other => {
                self.m.log(format!("stopped: {other:?} {}", self.m.regs_string()));
                2
            }
        }
    }

    /// 320×200 palette indices followed by 256 RGB triples (8-bit).
    pub fn frame(&mut self) -> Vec<u8> {
        self.frame[..64000].copy_from_slice(&self.m.hw.mem[0xa0000..0xa0000 + 64000]);
        for (i, c) in self.m.hw.vga.dac.iter().enumerate() {
            for k in 0..3 {
                self.frame[64000 + i * 3 + k] = c[k] << 2 | c[k] >> 4;
            }
        }
        self.frame.clone()
    }

    /// Audio produced since the last call: interleaved stereo f32, 48 kHz.
    #[wasm_bindgen(js_name = takeAudio)]
    pub fn take_audio(&mut self) -> Vec<f32> {
        self.m.take_audio()
    }

    /// Snapshot the whole machine (compressed bytes).
    #[wasm_bindgen(js_name = saveState)]
    pub fn save_state(&mut self) -> Vec<u8> {
        self.m.save_state()
    }

    /// Restore a snapshot taken with `saveState` on the same DNCDPRG.EXE.
    #[wasm_bindgen(js_name = loadState)]
    pub fn load_state(&mut self, data: &[u8]) -> Result<(), JsValue> {
        self.m.load_state(data).map_err(|e| JsValue::from_str(&e))?;
        if let Some(hd) = &self.hd {
            let mut hd = hd.borrow_mut();
            hd.follower.reset();
            hd.full = true;
        }
        Ok(())
    }

    /// Use an HD art pack (a ZIP made by `dune-hd`) for HD sprites.
    #[wasm_bindgen(js_name = setHdPack)]
    pub fn set_hd_pack(&mut self, zip: Option<Blob>) -> Result<(), JsValue> {
        self.pack = match zip {
            Some(blob) => {
                let reader = FileReaderSync::new()?;
                let len = blob.size() as u64;
                let mut file = BlobFile { blob, reader, pos: 0, len };
                let index = gfx::pack::PackIndex::read(&mut file).map_err(|e| JsValue::from_str(&e.to_string()))?;
                Some(Rc::new(RefCell::new(Pack { file, index })))
            }
            None => None,
        };
        if let Some(hd) = &self.hd {
            hd.borrow_mut().follower.comp.set_art(self.art_source());
        }
        Ok(())
    }

    fn art_source(&self) -> Option<gfx::compose::ArtSource> {
        let pack = self.pack.clone()?;
        Some(Box::new(move |hash, k| {
            let mut p = pack.borrow_mut();
            let Pack { file, index } = &mut *p;
            index.sprite(file, hash, k)
        }))
    }

    /// Show HD frames or not (the drawing is still followed).
    #[wasm_bindgen(js_name = setHdVisible)]
    pub fn set_hd_visible(&mut self, visible: bool) {
        if let Some(hd) = &self.hd {
            let mut hd = hd.borrow_mut();
            hd.full |= visible && !hd.visible;
            hd.visible = visible;
        }
    }

    /// Follow the drawing for HD sprites and text at `k`× (2 or 4); any
    /// other value stops following.
    #[wasm_bindgen(js_name = setHd)]
    pub fn set_hd(&mut self, k: u32) {
        let k = k as usize;
        if k != 2 && k != 4 {
            self.hd = None;
            self.m.gfx.enabled = false;
            self.m.gfx.hook = None;
            return;
        }
        if self.hd.as_ref().is_some_and(|h| h.borrow().follower.comp.k == k) {
            return;
        }
        let mut follower = gfx::compose::Follower::new(k);
        // About 4 bytes per cached sprite texel: 64 MB at 4×, 24 MB at 2×.
        follower.comp.sprite_budget = if k == 4 { 16 << 20 } else { 6 << 20 };
        follower.comp.set_art(self.art_source());
        let size = 320 * k * 200 * k * 4;
        let hd = Rc::new(RefCell::new(Hd { follower, cur: vec![0; size], screen: vec![0; 64000], fallback: vec![0; 200], full: true, visible: true }));
        let h = hd.clone();
        self.m.gfx.enabled = true;
        self.m.gfx.hook = Some(Box::new(move |e, hw| {
            let mut h = h.borrow_mut();
            match e {
                pc::gfx::GfxEvent::Enter(c) => {
                    h.follower.enter(c.slot, &c.regs(), gfx::Mem(&hw.mem), &hw.vga.dac);
                }
                pc::gfx::GfxEvent::Return { .. } => h.follower.leave(gfx::Mem(&hw.mem)),
            }
        }));
        self.hd = Some(hd);
    }

    /// The HD screen rows that changed since the last call, as texels to
    /// resolve with the palette (see `gfx::compose::Compositor::present_texels`).
    /// Header (8 bytes): k, 0, first row (u16), end row (u16), pixels
    /// falling back to low resolution (u16); then the rows. Empty when HD is
    /// off; header only when nothing changed.
    #[wasm_bindgen(js_name = hdFrame)]
    pub fn hd_frame(&mut self) -> Vec<u8> {
        let Some(hd) = &self.hd else { return Vec::new() };
        let mut hd = hd.borrow_mut();
        if !hd.visible {
            return Vec::new();
        }
        let Hd { follower, cur, screen, fallback, full, .. } = &mut *hd;
        let k = follower.comp.k;
        let now = &self.m.hw.mem[0xa0000..0xa0000 + 64000];
        // Rows to present: shadow changes, screen changes, and their
        // neighbours (a pixel's HD depends on its neighbours).
        let mut touched = follower.comp.take_screen_dirty();
        for (y, t) in touched.iter_mut().enumerate() {
            *t |= *full || now[y * 320..(y + 1) * 320] != screen[y * 320..(y + 1) * 320];
        }
        *full = false;
        let rows: Vec<bool> = (0..200).map(|y| touched[y.max(1) - 1] || touched[y] || touched[(y + 1).min(199)]).collect();
        let (Some(first), Some(last)) = (rows.iter().position(|&r| r), rows.iter().rposition(|&r| r)) else {
            let mut out = vec![k as u8, 0, 0, 0, 0, 0];
            out.extend_from_slice(&(fallback.iter().map(|&n| n as u32).sum::<u32>().min(65535) as u16).to_le_bytes());
            return out;
        };
        follower.comp.present_texels(now, cur, &rows, fallback);
        screen.copy_from_slice(now);
        let row = 320 * k * 4;
        let (y0, y1) = (first * k, (last + 1) * k);
        let mut out = Vec::with_capacity(8 + (y1 - y0) * row);
        out.extend_from_slice(&[k as u8, 0]);
        out.extend_from_slice(&(y0 as u16).to_le_bytes());
        out.extend_from_slice(&(y1 as u16).to_le_bytes());
        out.extend_from_slice(&(fallback.iter().map(|&n| n as u32).sum::<u32>().min(65535) as u16).to_le_bytes());
        out.extend_from_slice(&cur[y0 * row..y1 * row]);
        out
    }


    /// Host volume for FM music and digitized voices (1.0 = original).
    #[wasm_bindgen(js_name = setVolume)]
    pub fn set_volume(&mut self, music: f32, voice: f32) {
        self.m.hw.audio.music_gain = music;
        self.m.hw.audio.voice_gain = voice;
    }

    /// Battery saver: skip ahead while the game busy-waits.
    #[wasm_bindgen(js_name = setBatterySaver)]
    pub fn set_battery_saver(&mut self, on: bool) {
        self.m.idle_skip = on;
    }

    /// Songs (bit n = song n, see `pc::SONG_NAMES`) the page plays from a
    /// replacement recording; their FM rendition is muted while they play.
    #[wasm_bindgen(js_name = setReplacedSongs)]
    pub fn set_replaced_songs(&mut self, mask: u16) {
        self.m.replaced_songs = mask;
        let cur = self.m.music.current;
        self.m.hw.audio.opl_muted = cur != 0 && mask & (1 << cur) != 0;
    }

    /// Music events since the last call, flattened as
    /// [time_ms, kind, a, b] per event: kind 1 play(a = song), 2 stop,
    /// 3 resume, 4 fade(a = ticks, b = volume).
    #[wasm_bindgen(js_name = takeMusicEvents)]
    pub fn take_music_events(&mut self) -> Vec<u32> {
        use pc::music::MusicEvent::*;
        let mut out = Vec::new();
        while let Some((at, e)) = self.m.music.events.pop_front() {
            let (kind, a, b) = match e {
                Play { song } => (1, song as u32, 0),
                Stop => (2, 0, 0),
                Resume => (3, 0, 0),
                Fade { ticks, volume } => (4, ticks as u32, volume as u32),
            };
            out.extend_from_slice(&[(at / 1_000_000) as u32, kind, a, b]);
        }
        out
    }

    /// [song, driver status, voice playing (0/1), ms since FM music was audible].
    #[wasm_bindgen(js_name = musicState)]
    pub fn music_state(&self) -> Vec<u32> {
        let s = self.m.music_state();
        vec![s.song as u32, s.status as u32, s.voice as u32, s.quiet_ms]
    }

    pub fn key(&mut self, scancode: u8, pressed: bool) {
        self.m.key(scancode, pressed);
    }

    pub fn mouse(&mut self, x: i32, y: i32, buttons: u16) {
        self.m.mouse_input(x, y, buttons);
    }

    /// Diagnostics log lines since the last call.
    #[wasm_bindgen(js_name = takeLog)]
    pub fn take_log(&mut self) -> String {
        let s = self.m.log.iter().cloned().collect::<Vec<_>>().join("\n");
        self.m.log.clear();
        s
    }

    pub fn instructions(&self) -> f64 {
        self.m.cpu.instructions as f64
    }

    #[wasm_bindgen(js_name = virtualSeconds)]
    pub fn virtual_seconds(&self) -> f64 {
        self.m.now_ns() as f64 / 1e9
    }
}
