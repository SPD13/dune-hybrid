//! The messages kept, as Rust state (Stage B of
//! `docs/readable-port-plan.md`).
//!
//! The game keeps the last ten messages it told (an event's phrase), the
//! oldest first and the newest last: `add_message` (26DAh) adds one
//! unless it is there, dropping the oldest when full
//! (`drop_oldest_message`, which moves them down with `rep movsw`, in the
//! memory, as the original; the table follows those writes). Their count
//! is the economy's [`message_count`](super::economy::Economy).
//!
//! A table of one record ([`Which::Messages`], 14h bytes at DS:1179h) in
//! the generic record tables (`hand/record_table.rs`, the memory kept as
//! the checked mirror); [`Messages`] is its typed view.

use super::Ctx;
use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::Messages.base();

/// The most messages kept.
pub const MAX_MESSAGES: usize = 10;

/// The messages kept.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Messages {
    /// From [`g::MESSAGES`] (1179h): each a phrase, the oldest first.
    pub list: [u16; MAX_MESSAGES],
}

/// Message `i` kept (from 0, the oldest). Past the tenth (a count the
/// game never makes) the word where the code would read it.
pub(super) fn entry(c: &mut Ctx, i: u16) -> u16 {
    if (i as usize) < MAX_MESSAGES {
        return c.messages().list[i as usize];
    }
    c.word(g::MESSAGES + 2 * i)
}

/// Message `i` set to `v` (past the tenth, the word where the code would
/// write it).
pub(super) fn set_entry(c: &mut Ctx, i: u16, v: u16) {
    if (i as usize) < MAX_MESSAGES {
        c.update_messages(|t| t.list[i as usize] = v);
    } else {
        c.set_word(g::MESSAGES + 2 * i, v);
    }
}

impl Single for Messages {
    const WHICH: Which = Which::Messages;

    fn from_bytes(b: &[u8]) -> Messages {
        let mut list = [0; MAX_MESSAGES];
        for (i, v) in list.iter_mut().enumerate() {
            *v = b[2 * i] as u16 | (b[2 * i + 1] as u16) << 8;
        }
        Messages { list }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        for (i, v) in self.list.iter().enumerate() {
            b[2 * i..2 * i + 2].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
}

const _: () = assert!(g::MESSAGES == 0x1179);
const _: () = assert!(AT == g::MESSAGES && 2 * MAX_MESSAGES == 0x14);
