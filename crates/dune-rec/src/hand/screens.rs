//! The stack of UI screens: [`TOP`] points at the current entry (its list
//! of UI elements, its handler), entries 4 bytes from [`BOTTOM`]. A list
//! starts with a byte: its kind in the low 4 bits (0Fh: none to leave),
//! FFh at the end of the stack.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CS, CX, DI, DX, ES, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
    pub const CURRENT_UI_ELEMENTS: u16 = 0xd41b;
    pub const CALL_RESTORE_CURSOR: u16 = 0xdbb2;
    pub const DRAW_MOUSE: u16 = 0xdbec;
    pub const HIGHLIGHT_HOVERED_TEXT_ACTION_ITEM: u16 = 0xd50f;
    pub const DRAW_UI_ELEMENTS_LIST: u16 = 0xd1f2;
}

const TOP: u16 = 0x21da;
const BOTTOM: u16 = 0x21be;
/// Bit 7: drawing for a transition; bit 0: redraw asked.
const TRANSITION: u16 = g::TRANSITION_ACTIVE;
/// The cursor's UI element (its sprite at +2).
const CURSOR_ELEMENT: u16 = 0x1f7e;

/// `loc_0d2fd`: back to the screen below (unless at the bottom),
/// `loc_0d36d` with CL = FFh on its list (BP).
pub fn pop_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd2fd)?;
    pop(&mut Ctx::new(m), start, 0, |_| {})
}

/// From D2FD (the stretch from `at`, `prefix` instructions before, done by
/// `ops`).
fn pop(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    let si = c.word(TOP);
    let bottom = si == BOTTOM;
    c.run(at, prefix + if bottom { 4 } else { 8 })?;
    ops(c);
    c.set_reg(SI, si);
    c.cmp(W, si, BOTTOM);
    if bottom {
        return c.ret();
    }
    let si = c.sub(W, si, 4);
    c.set_reg(SI, si);
    c.set_word(TOP, si);
    let bp = c.word(si);
    c.set_reg(BP, bp);
    c.set_reg8(r8::CL, 0xff);
    c.call(0xd36d, 0xd315)?;
    c.run(0xd315, 1)?;
    c.ret()
}

/// `loc_0d2ea`: leaves the current screen: its handler (word 2), then
/// [`pop_screen`]; nothing for a list of kind 0Fh.
pub fn leave_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd2ea)?;
    let mut c = Ctx::new(m);
    let si = c.word(TOP);
    let di = c.word(si);
    let kind = c.byte(di) & 0x0f;
    c.run(start, if kind == 0x0f { 7 } else { 8 })?;
    c.set_reg(SI, si);
    c.set_reg(DI, di);
    let b = c.byte(di);
    let al = c.and(B, b as u16, 0x0f);
    c.set_reg8(r8::AL, al as u8);
    c.cmp(B, al, 0x0f);
    if kind == 0x0f {
        return c.ret();
    }
    let h = c.word(si.wrapping_add(2));
    c.set_reg(AX, h);
    c.call(h, 0xd2fd)?;
    pop(&mut c, 0xd2fd, 0, |_| {})
}

/// `loc_0d2bd`: leaves screens until the current list is the stack's end
/// (FFh) or of kind 0; drawn as for a transition (bit 7), the flag kept.
pub fn leave_screens(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd2bd)?;
    let mut c = Ctx::new(m);
    let (mut at, mut extra) = (start, 2);
    let saved = c.byte(TRANSITION);
    let mut first = true;
    loop {
        let top = c.word(TOP);
        let list = c.word(top);
        let b = c.byte(list);
        let stop = b == 0xff || b & 0x0f == 0;
        c.run(at, extra + 6 + if b == 0xff { 0 } else { 2 } + if stop { 3 } else { 1 })?;
        if first {
            c.set_reg8(r8::AL, saved);
            let ax = c.reg(AX);
            c.push(ax);
            first = false;
        }
        c.set_byte(TRANSITION, 0x80);
        c.set_reg(SI, list);
        c.lods(B, pc::cpu::DS);
        c.cmp(B, b as u16, 0xff);
        if b != 0xff {
            let al = c.and(B, b as u16, 0x0f);
            c.set_reg8(r8::AL, al as u8);
        }
        if stop {
            let ax = c.pop();
            c.set_reg(AX, ax);
            c.set_byte(TRANSITION, ax as u8);
            return c.ret();
        }
        c.call(0xd2ea, 0xd2d8)?;
        (at, extra) = (0xd2d8, 1);
    }
}

/// `loc_0d316`: a redraw asked (bit 0 of the transition flag) when
/// [35A6h] is 0.
pub fn ask_redraw(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd316)?;
    let mut c = Ctx::new(m);
    let v = c.word(0x35a6);
    c.run(start, if v == 0 { 4 } else { 3 })?;
    c.cmp(W, v, 0);
    if v == 0 {
        let t = c.byte(TRANSITION);
        let t = c.or(B, t as u16, 1);
        c.set_byte(TRANSITION, t as u8);
    }
    c.ret()
}

/// `loc_0d454`: AX, BX = the place of item CL of the current list (its
/// 4-byte table after the list's header); for CL = [DCE5h] (the text
/// action item) AX = A0h and BX one of D423h, D429h, 0 by [DCE4h]'s sign.
/// CH = 0.
pub fn item_place(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd454)?;
    let mut c = Ctx::new(m);
    let top = c.word(TOP);
    let list = c.word(top);
    let cl = c.reg8(r8::CL);
    let special = c.byte(0xdce5);
    let text = cl == special;
    let k = c.byte(0xdce4) as i8;
    c.run(start, 6 + if !text { 10 } else if k < 0 { 5 } else if k > 0 { 7 } else { 8 })?;
    let si = list.wrapping_add(1);
    c.set_reg(SI, si);
    c.set_reg8(r8::CH, 0);
    let ch = c.reg8(r8::CH);
    c.xor(B, ch as u16, ch as u16);
    c.cmp(B, cl as u16, special as u16);
    if !text {
        let n = c.lods(B, pc::cpu::DS) as u8;
        let skip = n as i8 as u16;
        c.set_reg(AX, skip);
        let si = c.reg(SI);
        let si = c.add(W, si, skip);
        let off = c.shl1(W, cl as u16, 2);
        c.set_reg(AX, off);
        let si = c.add(W, si, off);
        c.set_reg(SI, si);
        let (x, y) = (c.word(si), c.word(si.wrapping_add(2)));
        c.set_reg(AX, x);
        c.set_reg(BX, y);
        return c.ret();
    }
    c.set_reg(AX, 0xa0);
    c.set_reg(BX, 0xd423);
    c.cmp(B, k as u8 as u16, 0);
    if k >= 0 {
        c.set_reg(BX, 0xd429);
        if k == 0 {
            let z = c.xor(W, 0xd429, 0xd429);
            c.set_reg(BX, z);
        }
    }
    c.ret()
}

/// From D621 (the stretch from `at`, `prefix` instructions before, done by
/// `ops`): the cursor's sprite AX; when it changes on the cursor's own
/// list, the cursor redrawn with the first item's place (`loc_0d454`,
/// `loc_0d48a`). Registers kept (AX pushed before).
fn set_cursor_sprite(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 1)?;
    ops(c);
    c.call(code::SAVE_REGS, 0xd624)?;
    c.run(0xd624, 1)?;
    c.call(code::CURRENT_UI_ELEMENTS, 0xd627)?;
    let (ax, bp) = (c.reg(AX), c.reg(BP));
    let old = c.word(CURSOR_ELEMENT + 2);
    let same = old == ax;
    let ours = bp == CURSOR_ELEMENT;
    c.run(0xd627, 4 + if same { 1 } else if !ours { 3 } else { 3 })?;
    c.set_reg(SI, CURSOR_ELEMENT);
    c.cmp(W, old, ax);
    c.set_word(CURSOR_ELEMENT + 2, ax);
    if !same {
        c.cmp(W, bp, CURSOR_ELEMENT);
        if ours {
            c.call(code::CALL_RESTORE_CURSOR, 0xd639)?;
            c.run(0xd639, 2)?;
            let cx = c.reg(CX);
            c.xor(W, cx, cx);
            c.set_reg(CX, 0);
            c.call(0xd454, 0xd63e)?;
            c.run(0xd63e, 1)?;
            c.call(0xd48a, 0xd641)?;
            c.run(0xd641, 2)?;
            c.set_byte(0xdce7, 0xff);
            c.call(code::DRAW_MOUSE, 0xd649)?;
            c.run(0xd649, 1)?;
        }
    }
    c.call(code::RESTORE_REGS, 0xd64c)?;
    c.run(0xd64c, 2)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// `loc_0d617`: the cursor's sprite 90h (see [`set_cursor_sprite`]).
pub fn cursor_sprite_90(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd617)?;
    set_cursor_sprite(&mut Ctx::new(m), start, 3, |c| {
        let ax = c.reg(AX);
        c.push(ax);
        c.set_reg(AX, 0x90);
    })
}

/// `loc_0d61d`: the cursor's sprite 9Fh (see [`set_cursor_sprite`]).
pub fn cursor_sprite_9f(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd61d)?;
    set_cursor_sprite(&mut Ctx::new(m), start, 2, |c| {
        let ax = c.reg(AX);
        c.push(ax);
        c.set_reg(AX, 0x9f);
    })
}

/// `loc_0d64e`: `highlight_hovered_text_action_item` at (0, 0) (BX, DX
/// kept).
pub fn highlight_at_origin(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd64e)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let (bx, dx) = (c.reg(BX), c.reg(pc::cpu::DX));
    c.push(bx);
    c.push(dx);
    c.xor(W, bx, bx);
    c.set_reg(BX, 0);
    c.xor(W, dx, dx);
    c.set_reg(pc::cpu::DX, 0);
    c.call(code::HIGHLIGHT_HOVERED_TEXT_ACTION_ITEM, 0xd657)?;
    c.run(0xd657, 3)?;
    let dx = c.pop();
    c.set_reg(pc::cpu::DX, dx);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.ret()
}

/// `loc_0d65a`: the UI element at DI drawn pressed (word 0Ah one up for
/// the while), when it has bit 20h of byte 9; noted at [DC60h].
pub fn draw_element_pressed(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd65a)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let f = c.byte(di.wrapping_add(9));
    let on = f & 0x20 != 0;
    c.run(start, if on { 9 } else { 3 })?;
    c.test(B, f as u16, 0x20);
    if !on {
        return c.ret();
    }
    c.set_word(0xdc60, di);
    let v = c.word(di.wrapping_add(0x0a));
    let v = c.inc(W, v);
    c.set_word(di.wrapping_add(0x0a), v);
    let si = c.reg(SI);
    c.push(si);
    c.push(di);
    c.set_reg(SI, di);
    c.set_reg(CX, 1);
    c.call(code::DRAW_UI_ELEMENTS_LIST, 0xd671)?;
    c.run(0xd671, 4)?;
    let di = c.pop();
    c.set_reg(DI, di);
    let si = c.pop();
    c.set_reg(SI, si);
    let v = c.word(di.wrapping_add(0x0a));
    let v = c.dec(W, v);
    c.set_word(di.wrapping_add(0x0a), v);
    c.ret()
}

/// `loc_0d741`: for [1B0Ch] 3..5, the rect at 2458h drawn by the driver
/// (function at 38DDh) in colour F0h on the screen buffer.
pub fn draw_rect_2458(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd741)?;
    let mut c = Ctx::new(m);
    let v = c.word(0x1b0c);
    let d = v.wrapping_sub(3);
    c.run(start, if d >= 3 { 5 } else { 7 })?;
    let d = c.sub(W, v, 3);
    c.set_reg(AX, d);
    c.cmp(W, d, 3);
    if d >= 3 {
        return c.ret();
    }
    c.set_reg(SI, 0x2458);
    let screen = c.word(g::SCREEN_BUFFER);
    c.set_sreg(ES, screen);
    c.set_reg8(r8::AL, 0xf0);
    c.interp(0xd755, 0xd759)?;
    c.run(0xd759, 1)?;
    c.ret()
}

/// `loc_0d95e`: the click handler AX.
pub fn set_click_handler(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd95e)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let ax = c.reg(AX);
    c.set_word(g::MAP_CLICK_HANDLER, ax);
    c.ret()
}

/// `loc_0d0e3`: the index of AL in the 9 characters at CS:D0D1h: AL =
/// the byte 8 after it, AH = 5 + its index (CF clear); CF set when not
/// there. CX, DI, ES kept.
pub fn special_char(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd0e3)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    let (cx, di, es) = (c.reg(CX), c.reg(DI), c.sreg(ES));
    c.push(cx);
    c.push(di);
    c.push(es);
    let cs = c.sreg(CS);
    c.push(cs);
    let v = c.pop();
    c.set_sreg(ES, v);
    c.set_reg(DI, 0xd0d1);
    c.set_reg(CX, 9);
    c.m.cpu.string_op(&mut c.m.hw, 0xae, None, 0xf2); // repne scasb
    let found = c.flag(pc::cpu::ZF);
    c.run(0xd0f0, if found { 6 } else { 3 } + 3)?;
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_flag(CF, true);
    if found {
        let di = c.reg(DI);
        let al = c.byte_in(CS, di.wrapping_add(8));
        c.set_reg8(r8::AL, al);
        let left = c.reg8(r8::CL);
        let ah = c.sub(B, 0x0d, left as u16) as u8;
        c.set_reg8(r8::AH, ah);
    }
    let di = c.pop();
    c.set_reg(DI, di);
    let cx = c.pop();
    c.set_reg(CX, cx);
    c.ret()
}

/// `loc_0db03`: the cursor moved to (DX, BX): restored, the position set
/// (`set_mouse_pos`), drawn again.
pub fn move_cursor(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdb03)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::CALL_RESTORE_CURSOR, 0xdb06)?;
    c.run(0xdb06, 3)?;
    let (dx, bx) = (c.reg(pc::cpu::DX), c.reg(BX));
    c.set_word(g::MOUSE_X, dx);
    c.set_word(g::MOUSE_Y, bx);
    c.call(0xdae3, 0xdb11)?;
    c.run(0xdb11, 1)?;
    super::mouse::draw_mouse(c.m, 0xdbec)
}

/// `loc_0df07`: the 5 UI elements at 1B48h drawn, the sprite sheet
/// [2784h] opened again, `loc_0d397`, and the cursor drawn.
pub fn draw_1b48_and_cursor(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdf07)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let sheet = c.word(0x2784);
    c.push(sheet);
    c.set_reg(SI, 0x1b48);
    c.set_reg(CX, 5);
    c.call(code::DRAW_UI_ELEMENTS_LIST, 0xdf14)?;
    c.run(0xdf14, 2)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.call(0xc13e, 0xdf18)?;
    c.run(0xdf18, 1)?;
    c.call(0xd397, 0xdf1b)?;
    c.run(0xdf1b, 1)?;
    super::mouse::draw_mouse(c.m, 0xdbec)
}

/// From D72B (the stretch from `at`, `prefix` instructions before, done by
/// `ops`): the 6 UI elements at SI copied to 1B8Eh, the rect at 2458h
/// (`loc_0d741`), and the copies drawn.
fn set_buttons(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 5)?;
    ops(c);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(DI, BUTTONS);
    c.set_reg(CX, 0x2a);
    c.rep(0xa5);
    c.run(0xd735, 1)?;
    c.call(0xd741, 0xd738)?;
    c.run(0xd738, 3)?;
    c.set_reg(SI, BUTTONS);
    c.set_reg(CX, 6);
    super::gfx::draw_ui_elements_list_at_ds_si(c.m, 0xd1f2)
}

/// The 6 buttons of the panel (UI elements).
const BUTTONS: u16 = 0x1b8e;

/// `loc_0d72b`: the buttons from SI (see [`set_buttons`]).
pub fn set_buttons_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd72b)?;
    set_buttons(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_0d717`: the buttons for the screen: 1CCAh with hotspots (46EBh),
/// else 1D72h with bits 3 of [11C9h], else 1C76h.
pub fn set_buttons_for_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd717)?;
    let mut c = Ctx::new(m);
    let kinds = c.byte(0x46eb);
    let f = c.byte(0x11c9);
    let prefix = if kinds != 0 { 4 } else if f & 3 == 0 { 5 } else { 6 };
    set_buttons(&mut c, start, prefix, |c| {
        c.cmp(B, kinds as u16, 0);
        if kinds != 0 {
            c.set_reg(SI, 0x1cca);
            return;
        }
        c.set_reg(SI, 0x1c76);
        c.test(B, f as u16, 3);
        if f & 3 != 0 {
            c.set_reg(SI, 0x1d72);
        }
    })
}

/// `loc_0d763`: the two gauges at 1C0Ch drawn at 40h, then at 41h + the
/// two signed bytes at 1152h.
pub fn draw_gauges(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd763)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let si = 0x1c0cu16;
    c.set_reg(SI, si);
    c.set_reg(AX, 0x40);
    c.set_word(si + 0x0a, 0x40);
    c.set_word(si + 0x18, 0x40);
    c.set_reg(CX, 2);
    c.call(code::DRAW_UI_ELEMENTS_LIST, 0xd775)?;
    c.run(0xd775, 11)?;
    c.set_reg(SI, si);
    let a = c.byte(0x1152) as i8 as u16;
    let a = c.add(W, a, 0x41);
    c.set_word(si + 0x0a, a);
    let b = c.byte(0x1153) as i8 as u16;
    let b = c.add(W, b, 0x41);
    c.set_reg(AX, b);
    c.set_word(si + 0x18, b);
    c.set_reg(CX, 2);
    super::gfx::draw_ui_elements_list_at_ds_si(c.m, 0xd1f2)
}

/// `loc_0d6b7`: the UI element of the current list (or of 1B48h's five
/// with [46D9h]) under the cursor (DX, BX): DI = it, CF set; CF clear when
/// none (ZF set and nothing done when there is no list for the cursor).
pub fn element_under_cursor(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd6b7)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xd694, 0xd6ba)?;
    if c.flag(pc::cpu::ZF) {
        c.run(0xd6ba, 2)?;
        return c.ret();
    }
    let (dx, bx) = (c.reg(pc::cpu::DX), c.reg(BX));
    let list = 0x1ae4u16;
    let mut cx = c.word(list);
    let mut di = list.wrapping_add(2);
    let handler = c.word(g::MAP_CLICK_HANDLER);
    let few = handler == 0x1ad6;
    if few {
        cx = cx.wrapping_sub(5);
    }
    let alt = c.byte(0x46d9) != 0;
    if alt {
        cx = 5;
        di = 0x1b48;
    }
    let mut n = 1 + 5 + few as u32 + 2 + if alt { 2 } else { 0 };
    // The scan, purely.
    let (mut k, mut d) = (cx, di);
    loop {
        let f = c.byte(d.wrapping_add(8));
        let r = [c.word(d), c.word(d.wrapping_add(2)), c.word(d.wrapping_add(4)), c.word(d.wrapping_add(6))];
        n += 2;
        let inside = if f & 0x80 == 0 {
            false
        } else {
            n += 2;
            if dx <= r[0] {
                false
            } else {
                n += 2;
                if dx >= r[2] {
                    false
                } else {
                    n += 2;
                    if bx <= r[1] {
                        false
                    } else {
                        n += 4;
                        bx.wrapping_sub(1) < r[3]
                    }
                }
            }
        };
        if inside {
            n += 1;
            break;
        }
        n += 2;
        k = k.wrapping_sub(1);
        if k == 0 {
            n += 2;
            break;
        }
        d = d.wrapping_add(0x0e);
    }
    c.run(0xd6ba, n)?;
    c.set_reg(DI, list);
    let w = c.word(list);
    c.set_reg(CX, w);
    let d0 = c.add(W, list, 2);
    c.set_reg(DI, d0);
    c.cmp(W, handler, 0x1ad6);
    if few {
        let v = c.sub(W, w, 5);
        c.set_reg(CX, v);
    }
    let a = c.byte(0x46d9);
    c.cmp(B, a as u16, 0);
    if alt {
        c.set_reg(CX, 5);
        c.set_reg(DI, 0x1b48);
    }
    loop {
        let d = c.reg(DI);
        let f = c.byte(d.wrapping_add(8));
        c.cmp(B, f as u16, 0);
        let mut inside = false;
        if f & 0x80 != 0 {
            let r0 = c.word(d);
            c.cmp(W, dx, r0);
            if dx > r0 {
                let r2 = c.word(d.wrapping_add(4));
                c.cmp(W, dx, r2);
                if dx < r2 {
                    let r1 = c.word(d.wrapping_add(2));
                    c.cmp(W, bx, r1);
                    if bx > r1 {
                        let b1 = c.dec(W, bx);
                        let r3 = c.word(d.wrapping_add(6));
                        c.cmp(W, b1, r3);
                        let cf = c.flag(CF);
                        c.inc(W, b1);
                        c.set_flag(CF, cf);
                        inside = b1 < r3;
                    }
                }
            }
        }
        if inside {
            return c.ret();
        }
        let d = c.add(W, d, 0x0e);
        c.set_reg(DI, d);
        let k = c.reg(CX).wrapping_sub(1);
        c.set_reg(CX, k);
        if k == 0 {
            c.set_flag(CF, false);
            return c.ret();
        }
    }
}

/// `loc_0301a`: the buttons from 1D1Eh (see [`set_buttons`]).
pub fn set_buttons_1d1e(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x301a)?;
    set_buttons(&mut Ctx::new(m), start, 2, |c| c.set_reg(SI, 0x1d1e))
}

/// `loc_03901`: the frame task `loc_03916` every 10h ticks.
pub fn add_task_3916(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3901)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_reg(SI, 0x3916);
    c.set_reg(BP, 0x10);
    super::tasks::add_frame_task(c.m, 0xda25)
}

/// `loc_04b2b`: the rect 1492h copied from the back buffer (unless
/// [4728h] is negative); then the cursor: hidden below row 88h..97h
/// (`call_restore_cursor` there), [DC46h] one down at the top (to 80h
/// when it stays shown).
pub fn cursor_over_panel(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4b2b)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0x4728);
    if f & 0x80 != 0 {
        c.run(start, 3)?;
        c.cmp(B, f as u16, 0);
    } else {
        c.run(start, 4)?;
        c.cmp(B, f as u16, 0);
        c.set_reg(SI, 0x1492);
        c.call(0xc46f, 0x4b38)?;
        c.run(0x4b38, 1)?;
    }
    let y = c.word(g::CURSOR_Y);
    let shown = c.byte(g::CURSOR_SHOWN);
    let after = shown.wrapping_sub(1);
    let n = if y >= 0x98 { 4 } else if y >= 0x88 { 5 } else if after & 0x80 != 0 { 8 } else { 9 };
    c.run(0xdbca, n)?;
    c.set_reg(AX, y);
    c.cmp(W, y, 0x98);
    if y >= 0x98 {
        return c.ret();
    }
    c.cmp(W, y, 0x88);
    if y >= 0x88 {
        return super::mouse::call_restore_cursor(c.m, 0xdbb2);
    }
    let v = c.dec(B, shown as u16) as u8;
    c.set_byte(g::CURSOR_SHOWN, v);
    if v & 0x80 == 0 {
        c.set_byte(g::CURSOR_SHOWN, 0x80);
    }
    c.ret()
}

/// `loc_0d280`: with [0DCE6h] above 0 (cleared), registers kept:
/// `loc_0d239`, then 11h steps of the transition `loc_0c0d5` (SI
/// [0DBDEh], AL 18h), each held 6 ticks of [0CE7Ah] by `loc_0a7c2`
/// calls; `loc_0d23d`.
pub fn fade_panel(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd280)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0xdce6);
    if (k as i8) <= 0 {
        c.run(start, 3)?;
        c.cmp(B, k as u16, 0);
        return c.ret();
    }
    c.run(start, 3)?;
    c.cmp(B, k as u16, 0);
    c.call(0xe270, 0xd28a)?;
    c.run(0xd28a, 2)?;
    c.set_byte(0xdce6, 0);
    c.call(0xd239, 0xd292)?;
    c.run(0xd292, 1)?;
    c.set_reg(CX, 0x11);
    loop {
        c.run(0xd295, 5)?;
        let cx = c.reg(CX);
        c.push(cx);
        let t = c.word(0xce7a);
        c.push(t);
        let si = c.word(0xdbde);
        c.set_reg(SI, si);
        c.set_reg8(r8::AL, 0x18);
        c.call(0xc0d5, 0xd2a3)?;
        c.run(0xd2a3, 1)?;
        let bx = c.pop();
        c.set_reg(BX, bx);
        loop {
            c.run(0xd2a4, 2)?;
            let bx = c.reg(BX);
            c.push(bx);
            c.call(0xa7c2, 0xd2a8)?;
            let t = c.word(0xce7a);
            let d = t.wrapping_sub(c.word_in(SS, c.reg(SP)));
            let more = d < 6;
            c.run(0xd2a8, 5)?;
            let bx = c.pop();
            c.set_reg(BX, bx);
            c.set_reg(AX, t);
            let d2 = c.sub(W, t, bx);
            c.set_reg(AX, d2);
            c.cmp(W, d2, 6);
            if !more {
                break;
            }
        }
        c.run(0xd2b3, 2)?;
        let cx = c.pop().wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.run(0xd2b6, 1)?;
    c.call(0xd23d, 0xd2b9)?;
    c.run(0xd2b9, 1)?;
    c.call(0xe283, 0xd2bc)?;
    c.run(0xd2bc, 1)?;
    c.ret()
}

/// `loc_0d23d`: the panel 1AF4h nudged one step toward a multiple of 3
/// (its word 0Ah; words 0Ah, 18h moved by 1 or -1), drawn
/// (`draw_ui_elements_list_at_ds_si` CX 2, `loc_01a34`), a pause of 0Ah,
/// moved again and drawn. Bails at entry where the division would fault.
pub fn nudge_panel(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd23d)?;
    let mut c = Ctx::new(m);
    let a = c.word(0x1afe);
    if a / 3 > 0xff {
        return Err(Bail);
    }
    let r = (a % 3) as u8;
    if r == 0 {
        c.run(start, 9)?;
    } else {
        c.run(start, 8 + 3 + 6)?;
    }
    let cx = c.reg(CX);
    c.xor(W, cx, cx);
    c.set_reg(SI, 0x1af4);
    let ax = c.sub(W, a, 0);
    c.set_reg(AX, ax);
    c.set_reg(CX, 3);
    c.m.cpu.mul_div(&mut c.m.hw, B, 6, 3);
    c.cmp(B, 0, r as u16);
    if r == 0 {
        return c.ret();
    }
    let step = 0xffffu16;
    c.set_reg(AX, 1);
    let step = { let _ = step; c.neg(W, 1) };
    c.set_reg(AX, step);
    c.push(step);
    c.push(0x1af4);
    for off in [0x0au16, 0x18] {
        let v = c.word(0x1af4 + off);
        let v = c.add(W, v, step);
        c.set_word(0x1af4 + off, v);
    }
    c.set_reg(CX, 2);
    c.call(0xd1f2, 0xd265)?;
    c.run(0xd265, 1)?;
    c.call(0x1a34, 0xd268)?;
    c.run(0xd268, 2)?;
    c.set_reg(AX, 0x0a);
    c.call(0xe387, 0xd26e)?;
    c.run(0xd26e, 6)?;
    let si = c.pop();
    c.set_reg(SI, si);
    let ax = c.pop();
    c.set_reg(AX, ax);
    for off in [0x0au16, 0x18] {
        let v = c.word(si.wrapping_add(off));
        let v = c.add(W, v, ax);
        c.set_word(si.wrapping_add(off), v);
    }
    c.set_reg(CX, 2);
    c.call(0xd1f2, 0xd27c)?;
    c.run(0xd27c, 1)?;
    c.call(0x1a34, 0xd27f)?;
    c.run(0xd27f, 1)?;
    c.ret()
}

/// `loc_0bdbb`: with [0DD0Fh] set, on the back buffer: `loc_0dbb2`,
/// `loc_0bdfa`, `loc_0bed7`, [115Ch] = the time [2] to 16, `loc_0d075`,
/// the text 2494h (`loc_0d1a6`); then each of the 6 bytes at 0DD17h equal
/// to its pair at 0DD11h counted down, and when the last pair matched
/// `loc_0dbec`.
pub fn stats_screen_marks(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbdbb)?;
    let mut c = Ctx::new(m);
    let k = c.word(0xdd0f);
    if k == 0 {
        c.run(start, 3)?;
        c.cmp(W, 0, 0);
        return c.ret();
    }
    c.run(start, 3)?;
    c.cmp(W, k, 0);
    for (at, to, back) in [(0xbdc2u16, 0xc08eu16, 0xbdc5u16), (0xbdc5, 0xdbb2, 0xbdc8), (0xbdc8, 0xbdfa, 0xbdcb), (0xbdcb, 0xbed7, 0xbdce)] {
        if at != 0xbdc2 {
            c.run(at, 1)?;
        }
        c.call(to, back)?;
    }
    c.run(0xbdce, 4)?;
    let t = c.word(2);
    let t = c.and(W, t, 0xfff0);
    c.set_reg(AX, t);
    c.set_word(0x115c, t);
    c.call(0xd075, 0xbdda)?;
    c.run(0xbdda, 2)?;
    c.set_reg(SI, 0x2494);
    c.call(0xd1a6, 0xbde0)?;
    let step = c.string_step(B);
    let (mut si, mut di, mut cx) = (0xdd11u16, 0xdd17u16, 6u16);
    let mut n = 5;
    let mut ops: Vec<(u16, u16, bool)> = Vec::new();
    let mut call = false;
    let mut dec_at: Vec<u16> = Vec::new();
    loop {
        let a = c.byte(si);
        let mut b = c.byte(di);
        if dec_at.contains(&di) {
            b = b.wrapping_sub(dec_at.iter().filter(|&&p| p == di).count() as u8);
        }
        si = si.wrapping_add(step);
        di = di.wrapping_add(step);
        cx -= 1;
        n += 2;
        ops.push((a as u16, b as u16, a == b));
        if a != b && cx != 0 {
            continue;
        }
        n += 1;
        if a != b {
            n += 1;
            break;
        }
        n += 3;
        dec_at.push(di.wrapping_sub(1));
        if cx != 0 {
            continue;
        }
        n += 2;
        call = true;
        break;
    }
    c.run(0xbde0, if call { n - 1 } else { n })?;
    c.set_reg(SI, 0xdd11);
    c.set_reg(DI, 0xdd17);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    let mut cx = 6u16;
    for &(a, b, eq) in &ops {
        c.cmp(B, a, b);
        let (s, d) = (c.reg(SI), c.reg(DI));
        c.set_reg(SI, s.wrapping_add(step));
        c.set_reg(DI, d.wrapping_add(step));
        cx -= 1;
        c.set_reg(CX, cx);
        if eq {
            let d = c.reg(DI).wrapping_sub(1);
            let v = c.byte(d);
            let v = c.dec(B, v as u16);
            c.set_byte(d, v as u8);
            let x = c.inc(W, cx);
            c.set_reg(CX, x.wrapping_sub(1));
        }
    }
    if call {
        c.call(0xdbec, 0xbdf9)?;
        c.run(0xbdf9, 1)?;
    }
    c.ret()
}

/// `loc_0d7b7`: every 64 ticks of [0CE7Ah] (a new [0DCF1h]): the gauges'
/// blink ([2222h], a counter per side) stepped: on odd steps a side's
/// gauge [1152h]/[1153h] shows FFh for the redraw (`draw_gauges` between
/// `loc_0c137`, `loc_0dbb2`, `loc_0dbec`, the resource [2784h] reopened)
/// unless [1AFEh]; the real values come back.
pub fn blink_gauges(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd7b7)?;
    let mut c = Ctx::new(m);
    let t = c.word(0xce7a);
    let ah = ((t << 2) >> 8) as u8;
    let last = c.byte(0xdcf1);
    if ah == last {
        c.run(start, 6)?;
        let a = c.shl1(W, t, 2);
        c.set_reg(AX, a);
        c.cmp(B, a >> 8, last as u16);
        return c.ret();
    }
    let ax = c.word(0x2222);
    if ax == 0 {
        c.run(start, 10)?;
        let a = c.shl1(W, t, 2);
        c.cmp(B, a >> 8, last as u16);
        c.set_byte(0xdcf1, ah);
        c.set_reg(AX, 0);
        c.or(W, 0, 0);
        return c.ret();
    }
    let (al, ah2) = (ax as u8, (ax >> 8) as u8);
    let g = c.word(0x1152);
    let a1 = al != 0;
    let odd1 = a1 && al.wrapping_sub(1) & 1 != 0;
    let a2 = ah2 != 0;
    let odd2 = a2 && ah2.wrapping_sub(1) & 1 != 0;
    let k = c.word(0x1afe);
    let n = 9 + 4 + if a1 { 3 + odd1 as u32 } else { 0 } + 2 + if a2 { 3 + odd2 as u32 } else { 0 } + 4 + 2;
    c.run(start, n)?;
    let a = c.shl1(W, t, 2);
    c.cmp(B, a >> 8, last as u16);
    c.set_byte(0xdcf1, ah);
    c.or(W, ax, ax);
    c.set_reg(BX, g);
    c.push(g);
    let mut l = al;
    let mut h = ah2;
    let mut bx = g;
    c.or(B, al as u16, al as u16);
    if a1 {
        l = c.dec(B, al as u16) as u8;
        c.test(B, l as u16, 1);
        if odd1 {
            bx = bx & 0xff00 | 0xff;
        }
    }
    c.or(B, ah2 as u16, ah2 as u16);
    if a2 {
        h = c.dec(B, ah2 as u16) as u8;
        c.test(B, h as u16, 1);
        if odd2 {
            bx = bx & 0x00ff | 0xff00;
        }
    }
    let ax2 = (h as u16) << 8 | l as u16;
    c.set_reg(AX, ax2);
    c.set_reg(BX, bx);
    c.set_word(0x2222, ax2);
    c.set_word(0x1152, bx);
    c.cmp(W, k, 0);
    if k == 0 {
        let r = c.word(0x2784);
        c.push(r);
        c.call(0xc137, 0xd803)?;
        c.run(0xd803, 1)?;
        c.call(0xdbb2, 0xd806)?;
        c.run(0xd806, 1)?;
        c.call(0xd763, 0xd809)?;
        c.run(0xd809, 1)?;
        c.call(0xdbec, 0xd80c)?;
        c.run(0xd80c, 2)?;
        let a = c.pop();
        c.set_reg(AX, a);
        c.call(0xc13e, 0xd810)?;
        c.run(0xd810, 2)?;
    }
    let v = c.pop();
    c.set_word(0x1152, v);
    c.ret()
}

/// `loc_0bc99`: a box opening from (DX, BX): the rect 0DD06h (colour 7)
/// grown 8 times (by 4 then 2 at the start, 4 then 2 at the end, kept
/// within the bounds at 2440h), each step outlined (`draw_rect_outline_at_si`)
/// and `loc_0bd00`; on the back buffer.
pub fn open_box(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbc99)?;
    let mut c = Ctx::new(m);
    c.run(start, 10)?;
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.set_reg(SI, 0xdd06);
    c.set_word(0xdd06, dx);
    c.set_word(0xdd08, bx);
    let d = c.inc(W, dx);
    let d = c.inc(W, d);
    c.set_reg(DX, d);
    let b = c.inc(W, bx);
    c.set_reg(BX, b);
    c.set_word(0xdd0a, d);
    c.set_word(0xdd0c, b);
    c.set_byte(0xdd0e, 7);
    c.call(0xc08e, 0xbcb1)?;
    c.run(0xbcb1, 1)?;
    c.set_reg(CX, 8);
    loop {
        let v = [c.word(0xdd06), c.word(0xdd08), c.word(0xdd0a), c.word(0xdd0c)];
        let b = [c.word(0x2440), c.word(0x2442), c.word(0x2444), c.word(0x2446)];
        let steps = [0xfffcu16, 0xfffe, 4, 2];
        let mut n = 5 + 2 + 2;
        for k in 0..4 {
            let a = v[k].wrapping_add(steps[k]);
            let clamp = if k < 2 { a < b[k] } else { a >= b[k] };
            n += 9 + clamp as u32;
        }
        c.run(0xbcb4, n)?;
        let cx = c.reg(CX);
        c.push(cx);
        c.set_reg(SI, 0xdd06);
        c.set_reg(DI, 0x2440);
        let mut ax = 0;
        let mut dx = 0;
        for k in 0..4u16 {
            if k == 0 || k == 2 {
                c.set_reg(CX, 2);
                dx = if k == 0 { 0xfffc } else { 4 };
            }
            let p = 0xdd06 + 2 * k;
            let q = 0x2440 + 2 * k;
            let a = c.add(W, v[k as usize], dx);
            c.cmp(W, a, b[k as usize]);
            let clamp = if k < 2 { a < b[k as usize] } else { a >= b[k as usize] };
            ax = if clamp { b[k as usize] } else { a };
            c.set_word(p, ax);
            c.add(W, q, 2);
            c.add(W, p, 2);
            dx = c.m.cpu.shift(7, W, dx as u32, 1) as u16;
            c.set_reg(CX, 1 - (k & 1));
        }
        c.set_reg(AX, ax);
        c.set_reg(DX, dx);
        c.set_reg(DI, 0x2448);
        c.set_reg(SI, 0xdd06);
        c.call(0xc551, 0xbcf9)?;
        c.run(0xbcf9, 1)?;
        c.call(0xbd00, 0xbcfc)?;
        let cx = c.word_in(SS, c.reg(SP)).wrapping_sub(1);
        c.run(0xbcfc, 2 + (cx == 0) as u32)?;
        c.pop();
        c.set_reg(CX, cx);
        if cx == 0 {
            return c.ret();
        }
    }
}

/// `sub_44ab`: one blink of the map pointer: [4751h] counted; on the back
/// buffer (the front pointer kept) `loc_05b93`, `loc_0c137`, the rect
/// 4749h (`loc_0db74`) clipped to D834h and shown
/// (`draw_head_if_needed_and_update_screen_rect`), on odd counts the
/// sprite 4Ch at its corner (`draw_sprite_clipped_clobbering_bx_dx`);
/// then [`draw_mouse_cursor_if_needed`].
pub fn blink_map_pointer(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x44ab)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let v = c.byte(0x4751);
    let v = c.inc(B, v as u16);
    c.set_byte(0x4751, v as u8);
    let fb = c.word(0xdbda);
    c.push(fb);
    c.call(0xc08e, 0x44b6)?;
    c.run(0x44b6, 1)?;
    c.call(0x5b93, 0x44b9)?;
    c.run(0x44b9, 1)?;
    c.call(0xc137, 0x44bc)?;
    c.run(0x44bc, 2)?;
    c.set_reg(SI, 0x4749);
    c.call(0xdb74, 0x44c2)?;
    let si = c.reg(SI);
    let r = [c.word(si), c.word(si.wrapping_add(2)), c.word(si.wrapping_add(4)), c.word(si.wrapping_add(6))];
    let b = [c.word(0xd834), c.word(0xd836), c.word(0xd838), c.word(0xd83a)];
    let (c2, c3, c0, c1) = (r[2] >= b[2], r[3] >= b[3], r[0] < b[0], r[1] < b[1]);
    c.run(0x44c2, 7 + c2 as u32 + 2 + c3 as u32 + 2 + c0 as u32 + 2 + c1 as u32 + 1)?;
    c.set_reg(SI, 0xd834);
    c.cmp(W, r[2], b[2]);
    let bp = if c2 { b[2] } else { r[2] };
    c.cmp(W, r[3], b[3]);
    let ax = if c3 { b[3] } else { r[3] };
    c.cmp(W, r[0], b[0]);
    let dx = if c0 { b[0] } else { r[0] };
    c.cmp(W, r[1], b[1]);
    let bx = if c1 { b[1] } else { r[1] };
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.set_reg(BP, bp);
    c.set_reg(AX, ax);
    c.call(0xc4fb, 0x44f1)?;
    let k = c.byte(0x4751);
    let odd = k & 1 != 0;
    c.run(0x44f1, 3 + if odd { 4 } else { 0 })?;
    let bx = c.reg(BX);
    c.set_reg(BX, bx & 0xff00 | k as u16);
    let bl = c.shr1(B, k as u16, 1);
    c.set_reg(BX, bx & 0xff00 | bl);
    if odd {
        c.set_reg(AX, 0x4c);
        let (x, y) = (c.word(0x4749), c.word(0x474b));
        c.set_reg(DX, x);
        c.set_reg(BX, y);
        c.call(0xc30d, 0x4507)?;
    }
    c.run(0x4507, 2)?;
    let fb = c.pop();
    c.set_word(0xdbda, fb);
    super::mouse::draw_mouse_cursor_if_needed(c.m, 0xdb67)
}
