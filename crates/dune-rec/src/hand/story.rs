//! Story progress, the intro script, and one-line flag setters.

use super::{B, Ctx, W, entered, globals as g};
use pc::{
    Machine,
    cpu::{AX, BX, SI, Width},
    rec::Bail,
};

/// Code offsets.
mod code {
    /// Called when the phase changes, before its handler.
    pub const ON_PHASE_CHANGE: u16 = 0xb17a;
    /// Phase handlers: a word table in the code segment, one per 4 phases.
    pub const PHASE_HANDLERS: u16 = 0x11e7;
    /// The last phase with a handler.
    pub const LAST_HANDLED_PHASE: u8 = 0x6c;
    pub const INCREASE_CHARISMA_BY_AL: u16 = 0x6f78;
    pub const GET_INGAME_DAY_Q: u16 = 0x1ac5;
}

/// A bare `ret`: stubs left in the program.
pub fn nothing(m: &mut Machine, start: u16) -> Result<(), Bail> {
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.ret()
}

/// `intro_script_set_offset` (0945): the intro script goes on at SI.
pub fn intro_script_set_offset(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0945)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let si = c.reg(SI);
    c.set_word(g::SCENE_SEQUENCE_OFFSET, si);
    c.ret()
}

/// Generates a port for `mov byte [flag], value; ret`.
macro_rules! set_flag {
    ($(#[$doc:meta])* $name:ident, $entry:expr, $flag:expr, $value:expr) => {
        $(#[$doc])*
        pub fn $name(m: &mut Machine, start: u16) -> Result<(), Bail> {
            entered(start, $entry)?;
            let mut c = Ctx::new(m);
            c.run(start, 2)?;
            c.set_byte($flag, $value);
            c.ret()
        }
    };
}

set_flag!(
    /// `sub_10b2`: [0FFBh] = 1.
    set_flag_0ffb, 0x10b2, g::FLAG_0FFB, 1
);
set_flag!(
    /// `sub_1127`: [0FDBh] = FFh.
    set_flag_0fdb, 0x1127, g::FLAG_0FDB, 0xff
);
set_flag!(
    /// `sub_a1d6`: [47A5h] = 0.
    clear_flag_47a5, 0xa1d6, g::FLAG_47A5, 0
);
set_flag!(
    /// `sub_a5aa`: [28BEh] = 0.
    clear_flag_28be, 0xa5aa, g::FLAG_28BE, 0
);

/// `sub_112d`: sets bit 1 of [1067h]. Flags from the `or`.
pub fn set_bit_1067(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x112d)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let bits = c.byte(g::BITS_1067);
    let bits = c.or(B, bits as u16, 2) as u8;
    c.set_byte(g::BITS_1067, bits);
    c.ret()
}

/// `set_game_phase` (121F): raise the game phase to AL. Phases only go up:
/// a lower or equal AL changes nothing. On a change the phase-change hook
/// runs, then the new phase's handler if it has one (phases up to 6Ch, one
/// handler per 4 phases).
///
/// Exit: AL as given; BX = the handler index if one ran. Flags: from the
/// comparison that ended it, or as the handler left them.
pub fn set_game_phase(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x121f)?;
    let mut c = Ctx::new(m);
    let new = c.reg8(super::r8::AL);
    let current = c.byte(g::GAME_PHASE);
    c.run(0x121f, 2)?; // cmp al, [2Ah]; jbe
    c.cmp(B, new as u16, current as u16);
    if new <= current {
        c.run(0x1242, 1)?;
        return c.ret();
    }
    c.run(0x1225, 3)?; // mov [2Ah], al; mov byte [0FFh], 0; call
    c.set_byte(g::GAME_PHASE, new);
    c.set_byte(g::PHASE_CHANGED_FLAG, 0);
    c.call(code::ON_PHASE_CHANGE, 0x1230)?;

    // The hook may have moved the phase on: read it again.
    let phase = c.byte(g::GAME_PHASE);
    c.run(0x1230, 3)?; // mov bl, [2Ah]; cmp bl, 6Ch; ja
    c.set_reg8(super::r8::BL, phase);
    c.cmp(B, phase as u16, code::LAST_HANDLED_PHASE as u16);
    if phase > code::LAST_HANDLED_PHASE {
        c.run(0x1242, 1)?;
        return c.ret();
    }
    c.run(0x1239, 3)?; // xor bh, bh; shr bx, 1; call cs:[bx+11E7h]
    let index = (phase >> 1) as u16;
    c.set_reg(BX, phase as u16);
    c.m.cpu.logic(Width::B, 0); // xor bh, bh
    let bx = c.shr1(Width::W, phase as u16, 1);
    c.set_reg(BX, bx);
    let handler = c.code_word(code::PHASE_HANDLERS.wrapping_add(index));
    c.call(handler, 0x1242)?;
    c.run(0x1242, 1)?;
    c.ret()
}

/// `sub_1011`: [122Ah] -= 1, `sub_105b` with SI = 1021h, then [0FE8h] = 9.
pub fn sub_1011(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1011)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let v = c.byte(g::COUNTER_122A);
    let v = c.dec(B, v as u16) as u8;
    c.set_byte(g::COUNTER_122A, v);
    c.set_reg(SI, 0x1021);
    c.call(0x105b, 0x101b)?;
    c.run(0x101b, 2)?;
    set_0fe8(&mut c)
}

/// `mov byte [0FE8h], 9; ret` (101B).
fn set_0fe8(c: &mut Ctx) -> Result<(), Bail> {
    c.set_byte(g::BYTE_0FE8, 9);
    c.ret()
}

/// `sub_117b`: sets bit 40h of [0Ah], charisma += 28h, then [0FE8h] = 9.
pub fn sub_117b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x117b)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let bits = c.byte(g::STORY_BITS_000A);
    let bits = c.or(B, bits as u16, 0x40) as u8;
    c.set_byte(g::STORY_BITS_000A, bits);
    c.set_reg8(super::r8::AL, 0x28);
    c.call(code::INCREASE_CHARISMA_BY_AL, 0x1185)?;
    c.run(0x1185, 3)?; // jmp 101Bh; mov; ret
    set_0fe8(&mut c)
}

/// `sub_11b3`: [1038h] = 5, [11D0h] += 0Ch, [1156h] = the day + 3,
/// [1141h] += 1. Flags from the `inc`; AX = the day + 3.
pub fn sub_11b3(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x11b3)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_byte(g::BYTE_1038, 5);
    let v = c.byte(g::BYTE_11D0);
    let v = c.add(B, v as u16, 0x0c) as u8;
    c.set_byte(g::BYTE_11D0, v);
    c.call(code::GET_INGAME_DAY_Q, 0x11c0)?;
    c.run(0x11c0, 4)?;
    let day = c.reg(AX);
    let day = c.add(W, day, 3);
    c.set_reg(AX, day);
    c.set_word(g::WORD_1156, day);
    let v = c.byte(g::BYTE_1141);
    let v = c.inc(B, v as u16) as u8;
    c.set_byte(g::BYTE_1141, v);
    c.ret()
}

/// Troop record fields.
mod troop {
    pub const SIZE: u16 = 0x1b;
    /// Flags: 80h or 20h, not motivated by charisma.
    pub const FLAGS: u16 = 0x03;
    pub const MOTIVATION: u16 = 0x15;
    pub const MAX_MOTIVATION: u8 = 0x64;
}

/// The most charisma there is.
const MAX_CHARISMA: u8 = 0xc8;

/// `increase_charisma_by_al` (6F78): charisma += AL (at most C8h). Every
/// 4 points crossed add one to each free troop's motivation (at most 64h).
/// Exit: AL = the number of steps crossed; flags from its computation, or
/// from the last troop's check.
pub fn increase_charisma_by_al(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use super::r8::{AH, AL};
    entered(start, 0x6f78)?;
    let mut c = Ctx::new(m);
    c.check(start, 11)?;
    let old = c.byte(g::CHARISMA);
    c.set_reg8(AH, old);
    let gain = c.reg8(AL);
    let mut new = c.add(B, gain as u16, old as u16) as u8;
    c.cmp(B, new as u16, MAX_CHARISMA as u16);
    let mut n = 4 + 6; // mov; add; cmp; jbe; mov [29h]; and; sub; shr; shr; jne
    if new > MAX_CHARISMA {
        new = MAX_CHARISMA;
        n += 1;
    }
    c.set_byte(g::CHARISMA, new);
    let ax = c.and(W, (old as u16) << 8 | new as u16, 0xfcfc);
    let steps = c.sub(B, ax & 0xff, ax >> 8);
    let steps = c.shr1(B, steps, 2) as u8;
    c.set_reg(AX, ax & 0xff00 | steps as u16);
    if steps == 0 {
        c.count(n + 1);
        return c.ret();
    }
    c.count(n);

    // 6F56: every troop.
    c.run(0x6f56, 2)?;
    let si = c.reg(SI);
    c.push(si);
    let mut t = g::TROOPS;
    c.set_reg(SI, t);
    loop {
        let flags = c.byte(t + troop::FLAGS);
        let free = flags & 0xa0 == 0;
        let motivation = c.byte(t + troop::MOTIVATION);
        let raised = motivation.wrapping_add(steps);
        let capped = free && raised > troop::MAX_MOTIVATION;
        c.run(0x6f5a, 2 + if free { 3 + capped as u32 } else { 0 } + 3)?;
        c.test(B, flags as u16, 0xa0);
        if free {
            let v = c.add(B, motivation as u16, steps as u16) as u8;
            c.set_byte(t + troop::MOTIVATION, v);
            c.cmp(B, v as u16, troop::MAX_MOTIVATION as u16);
            if capped {
                c.set_byte(t + troop::MOTIVATION, troop::MAX_MOTIVATION);
            }
        }
        t = c.add(W, t, troop::SIZE);
        c.set_reg(SI, t);
        c.cmp(W, t, g::TROOPS_END);
        if t >= g::TROOPS_END {
            break;
        }
    }
    c.run(0x6f76, 2)?;
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}
