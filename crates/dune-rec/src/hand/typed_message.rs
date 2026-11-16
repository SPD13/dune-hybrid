//! The message typed out on the message line as Rust state (Stage B of
//! `docs/readable-port-plan.md`): phrase 57h and the like, one
//! character per run of the frame task `sub_46b5` (set up by
//! `loc_04658`, dropped by `loc_0469b`).
//!
//! A table of one record ([`Which::TypedMessage`], 0Ah bytes at 473Fh)
//! in the generic record tables (`hand/record_table.rs`, the memory kept
//! as the checked mirror); [`TypedMessage`] is its typed view.

use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::TypedMessage.base();

/// The message being typed out.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct TypedMessage {
    /// [473Fh]: far pointer (offset, segment) to its next character; an
    /// offset of 0 when none runs.
    pub next: [u16; 2],
    /// [4743h]: where the next character goes (x, y on the screen).
    pub pen: [u16; 2],
    /// [4747h]: its colour.
    pub colour: u16,
}

impl Single for TypedMessage {
    const WHICH: Which = Which::TypedMessage;

    fn from_bytes(b: &[u8]) -> TypedMessage {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        TypedMessage { next: [w(0), w(2)], pen: [w(4), w(6)], colour: w(8) }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        for (k, v) in [self.next[0], self.next[1], self.pen[0], self.pen[1], self.colour].into_iter().enumerate() {
            b[2 * k..2 * k + 2].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
}
