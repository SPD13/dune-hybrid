//! Game time: day, time of day, the sky's palette.

use super::entry;
use super::pure;
use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BX, CX},
    rec::Bail,
};

/// `get_ingame_time_of_day` (1AE0): AX = the time-of-day slot (0..15).
/// Flags from `and ax, 0Fh`.
pub fn get_ingame_time_of_day(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1ae0)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let slot = c.clock().time_of_day();
    c.set_reg(AX, slot);
    c.ret()
}

/// `get_ingame_day_q` (1AC5): AX = the game time / 16 (the day, counted
/// from 0). Flags from the last of four `shr ax, 1`.
pub fn get_ingame_day_q(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1ac5)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let day = c.clock().day_from_0();
    c.set_reg(AX, day);
    c.ret()
}

/// [`get_ingame_day_q`] called: the day (game time / 16, from 0; AX).
pub(super) fn game_day(c: &mut Ctx, ret: u16) -> Result<u16, Bail> {
    c.call(0x1ac5, ret)?;
    Ok(c.reg(AX))
}

/// `get_ingame_day_in_ax` (1AD1): AX = (game time + 3) / 16, the day as the
/// game shows it (a day turns 3 slots early). Flags from the last `shr`.
pub fn get_ingame_day(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1ad1)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let day = c.clock().day();
    c.set_reg(AX, day);
    c.ret()
}

/// `get_sky_palette_id_from_game_time_in_bl` (395C): the sky palette for
/// the current game time: the time-of-day slot's base palette plus 4 × the
/// palette group.
///
/// Exit: BL = AL = the palette id, AH = 4 × group, BH = 22h (the table's
/// page). Flags from the final addition.
pub fn sky_palette_for_game_time(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x395c)?;
    let mut c = Ctx::new(m);
    c.run(start, 10)?;
    let time = c.game_time();
    // mov ah, al; shr ah, 1 (twice); and ax, 1C0Fh: the flags of the `and`
    // only depend on its result.
    let ah = (time as u8 >> 2) as u16;
    let ax = pure::and(W, ah << 8 | time & 0xff, 0x1c0f);
    let (slot, group4) = (ax & 0xff, ax >> 8);
    let base = c.byte(g::SKY_PALETTE_BY_SLOT + slot);
    let id = pure::add(B, base as u16, group4) as u8;
    c.set_reg(AX, group4 << 8 | id as u16);
    c.set_reg(BX, g::SKY_PALETTE_BY_SLOT & 0xff00);
    c.set_reg8(r8::BL, id);
    c.ret()
}


/// `loc_0b2b9`: the clock stopped once more.
pub fn stop_clock(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb2b9)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.update_clock(|k| k.stop());
    c.ret()
}

/// `loc_0b2b3`: one stop less (not below 0).
pub fn release_clock(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb2b3)?;
    let mut c = Ctx::new(m);
    let below = c.clock().stopped.wrapping_sub(1) & 0x80 != 0;
    c.run(start, if below { 4 } else { 3 })?;
    c.update_clock(|k| k.release());
    c.ret()
}

/// `loc_0b2be`: the clock running.
pub fn start_clock(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb2be)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.update_clock(|k| k.start());
    c.ret()
}


/// `loc_00fd9`: the game clock run CX steps at once (the clock started;
/// each step the countdown restarted, the pending step run
/// (`loc_01b23`), the game time on and its step run).
pub fn run_clock_ticks(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0fd9)?;
    let mut c = Ctx::new(m);
    clock_steps(&mut c, 0x0fd9)
}

/// The leaders after `run_clock_ticks`' calls, where the interpreter can
/// come back to Rust when a call gave up partway: the rest of the step
/// and the steps after it.
pub fn run_clock_ticks_at(m: &mut Machine, start: u16) -> Result<(), Bail> {
    match start {
        0x0fe1 | 0x0ff6 | 0x1002 => {
            let mut c = Ctx::new(m);
            clock_steps(&mut c, start)
        }
        _ => Err(Bail),
    }
}

/// `run_clock_ticks` from `at` (its entry or a leader after a call).
fn clock_steps(c: &mut Ctx, mut at: u16) -> Result<(), Bail> {
    loop {
        match at {
            0x0fd9 => {
                c.run(0x0fd9, 2)?;
                c.set_byte(g::CLOCK_TICKING, 1);
                c.call(entry::START_CLOCK, 0x0fe1)?;
                at = 0x0fe1;
            }
            0x0fe1 => {
                let cx = c.reg(CX);
                if cx as i16 <= 0 {
                    c.run(0x0fe1, 4)?;
                    c.set_byte(g::CLOCK_TICKING, 0);
                    return c.ret();
                }
                let pending = c.clock().steps_pending != 0;
                c.run(0x0fe1, 7 + pending as u32)?;
                c.push(cx);
                let reset = c.clock().period;
                c.set_reg(AX, reset);
                c.update_clock(|k| k.restart_countdown());
                if pending {
                    c.call(0x1b23, 0x0ff6)?;
                }
                at = 0x0ff6;
            }
            0x0ff6 => {
                c.run(0x0ff6, 3)?;
                c.update_clock(|k| k.advance());
                c.call(0x1b23, 0x1002)?;
                at = 0x1002;
            }
            _ => {
                // 1002h: the next step, or done.
                let sp = c.reg(pc::cpu::SP);
                let cx = c.word_in(pc::cpu::SS, sp).wrapping_sub(1);
                if cx == 0 {
                    c.run(0x1002, 4)?;
                    c.pop();
                    c.set_reg(CX, 0);
                    c.set_byte(g::CLOCK_TICKING, 0);
                    return c.ret();
                }
                c.run(0x1002, 2)?;
                c.pop();
                c.set_reg(CX, cx);
                at = 0x0fd9;
            }
        }
    }
}
