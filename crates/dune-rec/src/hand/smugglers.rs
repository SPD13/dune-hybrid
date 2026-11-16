//! The smugglers as Rust state (Stage B of `docs/readable-port-plan.md`):
//! where each one is, the equipment it has and at what price, and what
//! the player owes it.
//!
//! A table of six records of 11h bytes ([`Which::Smugglers`], at
//! [`g::SMUGGLERS`]) in the generic record tables
//! (`hand/record_table.rs`, the memory kept as the checked mirror);
//! [`Smuggler`] is the view of one. The one being talked to is at
//! [10B4h] (character 13's word 0Ch); `loc_0235f` copies some of its
//! fields to [`g::SMUGGLER_FLAGS`] .. [`g::SMUGGLER_DEBT`] for the
//! dialogue's conditions.

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the table starts in the data segment.
pub const AT: u16 = Which::Smugglers.base();
/// The size of a record.
pub const SIZE: u16 = 0x11;

/// The record of smuggler `i`.
pub const fn record(i: u16) -> u16 {
    AT + SIZE * i
}

/// The smuggler whose record is at `offset`, if it is one.
pub fn index(offset: u16) -> Option<usize> {
    record_table::index(Which::Smugglers, offset)
}

/// How many kinds of equipment there are (the ornithopter, the krys
/// knives, laser guns, weirding modules, atomics; 0E8h + the kind is the
/// command phrase naming it).
pub const KINDS: usize = 5;

/// A smuggler.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Smuggler {
    /// +0: the byte 0 of the location it is at (`loc_02318` finds it by
    /// it).
    pub location: u8,
    /// +1: how many answers it stands haggling before it breaks off
    /// (2483h).
    pub patience: u8,
    /// +2: bits. 8: met; 20h, 40h: how the last talk ended (2538h,
    /// 254Ch), cleared by a purchase.
    pub flags: u8,
    /// +3: the day of the last visit (239Dh).
    pub visited_on: u8,
    /// +4..8: how many of each kind of equipment it has (one less by a
    /// purchase; refilled each day, `loc_01c46`).
    pub stock: [u8; KINDS],
    /// +9..0Dh: each kind's price, halved (the price asked is twice its
    /// low 7 bits, 23CAh); 80h: the kind is sold there (restocked).
    pub prices: [u8; KINDS],
    /// +0Eh: the spice the player owes it (purchases are on credit,
    /// 23F4h; paid at 2517h).
    pub debt: u16,
    /// +10h: the day of the last purchase on credit (2403h).
    pub debt_since: u8,
}

impl Single for Smuggler {
    const WHICH: Which = Which::Smugglers;

    fn from_bytes(b: &[u8]) -> Smuggler {
        let mut stock = [0; KINDS];
        stock.copy_from_slice(&b[4..9]);
        let mut prices = [0; KINDS];
        prices.copy_from_slice(&b[9..0x0e]);
        Smuggler {
            location: b[0],
            patience: b[1],
            flags: b[2],
            visited_on: b[3],
            stock,
            prices,
            debt: b[0x0e] as u16 | (b[0x0f] as u16) << 8,
            debt_since: b[0x10],
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        b[0] = self.location;
        b[1] = self.patience;
        b[2] = self.flags;
        b[3] = self.visited_on;
        b[4..9].copy_from_slice(&self.stock);
        b[9..0x0e].copy_from_slice(&self.prices);
        b[0x0e..0x10].copy_from_slice(&self.debt.to_le_bytes());
        b[0x10] = self.debt_since;
        b
    }
}

const _: () = assert!(g::SMUGGLERS == AT && record(6) < g::CHANCES);
