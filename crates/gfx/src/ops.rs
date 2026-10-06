//! Driver calls decoded into drawing operations.
//!
//! Slot `n` is the game's far call to `driver:0100h + 3n`. The register
//! conventions below were read from the driver's code.

use crate::Mem;

/// Registers at a driver call (plus the driver's Y offset, `drv:01A3`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Regs {
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub si: u16,
    pub di: u16,
    pub bp: u16,
    pub ds: u16,
    pub es: u16,
    pub ss: u16,
    pub flags: u16,
    pub y_offset: u16,
    /// The driver's segment (some calls use the driver's own variables).
    pub drv: u16,
}

impl Regs {
    #[inline]
    pub fn ch(&self) -> u8 {
        (self.cx >> 8) as u8
    }
    #[inline]
    pub fn cl(&self) -> u8 {
        self.cx as u8
    }
}

/// Sprite pixel formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    /// 4 bits per pixel; colour = nibble + palette offset, 0 transparent.
    Nibble { pal: u8 },
    /// 8 bits per pixel, every value drawn (palette offset FEh).
    Byte,
    /// 8 bits per pixel, 0 transparent (palette offset FFh).
    ByteKey,
}

impl Format {
    pub fn from_pal(pal: u8) -> Format {
        match pal {
            0xff => Format::ByteKey,
            0xfe => Format::Byte,
            p => Format::Nibble { pal: p },
        }
    }
}

/// A sprite as the driver sees it: the 16-bit width/flags word, height and
/// where its pixel data is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteRef {
    /// Width (bits 0-8) and flags: 8000h RLE, 4000h mirror, 2000h upside down.
    pub wflags: u16,
    pub height: u8,
    pub format: Format,
    pub seg: u16,
    pub off: u16,
}

impl SpriteRef {
    pub fn width(&self) -> u16 {
        self.wflags & 0x1ff
    }
    pub fn rle(&self) -> bool {
        self.wflags & 0x8000 != 0
    }
    pub fn hflip(&self) -> bool {
        self.wflags & 0x4000 != 0
    }
    pub fn vflip(&self) -> bool {
        self.wflags & 0x2000 != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x0: u16,
    pub y0: u16,
    pub x1: u16,
    pub y1: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DrawOp {
    /// Slot 5: sprite at (x, y).
    Blit { sprite: SpriteRef, x: u16, y: u16 },
    /// Slot 6: 4-bit sprite at (x, y), clipped to `clip` (never flipped).
    BlitClipped { sprite: SpriteRef, x: u16, y: u16, clip: Rect },
    /// Slot 35 (and slot 5 for flipped raw 4-bit sprites): scaled sprite,
    /// `out_w` × `out_h` pixels stepping the source by `step`/256.
    BlitScaled { sprite: SpriteRef, x: u16, y: u16, out_w: u16, out_h: u16, step: u16 },
    /// Slot 7: 1-bit glyph, MSB first, one byte per row.
    Glyph { x: u16, y: u16, w: u8, h: u8, fg: u8, bg: Option<u8>, seg: u16, off: u16 },
    /// Slot 8: clear 64000 bytes.
    Clear,
    /// Slots 9/10: fill [x0, x1) × [y0, y1) with a colour.
    Fill { rect: Rect, colour: u8 },
    /// Slots 11/13/15: copy 64000 bytes from segment `src`.
    Copy { src: u16 },
    /// Slots 12/14/16: copy a w × h rectangle at (x, y) from segment `src`
    /// (same position in both buffers).
    CopyRect { src: u16, x: u16, y: u16, w: u16, h: u16 },
    /// Slot 18: copy 48640 bytes (152 lines) from segment `src`.
    CopyTop { src: u16 },
    /// Slot 36: a dithered colour ramp of `len` pixels from (x, y) (floors
    /// and walls): colour `ramp >> 8` (8.8 fixed point, + `step` per pixel)
    /// plus -1..2 from a noise generator (`noise` shifted right, XORed with
    /// `pattern` when a 1 falls out). Leftwards when `backwards`.
    Gradient { x: u16, y: u16, len: u16, ramp: u16, step: u16, noise: u16, pattern: u16, backwards: bool },
    /// Slot 25: save the rectangle `rect` of the buffer `src` into a packed
    /// buffer at `dst:at` (row after row, no padding).
    SaveRect { rect: Rect, src: u16, dst: u16, at: u16 },
    /// Slot 26: write a packed rectangle from `src:from` back into `rect`.
    RestoreRect { rect: Rect, src: u16, from: u16 },
    /// Slot 37: enlarge the region of `src` at (x, y) to 320 × ~152 at the
    /// target's Y offset, by pixel replication; `variant` 1-7 picks the
    /// factor (8/7, 4/3, 3/2, 2, 3, 4, 8).
    Zoom { src: u16, x: u16, y: u16, variant: u8 },
    /// Slot 3: the mouse cursor at (x, y) on the screen, from the record at
    /// `seg:off`: hotspot x, hotspot y, then 16 rows of a 1-bit mask (1 =
    /// transparent) and 16 rows of a 1-bit shape (1 = colour 15, else 0),
    /// MSB first. The pixels it covers are saved to A000:FA00 first.
    Cursor { x: u16, y: u16, seg: u16, off: u16 },
    /// Slot 4: put back the pixels the cursor covered (from A000:FA00);
    /// `at`, `w` and `h` are where it was drawn (driver variables).
    CursorRestore { at: u16, w: u16, h: u16 },
    /// Slot 33: set the Y offset to `lines` × 320.
    SetYOffset { lines: u16 },
    /// Palette, retrace, mode and other calls that do not draw.
    NoDraw,
    /// A drawing slot this crate does not model (yet).
    Other { slot: u8 },
}

impl DrawOp {
    /// Decode the call to driver slot `slot`. `mem` is read for operands that
    /// live in memory (fill and clip rectangles, sprite palette bytes).
    pub fn decode(slot: u8, r: &Regs, mem: Mem) -> DrawOp {
        use DrawOp::*;
        let rect_at = |seg: u16, off: u16| Rect {
            x0: mem.u16(seg, off),
            y0: mem.u16(seg, off.wrapping_add(2)),
            x1: mem.u16(seg, off.wrapping_add(4)),
            y1: mem.u16(seg, off.wrapping_add(6)),
        };
        match slot {
            5 => {
                let sprite = SpriteRef { wflags: r.di, height: r.cl(), format: Format::from_pal(r.ch()), seg: r.ds, off: r.si };
                if !matches!(sprite.format, Format::Nibble { .. }) && sprite.rle() && sprite.hflip() {
                    // The driver's mirrored 8-bit RLE paths are broken (the
                    // opaque one, driver:0F25, tests JZ where the others test
                    // JS and misreads run codes; both overrun and can hang).
                    // The game never takes them.
                    Other { slot }
                } else if matches!(sprite.format, Format::Nibble { .. }) && !sprite.rle() && sprite.wflags & 0x6000 != 0 {
                    // Flipped raw 4-bit sprites take the scaling path at 1:1.
                    BlitScaled { sprite, x: r.dx, y: r.bx, out_w: r.di & 0x3ff, out_h: r.cl() as u16, step: 0x100 }
                } else {
                    Blit { sprite, x: r.dx, y: r.bx }
                }
            }
            6 => {
                // The palette offset comes from the sprite header (the byte
                // before the pixels), not from CH.
                let format = if r.ch() >= 0xfe { Format::from_pal(r.ch()) } else { Format::Nibble { pal: mem.u8(r.ds, r.si.wrapping_sub(1)) } };
                // Flips are ignored (raw) or masked off (RLE) by this slot.
                let wflags = if r.di & 0x8000 != 0 { 0x8000 | (r.di & 0x1fff) } else { r.di };
                let sprite = SpriteRef { wflags, height: r.cl(), format, seg: r.ds, off: r.si };
                if matches!(format, Format::Nibble { .. }) { BlitClipped { sprite, x: r.dx, y: r.bx, clip: rect_at(r.ss, r.bp) } } else { Other { slot } }
            }
            35 => {
                // The source is raw 4-bit rows (the game decodes RLE sprites
                // into its scratch buffer first; the RLE bit may still be
                // set). Its height follows from the rows the scaling reads.
                let out_h = r.cl() as u32;
                let height = if out_h == 0 { 0 } else { (((out_h - 1) * r.bp as u32) >> 8) + 1 }.min(255) as u8;
                let sprite = SpriteRef { wflags: r.di & 0x7fff, height, format: Format::Nibble { pal: r.ch() }, seg: r.ds, off: r.si };
                BlitScaled { sprite, x: r.dx, y: r.bx, out_w: r.ax & 0x3ff, out_h: r.cl() as u16, step: r.bp }
            }
            7 => {
                let fg = r.ax as u8;
                let bg = (r.ax >> 8) as u8;
                Glyph { x: r.dx, y: r.bx, w: r.cl(), h: r.ch(), fg, bg: (bg != 0).then_some(bg), seg: r.ds, off: r.si }
            }
            8 => Clear,
            9 => Fill { rect: rect_at(r.ds, r.si), colour: 0 },
            10 => Fill { rect: rect_at(r.ds, r.si), colour: r.ax as u8 },
            11 | 13 | 15 => Copy { src: r.ds },
            12 | 16 => CopyRect { src: r.ds, x: r.dx, y: r.bx, w: r.bp, h: r.ax },
            14 => CopyRect { src: r.si, x: r.dx, y: r.bx, w: r.bp, h: r.ax },
            18 => CopyTop { src: r.si },
            33 => SetYOffset { lines: r.ax },
            3 => Cursor { x: r.dx, y: r.bx, seg: r.ds, off: r.si },
            4 => CursorRestore { at: mem.u16(r.drv, 0x18a), w: mem.u16(r.drv, 0x18c), h: mem.u16(r.drv, 0x18e) },
            25 => SaveRect { rect: rect_at(r.ss, r.bp), src: r.es, dst: r.ds, at: r.si },
            26 => RestoreRect { rect: rect_at(r.ss, r.bp), src: r.ds, from: r.si },
            37 if (1..=7).contains(&r.bp) => Zoom { src: r.ds, x: r.dx, y: r.bx, variant: r.bp as u8 },
            36 => Gradient { x: r.dx, y: r.bx, len: r.cx, ramp: r.ax, step: r.di, noise: r.bp, pattern: r.si, backwards: r.flags & 0x400 != 0 },
            // 0 mode, 1 info, 2/41/32/28 palette and retrace, 20/34 no-ops.
            0 | 1 | 2 | 20 | 28 | 32 | 34 | 41 => NoDraw,
            _ => Other { slot },
        }
    }

    /// The buffer segment this operation writes, if it draws.
    pub fn target(&self, r: &Regs) -> Option<u16> {
        match self {
            DrawOp::NoDraw | DrawOp::SetYOffset { .. } => None,
            DrawOp::SaveRect { dst, .. } => Some(*dst),
            DrawOp::Cursor { .. } | DrawOp::CursorRestore { .. } => Some(0xa000),
            _ => Some(r.es),
        }
    }
}
