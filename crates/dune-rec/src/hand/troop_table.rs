//! The troops as Rust state (Stage B of `docs/readable-port-plan.md`), the
//! clock's pattern (`hand/clock.rs`) for the troop table.
//!
//! Each record of the table at [`g::TROOPS`] is a [`TroopRecord`] kept
//! with the machine ([`super::native`]), read from the memory the first
//! time it is used. The [`super::game::Troop`] view reads and writes
//! through it; every change is written through to the memory (the
//! mirror: the suite compares it, the save image copies it, and code that
//! doesn't use the view yet reads it), and every access first compares
//! the record with its 27 bytes there. A difference is a write the view
//! didn't make: the record takes the memory's bytes. A port's writes
//! that don't use the view yet are followed (`Ctx`'s writes tell the
//! table, [`follow`]); a save loaded and the tools' own stepping are told
//! ([`outside`]). What is left is the generated code finishing a port
//! that gave up (at a stop for an interrupt; it is the original's code,
//! in Rust): counted apart ([`after_bails`]), as the record was verified
//! before the bail. Anything else is unforeseen ([`syncs`]; the suite
//! wants none). `TROOPS_TRACE=1` prints each with the offsets that
//! differed.

use super::{Ctx, globals as g};
use pc::{Machine, cpu::Bus};

/// The records in the table.
pub const COUNT: usize = ((g::TROOPS_END - g::TROOPS) / SIZE) as usize;
const SIZE: u16 = 0x1b;

/// A troop's record: 1Bh bytes (see [`super::game::Troop`] for what the
/// ports found each field doing).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct TroopRecord {
    /// Byte 0: its number.
    pub id: u8,
    /// Byte 1: the next troop in its location's chain (0: the last).
    pub next: u8,
    /// Byte 2: not known yet.
    pub byte_02: u8,
    /// Byte 3: the job (low four bits) and the state bits
    /// ([`super::game::troop_state`]).
    pub state: u8,
    /// Word 4: its location (a location record's offset).
    pub location: u16,
    /// Words 6, 8: its own map position while travelling.
    pub x: u16,
    pub y: u16,
    /// Word 0Ah: the game time it last changed.
    pub changed_at: u16,
    /// Word 0Ch: its production rate.
    pub rate: u16,
    /// Word 0Eh: cleared with the rate when stamped.
    pub word_0e: u16,
    /// Words 10h, 12h: bits, not all known.
    pub word_10: u16,
    pub word_12: u16,
    /// Byte 14h: the day it joined or was last given a job.
    pub day: u8,
    /// Byte 15h: its motivation (0 to 100).
    pub motivation: u8,
    /// Bytes 16h to 18h: its skills.
    pub skills: [u8; 3],
    /// Byte 19h: one bit per need of its location it covers.
    pub needs: u8,
    /// Byte 1Ah: its size.
    pub size: u8,
}

impl TroopRecord {
    fn from_bytes(b: &[u8; SIZE as usize]) -> TroopRecord {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        TroopRecord {
            id: b[0],
            next: b[1],
            byte_02: b[2],
            state: b[3],
            location: w(4),
            x: w(6),
            y: w(8),
            changed_at: w(0x0a),
            rate: w(0x0c),
            word_0e: w(0x0e),
            word_10: w(0x10),
            word_12: w(0x12),
            day: b[0x14],
            motivation: b[0x15],
            skills: [b[0x16], b[0x17], b[0x18]],
            needs: b[0x19],
            size: b[0x1a],
        }
    }

    fn to_bytes(&self) -> [u8; SIZE as usize] {
        let mut b = [0u8; SIZE as usize];
        let mut w = |i: usize, v: u16| {
            b[i] = v as u8;
            b[i + 1] = (v >> 8) as u8;
        };
        w(4, self.location);
        w(6, self.x);
        w(8, self.y);
        w(0x0a, self.changed_at);
        w(0x0c, self.rate);
        w(0x0e, self.word_0e);
        w(0x10, self.word_10);
        w(0x12, self.word_12);
        b[0] = self.id;
        b[1] = self.next;
        b[2] = self.byte_02;
        b[3] = self.state;
        b[0x14] = self.day;
        b[0x15] = self.motivation;
        b[0x16..0x19].copy_from_slice(&self.skills);
        b[0x19] = self.needs;
        b[0x1a] = self.size;
        b
    }
}

/// The troops' part of the Rust state kept with a machine.
#[derive(Default)]
pub struct TroopTable {
    /// By record; none until first used (or after a boundary).
    records: Vec<Option<TroopRecord>>,
    /// By record: the count of bails (`Rec::bails`) when it last matched.
    verified_at: Vec<u64>,
    /// Unforeseen writes (see the module's notes).
    syncs: u64,
    /// Writes of the generated code after a port gave up.
    after_bails: u64,
}

fn table(m: &mut Machine) -> &mut TroopTable {
    let t = &mut super::native::native(m).troops;
    if t.records.is_empty() {
        t.records = vec![None; COUNT];
        t.verified_at = vec![0; COUNT];
    }
    t
}

/// How often a record took the memory's bytes written by the generated
/// code after a port gave up.
pub fn after_bails(m: &mut Machine) -> u64 {
    table(m).after_bails
}

fn bails(m: &Machine) -> u64 {
    m.rec.as_ref().map_or(0, |r| r.bails)
}

/// How often a record took the memory's bytes, unforeseen.
pub fn syncs(m: &mut Machine) -> u64 {
    table(m).syncs
}

/// The record at `offset` in the data segment, if it starts one.
pub fn index(offset: u16) -> Option<usize> {
    let k = offset.checked_sub(g::TROOPS)?;
    (offset < g::TROOPS_END && k % SIZE == 0).then_some((k / SIZE) as usize)
}

fn record_offset(i: usize) -> u16 {
    g::TROOPS + i as u16 * SIZE
}

/// The record's bytes in the memory.
fn mirror(m: &mut Machine, i: usize) -> [u8; SIZE as usize] {
    let mut b = [0u8; SIZE as usize];
    for (k, v) in b.iter_mut().enumerate() {
        let a = m.cpu.phys(g::GAME_DS, record_offset(i) + k as u16);
        *v = m.hw.read8(a);
    }
    b
}

/// Record `i`, in step with the memory.
pub(super) fn get(c: &mut Ctx, i: usize) -> TroopRecord {
    let bytes = mirror(c.m, i);
    let mem = TroopRecord::from_bytes(&bytes);
    let now = bails(c.m);
    let t = table(c.m);
    let since = t.verified_at[i];
    t.verified_at[i] = now;
    match t.records[i] {
        Some(r) if r == mem => r,
        Some(r) => {
            if now > since {
                t.after_bails += 1;
            } else {
                t.syncs += 1;
            }
            t.records[i] = Some(mem);
            if std::env::var_os("TROOPS_TRACE").is_some() {
                let was = r.to_bytes();
                let at: Vec<String> = (0..SIZE as usize).filter(|&k| was[k] != bytes[k]).map(|k| format!("{k:02x}:{:02x}>{:02x}", was[k], bytes[k])).collect();
                let why = if now > since { "after a bail" } else { "UNFORESEEN" };
                eprintln!("troops sync-in ({why}), troop {} at insn {} ({:04x}): {}", i + 1, c.m.cpu.instructions, c.m.cpu.ip, at.join(" "));
            }
            mem
        }
        None => {
            t.records[i] = Some(mem);
            mem
        }
    }
}

/// Record `i` changed to `r`: kept, and its changed bytes written through.
pub(super) fn set(c: &mut Ctx, i: usize, r: TroopRecord) {
    let old = get(c, i).to_bytes();
    table(c.m).records[i] = Some(r);
    let new = r.to_bytes();
    for k in 0..SIZE as usize {
        if new[k] != old[k] {
            let a = c.m.cpu.phys(g::GAME_DS, record_offset(i) + k as u16);
            c.m.hw.write8(a, new[k]);
        }
    }
}

/// Byte `field` of record `i`.
pub(super) fn byte(c: &mut Ctx, i: usize, field: u16) -> u8 {
    get(c, i).to_bytes()[field as usize]
}

/// Byte `field` of record `i` set.
pub(super) fn set_byte(c: &mut Ctx, i: usize, field: u16, v: u8) {
    let mut b = get(c, i).to_bytes();
    b[field as usize] = v;
    set(c, i, TroopRecord::from_bytes(&b));
}

/// A port wrote records `first..=last` other than through the view: those
/// cached take the memory's bytes (not counted).
pub(super) fn follow(m: &mut Machine, first: usize, last: usize) {
    for i in first..=last.min(COUNT - 1) {
        if table(m).records[i].is_some() {
            let mem = TroopRecord::from_bytes(&mirror(m, i));
            table(m).records[i] = Some(mem);
        }
    }
}

/// The machine ran outside the recompiled code on purpose, or a save was
/// loaded: every record is read from the memory again when next used.
pub fn outside(m: &mut Machine) {
    let t = table(m);
    t.records.iter_mut().for_each(|r| *r = None);
}
