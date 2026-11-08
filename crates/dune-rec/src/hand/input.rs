//! Input: keyboard scancode, mouse buttons and position.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BX, CX, DX, SI},
    rec::Bail,
};

/// `clear_some_mouse_rect` (DAA3).
pub fn clear_some_mouse_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdaa3)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_word(g::MOUSE_RECT, 0);
    c.ret()
}

/// `set_some_mouse_rect` (DAAA): the rectangle at SI.
pub fn set_some_mouse_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdaaa)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let si = c.reg(SI);
    c.set_word(g::MOUSE_RECT, si);
    c.ret()
}

/// `mouse_stuff` (DB4C): the mouse buttons and position. Records the
/// buttons (bits 0..1) as seen; returns AX = buttons | 4 × (newly
/// changed bits of the state byte), (DX, BX) = position.
/// Flags from `xor ah, ah`.
pub fn mouse_stuff(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdb4c)?;
    let mut c = Ctx::new(m);
    c.run(start, 11)?;
    let state = c.word(g::MOUSE_BUTTONS);
    let buttons = c.and(B, state & 0xff, 3);
    c.set_byte(g::MOUSE_BUTTONS_SEEN, buttons as u8);
    let changed = c.xor(B, state >> 8, buttons);
    let changed = c.add(B, changed, changed);
    let changed = c.add(B, changed, changed);
    let al = c.or(B, buttons, changed);
    c.xor(B, changed, changed);
    c.set_reg(AX, al);
    let (x, y) = (c.word(g::MOUSE_X), c.word(g::MOUSE_Y));
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    c.ret()
}

/// `get_and_reset_key_scancode` (DD5A): AL = the last key's scancode
/// (taken, so it reads once); ZF set if none. Flags from `or al, al`.
pub fn get_and_reset_key_scancode(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdd5a)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let al = c.reg8(r8::AL);
    c.xor(B, al as u16, al as u16);
    let key = c.byte(g::KEY_HIT);
    c.set_byte(g::KEY_HIT, 0);
    c.set_reg8(r8::AL, key);
    c.or(B, key as u16, key as u16);
    c.ret()
}

/// `kb_clear_scancode` (DE4E): forget the last key.
pub fn kb_clear_scancode(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xde4e)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(g::KEY_HIT, 0);
    c.ret()
}

/// `kb_check_for_esc_key_hit` (DE54): if the last key was Esc (scancode
/// 1), take it and set the Esc flag; else clear the flag. ZF set for Esc
/// (flags from the `cmp`).
pub fn kb_check_for_esc_key_hit(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xde54)?;
    let mut c = Ctx::new(m);
    let key = c.byte(g::KEY_HIT);
    let esc = key == 1;
    c.run(start, if esc { 7 } else { 4 })?;
    c.set_byte(g::ESC_HIT, 0);
    c.cmp(B, key as u16, 1);
    if esc {
        c.set_byte(g::ESC_HIT, 1);
        c.set_byte(g::KEY_HIT, 0);
    }
    c.ret()
}

/// `loc_0dd10`: the joystick (`read_game_port`) as keys: [CEE6h] FFh with
/// a button (bits 30h); [CEE1h]/[CEDFh] FFh for left/right of the dead
/// zone at 39ABh/39ADh, [CEE4h]/[CEDCh] for up/down of 39AFh/39B1h.
pub fn joystick_as_keys(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdd10)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xdce0, 0xdd13)?;
    let (al, dx, bx) = (c.reg8(super::r8::AL), c.reg(DX), c.reg(BX));
    let button = al & 0x30 != 0;
    let (x0, x1, y0, y1) = (c.word(0x39ab), c.word(0x39ad), c.word(0x39af), c.word(0x39b1));
    let axis = |v: u16, lo: u16, hi: u16| -> (u32, u16) {
        if v <= lo {
            (5, 0xff00)
        } else if v >= hi {
            (5, 0x00ff)
        } else {
            (6, 0)
        }
    };
    let (nx, ax_x) = axis(dx, x0, x1);
    let (ny, ax_y) = axis(bx, y0, y1);
    c.run(0xdd13, 4 + button as u32 + 1 + nx + 2 + ny + 3)?;
    c.set_reg8(super::r8::AH, al);
    c.xor(B, 0, 0);
    c.set_reg8(super::r8::AL, 0);
    let mut v = 0u8;
    c.test(B, al as u16, 0x30);
    if button {
        v = 0xff;
    }
    c.set_byte(0xcee6, v);
    for (val, lo, hi, a, b, res) in [(dx, x0, x1, 0xcee1u16, 0xcedfu16, ax_x), (bx, y0, y1, 0xcee4, 0xcedc, ax_y)] {
        c.cmp(W, val, lo);
        if val > lo {
            c.cmp(W, val, hi);
            if val < hi {
                c.xor(W, 0xff, 0xff);
            }
        }
        c.set_reg(AX, res);
        c.set_byte(a, res as u8);
        c.set_byte(b, (res >> 8) as u8);
    }
    c.ret()
}

/// `loc_0daaf`: the cursor moved by (DX, BX), clamped (signed) to the
/// bounds at 0DC3Ah, into [0DC36h], [0DC38h]; unless bit 40h of [2942h],
/// the mouse driver told (INT 33h function 4, scaled by the shifts in
/// [2580h]).
pub fn move_cursor_by(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdaaf)?;
    let mut c = Ctx::new(m);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let b = [c.word(0xdc3a), c.word(0xdc3c), c.word(0xdc3e), c.word(0xdc40)];
    let (x0, y0) = (c.word(0xdc36), c.word(0xdc38));
    let f = c.byte(0x2942);
    let mut n = 3;
    let mut clamp = |v: u16, lo: u16, hi: u16| -> u16 {
        let mut v = v;
        n += 2;
        if (v as i16) < lo as i16 {
            v = lo;
            n += 1;
        }
        n += 3;
        if (v as i16) > hi as i16 {
            v = hi;
            n += 1;
        }
        v
    };
    let x = clamp(x0.wrapping_add(dx), b[0], b[1]);
    let y = clamp(y0.wrapping_add(bx), b[2], b[3]);
    n += 7;
    let tell = f & 0x40 == 0;
    c.run(start, n + if tell { 8 } else { 1 })?;
    let replay = |c: &mut Ctx, v: u16, lo: u16, hi: u16| -> u16 {
        c.cmp(W, v, lo);
        let v = if (v as i16) < lo as i16 { lo } else { v };
        c.cmp(W, v, hi);
        if (v as i16) > hi as i16 { hi } else { v }
    };
    let a = c.add(W, x0, dx);
    let v = replay(&mut c, a, b[0], b[1]);
    c.set_word(0xdc36, v);
    let a = c.add(W, y0, bx);
    let v = replay(&mut c, a, b[2], b[3]);
    c.set_word(0xdc38, v);
    c.set_reg(SI, 0xdc40);
    c.set_reg(AX, v);
    c.test(B, f as u16, 0x40);
    if !tell {
        return c.ret();
    }
    let sh = c.word(0x2580);
    let ax = c.shl(W, x, sh as u8);
    let d = c.shl(W, y, (sh >> 8) as u8);
    c.set_reg(DX, d);
    c.set_reg(CX, ax);
    c.set_reg(AX, 4);
    c.interp(0xdb00, 0xdb02)?;
    c.run(0xdb02, 1)?;
    c.ret()
}

/// `loc_0d962`: the cursor's glide toward its target ([0DC4Ch],
/// [0DC4Eh]): every 6 ticks or more (from [0DC4Ah]) one step of the
/// distance / 2^CX (CX 2, 1 or 0 for gaps up to 0Ch, 18h and more; CX 0
/// takes 3/4) by [`move_cursor_by`], [0DC4Bh] counted down (0 when
/// there); then [`mouse_stuff`].
pub fn cursor_glide(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd962)?;
    let mut c = Ctx::new(m);
    let t = c.word(0xce7a);
    let last = c.byte(0xdc4a);
    let gap = (t as u8).wrapping_sub(last);
    if gap < 6 {
        c.run(start, 5)?;
        c.set_reg(AX, t);
        let a = c.sub(B, t & 0xff, last as u16);
        c.set_reg8(r8::AL, a as u8);
        c.cmp(B, a, 6);
        return mouse_stuff(c.m, 0xdb4c);
    }
    let cx: u16 = if gap < 0x0c { 2 } else if gap < 0x18 { 1 } else { 0 };
    let (tx, ty) = (c.word(0xdc4c), c.word(0xdc4e));
    let (cx0, cy0) = (c.word(0xdc36), c.word(0xdc38));
    let dx = tx.wrapping_sub(cx0);
    let dy = ty.wrapping_sub(cy0);
    let leg = |d: u16| -> u32 {
        if d == 0 { 0 } else if cx != 0 { 4 } else { 5 }
    };
    let n = 4 + 3 + if gap >= 0x0c { 3 + (gap >= 0x18) as u32 } else { 0 } + 7 + leg(dx) + 2 + leg(dy) + 3;
    let fdx = step(dx, cx);
    let fdy = step(dy, cx);
    let zero = fdx | fdy == 0;
    c.run(start, n + if zero { 3 } else { 1 })?;
    c.set_reg(AX, t);
    let a = c.sub(B, t & 0xff, last as u16);
    c.cmp(B, a, 6);
    c.set_reg(CX, 2);
    c.cmp(B, a, 0x0c);
    if gap >= 0x0c {
        c.dec(W, 2);
        c.cmp(B, a, 0x18);
        if gap >= 0x18 {
            c.dec(W, 1);
        }
    }
    c.set_reg(CX, cx);
    c.set_reg(AX, t);
    c.set_byte(0xdc4a, t as u8);
    let k = c.byte(0xdc4b);
    let k = c.dec(B, k as u16);
    c.set_byte(0xdc4b, k as u8);
    let d = leg_ops(&mut c, tx, cx0, cx);
    let b = leg_ops(&mut c, ty, cy0, cx);
    c.set_reg(DX, d);
    c.set_reg(BX, b);
    c.set_reg(AX, b);
    c.or(W, b, d);
    c.set_reg(AX, b | d);
    if zero {
        c.set_byte(0xdc4b, 0);
        return mouse_stuff(c.m, 0xdb4c);
    }
    c.call(0xdaaf, 0xd9ca)?;
    c.run(0xd9ca, 2)?;
    c.set_byte(0xdc34, 0);
    mouse_stuff(c.m, 0xdb4c)
}

/// The step for one axis of [`cursor_glide`].
fn step(d: u16, cx: u16) -> u16 {
    if d == 0 {
        0
    } else if cx != 0 {
        ((d as i16) >> cx) as u16 | 1
    } else {
        d.wrapping_sub(((d as i16) >> 2) as u16)
    }
}

fn leg_ops(c: &mut Ctx, to: u16, from: u16, cx: u16) -> u16 {
    let d = c.sub(W, to, from);
    if d == 0 {
        return 0;
    }
    if cx != 0 {
        let s = c.m.cpu.shift(7, W, d as u32, cx as u32) as u16;
        c.or(B, s & 0xff, 1) | s & 0xff00
    } else {
        let a = c.m.cpu.shift(7, W, d as u32, 1) as u16;
        let a = c.m.cpu.shift(7, W, a as u32, 1) as u16;
        c.set_reg(AX, a);
        c.sub(W, d, a)
    }
}
