//! Where the player is, as Rust state (Stage B of
//! `docs/readable-port-plan.md`): a map position, or a location and its
//! room (`get_map_position`, 407Eh, reads it; the characters keep theirs
//! the same way, `hand/characters.rs`).
//!
//! A table of one record ([`Which::Player`], 4 bytes at DS:4) in the
//! generic record tables (`hand/record_table.rs`, the memory kept as the
//! checked mirror); [`PlayerAt`] is its typed view.

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::Player.base();

/// Where the player is.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct PlayerAt {
    /// [4] ([`g::PLAYER_X`]): the map x; at a location, its low byte the
    /// room (1..) and its high byte the location's kind ([`g::PLACE_KIND`]).
    pub x: u16,
    /// [6] ([`g::PLAYER_Y`]): the map y; 80h in its low byte: at the
    /// location whose index is its high byte.
    pub y: u16,
}

impl PlayerAt {
    /// At a location (y's low byte 80h): its index.
    pub fn location(&self) -> Option<u8> {
        (self.y & 0xff == 0x80).then_some((self.y >> 8) as u8)
    }
}

impl Single for PlayerAt {
    const WHICH: Which = Which::Player;

    fn from_bytes(b: &[u8]) -> PlayerAt {
        PlayerAt { x: b[0] as u16 | (b[1] as u16) << 8, y: b[2] as u16 | (b[3] as u16) << 8 }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        b[0..2].copy_from_slice(&self.x.to_le_bytes());
        b[2..4].copy_from_slice(&self.y.to_le_bytes());
        b
    }
}

const _: () = assert!(g::PLAYER_X == AT && g::PLAYER_Y == AT + 2);
