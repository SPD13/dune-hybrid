//! Voice files (streamed in 8 KB parts into two buffers) and music
//! loading.

use super::pure;
use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DS, DX, ES, SI, SP, SS, ZF},
    rec::Bail,
};

mod code {
    pub const OPEN_RES_OR_FILE: u16 = 0xf1fb;
    pub const CHECK_PCM_VOICE_FILE_OPEN: u16 = 0xaba3;
    pub const READ_AUDIO_FILE: u16 = 0xa93f;
    pub const MUSIC_ENABLED: u16 = 0xae28;
    pub const MIDI_FUNC_2_0: u16 = 0xaeb7;
    pub const LOAD_RESOURCE_TO: u16 = 0xf0b9;
    pub const ALLOC_CHECK: u16 = 0xf11c;
    pub const HSQ_DECOMP_SKIP_HEADER: u16 = 0xf403;
    pub const OPEN_RESOURCE_BY_INDEX: u16 = 0xc13e;
    pub const HEX_DIGIT: u16 = 0xa8b1;
}

/// The second voice buffer (8 bytes after the first: a far pointer, the
/// size, a state byte, a part number).
const VOC_BUFFER_2: u16 = 0x3819;
/// Bytes per part read.
const PART: u16 = 0x2000;
/// Music resources are numbered from A4h.
const MUSIC_RESOURCES: u16 = 0xa4;

/// `loc_0ae28`: ZF clear if music is enabled (settings bit 100h).
pub fn music_enabled(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xae28)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let s = c.word(g::SOUND_SETTINGS);
    c.test(W, s, 0x100);
    c.ret()
}

/// `loc_0aec6`: CF set if sound is off (the command line's bit 10h, or
/// music disabled).
pub fn sound_off(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xaec6)?;
    let mut c = Ctx::new(m);
    let args = c.byte(g::CMD_ARGS_MEMORY);
    if args & 0x10 != 0 {
        c.run(start, 4)?; // test; jne; stc; ret
        c.test(B, args as u16, 0x10);
        c.set_flag(CF, true);
        return c.ret();
    }
    c.run(start, 3)?;
    c.test(B, args as u16, 0x10);
    c.call(code::MUSIC_ENABLED, 0xaed0)?;
    let on = !c.flag(pc::cpu::ZF);
    c.run(0xaed0, 3)?; // je; clc / stc; ret
    c.set_flag(CF, !on);
    c.ret()
}

/// From A93F: reads the voice file's next part (8 KB, or what is left)
/// to ES:DX for the buffer at SI: its size at +4, state 1 at +6, its part
/// number at +7 (bit 80h on the last part, whose file is then closed).
/// CF set if the read failed.
fn read_part(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xa93f, 4)?;
    let dx = c.reg(DX);
    c.push(dx);
    let (low, high) = (c.word(g::VOICE_FILE_POS), c.word(g::VOICE_FILE_POS + 2));
    c.set_reg(DX, low);
    c.set_reg(CX, high);
    c.set_reg(AX, 0x4200);
    c.interp(0xa94b, 0xa94d)?; // DOS seek
    let left = c.word(g::VOICE_FILE_LEFT);
    let left_high = c.word(g::VOICE_FILE_LEFT + 2);
    let last = (left_high as u32) << 16 | left as u32;
    let short = last < PART as u32;
    c.run(0xa94d, 8 + if short { 2 } else { 0 } + 3)?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    let (si, ds) = (c.reg(SI), c.sreg(DS));
    c.push(si);
    c.push(ds);
    c.set_reg(CX, PART);
    c.set_reg(AX, left);
    let l = c.sub(W, left, PART);
    c.set_word(g::VOICE_FILE_LEFT, l);
    let h = c.sbb(W, left_high, 0);
    c.set_word(g::VOICE_FILE_LEFT + 2, h);
    if short {
        let n = c.inc(W, left);
        c.set_reg(CX, n);
    }
    let es = c.sreg(ES);
    c.push(es);
    let es = c.pop();
    c.set_sreg(DS, es);
    c.set_reg8(r8::AH, 0x3f);
    c.interp(0xa968, 0xa96a)?; // DOS read
    let failed = c.flag(CF);
    if failed {
        c.run(0xa96a, 5)?;
    }
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let si = c.pop();
    c.set_reg(SI, si);
    let read = c.reg(AX);
    if failed {
        c.set_word(si.wrapping_add(4), read);
        return c.ret();
    }
    let part = c.byte(g::VOICE_PART);
    let counted = part < 0x3f;
    let remaining_high = c.word(g::VOICE_FILE_LEFT + 2);
    let more = remaining_high & 0x8000 == 0;
    c.run(0xa96a, 11 + if counted { 2 } else { 0 } + 4 + if more { 1 } else { 1 })?;
    c.set_word(si.wrapping_add(4), read);
    let p = c.word(g::VOICE_FILE_POS);
    let p = c.add(W, p, read);
    c.set_word(g::VOICE_FILE_POS, p);
    let p = c.word(g::VOICE_FILE_POS + 2);
    let p = c.adc(W, p, 0);
    c.set_word(g::VOICE_FILE_POS + 2, p);
    c.set_byte(g::VOC_LOADED, 0xff);
    c.set_byte(si.wrapping_add(6), 1);
    c.set_reg8(r8::BL, part);
    c.cmp(B, part as u16, 0x3f);
    let mut bl = part;
    if counted {
        let p = c.inc(B, part as u16) as u8;
        c.set_byte(g::VOICE_PART, p);
        bl = c.inc(B, part as u16) as u8;
        c.set_reg8(r8::BL, bl);
    }
    c.set_byte(si.wrapping_add(7), bl);
    c.cmp(W, remaining_high, 0);
    c.set_flag(CF, false);
    if more {
        return c.ret();
    }
    let v = c.or(B, bl as u16, 0x80) as u8;
    c.set_byte(si.wrapping_add(7), v);
    super::audio::close_pcm_voice_file_handle(c.m, 0xa9a1)
}

/// `read_audio_file` (A93F).
pub fn read_audio_file(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa93f)?;
    read_part(&mut Ctx::new(m))
}

/// `open_pcm_voice_file` (A90B): opens the voice file named at 37DAh
/// (from DUNE.DAT or on its own), both buffers empty; then reads its first
/// part into the first buffer. CF set if not found.
pub fn open_pcm_voice_file(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa90b)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    c.set_reg(DX, g::VOICE_FILE_NAME);
    let ax = c.reg(AX);
    let zero = c.xor(W, ax, ax);
    c.set_reg(AX, zero);
    c.set_word(g::VOC_BUFFER, 0);
    c.set_word(g::PCM_STATE, 0);
    c.set_word(g::PCM_STATE_2, 0);
    c.set_byte(g::VOICE_PART, 0);
    c.call(code::OPEN_RES_OR_FILE, 0xa91f)?;
    if c.flag(CF) {
        c.run(0xa91f, 2)?;
        return c.ret();
    }
    c.run(0xa91f, 10)?;
    let bx = c.reg(BX);
    c.set_word(g::VOICE_FILE_HANDLE, bx);
    // The size from the open is one more than the last offset.
    let cx = c.reg(CX);
    let cx = c.sub(W, cx, 1);
    c.set_reg(CX, cx);
    let bp = c.reg(BP);
    let bp = c.sbb(W, bp, 0);
    c.set_reg(BP, bp);
    c.set_word(g::VOICE_FILE_LEFT, cx);
    c.set_word(g::VOICE_FILE_LEFT + 2, bp);
    let (ax, dx) = (c.reg(AX), c.reg(DX));
    c.set_word(g::VOICE_FILE_POS, ax);
    c.set_word(g::VOICE_FILE_POS + 2, dx);
    c.set_reg(SI, g::VOC_BUFFER);
    let (dx, es) = (c.word(g::VOC_BUFFER), c.word(g::VOC_BUFFER + 2));
    c.set_reg(DX, dx);
    c.set_sreg(ES, es);
    read_part(&mut c)
}

/// `loc_0a9b9`: with the voice file open, refills a free buffer (the
/// first, or the second if the first is busy; none if both are) with
/// the next part, then PCM driver function 9 on it.
pub fn refill_voice_buffer(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa9b9)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::CHECK_PCM_VOICE_FILE_OPEN, 0xa9bc)?;
    if c.flag(pc::cpu::ZF) {
        c.run(0xa9bc, 2)?;
        return c.ret();
    }
    let first = c.byte(g::PCM_STATE);
    let second = c.byte(g::PCM_STATE_2);
    if first != 0 && second != 0 {
        c.run(0xa9bc, 7)?;
        c.set_reg(SI, g::VOC_BUFFER);
        c.cmp(B, first as u16, 0);
        c.cmp(B, second as u16, 0);
        return c.ret();
    }
    c.run(0xa9bc, if first == 0 { 4 } else { 7 } + 5)?;
    c.set_reg(SI, g::VOC_BUFFER);
    c.cmp(B, first as u16, 0);
    if first != 0 {
        c.cmp(B, second as u16, 0);
        c.set_reg(SI, VOC_BUFFER_2);
    }
    let handle = c.word(g::VOICE_FILE_HANDLE);
    c.set_reg(BX, handle);
    let si = c.reg(SI);
    let (dx, es) = (c.word(si), c.word(si.wrapping_add(2)));
    c.set_sreg(ES, es);
    let dx = c.add(W, dx, 6);
    c.set_reg(DX, dx);
    c.push(si);
    c.call(code::READ_AUDIO_FILE, 0xa9df)?;
    let failed = c.flag(CF);
    c.run(0xa9df, if failed { 3 } else { 2 })?;
    let si = c.pop();
    c.set_reg(SI, si);
    if failed {
        return c.ret();
    }
    c.interp(0xa9e2, 0xa9e6)?;
    c.run(0xa9e6, 1)?;
    c.ret()
}

/// `load_music` (AE62): loads music AL into the music buffer, unless it
/// is the loaded one (MIDI driver function 2 first). With the buffer below
/// the allocator's end directly; else stored (HSQ off) into free memory
/// and decompressed into the buffer, the sprite sheet reopened after.
/// AX kept.
pub fn load_music(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xae62)?;
    let mut c = Ctx::new(m);
    let music = c.reg8(r8::AL);
    let loaded = c.byte(g::MUSIC_LOADED);
    if music == loaded {
        c.run(start, 3)?;
        return c.ret();
    }
    c.run(start, 3)?;
    c.call(code::MIDI_FUNC_2_0, 0xae6b)?;
    let (di, es) = (c.word(g::MUSIC_DATA), c.word(g::MUSIC_DATA + 2));
    let end = c.word(g::ALLOCATOR_LAST_SEGMENT);
    let direct = es < end;
    c.run(0xae6b, if direct { 9 } else { 8 })?;
    let music = c.reg8(r8::AL);
    c.set_byte(g::MUSIC_LOADED, music);
    let ax = c.reg(AX);
    c.push(ax);
    let resource = pure::add(W, ax, MUSIC_RESOURCES);
    c.set_reg(AX, resource);
    c.set_reg(SI, resource);
    c.set_reg(DI, di);
    c.set_sreg(ES, es);
    c.set_reg(AX, es);
    if direct {
        c.call(code::LOAD_RESOURCE_TO, 0xae83)?;
        c.run(0xae83, 2)?;
        let ax = c.pop();
        c.set_reg(AX, ax);
        return c.ret();
    }
    c.run(0xae85, 4)?;
    let sheet = c.word(g::RESOURCE_ID);
    c.push(sheet);
    c.push(resource);
    c.set_reg(CX, 0x2af);
    c.call(code::ALLOC_CHECK, 0xae90)?;
    c.run(0xae90, 3)?;
    let si = c.pop();
    c.set_reg(SI, si);
    let v = c.byte(g::DISABLE_HSQ);
    let v = pure::inc(B, v as u16) as u8;
    c.set_byte(g::DISABLE_HSQ, v);
    c.call(code::LOAD_RESOURCE_TO, 0xae98)?;
    c.run(0xae98, 8)?;
    let v = c.byte(g::DISABLE_HSQ);
    let v = pure::dec(B, v as u16) as u8;
    c.set_byte(g::DISABLE_HSQ, v);
    let (ds, di, es) = (c.sreg(DS), c.reg(DI), c.sreg(ES));
    c.push(ds);
    c.push(di);
    c.push(es);
    let (to, to_seg) = (c.word(g::MUSIC_DATA), c.word(g::MUSIC_DATA + 2));
    c.set_reg(DI, to);
    c.set_sreg(ES, to_seg);
    let from_seg = c.pop();
    c.set_sreg(DS, from_seg);
    let from = c.pop();
    c.set_reg(SI, from);
    c.call(code::HSQ_DECOMP_SKIP_HEADER, 0xaea8)?;
    c.run(0xaea8, 3)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let sheet = c.pop();
    c.set_reg(AX, sheet);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0xaead)?;
    c.run(0xaead, 2)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// `create_voc_file_name_from_bx` (A8BC): the voice file name at 37DBh:
/// the letter for BL at 0 and 3, then, from AX (now in BX), the hex digits
/// (`sub_a8b1`) of AH, of AL's high and low nibbles; 'O' (or 'I' in some
/// places); then the letter for AH's high nibble | [47E0h], or ' '.
pub fn create_voc_file_name_from_bx(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa8bc)?;
    let mut c = Ctx::new(m);
    c.run(start, 14)?;
    c.set_reg(DI, g::VOICE_FILE_NAME + 1);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    let ax = c.reg(AX);
    c.push(ax);
    let bl = c.reg8(r8::BL);
    let letter = pure::add(B, bl as u16, 0x41) as u8;
    c.set_reg8(r8::AL, letter);
    c.stos(B);
    for _ in 0..2 {
        let di = c.reg(DI);
        let di = pure::inc(W, di);
        c.set_reg(DI, di);
    }
    c.stos(B);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.set_reg8(r8::CL, 4);
    c.set_reg8(r8::AL, (bx >> 8) as u8);
    c.call(code::HEX_DIGIT, 0xa8d2)?;
    c.run(0xa8d2, 4)?;
    c.stos(B);
    let (bl, cl) = (c.reg8(r8::BL), c.reg8(r8::CL));
    let high = c.m.cpu.shift(5, B, bl as u32, (cl & 0x1f) as u32) as u8; // shr al, cl
    c.set_reg8(r8::AL, high);
    c.call(code::HEX_DIGIT, 0xa8da)?;
    c.run(0xa8da, 3)?;
    c.stos(B);
    let bl = c.reg8(r8::BL);
    c.set_reg8(r8::AL, bl);
    c.call(code::HEX_DIGIT, 0xa8e0)?;
    let (ea, b6, b4) = (c.byte(g::BYTE_00EA), c.byte(g::WORD_0006), c.byte(g::WORD_0004));
    let n = if (ea as i8) > 0 {
        4
    } else if b6 != 0x80 {
        6
    } else if b4 == 1 {
        8
    } else {
        9
    };
    let cl = c.reg8(r8::CL);
    let ch_bh = (c.reg8(r8::BH) as u32 >> (cl & 0x1f).min(8)) as u8;
    let extra = c.byte(g::VOICE_LANGUAGE_PART);
    let none = ch_bh | extra == 0;
    c.run(0xa8e0, n + 5 + if none { 0 } else { 2 } + 2)?;
    c.stos(B);
    c.set_reg8(r8::AL, b'O');
    if (ea as i8) <= 0 {
        if b6 == 0x80 {
            if b4 != 1 {
                c.set_reg8(r8::AL, b'I');
            }
        }
    }
    c.stos(B);
    c.set_reg8(r8::AL, b' ');
    let bh = c.reg8(r8::BH);
    let bh = c.m.cpu.shift(5, B, bh as u32, (cl & 0x1f) as u32) as u8;
    let bh = pure::or(B, bh as u16, extra as u16) as u8;
    c.set_reg8(r8::BH, bh);
    if !none {
        let l = pure::add(B, bh as u16, 0x41) as u8;
        c.set_reg8(r8::AL, l);
    }
    c.stos(B);
    c.ret()
}

/// `loc_0a8b1`: the hex digit of AL's low 4 bits, in AL.
pub fn hex_digit(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa8b1)?;
    let mut c = Ctx::new(m);
    let al = c.reg8(r8::AL);
    let d = (al & 0x0f) + 0x30;
    c.run(start, if d <= 0x39 { 5 } else { 6 })?;
    let v = pure::and(B, al as u16, 0x0f);
    let mut v = pure::add(B, v, 0x30);
    if v > 0x39 {
        v = pure::add(B, v, 7);
    }
    c.set_reg8(r8::AL, v as u8);
    c.ret()
}

/// `loc_0a9f4`: the voice buffer playing (its far pointer at [3824h]) is
/// refilled from 6 bytes in (`loc_0aa70`), handed to the PCM driver
/// (function at 39A1h), and [3824h] swaps to the other buffer (3811h /
/// 3819h).
pub fn swap_voice_buffer(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa9f4)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let si = c.word(g::VOICE_BUFFER_PLAYING);
    c.set_reg(SI, si);
    let (off, seg) = (c.word(si), c.word(si.wrapping_add(2)));
    c.set_sreg(ES, seg);
    let di = c.add(W, off, 6);
    c.set_reg(DI, di);
    c.call(0xaa70, 0xaa00)?;
    c.interp(0xaa00, 0xaa04)?;
    c.run(0xaa04, 4)?;
    let x = c.xor(W, 0x3811, 0x3819);
    c.set_reg(AX, x);
    let v = c.word(g::VOICE_BUFFER_PLAYING);
    let v = c.xor(W, v, x);
    c.set_word(g::VOICE_BUFFER_PLAYING, v);
    c.ret()
}

/// `loc_0abd5`: `loc_0a7c2` while a voice plays (`is_voc_pcm_playing`).
pub fn while_voice_playing(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xabd5)?;
    let mut c = Ctx::new(m);
    loop {
        c.run(0xabd5, 1)?;
        c.call(0xabcc, 0xabd8)?;
        c.run(0xabd8, 2)?;
        if c.flag(ZF) {
            return c.ret();
        }
        c.call(0xa7c2, 0xabd5)?;
    }
}

/// `loc_0ad43`: the music asked for at [DBCCh] played (`play_music`), the
/// current one forgotten first.
pub fn play_requested_music(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xad43)?;
    let mut c = Ctx::new(m);
    let al = c.byte(0xdbcc);
    c.run(start, if al == 0 { 5 } else { 4 })?;
    c.set_reg8(r8::AL, al);
    c.set_byte(g::CURRENT_MUSIC, 0);
    c.or(B, al as u16, al as u16);
    if al == 0 {
        return c.ret();
    }
    super::audio::play_music(c.m, 0xad95)
}

/// `loc_0adbe`: unless sound is off, music is on its own (bit 0 of the
/// music settings) or [DBCDh] has bit 40h, the driver's function at
/// 397Dh with AX = 12Ch, BX = 0; [DBCDh] = what it returns (BX kept).
pub fn music_driver_397d(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xadbe)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xaec6, 0xadc1)?;
    if c.flag(CF) {
        c.run(0xadc1, 2)?;
        return c.ret();
    }
    let set = c.byte(g::MUSIC_SETTINGS);
    let present = c.byte(g::IS_SOUND_PRESENT);
    let n = if set & 1 != 0 { 4 } else if present & 0x40 != 0 { 6 } else { 8 };
    c.run(0xadc1, n)?;
    c.test(B, set as u16, 1);
    if set & 1 != 0 {
        return c.ret();
    }
    c.test(B, present as u16, 0x40);
    if present & 0x40 != 0 {
        return c.ret();
    }
    let bx = c.reg(BX);
    c.push(bx);
    c.set_reg(AX, 0x12c);
    c.xor(W, bx, bx);
    c.set_reg(BX, 0);
    c.interp(0xadd7, 0xaddb)?;
    c.run(0xaddb, 3)?;
    let al = c.reg8(r8::AL);
    c.set_byte(g::IS_SOUND_PRESENT, al);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.ret()
}

/// `loc_0acbf`: the 12h bytes at 37FAh (SS) shuffled (18 swaps of two
/// random places 0..8, the third generator stirred by the timer). SI = them.
pub fn shuffle_37fa(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xacbf)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let bp = 0x37fau16;
    c.set_reg(BP, bp);
    c.set_reg(CX, 0x12);
    loop {
        c.run(0xacc5, 2)?;
        c.set_reg(BX, 8);
        c.call(0xe3df, 0xaccb)?;
        c.run(0xaccb, 5)?;
        let a = c.reg(AX);
        c.set_reg(SI, a);
        let t = c.word(g::TIMER);
        let cx = c.reg(CX);
        let t = pure::xor(W, t, cx);
        c.set_reg(AX, t);
        let s = c.word(g::RAND_BELOW_SEED);
        let s = pure::add(W, s, t);
        c.set_word(g::RAND_BELOW_SEED, s);
        c.call(0xe3df, 0xacd9)?;
        let cx = c.reg(CX).wrapping_sub(1);
        c.run(0xacd9, 5 + if cx == 0 { 2 } else { 0 })?;
        let b = c.reg(AX);
        c.set_reg(DI, b);
        let si = c.reg(SI);
        let x = c.byte_in(SS, bp.wrapping_add(si));
        let y = c.byte_in(SS, bp.wrapping_add(b));
        c.m.cpu.write8(&mut c.m.hw, SS, bp.wrapping_add(b), x);
        c.m.cpu.write8(&mut c.m.hw, SS, bp.wrapping_add(si), y);
        c.set_reg8(r8::AL, y);
        c.set_reg(CX, cx);
        if cx == 0 {
            c.set_reg(SI, bp);
            return c.ret();
        }
    }
}

/// `loc_0aba9`: with PCM on, `loc_0ab92` until the voice file is closed
/// or 1000 ticks went by; then the voice not playing.
pub fn wait_voice_done(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xaba9)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xae2f, 0xabac)?;
    if c.flag(ZF) {
        c.run(0xabac, 2)?;
        return c.ret();
    }
    c.run(0xabac, 4)?;
    let t0 = c.word(g::TIMER);
    c.set_reg(BX, t0);
    loop {
        let bx = c.reg(BX);
        c.push(bx);
        c.call(0xab92, 0xabb6)?;
        c.run(0xabb6, 1)?;
        c.call(0xaba3, 0xabb9)?;
        let closed = c.flag(ZF);
        if closed {
            c.run(0xabb9, 4)?;
            let bx = c.pop();
            c.set_reg(BX, bx);
            c.set_byte(g::VOC_PCM_PLAYING, 0);
            return c.ret();
        }
        let sp = c.reg(pc::cpu::SP);
        let bx = c.word_in(SS, sp);
        let t = c.word(g::TIMER);
        let left = t.wrapping_sub(bx) < 0x3e8;
        c.run(0xabb9, 6 + if left { 2 } else { 2 })?;
        let bx = c.pop();
        c.set_reg(BX, bx);
        let d = c.sub(W, t, bx);
        c.set_reg(AX, d);
        c.cmp(W, d, 0x3e8);
        if !left {
            c.set_byte(g::VOC_PCM_PLAYING, 0);
            return c.ret();
        }
    }
}

/// `loc_0b02c`: the voice stopped, voice 2 started, then transition 0Eh.
pub fn voice_2_and_transition(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb02c)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xac14, 0xb02f)?;
    c.run(0xb02f, 2)?;
    c.set_reg8(r8::AL, 2);
    c.call(0xab15, 0xb034)?;
    c.run(0xb034, 2)?;
    c.set_reg8(r8::AL, 0x0e);
    super::screen::transition_al(c.m, 0xc108)
}

/// `loc_0ab92` (a frame task): `loc_0a9b9`; when `loc_0aba3` says done
/// (ZF), `loc_0aded` and the task removes itself.
pub fn voice_task_ab92(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xab92)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xa9b9, 0xab95)?;
    c.run(0xab95, 1)?;
    c.call(0xaba3, 0xab98)?;
    if !c.flag(pc::cpu::ZF) {
        c.run(0xab98, 2)?;
        return c.ret();
    }
    c.run(0xab98, 2)?;
    c.call(0xaded, 0xab9d)?;
    c.run(0xab9d, 2)?;
    c.set_reg(SI, 0xab92);
    super::tasks::remove_frame_task(c.m, 0xda5f)
}

/// `loc_0c868`: unless a voice plays (`is_voc_pcm_playing`), the sound
/// sequence of the scene [22A6h] (below 11h, with a place at 27B6h):
/// 2792h, or one of 2789h/278Eh at random out of the night; each byte
/// to its 0 either a pause of 12Ch (negative) or `loc_0c8c1` with it;
/// then `loc_0c4dd`.
pub fn scene_sound_sequence(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc868)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xabcc, 0xc86b)?;
    if !c.flag(pc::cpu::ZF) {
        c.run(0xc86b, 2)?;
        return c.ret();
    }
    let sc = c.word(0x22a6);
    if sc >= 0x11 {
        c.run(0xc86b, 5)?;
        c.set_reg(SI, sc);
        return c.ret();
    }
    // The scene's place (27B6h, 4 bytes a scene): none, no sound.
    let p = sc << 2;
    let (dx, bx) = (c.word(p.wrapping_add(0x27b6)), c.word(p.wrapping_add(0x27b8)));
    let night = c.byte(0x227d);
    if dx | bx == 0 {
        c.run(0xc86b, 12)?;
    } else {
        c.run(0xc86b, 11 + 3 + if night == 0 { 3 } else { 0 })?;
    }
    c.set_reg(SI, p);
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.set_reg(AX, bx | dx);
    if dx | bx == 0 {
        return c.ret();
    }
    c.set_reg(SI, 0x2792);
    if night == 0 {
        c.push(bx);
        c.set_reg(BX, 1);
        c.call(0xe3b7, 0xc89a)?;
        let z = c.flag(pc::cpu::ZF);
        c.run(0xc89a, 3 + (!z) as u32)?;
        let b = c.pop();
        c.set_reg(BX, b);
        c.set_reg(SI, if z { 0x2789 } else { 0x278e });
    }
    loop {
        let si = c.reg(SI);
        let al = c.byte(si);
        if al == 0 {
            c.run(0xc8a3, 4)?;
            c.lods(B, DS);
            c.call(0xc4dd, 0xc8c0)?;
            c.run(0xc8c0, 1)?;
            return c.ret();
        }
        if al & 0x80 != 0 {
            c.run(0xc8a3, 6)?;
            c.lods(B, DS);
            c.or(B, al as u16, al as u16); // the wait (`loc_0e387`) pushes these flags
            c.set_reg(AX, 0x12c);
            c.call(0xe387, 0xc8b0)?;
            c.run(0xc8b0, 1)?;
            continue;
        }
        c.run(0xc8a3, 8)?;
        c.lods(B, DS);
        let si = c.reg(SI);
        c.push(si);
        c.set_reg(AX, al as u16);
        c.set_reg(BP, al as u16);
        c.call(0xc8c1, 0xc8ba)?;
        c.run(0xc8ba, 2)?;
        let s = c.pop();
        c.set_reg(SI, s);
    }
}

/// `transfer_sd_block_qq`: the sound block at [0DBDEh]:[0DC1Ch] (its
/// first word the size with the header) copied to ES:DI; the header SI
/// gets its length (word 4) and bytes 6, 7 = 1, 41h.
pub fn transfer_sd_block(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xaa70)?;
    let mut c = Ctx::new(m);
    c.run(start, 9)?;
    let (si0, ds0) = (c.reg(SI), c.sreg(DS));
    c.push(si0);
    c.push(ds0);
    let p = c.word(0xdc1c);
    let seg = c.word(0xdbde);
    c.set_reg(SI, p);
    c.set_sreg(DS, seg);
    // The block less its 4-byte header: words, then the odd byte
    // (`shr ax, 1`; `adc cl, cl` with CL 0 after the words). The `shr`
    // stays exact: a stop at AA84h resumes the original at the `adc`.
    let size = c.lods(W, DS).wrapping_sub(4);
    c.set_reg(AX, size);
    let words = c.shr1(W, size, 1);
    c.set_reg(CX, words);
    c.rep(0xa5);
    c.run(0xaa84, 2)?;
    let cl = c.reg8(r8::CL).wrapping_mul(2) | (size & 1) as u8;
    c.set_reg8(r8::CL, cl);
    c.rep(0xa4);
    c.run(0xaa88, 6)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let si = c.pop();
    c.set_reg(SI, si);
    let ax = c.reg(AX);
    c.set_word(si.wrapping_add(4), ax);
    c.set_byte(si.wrapping_add(6), 1);
    c.set_byte(si.wrapping_add(7), 0x41);
    c.ret()
}

/// `loc_0ab45`: when `loc_0ae2f` finds a voice (not ZF), `loc_0ade0`
/// (AX kept), then [`play_voice_19`].
pub fn play_voice_after_ade0(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xab45)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xae2f, 0xab48)?;
    if c.flag(ZF) {
        c.run(0xab48, 2)?;
        return c.ret();
    }
    c.run(0xab48, 3)?;
    let ax = c.reg(AX);
    c.push(ax);
    c.call(0xade0, 0xab4e)?;
    c.run(0xab4e, 1)?;
    let a = c.pop();
    c.set_reg(AX, a);
    play_voice_19_at(&mut c)
}

/// `loc_0ab4f`: the voice AX (block 19h, `loc_0a8bc`) played unless one
/// is playing (`is_voc_pcm_playing`) or none is found (`loc_0ae2f`):
/// `loc_0ac14`, then unless `loc_0a90b` fails, its data (`loc_0a9b9`,
/// [3811h] + 1Ah), the frame task AB92h (BP 1) and the driver's start
/// (far [3991h]); registers and ES kept.
pub fn play_voice_19(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xab4f)?;
    let mut c = Ctx::new(m);
    play_voice_19_at(&mut c)
}

fn play_voice_19_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xab4f, 3)?;
    c.set_byte(0x47e0, 0);
    c.set_reg(BX, 0x19);
    c.call(0xa8bc, 0xab5a)?;
    c.run(0xab5a, 2)?;
    c.set_byte(0x37e2, 0x49);
    c.call(0xabcc, 0xab62)?;
    if !c.flag(ZF) {
        c.run(0xab62, 2)?;
        return c.ret();
    }
    c.run(0xab62, 2)?;
    c.call(0xae2f, 0xab67)?;
    if c.flag(ZF) {
        c.run(0xab67, 2)?;
        return c.ret();
    }
    c.run(0xab67, 3)?;
    let es = c.sreg(ES);
    c.push(es);
    c.call(0xe270, 0xab6d)?;
    c.run(0xab6d, 1)?;
    c.call(0xac14, 0xab70)?;
    c.run(0xab70, 1)?;
    c.call(0xa90b, 0xab73)?;
    if !c.flag(CF) {
        c.run(0xab73, 3)?;
        let v = c.word(0x3811);
        let v = c.add(W, v, 0x1a);
        c.set_word(0x3811, v);
        c.call(0xa9b9, 0xab7d)?;
        c.run(0xab7d, 3)?;
        c.set_reg(SI, 0xab92);
        c.set_reg(BP, 1);
        c.call(0xda25, 0xab86)?;
        c.run(0xab86, 1)?;
        c.set_reg(SI, 0x3811);
        c.interp(0xab89, 0xab8d)?;
        c.run(0xab8d, 1)?;
    } else {
        c.run(0xab73, 2)?;
    }
    c.call(0xe283, 0xab90)?;
    c.run(0xab90, 2)?;
    let es = c.pop();
    c.set_sreg(ES, es);
    c.ret()
}

/// `loc_0aa96`: AX = the music for the situation: 0Ah in phase 48h during
/// a scene ([4774h]); else 0Dh (busy [46D9h]), 1 ([0DD03h]), 2 (a
/// negative [0FBh]), 3 ([0C6h]), 4 ([0EAh] above 0), then by the place:
/// at a sietch (byte 80h) 9/8 by its kind and 0Ah from phase 48h with
/// bit 0 of BL clear, 0Ch/0Bh/0Ah for palaces; else 5/6/7 by the map
/// mode [11C9h].
pub fn music_for_situation(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xaa96)?;
    let mut c = Ctx::new(m);
    let saved = c.m.cpu.flags();
    let n = music_walk(&mut c, false);
    c.m.cpu.set_flags(saved);
    c.run(start, n)?;
    music_walk(&mut c, true);
    c.ret()
}

/// The code of `loc_0aa96`; with `write` its registers too. Its count.
fn music_walk(c: &mut Ctx, write: bool) -> u32 {
    let mut n = 1u32;
    let mut ax = c.xor(W, 0, 0);
    let mut dx = c.reg(DX);
    let mut bx = c.reg(BX);
    let fin = |c: &mut Ctx, n: u32, ax: u16, dx: u16, bx: u16| -> u32 {
        if write {
            c.set_reg(AX, ax);
            c.set_reg(DX, dx);
            c.set_reg(BX, bx);
        }
        n + 1
    };
    let k = c.byte(0x4774);
    c.cmp(B, k as u16, 0);
    n += 2;
    let ph = c.byte(0x2a);
    if k != 0 {
        c.cmp(B, ph as u16, 0x48);
        n += 2;
        if ph != 0x48 {
            return fin(c, n, ax, dx, bx);
        }
        ax = 0x0a;
        return fin(c, n + 2, ax, dx, bx);
    }
    ax = 0x0d;
    for (addr, kind) in [(0x46d9u16, 0), (0xdd03, 0), (0xfb, 1), (0xc6, 0), (0xea, 2)] {
        let v = c.byte(addr);
        c.cmp(B, v as u16, 0);
        n += 3;
        let stop = match kind {
            0 => v != 0,
            1 => v & 0x80 != 0,
            _ => (v as i8) > 0,
        };
        if stop {
            return fin(c, n, ax, dx, bx);
        }
        ax = if addr == 0x46d9 { 1 } else { c.inc(W, ax) };
    }
    n += 5;
    dx = c.word(4);
    bx = c.word(6);
    c.cmp(B, bx & 0xff, 0x80);
    let at_place = bx & 0xff == 0x80;
    if at_place {
        c.cmp(B, dx & 0xff, 1);
        n += 2;
    }
    if !at_place || dx & 0xff == 1 {
        let mode = c.byte(0x11c9);
        let b = c.and(B, mode as u16, 3);
        bx = bx & 0xff00 | b;
        n += 3;
        if b == 0 {
            return fin(c, n, ax, dx, bx);
        }
        ax = c.inc(W, ax);
        let b2 = c.dec(B, b);
        bx = bx & 0xff00 | b2;
        n += 3;
        if b2 == 0 {
            return fin(c, n, ax, dx, bx);
        }
        ax = c.inc(W, ax);
        return fin(c, n + 1, ax, dx, bx);
    }
    let dh = dx >> 8;
    c.cmp(B, dh, 0x20);
    n += 2;
    if dh < 0x20 {
        let a = c.sub(B, 9, 0);
        let _ = a;
        c.cmp(B, dh, 7);
        let al = c.sbb(B, 9, 0);
        ax = ax & 0xff00 | al;
        n += 3;
        c.cmp(B, ph as u16, 0x48);
        n += 2;
        if ph < 0x48 {
            return fin(c, n, ax, dx, bx);
        }
        let b = c.shr1(B, bx & 0xff, 1);
        let cf = c.flag(pc::cpu::CF);
        bx = bx & 0xff00 | b;
        n += 2;
        if cf {
            return fin(c, n, ax, dx, bx);
        }
        ax = ax & 0xff00 | 0x0a;
        return fin(c, n + 2, ax, dx, bx);
    }
    ax = ax & 0xff00 | 0x0c;
    n += 2;
    if dh != 0x20 {
        return fin(c, n, ax, dx, bx);
    }
    ax = c.dec(W, ax);
    c.cmp(B, dx & 0xff, 3);
    n += 3;
    if dx & 0xff != 3 {
        return fin(c, n, ax, dx, bx);
    }
    ax = ax & 0xff00 | 0x0a;
    fin(c, n + 1, ax, dx, bx)
}

/// `loc_0ace6`: the music's frame step, unless a voice plays: in the
/// sequence mode (bit 1 of [3810h]) by day and with a song running,
/// every C8h ticks the next song of the list [380Eh] (restarting at
/// 37FAh, `loc_0acbf` first in mode 2); else after `loc_0aec6` the
/// pending song [0DBCCh] ([0DBCBh] cleared); a new song starts at
/// `music_start` (AD95h).
pub fn music_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xace6)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xabcc, 0xace9)?;
    if !c.flag(ZF) {
        c.run(0xace9, 2)?;
        return c.ret();
    }
    let f = c.byte(0x3810);
    if f & 1 == 0 {
        c.run(0xace9, 4)?;
        c.call(0xaec6, 0xad3a)?;
        if c.flag(CF) {
            c.run(0xad3a, 2)?;
            return c.ret();
        }
        let s = c.byte(0xdbcd);
        if s & 0x80 != 0 {
            c.run(0xad3a, 4)?;
            return c.ret();
        }
        let al = c.byte(0xdbcc);
        c.run(0xad3a, 7 + if al == 0 { 1 } else { 0 })?;
        c.set_reg8(r8::AL, al);
        c.set_byte(0xdbcb, 0);
        if al == 0 {
            return c.ret();
        }
        return music_start_at(&mut c, 0xad95, 0);
    }
    let night = c.byte(0x227d);
    let s = c.byte(0xdbcd);
    if night != 0 || s & 0x80 != 0 {
        c.run(0xace9, 3 + 2 + if night != 0 { 1 } else { 3 })?;
        return c.ret();
    }
    let t0 = c.word(0xdbd2);
    let now = c.word(0xce7a);
    let base = if t0 == 0 { now } else { t0 };
    let el = now.wrapping_sub(base);
    let n = 3 + 4 + 3 + if t0 == 0 { 2 } else { 0 } + 4;
    if el < 0xc8 {
        c.run(0xace9, n + 1)?;
    } else {
        c.run(0xace9, n)?;
    }
    c.set_reg(AX, t0);
    if t0 == 0 {
        c.set_reg(AX, now);
        c.set_word(0xdbd2, now);
    }
    let a = pure::sub(W, base, now);
    let a = pure::neg(W, a);
    c.set_reg(AX, a);
    if el < 0xc8 {
        return c.ret();
    }
    let si = c.word(0x380e);
    let b = c.byte(si);
    let step = c.string_step(B);
    if b & 0x80 == 0 {
        c.run(0xad18, 4 + 2)?;
        c.set_reg(SI, si);
        c.lods(B, DS);
    } else {
        let b2 = c.byte(0x37fa);
        let two = f & 2 != 0;
        c.run(0xad18, 8 + if two { 1 } else { 2 })?;
        c.set_reg(SI, si);
        c.lods(B, DS);
        c.set_reg(SI, 0x37fa);
        c.lods(B, DS);
        let _ = (b2, step);
        if two {
            c.call(0xacbf, 0xad2f)?;
            c.run(0xad2f, 3)?;
            c.lods(B, DS);
        }
    }
    let si = c.reg(SI);
    c.set_word(0x380e, si);
    music_start_at(&mut c, 0xad95, 0)
}

/// From `loc_0ad95` (after `pre` instructions from `at`): the song AL
/// (AH 0) started unless `loc_0aec6` refuses or it already plays
/// ([0DBCBh]): `loc_0ae62`, the driver's start (far [3971h], the song
/// data [0DBB6h], mode bit 1 of [3810h]) into [0DBCDh], [0DBD2h] 0.
fn music_start_at(c: &mut Ctx, at: u16, pre: u32) -> Result<(), Bail> {
    c.run(at, pre + 2)?;
    let al = c.reg8(r8::AL);
    c.xor(B, 0, 0);
    c.set_reg(AX, al as u16);
    c.call(0xaec6, 0xad9a)?;
    if c.flag(CF) {
        c.run(0xad9a, 2)?;
        return c.ret();
    }
    let al = c.reg8(r8::AL);
    let cur = c.byte(0xdbcb);
    if al == cur {
        c.run(0xad9a, 4)?;
        c.cmp(B, al as u16, cur as u16);
        return c.ret();
    }
    c.run(0xad9a, 4)?;
    c.cmp(B, al as u16, cur as u16);
    c.call(0xae62, 0xada5)?;
    c.run(0xada5, 4)?;
    let al = c.reg8(r8::AL);
    c.set_byte(0xdbcb, al);
    let (si, es) = (c.word(0xdbb6), c.word(0xdbb8));
    c.set_reg(SI, si);
    c.set_sreg(ES, es);
    let f = c.byte(0x3810);
    let a = c.and(B, f as u16, 1);
    c.set_reg8(r8::AL, a as u8);
    c.interp(0xadb1, 0xadb5)?;
    c.run(0xadb5, 4)?;
    let al = c.reg8(r8::AL);
    c.set_byte(0xdbcd, al);
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.set_word(0xdbd2, 0);
    c.ret()
}

/// `loc_0ad5e`: the music for the situation (`music_for_situation`)
/// started, unless `loc_0aec6` refuses: in the sequence mode the next
/// song of the list (while one runs), else the song from 375Ch by the
/// situation: a plain one by the driver (far [3979h]), a marked one (bit
/// 80h, low 6 bits) through [`music_driver_397d`] when it changes.
pub fn pick_music(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xad5e)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xaec6, 0xad61)?;
    if c.flag(CF) {
        c.run(0xad61, 2)?;
        return c.ret();
    }
    c.run(0xad61, 2)?;
    c.call(0xaa96, 0xad66)?;
    let f = c.byte(0x3810);
    if f != 0 {
        let s = c.byte(0xdbcd);
        if s & 0x80 != 0 {
            c.run(0xad66, 5)?;
            c.cmp(B, f as u16, 0);
            c.cmp(B, s as u16, 0);
            return c.ret();
        }
        let ops = |c: &mut Ctx| {
            c.cmp(B, f as u16, 0);
            c.cmp(B, s as u16, 0);
        };
        return song_list_at(&mut c, 0xad66, 4, f, ops);
    }
    let al = c.reg8(r8::AL);
    let v = c.byte(0x375cu16.wrapping_add(al as u16));
    if v == 0 {
        c.run(0xad66, 2 + 4 + 1)?;
        c.cmp(B, 0, 0);
        c.set_reg(BX, 0x375c);
        c.set_reg8(r8::AL, 0);
        c.or(B, 0, 0);
        return c.ret();
    }
    if v & 0x80 == 0 {
        c.run(0xad66, 2 + 4 + 2 + 1)?;
        c.cmp(B, 0, 0);
        c.set_reg(BX, 0x375c);
        c.set_reg8(r8::AL, v);
        c.or(B, v as u16, v as u16);
        c.or(B, v as u16, v as u16);
        c.set_byte(0xdbcc, v);
        c.interp(0xad84, 0xad88)?;
        c.run(0xad88, 1)?;
        return c.ret();
    }
    let a = v & 0x3f;
    let cur = c.byte(0xdbcb);
    c.run(0xad66, 2 + 4 + 2 + 4 + if a == cur { 1 } else { 0 })?;
    c.cmp(B, 0, 0);
    c.set_reg(BX, 0x375c);
    c.or(B, v as u16, v as u16);
    c.or(B, v as u16, v as u16);
    let a = c.and(B, v as u16, 0x3f);
    c.set_reg8(r8::AL, a as u8);
    c.set_byte(0xdbcc, a as u8);
    c.cmp(B, a, cur as u16);
    if a as u8 == cur {
        return c.ret();
    }
    music_driver_397d(c.m, 0xadbe)
}

/// From `loc_0ad18` (after `pre` instructions from `at` with `ops`):
/// the next song of the list [380Eh] (from 37FAh again at its end,
/// `loc_0acbf` first in mode 2 of `f`) to `play_music`.
fn song_list_at(c: &mut Ctx, at: u16, pre: u32, f: u8, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    let si = c.word(0x380e);
    let b = c.byte(si);
    if b & 0x80 == 0 {
        c.run(at, pre + 4 + 2)?;
        ops(c);
        c.set_reg(SI, si);
        c.lods(B, DS);
        c.or(B, b as u16, b as u16);
    } else {
        let two = f & 2 != 0;
        c.run(at, pre + 8 + if two { 1 } else { 2 })?;
        ops(c);
        c.set_reg(SI, si);
        c.lods(B, DS);
        c.or(B, b as u16, b as u16);
        c.set_reg(SI, 0x37fa);
        c.lods(B, DS);
        c.test(B, f as u16, 2);
        if two {
            c.call(0xacbf, 0xad2f)?;
            c.run(0xad2f, 3)?;
            c.lods(B, DS);
        }
    }
    let si = c.reg(SI);
    c.set_word(0x380e, si);
    super::audio::play_music(c.m, 0xad95)
}

/// `loc_0ae3f`: with music on (`music_enabled`) and no music buffer yet
/// (the far pointer at DBB6h 0) one of 9C40h bytes allocated into it
/// (`bump_alloc_get_addr_in_di_and_bump`).
pub fn alloc_music_buffer(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xae3f)?;
    let mut c = Ctx::new(m);
    c.run(0xae3f, 1)?;
    c.call(0xae28, 0xae42)?;
    if c.flag(ZF) {
        c.run(0xae42, 2)?;
        return c.ret();
    }
    let (a, b) = (c.word(0xdbb6), c.word(0xdbb8));
    if a | b != 0 {
        c.run(0xae42, 6)?;
        c.set_reg(DI, 0xdbb6);
        c.set_reg(AX, a);
        let v = pure::or(W, a, b);
        c.set_reg(AX, v);
        return c.ret();
    }
    c.run(0xae42, 7)?;
    c.set_reg(DI, 0xdbb6);
    c.set_reg(AX, a);
    c.set_reg(AX, 0);
    c.set_reg(CX, 0x9c40);
    c.jump(0xf0f6)
}

/// `loc_0ae54`: with voices on (`loc_0ae2f`) a voice buffer of 4E20h
/// bytes allocated into the far pointer at 3811h.
pub fn alloc_voice_buffer(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xae54)?;
    let mut c = Ctx::new(m);
    c.run(0xae54, 1)?;
    c.call(0xae2f, 0xae57)?;
    if c.flag(ZF) {
        c.run(0xae57, 2)?;
        return c.ret();
    }
    c.run(0xae57, 4)?;
    c.set_reg(DI, 0x3811);
    c.set_reg(CX, 0x4e20);
    c.jump(0xf0f6)
}

/// `lip_sync_frame_task`: the speaking head's frame task. With a voice
/// playing (`is_voc_pcm_playing`): without lip-sync timing ([0DC26h] 0)
/// the mouth follows the sound (`loc_0a9e7`; at its end the next
/// phrase); else the lip-sync frames due by the clock [0CE7Ah] (the
/// 32-bit time [0DC2Ch]:[0DC2Eh] stepped by [2882h]:[2884h], frames
/// read by `loc_0a814`, the last one shown by `loc_0a82e`; at the end
/// the next phrase). Then the voice buffer kept fed (`loc_0a9b9`). The
/// next phrase ([0DC30h], taken): the task stopped (`loc_0a7a5`) and
/// a queued phrase loaded (`load_voc_and_lipsync_data` + [0D810h], BL
/// 0Eh) and started (`loc_0a75c`).
pub fn lip_sync_frame_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa7c2)?;
    let mut c = Ctx::new(m);
    c.run(0xa7c2, 1)?;
    c.call(0xabcc, 0xa7c5)?;
    if c.flag(ZF) {
        c.run(0xa7c5, 2)?;
        return c.ret();
    }
    let k = c.word(0xdc26);
    if k == 0 {
        c.run(0xa7c5, 4)?;
        c.call(0xa9e7, 0xa7d1)?;
        if !c.flag(ZF) {
            c.run(0xa7d1, 1)?;
            return next_phrase_a789(&mut c);
        }
        c.run(0xa7d1, 3)?;
        return feed_a9b9(&mut c);
    }
    let now = c.word(0xce7a);
    let (t_hi, t_lo) = (c.word(0xdc2c), c.word(0xdc2e));
    let (cx, si) = (c.word(0x2882), c.word(0x2884));
    let (_, carry) = t_lo.overflowing_add(si);
    let bx = t_hi.wrapping_add(cx).wrapping_add(carry as u16);
    let dx = now.wrapping_sub(bx);
    if dx & 0x8000 != 0 {
        c.run(0xa7c5, 3 + 10 + 1)?;
        frame_regs(&mut c, now, t_hi, t_lo, cx, si);
        return feed_a9b9(&mut c);
    }
    c.run(0xa7c5, 3 + 10)?;
    frame_regs(&mut c, now, t_hi, t_lo, cx, si);
    loop {
        c.run(0xa7f3, 2)?;
        let di = c.reg(DI);
        c.push(di);
        c.call(0xa814, 0xa7f7)?;
        let cf = c.flag(CF);
        let sp = c.reg(SP);
        let di = c.word_in(SS, sp);
        if !cf {
            c.run(0xa7f7, 2)?;
            let v = c.pop();
            c.set_reg(DI, v);
            return next_phrase_a789(&mut c);
        }
        let (dx, cx, si, bp, bx) = (c.reg(DX), c.reg(CX), c.reg(SI), c.reg(BP), c.reg(BX));
        // `sub di, si; sbb dx, cx`: below when DX:DI < CX:SI.
        let b1 = di < si;
        let (x, b2) = dx.overflowing_sub(cx);
        let below = b2 || x < b1 as u16;
        c.run(0xa7f7, 2 + 3 + if below { 0 } else { 3 })?;
        c.pop();
        // DX:DI less a frame's length; the frame's time BX:BP on by one.
        let left = ((dx as u32) << 16 | di as u32).wrapping_sub((cx as u32) << 16 | si as u32);
        c.set_reg(DI, left as u16);
        c.set_reg(DX, (left >> 16) as u16);
        if below {
            break;
        }
        let due = ((bx as u32) << 16 | bp as u32).wrapping_add((cx as u32) << 16 | si as u32);
        c.set_reg(BP, due as u16);
        c.set_reg(BX, (due >> 16) as u16);
    }
    c.run(0xa806, 3)?;
    let (bx, bp) = (c.reg(BX), c.reg(BP));
    c.set_word(0xdc2c, bx);
    c.set_word(0xdc2e, bp);
    c.call(0xa82e, 0xa811)?;
    c.run(0xa811, 1)?;
    feed_a9b9(&mut c)
}

/// From A7D5h: DX the time past the next frame, DI 0, BX:BP the next
/// frame's time, CX:SI the step (counted by the caller).
fn frame_regs(c: &mut Ctx, now: u16, t_hi: u16, t_lo: u16, cx: u16, si: u16) {
    // BX:BP = the frame's time + its length (CX:SI); DX how far now is.
    let due = ((t_hi as u32) << 16 | t_lo as u32).wrapping_add((cx as u32) << 16 | si as u32);
    c.set_reg(DI, 0);
    c.set_reg(CX, cx);
    c.set_reg(SI, si);
    c.set_reg(BP, due as u16);
    c.set_reg(BX, (due >> 16) as u16);
    c.set_reg(DX, now.wrapping_sub((due >> 16) as u16));
}

/// `loc_0a789`: the next phrase [0DC30h] (taken): none stops the task
/// (`loc_0a7a5`); else after stopping it the phrase loaded
/// (`load_voc_and_lipsync_data`, + [0D810h], BL 0Eh) and, when found,
/// started (`loc_0a75c`).
fn next_phrase_a789(c: &mut Ctx) -> Result<(), Bail> {
    let p = c.word(0xdc30);
    c.run(0xa789, 4)?;
    c.xor(W, 0, 0);
    c.set_word(0xdc30, 0);
    c.set_reg(AX, p);
    c.or(W, p, p);
    if p == 0 {
        return stop_a7a5(c);
    }
    c.run(0xa793, 2)?;
    c.push(p);
    c.call(0xa7a5, 0xa797)?;
    c.run(0xa797, 4)?;
    let p = c.pop();
    let o = c.word(0xd810);
    let a = c.add(W, p, o);
    c.set_reg(AX, a);
    c.set_reg8(r8::BL, 0x0e);
    c.call(0xa6cc, 0xa7a1)?;
    if !c.flag(CF) {
        c.run(0xa7a1, 2)?;
        return c.ret();
    }
    c.run(0xa7a1, 2)?;
    c.run(0xa75c, 1)?;
    c.call(0x9197, 0xa75f)?;
    c.run(0xa75f, 3)?;
    c.set_reg(BP, 0);
    c.set_reg(SI, 0xa7c2);
    c.call(0xda25, 0xa768)?;
    c.run(0xa768, 2)?;
    c.set_byte(0xdc2b, 1);
    c.set_reg(SI, 0x3811);
    c.interp(0xa770, 0xa774)?;
    c.run(0xa774, 4)?;
    let t = c.word(0xce7a);
    c.set_reg(AX, t);
    c.set_word(0xdc2c, t);
    c.set_word(0xdc2e, 0x8000);
    c.call(0xa814, 0xa783)?;
    if !c.flag(CF) {
        c.run(0xa783, 2)?;
        return c.ret();
    }
    c.run(0xa783, 2)?;
    c.call(0xa82e, 0xa788)?;
    c.run(0xa788, 1)?;
    c.ret()
}

/// `loc_0a7a5` reached by a jump: the task removed, [0DC26h] 0,
/// `loc_0d61d`; with the voice still playing it is stopped (`loc_0abc6`,
/// `loc_0a9a1`) and the music volume restored (`loc_0aded`).
fn stop_a7a5(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xa7a5, 2)?;
    c.set_reg(SI, 0xa7c2);
    c.call(0xda5f, 0xa7ab)?;
    c.run(0xa7ab, 2)?;
    c.set_word(0xdc26, 0);
    c.call(0xd61d, 0xa7b4)?;
    c.run(0xa7b4, 1)?;
    c.call(0xabcc, 0xa7b7)?;
    if c.flag(ZF) {
        c.run(0xa7b7, 2)?;
        return c.ret();
    }
    c.run(0xa7b7, 2)?;
    c.call(0xabc6, 0xa7bc)?;
    c.run(0xa7bc, 1)?;
    c.call(0xa9a1, 0xa7bf)?;
    let bl = c.byte(0x2896);
    let bh = c.byte(0x28ae);
    c.run(0xa7bf, 1 + 5 + (bl < 4) as u32)?;
    c.set_reg(AX, 0x190);
    c.set_reg8(r8::BL, bl);
    c.set_reg8(r8::BH, bh);
    c.cmp(B, bl as u16, 4);
    if bl < 4 {
        c.set_reg8(r8::BL, 4);
    }
    c.interp(0xadff, 0xae03)?;
    c.run(0xae03, 1)?;
    c.ret()
}

/// `loc_0a9b9`: with the voice buffer in use (`loc_0aba3`) its free half
/// (3811h, or 3819h when the first is busy and the second free) refilled
/// (`loc_0a93f` from its data + 6, BX [3821h]) and handed to the driver
/// (far [39A1h]).
fn feed_a9b9(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xa9b9, 1)?;
    c.call(0xaba3, 0xa9bc)?;
    if c.flag(ZF) {
        c.run(0xa9bc, 2)?;
        return c.ret();
    }
    let (a, b) = (c.byte(0x3817), c.byte(0x381f));
    if a != 0 && b != 0 {
        c.run(0xa9bc, 7)?;
        c.set_reg(SI, 0x3811);
        c.cmp(B, a as u16, 0);
        c.cmp(B, b as u16, 0);
        return c.ret();
    }
    let si = if a == 0 { 0x3811u16 } else { 0x3819 };
    c.run(0xa9bc, 4 + if a == 0 { 0 } else { 3 } + 5)?;
    c.set_reg(SI, 0x3811);
    c.cmp(B, a as u16, 0);
    if a != 0 {
        c.cmp(B, b as u16, 0);
        c.set_reg(SI, 0x3819);
    }
    let bx = c.word(0x3821);
    c.set_reg(BX, bx);
    let dx = c.word(si);
    let es = c.word(si.wrapping_add(2));
    c.set_sreg(ES, es);
    let dx = c.add(W, dx, 6);
    c.set_reg(DX, dx);
    c.push(si);
    c.call(0xa93f, 0xa9df)?;
    let cf = c.flag(CF);
    c.run(0xa9df, 2 + (cf as u32))?;
    let v = c.pop();
    c.set_reg(SI, v);
    if cf {
        return c.ret();
    }
    c.interp(0xa9e2, 0xa9e6)?;
    c.run(0xa9e6, 1)?;
    c.ret()
}

/// `decode_sd_block` (AA0F): with a voice file ([0DC1Ch] not FFFFh) the
/// next block read (`loc_0ac14`), both half-buffers emptied, the first
/// filled (`loc_0aa70`); its sound data less 20h bytes ([3815h], also
/// [381Dh]) moved down over its 1Ah-byte header, the second half given
/// the same 6-byte header and filled with silence (80h); then the
/// driver started on the second (far [3991h], [3824h] 3819h) and fed
/// the first (far [39A1h]).
pub fn decode_sd_block(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{DS, ES};
    entered(start, 0xaa0f)?;
    let mut c = Ctx::new(m);
    let ax = c.word(0xdc1c);
    c.run(0xaa0f, if ax == 0xffff { 4 } else { 5 })?;
    c.set_reg(AX, ax.wrapping_add(1));
    if ax == 0xffff {
        return c.ret();
    }
    c.set_byte(0x376a, 0xff);
    c.call(0xac14, 0xaa1d)?;
    c.run(0xaa1d, 7)?;
    c.set_reg(AX, 0);
    c.set_word(0x3811, 0);
    c.set_word(0x3817, 0);
    c.set_word(0x381f, 0);
    c.set_reg(SI, 0x3811);
    let (d, e) = (c.word(0x3811), c.word(0x3813));
    c.set_reg(DI, d);
    c.set_sreg(ES, e);
    c.call(0xaa70, 0xaa30)?;
    c.run(0xaa30, 7)?;
    let v = c.word(0x3815).wrapping_sub(0x20);
    c.set_word(0x3815, v);
    c.set_word(0x381d, v);
    c.set_reg(CX, v.wrapping_add(6));
    c.set_reg(SI, 0x1a);
    let di = c.reg(DI);
    c.xor(W, di, di); // `xor di, di`: the driver's far call sees these flags
    c.set_reg(DI, 0);
    c.m.cpu.string_op(&mut c.m.hw, 0xa4, Some(ES), 0xf3);
    c.run(0xaa48, 9)?;
    let (d, e) = (c.word(0x3819), c.word(0x381b));
    c.set_reg(DI, d);
    c.set_sreg(ES, e);
    let n = c.word(0x3815);
    c.set_reg(CX, n);
    let ds = c.sreg(DS);
    c.push(ds);
    let (s, sg) = (c.word(0x3811), c.word(0x3813));
    c.set_reg(SI, s);
    c.set_sreg(DS, sg);
    c.movs(W);
    c.movs(W);
    c.movs(W);
    c.set_reg8(r8::AL, 0x80);
    c.rep(0xaa);
    c.run(0xaa5c, 3)?;
    let v = c.pop();
    c.set_sreg(DS, v);
    c.set_reg(SI, 0x3819);
    c.set_word(0x3824, 0x3819);
    c.interp(0xaa64, 0xaa68)?;
    c.run(0xaa68, 1)?;
    c.set_reg(SI, 0x3811);
    c.interp(0xaa6b, 0xaa6f)?;
    c.run(0xaa6f, 1)?;
    c.ret()
}

/// `set_lipsync_data_to_al` (A82E): mouth frame AL shown, unless it is
/// already ([0DC2Ah]); SI the lip-sync table [0DC28h], then `lip_frame`.
pub fn set_lipsync_data_to_al(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa82e)?;
    let mut c = Ctx::new(m);
    let al = c.reg8(r8::AL);
    let cur = c.byte(0xdc2a);
    if al == cur {
        c.run(0xa82e, 3)?;
        c.cmp(B, al as u16, cur as u16);
        return c.ret();
    }
    c.run(0xa82e, 5)?;
    c.cmp(B, al as u16, cur as u16);
    c.set_byte(0xdc2a, al);
    let si = c.word(0xdc28);
    c.set_reg(SI, si);
    lip_frame_9de3(&mut c)
}

/// 9DE3h: the mouth frame AL drawn (talking heads up to [47C4h] 0Fh).
/// After `setup_lip_sync_data_from_current`, a head ([46EBh] not
/// negative): its rectangle (`loc_05b99` into D834h) moved by the head's
/// position [1BF0h]/[1BF2h], the frame's entry (from [47D2h]: AL plus 4
/// per pose [47D0h] - 1, once more for head 0Dh [22A6h]) copied into the
/// sprite list (`loc_09bee`); unless [0EAh] is positive, the list drawn
/// (`loc_09d2d`, `loc_0908c`): directly with two sprites or more (and
/// `loc_0c4f0` after the mouse is hidden), else in the back buffer
/// ([0DBDAh] kept; `loc_0c08e`); then the mouse drawn again
/// (`draw_mouse_cursor_if_needed`). Else a sprite (`lip_sprite_9e75`).
fn lip_frame_9de3(c: &mut Ctx) -> Result<(), Bail> {
    let k = c.word(0x47c4);
    if k >= 0x10 {
        c.run(0x9de3, 3)?;
        c.cmp(W, k, 0x10);
        return c.ret();
    }
    c.run(0x9de3, 5)?;
    c.cmp(W, k, 0x10);
    let ax = c.reg(AX);
    c.push(ax);
    let si = c.reg(SI);
    c.push(si);
    c.call(0x9197, 0x9def)?;
    let f = c.byte(0x46eb);
    let neg = f & 0x80 != 0;
    c.run(0x9def, 4 + if neg { 0 } else { 2 })?;
    let si = c.pop();
    c.set_reg(SI, si);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.cmp(B, f as u16, 0);
    if neg {
        return lip_sprite_9e75(c);
    }
    c.set_reg(DI, 0xd834);
    c.call(0x5b99, 0x9dfe)?;
    let d = c.byte(0x47d0);
    let none = d.wrapping_sub(1) & 0x80 != 0;
    let kind = c.word(0x22a6);
    c.run(0x9dfe, 10 + if none { 0 } else { 2 + (kind == 0xd) as u32 + 3 } + 4)?;
    let dx = c.word(0x1bf0);
    c.set_reg(DX, dx);
    let bx = c.word(0x1bf2);
    c.set_reg(BX, bx);
    let di = c.reg(DI);
    for (o, v) in [(8u16, dx), (6, bx), (4, dx), (2, bx)] {
        let a = di.wrapping_sub(o);
        let w = c.word(a);
        let w = c.add(W, w, v);
        c.set_word(a, w);
    }
    let si = c.word(0x47d2);
    c.set_reg(SI, si);
    let ah = c.dec(B, d as u16);
    c.set_reg8(r8::AH, ah as u8);
    let mut al = c.reg8(r8::AL) as u16;
    if !none {
        c.cmp(W, kind, 0xd);
        if kind == 0xd {
            al = c.add(B, al, ah);
            c.set_reg8(r8::AL, al as u8);
        }
        let a = c.shl1(B, ah, 2);
        c.set_reg8(r8::AH, a as u8);
        al = c.add(B, al, a);
        c.set_reg8(r8::AL, al as u8);
    }
    let ah = c.reg8(r8::AH) as u16;
    c.xor(B, ah, ah);
    let ax = c.shl1(W, al, 1);
    c.set_reg(AX, ax);
    let si = c.add(W, si, ax);
    c.set_reg(SI, si);
    c.call(0x9bee, 0x9e36)?;
    let e = c.byte(0xea);
    let skip = (e as i8) > 0;
    c.run(0x9e36, 3 + skip as u32)?;
    c.set_reg(SI, 0x4608);
    c.cmp(B, e as u16, 0);
    if skip {
        return c.ret();
    }
    let n = c.word(0x4608);
    if n >= 2 {
        c.run(0x9e40, 3)?;
        c.cmp(W, n, 2);
        c.call(0x9d2d, 0x9e48)?;
        c.run(0x9e48, 1)?;
        c.call(0x908c, 0x9e4b)?;
        c.run(0x9e4b, 2)?;
        c.set_reg(SI, 0xd834);
        c.call(0xdb74, 0x9e51)?;
        c.run(0x9e51, 1)?;
        c.call(0xc4f0, 0x9e54)?;
        c.run(0x9e54, 1)?;
        return c.jump(0xdb67);
    }
    c.run(0x9e40, 4)?;
    c.cmp(W, n, 2);
    c.set_reg(SI, 0xd834);
    c.call(0xdb74, 0x9e5d)?;
    c.run(0x9e5d, 2)?;
    let v = c.word(0xdbda);
    c.push(v);
    c.call(0xc08e, 0x9e64)?;
    c.run(0x9e64, 2)?;
    c.set_reg(SI, 0x4608);
    c.call(0x9d2d, 0x9e6a)?;
    c.run(0x9e6a, 1)?;
    c.call(0x908c, 0x9e6d)?;
    c.run(0x9e6d, 2)?;
    let v = c.pop();
    c.set_word(0xdbda, v);
    c.jump(0xdb67)
}

/// 9E75h: the mouth as a sprite of the sheet [0DBB2h]: the frame's byte
/// (from [47D2h]: AL plus 4 per pose but for kind 0Ch [47C4h]) less 2
/// picks the sprite from the table [47CCh]; drawn (`loc_09d94`, the
/// mouse first hidden from 47D4h) into the screen [0DBD6h] then
/// refreshed (`loc_0c4f0`) when its +3 byte is set, else into
/// [0DBD8h]; then the mouse drawn again.
fn lip_sprite_9e75(c: &mut Ctx) -> Result<(), Bail> {
    let k = c.word(0x47c4);
    let twelve = k == 0xc;
    c.run(0x9e75, 3 + if twelve { 0 } else { 5 } + 6)?;
    let si = c.word(0x47d2);
    c.set_reg(SI, si);
    c.cmp(W, k, 0xc);
    let mut al = c.reg8(r8::AL) as u16;
    if !twelve {
        let d = c.byte(0x47d0);
        let ah = c.dec(B, d as u16);
        let ah = c.shl1(B, ah, 2);
        c.set_reg8(r8::AH, ah as u8);
        al = c.add(B, al, ah);
        c.set_reg8(r8::AL, al as u8);
    }
    let ah = c.reg8(r8::AH) as u16;
    c.xor(B, ah, ah);
    let ax = c.shl1(W, al, 1);
    c.set_reg(AX, ax);
    let si = c.add(W, si, ax);
    c.set_reg(SI, si);
    c.push(si);
    c.set_reg(SI, 0x47d4);
    c.call(0xdb74, 0x9e99)?;
    let sp = c.reg(SP);
    let s0 = c.word_in(SS, sp);
    let seg = c.word_in(SS, 0xdbb2);
    let b = c.byte_at_seg(seg, s0);
    let base = c.word_in(SS, 0x47cc);
    let ax = (b.wrapping_sub(2) as u16) << 1;
    let off = c.word_at(seg, ax.wrapping_add(base));
    let s1 = base.wrapping_add(off);
    let shown = c.byte_at_seg(seg, s1.wrapping_add(3)) != 0;
    c.run(0x9e99, 12 + 2)?;
    let si = c.pop();
    c.set_reg(SI, si);
    let ds = c.sreg(DS);
    c.push(ds);
    c.set_sreg(DS, seg);
    let b = c.lods(B, DS);
    c.xor(B, 0, 0);
    let a = c.sub(B, b, 2);
    let a = c.shl1(W, a, 1);
    c.set_reg(AX, a);
    c.set_reg(BP, a);
    c.set_reg(SI, base);
    let s = c.add(W, base, off);
    c.set_reg(SI, s);
    let v = c.byte_at_seg(seg, s.wrapping_add(3));
    c.cmp(B, v as u16, 0);
    if shown {
        let es = c.word_in(SS, 0xdbd6);
        c.set_sreg(ES, es);
        c.call(0x9d94, 0x9ebf)?;
        c.run(0x9ebf, 3)?;
        let v = c.pop();
        c.set_sreg(DS, v);
        c.set_reg(SI, 0x47d4);
        c.call(0xc4f0, 0x9ec6)?;
        c.run(0x9ec6, 1)?;
        return c.jump(0xdb67);
    }
    let es = c.word_in(SS, 0xdbd8);
    c.set_sreg(ES, es);
    c.call(0x9d94, 0x9ed1)?;
    c.run(0x9ed1, 2)?;
    let v = c.pop();
    c.set_sreg(DS, v);
    c.jump(0xdb67)
}

/// `load_voc_and_lipsync_data` (A6CC): phrase AX of talking head BL
/// (up to 0Eh) loaded: its voice and lip-sync files (`loc_0a8bc`, after
/// `loc_0a7a5` stops the last). BX FFFFh: the test sound (0FFFh or
/// 1FFFh in turn: the immediate at A6D2h, flipped in the code each
/// time) for head [22A6h]. Else the phrase number made relative to the
/// head's first ([0D814h] + 3E7h with [47DCh], else [bx*2 - 280Ch]
/// unless the intro), heads 0Eh saying phrase 2Ch/2Dh becoming 0Ch
/// ([47C4h]); its lip-sync table (`loc_09123`, 8 bytes per head from
/// 27FAh, [0DC28h]). Then the first lip-sync data read (`loc_0a83f`,
/// once more for the other half [37E2h]); CF set (after `loc_0ade0`,
/// `loc_0d617`) if there is none.
pub fn load_voc_and_lipsync_data(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::CS;
    entered(start, 0xa6cc)?;
    let mut c = Ctx::new(m);
    let bx = c.reg(BX);
    if bx == 0xffff {
        c.run(0xa6cc, 2 + 4)?;
        c.cmp(W, bx, 0xffff);
        let imm = c.code_word(0xa6d2);
        c.set_reg(AX, imm);
        let b = c.byte_in(CS, 0xa6d3);
        let b = c.xor(B, b as u16, 0x10);
        c.m.cpu.write8(&mut c.m.hw, CS, 0xa6d3, b as u8);
        let h = c.word(0x22a6);
        c.set_reg(BX, h);
        c.call(0xa8bc, 0xa6e1)?;
        c.run(0xa6e1, 1)?;
        c.call(0xa7a5, 0xa6e4)?;
        c.run(0xa6e4, 1)?;
    } else {
        let ax = c.reg(AX);
        let big = (bx & 0xff) >= 0xe;
        let bl = if big { 0xe } else { bx & 0xff };
        let bx1 = (bx & 0xff00) | bl;
        let f = c.byte(0x47dc);
        let intro = c.byte(0x227d);
        let mut n = 2 + 3 + big as u32 + 3;
        if f != 0 {
            n += 3;
        } else {
            n += 2 + if intro == 0 { 3 } else { 0 };
        }
        c.run(0xa6cc, n)?;
        c.cmp(W, bx, 0xffff);
        c.push(bx);
        c.cmp(B, bx & 0xff, 0xe);
        c.set_reg(BX, bx1);
        let h = c.and(B, ax >> 8, 0xf3);
        let mut ax = (h << 8) | (ax & 0xff);
        c.set_reg(AX, ax);
        c.cmp(B, f as u16, 0);
        if f != 0 {
            let d = c.word(0xd814);
            ax = c.sub(W, ax, d);
            ax = c.add(W, ax, 0x3e7);
            c.set_reg(AX, ax);
        } else {
            c.cmp(B, intro as u16, 0);
            if intro == 0 {
                let b2 = c.shl1(W, bx1, 1);
                c.set_reg(BX, b2);
                let d = c.word(b2.wrapping_sub(0x280c));
                ax = c.sub(W, ax, d);
                c.set_reg(AX, ax);
                let b3 = c.shr1(W, b2, 1);
                c.set_reg(BX, b3);
            }
        }
        let bl = c.reg8(r8::BL);
        let twelve = bl == 0xe && (ax as u8 & 0xfe) == 0x2c;
        c.run(0xa710, 2 + if bl == 0xe { 4 + if twelve { 4 } else { 0 } } else { 0 } + 1)?;
        c.cmp(B, bl as u16, 0xe);
        if bl == 0xe {
            c.set_reg(CX, ax);
            let cl = c.and(B, ax & 0xff, 0xfe);
            c.set_reg8(r8::CL, cl as u8);
            c.cmp(B, cl, 0x2c);
            if twelve {
                let v = c.pop();
                let v = (v & 0xff00) | 0x0c;
                c.set_reg(CX, v);
                c.set_word(0x47c4, v);
                c.push(v);
            }
        }
        c.call(0xa8bc, 0xa72a)?;
        c.run(0xa72a, 1)?;
        c.call(0xa7a5, 0xa72d)?;
        c.run(0xa72d, 2)?;
        let v = c.pop();
        c.set_reg(AX, v);
        c.call(0x9123, 0xa731)?;
        c.run(0xa731, 6)?;
        let a = c.reg(AX);
        let a = c.shl1(W, a, 3);
        let a = c.add(W, a, 0x27fa);
        c.set_reg(AX, a);
        c.set_reg(SI, a);
        c.set_word(0xdc28, a);
    }
    c.run(0xa740, 2)?;
    c.set_byte(0xdc2a, 0xff);
    c.call(0xa83f, 0xa748)?;
    let mut fail = c.flag(CF);
    if !fail {
        c.run(0xa748, 3)?;
        let v = c.byte(0x37e2);
        let v = c.xor(B, v as u16, 6);
        c.set_byte(0x37e2, v as u8);
        c.call(0xa83f, 0xa752)?;
        fail = c.flag(CF);
        c.run(0xa752, 1 + if fail { 1 } else { 1 })?;
        if !fail {
            return c.ret();
        }
    } else {
        c.run(0xa748, 2)?;
    }
    c.call(0xade0, 0xa757)?;
    c.run(0xa757, 1)?;
    c.call(0xd617, 0xa75a)?;
    c.run(0xa75a, 2)?;
    c.set_flag(CF, true);
    c.ret()
}
