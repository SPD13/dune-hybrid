//! The flight as Rust state (Stage B of `docs/readable-port-plan.md`):
//! whether the player flies, the map marker's state while the map shows
//! the flight, and its clock (`sub_4f09`, the flight's step;
//! `loc_04b3b`, a step more).
//!
//! A table of one record ([`Which::Flight`], 6 bytes at 4727h) in the
//! generic record tables (`hand/record_table.rs`, the memory kept as the
//! checked mirror); [`Flight`] is its typed view.

use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::Flight.base();

/// The flight.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Flight {
    /// [4727h]: FFh while flying (`loc_04ab8`), 0 when the communicator
    /// calls.
    pub flying: u8,
    /// [4728h]: the marker on the map: 0 shown, 1 hidden for this step,
    /// bit 0 set by a picture screen, negative (80h) the map not drawn
    /// (a scene's video plays).
    pub marker: u8,
    /// [4729h]: the tick the ring of points last moved (every 300h ticks).
    pub last_move: u16,
    /// [472Bh]: the steps flown (every 16th tick one; the map position
    /// follows).
    pub steps: u16,
}

impl Single for Flight {
    const WHICH: Which = Which::Flight;

    fn from_bytes(b: &[u8]) -> Flight {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        Flight { flying: b[0], marker: b[1], last_move: w(2), steps: w(4) }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        b[0] = self.flying;
        b[1] = self.marker;
        b[2..4].copy_from_slice(&self.last_move.to_le_bytes());
        b[4..6].copy_from_slice(&self.steps.to_le_bytes());
        b
    }
}
