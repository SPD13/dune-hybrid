//! More of the game screen: the date and time panel, the communication
//! device's animations, the ornithopter, the settings sliders, talking
//! heads without lip sync, lip sync start and stop.

use super::pure;
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
    pub const MUSIC_ENABLED: u16 = 0xae28;
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
    let ax = pure::and(W, step, 7);
    let mut al = ax;
    if !low {
        al = pure::sub(B, al, 8);
        al = pure::neg(B, al);
    }
    let glow = pure::add(B, al, 2);
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
    let zero = pure::xor(W, ax, ax);
    c.set_reg(AX, zero);
    c.call(code::DRAW_SPRITE_CLIPPED, 0x3aae)?;
    c.run(0x3aae, 4)?;
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let dx = pure::add(W, dx, 6);
    c.set_reg(DX, dx);
    let bx = pure::add(W, bx, 0x1e);
    c.set_reg(BX, bx);
    c.set_reg(AX, 1);
    c.call(code::DRAW_SPRITE_CLIPPED, 0x3aba)?;
    let speed = c.byte(g::ORNI_SPEED);
    let under = speed < 0x0f;
    let a = if under { 0 } else { speed - 0x0f };
    let capped = a > 5;
    c.run(0x3aba, 7 + under as u32 + 2 + capped as u32 + 2)?;
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let dx = pure::sub(W, dx, 2);
    c.set_reg(DX, dx);
    let bx = pure::add(W, bx, 0x14);
    c.set_reg(BX, bx);
    c.set_reg(AX, speed as u16);
    c.push(speed as u16);
    let mut al = pure::sub(B, speed as u16, 0x0f);
    if under {
        al = pure::xor(W, al, al);
    }
    if capped {
        al = 5;
    }
    let rotor = pure::add(B, al, 2);
    c.set_reg(AX, rotor);
    c.call(code::DRAW_SPRITE_CLIPPED, 0x3ad7)?;
    let sp = c.reg(SP);
    let saved = c.word_in(SS, sp) as u8;
    let capped = saved >= 0x0e;
    c.run(0x3ad7, 5 + capped as u32 + 2)?;
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let dx = pure::sub(W, dx, 0x55);
    c.set_reg(DX, dx);
    let bx = pure::sub(W, bx, 0x35);
    c.set_reg(BX, bx);
    let ax = c.pop();
    c.set_reg(AX, ax);
    let al = if capped { 0x0e } else { ax & 0xff };
    let shadow = pure::add(B, al, 8);
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
    let zero = pure::xor(W, cx, cx);
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
            c.set_reg8(r8::AH, 0);
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
        c.set_reg8(r8::AH, 0);
        if wide {
            c.set_reg8(r8::AH, 1);
            c.lods(B, DS);
        }
        let si = c.reg(SI);
        c.push(si);
        let ax = c.reg(AX);
        let ax = pure::sub(W, ax, 2);
        let index = pure::shl1(W, ax, 1);
        c.set_reg(AX, index);
        c.set_reg(BP, index);
        let table = c.word_in(SS, g::HEAD_FRAME_TABLE);
        let off = c.word_in(DS, index.wrapping_add(table));
        let si = pure::add(W, table, off);
        c.set_reg(SI, si);
        loop {
            let si = c.reg(SI);
            let b = c.byte_in(DS, si);
            if b == 0 {
                c.run(0x9c18, 5)?; // lodsb; or; je; pop si; jmp
                c.lods(B, DS);
                let si = c.pop();
                c.set_reg(SI, si);
                break;
            }
            c.run(0x9c18, 7)?;
            c.lods(B, DS);
            c.stos(B);
            c.movs(W);
            let cx = c.reg(CX);
            let cx = pure::inc(W, cx);
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
    lit_button(&mut c, 0xa432, 1, |c| {
        let language = c.byte(g::LANGUAGE);
        c.set_reg8(r8::AL, language);
    })
}

/// From A435: button AL lit (sprite 1Ch + 2 x the button), on the screen;
/// the stretch from `at` has `prefix` instructions before A435, done by
/// `ops`.
fn lit_button(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 2)?;
    ops(c);
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

/// `loc_0a44c`: the button of the setting at [28E7h] (+8) lit.
pub fn settings_ui_draw_second_buttons(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa44c)?;
    let mut c = Ctx::new(m);
    lit_button(&mut c, start, 3, |c| {
        let v = c.byte(SETTINGS_SECOND_BUTTON);
        let al = pure::add(B, v as u16, 8);
        c.set_reg8(r8::AL, al as u8);
    })
}

/// `loc_0a465`: the screen position (DX, BX) of button AL: x from the
/// table at 28DCh in steps of 7, the y and the base x at 28C7h; plus the
/// settings screen's origin. AX kept.
pub fn settings_ui_button_position(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa465)?;
    let mut c = Ctx::new(m);
    c.run(start, 9)?;
    let ax = c.reg(AX);
    c.push(ax);
    let y = c.word(BUTTONS_Y);
    c.set_reg(DX, y);
    let col = c.byte(BUTTON_COLUMNS.wrapping_add(ax & 0xff));
    c.set_reg(AX, ax & 0xff00 | col as u16);
    c.set_reg(BX, BUTTON_COLUMNS & 0xff00 | 7);
    c.mul8(7);
    let x = c.reg(AX);
    let base = c.word(BUTTONS_Y + 2);
    let x = pure::add(W, x, base);
    c.set_reg(BX, x);
    c.call(code::SETTINGS_UI_ADD_GLOBAL_OFFSET, 0xa47b)?;
    c.run(0xa47b, 2)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// The setting whose button [`settings_ui_draw_second_buttons`] lights.
const SETTINGS_SECOND_BUTTON: u16 = 0x28e7;
/// The buttons: their row's two words, and a column per button.
const BUTTONS_Y: u16 = 0x28c7;
const BUTTON_COLUMNS: u16 = 0x28dc;
/// The settings' records (8 bytes: value, shown, x, y, ...): three
/// sliders, three switches.
const SLIDERS: [u16; 3] = [0x288e, 0x2896, 0x289e];
const SWITCHES: [u16; 3] = [0x28a6, 0x28ae, 0x28b6];
/// What the cursor is on: 1 a slider, 2 a switch, 0 nothing.
const SETTINGS_HOVER: u16 = 0x28be;

/// From A685 (the stretch from `at`, `prefix` instructions before it, the
/// last `mov si, record`): whether the cursor (DX, BX, relative) is on the
/// setting `record`, shown, in a `w` x `h` box from its x, y; CF set when
/// it is (AX, BP the offsets in it). Returns.
fn on_setting(c: &mut Ctx, at: u16, prefix: u32, record: u16, w: u16, h: u16) -> Result<(), Bail> {
    let shown = c.byte(record.wrapping_add(1));
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let (x, y) = (c.word(record.wrapping_add(2)), c.word(record.wrapping_add(4)));
    let ax = dx.wrapping_sub(x);
    c.run(at, prefix + if shown < 1 { 4 } else if ax >= w { 10 } else { 11 })?;
    c.set_reg(SI, record);
    c.cmp(B, shown as u16, 1);
    let cf = c.flag(CF);
    c.set_flag(CF, !cf);
    if shown >= 1 {
        let ax = c.sub(W, dx, x);
        c.set_reg(AX, ax);
        let bp = c.sub(W, bx, y);
        c.set_reg(BP, bp);
        c.cmp(W, ax, w);
        if ax < w {
            c.cmp(W, bp, h);
        }
    }
    c.ret()
}

/// A672 / A69F: [`on_setting`] for each of three records until one is hit
/// (CF set). `test` is A685 or A6B2, the first two through it.
fn on_any(c: &mut Ctx, entry: u16, records: [u16; 3], test: u16, w: u16, h: u16) -> Result<(), Bail> {
    c.run(entry, 2)?;
    c.set_reg(SI, records[0]);
    c.call(test, entry + 6)?;
    if c.flag(CF) {
        c.run(entry + 6, 2)?;
        return c.ret();
    }
    c.run(entry + 6, 3)?;
    c.set_reg(SI, records[1]);
    c.call(test, entry + 0x0e)?;
    if c.flag(CF) {
        c.run(entry + 0x0e, 2)?;
        return c.ret();
    }
    on_setting(c, entry + 0x0e, 2, records[2], w, h)
}

/// `loc_0a685`: [`on_setting`] for the slider at SI (16h x 5).
pub fn settings_on_slider(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa685)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    on_setting(&mut c, start, 0, si, 0x16, 5)
}

/// `loc_0a6b2`: [`on_setting`] for the switch at SI (0Dh x 0Bh).
pub fn settings_on_switch(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa6b2)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    on_setting(&mut c, start, 0, si, 0x0d, 0x0b)
}

/// `loc_0a672`: whether the cursor is on a slider (SI = it).
pub fn settings_on_a_slider(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa672)?;
    on_any(&mut Ctx::new(m), start, SLIDERS, 0xa685, 0x16, 5)
}

/// `loc_0a69f`: whether the cursor is on a switch (SI = it).
pub fn settings_on_a_switch(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa69f)?;
    on_any(&mut Ctx::new(m), start, SWITCHES, 0xa6b2, 0x0d, 0x0b)
}

/// `loc_0a594`: what the cursor is on, at [`SETTINGS_HOVER`].
pub fn settings_hover(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa594)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xa672, 0xa597)?;
    if c.flag(CF) {
        c.run(0xa597, 3)?;
        c.set_byte(SETTINGS_HOVER, 1);
        return c.ret();
    }
    c.run(0xa597, 2)?;
    c.call(0xa69f, 0xa5a2)?;
    let on = c.flag(CF);
    c.run(0xa5a2, 3)?;
    c.set_byte(SETTINGS_HOVER, if on { 2 } else { 0 });
    c.ret()
}

/// `loc_0a637`: the music driver's volume (function at 39A5h) from the
/// first slider, FFh when music is off; AH = the first switch.
pub fn settings_apply_music_volume(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa637)?;
    let mut c = Ctx::new(m);
    let settings = c.word(g::SOUND_SETTINGS);
    let off = settings & 4 == 0;
    c.run(start, if off { 5 } else { 4 })?;
    c.test(W, settings, 4);
    if off {
        c.set_byte(SLIDERS[0], 0xff);
    }
    let (v, sw) = (c.byte(SLIDERS[0]), c.byte(SWITCHES[0]));
    c.set_reg(AX, (sw as u16) << 8 | v as u16);
    c.interp(0xa64b, 0xa64f)?;
    c.run(0xa64f, 1)?;
    c.ret()
}

/// `loc_0a650`: the sound driver's volume (3985h) from the second slider
/// (at least 4), both it and the third FFh when sound is off; AH = the
/// second switch.
pub fn settings_apply_sound_volume(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa650)?;
    let mut c = Ctx::new(m);
    let settings = c.word(g::SOUND_SETTINGS);
    let off = settings & 0x400 == 0;
    let v = if off { 0xff } else { c.byte(SLIDERS[1]) };
    c.run(start, 2 + if off { 3 } else { 0 } + 4 + (v < 4) as u32)?;
    c.test(W, settings, 0x400);
    if off {
        c.set_reg8(r8::AL, 0xff);
        c.set_byte(SLIDERS[1], 0xff);
        c.set_byte(SLIDERS[2], 0xff);
    }
    let sw = c.byte(SWITCHES[1]);
    c.set_reg(AX, (sw as u16) << 8 | v as u16);
    c.cmp(B, v as u16, 4);
    if v < 4 {
        c.set_reg8(r8::AL, 4);
    }
    c.interp(0xa66d, 0xa671)?;
    c.run(0xa671, 1)?;
    c.ret()
}

/// `loc_0ade0`: the voice volume (function at 397Dh): AX = 64h, BL = the
/// third slider (at least 4), BH = the third switch.
pub fn settings_apply_voice_volume(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xade0)?;
    let mut c = Ctx::new(m);
    let (v, sw) = (c.byte(SLIDERS[2]), c.byte(SWITCHES[2]));
    c.run(start, 6 + (v < 4) as u32)?;
    c.set_reg(AX, 0x64);
    c.set_reg(BX, (sw as u16) << 8 | v as u16);
    c.cmp(B, v as u16, 4);
    if v < 4 {
        c.set_reg8(r8::BL, 4);
    }
    c.interp(0xadff, 0xae03)?;
    c.run(0xae03, 1)?;
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
    let al = pure::shr1(B, ax & 0xff, 2);
    let ax = al as u8 as i8 as u16;
    let bx = c.reg(BX);
    let x = pure::add(W, ax, bx);
    c.set_reg(BX, x);
    let origin = c.word(g::SETTINGS_UI_ORIGIN_Y);
    let rel = pure::sub(W, x, origin);
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

/// `loc_0ac3a`: the three settings buttons of list 201Ah (SS) hidden
/// (bit 40h); with music on (`loc_0ae28`), shown again, CL = 1 or 2 by
/// the music settings' bit 0 (0 with bit 10h of [2943h]); CL = FFh with
/// music off.
pub fn settings_music_buttons(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xac3a)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let bp = 0x201au16;
    c.set_reg(BP, bp);
    for k in [3u16, 7, 0x0b] {
        let v = c.byte_in(SS, bp + k);
        let v = pure::or(B, v as u16, 0x40) as u8;
        c.m.cpu.write8(&mut c.m.hw, SS, bp + k, v);
    }
    c.set_reg8(r8::CL, 0xff);
    c.call(code::MUSIC_ENABLED, 0xac4e)?;
    if c.flag(ZF) {
        c.run(0xac4e, 2)?;
        return c.ret();
    }
    let f = c.byte(0x2943);
    let set = c.byte(g::MUSIC_SETTINGS);
    c.run(0xac4e, 7 + if f & 0x10 != 0 { 1 } else { 4 })?;
    let bp = c.reg(BP);
    for k in [3u16, 7, 0x0b] {
        let v = c.byte_in(SS, bp.wrapping_add(k));
        let v = pure::and(B, v as u16, 0xbf) as u8;
        c.m.cpu.write8(&mut c.m.hw, SS, bp.wrapping_add(k), v);
    }
    c.set_reg(CX, 0);
    if f & 0x10 == 0 {
        let cl = pure::and(B, set as u16, 1);
        let cx = pure::inc(W, cl);
        c.set_reg(CX, cx);
    }
    c.ret()
}

/// `loc_019df`: up to 5 sprites 2 in a row from (DX, BX), 4 apart, for
/// CL (DX, DI kept).
pub fn draw_marks(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x19df)?;
    let mut c = Ctx::new(m);
    let cl = c.reg8(r8::CL);
    if cl == 0 {
        c.run(start, 3)?;
        c.set_reg8(r8::CH, 0);
        return c.ret();
    }
    c.run(start, 6 + (cl > 5) as u32 + 3)?;
    c.set_reg8(r8::CH, 0);
    let (dx, di) = (c.reg(DX), c.reg(DI));
    c.push(dx);
    c.push(di);
    let n = cl.min(5) as u16;
    c.set_reg(CX, n);
    c.push(n);
    c.set_reg(AX, 2);
    c.call(code::DRAW_SPRITE, 0x19f3)?;
    loop {
        let sp = c.reg(SP);
        let left = c.word_in(SS, sp).wrapping_sub(1);
        c.run(0x19f3, 3 + if left == 0 { 3 } else { 3 })?;
        let cx = c.pop();
        let _ = cx;
        let dx = c.reg(DX);
        let dx = pure::add(W, dx, 4);
        c.set_reg(DX, dx);
        c.set_reg(CX, left);
        if left == 0 {
            let di = c.pop();
            c.set_reg(DI, di);
            let dx = c.pop();
            c.set_reg(DX, dx);
            return c.ret();
        }
        c.push(left);
        c.set_reg(AX, 2);
        c.call(code::DRAW_SPRITE, 0x19f3)?;
    }
}

/// `loc_01a0f`: unless [1AFEh] is set, the date panel's frame (UI element
/// 1AF4h) and the date drawn and copied to the screen (rect 1F06h), the
/// cursor restored first; the sprite sheet [2784h] opened again.
pub fn redraw_date_panel(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1a0f)?;
    let mut c = Ctx::new(m);
    let v = c.word(0x1afe);
    c.run(start, 3)?;
    if v != 0 {
        return c.ret();
    }
    c.call(0xdbb2, 0x1a19)?;
    c.run(0x1a19, 2)?;
    let sheet = c.word(0x2784);
    c.push(sheet);
    c.call(0xc137, 0x1a20)?;
    c.run(0x1a20, 2)?;
    c.set_reg(SI, 0x1af4);
    c.call(0xd200, 0x1a26)?;
    c.run(0x1a26, 1)?;
    c.call(0x1a34, 0x1a29)?;
    c.run(0x1a29, 2)?;
    c.set_reg(SI, 0x1f06);
    c.call(0xc4aa, 0x1a2f)?;
    c.run(0x1a2f, 2)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0x1a33)?;
    c.run(0x1a33, 1)?;
    c.ret()
}

/// `loc_015a8`: [11EBh]'s phrase drawn at (0Ah, 9Dh) with the glyphs of
/// `font_draw_glyph_func`, in colour CX.
pub fn draw_phrase_11eb(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x15a8)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xd08f, 0x15ab)?;
    c.run(0x15ab, 4)?;
    let p = c.word(0x11eb);
    c.set_reg(AX, p);
    c.set_reg(DX, 0x0a);
    c.set_reg(BX, 0x9d);
    super::text::font_draw_string_with_color_at_pos(c.m, 0xd194)
}

/// `loc_018ba`: leaving a place: three UI elements' words cleared, the
/// room's frame task removed, the voice stopped (PCM function 5), and
/// more (`loc_04d00`, the screens left, [11CAh] set, `loc_098e6`);
/// `loc_00b21` when [2Bh] is set.
pub fn leave_place(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x18ba)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    for at in [0x1c06, 0x1c14, 0x1c22] {
        c.set_word(at, 0);
    }
    c.call(0x39e6, 0x18cf)?;
    let mut at = 0x18cf;
    for target in [0xac30, 0x4d00, 0xd2bd, 0x4aca, 0x98e6] {
        c.run(at, 1)?;
        c.call(target, at + 3)?;
        at += 3;
    }
    let v = c.byte(0x2b);
    c.run(0x18de, if v == 0 { 4 } else { 4 })?;
    c.set_byte(0x46df, 0);
    c.cmp(B, v as u16, 0);
    if v == 0 {
        return c.ret();
    }
    c.call(0x0b21, 0x18ed)?;
    c.run(0x18ed, 1)?;
    c.ret()
}

/// `loc_00d0d`: with ZF (from the caller) or BL = 0Ah, sprite 37h (36h for
/// BL = 0Ah) of the scene's resource (`open_onmap_resource`) and sprite
/// 35h, their palettes to the driver (functions at 38BDh and 394Dh);
/// else the driver's function at 3951h. BX = 180h, CX = 54h.
pub fn scene_picture(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0d0d)?;
    let mut c = Ctx::new(m);
    let zf = c.flag(pc::cpu::ZF);
    let bl = c.reg8(r8::BL);
    let picture = zf || bl == 0x0a;
    c.run(start, if zf { 6 } else if picture { 9 } else { 8 })?;
    c.set_reg8(r8::AL, bl);
    c.set_reg(BX, 0x180);
    c.set_reg(CX, 0x54);
    c.set_reg8(r8::DL, 0x37);
    if !zf {
        let dx = c.reg(DX);
        let dx = c.dec(W, dx);
        c.set_reg(DX, dx);
        c.cmp(B, bl as u16, 0x0a);
    }
    if !picture {
        c.interp(0x0d1e, 0x0d22)?;
        c.run(0x0d22, 1)?;
        return c.ret();
    }
    c.call(0xc13b, 0x0d26)?;
    c.run(0x0d26, 3)?;
    let dl = c.reg8(r8::DL);
    c.set_reg(AX, dl as u16);
    c.xor(B, 0, 0);
    c.call(0xc1f4, 0x0d2d)?;
    c.run(0x0d2d, 1)?;
    let si = c.reg(SI);
    c.set_reg(DX, si.wrapping_add(6));
    c.interp(0x0d30, 0x0d34)?;
    c.run(0x0d34, 1)?;
    c.call(0xc0f4, 0x0d37)?;
    c.run(0x0d37, 2)?;
    c.set_reg(AX, 0x35);
    c.call(0xc1f4, 0x0d3d)?;
    c.run(0x0d3d, 1)?;
    let si = c.reg(SI);
    c.set_reg(DX, si.wrapping_add(6));
    c.interp(0x0d40, 0x0d44)?;
    c.run(0x0d44, 1)?;
    c.ret()
}

/// `settings_ui_draw`: the settings panel drawn on the screen (sprite 0
/// of resource 55h at [2886h]; volume sliders, counters, language and
/// other buttons, music buttons), then its handlers (BX A541h) and the
/// fade ([`unwind_and_fade`](super::screens::unwind_and_fade)).
pub fn settings_ui_draw(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa3f9)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let fb = c.word(0xdbda);
    c.push(fb);
    c.call(0xc08e, 0xa400)?;
    c.run(0xa400, 2)?;
    c.set_reg(AX, 0x55);
    c.call(0xc13e, 0xa406)?;
    c.run(0xa406, 4)?;
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    let (dx, bx) = (c.word(0x2886), c.word(0x2888));
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.call(0xc22f, 0xa413)?;
    c.run(0xa413, 2)?;
    let fb = c.pop();
    c.set_word(0xdbda, fb);
    c.call(0xa4c6, 0xa41a)?;
    for (at, to) in [(0xa41au16, 0xa47du16), (0xa41d, 0xa42c), (0xa420, 0xa44c), (0xa423, 0xac3a)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    c.run(0xa426, 2)?;
    c.set_reg(BX, 0xa541);
    super::screens::unwind_and_fade(c.m, 0xd32f)
}

/// `loc_0a576`: a click in the settings panel ([2886h]; outside it the
/// panel is left, `leave_and_fade`): on the voice test box (28BFh) a test
/// voice (4 / 5) played when PCM is on; on the language row (28C7h) the
/// language for the column (table 28CFh by (BX - top) / 7): above 7 the
/// mode [28E7h] = it - 8, below 7 a new language [0CEEBh] (bit 1 of
/// [28E7h] cleared, its resources reloaded), and the panel redrawn;
/// else [28BEh] = 1 on a slider, 2 on a switch, 0.
pub fn settings_click(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa576)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(DI, 0x2886);
    c.call(0xd6fe, 0xa57c)?;
    if !c.flag(CF) {
        c.run(0xa57c, 2)?;
        return super::screens::leave_and_fade(c.m, 0xd2e2);
    }
    c.run(0xa57c, 2)?;
    c.call(0xa453, 0xa584)?;
    c.run(0xa584, 2)?;
    c.set_reg(DI, 0x28bf);
    c.call(0xd6fe, 0xa58a)?;
    if c.flag(CF) {
        c.run(0xa58a, 2)?;
        c.call(0xae2f, 0xa556)?;
        if c.flag(ZF) {
            c.run(0xa556, 2)?;
            return c.ret();
        }
        c.run(0xa556, 4)?;
        c.set_reg(AX, 4);
        c.set_reg(BX, 5);
        c.call(0xa8bc, 0xa561)?;
        c.run(0xa561, 1)?;
        c.call(0xa83f, 0xa564)?;
        c.run(0xa564, 1)?;
        c.call(0xade0, 0xa567)?;
        c.run(0xa567, 3)?;
        c.set_byte(0xdc2b, 1);
        c.set_reg(SI, 0x3811);
        let to = c.word(0x3991);
        c.call(to, 0xa573)?;
        c.run(0xa573, 1)?;
        return super::voice::wait_voice_done(c.m, 0xaba9);
    }
    c.run(0xa58a, 3)?;
    c.set_reg(DI, 0x28c7);
    c.call(0xd6fe, 0xa592)?;
    if !c.flag(CF) {
        c.run(0xa592, 2)?;
        c.call(0xa672, 0xa597)?;
        if c.flag(CF) {
            c.run(0xa597, 3)?;
            c.set_byte(0x28be, 1);
            return c.ret();
        }
        c.run(0xa597, 2)?;
        c.call(0xa69f, 0xa5a2)?;
        let v = if c.flag(CF) { 2 } else { 0 };
        c.run(0xa5a2, 3)?;
        c.set_byte(0x28be, v);
        return c.ret();
    }
    // The language row.
    let (bx, di) = (c.reg(BX), c.reg(DI));
    let top = c.word(di.wrapping_add(2));
    let ax = bx.wrapping_sub(top);
    if ax / 7 > 0xff {
        c.m.cpu.ip = 0xa592;
        return Err(Bail);
    }
    let q = (ax / 7) as u8;
    let al = c.byte(0x28cfu16.wrapping_add(q as u16));
    let lang = c.byte(0xceeb);
    let n = 1 + 8 + if al > 7 {
        5
    } else if al == 7 {
        2
    } else if al == lang {
        3
    } else {
        5
    };
    c.run(0xa592, n)?;
    let ax = c.sub(W, bx, top);
    c.set_reg(AX, ax);
    c.set_reg8(r8::BL, 7);
    c.m.cpu.mul_div(&mut c.m.hw, B, 6, 7); // div bl
    c.set_reg(BX, 0x28cf);
    c.set_reg8(r8::AL, al);
    c.cmp(B, al as u16, 7);
    if al == 7 {
        return c.ret();
    }
    if al > 7 {
        let m8 = c.sub(B, al as u16, 8);
        c.set_reg8(r8::AL, m8 as u8);
        c.set_byte(0x28e7, m8 as u8);
        return settings_ui_draw(c.m, 0xa3f9);
    }
    c.cmp(B, al as u16, lang as u16);
    if al == lang {
        return c.ret();
    }
    let v = c.byte(0x28e7);
    let v = c.and(B, v as u16, 0xfd);
    c.set_byte(0x28e7, v as u8);
    c.set_byte(0xceeb, al);
    c.call(0xcfe4, 0xa5db)?;
    c.run(0xa5db, 1)?;
    settings_ui_draw(c.m, 0xa3f9)
}

/// `sub_a5df`: a drag in the settings panel: the offset and hovered item
/// (`settings_ui_sub_global_offset`, BX - CX, `settings_hover`). In mode
/// 2 ([28BEh]) the counter SI steps by 0Ah (down by the drag's sign:
/// CX flipped below item 6, DI from item 5) unless it would reach F1h,
/// then is redrawn (`loc_0a49c`); in mode 1 a drag sets the slider to
/// ~((x - 22h) * 4) within 40h and redraws it
/// (`settings_ui_draw_slider`). Both return to the item's handler
/// (+6, pushed).
pub fn settings_drag(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa5df)?;
    let mut c = Ctx::new(m);
    c.run(0xa5df, 1)?;
    c.call(0xa453, 0xa5e2)?;
    c.run(0xa5e2, 2)?;
    let (bx, cx) = (c.reg(BX), c.reg(CX));
    let b = pure::sub(W, bx, cx);
    c.set_reg(BX, b);
    c.call(0xa594, 0xa5e7)?;
    let mode = c.byte(0x28be);
    let si = c.reg(SI);
    let cx = c.reg(CX);
    if mode == 1 {
        if cx == 0 {
            c.run(0xa5e7, 4)?;
            return c.ret();
        }
        let x = c.word(si.wrapping_add(4));
        let a = x.wrapping_add(cx).wrapping_sub(0x22);
        if a >= 0x40 {
            c.run(0xa5e7, 2 + 1 + 5 + 1)?;
        } else {
            c.run(0xa5e7, 2 + 1 + 5 + 6)?;
        }
        c.set_reg(AX, x);
        let v = pure::add(W, x, cx);
        let v = pure::sub(W, v, 0x22);
        c.set_reg(AX, v);
        if a >= 0x40 {
            return c.ret();
        }
        let v = pure::shl1(W, v, 2);
        let v = !v;
        c.set_reg(AX, v);
        c.set_byte(si, v as u8);
        let h = c.word(si.wrapping_add(6));
        c.push(h);
        return c.jump(0xa502);
    }
    if mode != 2 {
        c.run(0xa5e7, 5)?;
        return c.ret();
    }
    let (ax, bp, di) = (c.reg(AX), c.reg(BP), c.reg(DI));
    let cx2 = if ax < 6 { cx.wrapping_neg() } else { cx };
    let di2 = if bp >= 5 { di.wrapping_neg() } else { di };
    let s = cx2.wrapping_add(di2);
    let al = if s & 0x8000 != 0 { 0xf6u8 } else { 0x0a };
    let cur = c.byte(si);
    let v = al.wrapping_add(cur);
    let ok = v < 0xf1;
    c.run(
        0xa5e7,
        4 + 2 + (ax < 6) as u32 + 2 + (bp >= 5) as u32 + 3 + (s & 0x8000 != 0) as u32 + 3
            + if ok { 3 } else { 1 },
    )?;
    if ax < 6 {
        c.set_reg(CX, cx2);
    }
    if bp >= 5 {
        c.set_reg(DI, di2);
    }
    c.set_reg8(r8::AL, 0x0a);
    c.set_reg(CX, s);
    if s & 0x8000 != 0 {
        c.set_reg8(r8::AL, 0xf6);
    }
    c.set_reg8(r8::AL, v);
    if !ok {
        return c.ret();
    }
    c.set_byte(si, v);
    let h = c.word(si.wrapping_add(6));
    c.push(h);
    c.jump(0xa49c)
}
