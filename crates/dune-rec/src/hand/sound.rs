//! Sound: PCM voice state, lip sync, the PIT timer rate.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, CF, DI, DS, ES, IF},
    rec::Bail,
};

/// The data segment's value, which `check_pcm_enabled` loads itself.
const GAME_DS: u16 = 0x1f4b;

/// `advance_lipsync` (A814): the next lip-sync byte. CF set and the read
/// pointer advanced if there is one (DI past it, AL = it); CF clear at the
/// end (byte FFh, AL = FFh) or without lip-sync data (DI = 0).
/// Exit: ES = the lip-sync segment once data exists.
pub fn advance_lipsync(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa814)?;
    let mut c = Ctx::new(m);
    c.check(start, 11)?;
    let di = c.word(g::LIPSYNC_PTR);
    c.set_reg(DI, di);
    c.or(W, di, di);
    if di == 0 {
        c.count(4);
        return c.ret();
    }
    let seg = c.word(g::LIPSYNC_SEG);
    c.set_sreg(ES, seg);
    let shape = c.byte_in(ES, di);
    c.set_reg8(r8::AL, shape);
    c.cmp(B, shape as u16, 0xff);
    if shape == 0xff {
        c.count(8);
        return c.ret();
    }
    let di = c.inc(W, di);
    c.set_reg(DI, di);
    c.set_word(g::LIPSYNC_PTR, di);
    c.set_flag(CF, true);
    c.count(11);
    c.ret()
}

/// `pcm_test_audio_done` (A9E7): ZF set if either PCM state is 3 (done).
/// Flags from the last `cmp`.
pub fn pcm_test_audio_done(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa9e7)?;
    let mut c = Ctx::new(m);
    let first = c.byte(g::PCM_STATE);
    c.run(start, if first == 3 { 3 } else { 4 })?;
    c.cmp(B, first as u16, 3);
    if first != 3 {
        let second = c.byte(g::PCM_STATE_2);
        c.cmp(B, second as u16, 3);
    }
    c.ret()
}

/// `check_pcm_voice_file_open` (ABA3): ZF clear if the voice file is open.
pub fn check_pcm_voice_file_open(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xaba3)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let handle = c.word(g::VOICE_FILE_HANDLE);
    c.cmp(W, handle, 0);
    c.ret()
}

/// `set_voc_pcm_is_not_playing` (ABC6).
pub fn set_voc_pcm_is_not_playing(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xabc6)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(g::VOC_PCM_PLAYING, 0);
    c.ret()
}

/// `is_voc_pcm_playing` (ABCC): ZF clear while a VOC sample plays.
pub fn is_voc_pcm_playing(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xabcc)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let playing = c.byte(g::VOC_PCM_PLAYING);
    c.cmp(B, playing as u16, 0);
    c.ret()
}

/// `check_pcm_enabled` (AE2F): ZF clear if PCM sound is enabled (bit 0 of
/// the settings), whatever DS is. Registers kept.
pub fn check_pcm_enabled(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xae2f)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    let ax = c.reg(AX);
    c.push(ax);
    let ds = c.sreg(DS);
    c.push(ds);
    let settings = c.word_at(GAME_DS, g::SOUND_SETTINGS);
    c.test(W, settings, 1);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// `set_pit_timer_frequency` (E8A8): PIT channel 0 divisor = AX (mode 3),
/// with interrupts off while it is written. Exit: AL = AH; flags restored.
pub fn set_pit_timer_frequency(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe8a8)?;
    let mut c = Ctx::new(m);
    c.check(start, 11)?;
    let flags = c.m.cpu.flags();
    c.push(flags);
    let divisor = c.reg(AX);
    c.push(divisor);
    c.set_flag(IF, false);
    c.set_reg8(r8::AL, 0x36);
    c.count(4); // pushf; push ax; cli; mov al, 36h
    c.out(0x43, 0x36); // channel 0, low then high byte, square wave
    c.count(2); // out; pop ax
    let divisor = c.pop();
    c.set_reg(AX, divisor);
    c.out(0x40, divisor as u8);
    c.count(2); // out; mov al, ah
    let high = (divisor >> 8) as u8;
    c.set_reg8(r8::AL, high);
    c.out(0x40, high);
    c.count(3); // out; popf; ret
    let flags = c.pop();
    c.m.cpu.set_flags(flags);
    c.ret()
}

/// `call_pcm_audio_vtable_func_5` (AC30): PCM driver function 5.
pub fn call_pcm_audio_vtable_func_5(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xac30)?;
    let mut c = Ctx::new(m);
    c.interp(0xac30, 0xac34)?;
    c.run(0xac34, 1)?;
    c.ret()
}

/// `midi_func_2_0` (AEB7): MIDI driver function 2; its AL says whether
/// sound is present. AX kept.
pub fn midi_func_2_0(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xaeb7)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let ax = c.reg(AX);
    c.push(ax);
    c.set_byte(g::CURRENT_MUSIC, 0);
    c.interp(0xaebd, 0xaec1)?;
    c.run(0xaec1, 3)?;
    let present = c.reg8(r8::AL);
    c.set_byte(g::IS_SOUND_PRESENT, present);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// `check_midi` (DE0C): with MIDI music ([0DBCDh] bit 7) the tempo
/// position (`loc_0e270`), then the song played on (`loc_0dd63`) until
/// BX is reached: [0DBCEh] * 16 + (60h - [0DBD0h]) / 6 (or the player
/// stops, CF); then `loc_0de4e` and `loc_0e283` with its flags kept.
/// ZF clear when the position was reached (or without MIDI).
pub fn check_midi(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{BX, DX, SI};
    entered(start, 0xde0c)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0xdbcd);
    if f & 0x80 == 0 {
        c.run(0xde0c, 6)?;
        c.cmp(B, f as u16, 0);
        let ax = c.reg(AX);
        c.push(ax);
        c.or(B, ax & 0xff, 1);
        let v = c.pop();
        c.set_reg(AX, v);
        return c.ret();
    }
    c.run(0xde0c, 3)?;
    c.cmp(B, f as u16, 0);
    c.call(0xe270, 0xde16)?;
    c.run(0xde16, 3)?;
    c.set_byte(0xcee8, 0);
    c.set_reg(SI, 0xffff);
    c.set_reg(DI, 0xffff);
    loop {
        c.run(0xde20, 14)?;
        c.set_reg(AX, 0x60);
        let t = c.word(0xdbd0);
        let a = c.sub(W, 0x60, t);
        let ah = a >> 8;
        c.xor(B, ah, ah);
        c.set_reg(AX, a & 0xff);
        c.set_reg8(r8::DL, 6);
        c.m.cpu.mul_div(&mut c.m.hw, B, 6, 6);
        let al = c.reg8(r8::AL);
        let al = c.and(B, al as u16, 0xf);
        c.set_reg8(r8::AL, al as u8);
        let d = c.word(0xdbce);
        c.set_reg(DX, d);
        let d = c.shl1(W, d, 4);
        let dl = c.or(B, d & 0xff, al);
        let dx = (d & 0xff00) | dl;
        c.set_reg(DX, dx);
        let bx = c.reg(BX);
        c.cmp(W, bx, dx);
        if bx <= dx {
            c.run(0xde4a, 2)?;
            let al = c.reg8(r8::AL);
            let v = c.or(B, al as u16, 1);
            c.set_reg8(r8::AL, v as u8);
            break;
        }
        c.run(0xde41, 2)?;
        c.push(bx);
        c.call(0xdd63, 0xde45)?;
        let cf = c.flag(CF);
        c.run(0xde45, 2 + (!cf) as u32)?;
        let v = c.pop();
        c.set_reg(BX, v);
        if cf {
            break;
        }
    }
    c.run(0xdde7, 2)?;
    let f = c.m.cpu.flags();
    c.push(f);
    c.call(0xde4e, 0xddeb)?;
    c.run(0xddeb, 2)?;
    let f = c.pop();
    c.m.cpu.set_flags(f);
    c.call(0xe283, 0xddef)?;
    c.run(0xddef, 1)?;
    c.ret()
}
