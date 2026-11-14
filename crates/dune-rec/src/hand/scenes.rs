//! Scenes and screens: the book's page turn, the end-of-chapter video,
//! the stats screen, the result bars, map statistics, the room's frame
//! task, the IRUL videos (with subtitles), the font glyph routines, the
//! pause key.

use super::pure;
use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DS, DX, ES, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const FIND_LAST_NUMERIC_DIGIT: u16 = 0xd03c;
    pub const STRING_REPLACE_NUMBER: u16 = 0xe2e3;
    pub const STRING_REPLACE_5_DIGITS: u16 = 0xe31c;
    pub const SET_SCREEN_AS_ACTIVE_FRAMEBUFFER: u16 = 0xc08e;
    pub const LOAD_ICONES_SPRITES: u16 = 0xc137;
    pub const DRAW_SPRITE: u16 = 0xc2fd;
    pub const DRAW_SPRITE_CLOBBERING_BX_DX: u16 = 0xc22f;
    pub const WAIT_A_BIT: u16 = 0xe387;
    pub const DRAW_UI_ELEMENTS_LIST: u16 = 0xd1f2;
    pub const MIDI_FUNC_2_0: u16 = 0xaeb7;
    pub const TRANSITION: u16 = 0xc108;
    pub const KB_CLEAR_SCANCODE: u16 = 0xde4e;
    pub const HNM_DO_FRAME_SKIPPABLE: u16 = 0xc9e8;
    pub const CHECK_IF_HNM_COMPLETE: u16 = 0xcc85;
    pub const HNM_CLOSE_RESOURCE: u16 = 0xca01;
    pub const PCM_STOP_VOC: u16 = 0xac14;
    pub const SET_FB1_AS_ACTIVE_FRAMEBUFFER: u16 = 0xc07c;
    pub const COPY_ACTIVE_FRAMEBUFFER_TO_FRAMEBUFFER_2: u16 = 0xc412;
    pub const GFX_CLEAR_ACTIVE_FRAMEBUFFER: u16 = 0xc0ad;
    pub const GET_INGAME_DAY: u16 = 0x1ad1;
    pub const FIND_STRING_AND_REPLACE_DIGITS: u16 = 0xe2db;
    pub const STRING_UPDATE_ORDINAL_SUFFIX: u16 = 0xbfa7;
    pub const FONT_SELECT_TALL_FONT: u16 = 0xd068;
    pub const RESTORE_MOUSE_IF_RECT_INTERSECTS: u16 = 0xdb74;
    pub const FONT_SET_DRAW_POSITION: u16 = 0xd04e;
    pub const FONT_DRAW_GLYPH_SMALL: u16 = 0xd12f;
    pub const SUB_D41B: u16 = 0xd41b;
    pub const SUB_C0D5: u16 = 0xc0d5;
    pub const OPEN_RESOURCE_BY_INDEX: u16 = 0xc13e;
    pub const IRUL_SUBTITLE: u16 = 0xcf4b;
    pub const PLAY_MUSIC_MORNING: u16 = 0xad57;
    pub const FONT_GET_DRAW_POSITION: u16 = 0xd05f;
}

/// `ui_draw_book_turning_page` (B1AF): the page turning (sprites 0Ah,
/// 0Bh of the icons, the order by DX's sign) at (1Bh, 9Eh), 10 ticks each,
/// then the book's element; on the screen. DX kept.
pub fn ui_draw_book_turning_page(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb1af)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let dx = c.reg(DX);
    c.push(dx);
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0xb1b3)?;
    c.run(0xb1b3, 1)?;
    c.call(code::LOAD_ICONES_SPRITES, 0xb1b6)?;
    let dx = c.reg(DX);
    let back = dx & 0x8000 != 0;
    c.run(0xb1b6, 3 + if back { 0 } else { 1 } + 4)?;
    c.set_reg(AX, 0x0b);
    c.or(W, dx, dx);
    let mut page = 0x0b;
    if !back {
        page = c.dec(W, 0x0b);
    }
    c.set_reg(AX, page);
    c.set_reg(BX, 0x9e);
    c.set_reg(DX, 0x1b);
    c.push(page);
    c.call(code::DRAW_SPRITE, 0xb1c8)?;
    c.run(0xb1c8, 2)?;
    c.set_reg(AX, 0x0a);
    c.call(code::WAIT_A_BIT, 0xb1ce)?;
    let next_is_b = page + 1 == 0x0b;
    c.run(0xb1ce, 4 + if next_is_b { 0 } else { 1 } + 1)?;
    let page = c.pop();
    let next = c.inc(W, page);
    c.cmp(W, next, 0x0b);
    let other = if next_is_b { next } else { c.sub(W, next, 2) };
    c.set_reg(AX, other);
    c.call(code::DRAW_SPRITE_CLOBBERING_BX_DX, 0xb1db)?;
    c.run(0xb1db, 2)?;
    c.set_reg(AX, 0x0a);
    c.call(code::WAIT_A_BIT, 0xb1e1)?;
    c.run(0xb1e1, 3)?;
    c.set_reg(SI, 0x1af4);
    c.set_reg(CX, 1);
    c.call(code::DRAW_UI_ELEMENTS_LIST, 0xb1ea)?;
    c.run(0xb1ea, 2)?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `sub_b1ee`: the chapter video: palette 2 to 1, MIDI function 2, a
/// voice flag, transition 34h to B236h (which loads it), then plays it to
/// its end or a key (no pause meanwhile); closes it, stops the voice,
/// palette function 37, transition 34h to B23Fh, the frame copied.
pub fn sub_b1ee(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb1ee)?;
    let mut c = Ctx::new(m);
    c.interp(0xb1ee, 0xb1f2)?;
    c.run(0xb1f2, 1)?;
    c.call(code::MIDI_FUNC_2_0, 0xb1f5)?;
    c.run(0xb1f5, 4)?;
    c.set_byte(g::VOC_PCM_PLAYING, 1);
    c.set_reg8(r8::AL, 0x34);
    c.set_reg(BP, 0xb236);
    c.call(code::TRANSITION, 0xb202)?;
    c.run(0xb202, 1)?;
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0xb205)?;
    c.run(0xb205, 2)?;
    c.set_byte(g::PAUSE_ALLOWED, 0);
    c.call(code::KB_CLEAR_SCANCODE, 0xb20d)?;
    c.run(0xb20d, 1)?;
    c.call(code::HNM_DO_FRAME_SKIPPABLE, 0xb210)?;
    loop {
        if c.flag(CF) {
            c.run(0xb210, 2)?;
            break;
        }
        c.run(0xb210, 2)?;
        c.call(code::CHECK_IF_HNM_COMPLETE, 0xb215)?;
        c.run(0xb215, 2)?;
        if !c.flag(pc::cpu::ZF) {
            break;
        }
        c.call(code::HNM_DO_FRAME_SKIPPABLE, 0xb210)?;
    }
    c.call(code::HNM_CLOSE_RESOURCE, 0xb21a)?;
    c.run(0xb21a, 2)?;
    let p = c.byte(g::PAUSE_ALLOWED);
    let p = c.inc(B, p as u16) as u8;
    c.set_byte(g::PAUSE_ALLOWED, p);
    c.call(code::PCM_STOP_VOC, 0xb221)?;
    c.run(0xb221, 1)?;
    c.call(code::SET_FB1_AS_ACTIVE_FRAMEBUFFER, 0xb224)?;
    c.interp(0xb224, 0xb228)?;
    c.run(0xb228, 3)?;
    c.set_reg8(r8::AL, 0x34);
    c.set_reg(BP, 0xb23f);
    c.call(code::TRANSITION, 0xb230)?;
    c.run(0xb230, 1)?;
    c.call(code::COPY_ACTIVE_FRAMEBUFFER_TO_FRAMEBUFFER_2, 0xb233)?;
    c.run(0xb233, 1)?; // jmp ABC6h
    super::sound::set_voc_pcm_is_not_playing(c.m, 0xabc6)
}

/// `sub_b236`: a cleared screen, then video [243Eh] opened at its first
/// frame.
pub fn sub_b236(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb236)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::GFX_CLEAR_ACTIVE_FRAMEBUFFER, 0xb239)?;
    c.run(0xb239, 2)?;
    let video = c.word(g::CHAPTER_VIDEO);
    c.set_reg(AX, video);
    super::hnmload::load_first_frame(&mut c)
}

/// `ui_stats_draw_ingame_day_and_charisma` (BDFA): the day (with its
/// ordinal suffix) into string C2h, charisma / 2 into C3h, then the stats
/// list (2482h) in the tall font.
pub fn ui_stats_draw_ingame_day_and_charisma(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbdfa)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(SI, 0xc2);
    c.call(code::GET_INGAME_DAY, 0xbe00)?;
    c.run(0xbe00, 2)?;
    let ax = c.reg(AX);
    let ax = pure::inc(W, ax);
    c.set_reg(AX, ax);
    c.call(code::FIND_STRING_AND_REPLACE_DIGITS, 0xbe04)?;
    c.run(0xbe04, 1)?;
    c.call(code::STRING_UPDATE_ORDINAL_SUFFIX, 0xbe07)?;
    c.run(0xbe07, 5)?;
    let charisma = c.byte(g::CHARISMA);
    c.set_reg(AX, charisma as u16);
    let half = pure::shr1(W, charisma as u16, 1);
    c.set_reg(AX, half);
    c.set_reg(SI, 0xc3);
    c.call(code::FIND_STRING_AND_REPLACE_DIGITS, 0xbe14)?;
    c.run(0xbe14, 1)?;
    c.call(code::FONT_SELECT_TALL_FONT, 0xbe17)?;
    c.run(0xbe17, 2)?;
    c.set_reg(SI, 0x2482);
    super::text::font_draw_phrase_list(c.m, 0xd1a6)
}

/// `sub_be57`: the result bars (a globe frame task): each of the 6 bars at
/// DD11h moves one step toward its target (at +6, up to 1Eh high), drawn
/// at its place (24EEh) with sprites 37h/38h and 39h on top; a bar at its
/// target gets its letter (116Ah, small font, colour 3Fh or 25h). On the
/// screen.
pub fn sub_be57(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbe57)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0xbe5a)?;
    c.run(0xbe5a, 2)?;
    c.set_reg(SI, g::GAME_AREA_RECT);
    c.call(code::RESTORE_MOUSE_IF_RECT_INTERSECTS, 0xbe60)?;
    c.run(0xbe60, 1)?;
    c.call(code::LOAD_ICONES_SPRITES, 0xbe63)?;
    c.run(0xbe63, 2)?;
    c.set_reg(SI, g::RESULT_BARS);
    let bp = c.reg(BP);
    let zero = c.xor(W, bp, bp);
    c.set_reg(BP, zero);
    loop {
        let (si, bp) = (c.reg(SI), c.reg(BP));
        let (target, current) = (c.byte(si), c.byte(si + 6));
        let last = bp + 1 == 6;
        let tail = 4 + if last { 1 } else { 0 };
        if target == current {
            c.run(0xbe68, 3 + tail)?;
            let d = c.sub(B, target as u16, current as u16);
            c.set_reg8(r8::CL, d as u8);
        } else {
            let up = target >= current;
            let now = if up { current.wrapping_add(1) } else { current.wrapping_sub(1) };
            let capped = now >= 0x1e;
            c.run(0xbe68, 5 + !up as u32 + 4 + capped as u32 + 13)?;
            let d = c.sub(B, target as u16, current as u16);
            c.set_reg8(r8::CL, d as u8);
            let mut al = 1u8;
            c.set_reg8(r8::AL, 1);
            if !up {
                al = c.neg(B, 1) as u8;
                c.set_reg8(r8::AL, al);
            }
            let v = c.add(B, current as u16, al as u16) as u8;
            c.set_byte(si + 6, v);
            c.set_reg8(r8::AL, v);
            c.cmp(B, v as u16, 0x1e);
            let h = if capped { 0x1e } else { v };
            c.set_reg8(r8::AL, h);
            let slot = c.shl1(W, bp, 2);
            c.set_reg(BX, slot);
            let x = c.word(slot.wrapping_add(0x24ee));
            let base = c.word(slot.wrapping_add(0x24f0));
            c.set_reg(DX, x);
            c.xor(B, 0, 0);
            let y = c.sub(W, base, h as u16);
            c.set_reg(BX, y);
            c.push(si);
            c.push(bp);
            c.set_reg(AX, 0x37);
            let half = c.shr1(W, bp, 1);
            c.set_reg(BP, half);
            let sprite = c.adc(W, 0x37, 0);
            c.set_reg(AX, sprite);
            c.call(code::DRAW_SPRITE, 0xbea0)?;
            c.run(0xbea0, 3)?;
            let bx = c.reg(BX);
            let bx = c.sub(W, bx, 0x0a);
            c.set_reg(BX, bx);
            c.set_reg(AX, 0x39);
            c.call(code::DRAW_SPRITE, 0xbea9)?;
            let sp = c.reg(pc::cpu::SP);
            let si2 = c.word_in(SS, sp.wrapping_add(2));
            let reached = c.byte(si2) == c.byte(si2 + 6);
            let odd = bp & 1 != 0;
            c.run(0xbea9, 5 + if reached { 3 + odd as u32 + 3 } else { tail })?;
            let bp = c.pop();
            c.set_reg(BP, bp);
            let si = c.pop();
            c.set_reg(SI, si);
            let t = c.byte(si);
            c.set_reg8(r8::AL, t);
            let cur = c.byte(si + 6);
            c.cmp(B, t as u16, cur as u16);
            if reached {
                c.test(W, bp, 1);
                c.set_reg(AX, if odd { 0x25 } else { 0x3f });
                let colour = c.reg(AX);
                c.set_word(g::TEXT_COLOUR, colour);
                let dx = c.reg(DX);
                let dx = c.add(W, dx, 4);
                c.set_reg(DX, dx);
                c.call(code::FONT_SET_DRAW_POSITION, 0xbec6)?;
                let bp = c.reg(BP);
                let letter = c.byte_in(SS, bp.wrapping_add(0x116a));
                c.run(0xbec6, 2)?;
                c.set_reg8(r8::AL, letter);
                c.call(code::FONT_DRAW_GLYPH_SMALL, 0xbecd)?;
                let last = c.reg(BP) + 1 == 6;
                c.run(0xbecd, 4 + if last { 1 } else { 0 })?;
            }
        }
        // BECD: the next bar.
        let si = c.reg(SI);
        let si = c.inc(W, si);
        c.set_reg(SI, si);
        let bp = c.reg(BP);
        let bp = c.inc(W, bp);
        c.set_reg(BP, bp);
        c.cmp(W, bp, 6);
        if bp >= 6 {
            return super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c);
        }
    }
}

/// `map_func` (BFE3): statistics of the map (the C5F9h cells at
/// [DD00h]:0, by bits 4..5): [A2h] = 1 + 200 × the share of cells with
/// some, [A4h] = 200 × the share of those not fully (30h). Approximately:
/// the products are 32-bit, as the original computes them.
pub fn map_func(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbfe3)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let ds = c.sreg(DS);
    c.push(ds);
    for r in [BX, DX] {
        let v = c.reg(r);
        let zero = c.xor(W, v, v);
        c.set_reg(r, zero);
    }
    c.set_reg(CX, 0xc5f9);
    let si = c.reg(SI);
    let zero = c.xor(W, si, si);
    c.set_reg(SI, zero);
    let map = c.word(g::MAP_GFX_DATA + 2);
    c.set_sreg(DS, map);
    loop {
        let (si, cx) = (c.reg(SI), c.reg(CX));
        let kind = c.byte_in(DS, si) & 0x30;
        c.run(0xbff1, 3 + if kind == 0 { 0 } else { 3 + (kind != 0x30) as u32 } + 1)?;
        c.lods(B, DS);
        let al = c.and(B, kind as u16, 0x30);
        c.set_reg8(r8::AL, al as u8);
        if al != 0 {
            let dx = c.reg(DX);
            let dx = c.inc(W, dx);
            c.set_reg(DX, dx);
            c.cmp(B, al, 0x30);
            if al != 0x30 {
                let bx = c.reg(BX);
                let bx = c.inc(W, bx);
                c.set_reg(BX, bx);
            }
        }
        let left = cx.wrapping_sub(1);
        c.set_reg(CX, left);
        if left == 0 {
            break;
        }
    }
    let (dx, bx, si) = (c.reg(DX), c.reg(BX), c.reg(SI));
    let cells = si.wrapping_sub(0x188).wrapping_add(1);
    if cells == 0 || dx.wrapping_sub(bx) >= cells || bx >= cells {
        c.m.cpu.ip = 0xbffe; // a division would fault: the interpreter's
        return Err(Bail);
    }
    c.run(0xbffe, 21)?;
    let some = c.sub(W, dx, bx);
    c.set_reg(DX, some);
    c.set_reg(AX, 0);
    c.xor(W, 0, 0);
    let s = c.sub(W, si, 0x188);
    let cells = c.inc(W, s);
    c.set_reg(SI, cells);
    let share = |c: &mut Ctx| {
        c.m.cpu.mul_div(&mut c.m.hw, W, 6, cells as u32); // div si
        c.set_reg(DX, 0x64);
        c.mul16(0x64);
        let (ax, dx) = (c.reg(AX), c.reg(DX));
        let ax = c.add(W, ax, ax);
        c.set_reg(AX, ax);
        let dx = c.adc(W, dx, 0);
        c.set_reg(DX, dx);
    };
    share(&mut c);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.set_reg(BX, dx);
    c.set_reg(DX, bx);
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    share(&mut c);
    let dx = c.reg(DX);
    let dx = c.inc(W, dx);
    c.set_reg(DX, dx);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let bx = c.reg(BX);
    c.set_word(g::MAP_SHARE_A, dx);
    c.set_word(g::MAP_SHARE_B, bx);
    c.ret()
}

/// `room_frame_task` (C0B6): unless `sub_d41b` gives BP 2012h or 201Ah,
/// transition 0Ch; after its 18h steps, voice 4.
pub fn room_frame_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc0b6)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SUB_D41B, 0xc0b9)?;
    let bp = c.reg(BP);
    if bp == 0x2012 {
        c.run(0xc0b9, 3)?;
        c.cmp(W, bp, 0x2012);
        return c.ret();
    }
    if bp == 0x201a {
        c.run(0xc0b9, 5)?;
        c.cmp(W, bp, 0x2012);
        c.cmp(W, bp, 0x201a);
        return c.ret();
    }
    c.run(0xc0b9, 6)?;
    c.cmp(W, bp, 0x2012);
    c.cmp(W, bp, 0x201a);
    c.set_reg8(r8::AL, 0x0c);
    c.call(code::SUB_C0D5, 0xc0ca)?;
    let cx = c.reg(CX);
    if cx != 0x18 {
        c.run(0xc0ca, 3)?;
        c.cmp(W, cx, 0x18);
        return c.ret();
    }
    c.run(0xc0ca, 4)?;
    c.cmp(W, cx, 0x18);
    c.set_reg8(r8::AL, 4);
    super::audio::audio_start_voc(c.m, 0xab15)
}

/// `load_IRULn_HSQ` (CEFC): the subtitles for the language (resource 69h
/// + language), their list pointer at [3622h] to 35A8h, no vertical
/// offset, a cleared screen, then video 19h at its first frame.
pub fn load_irul_hsq(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcefc)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let language = c.byte(g::LANGUAGE);
    let r = c.add(B, 0x69, language as u16);
    c.set_reg(AX, r);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0xcf06)?;
    c.run(0xcf06, 2)?;
    c.set_word(g::SUBTITLE_NEXT, 0x35a8);
    let ax = c.reg(AX);
    let zero = c.xor(W, ax, ax);
    c.set_reg(AX, zero);
    c.interp(0xcf0e, 0xcf12)?;
    c.run(0xcf12, 1)?;
    c.call(code::GFX_CLEAR_ACTIVE_FRAMEBUFFER, 0xcf15)?;
    c.run(0xcf15, 2)?;
    c.set_reg(AX, 0x19);
    super::hnmload::load_first_frame(&mut c)
}

/// `play_IRULx_HSQ` (CF1B): plays the video on the screen, showing each
/// subtitle when its frame comes (`IRULx_draw_or_clear_subtitle`), to its
/// end or a key (CF set then); then closes it, stops the voice, the
/// morning music. The framebuffer kept.
pub fn play_irul_hsq(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcf1b)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.push(fb);
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0xcf22)?;
    loop {
        let next = c.word(g::SUBTITLE_NEXT);
        let at = c.word(next);
        let frame = c.word(g::HNM_FRAME);
        let due = at <= frame;
        c.run(0xcf22, 4 + 1)?;
        c.set_reg(SI, next);
        c.lods(W, DS);
        c.cmp(W, at, frame);
        if due {
            c.call(code::IRUL_SUBTITLE, 0xcf30)?;
            c.run(0xcf30, 1)?;
        }
        c.call(code::HNM_DO_FRAME_SKIPPABLE, 0xcf33)?;
        if c.flag(CF) {
            c.run(0xcf33, 3)?; // jb; pushf; call
            break;
        }
        c.run(0xcf33, 2)?;
        c.call(code::CHECK_IF_HNM_COMPLETE, 0xcf38)?;
        if c.flag(pc::cpu::ZF) {
            c.run(0xcf38, 1)?;
            continue;
        }
        c.run(0xcf38, 4)?; // je; clc; pushf; call
        c.set_flag(CF, false);
        break;
    }
    let flags = c.m.cpu.flags();
    c.push(flags);
    c.call(code::HNM_CLOSE_RESOURCE, 0xcf3f)?;
    c.run(0xcf3f, 1)?;
    c.call(code::PCM_STOP_VOC, 0xcf42)?;
    c.run(0xcf42, 1)?;
    c.call(code::PLAY_MUSIC_MORNING, 0xcf45)?;
    c.run(0xcf45, 3)?;
    let flags = c.pop();
    c.m.cpu.set_flags(flags);
    let fb = c.pop();
    c.set_word(g::FRAMEBUFFER_ACTIVE, fb);
    c.ret()
}

/// A glyph of a bitmap font (D096, D12F): character AL's rows (`rows` ×
/// AL bytes into the font, from [`table`]), its width from `widths`, at
/// the text position, which moves on; in the text colour, through driver
/// function 7. Registers kept.
fn glyph(c: &mut Ctx, entry: u16, rows: u16, table: u16, widths: u16, height: u8) -> Result<(), Bail> {
    c.run(entry, 18)?;
    let regs = [AX, BX, CX, DX, SI, pc::cpu::DI, BP];
    for r in regs {
        let v = c.reg(r);
        c.push(v);
    }
    let es = c.sreg(ES);
    c.push(es);
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    c.set_reg8(r8::AH, 0);
    let ch = c.reg(AX);
    let si = c.shl1(W, ch, 3);
    let si = if rows == 9 { c.add(W, si, ch) } else { c.sub(W, si, ch) };
    let base = c.word(table);
    let si = c.add(W, si, base);
    c.set_reg(SI, si);
    c.set_reg(BX, widths);
    let width = c.byte(widths.wrapping_add(ch));
    c.set_reg8(r8::AL, width);
    c.call(code::FONT_GET_DRAW_POSITION, entry + 0x1f)?;
    c.run(entry + 0x1f, 5)?;
    let ax = c.reg(AX);
    let x = c.word(g::FONT_X);
    let x = c.add(W, x, ax);
    c.set_word(g::FONT_X, x);
    c.set_reg8(r8::CL, ax as u8);
    c.set_reg8(r8::CH, height);
    let colour = c.word(g::TEXT_COLOUR);
    c.set_reg(AX, colour);
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.set_sreg(ES, fb);
    c.interp(entry + 0x2e, entry + 0x32)?;
    c.run(entry + 0x32, 9)?;
    let es = c.pop();
    c.set_sreg(ES, es);
    for r in regs.into_iter().rev() {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// `sub_d096`: the tall font's glyph routine (9 rows).
pub fn tall_font_glyph(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd096)?;
    glyph(&mut Ctx::new(m), 0xd096, 9, g::TALL_FONT_ROWS, 0xceec, 9)
}

/// `font_draw_glyph_func_small` (D12F): the small font's (7 rows).
pub fn small_font_glyph(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd12f)?;
    glyph(&mut Ctx::new(m), 0xd12f, 7, g::SMALL_FONT_ROWS, 0xcf6c, 7)
}

/// `pause_if_p_key_pressed` (DE7B): nothing unless P was pressed and the
/// pause is allowed. The pause itself (its screen and the wait for a key)
/// is left to the generated code: the port bails there.
pub fn pause_if_p_key_pressed(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xde7b)?;
    let mut c = Ctx::new(m);
    let p = c.byte(g::KEY_P_STATUS);
    if p == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let allowed = c.byte(g::PAUSE_ALLOWED);
    if allowed == 0 {
        c.run(start, 5)?;
        return c.ret();
    }
    Err(Bail)
}

/// From BF7D (the stretch from `at`, `prefix` instructions before, done by
/// `ops`): the stat (pushed, over DI) stored at [DI], and its change
/// marked at [BP] (SS): 2 up, 1 down; the same value marks 3 only in a new
/// 16-tick period (not the one at [`STATS_PERIOD`]). DI on by 2, BP by 1.
fn note_stat(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    let sp = c.reg(SP);
    let new = c.word_in(SS, sp);
    let di = c.word_in(SS, sp.wrapping_add(2));
    let old = c.word(di);
    let period = c.word(g::GAME_TIME) & 0xfff0;
    let saved = c.word(STATS_PERIOD);
    let n = if new == old {
        if saved == period { 14 } else { 17 }
    } else if new < old {
        13
    } else {
        14
    };
    c.run(at, prefix + n)?;
    ops(c);
    let new = c.pop();
    let di = c.pop();
    c.set_reg(DX, new);
    c.set_word(di, new);
    c.cmp(W, new, old);
    let mut ax = old & 0xff00 | 3;
    let mark = if new == old {
        let t = c.word(g::GAME_TIME);
        let t = c.and(W, t, 0xfff0);
        ax = t;
        c.cmp(W, saved, t);
        if saved == t {
            None
        } else {
            ax = t & 0xff00 | 3;
            Some(3)
        }
    } else if new < old {
        ax = ax & 0xff00 | 2;
        Some(2)
    } else {
        let v = c.dec(B, 2);
        ax = ax & 0xff00 | v;
        Some(1)
    };
    c.set_reg(AX, ax);
    let bp = c.reg(BP);
    if let Some(v) = mark {
        c.m.cpu.write8(&mut c.m.hw, SS, bp, v);
    }
    let di = c.add(W, di, 2);
    c.set_reg(DI, di);
    let bp = c.inc(W, bp);
    c.set_reg(BP, bp);
    c.ret()
}

/// The 16-tick period the stats were last compared in.
const STATS_PERIOD: u16 = 0x115c;

/// `loc_0bf73`: stat AX into the 3-digit number of the string at ES:SI
/// (`find_last_numeric_digit_in_str_at_es_si`, `string_replace_number_
/// ending_at_es_si`), then [`note_stat`].
pub fn stat_3_digits(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbf73)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let (di, ax) = (c.reg(DI), c.reg(AX));
    c.push(di);
    c.push(ax);
    c.call(code::FIND_LAST_NUMERIC_DIGIT, 0xbf78)?;
    c.run(0xbf78, 3)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.push(ax);
    c.call(code::STRING_REPLACE_NUMBER, 0xbf7d)?;
    note_stat(&mut c, 0xbf7d, 0, |_| {})
}

/// `loc_0bf61`: stat AX as 5 digits (one more place, a '0' first), then
/// [`note_stat`].
pub fn stat_5_digits(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbf61)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let (di, ax) = (c.reg(DI), c.reg(AX));
    c.push(di);
    c.push(ax);
    c.call(code::FIND_LAST_NUMERIC_DIGIT, 0xbf66)?;
    c.run(0xbf66, 5)?;
    let si = c.reg(SI);
    let si = c.dec(W, si);
    c.set_reg(SI, si);
    c.m.cpu.write8(&mut c.m.hw, pc::cpu::ES, si, 0x30);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.push(ax);
    c.call(code::STRING_REPLACE_5_DIGITS, 0xbf70)?;
    note_stat(&mut c, 0xbf70, 2, |c| {
        let si = c.reg(SI);
        let si = c.inc(W, si);
        c.set_reg(SI, si);
    })
}

/// `loc_0bd00`: `sub_c0d5` with AX = 0, CX = 0Ah on the rect at DD06h
/// grown by 1 on each side (back after).
pub fn grown_rect_c0d5(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbd00)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    c.set_reg(AX, 0);
    c.set_reg(CX, 0x0a);
    let r = 0xdd06u16;
    c.set_reg(SI, r);
    for (k, up) in [(0u16, true), (2, true), (4, false), (6, false)] {
        let v = c.word(r + k);
        let v = if up { pure::inc(W, v) } else { pure::dec(W, v) };
        c.set_word(r + k, v);
    }
    c.call(code::SUB_C0D5, 0xbd16)?;
    c.run(0xbd16, 6)?;
    c.set_reg(SI, r);
    for (k, up) in [(0u16, false), (2, false), (4, true), (6, true)] {
        let v = c.word(r + k);
        let v = if up { pure::inc(W, v) } else { pure::dec(W, v) };
        c.set_word(r + k, v);
    }
    c.ret()
}

/// `loc_0bf26`: the stats panel's numbers: phrase 0C4h fetched
/// (`get_phrase_or_command_string_si`), then [0A4h], [0A2h] with
/// [`stat_3_digits`] and [0A8h], [0A6h], [0ACh], [0AAh] with
/// [`stat_5_digits`] (the old values at 115Eh, the change marks at
/// 116Ah).
pub fn stats_numbers(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbf26)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_reg(DI, 0x115e);
    c.set_reg(BP, 0x116a);
    c.set_reg(SI, 0xc4);
    c.call(0xcf70, 0xbf32)?;
    for (at, src, to) in [
        (0xbf32u16, 0xa4u16, 0xbf73u16),
        (0xbf38, 0xa2, 0xbf73),
        (0xbf3e, 0xa8, 0xbf61),
        (0xbf44, 0xa6, 0xbf61),
        (0xbf4a, 0xac, 0xbf61),
    ] {
        c.run(at, 2)?;
        let v = c.word(src);
        c.set_reg(AX, v);
        c.call(to, at + 6)?;
    }
    c.run(0xbf50, 2)?;
    let v = c.word(0xaa);
    c.set_reg(AX, v);
    stat_5_digits(c.m, 0xbf61)
}
