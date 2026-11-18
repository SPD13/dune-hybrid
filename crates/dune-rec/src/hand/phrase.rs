//! The phrase file, the phrase put together and its voice, as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! Which phrase file is asked for and which is loaded, the last control
//! code met in the phrase being put together, the phrase whose voice
//! plays and the top row of the text sheet's last blit.
//!
//! A table of one record
//! ([`Which::Phrase`], 8h bytes at DS:477Ch) in the generic record tables
//! (`hand/record_table.rs`, the memory kept as the checked mirror);
//! [`Phrase`] is its typed view, each field at its global's offset
//! ([`super::globals`] says what each one is).

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::Phrase.base();

/// The phrase file, the phrase put together and its voice.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Phrase {
    /// [`g::PHRASE_PART`] (477Ch).
    pub part: u16,
    /// [`g::PHRASE_LOADED`] (477Eh).
    pub loaded: u8,
    /// [`g::PHRASE_CODE`] (477Fh).
    pub code: u8,
    /// [`g::VOICE_PHRASE`] (4780h).
    pub voice: u16,
    /// [`g::TEXT_SHEET_TOP`] (4782h).
    pub sheet_top: u16,
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

impl Single for Phrase {
    const WHICH: Which = Which::Phrase;

    fn from_bytes(b: &[u8]) -> Phrase {
        Phrase {
            part: word(b, at(g::PHRASE_PART)),
            loaded: b[at(g::PHRASE_LOADED)],
            code: b[at(g::PHRASE_CODE)],
            voice: word(b, at(g::VOICE_PHRASE)),
            sheet_top: word(b, at(g::TEXT_SHEET_TOP)),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        set_word(&mut b, at(g::PHRASE_PART), self.part);
        b[at(g::PHRASE_LOADED)] = self.loaded;
        b[at(g::PHRASE_CODE)] = self.code;
        set_word(&mut b, at(g::VOICE_PHRASE), self.voice);
        set_word(&mut b, at(g::TEXT_SHEET_TOP), self.sheet_top);
        b
    }
}

const _: () = assert!(g::PHRASE_PART == 0x477c);
const _: () = assert!(g::PHRASE_LOADED == 0x477e);
const _: () = assert!(g::PHRASE_CODE == 0x477f);
const _: () = assert!(g::VOICE_PHRASE == 0x4780);
const _: () = assert!(g::TEXT_SHEET_TOP == 0x4782);
const _: () = assert!(AT == 0x477c && (AT + 0x08) as usize >= 0x4784);
