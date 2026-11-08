//! Voice files (streamed in 8 KB parts into two buffers) and music
//! loading.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DS, DX, ES, SI, SS, ZF},
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
        c.cmp(B, music as u16, loaded as u16);
        return c.ret();
    }
    c.run(start, 3)?;
    c.cmp(B, music as u16, loaded as u16);
    c.call(code::MIDI_FUNC_2_0, 0xae6b)?;
    let (di, es) = (c.word(g::MUSIC_DATA), c.word(g::MUSIC_DATA + 2));
    let end = c.word(g::ALLOCATOR_LAST_SEGMENT);
    let direct = es < end;
    c.run(0xae6b, if direct { 9 } else { 8 })?;
    let music = c.reg8(r8::AL);
    c.set_byte(g::MUSIC_LOADED, music);
    let ax = c.reg(AX);
    c.push(ax);
    let resource = c.add(W, ax, MUSIC_RESOURCES);
    c.set_reg(AX, resource);
    c.set_reg(SI, resource);
    c.set_reg(DI, di);
    c.set_sreg(ES, es);
    c.set_reg(AX, es);
    c.cmp(W, es, end);
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
    let v = c.inc(B, v as u16) as u8;
    c.set_byte(g::DISABLE_HSQ, v);
    c.call(code::LOAD_RESOURCE_TO, 0xae98)?;
    c.run(0xae98, 8)?;
    let v = c.byte(g::DISABLE_HSQ);
    let v = c.dec(B, v as u16) as u8;
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
    let letter = c.add(B, bl as u16, 0x41) as u8;
    c.set_reg8(r8::AL, letter);
    c.stos(B);
    for _ in 0..2 {
        let di = c.reg(DI);
        let di = c.inc(W, di);
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
    c.cmp(B, ea as u16, 0);
    if (ea as i8) <= 0 {
        c.cmp(B, b6 as u16, 0x80);
        if b6 == 0x80 {
            c.cmp(B, b4 as u16, 1);
            if b4 != 1 {
                c.set_reg8(r8::AL, b'I');
            }
        }
    }
    c.stos(B);
    c.set_reg8(r8::AL, b' ');
    let bh = c.reg8(r8::BH);
    let bh = c.m.cpu.shift(5, B, bh as u32, (cl & 0x1f) as u32) as u8;
    let bh = c.or(B, bh as u16, extra as u16) as u8;
    c.set_reg8(r8::BH, bh);
    if !none {
        let l = c.add(B, bh as u16, 0x41) as u8;
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
    let v = c.and(B, al as u16, 0x0f);
    let mut v = c.add(B, v, 0x30);
    c.cmp(B, v, 0x39);
    if v > 0x39 {
        v = c.add(B, v, 7);
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
        let t = c.xor(W, t, cx);
        c.set_reg(AX, t);
        let s = c.word(g::RAND_BELOW_SEED);
        let s = c.add(W, s, t);
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
        c.cmp(W, sc, 0x11);
        return c.ret();
    }
    let p = sc << 2;
    let (dx, bx) = (c.word(p.wrapping_add(0x27b6)), c.word(p.wrapping_add(0x27b8)));
    if dx | bx == 0 {
        c.run(0xc86b, 12)?;
        c.set_reg(SI, sc);
        c.cmp(W, sc, 0x11);
        let s = c.shl1(W, sc, 2);
        c.set_reg(SI, s);
        c.set_reg(DX, dx);
        c.set_reg(BX, bx);
        let a = c.or(W, bx, dx);
        c.set_reg(AX, a);
        return c.ret();
    }
    let night = c.byte(0x227d);
    c.run(0xc86b, 11 + 3 + if night == 0 { 3 } else { 0 })?;
    c.set_reg(SI, sc);
    c.cmp(W, sc, 0x11);
    let s = c.shl1(W, sc, 2);
    c.set_reg(SI, s);
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    let a = c.or(W, bx, dx);
    c.set_reg(AX, a);
    c.set_reg(SI, 0x2792);
    c.cmp(B, night as u16, 0);
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
            c.or(B, 0, 0);
            c.call(0xc4dd, 0xc8c0)?;
            c.run(0xc8c0, 1)?;
            return c.ret();
        }
        if al & 0x80 != 0 {
            c.run(0xc8a3, 6)?;
            c.lods(B, DS);
            c.or(B, al as u16, al as u16);
            c.set_reg(AX, 0x12c);
            c.call(0xe387, 0xc8b0)?;
            c.run(0xc8b0, 1)?;
            continue;
        }
        c.run(0xc8a3, 8)?;
        c.lods(B, DS);
        c.or(B, al as u16, al as u16);
        let si = c.reg(SI);
        c.push(si);
        c.xor(B, 0, 0);
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
    let len = c.lods(W, DS);
    let ax = c.sub(W, len, 4);
    c.set_reg(AX, ax);
    let cx = c.shr1(W, ax, 1);
    c.set_reg(CX, cx);
    c.rep(0xa5);
    c.run(0xaa84, 2)?;
    let cl = c.reg8(r8::CL);
    let cl = c.adc(B, cl as u16, cl as u16);
    c.set_reg8(r8::CL, cl as u8);
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
        c.test(B, f as u16, 1);
        c.call(0xaec6, 0xad3a)?;
        if c.flag(CF) {
            c.run(0xad3a, 2)?;
            return c.ret();
        }
        let s = c.byte(0xdbcd);
        if s & 0x80 != 0 {
            c.run(0xad3a, 4)?;
            c.cmp(B, s as u16, 0);
            return c.ret();
        }
        let al = c.byte(0xdbcc);
        c.run(0xad3a, 7 + if al == 0 { 1 } else { 0 })?;
        c.cmp(B, s as u16, 0);
        c.set_reg8(r8::AL, al);
        c.set_byte(0xdbcb, 0);
        c.or(B, al as u16, al as u16);
        if al == 0 {
            return c.ret();
        }
        return music_start_at(&mut c, 0xad95, 0);
    }
    let night = c.byte(0x227d);
    let s = c.byte(0xdbcd);
    if night != 0 || s & 0x80 != 0 {
        c.run(0xace9, 3 + 2 + if night != 0 { 1 } else { 3 })?;
        c.test(B, f as u16, 1);
        c.cmp(B, night as u16, 0);
        if night == 0 {
            c.cmp(B, s as u16, 0);
        }
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
    c.test(B, f as u16, 1);
    c.cmp(B, 0, 0);
    c.cmp(B, s as u16, 0);
    c.set_reg(AX, t0);
    c.or(W, t0, t0);
    if t0 == 0 {
        c.set_reg(AX, now);
        c.set_word(0xdbd2, now);
    }
    let a = c.sub(W, base, now);
    let a = c.neg(W, a);
    c.set_reg(AX, a);
    c.cmp(W, a, 0xc8);
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
        c.or(B, b as u16, b as u16);
    } else {
        let b2 = c.byte(0x37fa);
        let two = f & 2 != 0;
        c.run(0xad18, 8 + if two { 1 } else { 2 })?;
        c.set_reg(SI, si);
        c.lods(B, DS);
        c.or(B, b as u16, b as u16);
        c.set_reg(SI, 0x37fa);
        c.lods(B, DS);
        c.test(B, f as u16, 2);
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
