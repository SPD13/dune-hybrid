//! The game screen's UI: the talking head's animation, the
//! communication device, the sky, the sun and moon, location names, and
//! smaller game-logic steps they trigger.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DS, DX, ES, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const SET_SCREEN_AS_ACTIVE_FRAMEBUFFER: u16 = 0xc08e;
    pub const OPEN_RESOURCE_BY_INDEX: u16 = 0xc13e;
    pub const DRAW_ICONS_LIST_AT_SI: u16 = 0xc21b;
    pub const DRAW_SPRITE_CLOBBERING_BX_DX: u16 = 0xc22f;
    pub const DRAW_SPRITE: u16 = 0xc2fd;
    pub const GET_SUBRESOURCE_POINTER: u16 = 0xc1f4;
    pub const HEAD_STEP: u16 = 0x17be;
    pub const WAIT_A_BIT: u16 = 0xe387;
    pub const WAIT_INTERRUPTIBLE: u16 = 0xe353;
    pub const WAIT_PROCESSING_FRAME_TASKS: u16 = 0xe3a0;
    pub const COMM_DRAW_FLICKER: u16 = 0x27c9;
    pub const COMM_DRAW_GLOW_INDEX: u16 = 0x27b6;
    pub const CALC_SAL_INDEX: u16 = 0x5e4f;
    pub const LOAD_RESOURCE_TO: u16 = 0xf0b9;
    pub const ADJUST_SUB_RESOURCE_POINTERS: u16 = 0x0098;
    pub const SKY_PALETTE_FOR_GAME_TIME: u16 = 0x395c;
    pub const OPEN_SKY_OR_SKYDN_PALETTE: u16 = 0x3971;
    pub const SUB_398C: u16 = 0x398c;
    pub const SET_SKY_PALETTE: u16 = 0x388d;
    pub const SUB_3E80: u16 = 0x3e80;
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
    pub const SOUND_OFF: u16 = 0xaec6;
    pub const SUB_AD43: u16 = 0xad43;
    pub const GET_LOCATION_TYPE_STRING_OFFSET: u16 = 0x6231;
    pub const SUB_7085: u16 = 0x7085;
}

/// `adjust_sub_resource_pointers` (0098): turns the offset table at the
/// start of the resource at ES:DI (its first word: the table's size) into
/// pointers, by adding DI. Exit: DI past the table, CX = 0, AX the last;
/// BX = the resource; flags from the last `add`.
pub fn adjust_sub_resource_pointers(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0098)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let di = c.reg(DI);
    let size = c.word_in(ES, di);
    let n = c.shr1(W, size, 1);
    c.set_reg(CX, n);
    c.set_reg(BX, di);
    loop {
        let left = c.reg(CX).wrapping_sub(1);
        c.run(0x009f, if left == 0 { 5 } else { 4 })?;
        let at = c.reg(DI);
        let offset = c.word_in(ES, at);
        let base = c.reg(BX);
        let p = c.add(W, offset, base);
        c.set_reg(AX, p);
        c.stos(W);
        c.set_reg(CX, left);
        if left == 0 {
            return c.ret();
        }
    }
}

/// `intro_script_load_word` (093F): AX = the intro script's next word (in
/// the code segment), the script moved past it.
pub fn intro_script_load_word(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x093f)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let at = c.word(g::SCENE_SEQUENCE_OFFSET);
    c.set_reg(SI, at);
    c.lods(W, pc::cpu::CS);
    let si = c.reg(SI);
    c.set_word(g::SCENE_SEQUENCE_OFFSET, si);
    c.ret()
}

/// `ui_head_animate_up` (17E6): steps the talking head's animation up to
/// 0Ah, one step every 8 ticks, unless [11C9h] is set. Flags from the last
/// `cmp`.
pub fn ui_head_animate_up(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x17e6)?;
    let mut c = Ctx::new(m);
    loop {
        let blocked = c.byte(g::HEAD_ANIMATION_BLOCKED);
        if blocked != 0 {
            c.run(0x17e6, 3)?;
            c.cmp(B, blocked as u16, 0);
            return c.ret();
        }
        let step = c.byte(g::HEAD_ANIMATION_STEP);
        if step == 0x0a {
            c.run(0x17e6, 5)?;
            c.cmp(B, 0, 0);
            c.cmp(B, step as u16, 0x0a);
            return c.ret();
        }
        c.run(0x17e6, 6)?;
        c.cmp(B, 0, 0);
        c.cmp(B, step as u16, 0x0a);
        let step = c.inc(B, step as u16) as u8;
        c.set_byte(g::HEAD_ANIMATION_STEP, step);
        c.call(code::HEAD_STEP, 0x17fb)?;
        c.run(0x17fb, 2)?;
        c.set_reg(AX, 8);
        c.call(code::WAIT_A_BIT, 0x1801)?;
        c.run(0x1801, 1)?;
    }
}

/// `sub_d95b`: the map's click handler to 2572h. AX = it.
pub fn sub_d95b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd95b)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    set_click_handler(&mut c)
}

fn set_click_handler(c: &mut Ctx) -> Result<(), Bail> {
    c.set_reg(AX, 0x2572);
    c.set_word(g::MAP_CLICK_HANDLER, 0x2572);
    c.ret()
}

/// `sub_19fc`: no mouse rectangle; the rectangle at 143Ch from the second
/// framebuffer; `sub_c0d5` with 12h; the click handler to 2572h.
pub fn sub_19fc(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x19fc)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xdaa3, 0x19ff)?;
    c.run(0x19ff, 3)?;
    c.set_reg(SI, 0x143c);
    c.push(0x143c);
    c.call(0xc446, 0x1a06)?;
    c.run(0x1a06, 3)?;
    let si = c.pop();
    c.set_reg(SI, si);
    c.set_reg8(r8::AL, 0x12);
    c.call(0xc0d5, 0x1a0c)?;
    c.run(0x1a0c, 4)?; // jmp D95Bh; mov ax; mov; ret
    set_click_handler(&mut c)
}

/// `draw_moon_and_sun` (1A9B): sprite AX of the loaded sheet at the
/// position at SI (two words; none if x is 0) on the screen buffer,
/// clipped to the rectangle at 1EFEh (driver function 6). Exit: SI past
/// the position; AX kept; flags from `or dx, dx` when nothing is drawn.
pub fn draw_moon_and_sun(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1a9b)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let x = c.word_in(DS, si);
    c.run(start, if x == 0 { 9 } else { 10 })?;
    let ax = c.reg(AX);
    c.push(ax);
    let x = c.lods(W, DS);
    c.set_reg(DX, x);
    let y = c.lods(W, DS);
    c.set_reg(BX, y);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.or(W, x, x);
    if x == 0 {
        return c.ret();
    }
    let si = c.reg(SI);
    c.push(si);
    c.call(code::GET_SUBRESOURCE_POINTER, 0x1aab)?;
    c.run(0x1aab, 10)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.sreg(ES);
    c.push(es);
    let screen = c.word(g::SCREEN_BUFFER);
    c.set_sreg(ES, screen);
    let sheet = c.pop();
    c.set_sreg(DS, sheet);
    let size = c.lods(W, DS);
    c.set_reg(DI, size);
    let lines = c.lods(W, DS);
    c.set_reg(CX, lines);
    let ch = c.reg8(r8::CH);
    c.xor(B, ch as u16, ch as u16);
    c.set_reg8(r8::CH, 0);
    c.set_reg(BP, g::SKY_CLIP_RECT);
    c.interp(0x1abd, 0x1ac2)?;
    c.run(0x1ac2, 3)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `sub_1ae7`: after `sub_d41b`, when BP is 1F7Eh: once per timer tick,
/// and once [4772h] ticks have passed since [476Eh], the talking head's
/// next zoom (`sub_c85b`, `sub_c868`).
pub fn sub_1ae7(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1ae7)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xd41b, 0x1aea)?;
    let bp = c.reg(BP);
    if bp != 0x1f7e {
        c.run(0x1aea, 3)?;
        c.cmp(W, bp, 0x1f7e);
        return c.ret();
    }
    let now = c.word(g::TIMER);
    let seen = c.word(g::HEAD_ZOOM_SEEN);
    if now == seen {
        c.run(0x1aea, 6)?;
        c.cmp(W, bp, 0x1f7e);
        c.set_reg(AX, now);
        c.cmp(W, now, seen);
        return c.ret();
    }
    let since = c.word(g::HEAD_ZOOM_START);
    let every = c.word(g::TIME_BETWEEN_FACE_ZOOMS);
    let due = now.wrapping_sub(since) >= every;
    c.run(0x1aea, 10)?; // the same count either way: ret, or call
    c.cmp(W, bp, 0x1f7e);
    c.cmp(W, now, seen);
    c.set_word(g::HEAD_ZOOM_SEEN, now);
    let passed = c.sub(W, now, since);
    c.set_reg(AX, passed);
    c.cmp(W, passed, every);
    if !due {
        return c.ret();
    }
    c.call(0xc85b, 0x1b09)?;
    c.run(0x1b09, 1)?;
    c.call(0xc868, 0x1b0c)?;
    c.run(0x1b0c, 1)?;
    c.ret()
}

/// `sub_1dda`: `sub_29ee` with 30Bh when bit 10h of [BFh] is set, bit 8 of
/// [10h] clear, [0Bh] not 8 and [C2h] 0. Flags from the check that ended
/// it.
pub fn sub_1dda(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1dda)?;
    let mut c = Ctx::new(m);
    let (a, b, d, e) = (c.byte(0x00bf), c.word(0x0010), c.byte(0x000b), c.byte(0x00c2));
    let checks = [a & 0x10 == 0, b & 8 != 0, d == 8, e != 0];
    let failed = checks.iter().position(|&f| f);
    c.run(start, match failed {
        Some(i) => 2 * i as u32 + 3,
        None => 10,
    })?;
    for i in 0..=failed.unwrap_or(3) {
        match i {
            0 => c.test(B, a as u16, 0x10),
            1 => c.test(W, b, 8),
            2 => c.cmp(B, d as u16, 8),
            _ => c.cmp(B, e as u16, 0),
        }
    }
    if failed.is_some() {
        return c.ret();
    }
    c.set_reg(AX, 0x030b);
    c.call(0x29ee, 0x1dfd)?;
    c.run(0x1dfd, 1)?;
    c.ret()
}

/// `sub_1dfe` (jumps to 1D10): every 2nd call (bit 15 of [0] rotated
/// out), troops with bit 80h of +10h grow their +1Ah by 1 (up to C8h).
/// Flags from the last check.
pub fn sub_1dfe(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1dfe)?;
    let mut c = Ctx::new(m);
    let bits = c.word(0x0000);
    let due = bits & 0x8000 != 0;
    c.run(start, 4)?; // jmp; rol; jae; then mov si, or ret
    let bits = c.m.cpu.shift(0, W, bits as u32, 1) as u16; // rol
    c.set_word(0x0000, bits);
    if !due {
        return c.ret();
    }
    let mut t = g::TROOPS;
    c.set_reg(SI, t);
    loop {
        let flags = c.byte(t + 0x10);
        let grows = flags & 0x80 != 0;
        let v = c.byte(t + 0x1a);
        let room = v.wrapping_sub(1) < 0xc7;
        c.run(0x1d19, 2 + if grows { 4 + room as u32 } else { 0 } + 3)?;
        c.test(B, flags as u16, 0x80);
        if grows {
            c.set_reg8(r8::AL, v);
            let al = c.dec(B, v as u16);
            c.set_reg8(r8::AL, al as u8);
            c.cmp(B, al, 0xc7);
            if room {
                let v = c.inc(B, v as u16) as u8;
                c.set_byte(t + 0x1a, v);
            }
        }
        t = c.add(W, t, 0x1b);
        c.set_reg(SI, t);
        c.cmp(W, t, 0x0fa0);
        if t >= 0x0fa0 {
            break;
        }
    }
    c.run(0x1d34, 1)?;
    c.ret()
}

/// From 275F: draws the communication device's glow AL (sprite 0Bh + AL
/// & 7, at (64h, 56h)) to the screen, then back to the front buffer.
fn glow(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x275f, 1)?;
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0x2762)?;
    c.run(0x2762, 5)?;
    let ax = c.reg(AX);
    let ax = c.and(W, ax, 7);
    let sprite = c.add(B, ax, 0x0b);
    c.set_reg(AX, sprite);
    c.set_reg(BX, 0x56);
    c.set_reg(DX, 0x64);
    c.call(code::DRAW_SPRITE_CLOBBERING_BX_DX, 0x2770)?;
    c.run(0x2770, 1)?; // jmp C07Ch
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `comm_draw_glow` (275F).
pub fn comm_draw_glow(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x275f)?;
    glow(&mut Ctx::new(m))
}

/// `comm_draw_panels` (274E): the communication device's panels (icons
/// of resource 15h listed at 14C8h), then glow 1.
pub fn comm_draw_panels(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x274e)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0x2751)?;
    c.run(0x2751, 2)?;
    c.set_reg(AX, 0x15);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0x2757)?;
    c.run(0x2757, 2)?;
    c.set_reg(SI, 0x14c8);
    c.call(code::DRAW_ICONS_LIST_AT_SI, 0x275d)?;
    c.run(0x275d, 1)?;
    c.set_reg8(r8::AL, 1);
    glow(&mut c)
}

/// `comm_fade_in_glow` (2795): glow steps 0 to 0Ch ([D816h]), each for 9
/// ticks (interruptible wait, drawing step `comm_draw_glow_index`).
#[allow(dead_code)]
pub fn comm_fade_in_glow(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2795)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(AX, 0x15);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0x279b)?;
    c.run(0x279b, 4)?;
    c.set_word(g::COMM_STEP, 0);
    loop {
        c.set_reg(BP, code::COMM_DRAW_GLOW_INDEX);
        c.set_reg(AX, 9);
        c.call(code::WAIT_INTERRUPTIBLE, 0x27aa)?;
        let step = c.word(g::COMM_STEP).wrapping_add(1);
        let more = step < 0x0d;
        c.run(0x27aa, if more { 3 + 3 } else { 4 })?;
        let step = c.inc(W, step.wrapping_sub(1));
        c.set_word(g::COMM_STEP, step);
        c.cmp(W, step, 0x0d);
        if !more {
            return c.ret();
        }
    }
}

/// `comm_animate_incoming_call` (281C): CX flicker steps of the
/// communication device ([D816h] counting), 9 ticks each.
#[allow(dead_code)]
pub fn comm_animate_incoming_call(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x281c)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(AX, 0x15);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0x2822)?;
    c.run(0x2822, 3)?;
    c.set_word(g::COMM_STEP, 0);
    loop {
        let cx = c.reg(CX);
        c.push(cx);
        c.call(code::COMM_DRAW_FLICKER, 0x282c)?;
        c.run(0x282c, 2)?;
        c.set_reg(AX, 9);
        c.call(code::WAIT_PROCESSING_FRAME_TASKS, 0x2832)?;
        let sp = c.reg(SP);
        let left = c.word_in(SS, sp).wrapping_sub(1);
        c.run(0x2832, if left == 0 { 4 } else { 3 + 2 })?;
        let step = c.word(g::COMM_STEP);
        let step = c.inc(W, step);
        c.set_word(g::COMM_STEP, step);
        c.pop();
        c.set_reg(CX, left);
        if left == 0 {
            return c.ret();
        }
    }
}

/// `open_SAL_resource` (2D74): loads the SAL (the place's scene layout)
/// for the location at [114Eh], unless it is loaded: by its kind
/// (`calc_SAL_index`), kinds 0 and 1 only with bit 1 of [4732h] clear,
/// 4 as 3, none above 4.
pub fn open_sal_resource(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2d74)?;
    let mut c = Ctx::new(m);
    let si = c.word(g::MAP_POSITION_RECORD);
    if si < 0x100 {
        c.run(start, 4)?;
        c.set_reg(SI, si);
        c.cmp(W, si, 0x100);
        return c.ret();
    }
    c.run(start, 5)?;
    c.set_reg(SI, si);
    c.cmp(W, si, 0x100);
    let ax = c.reg(AX);
    let zero = c.xor(W, ax, ax);
    c.set_reg(AX, zero);
    c.call(code::CALC_SAL_INDEX, 0x2d83)?;
    c.check(0x2d83, 20)?;
    let mut n = 2;
    let mut sal = c.reg(AX);
    c.cmp(W, sal, 2);
    if sal < 2 {
        let bits = c.byte(g::SAL_FLAGS);
        c.test(B, bits as u16, 1);
        n += 2;
        if bits & 1 != 0 {
            c.count(n + 1);
            return c.ret();
        }
    }
    c.cmp(W, sal, 4);
    n += 2;
    if sal > 4 {
        c.count(n + 1);
        return c.ret();
    }
    n += 1; // jne
    if sal == 4 {
        sal = c.dec(W, sal);
        c.set_reg(AX, sal);
        n += 1;
    }
    let loaded = c.byte(g::LOADED_SAL_INDEX);
    c.cmp(B, loaded as u16, sal & 0xff);
    n += 2;
    if loaded as u16 == sal & 0xff {
        c.count(n + 1);
        return c.ret();
    }
    c.count(n + 7);
    c.set_byte(g::LOADED_SAL_INDEX, sal as u8);
    let resource = c.add(W, sal, 0xa1);
    c.set_reg(AX, resource);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(DI, g::SAL_BUFFER);
    c.set_reg(SI, resource);
    c.call(code::LOAD_RESOURCE_TO, 0x2dad)?;
    c.run(0x2dad, 1)?;
    c.call(code::ADJUST_SUB_RESOURCE_POINTERS, 0x2db0)?;
    c.run(0x2db0, 1)?;
    c.ret()
}

/// From 39B9: the sky's palette rows (driver function 37): 1C5h colours
/// from row DBh, or for the night sky F0h from 180h; then, outside the
/// palace's map view, 30h from 2D0h. DX moves on by the first count.
fn sky_palette_rows(c: &mut Ctx) -> Result<(), Bail> {
    sky_rows_at(c, 0x39b9)
}

/// [`sky_palette_rows`] at `base` (39B9h, or 398Ch for its twin through
/// the driver's function at 38BDh).
fn sky_rows_at(c: &mut Ctx, base: u16) -> Result<(), Bail> {
    let at = |off: u16| base + (off - 0x39b9);
    let variant = c.byte(g::SKY_VARIANT);
    let night = variant == 0;
    c.run(base, if night { 7 } else { 5 })?;
    let (mut cx, mut bx) = (0x1c5, 0xdb);
    c.set_reg(CX, cx);
    c.set_reg(BX, bx);
    c.cmp(B, variant as u16, 0);
    if night {
        (cx, bx) = (0xf0, 0x180);
        c.set_reg(CX, cx);
        c.set_reg(BX, bx);
    }
    c.push(cx);
    c.interp(at(0x39cd), at(0x39d1))?;
    let flag = c.byte(g::MAP_VIEW_FLAG);
    c.run(at(0x39d1), if flag != 0 { 5 } else { 6 })?;
    let cx = c.pop();
    c.set_reg(CX, cx);
    let dx = c.reg(DX);
    let dx = c.add(W, dx, cx);
    c.set_reg(DX, dx);
    c.cmp(B, flag as u16, 0);
    if flag != 0 {
        return c.ret();
    }
    c.set_reg(CX, 0x30);
    c.set_reg(BX, 0x2d0);
    c.interp(at(0x39e1), at(0x39e5))?;
    c.run(at(0x39e5), 1)?;
    c.ret()
}

/// `loc_0398c`: [`sky_palette_rows`]' twin (the driver's function at
/// 38BDh).
pub fn sky_palette_rows_38bd(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x398c)?;
    sky_rows_at(&mut Ctx::new(m), 0x398c)
}

/// `loc_038a2`: frame tasks to 30h, the sky palette (`open_sky_or_skydn_
/// palette_al_sub_bl`) and its rows.
pub fn sky_palette_with_tasks(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x38a2)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(0x46d7, 0x30);
    c.call(0x3971, 0x38aa)?;
    c.run(0x38aa, 1)?;
    sky_palette_rows(&mut c)
}

/// `loc_039b9`.
pub fn sub_39b9(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x39b9)?;
    sky_palette_rows(&mut Ctx::new(m))
}

/// `set_sky_palette` (388D): the sky's palette for the game time. With
/// frame tasks running, only when it changed (then frame tasks to 30h and
/// the rows set); without, the palette and `sub_398c`.
pub fn set_sky_palette(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x388d)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(g::SKY_PALETTE_SET, 1);
    c.call(code::SKY_PALETTE_FOR_GAME_TIME, 0x3895)?;
    let tasks = c.byte(g::FRAME_TASKS_ACTIVE);
    if tasks == 0 {
        c.run(0x3895, 3)?;
        c.cmp(B, 0, 0);
        c.call(code::OPEN_SKY_OR_SKYDN_PALETTE, 0x38b0)?;
        c.run(0x38b0, 1)?;
        c.call(code::SUB_398C, 0x38b3)?;
        c.run(0x38b3, 1)?;
        return c.ret();
    }
    let current = c.byte(g::PALETTE_SUB);
    let id = c.reg8(r8::BL);
    if current == id {
        c.run(0x3895, 5)?;
        c.cmp(B, tasks as u16, 0);
        c.cmp(B, current as u16, id as u16);
        return c.ret();
    }
    c.run(0x3895, 6)?;
    c.cmp(B, tasks as u16, 0);
    c.cmp(B, current as u16, id as u16);
    c.set_byte(g::FRAME_TASKS_ACTIVE, 0x30);
    c.call(code::OPEN_SKY_OR_SKYDN_PALETTE, 0x38aa)?;
    c.run(0x38aa, 1)?; // jmp 39B9h
    sky_palette_rows(&mut c)
}

/// `draw_sky` (38B4): the sky's palette, then its 4 rows of 8 tiles
/// (sprites 0..3 of resource 28h, 28h wide, rows 14h apart).
pub fn draw_sky(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x38b4)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SET_SKY_PALETTE, 0x38b7)?;
    c.run(0x38b7, 2)?;
    c.set_reg(AX, 0x28);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0x38bd)?;
    c.run(0x38bd, 4)?;
    for (r, v) in [(AX, 0), (BP, 0x14), (BX, 0)] {
        if v == 0 {
            let x = c.reg(r);
            c.xor(W, x, x);
        }
        c.set_reg(r, v);
    }
    c.set_reg(CX, 4);
    c.run(0x38c7, 5)?;
    loop {
        // A row: CX and BP kept on the stack.
        let dx = c.reg(DX);
        let zero = c.xor(W, dx, dx);
        c.set_reg(DX, zero);
        let (cx, bp) = (c.reg(CX), c.reg(BP));
        c.push(cx);
        c.push(bp);
        loop {
            let ax = c.reg(AX);
            c.push(ax);
            c.call(code::DRAW_SPRITE, 0x38cf)?;
            let x = c.reg(DX).wrapping_add(0x28);
            let more = x < 0x140;
            let sp = c.reg(SP);
            let left = c.word_in(SS, sp.wrapping_add(4)).wrapping_sub(1);
            c.run(0x38cf, if more { 4 + 2 } else { 4 + 5 + if left == 0 { 1 } else { 5 } })?;
            let ax = c.pop();
            c.set_reg(AX, ax);
            let dx = c.reg(DX);
            let dx = c.add(W, dx, 0x28);
            c.set_reg(DX, dx);
            c.cmp(W, dx, 0x140);
            if !more {
                break;
            }
        }
        let bp = c.pop();
        c.set_reg(BP, bp);
        let cx = c.pop();
        let ax = c.reg(AX);
        let ax = c.inc(W, ax);
        c.set_reg(AX, ax);
        let bx = c.reg(BX);
        let bx = c.add(W, bx, bp);
        c.set_reg(BX, bx);
        let left = cx.wrapping_sub(1);
        c.set_reg(CX, left);
        if left == 0 {
            return c.ret();
        }
    }
}

/// `draw_gradient_section` (3E13): `sub_3e80` for the section from (DX,
/// BX) to (DI, CX), with the start noted at [47E8h] and the extent
/// (DI - DX, CX - BX) in DX, BX. Registers kept.
pub fn draw_gradient_section(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3e13)?;
    let mut c = Ctx::new(m);
    c.run(start, 11)?;
    let (bx, cx, dx, di) = (c.reg(BX), c.reg(CX), c.reg(DX), c.reg(DI));
    for v in [bx, cx, dx, di] {
        c.push(v);
    }
    c.set_word(g::GRADIENT_X, dx);
    c.set_word(g::GRADIENT_Y, bx);
    let b = c.sub(W, bx, cx);
    let d = c.sub(W, dx, di);
    let b = c.neg(W, b);
    let d = c.neg(W, d);
    c.set_reg(BX, b);
    c.set_reg(DX, d);
    c.call(code::SUB_3E80, 0x3e2a)?;
    c.run(0x3e2a, 5)?;
    for r in [DI, DX, CX, BX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// `sub_4ad7`: `sub_4e8d`; if it returned bit 0 of AL, [11C8h] = 1 and
/// `sub_5119` with AL = 4.
pub fn sub_4ad7(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4ad7)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x4e8d, 0x4ada)?;
    let al = c.reg8(r8::AL);
    let set = al & 1 != 0;
    c.run(0x4ada, if set { 6 } else { 4 })?;
    c.set_reg8(r8::AH, 4);
    let al = c.shr1(B, al as u16, 1) as u8;
    c.set_reg8(r8::AL, al);
    if !set {
        return c.ret();
    }
    c.set_byte(g::BYTE_11C8, 1);
    c.set_reg8(r8::AL, 4);
    c.call(0x5119, 0x4aea)?;
    c.run(0x4aea, 1)?;
    c.ret()
}

/// From AE04: with sound on, music not off in the settings, and the
/// driver ready (sign of [DBCDh] clear, or bit 40h set), `sub_ad43`
/// (registers kept).
fn music_step(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xae04, 1)?;
    c.call(code::SOUND_OFF, 0xae07)?;
    if c.flag(CF) {
        c.run(0xae07, 2)?;
        return c.ret();
    }
    let settings = c.byte(g::MUSIC_SETTINGS);
    if settings & 1 != 0 {
        c.run(0xae07, 4)?;
        c.test(B, settings as u16, 1);
        return c.ret();
    }
    let state = c.byte(g::IS_SOUND_PRESENT);
    let ready = state & 0x80 == 0 || state & 0x40 != 0;
    c.run(0xae07, if state & 0x80 == 0 { 6 } else { 8 })?;
    c.test(B, settings as u16, 1);
    c.cmp(B, state as u16, 0);
    if state & 0x80 != 0 {
        c.test(B, state as u16, 0x40);
    }
    if !ready {
        return c.ret();
    }
    c.call(code::SAVE_REGS, 0xae21)?;
    c.run(0xae21, 1)?;
    c.call(code::SUB_AD43, 0xae24)?;
    c.run(0xae24, 1)?;
    c.call(code::RESTORE_REGS, 0xae27)?;
    c.run(0xae27, 1)?;
    c.ret()
}

/// `loc_0ae04`.
pub fn sub_ae04(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xae04)?;
    music_step(&mut Ctx::new(m))
}

/// `ui_transition_to_map_interface` (5A3D): the map's resources, the
/// palette, the map drawn by 5A56h with transition 2, then the music step.
pub fn ui_transition_to_map_interface(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5a3d)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(g::BYTE_00FB, 0xff);
    c.call(0xc13b, 0x5a45)?;
    c.interp(0x5a45, 0x5a49)?;
    c.run(0x5a49, 4)?;
    c.set_reg(BP, 0x5a56);
    c.set_reg8(r8::AL, 2);
    let dx = c.reg(DX);
    let zero = c.xor(W, dx, dx);
    c.set_reg(DX, zero);
    c.call(0xc108, 0x5a53)?;
    c.run(0x5a53, 1)?; // jmp AE04h
    music_step(&mut c)
}

/// `draw_string_location_type` (629D): the kind of the location at DI
/// (string 44h + its kind index) at (DX, BX) in colour CX.
pub fn draw_string_location_type(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x629d)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::GET_LOCATION_TYPE_STRING_OFFSET, 0x62a0)?;
    c.run(0x62a0, 2)?;
    let ax = c.reg(AX);
    let string = c.add(W, ax, 0x44);
    c.set_reg(AX, string);
    super::text::font_draw_string_with_color_at_pos(c.m, 0xd194)
}

/// `draw_location_name` (62A6): the name of the location at DI (string
/// [DI], then ' ' or '-' when [DI+1] is 3 or more, then string 0Ch +
/// [DI+1]) at (DX, BX) in colour CX.
pub fn draw_location_name(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x62a6)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let di = c.reg(DI);
    let first = c.byte(di) as u16;
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    let s = c.add(W, first, 0);
    c.set_reg(AX, s);
    c.call(0xd194, 0x62b0)?;
    let di = c.reg(DI);
    let second = c.byte(di.wrapping_add(1));
    let dash = second >= 3;
    c.run(0x62b0, if dash { 5 } else { 4 })?;
    c.cmp(B, second as u16, 3);
    c.set_reg8(r8::AL, if dash { b'-' } else { b' ' });
    let draw = c.word(g::FONT_DRAW);
    c.call(draw, 0x62be)?;
    c.run(0x62be, 4)?;
    let di = c.reg(DI);
    let second = c.byte(di.wrapping_add(1)) as u16;
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    let s = c.add(W, second, 0x0c);
    c.set_reg(AX, s);
    super::gfx::font_draw_phrase_or_command_string(c.m, 0xd19b)
}

/// `decrease_troop_motivation` (6F93): the troop at SI loses AL motivation
/// (not below 0); below 5 it is set to 4, `sub_7085` runs and the troop
/// gets bit 20h of +12h. AX kept.
pub fn decrease_troop_motivation(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6f93)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let at = si.wrapping_add(0x15);
    let old = c.byte(at);
    let al = c.reg8(r8::AL);
    let under = al > old;
    let v = if under { 0 } else { old - al };
    let low = v < 5;
    c.run(start, 2 + under as u32 + 2 + if low { 3 } else { 1 })?;
    let r = c.sub(B, old as u16, al as u16) as u8;
    c.set_byte(at, r);
    if under {
        c.set_byte(at, 0);
    }
    c.cmp(B, v as u16, 5);
    if !low {
        return c.ret();
    }
    c.set_byte(at, 4);
    let ax = c.reg(AX);
    c.push(ax);
    c.call(code::SUB_7085, 0x6faa)?;
    c.run(0x6faa, 3)?;
    let si = c.reg(SI);
    let f = c.word(si.wrapping_add(0x12));
    let f = c.or(W, f, 0x20);
    c.set_word(si.wrapping_add(0x12), f);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// `loc_0a49c`: the counter at SI on the back buffer (the front buffer
/// pointer [0DBDAh] kept): resource 55h opened, then sprite 3 + the tens
/// of its byte at the place in the words after it (`loc_0a45c`), drawn
/// by `loc_0c22f`; the byte becomes 1. SI past it.
pub fn draw_counter(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa49c)?;
    let mut c = Ctx::new(m);
    draw_counter_at(&mut c)
}

fn draw_counter_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xa49c, 2)?;
    let fb = c.word(0xdbda);
    c.push(fb);
    c.call(0xc08e, 0xa4a3)?;
    c.run(0xa4a3, 2)?;
    c.set_reg(AX, 0x55);
    c.call(0xc13e, 0xa4a9)?;
    c.run(0xa4a9, 10)?;
    c.lods(B, DS);
    c.aam(10);
    let ah = c.reg8(r8::AH);
    c.xor(B, 0, 0);
    let al = c.add(B, ah as u16, 3);
    c.set_reg(AX, al);
    let si = c.reg(SI);
    c.set_byte(si, 1);
    let si = c.inc(W, si);
    c.set_reg(SI, si);
    let dx = c.word(si);
    let bx = c.word(si.wrapping_add(2));
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.call(0xa45c, 0xa4be)?;
    c.run(0xa4be, 1)?;
    c.call(0xc22f, 0xa4c1)?;
    c.run(0xa4c1, 2)?;
    let fb = c.pop();
    c.set_word(0xdbda, fb);
    c.ret()
}

/// `loc_0a47d`: the counters by the bits of [0DBC8h]: 8 the one at
/// 28A6h, 800h those at 28AEh and 28B6h ([`draw_counter`]).
pub fn draw_counters(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa47d)?;
    let mut c = Ctx::new(m);
    let f = c.word(0xdbc8);
    if f & 8 != 0 {
        c.run(start, 4)?;
        c.test(W, f, 8);
        c.set_reg(SI, 0x28a6);
        c.call(0xa49c, 0xa48b)?;
    } else {
        c.run(start, 2)?;
        c.test(W, f, 8);
    }
    let f = c.word(0xdbc8);
    if f & 0x800 == 0 {
        c.run(0xa48b, 3)?;
        c.test(W, f, 0x800);
        return c.ret();
    }
    c.run(0xa48b, 4)?;
    c.test(W, f, 0x800);
    c.set_reg(SI, 0x28ae);
    c.call(0xa49c, 0xa499)?;
    c.run(0xa499, 1)?;
    c.set_reg(SI, 0x28b6);
    draw_counter_at(&mut c)
}

/// `loc_00fb2`: `table_2280`, the sky palette (`loc_03971`), then the sky
/// rows: [`sub_39b9`] while [46D7h] runs, else [`sky_palette_rows_38bd`].
pub fn sky_refresh(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0fb2)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x395f, 0x0fb5)?;
    c.run(0x0fb5, 1)?;
    c.call(0x3971, 0x0fb8)?;
    let k = c.byte(0x46d7);
    c.run(0x0fb8, 3)?;
    c.cmp(B, k as u16, 0);
    if k != 0 {
        sub_39b9(c.m, 0x39b9)
    } else {
        sky_palette_rows_38bd(c.m, 0x398c)
    }
}

/// `loc_038f1`: the sky palette and rows redrawn (`loc_03971`,
/// [`sub_39b9`]); unless [46D7h] already ran (it becomes 40h), the frame
/// task 3916h every 10h ticks.
pub fn start_sky_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x38f1)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x3971, 0x38f4)?;
    c.run(0x38f4, 1)?;
    c.call(0x39b9, 0x38f7)?;
    let old = c.byte(0x46d7);
    c.run(0x38f7, if old != 0 { 5 } else { 7 })?;
    c.set_byte(0x46d7, 0x40);
    c.set_reg8(r8::AL, old);
    c.or(B, old as u16, old as u16);
    if old != 0 {
        return c.ret();
    }
    c.set_reg(SI, 0x3916);
    c.set_reg(BP, 0x10);
    super::tasks::add_frame_task(c.m, 0xda25)
}

/// `loc_038e1`: with [46DFh] set, when the sky palette for the time
/// (`get_sky_palette_id_from_game_time_in_bl`) differs from [46D6h],
/// [`start_sky_task`].
pub fn sky_if_changed(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x38e1)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x46df);
    if k == 0 {
        c.run(start, 3)?;
        c.cmp(B, 0, 0);
        return c.ret();
    }
    c.run(start, 3)?;
    c.cmp(B, k as u16, 0);
    c.call(0x395c, 0x38eb)?;
    let bl = c.reg8(r8::BL);
    let cur = c.byte(0x46d6);
    c.run(0x38eb, if cur == bl { 3 } else { 2 })?;
    c.cmp(B, cur as u16, bl as u16);
    if cur == bl {
        return c.ret();
    }
    start_sky_task(c.m, 0x38f1)
}

/// `sub_a541`: [28E8h] = [28E7h], `clear_some_mouse_rect`, `sub_d95b`,
/// then the rect 2886h drawn ([`draw_head_if_needed_and_update_screen_rect_at_si`]).
pub fn redraw_2886(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa541)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let v = c.byte(0x28e7);
    c.set_reg8(r8::AL, v);
    c.set_byte(0x28e8, v);
    c.call(0xdaa3, 0xa54a)?;
    c.run(0xa54a, 1)?;
    c.call(0xd95b, 0xa54d)?;
    c.run(0xa54d, 2)?;
    c.set_reg(SI, 0x2886);
    super::draw::draw_head_if_needed_and_update_screen_rect_at_si(c.m, 0xc4f0)
}

/// `loc_017be`: the head panel 1E6Eh redrawn: front buffer active; the
/// rect to the screen (`loc_0c446`), or with [0CE66h] the picture
/// 0CD9Eh by the driver (far [391Dh], BP 1E76h, ES the front buffer);
/// `loc_01797`, then [`draw_head_if_needed_and_update_screen_rect_at_si`].
pub fn redraw_head_panel(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x17be)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xc07c, 0x17c1)?;
    let k = c.byte(0xce66);
    c.run(0x17c1, 4 + if k == 0 { 1 } else { 3 })?;
    c.set_reg(SI, 0x1e6e);
    c.push(0x1e6e);
    c.cmp(B, k as u16, 0);
    if k == 0 {
        c.call(0xc446, 0x17cf)?;
        c.run(0x17cf, 2)?;
    } else {
        c.set_reg(BP, 0x1e76);
        c.set_reg(SI, 0xcd9e);
        let es = c.word(0xdbd6);
        c.set_sreg(ES, es);
        c.interp(0x17db, 0x17df)?;
        c.run(0x17df, 1)?;
    }
    c.call(0x1797, 0x17e2)?;
    c.run(0x17e2, 2)?;
    let s = c.pop();
    c.set_reg(SI, s);
    super::draw::draw_head_if_needed_and_update_screen_rect_at_si(c.m, 0xc4f0)
}

/// `loc_03916` (a frame task): while [46DFh] is set, the sky faded one
/// step ([46D7h] counting down) by the driver (far [3951h]; the top rows
/// and, by day, the band 30h at 2D0h); at 0, or without [46DFh], the
/// task removes itself.
pub fn sky_fade_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3916)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x46df);
    if k == 0 {
        c.run(start, 5)?;
        c.cmp(B, 0, 0);
        c.set_byte(0x46d7, 0);
        c.set_reg(SI, 0x3916);
        return super::tasks::remove_frame_task(c.m, 0xda5f);
    }
    let alt = c.byte(0x22e3);
    c.run(start, 2 + 4 + if alt == 0 { 2 } else { 0 } + 2)?;
    c.cmp(B, k as u16, 0);
    c.cmp(B, alt as u16, 0);
    let (cx, bx) = if alt == 0 { (0xf0, 0x180) } else { (0x1c5, 0xdb) };
    c.set_reg(CX, cx);
    c.set_reg(BX, bx);
    let al = c.byte(0x46d7);
    c.set_reg8(r8::AL, al);
    let ax = c.reg(AX);
    c.push(ax);
    c.interp(0x3934, 0x3938)?;
    let night = c.byte(0x227d);
    c.run(0x3938, 3 + if night == 0 { 2 } else { 0 })?;
    let a = c.pop();
    c.set_reg(AX, a);
    c.cmp(B, night as u16, 0);
    if night == 0 {
        c.set_reg(CX, 0x30);
        c.set_reg(BX, 0x2d0);
        c.interp(0x3946, 0x394a)?;
    }
    let v = c.byte(0x46d7);
    let r = v.wrapping_sub(1);
    c.run(0x394a, if r != 0 { 3 } else { 5 })?;
    let r = c.dec(B, v as u16);
    c.set_byte(0x46d7, r as u8);
    if r != 0 {
        return c.ret();
    }
    c.set_byte(0x46d7, 0);
    c.set_reg(SI, 0x3916);
    super::tasks::remove_frame_task(c.m, 0xda5f)
}

/// `loc_043e3`: the view brought to the screen: normally the rect 14ACh
/// (`loc_0c446`, then [`draw_head_if_needed_and_update_screen_rect_at_si`]);
/// with [473Eh] at a place [6] 80h, the sky palette, the game area and
/// `sub_c4dd`, then the palette ([`update_screen_palette`]); else
/// (or with [0DBEAh]) `loc_04abe` and the palette.
pub fn show_view(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x43e3)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x473e);
    if k == 0 {
        let e = c.word(0xdbea);
        if e == 0 {
            c.run(start, 7)?;
            c.cmp(B, 0, 0);
            c.cmp(W, 0, 0);
            c.set_reg(SI, 0x14ac);
            c.push(0x14ac);
            c.call(0xc446, 0x43f8)?;
            c.run(0x43f8, 2)?;
            let s = c.pop();
            c.set_reg(SI, s);
            return super::draw::draw_head_if_needed_and_update_screen_rect_at_si(c.m, 0xc4f0);
        }
        c.run(start, 5)?;
        c.cmp(B, 0, 0);
        c.cmp(W, e, 0);
    } else {
        let p = c.byte(6);
        if p == 0x80 {
            c.run(start, 5)?;
            c.cmp(B, k as u16, 0);
            c.cmp(B, 0x80, 0x80);
            c.call(0x388d, 0x4406)?;
            c.run(0x4406, 1)?;
            c.call(0xc43e, 0x4409)?;
            c.run(0x4409, 1)?;
            c.call(0xc4dd, 0x440c)?;
            c.run(0x440c, 1)?;
            return super::gfx::update_screen_palette(c.m, 0xc0f4);
        }
        c.run(start, 5)?;
        c.cmp(B, k as u16, 0);
        c.cmp(B, p as u16, 0x80);
    }
    c.call(0x4abe, 0x4412)?;
    c.run(0x4412, 1)?;
    super::gfx::update_screen_palette(c.m, 0xc0f4)
}

/// `loc_01843`: with a head shown ([0E8h]), its blink: frame 9
/// ([`redraw_head_panel`]), a pause of 8, frame 8.
pub fn head_blink(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1843)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0xe8);
    if k == 0 {
        c.run(start, 3)?;
        c.cmp(B, 0, 0);
        return c.ret();
    }
    c.run(start, 4)?;
    c.cmp(B, k as u16, 0);
    c.set_byte(0xe8, 9);
    c.call(0x17be, 0x1852)?;
    c.run(0x1852, 2)?;
    c.set_reg(AX, 8);
    c.call(0xe387, 0x1858)?;
    c.run(0x1858, 2)?;
    c.set_byte(0xe8, 8);
    redraw_head_panel(c.m, 0x17be)
}

/// `chankiss`: `loc_01392`, `loc_037b2`, the driver's copy (far
/// [3949h]: the front buffer [0DBDAh] to [0DBDEh], from (0, 32h), mode 3),
/// the game area to the screen, resource 27h opened with SI 2298h,
/// `loc_0c21b`, the palette, then [`sub_c4dd`].
pub fn chankiss(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1442)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x1392, 0x1445)?;
    c.run(0x1445, 1)?;
    c.call(0x37b2, 0x1448)?;
    c.run(0x1448, 6)?;
    let es = c.word(0xdbde);
    c.set_sreg(ES, es);
    let ds = c.sreg(DS);
    c.push(ds);
    let nds = c.word(0xdbda);
    c.set_sreg(DS, nds);
    c.xor(W, 0, 0);
    c.set_reg(DX, 0);
    c.set_reg(BX, 0x32);
    c.set_reg(BP, 3);
    c.interp(0x1459, 0x145e)?;
    c.run(0x145e, 2)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.call(0xc43e, 0x1462)?;
    c.run(0x1462, 3)?;
    c.set_reg(SI, 0x2298);
    c.set_reg(AX, 0x27);
    c.call(0xc13e, 0x146b)?;
    c.run(0x146b, 1)?;
    c.call(0xc21b, 0x146e)?;
    c.run(0x146e, 1)?;
    c.call(0xc0f4, 0x1471)?;
    c.run(0x1471, 1)?;
    super::draw::sub_c4dd(c.m, 0xc4dd)
}

/// `loc_03af9`: the speaker's portrait brought in: outside a dialogue
/// ([2Bh]) just the game area to the screen; else once ([47A4h] gets bit
/// 80h) for the speaker [47C4h] (0Fh + [476Ch] for 0Fh) with a place in
/// 47F8h (x, y; clamped to F0h, 71h): `loc_037b5`, the driver's copy
/// (far [3949h], mode 6) from the front buffer to [0DBDEh], then the
/// game area to the screen ([`copy_game_area_to_screen_fb2_to_fb1`]).
pub fn bring_portrait(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3af9)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x2b);
    if k != 0 {
        c.run(start, 3)?;
        c.cmp(B, k as u16, 0);
        return super::screen::copy_game_area_to_screen_fb2_to_fb1(c.m, 0xc43e);
    }
    let st = c.byte(0x47a4);
    if st & 0x80 != 0 {
        c.run(start, 5)?;
        c.cmp(B, 0, 0);
        c.cmp(B, st as u16, 0);
        return c.ret();
    }
    let ax0 = c.word(0x47c4);
    let f = ax0 & 0xff == 0x0f;
    let add = c.byte(0x476c);
    let ax = if f { ax0 & 0xff00 | (0x0fu8.wrapping_add(add)) as u16 } else { ax0 };
    let di = ax.wrapping_shl(2);
    let dx = c.word(di.wrapping_add(0x47f8));
    let neg = dx & 0x8000 != 0;
    c.run(start, 4 + 3 + f as u32 + 6 + if neg { 1 } else { 4 })?;
    c.cmp(B, 0, 0);
    c.cmp(B, st as u16, 0);
    c.set_reg(AX, ax0);
    c.cmp(B, ax0 & 0xff, 0x0f);
    if f {
        let a = c.add(B, 0x0f, add as u16);
        c.set_reg(AX, ax0 & 0xff00 | a);
    }
    let d = c.shl1(W, ax, 2);
    c.set_reg(DI, d);
    c.set_reg(DX, dx);
    c.or(W, dx, dx);
    if neg {
        return c.ret();
    }
    let y = c.word(di.wrapping_add(0x47fa));
    c.push(y);
    c.push(dx);
    let v = c.or(B, st as u16, 0x80);
    c.set_byte(0x47a4, v as u8);
    c.call(0x37b5, 0x3b30)?;
    let dx = c.word_in(SS, c.reg(SP));
    let bx = c.word_in(SS, c.reg(SP).wrapping_add(2));
    let (cx1, cx2) = (dx >= 0xf0, bx >= 0x71);
    c.run(0x3b30, 4 + cx1 as u32 + 2 + cx2 as u32 + 4)?;
    let dx = c.pop();
    let bx = c.pop();
    c.cmp(W, dx, 0xf0);
    let dx = if cx1 { 0xf0 } else { dx };
    c.cmp(W, bx, 0x71);
    let bx = if cx2 { 0x71 } else { bx };
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    let es = c.word(0xdbde);
    c.set_sreg(ES, es);
    let ds = c.sreg(DS);
    c.push(ds);
    let nds = c.word(0xdbda);
    c.set_sreg(DS, nds);
    c.set_reg(BP, 6);
    c.interp(0x3b4f, 0x3b54)?;
    c.run(0x3b54, 2)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    super::screen::copy_game_area_to_screen_fb2_to_fb1(c.m, 0xc43e)
}

/// `ui_draw_head_and_shoulders`: on the screen (resource [2784h] kept):
/// `loc_0c137`, sprite 0Fh at (7Eh, 94h) and the head 10h + [0E8h] at
/// (96h, 89h) (`draw_sprite_clobbering_bx_dx`); the resource reopened.
pub fn ui_draw_head_and_shoulders(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1797)?;
    let mut c = Ctx::new(m);
    head_and_shoulders_at(&mut c)
}

fn head_and_shoulders_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x1797, 2)?;
    let r = c.word(0x2784);
    c.push(r);
    c.call(0xc137, 0x179e)?;
    c.run(0x179e, 4)?;
    c.set_reg(DX, 0x7e);
    c.set_reg(BX, 0x94);
    c.set_reg(AX, 0x0f);
    c.call(0xc22f, 0x17aa)?;
    c.run(0x17aa, 5)?;
    let h = c.byte(0xe8);
    let al = c.add(B, 0x10, h as u16);
    c.set_reg(AX, al);
    c.set_reg(DX, 0x96);
    c.set_reg(BX, 0x89);
    c.call(0xc22f, 0x17ba)?;
    c.run(0x17ba, 2)?;
    let a = c.pop();
    c.set_reg(AX, a);
    super::resources::open_resource_by_index(c.m, 0xc13e)
}

/// `loc_00086`: on the front buffer, `loc_0c0ad`, the panel 1AE4h
/// (`loc_0c097`, BP D1EFh), then [`ui_draw_head_and_shoulders`].
pub fn draw_head_panel_0086(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0086)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xc07c, 0x0089)?;
    c.run(0x0089, 1)?;
    c.call(0xc0ad, 0x008c)?;
    c.run(0x008c, 3)?;
    c.set_reg(SI, 0x1ae4);
    c.set_reg(BP, 0xd1ef);
    c.call(0xc097, 0x0095)?;
    c.run(0x0095, 1)?;
    head_and_shoulders_at(&mut c)
}

/// `loc_0439f`: the scene 24h on the front buffer: outside [473Eh] its
/// frame (`loc_0c49a`, `loc_0c412`, the driver's palette far [3935h],
/// `loc_05b69`, the rect 14A4h filled F5h by far [38DDh]) and
/// [`sub_c4dd`]; else (`loc_038b4` out of a dialogue) only the rect 14B4h
/// (`loc_0c21b`).
pub fn scene_24(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x439f)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xc07c, 0x43a2)?;
    let k = c.byte(0x473e);
    if k == 0 {
        c.run(0x43a2, 4)?;
        c.cmp(B, 0, 0);
        c.set_reg(AX, 0x24);
        c.call(0xc13e, 0x43af)?;
        c.run(0x43af, 1)?;
        c.call(0xc49a, 0x43b2)?;
        c.run(0x43b2, 1)?;
        c.call(0xc412, 0x43b5)?;
        c.interp(0x43b5, 0x43b9)?;
        c.run(0x43b9, 1)?;
        c.call(0x5b69, 0x43bc)?;
        c.run(0x43bc, 3)?;
        c.set_reg(SI, 0x14a4);
        c.set_reg8(r8::AL, 0xf5);
        let es = c.word(0xdbda);
        c.set_sreg(ES, es);
        c.interp(0x43c5, 0x43c9)?;
        c.run(0x43c9, 1)?;
        return super::draw::sub_c4dd(c.m, 0xc4dd);
    }
    let d = c.byte(0x2b);
    c.run(0x43a2, 4 + if d == 0 { 1 } else { 2 })?;
    c.cmp(B, k as u16, 0);
    c.cmp(B, d as u16, 0);
    if d == 0 {
        c.call(0x38b4, 0x43d6)?;
        c.run(0x43d6, 2)?;
    }
    c.set_reg(AX, 0x24);
    c.call(0xc13e, 0x43dc)?;
    c.run(0x43dc, 2)?;
    c.set_reg(SI, 0x14b4);
    c.call(0xc21b, 0x43e2)?;
    c.run(0x43e2, 1)?;
    c.ret()
}

/// `loc_01948`: the people panel: counts in a table of 24h bytes on the
/// stack: each of the 10h records from FD8h at the current place ([7]
/// against byte 3) and known (`loc_0127c` with byte 0Eh) counts at its
/// slot (byte 0 - 1, +0Ch with bit 40h of byte 0Fh), the place [4] up to
/// 0Ch at 17h + it; then for the 0Bh slots of the list 1426h (offsets
/// from (([120Dh], [120Fh])) both counts drawn (`loc_019df`) and the
/// third as sprite 1.
pub fn people_panel(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1948)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let sp = c.reg(SP);
    let sp = c.sub(W, sp, 0x24);
    c.set_reg(SP, sp);
    c.set_reg(DI, sp);
    let ss = c.sreg(SS);
    c.push(ss);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(CX, 0x24);
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.rep(0xaa);
    c.run(0x1956, 4)?;
    let tab = sp;
    c.set_reg(SI, 0xfd8);
    c.set_reg(DI, tab);
    c.set_reg(CX, 0x10);
    let place = c.byte(7);
    c.set_reg8(r8::DH, place);
    for i in 0..0x10u16 {
        let si = 0xfd8 + 0x10 * i;
        c.set_reg(SI, si);
        let b3 = c.byte(si.wrapping_add(3));
        'one: {
            if b3 != place {
                c.run(0x1962, 2)?;
                c.cmp(B, place as u16, b3 as u16);
                break 'one;
            }
            c.run(0x1962, 4)?;
            c.cmp(B, place as u16, b3 as u16);
            let al = c.byte(si.wrapping_add(0x0e));
            c.set_reg8(r8::AL, al);
            c.call(0x127c, 0x196d)?;
            if c.flag(CF) {
                c.run(0x196d, 1)?;
                break 'one;
            }
            let si = c.reg(SI);
            let w = c.word(si);
            let f = c.byte(si.wrapping_add(0x0f));
            let hi = f & 0x40 != 0;
            c.run(0x196d, 6 + hi as u32 + 1)?;
            let bl = c.dec(B, w & 0xff);
            c.xor(B, 0, 0);
            c.test(B, f as u16, 0x40);
            let mut bx = bl;
            if hi {
                bx = c.add(W, bx, 0x0c);
            }
            c.set_reg(BX, bx);
            let di = c.reg(DI);
            let v = c.byte(bx.wrapping_add(di));
            let v = c.inc(B, v as u16);
            c.set_byte(bx.wrapping_add(di), v as u8);
        }
        let si = c.reg(SI);
        let more = i != 0x0f;
        c.run(0x1980, 2)?;
        let s = c.add(W, si, 0x10);
        c.set_reg(SI, s);
        c.set_reg(CX, 0x0f - i);
        let _ = more;
    }
    let p4 = c.byte(4);
    let low = p4 <= 0x0c;
    c.run(0x1985, 4 + if low { 2 } else { 0 } + 3)?;
    let w4 = c.word(4);
    c.xor(B, 0, 0);
    let mut bx = w4 & 0xff;
    c.cmp(B, bx, 0x0c);
    let di = c.reg(DI);
    if low {
        bx = c.add(B, bx, 0x17);
        let v = c.byte(bx.wrapping_add(di));
        let v = c.inc(B, v as u16);
        c.set_byte(bx.wrapping_add(di), v as u8);
    }
    c.set_reg(BX, bx);
    c.set_reg(CX, 0x0b);
    let di = c.inc(W, di);
    c.set_reg(DI, di);
    c.set_reg(SI, 0x1426);
    for i in 0..0x0bu16 {
        c.run(0x199c, 14)?;
        let (x0, y0) = (c.word(0x120d), c.word(0x120f));
        let ox = c.lods(B, DS);
        c.xor(B, 0, 0);
        let dx = c.add(W, x0, ox);
        let oy = c.lods(B, DS);
        let bx = c.add(W, y0, oy);
        c.set_reg(AX, oy);
        let (cx, si, di) = (c.reg(CX), c.reg(SI), c.reg(DI));
        c.push(cx);
        c.push(si);
        c.push(di);
        let bx = c.add(W, bx, 2);
        c.set_reg(BX, bx);
        let dx = c.add(W, dx, 3);
        c.set_reg(DX, dx);
        let n = c.byte(di);
        c.set_reg8(r8::CL, n);
        c.call(0x19df, 0x19ba)?;
        c.run(0x19ba, 3)?;
        let bx = c.reg(BX);
        let bx = c.add(W, bx, 7);
        c.set_reg(BX, bx);
        let di = c.reg(DI);
        let n = c.byte(di.wrapping_add(0x0c));
        c.set_reg8(r8::CL, n);
        c.call(0x19df, 0x19c3)?;
        let di = c.reg(DI);
        let third = c.byte(di.wrapping_add(0x18));
        c.run(0x19c3, 4 + if third != 0 { 2 } else { 0 })?;
        let bx = c.reg(BX);
        let bx = c.sub(W, bx, 4);
        c.set_reg(BX, bx);
        let dx = c.reg(DX);
        let dx = c.add(W, dx, 9);
        c.set_reg(DX, dx);
        c.cmp(B, third as u16, 0);
        if third != 0 {
            c.set_reg(AX, 1);
            c.call(0xc22f, 0x19d5)?;
        }
        let last = i == 0x0a;
        c.run(0x19d5, 5 + if last { 2 } else { 0 })?;
        let d = c.pop();
        let s = c.pop();
        c.set_reg(SI, s);
        let x = c.pop();
        let d = c.inc(W, d);
        c.set_reg(DI, d);
        c.set_reg(CX, x.wrapping_sub(1));
    }
    let sp = c.reg(SP);
    let sp = c.add(W, sp, 0x24);
    c.set_reg(SP, sp);
    c.ret()
}

/// `loc_00083`: `loc_0cfa0`, then [`draw_head_panel_0086`].
pub fn draw_head_panel_0083(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0083)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xcfa0, 0x0086)?;
    draw_head_panel_0086(c.m, 0x0086)
}

/// `sub_4821`: scene 2Ah drawn (game area, palette, front buffer,
/// `loc_03a95`; for [4731h] 0Dh `loc_0ac30`); past 0Eh the place (DX, BX)
/// moved by the zoom ([4731h] - 0Eh steps of 5 along x, by CL's sign;
/// y up by its square / 2); `loc_03aa9`, `loc_03a95`; with [46FFh] the
/// overlay `loc_03a73` with [4731h] 0 meanwhile; `sub_c4dd`, and
/// `sky_fade_task` while [46D7h] runs. CX kept.
pub fn scene_2a(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4821)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let cx0 = c.reg(CX);
    c.push(cx0);
    c.call(0xc43e, 0x4825)?;
    c.run(0x4825, 2)?;
    c.set_reg(AX, 0x2a);
    c.call(0xc13e, 0x482b)?;
    for (at, to) in [(0x482bu16, 0xc0f4u16), (0x482e, 0xc07c), (0x4831, 0x3a95)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    let z = c.byte(0x4731);
    c.run(0x4834, 3 + if z == 0x0d { 2 } else { 0 })?;
    c.set_reg8(r8::AL, z);
    c.cmp(B, z as u16, 0x0d);
    if z == 0x0d {
        let ax = c.reg(AX);
        c.push(ax);
        c.call(0xac30, 0x483f)?;
        c.run(0x483f, 1)?;
        let a = c.pop();
        c.set_reg(AX, a);
    }
    let al = c.reg8(r8::AL);
    let steps = al.wrapping_sub(0x0e);
    let move_ = al > 0x0e;
    if move_ {
        let cl = (c.word_in(SS, c.reg(SP)) & 0xff) as u8;
        let neg = cl & 0x80 != 0;
        c.run(0x4840, 2 + 5 + neg as u32 + 1 + 3 * steps as u32 + 3 + 1)?;
        let a = c.sub(B, al as u16, 0x0e);
        c.set_reg8(r8::AL, a as u8);
        let x = c.pop();
        c.push(x);
        c.or(B, cl as u16, cl as u16);
        let mut cx = 5u16;
        if neg {
            cx = c.neg(W, 5);
        }
        c.set_reg(CX, cx);
        let mut dx = c.reg(DX);
        let mut ah = a;
        for _ in 0..steps {
            dx = c.sub(W, dx, cx);
            ah = c.dec(B, ah);
        }
        c.set_reg(DX, dx);
        c.set_reg(AX, ah << 8 | a);
        c.mul8(a as u8);
        let p = c.reg(AX);
        let p = c.m.cpu.shift(7, W, p as u32, 1) as u16;
        c.set_reg(AX, p);
        let bx = c.reg(BX);
        let bx = c.sub(W, bx, p);
        c.set_reg(BX, bx);
    } else {
        c.run(0x4840, 3)?;
        let a = c.sub(B, al as u16, 0x0e);
        c.set_reg8(r8::AL, a as u8);
    }
    c.call(0x3aa9, 0x4860)?;
    c.run(0x4860, 1)?;
    c.call(0x3a95, 0x4863)?;
    let ov = c.byte(0x46ff);
    c.run(0x4863, 3 + if ov != 0 { 4 } else { 1 })?;
    c.set_reg(CX, ov as u16);
    c.xor(B, 0, 0);
    if ov != 0 {
        let z = c.byte(0x4731);
        c.set_reg8(r8::AL, z);
        let ax = c.reg(AX);
        c.push(ax);
        c.set_byte(0x4731, 0);
        c.call(0x3a73, 0x4877)?;
        c.run(0x4877, 3)?;
        let a = c.pop();
        c.set_reg(AX, a);
        c.set_byte(0x4731, a as u8);
    }
    c.call(0xc4dd, 0x487e)?;
    let k = c.byte(0x46d7);
    c.run(0x487e, 2 + if k != 0 { 1 } else { 2 })?;
    c.cmp(B, k as u16, 0);
    if k != 0 {
        c.call(0x3916, 0x4888)?;
        c.run(0x4888, 2)?;
    }
    let x = c.pop();
    c.set_reg(CX, x);
    c.ret()
}

/// `sub_af26`: `loc_0d7ad`, the buttons 1E1Ah, `loc_0af00`, [2406h] 0,
/// `loc_0d33a` (BP 2032h, BX D917h, CX 0), then [`panel_with_head`].
pub fn open_panel_af26(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xaf26)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xd7ad, 0xaf29)?;
    c.run(0xaf29, 2)?;
    c.set_reg(SI, 0x1e1a);
    c.call(0xd72b, 0xaf2f)?;
    c.run(0xaf2f, 1)?;
    c.call(0xaf00, 0xaf32)?;
    c.run(0xaf32, 5)?;
    c.set_word(0x2406, 0);
    c.set_reg(BP, 0x2032);
    c.set_reg(BX, 0xd917);
    c.xor(W, 0, 0);
    c.set_reg(CX, 0);
    c.call(0xd33a, 0xaf43)?;
    panel_with_head_at(&mut c, 0xaf43)
}

/// `sub_af43`: bit 2 of [0C6h] set, `loc_0c432`, `loc_0c2f2` with AL
/// 32h, [1C30h] 0, then the head and shoulders.
pub fn panel_with_head(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xaf43)?;
    let mut c = Ctx::new(m);
    panel_with_head_at(&mut c, start)
}

fn panel_with_head_at(c: &mut Ctx, at: u16) -> Result<(), Bail> {
    c.run(at, 2)?;
    let v = c.byte(0xc6);
    let v = c.or(B, v as u16, 2);
    c.set_byte(0xc6, v as u8);
    c.call(0xc432, 0xaf4b)?;
    c.run(0xaf4b, 2)?;
    c.set_reg8(r8::AL, 0x32);
    c.call(0xc2f2, 0xaf50)?;
    c.run(0xaf50, 2)?;
    c.set_byte(0x1c30, 0);
    ui_draw_head_and_shoulders(c.m, 0x1797)
}

/// `loc_03d2f`: the character AL (AH: its sprite flags) drawn at (DX,
/// BX) from resource 26h (the place kept at 47F8h + 4 * AL; from 0Fh up
/// the extras as 0Fh with [476Ch] = AL - 0Fh): its sentence picked
/// (`pick_sentence`); in phases 15h..1Fh with flag 4 (`loc_0127c`, CF)
/// 11h, 35h lower; 0Ch with bit 10h of [10A7h] as 12h; sprites 2 * AL
/// and the next one; the resource opened before back.
pub fn draw_character(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3d2f)?;
    let mut c = Ctx::new(m);
    c.run(start, 10)?;
    let r = c.word(0x2784);
    c.push(r);
    let ax = c.reg(AX);
    c.push(ax);
    c.xor(B, 0, 0);
    let a = ax & 0xff;
    c.set_reg(AX, a);
    let di = c.shl1(W, a, 2);
    c.set_reg(DI, di);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.set_word(di.wrapping_add(0x47f8), dx);
    c.set_word(di.wrapping_add(0x47fa), bx);
    c.set_reg(AX, 0x26);
    c.call(0xc13e, 0x3d4a)?;
    let ax = c.word_in(SS, c.reg(SP));
    let al = ax as u8;
    c.run(0x3d4a, 5 + if al >= 0x0f { 3 } else { 0 })?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.set_reg8(r8::CH, (ax >> 8) as u8);
    c.cmp(B, al as u16, 0x0f);
    if al >= 0x0f {
        let e = c.sub(B, al as u16, 0x0f);
        c.set_byte(0x476c, e as u8);
        c.set_reg8(r8::AL, 0x0f);
    }
    c.call(0x9123, 0x3d5b)?;
    c.run(0x3d5b, 1)?;
    c.call(0x127c, 0x3d5e)?;
    let cf = c.flag(CF);
    let mut al = if cf { 0x11 } else { c.reg8(r8::AL) };
    let f = c.byte(0x10a7);
    let swap = al == 0x0c && f & 0x10 != 0;
    let n = 1 + if cf { 2 } else { 0 } + 2 + if al == 0x0c { 2 + swap as u32 } else { 0 } + 4;
    c.run(0x3d5e, n)?;
    if cf {
        c.set_reg8(r8::AL, 0x11);
        let bx = c.reg(BX);
        let bx = c.add(W, bx, 0x35);
        c.set_reg(BX, bx);
    }
    c.cmp(B, al as u16, 0x0c);
    if al == 0x0c {
        c.test(B, f as u16, 0x10);
        if swap {
            al = 0x12;
        }
    }
    let ch = c.reg8(r8::CH);
    let a = c.shl1(B, al as u16, 1);
    let ax = (ch as u16) << 8 | a;
    c.set_reg(AX, ax);
    c.push(ax);
    c.call(0xc2fd, 0x3d7a)?;
    c.run(0x3d7a, 3)?;
    let a = c.pop();
    let a = c.inc(W, a);
    c.set_reg(AX, a);
    c.call(0xc22f, 0x3d7f)?;
    c.run(0x3d7f, 2)?;
    let r = c.pop();
    c.set_reg(AX, r);
    super::resources::open_resource_by_index(c.m, 0xc13e)
}

/// `loc_0d48a`: the button CL's label (AX the phrase with flags, from
/// `font_select_small_font`) on the screen ([0DBDAh] kept; the back
/// buffer when [0DCE6h] is positive): at (5Dh, the button's +2 + 1) of
/// the button 1B48h + 0Eh * CL, in F3h on F5h, or with bit 14 clear
/// highlighted (+8 bit 7) in FAh, swapped when bit 15 is set; then a
/// space, the string, and the box (5Dh..E3h, 7 high) drawn by the
/// driver (far [38DDh]) in the background colour.
pub fn button_label(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd48a)?;
    let mut c = Ctx::new(m);
    c.run(0xd48a, 2)?;
    let fb = c.word(0xdbda);
    c.push(fb);
    c.call(0xc08e, 0xd491)?;
    let k = c.byte(0xdce6);
    let back = (k as i8) > 0;
    c.run(0xd491, 2 + back as u32)?;
    c.cmp(B, k as u16, 0);
    if back {
        c.call(0xc07c, 0xd49b)?;
    }
    c.run(0xd49b, 1)?;
    c.call(0xd075, 0xd49e)?;
    c.run(0xd49e, 9)?;
    let ax = c.reg(AX);
    c.set_reg(SI, ax);
    c.set_reg8(r8::AL, 0x0e);
    let cl = c.reg8(r8::CL);
    c.mul8(cl);
    let p = c.reg(AX);
    c.set_reg(DI, p);
    let di = c.add(W, p, 0x1b48);
    c.set_reg(DI, di);
    let bx = c.word(di.wrapping_add(2));
    let bx = c.inc(W, bx);
    c.set_reg(BX, bx);
    c.set_reg(DX, 0x5d);
    c.call(0xd04e, 0xd4b4)?;
    let text = ax & 0x3fff;
    let ah = (ax >> 8) as u8;
    let lit = ah & 0x40 == 0;
    let swap = lit && ah & 0x80 != 0;
    c.run(0xd4b4, 5 + if text == 0 { 0 } else { 3 + if lit { 4 + swap as u32 } else { 0 } + 2 })?;
    c.set_byte(0xdbe5, 0xf3);
    let f = c.byte(di.wrapping_add(8));
    let f = c.and(B, f as u16, 0x7f);
    c.set_byte(di.wrapping_add(8), f as u8);
    c.set_reg(AX, ax);
    c.and(W, ax, 0x3fff);
    c.set_reg(SI, text);
    if text != 0 {
        c.set_reg8(r8::AL, 0xf5);
        c.test(B, ah as u16, 0x40);
        if lit {
            let f = c.or(B, f, 0x80);
            c.set_byte(di.wrapping_add(8), f as u8);
            c.set_reg8(r8::AL, 0xfa);
            c.or(B, ah as u16, ah as u16);
            if swap {
                let old = c.byte(0xdbe5);
                c.set_byte(0xdbe5, 0xfa);
                c.set_reg8(r8::AL, old);
            }
        }
        let al = c.reg8(r8::AL);
        c.set_byte(0xdbe4, al);
        c.call(0xcf70, 0xd4e0)?;
        c.run(0xd4e0, 2)?;
        c.set_reg8(r8::AL, 0x20);
        let to = c.word(0x2518);
        c.call(to, 0xd4e6)?;
        c.run(0xd4e6, 1)?;
        c.call(0xd1bb, 0xd4e9)?;
    }
    c.run(0xd4e9, 1)?;
    c.call(0xd05f, 0xd4ec)?;
    c.run(0xd4ec, 8)?;
    c.set_reg(SI, 0xdce9);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.set_word(0xdce9, dx);
    c.set_word(0xdceb, bx);
    c.set_word(0xdced, 0xe3);
    let b = c.add(W, bx, 7);
    c.set_reg(BX, b);
    c.set_word(0xdcef, b);
    let bg = c.byte(0xdbe5);
    c.set_reg8(r8::AL, bg);
    let es = c.word(0xdbda);
    c.set_sreg(pc::cpu::ES, es);
    c.interp(0xd506, 0xd50a)?;
    c.run(0xd50a, 2)?;
    let v = c.pop();
    c.set_word(0xdbda, v);
    c.ret()
}
