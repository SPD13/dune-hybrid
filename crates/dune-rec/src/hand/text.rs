//! Text: the phrase and command strings, the font, drawing strings.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BX, CX, DS, DX, ES, SI, ZF},
    rec::Bail,
};

mod code {
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
    pub const OPEN_RESOURCE_BY_INDEX: u16 = 0xc13e;
    pub const GET_SUBRESOURCE_POINTER: u16 = 0xc1f4;
    pub const DRAW_SPRITE_CLOBBERING_BX_DX: u16 = 0xc22f;
    pub const FONT_SET_DRAW_POSITION: u16 = 0xd04e;
    pub const FONT_GET_DRAW_POSITION: u16 = 0xd05f;
    pub const LOAD_PHRASE_HSQ: u16 = 0xd00f;
    pub const CHECK_PCM_ENABLED: u16 = 0xae2f;
    pub const LOAD_RESOURCE_TO: u16 = 0xf0b9;
    pub const ADJUST_SUB_RESOURCE_POINTERS: u16 = 0x0098;
    /// The small font's glyph routine (`font_select_small_font`).
    pub const SMALL_FONT_DRAW: u16 = 0xd12f;
}

/// The font's sprite sheet.
const FONT_SHEET: u16 = 0x13;

/// String ids with bit 800h (after the decrement) are phrases (PHRASExx),
/// the others commands.
const PHRASE_BIT: u16 = 0x0800;

/// `get_phrase_or_command_string_si` (CF70): ES:SI = string SI (counted
/// from 1). Phrases load their PHRASE file first and note its last
/// offset at [47B4h]. BX kept; flags from the `shl`.
pub fn get_phrase_or_command_string_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcf70)?;
    let mut c = Ctx::new(m);
    let bx = c.reg(BX);
    let id = c.reg(SI).wrapping_sub(1);
    if id & PHRASE_BIT == 0 {
        c.run(start, 9)?;
        c.push(bx);
        c.dec(W, id.wrapping_add(1));
        c.test(W, id, PHRASE_BIT);
        let index = c.shl1(W, id, 1);
        let table = c.word(g::COMMAND_STRINGS);
        let seg = c.word(g::COMMAND_STRINGS + 2);
        c.set_sreg(ES, seg);
        let si = c.word_in(ES, table.wrapping_add(index));
        c.set_reg(SI, si);
        let bx = c.pop();
        c.set_reg(BX, bx);
        return c.ret();
    }
    c.run(start, 5)?;
    c.push(bx);
    c.dec(W, id.wrapping_add(1));
    c.set_reg(SI, id);
    c.test(W, id, PHRASE_BIT);
    c.call(code::LOAD_PHRASE_HSQ, 0xcf7b)?;
    c.run(0xcf7b, 9)?;
    let table = c.word(g::PHRASE_STRINGS);
    let seg = c.word(g::PHRASE_STRINGS + 2);
    c.set_sreg(ES, seg);
    let si = c.reg(SI);
    let index = c.and(W, si, 0x07ff);
    let index = c.shl1(W, index, 1);
    let si = c.word_in(ES, table.wrapping_add(index));
    c.set_reg(SI, si);
    let first = c.word_in(ES, table);
    let end = c.word_in(ES, first.wrapping_sub(2));
    c.set_reg(BX, end);
    c.set_word(g::PHRASE_END, end);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.ret()
}

/// `load_PHRASExx_HSQ` (D00F): loads the phrase file for the current part
/// of the game (PHRASE1x before [AAD6h], PHRASE2x after) in the current
/// language, unless it is the one loaded. Exit: AL = its resource; flags
/// from the `cmp` with the loaded one, or as the loading leaves them.
pub fn load_phrase_hsq(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd00f)?;
    let mut c = Ctx::new(m);
    let (part, split) = (c.word(g::PHRASE_PART), c.word(g::PHRASE_SPLIT));
    let first = part < split;
    let language = c.byte(g::LANGUAGE);
    let resource = (if first { 0x93u8 } else { 0x9a }).wrapping_add(language);
    let loaded = c.byte(g::PHRASE_LOADED);
    let n = 4 + !first as u32 + 3;
    if resource == loaded {
        c.run(start, n + 1)?;
    } else {
        c.run(start, n + 6)?;
    }
    c.set_reg(AX, part);
    c.cmp(W, part, split);
    c.set_reg8(r8::AL, if first { 0x93 } else { 0x9a });
    let al = c.add(B, if first { 0x93 } else { 0x9a }, language as u16) as u8;
    c.set_reg8(r8::AL, al);
    c.cmp(B, al as u16, loaded as u16);
    if resource == loaded {
        return c.ret();
    }
    let si = c.reg(SI);
    c.push(si);
    c.set_byte(g::PHRASE_LOADED, al);
    c.xor(B, 0, 0); // xor ah, ah
    c.set_reg(AX, al as u16);
    c.set_reg(SI, al as u16);
    let (di, es) = (c.word(g::PHRASE_STRINGS), c.word(g::PHRASE_STRINGS + 2));
    c.set_reg(pc::cpu::DI, di);
    c.set_sreg(ES, es);
    c.call(code::LOAD_RESOURCE_TO, 0xd035)?;
    c.run(0xd035, 2)?;
    let cx = c.reg(CX);
    c.push(cx);
    c.call(code::ADJUST_SUB_RESOURCE_POINTERS, 0xd039)?;
    c.run(0xd039, 3)?;
    let cx = c.pop();
    c.set_reg(CX, cx);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `check_amr_or_eng_language` (CFA0): with PCM voice, English and
/// American (languages 0 and 3) use voice language 2 ([28E7h], [28E8h]).
pub fn check_amr_or_eng_language(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcfa0)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::CHECK_PCM_ENABLED, 0xcfa3)?;
    if c.flag(ZF) {
        c.run(0xcfa3, 2)?;
        return c.ret();
    }
    let language = c.byte(g::LANGUAGE);
    let english = language == 0 || language == 3;
    c.run(0xcfa3, if language == 0 { 8 } else if english { 10 } else { 7 })?;
    c.set_reg8(r8::AL, language);
    c.or(B, language as u16, language as u16);
    if language != 0 {
        c.cmp(B, language as u16, 3);
    }
    if english {
        c.set_reg8(r8::AL, 2);
        c.set_byte(g::VOICE_LANGUAGE, 2);
        c.set_byte(g::VOICE_LANGUAGE + 1, 2);
    }
    c.ret()
}

/// `font_draw_glyph_func` (D16A): draws character AL at the text position
/// and moves it on by the glyph's width + 2. Registers kept (`save_regs`).
pub fn font_draw_glyph(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd16a)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SAVE_REGS, 0xd16d)?;
    c.run(0xd16d, 6)?;
    let es = c.sreg(ES);
    c.push(es);
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    let ch = c.reg8(r8::AL);
    let glyph = c.sub(B, ch as u16, 0x20);
    c.set_reg(AX, glyph);
    c.push(glyph);
    c.set_reg(AX, FONT_SHEET);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0xd179)?;
    c.run(0xd179, 3)?;
    let glyph = c.pop();
    c.set_reg(AX, glyph);
    c.push(glyph);
    c.call(code::GET_SUBRESOURCE_POINTER, 0xd17e)?;
    c.run(0xd17e, 4)?;
    c.lods(W, ES);
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    c.set_reg8(r8::AH, 0);
    let width = c.reg8(r8::AL);
    let advance = c.add(B, width as u16, 2);
    c.set_reg(AX, advance);
    c.call(code::FONT_GET_DRAW_POSITION, 0xd187)?;
    c.run(0xd187, 3)?;
    let x = c.word(g::FONT_X);
    let advance = c.reg(AX);
    let x = c.add(W, x, advance);
    c.set_word(g::FONT_X, x);
    let glyph = c.pop();
    c.set_reg(AX, glyph);
    c.call(code::DRAW_SPRITE_CLOBBERING_BX_DX, 0xd18f)?;
    c.run(0xd18f, 2)?;
    let es = c.pop();
    c.set_sreg(ES, es);
    c.call(code::RESTORE_REGS, 0xd193)?;
    c.run(0xd193, 1)?;
    c.ret()
}

/// `font_draw_string` (D1BB): draws the string at ES:SI up to FFh. 0Dh
/// starts a new line (0Ah lower, 7 in the small font); characters from
/// 80h up are drawn as '@'. Each character goes through the font's glyph
/// routine ([`g::FONT_DRAW`]). Exit: SI past the end; flags from the `cmp`.
pub fn font_draw_string(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd1bb)?;
    let mut c = Ctx::new(m);
    loop {
        let si = c.reg(SI);
        let ch = c.byte_in(ES, si);
        if ch == 0xff {
            c.run(0xd1bb, 4)?; // lodsb; cmp; je; ret
            c.lods(B, ES);
            c.cmp(B, 0xff, 0xff);
            return c.ret();
        }
        if ch == 0x0d {
            let small = c.word(g::FONT_DRAW) == code::SMALL_FONT_DRAW;
            c.run(0xd1bb, 5 + if small { 9 } else { 8 })?;
            c.lods(B, ES);
            c.cmp(B, 0x0d, 0xff);
            c.cmp(B, 0x0d, 0x0d);
            let line_x = c.word(g::FONT_LINE_X);
            c.set_word(g::FONT_X, line_x);
            let draw = c.word(g::FONT_DRAW);
            c.cmp(W, draw, code::SMALL_FONT_DRAW);
            let step = if small { 7 } else { 0x0a };
            c.set_reg(AX, step);
            let y = c.word(g::FONT_LINE_Y);
            let y = c.add(W, y, step);
            c.set_word(g::FONT_LINE_Y, y);
            let y = c.word(g::FONT_Y);
            let y = c.add(W, y, step);
            c.set_word(g::FONT_Y, y);
            continue;
        }
        c.run(0xd1bb, if ch & 0x80 != 0 { 9 } else { 8 })?;
        c.lods(B, ES);
        c.cmp(B, ch as u16, 0xff);
        c.cmp(B, ch as u16, 0x0d);
        c.or(B, ch as u16, ch as u16);
        if ch & 0x80 != 0 {
            c.set_reg8(r8::AL, b'@');
        }
        let draw = c.word(g::FONT_DRAW);
        c.call(draw, 0xd1cf)?;
        c.run(0xd1cf, 1)?; // jmp D1BBh
    }
}

/// `font_draw_phrase_or_command_string_with_color_at_pos` (D194): string
/// AX at (DX, BX) in colour CX.
pub fn font_draw_string_with_color_at_pos(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd194)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let colour = c.reg(CX);
    c.set_word(g::TEXT_COLOUR, colour);
    c.call(code::FONT_SET_DRAW_POSITION, 0xd19b)?;
    super::gfx::font_draw_phrase_or_command_string(c.m, 0xd19b)
}

/// `font_draw_phrase_list` (D1A6): draws the strings listed at SI (string,
/// x, y, colour), up to FFFFh. Exit: SI past it, CX = FFFFh; flags from the
/// `inc`.
pub fn font_draw_phrase_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd1a6)?;
    let mut c = Ctx::new(m);
    loop {
        let si = c.reg(SI);
        let string = c.word_in(DS, si);
        if string == 0xffff {
            c.run(0xd1a6, 5)?;
            c.lods(W, DS);
            c.set_reg(CX, string);
            let ax = c.inc(W, string);
            c.set_reg(AX, ax);
            return c.ret();
        }
        c.run(0xd1a6, 12)?;
        c.lods(W, DS);
        c.inc(W, string);
        let x = c.lods(W, DS);
        let y = c.lods(W, DS);
        let colour = c.lods(W, DS);
        c.set_reg(DX, x);
        c.set_reg(BX, y);
        c.set_reg(CX, colour);
        c.set_reg(AX, string);
        let si = c.reg(SI);
        c.push(si);
        c.call(0xd194, 0xd1b8)?;
        c.run(0xd1b8, 2)?; // pop si; jmp
        let si = c.pop();
        c.set_reg(SI, si);
    }
}
