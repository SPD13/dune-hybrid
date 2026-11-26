//! The save file from and into the Rust state (Stage B of
//! `docs/readable-port-plan.md`).
//!
//! A save (`DUNE37Sn.SAV`) is the game time (the day word [2]), then the
//! image packed (`compress_sav`). The image (567Ah bytes,
//! `create_save_in_memory`) is four blocks, each now held by a Rust
//! owner:
//! - the map's cells' states, four to a byte, the first cell in the top
//!   bits (317Fh: [`super::map_cells`]);
//! - the book, CS:00AAh (A2h: [`super::book`]);
//! - the dialogue table, DS:AA76h (11F8h: [`super::dialogue_table`]);
//! - the story block, DS:0-1260h (1261h: the record tables, the clock,
//!   the events, the loose variables).
//!
//! The packing: a run of equal bytes (up to FFh) as F7h, count, byte; a
//! run of one or two as the bytes themselves (an F7h byte always as a
//! run); after a 4-byte header: F7h, the last run's count, then the size
//! of the header and the packed bytes.
//!
//! [`SaveImage`] is the typed image: made from the machine
//! ([`SaveImage::from_machine`], the blocks read on the bus, so from their
//! owners), written as the file ([`SaveImage::to_file`]), read from one
//! ([`SaveImage::from_file`]) and laid into the machine
//! ([`SaveImage::apply`]). The game's own save and load still run; each
//! file the game writes is checked against the one made here, each one it
//! reads must pack back to itself, and after each restore by the port
//! the image applied must change nothing ([`check_restored`]).

use super::globals as g;
use pc::Machine;

/// The image's blocks: their sizes.
pub const MAP: usize = super::map_cells::CELLS / 4;
pub const BOOK: usize = super::book::LEN;
pub const DIALOGUE: usize = super::dialogue_table::LEN;
pub const STORY: usize = 0x1261;
/// The image's size.
pub const IMAGE: usize = MAP + BOOK + DIALOGUE + STORY;
const _: () = assert!(IMAGE == 0x567a);

/// The packing's escape byte.
pub const ESCAPE: u8 = 0xf7;

/// A save's state, by block.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SaveImage {
    /// The game time ([2]).
    pub day: u16,
    /// The map's cells' states, packed four to a byte.
    pub map: Vec<u8>,
    pub book: Vec<u8>,
    pub dialogue: Vec<u8>,
    pub story: Vec<u8>,
}

fn ds(off: u16) -> u32 {
    ((g::GAME_DS as u32) << 4) + off as u32
}

fn map_segment(m: &Machine) -> u16 {
    m.hw.peek(ds(g::MAP_GFX_SEGMENT)) as u16 | (m.hw.peek(ds(g::MAP_GFX_SEGMENT) + 1) as u16) << 8
}

impl SaveImage {
    /// The state as the bus serves it (the owners' bytes where owned).
    pub fn from_machine(m: &Machine) -> SaveImage {
        let peek = |a: u32| m.hw.peek(a);
        let map = match super::map_cells::get(m) {
            Some(mc) => mc.packed(),
            None => {
                let base = (map_segment(m) as u32) << 4;
                (0..MAP as u32)
                    .map(|q| (0..4).fold(0u8, |b, k| b << 2 | (super::map_cells::kind(peek(base + 4 * q + k)) >> 4)))
                    .collect()
            }
        };
        let cs = (pc::LOAD_SEG as u32) << 4;
        SaveImage {
            day: peek(ds(2)) as u16 | (peek(ds(3)) as u16) << 8,
            map,
            book: (0..BOOK as u32).map(|k| peek(cs + super::book::AT as u32 + k)).collect(),
            dialogue: (0..DIALOGUE as u32).map(|k| peek(ds(super::dialogue_table::AT) + k)).collect(),
            story: (0..STORY as u32).map(|k| peek(ds(0) + k)).collect(),
        }
    }

    /// The image, as `create_save_in_memory` lays it out.
    pub fn image(&self) -> Vec<u8> {
        [&self.map[..], &self.book, &self.dialogue, &self.story].concat()
    }

    /// From an image and the day.
    pub fn from_image(day: u16, img: &[u8]) -> Option<SaveImage> {
        if img.len() < IMAGE {
            return None;
        }
        let (map, rest) = img.split_at(MAP);
        let (book, rest) = rest.split_at(BOOK);
        let (dialogue, rest) = rest.split_at(DIALOGUE);
        Some(SaveImage { day, map: map.to_vec(), book: book.to_vec(), dialogue: dialogue.to_vec(), story: rest[..STORY].to_vec() })
    }

    /// The save file.
    pub fn to_file(&self) -> Vec<u8> {
        let (packed, last) = pack(&self.image());
        let mut f = Vec::with_capacity(6 + packed.len());
        f.extend_from_slice(&self.day.to_le_bytes());
        f.extend_from_slice(&[ESCAPE, last]);
        f.extend_from_slice(&(4 + packed.len() as u16).to_le_bytes());
        f.extend_from_slice(&packed);
        f
    }

    /// From a save file (None if it is not one).
    pub fn from_file(f: &[u8]) -> Option<SaveImage> {
        if f.len() < 6 {
            return None;
        }
        let day = u16::from_le_bytes([f[0], f[1]]);
        let size = u16::from_le_bytes([f[4], f[5]]) as usize;
        if size < 4 || f.len() < 2 + size {
            return None;
        }
        let img = unpack(f[2], &f[6..2 + size]);
        SaveImage::from_image(day, &img)
    }

    /// Laid into the machine as `restore_from_save_memory` does: the
    /// map's states into bits 4..5 of the cells, the other blocks whole,
    /// written on the bus (to their owners); with `dry`, only counted.
    /// Returns the bytes that change. (The day is in the story block.)
    pub fn apply(&self, m: &mut Machine, dry: bool) -> usize {
        let mut changed = 0;
        let mut put = |m: &mut Machine, a: u32, v: u8| {
            if m.hw.peek(a) != v {
                changed += 1;
                if !dry {
                    m.hw.poke(a, v);
                }
            }
        };
        let base = (map_segment(m) as u32) << 4;
        for (q, &b) in self.map.iter().enumerate() {
            for k in 0..4 {
                let a = base + 4 * q as u32 + k;
                let kind = (b >> (6 - 2 * k)) & 3;
                let v = (m.hw.peek(a) & !super::map_cells::KIND) | kind << 4;
                put(m, a, v);
            }
        }
        let cs = (pc::LOAD_SEG as u32) << 4;
        for (k, &v) in self.book.iter().enumerate() {
            put(m, cs + super::book::AT as u32 + k as u32, v);
        }
        for (k, &v) in self.dialogue.iter().enumerate() {
            put(m, ds(super::dialogue_table::AT) + k as u32, v);
        }
        for (k, &v) in self.story.iter().enumerate() {
            put(m, ds(0) + k as u32, v);
        }
        changed
    }
}

/// `img` packed as `compress_sav` packs it, and the last run's count.
pub fn pack(img: &[u8]) -> (Vec<u8>, u8) {
    let mut out = Vec::with_capacity(img.len());
    let mut i = 0;
    let mut last = 0u8;
    while i < img.len() {
        let b = img[i];
        let mut n = 1usize;
        while i + n < img.len() && img[i + n] == b && n < 0xff {
            n += 1;
        }
        last = n as u8;
        if b == ESCAPE || n > 2 {
            out.extend_from_slice(&[ESCAPE, n as u8, b]);
        } else {
            out.extend(std::iter::repeat_n(b, n));
        }
        i += n;
    }
    (out, last)
}

/// The packed bytes `p` (without the header) unpacked, `esc` the escape.
pub fn unpack(esc: u8, p: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(IMAGE);
    let mut i = 0;
    while i < p.len() {
        if p[i] == esc && i + 2 < p.len() {
            out.extend(std::iter::repeat_n(p[i + 2], p[i + 1] as usize));
            i += 3;
        } else {
            out.push(p[i]);
            i += 1;
        }
    }
    out
}

/// The checks' counts, kept with the machine ([`super::native`]).
#[derive(Default)]
pub struct SaveFileState {
    /// Save files the game wrote, and those that differed from the one
    /// made from the Rust state.
    pub written: u64,
    pub written_differing: u64,
    /// A slot's day read (the load menu), not checked.
    pub peeked: u64,
    /// Save files the game read whole, and those that did not pack back
    /// to themselves.
    pub read: u64,
    pub read_differing: u64,
    /// Restores by the port, and the bytes the image applied after them
    /// would have changed.
    pub restored: u64,
    pub restored_differing: u64,
    /// Restores whose image changed while they ran (not checked).
    pub restored_moving: u64,
}

fn state(m: &mut Machine) -> &mut SaveFileState {
    &mut super::native::native(m).save_file
}

/// The counts.
pub fn counts(m: &mut Machine) -> [u64; 7] {
    let s = state(m);
    [s.written, s.written_differing, s.read, s.read_differing, s.restored, s.restored_differing, s.restored_moving]
}

fn is_save(name: &str) -> bool {
    let n = name.rsplit(['/', '\\']).next().unwrap_or(name).to_ascii_uppercase();
    n.starts_with("DUNE37S") && n.ends_with(".SAV")
}

/// A file written or read by the game (`pc::Machine::file_hook`): a save
/// checked.
pub fn on_file(m: &mut Machine, name: &str, write: bool, data: &[u8]) {
    if !is_save(name) {
        return;
    }
    if write {
        let made = SaveImage::from_machine(m).to_file();
        let differs = made != data;
        if differs && std::env::var_os("SAVE_TRACE").is_some() {
            let at = made.iter().zip(data).position(|(a, b)| a != b);
            eprintln!("save file {name}: {} bytes written, {} made from the Rust state, first difference at {at:?}", data.len(), made.len());
        }
        let s = state(m);
        s.written += 1;
        s.written_differing += differs as u64;
    } else if data.len() < 6 {
        // The load menu reads a slot's day word to show it.
        state(m).peeked += 1;
    } else {
        let back = SaveImage::from_file(data).map(|s| s.to_file());
        if back.as_deref() != Some(data) && std::env::var_os("SAVE_TRACE").is_some() {
            let at = back.as_ref().and_then(|b| b.iter().zip(data).position(|(a, b)| a != b));
            eprintln!("save file {name}: {} bytes read, {:?} packed back, first difference at {at:?}", data.len(), back.as_ref().map(|b| b.len()));
        }
        let s = state(m);
        s.read += 1;
        s.read_differing += (back.as_deref() != Some(data)) as u64;
    }
}

/// The image at `seg:at` (where `restore_from_save_memory` takes it).
pub(super) fn image_at(m: &Machine, seg: u16, at: u16) -> Vec<u8> {
    let base = ((seg as u32) << 4) + at as u32;
    (0..IMAGE as u32).map(|k| m.hw.peek((base + k) & 0xf_ffff)).collect()
}

/// After `restore_from_save_memory` restored the image at `seg:at`
/// (`before`: as it was when it started): the image applied again must
/// change nothing. Unless the image itself changed meanwhile (the
/// harness's calls from any state, with the source over the stack): then
/// not checked.
pub(super) fn check_restored(m: &mut Machine, seg: u16, at: u16, before: &[u8]) {
    let img = image_at(m, seg, at);
    if img != before {
        state(m).restored_moving += 1;
        return;
    }
    let Some(s) = SaveImage::from_image(0, &img) else { return };
    let changed = s.apply(m, true);
    if changed > 0 && std::env::var_os("SAVE_TRACE").is_some() {
        eprintln!("restore from {seg:04x}:{at:04x}: the Rust load would change {changed} bytes");
    }
    let st = state(m);
    st.restored += 1;
    st.restored_differing += changed as u64;
}
