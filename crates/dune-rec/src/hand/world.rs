//! The game's state with no machine: the native mode's world (Stage C of
//! `docs/readable-port-plan.md`).
//!
//! A [`World`] is what a save keeps (the blocks of
//! [`SaveImage`](super::save_file::SaveImage): the story block DS:0-1260h,
//! the dialogue table, the book, the map's cells' states) and the three
//! random generators' seeds, which the save doesn't keep. The native rules
//! (`hand/native_day.rs`, ...) run on it directly: no registers, no
//! instruction counts, no 8086 frame; the typed records are read and
//! written through their views' byte layouts ([`World::get`],
//! [`World::set`]).
//!
//! The check (`NATIVE_CHECK=1`): where the original is about to run a
//! piece the native rules cover, the world is taken from the machine and
//! the rules run on it; when the original's piece returns, the machine's
//! world must equal the rules' ([`World::differences`]). The runner
//! prints `native: ...` with the counts.

use super::record_table::{Single, Which};
use super::save_file::SaveImage;
use super::globals as g;
use pc::Machine;

/// A location record's fields.
pub mod place {
    pub const SIZE: u16 = 0x1c;
    pub const FIRST: u16 = 0x100;
    pub const NAME_FIRST: u16 = 0;
    pub const KIND: u16 = 8;
    pub const FIRST_TROOP: u16 = 9;
    pub const FLAGS: u16 = 0x0a;
    pub const RADIUS: u16 = 0x0b;
    pub const SPICE: u16 = 0x12;
    pub const EQUIPMENT: u16 = 0x14;
}

/// A troop record's fields.
pub mod troop {
    pub const SIZE: u16 = 0x1b;
    pub const FIRST: u16 = 0x8aa;
    pub const ID: u16 = 0;
    pub const NEXT: u16 = 1;
    pub const STATE: u16 = 3;
    pub const LOCATION: u16 = 4;
    pub const CHANGED_AT: u16 = 0x0a;
    pub const RATE: u16 = 0x0c;
    pub const WORD_0E: u16 = 0x0e;
    pub const BYTE_10: u16 = 0x10;
    pub const WORD_10: u16 = 0x10;
    pub const WORD_12: u16 = 0x12;
    pub const BYTE_12: u16 = 0x12;
    pub const DAY: u16 = 0x14;
    pub const MOTIVATION: u16 = 0x15;
    pub const SKILLS: u16 = 0x16;
    pub const EQUIPMENT: u16 = 0x19;
    pub const SIZE_FIELD: u16 = 0x1a;
}

/// The planet's growth seed in the code segment (`loc_065b6`'s word).
pub const GROWTH_SEED: u16 = 0x65b4;

/// The story block's end: a walk past it is beyond what the rules model.
pub const STORY_END: u16 = 0x1261;

/// The state the native rules run on.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct World {
    pub save: SaveImage,
    /// [`g::RAND_SEED`] (D826h): `rand`'s.
    pub rand_seed: u16,
    /// [`g::RAND_MASKED_SEED`] (D824h): `rand_masked`'s.
    pub rand_masked_seed: u16,
    /// [`g::RAND_BELOW_SEED`] (D828h): `rand_below`'s.
    pub rand_below_seed: u16,
    /// The bytes of [`EXTRAS`], in order: the time step's state outside
    /// the story block (not saved).
    pub extras: Vec<u8>,
    /// The planet's growth seed, a word of the code (CS:65B4h, not saved).
    pub growth_seed: u16,
}

/// The memory outside the story block the rules read and write, by
/// offset and length: the day changed, the map's latitude table and its
/// rows' base (tables of the game's), a place's spare equipment, the
/// steps' scales by row (a table of the game's), the spice's tenths, the
/// tick's recheck and location, the troop selected on the map, the troop
/// shown.
pub const EXTRAS: [(u16, u16); 10] = [
    (g::DAY_CHANGED, 1),
    (g::LATITUDE_TABLE, 0x2f0),
    (0xdcfe, 2),
    (g::LOCATION_SPARE_EQUIPMENT, 7),
    (0x4880, 0x300),
    (0x46e1, 2),
    (g::TROOPS_TICK_RECHECK, 1),
    (g::TICK_LOCATION, 2),
    (0x46ef, 2),
    (g::SHOWN_TROOP, 1),
];

/// Where `off` is kept in [`World::extras`].
fn extra(off: u16) -> usize {
    let mut k = 0;
    for (at, len) in EXTRAS {
        if off >= at && off < at + len {
            return k + (off - at) as usize;
        }
        k += len as usize;
    }
    panic!("the world has no byte DS:{off:04x}")
}

fn ds(off: u16) -> u32 {
    ((g::GAME_DS as u32) << 4) + off as u32
}

fn peek_word(m: &Machine, off: u16) -> u16 {
    m.hw.peek(ds(off)) as u16 | (m.hw.peek(ds(off) + 1) as u16) << 8
}

impl World {
    /// The machine's world (the owners' bytes, as the bus serves them).
    pub fn from_machine(m: &Machine) -> World {
        World {
            save: SaveImage::from_machine(m),
            rand_seed: peek_word(m, g::RAND_SEED),
            rand_masked_seed: peek_word(m, g::RAND_MASKED_SEED),
            rand_below_seed: peek_word(m, g::RAND_BELOW_SEED),
            extras: EXTRAS.iter().flat_map(|&(at, len)| (0..len).map(move |k| m.hw.peek(ds(at + k)))).collect(),
            growth_seed: {
                let cs = ((pc::LOAD_SEG as u32) << 4) + GROWTH_SEED as u32;
                m.hw.peek(cs) as u16 | (m.hw.peek(cs + 1) as u16) << 8
            },
        }
    }

    /// Byte `off` of the story block (DS:0-1260h) or of [`EXTRAS`].
    pub fn byte(&self, off: u16) -> u8 {
        match self.save.story.get(off as usize) {
            Some(&v) => v,
            None => self.extras[extra(off)],
        }
    }
    pub fn set_byte(&mut self, off: u16, v: u8) {
        match self.save.story.get_mut(off as usize) {
            Some(b) => *b = v,
            None => self.extras[extra(off)] = v,
        }
    }
    /// Word `off` of the story block.
    pub fn word(&self, off: u16) -> u16 {
        self.byte(off) as u16 | (self.byte(off + 1) as u16) << 8
    }
    pub fn set_word(&mut self, off: u16, v: u16) {
        self.set_byte(off, v as u8);
        self.set_byte(off + 1, (v >> 8) as u8);
    }

    /// Map cell `o`'s state as the map's byte has it (bits 4..5: 0, 10h,
    /// 20h, 30h), from the packed states; 0 past the map.
    pub fn cell(&self, o: u16) -> u8 {
        let (q, k) = (o as usize / 4, o as usize % 4);
        self.save.map.get(q).map_or(0, |b| (b >> (2 * (3 - k)) & 3) << 4)
    }
    /// Map cell `o`'s state set (bits 4..5 of `v`).
    pub fn set_cell(&mut self, o: u16, v: u8) {
        let (q, k) = (o as usize / 4, o as usize % 4);
        if let Some(b) = self.save.map.get_mut(q) {
            let sh = 2 * (3 - k);
            *b = *b & !(3 << sh) | ((v >> 4) & 3) << sh;
        }
    }

    /// The game time ([2]).
    pub fn time(&self) -> u16 {
        self.word(2)
    }

    /// Record `i` of `T`'s table.
    pub(super) fn nth<T: Single>(&self, i: usize) -> T {
        let w = T::WHICH;
        let at = w.base() as usize + i * w.size() as usize;
        T::from_bytes(&self.save.story[at..at + w.size() as usize])
    }
    pub(super) fn set_nth<T: Single>(&mut self, i: usize, t: T) {
        let w = T::WHICH;
        let at = w.base() as usize + i * w.size() as usize;
        let b = t.to_bytes();
        self.save.story[at..at + w.size() as usize].copy_from_slice(&b[..w.size() as usize]);
    }
    /// The one record of `T`'s table.
    pub(super) fn get<T: Single>(&self) -> T {
        self.nth(0)
    }
    pub(super) fn update<T: Single>(&mut self, f: impl FnOnce(&mut T)) {
        let mut t = self.get();
        f(&mut t);
        self.set_nth(0, t);
    }

    /// `rand` (E3CCh): the next random word.
    pub fn rand(&mut self) -> u16 {
        let p = self.rand_seed as u32 * 0xcbd1;
        self.rand_seed = (p as u16).wrapping_add(1);
        ((p >> 16) as u16 & 0xff) << 8 | self.rand_seed >> 8
    }

    /// `rand_below` (E3DFh): a random number 0..=`max`.
    pub fn rand_below(&mut self, max: u16) -> u16 {
        if max == 0 {
            return 0;
        }
        let bits = 16 - max.leading_zeros();
        let mask = (0xffffu32 << bits) as u16 ^ 0xffff;
        loop {
            let p = self.rand_below_seed as u32 * 0xcbd1;
            self.rand_below_seed = (p as u16).wrapping_add(1);
            let v = (((p >> 16) as u16 & 0xff) << 8 | self.rand_below_seed >> 8) & mask;
            if v <= max {
                return v;
            }
        }
    }

    /// Where `self` and `other` differ, by table and field offset (at most
    /// `most` of them).
    pub fn differences(&self, other: &World, most: usize) -> Vec<String> {
        let mut out = Vec::new();
        let mut add = |s: String| {
            if out.len() < most {
                out.push(s);
            }
        };
        if self.save.day != other.save.day {
            add(format!("day {:04x} vs {:04x}", self.save.day, other.save.day));
        }
        for (name, a, b) in [
            ("rand seed", self.rand_seed, other.rand_seed),
            ("rand_masked seed", self.rand_masked_seed, other.rand_masked_seed),
            ("rand_below seed", self.rand_below_seed, other.rand_below_seed),
            ("growth seed", self.growth_seed, other.growth_seed),
        ] {
            if a != b {
                add(format!("{name} {a:04x} vs {b:04x}"));
            }
        }
        for (k, (a, b)) in self.save.story.iter().zip(&other.save.story).enumerate() {
            if a != b {
                let at = k as u16;
                let wh = Which::ALL.iter().find(|w| at >= w.base() && at < w.end());
                let var = super::variables::VARS.iter().find(|v| at >= v.at && at < v.at + v.len);
                let place = match (wh, var) {
                    (Some(w), _) => format!("{} #{} +{:x}h", w.name(), (at - w.base()) / w.size(), (at - w.base()) % w.size()),
                    (None, Some(v)) => format!("{} +{:x}h", v.name, at - v.at),
                    (None, None) => format!("DS:{at:04x}"),
                };
                add(format!("{place} (DS:{at:04x}) {a:02x} vs {b:02x}"));
            }
        }
        for (k, (a, b)) in self.extras.iter().zip(&other.extras).enumerate() {
            if a != b {
                let mut at = 0;
                let mut left = k as u16;
                for (o, len) in EXTRAS {
                    if left < len {
                        at = o + left;
                        break;
                    }
                    left -= len;
                }
                add(format!("DS:{at:04x} {a:02x} vs {b:02x}"));
            }
        }
        for (name, x, y) in [("map", &self.save.map, &other.save.map), ("book", &self.save.book, &other.save.book), ("dialogue", &self.save.dialogue, &other.save.dialogue)] {
            let n = x.iter().zip(y.iter()).filter(|(a, b)| a != b).count();
            if n != 0 {
                add(format!("{name}: {n} bytes"));
            }
        }
        out
    }
}

/// The native check's counts, kept with the machine.
#[derive(Default)]
pub struct NativeChecks {
    /// Per piece: (checked, equal, not checked: the original's piece gave
    /// up to the interpreter, or the rules can't run it).
    pub pieces: Vec<(&'static str, u64, u64, u64)>,
    /// Per piece and reason, the cases the rules don't model.
    pub why: Vec<((&'static str, &'static str), u64)>,
    /// The worlds the rules made, waiting for the original's to compare
    /// (a stack: a checked piece may call another).
    pub pending: Vec<Option<(&'static str, World)>>,
}

/// The registers a piece is entered with (its arguments).
#[derive(Clone, Copy, Debug)]
pub struct Regs {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub si: u16,
    pub di: u16,
    pub bp: u16,
}

impl Regs {
    pub fn of(m: &Machine) -> Regs {
        use pc::cpu::{AX, BP, BX, CX, DI, DX, SI};
        let r = &m.cpu.regs;
        Regs { ax: r[AX], bx: r[BX], cx: r[CX], dx: r[DX], si: r[SI], di: r[DI], bp: r[BP] }
    }
}

/// The port `frame` entered at `start`: with `NATIVE_CHECK`, entered at
/// its `entry`, the native `rules` run on the world before (with the
/// registers it is entered with) and must give the world it returns
/// with.
pub fn checked(
    m: &mut Machine,
    start: u16,
    entry: u16,
    piece: &'static str,
    rules: impl FnOnce(&mut World, Regs) -> Option<()>,
    frame: impl FnOnce(&mut Machine, u16) -> Result<(), super::Bail>,
) -> Result<(), super::Bail> {
    if start != entry || !checking() {
        return frame(m, start);
    }
    let regs = Regs::of(m);
    before(m, piece, |w| rules(w, regs));
    let r = frame(m, start);
    after(m, r.is_ok());
    r
}

thread_local! {
    static WHY: std::cell::Cell<&'static str> = const { std::cell::Cell::new("") };
}

/// A case the native rules don't model (yet): `None`, and why, for the
/// counts.
pub fn nm<T>(why: &'static str) -> Option<T> {
    WHY.with(|w| w.set(why));
    None
}

/// Whether `NATIVE_CHECK` is on.
pub fn checking() -> bool {
    use std::sync::OnceLock;
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var("NATIVE_CHECK").is_ok_and(|v| v != "0"))
}

fn count(m: &mut Machine, piece: &'static str, what: usize) {
    let n = &mut super::native::native(m).native_checks;
    let k = match n.pieces.iter().position(|p| p.0 == piece) {
        Some(k) => k,
        None => {
            n.pieces.push((piece, 0, 0, 0));
            n.pieces.len() - 1
        }
    };
    let p = &mut n.pieces[k];
    match what {
        0 => p.1 += 1,
        1 => p.2 += 1,
        _ => p.3 += 1,
    }
}

/// Before the original runs `piece`: the rules run on the machine's
/// world (`None`: they can't), kept to compare at [`after`].
pub fn before(m: &mut Machine, piece: &'static str, rules: impl FnOnce(&mut World) -> Option<()>) {
    let mut w = World::from_machine(m);
    WHY.with(|c| c.set(""));
    let ran = rules(&mut w).is_some();
    if !ran {
        let why = WHY.with(|c| c.get());
        let why = if why.is_empty() { "?" } else { why };
        let n = &mut super::native::native(m).native_checks.why;
        match n.iter_mut().find(|e| e.0 == (piece, why)) {
            Some(e) => e.1 += 1,
            None => n.push(((piece, why), 1)),
        }
    }
    super::native::native(m).native_checks.pending.push(ran.then_some((piece, w)));
    if !ran {
        count(m, piece, 2);
    }
}

/// After the original's `piece` returned (`done`: whole, not given up
/// to the interpreter): its world compared with the rules'.
pub fn after(m: &mut Machine, done: bool) {
    let Some(Some((piece, want))) = super::native::native(m).native_checks.pending.pop() else {
        return;
    };
    if !done {
        count(m, piece, 2);
        return;
    }
    let got = World::from_machine(m);
    count(m, piece, 0);
    let d = want.differences(&got, 8);
    if d.is_empty() {
        count(m, piece, 1);
    } else if std::env::var("NATIVE_TRACE").is_ok() || super::native::native(m).native_checks.pieces.iter().find(|p| p.0 == piece).is_some_and(|p| p.1 - p.2 <= 3) {
        eprintln!("native {piece} at time {:04x}: the rules' world vs the original's: {}", got.time(), d.join("; "));
    }
}

/// The cases not modelled, by piece and reason.
pub fn reasons(m: &mut Machine) -> Vec<((&'static str, &'static str), u64)> {
    super::native::native(m).native_checks.why.clone()
}

/// The counts, for the runner's report: (piece, checked, equal, not
/// checked).
pub fn counts(m: &mut Machine) -> Vec<(&'static str, u64, u64, u64)> {
    super::native::native(m).native_checks.pieces.clone()
}
