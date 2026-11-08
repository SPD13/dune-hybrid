//! Voice (VOC through the PCM driver) and music (through the MIDI
//! driver): starting, stopping, the drivers' timer hook.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, ES, SI, ZF},
    rec::Bail,
};

mod code {
    pub const IS_VOC_PCM_PLAYING: u16 = 0xabcc;
    pub const CHECK_PCM_ENABLED: u16 = 0xae2f;
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
    pub const PCM_STOP_VOC: u16 = 0xac14;
    pub const OPEN_VOC_RESOURCE: u16 = 0xabe9;
    pub const CLOSE_PCM_VOICE_FILE_HANDLE: u16 = 0xa9a1;
    pub const REMOVE_FRAME_TASK: u16 = 0xda5f;
    pub const LOAD_RESOURCE_TO: u16 = 0xf0b9;
    /// CF set when sound is off (`sub_aec6`).
    pub const SOUND_OFF: u16 = 0xaec6;
    pub const MIDI_FUNC_2_0: u16 = 0xaeb7;
    pub const STOP_MUSIC: u16 = 0xae62;
    /// The voice's frame task.
    pub const VOICE_TASK: u16 = 0xab92;
}

/// VOC resources are numbered from AEh.
const VOC_RESOURCES: u16 = 0xae;
/// A VOC file's header, skipped.
const VOC_HEADER: u16 = 0x1a;

/// `close_pcm_voice_file_handle` (A9A1): closes the voice file (unless it
/// is DUNE.DAT's handle). CF clear; AX kept.
pub fn close_pcm_voice_file_handle(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa9a1)?;
    let mut c = Ctx::new(m);
    let handle = c.word(g::VOICE_FILE_HANDLE);
    let dat = c.word(g::DUNE_DAT_HANDLE);
    let close = handle != 0 && handle != dat;
    c.run(start, if handle == 0 { 6 } else { 8 })?;
    let bx = c.reg(BX);
    c.xor(W, bx, bx);
    c.set_word(g::VOICE_FILE_HANDLE, 0);
    c.set_reg(BX, handle);
    c.or(W, handle, handle);
    if handle != 0 {
        c.cmp(W, handle, dat);
    }
    if close {
        let ax = c.reg(AX);
        c.push(ax);
        c.set_reg8(r8::AH, 0x3e);
        c.interp(0xa9b4, 0xa9b6)?; // DOS close
        c.run(0xa9b6, 3)?;
        let ax = c.pop();
        c.set_reg(AX, ax);
    }
    c.set_flag(CF, false);
    c.ret()
}

/// From ABE9: loads VOC resource AL into the voice buffer (past its
/// header), state 8101h (to play).
fn open_voc(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xabe9, 8)?;
    c.set_word(g::VOC_BUFFER, 0);
    let seg = c.word(g::VOC_BUFFER + 2);
    c.set_reg(DI, 0);
    c.set_sreg(ES, seg);
    let v = c.add(W, 0, VOC_HEADER);
    c.set_word(g::VOC_BUFFER, v);
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    c.set_reg8(r8::AH, 0);
    let voc = c.reg8(r8::AL);
    let resource = c.add(W, voc as u16, VOC_RESOURCES);
    c.set_reg(SI, resource);
    c.set_byte(g::VOC_LOADED, voc);
    c.call(code::LOAD_RESOURCE_TO, 0xac06)?;
    c.run(0xac06, 4)?;
    let size = c.reg(CX);
    let size = c.sub(W, size, VOC_HEADER);
    c.set_reg(CX, size);
    c.set_word(g::VOC_SIZE, size);
    c.set_word(g::PCM_STATE, 0x8101);
    c.ret()
}

/// `open_voc_resource` (ABE9).
pub fn open_voc_resource(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xabe9)?;
    open_voc(&mut Ctx::new(m))
}

/// `play_pcm_al` (ABDB): loads VOC AL for playing, with sound on and PCM
/// enabled (CF set otherwise).
pub fn play_pcm_al(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xabdb)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SOUND_OFF, 0xabde)?;
    if c.flag(CF) {
        c.run(0xabde, 2)?;
        return c.ret();
    }
    c.run(0xabde, 2)?;
    c.call(code::CHECK_PCM_ENABLED, 0xabe3)?;
    let enabled = !c.flag(ZF);
    c.run(0xabe3, 3)?; // stc; je; ret / call
    c.set_flag(CF, true);
    if !enabled {
        return c.ret();
    }
    c.call(code::CLOSE_PCM_VOICE_FILE_HANDLE, 0xabe9)?;
    open_voc(&mut c)
}

/// `pcm_stop_voc_q` (AC14): stops the voice: its frame task, its file,
/// PCM driver function 3. Registers kept.
pub fn pcm_stop_voc(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{DI, ES};
    entered(start, 0xac14)?;
    let mut c = Ctx::new(m);
    c.run(start, 9)?;
    for r in [AX, BX, CX, SI, DI, BP] {
        let v = c.reg(r);
        c.push(v);
    }
    let es = c.sreg(ES);
    c.push(es);
    c.set_reg(SI, code::VOICE_TASK);
    c.call(code::REMOVE_FRAME_TASK, 0xac21)?;
    c.run(0xac21, 1)?;
    c.call(code::CLOSE_PCM_VOICE_FILE_HANDLE, 0xac24)?;
    c.interp(0xac24, 0xac28)?;
    c.run(0xac28, 8)?;
    let es = c.pop();
    c.set_sreg(ES, es);
    for r in [BP, DI, SI, CX, BX, AX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// `audio_start_voc` (AB15): starts VOC AL, unless a voice plays or PCM is
/// disabled: the loaded one directly, else after stopping the current and
/// loading it (none for AL = 0). Registers kept.
pub fn audio_start_voc(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xab15)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::IS_VOC_PCM_PLAYING, 0xab18)?;
    if !c.flag(ZF) {
        c.run(0xab18, 2)?;
        return c.ret();
    }
    c.run(0xab18, 2)?;
    c.call(code::CHECK_PCM_ENABLED, 0xab1d)?;
    if c.flag(ZF) {
        c.run(0xab1d, 2)?;
        return c.ret();
    }
    c.run(0xab1d, 3)?;
    let es = c.sreg(ES);
    c.push(es);
    c.call(code::SAVE_REGS, 0xab23)?;
    let voc = c.reg8(r8::AL);
    let loaded = c.byte(g::VOC_LOADED);
    if voc == loaded {
        c.run(0xab23, 4)?; // cmp; je; les di; mov si
        c.cmp(B, voc as u16, loaded as u16);
        let (di, seg) = (c.word(g::VOC_BUFFER), c.word(g::VOC_BUFFER + 2));
        c.set_reg(DI, di);
        c.set_sreg(ES, seg);
    } else {
        c.run(0xab23, 3)?;
        c.cmp(B, voc as u16, loaded as u16);
        c.call(code::PCM_STOP_VOC, 0xab2c)?;
        let voc = c.reg8(r8::AL);
        c.run(0xab2c, 3)?;
        c.or(B, voc as u16, voc as u16);
        if voc == 0 {
            c.call(code::RESTORE_REGS, 0xab43)?;
            return audio_start_end(&mut c);
        }
        c.call(code::OPEN_VOC_RESOURCE, 0xab33)?;
        c.run(0xab33, 2)?; // jmp; mov si
    }
    c.set_reg(SI, g::VOC_BUFFER);
    c.interp(0xab3c, 0xab40)?; // PCM driver: play
    c.run(0xab40, 1)?;
    c.call(code::RESTORE_REGS, 0xab43)?;
    audio_start_end(&mut c)
}

/// `pop es; ret` (AB43).
fn audio_start_end(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xab43, 2)?;
    let es = c.pop();
    c.set_sreg(ES, es);
    c.ret()
}

/// From AD95: starts music AL, unless sound is off or it already plays:
/// stops the current one, then MIDI driver function 1 on the music data
/// with the stereo bit. Exit: AX = 0 once started.
fn play(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xad95, 2)?;
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    c.set_reg8(r8::AH, 0);
    c.call(code::SOUND_OFF, 0xad9a)?;
    if c.flag(CF) {
        c.run(0xad9a, 2)?;
        return c.ret();
    }
    let music = c.reg8(r8::AL);
    let current = c.byte(g::CURRENT_MUSIC);
    c.run(0xad9a, 4)?;
    c.cmp(B, music as u16, current as u16);
    if music == current {
        return c.ret();
    }
    c.call(code::STOP_MUSIC, 0xada5)?;
    c.run(0xada5, 4)?;
    let music = c.reg8(r8::AL);
    c.set_byte(g::CURRENT_MUSIC, music);
    let (si, seg) = (c.word(g::MUSIC_DATA), c.word(g::MUSIC_DATA + 2));
    c.set_reg(SI, si);
    c.set_sreg(ES, seg);
    let settings = c.byte(g::MUSIC_SETTINGS);
    let stereo = c.and(B, settings as u16, 1) as u8;
    c.set_reg8(r8::AL, stereo);
    c.interp(0xadb1, 0xadb5)?;
    c.run(0xadb5, 4)?;
    let present = c.reg8(r8::AL);
    c.set_byte(g::IS_SOUND_PRESENT, present);
    let ax = c.reg(AX);
    let zero = c.xor(W, ax, ax);
    c.set_reg(AX, zero);
    c.set_word(g::MUSIC_DBD2, 0);
    c.ret()
}

/// `play_music` (AD95).
pub fn play_music(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xad95)?;
    play(&mut Ctx::new(m))
}

/// MIDI driver function 2, then music `music` (AD50, AD57).
fn play_named(m: &mut Machine, start: u16, entry: u16, music: u8) -> Result<(), Bail> {
    entered(start, entry)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::MIDI_FUNC_2_0, entry + 3)?;
    c.run(entry + 3, 2)?; // mov al; jmp AD95h
    c.set_reg8(r8::AL, music);
    play(&mut c)
}

/// `play_music_WORMSUIT_HSQ` (AD50): music 3.
pub fn play_music_wormsuit(m: &mut Machine, start: u16) -> Result<(), Bail> {
    play_named(m, start, 0xad50, 3)
}

/// `play_music_MORNING_HSQ` (AD57): music 6.
pub fn play_music_morning(m: &mut Machine, start: u16) -> Result<(), Bail> {
    play_named(m, start, 0xad57, 6)
}

/// `pit_timer_callback_midi` (EFBA): the MIDI driver's tick (function 6),
/// unless its bit in the settings is set; notes what it returns.
/// BX kept.
pub fn pit_timer_callback_midi(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xefba)?;
    let mut c = Ctx::new(m);
    let settings = c.byte(g::CMD_ARGS_MEMORY);
    let off = settings & 0x10 != 0;
    c.run(start, if off { 5 } else { 4 })?;
    let bx = c.reg(BX);
    c.push(bx);
    c.test(B, settings as u16, 0x10);
    if !off {
        let cx = c.reg(CX);
        c.push(cx);
        c.interp(0xefc3, 0xefc7)?;
        c.run(0xefc7, 6)?;
        let (al, bx, cx) = (c.reg8(r8::AL), c.reg(BX), c.reg(CX));
        c.set_byte(g::IS_SOUND_PRESENT, al);
        c.set_word(g::MIDI_FUNC5_BX, bx);
        c.set_word(g::MIDI_FUNC5_CX, cx);
        let cx = c.pop();
        c.set_reg(CX, cx);
    }
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.ret()
}
