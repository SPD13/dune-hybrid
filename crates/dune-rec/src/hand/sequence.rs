//! The story sequence playing, as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! A story sequence is a script of steps the game plays by itself
//! (`loc_01771` starts one, `loc_01707` runs its next step, a click of
//! both buttons moves it on): whether one plays, the hovered item's
//! blink, where the player goes back to at its end, its scene's
//! position markers and its next step.
//!
//! A table of one record
//! ([`Which::Sequence`], 8h bytes at DS:4774h) in the generic record tables
//! (`hand/record_table.rs`, the memory kept as the checked mirror);
//! [`Sequence`] is its typed view, each field at its global's offset
//! ([`super::globals`] says what each one is).

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::Sequence.base();

/// The story sequence playing.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Sequence {
    /// [`g::SEQUENCE_PLAYING`] (4774h).
    pub playing: u8,
    /// [`g::BLINK_PHASE`] (4775h).
    pub blink: u8,
    /// [`g::SEQUENCE_RETURN`] (4776h).
    pub return_to: u16,
    /// [`g::SEQUENCE_MARKERS`] (4778h).
    pub markers: u16,
    /// [`g::SEQUENCE_NEXT`] (477Ah).
    pub next: u16,
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

impl Single for Sequence {
    const WHICH: Which = Which::Sequence;

    fn from_bytes(b: &[u8]) -> Sequence {
        Sequence {
            playing: b[at(g::SEQUENCE_PLAYING)],
            blink: b[at(g::BLINK_PHASE)],
            return_to: word(b, at(g::SEQUENCE_RETURN)),
            markers: word(b, at(g::SEQUENCE_MARKERS)),
            next: word(b, at(g::SEQUENCE_NEXT)),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        b[at(g::SEQUENCE_PLAYING)] = self.playing;
        b[at(g::BLINK_PHASE)] = self.blink;
        set_word(&mut b, at(g::SEQUENCE_RETURN), self.return_to);
        set_word(&mut b, at(g::SEQUENCE_MARKERS), self.markers);
        set_word(&mut b, at(g::SEQUENCE_NEXT), self.next);
        b
    }
}

const _: () = assert!(g::SEQUENCE_PLAYING == 0x4774);
const _: () = assert!(g::BLINK_PHASE == 0x4775);
const _: () = assert!(g::SEQUENCE_RETURN == 0x4776);
const _: () = assert!(g::SEQUENCE_MARKERS == 0x4778);
const _: () = assert!(g::SEQUENCE_NEXT == 0x477a);
const _: () = assert!(AT == 0x4774 && (AT + 0x08) as usize >= 0x477c);
