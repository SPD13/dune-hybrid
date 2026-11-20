//! The timer interrupt (INT 8, `pit_timer_callback`) and the video frame
//! step it runs (`hnm_render_timer_callback`). The handler is entered by
//! the interrupt, not by a call: the shadow check never sees it, `--rec
//! on` runs it (checked by whole-run state hashes, `STATE_HASH=1`).

use super::frame;
use super::entry;
use super::globals as g;
use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CS, CX, DI, DS, DX, ES, IF, SI, ZF},
    rec::Bail,
};


/// `pit_timer_callback` (EF6A), the INT 8 handler: unless [0CEEAh] is
/// above 0 (the clock held), the ticks [0CE7Ah] (with [0CE7Ch] above)
/// counted, and, unless [2788h], the half-minute countdown [46DBh]: at
/// its end it restarts from [146Eh] and the game time [2] and [46DDh] go
/// on; then the frame tasks (`loc_0efba`). Every tenth tick or so
/// ([0CE72h] counted down from CS:[0EFD9h]) the BIOS's own handler is
/// chained (left to the interpreter); else the PIC's EOI and, with a
/// video frame due ([0DBB5h]), `hnm_render_timer_callback`.
pub fn pit_timer_callback(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xef6a)?;
    let mut c = Ctx::new(m);
    let clock = c.clock();
    let held = clock.is_held();
    c.run(0xef6a, 8)?;
    let ax = c.reg(AX);
    c.push(ax);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.sreg(ES);
    c.push(es);
    c.set_reg(AX, g::GAME_DS);
    c.set_sreg(DS, g::GAME_DS);
    c.set_flag(pc::cpu::DF, false);
    // Its CF (clear) reaches the BIOS's handler when held: the `dec`
    // before the chain keeps CF (else the frame tasks' CF).
    if !held {
        timer_tick(&mut c)?;
    }
    timer_end(&mut c)
}

/// EF7Ah, where the interpreter can come back to Rust after the handler
/// gave up at its start: the clock's tick and the rest.
pub fn pit_timer_tick(m: &mut Machine, start: u16) -> Result<(), Bail> {
    if start != 0xef7a {
        return Err(Bail);
    }
    let mut c = Ctx::new(m);
    timer_tick(&mut c)?;
    timer_end(&mut c)
}

/// EF7Ah: the clock's tick ([`super::clock::Clock::tick`]), the cost by
/// the original's path through it; then the frame tasks.
fn timer_tick(c: &mut Ctx) -> Result<(), Bail> {
    let mut clock = c.clock();
    let wraps = clock.ticks as u16 == 0xffff;
    let frozen = clock.stopped != 0;
    let restart = !frozen && clock.countdown.wrapping_sub(1) & 0x8000 != 0;
    c.run(0xef7a, 2 + wraps as u32 + 2 + if frozen { 0 } else { 2 + 4 * restart as u32 } + 1)?;
    if clock.tick() {
        c.set_reg(AX, clock.period);
    }
    clock.set(c);
    c.call(entry::PIT_TIMER_CALLBACK_MIDI, 0xefa2)
}

/// EFA2h on: the BIOS's handler chained every tenth tick or so, else the
/// PIC's EOI and a video frame.
fn timer_end(c: &mut Ctx) -> Result<(), Bail> {
    let k = c.byte(g::BIOS_TICK_COUNTDOWN).wrapping_sub(1);
    c.run(0xefa2, 3)?;
    let es = c.pop();
    c.set_sreg(ES, es);
    let k = c.dec(B, k.wrapping_add(1) as u16) as u8; // the BIOS's handler sees the flags
    c.set_byte(g::BIOS_TICK_COUNTDOWN, k);
    if k & 0x80 != 0 {
        // EFD5h: the count restarted, DS and AX back, and on to the BIOS's
        // handler by a far jump whose target the program wrote into its
        // code (at EFDDh, the old INT 8 vector). The flags are the `dec`'s.
        c.run(0xefd5, 4)?;
        c.set_byte(g::BIOS_TICK_COUNTDOWN, 0x0a);
        let ds = c.pop();
        c.set_sreg(DS, ds);
        let ax = c.pop();
        c.set_reg(AX, ax);
        let ip = c.code_word(0xefdd);
        let cs = c.code_word(0xefdf);
        c.m.cpu.ip = ip;
        c.set_sreg(CS, cs);
        return Ok(());
    }
    let frame = c.byte(g::HNM_FLAG_MSB);
    c.check(0xefa9, 4 + (frame != 0) as u32)?;
    c.set_reg8(r8::AL, 0x20);
    c.count(1);
    c.out(0x20, 0x20);
    c.count(3 + (frame != 0) as u32);
    c.cmp(B, frame as u16, 0);
    if frame != 0 {
        c.call(entry::HNM_RENDER_TIMER_CALLBACK, 0xefb7)?;
    }
    c.run(0xefb7, 3)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.iret()
}

/// `hnm_render_timer_callback` (CEC9), from the timer with a frame due:
/// [0DBB5h] cleared meanwhile (restored at the end), interrupts on; when
/// `loc_0caa0` gives above and the frame [0DC1Eh] is FFFFh, unless
/// `loc_0cad4` gives CF, the frame decoded (`hnm_decode_video_frame`) and
/// shown (`loc_0cc4e`). All registers but AX kept, the flags too.
pub fn hnm_render_timer_callback(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcec9)?;
    let mut c = Ctx::new(m);
    c.run(0xcec9, 13)?;
    let f = c.m.cpu.flags();
    c.push(f);
    for r in [BX, CX, DX, SI, DI, BP] {
        let v = c.reg(r);
        c.push(v);
    }
    let es = c.sreg(ES);
    c.push(es);
    let due = c.byte(g::HNM_FLAG_MSB);
    c.set_byte(g::HNM_FLAG_MSB, 0);
    c.set_reg(AX, due as u16);
    // STI; its one-instruction hold is over: the next instruction is
    // already in the `run` above.
    c.set_flag(IF, true);
    c.push(due as u16);
    c.call(entry::HNM_DECODE_FRAME_IF_READ, 0xcedc)?;
    if c.flag(CF) || c.flag(ZF) {
        c.run(0xcedc, 1)?;
    } else {
        let f = c.video_state().palette_chunk.wrapping_add(1);
        c.run(0xcedc, 4 + (f == 0) as u32)?;
        c.set_reg(AX, f.wrapping_sub(1));
        let ax = c.inc(W, f.wrapping_sub(1));
        c.set_reg(AX, ax);
        if f == 0 {
            let frame_due = frame::frame_due(&mut c, 0xcee7)?;
            c.run(0xcee7, if !frame_due { 1 } else { 2 })?;
            if frame_due {
                c.call(entry::HNM_DECODE_VIDEO_FRAME, 0xceec)?;
                c.run(0xceec, 1)?;
                c.call(entry::HNM_CONSUME_CHUNK, 0xceef)?;
            }
        }
    }
    c.run(0xceef, 11)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.set_byte(g::HNM_FLAG_MSB, ax as u8);
    let es = c.pop();
    c.set_sreg(ES, es);
    for r in [BP, DI, SI, DX, CX, BX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    let f = c.pop();
    c.m.cpu.set_flags(f);
    c.ret()
}
