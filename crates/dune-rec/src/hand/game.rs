//! The game's data by name: views over its records in the data segment.
//!
//! The readable ports (stage A of `docs/readable-port-plan.md`) work with
//! these instead of offsets. Every accessor reads or writes exactly the
//! bytes the original does, so a port written with them is still exact
//! and the shadow check still compares it call by call.
//!
//! Names come from what the ports found each field doing; a field whose
//! meaning isn't known yet keeps its offset in its name (`word_10`).

#![allow(dead_code)] // views grow ahead of the ports that use them

use super::{Ctx, globals as g};

/// A troop: a 1Bh-byte record in the table at [`g::TROOPS`], numbered from
/// 1. (Byte 3 bit 80h marks a record that isn't a troop of yours.)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Troop(pub u16);

/// Bits of a troop's byte 3, above its job (the low four bits).
pub mod troop_state {
    /// Set by `mark_troop` (7085h).
    pub const MARKED: u8 = 0x10;
    /// Set when the troop is started (`start_troop`, 668Fh).
    pub const STARTED: u8 = 0x20;
    /// The troop is at its own location (word 4), not one found for it.
    pub const OWN_LOCATION: u8 = 0x40;
    /// Not a troop of yours.
    pub const NOT_A_TROOP: u8 = 0x80;
}

impl Troop {
    pub const SIZE: u16 = 0x1b;

    /// Troop `n` (from 1), as `troop_ptr` (6906h) finds it.
    pub fn number(n: u8) -> Troop {
        Troop(g::TROOPS.wrapping_add((n.wrapping_sub(1) as u16) * Self::SIZE))
    }

    /// Every record of the table, in order.
    pub fn all() -> impl Iterator<Item = Troop> {
        (g::TROOPS..g::TROOPS_END).step_by(Self::SIZE as usize).map(Troop)
    }

    /// Its offset in the data segment (what the original keeps in SI).
    pub fn offset(self) -> u16 {
        self.0
    }

    fn at(self, field: u16) -> u16 {
        self.0.wrapping_add(field)
    }

    /// Byte 3: the job (low four bits) and [`troop_state`] bits.
    pub fn state(self, c: &mut Ctx) -> u8 {
        c.byte(self.at(3))
    }
    pub fn set_state(self, c: &mut Ctx, v: u8) {
        c.set_byte(self.at(3), v)
    }
    /// The job: byte 3's low four bits.
    pub fn job(self, c: &mut Ctx) -> u8 {
        self.state(c) & 0x0f
    }

    /// Word 4: its location (a [`Location`] record's offset).
    pub fn location(self, c: &mut Ctx) -> u16 {
        c.word(self.at(4))
    }
    pub fn set_location(self, c: &mut Ctx, v: u16) {
        c.set_word(self.at(4), v)
    }

    /// Word 10h: its low two bits the step of its last skill gain (the
    /// skill's index + 1); bit 80h, 200h: not known yet.
    pub fn word_10(self, c: &mut Ctx) -> u16 {
        c.word(self.at(0x10))
    }
    pub fn set_word_10(self, c: &mut Ctx, v: u16) {
        c.set_word(self.at(0x10), v)
    }

    /// Word 0Ah: the game time it last changed (`stamp_record`).
    pub fn changed_at(self, c: &mut Ctx) -> u16 {
        c.word(self.at(0x0a))
    }
    pub fn set_changed_at(self, c: &mut Ctx, v: u16) {
        c.set_word(self.at(0x0a), v)
    }

    /// Word 12h: bits, not all known (bits 15..13 select the skills its
    /// activity trains, see `troop_skills_decay`; 400h, 40h, 30h).
    pub fn word_12(self, c: &mut Ctx) -> u16 {
        c.word(self.at(0x12))
    }

    /// Byte 14h: the day it joined or was last given a job.
    pub fn day(self, c: &mut Ctx) -> u8 {
        c.byte(self.at(0x14))
    }
    pub fn set_day(self, c: &mut Ctx, v: u8) {
        c.set_byte(self.at(0x14), v)
    }

    /// Byte 15h: its motivation (0 to 100).
    pub fn motivation(self, c: &mut Ctx) -> u8 {
        c.byte(self.at(0x15))
    }
    pub fn set_motivation(self, c: &mut Ctx, v: u8) {
        c.set_byte(self.at(0x15), v)
    }

    /// Byte 16h + `k`: its skill `k` (0 to 5Fh).
    pub fn skill(self, c: &mut Ctx, k: u16) -> u8 {
        c.byte(self.at(0x16).wrapping_add(k))
    }
    pub fn set_skill(self, c: &mut Ctx, k: u16, v: u8) {
        c.set_byte(self.at(0x16).wrapping_add(k), v)
    }
}

impl Troop {
    /// Byte 19h: bits, one per need of its location it covers.
    pub fn byte_19(self, c: &mut Ctx) -> u8 {
        c.byte(self.at(0x19))
    }
    pub fn set_byte_19(self, c: &mut Ctx, v: u8) {
        c.set_byte(self.at(0x19), v)
    }
}

/// Charisma (byte [29h]).
pub fn charisma(c: &mut Ctx) -> u8 {
    c.byte(g::CHARISMA)
}
pub fn set_charisma(c: &mut Ctx, v: u8) {
    c.set_byte(g::CHARISMA, v)
}

/// Any record stamped with the game time (`stamp_record`, 6B25h): word 0Ah
/// the time, words 0Ch and 0Eh cleared.
pub fn stamp(c: &mut Ctx, record: u16, time: u16) {
    c.set_word(record.wrapping_add(0x0a), time);
    c.set_word(record.wrapping_add(0x0c), 0);
    c.set_word(record.wrapping_add(0x0e), 0);
}

/// The game time (word [2]).
pub fn game_time(c: &mut Ctx) -> u16 {
    c.word(g::GAME_TIME)
}

/// The location the player is at ([114Eh]: a [`Location`] record).
pub fn here(c: &mut Ctx) -> u16 {
    c.word(g::MAP_POSITION_RECORD)
}

/// A location (sietch, palace, ...): a 1Ch-byte record in the table at
/// 100h, which ends with a byte FFh.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Location(pub u16);

impl Location {
    pub const SIZE: u16 = 0x1c;
    pub const FIRST: u16 = 0x100;

    /// Every record of the table, in order (up to the FFh one).
    pub fn all(c: &mut Ctx) -> Vec<Location> {
        let mut out = Vec::new();
        let mut l = Self::FIRST;
        loop {
            out.push(Location(l));
            l = l.wrapping_add(Self::SIZE);
            if c.byte(l) == 0xff {
                return out;
            }
        }
    }

    pub fn offset(self) -> u16 {
        self.0
    }

    /// Byte 19h: eight bits, not known yet (spread one per byte by
    /// `spread_location_bits`, 7EFBh), also summed as a number over the
    /// locations (`add_location_sum`, 765Eh).
    pub fn byte_19(self, c: &mut Ctx) -> u8 {
        c.byte(self.0.wrapping_add(0x19))
    }
    pub fn set_byte_19(self, c: &mut Ctx, v: u8) {
        c.set_byte(self.0.wrapping_add(0x19), v)
    }

    /// Byte 0Bh: 2 once a troop of yours has joined there (`troop_joins`).
    pub fn byte_0b(self, c: &mut Ctx) -> u8 {
        c.byte(self.0.wrapping_add(0x0b))
    }
    pub fn set_byte_0b(self, c: &mut Ctx, v: u8) {
        c.set_byte(self.0.wrapping_add(0x0b), v)
    }
}
