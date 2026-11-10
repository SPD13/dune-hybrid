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

/// `read_game_port` (DCE0): the joystick's two axes timed on the game
/// port (201h, interrupts off): CX the count while X's bit stays set
/// (returned in DX), BX the count for Y, up to 800h loops (then the
/// joystick is marked absent, [2942h] bit 7 cleared). AL the buttons
/// (the port read once more, inverted).
pub fn read_game_port(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{DI, IF};
    entered(start, 0xdce0)?;
    let mut c = Ctx::new(m);
    c.check(0xdce0, 7)?;
    c.set_reg(DX, 0x201);
    let f = c.m.cpu.flags();
    c.push(f);
    c.set_flag(IF, false);
    c.count(3);
    let al = c.reg8(r8::AL);
    c.out(0x201, al);
    c.count(4);
    let bx = c.reg(BX);
    c.xor(W, bx, bx);
    c.set_reg(BX, 0);
    c.set_reg(CX, 0);
    c.set_reg(DI, 0x800);
    let (mut bx, mut cx, mut di) = (0u16, 0u16, 0x800u16);
    loop {
        c.check(0xdced, 11)?;
        let v = c.inp(0x201);
        c.count(1);
        c.set_reg8(r8::AL, v);
        let ah = c.shr1(B, v as u16, 1);
        let ax = c.and(W, (ah << 8) | v as u16, 0x101);
        c.set_reg(AX, ax);
        if ax == 0 {
            c.count(4);
            break;
        }
        let cl = c.add(B, cx & 0xff, ax & 0xff);
        let ch = c.adc(B, cx >> 8, 0);
        cx = ch << 8 | cl;
        let bl = c.add(B, bx & 0xff, ax >> 8);
        let bh = c.adc(B, bx >> 8, 0);
        bx = bh << 8 | bl;
        di = c.dec(W, di);
        c.set_reg(CX, cx);
        c.set_reg(BX, bx);
        c.set_reg(DI, di);
        c.count(10);
        if di == 0 {
            c.check(0xdd04, 6)?;
            let f = c.byte(0x2942);
            let f = c.and(B, f as u16, 0x7f);
            c.set_byte(0x2942, f as u8);
            c.count(1);
            break;
        }
    }
    let v = c.inp(0x201);
    c.count(1);
    let f = c.pop();
    c.m.cpu.set_flags(f);
    c.set_reg(DX, cx);
    c.set_reg8(r8::AL, !v);
    c.count(4);
    c.ret()
}

/// `initialize_joystick` (EA32): without calibration (the four words at
/// 39ABh all 0), the centre read (`read_game_port`) and, if both axes
/// are within 4..403h, the thresholds set: 39ABh x/2, 39ADh x*3/2, 39AFh
/// y/2, 39B1h y*3/2.
pub fn initialize_joystick(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{DS, SI};
    entered(start, 0xea32)?;
    let mut c = Ctx::new(m);
    c.run(0xea32, 3 + 12 + 2 + 1)?;
    c.set_reg(SI, 0x39ab);
    let mut acc = c.xor(W, 0, 0);
    for _ in 0..4 {
        let w = c.lods(W, DS);
        acc = c.or(W, acc, w);
    }
    c.set_reg(CX, 0);
    c.set_reg(BX, acc);
    c.or(W, acc, acc);
    if acc != 0 {
        return c.ret();
    }
    c.call(0xdce0, 0xea46)?;
    for (at, r) in [(0xea46u16, DX), (0xea50, BX)] {
        let v = c.reg(r);
        let a = v.wrapping_sub(4);
        let bad = (a >> 8) >= 4;
        c.run(at, 4 + bad as u32)?;
        c.set_reg(AX, v);
        let a = c.sub(W, v, 4);
        c.set_reg(AX, a);
        c.cmp(B, a >> 8, 4);
        if bad {
            return c.ret();
        }
    }
    c.run(0xea5a, 12)?;
    let dx = c.reg(DX);
    let ax = c.shr1(W, dx, 1);
    let dx = c.add(W, dx, ax);
    c.set_reg(DX, dx);
    c.set_reg(SI, 0x39ab);
    c.set_word(0x39ab, ax);
    c.set_word(0x39ad, dx);
    let bx = c.reg(BX);
    let ax = c.shr1(W, bx, 1);
    let bx = c.add(W, bx, ax);
    c.set_reg(AX, ax);
    c.set_reg(BX, bx);
    c.set_word(0x39af, ax);
    c.set_word(0x39b1, bx);
    c.ret()
}

/// `initialize_mouse`: with a mouse driver (INT 33h's vector set, reset
/// AX 0 returning FFFFh), the cursor placed at 0,0 and the driver's
/// coordinate scale found: [2580h] counts up while a move to (1 << n, 0)
/// still reads back x 0, [2581h] the same for y; then the mickey ratio
/// (AX 0Fh: 10h >> each) and AX 13h.
pub fn initialize_mouse(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::ES;
    entered(start, 0xe97a)?;
    let mut c = Ctx::new(m);
    c.run(0xe97a, 1)?;
    c.set_reg(AX, 0x3533);
    c.interp(0xe97d, 0xe97f)?;
    let (es, bx) = (c.sreg(ES), c.reg(BX));
    let none = es | bx == 0;
    c.run(0xe97f, 3 + if none { 1 } else { 1 })?;
    c.set_reg(AX, es);
    c.or(W, es, bx);
    c.set_reg(AX, es | bx);
    if none {
        return c.ret();
    }
    c.set_reg(AX, 0);
    c.interp(0xe988, 0xe98a)?;
    let ax = c.reg(AX);
    let ok = ax == 0xffff;
    c.run(0xe98a, 2 + if ok { 3 } else { 1 })?;
    let ax = c.inc(W, ax);
    c.set_reg(AX, ax);
    if !ok {
        return c.ret();
    }
    c.xor(W, 0, 0);
    c.set_reg(CX, 0);
    c.xor(W, 0, 0);
    c.set_reg(DX, 0);
    c.set_reg(AX, 4);
    c.interp(0xe994, 0xe996)?;
    loop {
        let k = c.byte(0x2580).wrapping_add(1);
        let neg = k & 0x80 != 0;
        c.run(0xe996, 2 + if neg { 0 } else { 5 })?;
        let k = c.byte(0x2580) as u16;
        let k = c.inc(B, k);
        c.set_byte(0x2580, k as u8);
        if neg {
            break;
        }
        c.set_reg8(r8::CL, k as u8);
        c.set_reg(AX, 1);
        let a = c.shl(W, 1, k as u8);
        c.set_reg(AX, a);
        c.set_reg(CX, a);
        c.set_reg(AX, 4);
        c.interp(0xe9aa, 0xe9ac)?;
        c.run(0xe9ac, 1)?;
        c.set_reg(AX, 3);
        c.interp(0xe9af, 0xe9b1)?;
        c.run(0xe9b1, 1)?;
        if c.reg(CX) != 0 {
            break;
        }
    }
    loop {
        let k = c.byte(0x2581).wrapping_add(1);
        let neg = k & 0x80 != 0;
        c.run(0xe9b3, 2 + if neg { 0 } else { 4 })?;
        let k = c.byte(0x2581) as u16;
        let k = c.inc(B, k);
        c.set_byte(0x2581, k as u8);
        if neg {
            break;
        }
        c.set_reg8(r8::CL, k as u8);
        c.set_reg(DX, 1);
        let d = c.shl(W, 1, k as u8);
        c.set_reg(DX, d);
        c.set_reg(AX, 4);
        c.interp(0xe9c5, 0xe9c7)?;
        c.run(0xe9c7, 1)?;
        c.set_reg(AX, 3);
        c.interp(0xe9ca, 0xe9cc)?;
        c.run(0xe9cc, 2)?;
        let dx = c.reg(DX);
        c.or(W, dx, dx);
        if dx != 0 {
            break;
        }
    }
    c.run(0xe9d0, 10)?;
    c.set_reg(AX, 0x10);
    c.set_reg(DX, 0x10);
    let v = c.word(0x2580);
    let v = c.and(W, v, 0x7f7f);
    c.set_word(0x2580, v);
    c.set_reg(CX, v);
    let ax = c.m.cpu.shift(5, W, 0x10, v as u32 & 0xff) as u16;
    c.set_reg(AX, ax);
    c.set_reg8(r8::CL, (v >> 8) as u8);
    let dx = c.m.cpu.shift(5, W, 0x10, (v >> 8) as u32) as u16;
    c.set_reg(DX, dx);
    c.set_reg(CX, ax);
    c.set_reg(AX, 0x0f);
    c.push(dx);
    c.interp(0xe9eb, 0xe9ed)?;
    c.run(0xe9ed, 2)?;
    let v = c.pop();
    c.set_reg(DX, v);
    c.set_reg(AX, 0x13);
    c.interp(0xe9f1, 0xe9f3)?;
    c.run(0xe9f3, 1)?;
    c.ret()
}

/// `get_mouse_pos_etc` (DF1E): the input of a frame. After the keyboard
/// (`loc_0de7b`), without keyboard-only control ([2942h] bit 6) the
/// mouse read (INT 33h AX 3, scaled down by [2580h]/[2581h]) into
/// [0DC36h], [0DC38h] and its buttons into [0DC34h]; with a joystick
/// (bit 7) `loc_0dd10` adds it. Then the 13 movement keys (pressed at
/// [0CEC8h + k] or [0CEDBh + k]) summed from the table [0DC48h] (6
/// bytes each: button bits, dx, dy), the action keys ([0CEBAh],
/// [0CE9Dh], [0CEE6h]) give button 1 on their press ([0DC57h] the last
/// state). Without a move the cursor's speeds [0DC51h..] are reset; a
/// move (the keyboard cursor) is left to the original from DFB7h.
pub fn get_mouse_pos_etc(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{DI, DS};
    entered(start, 0xdf1e)?;
    let mut c = Ctx::new(m);
    c.run(0xdf1e, 1)?;
    c.call(0xde7b, 0xdf21)?;
    let f = c.byte(0x2942);
    let kb = f & 0x40 != 0;
    c.run(0xdf21, 3 + if kb { 0 } else { 1 })?;
    let al = c.reg8(r8::AL);
    c.xor(B, al as u16, al as u16);
    c.set_reg8(r8::AL, 0);
    c.test(B, f as u16, 0x40);
    if !kb {
        c.set_reg(AX, 3);
        c.interp(0xdf2d, 0xdf2f)?;
        c.run(0xdf2f, 10)?;
        let (cx, dx, bx) = (c.reg(CX), c.reg(DX), c.reg(BX));
        c.set_reg(AX, cx);
        let sc = c.word(0x2580);
        c.set_reg(CX, sc);
        let ax = c.m.cpu.shift(5, W, cx as u32, sc as u32 & 0xff) as u16;
        c.set_reg(AX, ax);
        c.set_reg8(r8::CL, (sc >> 8) as u8);
        let dx = c.m.cpu.shift(5, W, dx as u32, (sc >> 8) as u32) as u16;
        c.set_reg(DX, dx);
        c.set_reg(CX, ax);
        c.set_reg8(r8::AL, bx as u8);
        let b = c.and(B, bx & 0xff, 3);
        c.set_reg8(r8::AL, b as u8);
        c.set_word(0xdc36, ax);
        c.set_word(0xdc38, dx);
    }
    let f = c.byte(0x2942);
    let joy = f & 0x80 != 0;
    c.run(0xdf49, 3 + joy as u32)?;
    let al = c.reg8(r8::AL);
    c.set_byte(0xdc34, al);
    c.test(B, f as u16, 0x80);
    if joy {
        c.call(0xdd10, 0xdf56)?;
    }
    let di0 = c.word(0xdc48);
    let mut n = 6;
    for k in 0..13u16 {
        let a = c.byte(0xcec8 + k);
        let b = c.byte(0xcec8 + k + 1 + 0x12);
        n += 5 + if a | b != 0 { 3 } else { 0 };
    }
    c.run(0xdf56, n)?;
    c.set_reg(SI, 0xcec8);
    c.set_reg(DI, di0);
    let dx0 = c.reg(DX);
    c.xor(W, dx0, dx0);
    let (mut dx, mut bx, mut ax) = (0u16, 0u16, 0u16);
    c.set_reg(DX, 0);
    c.set_reg(BX, 0);
    c.set_reg(AX, 0);
    c.set_reg(CX, 0xd);
    let mut di = di0;
    for k in 0..13u16 {
        let a = c.lods(B, DS) as u8;
        let si = c.reg(SI);
        let b = c.byte(si.wrapping_add(0x12));
        let o = c.or(B, a as u16, b as u16) as u8;
        ax = (ax & 0xff00) | o as u16;
        c.set_reg(AX, ax);
        if o != 0 {
            let p = c.byte(di);
            let h = c.or(B, ax >> 8, p as u16);
            ax = (h << 8) | (ax & 0xff);
            c.set_reg(AX, ax);
            let v = c.word(di.wrapping_add(2));
            dx = c.add(W, dx, v);
            c.set_reg(DX, dx);
            let v = c.word(di.wrapping_add(4));
            bx = c.add(W, bx, v);
            c.set_reg(BX, bx);
        }
        di = c.add(W, di, 6);
        c.set_reg(DI, di);
        c.set_reg(CX, 12 - k);
    }
    c.run(0xdf79, 13)?;
    let a = c.byte(0xceba);
    let b = c.byte(0xce9d);
    let a = c.or(B, a as u16, b as u16);
    let b = c.byte(0xcee6);
    let a = c.or(B, a, b as u16);
    let a = c.and(B, a, 1) as u8;
    let old = c.byte(0xdc57);
    c.set_byte(0xdc57, a);
    let v = !old;
    let s = c.byte(0xdc34);
    let v = c.and(B, v as u16, s as u16);
    let v = c.or(B, v, a as u16) as u8;
    c.set_byte(0xdc34, v);
    c.set_reg(AX, ((a as u16) << 8) | v as u16);
    c.set_reg(AX, dx);
    let ax = c.or(W, dx, bx);
    c.set_reg(AX, ax);
    if ax != 0 {
        c.m.cpu.ip = 0xdfb7;
        return Err(Bail);
    }
    c.run(0xdf9d, 4)?;
    c.set_word(0xdc51, 0);
    c.set_word(0xdc53, 0);
    c.set_word(0xdc55, 0);
    c.ret()
}

/// `mouse_func_uncalled` (E9F4, a far routine for the mouse driver's
/// event callback): the position (CX, DX) into [0DC36h], [0DC38h], and
/// the button events in AL (bits 1-4: left down, left up, right down,
/// right up) applied to the buttons [0DC34h]. AX, CX, DS kept.
pub fn mouse_event(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{CF, CS, DS};
    entered(start, 0xe9f4)?;
    let mut c = Ctx::new(m);
    let ax = c.reg(AX);
    let al = ax as u8;
    let mut n = 7 + 3;
    if al >> 1 != 0 {
        n += 4;
        for k in 0..4 {
            n += 2 + ((al >> (k + 1)) & 1) as u32;
        }
    }
    c.run(0xe9f4, n)?;
    c.push(ax);
    let ds = c.sreg(DS);
    c.push(ds);
    let seg = c.word_in(CS, 0xea30);
    c.set_sreg(DS, seg);
    let (cx, dx) = (c.reg(CX), c.reg(DX));
    c.set_word(0xdc36, cx);
    c.set_word(0xdc38, dx);
    let mut a = c.shr1(B, al as u16, 1);
    c.set_reg8(r8::AL, a as u8);
    if a != 0 {
        c.push(cx);
        let mut cl = c.byte(0xdc34) as u16;
        c.set_reg8(r8::CL, cl as u8);
        for k in 0..4 {
            a = c.shr1(B, a, 1);
            c.set_reg8(r8::AL, a as u8);
            if c.flag(CF) {
                cl = match k {
                    0 => c.or(B, cl, 1),
                    1 => c.and(B, cl, 0xfe),
                    2 => c.or(B, cl, 2),
                    _ => c.and(B, cl, 0xfd),
                };
                c.set_reg8(r8::CL, cl as u8);
            }
        }
        c.set_byte(0xdc34, cl as u8);
        let v = c.pop();
        c.set_reg(CX, v);
    }
    let v = c.pop();
    c.set_sreg(DS, v);
    let v = c.pop();
    c.set_reg(AX, v);
    let ip = c.pop();
    let cs = c.pop();
    c.m.cpu.ip = ip;
    c.set_sreg(CS, cs);
    Ok(())
}
