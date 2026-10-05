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

#[wasm_bindgen]
pub struct Emu {
    m: Machine,
    frame: Vec<u8>,
    saves: Rc<RefCell<HashMap<String, Vec<u8>>>>,
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
        Ok(Emu { m, frame: vec![0; 64000 + 768], saves })
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
        self.m.load_state(data).map_err(|e| JsValue::from_str(&e))
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
