//! The places near, the desert, the messages being read, the attacks, as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! The nearest places to the one talked about (`nearest_places`, five
//! slots, with other variables in the bytes between them), the desert
//! walk, the message being read, the talking head's step, the leader
//! met, the Harkonnens' attacks and the days in this phase.
//!
//! Part of the story state (DS:0..FFh, the variables the condition
//! scripts test by their offset): a table of one record
//! ([`Which::Situation`], 36h bytes at DS:CAh) in the generic record tables
//! (`hand/record_table.rs`, the memory kept as the checked mirror);
//! [`Situation`] is its typed view, each field at its global's offset
//! ([`super::globals`] says what each one is).

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::Situation.base();

/// A slot of [`nearest_places`](super::map::nearest_places): 5 bytes.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Nearest {
    /// +0: the distance (the larger of the x distance's high byte and the
    /// y distance).
    pub distance: u16,
    /// +2: the location's record (FFFFh: none).
    pub record: u16,
    /// +4: the direction, 0..7.
    pub direction: u8,
}

/// The places near, the desert, the messages being read, the attacks.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Situation {
    /// [`g::NEAREST_PLACE`] (CAh).
    pub nearest: Nearest,
    /// [`g::SHIPMENT_DAYS_LEFT`] (CFh).
    pub shipment_days_left: u8,
    /// [`g::NEAREST_LOW_MAPPED`] (D0h).
    pub nearest_low_mapped: Nearest,
    /// [`g::CHARACTER_1_DAYS`] (D5h).
    pub character_1_days: u8,
    /// [`g::NEAREST_LOW_UNMAPPED`] (D6h).
    pub nearest_low_unmapped: Nearest,
    /// [`g::MESSAGES_READ_LIST`] (DBh).
    pub messages_read_list: u8,
    /// [`g::NEAREST_HIGH_MAPPED`] (DCh).
    pub nearest_high_mapped: Nearest,
    /// [`g::SIGHTING_SIDE`] (E1h).
    pub sighting_side: u8,
    /// [`g::NEAREST_HIGH_UNMAPPED`] (E2h).
    pub nearest_high_unmapped: Nearest,
    /// [`g::DESERT_RESCUES`] (E7h).
    pub desert_rescues: u8,
    /// [`g::HEAD_ANIMATION_STEP`] (E8h).
    pub head_animation_step: u8,
    /// [`g::MESSAGE_SUBJECT`] (E9h).
    pub message_subject: u8,
    /// [`g::EVENT_KIND`] (EAh).
    pub event_kind: u8,
    /// [`g::MESSAGE_NUMBER`] (EBh).
    pub message_number: u8,
    /// [`g::JOB10_COUNTER`] (ECh).
    pub job10_counter: u8,
    /// [`g::LEADER_MOTIVATION`] (EDh).
    pub leader_motivation: u8,
    /// [`g::LEADER_ASKED`] (EEh).
    pub leader_asked: u16,
    /// [`g::SENTENCE_BANK`] (F0h).
    pub sentence_bank: u16,
    /// [`g::EVENT_PLACE_NAME`] (F2h).
    pub event_place_name: u16,
    /// [`g::DESERT_FATIGUE`] (F4h).
    pub desert_fatigue: u8,
    /// [`g::DESERT_REMARKS`] (F5h).
    pub desert_remarks: u8,
    /// [`g::MARCH_SHOWN`] (F6h).
    pub march_shown: u8,
    /// [`g::TOPIC_PLACE_CHARACTERS`] (F7h).
    pub place_characters: u8,
    /// [`g::ATTACKS_PENDING`] (F8h).
    pub attacks_pending: u8,
    /// [`g::ATTACK_CLOCK`] (F9h).
    pub attack_clock: u8,
    /// [`g::TROOPS_RECHECK`] (FAh).
    pub troops_recheck: u8,
    /// [`g::MAP_INTERFACE`] (FBh).
    pub map_interface: u8,
    /// FCh: no code uses it (a script may); kept as it is.
    pub byte_fc: u8,
    /// [`g::BATTLE_BALANCE`] (FDh).
    pub battle_balance: u8,
    /// [`g::LAST_DAY_PHASE`] (FEh).
    pub last_day_phase: u8,
    /// [`g::DAYS_IN_PHASE`] (FFh).
    pub days_in_phase: u8,
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

impl Single for Situation {
    const WHICH: Which = Which::Situation;

    fn from_bytes(b: &[u8]) -> Situation {
        let near = |i: usize| Nearest { distance: word(b, i), record: word(b, i + 2), direction: b[i + 4] };
        Situation {
            nearest: near(at(g::NEAREST_PLACE)),
            shipment_days_left: b[at(g::SHIPMENT_DAYS_LEFT)],
            nearest_low_mapped: near(at(g::NEAREST_LOW_MAPPED)),
            character_1_days: b[at(g::CHARACTER_1_DAYS)],
            nearest_low_unmapped: near(at(g::NEAREST_LOW_UNMAPPED)),
            messages_read_list: b[at(g::MESSAGES_READ_LIST)],
            nearest_high_mapped: near(at(g::NEAREST_HIGH_MAPPED)),
            sighting_side: b[at(g::SIGHTING_SIDE)],
            nearest_high_unmapped: near(at(g::NEAREST_HIGH_UNMAPPED)),
            desert_rescues: b[at(g::DESERT_RESCUES)],
            head_animation_step: b[at(g::HEAD_ANIMATION_STEP)],
            message_subject: b[at(g::MESSAGE_SUBJECT)],
            event_kind: b[at(g::EVENT_KIND)],
            message_number: b[at(g::MESSAGE_NUMBER)],
            job10_counter: b[at(g::JOB10_COUNTER)],
            leader_motivation: b[at(g::LEADER_MOTIVATION)],
            leader_asked: word(b, at(g::LEADER_ASKED)),
            sentence_bank: word(b, at(g::SENTENCE_BANK)),
            event_place_name: word(b, at(g::EVENT_PLACE_NAME)),
            desert_fatigue: b[at(g::DESERT_FATIGUE)],
            desert_remarks: b[at(g::DESERT_REMARKS)],
            march_shown: b[at(g::MARCH_SHOWN)],
            place_characters: b[at(g::TOPIC_PLACE_CHARACTERS)],
            attacks_pending: b[at(g::ATTACKS_PENDING)],
            attack_clock: b[at(g::ATTACK_CLOCK)],
            troops_recheck: b[at(g::TROOPS_RECHECK)],
            map_interface: b[at(g::MAP_INTERFACE)],
            byte_fc: b[(0xfc - AT) as usize],
            battle_balance: b[at(g::BATTLE_BALANCE)],
            last_day_phase: b[at(g::LAST_DAY_PHASE)],
            days_in_phase: b[at(g::DAYS_IN_PHASE)],
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        set_word(&mut b, at(g::NEAREST_PLACE), self.nearest.distance);
        set_word(&mut b, at(g::NEAREST_PLACE) + 2, self.nearest.record);
        b[at(g::NEAREST_PLACE) + 4] = self.nearest.direction;
        b[at(g::SHIPMENT_DAYS_LEFT)] = self.shipment_days_left;
        set_word(&mut b, at(g::NEAREST_LOW_MAPPED), self.nearest_low_mapped.distance);
        set_word(&mut b, at(g::NEAREST_LOW_MAPPED) + 2, self.nearest_low_mapped.record);
        b[at(g::NEAREST_LOW_MAPPED) + 4] = self.nearest_low_mapped.direction;
        b[at(g::CHARACTER_1_DAYS)] = self.character_1_days;
        set_word(&mut b, at(g::NEAREST_LOW_UNMAPPED), self.nearest_low_unmapped.distance);
        set_word(&mut b, at(g::NEAREST_LOW_UNMAPPED) + 2, self.nearest_low_unmapped.record);
        b[at(g::NEAREST_LOW_UNMAPPED) + 4] = self.nearest_low_unmapped.direction;
        b[at(g::MESSAGES_READ_LIST)] = self.messages_read_list;
        set_word(&mut b, at(g::NEAREST_HIGH_MAPPED), self.nearest_high_mapped.distance);
        set_word(&mut b, at(g::NEAREST_HIGH_MAPPED) + 2, self.nearest_high_mapped.record);
        b[at(g::NEAREST_HIGH_MAPPED) + 4] = self.nearest_high_mapped.direction;
        b[at(g::SIGHTING_SIDE)] = self.sighting_side;
        set_word(&mut b, at(g::NEAREST_HIGH_UNMAPPED), self.nearest_high_unmapped.distance);
        set_word(&mut b, at(g::NEAREST_HIGH_UNMAPPED) + 2, self.nearest_high_unmapped.record);
        b[at(g::NEAREST_HIGH_UNMAPPED) + 4] = self.nearest_high_unmapped.direction;
        b[at(g::DESERT_RESCUES)] = self.desert_rescues;
        b[at(g::HEAD_ANIMATION_STEP)] = self.head_animation_step;
        b[at(g::MESSAGE_SUBJECT)] = self.message_subject;
        b[at(g::EVENT_KIND)] = self.event_kind;
        b[at(g::MESSAGE_NUMBER)] = self.message_number;
        b[at(g::JOB10_COUNTER)] = self.job10_counter;
        b[at(g::LEADER_MOTIVATION)] = self.leader_motivation;
        set_word(&mut b, at(g::LEADER_ASKED), self.leader_asked);
        set_word(&mut b, at(g::SENTENCE_BANK), self.sentence_bank);
        set_word(&mut b, at(g::EVENT_PLACE_NAME), self.event_place_name);
        b[at(g::DESERT_FATIGUE)] = self.desert_fatigue;
        b[at(g::DESERT_REMARKS)] = self.desert_remarks;
        b[at(g::MARCH_SHOWN)] = self.march_shown;
        b[at(g::TOPIC_PLACE_CHARACTERS)] = self.place_characters;
        b[at(g::ATTACKS_PENDING)] = self.attacks_pending;
        b[at(g::ATTACK_CLOCK)] = self.attack_clock;
        b[at(g::TROOPS_RECHECK)] = self.troops_recheck;
        b[at(g::MAP_INTERFACE)] = self.map_interface;
        b[(0xfc - AT) as usize] = self.byte_fc;
        b[at(g::BATTLE_BALANCE)] = self.battle_balance;
        b[at(g::LAST_DAY_PHASE)] = self.last_day_phase;
        b[at(g::DAYS_IN_PHASE)] = self.days_in_phase;
        b
    }
}

const _: () = assert!(g::NEAREST_PLACE == 0xca);
const _: () = assert!(g::SHIPMENT_DAYS_LEFT == 0xcf);
const _: () = assert!(g::NEAREST_LOW_MAPPED == 0xd0);
const _: () = assert!(g::CHARACTER_1_DAYS == 0xd5);
const _: () = assert!(g::NEAREST_LOW_UNMAPPED == 0xd6);
const _: () = assert!(g::MESSAGES_READ_LIST == 0xdb);
const _: () = assert!(g::NEAREST_HIGH_MAPPED == 0xdc);
const _: () = assert!(g::SIGHTING_SIDE == 0xe1);
const _: () = assert!(g::NEAREST_HIGH_UNMAPPED == 0xe2);
const _: () = assert!(g::DESERT_RESCUES == 0xe7);
const _: () = assert!(g::HEAD_ANIMATION_STEP == 0xe8);
const _: () = assert!(g::MESSAGE_SUBJECT == 0xe9);
const _: () = assert!(g::EVENT_KIND == 0xea);
const _: () = assert!(g::MESSAGE_NUMBER == 0xeb);
const _: () = assert!(g::JOB10_COUNTER == 0xec);
const _: () = assert!(g::LEADER_MOTIVATION == 0xed);
const _: () = assert!(g::LEADER_ASKED == 0xee);
const _: () = assert!(g::SENTENCE_BANK == 0xf0);
const _: () = assert!(g::EVENT_PLACE_NAME == 0xf2);
const _: () = assert!(g::DESERT_FATIGUE == 0xf4);
const _: () = assert!(g::DESERT_REMARKS == 0xf5);
const _: () = assert!(g::MARCH_SHOWN == 0xf6);
const _: () = assert!(g::TOPIC_PLACE_CHARACTERS == 0xf7);
const _: () = assert!(g::ATTACKS_PENDING == 0xf8);
const _: () = assert!(g::ATTACK_CLOCK == 0xf9);
const _: () = assert!(g::TROOPS_RECHECK == 0xfa);
const _: () = assert!(g::MAP_INTERFACE == 0xfb);
const _: () = assert!(g::BATTLE_BALANCE == 0xfd);
const _: () = assert!(g::LAST_DAY_PHASE == 0xfe);
const _: () = assert!(g::DAYS_IN_PHASE == 0xff);
const _: () = assert!(AT == 0xca && (AT + 0x36) as usize >= 0x100);
