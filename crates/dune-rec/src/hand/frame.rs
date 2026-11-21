//! The frame: running the frame tasks, waiting while checking for input,
//! stepping the HNM video, palette chunks, and small helpers.

use super::gfx;
use super::input;
use super::hnm;
use super::pure;
use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CS, CX, DI, DS, DX, ES, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const SUB_ACE6: u16 = 0xace6;
    pub const PAUSE_IF_P_KEY_PRESSED: u16 = 0xde7b;
    pub const READ_GAME_PORT: u16 = 0xdce0;
    pub const RAND: u16 = 0xe3cc;
    pub const PROCESS_FRAME_TASKS: u16 = 0xd9d2;
    pub const HNM_PREFETCH: u16 = 0xcb1a;
    pub const HNM_HANDLE_PAL_CHUNK: u16 = 0xce3b;
    pub const HNM_DECODE_VIDEO_FRAME: u16 = 0xcc96;
    pub const HNM_NEXT_FRAME: u16 = 0xcc4e;
    pub const HNM_FRAME_END: u16 = 0xca8f;
    pub const HNM_SET_TIMER: u16 = 0xca59;
    pub const PCM_PLAY_NEXT: u16 = 0xa9f4;
    pub const GFX_CLEAR_ACTIVE_FRAMEBUFFER: u16 = 0xc0ad;
    pub const SUB_D397: u16 = 0xd397;
    pub const SUB_B039: u16 = 0xb039;
}

/// `process_frame_tasks` (D9D2): the ticks since the last call (BX), then
/// each frame task whose counter reaches its period: its counter wraps
/// (mod the period) and it runs, with the walk's cursor (the entry, the
/// tasks left) on the stack at [`g::FRAME_TASK_WALK`].
pub fn process_frame_tasks(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd9d2)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SUB_ACE6, 0xd9d5)?;
    // The ticks, read right after a stretch of their `mov` alone (see
    // `ticks.rs`).
    c.run(0xd9d5, 1)?;
    let now = c.timer();
    c.set_reg(AX, now);
    let count = c.word(g::FRAME_TASKS);
    c.run(0xd9d8, 8)?; // ...; jcxz; then ret, or add si, 4
    let last = c.word(g::FRAME_TASKS_LAST_TICK);
    c.set_word(g::FRAME_TASKS_LAST_TICK, now);
    c.set_reg(CX, last);
    let mut ticks = now.wrapping_sub(last);
    c.set_reg(BX, ticks);
    c.set_reg(SI, g::FRAME_TASKS_LAST_TICK);
    c.set_reg(CX, count);
    if count == 0 {
        return c.ret();
    }
    c.set_reg(SI, g::FRAME_TASKS_LAST_TICK + 4);
    loop {
        // D9EB: an entry (period, counter, routine).
        let (si, cx) = (c.reg(SI), c.reg(CX));
        let period = c.word_in(DS, si);
        let counter = c.word(si.wrapping_add(2));
        let reached = ticks.wrapping_add(counter);
        let due = reached >= period;
        let left = cx.wrapping_sub(1);
        if !due {
            c.run(0xd9eb, 9 + if left == 0 { 2 } else { 0 })?;
            c.lods(W, DS);
            c.set_reg(BP, period);
            c.set_reg(AX, reached);
            let si = c.reg(SI);
            c.set_word(si, reached);
            c.set_reg(SI, si.wrapping_add(4));
            c.set_reg(CX, left);
            if left == 0 {
                c.set_word(g::FRAME_TASK_WALK, 0);
                return c.ret();
            }
            continue;
        }
        c.run(0xd9eb, 6 + 2 + if period != 0 { 3 } else { 0 } + 6)?;
        c.lods(W, DS);
        c.set_reg(BP, period);
        c.set_reg(AX, reached);
        if period != 0 {
            // Due: the counter keeps what passed the period (`div bp`).
            c.set_reg(AX, reached / period);
            c.set_reg(DX, reached % period);
            let si = c.reg(SI);
            c.set_word(si, reached % period);
        }
        let si = c.reg(SI);
        let si = c.sub(W, si, 2); // the task (called through) gets these flags
        c.set_reg(SI, si);
        c.push(ticks);
        c.push(cx);
        c.push(si);
        let sp = c.reg(SP);
        c.set_word(g::FRAME_TASK_WALK, sp);
        let routine = c.word(si.wrapping_add(4));
        c.call(routine, 0xda1b)?;
        // The cursor, as the task left it.
        let sp = c.reg(SP);
        let cx = c.word_in(SS, sp.wrapping_add(2));
        let left = cx.wrapping_sub(1);
        c.run(0xda1b, 5 + if left == 0 { 3 } else { 0 })?;
        let si = c.pop();
        c.pop();
        let bx = c.pop();
        c.set_reg(BX, bx);
        c.set_reg(SI, si.wrapping_add(6));
        c.set_reg(CX, left);
        if left == 0 {
            c.set_word(g::FRAME_TASK_WALK, 0);
            return c.ret();
        }
        // The ticks as saved (a task could change them).
        ticks = bx;
    }
}

/// `stc_on_user_input` (DD63): CF set on Esc, a key, a new mouse button
/// press, or (with the joystick) a new button; else the frame goes on: a
/// random seed to [0] and the frame tasks run (CF clear, AL bit 0 set).
pub fn stc_on_user_input(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdd63)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::PAUSE_IF_P_KEY_PRESSED, 0xdd66)?;
    c.run(0xdd66, 1)?;
    if input::esc_hit(&mut c, 0xdd69)? {
        c.run(0xdd69, 3)?;
        return input(&mut c);
    }
    let key = c.byte(g::KEY_HIT);
    if key != 0 {
        c.run(0xdd69, 5)?;
        c.cmp(B, key as u16, 0);
        return input(&mut c);
    }
    let args = c.byte(g::CMD_ARGS);
    c.run(0xdd69, if args & 0x40 != 0 { 5 } else { 6 })?;
    if args & 0x40 == 0 {
        c.set_reg(AX, 3);
        c.interp(0xdd7c, 0xdd7e)?; // the mouse's buttons
        let (bx, si) = (c.reg(BX), c.reg(SI));
        let pressed = (si ^ bx) & bx & 7;
        c.run(0xdd7e, if pressed != 0 { 7 } else { 5 })?;
        c.set_reg(SI, bx);
        // The buttons newly down; the exit's flags are the last `and`'s.
        let v = (si ^ bx) & bx;
        let bl = c.and(B, v & 0xff, 7);
        c.set_reg(BX, v & 0xff00 | bl);
        if pressed != 0 {
            return set_cf_ret(&mut c);
        }
    }
    let joystick = args & 0x80 != 0;
    c.run(0xdd89, if joystick { 4 } else { 5 })?;
    if joystick {
        let di = c.reg(DI);
        c.push(di);
        c.call(code::READ_GAME_PORT, 0xdd94)?;
        let sp = c.reg(SP);
        let (ax, di) = (c.reg(AX), c.word_in(SS, sp));
        let pressed = (di ^ ax) & ax & 0x30;
        c.run(0xdd94, if pressed != 0 { 8 } else { 6 } + if pressed != 0 { 0 } else { 3 })?;
        c.pop();
        c.set_reg(DI, ax);
        let v = (ax ^ di) & ax;
        let al = c.and(B, v & 0xff, 0x30);
        c.set_reg(AX, v & 0xff00 | al);
        if pressed != 0 {
            return set_cf_ret(&mut c);
        }
    }
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.push(si);
    c.push(di);
    c.call(code::RAND, 0xdda3)?;
    c.run(0xdda3, 2)?;
    let r = c.reg(AX);
    c.set_word(g::RANDOM_BITS, r);
    c.call(code::PROCESS_FRAME_TASKS, 0xdda9)?;
    c.run(0xdda9, 4)?;
    let di = c.pop();
    c.set_reg(DI, di);
    let si = c.pop();
    c.set_reg(SI, si);
    let al = c.reg8(r8::AL);
    let al = c.or(B, al as u16, 1) as u8;
    c.set_reg8(r8::AL, al);
    c.ret()
}

/// [`stc_on_user_input`] called: whether the player pressed Esc, a key or a button (else the frame went on).
pub(super) fn user_input(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    c.call_cf(0xdd63, ret)
}

/// DDAE: `stc; ret`, counted by the caller.
fn input(c: &mut Ctx) -> Result<(), Bail> {
    set_cf_ret(c)
}

fn set_cf_ret(c: &mut Ctx) -> Result<(), Bail> {
    c.set_flag(CF, true);
    c.ret()
}

/// `wait_processing_frame_tasks_interruptable` (E353): calls BP (to draw),
/// then waits AX ticks from before it, running the frame tasks
/// (`stc_on_user_input`, which ends the wait on input with CF set) in the
/// map view, or just checking for pause elsewhere. CF clear, AL bit 0 set
/// after a full wait.
pub fn wait_processing_frame_tasks_interruptable(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe353)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let ax = c.reg(AX);
    c.push(ax);
    let t0 = c.timer();
    c.push(t0);
    let target = c.reg(BP);
    c.call(target, 0xe35a)?;
    c.run(0xe35a, 2)?;
    let t0 = c.pop();
    c.set_reg(BX, t0);
    let ticks = c.pop();
    c.set_reg(BP, ticks);
    loop {
        let cinematic = c.byte(g::CINEMATIC);
        c.run(0xe35c, if cinematic != 0 { 2 + 8 } else { 3 })?;
        if cinematic != 0 {
            for r in [BX, CX, DX, SI, DI, BP] {
                let v = c.reg(r);
                c.push(v);
            }
            let es = c.sreg(ES);
            c.push(es);
            let stop = user_input(&mut c, 0xe36d)?;
            c.run(0xe36d, 8 + if stop { 1 } else { 1 })?;
            let es = c.pop();
            c.set_sreg(ES, es);
            for r in [BP, DI, SI, DX, CX, BX] {
                let v = c.pop();
                c.set_reg(r, v);
            }
            if stop {
                return c.ret();
            }
        } else {
            c.call(code::PAUSE_IF_P_KEY_PRESSED, 0xe37b)?;
        }
        let now = c.timer();
        let (bx, bp) = (c.reg(BX), c.reg(BP));
        let passed = now.wrapping_sub(bx);
        let more = passed < bp;
        c.run(0xe37b, if more { 4 } else { 6 })?;
        c.set_reg(AX, now);
        let p = pure::sub(W, now, bx);
        c.set_reg(AX, p);
        if !more {
            let al = pure::or(B, p & 0xff, 1);
            c.set_reg(AX, p & 0xff00 | al);
            return c.ret();
        }
    }
}

/// [`wait_processing_frame_tasks_interruptable`] called: BP called, then AX ticks from before it waited, running the frame tasks.
pub(super) fn draw_then_wait(c: &mut Ctx, draw: u16, ticks: u16, ret: u16) -> Result<(), Bail> {
    c.set_reg(BP, draw);
    c.set_reg(AX, ticks);
    c.call(0xe353, ret)
}

/// `hnm_wait_for_frame` (CAD4): CF clear when the next frame is due: with
/// the video's own timing, once [DBFFh] ticks have passed since [DC22h]
/// (then the timer restarts, `sub_ca59`); with a voice, once its buffer
/// played (`sub_a9f4`).
pub fn hnm_wait_for_frame(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcad4)?;
    let mut c = Ctx::new(m);
    let sync = c.video_state().voice_chunk;
    // The ticks and the voice buffer's state (the sound's interrupt) are
    // read right after a stretch of their instruction alone (see
    // `ticks.rs`).
    c.run(start, 3)?;
    let ax = c.inc(W, sync);
    c.set_reg(AX, ax);
    if sync == 0xffff {
        c.run(0xcada, 1)?;
        let now = c.timer();
        c.set_reg(AX, now);
        let tick = c.video_state().frame_tick;
        let passed = now.wrapping_sub(tick);
        let rate = c.video_state().frame_ticks;
        let early = passed >> 8 == 0 && (passed as u8) < rate;
        let n = 4 + if passed >> 8 != 0 { 1 } else { 2 + if early { 1 } else { 1 } };
        c.run(0xcadd, n - 1)?;
        let p = pure::sub(W, now, tick);
        c.set_reg(AX, p);
        c.or(B, p >> 8, p >> 8);
        if p >> 8 == 0 {
            c.cmp(B, p & 0xff, rate as u16);
            if early {
                return c.ret();
            }
        }
        c.call(code::HNM_SET_TIMER, 0xcaee)?;
        c.run(0xcaee, 2)?;
        return c.ret_cf(false);
    }
    c.run(0xcaf0, 1)?;
    let si = c.word(g::VOICE_BUFFER_PLAYING);
    c.set_reg(SI, si);
    c.run(0xcaf4, 1)?;
    let state = c.byte(si.wrapping_add(6));
    let busy = state != 0;
    c.cmp(B, state as u16, 1);
    c.run(0xcaf8, 3)?;
    let cf = c.flag(CF);
    c.set_flag(CF, !cf);
    if busy {
        return c.ret();
    }
    c.call(code::PCM_PLAY_NEXT, 0xcafe)?;
    c.run(0xcafe, 2)?;
    c.ret_cf(false)
}

/// [`hnm_wait_for_frame`] called: whether the next frame is due.
pub(super) fn frame_due(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    Ok(!c.call_cf(0xcad4, ret)?)
}

/// `hnm_do_frame` (CA60): with a video open: reads frames ahead until one
/// is complete (`sub_caa0`, prefetching), then when it is due shows it
/// (its palette chunk, the video data, the next frame), and prefetches;
/// [DBB5h] is the frame's high bit of [DBFEh] meanwhile.
pub fn hnm_do_frame(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xca60)?;
    let mut c = Ctx::new(m);
    loop {
        let handle = c.word(g::HNM_FILE_HANDLE);
        if handle == 0 {
            c.run(0xca60, 2)?;
            c.cmp(W, 0, 0);
            return frame_msb_cleared(&mut c);
        }
        let flags = c.video_state().flags;
        let high = flags & 0x80 != 0;
        c.run(0xca60, 5)?; // cmp; je; cmp; jns; then a call either way
        if high {
            c.call(code::HNM_FRAME_END, 0xca71)?;
            c.run(0xca71, 1)?;
        }
        if !hnm::frame_decoded(&mut c, 0xca74)? {
            c.run(0xca74, 2)?;
            c.call(code::HNM_PREFETCH, 0xca79)?;
            c.run(0xca79, 1)?; // jmp CA60h
            continue;
        }
        c.run(0xca74, 2)?;
        if frame_due(&mut c, 0xca7e)? {
            let pal = c.video_state().palette_chunk;
            c.run(0xca7e, 5)?; // jb; mov; inc; je; then a call either way
            c.set_reg(AX, pal);
            let ax = pure::inc(W, pal);
            c.set_reg(AX, ax);
            if pal != 0xffff {
                c.call(code::HNM_HANDLE_PAL_CHUNK, 0xca89)?;
                c.run(0xca89, 1)?;
            }
            c.call(code::HNM_DECODE_VIDEO_FRAME, 0xca8c)?;
            c.run(0xca8c, 1)?;
            c.call(code::HNM_NEXT_FRAME, 0xca8f)?;
        } else {
            c.run(0xca7e, 1)?;
        }
        // CA8F. The timer's handler reads [0DBB5h] (whether it renders a
        // frame): it is written right after a stretch of its `mov` alone
        // (see `ticks.rs`).
        c.run(0xca8f, 2)?;
        let flags = c.video_state().flags;
        let msb = pure::and(B, flags as u16, 0x80) as u8;
        c.set_reg8(r8::AL, msb);
        c.run(0xca94, 1)?;
        c.set_byte(g::HNM_FLAG_MSB, msb);
        c.run(0xca97, 1)?;
        c.call(code::HNM_PREFETCH, 0xca9a)?;
        return frame_msb_cleared(&mut c);
    }
}

/// CA9Ah: [0DBB5h] cleared (right after its stretch: see above); `ret`.
fn frame_msb_cleared(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xca9a, 1)?;
    c.set_byte(g::HNM_FLAG_MSB, 0);
    c.run(0xca9f, 1)?;
    c.ret()
}

/// From C1BA: applies the palette chunks at ES:SI (from 0, start colour
/// and count; count 0 is 256; 0100h skips 3 bytes; FFFFh ends) through
/// driver function 2. CX, DX, DI kept.
fn palette_chunks(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xc1ba, 3)?;
    for r in [CX, DX, DI] {
        let v = c.reg(r);
        c.push(v);
    }
    loop {
        let si = c.reg(SI);
        let w = c.word_in(ES, si);
        if w == 0x0100 {
            c.run(0xc1bd, 5)?;
            c.lods(W, ES);
            c.cmp(W, w, 0x100);
            let si = c.reg(SI);
            let si = c.add(W, si, 3);
            c.set_reg(SI, si);
            continue;
        }
        if w == 0xffff {
            c.run(0xc1bd, 6 + 4)?;
            c.lods(W, ES);
            c.cmp(W, w, 0x100);
            c.set_reg(BX, w);
            let ax = c.inc(W, w);
            c.set_reg(AX, ax);
            for r in [DI, DX, CX] {
                let v = c.pop();
                c.set_reg(r, v);
            }
            return c.ret();
        }
        let count = w >> 8;
        c.run(0xc1bd, 10 + if count == 0 { 1 } else { 0 } + 8)?;
        c.lods(W, ES);
        c.cmp(W, w, 0x100);
        c.set_reg(BX, w);
        let ax = c.inc(W, w);
        c.set_reg(AX, ax);
        c.set_reg8(r8::CL, count as u8);
        c.xor(B, 0, 0);
        c.set_reg(BX, w & 0xff);
        let cx = c.reg(CX);
        let mut cx = c.and(W, cx, 0xff);
        c.set_reg(CX, cx);
        if cx == 0 {
            cx = 0x100;
            c.set_reg8(r8::CH, 1);
            c.inc(B, 0);
        }
        let first = w & 0xff;
        c.set_reg(AX, first);
        let b2 = c.add(W, first, first);
        let bx = c.add(W, b2, first);
        c.set_reg(BX, bx);
        c.set_reg(AX, cx);
        let c2 = c.add(W, cx, cx);
        let bytes = c.add(W, c2, cx);
        c.set_reg(CX, bytes);
        let si = c.reg(SI);
        c.set_reg(DX, si);
        let si = c.add(W, si, bytes);
        c.set_reg(SI, si);
        c.interp(0xc1ea, 0xc1ee)?;
        c.run(0xc1ee, 1)?;
    }
}

/// `apply_palette` (C1BA).
pub fn apply_palette(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc1ba)?;
    palette_chunks(&mut Ctx::new(m))
}

/// `apply_sprite_sheet_palette` (C1AA): the sheet's palette (its chunks
/// from offset 2), unless it was the last applied.
pub fn apply_sprite_sheet_palette(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc1aa)?;
    let mut c = Ctx::new(m);
    let id = c.word(g::RESOURCE_ID);
    let last = c.byte(g::PALETTE_RESOURCE);
    let same = last == id as u8;
    c.run(start, 6)?; // mov; mov; xchg; cmp; je; then ret, or mov si, 2
    c.set_reg(AX, (id & 0xff) << 8 | last as u16);
    c.set_byte(g::PALETTE_RESOURCE, id as u8);
    c.cmp(B, last as u16, id & 0xff);
    if same {
        return c.ret();
    }
    c.set_reg(SI, 2);
    palette_chunks(&mut c)
}

/// `find_last_numeric_digit_in_str_at_es_si` (D03C): SI to the last digit
/// of the first number at ES:SI. Exit: AL = the next character - 30h.
pub fn find_last_numeric_digit(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd03c)?;
    let mut c = Ctx::new(m);
    for (at, digits) in [(0xd03c, false), (0xd044, true)] {
        loop {
            let si = c.reg(SI);
            let d = c.byte_in(ES, si).wrapping_sub(0x30);
            let more = (d <= 9) == digits;
            let last = digits && !more;
            c.run(at, 4 + if last { 2 } else { 0 })?;
            let ch = c.lods(B, ES);
            let d = pure::sub(B, ch, 0x30);
            c.set_reg8(r8::AL, d as u8);
            if !more {
                break;
            }
        }
    }
    let si = c.reg(SI);
    let si = pure::dec(W, si);
    c.set_reg(SI, si);
    c.ret()
}

/// `skip_zero_words`: skips the zero words at SI (at most CX): AX = 0 and BX past
/// the first non-zero one, or AX = FFFFh if none. Flags from the `dec`.
pub fn skip_zero_words(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc827)?;
    let mut c = Ctx::new(m);
    loop {
        let (si, cx) = (c.reg(SI), c.reg(CX));
        let w = c.word_in(DS, si);
        let left = cx.wrapping_sub(1);
        let again = w == 0 && left != 0;
        c.run(0xc827, 3 + if again { 0 } else if w == 0 { 3 } else { 5 })?;
        c.lods(W, DS);
        c.set_reg(CX, left);
        if again {
            continue;
        }
        if w != 0 {
            let si = c.reg(SI);
            c.set_reg(BX, si);
            c.set_reg(AX, 1);
        }
        let ax = c.reg(AX);
        let ax = c.dec(W, ax);
        c.set_reg(AX, ax);
        return c.ret();
    }
}

/// `loc_0c835` (the map's at [2786h]): of the CX records pointed to by
/// the words at SI (0: none), the one lowest down, by its bottom (word 4
/// plus word 6; 7FFFh with bit 40h of its byte 0Ch): DX that bottom
/// (FFFFh for none), BX past its word. Flags from `or dx, dx`.
pub fn lowest_record(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc835)?;
    let mut c = Ctx::new(m);
    if c.reg(CX) == 0 {
        // `loop` would go round 65536 times: left to the original.
        return Err(Bail);
    }
    c.run(start, 1)?;
    let mut dx = 0xffffu16;
    c.set_reg(DX, dx);
    loop {
        let si = c.reg(SI);
        let p = c.word_in(DS, si);
        let left = c.reg(CX).wrapping_sub(1);
        if p == 0 {
            c.run(0xc838, 4)?;
            c.lods(W, DS);
            c.set_reg(CX, left);
        } else {
            let bottom = c.word(p.wrapping_add(4)).wrapping_add(c.word(p.wrapping_add(6)));
            let hidden = c.byte(p.wrapping_add(0x0c)) & 0x40 != 0;
            let v = if hidden { 0x7fff } else { bottom };
            let lower = v <= dx;
            c.run(0xc838, 3 + 5 + hidden as u32 + 2 + 2 * lower as u32 + 1)?;
            c.lods(W, DS);
            c.set_reg(DI, p);
            c.set_reg(AX, v);
            if lower {
                dx = v;
                c.set_reg(DX, dx);
                c.set_reg(BX, si.wrapping_add(2));
            }
            c.set_reg(CX, left);
        }
        if left == 0 {
            break;
        }
    }
    c.run(0xc858, 2)?;
    c.or(W, dx, dx);
    c.ret()
}

/// `redraw_frieze_screen`: a cleared screen with the 12h UI elements at 1AE6h,
/// `sub_d397`, `sub_b039`; then [479Eh] = 0.
pub fn redraw_frieze_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb23f)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::GFX_CLEAR_ACTIVE_FRAMEBUFFER, 0xb242)?;
    c.run(0xb242, 3)?;
    gfx::draw_ui_elements(&mut c, g::FRIEZE_ELEMENTS, 0x12, 0xb24b)?;
    c.run(0xb24b, 1)?;
    c.call(code::SUB_D397, 0xb24e)?;
    c.run(0xb24e, 1)?;
    c.call(code::SUB_B039, 0xb251)?;
    c.run(0xb251, 3)?; // jmp 9901h; mov word [479Eh], 0; ret
    c.update_text_box(|t| t.shown = 0);
    c.ret()
}

/// `handle_ctrl_v_once` (B270): appends the 10 words at 242Ah to the
/// table in the code segment at [11BDh] (with a 0 after them), then makes
/// itself a bare `ret`: it runs once. The port reads its own first byte
/// where it runs.
pub fn handle_ctrl_v_once(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb270)?;
    let mut c = Ctx::new(m);
    if c.m.cpu.read8(&mut c.m.hw, CS, 0xb270) == 0xc3 {
        c.run(start, 1)?;
        return c.ret();
    }
    if c.m.cpu.read8(&mut c.m.hw, CS, 0xb270) != 0xbe {
        return Err(Bail); // patched otherwise: not this code
    }
    c.run(start, 5)?;
    c.set_reg(SI, 0x242a);
    let di = c.word(g::CTRL_V_TABLE_END);
    c.set_reg(DI, di);
    let cs = c.sreg(CS);
    c.push(cs);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(CX, 0x0a);
    let iterations = c.reg(CX) as u32; // the `rep` below
    c.run(0xb27c, 1 + iterations + 5)?;
    c.rep_counted(0xa5); // rep movsw
    let di = c.reg(DI);
    c.set_word(g::CTRL_V_TABLE_END, di);
    let ax = c.reg(AX);
    let zero = pure::xor(W, ax, ax);
    c.set_reg(AX, zero);
    c.stos(W);
    c.m.cpu.write8(&mut c.m.hw, CS, 0xb270, 0xc3);
    c.ret()
}
