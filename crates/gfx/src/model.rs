//! An exact low-resolution model of the driver: replays an operation's
//! writes, in the driver's order, to a [`Sink`]. Each write says where the
//! pixel comes from (a sprite pixel, a glyph bit, a fill, a copied byte), so
//! the same traversal drives the low-resolution model ([`apply`], verified
//! against the real driver) and the HD compositor.
//!
//! Each function mirrors the driver's address arithmetic (16-bit wrap inside
//! the segment, `y` clamped to 199, rows advanced by ±320 from where the
//! previous row ended).

use crate::{
    Mem,
    ops::{DrawOp, Format, Rect, Regs, SpriteRef},
    screen_offset,
    sprite::row_bytes,
};

/// Where a written pixel comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Px {
    /// Pixel (`col`, `row`) of the sprite's decoded image (unflipped
    /// coordinates; rows of 4-bit sprites are padded to whole words).
    Sprite { col: u16, row: u16 },
    /// Bit (`col`, `row`) of a glyph, set or not.
    Glyph { col: u8, row: u8, on: bool },
    /// A solid fill (or clear).
    Fill,
    /// Pixel (`col`, `row`) of the mouse cursor (drawn, not transparent).
    Cursor { col: u8, row: u8 },
    /// A ramp pixel: `ramp` is the undithered colour (8.8 fixed point).
    Gradient { ramp: u16 },
    /// Byte `at` of segment `seg`, copied.
    Copy { seg: u16, at: u16 },
    /// Byte `at` of segment `seg`, enlarged: this is part (`ix`, `iy`) of
    /// its `nx` × `ny` copies.
    Zoom { seg: u16, at: u16, ix: u8, iy: u8, nx: u8, ny: u8 },
}

/// Receives an operation's writes, in the driver's order.
pub trait Sink {
    /// The driver writes `value` at offset `at` of the target segment.
    fn put(&mut self, at: u16, value: u8, px: Px);
    /// The driver passes over a transparent sprite pixel at `at` without
    /// writing it (HD art may still cover part of it).
    fn skip(&mut self, _at: u16, _px: Px) {}
}

/// The low-resolution model: writes into the target segment's bytes.
pub struct LowRes<'a>(pub &'a mut [u8]);

impl Sink for LowRes<'_> {
    #[inline]
    fn put(&mut self, at: u16, value: u8, _px: Px) {
        self.0[at as usize] = value;
    }
}

/// Apply `op` to `dst` (the 65536 bytes of the target segment). `mem` is the
/// machine's memory before the call (sources are read from it). Returns
/// false for operations that are not modelled.
pub fn apply(op: &DrawOp, r: &Regs, mem: Mem, dst: &mut [u8]) -> bool {
    assert_eq!(dst.len(), 0x10000);
    walk(op, r, mem, &mut LowRes(dst))
}

/// Replay `op`'s writes to `dst`. Returns false if `op` is not modelled.
pub fn walk<S: Sink>(op: &DrawOp, r: &Regs, mem: Mem, dst: &mut S) -> bool {
    let yo = r.y_offset;
    match *op {
        DrawOp::Blit { sprite, x, y } => {
            return match sprite.format {
                Format::Nibble { pal } if sprite.rle() => nibble_rle(&sprite, pal, x, y, yo, mem, dst),
                Format::Nibble { pal } => nibble_raw(&sprite, pal, screen_offset(x, y, yo), 0, sprite.height as u16, mem, dst, None),
                _ if sprite.rle() => byte_rle(&sprite, x, y, yo, mem, dst),
                _ => byte_raw(&sprite, x, y, yo, mem, dst),
            };
        }
        DrawOp::BlitClipped { sprite, x, y, clip } => {
            let Format::Nibble { pal } = sprite.format else {
                return false;
            };
            return clipped(&sprite, pal, x, y, r.cx, clip, yo, mem, dst);
        }
        DrawOp::BlitScaled { sprite, x, y, out_w, out_h, step } => {
            let Format::Nibble { pal } = sprite.format else {
                return false;
            };
            return scaled(&sprite, pal, x, y, out_w, out_h, step, yo, mem, dst);
        }
        DrawOp::Glyph { x, y, w, h, fg, bg, seg, off } => {
            let mut row = screen_offset(x, y, yo);
            for i in 0..h {
                let mut bits = mem.u8(seg, off.wrapping_add(i as u16));
                let mut d = row;
                for c in 0..w {
                    let on = bits & 0x80 != 0;
                    bits <<= 1;
                    let px = Px::Glyph { col: c, row: i, on };
                    match (on, bg) {
                        (true, _) => dst.put(d, fg, px),
                        (false, Some(b)) => dst.put(d, b, px),
                        (false, None) => dst.skip(d, px),
                    }
                    d = d.wrapping_add(1);
                }
                row = row.wrapping_add(320);
            }
        }
        DrawOp::Clear => {
            for i in 0..64000u16 {
                dst.put(i, 0, Px::Fill);
            }
        }
        DrawOp::Fill { rect, colour } => {
            // The address computation clamps y0 to 199 in place, so the
            // height is taken from the clamped value.
            let y0 = rect.y0.min(199);
            if rect.x1 <= rect.x0 || y0 >= rect.y1 {
                return true;
            }
            let w = rect.x1 - rect.x0;
            let h = rect.y1 - y0;
            let mut row = screen_offset(rect.x0, rect.y0, yo);
            for _ in 0..h {
                for i in 0..w {
                    dst.put(row.wrapping_add(i), colour, Px::Fill);
                }
                row = row.wrapping_add(320);
            }
        }
        DrawOp::Copy { src } => {
            for i in 0..64000u16 {
                dst.put(i, mem.u8(src, i), Px::Copy { seg: src, at: i });
            }
        }
        DrawOp::CopyTop { src } => {
            for i in 0..48640u16 {
                dst.put(i, mem.u8(src, i), Px::Copy { seg: src, at: i });
            }
        }
        DrawOp::CopyRect { src, x, y, w, h } => {
            let mut row = screen_offset(x, y, yo);
            for _ in 0..h {
                for i in 0..w {
                    let a = row.wrapping_add(i);
                    dst.put(a, mem.u8(src, a), Px::Copy { seg: src, at: a });
                }
                row = row.wrapping_add(320);
            }
        }
        DrawOp::Gradient { x, y, len, ramp, step, noise, pattern, backwards } => {
            let mut at = screen_offset(x, y, yo);
            let (mut ramp, mut noise) = (ramp, noise);
            // LOOP with CX = 0 runs 65536 times.
            let n = if len == 0 { 0x10000 } else { len as u32 };
            for _ in 0..n {
                let carry = noise & 1 != 0;
                noise >>= 1;
                if carry {
                    noise ^= pattern;
                }
                let v = ((noise & 3) as u8).wrapping_sub(1).wrapping_add((ramp >> 8) as u8);
                dst.put(at, v, Px::Gradient { ramp });
                at = if backwards { at.wrapping_sub(1) } else { at.wrapping_add(1) };
                ramp = ramp.wrapping_add(step);
            }
        }
        DrawOp::SaveRect { rect, src, at, .. } => {
            let (start, w, rows) = rect_span(rect, yo);
            let (mut s, mut d) = (start, at);
            for _ in 0..rows {
                for _ in 0..w {
                    dst.put(d, mem.u8(src, s), Px::Copy { seg: src, at: s });
                    s = s.wrapping_add(1);
                    d = d.wrapping_add(1);
                }
                s = s.wrapping_add(320u16.wrapping_sub(w));
            }
        }
        DrawOp::RestoreRect { rect, src, from } => {
            let (start, w, rows) = rect_span(rect, yo);
            let (mut s, mut d) = (from, start);
            for _ in 0..rows {
                for _ in 0..w {
                    dst.put(d, mem.u8(src, s), Px::Copy { seg: src, at: s });
                    s = s.wrapping_add(1);
                    d = d.wrapping_add(1);
                }
                d = d.wrapping_add(320u16.wrapping_sub(w));
            }
        }
        DrawOp::Zoom { src, x, y, variant } => {
            let (cols, rows) = zoom_pattern(variant);
            let src_start = screen_offset(x, y, yo);
            let mut d_row = yo;
            for (sy, &ny) in (0u16..).zip(rows.iter()) {
                for iy in 0..ny {
                    let mut d = d_row;
                    for (sx, &nx) in (0u16..).zip(cols.iter()) {
                        let at = src_start.wrapping_add(sy.wrapping_mul(320)).wrapping_add(sx);
                        let v = mem.u8(src, at);
                        for ix in 0..nx {
                            dst.put(d, v, Px::Zoom { seg: src, at, ix, iy, nx, ny });
                            d = d.wrapping_add(1);
                        }
                    }
                    d_row = d_row.wrapping_add(320);
                }
            }
        }
        DrawOp::Cursor { x, y, seg, off } => {
            // Hotspot first; the position is clamped at the screen's edge.
            let x = x.saturating_sub(mem.u16(seg, off));
            let y = y.saturating_sub(mem.u16(seg, off.wrapping_add(2)));
            let rows = if y > 184 { 200 - y.min(200) } else { 16 };
            let w = (320u16.wrapping_sub(x)).min(16);
            let mut row_start = screen_offset(x, y, yo);
            let mut save: u16 = 0xfa00;
            for r in 0..rows {
                let mask = mem.u16(seg, off.wrapping_add(4 + 2 * r));
                let shape = mem.u16(seg, off.wrapping_add(4 + 32 + 2 * r));
                for c in 0..w {
                    let at = row_start.wrapping_add(c);
                    let under = mem.u8(0xa000, at);
                    dst.put(save, under, Px::Copy { seg: 0xa000, at });
                    save = save.wrapping_add(1);
                    let bit = 0x8000 >> c;
                    let px = Px::Cursor { col: c as u8, row: r as u8 };
                    if mask & bit == 0 {
                        dst.put(at, if shape & bit != 0 { 15 } else { 0 }, px);
                    } else {
                        dst.skip(at, px);
                    }
                }
                row_start = row_start.wrapping_add(320);
            }
        }
        DrawOp::CursorRestore { at, w, h } => {
            let mut row = at;
            let mut save: u16 = 0xfa00;
            for _ in 0..h {
                for c in 0..w {
                    dst.put(row.wrapping_add(c), mem.u8(0xa000, save), Px::Copy { seg: 0xa000, at: save });
                    save = save.wrapping_add(1);
                }
                row = row.wrapping_add(320);
            }
        }
        DrawOp::SetYOffset { .. } | DrawOp::NoDraw => {}
        DrawOp::Other { .. } => return false,
    }
    true
}

/// A rectangle {x0, y0, x1, y1} as the driver walks it: start offset,
/// width and row count (`LOOP`-style: 0 rows means 65536).
fn rect_span(r: Rect, yo: u16) -> (u16, u16, u32) {
    let start = screen_offset(r.x0, r.y0, yo);
    // The address computation clamps y0 to 199 in its register.
    let y0 = r.y0.min(199);
    let w = r.x1.wrapping_sub(r.x0);
    let rows = r.y1.wrapping_sub(y0);
    (start, w, if rows == 0 { 0x10000 } else { rows as u32 })
}

/// Slot 37's replication patterns: copies of each source column and row.
fn zoom_pattern(variant: u8) -> (Vec<u8>, Vec<u8>) {
    let rep = |unit: &[u8], n: usize| -> Vec<u8> { unit.iter().copied().cycle().take(unit.len() * n).collect() };
    match variant {
        1 => (rep(&[2, 1, 1, 1, 1, 1, 1], 40), rep(&[1, 1, 1, 1, 1, 1, 2], 19)),
        2 => (rep(&[2, 1, 1], 80), rep(&[1, 1, 2], 38)),
        3 => ([rep(&[2, 1], 106), vec![2]].concat(), rep(&[1, 2], 50)),
        4 => (rep(&[2], 160), rep(&[2], 76)),
        5 => ([rep(&[3], 106), vec![2]].concat(), [rep(&[3], 50), vec![2]].concat()),
        6 => (rep(&[4], 80), rep(&[4], 38)),
        _ => (rep(&[8], 40), rep(&[8], 19)),
    }
}

#[inline]
fn put_nibble<S: Sink>(dst: &mut S, at: u16, nib: u8, pal: u8, px: Px) {
    if nib != 0 {
        dst.put(at, nib.wrapping_add(pal), px);
    } else {
        dst.skip(at, px);
    }
}

/// Raw 4-bit rows of whole words. `clip_x` limits written columns.
#[allow(clippy::too_many_arguments)]
fn nibble_raw<S: Sink>(s: &SpriteRef, pal: u8, start: u16, first_row: u16, rows: u16, mem: Mem, dst: &mut S, clip_x: Option<(u16, u16, u16)>) -> bool {
    let rb = row_bytes(s.format, s.wflags) as u16;
    let mut src = s.off;
    let mut row = start;
    for r in 0..rows {
        for i in 0..rb {
            let b = mem.u8(s.seg, src.wrapping_add(i));
            for (k, nib) in [b & 15, b >> 4].into_iter().enumerate() {
                let col = i * 2 + k as u16;
                if let Some((x, x0, x1)) = clip_x {
                    let px = x.wrapping_add(col) as i16;
                    if px < x0 as i16 || px >= x1 as i16 {
                        continue;
                    }
                }
                put_nibble(dst, row.wrapping_add(col), nib, pal, Px::Sprite { col, row: first_row.wrapping_add(r) });
            }
        }
        src = src.wrapping_add(rb);
        row = row.wrapping_add(320);
    }
    true
}

/// Start address and direction for flipped sprites, as the driver sets them.
fn flipped_start(s: &SpriteRef, x: u16, y: u16, yo: u16) -> (u16, i16, i16) {
    let h = s.height as u16;
    let mut start = screen_offset(x, y, yo);
    let mut dy: i16 = 320;
    if s.vflip() {
        start = start.wrapping_add(h.wrapping_sub(1).wrapping_mul(320));
        dy = -320;
    }
    let mut dx: i16 = 1;
    if s.hflip() {
        start = start.wrapping_add(s.width()).wrapping_sub(1);
        dx = -1;
    }
    (start, dx, dy)
}

/// RLE 4-bit: rows end when the row's byte budget reaches exactly zero;
/// the next row starts 320 bytes (or -320) after where this one began,
/// computed from where the writes ended.
fn nibble_rle<S: Sink>(s: &SpriteRef, pal: u8, x: u16, y: u16, yo: u16, mem: Mem, dst: &mut S) -> bool {
    let (di, dx, dy) = flipped_start(s, x, y, yo);
    nibble_rle_rows(s, pal, s.off, di, dx, dy, 0, s.height as u16, None, mem, dst);
    true
}

/// The RLE row loop. `clip_x` = (x, x0, x1) keeps columns x0 ≤ x + col < x1.
#[allow(clippy::too_many_arguments)]
fn nibble_rle_rows<S: Sink>(
    s: &SpriteRef,
    pal: u8,
    src: u16,
    start: u16,
    dx: i16,
    dy: i16,
    first_row: u16,
    rows: u16,
    clip_x: Option<(u16, u16, u16)>,
    mem: Mem,
    dst: &mut S,
) {
    let rb = row_bytes(s.format, s.wflags) as u16;
    let mut di = start;
    let mut si = src;
    let mut next = || {
        let b = mem.u8(s.seg, si);
        si = si.wrapping_add(1);
        b
    };
    for r in 0..rows {
        let row = first_row.wrapping_add(r);
        let mut budget = rb;
        let mut guard = 0;
        let mut col: u16 = 0;
        loop {
            let n = next();
            let (count, rep) = if n < 0x80 { (n as u16 + 1, None) } else { (0x101 - n as u16, Some(next())) };
            budget = budget.wrapping_sub(count);
            for _ in 0..count {
                let b = rep.unwrap_or_else(&mut next);
                for nib in [b & 15, b >> 4] {
                    let visible = clip_x.is_none_or(|(x, x0, x1)| {
                        let px = x.wrapping_add(col) as i16;
                        px >= x0 as i16 && px < x1 as i16
                    });
                    if visible {
                        put_nibble(dst, di, nib, pal, Px::Sprite { col, row });
                    }
                    di = di.wrapping_add(dx as u16);
                    col = col.wrapping_add(1);
                }
            }
            guard += 1;
            if budget == 0 || guard > 4096 {
                break;
            }
        }
        // Back to the row start, then one line down (or up).
        di = di.wrapping_sub((2 * rb).wrapping_mul(dx as u16)).wrapping_add(dy as u16);
    }
}

/// Skip `rows` rows of RLE data the way the driver does (consuming codes
/// until exactly `rows * row_bytes` bytes are accounted for).
fn skip_rle(seg: u16, mut si: u16, bytes: u16, mem: Mem) -> u16 {
    let mut budget = bytes;
    let mut guard = 0;
    while budget != 0 && guard < 65536 {
        let n = mem.u8(seg, si);
        si = si.wrapping_add(1);
        if n < 0x80 {
            let count = n as u16 + 1;
            si = si.wrapping_add(count);
            budget = budget.wrapping_sub(count);
        } else {
            si = si.wrapping_add(1);
            budget = budget.wrapping_sub(0x101 - n as u16);
        }
        guard += 1;
    }
    si
}

/// Raw 8-bit (flags ignored by the driver).
fn byte_raw<S: Sink>(s: &SpriteRef, x: u16, y: u16, yo: u16, mem: Mem, dst: &mut S) -> bool {
    let w = s.width();
    let keyed = s.format == Format::ByteKey;
    let mut row = screen_offset(x, y, yo);
    let mut src = s.off;
    for r in 0..s.height as u16 {
        for i in 0..w {
            let v = mem.u8(s.seg, src.wrapping_add(i));
            let px = Px::Sprite { col: i, row: r };
            if keyed && v == 0 {
                dst.skip(row.wrapping_add(i), px);
            } else {
                dst.put(row.wrapping_add(i), v, px);
            }
        }
        src = src.wrapping_add(w);
        row = row.wrapping_add(320);
    }
    true
}

/// RLE 8-bit.
fn byte_rle<S: Sink>(s: &SpriteRef, x: u16, y: u16, yo: u16, mem: Mem, dst: &mut S) -> bool {
    let w = s.width();
    let keyed = s.format == Format::ByteKey;
    let (mut di, dx, dy) = flipped_start(s, x, y, yo);
    let mut si = s.off;
    let mut next = || {
        let b = mem.u8(s.seg, si);
        si = si.wrapping_add(1);
        b
    };
    for row in 0..s.height as u16 {
        let mut budget = w;
        let mut guard = 0;
        let mut col: u16 = 0;
        loop {
            let n = next();
            let (count, rep) = if n < 0x80 { (n as u16 + 1, None) } else { (0x101 - n as u16, Some(next())) };
            budget = budget.wrapping_sub(count);
            for _ in 0..count {
                let v = rep.unwrap_or_else(&mut next);
                let px = Px::Sprite { col, row };
                if keyed && v == 0 {
                    dst.skip(di, px);
                } else {
                    dst.put(di, v, px);
                }
                di = di.wrapping_add(dx as u16);
                col = col.wrapping_add(1);
            }
            guard += 1;
            if budget == 0 || guard > 4096 {
                break;
            }
        }
        di = di.wrapping_sub(w.wrapping_mul(dx as u16)).wrapping_add(dy as u16);
    }
    true
}

/// Slot 6: raw 4-bit sprite clipped to a rectangle. `cx` is the row count
/// register (the driver uses all 16 bits).
#[allow(clippy::too_many_arguments)]
fn clipped<S: Sink>(s: &SpriteRef, pal: u8, x: u16, y: u16, cx: u16, clip: Rect, yo: u16, mem: Mem, dst: &mut S) -> bool {
    let rb = row_bytes(s.format, s.wflags) as u16;
    let (mut y, mut rows, mut src) = (y, cx, s.off);
    let mut first_row = 0u16;
    // Top.
    let skip = clip.y0.wrapping_sub(y) as i16;
    if skip > 0 {
        let (r, borrow) = rows.overflowing_sub(skip as u16);
        if borrow || r == 0 {
            return true;
        }
        rows = r;
        y = y.wrapping_add(skip as u16);
        first_row = skip as u16;
        src = if s.rle() { skip_rle(s.seg, src, rb.wrapping_mul(skip as u16), mem) } else { src.wrapping_add(rb.wrapping_mul(skip as u16)) };
    }
    // Bottom.
    let over = y.wrapping_add(rows).wrapping_sub(clip.y1);
    if over != 0 && (over as i16) > 0 {
        let (r, borrow) = rows.overflowing_sub(over);
        if borrow || r == 0 {
            return true;
        }
        rows = r;
    }
    let clip_x = Some((x, clip.x0, clip.x1));
    if s.rle() {
        nibble_rle_rows(s, pal, src, screen_offset(x, y, yo), 1, 320, first_row, rows, clip_x, mem, dst);
        return true;
    }
    let sprite = SpriteRef { off: src, ..*s };
    nibble_raw(&sprite, pal, screen_offset(x, y, yo), first_row, rows, mem, dst, clip_x)
}

/// Slot 35 path: `out_w` × `out_h` pixels, source position advancing by
/// `step`/256 per pixel and per row (8.8 fixed point).
#[allow(clippy::too_many_arguments)]
fn scaled<S: Sink>(s: &SpriteRef, pal: u8, x: u16, y: u16, out_w: u16, out_h: u16, step: u16, yo: u16, mem: Mem, dst: &mut S) -> bool {
    let rb = row_bytes(s.format, s.wflags) as u16;
    let (mut x, mut y) = (x, y);
    let mut dy: u16 = 320;
    if s.vflip() {
        y = y.wrapping_add(out_h).wrapping_sub(1);
        dy = 320u16.wrapping_neg();
    }
    let mut dx: u16 = 1;
    if s.hflip() {
        x = x.wrapping_add(out_w).wrapping_sub(1);
        dx = 1u16.wrapping_neg();
    }
    let mut row = screen_offset(x, y, yo);
    let mut acc_row: u16 = 0;
    for _ in 0..out_h {
        // The driver multiplies the row bytes' low byte by the row index (8-bit MUL).
        let base = s.off.wrapping_add((rb & 0xff).wrapping_mul(acc_row >> 8));
        let mut acc: u16 = 0;
        let mut d = row;
        for _ in 0..out_w {
            let col = (acc >> 8) as u8;
            let b = mem.u8(s.seg, base.wrapping_add((col >> 1) as u16));
            let nib = if col & 1 != 0 { b >> 4 } else { b & 15 };
            put_nibble(dst, d, nib, pal, Px::Sprite { col: col as u16, row: acc_row >> 8 });
            d = d.wrapping_add(dx);
            acc = acc.wrapping_add(step);
        }
        row = row.wrapping_add(dy);
        acc_row = acc_row.wrapping_add(step);
    }
    true
}
