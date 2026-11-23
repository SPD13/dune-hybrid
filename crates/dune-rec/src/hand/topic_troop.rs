//! The troop talked about, as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! `describe_troop` (31F6h) copies a troop's record here, with what is
//! worked out from it (age, rate, skill phrases), for the condition
//! scripts to test; the change of its skills and whether it is out of
//! reach come with it.
//!
//! Part of the story state (DS:0..FFh, the variables the condition
//! scripts test by their offset): a table of one record
//! ([`Which::TopicTroop`], 21h bytes at DS:2Ch) in the generic record tables
//! (`hand/record_table.rs`, the memory kept as the checked mirror);
//! [`TopicTroop`] is its typed view, each field at its global's offset
//! ([`super::globals`] says what each one is).

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::TopicTroop.base();

/// The troop talked about.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct TopicTroop {
    /// [`g::TOPIC_TROOP_PLACE`] (2Ch).
    pub place: u16,
    /// [`g::TOPIC_TROOP_ID`] (2Eh).
    pub id: u8,
    /// [`g::TOPIC_TROOP_JOB`] (2Fh).
    pub job: u8,
    /// [`g::TOPIC_TROOP_STATE`] (30h).
    pub state: u8,
    /// [`g::TOPIC_TROOP_WORD_12_LOW`] (31h).
    pub word_12_low: u8,
    /// [`g::TOPIC_TROOP_WORD_10`] (32h).
    pub word_10: u16,
    /// [`g::TOPIC_TROOP_WORD_12`] (34h).
    pub word_12: u16,
    /// [`g::TOPIC_TROOP_MOTIVATION`] (36h).
    pub motivation: u8,
    /// [`g::TOPIC_TROOP_JOB_SKILL`] (37h).
    pub job_skill: u8,
    /// [`g::TOPIC_TROOP_SKILLS`] (38h).
    pub skills: [u8; 3],
    /// [`g::TOPIC_TROOP_EQUIPMENT`] (3Bh).
    pub equipment: u8,
    /// [`g::TOPIC_TROOP_SIZE`] (3Ch).
    pub size: u8,
    /// [`g::TOPIC_TROOP_EQUIPMENT_GAINED`] (3Dh).
    pub equipment_gained: u8,
    /// [`g::TOPIC_TROOP_EQUIPMENT_LOST`] (3Eh).
    pub equipment_lost: u8,
    /// [`g::TOPIC_TROOP_GAINED_HERE`] (3Fh).
    pub gained_here: u8,
    /// [`g::TOPIC_TROOP_DAYS`] (40h).
    pub days: u8,
    /// [`g::TOPIC_TROOP_AGE_DAYS`] (41h).
    pub age_days: u8,
    /// [`g::TOPIC_TROOP_AGE`] (42h).
    pub age: u16,
    /// [`g::TOPIC_TROOP_WORD_0C`] (44h).
    pub word_0c: u16,
    /// [`g::TOPIC_TROOP_WORD_0E`] (46h).
    pub word_0e: u16,
    /// [`g::TOPIC_TROOP_RATE`] (48h).
    pub rate: u16,
    /// [`g::TOPIC_TROOP_AVERAGE`] (4Ah).
    pub average: u16,
    /// [`g::TALK_OUT_OF_REACH`] (4Ch).
    pub out_of_reach: u8,
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

impl Single for TopicTroop {
    const WHICH: Which = Which::TopicTroop;

    fn from_bytes(b: &[u8]) -> TopicTroop {
        TopicTroop {
            place: word(b, at(g::TOPIC_TROOP_PLACE)),
            id: b[at(g::TOPIC_TROOP_ID)],
            job: b[at(g::TOPIC_TROOP_JOB)],
            state: b[at(g::TOPIC_TROOP_STATE)],
            word_12_low: b[at(g::TOPIC_TROOP_WORD_12_LOW)],
            word_10: word(b, at(g::TOPIC_TROOP_WORD_10)),
            word_12: word(b, at(g::TOPIC_TROOP_WORD_12)),
            motivation: b[at(g::TOPIC_TROOP_MOTIVATION)],
            job_skill: b[at(g::TOPIC_TROOP_JOB_SKILL)],
            skills: std::array::from_fn(|k| b[at(g::TOPIC_TROOP_SKILLS) + k]),
            equipment: b[at(g::TOPIC_TROOP_EQUIPMENT)],
            size: b[at(g::TOPIC_TROOP_SIZE)],
            equipment_gained: b[at(g::TOPIC_TROOP_EQUIPMENT_GAINED)],
            equipment_lost: b[at(g::TOPIC_TROOP_EQUIPMENT_LOST)],
            gained_here: b[at(g::TOPIC_TROOP_GAINED_HERE)],
            days: b[at(g::TOPIC_TROOP_DAYS)],
            age_days: b[at(g::TOPIC_TROOP_AGE_DAYS)],
            age: word(b, at(g::TOPIC_TROOP_AGE)),
            word_0c: word(b, at(g::TOPIC_TROOP_WORD_0C)),
            word_0e: word(b, at(g::TOPIC_TROOP_WORD_0E)),
            rate: word(b, at(g::TOPIC_TROOP_RATE)),
            average: word(b, at(g::TOPIC_TROOP_AVERAGE)),
            out_of_reach: b[at(g::TALK_OUT_OF_REACH)],
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        set_word(&mut b, at(g::TOPIC_TROOP_PLACE), self.place);
        b[at(g::TOPIC_TROOP_ID)] = self.id;
        b[at(g::TOPIC_TROOP_JOB)] = self.job;
        b[at(g::TOPIC_TROOP_STATE)] = self.state;
        b[at(g::TOPIC_TROOP_WORD_12_LOW)] = self.word_12_low;
        set_word(&mut b, at(g::TOPIC_TROOP_WORD_10), self.word_10);
        set_word(&mut b, at(g::TOPIC_TROOP_WORD_12), self.word_12);
        b[at(g::TOPIC_TROOP_MOTIVATION)] = self.motivation;
        b[at(g::TOPIC_TROOP_JOB_SKILL)] = self.job_skill;
        b[at(g::TOPIC_TROOP_SKILLS)..at(g::TOPIC_TROOP_SKILLS) + 3].copy_from_slice(&self.skills);
        b[at(g::TOPIC_TROOP_EQUIPMENT)] = self.equipment;
        b[at(g::TOPIC_TROOP_SIZE)] = self.size;
        b[at(g::TOPIC_TROOP_EQUIPMENT_GAINED)] = self.equipment_gained;
        b[at(g::TOPIC_TROOP_EQUIPMENT_LOST)] = self.equipment_lost;
        b[at(g::TOPIC_TROOP_GAINED_HERE)] = self.gained_here;
        b[at(g::TOPIC_TROOP_DAYS)] = self.days;
        b[at(g::TOPIC_TROOP_AGE_DAYS)] = self.age_days;
        set_word(&mut b, at(g::TOPIC_TROOP_AGE), self.age);
        set_word(&mut b, at(g::TOPIC_TROOP_WORD_0C), self.word_0c);
        set_word(&mut b, at(g::TOPIC_TROOP_WORD_0E), self.word_0e);
        set_word(&mut b, at(g::TOPIC_TROOP_RATE), self.rate);
        set_word(&mut b, at(g::TOPIC_TROOP_AVERAGE), self.average);
        b[at(g::TALK_OUT_OF_REACH)] = self.out_of_reach;
        b
    }
}

const _: () = assert!(g::TOPIC_TROOP_PLACE == 0x2c);
const _: () = assert!(g::TOPIC_TROOP_ID == 0x2e);
const _: () = assert!(g::TOPIC_TROOP_JOB == 0x2f);
const _: () = assert!(g::TOPIC_TROOP_STATE == 0x30);
const _: () = assert!(g::TOPIC_TROOP_WORD_12_LOW == 0x31);
const _: () = assert!(g::TOPIC_TROOP_WORD_10 == 0x32);
const _: () = assert!(g::TOPIC_TROOP_WORD_12 == 0x34);
const _: () = assert!(g::TOPIC_TROOP_MOTIVATION == 0x36);
const _: () = assert!(g::TOPIC_TROOP_JOB_SKILL == 0x37);
const _: () = assert!(g::TOPIC_TROOP_SKILLS == 0x38);
const _: () = assert!(g::TOPIC_TROOP_EQUIPMENT == 0x3b);
const _: () = assert!(g::TOPIC_TROOP_SIZE == 0x3c);
const _: () = assert!(g::TOPIC_TROOP_EQUIPMENT_GAINED == 0x3d);
const _: () = assert!(g::TOPIC_TROOP_EQUIPMENT_LOST == 0x3e);
const _: () = assert!(g::TOPIC_TROOP_GAINED_HERE == 0x3f);
const _: () = assert!(g::TOPIC_TROOP_DAYS == 0x40);
const _: () = assert!(g::TOPIC_TROOP_AGE_DAYS == 0x41);
const _: () = assert!(g::TOPIC_TROOP_AGE == 0x42);
const _: () = assert!(g::TOPIC_TROOP_WORD_0C == 0x44);
const _: () = assert!(g::TOPIC_TROOP_WORD_0E == 0x46);
const _: () = assert!(g::TOPIC_TROOP_RATE == 0x48);
const _: () = assert!(g::TOPIC_TROOP_AVERAGE == 0x4a);
const _: () = assert!(g::TALK_OUT_OF_REACH == 0x4c);
const _: () = assert!(AT == 0x2c && (AT + 0x21) as usize >= 0x4d);
