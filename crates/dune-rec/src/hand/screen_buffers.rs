//! The screen buffers as Rust state (Stage B of
//! `docs/readable-port-plan.md`): which segments the drawing goes to
//! (`set_fb1_as_active_framebuffer`, C07Ch; `set_screen_as_active_
//! framebuffer`, C08Eh; `gfx_call_bp_with_front_buffer_as_screen`,
//! C097h), set up at the start (E5F2h-E602h: the screen from the video
//! driver, the front and second buffers from the bump allocator).
//!
//! A table of one record ([`Which::ScreenBuffers`], 0Ch bytes at DBD4h)
//! in the generic record tables (`hand/record_table.rs`, the memory kept
//! as the checked mirror); [`ScreenBuffers`] is its typed view. The back
//! buffer ([DC32h]) is apart, not in it.

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::ScreenBuffers.base();

/// The screen buffers.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct ScreenBuffers {
    /// [DBD4h]: the front buffer's offset (from the allocator, when it
    /// had to give one).
    pub front_offset: u16,
    /// [DBD6h] ([`g::FRAMEBUFFER_FRONT`]): the front buffer's segment
    /// (Cryogenic: `framebufferFront`).
    pub front: u16,
    /// [DBD8h] ([`g::SCREEN_BUFFER`]): the screen's segment (the video
    /// memory, from the driver; Cryogenic: `screenBuffer`).
    pub screen: u16,
    /// [DBDAh] ([`g::FRAMEBUFFER_ACTIVE`]): the segment drawn to: one of
    /// the others or the back buffer (Cryogenic: `framebufferActive`).
    pub active: u16,
    /// [DBDCh]: the second buffer's offset.
    pub second_offset: u16,
    /// [DBDEh] ([`g::FRAMEBUFFER_2`]): the second buffer's segment
    /// (Cryogenic: `framebuffer2Segment`).
    pub second: u16,
}

impl Single for ScreenBuffers {
    const WHICH: Which = Which::ScreenBuffers;

    fn from_bytes(b: &[u8]) -> ScreenBuffers {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        ScreenBuffers {
            front_offset: w(0),
            front: w(2),
            screen: w(4),
            active: w(6),
            second_offset: w(8),
            second: w(0x0a),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        let words = [self.front_offset, self.front, self.screen, self.active, self.second_offset, self.second];
        for (k, v) in words.into_iter().enumerate() {
            b[2 * k..2 * k + 2].copy_from_slice(&v.to_le_bytes());
        }
        b
    }
}

const _: () = assert!(g::FRAMEBUFFER_FRONT == AT + 2);
const _: () = assert!(g::SCREEN_BUFFER == AT + 4);
const _: () = assert!(g::FRAMEBUFFER_ACTIVE == AT + 6);
const _: () = assert!(g::FRAMEBUFFER_2 == AT + 0x0a);
