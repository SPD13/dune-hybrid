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

pub mod compose;
pub mod dat;
pub mod hash;
pub mod head;
pub mod hsq;
pub mod mmpx;
pub mod model;
pub mod ops;
pub mod pack;
pub mod room;
pub mod sheet;
pub mod sprite;

pub use ops::{DrawOp, Regs};

/// Memory a running game keeps outside the emulated PC's: the bytes
/// subsystems ported to Rust own (`pc::hw::Owned`), served to a [`Mem`]
/// view in place of the raw memory's (which may hold a stale copy, or none).
pub trait Overlay {
    /// The byte at linear `addr`, if it is kept here.
    fn byte(&self, addr: u32) -> Option<u8>;
}

/// Read-only view of the emulated PC's memory, as the game's code sees it:
/// the raw memory, with the bytes an [`Overlay`] keeps read from it.
#[derive(Clone, Copy)]
pub struct Mem<'a> {
    raw: &'a [u8],
    over: Option<&'a dyn Overlay>,
}

impl<'a> Mem<'a> {
    /// The raw memory alone (an offline tool's, a test's).
    pub fn new(raw: &'a [u8]) -> Mem<'a> {
        Mem { raw, over: None }
    }

    /// The raw memory with the bytes `over` keeps.
    pub fn with(raw: &'a [u8], over: &'a dyn Overlay) -> Mem<'a> {
        Mem { raw, over: Some(over) }
    }

    /// The byte at linear `addr` (0 past the memory's end).
    #[inline]
    pub fn at(&self, addr: usize) -> u8 {
        if let Some(o) = self.over
            && let Some(v) = o.byte(addr as u32)
        {
            return v;
        }
        self.raw.get(addr).copied().unwrap_or(0)
    }

    #[inline]
    pub fn u8(&self, seg: u16, off: u16) -> u8 {
        self.at(((seg as usize) << 4) + off as usize)
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
