//! Input: keyboard scancode, mouse buttons and position.

use super::{B, Ctx, entered, globals as g, r8};
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
