//! Moving the cursor with the keyboard or the joystick: to the nearest
//! hotspot in the direction pressed, or by an accelerated step.
//!
//! The hotspot search keeps a frame on the stack at BP (SS): `[BP]` the
//! scoring routine (one of the `nearness_*` in [`super::math`], chosen by
//! the direction), `[BP+2]` the best score so far, `[BP+4]`, `[BP+6]` the
//! best point (x, y). DX, BX are the cursor. A candidate is scored with DX,
//! BX as they are and AX, SI its point; a lower score wins.

use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DS, DX, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const CURRENT_UI_ELEMENTS: u16 = 0xd41b;
    pub const POINT_IN_RECT: u16 = 0xd6fe;
    pub const RECT_HOTSPOT: u16 = 0xe159;
}

/// The pointer to the current list of UI elements.
const UI_ELEMENTS_PTR: u16 = 0x21da;
/// One list of UI elements, and the extra points it comes with (x, y
/// words, FFFFh at the end).
const UI_ELEMENTS_WITH_POINTS: u16 = 0x201a;
const POINTS: u16 = 0x28e9;
/// What else is a hotspot: bit 7 the particles (sprites on screen), not 0
/// the records at [`MARKERS`], bit 0 the 8 points at [`EIGHT_POINTS`]
/// (when [`EIGHT_POINTS_ON`] is set).
const HOTSPOT_KINDS: u16 = 0x46eb;
const PARTICLES: u16 = 0x3cbe;
const PARTICLE_SIZE: u16 = 0x11;
/// 6-byte records: a non-zero word, x, y (its low byte).
const MARKERS: u16 = 0xa5c0;
const EIGHT_POINTS: u16 = 0x2462;
const EIGHT_POINTS_ON: u16 = 0x4749;

/// `[BP+n]` (SS).
fn frame(c: &mut Ctx, n: u16) -> u16 {
    let bp = c.reg(BP);
    c.word_in(SS, bp.wrapping_add(n))
}
fn set_frame(c: &mut Ctx, n: u16, v: u16) {
    let bp = c.reg(BP);
    c.set_word_in(SS, bp.wrapping_add(n), v);
}

/// `call word ptr [bp]` at `at` (counted in the stretch before), then
/// `cmp ax, [bp+2]; jae`: whether the candidate scored better. Counts the
/// two, and `extra` more when it did.
fn score(c: &mut Ctx, at: u16, extra: u32) -> Result<bool, Bail> {
    let f = frame(c, 0);
    c.call(f, at + 3)?;
    let (ax, best) = (c.reg(AX), frame(c, 2));
    let better = ax < best;
    c.run(at + 3, 2 + if better { extra } else { 0 })?;
    c.cmp(W, ax, best);
    if better {
        set_frame(c, 2, ax);
    }
    Ok(better)
}

/// `cmp ax, dx; jne; cmp si, bx; je`: a candidate at the cursor itself is
/// skipped. Returns whether it is scored; counts nothing.
fn not_at_cursor(c: &mut Ctx, x: u16, y: u16) -> bool {
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.cmp(W, x, dx);
    if x != dx {
        return true;
    }
    c.cmp(W, y, bx);
    y != bx
}

/// `loc_0d41b`: BP = the current list of UI elements.
pub fn current_ui_elements(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd41b)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let p = c.word(UI_ELEMENTS_PTR);
    let list = c.word_in(SS, p); // [bp]: SS
    c.set_reg(BP, list);
    c.ret()
}

/// `loc_0d6fe`: CF set when the point DX, BX is strictly inside the rect at
/// DI (left, top, right, bottom).
pub fn point_in_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd6fe)?;
    let mut c = Ctx::new(m);
    let (dx, bx, di) = (c.reg(DX), c.reg(BX), c.reg(DI));
    let r = [c.word(di), c.word(di.wrapping_add(2)), c.word(di.wrapping_add(4)), c.word(di.wrapping_add(6))];
    // The comparisons made, in order, until one says outside.
    let steps = [(dx, r[0], dx > r[0]), (dx, r[2], dx < r[2]), (bx, r[1], bx > r[1])];
    let passed = steps.iter().take_while(|s| s.2).count();
    if passed == 3 {
        c.run(start, 8)?;
        for (a, b, _) in steps {
            c.cmp(W, a, b);
        }
        c.cmp(W, bx, r[3]);
    } else {
        c.run(start, 2 * passed as u32 + 4)?;
        for &(a, b, _) in &steps[..=passed] {
            c.cmp(W, a, b);
        }
        c.set_flag(CF, false);
    }
    c.ret()
}

/// `loc_0e159`: the hotspot of the rect at DI: AX = the middle across, SI a
/// quarter of the height above the bottom.
pub fn rect_hotspot(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe159)?;
    let mut c = Ctx::new(m);
    c.run(start, 10)?;
    hotspot(&mut c);
    c.ret()
}

fn hotspot(c: &mut Ctx) {
    let di = c.reg(DI);
    let bottom = c.word(di.wrapping_add(6));
    let top = c.word(di.wrapping_add(2));
    let h = c.sub(W, bottom, top);
    let q = c.shr1(W, h, 2);
    let si = c.sub(W, bottom, q);
    c.set_reg(SI, si);
    let (left, right) = (c.word(di), c.word(di.wrapping_add(4)));
    let sum = c.add(W, left, right);
    let x = c.shr1(W, sum, 1);
    c.set_reg(AX, x);
}

/// `loc_0e068`: the extra points of the one list of UI elements that has
/// them, when it is current.
pub fn hotspots_points(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe068)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let bp = c.reg(BP);
    c.push(bp);
    c.call(code::CURRENT_UI_ELEMENTS, 0xe06c)?;
    let list = c.reg(BP);
    let ours = list == UI_ELEMENTS_WITH_POINTS;
    c.run(0xe06c, 4)?;
    c.cmp(W, list, UI_ELEMENTS_WITH_POINTS);
    let bp = c.pop();
    c.set_reg(BP, bp);
    if !ours {
        return c.ret();
    }
    let mut di = POINTS;
    c.set_reg(DI, di);
    loop {
        let (y, x) = (c.word(di.wrapping_add(2)), c.word(di));
        let (dx, bx) = (c.reg(DX), c.reg(BX));
        let at_x = x == dx;
        c.run(0xe076, if at_x { 6 } else { 4 } + if at_x && y == bx { 0 } else { 1 })?;
        c.set_reg(SI, y);
        c.set_reg(AX, x);
        if not_at_cursor(&mut c, x, y) && score(&mut c, 0xe083, 5)? {
            let x = c.word(di);
            c.set_reg(AX, x);
            set_frame(&mut c, 4, x);
            let y = c.word(di.wrapping_add(2));
            c.set_reg(SI, y);
            set_frame(&mut c, 6, y);
        }
        let next = di.wrapping_add(4);
        let end = c.word(next) == 0xffff;
        c.run(0xe099, 3 + end as u32)?;
        di = c.add(W, di, 4);
        c.set_reg(DI, di);
        let v = c.word(di);
        c.cmp(W, v, 0xffff);
        if end {
            return c.ret();
        }
    }
}

/// `loc_0e0a2`: the particles (when bit 7 of [`HOTSPOT_KINDS`] is set): each
/// not hidden (bits C0h of its byte 0Ch) and not under the cursor, at
/// its rect's hotspot.
pub fn hotspots_particles(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe0a2)?;
    let mut c = Ctx::new(m);
    let kinds = c.byte(HOTSPOT_KINDS);
    let on = kinds & 0x80 != 0;
    c.run(start, if on { 2 } else { 3 })?;
    c.cmp(B, kinds as u16, 0);
    if !on {
        return c.ret();
    }
    let count = c.word(PARTICLES);
    c.run(0xe0a9, 4)?;
    c.set_reg(DI, PARTICLES);
    c.set_reg(CX, count);
    if count == 0 {
        return c.ret();
    }
    let mut di = c.add(W, PARTICLES, 2);
    c.set_reg(DI, di);
    loop {
        let flags = c.byte(di.wrapping_add(0x0c));
        let hidden = flags & 0xc0 != 0;
        c.run(0xe0b3, if hidden { 2 } else { 3 })?;
        c.test(B, flags as u16, 0xc0);
        if !hidden {
            c.call(code::POINT_IN_RECT, 0xe0bc)?;
            let under = c.flag(CF);
            c.run(0xe0bc, if under { 1 } else { 2 })?;
            if !under {
                c.call(code::RECT_HOTSPOT, 0xe0c1)?;
                c.run(0xe0c1, 1)?;
                if score(&mut c, 0xe0c1, 2)? {
                    c.call(code::RECT_HOTSPOT, 0xe0cf)?;
                    c.run(0xe0cf, 2)?;
                    let (ax, si) = (c.reg(AX), c.reg(SI));
                    set_frame(&mut c, 4, ax);
                    set_frame(&mut c, 6, si);
                }
            }
        }
        let cx = c.reg(CX).wrapping_sub(1);
        c.run(0xe0d5, 2 + (cx == 0) as u32)?;
        di = c.reg(DI);
        di = c.add(W, di, PARTICLE_SIZE);
        c.set_reg(DI, di);
        c.set_reg(CX, cx);
        if cx == 0 {
            return c.ret();
        }
    }
}

/// `loc_0e0db`: the records at [`MARKERS`] (when [`HOTSPOT_KINDS`] is not
/// 0), up to the first whose word 0 is 0.
pub fn hotspots_markers(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe0db)?;
    let mut c = Ctx::new(m);
    let kinds = c.byte(HOTSPOT_KINDS);
    c.run(start, 3)?;
    c.cmp(B, kinds as u16, 0);
    if kinds == 0 {
        return c.ret();
    }
    let mut di = MARKERS;
    c.set_reg(DI, di);
    loop {
        let w = c.word(di);
        if w == 0 {
            c.run(0xe0e5, 3)?;
            c.cmp(W, w, 0);
            return c.ret();
        }
        let y = c.word(di.wrapping_add(4)) & 0xff;
        let x = c.word(di.wrapping_add(2));
        let (dx, bx) = (c.reg(DX), c.reg(BX));
        let at_x = x == dx;
        c.run(0xe0e5, 2 + if at_x { 7 } else { 5 } + if at_x && y == bx { 0 } else { 1 })?;
        c.cmp(W, w, 0);
        let raw = c.word(di.wrapping_add(4));
        let y = c.and(W, raw, 0xff);
        c.set_reg(SI, y);
        c.set_reg(AX, x);
        if not_at_cursor(&mut c, x, y) && score(&mut c, 0xe0fc, 6)? {
            let x = c.word(di.wrapping_add(2));
            c.set_reg(AX, x);
            set_frame(&mut c, 4, x);
            let raw = c.word(di.wrapping_add(4));
            let y = c.and(W, raw, 0xff);
            c.set_reg(SI, y);
            set_frame(&mut c, 6, y);
        }
        c.run(0xe117, 2)?;
        di = c.add(W, di, 6);
        c.set_reg(DI, di);
    }
}

/// `loc_0e11c`: the 8 points at [`EIGHT_POINTS`] (when bit 0 of
/// [`HOTSPOT_KINDS`] and [`EIGHT_POINTS_ON`] are set).
pub fn hotspots_eight_points(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe11c)?;
    let mut c = Ctx::new(m);
    let kinds = c.byte(HOTSPOT_KINDS);
    if kinds & 1 == 0 {
        c.run(start, 3)?;
        c.test(B, kinds as u16, 1);
        return c.ret();
    }
    let on = c.word(EIGHT_POINTS_ON);
    c.run(start, if on == 0 { 5 } else { 6 })?;
    c.test(B, kinds as u16, 1);
    c.cmp(W, on, 0);
    if on == 0 {
        return c.ret();
    }
    let mut di = EIGHT_POINTS;
    c.set_reg(DI, di);
    c.set_reg(CX, 8);
    loop {
        let (x, y) = (c.word(di), c.word(di.wrapping_add(2)));
        let (dx, bx) = (c.reg(DX), c.reg(BX));
        let at_x = x == dx;
        c.run(0xe130, if at_x { 6 } else { 4 } + if at_x && y == bx { 0 } else { 1 })?;
        c.set_reg(AX, x);
        c.set_reg(SI, y);
        if not_at_cursor(&mut c, x, y) && score(&mut c, 0xe13d, 5)? {
            let (x, y) = (c.word(di), c.word(di.wrapping_add(2)));
            c.set_reg(AX, x);
            c.set_reg(SI, y);
            set_frame(&mut c, 4, x);
            set_frame(&mut c, 6, y);
        }
        let cx = c.reg(CX).wrapping_sub(1);
        c.run(0xe153, 2 + (cx == 0) as u32)?;
        di = c.reg(DI);
        di = c.add(W, di, 4);
        c.set_reg(DI, di);
        c.set_reg(CX, cx);
        if cx == 0 {
            return c.ret();
        }
    }
}

/// `loc_0e243`: one axis of an accelerated step: AL × CL (signed, AL the
/// direction, CL the time) in eighths, the fraction kept in [SI] (0..7).
/// Exit: AX = the whole pixels (signed), CH = the new fraction.
pub fn accelerated_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe243)?;
    let mut c = Ctx::new(m);
    let (al, cl) = (c.reg8(r8::AL), c.reg8(r8::CL));
    let p = (al as i8 as i16).wrapping_mul(cl as i8 as i16) as u16;
    let negative = p & 0x8000 != 0;
    c.run(start, if negative { 14 } else { 12 })?;
    c.m.cpu.mul_div(&mut c.m.hw, B, 5, cl as u32); // imul cl
    c.or(W, p, p);
    let mag = if negative { c.neg(W, p) } else { p };
    let si = c.reg(SI);
    let frac = c.byte(si);
    let sum = c.add(B, mag & 0xff, frac as u16) as u8;
    let ch = c.and(B, sum as u16, 7) as u8;
    c.set_reg8(r8::CH, ch);
    c.set_byte(si, ch);
    let ax = sum as i8 as i16 as u16; // cbw
    let mut ax = c.shr1(W, ax, 3);
    if negative {
        ax = c.neg(W, ax);
    }
    c.set_reg(AX, ax);
    c.ret()
}

/// `loc_092c9`: CF set when (DX, BX) is in the rect 1C0Ch (with [1152h]
/// not FFh) or 1C1Ah (with [1153h] not FFh), by [`point_in_rect`]; CX
/// the byte.
pub fn in_dialogue_rects(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x92c9)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x1152);
    if k == 0xff {
        c.run(start, 5)?;
        c.xor(W, 0, 0);
        c.set_reg(CX, 0xff);
        c.cmp(B, 0xff, 0xff);
        return c.ret();
    }
    c.run(start, 6)?;
    c.xor(W, 0, 0);
    c.set_reg(CX, k as u16);
    c.cmp(B, k as u16, 0xff);
    c.set_reg(DI, 0x1c0c);
    c.call(0xd6fe, 0x92da)?;
    if c.flag(CF) {
        c.run(0x92da, 2)?;
        return c.ret();
    }
    let k = c.byte(0x1153);
    if k == 0xff {
        c.run(0x92da, 5)?;
        c.set_reg8(r8::CL, k);
        c.cmp(B, 0xff, 0xff);
        return c.ret();
    }
    c.run(0x92da, 6)?;
    c.set_reg8(r8::CL, k);
    c.cmp(B, k as u16, 0xff);
    c.set_reg(DI, 0x1c1a);
    point_in_rect(c.m, 0xd6fe)
}

/// `loc_09285`: what the cursor (DX, BX) is on in a dialogue: below 98h
/// the speakers' places at 47F8h (17h of them; within 20h left and 50h
/// above), CF set with CX the speaker; else the extra place
/// [472Dh]/[472Fh] (within 4Eh and 3Ch), CX 2Fh with CF. From 98h down,
/// [`in_dialogue_rects`].
pub fn dialogue_hit(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9285)?;
    let mut c = Ctx::new(m);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    if bx >= 0x98 {
        c.run(start, 2)?;
        c.cmp(W, bx, 0x98);
        return in_dialogue_rects(c.m, 0x92c9);
    }
    let step = c.string_step(W);
    let mut n = 4;
    let mut p = 0x47f8u16;
    let mut hit = None;
    for i in 0..0x17u16 {
        let x = c.word(p);
        let y = c.word(p.wrapping_add(step));
        p = p.wrapping_add(step.wrapping_mul(2));
        n += 6;
        if x & 0x8000 == 0 {
            n += 3;
            if x.wrapping_sub(dx) >= 0xffe0 {
                n += 3;
                if y.wrapping_sub(bx) >= 0xffb0 {
                    hit = Some(i);
                    n += 4;
                    break;
                }
            }
        }
        n += 1;
    }
    let ex = c.word(0x472d);
    let ey = c.word(0x472f);
    let mut extra = false;
    if hit.is_none() {
        n += 3;
        if ex == 0 {
            n += 1;
        } else {
            n += 4;
            if ex.wrapping_sub(dx) < 0xffb2 {
                n += 1;
            } else {
                n += 4;
                let d = bx.wrapping_sub(ey);
                if d >= 0x3c {
                    n += 1;
                } else {
                    extra = true;
                    n += 2;
                }
            }
        }
    }
    c.run(start, n)?;
    c.cmp(W, bx, 0x98);
    c.set_reg(SI, 0x47f8);
    c.set_reg(CX, 0x17);
    for i in 0..0x17u16 {
        let x = c.lods(W, pc::cpu::DS);
        c.set_reg(DI, x);
        let y = c.lods(W, pc::cpu::DS);
        c.set_reg(BP, y);
        c.or(W, x, x);
        if x & 0x8000 == 0 {
            let d = c.sub(W, x, dx);
            c.set_reg(DI, d);
            c.cmp(W, d, 0xffe0);
            if d >= 0xffe0 {
                let e = c.sub(W, y, bx);
                c.set_reg(BP, e);
                c.cmp(W, e, 0xffb0);
                if hit == Some(i) {
                    let cx = c.sub(W, 0x17 - i, 0x17);
                    let cx = c.neg(W, cx);
                    c.set_reg(CX, cx);
                    c.set_flag(CF, true);
                    return c.ret();
                }
            }
        }
        c.set_reg(CX, 0x16 - i);
    }
    c.set_reg(AX, ex);
    c.or(W, ex, ex);
    if ex == 0 {
        return c.ret();
    }
    let a = c.sub(W, ex, dx);
    c.set_reg(AX, a);
    c.cmp(W, a, 0xffb2);
    let cf = c.flag(CF);
    c.set_flag(CF, !cf);
    if a < 0xffb2 {
        return c.ret();
    }
    c.set_reg(AX, bx);
    let a = c.sub(W, bx, ey);
    c.set_reg(AX, a);
    c.cmp(W, a, 0x3c);
    if extra {
        c.set_reg(CX, 0x2f);
    }
    c.ret()
}

/// `highlight_hovered_text_action_item`: the hovered item CL of the
/// current UI list (FFh none) found for the cursor (DX, BX) and, when it
/// changes [0DCE7h], the old one drawn plain and the new one highlighted
/// (`item_place`, `loc_0d48a` with bit 7 of AH), then the mouse. Fixed
/// with [4774h] ([4775h]). In the dialogue list 1F0Eh outside the map
/// modes: the speaker box 1BF0h (the speaker [47C4h]) or the speaker hit
/// (`dialogue_hit`; an extra speaker as 17h), its item 78h + speaker.
/// In the list 1F7Eh: the boxes 1BE2h (0), 1BF0h (1), with [1BF8h] the
/// box 1BFEh (3), a dialogue rect of the speaker (2). Else in the button
/// row (from y 98h) the column of 1B48h's boxes (0Eh apart). All
/// registers but AX kept.
pub fn highlight_hovered_text_action_item(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd50f)?;
    let mut c = Ctx::new(m);
    let fixed = c.byte(0x4774);
    c.run(start, if fixed != 0 { 10 } else { 9 })?;
    for r in [BX, CX, DX, SI, DI, BP] {
        let v = c.reg(r);
        c.push(v);
    }
    c.cmp(B, fixed as u16, 0);
    // The blocks: 0 D5DDh (CL found), 1 D575h, 2 D5B1h, 3 D5DBh,
    // 4 D55Dh (CX the speaker), 5 D545h, 6 D593h.
    let mut at;
    if fixed != 0 {
        let cl = c.byte(0x4775);
        c.set_reg8(r8::CL, cl);
        at = 0;
    } else {
        c.call(0xd41b, 0xd526)?;
        let bp = c.reg(BP);
        let mode = c.byte(0x11c9);
        if bp != 0x1f0e || mode != 0 {
            c.run(0xd526, if bp != 0x1f0e { 2 } else { 4 })?;
            c.cmp(W, bp, 0x1f0e);
            if bp == 0x1f0e {
                c.cmp(B, mode as u16, 0);
            }
            at = 1;
        } else {
            let f = c.byte(0x1bf8);
            if f & 0x80 != 0 {
                c.run(0xd526, 8)?;
                c.cmp(W, bp, 0x1f0e);
                c.cmp(B, 0, 0);
                c.set_reg(DI, 0x1bf0);
                c.cmp(B, f as u16, 0);
                c.call(0xd6fe, 0xd53f)?;
                let cf = c.flag(CF);
                c.run(0xd53f, 2)?;
                let sp = c.word(0x47c4);
                c.set_reg(CX, sp);
                at = if cf { 4 } else { 5 };
            } else {
                c.run(0xd526, 7)?;
                c.cmp(W, bp, 0x1f0e);
                c.cmp(B, 0, 0);
                c.set_reg(DI, 0x1bf0);
                c.cmp(B, f as u16, 0);
                at = 5;
            }
        }
    }
    loop {
        match at {
            5 => {
                c.run(0xd545, 2)?;
                let bp = c.reg(BP);
                c.push(bp);
                c.call(0x9285, 0xd549)?;
                let cf = c.flag(CF);
                let cl = c.reg8(r8::CL);
                if !cf {
                    c.run(0xd549, 2)?;
                    let b = c.pop();
                    c.set_reg(BP, b);
                    at = 1;
                    continue;
                }
                let a = cl.wrapping_sub(0x0f);
                let extra = c.byte(0x476b);
                let n = if cl < 0x0f { 5 } else if a.wrapping_add(1) != extra { 8 } else { 9 };
                c.run(0xd549, n)?;
                let b = c.pop();
                c.set_reg(BP, b);
                c.set_reg8(r8::AL, cl);
                let a = c.sub(B, cl as u16, 0x0f);
                c.set_reg8(r8::AL, a as u8);
                if cl >= 0x0f {
                    let a = c.inc(B, a);
                    c.set_reg8(r8::AL, a as u8);
                    c.cmp(B, a, extra as u16);
                    if a as u8 == extra {
                        c.set_reg(CX, 0x17);
                    }
                }
                at = 4;
            }
            4 => {
                c.run(0xd55d, 3)?;
                let cx = c.reg(CX);
                let bp = c.add(W, cx, 0x78);
                c.set_reg(BP, bp);
                c.xor(W, 0, 0);
                c.set_reg(CX, 0);
                loop {
                    c.run(0xd565, 1)?;
                    c.call(0xd454, 0xd568)?;
                    let ax = c.reg(AX);
                    let bp = c.reg(BP);
                    if ax == bp {
                        c.run(0xd568, 2)?;
                        c.cmp(W, ax, bp);
                        at = 0;
                        break;
                    }
                    let cx = c.reg(CX).wrapping_add(1);
                    let n = c.byte(0xdce8);
                    let more = (cx as u8) < n;
                    c.run(0xd568, if more { 5 } else { 6 })?;
                    c.cmp(W, ax, bp);
                    let cx = c.inc(W, cx.wrapping_sub(1));
                    c.set_reg(CX, cx);
                    c.cmp(B, cx & 0xff, n as u16);
                    if !more {
                        at = 3;
                        break;
                    }
                }
            }
            1 => {
                let bp = c.reg(BP);
                if bp != 0x1f7e {
                    c.run(0xd575, 2)?;
                    c.cmp(W, bp, 0x1f7e);
                    at = 2;
                    continue;
                }
                let f = c.byte(0x1bea);
                if f & 0x80 == 0 {
                    c.run(0xd575, 5)?;
                    c.cmp(W, bp, 0x1f7e);
                    c.set_reg(DI, 0x1be2);
                    c.cmp(B, f as u16, 0);
                    at = 6;
                    continue;
                }
                c.run(0xd575, 7)?;
                c.cmp(W, bp, 0x1f7e);
                c.set_reg(DI, 0x1be2);
                c.cmp(B, f as u16, 0);
                c.xor(W, 0, 0);
                c.set_reg(CX, 0);
                c.call(0xd6fe, 0xd589)?;
                if c.flag(CF) {
                    c.run(0xd589, 1)?;
                    at = 0;
                    continue;
                }
                c.run(0xd589, 3)?;
                c.set_reg(DI, 0x1bf0);
                c.call(0xd6fe, 0xd591)?;
                c.run(0xd591, 1)?;
                at = if c.flag(CF) { 0 } else { 6 };
            }
            6 => {
                let f = c.byte(0x1bf8);
                if f & 0x80 == 0 {
                    c.run(0xd593, 2)?;
                    c.cmp(B, f as u16, 0);
                    at = 2;
                    continue;
                }
                c.run(0xd593, 4)?;
                c.cmp(B, f as u16, 0);
                c.set_reg(DI, 0x1bfe);
                c.call(0xd6fe, 0xd5a0)?;
                let cf = c.flag(CF);
                if cf {
                    c.run(0xd5a0, 2)?;
                    c.set_reg8(r8::CL, 3);
                    at = 0;
                    continue;
                }
                c.run(0xd5a0, 3)?;
                c.set_reg8(r8::CL, 3);
                c.call(0x92c9, 0xd5a7)?;
                if !c.flag(CF) {
                    c.run(0xd5a7, 1)?;
                    at = 2;
                    continue;
                }
                let (cx, sp) = (c.reg(CX), c.word(0x47c4));
                c.run(0xd5a7, 4)?;
                c.cmp(W, cx, sp);
                c.set_reg8(r8::CL, 2);
                at = if cx == sp { 0 } else { 2 };
            }
            2 => {
                let (bx, dx) = (c.reg(BX), c.reg(DX));
                if bx < 0x98 {
                    c.run(0xd5b1, 2)?;
                    c.cmp(W, bx, 0x98);
                    at = 3;
                    continue;
                }
                let (l, r) = (c.word(0x1b48), c.word(0x1b4c));
                if dx < l || dx >= r {
                    c.run(0xd5b1, if dx < l { 6 } else { 8 })?;
                    c.cmp(W, bx, 0x98);
                    c.set_reg8(r8::CL, 0xff);
                    c.set_reg(DI, 0x1b48);
                    c.cmp(W, dx, l);
                    if dx >= l {
                        c.cmp(W, dx, r);
                    }
                    at = 0;
                    continue;
                }
                c.run(0xd5b1, 9)?;
                c.cmp(W, bx, 0x98);
                c.set_reg8(r8::CL, 0xff);
                c.set_reg(DI, 0x1b48);
                c.cmp(W, dx, l);
                c.cmp(W, dx, r);
                c.xor(W, 0, 0);
                c.set_reg(CX, 0);
                loop {
                    let di = c.reg(DI);
                    let (t, b) = (c.word(di.wrapping_add(2)), c.word(di.wrapping_add(6)));
                    if bx <= t {
                        c.run(0xd5c7, 2)?;
                        c.cmp(W, bx, t);
                        at = 3;
                        break;
                    }
                    if bx <= b {
                        c.run(0xd5c7, 4)?;
                        c.cmp(W, bx, t);
                        c.cmp(W, bx, b);
                        at = 0;
                        break;
                    }
                    let cx = c.reg(CX).wrapping_add(1);
                    let n = c.byte(0xdce8);
                    c.run(0xd5c7, 8)?;
                    c.cmp(W, bx, t);
                    c.cmp(W, bx, b);
                    let d = c.add(W, di, 0x0e);
                    c.set_reg(DI, d);
                    let cx = c.inc(W, cx.wrapping_sub(1));
                    c.set_reg(CX, cx);
                    c.cmp(B, cx & 0xff, n as u16);
                    if (cx as u8) >= n {
                        at = 3;
                        break;
                    }
                }
            }
            3 => {
                c.run(0xd5db, 1)?;
                c.set_reg8(r8::CL, 0xff);
                at = 0;
            }
            _ => break,
        }
    }
    // D5DDh: the change.
    let cl = c.reg8(r8::CL);
    let old = c.byte(0xdce7);
    if cl == old {
        c.run(0xd5dd, 4 + 7)?;
        c.set_reg8(r8::AL, cl);
        c.set_reg8(r8::CL, old);
        c.cmp(B, cl as u16, old as u16);
        return highlight_pops(&mut c);
    }
    c.run(0xd5dd, 5)?;
    c.set_reg8(r8::AL, cl);
    c.set_byte(0xdce7, cl);
    c.set_reg8(r8::CL, old);
    c.cmp(B, cl as u16, old as u16);
    c.call(0xdbb2, 0xd5ea)?;
    let n = c.byte(0xdce8);
    let cl = c.reg8(r8::CL);
    if cl & 0x80 != 0 || cl >= n {
        c.run(0xd5ea, if cl & 0x80 != 0 { 2 } else { 4 })?;
        c.or(B, cl as u16, cl as u16);
        if cl & 0x80 == 0 {
            c.cmp(B, cl as u16, n as u16);
        }
    } else {
        c.run(0xd5ea, 6)?;
        c.or(B, cl as u16, cl as u16);
        c.cmp(B, cl as u16, n as u16);
        let ax = c.reg(AX);
        c.push(ax);
        c.call(0xd454, 0xd5f8)?;
        c.run(0xd5f8, 1)?;
        c.call(0xd48a, 0xd5fb)?;
        c.run(0xd5fb, 1)?;
        let a = c.pop();
        c.set_reg(AX, a);
    }
    let al = c.reg8(r8::AL);
    let n = c.byte(0xdce8);
    if al >= n {
        c.run(0xd5fc, 3)?;
        c.cmp(B, al as u16, n as u16);
    } else {
        c.run(0xd5fc, 4)?;
        c.cmp(B, al as u16, n as u16);
        let ax = c.reg(AX);
        c.set_reg(CX, ax);
        c.call(0xd454, 0xd607)?;
        c.run(0xd607, 2)?;
        let ah = (c.reg(AX) >> 8) as u8;
        let h = c.or(B, ah as u16, 0x80);
        c.set_reg8(r8::AH, h as u8);
        c.call(0xd48a, 0xd60d)?;
        c.run(0xd60d, 1)?;
    }
    c.call(0xdbec, 0xd610)?;
    c.run(0xd610, 7)?;
    highlight_pops(&mut c)
}

/// D610h: the saved registers back (the run already counted).
fn highlight_pops(c: &mut Ctx) -> Result<(), Bail> {
    for r in [BP, DI, SI, DX, CX, BX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// `loc_0d338`: [`install_ui_list`] with no item highlighted (CL FFh).
pub fn install_ui_list_d338(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd338)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.set_reg8(r8::CL, 0xff);
    install_from(&mut c, 0)
}

/// `loc_0d33a`: the UI list BP (on the stack segment; its level byte
/// first, then 4-byte items to a 0 word) installed with its click
/// handler BX on the stack of lists 21D6h.. ([21DAh] the top): lists of
/// a higher level are closed first (their handlers +2 called), one of a
/// lower level is pushed above; its items enabled (bit 15 cleared) and
/// item CL (below 5) marked (bit 7 of +3); then the panel's 5 slots
/// drawn from the list (`loc_0d48a`; [0DCE4h] its count byte, [0DCE8h]
/// the slots used, [0DCE5h] the one with the more mark A0h) and the
/// hover highlight redone at the mouse ([`highlight_hovered_text_action_item`]).
pub fn install_ui_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd33a)?;
    let mut c = Ctx::new(m);
    install_from(&mut c, 0)
}

/// `loc_0d36d`: the list BP replacing the top one (see [`install_ui_list`]).
pub fn replace_ui_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd36d)?;
    let mut c = Ctx::new(m);
    install_from(&mut c, 1)
}

/// `loc_0d397`: the panel's slots redrawn from the top list (see
/// [`install_ui_list`]).
pub fn redraw_ui_slots(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd397)?;
    let mut c = Ctx::new(m);
    if c.flag(pc::cpu::DF) {
        return Err(Bail);
    }
    install_from(&mut c, 2)
}

/// [`install_ui_list`] from D33Ah (0), D36Dh (1) or D397h (2).
fn install_from(c: &mut Ctx, from: u8) -> Result<(), Bail> {
    if from == 0 {
        let mut k = 0;
        loop {
            k += 1;
            if k > 20 {
                c.m.cpu.ip = 0xd33a;
                return Err(Bail);
            }
            let si = c.word(0x21da);
            let di = c.word(si);
            let bp = c.reg(BP);
            let lv = c.byte_in(SS, bp);
            let top = c.byte(di);
            if lv == top {
                c.run(0xd33a, 5)?;
                c.set_reg(SI, si);
                c.set_reg(DI, di);
                c.set_reg8(r8::AL, lv);
                c.cmp(B, lv as u16, top as u16);
                break;
            }
            if lv < top {
                let full = si == 0x21d6;
                c.run(0xd33a, 8 + if full { 0 } else { 2 })?;
                c.set_reg(SI, si);
                c.set_reg(DI, di);
                c.set_reg8(r8::AL, lv);
                c.cmp(B, lv as u16, top as u16);
                c.cmp(W, si, 0x21d6);
                if !full {
                    let s = c.add(W, si, 4);
                    c.set_reg(SI, s);
                    c.set_word(0x21da, s);
                }
                break;
            }
            c.run(0xd33a, 11)?;
            c.set_reg(SI, si);
            c.set_reg(DI, di);
            c.set_reg8(r8::AL, lv);
            c.cmp(B, lv as u16, top as u16);
            let (bx, cx) = (c.reg(BX), c.reg(CX));
            c.push(bp);
            c.push(bx);
            c.push(cx);
            let h = c.word(si.wrapping_add(2));
            c.set_reg(AX, h);
            c.call(h, 0xd351)?;
            c.run(0xd351, 5)?;
            let t = c.word(0x21da);
            let t = c.sub(W, t, 4);
            c.set_word(0x21da, t);
            let v = c.pop();
            c.set_reg(CX, v);
            let v = c.pop();
            c.set_reg(BX, v);
            let v = c.pop();
            c.set_reg(BP, v);
        }
        c.run(0xd368, 2)?;
        let (si, bp, bx) = (c.reg(SI), c.reg(BP), c.reg(BX));
        c.set_word(si, bp);
        c.set_word(si.wrapping_add(2), bx);
    }
    if from <= 1 {
        c.run(0xd36d, 4)?;
        let si = c.word(0x21da);
        let bp = c.reg(BP);
        c.set_reg(SI, si);
        c.set_word(si, bp);
        c.set_reg(SI, bp);
        let mut p = c.add(W, bp, 2);
        c.set_reg(BP, p);
        let mut k = 0;
        loop {
            k += 1;
            if k > 200 {
                c.m.cpu.ip = 0xd378;
                return Err(Bail);
            }
            let w = c.word_in(SS, p);
            if w == 0 {
                c.run(0xd378, 2)?;
                c.cmp(W, w, 0);
                break;
            }
            c.run(0xd378, 5)?;
            c.cmp(W, w, 0);
            let w = c.and(W, w, 0x7fff);
            c.set_word_in(SS, p, w);
            p = c.add(W, p, 4);
            c.set_reg(BP, p);
        }
        let cx = c.reg(CX);
        let si = c.reg(SI);
        if cx < 5 {
            c.run(0xd388, 6)?;
            c.cmp(W, cx, 5);
            let b = c.shl1(W, cx, 2);
            c.set_reg(CX, b);
            c.set_reg(BX, b);
            let a = b.wrapping_add(si).wrapping_add(3);
            let v = c.byte(a);
            let v = c.or(B, v as u16, 0x80);
            c.set_byte(a, v as u8);
        } else {
            c.run(0xd388, 2)?;
            c.cmp(W, cx, 5);
        }
        if c.flag(pc::cpu::DF) {
            c.m.cpu.ip = 0xd397;
            return Err(Bail);
        }
    }
    // D397h: the slots.
    c.run(0xd397, 11)?;
    c.set_byte(0xdce7, 0xff);
    let t = c.word(0x21da);
    let l = c.word(t);
    let si = c.inc(W, l);
    c.set_reg(SI, si);
    let n = c.lods(B, DS) as u8;
    c.set_byte(0xdce4, n);
    let si = c.reg(SI).wrapping_add(n as i8 as i16 as u16);
    let ax = n as i8 as i16 as u16;
    let si = c.add(W, si.wrapping_sub(ax), ax);
    c.set_reg(SI, si);
    c.xor(W, 0, 0);
    c.set_reg(CX, 0);
    c.set_byte(0xdce8, 0);
    c.set_byte(0xdce5, 0xff);
    let mut cl = 0u8;
    loop {
        let si = c.reg(SI);
        let ax = c.word(si);
        if ax == 0 {
            break;
        }
        let cnt = c.byte(0xdce4);
        let next = c.word(si.wrapping_add(4));
        let more = cl >= 4 && (cnt != 0 || next != 0);
        let n = 3 + if cl < 4 { 2 } else if cnt != 0 { 4 } else if next == 0 { 6 } else { 6 } + if more { 3 } else { 0 } + 5;
        c.run(0xd3b5, n)?;
        c.set_reg(AX, ax);
        c.or(W, ax, ax);
        c.cmp(B, cl as u16, 4);
        if cl >= 4 {
            c.cmp(B, cnt as u16, 0);
            if cnt == 0 {
                c.cmp(W, next, 0);
            }
        }
        if more {
            let v = c.or(B, cnt as u16, 0x80);
            c.set_byte(0xdce4, v as u8);
            c.set_reg(AX, 0xa0);
            c.set_byte(0xdce5, cl);
        }
        let s = c.add(W, si, 4);
        c.set_reg(SI, s);
        let cx = c.reg(CX);
        c.push(cx);
        c.push(s);
        let u = c.byte(0xdce8);
        let u = c.inc(B, u as u16);
        c.set_byte(0xdce8, u as u8);
        c.call(0xd48a, 0xd3e5)?;
        let cx = c.word_in(SS, c.reg(SP).wrapping_add(2)).wrapping_add(1);
        let done = (cx as u8) >= 5;
        c.run(0xd3e5, 5 + if done { 1 } else { 0 })?;
        let s = c.pop();
        c.set_reg(SI, s);
        let x = c.pop();
        let x = c.inc(W, x);
        c.set_reg(CX, x);
        c.cmp(B, x & 0xff, 5);
        cl = x as u8;
        if done {
            return slots_end(c);
        }
    }
    // D3EFh: the empty slots.
    let cnt = c.byte(0xdce4);
    let n = 3 + 2 + if cnt != 0 { 4 } else { 1 } + 2;
    c.run(0xd3b5, n)?;
    c.set_reg(AX, 0);
    c.or(W, 0, 0);
    c.cmp(B, cnt as u16, 0);
    if cnt != 0 {
        c.set_reg(AX, 0xa0);
        c.set_byte(0xdce5, cl);
        let u = c.byte(0xdce8);
        let u = c.inc(B, u as u16);
        c.set_byte(0xdce8, u as u8);
    } else {
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
    }
    loop {
        let cx = c.reg(CX);
        c.push(cx);
        c.call(0xd48a, 0xd409)?;
        let cx = c.word_in(SS, c.reg(SP)).wrapping_add(1);
        let done = (cx as u8) >= 5;
        c.run(0xd409, 4 + if done { 0 } else { 3 })?;
        let x = c.pop();
        let x = c.inc(W, x);
        c.set_reg(CX, x);
        c.cmp(B, x & 0xff, 5);
        if done {
            return slots_end(c);
        }
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
    }
}

/// D410h: the hover highlight at the mouse.
fn slots_end(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xd410, 3)?;
    let (x, y) = (c.word(0xdc36), c.word(0xdc38));
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    highlight_hovered_text_action_item(c.m, 0xd50f)
}
