//! The phrases a phrase's codes stand for, as Rust state (Stage B of
//! `docs/readable-port-plan.md`).
//!
//! A phrase can hold codes 80h..8Fh, each standing for another phrase
//! put in its place (`loc_08979`): the word in slot (code & 0Fh) of this
//! table. The ports that describe a troop, a place or an encounter fill
//! the slots before the phrase is said: slot 0 the phrase
//! `draw_script_caption` draws, 1 and 2 a place's name ([`PLACE`]), 3
//! the equipment the smugglers offer ([`SMUGGLER_ITEM`]), 4 the subject,
//! 5 a detail, 6..8 a troop's three skills, 9 a direction, 0Ah a troop's
//! home.
//!
//! A table of one record ([`Which::PhraseArgs`], 20h bytes at DS:11EBh)
//! in the generic record tables (`hand/record_table.rs`, the memory kept
//! as the checked mirror); [`PhraseArgs`] is its typed view.

use super::Ctx;
use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::PhraseArgs.base();

/// The slots, one per code 80h..8Fh.
pub const SLOTS: usize = 16;

/// Slot 0: the phrase `draw_script_caption` draws ([`g::PHRASE_ARGS`]).
pub const CAPTION: usize = 0;
/// Slots 1, 2 (codes 81h, 82h): a place's name, its two words
/// ([`g::PHRASE_ARG_PLACE`]).
pub const PLACE: usize = 1;
/// Slot 3 (83h): the equipment offered ([`g::SMUGGLER_ITEM`]).
pub const SMUGGLER_ITEM: usize = 3;
/// Slot 4 (84h): a troop's job, an encounter's place
/// ([`g::PHRASE_ARG_SUBJECT`]).
pub const SUBJECT: usize = 4;
/// Slot 5 (85h): how long a troop has been at it, an encounter's side
/// ([`g::PHRASE_ARG_DETAIL`]).
pub const DETAIL: usize = 5;
/// Slots 6..8 (86h..88h): a troop's three skills
/// ([`g::PHRASE_ARG_SKILLS`]).
pub const SKILLS: usize = 6;
/// Slot 9 (89h): the direction of the nearest place
/// ([`g::PHRASE_ARG_DIRECTION`]).
pub const DIRECTION: usize = 9;
/// Slot 0Ah (8Ah): a troop's home ([`g::PHRASE_ARG_TROOP_HOME`]).
pub const TROOP_HOME: usize = 0x0a;

/// The phrases the codes stand for.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct PhraseArgs {
    /// By the code's low nibble.
    pub slots: [u16; SLOTS],
}

impl Single for PhraseArgs {
    const WHICH: Which = Which::PhraseArgs;

    fn from_bytes(b: &[u8]) -> PhraseArgs {
        let mut slots = [0; SLOTS];
        for (i, v) in slots.iter_mut().enumerate() {
            *v = b[2 * i] as u16 | (b[2 * i + 1] as u16) << 8;
        }
        PhraseArgs { slots }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        for (i, v) in self.slots.iter().enumerate() {
            b[2 * i..2 * i + 2].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
}

impl Ctx<'_> {
    /// Slot `i` (past the last, a case the game never makes, the word
    /// where the code would read it).
    pub(super) fn phrase_slot(&mut self, i: usize) -> u16 {
        if i < SLOTS {
            return self.phrase_args().slots[i];
        }
        self.word(slot_at(i))
    }

    /// Slot `i` set to `v`.
    pub(super) fn set_phrase_slot(&mut self, i: usize, v: u16) {
        if i < SLOTS {
            self.update_phrase_args(|t| t.slots[i] = v);
        } else {
            self.set_word(slot_at(i), v);
        }
    }
}

/// A slot's offset in the data segment.
const fn slot_at(i: usize) -> u16 {
    AT + 2 * i as u16
}

const _: () = assert!(AT == 0x11eb && g::PHRASE_ARGS == AT);
const _: () = assert!(slot_at(PLACE) == g::PHRASE_ARG_PLACE);
const _: () = assert!(slot_at(SMUGGLER_ITEM) == g::SMUGGLER_ITEM);
const _: () = assert!(slot_at(SUBJECT) == g::PHRASE_ARG_SUBJECT);
const _: () = assert!(slot_at(DETAIL) == g::PHRASE_ARG_DETAIL);
const _: () = assert!(slot_at(SKILLS) == g::PHRASE_ARG_SKILLS);
const _: () = assert!(slot_at(DIRECTION) == g::PHRASE_ARG_DIRECTION);
const _: () = assert!(slot_at(TROOP_HOME) == g::PHRASE_ARG_TROOP_HOME);
// Slot 0Bh is [1201h] (set to 109h in `phases.rs`, before the charisma raise at 10E9h).
const _: () = assert!(slot_at(0x0b) == g::WORD_1201);
