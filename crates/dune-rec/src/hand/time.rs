//! Game time: day, time of day, the sky's palette.

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
    let time = c.word(g::GAME_TIME);
    let slot = pure::and(W, time, 0x0f);
    c.set_reg(AX, slot);
    c.ret()
}

/// `get_ingame_day_q` (1AC5): AX = the game time / 16 (the day, counted
/// from 0). Flags from the last of four `shr ax, 1`.
pub fn get_ingame_day_q(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1ac5)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let time = c.word(g::GAME_TIME);
    let day = pure::shr1(W, time, 4);
    c.set_reg(AX, day);
    c.ret()
}

/// `get_ingame_day_in_ax` (1AD1): AX = (game time + 3) / 16, the day as the
/// game shows it (a day turns 3 slots early). Flags from the last `shr`.
pub fn get_ingame_day(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1ad1)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let time = c.word(g::GAME_TIME);
    let t = pure::add(W, time, 3);
    let day = pure::shr1(W, t, 4);
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
    let time = c.word(g::GAME_TIME);
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

/// While not 0, the game's clock does not run (the timer interrupt skips
/// it): a count of who stopped it.
const CLOCK_STOPPED: u16 = 0x2788;

/// `loc_0b2b9`: the clock stopped once more.
pub fn stop_clock(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb2b9)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let n = c.byte(CLOCK_STOPPED);
    let n = pure::inc(B, n as u16);
    c.set_byte(CLOCK_STOPPED, n as u8);
    c.ret()
}

/// `loc_0b2b3`: one stop less (not below 0).
pub fn release_clock(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb2b3)?;
    let mut c = Ctx::new(m);
    let n = c.byte(CLOCK_STOPPED);
    let below = n.wrapping_sub(1) & 0x80 != 0;
    c.run(start, if below { 4 } else { 3 })?;
    let n = pure::dec(B, n as u16);
    let n = if below { pure::inc(B, n) } else { n };
    c.set_byte(CLOCK_STOPPED, n as u8);
    c.ret()
}

/// `loc_0b2be`: the clock running.
pub fn start_clock(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb2be)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(CLOCK_STOPPED, 0);
    c.ret()
}

/// Set while [`run_clock_ticks`] runs.
const TICKING: u16 = 0x46da;

/// `loc_00fd9`: the game clock run CX ticks at once (the clock started;
/// each tick the countdown reset from [146Eh], the game time up one, and
/// `loc_01b23` once or twice with [46DDh]).
pub fn run_clock_ticks(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0fd9)?;
    let mut c = Ctx::new(m);
    loop {
        c.run(0x0fd9, 2)?;
        c.set_byte(TICKING, 1);
        c.call(0xb2be, 0x0fe1)?;
        let cx = c.reg(CX);
        if cx as i16 <= 0 {
            c.run(0x0fe1, 4)?;
            c.set_byte(TICKING, 0);
            return c.ret();
        }
        let pending = c.byte(0x46dd) != 0;
        c.run(0x0fe1, 7 + pending as u32)?;
        c.push(cx);
        let reset = c.word(0x146e);
        c.set_reg(AX, reset);
        c.set_word(0x46db, reset);
        if pending {
            c.call(0x1b23, 0x0ff6)?;
            c.run(0x0ff6, 3)?;
        } else {
            c.run(0x0ff6, 3)?;
        }
        let t = c.word(g::GAME_TIME);
        let t = pure::inc(W, t);
        c.set_word(g::GAME_TIME, t);
        c.set_byte(0x46dd, 1);
        c.call(0x1b23, 0x1002)?;
        let sp = c.reg(pc::cpu::SP);
        let cx = c.word_in(pc::cpu::SS, sp).wrapping_sub(1);
        if cx == 0 {
            c.run(0x1002, 4)?;
            c.pop();
            c.set_reg(CX, 0);
            c.set_byte(TICKING, 0);
            return c.ret();
        }
        c.run(0x1002, 2)?;
        c.pop();
        c.set_reg(CX, cx);
    }
}
