//! The dialogue's text being played as Rust state (Stage B of
//! `docs/readable-port-plan.md`): which phrase of a talking head's
//! sentence shows, what is left of it, and the head's tables it comes
//! from (`start_dialogue_text`, 9908h; `dialogue_text_step`, 99DDh;
//! `drop_dialogue_text`, 9B8Bh).
//!
//! A table of one record ([`Which::DialogueText`], 0Eh bytes at 47C6h)
//! in the generic record tables (`hand/record_table.rs`, the memory kept
//! as the checked mirror); [`DialogueText`] is its typed view. Code that
//! reads it through SS (the same segment in the game) reads the mirror.

use super::record_table::{self, Which};
use super::{Ctx, DS, globals as g};

/// Where the record starts in the data segment.
pub const AT: u16 = 0x47c6;

/// The dialogue's text being played.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct DialogueText {
    /// [47C6h]: the next phrase to show (in the head's sheet), 0 when no
    /// text plays.
    pub next: u16,
    /// [47C8h]: the phrase shown, 0 for none.
    pub shown: u16,
    /// [47CAh]: the head's sentences (a table in its sheet, set up when
    /// the head's sprites load).
    pub sentences: u16,
    /// [47CCh]: the head's frame table (its lip sync; read through SS).
    pub head_frames: u16,
    /// [47CEh]: the phrases left to show (from 4 × the words laid out; 8
    /// or 14h for some; below 0 when the sentence is over).
    pub left: u16,
    /// [47D0h]: which sentence of a set, 0 for a random one.
    pub sentence_kind: u8,
    /// [47D1h]: bits: 80h playing, 40h shown, 10h counted down.
    pub flags: u8,
    /// [47D2h]: the last of the head's sentence blocks.
    pub sentences_last: u16,
}

impl DialogueText {
    fn from_bytes(b: &[u8]) -> DialogueText {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        DialogueText {
            next: w(0),
            shown: w(2),
            sentences: w(4),
            head_frames: w(6),
            left: w(8),
            sentence_kind: b[0x0a],
            flags: b[0x0b],
            sentences_last: w(0x0c),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = record_table::Bytes::default();
        let mut w = |i: usize, v: u16| {
            b[i] = v as u8;
            b[i + 1] = (v >> 8) as u8;
        };
        w(0, self.next);
        w(2, self.shown);
        w(4, self.sentences);
        w(6, self.head_frames);
        w(8, self.left);
        w(0x0c, self.sentences_last);
        b[0x0a] = self.sentence_kind;
        b[0x0b] = self.flags;
        b
    }

    /// The record's bytes at DS (another data segment than the game's:
    /// not the table's).
    fn raw(c: &mut Ctx) -> DialogueText {
        let mut b = record_table::Bytes::default();
        for (k, v) in b.iter_mut().enumerate().take(Which::DialogueText.size() as usize) {
            *v = c.byte(AT + k as u16);
        }
        DialogueText::from_bytes(&b)
    }
}

const _: () = assert!(g::HEAD_FRAME_TABLE == AT + 6);

/// The dialogue's text, in step with the memory.
pub(super) fn get(c: &mut Ctx) -> DialogueText {
    if c.sreg(DS) != g::GAME_DS {
        return DialogueText::raw(c);
    }
    DialogueText::from_bytes(&record_table::get(c, Which::DialogueText, 0))
}

/// The dialogue's text changed to `t`: kept, and its changed bytes
/// written through.
pub(super) fn set(c: &mut Ctx, t: DialogueText) {
    if c.sreg(DS) != g::GAME_DS {
        let (old, new) = (DialogueText::raw(c).to_bytes(), t.to_bytes());
        for k in 0..Which::DialogueText.size() as usize {
            if old[k] != new[k] {
                c.set_byte(AT + k as u16, new[k]);
            }
        }
        return;
    }
    record_table::set(c, Which::DialogueText, 0, t.to_bytes());
}

/// The dialogue's text changed by `f`.
pub(super) fn update(c: &mut Ctx, f: impl FnOnce(&mut DialogueText)) {
    let mut t = get(c);
    f(&mut t);
    set(c, t);
}
