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
    cpu::{AX, BP, BX, CF, CX, DI, DX, SI, SS},
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
