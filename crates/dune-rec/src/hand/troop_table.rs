//! The troops as Rust state (Stage B of `docs/readable-port-plan.md`), the
//! clock's pattern (`hand/clock.rs`) for the troop table.
//!
//! Each record of the table at [`g::TROOPS`] is a [`TroopRecord`],
//! cached with the machine by the generic record tables
//! (`hand/record_table.rs`, which has the mirror and its checks); the
//! [`super::game::Troop`] view reads and writes through it.

use super::record_table::{self, Which};
use super::{Ctx, globals as g};
use pc::Machine;

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
    fn from_bytes(b: &[u8]) -> TroopRecord {
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

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = record_table::Bytes::default();
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

/// How often a troop record took the memory's bytes, unforeseen.
pub fn syncs(m: &mut Machine) -> u64 {
    record_table::syncs(m, Which::Troops)
}

/// How often a troop record took the memory's bytes the generated code
/// wrote.
pub fn after_bails(m: &mut Machine) -> u64 {
    record_table::after_generated(m, Which::Troops)
}

/// The record at `offset` in the data segment, if it starts one.
pub fn index(offset: u16) -> Option<usize> {
    record_table::index(Which::Troops, offset)
}

/// Record `i`, in step with the memory.
pub(super) fn get(c: &mut Ctx, i: usize) -> TroopRecord {
    TroopRecord::from_bytes(&record_table::get(c, Which::Troops, i))
}

/// Record `i` changed to `r`: kept, and its changed bytes written through.
pub(super) fn set(c: &mut Ctx, i: usize, r: TroopRecord) {
    record_table::set(c, Which::Troops, i, r.to_bytes());
}

/// Byte `field` of record `i`.
pub(super) fn byte(c: &mut Ctx, i: usize, field: u16) -> u8 {
    record_table::byte(c, Which::Troops, i, field)
}

/// Byte `field` of record `i` set.
pub(super) fn set_byte(c: &mut Ctx, i: usize, field: u16, v: u8) {
    record_table::set_byte(c, Which::Troops, i, field, v)
}
