//! The timer interrupt (INT 8, `pit_timer_callback`) and the video frame
//! step it runs (`hnm_render_timer_callback`). The handler is entered by
//! the interrupt, not by a call: the shadow check never sees it, `--rec
//! on` runs it (checked by whole-run state hashes, `STATE_HASH=1`).

use super::globals as g;
use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DS, DX, ES, IF, SI, ZF},
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
    let held = c.byte_at_seg(g::GAME_DS, 0xceea) as i8 > 0;
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
    let v = c.byte(0xceea);
    c.cmp(B, v as u16, 0);
    if !held {
        let t = c.word(g::TIMER).wrapping_add(1);
        let wraps = t == 0;
        let fz = c.byte(g::CLOCK_STOPPED);
        let frozen = fz != 0;
        let n = c.word(g::CLOCK_COUNTDOWN).wrapping_sub(1);
        let restart = !frozen && n & 0x8000 != 0;
        c.run(0xef7a, 2 + wraps as u32 + 2 + if frozen { 0 } else { 2 + 4 * restart as u32 } + 1)?;
        let t = c.inc(W, t.wrapping_sub(1));
        c.set_word(g::TIMER, t);
        if wraps {
            let h = c.word(g::TIMER_HIGH);
            let h = c.inc(W, h);
            c.set_word(g::TIMER_HIGH, h);
        }
        c.cmp(B, fz as u16, 0);
        if !frozen {
            let n = c.dec(W, n.wrapping_add(1));
            c.set_word(g::CLOCK_COUNTDOWN, n);
            if restart {
                let p = c.word(g::CLOCK_PERIOD);
                c.set_reg(AX, p);
                c.set_word(g::CLOCK_COUNTDOWN, p);
                let time = c.word(g::GAME_TIME);
                let time = c.inc(W, time);
                c.set_word(g::GAME_TIME, time);
                let d = c.byte(g::CLOCK_STEPS_PENDING);
                let d = c.inc(B, d as u16);
                c.set_byte(g::CLOCK_STEPS_PENDING, d as u8);
            }
        }
        c.call(0xefba, 0xefa2)?;
    }
    let k = c.byte(0xce72).wrapping_sub(1);
    c.run(0xefa2, 3)?;
    let es = c.pop();
    c.set_sreg(ES, es);
    let k = c.dec(B, k.wrapping_add(1) as u16) as u8;
    c.set_byte(0xce72, k);
    if k & 0x80 != 0 {
        // Chain to the BIOS's handler: the far jump's target is code the
        // program rewrote; the interpreter takes it from EFD5h.
        c.m.cpu.ip = 0xefd5;
        return Err(Bail);
    }
    let frame = c.byte(g::HNM_FLAG_MSB);
    c.check(0xefa9, 4 + (frame != 0) as u32)?;
    c.set_reg8(r8::AL, 0x20);
    c.count(1);
    c.out(0x20, 0x20);
    c.count(3 + (frame != 0) as u32);
    c.cmp(B, frame as u16, 0);
    if frame != 0 {
        c.call(0xcec9, 0xefb7)?;
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
    c.xor(W, 0, 0);
    let due = c.byte(g::HNM_FLAG_MSB);
    c.set_byte(g::HNM_FLAG_MSB, 0);
    c.set_reg(AX, due as u16);
    // STI; its one-instruction hold is over: the next instruction is
    // already in the `run` above.
    c.set_flag(IF, true);
    c.push(due as u16);
    c.call(0xcaa0, 0xcedc)?;
    if c.flag(CF) || c.flag(ZF) {
        c.run(0xcedc, 1)?;
    } else {
        let f = c.word(g::HNM_PAL_CHUNK).wrapping_add(1);
        c.run(0xcedc, 4 + (f == 0) as u32)?;
        c.set_reg(AX, f.wrapping_sub(1));
        let ax = c.inc(W, f.wrapping_sub(1));
        c.set_reg(AX, ax);
        if f == 0 {
            c.call(0xcad4, 0xcee7)?;
            if c.flag(CF) {
                c.run(0xcee7, 1)?;
            } else {
                c.run(0xcee7, 2)?;
                c.call(0xcc96, 0xceec)?;
                c.run(0xceec, 1)?;
                c.call(0xcc4e, 0xceef)?;
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
