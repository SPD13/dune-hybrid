//! Sound: PCM voice state, lip sync, the PIT timer rate.

use super::entry;
use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, CF, DI, DS, ES, IF},
    rec::Bail,
};


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
    let seg = c.voice_buffer(0).data[1];
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
    // The sound's interrupt writes the states: each is read right after
    // a stretch of its `cmp` alone, where the original reads it (see
    // `ticks.rs`).
    c.run(start, 1)?;
    let first = c.voice_buffer(0).state;
    c.cmp(B, first as u16, 3);
    if first == 3 {
        c.run(0xa9ec, 2)?;
    } else {
        c.run(0xa9ec, 1)?;
        c.run(0xa9ee, 1)?;
        let second = c.voice_buffer(1).state;
        c.cmp(B, second as u16, 3);
        c.run(0xa9f3, 1)?;
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
///
/// The sound's interrupt clears the byte: it is read right after a stretch
/// of its `cmp` alone (see `ticks.rs`).
pub fn is_voc_pcm_playing(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xabcc)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let playing = c.byte(g::VOC_PCM_PLAYING);
    c.cmp(B, playing as u16, 0);
    c.run(0xabd1, 1)?;
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
    let settings = c.word_at(g::GAME_DS, g::SOUND_SETTINGS);
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
    c.update_sound_state(|t| t.current_music = 0);
    c.interp(0xaebd, 0xaec1)?;
    c.run(0xaec1, 3)?;
    let present = c.reg8(r8::AL);
    c.update_sound_state(|t| t.present = present);
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
    let f = c.sound_state().present;
    if f & 0x80 == 0 {
        c.run(0xde0c, 6)?;
        let ax = c.reg(AX);
        c.push(ax);
        c.or(B, ax & 0xff, 1); // the exit's flags (ZF clear)
        let v = c.pop();
        c.set_reg(AX, v);
        return c.ret();
    }
    c.run(0xde0c, 3)?;
    c.call(entry::SAVE_REGS, 0xde16)?;
    c.run(0xde16, 3)?;
    c.set_byte(g::KEY_HIT, 0);
    c.set_reg(SI, 0xffff);
    c.set_reg(DI, 0xffff);
    loop {
        c.run(0xde20, 14)?;
        c.set_reg(AX, 0x60);
        let t = c.sound_state().midi_cx;
        // The target: [0DBCEh] * 16 + (60h - [0DBD0h]) / 6.
        let a = 0x60u16.wrapping_sub(t) & 0xff;
        let (q, r) = (a / 6, a % 6);
        let al = q as u8 & 0xf;
        c.set_reg(AX, (r << 8) | al as u16);
        c.set_reg8(r8::DL, 6);
        let dx = (c.sound_state().midi_bx << 4) | al as u16;
        c.set_reg(DX, dx);
        let bx = c.reg(BX);
        if bx <= dx {
            c.run(0xde4a, 2)?;
            let al = c.reg8(r8::AL);
            let v = c.or(B, al as u16, 1); // the exit's flags (ZF clear)
            c.set_reg8(r8::AL, v as u8);
            break;
        }
        c.run(0xde41, 2)?;
        c.push(bx);
        c.call(entry::STC_ON_USER_INPUT, 0xde45)?;
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
    c.call(entry::KB_CLEAR_SCANCODE, 0xddeb)?;
    c.run(0xddeb, 2)?;
    let f = c.pop();
    c.m.cpu.set_flags(f);
    c.call(entry::RESTORE_REGS, 0xddef)?;
    c.run(0xddef, 1)?;
    c.ret()
}
