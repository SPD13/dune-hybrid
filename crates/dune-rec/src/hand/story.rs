//! Story progress, the intro script, and one-line flag setters.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CX, DI, DX, SI, SP, SS, Width},
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
    pub const LOCATION_COORDS: u16 = 0x40ae;
    /// BP's routine for the record at DI.
    pub const FOR_RECORD: u16 = 0x661d;
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
    pub const SET_GAME_PHASE: u16 = 0x121f;
    pub const REMOVE_FRAME_TASK: u16 = 0xda5f;
    pub const GET_MAP_POSITION: u16 = 0x407e;
    pub const DIALOGUE_STEP_AB84: u16 = 0x96b5;
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

/// A counter the script loop looks at (and clears).
const COUNTER_47A8: u16 = 0x47a8;

set_flag!(
    /// `loc_0a1c4`: [47A5h] = FFh.
    set_flag_47a5, 0xa1c4, g::FLAG_47A5, 0xff
);

/// `loc_0a1e2`: ZF set when [`g::FLAG_47A5`] is FFh (`cmp` flags).
pub fn is_mode_47a5(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa1e2)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let v = c.byte(g::FLAG_47A5);
    c.cmp(B, v as u16, 0xff);
    c.ret()
}

/// `loc_0a1e8`: one more at [`COUNTER_47A8`].
pub fn count_47a8(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa1e8)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let v = c.byte(COUNTER_47A8);
    let v = c.inc(B, v as u16);
    c.set_byte(COUNTER_47A8, v as u8);
    c.ret()
}

/// Generates a port for `mov word [at], value; ret`.
macro_rules! set_word {
    ($(#[$doc:meta])* $name:ident, $entry:expr, $at:expr, $value:expr) => {
        $(#[$doc])*
        pub fn $name(m: &mut Machine, start: u16) -> Result<(), Bail> {
            entered(start, $entry)?;
            let mut c = Ctx::new(m);
            c.run(start, 2)?;
            c.set_word($at, $value);
            c.ret()
        }
    };
}

set_flag!(
    /// `loc_0100b`: [100Bh] = 1.
    set_flag_100b, 0x100b, 0x100b, 1
);
set_flag!(
    /// `loc_0101b`: [0FE8h] = 9.
    set_0fe8_to_9, 0x101b, g::BYTE_0FE8, 9
);
set_flag!(
    /// `loc_01bb2`: [473Bh] = 0.
    clear_flag_473b, 0x1bb2, 0x473b, 0
);
set_flag!(
    /// `loc_0215f`: [46D9h] = 7.
    set_46d9_to_7, 0x215f, 0x46d9, 7
);
set_flag!(
    /// `loc_04ab8`: [4727h] = FFh.
    set_flag_4727, 0x4ab8, 0x4727, 0xff
);
set_flag!(
    /// `loc_04ac4`: [11CAh] = 0.
    clear_flag_11ca, 0x4ac4, 0x11ca, 0
);
set_flag!(
    /// `loc_04aca`: [11CAh] = 1.
    set_flag_11ca, 0x4aca, 0x11ca, 1
);
set_flag!(
    /// `loc_050be`: [11CBh] = 0.
    clear_flag_11cb, 0x50be, 0x11cb, 0
);
set_word!(
    /// `loc_01392`: the handler at [2220h] = 1FBAh.
    set_2220_to_1fba, 0x1392, 0x2220, 0x1fba
);
set_word!(
    /// `loc_01399`: the handler at [2220h] = 1FAEh.
    set_2220_to_1fae, 0x1399, 0x2220, 0x1fae
);
set_word!(
    /// `loc_09901`: [479Eh] = 0.
    clear_479e, 0x9901, 0x479e, 0
);

/// `loc_011cb`: the phase to 60h (unchanged-flag cleared), and the
/// position of location 11Ch (`loc_040ae`) noted at 1048h with DL = 2.
pub fn go_to_phase_60(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x11cb)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_byte(g::PHASE_CHANGED_FLAG, 0);
    c.set_byte(g::GAME_PHASE, 0x60);
    c.set_reg(DI, 0x011c);
    c.call(code::LOCATION_COORDS, 0x11db)?;
    c.run(0x11db, 4)?;
    c.set_reg8(r8::DL, 2);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.set_word(0x1048, dx);
    c.set_word(0x104a, bx);
    c.ret()
}

/// `loc_01243`: the sum (BX, from 0) over the three records from 138h
/// (`loc_01258`, with BP = 1269h); 1000 or more counts one at [0C2h].
pub fn sum_three_records(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1243)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_reg(BP, 0x1269);
    let dx = c.reg(DX);
    let z = c.xor(W, dx, dx);
    c.set_reg(DX, z);
    let bx = c.reg(BX);
    let z = c.xor(W, bx, bx);
    c.set_reg(BX, z);
    c.call(0x1258, 0x124d)?;
    let bx = c.reg(BX);
    let big = bx >= 0x3e8;
    c.run(0x124d, if big { 4 } else { 3 })?;
    c.cmp(W, bx, 0x3e8);
    if big {
        let v = c.byte(0xc2);
        let v = c.inc(B, v as u16);
        c.set_byte(0xc2, v as u8);
    }
    c.ret()
}

/// `loc_01258`: `loc_0661d` (BP's routine) for the three 1Ch-byte records
/// from 138h (DI each).
pub fn for_three_records(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1258)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_reg(DI, 0x138);
    c.set_reg(CX, 3);
    c.push(3);
    c.call(code::FOR_RECORD, 0x1262)?;
    loop {
        let sp = c.reg(SP);
        let cx = c.word_in(SS, sp).wrapping_sub(1);
        c.run(0x1262, 3 + if cx == 0 { 1 } else { 2 })?;
        c.pop();
        let di = c.reg(DI);
        let di = c.add(W, di, 0x1c);
        c.set_reg(DI, di);
        c.set_reg(CX, cx);
        if cx == 0 {
            return c.ret();
        }
        c.push(cx);
        c.call(code::FOR_RECORD, 0x1262)?;
    }
}

/// `loc_0127c`: CF set when AL is 4 and the phase is 15h..1Fh.
pub fn is_4_in_phases_15_to_1f(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x127c)?;
    let mut c = Ctx::new(m);
    let al = c.reg8(r8::AL);
    let phase = c.byte(g::GAME_PHASE);
    c.run(start, if al != 4 { 4 } else { 6 })?;
    c.cmp(B, al as u16, 4);
    if al == 4 {
        c.cmp(B, phase as u16, 0x15);
        if phase >= 0x15 {
            c.cmp(B, phase as u16, 0x20);
            return c.ret();
        }
    }
    c.set_flag(pc::cpu::CF, false);
    c.ret()
}

/// `loc_01d9f`: unless bit 80h of [12h], `loc_01e01` on the record at
/// 1048h, then `loc_01eda` when it sets CF.
pub fn check_record_1048(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1d9f)?;
    let mut c = Ctx::new(m);
    let w = c.word(g::INTRO_0012);
    if w & 0x80 != 0 {
        c.run(start, 3)?;
        c.test(W, w, 0x80);
        return c.ret();
    }
    c.run(start, 4)?;
    c.test(W, w, 0x80);
    c.set_reg(SI, 0x1048);
    c.call(0x1e01, 0x1dad)?;
    let cf = c.flag(pc::cpu::CF);
    c.run(0x1dad, 2)?;
    if cf {
        c.call(0x1eda, 0x1db2)?;
        c.run(0x1db2, 1)?;
    }
    c.ret()
}

/// `loc_01ebe`: with bit 800h of the record at SI (+12h) and the phase
/// 60h..63h, the phase set to 64h (registers kept).
pub fn phase_64_for_record(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1ebe)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let w = c.word(si.wrapping_add(0x12));
    if w & 0x800 == 0 {
        c.run(start, 3)?;
        c.test(W, w, 0x800);
        return c.ret();
    }
    let phase = c.byte(g::GAME_PHASE);
    c.run(start, 7)?;
    c.test(W, w, 0x800);
    let d = c.sub(B, phase as u16, 0x60);
    c.set_reg8(r8::AL, d as u8);
    c.cmp(B, d, 4);
    if d >= 4 {
        return c.ret();
    }
    c.call(code::SAVE_REGS, 0x1ed1)?;
    c.run(0x1ed1, 2)?;
    c.set_reg8(r8::AL, 0x64);
    c.call(code::SET_GAME_PHASE, 0x1ed6)?;
    c.run(0x1ed6, 1)?;
    c.call(code::RESTORE_REGS, 0x1ed9)?;
    c.run(0x1ed9, 1)?;
    c.ret()
}

/// `loc_023d5`: [9Dh] down by an eighth of it.
pub fn decay_9d(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x23d5)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    let v = c.byte(0x9d) as u16;
    let e = c.shr1(B, v, 3);
    let v = c.sub(B, v, e);
    c.set_reg(AX, e << 8 | v);
    c.set_byte(0x9d, v as u8);
    c.ret()
}

/// `loc_02524`: AX moved from [0A0h] to [1172h].
pub fn move_a0_to_1172(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2524)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let ax = c.reg(AX);
    let v = c.word(0xa0);
    let v = c.sub(W, v, ax);
    c.set_word(0xa0, v);
    let w = c.word(0x1172);
    let w = c.add(W, w, ax);
    c.set_word(0x1172, w);
    c.ret()
}

/// `loc_03310`: AX = D1h + AL / 16.
pub fn al_sixteenth_plus_d1(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3310)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let ax = c.reg(AX);
    c.xor(B, ax >> 8, ax >> 8);
    let v = c.shr1(W, ax & 0xff, 4);
    let v = c.add(W, v, 0xd1);
    c.set_reg(AX, v);
    c.ret()
}

/// `loc_033be`: [94h], [96h], [5Ch], [5Eh] cleared; `loc_06603` with
/// BP = 3406h, then [9Ch] = `loc_033d9`'s AL.
pub fn reset_and_measure_9c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x33be)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let ax = c.reg(AX);
    let z = c.xor(W, ax, ax);
    c.set_reg(AX, z);
    for at in [0x94, 0x96, 0x5c, 0x5e] {
        c.set_word(at, 0);
    }
    c.set_reg(BP, 0x3406);
    c.call(0x6603, 0x33d2)?;
    c.run(0x33d2, 1)?;
    c.call(0x33d9, 0x33d5)?;
    c.run(0x33d5, 2)?;
    let al = c.reg8(r8::AL);
    c.set_byte(0x9c, al);
    c.ret()
}

/// `loc_0366f`: AX = a byte of [1152h] (the low one with bit 80h of [0],
/// else the high one; the low one when the high is FFh; FFFFh as is).
/// Flags from `or ax, ax`.
pub fn pick_1152(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x366f)?;
    let mut c = Ctx::new(m);
    let ax = c.word(0x1152);
    let w0 = c.word(0);
    let n = if ax == 0xffff {
        5
    } else if ax >> 8 == 0xff {
        8
    } else if w0 & 0x80 != 0 {
        10
    } else {
        11
    };
    c.run(start, n)?;
    c.cmp(W, ax, 0xffff);
    let mut v = ax;
    if ax != 0xffff {
        c.cmp(B, ax >> 8, 0xff);
        if ax >> 8 != 0xff {
            c.test(W, w0, 0x80);
            if w0 & 0x80 == 0 {
                v = v.rotate_left(8);
            }
        }
        c.xor(B, v >> 8, v >> 8);
        v &= 0xff;
    }
    c.or(W, v, v);
    c.set_reg(AX, v);
    c.ret()
}

/// `loc_036d3`: when [23h] is set: `loc_098b2`, [47A7h] = 0, `loc_036ee`
/// with BP = 3520h, and [23h] cleared.
pub fn handle_flag_23(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x36d3)?;
    let mut c = Ctx::new(m);
    let v = c.byte(0x23);
    if v == 0 {
        c.run(start, 3)?;
        c.cmp(B, 0, 0);
        return c.ret();
    }
    c.run(start, 3)?;
    c.cmp(B, v as u16, 0);
    c.call(0x98b2, 0x36dd)?;
    c.run(0x36dd, 3)?;
    c.set_byte(0x47a7, 0);
    c.set_reg(BP, 0x3520);
    c.call(0x36ee, 0x36e8)?;
    c.run(0x36e8, 2)?;
    c.set_byte(0x23, 0);
    c.ret()
}

/// `loc_0390a`: `loc_03916` while [46D7h] is set.
pub fn while_46d7(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x390a)?;
    let mut c = Ctx::new(m);
    let (mut at, mut jmp) = (start, 0);
    loop {
        let v = c.byte(0x46d7);
        c.run(at, jmp + 3)?;
        c.cmp(B, v as u16, 0);
        if v == 0 {
            return c.ret();
        }
        c.call(0x3916, 0x3914)?;
        (at, jmp) = (0x3914, 1);
    }
}

/// `loc_0395f`: BL = the table at 2280h (by AL's low 4 bits) plus AL's
/// bits 2..4 (in AH as 0..1Ch).
pub fn table_2280(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x395f)?;
    let mut c = Ctx::new(m);
    c.run(start, 9)?;
    let al = c.reg8(r8::AL) as u16;
    let ah = c.shr1(B, al, 2);
    let ax = c.and(W, ah << 8 | al, 0x1c0f);
    let v = c.byte(0x2280u16.wrapping_add(ax & 0xff));
    let s = c.add(B, v as u16, ax >> 8);
    c.set_reg(AX, ax & 0xff00 | s);
    c.set_reg(BX, 0x2200 | s);
    c.ret()
}

/// `loc_03ae9`: the 2Eh words at 47F8h to FFFFh. AX kept.
pub fn clear_47f8(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3ae9)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let ax = c.reg(AX);
    c.push(ax);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.set_reg(CX, 0x2e);
    c.set_reg(AX, 0xffff);
    c.set_reg(DI, 0x47f8);
    c.rep(0xab);
    c.run(0x3af7, 2)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// From 41CC (the stretch from `at`, `prefix` instructions before it done
/// by `ops`): [21FDh] = AL, and [1F11h] too when [1F12h] is 4FFBh.
fn note_21fd(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    let w = c.word(0x1f12);
    c.run(at, prefix + if w == 0x4ffb { 5 } else { 4 })?;
    ops(c);
    let al = c.reg8(r8::AL);
    c.set_byte(0x21fd, al);
    c.cmp(W, w, 0x4ffb);
    if w == 0x4ffb {
        c.set_byte(0x1f11, al);
    }
    c.ret()
}

/// `loc_041cc`: [21FDh] = AL (see [`note_21fd`]).
pub fn note_21fd_al(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x41cc)?;
    note_21fd(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_041c5`: [4726h] and [21FDh] cleared (see [`note_21fd`]).
pub fn clear_21fd(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x41c5)?;
    note_21fd(&mut Ctx::new(m), start, 2, |c| {
        c.set_byte(0x4726, 0);
        let al = c.reg8(r8::AL) as u16;
        let z = c.xor(B, al, al);
        c.set_reg8(r8::AL, z as u8);
    })
}

/// `loc_0469b`: when [473Fh] is set, cleared and the frame task at 46B5h
/// removed (registers kept).
pub fn remove_task_46b5(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x469b)?;
    let mut c = Ctx::new(m);
    let v = c.word(0x473f);
    if v == 0 {
        c.run(start, 3)?;
        c.cmp(W, 0, 0);
        return c.ret();
    }
    c.run(start, 4)?;
    c.cmp(W, v, 0);
    c.set_word(0x473f, 0);
    c.call(code::SAVE_REGS, 0x46ab)?;
    c.run(0x46ab, 2)?;
    c.set_reg(SI, 0x46b5);
    c.call(code::REMOVE_FRAME_TASK, 0x46b1)?;
    c.run(0x46b1, 1)?;
    c.call(code::RESTORE_REGS, 0x46b4)?;
    c.run(0x46b4, 1)?;
    c.ret()
}

/// From 5119 (`at`, `prefix` instructions before, done by `ops`): AL more
/// at [11C7h], [11CCh] = 80h.
fn add_11c7(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 3)?;
    ops(c);
    let (v, al) = (c.byte(0x11c7), c.reg8(r8::AL));
    let v = c.add(B, v as u16, al as u16);
    c.set_byte(0x11c7, v as u8);
    c.set_word(0x11cc, 0x80);
    c.ret()
}

/// `loc_05119`: see [`add_11c7`].
pub fn add_al_to_11c7(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5119)?;
    add_11c7(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_04965`: `loc_05124`, then [11C5h] = DI, [11C8h], [11C7h] = 0 and
/// [`add_11c7`].
pub fn start_11c5(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4965)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x5124, 0x4968)?;
    add_11c7(&mut c, 0x4968, 5, |c| {
        let cx = c.reg(CX);
        let z = c.xor(W, cx, cx);
        c.set_reg(CX, z);
        let di = c.reg(DI);
        c.set_word(0x11c5, di);
        c.set_byte(0x11c8, 0);
        c.set_byte(0x11c7, 0);
    })
}

/// `loc_04b3b`: one more tick at [472Bh] (every 16th `loc_00fd9` with
/// CX = 1); the map position (`get_map_position`, `loc_05206`,
/// `loc_040c3`) to [4], [6].
pub fn tick_472b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4b3b)?;
    let mut c = Ctx::new(m);
    let n = c.word(0x472b).wrapping_add(1);
    let tick = n & 0x0f == 0;
    c.run(start, if tick { 5 } else { 4 })?;
    let v = c.word(0x472b);
    let v = c.inc(W, v);
    c.set_word(0x472b, v);
    c.test(W, v, 0x0f);
    if tick {
        c.set_reg(CX, 1);
        c.call(0x0fd9, 0x4b4d)?;
        c.run(0x4b4d, 1)?;
    }
    c.call(code::GET_MAP_POSITION, 0x4b50)?;
    c.run(0x4b50, 1)?;
    c.call(0x5206, 0x4b53)?;
    c.run(0x4b53, 1)?;
    c.call(0x40c3, 0x4b56)?;
    c.run(0x4b56, 3)?;
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.set_word(4, dx);
    c.set_word(6, bx);
    c.ret()
}

/// `loc_04ded`: `loc_0661d` with BP = 4E04h, CX = 0 (SI kept); then
/// [4733h] = CX with CH = the record's byte 14h, and `loc_07f27` (ES
/// kept).
pub fn measure_4733(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4ded)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let cx = c.reg(CX);
    let z = c.xor(W, cx, cx);
    c.set_reg(CX, z);
    c.set_reg(BP, 0x4e04);
    let si = c.reg(SI);
    c.push(si);
    c.call(code::FOR_RECORD, 0x4df6)?;
    c.run(0x4df6, 4)?;
    let di = c.reg(DI);
    let ch = c.byte(di.wrapping_add(0x14));
    let cx = (ch as u16) << 8 | c.reg(CX) & 0xff;
    c.set_reg(CX, cx);
    c.set_word(0x4733, cx);
    let es = c.sreg(pc::cpu::ES);
    c.push(es);
    c.call(0x7f27, 0x4e01)?;
    c.run(0x4e01, 3)?;
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_0b17a` (called on each phase change): [`code::DIALOGUE_STEP_AB84`]
/// with bit 80h of [0C6h] set for the while.
pub fn on_phase_change(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb17a)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let v = c.byte(0xc6);
    c.set_reg8(r8::AL, v);
    let ax = c.reg(AX);
    c.push(ax);
    let v2 = c.or(B, v as u16, 0x80) as u8;
    c.set_reg8(r8::AL, v2);
    c.set_byte(0xc6, v2);
    c.call(code::DIALOGUE_STEP_AB84, 0xb186)?;
    c.run(0xb186, 3)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.set_byte(0xc6, ax as u8);
    c.ret()
}

/// `loc_0b254`: [243Eh] = 24h less what is left after AX in the 0Ch words
/// at 2426h (so 25h - 0Ch + its index), or 0 when it is not there. AX, SI
/// kept.
pub fn find_in_2426(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb254)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let (ax, si) = (c.reg(AX), c.reg(SI));
    c.push(ax);
    c.push(si);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.set_reg(DI, 0x2426);
    c.set_reg(CX, 0x0c);
    c.m.cpu.string_op(&mut c.m.hw, 0xaf, None, 0xf2); // repne scasw
    let found = c.flag(pc::cpu::ZF);
    c.run(0xb260, if found { 4 } else { 2 } + 4)?;
    let mut v = 0;
    if found {
        let cx = c.reg(CX);
        v = c.sub(W, 0x24, cx);
    }
    c.set_reg(AX, v);
    c.set_word(0x243e, v);
    let si = c.pop();
    c.set_reg(SI, si);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}
