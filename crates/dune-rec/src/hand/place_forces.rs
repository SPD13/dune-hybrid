//! The forces at the place talked about, as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! `count_place_troops` (34A5h) counts the troops at the place talked
//! about and on their way there, by side and by job;
//! `measure_place_forces` (33BEh) adds up both sides' strengths and the
//! odds; a battle there counts its losses.
//!
//! The counts are one array ([`PlaceForces::counts`], 60h..8Fh), as the
//! original addresses them: ours, the Harkonnens', by job; and the same
//! for those on their way from 7Eh. The counts by word 12h's low
//! nibble (from 71h) overlap the last job's and the second set's.
//!
//! Part of the story state (DS:0..FFh, the variables the condition
//! scripts test by their offset): a table of one record
//! ([`Which::PlaceForces`], 3Dh bytes at DS:60h) in the generic record tables
//! (`hand/record_table.rs`, the memory kept as the checked mirror);
//! [`PlaceForces`] is its typed view, each field at its global's offset
//! ([`super::globals`] says what each one is).

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::PlaceForces.base();

/// The forces at the place talked about.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PlaceForces {
    /// [`g::TOPIC_PLACE_OUR_TROOPS`] (60h).
    pub counts: [u8; 0x30],
    /// [`g::TOPIC_PLACE_IDLE`] (90h).
    pub idle: u8,
    /// [`g::TOPIC_PLACE_OUR_TOTAL`] (91h).
    pub our_total: u8,
    /// [`g::TOPIC_PLACE_HARKONNEN_TOTAL`] (92h).
    pub harkonnen_total: u8,
    /// 93h: no code uses it (a script may); kept as it is.
    pub byte_93: u8,
    /// [`g::TOPIC_PLACE_HARKONNEN_STRENGTH`] (94h).
    pub harkonnen_strength: u16,
    /// [`g::TOPIC_PLACE_OUR_STRENGTH`] (96h).
    pub our_strength: u16,
    /// [`g::BATTLE_HARKONNEN_LOSS`] (98h).
    pub harkonnen_loss: u16,
    /// [`g::BATTLE_OUR_LOSS`] (9Ah).
    pub our_loss: u16,
    /// [`g::TOPIC_PLACE_STRIKE_ODDS`] (9Ch).
    pub strike_odds: u8,
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

impl Single for PlaceForces {
    const WHICH: Which = Which::PlaceForces;

    fn from_bytes(b: &[u8]) -> PlaceForces {
        PlaceForces {
            counts: std::array::from_fn(|k| b[at(g::TOPIC_PLACE_OUR_TROOPS) + k]),
            idle: b[at(g::TOPIC_PLACE_IDLE)],
            our_total: b[at(g::TOPIC_PLACE_OUR_TOTAL)],
            harkonnen_total: b[at(g::TOPIC_PLACE_HARKONNEN_TOTAL)],
            byte_93: b[(0x93 - AT) as usize],
            harkonnen_strength: word(b, at(g::TOPIC_PLACE_HARKONNEN_STRENGTH)),
            our_strength: word(b, at(g::TOPIC_PLACE_OUR_STRENGTH)),
            harkonnen_loss: word(b, at(g::BATTLE_HARKONNEN_LOSS)),
            our_loss: word(b, at(g::BATTLE_OUR_LOSS)),
            strike_odds: b[at(g::TOPIC_PLACE_STRIKE_ODDS)],
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        b[at(g::TOPIC_PLACE_OUR_TROOPS)..at(g::TOPIC_PLACE_OUR_TROOPS) + 0x30].copy_from_slice(&self.counts);
        b[at(g::TOPIC_PLACE_IDLE)] = self.idle;
        b[at(g::TOPIC_PLACE_OUR_TOTAL)] = self.our_total;
        b[at(g::TOPIC_PLACE_HARKONNEN_TOTAL)] = self.harkonnen_total;
        b[(0x93 - AT) as usize] = self.byte_93;
        set_word(&mut b, at(g::TOPIC_PLACE_HARKONNEN_STRENGTH), self.harkonnen_strength);
        set_word(&mut b, at(g::TOPIC_PLACE_OUR_STRENGTH), self.our_strength);
        set_word(&mut b, at(g::BATTLE_HARKONNEN_LOSS), self.harkonnen_loss);
        set_word(&mut b, at(g::BATTLE_OUR_LOSS), self.our_loss);
        b[at(g::TOPIC_PLACE_STRIKE_ODDS)] = self.strike_odds;
        b
    }
}

const _: () = assert!(g::TOPIC_PLACE_OUR_TROOPS == 0x60);
const _: () = assert!(g::TOPIC_PLACE_IDLE == 0x90);
const _: () = assert!(g::TOPIC_PLACE_OUR_TOTAL == 0x91);
const _: () = assert!(g::TOPIC_PLACE_HARKONNEN_TOTAL == 0x92);
const _: () = assert!(g::TOPIC_PLACE_HARKONNEN_STRENGTH == 0x94);
const _: () = assert!(g::TOPIC_PLACE_OUR_STRENGTH == 0x96);
const _: () = assert!(g::BATTLE_HARKONNEN_LOSS == 0x98);
const _: () = assert!(g::BATTLE_OUR_LOSS == 0x9a);
const _: () = assert!(g::TOPIC_PLACE_STRIKE_ODDS == 0x9c);
const _: () = assert!(AT == 0x60 && (AT + 0x3d) as usize >= 0x9d);
// The counts by job follow the Harkonnens' count, for those there and on their way.
const _: () = assert!(
    g::TOPIC_PLACE_JOBS == g::TOPIC_PLACE_HARKONNEN_TROOPS + 1
        && g::TOPIC_PLACE_INBOUND_JOBS == g::TOPIC_PLACE_HARKONNEN_INBOUND + 1
);
