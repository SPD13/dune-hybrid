//! An exact low-resolution model of the driver: replays an operation's writes
//! into a copy of its target segment. Used to verify the decoding against
//! the real driver, and as the reference for the HD renderer.
//!
//! Each function mirrors the driver's write order and address arithmetic
//! (16-bit wrap inside the segment, `y` clamped to 199, rows advanced by
//! ±320 from where the previous row ended).

use crate::{
    Mem,
    ops::{DrawOp, Format, Rect, Regs, SpriteRef},
    screen_offset,
    sprite::row_bytes,
};

/// Apply `op` to `dst` (the 65536 bytes of the target segment). `mem` is the
/// machine's memory before the call (sources are read from it). Returns
/// false for operations that are not modelled.
pub fn apply(op: &DrawOp, r: &Regs, mem: Mem, dst: &mut [u8]) -> bool {
    assert_eq!(dst.len(), 0x10000);
    let yo = r.y_offset;
    match *op {
        DrawOp::Blit { sprite, x, y } => {
            return match sprite.format {
            Format::Nibble { pal } if sprite.rle() => nibble_rle(&sprite, pal, x, y, yo, mem, dst),
            Format::Nibble { pal } => nibble_raw(&sprite, pal, screen_offset(x, y, yo), sprite.height as u16, mem, dst, None),
            _ if sprite.rle() => byte_rle(&sprite, x, y, yo, mem, dst),
                _ => byte_raw(&sprite, x, y, yo, mem, dst),
            };
        }
        DrawOp::BlitClipped { sprite, x, y, clip } => {
            let Format::Nibble { pal } = sprite.format else { return false };
            return clipped(&sprite, pal, x, y, r.cx, clip, yo, mem, dst);
        }
        DrawOp::BlitScaled { sprite, x, y, out_w, out_h, step } => {
            let Format::Nibble { pal } = sprite.format else { return false };
            return scaled(&sprite, pal, x, y, out_w, out_h, step, yo, mem, dst);
        }
        DrawOp::Glyph { x, y, w, h, fg, bg, seg, off } => {
            let mut row = screen_offset(x, y, yo);
            for i in 0..h as u16 {
                let mut bits = mem.u8(seg, off.wrapping_add(i));
                let mut d = row;
                for _ in 0..w {
                    let on = bits & 0x80 != 0;
                    bits <<= 1;
                    match (on, bg) {
                        (true, _) => dst[d as usize] = fg,
                        (false, Some(b)) => dst[d as usize] = b,
                        (false, None) => {}
                    }
                    d = d.wrapping_add(1);
                }
                row = row.wrapping_add(320);
            }
        }
        DrawOp::Clear => dst[..64000].fill(0),
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
                    dst[row.wrapping_add(i) as usize] = colour;
                }
                row = row.wrapping_add(320);
            }
        }
        DrawOp::Copy { src } => {
            for i in 0..64000u16 {
                dst[i as usize] = mem.u8(src, i);
            }
        }
        DrawOp::CopyTop { src } => {
            for i in 0..48640u16 {
                dst[i as usize] = mem.u8(src, i);
            }
        }
        DrawOp::CopyRect { src, x, y, w, h } => {
            let mut row = screen_offset(x, y, yo);
            for _ in 0..h {
                for i in 0..w {
                    let a = row.wrapping_add(i);
                    dst[a as usize] = mem.u8(src, a);
                }
                row = row.wrapping_add(320);
            }
        }
        DrawOp::SetYOffset { .. } | DrawOp::NoDraw => {}
        DrawOp::Other { .. } => return false,
    }
    true
}

#[inline]
fn put_nibble(dst: &mut [u8], at: u16, nib: u8, pal: u8) {
    if nib != 0 {
        dst[at as usize] = nib.wrapping_add(pal);
    }
}

/// Raw 4-bit rows of whole words. `clip_x` limits written columns.
fn nibble_raw(s: &SpriteRef, pal: u8, start: u16, rows: u16, mem: Mem, dst: &mut [u8], clip_x: Option<(u16, u16, u16)>) -> bool {
    let rb = row_bytes(s.format, s.wflags) as u16;
    let mut src = s.off;
    let mut row = start;
    for _ in 0..rows {
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
                put_nibble(dst, row.wrapping_add(col), nib, pal);
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
fn nibble_rle(s: &SpriteRef, pal: u8, x: u16, y: u16, yo: u16, mem: Mem, dst: &mut [u8]) -> bool {
    let (di, dx, dy) = flipped_start(s, x, y, yo);
    nibble_rle_rows(s, pal, s.off, di, dx, dy, s.height as u16, None, mem, dst);
    true
}

/// The RLE row loop. `clip_x` = (x, x0, x1) keeps columns x0 ≤ x + col < x1.
#[allow(clippy::too_many_arguments)]
fn nibble_rle_rows(s: &SpriteRef, pal: u8, src: u16, start: u16, dx: i16, dy: i16, rows: u16, clip_x: Option<(u16, u16, u16)>, mem: Mem, dst: &mut [u8]) {
    let rb = row_bytes(s.format, s.wflags) as u16;
    let mut di = start;
    let mut si = src;
    let mut next = || {
        let b = mem.u8(s.seg, si);
        si = si.wrapping_add(1);
        b
    };
    for _ in 0..rows {
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
                        put_nibble(dst, di, nib, pal);
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
fn byte_raw(s: &SpriteRef, x: u16, y: u16, yo: u16, mem: Mem, dst: &mut [u8]) -> bool {
    let w = s.width();
    let keyed = s.format == Format::ByteKey;
    let mut row = screen_offset(x, y, yo);
    let mut src = s.off;
    for _ in 0..s.height {
        for i in 0..w {
            let v = mem.u8(s.seg, src.wrapping_add(i));
            if !(keyed && v == 0) {
                dst[row.wrapping_add(i) as usize] = v;
            }
        }
        src = src.wrapping_add(w);
        row = row.wrapping_add(320);
    }
    true
}

/// RLE 8-bit.
fn byte_rle(s: &SpriteRef, x: u16, y: u16, yo: u16, mem: Mem, dst: &mut [u8]) -> bool {
    let w = s.width();
    let keyed = s.format == Format::ByteKey;
    let (mut di, dx, dy) = flipped_start(s, x, y, yo);
    let mut si = s.off;
    let mut next = || {
        let b = mem.u8(s.seg, si);
        si = si.wrapping_add(1);
        b
    };
    for _ in 0..s.height {
        let mut budget = w;
        let mut guard = 0;
        loop {
            let n = next();
            let (count, rep) = if n < 0x80 { (n as u16 + 1, None) } else { (0x101 - n as u16, Some(next())) };
            budget = budget.wrapping_sub(count);
            for _ in 0..count {
                let v = rep.unwrap_or_else(&mut next);
                if !(keyed && v == 0) {
                    dst[di as usize] = v;
                }
                di = di.wrapping_add(dx as u16);
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
fn clipped(s: &SpriteRef, pal: u8, x: u16, y: u16, cx: u16, clip: Rect, yo: u16, mem: Mem, dst: &mut [u8]) -> bool {
    let rb = row_bytes(s.format, s.wflags) as u16;
    let (mut y, mut rows, mut src) = (y, cx, s.off);
    // Top.
    let skip = clip.y0.wrapping_sub(y) as i16;
    if skip > 0 {
        let (r, borrow) = rows.overflowing_sub(skip as u16);
        if borrow || r == 0 {
            return true;
        }
        rows = r;
        y = y.wrapping_add(skip as u16);
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
        nibble_rle_rows(s, pal, src, screen_offset(x, y, yo), 1, 320, rows, clip_x, mem, dst);
        return true;
    }
    let sprite = SpriteRef { off: src, ..*s };
    nibble_raw(&sprite, pal, screen_offset(x, y, yo), rows, mem, dst, clip_x)
}

/// Slot 35 path: `out_w` × `out_h` pixels, source position advancing by
/// `step`/256 per pixel and per row (8.8 fixed point).
#[allow(clippy::too_many_arguments)]
fn scaled(s: &SpriteRef, pal: u8, x: u16, y: u16, out_w: u16, out_h: u16, step: u16, yo: u16, mem: Mem, dst: &mut [u8]) -> bool {
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
            put_nibble(dst, d, nib, pal);
            d = d.wrapping_add(dx);
            acc = acc.wrapping_add(step);
        }
        row = row.wrapping_add(dy);
        acc_row = acc_row.wrapping_add(step);
    }
    true
}
