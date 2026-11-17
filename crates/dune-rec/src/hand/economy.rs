//! The spice, the smugglers' prices, the Emperor's demands, as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! The spice in stock and its daily figures, the smuggler's price and
//! mood, the answer given, the map's and the armies' statistics, the
//! Emperor's demands and the shipment; with them the endgame's stage,
//! the book and the messages.
//!
//! Part of the story state (DS:0..FFh, the variables the condition
//! scripts test by their offset): a table of one record
//! ([`Which::Economy`], 2Dh bytes at DS:9Dh) in the generic record tables
//! (`hand/record_table.rs`, the memory kept as the checked mirror);
//! [`Economy`] is its typed view, each field at its global's offset
//! ([`super::globals`] says what each one is).

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::Economy.base();

/// The spice, the smugglers' prices, the Emperor's demands.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Economy {
    /// [`g::SMUGGLER_PRICE`] (9Dh).
    pub smuggler_price: u8,
    /// [`g::SMUGGLER_MOOD`] (9Eh).
    pub smuggler_mood: u8,
    /// [`g::ANSWER`] (9Fh).
    pub answer: u8,
    /// [`g::SPICE`] (A0h).
    pub spice: u16,
    /// [`g::MAP_PERCENT_10_20`] (A2h).
    pub map_percent_10_20: u16,
    /// [`g::MAP_PERCENT_30`] (A4h).
    pub map_percent_30: u16,
    /// [`g::SPICE_INCOME`] (A6h).
    pub income: u16,
    /// [`g::FIELD_SPICE_ESTIMATE`] (A8h).
    pub field_spice_estimate: u16,
    /// [`g::ARMY_SIZE`] (AAh).
    pub army_size: u16,
    /// [`g::HARKONNEN_ARMY_SIZE`] (ACh).
    pub harkonnen_army_size: u16,
    /// [`g::SPICE_INCOME_BEFORE`] (AEh).
    pub income_before: u16,
    /// [`g::SPICE_INCOME_RISE`] (B0h).
    pub income_rise: u16,
    /// [`g::SPICE_INCOME_FALL`] (B2h).
    pub income_fall: u16,
    /// [`g::SHIPMENT_OFFERS`] (B4h).
    pub shipment_offers: [u16; 4],
    /// [`g::SPICE_DEMANDED`] (BCh).
    pub demanded: u16,
    /// [`g::SHIPMENT_RATIO`] (BEh).
    pub shipment_ratio: u8,
    /// [`g::SHIPMENT_FLAGS`] (BFh).
    pub shipment_flags: u8,
    /// [`g::SHIPMENT_CHOSEN`] (C0h).
    pub shipment_chosen: u16,
    /// [`g::ENDGAME_STAGE`] (C2h).
    pub endgame_stage: u8,
    /// [`g::DEMANDS`] (C3h).
    pub demands: u8,
    /// [`g::HARKONNEN_TAKEOVERS`] (C4h).
    pub harkonnen_takeovers: u8,
    /// [`g::ROOM_SEAT_SHIFT`] (C5h).
    pub room_seat_shift: u8,
    /// [`g::BOOK`] (C6h).
    pub book: u8,
    /// C7h: no code uses it (a script may); kept as it is.
    pub byte_c7: u8,
    /// [`g::MESSAGE_COUNT`] (C8h).
    pub message_count: u8,
    /// [`g::UNREAD_MESSAGES`] (C9h).
    pub unread_messages: u8,
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

impl Single for Economy {
    const WHICH: Which = Which::Economy;

    fn from_bytes(b: &[u8]) -> Economy {
        Economy {
            smuggler_price: b[at(g::SMUGGLER_PRICE)],
            smuggler_mood: b[at(g::SMUGGLER_MOOD)],
            answer: b[at(g::ANSWER)],
            spice: word(b, at(g::SPICE)),
            map_percent_10_20: word(b, at(g::MAP_PERCENT_10_20)),
            map_percent_30: word(b, at(g::MAP_PERCENT_30)),
            income: word(b, at(g::SPICE_INCOME)),
            field_spice_estimate: word(b, at(g::FIELD_SPICE_ESTIMATE)),
            army_size: word(b, at(g::ARMY_SIZE)),
            harkonnen_army_size: word(b, at(g::HARKONNEN_ARMY_SIZE)),
            income_before: word(b, at(g::SPICE_INCOME_BEFORE)),
            income_rise: word(b, at(g::SPICE_INCOME_RISE)),
            income_fall: word(b, at(g::SPICE_INCOME_FALL)),
            shipment_offers: std::array::from_fn(|k| word(b, at(g::SHIPMENT_OFFERS) + 2 * k)),
            demanded: word(b, at(g::SPICE_DEMANDED)),
            shipment_ratio: b[at(g::SHIPMENT_RATIO)],
            shipment_flags: b[at(g::SHIPMENT_FLAGS)],
            shipment_chosen: word(b, at(g::SHIPMENT_CHOSEN)),
            endgame_stage: b[at(g::ENDGAME_STAGE)],
            demands: b[at(g::DEMANDS)],
            harkonnen_takeovers: b[at(g::HARKONNEN_TAKEOVERS)],
            room_seat_shift: b[at(g::ROOM_SEAT_SHIFT)],
            book: b[at(g::BOOK)],
            byte_c7: b[(0xc7 - AT) as usize],
            message_count: b[at(g::MESSAGE_COUNT)],
            unread_messages: b[at(g::UNREAD_MESSAGES)],
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        b[at(g::SMUGGLER_PRICE)] = self.smuggler_price;
        b[at(g::SMUGGLER_MOOD)] = self.smuggler_mood;
        b[at(g::ANSWER)] = self.answer;
        set_word(&mut b, at(g::SPICE), self.spice);
        set_word(&mut b, at(g::MAP_PERCENT_10_20), self.map_percent_10_20);
        set_word(&mut b, at(g::MAP_PERCENT_30), self.map_percent_30);
        set_word(&mut b, at(g::SPICE_INCOME), self.income);
        set_word(&mut b, at(g::FIELD_SPICE_ESTIMATE), self.field_spice_estimate);
        set_word(&mut b, at(g::ARMY_SIZE), self.army_size);
        set_word(&mut b, at(g::HARKONNEN_ARMY_SIZE), self.harkonnen_army_size);
        set_word(&mut b, at(g::SPICE_INCOME_BEFORE), self.income_before);
        set_word(&mut b, at(g::SPICE_INCOME_RISE), self.income_rise);
        set_word(&mut b, at(g::SPICE_INCOME_FALL), self.income_fall);
        for (k, v) in self.shipment_offers.iter().enumerate() {
            set_word(&mut b, at(g::SHIPMENT_OFFERS) + 2 * k, *v);
        }
        set_word(&mut b, at(g::SPICE_DEMANDED), self.demanded);
        b[at(g::SHIPMENT_RATIO)] = self.shipment_ratio;
        b[at(g::SHIPMENT_FLAGS)] = self.shipment_flags;
        set_word(&mut b, at(g::SHIPMENT_CHOSEN), self.shipment_chosen);
        b[at(g::ENDGAME_STAGE)] = self.endgame_stage;
        b[at(g::DEMANDS)] = self.demands;
        b[at(g::HARKONNEN_TAKEOVERS)] = self.harkonnen_takeovers;
        b[at(g::ROOM_SEAT_SHIFT)] = self.room_seat_shift;
        b[at(g::BOOK)] = self.book;
        b[(0xc7 - AT) as usize] = self.byte_c7;
        b[at(g::MESSAGE_COUNT)] = self.message_count;
        b[at(g::UNREAD_MESSAGES)] = self.unread_messages;
        b
    }
}

const _: () = assert!(g::SMUGGLER_PRICE == 0x9d);
const _: () = assert!(g::SMUGGLER_MOOD == 0x9e);
const _: () = assert!(g::ANSWER == 0x9f);
const _: () = assert!(g::SPICE == 0xa0);
const _: () = assert!(g::MAP_PERCENT_10_20 == 0xa2);
const _: () = assert!(g::MAP_PERCENT_30 == 0xa4);
const _: () = assert!(g::SPICE_INCOME == 0xa6);
const _: () = assert!(g::FIELD_SPICE_ESTIMATE == 0xa8);
const _: () = assert!(g::ARMY_SIZE == 0xaa);
const _: () = assert!(g::HARKONNEN_ARMY_SIZE == 0xac);
const _: () = assert!(g::SPICE_INCOME_BEFORE == 0xae);
const _: () = assert!(g::SPICE_INCOME_RISE == 0xb0);
const _: () = assert!(g::SPICE_INCOME_FALL == 0xb2);
const _: () = assert!(g::SHIPMENT_OFFERS == 0xb4);
const _: () = assert!(g::SPICE_DEMANDED == 0xbc);
const _: () = assert!(g::SHIPMENT_RATIO == 0xbe);
const _: () = assert!(g::SHIPMENT_FLAGS == 0xbf);
const _: () = assert!(g::SHIPMENT_CHOSEN == 0xc0);
const _: () = assert!(g::ENDGAME_STAGE == 0xc2);
const _: () = assert!(g::DEMANDS == 0xc3);
const _: () = assert!(g::HARKONNEN_TAKEOVERS == 0xc4);
const _: () = assert!(g::ROOM_SEAT_SHIFT == 0xc5);
const _: () = assert!(g::BOOK == 0xc6);
const _: () = assert!(g::MESSAGE_COUNT == 0xc8);
const _: () = assert!(g::UNREAD_MESSAGES == 0xc9);
const _: () = assert!(AT == 0x9d && (AT + 0x2d) as usize >= 0xca);
