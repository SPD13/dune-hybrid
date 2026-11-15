//! The text box's layout as Rust state (Stage B of
//! `docs/readable-port-plan.md`): the box the dialogue's and the
//! book's text is laid out in (`draw_speech_bubble` sets it up, 8F28h;
//! `layout_text` and `break_lines` fill it).
//!
//! It is a table of one record ([`Which::TextBox`], 1Ch bytes at 4784h)
//! in the generic record tables (`hand/record_table.rs`, which keep the
//! memory as the checked mirror); [`TextBox`] is its typed view.

use super::record_table::{self, Which};
use super::{Ctx, DS, globals as g};

/// Where the record starts in the data segment.
pub const AT: u16 = 0x4784;

/// The text box's layout.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct TextBox {
    /// [4784h]..: the margins inside the box: left, right, top, bottom.
    pub margins: [u16; 4],
    /// [478Ch]: the words laid out so far (`break_lines` counts them;
    /// the text's reveal starts from 4 × this). Some screens set it
    /// themselves (1, 1Eh).
    pub words: u8,
    /// [478Dh]: the room left below the pen (the inner height, less each
    /// line's height as it is placed).
    pub room: u16,
    /// [478Fh]: the inner width (the box's, less the side margins).
    pub width: u16,
    /// [4791h], [4793h]: the pen, where the next line starts (x, y).
    pub pen: [u16; 2],
    /// [4795h], [4797h]: the inner top left corner (where the pen
    /// started).
    pub origin: [u16; 2],
    /// [4799h]: how the lines are placed: bit 0 spreads a line's words
    /// to the width, bit 1 centres each line, bits 2-3 spread the lines
    /// over the room (4) or centre the block (8, 0Ch).
    pub style: u8,
    /// [479Ah]: the step from line to line.
    pub line_height: u16,
    /// [479Ch]: the gaps of the current line still one pixel wider (what
    /// the spread's division left).
    pub wider_gaps: u16,
    /// [479Eh]: the box shown: its rectangle's offset (x, y, width,
    /// height), 0 for none ([`g::BOX_SHOWN`]).
    pub shown: u16,
}

impl TextBox {
    fn from_bytes(b: &[u8]) -> TextBox {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        TextBox {
            margins: [w(0), w(2), w(4), w(6)],
            words: b[8],
            room: w(9),
            width: w(0x0b),
            pen: [w(0x0d), w(0x0f)],
            origin: [w(0x11), w(0x13)],
            style: b[0x15],
            line_height: w(0x16),
            wider_gaps: w(0x18),
            shown: w(0x1a),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        let mut w = |i: usize, v: u16| {
            b[i] = v as u8;
            b[i + 1] = (v >> 8) as u8;
        };
        for (k, &m) in self.margins.iter().enumerate() {
            w(2 * k, m);
        }
        w(9, self.room);
        w(0x0b, self.width);
        w(0x0d, self.pen[0]);
        w(0x0f, self.pen[1]);
        w(0x11, self.origin[0]);
        w(0x13, self.origin[1]);
        w(0x16, self.line_height);
        w(0x18, self.wider_gaps);
        w(0x1a, self.shown);
        b[8] = self.words;
        b[0x15] = self.style;
        b
    }

    /// The record's bytes in the memory at DS (another data segment than
    /// the game's: not the table's).
    fn raw(c: &mut Ctx) -> TextBox {
        let mut b = [0; record_table::MAX];
        for (k, v) in b.iter_mut().enumerate() {
            *v = c.byte(AT + k as u16);
        }
        TextBox::from_bytes(&b)
    }
}

const _: () = assert!(g::BOX_SHOWN == AT + 0x1a);

/// The text box, in step with the memory.
pub(super) fn get(c: &mut Ctx) -> TextBox {
    if c.sreg(DS) != g::GAME_DS {
        return TextBox::raw(c);
    }
    TextBox::from_bytes(&record_table::get(c, Which::TextBox, 0))
}

/// The text box changed to `t`: kept, and its changed bytes written
/// through.
pub(super) fn set(c: &mut Ctx, t: TextBox) {
    if c.sreg(DS) != g::GAME_DS {
        let (old, new) = (TextBox::raw(c).to_bytes(), t.to_bytes());
        for k in 0..Which::TextBox.size() as usize {
            if old[k] != new[k] {
                c.set_byte(AT + k as u16, new[k]);
            }
        }
        return;
    }
    record_table::set(c, Which::TextBox, 0, t.to_bytes());
}

/// The text box changed by `f`.
pub(super) fn update(c: &mut Ctx, f: impl FnOnce(&mut TextBox)) {
    let mut t = get(c);
    f(&mut t);
    set(c, t);
}
