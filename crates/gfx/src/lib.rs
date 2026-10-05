//! The game's drawing operations, as seen at its DNVGA driver's entry points.
//!
//! Everything here is pure: it works from the registers of a driver call and
//! a view of memory, so it can run beside the emulator (to verify and to
//! record), in an offline tool (to extract and re-render sprites) or in a
//! browser.
//!
//! Driver facts (from disassembling DNVGA as loaded by the game):
//! - screen addresses are `y * 320 + x + [drv:01A3]`, with `y` clamped to 199
//!   and 16-bit wrap-around inside the target segment;
//! - sprites are 4 bits per pixel (two per byte, low nibble first, 0 is
//!   transparent, colour = nibble + palette offset) or 8 bits per pixel
//!   (palette offset FEh opaque, FFh with 0 transparent), raw or RLE.

pub mod dat;
pub mod hash;
pub mod hsq;
pub mod model;
pub mod ops;
pub mod sheet;
pub mod sprite;

pub use ops::{DrawOp, Regs};

/// Read-only view of the emulated PC's memory.
#[derive(Clone, Copy)]
pub struct Mem<'a>(pub &'a [u8]);

impl Mem<'_> {
    #[inline]
    pub fn u8(&self, seg: u16, off: u16) -> u8 {
        self.0.get(((seg as usize) << 4) + off as usize).copied().unwrap_or(0)
    }
    #[inline]
    pub fn u16(&self, seg: u16, off: u16) -> u16 {
        self.u8(seg, off) as u16 | (self.u8(seg, off.wrapping_add(1)) as u16) << 8
    }
    /// `len` bytes from seg:off (offsets wrap within the segment).
    pub fn bytes(&self, seg: u16, off: u16, len: usize) -> Vec<u8> {
        (0..len).map(|i| self.u8(seg, off.wrapping_add(i as u16))).collect()
    }
}

/// Byte offset of screen position (x, y) the way the driver computes it.
#[inline]
pub fn screen_offset(x: u16, y: u16, y_offset: u16) -> u16 {
    let y = if y >= 200 { 199 } else { y };
    y.wrapping_mul(320).wrapping_add(x).wrapping_add(y_offset)
}
