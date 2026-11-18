//! The sentence being said, as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! Whether a character says it (and whether one did when its voice
//! started), its dialogue entry's flags, the voice file's part, its
//! number part (state, entry, record) and the record `note_point`
//! noted, whose portrait a talker from 0Eh gets.
//!
//! A table of one record
//! ([`Which::Sentence`], Ch bytes at DS:47DCh) in the generic record tables
//! (`hand/record_table.rs`, the memory kept as the checked mirror);
//! [`Sentence`] is its typed view, each field at its global's offset
//! ([`super::globals`] says what each one is).

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::Sentence.base();

/// The sentence being said.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Sentence {
    /// [`g::TALKING`] (47DCh).
    pub talking: u8,
    /// [`g::VOICE_TALKING`] (47DDh).
    pub voice_talking: u8,
    /// [`g::ENTRY_FLAGS`] (47DEh).
    pub entry_flags: u16,
    /// [`g::VOICE_LANGUAGE_PART`] (47E0h).
    pub voice_part: u8,
    /// [`g::NUMBER_PART`] (47E1h).
    pub number_part: u8,
    /// [`g::NUMBER_ENTRY`] (47E2h).
    pub number_entry: u8,
    /// 47E3h: no code uses it (a script may); kept as it is.
    pub byte_e3: u8,
    /// [`g::NUMBER_RECORD`] (47E4h).
    pub number_record: u16,
    /// [`g::NOTED_RECORD`] (47E6h).
    pub noted_record: u16,
}

/// A field's offset in the record.
const fn at(global: u16) -> usize {
    (global - AT) as usize
}

fn word(b: &[u8], i: usize) -> u16 {
    b[i] as u16 | (b[i + 1] as u16) << 8
}

fn set_word(b: &mut [u8], i: usize, v: u16) {
    b[i..i + 2].copy_from_slice(&v.to_le_bytes());
}

impl Single for Sentence {
    const WHICH: Which = Which::Sentence;

    fn from_bytes(b: &[u8]) -> Sentence {
        Sentence {
            talking: b[at(g::TALKING)],
            voice_talking: b[at(g::VOICE_TALKING)],
            entry_flags: word(b, at(g::ENTRY_FLAGS)),
            voice_part: b[at(g::VOICE_LANGUAGE_PART)],
            number_part: b[at(g::NUMBER_PART)],
            number_entry: b[at(g::NUMBER_ENTRY)],
            byte_e3: b[(0x47e3 - AT) as usize],
            number_record: word(b, at(g::NUMBER_RECORD)),
            noted_record: word(b, at(g::NOTED_RECORD)),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        b[at(g::TALKING)] = self.talking;
        b[at(g::VOICE_TALKING)] = self.voice_talking;
        set_word(&mut b, at(g::ENTRY_FLAGS), self.entry_flags);
        b[at(g::VOICE_LANGUAGE_PART)] = self.voice_part;
        b[at(g::NUMBER_PART)] = self.number_part;
        b[at(g::NUMBER_ENTRY)] = self.number_entry;
        b[(0x47e3 - AT) as usize] = self.byte_e3;
        set_word(&mut b, at(g::NUMBER_RECORD), self.number_record);
        set_word(&mut b, at(g::NOTED_RECORD), self.noted_record);
        b
    }
}

const _: () = assert!(g::TALKING == 0x47dc);
const _: () = assert!(g::VOICE_TALKING == 0x47dd);
const _: () = assert!(g::ENTRY_FLAGS == 0x47de);
const _: () = assert!(g::VOICE_LANGUAGE_PART == 0x47e0);
const _: () = assert!(g::NUMBER_PART == 0x47e1);
const _: () = assert!(g::NUMBER_ENTRY == 0x47e2);
const _: () = assert!(g::NUMBER_RECORD == 0x47e4);
const _: () = assert!(g::NOTED_RECORD == 0x47e6);
const _: () = assert!(AT == 0x47dc && (AT + 0x0c) as usize >= 0x47e8);
