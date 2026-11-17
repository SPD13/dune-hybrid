//! The player's progress in the story, as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! Where the player is (place, room), the story bits, the characters met,
//! in the party and present, what the speaker and the smuggler being
//! talked to are like, why the scripts are asked, and the phase, the
//! charisma and the sietches held.
//!
//! Part of the story state (DS:0..FFh, the variables the condition
//! scripts test by their offset): a table of one record
//! ([`Which::Progress`], 24h bytes at DS:8h) in the generic record tables
//! (`hand/record_table.rs`, the memory kept as the checked mirror);
//! [`Progress`] is its typed view, each field at its global's offset
//! ([`super::globals`] says what each one is).

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::Progress.base();

/// The player's progress in the story.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Progress {
    /// [`g::PLACE_KIND`] (8h).
    pub place_kind: u8,
    /// [`g::PLACE_INDEX`] (9h).
    pub place_index: u8,
    /// [`g::EVENT_FLAGS`] (Ah).
    pub event_flags: u8,
    /// [`g::CURRENT_ROOM`] (Bh).
    pub room: u8,
    /// [`g::ENTERING_ROOM`] (Ch).
    pub entering_room: u8,
    /// [`g::PREVIOUS_ROOM`] (Dh).
    pub previous_room: u8,
    /// [`g::MET`] (Eh).
    pub met: u16,
    /// [`g::PARTY`] (10h).
    pub party: u16,
    /// [`g::PRESENT`] (12h).
    pub present: u16,
    /// [`g::TALKED_HERE`] (14h).
    pub talked_here: u16,
    /// [`g::SPEAKER_TIME_SINCE`] (16h).
    pub speaker_time_since: u16,
    /// [`g::SPEAKER_FLAGS`] (18h).
    pub speaker_flags: u8,
    /// [`g::SPEAKER_SAID_LINE`] (19h).
    pub speaker_said_line: u8,
    /// [`g::ANSWER_COUNT`] (1Ah).
    pub answer_count: u8,
    /// [`g::REQUESTS`] (1Bh).
    pub requests: u8,
    /// [`g::SMUGGLER_FLAGS`] (1Ch).
    pub smuggler_flags: u8,
    /// [`g::SMUGGLER_PATIENCE`] (1Dh).
    pub smuggler_patience: u8,
    /// [`g::SMUGGLER_DAYS_AWAY`] (1Eh).
    pub smuggler_days_away: u8,
    /// [`g::DEBT_DAYS`] (1Fh).
    pub debt_days: u8,
    /// [`g::SMUGGLER_DEBT`] (20h).
    pub smuggler_debt: u16,
    /// [`g::DEBTS`] (22h).
    pub debts: u8,
    /// [`g::TALK_REASON`] (23h).
    pub talk_reason: u8,
    /// [`g::MESSAGE_PARAM`] (24h).
    pub message_param: u8,
    /// [`g::SIETCHES_ENTERED`] (25h).
    pub sietches_entered: u8,
    /// [`g::FIRST_TIME_INSIDE`] (26h).
    pub first_time_inside: u8,
    /// [`g::SIETCHES_HELD`] (27h).
    pub sietches_held: u8,
    /// [`g::JOINED`] (28h).
    pub joined: u8,
    /// [`g::CHARISMA`] (29h).
    pub charisma: u8,
    /// [`g::GAME_PHASE`] (2Ah).
    pub phase: u8,
    /// [`g::HARKONNEN_PLACE`] (2Bh).
    pub harkonnen_place: u8,
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

impl Single for Progress {
    const WHICH: Which = Which::Progress;

    fn from_bytes(b: &[u8]) -> Progress {
        Progress {
            place_kind: b[at(g::PLACE_KIND)],
            place_index: b[at(g::PLACE_INDEX)],
            event_flags: b[at(g::EVENT_FLAGS)],
            room: b[at(g::CURRENT_ROOM)],
            entering_room: b[at(g::ENTERING_ROOM)],
            previous_room: b[at(g::PREVIOUS_ROOM)],
            met: word(b, at(g::MET)),
            party: word(b, at(g::PARTY)),
            present: word(b, at(g::PRESENT)),
            talked_here: word(b, at(g::TALKED_HERE)),
            speaker_time_since: word(b, at(g::SPEAKER_TIME_SINCE)),
            speaker_flags: b[at(g::SPEAKER_FLAGS)],
            speaker_said_line: b[at(g::SPEAKER_SAID_LINE)],
            answer_count: b[at(g::ANSWER_COUNT)],
            requests: b[at(g::REQUESTS)],
            smuggler_flags: b[at(g::SMUGGLER_FLAGS)],
            smuggler_patience: b[at(g::SMUGGLER_PATIENCE)],
            smuggler_days_away: b[at(g::SMUGGLER_DAYS_AWAY)],
            debt_days: b[at(g::DEBT_DAYS)],
            smuggler_debt: word(b, at(g::SMUGGLER_DEBT)),
            debts: b[at(g::DEBTS)],
            talk_reason: b[at(g::TALK_REASON)],
            message_param: b[at(g::MESSAGE_PARAM)],
            sietches_entered: b[at(g::SIETCHES_ENTERED)],
            first_time_inside: b[at(g::FIRST_TIME_INSIDE)],
            sietches_held: b[at(g::SIETCHES_HELD)],
            joined: b[at(g::JOINED)],
            charisma: b[at(g::CHARISMA)],
            phase: b[at(g::GAME_PHASE)],
            harkonnen_place: b[at(g::HARKONNEN_PLACE)],
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        b[at(g::PLACE_KIND)] = self.place_kind;
        b[at(g::PLACE_INDEX)] = self.place_index;
        b[at(g::EVENT_FLAGS)] = self.event_flags;
        b[at(g::CURRENT_ROOM)] = self.room;
        b[at(g::ENTERING_ROOM)] = self.entering_room;
        b[at(g::PREVIOUS_ROOM)] = self.previous_room;
        set_word(&mut b, at(g::MET), self.met);
        set_word(&mut b, at(g::PARTY), self.party);
        set_word(&mut b, at(g::PRESENT), self.present);
        set_word(&mut b, at(g::TALKED_HERE), self.talked_here);
        set_word(&mut b, at(g::SPEAKER_TIME_SINCE), self.speaker_time_since);
        b[at(g::SPEAKER_FLAGS)] = self.speaker_flags;
        b[at(g::SPEAKER_SAID_LINE)] = self.speaker_said_line;
        b[at(g::ANSWER_COUNT)] = self.answer_count;
        b[at(g::REQUESTS)] = self.requests;
        b[at(g::SMUGGLER_FLAGS)] = self.smuggler_flags;
        b[at(g::SMUGGLER_PATIENCE)] = self.smuggler_patience;
        b[at(g::SMUGGLER_DAYS_AWAY)] = self.smuggler_days_away;
        b[at(g::DEBT_DAYS)] = self.debt_days;
        set_word(&mut b, at(g::SMUGGLER_DEBT), self.smuggler_debt);
        b[at(g::DEBTS)] = self.debts;
        b[at(g::TALK_REASON)] = self.talk_reason;
        b[at(g::MESSAGE_PARAM)] = self.message_param;
        b[at(g::SIETCHES_ENTERED)] = self.sietches_entered;
        b[at(g::FIRST_TIME_INSIDE)] = self.first_time_inside;
        b[at(g::SIETCHES_HELD)] = self.sietches_held;
        b[at(g::JOINED)] = self.joined;
        b[at(g::CHARISMA)] = self.charisma;
        b[at(g::GAME_PHASE)] = self.phase;
        b[at(g::HARKONNEN_PLACE)] = self.harkonnen_place;
        b
    }
}

const _: () = assert!(g::PLACE_KIND == 0x08);
const _: () = assert!(g::PLACE_INDEX == 0x09);
const _: () = assert!(g::EVENT_FLAGS == 0x0a);
const _: () = assert!(g::CURRENT_ROOM == 0x0b);
const _: () = assert!(g::ENTERING_ROOM == 0x0c);
const _: () = assert!(g::PREVIOUS_ROOM == 0x0d);
const _: () = assert!(g::MET == 0x0e);
const _: () = assert!(g::PARTY == 0x10);
const _: () = assert!(g::PRESENT == 0x12);
const _: () = assert!(g::TALKED_HERE == 0x14);
const _: () = assert!(g::SPEAKER_TIME_SINCE == 0x16);
const _: () = assert!(g::SPEAKER_FLAGS == 0x18);
const _: () = assert!(g::SPEAKER_SAID_LINE == 0x19);
const _: () = assert!(g::ANSWER_COUNT == 0x1a);
const _: () = assert!(g::REQUESTS == 0x1b);
const _: () = assert!(g::SMUGGLER_FLAGS == 0x1c);
const _: () = assert!(g::SMUGGLER_PATIENCE == 0x1d);
const _: () = assert!(g::SMUGGLER_DAYS_AWAY == 0x1e);
const _: () = assert!(g::DEBT_DAYS == 0x1f);
const _: () = assert!(g::SMUGGLER_DEBT == 0x20);
const _: () = assert!(g::DEBTS == 0x22);
const _: () = assert!(g::TALK_REASON == 0x23);
const _: () = assert!(g::MESSAGE_PARAM == 0x24);
const _: () = assert!(g::SIETCHES_ENTERED == 0x25);
const _: () = assert!(g::FIRST_TIME_INSIDE == 0x26);
const _: () = assert!(g::SIETCHES_HELD == 0x27);
const _: () = assert!(g::JOINED == 0x28);
const _: () = assert!(g::CHARISMA == 0x29);
const _: () = assert!(g::GAME_PHASE == 0x2a);
const _: () = assert!(g::HARKONNEN_PLACE == 0x2b);
const _: () = assert!(AT == 0x08 && (AT + 0x24) as usize >= 0x2c);
