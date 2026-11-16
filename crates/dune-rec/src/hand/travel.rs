//! The player's travel on the map as Rust state (Stage B of
//! `docs/readable-port-plan.md`): where the flight goes and its heading,
//! how it steers, the step's carried fraction, and the screen kind and
//! hold that go with it (`loc_04944` sets a flight up; `loc_051cb`
//! steers it; `loc_05206` takes a step).
//!
//! A table of one record ([`Which::Travel`], 0Dh bytes at 11C1h) in the
//! generic record tables (`hand/record_table.rs`, the memory kept as the
//! checked mirror); [`Travel`] is its typed view.

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::Travel.base();

/// The player's travel.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Travel {
    /// [11C1h]: a map position kept (x, y; compared with the current one).
    pub kept: [u16; 2],
    /// [11C5h]: the location flown to, 0 for none.
    pub target: u16,
    /// [11C7h] ([`g::HEADING`]): the heading, a byte angle (40h a quarter
    /// turn).
    pub heading: u8,
    /// [11C8h] ([`g::BYTE_11C8`]): set when the flight has no location to
    /// home on (it keeps its heading).
    pub no_target: u8,
    /// [11C9h] ([`g::HEAD_ANIMATION_BLOCKED`]): the screen kind (bits 0,
    /// 1: 1 a picture, 2 the game area; 4, 8 set by other views); while
    /// set the talking head does not animate up.
    pub screen_kind: u8,
    /// [11CAh]: set while the travel is held (a call on the communicator,
    /// a dialogue); the flight's step waits.
    pub held: u8,
    /// [11CBh]: nonzero keeps the heading (no steering to the target;
    /// counted down by `loc_04944` for a far target).
    pub keep_heading: u8,
    /// [11CCh]: the step's carried fraction (80h at a new heading).
    pub carry: u16,
}

impl Single for Travel {
    const WHICH: Which = Which::Travel;

    fn from_bytes(b: &[u8]) -> Travel {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        Travel {
            kept: [w(0), w(2)],
            target: w(4),
            heading: b[6],
            no_target: b[7],
            screen_kind: b[8],
            held: b[9],
            keep_heading: b[0x0a],
            carry: w(0x0b),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        for (i, v) in [(0, self.kept[0]), (2, self.kept[1]), (4, self.target), (0x0b, self.carry)] {
            b[i..i + 2].copy_from_slice(&v.to_le_bytes());
        }
        b[6] = self.heading;
        b[7] = self.no_target;
        b[8] = self.screen_kind;
        b[9] = self.held;
        b[0x0a] = self.keep_heading;
        b
    }
}

const _: () = assert!(g::HEADING == AT + 6 && g::BYTE_11C8 == AT + 7 && g::HEAD_ANIMATION_BLOCKED == AT + 8);
