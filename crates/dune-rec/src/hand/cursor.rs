//! Moving the cursor with the keyboard or the joystick: to the nearest
//! hotspot in the direction pressed, or by an accelerated step.
//!
//! The hotspot search keeps a frame on the stack at BP (SS): `[BP]` the
//! scoring routine (one of the `nearness_*` in [`super::math`], chosen by
//! the direction), `[BP+2]` the best score so far, `[BP+4]`, `[BP+6]` the
//! best point (x, y). DX, BX are the cursor. A candidate is scored with DX,
//! BX as they are and AX, SI its point; a lower score wins.

use super::game::{UiElement, ui_flag};
use super::game::{Particle};
use super::screens;
use super::entry;
use super::globals as g;
use super::pure;
use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DS, DX, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const HOTSPOTS_EIGHT_POINTS: u16 = 0xe11c;
    pub const HOTSPOTS_MARKERS: u16 = 0xe0db;
    pub const HOTSPOTS_PARTICLES: u16 = 0xe0a2;
    pub const HOTSPOTS_POINTS: u16 = 0xe068;
    pub const KB_CLEAR_SCANCODE: u16 = 0xde4e;
    pub const MOVE_CURSOR_BY: u16 = 0xdaaf;
    pub const RECT_HOTSPOT: u16 = 0xe159;
}

/// One list of UI elements, and the extra points it comes with (x, y
/// words, FFFFh at the end).
const UI_ELEMENTS_WITH_POINTS: u16 = 0x201a;
const POINTS: u16 = 0x28e9;
const PARTICLE_SIZE: u16 = 0x11;
const EIGHT_POINTS: u16 = 0x2462;

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
    let p = c.word(g::UI_STACK_TOP);
    let list = c.word_in(SS, p); // [bp]: SS
    c.set_reg(BP, list);
    c.ret()
}

/// [`current_ui_elements`] called: the current list of UI elements (BP).
pub(super) fn current_ui_list(c: &mut Ctx, ret: u16) -> Result<u16, Bail> {
    c.call(entry::CURRENT_UI_ELEMENTS, ret)?;
    Ok(c.reg(BP))
}

/// `loc_0d6fe`: CF set when the point DX, BX is strictly inside the rect at
/// DI (left, top, right, bottom).
pub fn point_in_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd6fe)?;
    let mut c = Ctx::new(m);
    let (dx, bx, di) = (c.reg(DX), c.reg(BX), c.reg(DI));
    let r = [c.word(di), c.word(di.wrapping_add(2)), c.word(di.wrapping_add(4)), c.word(di.wrapping_add(6))];
    // The tests made, in order, until one says outside: two
    // instructions each, after the first four.
    let tests = [dx > r[0], dx < r[2], bx > r[1]];
    let passed = tests.iter().take_while(|&&t| t).count();
    c.run(start, if passed == 3 { 8 } else { 2 * passed as u32 + 4 })?;
    // Its callers read CF alone.
    c.ret_cf(passed == 3 && bx < r[3])
}

/// [`point_in_rect`] called: whether the point (DX, BX) is strictly inside the rect at DI.
pub(super) fn inside_rect(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    c.call_cf(0xd6fe, ret)
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

/// [`rect_hotspot`] called: the hotspot of the rect at DI: the middle across (AX), a quarter above the bottom (SI).
pub(super) fn rect_hotspot_point(c: &mut Ctx, ret: u16) -> Result<(u16, u16), Bail> {
    c.call(entry::RECT_HOTSPOT, ret)?;
    Ok((c.reg(AX), c.reg(SI)))
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
    let list = current_ui_list(&mut c, 0xe06c)?;
    let ours = list == UI_ELEMENTS_WITH_POINTS;
    c.run(0xe06c, 4)?;
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
        di = pure::add(W, di, 4);
        c.set_reg(DI, di);
        if end {
            return c.ret();
        }
    }
}

/// `loc_0e0a2`: the particles (when bit 7 of [`g::MAP_MODE`] is set): each
/// not hidden (bits C0h of its byte 0Ch) and not under the cursor, at
/// its rect's hotspot.
pub fn hotspots_particles(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe0a2)?;
    let mut c = Ctx::new(m);
    let kinds = c.map_view().mode;
    let on = kinds & 0x80 != 0;
    c.run(start, if on { 2 } else { 3 })?;
    if !on {
        return c.ret();
    }
    let count = c.word(g::PARTICLES);
    c.run(0xe0a9, 4)?;
    c.set_reg(DI, g::PARTICLES);
    c.set_reg(CX, count);
    if count == 0 {
        return c.ret();
    }
    let mut di = pure::add(W, g::PARTICLES, 2);
    c.set_reg(DI, di);
    loop {
        let flags = Particle(di).flags(&mut c);
        let hidden = flags & 0xc0 != 0;
        c.run(0xe0b3, if hidden { 2 } else { 3 })?;
        if !hidden {
            let under = inside_rect(&mut c, 0xe0bc)?;
            c.run(0xe0bc, if under { 1 } else { 2 })?;
            if !under {
                c.call(code::RECT_HOTSPOT, 0xe0c1)?;
                c.run(0xe0c1, 1)?;
                if score(&mut c, 0xe0c1, 2)? {
                    let (ax, si) = rect_hotspot_point(&mut c, 0xe0cf)?;
                    c.run(0xe0cf, 2)?;
                    set_frame(&mut c, 4, ax);
                    set_frame(&mut c, 6, si);
                }
            }
        }
        let cx = c.reg(CX).wrapping_sub(1);
        c.run(0xe0d5, 2 + (cx == 0) as u32)?;
        di = c.reg(DI);
        di = pure::add(W, di, PARTICLE_SIZE);
        c.set_reg(DI, di);
        c.set_reg(CX, cx);
        if cx == 0 {
            return c.ret();
        }
    }
}

/// `loc_0e0db`: the records at [`g::MAP_MARKERS`] (when [`g::MAP_MODE`] is not
/// 0), up to the first whose word 0 is 0.
pub fn hotspots_markers(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe0db)?;
    let mut c = Ctx::new(m);
    let kinds = c.map_view().mode;
    c.run(start, 3)?;
    if kinds == 0 {
        return c.ret();
    }
    let mut di = g::MAP_MARKERS;
    c.set_reg(DI, di);
    loop {
        let w = c.word(di);
        if w == 0 {
            c.run(0xe0e5, 3)?;
            return c.ret();
        }
        let y = c.word(di.wrapping_add(4)) & 0xff;
        let x = c.word(di.wrapping_add(2));
        let (dx, bx) = (c.reg(DX), c.reg(BX));
        let at_x = x == dx;
        c.run(0xe0e5, 2 + if at_x { 7 } else { 5 } + if at_x && y == bx { 0 } else { 1 })?;
        let raw = c.word(di.wrapping_add(4));
        let y = pure::and(W, raw, 0xff);
        c.set_reg(SI, y);
        c.set_reg(AX, x);
        if not_at_cursor(&mut c, x, y) && score(&mut c, 0xe0fc, 6)? {
            let x = c.word(di.wrapping_add(2));
            c.set_reg(AX, x);
            set_frame(&mut c, 4, x);
            let raw = c.word(di.wrapping_add(4));
            let y = pure::and(W, raw, 0xff);
            c.set_reg(SI, y);
            set_frame(&mut c, 6, y);
        }
        c.run(0xe117, 2)?;
        di = pure::add(W, di, 6);
        c.set_reg(DI, di);
    }
}

/// `loc_0e11c`: the 8 points at [`EIGHT_POINTS`] (when bit 0 of
/// [`g::MAP_MODE`] and [`g::MAP_POINTER_BOX`] are set).
pub fn hotspots_eight_points(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe11c)?;
    let mut c = Ctx::new(m);
    let kinds = c.map_view().mode;
    if kinds & 1 == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let on = c.word(g::MAP_POINTER_BOX);
    c.run(start, if on == 0 { 5 } else { 6 })?;
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
        di = pure::add(W, di, 4);
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
    let mag = if negative { pure::neg(W, p) } else { p };
    let si = c.reg(SI);
    let frac = c.byte(si);
    let sum = pure::add(B, mag & 0xff, frac as u16) as u8;
    let ch = pure::and(B, sum as u16, 7) as u8;
    c.set_reg8(r8::CH, ch);
    c.set_byte(si, ch);
    let ax = sum as i8 as i16 as u16; // cbw
    let mut ax = pure::shr1(W, ax, 3);
    if negative {
        ax = pure::neg(W, ax);
    }
    c.set_reg(AX, ax);
    c.ret()
}

/// [`accelerated_step`] called: the whole pixels of one axis of an accelerated step (AX, signed).
pub(super) fn accelerated_pixels(c: &mut Ctx, ret: u16) -> Result<u16, Bail> {
    c.call(entry::ACCELERATED_STEP, ret)?;
    Ok(c.reg(AX))
}

/// `loc_092c9`: CF set when (DX, BX) is in the rect 1C0Ch (with [1152h]
/// not FFh) or 1C1Ah (with [1153h] not FFh), by [`point_in_rect`]; CX
/// the byte.
pub fn in_dialogue_rects(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x92c9)?;
    let mut c = Ctx::new(m);
    let k = c.byte(g::SPEAKER_SLOTS);
    if k == 0xff {
        c.run(start, 5)?;
        c.set_reg(CX, 0xff);
        c.set_flag(CF, false); // its callers read CF alone
        return c.ret();
    }
    c.run(start, 6)?;
    c.set_reg(CX, k as u16);
    c.set_reg(DI, 0x1c0c);
    if inside_rect(&mut c, 0x92da)? {
        c.run(0x92da, 2)?;
        return c.ret();
    }
    let k = c.byte(g::SPEAKER_SLOT_2);
    if k == 0xff {
        c.run(0x92da, 5)?;
        c.set_reg8(r8::CL, k);
        return c.ret_cf(false);
    }
    c.run(0x92da, 6)?;
    c.set_reg8(r8::CL, k);
    c.set_reg(DI, 0x1c1a);
    point_in_rect(c.m, 0xd6fe)
}

/// [`in_dialogue_rects`] called: whether (DX, BX) is in one of the two dialogue rects (CX = its byte).
pub(super) fn in_dialogue_rect(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    c.call_cf(0x92c9, ret)
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
    let ex = c.word(g::ROOM_FIGURES_X);
    let ey = c.word(g::ROOM_FIGURES_Y);
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
    // The speakers' places, then the extra one; CF set on a hit.
    c.set_reg(SI, 0x47f8);
    c.set_reg(CX, 0x17);
    for i in 0..0x17u16 {
        let x = c.lods(W, pc::cpu::DS);
        c.set_reg(DI, x);
        let y = c.lods(W, pc::cpu::DS);
        c.set_reg(BP, y);
        if x & 0x8000 == 0 {
            let d = x.wrapping_sub(dx);
            c.set_reg(DI, d);
            if d >= 0xffe0 {
                c.set_reg(BP, y.wrapping_sub(bx));
                if hit == Some(i) {
                    c.set_reg(CX, i);
                    return c.ret_cf(true);
                }
            }
        }
        c.set_reg(CX, 0x16 - i);
    }
    c.set_reg(AX, ex);
    if ex == 0 {
        c.set_flag(CF, false); // `or ax, ax`
        return c.ret();
    }
    let a = ex.wrapping_sub(dx);
    c.set_reg(AX, a);
    if a < 0xffb2 {
        c.set_flag(CF, false); // `cmp; cmc`
        return c.ret();
    }
    c.set_reg(AX, bx.wrapping_sub(ey));
    if extra {
        c.set_reg(CX, 0x2f);
    }
    c.set_flag(CF, extra); // `cmp ax, 3Ch`
    c.ret()
}

/// [`dialogue_hit`] called: whether the cursor (DX, BX) is on a speaker's place of the dialogue (CX = which).
pub(super) fn on_dialogue_place(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    c.call_cf(0x9285, ret)
}

/// `highlight_hovered_text_action_item`: the hovered item CL of the
/// current UI list (FFh none) found for the cursor (DX, BX) and, when it
/// changes [`g::HOVERED_ITEM`], the old one drawn plain and the new one highlighted
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
    let fixed = c.sequence().playing;
    c.run(start, if fixed != 0 { 10 } else { 9 })?;
    for r in [BX, CX, DX, SI, DI, BP] {
        let v = c.reg(r);
        c.push(v);
    }
    // The blocks: 0 D5DDh (CL found), 1 D575h, 2 D5B1h, 3 D5DBh,
    // 4 D55Dh (CX the speaker), 5 D545h, 6 D593h.
    let mut at;
    if fixed != 0 {
        let cl = c.sequence().blink;
        c.set_reg8(r8::CL, cl);
        at = 0;
    } else {
        let ui_list = current_ui_list(&mut c, 0xd526)?;
        let mode = c.travel().screen_kind;
        if ui_list != 0x1f0e || mode != 0 {
            c.run(0xd526, if ui_list != 0x1f0e { 2 } else { 4 })?;
            if ui_list == 0x1f0e {
            }
            at = 1;
        } else {
            let f = c.byte(g::HEAD_BOX_FLAGS);
            c.run(0xd526, if f & 0x80 != 0 { 8 } else { 7 })?;
            if f & 0x80 != 0 {
                c.set_reg(DI, 0x1bf0);
                let inside_rect = inside_rect(&mut c, 0xd53f)?;
                c.run(0xd53f, 2)?;
                let sp = c.conversation().talker;
                c.set_reg(CX, sp);
                at = if inside_rect { 4 } else { 5 };
            } else {
                c.set_reg(DI, 0x1bf0);
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
                let on_dialogue_place = on_dialogue_place(&mut c, 0xd549)?;
                let cl = c.reg8(r8::CL);
                if !on_dialogue_place {
                    c.run(0xd549, 2)?;
                    let b = c.pop();
                    c.set_reg(BP, b);
                    at = 1;
                    continue;
                }
                let a = cl.wrapping_sub(0x0f);
                let extra = c.byte(g::CHARACTER_15_SPECIAL_SLOT);
                let n = if cl < 0x0f { 5 } else if a.wrapping_add(1) != extra { 8 } else { 9 };
                c.run(0xd549, n)?;
                let b = c.pop();
                c.set_reg(BP, b);
                c.set_reg8(r8::AL, cl);
                c.set_reg8(r8::AL, a);
                if cl >= 0x0f {
                    let a = a.wrapping_add(1);
                    c.set_reg8(r8::AL, a);
                    if a == extra {
                        c.set_reg(CX, 0x17);
                    }
                }
                at = 4;
            }
            4 => {
                c.run(0xd55d, 3)?;
                let cx = c.reg(CX);
                let bp = cx.wrapping_add(0x78);
                c.set_reg(BP, bp);
                c.set_reg(CX, 0);
                loop {
                    c.run(0xd565, 1)?;
                    let item = c.reg8(r8::CL);
                    let (item_x, _) = screens::item_position(&mut c, item, 0xd568)?;
                    let bp = c.reg(BP);
                    if item_x == bp {
                        c.run(0xd568, 2)?;
                        at = 0;
                        break;
                    }
                    let cx = c.reg(CX).wrapping_add(1);
                    let n = c.byte(g::ITEM_COUNT);
                    let more = (cx as u8) < n;
                    c.run(0xd568, if more { 5 } else { 6 })?;
                    c.set_reg(CX, cx);
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
                    at = 2;
                    continue;
                }
                let f = c.byte(g::TEXT_BOX_FLAGS);
                if f & 0x80 == 0 {
                    c.run(0xd575, 5)?;
                    c.set_reg(DI, g::TEXT_BOX);
                    at = 6;
                    continue;
                }
                c.run(0xd575, 7)?;
                c.set_reg(DI, g::TEXT_BOX);
                c.set_reg(CX, 0);
                if inside_rect(&mut c, 0xd589)? {
                    c.run(0xd589, 1)?;
                    at = 0;
                    continue;
                }
                c.run(0xd589, 3)?;
                c.set_reg(DI, 0x1bf0);
                let inside_rect = inside_rect(&mut c, 0xd591)?;
                c.run(0xd591, 1)?;
                at = if inside_rect { 0 } else { 6 };
            }
            6 => {
                let f = c.byte(g::HEAD_BOX_FLAGS);
                if f & 0x80 == 0 {
                    c.run(0xd593, 2)?;
                    at = 2;
                    continue;
                }
                c.run(0xd593, 4)?;
                c.set_reg(DI, 0x1bfe);
                let inside_rect = inside_rect(&mut c, 0xd5a0)?;
                if inside_rect {
                    c.run(0xd5a0, 2)?;
                    c.set_reg8(r8::CL, 3);
                    at = 0;
                    continue;
                }
                c.run(0xd5a0, 3)?;
                c.set_reg8(r8::CL, 3);
                if !in_dialogue_rect(&mut c, 0xd5a7)? {
                    c.run(0xd5a7, 1)?;
                    at = 2;
                    continue;
                }
                let (cx, sp) = (c.reg(CX), c.conversation().talker);
                c.run(0xd5a7, 4)?;
                c.set_reg8(r8::CL, 2);
                at = if cx == sp { 0 } else { 2 };
            }
            2 => {
                let (bx, dx) = (c.reg(BX), c.reg(DX));
                if bx < 0x98 {
                    c.run(0xd5b1, 2)?;
                    at = 3;
                    continue;
                }
                let (l, r) = (c.word(g::TEXT_ACTIONS), c.word(g::TEXT_ACTIONS_X1));
                if dx < l || dx >= r {
                    c.run(0xd5b1, if dx < l { 6 } else { 8 })?;
                    c.set_reg8(r8::CL, 0xff);
                    c.set_reg(DI, 0x1b48);
                    if dx >= l {
                    }
                    at = 0;
                    continue;
                }
                c.run(0xd5b1, 9)?;
                c.set_reg8(r8::CL, 0xff);
                c.set_reg(DI, 0x1b48);
                c.set_reg(CX, 0);
                loop {
                    let di = c.reg(DI);
                    let (t, b) = (UiElement(di).y0(&mut c), UiElement(di).y1(&mut c));
                    if bx <= t {
                        c.run(0xd5c7, 2)?;
                        at = 3;
                        break;
                    }
                    if bx <= b {
                        c.run(0xd5c7, 4)?;
                        at = 0;
                        break;
                    }
                    let cx = c.reg(CX).wrapping_add(1);
                    let n = c.byte(g::ITEM_COUNT);
                    c.run(0xd5c7, 8)?;
                    let d = di.wrapping_add(0x0e);
                    c.set_reg(DI, d);
                    c.set_reg(CX, cx);
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
    let old = c.byte(g::HOVERED_ITEM);
    if cl == old {
        c.run(0xd5dd, 4 + 7)?;
        c.set_reg8(r8::AL, cl);
        c.set_reg8(r8::CL, old);
        c.cmp(B, cl as u16, old as u16);
        return highlight_pops(&mut c);
    }
    c.run(0xd5dd, 5)?;
    c.set_reg8(r8::AL, cl);
    c.set_byte(g::HOVERED_ITEM, cl);
    c.set_reg8(r8::CL, old);
    c.call(entry::CALL_RESTORE_CURSOR, 0xd5ea)?;
    let n = c.byte(g::ITEM_COUNT);
    let cl = c.reg8(r8::CL);
    c.run(0xd5ea, if cl & 0x80 != 0 { 2 } else if cl >= n { 4 } else { 6 })?;
    if cl & 0x80 == 0 && cl < n {
        let ax = c.reg(AX);
        c.push(ax);
        c.call(entry::ITEM_PLACE, 0xd5f8)?;
        c.run(0xd5f8, 1)?;
        c.call(entry::BUTTON_LABEL, 0xd5fb)?;
        c.run(0xd5fb, 1)?;
        let a = c.pop();
        c.set_reg(AX, a);
    }
    let al = c.reg8(r8::AL);
    let n = c.byte(g::ITEM_COUNT);
    c.run(0xd5fc, if al >= n { 3 } else { 4 })?;
    if al >= n {
        c.cmp(B, al as u16, n as u16);
    } else {
        let ax = c.reg(AX);
        c.set_reg(CX, ax);
        c.call(entry::ITEM_PLACE, 0xd607)?;
        c.run(0xd607, 2)?;
        let ah = (c.reg(AX) >> 8) as u8;
        let h = ah | 0x80;
        c.set_reg8(r8::AH, h as u8);
        c.call(entry::BUTTON_LABEL, 0xd60d)?;
        c.run(0xd60d, 1)?;
    }
    c.call(entry::DRAW_MOUSE, 0xd610)?;
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
pub fn install_ui_list_unhighlighted(m: &mut Machine, start: u16) -> Result<(), Bail> {
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
/// drawn from the list (`loc_0d48a`; [0DCE4h] its count byte, [`g::ITEM_COUNT`]
/// the slots used, [`g::MORE_ITEM`] the one with the more mark A0h) and the
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
            let si = c.word(g::UI_STACK_TOP);
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
                let full = si == g::UI_STACK_LAST;
                c.run(0xd33a, 8 + if full { 0 } else { 2 })?;
                c.set_reg(SI, si);
                c.set_reg(DI, di);
                c.set_reg8(r8::AL, lv);
                c.cmp(B, lv as u16, top as u16);
                c.cmp(W, si, g::UI_STACK_LAST);
                if !full {
                    let s = c.add(W, si, 4);
                    c.set_reg(SI, s);
                    c.set_word(g::UI_STACK_TOP, s);
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
            let t = c.word(g::UI_STACK_TOP);
            let t = c.sub(W, t, 4);
            c.set_word(g::UI_STACK_TOP, t);
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
        let si = c.word(g::UI_STACK_TOP);
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
        c.run(0xd388, if cx < 5 { 6 } else { 2 })?;
        if cx < 5 {
            c.cmp(W, cx, 5);
            let b = c.shl1(W, cx, 2);
            c.set_reg(CX, b);
            c.set_reg(BX, b);
            let a = b.wrapping_add(si).wrapping_add(3);
            let v = c.byte(a);
            let v = c.or(B, v as u16, 0x80);
            c.set_byte(a, v as u8);
        } else {
            c.cmp(W, cx, 5);
        }
        if c.flag(pc::cpu::DF) {
            c.m.cpu.ip = 0xd397;
            return Err(Bail);
        }
    }
    // D397h: the slots.
    c.run(0xd397, 11)?;
    c.set_byte(g::HOVERED_ITEM, 0xff);
    let t = c.word(g::UI_STACK_TOP);
    let l = c.word(t);
    let si = c.inc(W, l);
    c.set_reg(SI, si);
    let n = c.lods(B, DS) as u8;
    c.set_byte(g::ITEM_SCROLL, n);
    let si = c.reg(SI).wrapping_add(n as i8 as i16 as u16);
    let ax = n as i8 as i16 as u16;
    let si = c.add(W, si.wrapping_sub(ax), ax);
    c.set_reg(SI, si);
    c.xor(W, 0, 0);
    c.set_reg(CX, 0);
    c.set_byte(g::ITEM_COUNT, 0);
    c.set_byte(g::MORE_ITEM, 0xff);
    let mut cl = 0u8;
    loop {
        let si = c.reg(SI);
        let ax = c.word(si);
        if ax == 0 {
            break;
        }
        let cnt = c.byte(g::ITEM_SCROLL);
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
            c.set_byte(g::ITEM_SCROLL, v as u8);
            c.set_reg(AX, 0xa0);
            c.set_byte(g::MORE_ITEM, cl);
        }
        let s = c.add(W, si, 4);
        c.set_reg(SI, s);
        let cx = c.reg(CX);
        c.push(cx);
        c.push(s);
        let u = c.byte(g::ITEM_COUNT);
        let u = c.inc(B, u as u16);
        c.set_byte(g::ITEM_COUNT, u as u8);
        c.call(entry::BUTTON_LABEL, 0xd3e5)?;
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
    let cnt = c.byte(g::ITEM_SCROLL);
    let n = 3 + 2 + if cnt != 0 { 4 } else { 1 } + 2;
    c.run(0xd3b5, n)?;
    c.set_reg(AX, 0);
    c.or(W, 0, 0);
    c.cmp(B, cnt as u16, 0);
    if cnt != 0 {
        c.set_reg(AX, 0xa0);
        c.set_byte(g::MORE_ITEM, cl);
        let u = c.byte(g::ITEM_COUNT);
        let u = c.inc(B, u as u16);
        c.set_byte(g::ITEM_COUNT, u as u8);
    } else {
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
    }
    loop {
        let cx = c.reg(CX);
        c.push(cx);
        c.call(entry::BUTTON_LABEL, 0xd409)?;
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
    let (x, y) = (c.mouse_state().x, c.mouse_state().y);
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    highlight_hovered_text_action_item(c.m, 0xd50f)
}

/// `loc_0dfb7`, the rest of `get_mouse_pos_etc` once the keys (or the
/// joystick) give a direction, DX and BX its sums (not both 0):
///
/// - with [`g::KEY_CTRL`] FFh, the cursor glides: each axis' direction
///   doubled up to three times by the speed it has built up ([0DC51h],
///   [0DC53h], the pixels moved so far), times the ticks since the last
///   step (at most 8, [0DC50h]) in eighths (`accelerated_step`, the
///   fractions in [0DC55h], [0DC56h]), added to the speed, and the cursor
///   moved by that (on into `move_cursor_by`);
/// - else it jumps to the nearest hotspot in that direction: a frame of
///   five words below SP (see the module's notes), the scoring routine
///   from the table at CS:DFA7h by the signs of DL and BL, the key
///   forgotten (`kb_clear_scancode`), then every UI element shown (bit 7
///   of its flags) not under the cursor, the extra points, the particles,
///   the markers and the eight points scored; the best one, if any, noted
///   at [0DC4Ch], [0DC4Eh] with [0DC4Bh] 64h and the time [0DC4Ah] (the
///   cursor moves there over the next frames).
///
/// With neither DL nor BL (only their high bytes set) it returns.
pub(super) fn keyboard_move(c: &mut Ctx) -> Result<(), Bail> {
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    if c.byte(g::KEY_CTRL) == 0xff {
        return glide(c, dx, bx);
    }
    let (dl, bl) = (dx as u8, bx as u8);
    // cmp, jnz; mov di, or dl, jz
    let mut n = 5;
    let di = if dl != 0 {
        let mut d = SCORERS + 2;
        n += 1; // jns
        if dl & 0x80 != 0 {
            d += 6;
            n += 1;
        }
        n += 2; // or bl, jz
        if bl != 0 {
            d -= 2;
            n += 2; // lea, js
            if bl & 0x80 == 0 {
                d += 4;
                n += 2; // add, jmp
            }
        }
        d
    } else {
        n += 3; // mov di, or bl, jz
        if bl == 0 {
            c.run(0xdfb7, n + 1)?; // the ret at DFA6
            c.set_reg(DI, SCORERS + 0x0c);
            return c.ret();
        }
        let mut d = SCORERS + 0x0c;
        n += 1; // js
        if bl & 0x80 == 0 {
            d += 2;
            n += 1;
        }
        d
    };
    let scorer = c.code_word(di);
    n += 5; // mov bx, cs:[di]; sub sp; mov bp, sp; mov [bp], bx; call
    c.run(0xdfb7, n)?;
    c.set_reg(DI, di);
    c.set_reg(BX, scorer);
    let sp = c.reg(SP).wrapping_sub(0x0a);
    c.set_reg(SP, sp);
    c.set_reg(BP, sp);
    set_frame(c, 0, scorer);
    c.call(code::KB_CLEAR_SCANCODE, 0xdff5)?;

    // The UI elements: [1AE4h] of them, 0Eh bytes each from 1AE6h.
    let count = c.word(UI_ELEMENTS);
    if count == 0 {
        // `loop` would go round 65536 times: left to the original.
        c.m.cpu.ip = 0xdff5;
        return Err(Bail);
    }
    let mouse = c.mouse_state();
    c.run(0xdff5, 6)?;
    c.set_reg(DX, mouse.x);
    c.set_reg(BX, mouse.y);
    c.set_reg(DI, UI_ELEMENTS);
    c.set_reg(CX, count);
    set_frame(c, 2, 0x8000);
    let mut di = UI_ELEMENTS + 2;
    c.set_reg(DI, di);
    loop {
        let shown = UiElement(di).flags(c) & ui_flag::LIVE != 0;
        c.run(0xe00a, if shown { 3 } else { 2 })?;
        if shown {
            let under = inside_rect(c, 0xe013)?;
            c.run(0xe013, if under { 1 } else { 2 })?;
            if !under {
                c.call(code::RECT_HOTSPOT, 0xe018)?;
                c.run(0xe018, 1)?;
                if score(c, 0xe018, 2)? {
                    let (ax, si) = rect_hotspot_point(c, 0xe026)?;
                    c.run(0xe026, 2)?;
                    set_frame(c, 4, ax);
                    set_frame(c, 6, si);
                }
            }
        }
        let cx = c.reg(CX).wrapping_sub(1);
        c.run(0xe02c, 2)?;
        di = pure::add(W, c.reg(DI), 0x0e);
        c.set_reg(DI, di);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.run(0xe031, 1)?;
    c.call(code::HOTSPOTS_POINTS, 0xe034)?;
    c.run(0xe034, 1)?;
    c.call(code::HOTSPOTS_PARTICLES, 0xe037)?;
    c.run(0xe037, 1)?;
    c.call(code::HOTSPOTS_MARKERS, 0xe03a)?;
    c.run(0xe03a, 1)?;
    c.call(code::HOTSPOTS_EIGHT_POINTS, 0xe03d)?;
    let found = frame(c, 2) & 0x8000 == 0;
    if found {
        let (x, y) = (frame(c, 4), frame(c, 6));
        let t = c.timer();
        c.run(0xe03d, 2 + 9 + 2)?;
        c.set_byte(g::KEY_SPACE, 0);
        c.set_word(g::GLIDE_TARGET, x);
        c.set_word(g::GLIDE_TARGET + 2, y);
        c.set_byte(g::GLIDE_STEPS, 0x64);
        c.set_reg(AX, t);
        c.set_byte(g::GLIDE_TIME, t as u8);
    } else {
        c.run(0xe03d, 2 + 2)?;
    }
    let sp = c.reg(SP).wrapping_add(0x0a);
    c.set_reg(SP, sp);
    c.ret()
}

/// `loc_0e1d1` (see [`keyboard_move`]): the glide's step, on into
/// `move_cursor_by` with DX, BX the pixels.
fn glide(c: &mut Ctx, dx: u16, bx: u16) -> Result<(), Bail> {
    let (mut dl, mut bl) = (dx as u8, bx as u8);
    // cmp, jnz, jmp; or dl, jz
    let mut n = 5;
    if dl != 0 {
        let v = c.word(g::GLIDE_SPEED);
        let neg = v & 0x8000 != 0;
        let a = if neg { v.wrapping_neg() } else { v };
        n += 3 + neg as u32 + 2; // mov, or, jns, [neg], cmp, jb
        for (k, limit) in [4u16, 0x0c, 0x24].into_iter().enumerate() {
            if a < limit {
                break;
            }
            dl = dl.wrapping_add(dl);
            n += if k < 2 { 3 } else { 1 }; // add, [cmp, jb]
        }
    }
    n += 2; // or bl, jz
    if bl != 0 {
        let v = c.word(g::GLIDE_SPEED + 2);
        // `jns` on the flags of `or bl, bl` (the `mov` keeps them).
        let neg = bl & 0x80 != 0;
        let a = if neg { v.wrapping_neg() } else { v };
        n += 2 + neg as u32 + 2; // mov, jns, [neg], cmp, jb
        for (k, limit) in [3u16, 0x0a, 0x1c].into_iter().enumerate() {
            if a < limit {
                break;
            }
            bl = bl.wrapping_add(bl);
            n += if k < 2 { 3 } else { 1 };
        }
    }
    // The ticks since the last step, at most 8.
    let t = c.timer() as u8;
    let last = c.byte(GLIDE_LAST_TICK);
    let d = t.wrapping_sub(last);
    let capped = d >= 8;
    let cl = if capped { 8 } else { d };
    // mov al, mov ah, xchg, sub, cmp, jb, [mov al, 8]; mov cl, mov si,
    // mov al, call
    n += 6 + capped as u32 + 4;
    c.run(0xdfb7, n)?;
    c.set_reg(DX, dx & 0xff00 | dl as u16);
    c.set_reg(BX, bx & 0xff00 | bl as u16);
    c.set_byte(GLIDE_LAST_TICK, t);
    c.set_reg(AX, (last as u16) << 8 | dl as u16);
    c.set_reg8(r8::CL, cl);
    c.set_reg(SI, g::GLIDE_FRACTIONS);
    let pixels = accelerated_pixels(c, 0xe22e)?;
    let speed = c.word(g::GLIDE_SPEED);
    c.run(0xe22e, 5)?;
    c.set_reg(DX, pixels);
    c.set_word(g::GLIDE_SPEED, speed.wrapping_add(pixels));
    c.set_reg(SI, g::GLIDE_FRACTIONS + 1);
    c.set_reg8(r8::AL, bl);
    let pixels = accelerated_pixels(c, 0xe23a)?;
    let speed = c.word(g::GLIDE_SPEED + 2);
    c.run(0xe23a, 3)?;
    c.set_reg(BX, pixels);
    c.set_word(g::GLIDE_SPEED + 2, speed.wrapping_add(pixels));
    c.jump(code::MOVE_CURSOR_BY)
}

/// The table of the scoring routines by direction (CS:DFA7h, 8 words).
const SCORERS: u16 = 0xdfa7;
/// The current UI elements: a count, then 0Eh-byte records.
const UI_ELEMENTS: u16 = 0x1ae4;
const GLIDE_LAST_TICK: u16 = 0xdc50;
