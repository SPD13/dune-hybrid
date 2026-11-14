//! The stack of UI screens: [`TOP`] points at the current entry (its list
//! of UI elements, its handler), entries 4 bytes from [`BOTTOM`]. A list
//! starts with a byte: its kind in the low 4 bits (0Fh: none to leave),
//! FFh at the end of the stack.

use super::pure;
use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CS, CX, DI, DS, DX, ES, SI, SP, SS},
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
    c.set_reg8(r8::AL, kind);
    c.cmp(B, kind as u16, 0x0f); // the handler (an indirect call) could see these
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
        if b != 0xff {
            let al = pure::and(B, b as u16, 0x0f);
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
    if v == 0 {
        let t = c.byte(TRANSITION);
        let t = pure::or(B, t as u16, 1);
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
    if !text {
        let n = c.lods(B, pc::cpu::DS) as u8;
        let skip = n as i8 as u16;
        c.set_reg(AX, skip);
        let si = c.reg(SI);
        let si = pure::add(W, si, skip);
        let off = pure::shl1(W, cl as u16, 2);
        c.set_reg(AX, off);
        let si = pure::add(W, si, off);
        c.set_reg(SI, si);
        let (x, y) = (c.word(si), c.word(si.wrapping_add(2)));
        c.set_reg(AX, x);
        c.set_reg(BX, y);
        return c.ret();
    }
    c.set_reg(AX, 0xa0);
    c.set_reg(BX, 0xd423);
    if k >= 0 {
        c.set_reg(BX, 0xd429);
        if k == 0 {
            let z = pure::xor(W, 0xd429, 0xd429);
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
    c.set_reg(BX, 0);
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
    if !on {
        return c.ret();
    }
    c.set_word(0xdc60, di);
    let v = c.word(di.wrapping_add(0x0a));
    let v = pure::inc(W, v);
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
    let v = pure::dec(W, v);
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
    c.set_reg(AX, d);
    // The driver's far call sees these flags.
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
pub(super) fn set_buttons(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
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
    let a = pure::add(W, a, 0x41);
    c.set_word(si + 0x0a, a);
    let b = c.byte(0x1153) as i8 as u16;
    let b = pure::add(W, b, 0x41);
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
    // The elements in turn: one whose rect (left, top, right, bottom;
    // bit 7 of +8 on) holds the cursor.
    let (mut k, mut d) = (cx, di);
    let mut found = false;
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
            found = true;
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
    if found {
        c.set_reg(DI, d);
        c.set_reg(CX, k);
        c.set_flag(CF, true);
    } else {
        c.set_reg(DI, d.wrapping_add(0x0e));
        c.set_reg(CX, 0);
        c.set_flag(CF, false);
    }
    c.ret()
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
    } else {
        c.run(start, 4)?;
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
    if y >= 0x98 {
        return c.ret();
    }
    if y >= 0x88 {
        return super::mouse::call_restore_cursor(c.m, 0xdbb2);
    }
    let v = pure::dec(B, shown as u16) as u8;
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
        return c.ret();
    }
    c.run(start, 3)?;
    for (at, to, back) in [(0xbdc2u16, 0xc08eu16, 0xbdc5u16), (0xbdc5, 0xdbb2, 0xbdc8), (0xbdc8, 0xbdfa, 0xbdcb), (0xbdcb, 0xbed7, 0xbdce)] {
        if at != 0xbdc2 {
            c.run(at, 1)?;
        }
        c.call(to, back)?;
    }
    c.run(0xbdce, 4)?;
    let t = c.word(2);
    let t = pure::and(W, t, 0xfff0);
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
    for &(_, _, eq) in &ops {
        let (s, d) = (c.reg(SI), c.reg(DI));
        c.set_reg(SI, s.wrapping_add(step));
        c.set_reg(DI, d.wrapping_add(step));
        cx -= 1;
        c.set_reg(CX, cx);
        if eq {
            let d = c.reg(DI).wrapping_sub(1);
            let v = c.byte(d);
            let v = pure::dec(B, v as u16);
            c.set_byte(d, v as u8);
            let x = pure::inc(W, cx);
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
        let a = pure::shl1(W, t, 2);
        c.set_reg(AX, a);
        return c.ret();
    }
    let ax = c.word(0x2222);
    if ax == 0 {
        c.run(start, 10)?;
        let _ = pure::shl1(W, t, 2);
        c.set_byte(0xdcf1, ah);
        c.set_reg(AX, 0);
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
    let _ = pure::shl1(W, t, 2);
    c.set_byte(0xdcf1, ah);
    c.set_reg(BX, g);
    c.push(g);
    let mut l = al;
    let mut h = ah2;
    let mut bx = g;
    if a1 {
        l = pure::dec(B, al as u16) as u8;
        if odd1 {
            bx = bx & 0xff00 | 0xff;
        }
    }
    if a2 {
        h = pure::dec(B, ah2 as u16) as u8;
        if odd2 {
            bx = bx & 0x00ff | 0xff00;
        }
    }
    let ax2 = (h as u16) << 8 | l as u16;
    c.set_reg(AX, ax2);
    c.set_reg(BX, bx);
    c.set_word(0x2222, ax2);
    c.set_word(0x1152, bx);
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
    let d = pure::inc(W, dx);
    let d = pure::inc(W, d);
    c.set_reg(DX, d);
    let b = pure::inc(W, bx);
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
            let _ = 0x2440 + 2 * k;
            let a = pure::add(W, v[k as usize], dx);
            let clamp = if k < 2 { a < b[k as usize] } else { a >= b[k as usize] };
            ax = if clamp { b[k as usize] } else { a };
            c.set_word(p, ax);
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
    let v = pure::inc(B, v as u16);
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
    let bp = if c2 { b[2] } else { r[2] };
    let ax = if c3 { b[3] } else { r[3] };
    let dx = if c0 { b[0] } else { r[0] };
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
    let bl = pure::shr1(B, k as u16, 1);
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

/// From `loc_0d735` (after `n` instructions from `at` with the effect of
/// `ops`): `loc_0d741` and the 6 buttons drawn.
fn buttons_from_d735(c: &mut Ctx, at: u16, n: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, n + 1)?;
    ops(c);
    c.call(0xd741, 0xd738)?;
    c.run(0xd738, 3)?;
    c.set_reg(SI, BUTTONS);
    c.set_reg(CX, 6);
    super::gfx::draw_ui_elements_list_at_ds_si(c.m, 0xd1f2)
}

/// `loc_02ffb`: the panel's buttons for the situation: in a dialogue or
/// a map mode the sets 1D1Eh / 1D72h ([`set_buttons_si`]); at a place
/// ([6] 80h, not 21h) its entry's (`list_entry`) 4 buttons, active (80h)
/// for codes from -5 up, the first ones by the current location and
/// [4]; elsewhere the plain set (1Dh.. and 23h).
pub fn panel_buttons(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2ffb)?;
    let mut c = Ctx::new(m);
    panel_buttons_at(&mut c, start, 0, |_| {})
}

/// `loc_02ffb` after `pre` instructions from `at` (with `ops`).
pub fn panel_buttons_at(c: &mut Ctx, at: u16, pre: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    let dlg = c.byte(0x2b);
    let mode = c.byte(0x11c9);
    let (ca, cb) = (c.byte(0x11ca), c.byte(0x11cb));
    if dlg != 0 || mode & 3 != 0 {
        let (n, si) = if dlg != 0 {
            (4, 0x1d1e)
        } else if ca != 0 {
            (8, 0x1d1e)
        } else if cb != 0 {
            (10, 0x1d72)
        } else {
            (11, 0x1d1e)
        };
        return set_buttons(c, at, pre + n, |c| {
            ops(c);
            c.cmp(B, dlg as u16, 0);
            if dlg == 0 {
                c.test(B, mode as u16, 3);
                c.cmp(B, ca as u16, 0);
                if ca == 0 {
                    c.cmp(B, cb as u16, 0);
                }
            }
            c.set_reg(SI, si);
        });
    }
    let bx = c.word(6);
    let dx = c.word(4);
    let place = bx & 0xff == 0x80 && dx >> 8 != 0x21;
    if !place {
        let n = 4 + 3 + if bx & 0xff == 0x80 { 3 } else { 0 } + 4 + 4 * 5 + 1;
        return buttons_from_d735(c, at, pre + n, |c| {
            ops(c);
            c.cmp(B, 0, 0);
            c.test(B, mode as u16, 3);
            c.set_reg(BX, bx);
            c.cmp(B, bx & 0xff, 0x80);
            if bx & 0xff == 0x80 {
                c.set_reg(DX, dx);
                c.cmp(B, dx >> 8, 0x21);
            }
            c.set_word(0x1b98, 0x23);
            let mut b = 0x1du16;
            let mut di = 0x1b98u16;
            for i in 0..4u16 {
                di = c.add(W, di, 0x0e);
                c.set_word(di, b);
                c.set_word(di.wrapping_sub(2), 0x80);
                b = c.inc(W, b);
                c.set_reg(CX, 3 - i);
            }
            c.set_reg(DI, di);
            c.set_reg(BX, b);
        });
    }
    c.run(at, pre + 4 + 7)?;
    ops(c);
    c.cmp(B, 0, 0);
    c.test(B, mode as u16, 3);
    c.set_reg(BX, bx);
    c.cmp(B, 0x80, 0x80);
    c.set_reg(DX, dx);
    c.cmp(B, dx >> 8, 0x21);
    c.call(0x3efe, 0x3035)?;
    let si = c.reg(SI).wrapping_add(1);
    let here = c.word(0x114e);
    let dl = c.reg8(r8::DL);
    let step = c.string_step(B);
    let mut n = 5 + (here == 0x100) as u32 + 3 + if dl == 1 { 2 } else { 0 } + 4;
    let mut p = si;
    for _ in 0..4 {
        let a = c.byte(p);
        p = p.wrapping_add(step);
        n += 5;
        if a != 0 {
            n += 2 + ((a as i8) >= -5) as u32;
        }
        n += 2;
    }
    c.run(0x3035, n + 2)?;
    let s = c.inc(W, c.reg(SI));
    c.set_reg(SI, s);
    c.set_reg(DI, 0x1b96);
    c.cmp(W, here, 0x100);
    let mut al = if here == 0x100 { 0x80u8 } else { 0x20 };
    let mut b = 0x21u16;
    c.cmp(B, dl as u16, 1);
    if dl == 1 {
        b = c.inc(W, b);
        al = 0x20;
    }
    c.set_reg(BX, b);
    c.set_word(0x1b98, b);
    c.set_byte(0x1bdc, al);
    c.set_byte(0x1cc4, al);
    let mut di = 0x1b96u16;
    for i in 0..4u16 {
        let a = c.lods(B, pc::cpu::DS);
        di = c.add(W, di, 0x0e);
        let mut ah = 0x20u8;
        c.or(B, a, a);
        if a != 0 {
            c.cmp(B, a, 0xfb);
            if (a as u8 as i8) >= -5 {
                ah = 0x80;
            }
        }
        c.set_reg(AX, (ah as u16) << 8 | a);
        c.set_byte(di, ah);
        c.set_reg(CX, 3 - i);
    }
    c.set_reg(DI, di);
    c.call(0xd741, 0xd738)?;
    c.run(0xd738, 3)?;
    c.set_reg(SI, BUTTONS);
    c.set_reg(CX, 6);
    super::gfx::draw_ui_elements_list_at_ds_si(c.m, 0xd1f2)
}

/// `loc_0445d`: the map pointer re-placed: its blink task removed; at the
/// map position (`get_map_position`) inside the view (not
/// `outside_view_after_b647`) the pointer sprite 4Ch's box at 4749h (x
/// less 0Dh, y less its height; right and bottom by its size), the blink
/// task back (every 12Ch) and the first blink ([`blink_map_pointer`]
/// with [4751h] 0); outside it [4749h] 0.
pub fn place_map_pointer(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x445d)?;
    let mut c = Ctx::new(m);
    place_map_pointer_at(&mut c)
}

fn place_map_pointer_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x445d, 2)?;
    c.set_reg(SI, 0x44ab);
    c.call(0xda5f, 0x4463)?;
    c.run(0x4463, 1)?;
    c.call(0x407e, 0x4466)?;
    c.run(0x4466, 1)?;
    c.call(0x62d6, 0x4469)?;
    if c.flag(CF) {
        c.run(0x4469, 3)?;
        c.set_word(0x4749, 0);
        return c.ret();
    }
    c.run(0x4469, 2)?;
    c.call(0xc137, 0x4475)?;
    c.run(0x4475, 2)?;
    c.set_reg(AX, 0x4c);
    c.call(0xc1f4, 0x447b)?;
    c.run(0x447b, 17)?;
    let a = c.lods(W, ES);
    let a = a & 0x0fff;
    c.set_reg(AX, a);
    c.and(B, a >> 8, 0x0f);
    let mut bp = a;
    let h = c.lods(W, ES) & 0xff;
    c.xor(B, 0, 0);
    c.set_reg(AX, h);
    let bx = c.reg(BX);
    let bx = c.sub(W, bx, h);
    let dx = c.reg(DX);
    let dx = c.sub(W, dx, 0x0d);
    bp = c.add(W, bp, dx);
    let ax = c.add(W, h, bx);
    c.set_reg(AX, ax);
    c.set_reg(BX, bx);
    c.set_reg(DX, dx);
    c.set_reg(DI, 0x4749);
    c.set_word(0x4749, dx);
    c.set_word(0x474b, bx);
    c.set_word(0x474d, bp);
    c.set_word(0x474f, ax);
    c.set_reg(SI, 0x44ab);
    c.set_reg(BP, 0x12c);
    c.call(0xda25, 0x44a6)?;
    c.run(0x44a6, 1)?;
    c.set_byte(0x4751, 0);
    blink_map_pointer(c.m, 0x44ab)
}

/// `sub_4377`: the map view's frame redrawn (`loc_05b93`, the zoomed
/// globe, icons, the shown locations; with [473Eh] the icons 14C0h of
/// resource 24h), the screen updated, and the map pointer re-placed
/// ([`place_map_pointer`]).
pub fn redraw_map_frame(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4377)?;
    let mut c = Ctx::new(m);
    for (at, to) in [(start, 0xc07cu16), (0x437a, 0x5b93), (0x437d, 0xb6c3), (0x4380, 0xc137), (0x4383, 0x5dce)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    let f = c.byte(0x473e);
    c.run(0x4386, if f != 0 { 4 } else { 3 })?;
    if f != 0 {
        c.set_reg(AX, 0x24);
        c.call(0xc13e, 0x4393)?;
        c.run(0x4393, 2)?;
        c.set_reg(SI, 0x14c0);
        c.call(0xc21b, 0x4399)?;
        c.run(0x4399, 1)?;
    }
    c.call(0xc4ed, 0x439c)?;
    c.run(0x439c, 1)?;
    place_map_pointer_at(&mut c)
}

/// `loc_0d2e2`: a redraw asked ([`ask_redraw`]), the screen left
/// ([`leave_screen`]), and the panel faded ([`fade_panel`]).
pub fn leave_and_fade(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd2e2)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xd316, 0xd2e5)?;
    c.run(0xd2e5, 1)?;
    c.call(0xd2ea, 0xd2e8)?;
    c.run(0xd2e8, 1)?;
    fade_panel(c.m, 0xd280)
}

/// `loc_0d32f`: a redraw asked, the UI handlers above the level BP
/// unwound (`loc_0d33a`), the panel faded.
pub fn unwind_and_fade(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd32f)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xd316, 0xd332)?;
    c.run(0xd332, 1)?;
    c.call(0xd33a, 0xd335)?;
    c.run(0xd335, 1)?;
    fade_panel(c.m, 0xd280)
}

/// `loc_02ebf`: the UI list [2220h] installed with the handler F66h
/// (`install_ui_list_d338`).
pub fn install_list_2220(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2ebf)?;
    let mut c = Ctx::new(m);
    list_2220_at(&mut c)
}

fn list_2220_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x2ebf, 3)?;
    let bp = c.word(0x2220);
    c.set_reg(BP, bp);
    c.set_reg(BX, 0x0f66);
    super::cursor::install_ui_list_d338(c.m, 0xd338)
}

/// `loc_02eb2`: the place's panel back: with [4774h] `loc_0301a`, the
/// dialogue reset and the list [2220h]; else the place's actions
/// (`loc_02e98` for [114Eh], `loc_02efb`, `setup_3090` outside the map
/// modes), the buttons, `loc_0d763` and the action list 1F0Eh installed.
/// Bails when the mouse is low (y from 74h: the original then returns
/// through `draw_mouse`).
pub fn place_panel_back(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2eb2)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x4774);
    if k != 0 {
        c.run(start, 3)?;
        c.cmp(B, k as u16, 0);
        c.call(0x301a, 0x2ebc)?;
        c.run(0x2ebc, 1)?;
        c.call(0x98e6, 0x2ebf)?;
        return list_2220_at(&mut c);
    }
    c.run(start, 4)?;
    c.cmp(B, 0, 0);
    let di = c.word(0x114e);
    c.set_reg(DI, di);
    c.call(0x2e98, 0x2ed0)?;
    c.run(0x2ed0, 1)?;
    c.call(0x2efb, 0x2ed3)?;
    let mode = c.byte(0x11c9);
    c.run(0x2ed3, if mode == 0 { 3 } else { 2 })?;
    c.cmp(B, mode as u16, 0);
    if mode == 0 {
        c.call(0x3090, 0x2edd)?;
    }
    let y = c.word(0xdc38);
    if y >= 0x74 {
        c.m.cpu.ip = 0x2edd;
        return Err(Bail);
    }
    c.run(0x2edd, 4)?;
    c.set_reg(AX, y);
    c.cmp(W, y, 0x74);
    c.call(0x2ffb, 0x2eef)?;
    c.run(0x2eef, 1)?;
    c.call(0xd763, 0x2ef2)?;
    c.run(0x2ef2, 3)?;
    c.set_reg(BP, 0x1f0e);
    c.set_reg(BX, 0x0f66);
    super::cursor::install_ui_list_d338(c.m, 0xd338)
}

/// `loc_02efb`: the action list 1F0Fh rebuilt (a 0 byte, then 4-byte
/// entries copied from the templates around 21DCh..2218h, an entry's
/// high byte 40h when disabled, a 0 word last), then the panel buttons
/// (the buttons, `loc_02ffb`, follow). At a place ([6] 80h): its first action 220Ch;
/// in a palace (place 1) the talk entries 2218h.. and 2214h (off from
/// phase 4Fh) in a dialogue, else 21DCh (off without [46FFh], after
/// `skill_needs_46fe`); at a sietch (BH 1) with troops [0C8h] in the
/// eighth place the troop entries (21E8h: [1248h] the phrase 27h / 26h /
/// 28h with [47A9h], off when [0C9h] is 0; the next off when [0C9h]
/// reaches [0C8h]), in the ninth 21F0h; then 21F4h. Elsewhere outside the
/// map modes 220Ch, 2214h and by the time of day 21E0h / 21E4h; in a map
/// mode 21FCh (or with [11CBh] 2200h, and from phase 32h also 2204h),
/// then 21F8h. Bails with DF set.
pub fn build_action_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2efb)?;
    let mut c = Ctx::new(m);
    if c.flag(pc::cpu::DF) {
        return Err(Bail);
    }
    let (bx, dx) = (c.word(6), c.word(4));
    let place = bx as u8 == 0x80;
    c.run(start, if place { 9 } else { 10 })?;
    let ds = c.sreg(DS);
    c.push(ds);
    let v = c.pop();
    c.set_sreg(ES, v);
    c.set_reg(DI, 0x1f0f);
    c.set_reg8(r8::AL, 0);
    c.stos(B);
    c.set_reg(BX, bx);
    c.set_reg(DX, dx);
    // An entry read (`lodsw`) and stored with its high byte 40h when
    // disabled (`cmp ..; sbb ah, ah; and ah, 40h; stosw; movsw`).
    let flagged = |c: &mut Ctx, v: u16, off: bool| {
        c.set_reg(AX, if off { 0x4000 } else { 0 } | v & 0xff);
        c.stos(W);
        c.movs(W);
    };
    let copy = |c: &mut Ctx, si: u16, n: u32| {
        c.set_reg(SI, si);
        for _ in 0..n {
            c.movs(W);
        }
    };
    if place {
        c.run(0x2f13, 5)?;
        copy(&mut c, 0x220c, 2);
        if dx as u8 == 1 {
            let t = c.byte(0x2b);
            if t != 0 {
                // In a dialogue: the talk entries, the last one off from
                // phase 4Fh.
                c.run(0x2f1d, 15)?;
                copy(&mut c, 0x2218, 4);
                c.set_reg(SI, 0x2214);
                let phase = c.byte(0x2a);
                let v = c.lods(W, DS);
                flagged(&mut c, v, phase < 0x4f);
            } else {
                c.run(0x2f1d, 5)?;
                let di = c.reg(DI);
                c.push(di);
                let p = c.word(0x114e);
                c.set_reg(DI, p);
                c.call(0x7f27, 0x2f45)?;
                c.run(0x2f45, 9)?;
                let d = c.pop();
                c.set_reg(DI, d);
                c.set_reg(SI, 0x21dc);
                let v = c.lods(W, DS);
                let skills = c.byte(0x46ff);
                flagged(&mut c, v, skills < 1);
            }
        } else {
            let bh = (bx >> 8) as u8;
            if bh != 1 {
                c.run(0x2f58, 2)?;
            } else if dx as u8 != 8 {
                c.run(0x2f58, 4)?;
                let nine = dx as u8 == 9;
                c.run(0x2f99, if nine { 5 } else { 2 })?;
                if nine {
                    copy(&mut c, 0x21f0, 2);
                }
            } else {
                let c8 = c.byte(0xc8);
                if c8 == 0 {
                    c.run(0x2f58, 6)?;
                } else {
                    // A sietch's troops: the first entry off without any
                    // there ([0C9h]; its phrase 26h then, 28h in a call),
                    // the second once they're all there.
                    let here = c.byte(0xc9);
                    let calling = c.byte(0x47a9) != 0;
                    c.run(0x2f58, 15 + calling as u32 + 11 + 2)?;
                    c.set_reg(SI, 0x21e8);
                    let v = c.lods(W, DS);
                    c.set_reg8(r8::CL, here);
                    let none = here < 1;
                    let phrase = if calling { 0x28 } else if none { 0x26 } else { 0x27 };
                    c.set_reg8(r8::CH, phrase);
                    c.set_byte(0x1248, phrase);
                    flagged(&mut c, v, none);
                    let v = c.lods(W, DS);
                    flagged(&mut c, v, here >= c8);
                }
            }
        }
        c.run(0x2fa3, 4)?;
        copy(&mut c, 0x21f4, 2);
        return end_2ff7(&mut c);
    }
    let mode = c.byte(0x11c9);
    if mode & 3 == 0 {
        c.run(0x2faa, 13)?;
        copy(&mut c, 0x220c, 2);
        c.set_reg(SI, 0x2214);
        let v = c.lods(W, DS);
        let phase = c.byte(0x2a);
        flagged(&mut c, v, phase < 0x4f);
        c.call(0x1ae0, 0x2fc9)?;
        // By the time of day (`loc_01ae0`): 21E0h or, from 0Bh, 21E4h.
        let hour = c.reg8(r8::AL);
        c.run(0x2fc9, 3 + if hour >= 0x0b { 1 } else { 0 } + 3)?;
        copy(&mut c, if hour >= 0x0b { 0x21e4 } else { 0x21e0 }, 2);
        c.run(0x2fa3, 4)?;
        copy(&mut c, 0x21f4, 2);
        return end_2ff7(&mut c);
    }
    let cb = c.byte(0x11cb);
    let p = c.byte(0x2a);
    let n = if cb == 0 { 3 } else if p < 0x32 { 6 } else { 9 };
    c.run(0x2faa, 2 + n + 5)?;
    c.set_reg(SI, 0x21fc);
    if cb != 0 {
        c.set_reg(SI, 0x2200);
        if p >= 0x32 {
            c.movs(W);
            c.movs(W);
            c.set_reg(SI, 0x2204);
        }
    }
    c.movs(W);
    c.movs(W);
    copy(&mut c, 0x21f8, 2);
    end_2ff7(&mut c)
}

/// From 2FF7h: the list's end word.
fn end_2ff7(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x2ff7, 3)?;
    c.set_reg(AX, 0);
    c.stos(W);
    c.ret()
}

/// `sub_d443` and its entries D42Fh, D434h, D439h, D43Eh: the panel's
/// button CL (0, or 4 down to 1 by entry) pressed: its place and handler
/// (`loc_0d454`); a handler (BX) runs unless the button is disabled (AH
/// bit 6).
pub fn press_panel_button(m: &mut Machine, start: u16) -> Result<(), Bail> {
    let n = match start {
        0xd42f => 4,
        0xd434 => 3,
        0xd439 => 2,
        0xd43e => 1,
        0xd443 => 0,
        _ => return Err(Bail),
    };
    let mut c = Ctx::new(m);
    if start == 0xd443 {
        c.run(0xd443, 2)?;
        let cx = c.reg(CX);
        c.xor(W, cx, cx);
    } else {
        c.run(start, 3)?;
    }
    c.set_reg(CX, n);
    c.call(0xd454, 0xd448)?;
    let (ax, bx) = (c.reg(AX), c.reg(BX));
    if bx == 0 {
        c.run(0xd448, 3)?;
        c.or(W, bx, bx);
        return c.ret();
    }
    c.run(0xd448, 5)?;
    c.or(W, bx, bx);
    c.test(B, ax >> 8, 0x40);
    if ax & 0x4000 != 0 {
        return c.ret();
    }
    c.jump(bx)
}
