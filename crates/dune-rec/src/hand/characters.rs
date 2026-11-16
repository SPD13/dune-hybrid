//! The characters as Rust state (Stage B of `docs/readable-port-plan.md`):
//! where each of the people the player meets is, what a click on them
//! does, and when they were last met (the talker's record is
//! FD8h + 10h × [`g::TALKER`]).
//!
//! A table of 10h records of 10h bytes ([`Which::Characters`], at FD8h)
//! in the generic record tables (`hand/record_table.rs`, the memory kept
//! as the checked mirror); [`Character`] is the view of one. Most of
//! the ports reach a record through SI (its bytes written, followed);
//! fields not yet understood keep their offsets in their names.

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the table starts in the data segment.
pub const AT: u16 = Which::Characters.base();
/// The size of a record.
pub const SIZE: u16 = 0x10;

/// The record of character `i` (FD8h + 10h × `i`).
pub const fn record(i: u16) -> u16 {
    AT + SIZE * i
}

/// The character whose record is at `offset`, if it is one.
pub fn index(offset: u16) -> Option<usize> {
    record_table::index(Which::Characters, offset)
}

/// A character.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Character {
    /// +0: where it is (byte 0 a place's room, 1..; byte 1 its state,
    /// set from DH when its place changes hands, `loc_06dbb`).
    pub place: u16,
    /// +2: the position of its place (compared with the player's [6]; 80h
    /// in its low byte: at a location, whose index is the high byte).
    pub position: u16,
    /// +4: where a click on it goes (`jmp [si+4]`, 923Dh).
    pub on_click: u16,
    /// +6.
    pub word_06: u16,
    /// +8: a game time noted (the speaker's mood counts from it with bit
    /// 40h of [`Self::flags`]).
    pub time_08: u16,
    /// +0Ah: a game time noted (the mood counts from it without bit 40h).
    pub time_0a: u16,
    /// +0Ch.
    pub word_0c: u16,
    /// +0Eh: tested by `loc_0127c` when the characters at the player's
    /// place are listed (196Ah).
    pub byte_0e: u8,
    /// +0Fh: bits. 40h: its gauge shown; 20h: set when a line ends
    /// (with 4 cleared).
    pub flags: u8,
}

impl Single for Character {
    const WHICH: Which = Which::Characters;

    fn from_bytes(b: &[u8]) -> Character {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        Character {
            place: w(0),
            position: w(2),
            on_click: w(4),
            word_06: w(6),
            time_08: w(8),
            time_0a: w(0x0a),
            word_0c: w(0x0c),
            byte_0e: b[0x0e],
            flags: b[0x0f],
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        let words = [self.place, self.position, self.on_click, self.word_06, self.time_08, self.time_0a, self.word_0c];
        for (k, v) in words.into_iter().enumerate() {
            b[2 * k..2 * k + 2].copy_from_slice(&v.to_le_bytes());
        }
        b[0x0e] = self.byte_0e;
        b[0x0f] = self.flags;
        b
    }
}

const _: () = assert!(g::BYTE_0FD8 == AT && g::FLAG_0FDB == AT + 3);
