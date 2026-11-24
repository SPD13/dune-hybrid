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

use super::Ctx;
use super::globals as g;
use super::record_table::{self, Single, Which};
use pc::cpu::DS;

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
    /// +0, +2: where it is, as the player's [4], [6] ([`g::PLAYER_X`]):
    /// a map position (x, y), or with 80h in y's low byte at the location
    /// whose index is y's high byte, x's low byte then its room (1..) and
    /// its high byte the location's kind (set again from DH when the
    /// place changes hands, `loc_06dbb`).
    pub x: u16,
    pub y: u16,
    /// +4: where a click on it goes (`jmp [si+4]`, 923Dh).
    pub on_click: u16,
    /// +6.
    pub word_06: u16,
    /// +8: when it joined the party (9608h; in the party the speaker's
    /// mood counts from it).
    pub joined_at: u16,
    /// +0Ah: when it left the party (9556h; out of it the mood counts
    /// from it).
    pub left_at: u16,
    /// +0Ch: for character 13, the smuggler being talked to (its record
    /// in [`g::SMUGGLERS`], [10B4h]).
    pub word_0c: u16,
    /// +0Eh: its number, its bit in the sets [`g::MET`], [`g::PARTY`],
    /// [`g::PRESENT`] and [`g::TALKED_HERE`] (0Eh: none).
    pub number: u8,
    /// +0Fh: its [`character_flag`] bits.
    pub flags: u8,
}

impl Single for Character {
    const WHICH: Which = Which::Characters;

    fn from_bytes(b: &[u8]) -> Character {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        Character {
            x: w(0),
            y: w(2),
            on_click: w(4),
            word_06: w(6),
            joined_at: w(8),
            left_at: w(0x0a),
            word_0c: w(0x0c),
            number: b[0x0e],
            flags: b[0x0f],
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        let words = [self.x, self.y, self.on_click, self.word_06, self.joined_at, self.left_at, self.word_0c];
        for (k, v) in words.into_iter().enumerate() {
            b[2 * k..2 * k + 2].copy_from_slice(&v.to_le_bytes());
        }
        b[0x0e] = self.number;
        b[0x0f] = self.flags;
        b
    }
}

const _: () = assert!(g::CHARACTERS == AT && g::CHARACTER_0_LOCATION == AT + 3);

/// Bits of a character's byte 0Fh ([`Character::flags`]).
pub mod character_flag {
    /// Set when one of its lines ends (`line_done`, with 4 cleared).
    pub const TALKED: u8 = 0x20;
    /// In the party: its gauge shown (`add_speaker_gauge`), it follows
    /// the player (`move_following_character`), `update_people` leaves
    /// it be.
    pub const IN_PARTY: u8 = 0x40;
}

/// Character numbers (the talker, [47C4h]; the record at FD8h + 10h * it).
/// A talker's head is resource talker + 2 (`open_talking_head_resource`),
/// and the game's resource names (the table at [31FFh]) name each: LETO,
/// JESS, HAWA, IDAH, GURN, STIL, KYNE, CHAN, HARA, BARO, FEYD, EMPR, HARK,
/// SMUG, then FRM1..FRM3 (heads 0Eh..10h, the Fremen chiefs) and GENERIC
/// (11h, the head of phases 15h..1Fh). The ones talked to in the running
/// game (Duke Leto, Jessica, Duncan, Stilgar, Chani, Harah, a Fremen chief)
/// read the same.
pub mod character_id {
    pub const DUKE_LETO: u16 = 0;
    pub const JESSICA: u16 = 1;
    pub const THUFIR_HAWAT: u16 = 2;
    pub const DUNCAN_IDAHO: u16 = 3;
    pub const GURNEY_HALLECK: u16 = 4;
    pub const STILGAR: u16 = 5;
    pub const LIET_KYNES: u16 = 6;
    pub const CHANI: u16 = 7;
    pub const HARAH: u16 = 8;
    pub const BARON: u16 = 9;
    pub const FEYD: u16 = 0x0a;
    pub const EMPEROR: u16 = 0x0b;
    /// HARK's head; with bit 10h of its flags it talks from sentence bank
    /// 0Ah (91A0h).
    pub const HARKONNEN: u16 = 0x0c;
    /// Talked to, the smuggler's mood [9Eh] is picked (2388h).
    pub const SMUGGLER: u16 = 0x0d;
    /// The Fremen troops' chiefs: a speaker from 0Fh up talks as 0Fh, the
    /// rest kept at [`CHARACTER_15_SLOT`](super::super::globals::CHARACTER_15_SLOT) (3D4Bh).
    pub const FREMEN_CHIEF: u16 = 0x0f;
    /// The head GENERIC.HSQ (the speaker in phases 15h..1Fh, 3D5Eh).
    pub const GENERIC_HEAD: u16 = 0x11;
    /// Paul's head (PAUL.HSQ), the second head of a two-head scene (99D4h).
    pub const PAUL_HEAD: u16 = 0x2d;
}

/// The view of the character record at an offset (what the ports hold
/// in SI): its fields through the table, as [`Character`] has them;
/// the memory for an offset that starts no record or another data
/// segment.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct CharacterAt(pub u16);

impl CharacterAt {
    fn native(self, c: &mut Ctx, field: u16) -> Option<usize> {
        if c.sreg(DS) != g::GAME_DS || field >= SIZE {
            return None;
        }
        index(self.0)
    }
    fn byte(self, c: &mut Ctx, field: u16) -> u8 {
        match self.native(c, field) {
            Some(i) => record_table::byte(c, Which::Characters, i, field),
            None => c.byte(self.0.wrapping_add(field)),
        }
    }
    fn set_byte(self, c: &mut Ctx, field: u16, v: u8) {
        match self.native(c, field) {
            Some(i) => record_table::set_byte(c, Which::Characters, i, field, v),
            None => c.set_byte(self.0.wrapping_add(field), v),
        }
    }
    fn word(self, c: &mut Ctx, field: u16) -> u16 {
        self.byte(c, field) as u16 | (self.byte(c, field + 1) as u16) << 8
    }
    fn set_word(self, c: &mut Ctx, field: u16, v: u16) {
        self.set_byte(c, field, v as u8);
        self.set_byte(c, field + 1, (v >> 8) as u8);
    }

    /// Word 2: [`Character::y`].
    pub(super) fn y(self, c: &mut Ctx) -> u16 {
        self.word(c, 2)
    }
    pub(super) fn set_y(self, c: &mut Ctx, v: u16) {
        self.set_word(c, 2, v)
    }
    /// Byte 3 (y's high byte): at a location, the location's index.
    pub(super) fn location_index(self, c: &mut Ctx) -> u8 {
        self.byte(c, 3)
    }
    /// Word 4: [`Character::on_click`].
    pub(super) fn on_click(self, c: &mut Ctx) -> u16 {
        self.word(c, 4)
    }
    /// Byte 0Eh: [`Character::number`].
    pub(super) fn number(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x0e)
    }
    /// Byte 0Fh: its [`character_flag`] bits.
    pub(super) fn flags(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x0f)
    }
    pub(super) fn set_flags(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x0f, v)
    }
}
