//! The mouse: its position and range (INT 33h), the cursor (drawn and
//! restored by the graphics driver), and which cursor image to show.
//!
//! [`g::CURSOR_SHOWN`] counts: 0 while the cursor is drawn, negative while
//! hidden (each hide one lower).

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CX, DI, DX, SI},
    rec::Bail,
};

mod code {
    pub const SHL_CX_AND_DX: u16 = 0xdb44;
    pub const GET_MOUSE_CURSOR_IMAGE_ADDR: u16 = 0xdc6a;
    pub const SUB_D6FE: u16 = 0xd6fe;
}

/// The cursor images.
mod cursor {
    pub const ARROW: u16 = 0x2584;
    pub const BUSY: u16 = 0x25c8;
    pub const LEFT: u16 = 0x260c;
    pub const UP: u16 = 0x2650;
    pub const RIGHT: u16 = 0x2694;
    pub const DOWN: u16 = 0x26d8;
}

/// No mouse driver (command-line bit 40h): the game reads the mouse
/// itself.
fn no_driver(c: &mut Ctx) -> bool {
    c.byte(g::CMD_ARGS) & 0x40 != 0
}

/// `set_mouse_pos` (DAE3): the driver's position (function 4) to the
/// game's, scaled by the shift counts at [2580h].
pub fn set_mouse_pos(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdae3)?;
    let mut c = Ctx::new(m);
    let args = c.byte(g::CMD_ARGS);
    if no_driver(&mut c) {
        c.run(start, 3)?;
        c.test(B, args as u16, 0x40);
        return c.ret();
    }
    c.run(start, 10)?;
    c.test(B, args as u16, 0x40);
    let (x, y) = (c.word(g::MOUSE_X), c.word(g::MOUSE_Y));
    let scale = c.word(g::MOUSE_SCALE);
    c.set_reg(CX, scale);
    let x = c.shl(W, x, scale as u8);
    let y = c.shl(W, y, (scale >> 8) as u8);
    c.set_reg8(r8::CL, (scale >> 8) as u8);
    c.set_reg(DX, y);
    c.set_reg(CX, x);
    c.set_reg(AX, 4);
    c.interp(0xdb00, 0xdb02)?;
    c.run(0xdb02, 1)?;
    c.ret()
}

/// `define_mouse_range` (DB14): the mouse's range (CX, DX) to (AX, BX),
/// noted at DC3Ah, and given to the driver (functions 7 and 8, scaled).
pub fn define_mouse_range(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdb14)?;
    let mut c = Ctx::new(m);
    let args = c.byte(g::CMD_ARGS);
    let none = args & 0x40 != 0;
    c.run(start, if none { 8 } else { 11 })?;
    c.set_reg(DI, g::MOUSE_RANGE);
    for (i, r) in [CX, DX, AX, BX].into_iter().enumerate() {
        let v = c.reg(r);
        c.set_word(g::MOUSE_RANGE + 2 * i as u16, v);
    }
    if none {
        return c.ret();
    }
    let (ax, bx) = (c.reg(AX), c.reg(BX));
    c.push(ax);
    c.push(bx);
    let sx = c.byte(g::MOUSE_SCALE);
    c.set_reg8(r8::AL, sx);
    c.call(code::SHL_CX_AND_DX, 0xdb31)?;
    c.run(0xdb31, 1)?;
    c.set_reg(AX, 7);
    c.interp(0xdb34, 0xdb36)?;
    c.run(0xdb36, 4)?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    let cx = c.pop();
    c.set_reg(CX, cx);
    let sy = c.byte(g::MOUSE_SCALE + 1);
    c.set_reg8(r8::AL, sy);
    c.call(code::SHL_CX_AND_DX, 0xdb3e)?;
    c.run(0xdb3e, 1)?;
    c.set_reg(AX, 8);
    c.interp(0xdb41, 0xdb43)?;
    c.run(0xdb43, 1)?;
    c.ret()
}

/// From DBB2: hides the cursor one level more (not below -1... the count
/// only goes down from 0 or below), restoring what was under it if it was
/// drawn (driver function 4). AX kept.
fn hide(c: &mut Ctx) -> Result<(), Bail> {
    let old = c.byte(g::CURSOR_SHOWN);
    let lower = old.wrapping_sub(1);
    let stays = lower & 0x80 == 0;
    let drawn = old & 0x80 == 0;
    c.run(0xdbb2, 4 + stays as u32 + 2 + if drawn { 0 } else { 2 })?;
    let ax = c.reg(AX);
    c.push(ax);
    c.set_reg8(r8::AL, old);
    let v = c.dec(B, old as u16) as u8;
    c.set_byte(g::CURSOR_SHOWN, v);
    if stays {
        let v = c.inc(B, v as u16) as u8;
        c.set_byte(g::CURSOR_SHOWN, v);
    }
    c.or(B, old as u16, old as u16);
    if drawn {
        c.interp(0xdbc4, 0xdbc8)?;
        c.run(0xdbc8, 2)?;
    }
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// `call_restore_cursor` (DBB2).
pub fn call_restore_cursor(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdbb2)?;
    hide(&mut Ctx::new(m))
}

/// From DBEC: shows the cursor one level more; when that makes it 0, draws
/// it at the mouse position (driver function 3), noting where. Registers
/// kept.
fn show(c: &mut Ctx) -> Result<(), Bail> {
    let old = c.byte(g::CURSOR_SHOWN);
    let new = old.wrapping_add(1);
    if new & 0x80 != 0 {
        c.run(0xdbec, 3)?;
        let v = c.inc(B, old as u16) as u8;
        c.set_byte(g::CURSOR_SHOWN, v);
        return c.ret();
    }
    if new != 0 {
        c.run(0xdbec, 5)?;
        let v = c.inc(B, old as u16) as u8;
        c.set_byte(g::CURSOR_SHOWN, v);
        let v = c.dec(B, v as u16) as u8;
        c.set_byte(g::CURSOR_SHOWN, v);
        return c.ret();
    }
    c.run(0xdbec, 15)?;
    let v = c.inc(B, old as u16) as u8;
    c.set_byte(g::CURSOR_SHOWN, v);
    for r in [AX, BX, CX, DX, SI, DI, BP] {
        let v = c.reg(r);
        c.push(v);
    }
    let (x, y) = (c.word(g::MOUSE_X), c.word(g::MOUSE_Y));
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    c.set_word(g::CURSOR_X, x);
    c.set_word(g::CURSOR_Y, y);
    let image = c.word(g::MOUSE_CURSOR_IMAGE);
    c.set_reg(SI, image);
    c.interp(0xdc0f, 0xdc13)?;
    c.run(0xdc13, 8)?;
    for r in [BP, DI, SI, DX, CX, BX, AX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// `draw_mouse` (DBEC).
pub fn draw_mouse(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdbec)?;
    show(&mut Ctx::new(m))
}

/// `draw_mouse_cursor_if_needed` (DB67): shows the cursor again if it was
/// hidden for a rectangle ([DC47h] negative).
pub fn draw_mouse_cursor_if_needed(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdb67)?;
    let mut c = Ctx::new(m);
    let hidden = c.byte(g::CURSOR_HIDDEN_FOR_RECT);
    if hidden & 0x80 == 0 {
        c.run(start, 3)?;
        c.cmp(B, hidden as u16, 0);
        return c.ret();
    }
    c.run(start, 4)?;
    c.cmp(B, hidden as u16, 0);
    let v = c.inc(B, hidden as u16) as u8;
    c.set_byte(g::CURSOR_HIDDEN_FOR_RECT, v);
    show(&mut c)
}

/// `restore_mouse_if_rect_intersects` (DB74): hides the cursor (noting it
/// at [DC47h]) if its 16×16 square, from where it was drawn less the
/// image's hot spot, meets the rectangle at SI.
pub fn restore_mouse_if_rect_intersects(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdb74)?;
    let mut c = Ctx::new(m);
    let shown = c.byte(g::CURSOR_SHOWN);
    if shown & 0x80 != 0 {
        c.run(start, 3)?;
        c.cmp(B, shown as u16, 0);
        return c.ret();
    }
    c.check(start, 23)?;
    c.cmp(B, shown as u16, 0);
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    let image = c.word(g::MOUSE_CURSOR_IMAGE);
    let (hx, hy) = (c.word(image), c.word(image.wrapping_add(2)));
    let x0 = c.word(g::CURSOR_X);
    let x = c.sub(W, x0, hx);
    let y = c.neg(W, hy);
    let y0 = c.word(g::CURSOR_Y);
    let y = c.add(W, y, y0);
    let si = c.reg(SI);
    let (l, t, r, b) = (c.word(si), c.word(si + 2), c.word(si + 4), c.word(si + 6));
    let mut n = 12;
    let mut meets = false;
    let (mut dx, mut bx) = (x, y);
    c.cmp(W, dx, r);
    if (dx as i16) < (r as i16) {
        c.cmp(W, bx, b);
        n += 2;
        if (bx as i16) < (b as i16) {
            dx = c.add(W, dx, 0x10);
            c.cmp(W, dx, l);
            n += 3;
            if (dx as i16) > (l as i16) {
                bx = c.add(W, bx, 0x10);
                c.cmp(W, bx, t);
                n += 3;
                meets = (bx as i16) > (t as i16);
            }
        }
    }
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    let dx = c.pop();
    c.set_reg(DX, dx);
    let bx = c.pop();
    c.set_reg(BX, bx);
    if !meets {
        c.count(n + 3);
        return c.ret();
    }
    c.count(n + 3);
    let v = c.byte(g::CURSOR_HIDDEN_FOR_RECT);
    let v = c.dec(B, v as u16) as u8;
    c.set_byte(g::CURSOR_HIDDEN_FOR_RECT, v);
    hide(&mut c)
}

/// `redraw_mouse` (DC20): the cursor image for the mouse position; shows
/// the cursor (count to 0), redrawn where the position or image changed.
/// Registers kept.
pub fn redraw_mouse(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdc20)?;
    let mut c = Ctx::new(m);
    c.run(start, 10)?;
    for r in [AX, BX, CX, DX, SI, DI, BP] {
        let v = c.reg(r);
        c.push(v);
    }
    let (x, y) = (c.word(g::MOUSE_X), c.word(g::MOUSE_Y));
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    c.call(code::GET_MOUSE_CURSOR_IMAGE_ADDR, 0xdc32)?;
    let image = c.reg(BP);
    let before = c.word(g::MOUSE_CURSOR_IMAGE);
    let shown = c.byte(g::CURSOR_SHOWN);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let (cx0, cy0) = (c.word(g::CURSOR_X), c.word(g::CURSOR_Y));
    let n = if shown & 0x80 != 0 {
        6
    } else if dx != cx0 {
        8
    } else if bx != cy0 {
        10
    } else {
        12
    };
    let unchanged = shown & 0x80 == 0 && dx == cx0 && bx == cy0 && image == before;
    c.run(0xdc32, n + if unchanged { 8 } else { 0 })?;
    c.set_reg(SI, image);
    c.set_word(g::MOUSE_CURSOR_IMAGE, image);
    c.set_reg(BP, before);
    c.set_reg8(r8::AL, shown);
    c.set_byte(g::CURSOR_SHOWN, 0);
    if shown & 0x80 == 0 {
    }
    if !unchanged {
        if shown & 0x80 == 0 {
            c.interp(0xdc52, 0xdc56)?; // restore under the old one
        }
        c.run(0xdc56, 2)?;
        c.set_word(g::CURSOR_X, dx);
        c.set_word(g::CURSOR_Y, bx);
        c.interp(0xdc5e, 0xdc62)?; // draw
        c.run(0xdc62, 8)?;
    }
    for r in [BP, DI, SI, DX, CX, BX, AX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// `get_mouse_cursor_image_addr` (DC6A): BP = the cursor image for the
/// mouse at (DX, BX): busy while [28BEh] is set; the arrow while [4723h]
/// is set, without a mouse rectangle, or below the game area (y 9Bh);
/// over the rectangle at [DC58h], an arrow pointing to the edge it is
/// within 32h (x) or 19h (y) of, else the plain arrow; `sub_d6fe` may
/// say busy (CF).
pub fn get_mouse_cursor_image_addr(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdc6a)?;
    let mut c = Ctx::new(m);
    let busy = c.byte(g::FLAG_28BE);
    if busy != 0 {
        c.run(start, 4)?;
        c.cmp(B, busy as u16, 0);
        c.set_reg(BP, cursor::BUSY);
        return c.ret();
    }
    let plain = c.byte(g::CURSOR_PLAIN);
    let rect = c.word(g::MOUSE_RECT);
    let bx = c.reg(BX);
    let below = (bx as i16) >= 0x9b;
    let n = if plain != 0 {
        7
    } else if rect == 0 {
        10
    } else if below {
        12
    } else {
        12
    };
    c.run(start, n)?;
    c.cmp(B, 0, 0);
    c.set_reg(BP, cursor::ARROW);
    c.cmp(B, plain as u16, 0);
    if plain != 0 {
        return c.ret();
    }
    c.set_reg(DI, rect);
    c.or(W, rect, rect);
    if rect == 0 {
        return c.ret();
    }
    c.cmp(W, bx, 0x9b);
    if below {
        return c.ret();
    }
    c.call(code::SUB_D6FE, 0xdc8f)?;
    let failed = c.flag(pc::cpu::CF);
    c.check(0xdc8f, 19)?;
    c.set_reg(BP, cursor::BUSY);
    let mut n = 2;
    if failed {
        c.count(n + 1);
        return c.ret();
    }
    let di = c.reg(DI);
    let (l, t, r, b) = (c.word(di), c.word(di + 2), c.word(di + 4), c.word(di + 6));
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let lt = |a: u16, b: u16| (a as i16) < (b as i16);
    c.cmp(W, bx, t);
    n += 2;
    let image;
    if !lt(bx, t) && {
        c.cmp(W, bx, b);
        n += 2;
        lt(bx, b)
    } {
        // Within the rectangle's rows: near its left or right side.
        c.set_reg(BP, cursor::DOWN);
        let d = c.sub(W, l, dx);
        c.set_reg(AX, d);
        c.cmp(W, d, 0x32);
        n += 5;
        if d < 0x32 {
            return finish(&mut c, n + 1);
        }
        c.set_reg(BP, cursor::UP);
        let d = c.sub(W, dx, r);
        c.set_reg(AX, d);
        c.cmp(W, d, 0x32);
        n += 5;
        if d < 0x32 {
            return finish(&mut c, n + 1);
        }
        n += 1; // jmp DCDCh
        image = cursor::ARROW;
    } else {
        c.cmp(W, dx, l);
        n += 2;
        if lt(dx, l) || {
            c.cmp(W, dx, r);
            n += 2;
            !lt(dx, r)
        } {
            image = cursor::ARROW;
        } else {
            c.set_reg(BP, cursor::LEFT);
            let d = c.sub(W, t, bx);
            c.set_reg(AX, d);
            c.cmp(W, d, 0x19);
            n += 5;
            if d < 0x19 {
                return finish(&mut c, n + 1);
            }
            c.set_reg(BP, cursor::RIGHT);
            let d = c.sub(W, bx, b);
            c.set_reg(AX, d);
            c.cmp(W, d, 0x19);
            n += 5;
            if d < 0x19 {
                return finish(&mut c, n + 1);
            }
            image = cursor::ARROW;
        }
    }
    c.set_reg(BP, image);
    finish(&mut c, n + 2)
}

/// Counts the instructions of `get_mouse_cursor_image_addr`'s path, then
/// `ret`.
fn finish(c: &mut Ctx, n: u32) -> Result<(), Bail> {
    c.count(n);
    c.ret()
}
