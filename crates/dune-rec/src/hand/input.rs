//! Input: keyboard scancode, mouse buttons and position.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BX, DX, SI},
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
