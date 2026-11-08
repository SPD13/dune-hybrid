//! More of the game screen: the date and time panel, the communication
//! device's animations, the ornithopter, the settings sliders, talking
//! heads without lip sync, lip sync start and stop.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::cpu::Bus;
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CS, CX, DI, DS, DX, ES, SI, SP, SS, ZF},
    rec::Bail,
};

mod code {
    pub const SET_SCREEN_AS_ACTIVE_FRAMEBUFFER: u16 = 0xc08e;
    pub const OPEN_RESOURCE_BY_INDEX: u16 = 0xc13e;
    pub const DRAW_ICONS_LIST_AT_SI: u16 = 0xc21b;
    pub const DRAW_SPRITE_CLOBBERING_BX_DX: u16 = 0xc22f;
    pub const DRAW_SPRITE: u16 = 0xc2fd;
    pub const DRAW_SPRITE_CLIPPED: u16 = 0xc305;
    pub const RAND_MASKED: u16 = 0xe3b7;
    pub const WAIT_INTERRUPTIBLE: u16 = 0xe353;
    pub const COMM_DRAW_GLOW: u16 = 0x275f;
    pub const COMM_DRAW_GLOW_INDEX: u16 = 0x27b6;
    pub const AUDIO_START_VOC: u16 = 0xab15;
    pub const DRAW_MOON_AND_SUN: u16 = 0x1a9b;
    pub const FONT_SELECT_SMALL_FONT: u16 = 0xd075;
    pub const GET_INGAME_DAY: u16 = 0x1ad1;
    pub const DRAW_NUMBER: u16 = 0xe290;
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
    pub const FONT_SET_DRAW_POSITION: u16 = 0xd04e;
    pub const GET_PHRASE_OR_COMMAND_STRING_SI: u16 = 0xcf70;
    pub const SUB_88F1: u16 = 0x88f1;
    pub const FORMAT_INTERPOLATED_STRING: u16 = 0x8944;
    pub const FONT_DRAW_STRING: u16 = 0xd1bb;
    pub const CHECK_PCM_ENABLED: u16 = 0xae2f;
    pub const SUB_AE28: u16 = 0xae28;
    pub const SETTINGS_UI_DRAW_SLIDER: u16 = 0xa502;
    pub const SETTINGS_UI_ADD_GLOBAL_OFFSET: u16 = 0xa45c;
    pub const SUB_A465: u16 = 0xa465;
    pub const REMOVE_FRAME_TASK: u16 = 0xda5f;
    pub const SUB_D61D: u16 = 0xd61d;
    pub const IS_VOC_PCM_PLAYING: u16 = 0xabcc;
    pub const SET_VOC_PCM_IS_NOT_PLAYING: u16 = 0xabc6;
    pub const CLOSE_PCM_VOICE_FILE_HANDLE: u16 = 0xa9a1;
    pub const PCM_CALL_AC30: u16 = 0xac30;
    pub const PCM_STOP_VOC: u16 = 0xac14;
    pub const OPEN_PCM_VOICE_FILE: u16 = 0xa90b;
    pub const SUB_A9B9: u16 = 0xa9b9;
    pub const LIP_SYNC_FRAME_TASK: u16 = 0xa7c2;
}

/// `loc_00b21`: PCM driver function 5; the on-map resource back to 25h
/// (patched into `open_onmap_resource`); task B45h removed; [3CBEh] = 0
/// unless outside the map view with the sign of [FBh] set.
pub fn sub_0b21(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0b21)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::PCM_CALL_AC30, 0x0b24)?;
    c.run(0x0b24, 3)?;
    c.m.cpu.write8(&mut c.m.hw, CS, 0xc13c, 0x25);
    c.set_reg(SI, 0x0b45);
    c.call(code::REMOVE_FRAME_TASK, 0x0b30)?;
    let view = c.byte(g::MAP_VIEW_FLAG);
    let fb = c.byte(g::BYTE_00FB);
    let skip = view == 0 && fb & 0x80 != 0;
    c.run(0x0b30, if view != 0 { 4 } else if skip { 5 } else { 6 })?;
    c.cmp(B, view as u16, 0);
    if view == 0 {
        c.cmp(B, fb as u16, 0);
    }
    if !skip {
        c.set_word(g::WORD_3CBE, 0);
    }
    c.ret()
}

/// `ui_draw_date_and_time_indicator` (1A34): unless [1AFEh] is set, on the
/// screen: the sun and moon for the time of day (positions at 1E7Eh, 8
/// bytes a slot; sprites 4Ah, 4Bh), then the day of the year in the
/// small font (right-aligned at BEh), and a space. The framebuffer kept.
pub fn ui_draw_date_and_time_indicator(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1a34)?;
    let mut c = Ctx::new(m);
    let hidden = c.word(g::WORD_1AFE);
    if hidden != 0 {
        c.run(start, 3)?;
        c.cmp(W, hidden, 0);
        return c.ret();
    }
    c.run(start, 4)?;
    c.cmp(W, 0, 0);
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.push(fb);
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0x1a42)?;
    c.run(0x1a42, 9)?;
    let time = c.word(g::GAME_TIME);
    let slot = c.and(W, time, 0x0f);
    let off = c.shl1(W, slot, 3);
    let si = c.add(W, off, g::SUN_MOON_POSITIONS);
    c.set_reg(AX, si);
    c.set_reg(SI, si);
    c.set_reg(AX, 0x4a);
    c.call(code::DRAW_MOON_AND_SUN, 0x1a59)?;
    c.run(0x1a59, 2)?;
    c.set_reg(AX, 0x4b);
    c.call(code::DRAW_MOON_AND_SUN, 0x1a5f)?;
    c.run(0x1a5f, 1)?;
    c.call(code::FONT_SELECT_SMALL_FONT, 0x1a62)?;
    c.run(0x1a62, 2)?;
    c.set_word(g::TEXT_COLOUR, 0xf1fa);
    c.call(code::GET_INGAME_DAY, 0x1a6b)?;
    // The day of the year: AX + 365, less 365 while not below.
    c.run(0x1a6b, 2)?;
    c.set_reg(BX, 0x16d);
    let ax = c.reg(AX);
    let mut day = c.add(W, ax, 0x16d);
    loop {
        let more = day.wrapping_sub(0x16d) >= 0x16d;
        c.run(0x1a70, 3)?;
        day = c.sub(W, day, 0x16d);
        c.cmp(W, day, 0x16d);
        if !more {
            break;
        }
    }
    let day = day.wrapping_add(1);
    let n = if day >= 100 { 6 } else if day >= 10 { 9 } else { 10 };
    c.run(0x1a76, n)?;
    let day = c.inc(W, day.wrapping_sub(1));
    c.set_reg(AX, day);
    c.set_reg(DX, 0x0b);
    c.set_reg(BX, 0xbe);
    c.cmp(W, day, 100);
    if day < 100 {
        let dl = c.sub(B, 0x0b, 2);
        c.set_reg(DX, dl);
        c.cmp(W, day, 10);
        if day < 10 {
            let dl = c.sub(B, dl, 2);
            c.set_reg(DX, dl);
        }
    }
    c.call(code::DRAW_NUMBER, 0x1a90)?;
    c.run(0x1a90, 2)?;
    c.set_reg8(r8::AL, b' ');
    let draw = c.word(g::FONT_DRAW);
    c.call(draw, 0x1a96)?;
    c.run(0x1a96, 2)?;
    let fb = c.pop();
    c.set_word(g::FRAMEBUFFER_ACTIVE, fb);
    c.ret()
}

/// `comm_fade_out_glow` (2773): sound 9, then glow steps 4 down to 0, 9
/// ticks each, then the panels again.
pub fn comm_fade_out_glow(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2773)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(AX, 0x15);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0x2779)?;
    c.run(0x2779, 2)?;
    c.set_reg8(r8::AL, 9);
    c.call(code::AUDIO_START_VOC, 0x277e)?;
    c.run(0x277e, 4)?;
    c.set_word(g::COMM_STEP, 4);
    loop {
        c.set_reg(BP, code::COMM_DRAW_GLOW_INDEX);
        c.set_reg(AX, 9);
        c.call(code::WAIT_INTERRUPTIBLE, 0x278d)?;
        let step = c.word(g::COMM_STEP);
        let more = step.wrapping_sub(1) & 0x8000 == 0;
        c.run(0x278d, if more { 2 + 3 } else { 3 })?;
        let step = c.dec(W, step);
        c.set_word(g::COMM_STEP, step);
        if !more {
            return super::ui::comm_draw_panels(c.m, 0x274e);
        }
    }
}

/// From 27C9: the flicker: 4 random sprites (17h + 0..15, never the same
/// twice running) in a row at y = A3h from x = 67h, 3 apart; then the
/// panel icons with sprite 13h + step & 3. Drawn on the screen, then back
/// to the front buffer.
fn flicker(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x27c9, 1)?;
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0x27cc)?;
    c.run(0x27cc, 4)?;
    c.set_reg(BX, 0x67);
    loop {
        let x = c.reg(BX);
        c.push(x);
        c.set_reg(BX, 0x0f);
        c.call(code::RAND_MASKED, 0x27d6)?;
        let (al, dl) = (c.reg8(r8::AL), c.reg8(r8::DL));
        if al == dl {
            c.run(0x27d6, 3)?; // cmp; je; call again
            c.cmp(B, al as u16, dl as u16);
            continue_rand(c)?;
        }
        c.run(0x27d6, 8)?;
        let al = c.reg8(r8::AL);
        let dl = c.reg8(r8::DL);
        c.cmp(B, al as u16, dl as u16);
        c.set_reg8(r8::DL, al);
        let ax = c.reg(AX);
        let sprite = c.add(W, ax, 0x17);
        c.set_reg(AX, sprite);
        let x = c.pop();
        c.set_reg(BX, x);
        let dx = c.reg(DX);
        c.push(dx);
        c.set_reg(DX, 0xa3);
        c.call(code::DRAW_SPRITE, 0x27e7)?;
        let next = c.reg(BX).wrapping_add(3);
        let more = next <= 0x70;
        c.run(0x27e7, if more { 4 + 3 } else { 4 + 7 })?;
        let dx = c.pop();
        c.set_reg(DX, dx);
        let bx = c.reg(BX);
        let bx = c.add(W, bx, 3);
        c.set_reg(BX, bx);
        c.cmp(W, bx, 0x70);
        if !more {
            break;
        }
    }
    c.set_reg(SI, g::COMM_PANEL_ICONS);
    let step = c.word(g::COMM_STEP);
    let ax = c.and(W, step, 3);
    let sprite = c.add(B, ax, 0x13);
    c.set_reg(AX, sprite);
    c.set_byte(g::COMM_PANEL_ICONS, sprite as u8);
    c.set_byte(g::COMM_PANEL_ICONS + 6, sprite as u8);
    c.call(code::DRAW_ICONS_LIST_AT_SI, 0x2803)?;
    c.run(0x2803, 1)?; // jmp C07Ch
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// The retry of the random sprite (27D3) until it differs from DL.
fn continue_rand(c: &mut Ctx) -> Result<(), Bail> {
    loop {
        c.call(code::RAND_MASKED, 0x27d6)?;
        let (al, dl) = (c.reg8(r8::AL), c.reg8(r8::DL));
        if al != dl {
            return Ok(());
        }
        c.run(0x27d6, 3)?;
        c.cmp(B, al as u16, dl as u16);
    }
}

/// `comm_draw_flicker` (27C9).
pub fn comm_draw_flicker(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x27c9)?;
    flicker(&mut Ctx::new(m))
}

/// `comm_draw_glow_index` (27B6): glow 2 + the step's distance from 0 in
/// a cycle of 8 (0..4 up, 5..7 down), then the flicker.
pub fn comm_draw_glow_index(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x27b6)?;
    let mut c = Ctx::new(m);
    let step = c.word(g::COMM_STEP);
    let low = step & 7 < 5;
    c.run(start, if low { 6 } else { 8 })?;
    let ax = c.and(W, step, 7);
    c.cmp(B, ax, 5);
    let mut al = ax;
    if !low {
        al = c.sub(B, al, 8);
        al = c.neg(B, al);
    }
    let glow = c.add(B, al, 2);
    c.set_reg(AX, glow);
    c.call(code::COMM_DRAW_GLOW, 0x27c9)?;
    flicker(&mut c)
}

/// `draw_orni` (3AA9): the ornithopter at (DX, BX): body (sprite 0),
/// wings (1, at +6, +1Eh), the rotor by [4731h] (2 + 0..5, from 0Fh on),
/// and the shadow (8 + up to 0Eh) at -55h, -35h from there; clipped.
pub fn draw_orni(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3aa9)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let ax = c.reg(AX);
    let zero = c.xor(W, ax, ax);
    c.set_reg(AX, zero);
    c.call(code::DRAW_SPRITE_CLIPPED, 0x3aae)?;
    c.run(0x3aae, 4)?;
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let dx = c.add(W, dx, 6);
    c.set_reg(DX, dx);
    let bx = c.add(W, bx, 0x1e);
    c.set_reg(BX, bx);
    c.set_reg(AX, 1);
    c.call(code::DRAW_SPRITE_CLIPPED, 0x3aba)?;
    let speed = c.byte(g::ORNI_SPEED);
    let under = speed < 0x0f;
    let a = if under { 0 } else { speed - 0x0f };
    let capped = a > 5;
    c.run(0x3aba, 7 + under as u32 + 2 + capped as u32 + 2)?;
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let dx = c.sub(W, dx, 2);
    c.set_reg(DX, dx);
    let bx = c.add(W, bx, 0x14);
    c.set_reg(BX, bx);
    c.set_reg(AX, speed as u16);
    c.xor(B, 0, 0); // xor ah, ah
    c.push(speed as u16);
    let mut al = c.sub(B, speed as u16, 0x0f);
    if under {
        al = c.xor(W, al, al);
    }
    c.cmp(B, al, 5);
    if capped {
        al = 5;
    }
    let rotor = c.add(B, al, 2);
    c.set_reg(AX, rotor);
    c.call(code::DRAW_SPRITE_CLIPPED, 0x3ad7)?;
    let sp = c.reg(SP);
    let saved = c.word_in(SS, sp) as u8;
    let capped = saved >= 0x0e;
    c.run(0x3ad7, 5 + capped as u32 + 2)?;
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let dx = c.sub(W, dx, 0x55);
    c.set_reg(DX, dx);
    let bx = c.sub(W, bx, 0x35);
    c.set_reg(BX, bx);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.cmp(B, ax & 0xff, 0x0e);
    let al = if capped { 0x0e } else { ax & 0xff };
    let shadow = c.add(B, al, 8);
    c.set_reg(AX, ax & 0xff00 | shadow);
    super::screen::draw_sprite_clipped_clobbering(c.m, 0xc30d)
}

/// `font_draw_interpolated_string_w_color_at_pos` (8865): string AX with
/// its fields filled in (`format_interpolated_string` into A6B0h), at
/// (DX, BX) in colour CX. Registers kept.
pub fn font_draw_interpolated_string(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8865)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SAVE_REGS, 0x8868)?;
    c.run(0x8868, 3)?;
    let ax = c.reg(AX);
    c.set_reg(SI, ax);
    let cx = c.reg(CX);
    c.set_word(g::TEXT_COLOUR, cx);
    c.call(code::FONT_SET_DRAW_POSITION, 0x8871)?;
    c.run(0x8871, 1)?;
    c.call(code::GET_PHRASE_OR_COMMAND_STRING_SI, 0x8874)?;
    c.run(0x8874, 1)?;
    c.call(code::SUB_88F1, 0x8877)?;
    c.run(0x8877, 3)?;
    c.set_reg(DI, g::FORMATTED_STRING);
    c.push(g::FORMATTED_STRING);
    c.call(code::FORMAT_INTERPOLATED_STRING, 0x887e)?;
    c.run(0x887e, 4)?;
    let si = c.pop();
    c.set_reg(SI, si);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.call(code::FONT_DRAW_STRING, 0x8884)?;
    c.run(0x8884, 1)?;
    c.call(code::RESTORE_REGS, 0x8887)?;
    c.run(0x8887, 1)?;
    c.ret()
}

/// `setup_non_lip_sync_data_structure` (9BEE): from the list of frames at
/// SI (in the sprite sheet's segment; a frame number, 1 for a two-byte
/// one, 0 ends), builds the 3-byte entries at 460Ah from each frame's
/// table (`[47CCh]`), counting them at [4608h]. Exit: DS = SS.
pub fn setup_non_lip_sync_data_structure(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9bee)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let cx = c.reg(CX);
    let zero = c.xor(W, cx, cx);
    c.set_reg(CX, zero);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    let sheet = c.word_in(SS, g::SPRITE_SHEET + 2);
    c.set_sreg(DS, sheet);
    c.set_reg(DI, g::HEAD_FRAMES);
    loop {
        let si = c.reg(SI);
        let frame = c.byte_in(DS, si);
        if frame == 0 {
            c.run(0x9bfa, 8)?;
            c.lods(B, DS);
            let ah = c.reg8(r8::AH);
            c.xor(B, ah as u16, ah as u16);
            c.set_reg8(r8::AH, 0);
            c.or(B, 0, 0);
            let cx = c.reg(CX);
            c.set_word_in(SS, g::HEAD_FRAME_COUNT, cx);
            let ss = c.sreg(SS);
            c.set_sreg(DS, ss);
            c.push(ss);
            c.pop();
            return c.ret();
        }
        let wide = frame == 1;
        c.run(0x9bfa, 6 + if wide { 2 } else { 0 } + 6)?;
        c.lods(B, DS);
        let ah = c.reg8(r8::AH);
        c.xor(B, ah as u16, ah as u16);
        c.set_reg8(r8::AH, 0);
        c.or(B, frame as u16, frame as u16);
        c.cmp(B, frame as u16, 1);
        if wide {
            c.set_reg8(r8::AH, 1);
            c.lods(B, DS);
        }
        let si = c.reg(SI);
        c.push(si);
        let ax = c.reg(AX);
        let ax = c.sub(W, ax, 2);
        let index = c.shl1(W, ax, 1);
        c.set_reg(AX, index);
        c.set_reg(BP, index);
        let table = c.word_in(SS, g::HEAD_FRAME_TABLE);
        let off = c.word_in(DS, index.wrapping_add(table));
        let si = c.add(W, table, off);
        c.set_reg(SI, si);
        loop {
            let si = c.reg(SI);
            let b = c.byte_in(DS, si);
            if b == 0 {
                c.run(0x9c18, 5)?; // lodsb; or; je; pop si; jmp
                c.lods(B, DS);
                c.or(B, 0, 0);
                let si = c.pop();
                c.set_reg(SI, si);
                break;
            }
            c.run(0x9c18, 7)?;
            c.lods(B, DS);
            c.or(B, b as u16, b as u16);
            c.stos(B);
            c.movs(W);
            let cx = c.reg(CX);
            let cx = c.inc(W, cx);
            c.set_reg(CX, cx);
        }
    }
}

/// `draw_talking_head_at_si` (9D2D): the head's parts listed at SI (a
/// count, then sprite, x, y bytes; offset by [1BF0h], [1BF2h]), clipped,
/// into the active framebuffer. Exit: DS = SS, SI past the list.
pub fn draw_talking_head_at_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9d2d)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let n = c.lods(W, DS);
    c.set_reg(CX, n);
    loop {
        c.run(0x9d30, 22)?;
        let cx = c.reg(CX);
        c.push(cx);
        let sprite = c.lods(B, DS);
        let ah = c.reg8(r8::AH);
        c.xor(B, ah as u16, ah as u16);
        c.set_reg8(r8::AH, 0);
        c.set_reg(BP, sprite);
        let x = c.lods(B, DS);
        c.set_reg(DX, x);
        let y = c.lods(B, DS);
        c.set_reg(BX, y);
        let ox = c.word(g::HEAD_ORIGIN_X);
        let dx = c.add(W, x, ox);
        c.set_reg(DX, dx);
        let oy = c.word(g::HEAD_ORIGIN_Y);
        let bx = c.add(W, y, oy);
        c.set_reg(BX, bx);
        let si = c.reg(SI);
        c.push(si);
        let bp = c.dec(W, sprite);
        c.set_reg(BP, bp);
        let fb = c.word(g::FRAMEBUFFER_ACTIVE);
        c.set_sreg(ES, fb);
        let (sheet, seg) = (c.word(g::SPRITE_SHEET), c.word(g::SPRITE_SHEET + 2));
        c.set_reg(SI, sheet);
        c.set_sreg(DS, seg);
        let index = c.shl1(W, bp, 1);
        c.set_reg(BP, index);
        let off = c.word_in(DS, index.wrapping_add(sheet));
        let si = c.add(W, sheet, off);
        c.set_reg(SI, si);
        let size = c.lods(W, DS);
        c.set_reg(DI, size);
        c.lods(W, DS);
        let ah = c.reg8(r8::AH);
        c.xor(B, ah as u16, ah as u16);
        c.set_reg8(r8::AH, 0);
        let ax = c.reg(AX);
        c.set_reg(CX, ax);
        c.set_reg(BP, g::RECT_D834);
        c.interp(0x9d5e, 0x9d63)?;
        let sp = c.reg(SP);
        let left = c.word_in(SS, sp.wrapping_add(2)).wrapping_sub(1);
        c.run(0x9d63, if left == 0 { 6 } else { 5 })?;
        let ss = c.sreg(SS);
        c.push(ss);
        let ds = c.pop();
        c.set_sreg(DS, ds);
        let si = c.pop();
        c.set_reg(SI, si);
        c.pop();
        c.set_reg(CX, left);
        if left == 0 {
            return c.ret();
        }
    }
}

/// `settings_ui_draw_language_buttons` (A42C): the language buttons, the
/// current one lit (sprite 1Ch + 2 × `sub_a465`), on the screen.
pub fn settings_ui_draw_language_buttons(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa42c)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(AX, 0x55);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0xa432)?;
    c.run(0xa432, 3)?;
    let language = c.byte(g::LANGUAGE);
    c.set_reg8(r8::AL, language);
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.push(fb);
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0xa43c)?;
    c.run(0xa43c, 2)?;
    let al = c.reg8(r8::AL);
    c.set_reg(AX, al as i8 as u16);
    c.call(code::SUB_A465, 0xa440)?;
    c.run(0xa440, 3)?;
    let ax = c.reg(AX);
    let ax = c.shl1(W, ax, 1);
    let al = c.add(B, ax & 0xff, 0x1c);
    c.set_reg(AX, ax & 0xff00 | al);
    c.call(code::DRAW_SPRITE_CLOBBERING_BX_DX, 0xa447)?;
    c.run(0xa447, 2)?;
    let fb = c.pop();
    c.set_word(g::FRAMEBUFFER_ACTIVE, fb);
    c.ret()
}

/// `settings_ui_draw_slider` (A502): the slider at SI (a byte value, then
/// its x): the track (sprite 1 at y 22h) and the knob (sprite 2) where the
/// value puts it; notes the knob's x (+3) and marks it drawn (+1). On the
/// screen, the framebuffer kept.
pub fn settings_ui_draw_slider(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa502)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.push(fb);
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0xa509)?;
    c.run(0xa509, 3)?;
    let si = c.reg(SI);
    c.push(si);
    c.set_reg(AX, 0x55);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0xa510)?;
    c.run(0xa510, 3)?;
    let si = c.reg(SI);
    let x = c.word(si.wrapping_add(2));
    c.set_reg(DX, x);
    c.set_reg(BX, 0x22);
    c.call(code::SETTINGS_UI_ADD_GLOBAL_OFFSET, 0xa519)?;
    c.run(0xa519, 2)?;
    c.set_reg(AX, 1);
    c.call(code::DRAW_SPRITE, 0xa51f)?;
    c.run(0xa51f, 13)?;
    let si = c.pop();
    c.set_reg(SI, si);
    c.lods(B, DS); // the value, in AL
    let si = c.reg(SI);
    c.set_byte(si, 1);
    let ax = !c.reg(AX);
    c.set_reg(AX, ax);
    let al = c.shr1(B, ax & 0xff, 2);
    let ax = al as u8 as i8 as u16;
    let bx = c.reg(BX);
    let x = c.add(W, ax, bx);
    c.set_reg(BX, x);
    let origin = c.word(g::SETTINGS_UI_ORIGIN_Y);
    let rel = c.sub(W, x, origin);
    c.set_reg(AX, rel);
    c.set_word(si.wrapping_add(3), rel);
    c.set_reg(AX, 2);
    c.call(code::DRAW_SPRITE_CLOBBERING_BX_DX, 0xa53c)?;
    c.run(0xa53c, 2)?;
    let fb = c.pop();
    c.set_word(g::FRAMEBUFFER_ACTIVE, fb);
    c.ret()
}

/// `settings_ui_draw_volume_sliders` (A4C6): the voice slider with PCM
/// (marked undrawn unless setting bit 4), and the two music sliders when
/// `sub_ae28` allows (both undrawn unless bit 400h).
pub fn settings_ui_draw_volume_sliders(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa4c6)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::CHECK_PCM_ENABLED, 0xa4c9)?;
    if c.flag(ZF) {
        c.run(0xa4c9, 2)?;
    } else {
        c.run(0xa4c9, 3)?;
        c.set_reg(SI, g::SLIDER_VOICE);
        c.call(code::SETTINGS_UI_DRAW_SLIDER, 0xa4d1)?;
        let s = c.word(g::SOUND_SETTINGS);
        c.run(0xa4d1, if s & 4 != 0 { 3 } else { 4 })?;
        c.test(W, s, 4);
        if s & 4 == 0 {
            c.set_byte(g::SLIDER_VOICE + 1, 0);
        }
    }
    c.call(code::SUB_AE28, 0xa4e1)?;
    if c.flag(ZF) {
        c.run(0xa4e1, 2)?;
        return c.ret();
    }
    c.run(0xa4e1, 3)?;
    c.set_reg(SI, g::SLIDER_MUSIC);
    c.call(code::SETTINGS_UI_DRAW_SLIDER, 0xa4e9)?;
    c.run(0xa4e9, 2)?;
    c.set_reg(SI, g::SLIDER_MUSIC_2);
    c.call(code::SETTINGS_UI_DRAW_SLIDER, 0xa4ef)?;
    let s = c.word(g::SOUND_SETTINGS);
    let on = s & 0x400 != 0;
    c.run(0xa4ef, if on { 3 } else { 5 })?;
    c.test(W, s, 0x400);
    if !on {
        c.set_byte(g::SLIDER_MUSIC + 1, 0);
        c.set_byte(g::SLIDER_MUSIC_2 + 1, 0);
    }
    c.ret()
}

/// From ADED: PCM driver function 4 (volume): AX = 190h, BL = the voice
/// volume (at least 4), BH = [28AEh].
fn pcm_volume(c: &mut Ctx) -> Result<(), Bail> {
    let bl = c.byte(g::SLIDER_MUSIC);
    let low = bl < 4;
    c.run(0xaded, if low { 6 } else { 5 })?;
    c.set_reg(AX, 0x190);
    c.set_reg8(r8::BL, bl);
    let bh = c.byte(g::BYTE_28AE);
    c.set_reg8(r8::BH, bh);
    c.cmp(B, bl as u16, 4);
    if low {
        c.set_reg8(r8::BL, 4);
    }
    c.interp(0xadff, 0xae03)?;
    c.run(0xae03, 1)?;
    c.ret()
}

/// `loc_0aded`.
pub fn sub_aded(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xaded)?;
    pcm_volume(&mut Ctx::new(m))
}

/// `lip_sync_stop` (A7A5): the lip-sync task removed, no lip-sync data,
/// `sub_d61d`; a voice still playing is stopped (closed, volume reset).
pub fn lip_sync_stop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa7a5)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(SI, code::LIP_SYNC_FRAME_TASK);
    c.call(code::REMOVE_FRAME_TASK, 0xa7ab)?;
    c.run(0xa7ab, 2)?;
    c.set_word(g::LIPSYNC_PTR, 0);
    c.call(code::SUB_D61D, 0xa7b4)?;
    c.run(0xa7b4, 1)?;
    c.call(code::IS_VOC_PCM_PLAYING, 0xa7b7)?;
    c.run(0xa7b7, 2)?;
    if c.flag(ZF) {
        return c.ret();
    }
    c.call(code::SET_VOC_PCM_IS_NOT_PLAYING, 0xa7bc)?;
    c.run(0xa7bc, 1)?;
    c.call(code::CLOSE_PCM_VOICE_FILE_HANDLE, 0xa7bf)?;
    c.run(0xa7bf, 1)?; // jmp ADEDh
    pcm_volume(&mut c)
}

/// `voc_get_lipsync_data` (A83F): with PCM: stops the voice, opens its
/// file (`open_pcm_voice_file`); if the VOC's first block is type 5 (a
/// text block: the lip-sync data), points [DC26h] at its data and skips
/// it; then `sub_a9b9`. CF set when opened.
pub fn voc_get_lipsync_data(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa83f)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_word(g::LIPSYNC_PTR, 0);
    c.call(code::CHECK_PCM_ENABLED, 0xa848)?;
    c.run(0xa848, 2)?;
    if c.flag(ZF) {
        return c.ret();
    }
    c.call(code::PCM_STOP_VOC, 0xa84d)?;
    c.run(0xa84d, 1)?;
    c.call(code::OPEN_PCM_VOICE_FILE, 0xa850)?;
    if c.flag(CF) {
        c.run(0xa850, 3)?; // cmc; jae; ret
        c.set_flag(CF, false);
        return c.ret();
    }
    let (buf, seg) = (c.word(g::VOC_BUFFER), c.word(g::VOC_BUFFER + 2));
    let mut di = buf.wrapping_add(0x1a);
    let kind = c.m.cpu.phys(seg, di);
    let text = c.m.hw.read8(kind) == 5;
    c.run(0xa850, 6 + if text { 6 } else { 0 } + 3)?;
    c.set_flag(CF, true);
    c.set_reg(DI, buf);
    c.set_sreg(ES, seg);
    di = c.add(W, buf, 0x1a);
    c.set_reg(DI, di);
    let kind = c.byte_in(ES, di);
    c.cmp(B, kind as u16, 5);
    if text {
        let len = c.word_in(ES, di.wrapping_add(1));
        c.set_reg(CX, len);
        di = c.add(W, di, 4);
        c.set_reg(DI, di);
        c.set_reg(AX, di);
        let data = c.add(W, di, 2);
        c.set_reg(AX, data);
        c.set_word(g::LIPSYNC_PTR, data);
        di = c.add(W, di, len);
        c.set_reg(DI, di);
    }
    c.set_word(g::VOC_BUFFER, di);
    let size = c.word(g::VOC_SIZE);
    let size = c.sub(W, size, di);
    c.set_word(g::VOC_SIZE, size);
    c.call(code::SUB_A9B9, 0xa87c)?;
    c.run(0xa87c, 2)?;
    c.set_flag(CF, true);
    c.ret()
}

/// `loc_0c0d5`: transition AL from the front buffer to the screen (driver
/// function 30, paced by the timer).
pub fn sub_c0d5(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc0d5)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let screen = c.word(g::SCREEN_BUFFER);
    c.set_sreg(ES, screen);
    let front = c.word(g::FRAMEBUFFER_FRONT);
    c.set_sreg(DS, front);
    c.set_reg(BP, g::TIMER);
    c.interp(0xc0e1, 0xc0e6)?;
    c.run(0xc0e6, 2)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}

/// `loc_0c85b`: the talking head's next zoom in 6000 (1770h) ticks from
/// now. AX = the timer.
pub fn sub_c85b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc85b)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let now = c.word(g::TIMER);
    c.set_reg(AX, now);
    c.set_word(g::HEAD_ZOOM_START, now);
    c.set_word(g::TIME_BETWEEN_FACE_ZOOMS, 0x1770);
    c.ret()
}
