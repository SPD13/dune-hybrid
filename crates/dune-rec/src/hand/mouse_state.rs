//! The mouse and its cursor as Rust state (Stage B of
//! `docs/readable-port-plan.md`): the buttons and the position the
//! mouse handler notes, the range it is held in, and where and whether
//! the cursor is drawn (`hand/mouse.rs`, `hand/cursor.rs`).
//!
//! A table of one record ([`Which::MouseState`], 14h bytes at DC34h) in
//! the generic record tables (`hand/record_table.rs`, the memory kept as
//! the checked mirror); [`MouseState`] is its typed view. The mouse
//! handler runs from the driver's callback, in the generated code or a
//! port: its writes are counted like any other.

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::MouseState.base();

/// The mouse and its cursor.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct MouseState {
    /// [DC34h] ([`g::MOUSE_BUTTONS`]): the buttons down (bits 0, 1), as
    /// the handler last saw them.
    pub buttons: u8,
    /// [DC35h] ([`g::MOUSE_BUTTONS_SEEN`]): the buttons as `mouse_stuff`
    /// last returned them (what changed since is a click).
    pub buttons_seen: u8,
    /// [DC36h] ([`g::MOUSE_X`]): the position.
    pub x: u16,
    /// [DC38h] ([`g::MOUSE_Y`]).
    pub y: u16,
    /// [DC3Ah] ([`g::MOUSE_RANGE`]): the range it is held in: left, top,
    /// right, bottom (`define_mouse_range`).
    pub range: [u16; 4],
    /// [DC42h] ([`g::CURSOR_X`]): where the cursor was drawn (x, y).
    pub cursor: [u16; 2],
    /// [DC46h] ([`g::CURSOR_SHOWN`]): 0 while the cursor is drawn,
    /// negative while hidden (each hide one lower).
    pub cursor_shown: u8,
    /// [DC47h] ([`g::CURSOR_HIDDEN_FOR_RECT`]): negative while it is
    /// hidden for a rectangle being drawn.
    pub hidden_for_rect: u8,
}

impl MouseState {
    /// The buttons and the buttons seen as the word the original loads.
    pub fn buttons_word(&self) -> u16 {
        self.buttons as u16 | (self.buttons_seen as u16) << 8
    }
}

impl Single for MouseState {
    const WHICH: Which = Which::MouseState;

    fn from_bytes(b: &[u8]) -> MouseState {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        MouseState {
            buttons: b[0],
            buttons_seen: b[1],
            x: w(2),
            y: w(4),
            range: [w(6), w(8), w(0x0a), w(0x0c)],
            cursor: [w(0x0e), w(0x10)],
            cursor_shown: b[0x12],
            hidden_for_rect: b[0x13],
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        b[0] = self.buttons;
        b[1] = self.buttons_seen;
        let words = [self.x, self.y, self.range[0], self.range[1], self.range[2], self.range[3], self.cursor[0], self.cursor[1]];
        for (k, v) in words.into_iter().enumerate() {
            b[2 + 2 * k..4 + 2 * k].copy_from_slice(&v.to_le_bytes());
        }
        b[0x12] = self.cursor_shown;
        b[0x13] = self.hidden_for_rect;
        b
    }
}

const _: () = assert!(g::MOUSE_BUTTONS == AT && g::MOUSE_BUTTONS_SEEN == AT + 1);
const _: () = assert!(g::MOUSE_X == AT + 2 && g::MOUSE_Y == AT + 4 && g::MOUSE_RANGE == AT + 6);
const _: () = assert!(g::CURSOR_X == AT + 0x0e && g::CURSOR_Y == AT + 0x10);
const _: () = assert!(g::CURSOR_SHOWN == AT + 0x12 && g::CURSOR_HIDDEN_FOR_RECT == AT + 0x13);
