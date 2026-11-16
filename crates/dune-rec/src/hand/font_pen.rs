//! The font's pen as Rust state (Stage B of `docs/readable-port-plan.md`):
//! where the next character is drawn and where the line started
//! (`font_set_draw_position`, D04Eh; the character drawing moves it on).
//!
//! A table of one record ([`Which::FontPen`], 8 bytes at D82Ch) in the
//! generic record tables (`hand/record_table.rs`, the memory kept as the
//! checked mirror); [`FontPen`] is its typed view. The character drawing
//! in the generated code writes it often (counted apart).

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::FontPen.base();

/// The font's pen.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct FontPen {
    /// [D82Ch] ([`g::FONT_X`]): where the next character goes (x, y;
    /// Cryogenic: `CharacterXCoord`, `CharacterYCoord`).
    pub at: [u16; 2],
    /// [D830h] ([`g::FONT_LINE_X`]): where the line started (x, y; a new
    /// line goes back to its x).
    pub line: [u16; 2],
}

impl Single for FontPen {
    const WHICH: Which = Which::FontPen;

    fn from_bytes(b: &[u8]) -> FontPen {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        FontPen { at: [w(0), w(2)], line: [w(4), w(6)] }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        for (k, v) in [self.at[0], self.at[1], self.line[0], self.line[1]].into_iter().enumerate() {
            b[2 * k..2 * k + 2].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
}

const _: () = assert!(g::FONT_X == AT && g::FONT_Y == AT + 2 && g::FONT_LINE_X == AT + 4 && g::FONT_LINE_Y == AT + 6);
