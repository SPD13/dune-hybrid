//! The place talked about, as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! `describe_place` (331Eh) copies a location's record here for the
//! condition scripts, with its equipment and the kinds its troops leave
//! spare; its troops are counted in `hand/place_forces.rs`.
//!
//! Part of the story state (DS:0..FFh, the variables the condition
//! scripts test by their offset): a table of one record
//! ([`Which::TopicPlace`], 13h bytes at DS:4Dh) in the generic record tables
//! (`hand/record_table.rs`, the memory kept as the checked mirror);
//! [`TopicPlace`] is its typed view, each field at its global's offset
//! ([`super::globals`] says what each one is).

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::TopicPlace.base();

/// The place talked about.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct TopicPlace {
    /// [`g::TOPIC_PLACE_KIND`] (4Dh).
    pub kind: u8,
    /// [`g::TOPIC_PLACE_NAME`] (4Eh).
    pub name: u16,
    /// [`g::TOPIC_PLACE_CHANCE`] (50h).
    pub chance: u8,
    /// [`g::TOPIC_PLACE_STATUS`] (51h).
    pub status: u8,
    /// [`g::TOPIC_PLACE_SPICE`] (52h).
    pub spice: u8,
    /// [`g::TOPIC_PLACE_SPARE_KINDS`] (53h).
    pub spare_kinds: u8,
    /// [`g::TOPIC_PLACE_WATER`] (54h).
    pub water: u8,
    /// [`g::TOPIC_PLACE_EQUIPMENT`] (55h).
    pub equipment: [u8; 7],
    /// [`g::TOPIC_PLACE_TROOP_BITS_10`] (5Ch).
    pub troop_bits_10: u16,
    /// [`g::TOPIC_PLACE_TROOP_BITS_12`] (5Eh).
    pub troop_bits_12: u16,
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

impl Single for TopicPlace {
    const WHICH: Which = Which::TopicPlace;

    fn from_bytes(b: &[u8]) -> TopicPlace {
        TopicPlace {
            kind: b[at(g::TOPIC_PLACE_KIND)],
            name: word(b, at(g::TOPIC_PLACE_NAME)),
            chance: b[at(g::TOPIC_PLACE_CHANCE)],
            status: b[at(g::TOPIC_PLACE_STATUS)],
            spice: b[at(g::TOPIC_PLACE_SPICE)],
            spare_kinds: b[at(g::TOPIC_PLACE_SPARE_KINDS)],
            water: b[at(g::TOPIC_PLACE_WATER)],
            equipment: std::array::from_fn(|k| b[at(g::TOPIC_PLACE_EQUIPMENT) + k]),
            troop_bits_10: word(b, at(g::TOPIC_PLACE_TROOP_BITS_10)),
            troop_bits_12: word(b, at(g::TOPIC_PLACE_TROOP_BITS_12)),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        b[at(g::TOPIC_PLACE_KIND)] = self.kind;
        set_word(&mut b, at(g::TOPIC_PLACE_NAME), self.name);
        b[at(g::TOPIC_PLACE_CHANCE)] = self.chance;
        b[at(g::TOPIC_PLACE_STATUS)] = self.status;
        b[at(g::TOPIC_PLACE_SPICE)] = self.spice;
        b[at(g::TOPIC_PLACE_SPARE_KINDS)] = self.spare_kinds;
        b[at(g::TOPIC_PLACE_WATER)] = self.water;
        b[at(g::TOPIC_PLACE_EQUIPMENT)..at(g::TOPIC_PLACE_EQUIPMENT) + 7].copy_from_slice(&self.equipment);
        set_word(&mut b, at(g::TOPIC_PLACE_TROOP_BITS_10), self.troop_bits_10);
        set_word(&mut b, at(g::TOPIC_PLACE_TROOP_BITS_12), self.troop_bits_12);
        b
    }
}

const _: () = assert!(g::TOPIC_PLACE_KIND == 0x4d);
const _: () = assert!(g::TOPIC_PLACE_NAME == 0x4e);
const _: () = assert!(g::TOPIC_PLACE_CHANCE == 0x50);
const _: () = assert!(g::TOPIC_PLACE_STATUS == 0x51);
const _: () = assert!(g::TOPIC_PLACE_SPICE == 0x52);
const _: () = assert!(g::TOPIC_PLACE_SPARE_KINDS == 0x53);
const _: () = assert!(g::TOPIC_PLACE_WATER == 0x54);
const _: () = assert!(g::TOPIC_PLACE_EQUIPMENT == 0x55);
const _: () = assert!(g::TOPIC_PLACE_TROOP_BITS_10 == 0x5c);
const _: () = assert!(g::TOPIC_PLACE_TROOP_BITS_12 == 0x5e);
const _: () = assert!(AT == 0x4d && (AT + 0x13) as usize >= 0x60);
