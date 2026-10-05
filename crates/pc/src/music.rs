//! Music tracking: notice when the game starts, stops or fades a song, so a
//! host can play a replacement recording in sync.
//!
//! The game calls its music driver through far pointers in its data segment
//! (filled when the driver is loaded, seg000:e57b):
//!
//! | `DS:` | driver entry | used for |
//! |---|---|---|
//! | 3971h | +0103h | play song (ES:SI = song data), from play_music seg000:adb1 and the intro |
//! | 3975h | +0106h | stop (midi_func_2_0, seg000:aebd) |
//! | 3979h | +0109h | (resume/continue, seg000:ad84) |
//! | 397Dh | +010Ch | dynamics: fade to volume BL/BH over AX ticks (seg000:add7, adff) |
//!
//! The machine watches those four entry addresses. Songs are identified by
//! comparing the data at ES:SI with the songs in DUNE.DAT.

use std::collections::VecDeque;

use gfx::dat;

use crate::{GAME_DS, SONG_NAMES, fs::FileSystem};

const VTABLE: [u16; 4] = [0x3971, 0x3975, 0x3979, 0x397d];
const FINGERPRINT: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicEvent {
    /// Song number 1..=10 (see [`SONG_NAMES`]), 0 if unrecognized.
    Play { song: u8 },
    Stop,
    /// Driver entry +0109h.
    Resume,
    /// Fade toward `volume` (BL, 0..~F) over `ticks` 200 Hz ticks.
    Fade { ticks: u16, volume: u8 },
}

#[derive(Default)]
pub struct MusicTracker {
    /// Linear addresses of the four driver entries (0 until the driver is loaded).
    pub(crate) watch: [u32; 4],
    /// First bytes of each song's data, for identification.
    fingerprints: Option<Vec<(u8, Vec<u8>)>>,
    /// Events with the virtual time (ns) they happened at.
    pub events: VecDeque<(u64, MusicEvent)>,
    /// Song currently playing according to the driver calls (0 = none).
    pub current: u8,
}

impl MusicTracker {
    /// Re-read the driver table (cheap; called on device events).
    pub(crate) fn refresh(&mut self, mem: &[u8]) {
        for (i, off) in VTABLE.iter().enumerate() {
            let a = GAME_DS as usize * 16 + *off as usize;
            let o = u16::from_le_bytes([mem[a], mem[a + 1]]) as u32;
            let s = u16::from_le_bytes([mem[a + 2], mem[a + 3]]) as u32;
            self.watch[i] = if s == 0 { 0 } else { ((s << 4) + o) & 0xf_ffff };
        }
    }

    #[inline]
    pub(crate) fn hit(&self, linear: u32) -> Option<usize> {
        if linear == 0 {
            return None;
        }
        self.watch.iter().position(|&w| w == linear)
    }

    fn load_fingerprints(fs: &mut dyn FileSystem) -> Vec<(u8, Vec<u8>)> {
        let mut out = Vec::new();
        let Some(mut f) = fs.open("DUNE.DAT") else { return out };
        let Ok(toc) = dat::toc(&mut f) else { return out };
        for e in &toc {
            let Some(stem) = e.name.strip_suffix(".HSQ") else { continue };
            let Some(idx) = SONG_NAMES.iter().position(|&n| n == stem) else { continue };
            if let Ok(data) = dat::load(&mut f, e) {
                let n = data.len().min(FINGERPRINT);
                out.push((idx as u8 + 1, data[..n].to_vec()));
            }
        }
        out
    }

    /// Which song is at `data` (song bytes in emulated memory)?
    pub(crate) fn identify(&mut self, fs: &mut dyn FileSystem, data: &[u8]) -> u8 {
        let prints = self.fingerprints.get_or_insert_with(|| Self::load_fingerprints(fs));
        prints
            .iter()
            .find(|(_, p)| data.len() >= p.len() && data[..p.len()] == p[..])
            .map(|(id, _)| *id)
            .unwrap_or(0)
    }

    pub(crate) fn push(&mut self, at_ns: u64, e: MusicEvent) {
        match e {
            MusicEvent::Play { song } => self.current = song,
            MusicEvent::Stop => self.current = 0,
            _ => {}
        }
        if self.events.len() < 64 {
            self.events.push_back((at_ns, e));
        }
    }
}
