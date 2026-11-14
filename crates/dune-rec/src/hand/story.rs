//! Story progress, the intro script, and one-line flag setters.

use super::pure;
use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DS, DX, SI, SP, SS, Width},
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
    pub const FOR_EACH_TROOP_3127: u16 = 0x6603;
    pub const ADD_FRAME_TASK: u16 = 0xda25;
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
    let bits = pure::or(B, bits as u16, 2) as u8;
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
    let v = pure::add(B, v as u16, 0x0c) as u8;
    c.set_byte(g::BYTE_11D0, v);
    c.call(code::GET_INGAME_DAY_Q, 0x11c0)?;
    c.run(0x11c0, 4)?;
    let day = c.reg(AX);
    let day = pure::add(W, day, 3);
    c.set_reg(AX, day);
    c.set_word(g::WORD_1156, day);
    let v = c.byte(g::BYTE_1141);
    let v = pure::inc(B, v as u16) as u8;
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
    let mut new = pure::add(B, gain as u16, old as u16) as u8;
    let mut n = 4 + 6; // mov; add; cmp; jbe; mov [29h]; and; sub; shr; shr; jne
    if new > MAX_CHARISMA {
        new = MAX_CHARISMA;
        n += 1;
    }
    c.set_byte(g::CHARISMA, new);
    let ax = pure::and(W, (old as u16) << 8 | new as u16, 0xfcfc);
    let steps = pure::sub(B, ax & 0xff, ax >> 8);
    let steps = pure::shr1(B, steps, 2) as u8;
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
        if free {
            let v = pure::add(B, motivation as u16, steps as u16) as u8;
            c.set_byte(t + troop::MOTIVATION, v);
            if capped {
                c.set_byte(t + troop::MOTIVATION, troop::MAX_MOTIVATION);
            }
        }
        t = pure::add(W, t, troop::SIZE);
        c.set_reg(SI, t);
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
    let v = pure::inc(B, v as u16);
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
    let z = pure::xor(W, dx, dx);
    c.set_reg(DX, z);
    let bx = c.reg(BX);
    let z = pure::xor(W, bx, bx);
    c.set_reg(BX, z);
    c.call(0x1258, 0x124d)?;
    let bx = c.reg(BX);
    let big = bx >= 0x3e8;
    c.run(0x124d, if big { 4 } else { 3 })?;
    if big {
        let v = c.byte(0xc2);
        let v = pure::inc(B, v as u16);
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
        let di = pure::add(W, di, 0x1c);
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
        return c.ret();
    }
    let phase = c.byte(g::GAME_PHASE);
    c.run(start, 7)?;
    let d = pure::sub(B, phase as u16, 0x60);
    c.set_reg8(r8::AL, d as u8);
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
    let e = pure::shr1(B, v, 3);
    let v = pure::sub(B, v, e);
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
    let v = pure::sub(W, v, ax);
    c.set_word(0xa0, v);
    let w = c.word(0x1172);
    let w = pure::add(W, w, ax);
    c.set_word(0x1172, w);
    c.ret()
}

/// `loc_03310`: AX = D1h + AL / 16.
pub fn al_sixteenth_plus_d1(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3310)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let ax = c.reg(AX);
    let v = pure::shr1(W, ax & 0xff, 4);
    let v = pure::add(W, v, 0xd1);
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
    let z = pure::xor(W, ax, ax);
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
        return c.ret();
    }
    c.run(start, 3)?;
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
        let z = pure::xor(B, al, al);
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
        return c.ret();
    }
    c.run(start, 4)?;
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
    let v = pure::inc(W, v);
    c.set_word(0x472b, v);
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
    let z = pure::xor(W, cx, cx);
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
    let v2 = pure::or(B, v as u16, 0x80) as u8;
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

/// `loc_01bec`: in a place ([2Bh]): `loc_0503c` on location [114Eh];
/// [46D9h] to 6 when set; and when the place is left by it, `loc_00b21`
/// and bit 0 of [473Bh].
pub fn place_1bec(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1bec)?;
    let mut c = Ctx::new(m);
    let v = c.byte(0x2b);
    if v == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    c.run(start, 4)?;
    let di = c.word(0x114e);
    c.set_reg(DI, di);
    c.call(0x503c, 0x1bfa)?;
    let f = c.byte(0x46d9);
    let still = c.byte(0x2b);
    c.run(0x1bfa, 2 + (f != 0) as u32 + 2 + if still != 0 { 1 } else { 2 })?;
    if f != 0 {
        c.set_byte(0x46d9, 6);
    }
    if still != 0 {
        return c.ret();
    }
    let di = c.reg(DI);
    c.push(di);
    c.call(0x0b21, 0x1c11)?;
    c.run(0x1c11, 3)?;
    let di = c.pop();
    c.set_reg(DI, di);
    let b = c.byte(0x473b);
    let b = pure::or(B, b as u16, 1);
    c.set_byte(0x473b, b as u8);
    c.ret()
}

/// `loc_01c18`: by [46EBh]: negative, the map's resource opened and the
/// marks of [46FAh] (`loc_078e9`) and [46F8h] (`loc_0600e`) redrawn; 0
/// with [0FBh] negative, the stats screen (`loc_0bdbb`).
pub fn redraw_1c18(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1c18)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x46eb);
    if k & 0x80 == 0 {
        let fb = c.byte(0xfb);
        let n = if k != 0 { 4 } else if fb & 0x80 == 0 { 6 } else { 6 };
        c.run(start, n)?;
        if k != 0 {
            return c.ret();
        }
        if fb & 0x80 == 0 {
            return c.ret();
        }
        c.call(0xbdbb, 0x1c45)?;
        c.run(0x1c45, 1)?;
        return c.ret();
    }
    c.run(start, 3)?;
    c.call(0xc13b, 0x1c22)?;
    let si = c.word(0x46fa);
    c.run(0x1c22, 3 + (si != 0) as u32)?;
    c.set_reg(SI, si);
    if si != 0 {
        c.call(0x78e9, 0x1c2d)?;
    }
    let di = c.word(0x46f8);
    c.run(0x1c2d, if di != 0 { 4 } else { 4 })?;
    c.set_reg(DI, di);
    if di == 0 {
        return c.ret();
    }
    c.call(0x600e, 0x1c38)?;
    c.run(0x1c38, 1)?;
    c.ret()
}

/// `loc_0235f`: the 11h-byte record at SI made current (at [10B4h]): its
/// bytes 2, 1 to [1Ch], [1Dh], word 0Eh to [20h], and [1Fh] = the days
/// since its byte 10h (when word 0Eh is set; else 0).
pub fn select_record_235f(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x235f)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let w = c.word(si.wrapping_add(0x0e));
    c.run(start, if w == 0 { 8 } else { 9 })?;
    c.set_word(0x10b4, si);
    let b2 = c.byte(si.wrapping_add(2));
    c.set_reg8(r8::AL, b2);
    c.set_byte(0x1c, b2);
    c.set_reg(AX, w);
    c.set_word(0x20, w);
    c.set_byte(0x1f, 0);
    if w != 0 {
        c.call(code::GET_INGAME_DAY_Q, 0x237b)?;
        c.run(0x237b, 2)?;
        let al = c.reg8(r8::AL);
        let si = c.reg(SI);
        let b = c.byte(si.wrapping_add(0x10));
        let d = pure::sub(B, al as u16, b as u16) as u8;
        c.set_reg8(r8::AL, d);
        c.set_byte(0x1f, d);
    }
    c.run(0x2381, 3)?;
    let si = c.reg(SI);
    let b1 = c.byte(si.wrapping_add(1));
    c.set_reg8(r8::AL, b1);
    c.set_byte(0x1d, b1);
    c.ret()
}

/// `loc_02318`: the record from 10D8h (11h bytes each) whose byte 0 is
/// the byte at DI made current (`loc_0235f`); [1Eh] = the days since its
/// byte 3 (1 the first time, marking bit 8 of byte 2); [9Dh], [9Fh]
/// cleared; [11F1h] = 0E8h + the low 3 bits of [0] modulo [1141h].
pub fn select_record_2318(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2318)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let key = c.byte(di);
    let mut si = 0x10c7u16;
    let mut k = 0;
    loop {
        si = si.wrapping_add(0x11);
        k += 1;
        if c.byte(si) == key {
            break;
        }
    }
    c.run(start, 2 + 3 * k + 1)?;
    c.set_reg8(r8::AL, key);
    let mut s = 0x10c7u16;
    loop {
        s = pure::add(W, s, 0x11);
        let v = c.byte(s);
        if v == key {
            break;
        }
    }
    c.set_reg(SI, s);
    c.call(0x235f, 0x2327)?;
    c.run(0x2327, 1)?;
    c.call(code::GET_INGAME_DAY_Q, 0x232a)?;
    let si = c.reg(SI);
    let f = c.byte(si.wrapping_add(2));
    let first = f & 8 == 0;
    let m0 = c.word(0) & 7;
    let modulus = c.byte(0x1141) as u16;
    if modulus == 0 {
        c.m.cpu.ip = 0x232a;
        return Err(Bail);
    }
    let mut v = m0;
    let mut rounds = 0;
    while v >= modulus {
        v -= modulus;
        rounds += 1;
    }
    c.run(0x232a, 3 + if first { 2 } else { 0 } + 4 + 4 * rounds + 2 + 4)?;
    let al = c.reg8(r8::AL);
    let b3 = c.byte(si.wrapping_add(3));
    let mut d = pure::sub(B, al as u16, b3 as u16) as u8;
    if first {
        d = 1;
        let f = pure::or(B, f as u16, 8);
        c.set_byte(si.wrapping_add(2), f as u8);
    }
    c.set_reg8(r8::AL, d);
    c.set_byte(0x1e, d);
    c.set_byte(0x9d, 0);
    let w = c.word(0);
    let mut ax = pure::and(W, w, 7);
    loop {
        if ax & 0xff < modulus {
            break;
        }
        let a = pure::sub(B, ax & 0xff, modulus);
        ax = ax & 0xff00 | a;
    }
    let ax = pure::add(W, ax, 0xe8);
    c.set_reg(AX, ax);
    c.set_word(0x11f1, ax);
    c.set_byte(0x9f, 0);
    c.ret()
}

/// `loc_023e6`: the record at DI loses bits 60h of byte 2; [9Dh] (taken,
/// cleared) added to [20h] and its word 0Eh (one more at [22h] when that
/// was 0); its byte 10h = the day; and the stock moves: the record's byte
/// 4 + i one down, the location [114Eh]'s byte 14h + i one up (i =
/// [11F1h] - 0E8h).
pub fn take_record_23e6(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x23e6)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let gain = c.byte(0x9d) as u16;
    let w = c.word(di.wrapping_add(0x0e)).wrapping_add(gain);
    let new = w == gain;
    c.run(start, 7 + new as u32 + 1)?;
    let f = c.byte(di.wrapping_add(2));
    let f = pure::and(B, f as u16, 0x9f);
    c.set_byte(di.wrapping_add(2), f as u8);
    c.set_byte(0x9d, 0);
    c.set_reg(AX, gain);
    let v = c.word(0x20);
    let v = pure::add(W, v, gain);
    c.set_word(0x20, v);
    let v = c.word(di.wrapping_add(0x0e));
    let v = pure::add(W, v, gain);
    c.set_word(di.wrapping_add(0x0e), v);
    if new {
        let b = c.byte(0x22);
        let b = pure::inc(B, b as u16);
        c.set_byte(0x22, b as u8);
    }
    c.call(code::GET_INGAME_DAY_Q, 0x2403)?;
    c.run(0x2403, 8)?;
    let di = c.reg(DI);
    let al = c.reg8(r8::AL);
    c.set_byte(di.wrapping_add(0x10), al);
    let i = c.word(0x11f1);
    let i = pure::sub(W, i, 0xe8);
    c.set_reg(AX, i);
    c.set_reg(BX, i);
    let at = i.wrapping_add(di).wrapping_add(4);
    let b = c.byte(at);
    let b = pure::dec(B, b as u16);
    c.set_byte(at, b as u8);
    let si = c.word(0x114e);
    c.set_reg(SI, si);
    let at = i.wrapping_add(si).wrapping_add(0x14);
    let b = c.byte(at);
    let b = pure::inc(B, b as u16);
    c.set_byte(at, b as u8);
    c.ret()
}

/// `loc_024d2`: AH = how many of 1, 40h, 80h, 90h, FFh are above [0BEh]
/// (AL = it, CL = 0).
pub fn grade_be(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x24d2)?;
    let mut c = Ctx::new(m);
    c.run(start, 14)?;
    let al = c.byte(0xbe);
    c.set_reg8(r8::AL, al);
    c.xor(B, 0, 0);
    c.set_reg8(r8::CL, 0);
    c.xor(B, 0, 0);
    let mut ah = 0u16;
    for t in [1u16, 0x40, 0x80, 0x90, 0xff] {
        c.cmp(B, al as u16, t);
        ah = c.adc(B, ah, 0);
    }
    c.set_reg8(r8::AH, ah as u8);
    c.ret()
}

/// `loc_0272f`: the first word of the list at 1179h dropped (the 9 after it
/// moved down, a 0 after), when [0C8h] counts any.
pub fn shift_list_1179(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x272f)?;
    let mut c = Ctx::new(m);
    let n = c.byte(0xc8);
    if n == 0 {
        c.run(start, 4)?;
        c.set_reg(CX, 0);
        return c.ret();
    }
    c.run(start, 11)?;
    c.set_reg(CX, n as u16);
    c.set_reg(SI, 0x1179);
    let v = pure::dec(B, n as u16);
    c.set_byte(0xc8, v as u8);
    c.set_reg(DI, 0x1179);
    let si = pure::add(W, 0x1179, 2);
    c.set_reg(SI, si);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.set_reg(CX, 9);
    c.rep(0xa5);
    c.run(0x274a, 3)?;
    c.set_reg(AX, 0);
    c.stos(W);
    c.ret()
}

/// `loc_02a34`: the countdown at 1190h one down (at 0, [118Fh] cleared);
/// then its list (from 1191h, 4-byte entries) shifted down one, zeros
/// after.
pub fn shift_list_1191(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2a34)?;
    let mut c = Ctx::new(m);
    let v = c.byte(0x1190).wrapping_sub(1);
    c.run(start, 3 + (v == 0) as u32 + 7)?;
    c.set_reg(SI, 0x1190);
    let b = c.byte(0x1190);
    let v = pure::dec(B, b as u16);
    c.set_byte(0x1190, v as u8);
    if v == 0 {
        c.set_byte(0x118f, 0);
    }
    let si = pure::inc(W, 0x1190);
    c.set_reg(DI, si);
    let si = pure::add(W, si, 4);
    c.set_reg(SI, si);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.set_reg(CX, 0x12);
    c.rep(0xa5);
    c.run(0x2a4c, 4)?;
    c.set_reg(AX, 0);
    c.stos(W);
    c.stos(W);
    c.ret()
}

/// `loc_028e1`: at hour 0Ch [46D9h] = 7; when [47A9h] is set: the screen
/// shown, transition 8 (BP = 2DD3h), the flag cleared and the glow faded
/// out (`comm_fade_out_glow`); [47A6h] = FFh.
pub fn after_call_28e1(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x28e1)?;
    let mut c = Ctx::new(m);
    let h = c.byte(0x24);
    c.run(start, if h == 0x0c { 3 } else { 2 })?;
    c.cmp(B, h as u16, 0x0c);
    if h == 0x0c {
        c.call(0x215f, 0x28eb)?;
    }
    let f = c.byte(0x47a9);
    c.run(0x28eb, 3)?;
    c.cmp(B, f as u16, 0);
    if f == 0 {
        return c.ret();
    }
    c.call(0xc49a, 0x28f5)?;
    c.run(0x28f5, 4)?;
    c.set_byte(0x47a6, 0xff);
    c.set_reg8(r8::AL, 8);
    c.set_reg(BP, 0x2dd3);
    c.call(0xc108, 0x2902)?;
    c.run(0x2902, 2)?;
    c.set_byte(0x47a9, 0);
    c.call(0x2773, 0x290a)?;
    c.run(0x290a, 1)?;
    c.ret()
}

/// `loc_02b00`: a message: with DI, `loc_0331e` first (AX kept); [0EAh] =
/// AL, the phrase AH said (`loc_096d8`, then `loc_09945`); [0EAh] = FFh,
/// `loc_09ef1` with AL = 1; `loc_0331e` on [11CEh] (as it was). CF set.
pub fn message_2b00(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2b00)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    c.run(start, if di == 0 { 3 } else { 5 })?;
    let w = c.word(0x11ce);
    c.push(w);
    c.or(W, di, di);
    if di != 0 {
        let ax = c.reg(AX);
        c.push(ax);
        c.call(0x331e, 0x2b0c)?;
        c.run(0x2b0c, 1)?;
        let ax = c.pop();
        c.set_reg(AX, ax);
    }
    c.run(0x2b0d, 4)?;
    let ax = c.reg(AX);
    c.set_byte(0xea, ax as u8);
    c.xor(B, 0, 0);
    c.set_reg(AX, ax >> 8);
    c.call(0x96d8, 0x2b17)?;
    c.run(0x2b17, 1)?;
    c.call(0x9945, 0x2b1a)?;
    c.run(0x2b1a, 3)?;
    c.set_byte(0xea, 0xff);
    c.set_reg8(r8::AL, 1);
    c.call(0x9ef1, 0x2b24)?;
    c.run(0x2b24, 2)?;
    let di = c.pop();
    c.set_reg(DI, di);
    c.call(0x331e, 0x2b28)?;
    c.run(0x2b28, 2)?;
    c.set_flag(pc::cpu::CF, true);
    c.ret()
}

/// The list at 1190h: a count byte, then 4-byte entries (a word whose low
/// byte is a kind, 1 for spent, and a word).
const LIST_1190: u16 = 0x1190;

/// `loc_02a7f`: the spent entries (kind 1) dropped from [`LIST_1190`],
/// the count fixed and a zero entry after when it shrank.
pub fn compact_list_1190(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2a7f)?;
    let mut c = Ctx::new(m);
    let n = c.byte(LIST_1190) as u16;
    if n == 0 {
        c.run(start, 7)?;
        c.set_reg(SI, LIST_1190);
        c.set_reg(CX, 0);
        c.set_reg(BP, 0);
        c.lods(B, pc::cpu::DS);
        return c.ret();
    }
    let mut kept = 0u16;
    let mut count = 9;
    for i in 0..n {
        let k = c.byte(LIST_1190 + 1 + 4 * i);
        if k == 1 {
            count += 6;
        } else {
            count += 7;
            kept += 1;
        }
    }
    let shrank = kept as u8 != n as u8;
    count += 5 + if shrank { 4 } else { 1 };
    c.run(start, count)?;
    c.set_reg(SI, LIST_1190);
    c.set_reg(CX, 0);
    c.set_reg(BP, 0);
    c.lods(B, pc::cpu::DS);
    c.set_reg(CX, n);
    let si = c.reg(SI);
    c.set_reg(DI, si);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    let mut bp = 0u16;
    for i in 0..n {
        let w = c.lods(W, pc::cpu::DS);
        if w & 0xff == 1 {
            let si = c.reg(SI);
            let si = pure::add(W, si, 2);
            c.set_reg(SI, si);
        } else {
            c.stos(W);
            c.movs(W);
            bp = pure::inc(W, bp);
            c.set_reg(BP, bp);
        }
        c.set_reg(CX, n - 1 - i);
    }
    c.set_reg(CX, bp);
    let old = c.byte(LIST_1190);
    c.set_byte(LIST_1190, bp as u8);
    c.set_reg(CX, (bp & 0xff) << 8 | old as u16);
    if shrank {
        c.set_reg(AX, 0);
        c.stos(W);
        c.stos(W);
    }
    c.ret()
}

/// `loc_02aaf`: the entry of [`LIST_1190`] for kind AL (for kind 0Fh, the
/// one whose word is DI): CF set, AX, DI = its words. SI kept.
pub fn find_in_list_1190(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2aaf)?;
    let mut c = Ctx::new(m);
    let n = c.byte(LIST_1190) as u16;
    let (al, di) = (c.reg8(r8::AL), c.reg(DI));
    let mut count = 5;
    let mut found = None;
    if n == 0 {
        count += 3;
    } else {
        count += 1;
        for i in 0..n {
            let e = LIST_1190 + 1 + 4 * i;
            let k = c.byte(e.wrapping_add(1));
            count += 2;
            if k == al {
                count += 2;
                if al != 0x0f {
                    found = Some(i);
                    break;
                }
                count += 2;
                if c.word(e.wrapping_add(2)) == di {
                    found = Some(i);
                    break;
                }
            }
            count += 2;
        }
        count += if found.is_some() { 5 } else { 3 };
    }
    c.run(start, count)?;
    let si = c.reg(SI);
    c.push(si);
    c.set_reg(SI, LIST_1190);
    c.xor(W, 0, 0);
    c.set_reg(CX, n);
    if n != 0 {
        let mut e = c.inc(W, LIST_1190);
        for i in 0..n {
            c.set_reg(SI, e);
            let k = c.byte(e.wrapping_add(1));
            c.cmp(B, al as u16, k as u16);
            if k == al {
                c.cmp(B, al as u16, 0x0f);
                if al != 0x0f {
                    break;
                }
                let w = c.word(e.wrapping_add(2));
                c.cmp(W, di, w);
                if w == di {
                    break;
                }
            }
            if found == Some(i) {
                break;
            }
            e = c.add(W, e, 4);
            c.set_reg(SI, e);
            c.set_reg(CX, n - 1 - i);
        }
    }
    if let Some(i) = found {
        let e = LIST_1190 + 1 + 4 * i;
        let (w0, w1) = (c.word(e), c.word(e.wrapping_add(2)));
        c.set_reg(AX, w0);
        c.set_reg(DI, w1);
    }
    let si = c.pop();
    c.set_reg(SI, si);
    c.set_flag(pc::cpu::CF, found.is_some());
    c.ret()
}

/// `loc_03127`: the four records at 109Ah.. reset (word 7F80h), [476Ah],
/// [476Bh] cleared; at a location ([6] low byte 80h): `loc_06603` with
/// BP = 316Eh, and in a palace (kind 21h) the record at 10A8h set to the
/// player's place and made current (`loc_02318`); then `loc_0331e`.
pub fn reset_records_3127(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3127)?;
    let mut c = Ctx::new(m);
    let bx = c.word(6);
    let at = bx & 0xff == 0x80;
    c.run(start, if at { 14 } else { 11 })?;
    c.set_byte(0x476b, 0);
    c.set_byte(0x476a, 0);
    c.set_reg(AX, 0x7f80);
    for a in [0x10ca, 0x10ba, 0x10aa, 0x109a] {
        c.set_word(a, 0x7f80);
    }
    c.set_reg(BX, bx);
    if !at {
        return c.ret();
    }
    let di = c.word(0x114e);
    c.set_reg(DI, di);
    let dx = c.word(4);
    c.set_reg(DX, dx);
    c.set_reg(BP, 0x316e);
    c.call(code::FOR_EACH_TROOP_3127, 0x3157)?;
    let di = c.reg(DI);
    let kind = c.byte(di.wrapping_add(8));
    if kind != 0x21 {
        c.run(0x3157, 3)?;
    } else {
        c.run(0x3157, 6)?;
        let (dx, bx) = (c.reg(DX), c.reg(BX));
        c.set_word(0x10a8, dx);
        c.set_word(0x10aa, bx);
        c.push(di);
        c.call(0x2318, 0x3169)?;
        c.run(0x3169, 2)?;
        let di = c.pop();
        c.set_reg(DI, di);
    }
    c.call(0x331e, 0x316d)?;
    c.run(0x316d, 1)?;
    c.ret()
}

/// From 3093 (the stretch from `at`, `prefix` instructions before, done
/// by `ops`): `loc_03127`; then at the end of the list at 1F0Ch (4-byte
/// entries to a 0), [12h] cleared, `loc_036ee` with BP = 30B9h and 3120h,
/// and a 0 word after (ES = DS).
fn setup_3093(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx) -> Result<(), Bail>) -> Result<(), Bail> {
    c.run(at, prefix + 1)?;
    ops(c)?;
    c.call(0x3127, 0x3096)?;
    let mut di = 0x1f0cu16;
    let mut k = 0;
    loop {
        di = di.wrapping_add(4);
        k += 1;
        if c.word(di) == 0 {
            break;
        }
    }
    c.run(0x3096, 1 + 3 * k + 5)?;
    let mut d = 0x1f0cu16;
    for _ in 0..k {
        d = c.add(W, d, 4);
        let w = c.word(d);
        c.cmp(W, w, 0);
    }
    c.set_reg(DI, d);
    c.set_word(g::INTRO_0012, 0);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.set_reg(BP, 0x30b9);
    c.call(0x36ee, 0x30af)?;
    c.run(0x30af, 2)?;
    c.set_reg(BP, 0x3120);
    c.call(0x36ee, 0x30b5)?;
    c.run(0x30b5, 3)?;
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.stos(W);
    c.ret()
}

/// `loc_03093`: see [`setup_3093`].
pub fn setup_3093_entry(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3093)?;
    setup_3093(&mut Ctx::new(m), start, 0, |_| Ok(()))
}

/// `loc_03090`: `loc_098e6`, then [`setup_3093`].
pub fn setup_3090(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3090)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x98e6, 0x3093)?;
    setup_3093(&mut c, 0x3093, 0, |_| Ok(()))
}

/// `loc_03385`: [0F7h] = the bits (byte 0Eh) of the records 1018h, 1028h,
/// 1048h at the location DI, unless it is [1150h].
pub fn mark_records_at_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3385)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let skip = di == c.word(0x1150);
    if skip {
        c.run(start, 4)?;
        c.set_byte(0xf7, 0);
        c.cmp(W, di, di);
        return c.ret();
    }
    let off = di.wrapping_sub(0x100);
    if off / 0x1c > 0xff {
        c.m.cpu.ip = start;
        return Err(Bail);
    }
    c.run(start, 12)?;
    c.set_byte(0xf7, 0);
    let other = c.word(0x1150);
    c.cmp(W, di, other);
    let off = c.sub(W, di, 0x100);
    c.set_reg(AX, off);
    c.set_reg8(r8::BL, 0x1c);
    c.m.cpu.mul_div(&mut c.m.hw, B, 6, 0x1c); // div bl
    let ax = c.reg(AX);
    let ax = c.inc(W, ax);
    c.set_reg(AX, ax);
    c.set_reg(BX, (ax & 0xff) << 8 | 0x80);
    c.set_reg(SI, 0x1018);
    c.call(0x33ad, 0x33a4)?;
    c.run(0x33a4, 2)?;
    c.set_reg(SI, 0x1028);
    c.call(0x33ad, 0x33aa)?;
    let bx = c.reg(BX);
    let si = 0x1048u16;
    let w = c.word(si.wrapping_add(2));
    c.run(0x33aa, 1 + if w == bx { 7 } else { 3 })?;
    c.set_reg(SI, si);
    c.cmp(W, bx, w);
    if w == bx {
        let cl = c.byte(si.wrapping_add(0x0e));
        c.set_reg8(r8::CL, cl);
        let bit = c.shl(B, 1, cl) as u8;
        c.set_reg8(r8::AL, bit);
        let v = c.byte(0xf7);
        let v = c.or(B, v as u16, bit as u16);
        c.set_byte(0xf7, v as u8);
    }
    c.ret()
}

/// `loc_033d9`: AL = 1F8h x the smaller of [94h], [96h] / the larger, in
/// 1/256ths and at most FCh; negative when [96h] is the smaller.
pub fn ratio_94_96(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x33d9)?;
    let mut c = Ctx::new(m);
    let (a, d) = (c.word(0x96), c.word(0x94));
    let below = a < d;
    let (big, small) = if below { (d, a) } else { (a, d) };
    let mut n = 5 + below as u32 + 2;
    let mut q = None;
    if small == 0 {
        n += 1;
    } else {
        n += 5;
        if big >> 8 >= small {
            n += 1;
        } else {
            n += 4;
            let v = ((big as u32) << 8) / small as u32 >> 1;
            if v >= 0xfc {
                n += 1;
            } else {
                q = Some(v as u16);
            }
        }
    }
    n += 2 + below as u32 + 1;
    c.run(start, n)?;
    c.set_reg(AX, a);
    c.set_reg(DX, d);
    c.cmp(W, a, d);
    let f = c.m.cpu.flags();
    c.push(f);
    c.set_reg(AX, big);
    c.set_reg(DX, small);
    c.set_reg(CX, small);
    if small != 0 {
        c.set_reg(DX, big >> 8);
        c.set_reg(AX, (big & 0xff) << 8);
        c.cmp(W, big >> 8, small);
        if big >> 8 < small {
            c.m.cpu.mul_div(&mut c.m.hw, W, 6, small as u32); // div cx
            let v = c.reg(AX);
            let v = c.shr1(W, v, 1);
            c.set_reg(AX, v);
            c.cmp(W, v, 0xfc);
        }
    }
    let ax = q.unwrap_or(0xfc);
    c.set_reg(AX, ax);
    let f = c.pop();
    c.m.cpu.set_flags(f);
    if below {
        let al = c.neg(B, ax & 0xff);
        c.set_reg(AX, ax & 0xff00 | al);
    }
    c.ret()
}

/// `loc_0348a`: from `loc_06efd`'s AL: (2 AL + the troop's byte 16h, at
/// most FFh) x its byte 1Ah / 1000h, as AL; at least 1 when byte 1Ah is
/// set.
pub fn troop_strength(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x348a)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x6efd, 0x348d)?;
    let si = c.reg(SI);
    let al = c.reg8(r8::AL);
    let b16 = c.byte(si.wrapping_add(0x16));
    let b1a = c.byte(si.wrapping_add(0x1a));
    let (s, carry) = al.wrapping_add(al).overflowing_add(b16);
    let v = if carry { 0xff } else { s };
    let r = (v as u16 * b1a as u16) >> 12;
    c.run(0x348d, 4 + carry as u32 + 6 + 4 + if r == 0 { 4 } else { 1 })?;
    c.xor(B, 0, 0);
    let a2 = c.add(B, al as u16, al as u16);
    let s = c.add(B, a2, b16 as u16);
    c.set_reg(AX, if carry { 0xff } else { s });
    c.mul8(b1a);
    let p = c.reg(AX);
    let p = c.shr1(W, p, 4);
    let ah = p >> 8;
    c.xor(B, 0, 0);
    c.or(W, ah, ah);
    let mut al = ah;
    if ah == 0 {
        c.cmp(B, b1a as u16, 1);
        let cf = c.flag(pc::cpu::CF);
        c.set_flag(pc::cpu::CF, !cf);
        al = c.adc(B, 0, 0);
    }
    c.set_reg(AX, al);
    c.ret()
}

/// `loc_034a5`: the 33h bytes at 60h cleared, `loc_06639` with BP =
/// 34D0h; then [91h] = [60h] + [7Eh], [92h] = [61h] + [7Fh]. SI kept.
pub fn tally_34a5(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x34a5)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    let si = c.reg(SI);
    c.push(si);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    let di = c.reg(DI);
    c.push(di);
    c.set_reg(DI, 0x60);
    c.set_reg(CX, 0x33);
    c.xor(B, 0, 0);
    c.set_reg8(r8::AL, 0);
    c.m.cpu.string_op(&mut c.m.hw, 0xaa, None, 0xf3); // rep stosb
    c.run(0x34b3, 3)?;
    let di = c.pop();
    c.set_reg(DI, di);
    c.set_reg(BP, 0x34d0);
    c.call(0x6639, 0x34ba)?;
    c.run(0x34ba, 8)?;
    for (a, b, to) in [(0x60u16, 0x7eu16, 0x91u16), (0x61, 0x7f, 0x92)] {
        let x = c.byte(a);
        let y = c.byte(b);
        let s = c.add(B, x as u16, y as u16) as u8;
        c.set_reg8(r8::AL, s);
        c.set_byte(to, s);
    }
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_03df4`: AL into the CL-slot ring at DI: at (AL + CH) mod CL when
/// that slot is free (FFh), else at the first free one (none: nothing).
pub fn put_in_ring(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3df4)?;
    let mut c = Ctx::new(m);
    let (ax, cx, di) = (c.reg(AX), c.reg(CX), c.reg(DI));
    let (ch, cl) = ((cx >> 8) as u8, cx as u8);
    if cl == 0 {
        return Err(Bail);
    }
    let mut bl = (ax as u8).wrapping_add(ch);
    let mut rounds = 0;
    loop {
        rounds += 1;
        let (r, borrow) = bl.overflowing_sub(cl);
        bl = r;
        if borrow {
            break;
        }
    }
    let bl = bl.wrapping_add(cl);
    let bx = ax & 0xff00 | bl as u16;
    let free = c.byte(bx.wrapping_add(di)) == 0xff;
    let mut n = 2 + 2 * rounds + 3;
    let mut slot = None;
    if free {
        n += 2;
        slot = Some(bx);
    } else {
        n += 1;
        let mut b = 0u16;
        loop {
            n += 3;
            if b as u8 >= cl {
                n += 1;
                break;
            }
            n += 2;
            if c.byte(b.wrapping_add(di)) == 0xff {
                n += 2;
                slot = Some(b);
                break;
            }
            b += 1;
        }
    }
    c.run(start, n)?;
    let mut b = c.add(B, ax & 0xff, ch as u16);
    loop {
        let r = c.sub(B, b, cl as u16);
        b = r;
        if c.flag(pc::cpu::CF) {
            break;
        }
    }
    let b = c.add(B, b, cl as u16);
    let bx = ax & 0xff00 | b;
    let v = c.byte(bx.wrapping_add(di));
    c.cmp(B, v as u16, 0xff);
    c.set_reg(BX, bx);
    if !free {
        let mut b = 0xffffu16;
        loop {
            b = c.inc(W, b);
            c.cmp(B, b & 0xff, cl as u16);
            if b as u8 >= cl {
                break;
            }
            let v = c.byte(b.wrapping_add(di));
            c.cmp(B, v as u16, 0xff);
            if v == 0xff {
                break;
            }
        }
        c.set_reg(BX, b);
    }
    if let Some(s) = slot {
        c.set_byte(s.wrapping_add(di), ax as u8);
    }
    c.ret()
}

/// From 36EE (the stretch from `at`, `prefix` instructions before, done by
/// `ops`): BP's routine for each of the 16 records at 0FD8h (10h bytes)
/// at the player's place ([4], [6]), with BX, DX as given and CX, SI, BP
/// kept. BX, DX kept.
fn for_records_here(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 6)?;
    ops(c);
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.set_reg(SI, 0x0fd8);
    c.set_reg(CX, 0x10);
    let (y, x) = (c.word(6), c.word(4));
    c.set_reg(BX, y);
    c.set_reg(DX, x);
    let mut tail = 0;
    loop {
        let si = c.reg(SI);
        let cx = c.reg(CX);
        let (y, x) = (c.reg(BX), c.reg(DX));
        let (w2, w0) = (c.word(si.wrapping_add(2)), c.word(si));
        let last = cx.wrapping_sub(1) == 0;
        let hit = y == w2 && x == w0;
        if !hit {
            let n = if y != w2 { 2 } else { 4 };
            c.run(0x36fe, n + 2 + if last { 3 } else { 0 })?;
            c.cmp(W, y, w2);
            if y == w2 {
                c.cmp(W, x, w0);
            }
        } else {
            c.run(0x36fe, 12)?;
            c.cmp(W, y, w2);
            c.cmp(W, x, w0);
            let dx = c.pop();
            let bx = c.pop();
            c.set_reg(DX, dx);
            c.set_reg(BX, bx);
            c.push(bx);
            c.push(dx);
            c.push(cx);
            c.push(si);
            let bp = c.reg(BP);
            c.push(bp);
            c.call(bp, 0x3710)?;
            c.run(0x3710, 5 + 2 + if last { 3 } else { 0 })?;
            let bp = c.pop();
            c.set_reg(BP, bp);
            let si = c.pop();
            c.set_reg(SI, si);
            let cx = c.pop();
            c.set_reg(CX, cx);
            let (y, x) = (c.word(6), c.word(4));
            c.set_reg(BX, y);
            c.set_reg(DX, x);
        }
        let si = c.reg(SI);
        let si = c.add(W, si, 0x10);
        c.set_reg(SI, si);
        let cx = c.reg(CX).wrapping_sub(1);
        c.set_reg(CX, cx);
        tail += 1;
        if cx == 0 {
            let _ = tail;
            let dx = c.pop();
            c.set_reg(DX, dx);
            let bx = c.pop();
            c.set_reg(BX, bx);
            return c.ret();
        }
    }
}

/// `loc_036ee`: see [`for_records_here`].
pub fn for_records_here_bp(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x36ee)?;
    for_records_here(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_040c3`: [`for_records_here`] with `sub_40c9`.
pub fn for_records_here_40c9(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x40c3)?;
    for_records_here(&mut Ctx::new(m), start, 2, |c| c.set_reg(BP, 0x40c9))
}

/// `loc_04944`: `loc_050be`; with DI from FFF0h up (a screen point) the
/// map point under it (`loc_0b5f9`) made the place ([11CBh] one down,
/// `get_map_position`, `loc_05133`), DI = [1150h], CL = 1; else
/// `loc_05124` and CL = 0. Then [11C5h] = DI, [11C8h] = CL and
/// [`add_11c7`].
pub fn place_4944(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4944)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x50be, 0x4947)?;
    let di = c.reg(DI);
    if di < 0xfff0 {
        c.run(0x4947, 3)?;
        c.cmp(W, di, 0xfff0);
        c.call(0x5124, 0x4968)?;
        return add_11c7(&mut c, 0x4968, 5, |c| {
            let cx = c.reg(CX);
            let z = c.xor(W, cx, cx);
            c.set_reg(CX, z);
            let di = c.reg(DI);
            c.set_word(0x11c5, di);
            c.set_byte(0x11c8, 0);
            c.set_byte(0x11c7, 0);
        });
    }
    c.run(0x4947, 4)?;
    c.cmp(W, di, 0xfff0);
    let v = c.byte(0x11cb);
    let v = c.dec(B, v as u16);
    c.set_byte(0x11cb, v as u8);
    c.call(0xb5f9, 0x4953)?;
    c.run(0x4953, 3)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.set_reg(CX, bx);
    c.set_reg(DI, dx);
    c.call(code::GET_MAP_POSITION, 0x495a)?;
    c.run(0x495a, 1)?;
    c.call(0x5133, 0x495d)?;
    add_11c7(&mut c, 0x495d, 7, |c| {
        let di = c.word(0x1150);
        c.set_reg(DI, di);
        c.set_reg8(r8::CL, 1);
        c.set_word(0x11c5, di);
        c.set_byte(0x11c8, 1);
        c.set_byte(0x11c7, 0);
    })
}

/// `loc_05124`: `loc_05133` for the map position and the place at DI
/// (its words 2, 4 as DI, CX). DI kept.
pub fn place_5124(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5124)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let di = c.reg(DI);
    c.push(di);
    c.call(code::GET_MAP_POSITION, 0x5128)?;
    c.run(0x5128, 3)?;
    let di = c.reg(DI);
    let (cx, d) = (c.word(di.wrapping_add(4)), c.word(di.wrapping_add(2)));
    c.set_reg(CX, cx);
    c.set_reg(DI, d);
    c.call(0x5133, 0x5131)?;
    c.run(0x5131, 2)?;
    let di = c.pop();
    c.set_reg(DI, di);
    c.ret()
}

/// `loc_0456c`: AX = 2BCh + the phrase number of the place DI: DI itself
/// when its high byte is FFh, else from its word 0 ((low - 1) x 16 + high).
pub fn place_phrase(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x456c)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let direct = di >> 8 == 0xff;
    c.run(start, if direct { 5 } else { 13 })?;
    let mut ax = di;
    if !direct {
        let w = c.word(di);
        let w = pure::dec(W, w);
        let al = pure::shl1(B, w & 0xff, 4);
        let al = pure::or(B, al, w >> 8);
        ax = al;
    }
    let ax = pure::add(W, ax, 0x2bc);
    c.set_reg(AX, ax);
    c.ret()
}

/// `loc_04658`: unless a message runs ([473Fh]): phrase 57h shown as a
/// message (its far pointer at 473Fh, 55h at 4743h, the box at 4745h by
/// [473Eh]) by the frame task 46B5h every 18h ticks. Registers kept.
pub fn start_message_57(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4658)?;
    let mut c = Ctx::new(m);
    let v = c.word(0x473f);
    c.run(start, 3)?;
    if v != 0 {
        return c.ret();
    }
    c.call(code::SAVE_REGS, 0x4662)?;
    c.run(0x4662, 2)?;
    c.set_reg(SI, 0x57);
    c.call(0xcf70, 0x4668)?;
    let f = c.byte(0x473e);
    c.run(0x4668, 7 + if f != 0 { 2 } else { 0 } + 5)?;
    let es = c.sreg(pc::cpu::ES);
    c.set_word(0x4741, es);
    let si = c.reg(SI);
    c.set_word(0x473f, si);
    c.set_word(0x4743, 0x55);
    let mut cx = 0xf561u16;
    let mut ax = 0x22u16;
    if f != 0 {
        ax = pure::add(B, ax, 4);
        cx = 0x2000 | cx & 0xff;
    }
    c.set_reg(CX, cx);
    c.set_reg(AX, ax);
    c.set_word(0x4745, ax);
    c.set_word(0x4747, cx);
    c.set_reg(SI, 0x46b5);
    c.set_reg(BP, 0x18);
    c.call(code::ADD_FRAME_TASK, 0x4697)?;
    c.run(0x4697, 1)?;
    c.call(code::RESTORE_REGS, 0x469a)?;
    c.run(0x469a, 1)?;
    c.ret()
}

/// `loc_0368b`: by the screen kind (bits 3 of [11C9h]): 2, `loc_04aeb` and
/// the game rect copied; 1, picture 34h with message AX (`loc_0978e`,
/// unless negative), the screen copied ([473Eh], [47A4h] set). [4728h]
/// bit 0 set. Registers kept.
pub fn show_screen_368b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x368b)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SAVE_REGS, 0x368e)?;
    let k = c.byte(0x11c9) & 3;
    c.run(0x368e, if k == 2 { 6 } else if k == 1 { 12 } else { 8 })?;
    let v = c.byte(0x4728);
    let v = c.or(B, v as u16, 1);
    c.set_byte(0x4728, v as u8);
    let bl = c.byte(0x11c9);
    let bl = c.and(B, bl as u16, 3) as u8;
    c.set_reg8(r8::BL, bl);
    c.cmp(B, bl as u16, 2);
    if k == 2 {
        c.call(0x4aeb, 0x36ce)?;
        c.run(0x36ce, 1)?;
        c.call(0xc474, 0x36d1)?;
        c.run(0x36d1, 2)?;
    } else {
        let bl = c.dec(B, bl as u16) as u8;
        c.set_reg8(r8::BL, bl);
        if k == 1 {
            c.set_byte(0x473e, 1);
            c.set_byte(0x47a4, 1);
            let ax = c.reg(AX);
            c.push(ax);
            c.set_reg8(r8::AL, 0x34);
            c.call(0xc2f2, 0x36b3)?;
            c.run(0x36b3, 1)?;
            c.call(0xc0f4, 0x36b6)?;
            c.run(0x36b6, 1)?;
            c.call(0xc412, 0x36b9)?;
            let sp = c.reg(pc::cpu::SP);
            let ax = c.word_in(pc::cpu::SS, sp);
            let neg = ax & 0x8000 != 0;
            c.run(0x36b9, if neg { 4 } else { 5 })?;
            c.pop();
            c.set_reg(AX, ax);
            c.or(W, ax, ax);
            if !neg {
                c.set_word(0x47c4, ax);
                c.call(0x978e, 0x36c4)?;
                c.run(0x36c4, 1)?;
            }
            c.call(0xc4dd, 0x36c7)?;
            c.run(0x36c7, 1)?;
        }
    }
    c.call(code::RESTORE_REGS, 0x36ca)?;
    c.run(0x36ca, 1)?;
    c.ret()
}

/// `loc_0503c`: entering the location at DI: [0FDh], [2Bh] cleared; a
/// place to show ([2Bh] set, `loc_06144`, [11DDh] = its picture 2Fh..33h
/// by kind) when it has bit 2 of byte 0Ah or is of a kind not passed by
/// `location_kind_check` with troops counted (`loc_05098`, DX); else with
/// CX, [46D9h] = 4.
pub fn enter_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x503c)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let f = c.byte(di.wrapping_add(0x0a));
    c.run(start, if f & 2 != 0 { 4 } else { 5 })?;
    c.set_byte(0xfd, 0);
    c.set_byte(0x2b, 0);
    c.test(B, f as u16, 2);
    if f & 2 == 0 {
        c.call(0x5d36, 0x504f)?;
        if c.flag(pc::cpu::CF) {
            c.run(0x504f, 2)?;
            return c.ret();
        }
        c.run(0x504f, 2)?;
        c.call(0x5098, 0x5054)?;
        let (dx, cx) = (c.reg(DX), c.reg(CX));
        if dx == 0 {
            c.run(0x5054, if cx == 0 { 4 } else { 5 })?;
            c.or(W, 0, 0);
            if cx != 0 {
                c.set_byte(0x46d9, 4);
            }
            return c.ret();
        }
        c.run(0x5054, 2)?;
        c.or(W, dx, dx);
    }
    c.run(0x5058, 2)?;
    let v = c.byte(0x2b);
    let v = c.inc(B, v as u16);
    c.set_byte(0x2b, v as u8);
    c.call(0x6144, 0x505f)?;
    let di = c.reg(DI);
    let kind = c.byte(di.wrapping_add(8));
    let n = if kind < 0x20 { 4 } else if kind == 0x30 { 7 } else if kind < 0x28 { 9 } else { 10 };
    c.run(0x505f, n + 2)?;
    c.set_reg8(r8::AL, kind);
    let mut ah = 0x2fu16;
    c.cmp(B, kind as u16, 0x20);
    let mut al = kind as u16;
    if kind >= 0x20 {
        ah = c.inc(B, ah);
        c.cmp(B, kind as u16, 0x30);
        if kind != 0x30 {
            al = c.sub(B, kind as u16, 0x28);
            if kind >= 0x28 {
                ah = c.add(B, ah, 3);
            }
        }
    }
    c.set_reg(AX, ah << 8 | al);
    c.set_byte(0x11dd, ah as u8);
    c.ret()
}

/// `sub_4415`: leaving the map's hotspot mode (when [46EBh] was set): the
/// markers cleared, the mouse rect, the frame task 44ABh and the message
/// gone, the rects and panel put back (`loc_05ba0`, `loc_043e3`, the
/// palette, the buttons, `loc_02ffb`; [11C9h] cleared without a place;
/// `loc_049d4` with [4728h] positive); then [11CAh] cleared.
pub fn leave_hotspot_mode(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4415)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x46eb);
    c.run(start, if k == 0 { 5 } else { 6 })?;
    c.xor(B, 0, 0);
    c.set_byte(0x46eb, 0);
    c.set_reg8(r8::AL, k);
    c.or(B, k as u16, k as u16);
    if k == 0 {
        return c.ret();
    }
    c.set_word(0xa5c0, 0);
    c.call(0xdaa3, 0x4429)?;
    c.run(0x4429, 2)?;
    c.set_reg(SI, 0x44ab);
    c.call(code::REMOVE_FRAME_TASK, 0x442f)?;
    let mut at = 0x442f;
    for t in [0x469b, 0x5ba0, 0x43e3, 0xc0f4] {
        c.run(at, 1)?;
        c.call(t, at + 3)?;
        at += 3;
    }
    let place = c.word(0x11c5);
    c.run(0x443b, 2 + (place == 0) as u32 + 1)?;
    c.cmp(W, place, 0);
    if place == 0 {
        c.set_byte(0x11c9, 0);
    }
    c.call(0xd95b, 0x444a)?;
    c.run(0x444a, 1)?;
    c.call(0xd717, 0x444d)?;
    c.run(0x444d, 1)?;
    c.call(0x2ffb, 0x4450)?;
    let f = c.byte(0x4728);
    let pos = (f as i8) > 0;
    c.run(0x4450, 2 + pos as u32 + if pos { 0 } else { 1 })?;
    c.cmp(B, f as u16, 0);
    if pos {
        c.call(0x49d4, 0x445a)?;
        c.run(0x445a, 1)?;
    }
    clear_flag_11ca(c.m, 0x4ac4)
}

/// `sub_46b5` (a frame task): the message's next character drawn (small
/// font, colour and place at 4743h..4747h, on the screen), again for a
/// space; a negative byte ends it.
pub fn message_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x46b5)?;
    let mut c = Ctx::new(m);
    loop {
        let (si, seg) = (c.word(0x473f), c.word(0x4741));
        let ch = c.byte_at_seg(seg, si);
        let end = ch & 0x80 != 0;
        c.run(0x46b5, if end { 5 } else { 7 })?;
        c.set_reg(SI, si);
        c.set_sreg(pc::cpu::ES, seg);
        c.set_reg8(r8::AL, ch);
        if end {
            return c.ret();
        }
        let p = c.word(0x473f);
        let p = pure::inc(W, p);
        c.set_word(0x473f, p);
        c.set_reg(SI, 0x14a4);
        c.call(0xdb74, 0x46ca)?;
        c.run(0x46ca, 2)?;
        let fb = c.word(g::FRAMEBUFFER_ACTIVE);
        c.push(fb);
        c.call(0xc08e, 0x46d1)?;
        c.run(0x46d1, 3)?;
        let (x, y) = (c.word(0x4743), c.word(0x4745));
        c.set_reg(DX, x);
        c.set_reg(BX, y);
        c.call(0xd04e, 0x46dc)?;
        c.run(0x46dc, 3)?;
        let colour = c.word(0x4747);
        c.set_reg(CX, colour);
        c.set_word(g::TEXT_COLOUR, colour);
        c.call(0xd075, 0x46e7)?;
        c.run(0x46e7, 2)?;
        let ax = c.reg(AX);
        c.push(ax);
        c.call(0xd12f, 0x46eb)?;
        c.run(0x46eb, 1)?;
        c.call(0xd05f, 0x46ee)?;
        c.run(0x46ee, 5)?;
        let (dx, bx) = (c.reg(DX), c.reg(BX));
        c.set_word(0x4743, dx);
        c.set_word(0x4745, bx);
        let ax = c.pop();
        c.set_reg(AX, ax);
        let fb = c.pop();
        c.set_word(g::FRAMEBUFFER_ACTIVE, fb);
        c.call(0xdb67, 0x46fe)?;
        let al = c.reg8(r8::AL);
        c.run(0x46fe, if al == 0x20 { 2 } else { 3 })?;
        if al != 0x20 {
            return c.ret();
        }
    }
}

/// `loc_08347`: the 3 words at 11D5h moved down to 11D3h. SI, DI kept; ES
/// = DS.
pub fn shift_11d3(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8347)?;
    let mut c = Ctx::new(m);
    c.run(start, 12)?;
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.push(si);
    c.push(di);
    c.set_reg(DI, 0x11d3);
    c.set_reg(SI, 0x11d5);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    for _ in 0..3 {
        c.movs(W);
    }
    let d = c.pop();
    c.set_reg(DI, d);
    let s = c.pop();
    c.set_reg(SI, s);
    c.ret()
}

/// `loc_0998e`: the saved game state swapped with the copy at 4C60h
/// (`swap_byte_arrays`: 4 words from 1BF0h, 1 from 22A6h, C9h from 4540h,
/// 7 from 47C4h).
pub fn swap_game_state(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x998e)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_reg(DI, 0x4c60);
    c.set_reg(SI, 0x1bf0);
    c.set_reg(CX, 4);
    c.call(0x99b2, 0x999a)?;
    c.run(0x999a, 3)?;
    c.set_reg(SI, 0x22a6);
    c.set_reg(CX, 1);
    c.call(0x99b2, 0x99a3)?;
    c.run(0x99a3, 3)?;
    c.set_reg(SI, 0x4540);
    c.set_reg(CX, 0xc9);
    c.call(0x99b2, 0x99ac)?;
    c.run(0x99ac, 2)?;
    c.set_reg(SI, 0x47c4);
    c.set_reg(CX, 7);
    super::video::swap_byte_arrays(c.m, 0x99b2)
}

/// `loc_04d6c`: the script at CS:[0AA6Eh] (restarted from CS:[167h] +
/// its first word at an FFh) run up to its 0: each byte (1 takes the next
/// as AL, AH 1) to `loc_04da0` with DS = CS; [0AA6Eh] after the 0.
pub fn run_cs_script(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4d6c)?;
    let mut c = Ctx::new(m);
    let si = c.word(0xaa6e);
    let b = c.byte_in(pc::cpu::CS, si);
    if b == 0xff {
        c.run(start, 8)?;
        c.set_reg(SI, si);
        let s = c.code_word(0x167);
        let v = c.code_word(s);
        let s = pure::add(W, s, v);
        c.set_reg(SI, s);
        c.set_word(0xaa6e, s);
    } else {
        c.run(start, 5)?;
        c.set_reg(SI, si);
    }
    let cs = c.sreg(pc::cpu::CS);
    c.push(cs);
    let v = c.pop();
    c.set_sreg(pc::cpu::DS, v);
    loop {
        let p = c.reg(SI);
        let al = c.byte_in(pc::cpu::DS, p);
        if al == 0 {
            c.run(0x4d84, 8)?;
            c.lods(B, pc::cpu::DS);
            c.set_reg8(r8::AH, 0);
            let ss = c.sreg(SS);
            c.push(ss);
            let v = c.pop();
            c.set_sreg(pc::cpu::DS, v);
            let si = c.reg(SI);
            c.set_word(0xaa6e, si);
            return c.ret();
        }
        c.run(0x4d84, 8 + if al == 1 { 2 } else { 0 })?;
        c.lods(B, pc::cpu::DS);
        c.set_reg8(r8::AH, 0);
        if al == 1 {
            c.set_reg8(r8::AH, 1);
            c.lods(B, pc::cpu::DS);
        }
        let si = c.reg(SI);
        c.push(si);
        c.call(0x4da0, 0x4d96)?;
        c.run(0x4d96, 2)?;
        let s = c.pop();
        c.set_reg(SI, s);
    }
}

/// `loc_026ac`: the driver's picture call (far [391Dh]: BP 15AAh, SI
/// 4C60h, ES the front buffer); then, from the words at SI, unless the
/// first is negative, `loc_0c22f` (BX, DX, AX); the area (7Eh, 4Ch) to
/// (C2h, 4Ch) copied to the screen unless the click handler is 1AD6h.
/// SI past the words read.
pub fn draw_scene_picture(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x26ac)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let si = c.reg(SI);
    c.push(si);
    c.set_reg(BP, 0x15aa);
    c.set_reg(SI, 0x4c60);
    let es = c.word(0xdbd6);
    c.set_sreg(pc::cpu::ES, es);
    c.interp(0x26b7, 0x26bb)?;
    let step = c.string_step(W);
    let si = c.word_in(SS, c.reg(SP));
    let a = c.word(si);
    if a & 0x8000 == 0 {
        c.run(0x26bb, 10)?;
        let s = c.pop();
        c.set_reg(SI, s);
        let ax = c.lods(W, pc::cpu::DS);
        c.or(W, ax, ax);
        c.set_reg(BX, ax);
        let dx = c.lods(W, pc::cpu::DS);
        c.set_reg(DX, dx);
        let ax = c.lods(W, pc::cpu::DS);
        let bx = c.reg(BX);
        c.set_reg(BX, ax);
        c.set_reg(AX, bx);
        c.call(0xc22f, 0x26cb)?;
        c.run(0x26cb, 5)?;
    } else {
        let _ = step;
        c.run(0x26bb, 9)?;
        let s = c.pop();
        c.set_reg(SI, s);
        let ax = c.lods(W, pc::cpu::DS);
        c.or(W, ax, ax);
    }
    c.set_reg(DX, 0x7e);
    c.set_reg(BX, 0x4c);
    c.set_reg(BP, 0xc2);
    c.set_reg(AX, 0x4c);
    let h = c.word(g::MAP_CLICK_HANDLER);
    if h == 0x1ad6 {
        c.run(0xc526, 3)?;
        c.cmp(W, h, 0x1ad6);
        return c.ret();
    }
    c.run(0xc526, 5)?;
    c.cmp(W, h, 0x1ad6);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.word(0xdbd8);
    let nds = c.word(0xdbd6);
    c.set_sreg(pc::cpu::ES, es);
    c.set_sreg(pc::cpu::DS, nds);
    c.interp(0xc537, 0xc53c)?;
    c.run(0xc53c, 2)?;
    let ds = c.pop();
    c.set_sreg(pc::cpu::DS, ds);
    c.ret()
}

/// `loc_0e851`: AX = [39B9h] + 2F13h compared with [0CE68h] (flags).
pub fn compare_39b9(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe851)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let a = c.word(0x39b9);
    let a = c.add(W, a, 0x2f13);
    c.set_reg(AX, a);
    let b = c.word(0xce68);
    c.cmp(W, a, b);
    c.ret()
}

/// `loc_00d45`: the sky by the time of day redrawn, then three times
/// darkened by 28h with a wait for [46D7h] to run from 20h below 10h;
/// then a wait for it to reach 0. [227Dh] counted up meanwhile.
pub fn sky_flash(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0d45)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x395c, 0x0d48)?;
    c.run(0x0d48, 1)?;
    c.call(0x3971, 0x0d4b)?;
    c.run(0x0d4b, 1)?;
    c.call(0x39b9, 0x0d4e)?;
    c.run(0x0d4e, 2)?;
    let v = c.byte(0x227d);
    let v = c.inc(B, v as u16);
    c.set_byte(0x227d, v as u8);
    c.set_reg(CX, 3);
    loop {
        c.run(0x0d55, 3)?;
        let cx = c.reg(CX);
        c.push(cx);
        c.set_reg8(r8::BL, 0x28);
        c.call(0x3971, 0x0d5b)?;
        c.run(0x0d5b, 1)?;
        c.call(0x398c, 0x0d5e)?;
        c.run(0x0d5e, 1)?;
        c.call(0xc0f4, 0x0d61)?;
        c.run(0x0d61, 4)?;
        c.set_byte(0x46d7, 0x20);
        loop {
            c.set_reg(BP, 0x391d);
            c.set_reg(AX, 3);
            c.call(0xe353, 0x0d6f)?;
            let k = c.byte(0x46d7);
            if k >= 0x10 {
                c.run(0x0d6f, 5)?;
                c.cmp(B, k as u16, 0x10);
                continue;
            }
            c.run(0x0d6f, 4)?;
            c.cmp(B, k as u16, 0x10);
            break;
        }
        let cx = c.pop().wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.run(0x0d79, 3)?;
    loop {
        c.set_reg(BP, 0x391d);
        c.set_reg(AX, 3);
        c.call(0xe353, 0x0d82)?;
        let k = c.byte(0x46d7);
        if k != 0 {
            c.run(0x0d82, 5)?;
            c.cmp(B, k as u16, 0);
            continue;
        }
        c.run(0x0d82, 4)?;
        c.cmp(B, k as u16, 0);
        break;
    }
    let v = c.byte(0x227d);
    let v = c.dec(B, v as u16);
    c.set_byte(0x227d, v as u8);
    c.ret()
}

/// `loc_04ec6`: [0DC02h] = a mode from [487Eh] (below 2 itself), the
/// low nibble of AL and [0DC00h] (AX): nibble 8 or more gives 3 (4 for
/// AX past 2 but not 5); below, AX up to 2 keeps [487Eh], up to 4 gives
/// 5, past 4 gives 2 (also into [487Eh]). BX kept.
pub fn pick_mode_dc02(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4ec6)?;
    let mut c = Ctx::new(m);
    let bx0 = c.reg(BX);
    let b = c.word(0x487e);
    let al = c.reg8(r8::AL);
    let ax = c.word(0xdc00);
    if b < 2 {
        c.run(start, 4 + 3)?;
        c.push(bx0);
        c.cmp(W, b, 2);
        c.set_word(0xdc02, b);
    } else if al & 0x0f < 8 {
        let n = if ax <= 2 { 2 } else if ax <= 4 { 5 } else { 7 };
        c.run(start, 8 + n + 3)?;
        c.push(bx0);
        c.cmp(W, b, 2);
        let a = c.and(B, al as u16, 0x0f);
        c.cmp(B, a, 8);
        c.set_reg(AX, ax);
        c.cmp(W, ax, 2);
        let mut v = b;
        if ax > 2 {
            v = 5;
            c.cmp(W, ax, 4);
            if ax > 4 {
                v = 2;
                c.set_word(0x487e, 2);
            }
        }
        c.set_word(0xdc02, v);
    } else {
        let n = if ax <= 2 { 3 } else if ax == 5 { 5 } else { 6 };
        c.run(start, 8 + n + 3)?;
        c.push(bx0);
        c.cmp(W, b, 2);
        let a = c.and(B, al as u16, 0x0f);
        c.cmp(B, a, 8);
        c.set_reg(AX, ax);
        c.cmp(W, ax, 2);
        let mut v = 3;
        if ax > 2 {
            c.cmp(W, ax, 5);
            if ax != 5 {
                v = 4;
            }
        }
        c.set_word(0xdc02, v);
    }
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.ret()
}

/// `loc_06c46`: in phases 2Dh..2Fh, without bit 10h of [10h], with a
/// location [473Ch] that `loc_0331e` leaves [66h] set for: the game phase
/// to 30h.
pub fn phase_30_check(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6c46)?;
    let mut c = Ctx::new(m);
    let ph = c.byte(0x2a);
    let al = ph.wrapping_sub(0x2d);
    let w = c.word(0x10);
    let di = c.word(0x473c);
    let n = if al >= 3 { 5 } else if w & 0x10 != 0 { 7 } else if di == 0 { 10 } else { 0 };
    if n != 0 {
        c.run(start, n)?;
        c.set_reg8(r8::AL, ph);
        let al = pure::sub(B, ph as u16, 0x2d);
        c.set_reg8(r8::AL, al as u8);
        if al >= 3 {
            return c.ret();
        }
        if w & 0x10 != 0 {
            return c.ret();
        }
        c.set_reg(DI, di);
        return c.ret();
    }
    c.run(start, 10)?;
    c.set_reg8(r8::AL, ph);
    let al = pure::sub(B, ph as u16, 0x2d);
    c.set_reg8(r8::AL, al as u8);
    c.set_reg(DI, di);
    c.call(0x331e, 0x6c62)?;
    let k = c.byte(0x66);
    if k == 0 {
        c.run(0x6c62, 3)?;
        return c.ret();
    }
    c.run(0x6c62, 4)?;
    c.set_reg8(r8::AL, 0x30);
    set_game_phase(c.m, 0x121f)
}

/// `loc_040d5`: [23h] = 7, `loc_036d3`, `loc_04ac4`, then `loc_040e6`
/// for the records here ([`for_records_here_bp`]).
pub fn records_here_40e6(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x40d5)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(0x23, 7);
    c.call(0x36d3, 0x40dd)?;
    c.run(0x40dd, 1)?;
    c.call(0x4ac4, 0x40e0)?;
    c.run(0x40e0, 2)?;
    c.set_reg(BP, 0x40e6);
    for_records_here_bp(c.m, 0x36ee)
}

/// `loc_04182`: in mode 1 of [11C9h]: unless [11CBh] is clear with the
/// location [11C5h] past `location_kind_check` (CF), at a map place
/// (`get_map_position`, `loc_0b532`) with bits 30h, `note_21fd_al` 40h
/// ([23h] 4 the first time), and after the [4726h] countdown by 20h
/// [46D9h] = 2; else [4726h] cleared and [`clear_21fd`].
pub fn map_place_watch(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4182)?;
    let mut c = Ctx::new(m);
    let mode = c.byte(0x11c9);
    let al = (mode & 3).wrapping_sub(1);
    if al != 0 {
        c.run(start, 5)?;
        c.set_reg8(r8::AL, mode);
        let a = c.and(B, mode as u16, 3);
        let a = c.dec(B, a);
        c.set_reg8(r8::AL, a as u8);
        return c.ret();
    }
    let k = c.byte(0x11cb);
    c.run(start, if k == 0 { 8 } else { 7 })?;
    c.set_reg8(r8::AL, mode);
    let a = c.and(B, mode as u16, 3);
    let a = c.dec(B, a);
    c.set_reg8(r8::AL, a as u8);
    c.cmp(B, k as u16, 0);
    let drop_it = |c: &mut Ctx, at: u16, n: u32| -> Result<(), Bail> {
        c.run(at, n)?;
        c.set_byte(0x4726, 0);
        c.xor(B, 0, 0);
        c.set_reg8(r8::AL, 0);
        note_21fd_al(c.m, 0x41cc)
    };
    if k == 0 {
        let di = c.word(0x11c5);
        c.set_reg(DI, di);
        c.call(0x5d36, 0x4199)?;
        if c.flag(pc::cpu::CF) {
            return drop_it(&mut c, 0x4199, 3);
        }
        c.run(0x4199, 2)?;
    }
    c.call(0x407e, 0x419e)?;
    c.run(0x419e, 1)?;
    c.call(0xb532, 0x41a1)?;
    let al = c.reg8(r8::AL);
    let a = al & 0x30;
    if a < 0x30 {
        c.run(0x41a1, 3)?;
        let a = c.and(B, al as u16, 0x30);
        c.set_reg8(r8::AL, a as u8);
        c.cmp(B, a, 0x30);
        return drop_it(&mut c, 0x41c5, 2);
    }
    let t = c.byte(0x4726);
    c.run(0x41a1, 5 + (t == 0) as u32 + 2)?;
    let a = c.and(B, al as u16, 0x30);
    c.set_reg8(r8::AL, a as u8);
    c.cmp(B, a, 0x30);
    c.cmp(B, t as u16, 0);
    if t == 0 {
        c.set_byte(0x23, 4);
    }
    c.set_reg8(r8::AL, 0x40);
    c.call(0x41cc, 0x41b8)?;
    let t = c.byte(0x4726);
    let r = t.wrapping_sub(0x20);
    c.run(0x41b8, if r != 0 { 3 } else { 4 })?;
    let r = c.sub(B, t as u16, 0x20);
    c.set_byte(0x4726, r as u8);
    if r != 0 {
        return c.ret();
    }
    c.set_byte(0x46d9, 2);
    c.ret()
}

/// `loc_026da`: the event AX added to the list of [0C8h] words at 1179h
/// unless there (registers kept): with 10 already, the oldest dropped
/// (`loc_0272f`) first; [0C8h], [0C9h] counted; from phase 38h, out of
/// room 8 ([0Bh]), `loc_029ee` with AX 201h.
pub fn note_event(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x26da)?;
    let mut c = Ctx::new(m);
    note_event_at(&mut c)
}

fn note_event_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x26da, 1)?;
    c.call(0xe270, 0x26dd)?;
    let ax = c.reg(AX);
    let n0 = c.byte(0xc8) as u16;
    let mut found = false;
    let mut k = 0u16;
    while k < n0 {
        if c.word(0x1179 + 2 * k) == ax {
            found = true;
            break;
        }
        k += 1;
    }
    if n0 == 0 {
        c.run(0x26dd, 3)?;
    } else if found {
        c.run(0x26dd, 4 + 4 * k as u32 + 3)?;
    } else {
        c.run(0x26dd, 4 + 4 * n0 as u32)?;
    }
    c.set_reg(CX, n0);
    c.xor(B, 0, 0);
    if n0 != 0 {
        c.set_reg(SI, 0x1179);
        for i in 0..=k.min(n0.saturating_sub(1)) {
            let p = 0x1179 + 2 * i;
            let w = c.word(p);
            c.cmp(W, ax, w);
            if found && i == k {
                c.set_reg(SI, p);
                c.set_reg(CX, n0 - k);
                c.call(0xe283, 0x272e)?;
                c.run(0x272e, 1)?;
                return c.ret();
            }
            c.add(W, p, 2);
            c.set_reg(SI, p + 2);
        }
        c.set_reg(CX, 0);
    }
    let cl = c.byte(0xc8);
    let full = cl >= 0x0a;
    c.run(0x26f1, 4 + if full { 2 } else { 0 })?;
    c.set_reg8(r8::CL, cl);
    c.set_reg(SI, 0x1179);
    c.cmp(B, cl as u16, 0x0a);
    let mut cl = cl;
    if full {
        c.push(ax);
        c.call(0x272f, 0x2701)?;
        c.run(0x2701, 3)?;
        cl = 9;
        c.set_reg8(r8::CL, 9);
        c.set_reg(SI, 0x1179);
        let a = c.pop();
        c.set_reg(AX, a);
    }
    let ph = c.byte(0x2a);
    let room = c.byte(0x0b);
    let call = ph >= 0x38 && room != 8;
    c.run(0x2707, 8 + if ph >= 0x38 { 2 + if call { 2 } else { 0 } } else { 0 } + if call { 0 } else { 1 })?;
    let d = c.add(B, cl as u16, cl as u16);
    c.xor(B, 0, 0);
    c.set_reg(CX, d);
    let si = c.add(W, 0x1179, d);
    c.set_reg(SI, si);
    let ax = c.reg(AX);
    c.set_word(si, ax);
    let v = c.byte(0xc8);
    let v = c.inc(B, v as u16);
    c.set_byte(0xc8, v as u8);
    let v = c.byte(0xc9);
    let v = c.inc(B, v as u16);
    c.set_byte(0xc9, v as u8);
    c.cmp(B, ph as u16, 0x38);
    if ph >= 0x38 {
        c.cmp(B, room as u16, 8);
    }
    if call {
        c.set_reg(AX, 0x201);
        c.call(0x29ee, 0x272b)?;
        c.run(0x272b, 1)?;
    }
    c.call(0xe283, 0x272e)?;
    c.run(0x272e, 1)?;
    c.ret()
}

/// `sub_1045`: record FD8h's byte 0 = 5, `loc_0101b`, then event 10Bh
/// ([`note_event`]).
pub fn event_10b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1045)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(0xfd8, 5);
    c.call(0x101b, 0x104d)?;
    c.run(0x104d, 2)?;
    c.set_reg(AX, 0x10b);
    note_event_at(&mut c)
}

/// `sub_1103`: `loc_029ee` with AX 4, then event 1409h ([`note_event`]).
pub fn event_1409(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1103)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(AX, 4);
    c.call(0x29ee, 0x1109)?;
    c.run(0x1109, 2)?;
    c.set_reg(AX, 0x1409);
    note_event_at(&mut c)
}

/// `loc_03950`: [46D7h] = 0 and the frame task 3916h removed.
pub fn stop_task_3916(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3950)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_byte(0x46d7, 0);
    c.set_reg(SI, 0x3916);
    super::tasks::remove_frame_task(c.m, 0xda5f)
}

/// `loc_029f0`: with bit 1 of [0Ah], the pair (AX, DI) added to the list
/// at 1190h (count byte, then 4-byte entries) unless there; with 10
/// already, the oldest dropped (`shift_list_1191`) first.
pub fn note_pair(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x29f0)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0x0a);
    if f & 1 == 0 {
        c.run(start, 3)?;
        c.test(B, f as u16, 1);
        return c.ret();
    }
    let (ax, di) = (c.reg(AX), c.reg(DI));
    let n0 = c.byte(0x1190) as u16;
    let mut n = 2 + 4;
    let mut found = None;
    if n0 != 0 {
        n += 1;
        for i in 0..n0 {
            let p = 0x1191 + 4 * i;
            let w = c.word(p);
            n += 2;
            if w == ax {
                n += 2;
                if c.word(p + 2) == di {
                    found = Some(i);
                    n += 1;
                    break;
                }
            }
            n += 2;
        }
        if found.is_none() {
            n += 2;
        }
    }
    if found.is_none() {
        n += 2;
    }
    let full = n0 >= 0x0a;
    if found.is_none() {
        n += if full { 4 } else { 0 };
    }
    c.run(start, n)?;
    c.test(B, f as u16, 1);
    c.set_reg(SI, 0x1190);
    c.set_reg(CX, n0);
    c.xor(B, 0, 0);
    if n0 != 0 {
        c.set_reg(SI, 0x1191);
        let mut p = 0x1191u16;
        for i in 0..n0 {
            let w = c.word(p);
            c.cmp(W, ax, w);
            if w == ax {
                let w2 = c.word(p + 2);
                c.cmp(W, di, w2);
                if found == Some(i) {
                    c.set_reg(SI, p);
                    c.set_reg(CX, n0 - i);
                    return c.ret();
                }
            }
            p = c.add(W, p, 4);
            c.set_reg(SI, p);
        }
        c.set_reg(SI, 0x1190);
        c.set_reg(CX, n0);
    }
    c.cmp(W, n0, 0x0a);
    let mut cx = n0;
    if full {
        c.push(ax);
        c.push(0x1190);
        c.push(di);
        c.call(0x2a34, 0x2a1f)?;
        c.run(0x2a1f, 4)?;
        let d = c.pop();
        c.set_reg(DI, d);
        let s = c.pop();
        c.set_reg(SI, s);
        let a = c.pop();
        c.set_reg(AX, a);
        cx = 9;
        c.set_reg(CX, 9);
    }
    c.run(0x2a25, 8)?;
    let si = c.reg(SI);
    let v = c.byte(si);
    let v = c.inc(B, v as u16);
    c.set_byte(si, v as u8);
    let si = c.inc(W, si);
    let cx = c.add(W, cx, cx);
    let cx = c.add(W, cx, cx);
    c.set_reg(CX, cx);
    let si = c.add(W, si, cx);
    c.set_reg(SI, si);
    let (ax, di) = (c.reg(AX), c.reg(DI));
    c.set_word(si, ax);
    c.set_word(si.wrapping_add(2), di);
    c.ret()
}

/// `loc_02ad8`: the pending event [1191h] (AH a bit of [12h], DI its
/// place [1193h]): when that bit is set (and, for bit 0Fh, it is the
/// current location) it is told: `loc_0dbb2`, `loc_0331e` for its place
/// ([11CEh] kept), the sentence AH by `loc_096d8`, `loc_09945`, `loc_09ef1`
/// with AL 1 ([0EAh] AL meanwhile, then FFh); CF set. Else CF clear.
pub fn tell_pending_event(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2ad8)?;
    let mut c = Ctx::new(m);
    let ev = c.word(0x1191);
    let w12 = c.word(0x12);
    let cl = (ev >> 8) as u8;
    let sh = (cl & 0x1f) as u32;
    let bit = sh < 16 && (w12 >> sh) & 1 != 0;
    let di = c.word(0x1193);
    let here = c.word(0x114e);
    let tell = bit && (cl != 0x0f || di == here);
    let n = 7 + if bit { 3 + if cl == 0x0f { 2 } else { 0 } } else { 0 };
    c.run(start, n + if tell { 2 } else { 3 })?;
    let ax0 = c.reg(AX);
    c.push(ax0);
    c.set_reg(AX, ev);
    c.set_reg8(r8::CL, cl);
    let b = c.m.cpu.shift(5, W, w12 as u32, (cl & 0x1f) as u32) as u16;
    let b = c.shr1(W, b, 1);
    c.set_reg(BX, b);
    if bit {
        c.set_reg(DI, di);
        c.cmp(B, cl as u16, 0x0f);
        if cl == 0x0f {
            c.cmp(W, di, here);
        }
    }
    if !tell {
        let a = c.pop();
        c.set_reg(AX, a);
        c.set_flag(CF, false);
        return c.ret();
    }
    let sp = c.reg(SP);
    let sp = c.add(W, sp, 2);
    c.set_reg(SP, sp);
    c.call(0xdbb2, 0x2b00)?;
    let di = c.reg(DI);
    let keep = c.word(0x11ce);
    c.run(0x2b00, 3 + if di != 0 { 2 } else { 0 })?;
    c.push(keep);
    c.or(W, di, di);
    if di != 0 {
        let ax = c.reg(AX);
        c.push(ax);
        c.call(0x331e, 0x2b0c)?;
        c.run(0x2b0c, 1)?;
        let a = c.pop();
        c.set_reg(AX, a);
    }
    c.run(0x2b0d, 4)?;
    let ax = c.reg(AX);
    c.set_byte(0xea, ax as u8);
    c.xor(B, 0, 0);
    c.set_reg(AX, ax >> 8);
    c.call(0x96d8, 0x2b17)?;
    c.run(0x2b17, 1)?;
    c.call(0x9945, 0x2b1a)?;
    c.run(0x2b1a, 3)?;
    c.set_byte(0xea, 0xff);
    c.set_reg8(r8::AL, 1);
    c.call(0x9ef1, 0x2b24)?;
    c.run(0x2b24, 2)?;
    let d = c.pop();
    c.set_reg(DI, d);
    c.call(0x331e, 0x2b28)?;
    c.run(0x2b28, 2)?;
    c.set_flag(CF, true);
    c.ret()
}

/// `loc_02a51`: the pending events at 1190h for the bit AL (for 0Fh only
/// those at the place DI) dropped, the rest moved down; when any went,
/// the list's end gets an empty entry. BP the count kept. Bails at entry
/// with DF set.
pub fn drop_events(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2a51)?;
    let mut c = Ctx::new(m);
    if c.flag(pc::cpu::DF) {
        return Err(Bail);
    }
    let dl = c.reg8(r8::AL);
    let bx = c.reg(DI);
    let cnt = c.byte(0x1190);
    if cnt == 0 {
        c.run(start, 9)?;
        c.set_reg(SI, 0x1191);
        c.set_reg8(r8::DL, dl);
        c.set_reg(BX, bx);
        c.xor(W, 0, 0);
        c.set_reg(CX, 0);
        c.set_reg(BP, 0);
        c.set_reg8(r8::AL, 0);
        return c.ret();
    }
    let mut n = 8 + 3;
    let mut kept = 0u16;
    for i in 0..cnt as u16 {
        let p = 0x1191 + 4 * i;
        let ev = c.word(p);
        let place = c.word(p + 2);
        n += 3;
        let drop = if (ev >> 8) as u8 == dl {
            n += 2;
            if dl == 0x0f {
                n += 2;
                bx == place
            } else {
                true
            }
        } else {
            false
        };
        n += if drop { 2 } else { 3 };
        n += 1;
        if !drop {
            kept += 1;
        }
    }
    let changed = (kept as u8) != cnt;
    n += 1 + 5 + if changed { 4 } else { 1 };
    c.run(start, n)?;
    c.set_reg(SI, 0x1190);
    c.set_reg8(r8::DL, dl);
    c.set_reg(BX, bx);
    c.xor(W, 0, 0);
    c.set_reg(CX, 0);
    c.set_reg(BP, 0);
    c.lods(B, DS);
    c.set_reg(CX, cnt as u16);
    let si = c.reg(SI);
    c.set_reg(DI, si);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    let mut bp = 0u16;
    for i in 0..cnt as u16 {
        let ev = c.lods(W, DS);
        let ah = ev >> 8;
        c.cmp(B, ah, dl as u16);
        let mut drop = false;
        if ah as u8 == dl {
            c.cmp(B, dl as u16, 0x0f);
            if dl == 0x0f {
                let si = c.reg(SI);
                let place = c.word(si);
                c.cmp(W, bx, place);
                drop = bx == place;
            } else {
                drop = true;
            }
        }
        if drop {
            let si = c.reg(SI);
            let si = c.add(W, si, 2);
            c.set_reg(SI, si);
        } else {
            c.stos(W);
            c.movs(W);
            bp = c.inc(W, bp);
            c.set_reg(BP, bp);
        }
        c.set_reg(CX, cnt as u16 - 1 - i);
    }
    let cl = bp as u8;
    c.set_byte(0x1190, cl);
    c.set_reg(CX, (cl as u16) << 8 | cnt as u16);
    c.cmp(B, cnt as u16, cl as u16);
    if !changed {
        return c.ret();
    }
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.stos(W);
    c.stos(W);
    c.ret()
}

/// `loc_032c7`: the age of the record SI ([2] - its word 0Ah) into [42h]
/// ([41h] its /16); [11F5h] = phrase 74h for bit 10h of its byte 3, else
/// 70h..73h by the age (3, 10h, 20h); for 73h the phrase is fetched
/// (`get_phrase_or_command_string_si`, `loc_0d03c`) and the age/16
/// written into it (`string_replace_number_ending_at_es_si`).
pub fn record_age_phrase(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x32c7)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let t = c.word(2);
    let a = c.word(si.wrapping_add(0x0a));
    let age = t.wrapping_sub(a);
    let b3 = c.byte(si.wrapping_add(3));
    let (phrase, n) = if b3 & 0x10 != 0 {
        (0x74u16, 10)
    } else if age < 3 {
        (0x70, 13)
    } else if age < 0x10 {
        (0x71, 16)
    } else if age < 0x20 {
        (0x72, 19)
    } else {
        (0x73, 24)
    };
    let full = phrase == 0x73;
    c.run(start, if full { n } else { n + 2 })?;
    let ax = pure::sub(W, t, a);
    c.set_word(0x42, ax);
    c.set_reg(DX, ax);
    c.set_reg8(r8::CL, 4);
    let q = c.m.cpu.shift(5, W, ax as u32, 4) as u16;
    c.set_byte(0x41, q as u8);
    let mut ax = 0x74u16;
    if b3 & 0x10 == 0 {
        ax = 0x70;
        for lim in [3u16, 0x10, 0x20] {
            if age < lim {
                break;
            }
            ax = pure::inc(W, ax);
        }
    }
    c.set_reg(AX, ax);
    if full {
        c.push(ax);
        c.push(si);
        c.set_reg(SI, ax);
        c.call(0xcf70, 0x32fe)?;
        c.run(0x32fe, 1)?;
        c.call(0xd03c, 0x3301)?;
        c.run(0x3301, 4)?;
        let dx = c.reg(DX);
        c.set_reg(AX, dx);
        c.set_reg8(r8::CL, 4);
        let q = c.m.cpu.shift(5, W, dx as u32, 4) as u16;
        c.set_reg(AX, q);
        c.call(0xe2e3, 0x330a)?;
        c.run(0x330a, 4)?;
        let s = c.pop();
        c.set_reg(SI, s);
        let a = c.pop();
        c.set_reg(AX, a);
        ax = a;
    }
    c.set_word(0x11f5, ax);
    c.ret()
}

/// `loc_01771`: [477Ah] = AX, [4778h] = 0, [4774h] counted, `loc_0b2b9`,
/// `loc_0ad5e`, [4776h] = ([46E0h], [4]); then the frame task 176Bh every
/// 64h ticks.
pub fn start_task_176b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1771)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let ax = c.reg(AX);
    c.set_word(0x477a, ax);
    c.set_word(0x4778, 0);
    let v = c.byte(0x4774);
    let v = pure::inc(B, v as u16);
    c.set_byte(0x4774, v as u8);
    c.call(0xb2b9, 0x1781)?;
    c.run(0x1781, 1)?;
    c.call(0xad5e, 0x1784)?;
    c.run(0x1784, 6)?;
    let lo = c.byte(4);
    let hi = c.byte(0x46e0);
    let ax = (hi as u16) << 8 | lo as u16;
    c.set_reg(AX, ax);
    c.set_word(0x4776, ax);
    c.set_reg(BP, 0x64);
    c.set_reg(SI, 0x176b);
    super::tasks::add_frame_task(c.m, 0xda25)
}

/// `loc_04da0`: the sprite list AX - 2 of the table CS:169h: each entry
/// (sprite, x, y; to a 0) drawn by the driver's blit (far [38CDh]) at (x
/// + [0AA66h], y + [0AA68h]) from the sprite table [0DBB0h] to the screen
/// [0DBDAh], clipped by 0AA66h.
pub fn draw_cs_sprite_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4da0)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let ax = c.reg(AX);
    let ax = c.sub(W, ax, 2);
    let ax = c.shl1(W, ax, 1);
    c.set_reg(AX, ax);
    c.set_reg(BP, ax);
    let off = c.code_word(ax.wrapping_add(0x169));
    let si = c.add(W, 0x169, off);
    c.set_reg(SI, si);
    sprite_list_loop(&mut c, 0x4dad, 24, &[0xaa66, 0xaa68], &[], 0xaa66, 0x4de3, true)
}

/// The loop shared by `loc_04da0` and `loc_09d94`: from `at`, entries of
/// (sprite, x, y) at SI to a 0, x and y offset by the SS words `add`
/// and less `sub`, each blitted (far SS:[38CDh] at `blit`) from the
/// sprite table SS:[0DBB0h] (DS kept), BP `clip`. `n` counts an entry up
/// to the blit; `set_es` loads ES from SS:[0DBDAh] first.
#[allow(clippy::too_many_arguments)]
fn sprite_list_loop(
    c: &mut Ctx,
    at: u16,
    n: u32,
    add: &[u16],
    sub: &[u16],
    clip: u16,
    blit: u16,
    set_es: bool,
) -> Result<(), Bail> {
    loop {
        let si = c.reg(SI);
        let al = c.byte(si);
        if al == 0 {
            c.run(at, 4)?;
            c.lods(B, DS);
            if set_es {
                c.or(B, 0, 0);
            } else {
                c.and(W, 0, 0xff);
                c.set_reg(AX, 0);
            }
            return c.ret();
        }
        c.run(at, n)?;
        c.lods(B, DS);
        if set_es {
            c.or(B, al as u16, al as u16);
        } else {
            let ax = c.reg(AX);
            let v = c.and(W, ax, 0xff);
            c.set_reg(AX, v);
        }
        c.xor(B, 0, 0);
        c.set_reg(AX, al as u16);
        let x = c.lods(B, DS);
        let y = c.lods(B, DS);
        let (mut dx, mut bx) = (x, y);
        for (i, &a) in add.iter().enumerate() {
            let v = c.word_in(SS, a);
            if i % 2 == 0 {
                dx = c.add(W, dx, v);
            } else {
                bx = c.add(W, bx, v);
            }
        }
        for (i, &a) in sub.iter().enumerate() {
            let v = c.word_in(SS, a);
            if i % 2 == 0 {
                dx = c.sub(W, dx, v);
            } else {
                bx = c.sub(W, bx, v);
            }
        }
        c.set_reg(DX, dx);
        c.set_reg(BX, bx);
        let si = c.reg(SI);
        c.push(si);
        let ds = c.sreg(DS);
        c.push(ds);
        let bp = c.dec(W, al as u16);
        if set_es {
            let es = c.word_in(SS, 0xdbda);
            c.set_sreg(pc::cpu::ES, es);
        }
        let (tab, seg) = (c.word_in(SS, 0xdbb0), c.word_in(SS, 0xdbb2));
        c.set_sreg(DS, seg);
        let bp = c.shl1(W, bp, 1);
        let off = c.word_in(DS, bp.wrapping_add(tab));
        let s = c.add(W, tab, off);
        c.set_reg(SI, s);
        let w = c.lods(W, DS);
        c.set_reg(DI, w);
        let h = c.lods(W, DS);
        c.xor(B, 0, 0);
        let h = h & 0xff;
        c.set_reg(AX, h);
        c.set_reg(CX, h);
        c.set_reg(BP, clip);
        c.interp(blit, blit + 5)?;
        c.run(blit + 5, 3)?;
        let ds = c.pop();
        c.set_sreg(DS, ds);
        let s = c.pop();
        c.set_reg(SI, s);
    }
}

/// `loc_09d94`: the sprite list at SI drawn as [`draw_cs_sprite_list`]
/// does, at (x, y) + 1BF0h - 46D2h + 47D4h (SS words), clipped by 47D4h,
/// to ES.
pub fn draw_dialogue_sprites(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9d94)?;
    let mut c = Ctx::new(m);
    sprite_list_loop(
        &mut c,
        0x9d94,
        27,
        &[0x1bf0, 0x1bf2, 0x47d4, 0x47d6],
        &[0x46d2, 0x46d4],
        0x47d4,
        0x9dda,
        false,
    )
}

/// `loc_00a3e`: the frame task 0A16h removed.
pub fn stop_task_0a16(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0a3e)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(SI, 0x0a16);
    super::tasks::remove_frame_task(c.m, 0xda5f)
}

/// `loc_04d00`: the frame task 4BB9h removed.
pub fn stop_task_4bb9(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4d00)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(SI, 0x4bb9);
    super::tasks::remove_frame_task(c.m, 0xda5f)
}

/// `loc_008f0`: the place (DX, BX) made current ([4], [6], [8] its DH;
/// [47A4h], [46DFh] cleared; [114Eh] the location BH, E4h + 1Ch·BH),
/// then its SAL resource (`open_SAL_resource`).
pub fn set_current_place(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x08f0)?;
    let mut c = Ctx::new(m);
    c.run(start, 11)?;
    c.set_reg8(r8::AL, 0);
    c.set_byte(0x47a4, 0);
    c.set_byte(0x46df, 0);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.set_word(4, dx);
    c.set_word(6, bx);
    c.set_byte(8, (dx >> 8) as u8);
    c.set_reg8(r8::AL, 0x1c);
    c.mul8((bx >> 8) as u8);
    let ax = c.reg(AX);
    let ax = pure::add(W, ax, 0xe4);
    c.set_reg(AX, ax);
    c.set_word(0x114e, ax);
    super::ui::open_sal_resource(c.m, 0x2d74)
}

/// `sub_110f`: record FD8h's byte 10h set to 0Ah (word 12h 180h) unless
/// it is 8; then event 280Ah ([`note_event`]).
pub fn event_280a(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x110f)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0xfe8);
    c.run(start, if k == 8 { 4 } else { 6 })?;
    c.cmp(B, k as u16, 8);
    if k != 8 {
        c.set_byte(0xfe8, 0x0a);
        c.set_word(0xfea, 0x180);
    }
    c.set_reg(AX, 0x280a);
    note_event_at(&mut c)
}

/// `loc_029ee`: [`note_pair`] with DI 0.
pub fn note_pair_0(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x29ee)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.set_reg(DI, 0);
    note_pair(c.m, 0x29f0)
}

/// `sub_1166`: [1141h] counted, record FD8h's byte 10h 2 (word 12h
/// 180h), then pair 105h ([`note_pair_0`]).
pub fn event_105(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1166)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let v = c.byte(0x1141);
    let v = pure::inc(B, v as u16);
    c.set_byte(0x1141, v as u8);
    c.set_byte(0xfe8, 2);
    c.set_word(0xfea, 0x180);
    c.set_reg(AX, 0x105);
    note_pair_0(c.m, 0x29ee)
}

/// `loc_022b1`: the bands of AX against [0BCh] into SS:B4h (four words)
/// and bits 6 of [0BFh]: below it (AX, 3/4·AX, AX/2, 1/4·AX); else
/// ([0BCh], AX) then by 3/2 and 2 times it.
pub fn bands_b4(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x22b1)?;
    let mut c = Ctx::new(m);
    let ax = c.reg(AX);
    let bx = c.word(0xbc);
    let cx = (bx >> 1).wrapping_add(bx);
    let dx = bx.wrapping_add(bx);
    let si = ax >> 1;
    let di = (ax >> 2).wrapping_add(si);
    let n = 16 + if ax < bx {
        6
    } else if ax < cx {
        4 + 4
    } else if ax < dx {
        4 + 2 + 4
    } else {
        4 + 2 + 4
    };
    c.run(start, n)?;
    c.set_reg(BP, 0xb4);
    let f = c.byte(0xbf);
    let f = c.and(B, f as u16, 0xf9);
    c.set_byte(0xbf, f as u8);
    c.set_reg(BX, bx);
    let x = c.shr1(W, bx, 1);
    let x = c.add(W, x, bx);
    c.set_reg(CX, x);
    let d = c.add(W, bx, bx);
    c.set_reg(DX, d);
    let s = c.shr1(W, ax, 1);
    c.set_reg(SI, s);
    let t = c.shr1(W, ax, 2);
    let t = c.add(W, t, s);
    c.set_reg(DI, t);
    c.cmp(W, ax, bx);
    let put = |c: &mut Ctx, i: u16, v: u16| c.set_word_in(SS, 0xb4 + 2 * i, v);
    if ax < bx {
        put(&mut c, 0, ax);
        put(&mut c, 1, di);
        put(&mut c, 2, si);
        let d2 = c.sub(W, di, si);
        c.set_reg(DI, d2);
        put(&mut c, 3, d2);
        return c.ret();
    }
    put(&mut c, 0, bx);
    put(&mut c, 1, ax);
    c.cmp(W, ax, cx);
    let bits = if ax < cx {
        put(&mut c, 2, di);
        put(&mut c, 3, si);
        2
    } else {
        c.cmp(W, ax, dx);
        if ax < dx {
            put(&mut c, 2, di);
            put(&mut c, 3, cx);
            4
        } else {
            put(&mut c, 2, cx);
            put(&mut c, 3, dx);
            6
        }
    };
    let v = c.or(B, f, bits);
    c.set_byte(0xbf, v as u8);
    c.ret()
}

/// `sub_102f`: bit 80h of [124Ah] and [1247h] cleared, [121Dh] = FFFFh,
/// then [`start_task_176b`] with AX 1321h.
pub fn start_1321(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x102f)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    for p in [0x124au16, 0x1247] {
        let v = c.byte(p);
        let v = pure::and(B, v as u16, 0x7f);
        c.set_byte(p, v as u8);
    }
    c.set_word(0x121d, 0xffff);
    c.set_reg(AX, 0x1321);
    start_task_176b(c.m, 0x1771)
}

/// `loc_01071`: the game set to phase 15h with record FD8h's state
/// ([0FFh] 0, [0FDBh] 1, [1018h] 200Bh, [101Ah] 180h, [0FE8h] 0Ah,
/// [0D5h] FFh), `loc_02090`, bit 1 of [0Ah], then pair 1
/// ([`note_pair_0`]).
pub fn start_phase_15(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1071)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    c.set_byte(0xff, 0);
    c.set_byte(0x2a, 0x15);
    c.set_byte(0xfdb, 1);
    c.set_word(0x1018, 0x200b);
    c.set_word(0x101a, 0x180);
    c.set_byte(0xfe8, 0x0a);
    c.set_byte(0xd5, 0xff);
    c.call(0x2090, 0x1099)?;
    c.run(0x1099, 3)?;
    let v = c.byte(0x0a);
    let v = pure::or(B, v as u16, 1);
    c.set_byte(0x0a, v as u8);
    c.set_reg(AX, 1);
    note_pair_0(c.m, 0x29ee)
}

/// `loc_0331e`: the location DI becomes the one talked about ([11CEh]):
/// its fields copied to the condition variables (4Dh..54h, its 7 needs
/// to 55h, the name's region by 1141h), `loc_033be`, `loc_034a5`, the
/// needs with troops' skills (`skill_needs_46fe`) as a bit set in [53h],
/// `loc_03385`, `loc_05274`. SI, DI kept.
pub fn select_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x331e)?;
    let mut c = Ctx::new(m);
    c.run(start, 24)?;
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.set_word(0x11ce, di);
    c.push(si);
    c.push(di);
    let (b0, b1) = (c.byte(di), c.byte(di.wrapping_add(1)));
    let ax = (b0 as u16) << 8 | b1 as u16;
    c.set_word(0x4e, ax);
    c.set_reg(BX, 0x1141);
    let r = c.byte(0x1141u16.wrapping_add(b1 as u16));
    c.set_byte(0x50, r);
    let mut al = r;
    for (src, dst) in [(0x0au16, 0x51u16), (0x12, 0x52), (0x1b, 0x54), (8, 0x4d)] {
        al = c.byte(di.wrapping_add(src));
        c.set_byte(dst, al);
    }
    c.set_reg(AX, (b0 as u16) << 8 | al as u16);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.push(di);
    c.set_reg(SI, di.wrapping_add(0x14));
    c.set_reg(DI, 0x55);
    c.set_reg(CX, 7);
    c.rep(0xa4);
    c.run(0x3359, 2)?;
    let d = c.pop();
    c.set_reg(DI, d);
    c.call(0x33be, 0x335d)?;
    c.run(0x335d, 1)?;
    c.call(0x34a5, 0x3360)?;
    c.run(0x3360, 1)?;
    c.call(0x7f27, 0x3363)?;
    c.run(0x3363, 3 + 28 + 6)?;
    c.set_reg(DI, 0x46fe);
    let mut al = 0xffu16;
    for i in 0..7u16 {
        let b = c.byte(0x46fe + i);
        c.cmp(B, b as u16, 1);
        al = c.rcl1(B, al);
        c.inc(W, 0x46fe + i);
    }
    c.set_reg(DI, 0x46fe + 7);
    c.set_reg(CX, 0);
    let al = !al & 0xff;
    c.set_reg8(r8::CL, 1);
    let al = c.shl(B, al, 1);
    c.set_reg8(r8::AL, al as u8);
    c.set_byte(0x53, al as u8);
    let d = c.pop();
    c.set_reg(DI, d);
    c.call(0x3385, 0x3380)?;
    c.run(0x3380, 1)?;
    c.call(0x5274, 0x3383)?;
    c.run(0x3383, 2)?;
    let s = c.pop();
    c.set_reg(SI, s);
    c.ret()
}

/// `ui_comm`: the sietch to call: of the sietches from 100h (below 20h,
/// without bits 82h of byte 0Ah, west of BX, from 64h; `loc_01e24`
/// clear) whose state (`select_location`) changed ([60h] - [63h]) and
/// whose troop leader BP ([0E4h], or [0DEh] past 1Eh of [0E2h] with [0DCh]
/// below 1Eh; not 11Ch) has troops but none counted by `loc_05098`'s
/// DX, the westmost. DI it ([0D816h], 0 for none, flags), BP [0D818h].
pub fn ui_comm(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2017)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_reg(DI, 0x100);
    c.set_word(0xd816, 0);
    c.set_reg(BX, 0x64);
    let mut di = 0x100u16;
    loop {
        let bx = c.reg(BX);
        let b8 = c.byte(di.wrapping_add(8));
        let f = c.byte(di.wrapping_add(0x0a));
        let x = c.word(di.wrapping_add(4));
        'one: {
            if b8 >= 0x20 {
                c.run(0x2023, 2)?;
                c.cmp(B, b8 as u16, 0x20);
                break 'one;
            }
            if f & 0x82 != 0 {
                c.run(0x2023, 4)?;
                c.cmp(B, b8 as u16, 0x20);
                c.test(B, f as u16, 0x82);
                break 'one;
            }
            if (bx as i16) <= x as i16 {
                c.run(0x2023, 6)?;
                c.cmp(B, b8 as u16, 0x20);
                c.test(B, f as u16, 0x82);
                c.cmp(W, bx, x);
                break 'one;
            }
            c.run(0x2023, 7)?;
            c.cmp(B, b8 as u16, 0x20);
            c.test(B, f as u16, 0x82);
            c.cmp(W, bx, x);
            c.call(0x1e24, 0x2037)?;
            if c.flag(CF) {
                c.run(0x2037, 1)?;
                break 'one;
            }
            c.run(0x2037, 3)?;
            c.push(bx);
            c.call(0x331e, 0x203d)?;
            let (a, b) = (c.byte(0x60), c.byte(0x63));
            if a == b {
                c.run(0x203d, 4)?;
                let v = c.pop();
                c.set_reg(BX, v);
                c.set_reg8(r8::AL, a);
                let d = c.sub(B, a as u16, b as u16);
                c.set_reg8(r8::AL, d as u8);
                break 'one;
            }
            let (e4, e2, dc, de) = (c.word(0xe4), c.word(0xe2), c.word(0xdc), c.word(0xde));
            let skip = e2 >= 0x1e && dc >= 0x1e;
            let bp = if e2 >= 0x1e { de } else { e4 };
            let n = 4 + 3 + if e2 >= 0x1e { 2 + (!skip) as u32 } else { 0 };
            if skip {
                c.run(0x203d, n)?;
            } else if bp == 0x11c {
                c.run(0x203d, n + 2)?;
            } else {
                c.run(0x203d, n + 2 + 3)?;
            }
            let v = c.pop();
            c.set_reg(BX, v);
            c.set_reg8(r8::AL, a);
            let d = c.sub(B, a as u16, b as u16);
            c.set_reg8(r8::AL, d as u8);
            c.set_reg(BP, e4);
            c.cmp(W, e2, 0x1e);
            if e2 >= 0x1e {
                c.cmp(W, dc, 0x1e);
                if skip {
                    break 'one;
                }
                c.set_reg(BP, de);
            }
            c.cmp(W, bp, 0x11c);
            if bp == 0x11c {
                break 'one;
            }
            c.push(di);
            c.set_reg(DI, bp);
            c.call(0x5098, 0x2069)?;
            let (cx, dx, d2) = (c.reg(CX), c.reg(DX), c.reg(DI));
            let take = cx != 0 && dx == 0;
            c.run(0x2069, 3 + if cx == 0 { 0 } else { 2 } + if take { 3 } else { 0 })?;
            c.set_reg(BP, d2);
            let d = c.pop();
            c.set_reg(DI, d);
            if cx == 0 {
                break 'one;
            }
            c.or(W, dx, dx);
            if take {
                let x = c.word(d.wrapping_add(4));
                c.set_reg(BX, x);
                c.set_word(0xd816, d);
                c.set_word(0xd818, d2);
            }
        }
        di = c.reg(DI);
        let nd = di.wrapping_add(0x1c);
        let nb = c.byte(nd);
        let more = nb < 8;
        c.run(0x207d, 3 + if more { 0 } else { 4 })?;
        let d = c.add(W, di, 0x1c);
        c.set_reg(DI, d);
        c.cmp(B, nb as u16, 8);
        di = d;
        if !more {
            break;
        }
    }
    let bp = c.word(0xd818);
    c.set_reg(BP, bp);
    let di = c.word(0xd816);
    c.set_reg(DI, di);
    c.or(W, di, di);
    c.ret()
}

/// `sal_read_position_markers`: the 17h markers at [47F6h] cleared (FFh),
/// then filled: from the CS list [4778h] while [4774h] runs, else (with
/// a count at SI) `loc_03df4` for each person present ([12h] xor [10h],
/// AX the bit) and the extra ones of [476Ah] (10h on); [47F6h] then
/// moves on by the byte at SI - 1.
pub fn sal_read_position_markers(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3d83)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.set_reg(AX, 0xffff);
    c.set_reg(CX, 0x17);
    let di = c.word(0x47f6);
    c.set_reg(DI, di);
    c.rep(0xaa);
    let k = c.byte(0x4774);
    let list = c.word(0x4778);
    let di = c.word(0x47f6);
    if k != 0 && list != 0 {
        c.run(0x3d91, 3 + 3 + 5)?;
        c.set_reg(DI, di);
        c.cmp(B, k as u16, 0);
        c.set_reg(AX, list);
        c.or(W, list, list);
        let si = c.reg(SI);
        c.push(si);
        c.set_reg(SI, list);
        let n = c.lods(B, pc::cpu::CS);
        c.set_reg8(r8::CL, n as u8);
        c.m.cpu.string_op(&mut c.m.hw, 0xa4, Some(pc::cpu::CS), 0xf3);
        c.run(0x3dad, 2)?;
        let s = c.pop();
        c.set_reg(SI, s);
    } else {
        let si = c.reg(SI);
        let cl = c.byte(si);
        let (w12, w10) = (c.word(0x12), c.word(0x10));
        let pre = if k == 0 { 3 } else { 6 };
        c.run(0x3d91, pre + 5 + if cl != 0 { 3 } else { 0 })?;
        c.set_reg(DI, di);
        c.cmp(B, k as u16, 0);
        if k != 0 {
            c.set_reg(AX, list);
            c.or(W, list, list);
        }
        let dx = c.xor(W, w12, w10);
        c.set_reg(DX, dx);
        c.set_reg8(r8::CL, cl);
        c.or(B, cl as u16, cl as u16);
        if cl != 0 {
            let ch = c.byte(0xc5);
            let ch = c.and(B, ch as u16, 0x0f);
            c.set_reg8(r8::CH, ch as u8);
            c.set_reg(AX, 0xffff);
            loop {
                let dx = c.reg(DX);
                let cf = dx & 1 != 0;
                c.run(0x3dc8, 3 + cf as u32)?;
                let ax = c.reg(AX);
                let ax = c.inc(W, ax);
                c.set_reg(AX, ax);
                let d = c.shr1(W, dx, 1);
                c.set_reg(DX, d);
                if cf {
                    c.call(0x3df4, 0x3dd0)?;
                }
                let dx = c.reg(DX);
                c.run(0x3dd0, 2)?;
                c.or(W, dx, dx);
                if dx == 0 {
                    break;
                }
            }
            let dl = c.byte(0x476a);
            let dx = c.reg(DX) & 0xff00 | dl as u16;
            let d = dx.wrapping_sub(1);
            let more = (d as i16) > 0;
            c.run(0x3dd4, 3 + more as u32)?;
            c.set_reg(DX, dx);
            let d = c.dec(W, dx);
            c.set_reg(DX, d);
            if more {
                c.set_reg(AX, 0x0f);
                loop {
                    c.run(0x3dde, 2)?;
                    let ax = c.reg(AX);
                    let ax = c.inc(W, ax);
                    c.set_reg(AX, ax);
                    c.call(0x3df4, 0x3de2)?;
                    let dx = c.reg(DX);
                    let d = dx.wrapping_sub(1);
                    c.run(0x3de2, 2)?;
                    let d2 = c.dec(W, dx);
                    c.set_reg(DX, d2);
                    if d == 0 {
                        break;
                    }
                }
            }
        }
    }
    c.run(0x3de5, 7)?;
    let b = c.lods(B, DS);
    c.xor(B, 0, 0);
    let di = c.word(0x47f6);
    let ax = c.dec(W, b);
    c.set_reg(AX, ax);
    let di = c.add(W, di, ax);
    c.set_reg(DI, di);
    c.set_word(0x47f6, di);
    c.ret()
}

/// `sub_11e6`: of the locations from 138h with a kind past
/// `location_kind_check` and no troops counted by `loc_05098` (DX), the
/// one with the largest y (word 4, above -64h; 100h for none) becomes
/// record 1048h's place (its coordinates, `location_coords`, DL 3) and
/// [0F2h] its name; then event 2B0Ah ([`note_event`]).
pub fn event_2b0a(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x11e6)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_reg(DI, 0x138);
    c.set_reg(BX, 0xff9c);
    loop {
        let (di, bx) = (c.reg(DI), c.reg(BX));
        let ax = c.word(di.wrapping_add(4));
        'one: {
            if (ax as i16) <= bx as i16 {
                c.run(0x1f19, 3)?;
                c.set_reg(AX, ax);
                c.cmp(W, ax, bx);
                break 'one;
            }
            c.run(0x1f19, 4)?;
            c.set_reg(AX, ax);
            c.cmp(W, ax, bx);
            c.call(0x5d36, 0x1f23)?;
            if c.flag(CF) {
                c.run(0x1f23, 1)?;
                break 'one;
            }
            c.run(0x1f23, 2)?;
            c.call(0x5098, 0x1f28)?;
            let dx = c.reg(DX);
            if dx != 0 {
                c.run(0x1f28, 2)?;
                c.or(W, dx, dx);
                break 'one;
            }
            let (di, bx) = (c.reg(DI), c.reg(BX));
            let ax = c.word(di.wrapping_add(4));
            let more = (ax as i16) > bx as i16;
            c.run(0x1f28, 5 + if more { 2 } else { 0 })?;
            c.or(W, 0, 0);
            c.set_reg(AX, ax);
            c.cmp(W, ax, bx);
            if more {
                c.set_reg(BX, ax);
                c.set_reg(SI, di);
            }
        }
        let di = c.reg(DI);
        let nd = di.wrapping_add(0x1c);
        let nb = c.byte(nd);
        let bx = c.reg(BX);
        let more = nb < 8;
        c.run(0x1f37, 3 + if more { 0 } else { 2 + (bx == 0xff9c) as u32 + 2 })?;
        let d = c.add(W, di, 0x1c);
        c.set_reg(DI, d);
        c.cmp(B, nb as u16, 8);
        if more {
            continue;
        }
        c.cmp(W, bx, 0xff9c);
        if bx == 0xff9c {
            c.set_reg(SI, 0x100);
        }
        let si = c.reg(SI);
        c.set_reg(DI, si);
        break;
    }
    c.call(0x40ae, 0x1f4c)?;
    c.run(0x1f4c, 8)?;
    let (dx, bx, di) = (c.reg(DX), c.reg(BX), c.reg(DI));
    let dx = dx & 0xff00 | 3;
    c.set_reg(DX, dx);
    c.set_word(0x1048, dx);
    c.set_word(0x104a, bx);
    let ax = (c.byte(di) as u16) << 8 | c.byte(di.wrapping_add(1)) as u16;
    c.set_word(0xf2, ax);
    c.set_reg(AX, 0x2b0a);
    note_event_at(&mut c)
}

/// `loc_00a23`: one step of the scene's video: at night to the back
/// buffer and [`hnm_do_frame`]; else on the front buffer
/// `hnm_do_frame_and_check_if_frame_advanced`, and on a new frame
/// `sub_c4dd` and the mouse ([`draw_mouse`]); CF clear when none.
pub fn video_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0a23)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x227d);
    if k != 0 {
        c.run(start, 3)?;
        c.cmp(B, k as u16, 0);
        c.call(0xc08e, 0x0a2d)?;
        c.run(0x0a2d, 1)?;
        return super::frame::hnm_do_frame(c.m, 0xca60);
    }
    c.run(start, 3)?;
    c.cmp(B, 0, 0);
    c.call(0xc07c, 0x0a33)?;
    c.run(0x0a33, 1)?;
    c.call(0xc9f4, 0x0a36)?;
    if c.flag(pc::cpu::ZF) {
        c.run(0x0a36, 3)?;
        c.set_flag(CF, false);
        return c.ret();
    }
    c.run(0x0a36, 2)?;
    c.call(0xc4dd, 0x0a3b)?;
    c.run(0x0a3b, 1)?;
    super::mouse::draw_mouse(c.m, 0xdbec)
}

/// `loc_00c3b`: one step of the creature SI (a bird or a worm sign):
/// when its timer (byte 1) runs out, a new random move (its byte 7 by
/// the seed, the wait 0..7 from `rand_masked`, flags in word 2); then its
/// sprite: for kind bit 10h clear from the table 15A2h, else turning by
/// 0Ah (at random, within 0..3Fh, flipping at the ends) with `loc_05198`;
/// drawn by `loc_0c60b` from the place table 1592h (by its bits 0Ch).
pub fn creature_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0c3b)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let t = c.byte(si.wrapping_add(1));
    let t1 = t.wrapping_sub(1);
    if t1 & 0x80 != 0 {
        let seed = c.word(0);
        let a = seed & 3 == 0;
        let b = a && seed & 0xc == 0;
        c.run(start, 2 + 2 + if a { 3 + b as u32 } else { 0 } + 1)?;
        let v = pure::dec(B, t as u16);
        c.set_byte(si.wrapping_add(1), v as u8);
        if a {
            c.set_byte(si.wrapping_add(7), 0x0b);
            if b {
                c.set_byte(si.wrapping_add(7), 0x11);
            }
        }
        c.call(0xe3cc, 0x0c5b)?;
        let k = c.byte(0x473a);
        c.run(0x0c5b, 2 + (k != 0) as u32 + 3)?;
        let mut ax = c.reg(AX);
        if k != 0 {
            let al = pure::and(B, ax & 0xff, 0xef);
            ax = ax & 0xff00 | al;
            c.set_reg(AX, ax);
        }
        c.set_reg(CX, ax);
        c.set_reg(BX, 7);
        c.call(0xe3b7, 0x0c6c)?;
        let al = c.reg8(r8::AL);
        let hi = al >= 4;
        c.run(0x0c6c, 3 + hi as u32 + 1)?;
        let si = c.reg(SI);
        c.set_byte(si.wrapping_add(1), al);
        let mut cx = c.reg(CX);
        if hi {
            let ch = pure::or(B, cx >> 8, 0x40);
            cx = ch << 8 | cx & 0xff;
            c.set_reg(CX, cx);
        }
        c.set_word(si.wrapping_add(2), cx);
    } else {
        c.run(start, 2)?;
        let v = pure::dec(B, t as u16);
        c.set_byte(si.wrapping_add(1), v as u8);
    }
    let si = c.reg(SI);
    let al = c.byte(si.wrapping_add(2));
    let ax = c.reg(AX) & 0xff00 | al as u16;
    let b3 = c.byte(si.wrapping_add(3));
    if al & 0x10 == 0 {
        c.run(0x0c79, 12)?;
        c.set_byte(si, 8);
        c.set_reg(AX, ax);
        c.set_reg(DI, ax);
        let bx = c.reg(BX) & 0xff00 | b3 as u16;
        let a = pure::and(W, ax, 0x10);
        let bx = pure::and(W, bx, 6);
        let s = c.word(bx.wrapping_add(0x15a2));
        c.set_reg(SI, s);
        let bx = pure::add(W, bx, bx);
        c.set_reg(BX, bx);
        let a = pure::add(W, a, bx);
        let l = pure::add(B, a & 0xff, 4);
        c.set_reg(AX, a & 0xff00 | l);
    } else {
        let mut n = 6 + 5;
        let ah0 = b3 & 0xc0;
        let turn = ah0 & 0x40 != 0;
        let seed = c.word(0);
        let mut roll = false;
        if turn {
            n += 2;
            roll = seed & 0x8000 != 0;
            if roll {
                let neg = ah0 & 0x80 != 0;
                n += 3 + neg as u32 + 2;
                let cl: u8 = if neg { 0xf6 } else { 0x0a };
                let v = (b3 & 0x3f).wrapping_add(cl);
                let mut a = v;
                if v & 0x80 != 0 {
                    n += 2;
                    a = 0;
                }
                n += 2 + if a >= 0x40 { 2 } else { 0 } + 2;
            }
        }
        n += 2;
        c.run(0x0c79, n)?;
        c.set_byte(si, 8);
        c.set_reg(DI, ax);
        let bx = c.reg(BX) & 0xff00 | b3 as u16;
        c.set_reg(BX, bx);
        let mut ah = pure::and(B, b3 as u16, 0xc0);
        let mut al = pure::and(B, b3 as u16, 0x3f);
        if turn {
            let s = pure::rol1(W, seed);
            c.set_word(0, s);
            if roll {
                let mut cl = 0x0au16;
                if ah & 0x80 != 0 {
                    cl = pure::neg(B, cl);
                }
                c.set_reg8(r8::CL, cl as u8);
                al = pure::add(B, al, cl);
                if al & 0x80 != 0 {
                    ah = pure::xor(B, ah, 0x80);
                    al = pure::xor(B, 0, 0);
                }
                if al >= 0x40 {
                    al = 0x3f;
                    ah = pure::xor(B, ah, 0x80);
                }
                let v = pure::or(B, ah, al);
                ah = v;
                c.set_byte(si.wrapping_add(3), v as u8);
            }
        }
        let a = pure::add(B, al, 0xe0);
        c.set_reg(AX, ah << 8 | a);
        c.call(0x5198, 0x0ccf)?;
        c.run(0x0ccf, 3)?;
        let (bx, dx) = (c.reg(BX), c.reg(DX));
        let dx = (bx & 0xff) << 8 | dx & 0xff;
        c.set_reg(DX, dx);
        c.set_reg(SI, dx);
        c.set_reg(AX, 0x14);
    }
    c.run(0x0cd6, 4)?;
    let di = c.reg(DI);
    let di = pure::and(W, di, 0x0c);
    c.set_reg(DI, di);
    let (bx, dx) = (c.word(di.wrapping_add(0x1594)), c.word(di.wrapping_add(0x1592)));
    c.set_reg(BX, bx);
    c.set_reg(DX, dx);
    c.call(0xc60b, 0x0ce4)?;
    c.run(0x0ce4, 2)?;
    let di = c.reg(DI);
    c.set_word(di.wrapping_add(0x0d), 0);
    c.ret()
}

/// `loc_031f6`: the troop SI described in the condition variables
/// (registers kept): its location (2Ch), id, activity, the age phrase
/// (`record_age_phrase`) and rate (`troop_rate_by_age`, [48h]), words
/// 10h, 12h, motivation (`troop_motivation`), its three skills with
/// their phrases (`al_sixteenth_plus_d1`), words 0Ch, 0Eh, the skill of
/// its activity (`loc_0693b`), byte 19h, size, the days since byte 14h,
/// and its location (`select_location`).
pub fn describe_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x31f6)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xe270, 0x31f9)?;
    c.run(0x31f9, 11)?;
    let si = c.reg(SI);
    let di = c.word(si.wrapping_add(4));
    c.set_reg(DI, di);
    c.set_word(0x2c, di);
    let id = c.byte(si);
    c.set_byte(0x2e, id);
    let b3 = c.byte(si.wrapping_add(3));
    c.set_byte(0x30, b3);
    let ax = c.reg(AX) & 0xff00 | b3 as u16;
    let a = pure::and(W, ax, 0x0f);
    c.set_byte(0x2f, a as u8);
    let a = pure::add(W, a, 0x18);
    c.set_reg(AX, a);
    c.set_word(0x11f3, a);
    c.call(0x32c7, 0x321a)?;
    c.run(0x321a, 1)?;
    c.call(0x329d, 0x321d)?;
    c.run(0x321d, 10)?;
    let si = c.reg(SI);
    let ax = c.reg(AX);
    c.set_word(0x48, ax);
    let w = c.word(si.wrapping_add(0x10));
    c.set_word(0x32, w);
    let w = c.word(si.wrapping_add(0x12));
    c.set_word(0x34, w);
    let a = pure::and(W, w, 0x0f);
    c.set_byte(0x31, a as u8);
    let a = pure::add(W, a, 0);
    c.set_reg(AX, a);
    c.set_word(0x11ff, a);
    c.call(0x6efd, 0x323b)?;
    for (at, src, dst, word, ret) in [
        (0x323bu16, 0x16u16, 0x38u16, 0x11f7u16, 0x3247u16),
        (0x3247, 0x17, 0x39, 0x11f9, 0x3253),
        (0x3253, 0x18, 0x3a, 0x11fb, 0x325f),
    ] {
        c.run(at, 4)?;
        let ax = c.reg(AX);
        if at == 0x323b {
            c.set_byte(0x36, ax as u8);
        } else {
            let prev = if at == 0x3247 { 0x11f7 } else { 0x11f9 };
            c.set_word(prev, ax);
        }
        let si = c.reg(SI);
        let b = c.byte(si.wrapping_add(src));
        c.set_reg8(r8::AL, b);
        c.set_byte(dst, b);
        let _ = word;
        c.call(0x3310, ret)?;
    }
    c.run(0x325f, 9)?;
    let ax = c.reg(AX);
    c.set_word(0x11fb, ax);
    let si = c.reg(SI);
    let w = c.word(si.wrapping_add(0x0c));
    c.set_word(0x44, w);
    let w = c.word(si.wrapping_add(0x0e));
    c.set_word(0x46, w);
    let a = pure::add(W, w & 0xff, 0xe8);
    c.set_reg(AX, a);
    c.set_word(0x11f1, a);
    c.call(0x693b, 0x3279)?;
    c.run(0x3279, 8)?;
    let ax = c.reg(AX);
    c.set_reg(BP, ax);
    let si = c.reg(SI);
    let s = c.byte_in(SS, ax.wrapping_add(si).wrapping_add(0x16));
    c.set_byte(0x37, s);
    let b = c.byte(si.wrapping_add(0x19));
    c.set_byte(0x3b, b);
    let z = c.byte(si.wrapping_add(0x1a));
    c.set_byte(0x3c, z);
    c.set_reg(AX, ax & 0xff00 | z as u16);
    c.call(0x1ac5, 0x3290)?;
    c.run(0x3290, 3)?;
    let al = c.reg8(r8::AL);
    let si = c.reg(SI);
    let d = c.byte(si.wrapping_add(0x14));
    let v = pure::sub(B, al as u16, d as u16);
    c.set_reg8(r8::AL, v as u8);
    c.set_byte(0x40, v as u8);
    c.call(0x331e, 0x3299)?;
    c.run(0x3299, 1)?;
    c.call(0xe283, 0x329c)?;
    c.run(0x329c, 1)?;
    c.ret()
}

/// `sub_1027`: bit 80h of [122Eh] cleared, then the panel's buttons
/// (`panel_buttons`).
pub fn buttons_after_122e(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1027)?;
    let mut c = Ctx::new(m);
    super::screens::panel_buttons_at(&mut c, start, 2, |c| {
        let v = c.byte(0x122e);
        let v = c.and(B, v as u16, 0x7f);
        c.set_byte(0x122e, v as u8);
    })
}

/// `sub_10a4`: bit 80h of [1245h] cleared, [1217h] FFFFh, then the panel's
/// buttons.
pub fn buttons_after_1245(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x10a4)?;
    let mut c = Ctx::new(m);
    super::screens::panel_buttons_at(&mut c, start, 3, |c| {
        let v = c.byte(0x1245);
        let v = c.and(B, v as u16, 0x7f);
        c.set_byte(0x1245, v as u8);
        c.set_word(0x1217, 0xffff);
    })
}

/// `loc_01188`: bit 80h of [1259h] cleared, [1211h] FFFFh, then the panel's
/// buttons.
pub fn buttons_after_1259(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1188)?;
    let mut c = Ctx::new(m);
    super::screens::panel_buttons_at(&mut c, start, 3, |c| {
        let v = c.byte(0x1259);
        let v = c.and(B, v as u16, 0x7f);
        c.set_byte(0x1259, v as u8);
        c.set_word(0x1211, 0xffff);
    })
}

/// `loc_02090`: the next spice demand: [118Dh] = the day; [0BCh] =
/// ([0C3h], counted, ·96h + 64h) · (E0h + a random 0..3Fh) / 256, for
/// [0BEh] below 80h scaled by (100h + ~(2·[0BEh]))/256; FFFFh on any
/// overflow. [0CFh] 0, [0BFh] |= 90h, then event 20Bh (30Bh for [0BEh]
/// below 80h) ([`note_event`]).
pub fn next_demand(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2090)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x1ac5, 0x2093)?;
    let ax0 = c.reg(AX);
    let k = c.byte(0xc3);
    let be = c.byte(0xbe);
    let pos = be & 0x80 == 0;
    let tail_n = |out: bool| out as u32 + 7 + pos as u32;
    let p = ((ax0 & 0xff00) | k as u16) as u32 * 0x96;
    let over1 = p >> 16 != 0;
    let (s, carry) = (p as u16).overflowing_add(0x64);
    let head = |c: &mut Ctx| {
        c.set_word(0x118d, ax0);
        c.set_reg8(r8::AL, k);
        let v = c.inc(B, k as u16);
        c.set_byte(0xc3, v as u8);
        c.set_reg(BX, 0x96);
        c.mul16(0x96);
        let dx = c.reg(DX);
        c.or(W, dx, dx);
    };
    if over1 {
        c.run(0x2093, 8 + tail_n(true))?;
        head(&mut c);
        return demand_tail(&mut c, true);
    }
    if carry {
        c.run(0x2093, 10 + tail_n(true))?;
        head(&mut c);
        let ax = c.reg(AX);
        let a = c.add(W, ax, 0x64);
        c.set_reg(AX, a);
        return demand_tail(&mut c, true);
    }
    c.run(0x2093, 13)?;
    head(&mut c);
    let ax = c.reg(AX);
    let a = c.add(W, ax, 0x64);
    c.set_reg(AX, a);
    c.set_reg(CX, s);
    c.set_reg(BX, 0x3f);
    c.call(0xe3b7, 0x20ef)?;
    let r = c.reg(AX).wrapping_add(0xe0);
    let q = r as u32 * s as u32;
    let (dxq, axq) = ((q >> 16) as u16, q as u16);
    let d = (dxq & 0xff) << 8 | axq >> 8;
    let mut out;
    let n;
    if dxq >> 8 != 0 {
        out = true;
        n = 4;
    } else if !pos {
        out = false;
        n = 9;
    } else {
        let a2 = 0x100u16 | (!(be << 1)) as u16;
        let m2 = a2 as u32 * d as u32;
        let dx2 = ((m2 >> 16) as u16).swap_bytes();
        let ah2 = dx2 & 0xff;
        out = ah2 != 0;
        n = 16;
    }
    c.run(0x20ef, n + tail_n(out))?;
    let ax = c.reg(AX);
    let a = c.add(W, ax, 0xe0);
    c.set_reg(AX, a);
    c.mul16(s);
    let dx = c.reg(DX);
    c.or(B, dx >> 8, dx >> 8);
    if dx >> 8 == 0 {
        let ax = c.reg(AX);
        let d = (dx & 0xff) << 8 | ax >> 8;
        c.set_reg(DX, d);
        let sh = c.shl1(B, be as u16, 1);
        c.set_reg(AX, ax & 0xff00 | sh);
        if pos {
            let al = !(sh as u8);
            c.set_reg(AX, 0x100 | al as u16);
            c.mul16(d);
            let (dxr, axr) = (c.reg(DX), c.reg(AX));
            let dxs = dxr.swap_bytes();
            let new_dx = dxs & 0xff00 | axr >> 8;
            let new_ax = (dxs & 0xff) << 8 | axr & 0xff;
            c.set_reg(DX, new_dx);
            c.set_reg(AX, new_ax);
            c.or(B, new_ax >> 8, new_ax >> 8);
            out = new_ax >> 8 != 0;
        }
    }
    demand_tail(&mut c, out)
}

fn demand_tail(c: &mut Ctx, out: bool) -> Result<(), Bail> {
    if out {
        c.set_reg(DX, 0xffff);
    }
    let dx = c.reg(DX);
    c.set_word(0xbc, dx);
    c.set_byte(0xcf, 0);
    let v = c.byte(0xbf);
    let v = c.or(B, v as u16, 0x90);
    c.set_byte(0xbf, v as u8);
    c.set_reg(AX, 0x20b);
    let be = c.byte(0xbe);
    c.cmp(B, be as u16, 0);
    if be & 0x80 == 0 {
        let ah = c.inc(B, 2);
        c.set_reg(AX, ah << 8 | 0x0b);
    }
    note_event_at(c)
}

/// `loc_02170`: the 9 people records from FD8h updated (registers
/// kept), those without bit 40h of byte 0Fh: record 1008h at its home
/// (180h) gets state 4; one away from a place (byte 2 not 80h) moves on
/// (`loc_0221d`); at a place in state 1 (below 21h, not the current [9],
/// at a location without bit 2 of byte 0Ah) the state counted up, and at
/// place 1 its next state from 144Dh by byte 0Eh (6 becomes 0Ah before
/// phase 24h; bit 4 of byte 0Fh cleared); others from 1028h at place 1
/// get `loc_021fa`'s.
pub fn update_people(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2170)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xe270, 0x2173)?;
    c.run(0x2173, 2)?;
    c.set_reg(SI, 0xfd8);
    c.set_reg(CX, 9);
    for i in 0..9u16 {
        let si = 0xfd8 + 0x10 * i;
        let f = c.byte(si.wrapping_add(0x0f));
        'one: {
            if f & 0x40 != 0 {
                c.run(0x2179, 2)?;
                c.test(B, f as u16, 0x40);
                break 'one;
            }
            let mut dx = c.word(si);
            let bx = c.word(si.wrapping_add(2));
            let home = si == 0x1008 && bx == 0x180;
            let n0 = 2 + 4 + if si == 0x1008 { 2 + if home { 2 } else { 0 } } else { 0 } + 2;
            if home {
                dx = dx & 0xff00 | 4;
            }
            let ops0 = |c: &mut Ctx| {
                c.test(B, f as u16, 0x40);
                c.set_reg(DX, dx);
                c.set_reg(BX, bx);
                c.cmp(W, si, 0x1008);
                if si == 0x1008 {
                    c.cmp(W, bx, 0x180);
                    if home {
                        c.set_word(si, dx);
                    }
                }
                c.cmp(B, bx & 0xff, 0x80);
            };
            if bx & 0xff != 0x80 {
                c.run(0x2179, n0 + 1)?;
                ops0(&mut c);
                c.call(0x221d, 0x21f1)?;
                break 'one;
            }
            let dl = dx as u8;
            let (dh, bh) = ((dx >> 8) as u8, (bx >> 8) as u8);
            if dl != 1 {
                let fewer = si < 0x1028;
                if fewer || bh != 1 {
                    c.run(0x2179, n0 + 2 + 2 + if fewer { 0 } else { 2 })?;
                    ops0(&mut c);
                    c.cmp(B, dl as u16, 1);
                    c.cmp(W, si, 0x1028);
                    if !fewer {
                        c.cmp(B, bh as u16, 1);
                    }
                    break 'one;
                }
                c.run(0x2179, n0 + 2 + 5)?;
                ops0(&mut c);
                c.cmp(B, dl as u16, 1);
                c.cmp(W, si, 0x1028);
                c.cmp(B, 1, 1);
                c.call(0x21fa, 0x21ea)?;
                c.run(0x21ea, 2)?;
                let al = c.reg8(r8::AL);
                c.set_byte(si, al);
                break 'one;
            }
            let cur = c.byte(9);
            let loc = (0x1cu16 * bh as u16).wrapping_add(0xe4);
            let lf = c.byte(loc.wrapping_add(0x0a));
            let mut n = n0 + 2 + 2;
            let stage = if dh >= 0x21 {
                0
            } else if bh == cur {
                n += 2;
                1
            } else if lf & 2 != 0 {
                n += 2 + 6;
                2
            } else if bh != 1 {
                n += 2 + 6 + 3;
                3
            } else {
                n += 2 + 6 + 3 + 7;
                4
            };
            let e = c.byte(si.wrapping_add(0x0e));
            let nx = c.byte(0x144du16.wrapping_add(e as u16));
            let ph = c.byte(0x2a);
            if stage == 4 && nx == 6 {
                n += 2 + if ph < 0x24 { 2 } else { 0 };
            }
            c.run(0x2179, n)?;
            ops0(&mut c);
            c.cmp(B, 1, 1);
            c.cmp(B, dh as u16, 0x21);
            if stage == 0 {
                break 'one;
            }
            c.cmp(B, bh as u16, cur as u16);
            if stage == 1 {
                break 'one;
            }
            c.set_reg8(r8::AL, 0x1c);
            c.mul8(bh);
            let a = c.reg(AX);
            let a = c.add(W, a, 0xe4);
            c.set_reg(AX, a);
            c.set_reg(DI, a);
            c.test(B, lf as u16, 2);
            if stage == 2 {
                break 'one;
            }
            let v = c.byte(si);
            let v = c.inc(B, v as u16);
            c.set_byte(si, v as u8);
            c.cmp(B, bh as u16, 1);
            if stage == 3 {
                break 'one;
            }
            c.set_reg(BX, 0x144d);
            c.set_reg8(r8::AL, nx);
            c.set_byte(si, nx);
            let g = c.byte(si.wrapping_add(0x0f));
            let g = c.and(B, g as u16, 0xfb);
            c.set_byte(si.wrapping_add(0x0f), g as u8);
            c.cmp(B, nx as u16, 6);
            if nx == 6 {
                c.cmp(B, ph as u16, 0x24);
                if ph < 0x24 {
                    c.set_byte(si, 0x0a);
                }
            }
        }
        let last = i == 8;
        c.run(0x21f1, 2 + if last { 1 } else { 0 })?;
        let s = c.add(W, si, 0x10);
        c.set_reg(SI, s);
        c.set_reg(CX, 8 - i);
    }
    c.call(0xe283, 0x21f9)?;
    c.run(0x21f9, 1)?;
    c.ret()
}

/// `loc_045de`: on the screen (the front buffer kept), the message line
/// redrawn (`loc_0dbb2`, `loc_0d075`; DX 55h, BX 22h or 26h with
/// [473Eh]): no message (DI 0) `start_message_57` and the colour CX into
/// [0DBE4h]; a place (DI below FFF0h) its type and name; else the
/// string A4h and for the codes FFF0h..FFF7h the phrase DAh + code; then
/// padded with spaces (the putter [2518h]) to EDh, and the mouse drawn.
pub fn redraw_message_line(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x45de)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let fb = c.word(0xdbda);
    c.push(fb);
    c.call(0xc08e, 0x45e5)?;
    c.run(0x45e5, 1)?;
    c.call(0xdbb2, 0x45e8)?;
    c.run(0x45e8, 1)?;
    c.call(0xd075, 0x45eb)?;
    let f = c.byte(0x473e);
    let di = c.reg(DI);
    c.run(0x45eb, 5 + if f != 0 { 2 } else { 0 } + 3)?;
    c.set_reg(DX, 0x55);
    c.set_reg(BX, 0x22);
    c.set_reg(CX, 0xf5fe);
    c.cmp(B, f as u16, 0);
    if f != 0 {
        let b = c.add(W, 0x22, 4);
        c.set_reg(BX, b);
        c.set_reg(CX, 0x20fe);
    }
    c.or(W, di, di);
    if di == 0 {
        c.call(0x4658, 0x462d)?;
        c.run(0x462d, 2)?;
        let cx = c.reg(CX);
        c.set_word(0xdbe4, cx);
        c.call(0xd04e, 0x4634)?;
        c.run(0x4634, 1)?;
    } else {
        c.call(0x469b, 0x4607)?;
        let di = c.reg(DI);
        c.run(0x4607, 4)?;
        c.cmp(W, di, 0xfff0);
        if di < 0xfff0 {
            let cx = c.reg(CX);
            c.push(cx);
            c.call(0x629d, 0x463a)?;
            c.run(0x463a, 1)?;
            c.call(0xd05f, 0x463d)?;
            c.run(0x463d, 2)?;
            let x = c.pop();
            c.set_reg(CX, x);
            c.call(0x62a6, 0x4641)?;
        } else {
            c.set_reg(AX, 0xa4);
            c.call(0xd194, 0x4612)?;
            let di = c.reg(DI);
            let d = di.wrapping_sub(0xfff0);
            c.run(0x4612, if d >= 8 { 3 } else { 5 })?;
            let d = c.sub(W, di, 0xfff0);
            c.set_reg(DI, d);
            c.cmp(W, d, 8);
            if d < 8 {
                c.set_reg8(r8::AL, 0x20);
                let to = c.word(0x2518);
                c.call(to, 0x4620)?;
                c.run(0x4620, 3)?;
                let di = c.reg(DI);
                let a = c.add(W, di, 0xda);
                c.set_reg(AX, a);
                c.call(0xd19b, 0x4628)?;
                c.run(0x4628, 1)?;
            }
        }
    }
    let mut i = 0;
    loop {
        let v = c.word(0xd82c);
        i += 1;
        if i > 1000 {
            c.m.cpu.ip = 0x4641;
            return Err(Bail);
        }
        c.run(0x4641, 4)?;
        c.cmp(W, v, 0xed);
        if v > 0xed {
            break;
        }
        c.set_reg8(r8::AL, 0x20);
        let to = c.word(0x2518);
        c.call(to, 0x464f)?;
        c.run(0x464f, 1)?;
    }
    let fb = c.pop();
    c.set_word(0xdbda, fb);
    super::mouse::draw_mouse(c.m, 0xdbec)
}

/// `sub_3520`: once per scene ([47A7h]), the troop SI's numbered
/// sentence (+0Eh, `say_numbered`; when it is not, the event for the
/// speaker [47C4h] and the place [114Eh] found and told, `loc_02b00`)
/// and the events of that pair dropped; [0E7h] 0. Then by the stage
/// [23h]: 3 and 4 install the panel 1F92h / 1F9Eh (4: its bit 40h
/// cleared, the buttons redrawn) with the handler 97CFh and the counters
/// [1BEAh], [1BF8h] 0; before stage 64h (with no [4774h]) the speaker
/// started and the panel faded.
pub fn scene_sentence(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3520)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x47a7);
    if k != 0 {
        c.run(start, 3)?;
        c.cmp(B, k as u16, 0);
        return c.ret();
    }
    c.run(start, 6)?;
    c.cmp(B, 0, 0);
    let si = c.reg(SI);
    let al = c.byte(si.wrapping_add(0x0e));
    c.xor(B, 0, 0);
    c.set_reg(AX, al as u16);
    c.push(si);
    c.call(0x96f1, 0x3530)?;
    if c.flag(CF) {
        c.run(0x3530, 5)?;
        let s = c.pop();
        c.set_reg(SI, s);
        let (ax, di) = (c.word(0x47c4), c.word(0x114e));
        c.set_reg(AX, ax);
        c.set_reg(DI, di);
        c.call(0x2aaf, 0x353d)?;
        if !c.flag(CF) {
            c.run(0x353d, 2)?;
            return c.ret();
        }
        c.run(0x353d, 2)?;
        c.call(0x2b00, 0x3542)?;
    } else {
        c.run(0x3530, 2)?;
        let s = c.pop();
        c.set_reg(SI, s);
    }
    c.run(0x3542, 3)?;
    let (ax, di) = (c.word(0x47c4), c.word(0x114e));
    c.set_reg(AX, ax);
    c.set_reg(DI, di);
    c.call(0x2a51, 0x354c)?;
    let st = c.byte(0x23);
    let ops = |c: &mut Ctx| {
        c.set_byte(0xe7, 0);
        let v = c.byte(0x47a7);
        let v = c.inc(B, v as u16);
        c.set_byte(0x47a7, v as u8);
        c.cmp(B, st as u16, 3);
    };
    if st == 3 {
        c.run(0x354c, 7)?;
        ops(&mut c);
        c.set_reg(BP, 0x1f92);
        c.set_reg(BX, 0x97cf);
        c.call(0xd323, 0x3565)?;
        c.run(0x3565, 3)?;
        c.set_word(0x1bea, 0);
        c.set_word(0x1bf8, 0);
        return c.ret();
    }
    if st == 4 {
        c.run(0x354c, 10)?;
        ops(&mut c);
        c.cmp(B, 4, 4);
        c.set_reg(BP, 0x1f9e);
        let v = c.byte_in(SS, 0x1fa9);
        let v = c.and(B, v as u16, 0xbf);
        c.m.cpu.write8(&mut c.m.hw, SS, 0x1fa9, v as u8);
        c.set_reg(BX, 0x97cf);
        c.call(0xd323, 0x3586)?;
        c.run(0x3586, 3)?;
        c.set_word(0x1bea, 0);
        c.set_word(0x1bf8, 0);
        return super::screens::panel_buttons(c.m, 0x2ffb);
    }
    let w = c.byte(0x4774);
    if w != 0 {
        c.run(0x354c, 9)?;
        ops(&mut c);
        c.cmp(B, st as u16, 4);
        c.cmp(B, w as u16, 0);
        return c.ret();
    }
    if st >= 0x64 {
        c.run(0x354c, 11)?;
        ops(&mut c);
        c.cmp(B, st as u16, 4);
        c.cmp(B, 0, 0);
        c.cmp(B, st as u16, 0x64);
        return c.ret();
    }
    c.run(0x354c, 12)?;
    ops(&mut c);
    c.cmp(B, st as u16, 4);
    c.cmp(B, 0, 0);
    c.cmp(B, st as u16, 0x64);
    let ax = c.word(0x47c4);
    c.set_reg(AX, ax);
    c.call(0x93df, 0x35a9)?;
    c.run(0x35a9, 1)?;
    c.call(0xd280, 0x35ac)?;
    c.run(0x35ac, 1)?;
    c.ret()
}

/// `sub_450e`: in a map mode, a click at (DX, BX): the message subject
/// updated (`message_subject`); a place picked (DI, not the current one
/// [114Eh] in mode 0) is announced (`place_phrase`, its voice), its name
/// blinked 9 times on the message line, and the troop sent there
/// ([4732h] 80h, [`arrive_from_4703`]).
pub fn map_pick_place(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x450e)?;
    let mut c = Ctx::new(m);
    let mode = c.byte(0x11c9);
    if mode & 0x0f == 0 {
        c.run(start, 3)?;
        c.test(B, mode as u16, 0x0f);
        return c.ret();
    }
    c.run(start, 5)?;
    c.test(B, mode as u16, 0x0f);
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.call(0x4586, 0x451a)?;
    let di = c.word(0x46fc);
    let mode = c.byte(0x11c9);
    let cur = c.word(0x114e);
    let ops = |c: &mut Ctx| {
        let d = c.pop();
        c.set_reg(DX, d);
        let b = c.pop();
        c.set_reg(BX, b);
        c.set_reg(DI, di);
        c.or(W, di, di);
    };
    if di == 0 {
        c.run(0x451a, 6)?;
        ops(&mut c);
        return c.ret();
    }
    if di & 0x8000 == 0 {
        if mode & 3 == 0 && di == cur {
            c.run(0x451a, 11)?;
            ops(&mut c);
            c.test(B, mode as u16, 3);
            c.cmp(W, di, cur);
            return c.ret();
        }
        c.run(0x451a, if mode & 3 != 0 { 8 } else { 10 } + 4)?;
        ops(&mut c);
        c.test(B, mode as u16, 3);
        if mode & 3 == 0 {
            c.cmp(W, di, cur);
        }
    } else {
        c.run(0x451a, 6 + 4)?;
        ops(&mut c);
    }
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.push(di);
    c.call(0x456c, 0x453a)?;
    c.run(0x453a, 1)?;
    c.call(0xab45, 0x453d)?;
    c.run(0x453d, 2)?;
    let di = c.pop();
    c.set_reg(DI, di);
    c.set_reg(CX, 9);
    let mut k = 0;
    loop {
        k += 1;
        if k > 9 {
            c.m.cpu.ip = 0x4541;
            return Err(Bail);
        }
        c.run(0x4541, 4)?;
        let (cx, di) = (c.reg(CX), c.reg(DI));
        c.push(cx);
        c.push(di);
        c.set_reg(AX, 0x14);
        c.call(0xe3a0, 0x4549)?;
        c.run(0x4549, 3)?;
        let di = c.reg(DI);
        c.push(di);
        c.xor(W, 0, 0);
        c.set_reg(DI, 0);
        c.call(0x45de, 0x454f)?;
        c.run(0x454f, 3)?;
        let d = c.pop();
        c.set_reg(DI, d);
        c.set_reg(AX, 0x0a);
        c.call(0xe3a0, 0x4556)?;
        c.run(0x4556, 1)?;
        c.call(0x45de, 0x4559)?;
        c.run(0x4559, 3)?;
        let d = c.pop();
        c.set_reg(DI, d);
        let cx = c.pop().wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.run(0x455d, 2)?;
    let di = c.reg(DI);
    c.push(di);
    c.call(0xaba9, 0x4561)?;
    c.run(0x4561, 5)?;
    let d = c.pop();
    c.set_reg(DI, d);
    let d = c.pop();
    c.set_reg(DX, d);
    let b = c.pop();
    c.set_reg(BX, b);
    c.set_byte(0x4732, 0x80);
    arrive_from_4703(&mut c)
}

/// `loc_04703`: arrival at the place picked: `loc_04944`, the sky,
/// the map mode's bits 2..3 set from its low ones, the music, out of a
/// dialogue (`loc_00b21`), the screen left, `stop_task_4d00`; from a map
/// mode (low 2 bits of the old mode) `loc_02eb2` and the palette; else
/// [472Bh] 0, `loc_041c5` (the head down in mode 1), the game rect to
/// the screen, the place's records, `loc_04795` with the heading [11C7h]
/// kept, [8] FFh, `loc_04b3b`, [114Eh] and [4729h] 0, `loc_02dbf`
/// outside the map views, `loc_04ab8`; then in mode 1 the troop's
/// counter [[1150h] + 15h] down and the voice stopped, else `loc_02eb2`
/// and the palette.
pub(super) fn arrive_from_4703(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x4703, 1)?;
    c.call(0x4944, 0x4706)?;
    c.run(0x4706, 1)?;
    c.call(0x38e1, 0x4709)?;
    c.run(0x4709, 6)?;
    let m0 = c.byte(0x11c9);
    c.set_reg8(r8::AL, m0);
    let ax = c.reg(AX);
    c.push(ax);
    let a = c.shr1(B, m0 as u16, 2);
    c.set_reg8(r8::AL, a as u8);
    let v = c.or(B, m0 as u16, a);
    c.set_byte(0x11c9, v as u8);
    c.call(0xad5e, 0x4718)?;
    let t = c.byte(0x2b);
    c.run(0x4718, if t != 0 { 4 } else { 3 })?;
    c.cmp(B, t as u16, 0);
    if t != 0 {
        c.set_byte(0x2b, 0);
        c.call(0x0b21, 0x4727)?;
        c.run(0x4727, 1)?;
    }
    c.call(0xd2ea, 0x472a)?;
    c.run(0x472a, 1)?;
    c.call(0x4d00, 0x472d)?;
    let ax = c.word_in(SS, c.reg(SP));
    if ax & 3 != 0 {
        c.run(0x472d, 3)?;
        let a = c.pop();
        c.set_reg(AX, a);
        c.test(B, a & 0xff, 3);
        return palette_after_2eb2(c);
    }
    c.run(0x472d, 6)?;
    let a = c.pop();
    c.set_reg(AX, a);
    c.test(B, a & 0xff, 3);
    c.set_word(0x472b, 0);
    c.push(a);
    c.call(0x41c5, 0x473c)?;
    let m1 = c.byte(0x11c9);
    let d = (m1 & 3).wrapping_sub(1);
    c.run(0x473c, 5)?;
    let a = c.and(B, m1 as u16, 3);
    let a = c.dec(B, a);
    c.set_reg8(r8::AL, a as u8);
    if d == 0 {
        c.call(0x181e, 0x4748)?;
        c.run(0x4748, 1)?;
    }
    c.call(0xc474, 0x474b)?;
    c.run(0x474b, 1)?;
    c.call(0x40d5, 0x474e)?;
    c.run(0x474e, 4)?;
    let a = c.pop();
    c.set_reg(AX, a);
    let h = c.byte(0x11c7);
    c.set_reg8(r8::BL, h);
    let bx = c.reg(BX);
    c.push(bx);
    c.call(0x4795, 0x4757)?;
    c.run(0x4757, 4)?;
    let a = c.pop();
    c.set_reg(AX, a);
    c.set_byte(0x11c7, a as u8);
    c.set_byte(8, 0xff);
    c.call(0x4b3b, 0x4763)?;
    let v = c.byte(0x46eb);
    let neg = v & 0x80 != 0;
    c.run(0x4763, if neg { 5 } else { 5 })?;
    c.set_word(0x114e, 0);
    c.set_word(0x4729, 0);
    c.cmp(B, v as u16, 0);
    if !neg {
        c.call(0x2dbf, 0x4779)?;
        c.run(0x4779, 1)?;
    }
    c.call(0x4ab8, 0x477c)?;
    let m2 = c.byte(0x11c9);
    let d = (m2 & 3).wrapping_sub(1);
    if d != 0 {
        c.run(0x477c, 4)?;
        let a = c.and(B, m2 as u16, 3);
        let a = c.dec(B, a);
        c.set_reg8(r8::AL, a as u8);
        return palette_after_2eb2(c);
    }
    c.run(0x477c, 7)?;
    let a = c.and(B, m2 as u16, 3);
    let a = c.dec(B, a);
    c.set_reg8(r8::AL, a as u8);
    let di = c.word(0x1150);
    c.set_reg(DI, di);
    let v = c.byte(di.wrapping_add(0x15));
    let v = c.dec(B, v as u16);
    c.set_byte(di.wrapping_add(0x15), v as u8);
    super::audio::pcm_stop_voc(c.m, 0xac14)
}

/// From 478Fh: `loc_02eb2`, then the palette.
fn palette_after_2eb2(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x478f, 1)?;
    c.call(0x2eb2, 0x4792)?;
    c.run(0x4792, 1)?;
    super::gfx::update_screen_palette(c.m, 0xc0f4)
}

/// `sub_1dd7` (`loc_01f64`): the Harkonnen take a location: from phase
/// 3Ch, or 70h days after [1154h], unless [2] bit 10h or [11BCh] was set
/// (cleared), on the carry out of rotating [0] left: the target
/// (`ui_comm`, ZF: none); [0C4h] up; twice the troops at the location BP
/// (+9, chained by +1) with bit 80h of +10h are taken (+3 8Dh, `loc_084a6`,
/// bit 10h of +10h cleared, `loc_08357`); the location DI flagged 2, its
/// troops (`for_location_troops_8403`), the place records at 0FD8h (9 of
/// 10h) of the place BX moved to DX, the event 0Ch (0Dh for the troop
/// 8E0h's location) noted (`call_29f0_f`); for the current place [6]
/// its new id DX in [4] and [0Bh], [473Bh], [2Bh] and `note_60f8`, else
/// the shown locations counted.
pub fn harkonnen_take(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1dd7)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let ph = c.byte(0x2a);
    if ph < 0x3c {
        let d = c.word(2);
        let t = c.word(0x1154);
        let a = d.wrapping_sub(t);
        if d < t || a < 0x70 {
            c.run(0x1f64, if d < t { 6 } else { 8 })?;
            c.cmp(B, ph as u16, 0x3c);
            let a = c.sub(W, d, t);
            c.set_reg(AX, a);
            if d >= t {
                c.cmp(W, a, 0x70);
            }
            return c.ret();
        }
        c.run(0x1f64, 7)?;
        c.cmp(B, ph as u16, 0x3c);
        let a = c.sub(W, d, t);
        c.set_reg(AX, a);
        c.cmp(W, a, 0x70);
    } else {
        c.run(0x1f64, 2)?;
        c.cmp(B, ph as u16, 0x3c);
    }
    let d = c.word(2);
    if d & 0x10 != 0 {
        c.run(0x1f79, 3)?;
        c.test(W, d, 0x10);
        return c.ret();
    }
    let k = c.byte(0x11bc);
    if k != 0 {
        c.run(0x1f79, 7)?;
        c.test(W, d, 0x10);
        c.xor(B, 0, 0);
        c.set_byte(0x11bc, 0);
        c.set_reg8(r8::AL, k);
        c.or(B, k as u16, k as u16);
        return c.ret();
    }
    let w = c.word(0);
    let carry = w & 0x8000 != 0;
    c.run(0x1f79, if carry { 9 } else { 9 })?;
    c.test(W, d, 0x10);
    c.xor(B, 0, 0);
    c.set_byte(0x11bc, 0);
    c.set_reg8(r8::AL, 0);
    c.or(B, 0, 0);
    let r = c.rol1(W, w);
    c.set_word(0, r);
    if !carry {
        return c.ret();
    }
    c.call(0x2017, 0x1f95)?;
    if c.flag(pc::cpu::ZF) {
        c.run(0x1f95, 2)?;
        return c.ret();
    }
    c.run(0x1f95, 3)?;
    let v = c.byte(0xc4);
    let v = c.inc(B, v as u16);
    c.set_byte(0xc4, v as u8);
    c.set_reg(CX, 2);
    // Twice: the first troop of the chain with bit 80h of +10h taken.
    loop {
        let bp = c.reg(BP);
        let al = c.byte_in(SS, bp.wrapping_add(9));
        c.run(0x1f9e, 1)?;
        c.set_reg8(r8::AL, al);
        let mut found = false;
        let mut i = 0;
        loop {
            i += 1;
            if i > 300 {
                c.m.cpu.ip = 0x1fa1;
                return Err(Bail);
            }
            let al = c.reg8(r8::AL);
            if al == 0 {
                c.run(0x1fa1, 2)?;
                c.or(B, 0, 0);
                break;
            }
            c.run(0x1fa1, 3)?;
            c.or(B, al as u16, al as u16);
            c.call(0x6906, 0x1fa8)?;
            let si = c.reg(SI);
            let nx = c.byte(si.wrapping_add(1));
            let f = c.byte(si.wrapping_add(0x10));
            c.run(0x1fa8, if f & 0x80 != 0 { 9 } else { 3 })?;
            c.set_reg8(r8::AL, nx);
            c.test(B, f as u16, 0x80);
            if f & 0x80 != 0 {
                found = true;
                break;
            }
        }
        if !found {
            break;
        }
        let (cx, bp, si, di) = (c.reg(CX), c.reg(BP), c.reg(SI), c.reg(DI));
        c.push(cx);
        c.push(bp);
        c.push(si);
        c.push(di);
        c.set_byte(si.wrapping_add(3), 0x8d);
        c.call(0x84a6, 0x1fbc)?;
        c.run(0x1fbc, 5)?;
        let d = c.pop();
        c.set_reg(DI, d);
        let s = c.pop();
        c.set_reg(SI, s);
        let f = c.byte(s.wrapping_add(0x10));
        let f = c.and(B, f as u16, 0xef);
        c.set_byte(s.wrapping_add(0x10), f as u8);
        c.push(d);
        c.call(0x8357, 0x1fc6)?;
        c.run(0x1fc6, 4)?;
        let d = c.pop();
        c.set_reg(DI, d);
        let b = c.pop();
        c.set_reg(BP, b);
        let cx = c.pop().wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.run(0x1fcb, 2)?;
    let di = c.reg(DI);
    let f = c.byte(di.wrapping_add(0x0a));
    let f = c.or(B, f as u16, 2);
    c.set_byte(di.wrapping_add(0x0a), f as u8);
    c.call(0x83fd, 0x1fd2)?;
    c.run(0x1fd2, 1)?;
    c.call(0x40ae, 0x1fd5)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    let mut n = 2;
    for i in 0..9u16 {
        let p = 0xfd8 + 0x10 * i;
        n += 4 + (c.word(p + 2) == bx) as u32;
    }
    let di = c.reg(DI);
    let own = c.word(0x8e4) == di;
    n += 4 + own as u32 + 3;
    c.run(0x1fd5, n)?;
    c.set_reg(SI, 0xfd8);
    c.set_reg(CX, 9);
    for i in 0..9u16 {
        let p = 0xfd8 + 0x10 * i;
        let v = c.word(p + 2);
        c.cmp(W, bx, v);
        if v == bx {
            c.set_word(p, dx);
        }
        let s = c.add(W, p, 0x10);
        c.set_reg(SI, s);
        c.set_reg(CX, 8 - i);
    }
    c.set_reg8(r8::AL, 0x0c);
    c.set_reg(SI, 0x8e0);
    let o = c.word(0x8e4);
    c.cmp(W, di, o);
    if own {
        let a = c.inc(B, 0x0c);
        c.set_reg8(r8::AL, a as u8);
    }
    c.push(bx);
    c.push(dx);
    c.call(0x71b2, 0x1ff8)?;
    let cur = c.word(6);
    let bx = c.word_in(SS, c.reg(SP).wrapping_add(2));
    if bx != cur {
        c.run(0x1ff8, 5)?;
        let d = c.pop();
        c.set_reg(DX, d);
        let b = c.pop();
        c.set_reg(BX, b);
        c.cmp(W, b, cur);
        return super::map::count_shown_location(c.m, 0x5d50);
    }
    c.run(0x1ff8, 10)?;
    let d = c.pop();
    c.set_reg(DX, d);
    let b = c.pop();
    c.set_reg(BX, b);
    c.cmp(W, b, cur);
    c.set_word(4, d);
    let al = d as u8;
    c.set_reg8(r8::AL, al);
    c.set_byte(0x0b, al);
    let v = c.byte(0x473b);
    let v = c.or(B, v as u16, al as u16);
    c.set_byte(0x473b, v as u8);
    c.set_byte(0x2b, al);
    c.call(0x6144, 0x2013)?;
    c.run(0x2013, 1)?;
    c.ret()
}

/// `sub_1dd4` (`loc_020a4`): the spice demand's day check, with bit 7 of
/// [0BFh]. On the day [118Dh] (the day count from `get_ingame_day_q`):
/// while one is pending ([0C2h]) or otherwise, the days left in [0CFh]
/// (negated difference). With bit 4: the reminders 0..3 days late
/// (`grade_be` and the table at CS:2161h) noted as event 0Bh + code,
/// later ones and [11BBh] set [46D9h] 7. On the day itself a new demand:
/// [0C3h] up, the amount 96h * count + 64h scaled by a random E0h..11Fh
/// / 256 and by the stock factor ([0BEh] doubled, inverted, + 100h)
/// (FFFFh on overflow) in [0BCh], [0CFh] 0, [0BFh] bits 90h, and the
/// event 20Bh (30Bh with [0BEh] positive).
pub fn spice_demand_day(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1dd4)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let f = c.byte(0xbf);
    if f & 0x80 == 0 {
        c.run(0x20a4, 3)?;
        c.test(B, f as u16, 0x80);
        return c.ret();
    }
    c.run(0x20a4, 3)?;
    c.test(B, f as u16, 0x80);
    c.call(0x1ac5, 0x20ae)?;
    let ax = c.reg(AX);
    let day = c.word(0x118d);
    let d = ax.wrapping_sub(day);
    let pend = c.byte(0xc2);
    let f = c.byte(0xbf);
    let late = c.byte(0x11bb);
    if pend != 0 || f & 0x10 == 0 && late == 0 {
        // 2098h / 20C6h: the days left.
        let n = if pend != 0 { 2 } else { 6 } + 2 + if d != 0 { 3 } else { 0 };
        if pend == 0 && d == 0 {
            return new_demand(&mut c, ax, day, f);
        }
        c.run(0x20ae, n + if d == 0 { 1 } else { 0 })?;
        c.cmp(B, pend as u16, 0);
        if pend == 0 {
            c.test(B, f as u16, 0x10);
            c.cmp(B, 0, 0);
        }
        let a = c.sub(W, ax, day);
        c.set_reg(AX, a);
        if d != 0 {
            let a = c.neg(W, a);
            c.set_reg(AX, a);
            c.set_byte(0xcf, a as u8);
        }
        return c.ret();
    }
    if f & 0x10 == 0 {
        // [11BBh] set: 215Fh.
        c.run(0x20ae, 7 + 2)?;
        c.cmp(B, 0, 0);
        c.test(B, f as u16, 0x10);
        c.cmp(B, late as u16, 0);
        c.set_byte(0x46d9, 7);
        return c.ret();
    }
    // 2131h: reminders.
    if d == 0 {
        c.run(0x20ae, 6 + 1)?;
        c.cmp(B, 0, 0);
        c.test(B, f as u16, 0x10);
        let a = c.sub(W, ax, day);
        c.set_reg(AX, a);
        return c.ret();
    }
    if d >= 4 {
        c.run(0x20ae, 8 + 2)?;
        c.cmp(B, 0, 0);
        c.test(B, f as u16, 0x10);
        let a = c.sub(W, ax, day);
        c.set_reg(AX, a);
        c.cmp(W, a, 4);
        c.set_byte(0x46d9, 7);
        return c.ret();
    }
    c.run(0x20ae, 12)?;
    c.cmp(B, 0, 0);
    c.test(B, f as u16, 0x10);
    let a = c.sub(W, ax, day);
    c.set_reg(AX, a);
    c.cmp(W, a, 4);
    let a = c.shl1(B, a & 0xff, 2);
    c.set_reg(AX, d & 0xff00 | a);
    c.set_reg8(r8::BL, a as u8);
    c.call(0x24d2, 0x2145)?;
    let ah = (c.reg(AX) >> 8) as u8;
    let g = if ah < 3 { ah } else { 2 };
    let bl = c.reg8(r8::BL);
    let idx = g.wrapping_add(bl);
    let code = c.byte_in(pc::cpu::CS, 0x2161u16.wrapping_add(idx as u16));
    c.run(0x2145, 3 + if ah >= 3 { 1 } else { 0 } + 5 + if code == 0 { 2 } else { 3 })?;
    c.set_reg8(r8::AL, ah);
    c.cmp(B, ah as u16, 3);
    let a = c.add(B, g as u16, bl as u16);
    c.set_reg(BX, 0x2161);
    c.set_reg8(r8::AL, code);
    let _ = a;
    c.or(B, code as u16, code as u16);
    if code == 0 {
        c.set_byte(0x46d9, 7);
        return c.ret();
    }
    c.set_reg(AX, (code as u16) << 8 | 0x0b);
    note_event(c.m, 0x26da)
}

/// `loc_020d2` (after the day test, at 20AEh): the new demand.
fn new_demand(c: &mut Ctx, ax: u16, day: u16, f: u8) -> Result<(), Bail> {
    c.run(0x20ae, 6 + 2 + 11)?;
    c.cmp(B, 0, 0);
    c.test(B, f as u16, 0x10);
    c.cmp(B, 0, 0);
    let a = c.sub(W, ax, day);
    c.set_reg(AX, a);
    let k = c.byte(0xc3);
    c.set_reg8(r8::AL, k);
    let v = c.inc(B, k as u16);
    c.set_byte(0xc3, v as u8);
    c.set_reg(BX, 0x96);
    c.mul16(0x96);
    let (ax, dx) = (c.reg(AX), c.reg(DX));
    c.or(W, dx, dx);
    let cx = c.add(W, ax, 0x64);
    c.set_reg(AX, cx);
    c.set_reg(CX, cx);
    c.set_reg(BX, 0x3f);
    c.call(0xe3b7, 0x20ef)?;
    let r = c.reg(AX).wrapping_add(0xe0);
    let p = r as u32 * cx as u32;
    let (pdx, pax) = ((p >> 16) as u16, p as u16);
    let be = c.byte(0xbe);
    let sh = be << 1;
    let neg = be & 0x80 != 0;
    let mut n = 4u32;
    let mut ovf = false;
    if pdx >> 8 != 0 {
        ovf = true;
    } else {
        let d = (pdx & 0xff) << 8 | pax >> 8;
        n += 5;
        if !neg {
            n += 7;
            let a = (!sh) as u16 | 0x100;
            let q = a as u32 * d as u32;
            let (qdx, qax) = ((q >> 16) as u16, q as u16);
            // xchg dl, dh; xchg dl, ah: DX = (qdx_lo << 8) | qax_hi; AH = qdx_hi.
            let ah = qdx >> 8;
            let _ = qax;
            if ah != 0 {
                ovf = true;
            }
        }
    }
    // Replay with the exact instruction sequence.
    let pos = be & 0x80 == 0;
    let total = n + if ovf { 1 } else { 0 } + 6 + if pos { 1 } else { 0 } + 1;
    c.run(0x20ef, total)?;
    c.set_reg(AX, r);
    let a = c.add(W, r.wrapping_sub(0xe0), 0xe0);
    c.set_reg(AX, a);
    c.mul16(cx);
    c.or(B, pdx >> 8, pdx >> 8);
    if pdx >> 8 == 0 {
        let d = (pdx & 0xff) << 8 | pax >> 8;
        c.set_reg(DX, d);
        c.set_reg8(r8::AL, be);
        let s = c.shl1(B, be as u16, 1);
        c.set_reg8(r8::AL, s as u8);
        if !neg {
            c.set_reg(AX, 0x100 | (!s as u8) as u16);
            c.mul16(d);
            let (qax, qdx) = (c.reg(AX), c.reg(DX));
            let nd = (qdx & 0xff) << 8 | qax >> 8;
            let ah = qdx >> 8;
            c.set_reg(DX, nd);
            c.set_reg(AX, ah << 8 | qax & 0xff);
            c.or(B, ah, ah);
        }
    }
    if ovf {
        c.set_reg(DX, 0xffff);
    }
    let dx = c.reg(DX);
    c.set_word(0xbc, dx);
    c.set_byte(0xcf, 0);
    let v = c.byte(0xbf);
    let v = c.or(B, v as u16, 0x90);
    c.set_byte(0xbf, v as u8);
    let be2 = c.byte(0xbe);
    c.cmp(B, be2 as u16, 0);
    let mut ax = 0x20bu16;
    if be2 & 0x80 == 0 {
        let h = c.inc(B, 2);
        ax = h << 8 | 0x0b;
    }
    c.set_reg(AX, ax);
    note_event(c.m, 0x26da)
}

/// `sub_0b45`: the scene's frame step for the creatures and particles.
/// The scene timer 4856h +7 down (from 10h, by day, the scene picture
/// `loc_00d0d`); outside the map modes and dialogues: the spawn counter
/// +4 down; when it runs out, the burst counter +6: a new wait (random,
/// +6 = low 7 bits, +4 = the high byte) or a particle (random; kind 1Ch
/// + low 3 bits at x of bits 7..15 (below 140h), y 30h..5Fh); the
/// creature counter +0 down (`creature_step`). Then each particle of the
/// list 3CBEh: its sprite +8 stepped (14h..1Bh: 2 random bits from the
/// seed [0]; 1Ch..2Dh up by one; else the motion +0Dh unpacked twice
/// with 14h + 3 seed bits) and drawn at its offset (+0Ah: DL, DH signed)
/// by `loc_0c661`; off screen (x from 140h, y or bottom negative) or
/// past 2Dh removed.
pub fn scene_particles_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0b45)?;
    let mut c = Ctx::new(m);
    let t = c.byte(0x485d).wrapping_sub(1);
    let day = c.byte(0xea);
    let pic = !((day as i8) > 0) && t <= 0x10;
    c.run(start, 4 + if (day as i8) > 0 { 0 } else { 3 } + if pic { 2 } else { 0 })?;
    c.set_reg(SI, 0x4856);
    let t = c.dec(B, t.wrapping_add(1) as u16);
    c.set_byte(0x485d, t as u8);
    c.cmp(B, day as u16, 0);
    if (day as i8) <= 0 {
        c.set_reg8(r8::BL, t as u8);
        c.cmp(B, t, 0x10);
        if pic {
            c.push(0x4856);
            c.call(0x0d0d, 0x0b5e)?;
            c.run(0x0b5e, 1)?;
            let s = c.pop();
            c.set_reg(SI, s);
        }
    }
    let a = c.word(0x479e);
    let b = c.byte(0x46eb);
    let v = a & 0xff00 | ((a as u8) | b) as u16;
    if v != 0 {
        c.run(0x0b5f, 5)?;
        c.set_reg(AX, a);
        let l = c.or(B, a & 0xff, b as u16);
        c.set_reg(AX, a & 0xff00 | l);
        c.or(W, v, v);
        return c.ret();
    }
    let si = c.reg(SI);
    let w4 = c.word(si.wrapping_add(4)).wrapping_sub(1);
    let neg4 = w4 & 0x8000 != 0;
    let b6 = c.byte(si.wrapping_add(6)).wrapping_sub(1);
    let neg6 = b6 & 0x80 != 0;
    c.run(0x0b5f, 4 + 2 + if neg4 { 3 } else { 0 })?;
    c.set_reg(AX, a);
    let l = c.or(B, a & 0xff, b as u16);
    c.set_reg(AX, a & 0xff00 | l);
    c.or(W, v, v);
    let w = c.dec(W, w4.wrapping_add(1));
    c.set_word(si.wrapping_add(4), w);
    if neg4 {
        let b = c.dec(B, b6.wrapping_add(1) as u16);
        c.set_byte(si.wrapping_add(6), b as u8);
        c.call(0xe3cc, if neg6 { 0x0b78 } else { 0x0b89 })?;
        let r = c.reg(AX);
        if neg6 {
            c.run(0x0b78, 6)?;
            let a = c.and(B, r & 0xff, 0x7f);
            c.set_byte(si.wrapping_add(6), a as u8);
            c.xor(B, 0, 0);
            c.set_reg(AX, r >> 8);
            c.set_word(si.wrapping_add(4), r >> 8);
        } else {
            let bl = (r & 0x7f) as u8;
            let x = r.swap_bytes();
            let dh = (x >> 8) as u8;
            let dh = dh.rotate_left(1);
            let dxv = ((dh as u16) << 8 | (x & 0xff)) & 0x1ff;
            let n = if bl >= 0x60 { 5 } else if bl < 0x30 { 7 } else if dxv >= 0x140 { 12 } else { 18 };
            c.run(0x0b89, n)?;
            c.set_reg(BX, r);
            c.set_reg(DX, r);
            let b = c.and(W, r, 0x7f);
            c.set_reg(BX, b);
            c.cmp(B, b, 0x60);
            if bl < 0x60 {
                c.cmp(B, b, 0x30);
                if bl >= 0x30 {
                    c.set_reg(DX, x);
                    let h = c.m.cpu.shift(0, B, (x >> 8) as u32, 1) as u16;
                    let d = c.and(W, h << 8 | x & 0xff, 0x1ff);
                    c.set_reg(DX, d);
                    c.cmp(W, d, 0x140);
                    if d < 0x140 {
                        c.set_reg(AX, b);
                        let a = c.and(W, b, 7);
                        let a = c.add(W, a, 0x1c);
                        c.set_reg(AX, a);
                        c.push(si);
                        c.xor(W, 0, 0);
                        c.set_reg(SI, 0);
                        c.call(0xc60b, 0x0bb6)?;
                        c.run(0x0bb6, 1)?;
                        let s = c.pop();
                        c.set_reg(SI, s);
                    }
                }
            }
        }
    }
    let si = c.reg(SI);
    let k = c.byte(si).wrapping_sub(1);
    c.run(0x0bb7, if k & 0x80 != 0 { 3 } else { 2 })?;
    let k = c.dec(B, k.wrapping_add(1) as u16);
    c.set_byte(si, k as u8);
    if k & 0x80 != 0 {
        c.call(0x0c3b, 0x0bbe)?;
    }
    let n = c.word(0x3cbe);
    c.run(0x0bbe, if n == 0 { 4 } else { 3 })?;
    c.set_reg(DI, 0x3cc0);
    c.set_reg(CX, n);
    if n == 0 {
        return c.ret();
    }
    loop {
        let di = c.reg(DI);
        let (ax, dx) = (c.word(di.wrapping_add(8)), c.word(di.wrapping_add(0x0a)));
        let al = ax as u8;
        let cx = c.reg(CX);
        let mut remove = false;
        if al < 0x14 {
            c.run(0x0bc6, 11)?;
            c.set_reg(AX, ax);
            c.set_reg(DX, dx);
            c.push(cx);
            c.cmp(B, al as u16, 0x14);
            let mut a = c.shr1(W, ax, 2);
            for _ in 0..2 {
                let s = c.word(0);
                let s = c.rol1(W, s);
                c.set_word(0, s);
                a = c.rcl1(W, a);
            }
            c.set_reg(AX, a);
        } else if al >= 0x1c {
            let a1 = ax.wrapping_add(1);
            let past = (a1 as u8) > 0x2d;
            c.run(0x0bc6, if past { 11 } else { 10 })?;
            c.set_reg(AX, ax);
            c.set_reg(DX, dx);
            c.push(cx);
            c.cmp(B, al as u16, 0x14);
            c.cmp(B, al as u16, 0x1c);
            let a = c.inc(W, ax);
            c.set_reg(AX, a);
            c.cmp(B, a & 0xff, 0x2d);
            remove = past;
        } else {
            c.run(0x0bc6, 9)?;
            c.set_reg(AX, ax);
            c.set_reg(DX, dx);
            c.push(cx);
            c.cmp(B, al as u16, 0x14);
            c.cmp(B, al as u16, 0x1c);
            let b = c.word(di.wrapping_add(0x0d));
            c.set_reg(BX, b);
            c.call(0x0cea, 0x0be2)?;
            c.run(0x0be2, 1)?;
            c.call(0x0cea, 0x0be5)?;
            c.run(0x0be5, 8)?;
            let bx = c.reg(BX);
            c.set_word(di.wrapping_add(0x0d), bx);
            let s = c.word(0);
            c.set_reg8(r8::CL, 3);
            let s = c.m.cpu.shift(0, W, s as u32, 3) as u16;
            c.set_word(0, s);
            let a = c.and(W, s, 7);
            let a = c.add(B, a, 0x14);
            c.set_reg(AX, a);
        }
        if !remove {
            c.run(0x0c09, 10)?;
            let ax = c.reg(AX);
            c.set_word(di.wrapping_add(8), ax);
            let dx = c.reg(DX);
            let y = (dx >> 8) as u8 as i8 as i16 as u16;
            let x = dx as u8 as i8 as i16 as u16;
            c.set_reg(BX, y);
            c.set_reg(AX, x);
            c.set_reg(DX, x);
            c.push(di);
            c.call(0xc661, 0x0c1c)?;
            let di = c.word_in(SS, c.reg(SP));
            let (x0, y0, y1) = (c.word(di), c.word(di.wrapping_add(4)), c.word(di.wrapping_add(6)));
            let off = x0 >= 0x140 || y0 & 0x8000 != 0 || y1 & 0x8000 != 0;
            let n = if x0 >= 0x140 { 3 } else if y0 & 0x8000 != 0 { 6 } else { 8 };
            c.run(0x0c1c, n)?;
            let d = c.pop();
            c.set_reg(DI, d);
            c.cmp(W, x0, 0x140);
            if x0 < 0x140 {
                c.xor(W, 0, 0);
                c.set_reg(AX, 0);
                c.cmp(W, y0, 0);
                if y0 & 0x8000 == 0 {
                    c.cmp(W, y1, 0);
                }
            }
            remove = off;
        }
        if remove {
            c.run(0x0c2f, 2)?;
            let di = c.reg(DI);
            c.push(di);
            c.call(0xc58a, 0x0c33)?;
            c.run(0x0c33, 1)?;
            let d = c.pop();
            c.set_reg(DI, d);
        }
        let cx = c.word_in(SS, c.reg(SP)).wrapping_sub(1);
        c.run(0x0c34, if cx == 0 { 4 } else { 3 })?;
        c.pop();
        let di = c.reg(DI);
        let d = c.add(W, di, 0x11);
        c.set_reg(DI, d);
        c.set_reg(CX, cx);
        if cx == 0 {
            return c.ret();
        }
    }
}

/// `loc_035ad`: a scene starts once ([47A6h], cleared; [1Ah] and [47A7h]
/// 0). At a place: [1Ah] 1, in place 8 with the flags [0C0h] & [1158h]
/// and bit 3 of [12h] `loc_02566`, then each record here gets
/// `scene_sentence` ([`for_records_here_bp`] with BP 3520h). In a map
/// mode (stage [23h] 0): with a place [1152h] its encounters and watch,
/// and for a stage the picked record (`pick_1152`) shown with its voice
/// after a wait of 4Bh, its sentence (`loc_096d8`) and record
/// (`loc_03551`); without one the watch, and for a stage the scene 24h
/// with the phrase BFh, the panel 1F9Eh (bit 40h) and its buttons.
pub fn scene_start(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x35ad)?;
    let mut c = Ctx::new(m);
    let mode = c.byte(0x11c9);
    let was = c.byte(0x47a6);
    if mode == 0 {
        if was != 0 {
            c.run(start, 9)?;
            scene_flags(&mut c, mode, was, false);
            return c.ret();
        }
        let k = c.byte(0x0b);
        let f = c.word(0xc0) & c.word(0x1158);
        let t = c.word(0x12);
        let call = k == 8 && f != 0 && t & 8 != 0;
        let n = 11 + if k != 8 { 0 } else if f == 0 { 3 } else { 5 } + call as u32;
        c.run(start, n)?;
        scene_flags(&mut c, mode, was, false);
        let v = c.byte(0x1a);
        let v = c.inc(B, v as u16);
        c.set_byte(0x1a, v as u8);
        c.cmp(B, k as u16, 8);
        if k == 8 {
            let (a, b) = (c.word(0xc0), c.word(0x1158));
            c.set_reg(AX, a);
            let x = c.and(W, a, b);
            c.set_reg(AX, x);
            if x != 0 {
                c.test(W, t, 8);
            }
        }
        if call {
            c.call(0x2566, 0x35e3)?;
        }
        c.run(0x35e3, 2)?;
        c.set_reg(BP, 0x3520);
        return for_records_here_bp(c.m, 0x36ee);
    }
    if was != 0 {
        c.run(start, 10)?;
        scene_flags(&mut c, mode, was, true);
        return c.ret();
    }
    let p = c.word(0x1152);
    c.run(start, 12)?;
    scene_flags(&mut c, mode, was, true);
    c.cmp(W, p, 0xffff);
    if p != 0xffff {
        c.call(0x40f9, 0x3606)?;
        c.run(0x3606, 1)?;
        c.call(0x4182, 0x3609)?;
        let st = c.byte(0x23);
        if st == 0 {
            c.run(0x3609, 3)?;
            c.cmp(B, 0, 0);
            return c.ret();
        }
        c.run(0x3609, 3)?;
        c.cmp(B, st as u16, 0);
        c.call(0x366f, 0x3613)?;
        if c.flag(pc::cpu::SF) {
            c.run(0x3613, 2)?;
            return c.ret();
        }
        c.run(0x3613, 2)?;
        c.call(0xdbb2, 0x3618)?;
        c.run(0x3618, 1)?;
        c.call(0x368b, 0x361b)?;
        c.run(0x361b, 3)?;
        let ax = c.reg(AX);
        c.push(ax);
        c.set_reg(AX, 0x4b);
        c.call(0xe387, 0x3622)?;
        c.run(0x3622, 3)?;
        let a = c.pop();
        c.set_reg(AX, a);
        c.push(a);
        c.call(0x96d8, 0x3627)?;
        let cf = c.flag(CF);
        c.run(0x3627, if cf { 3 } else { 7 })?;
        let a = c.pop();
        c.set_reg(AX, a);
        if cf {
            return c.ret();
        }
        c.set_reg8(r8::CL, 0x10);
        c.mul8(0x10);
        let a = c.reg(AX);
        let a = c.add(W, a, 0xfd8);
        c.set_reg(AX, a);
        c.set_reg(SI, a);
        c.call(0x3551, 0x3636)?;
        c.run(0x3636, 1)?;
        return c.ret();
    }
    c.call(0x4182, 0x363a)?;
    let st = c.byte(0x23);
    if st == 0 {
        c.run(0x363a, 3)?;
        c.cmp(B, 0, 0);
        return c.ret();
    }
    c.run(0x363a, 3)?;
    c.cmp(B, st as u16, 0);
    c.call(0xdbb2, 0x3644)?;
    c.run(0x3644, 1)?;
    c.call(0x439f, 0x3647)?;
    c.run(0x3647, 5)?;
    c.set_reg(CX, 0x200c);
    c.set_reg(DX, 0x66);
    c.set_reg(BX, 0x4e);
    c.set_reg(AX, 0xbf);
    c.call(0xd194, 0x3656)?;
    for (at, to) in [(0x3656u16, 0xc0f4u16), (0x3659, 0xc4dd), (0x365c, 0x4aca)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    c.run(0x365f, 4)?;
    c.set_reg(BP, 0x1f9e);
    let v = c.byte_in(SS, 0x1fa9);
    let v = c.or(B, v as u16, 0x40);
    c.m.cpu.write8(&mut c.m.hw, SS, 0x1fa9, v as u8);
    c.set_reg(BX, 0x4abe);
    c.call(0xd323, 0x366c)?;
    c.run(0x366c, 1)?;
    super::screens::panel_buttons(c.m, 0x2ffb)
}

/// `loc_035ad`'s start: [1Ah], [47A7h] (and in a map mode [23h]) 0,
/// [47A6h] taken.
fn scene_flags(c: &mut Ctx, mode: u8, was: u8, map: bool) {
    c.cmp(B, mode as u16, 0);
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.set_byte(0x1a, 0);
    c.set_byte(0x47a7, 0);
    if map {
        c.set_byte(0x23, 0);
    }
    c.set_byte(0x47a6, 0);
    c.set_reg8(r8::AL, was);
    c.or(B, was as u16, was as u16);
}

/// `loc_02e52`: the scene refreshed after a move: `scene_start`, the time
/// noted ([0DC5Ah] = [0CE7Ah]); unless the scene took over ([47A7h]):
/// travelling ([4735h] bit 7) a travel step (`loc_03723`); else outside a
/// place ([8] not FFh) the head raised (`ui_head_animate_up`) unless
/// [4774h]; at a place in the room mode the speaker [47AAh]'s phrase
/// ([47C4h] = its +0Eh, `loc_0978e`).
pub fn scene_refresh(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2e52)?;
    let mut c = Ctx::new(m);
    refresh_at(&mut c)
}

fn refresh_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x2e52, 1)?;
    c.call(0x35ad, 0x2e55)?;
    let t = c.word(0xce7a);
    let k = c.byte(0x47a7);
    let note = |c: &mut Ctx| {
        c.set_reg(AX, t);
        c.set_word(0xdc5a, t);
        c.cmp(B, k as u16, 0);
    };
    if k != 0 {
        c.run(0x2e55, 5)?;
        note(c);
        return c.ret();
    }
    let al = c.byte(0x4735);
    let head = |c: &mut Ctx| {
        note(c);
        c.set_reg8(r8::AL, al);
        c.or(B, al as u16, al as u16);
    };
    if al & 0x80 != 0 {
        c.run(0x2e55, 8)?;
        head(c);
        return travel_step(c);
    }
    let b8 = c.byte(8);
    if b8 != 0xff {
        let p = c.byte(0x4774);
        c.run(0x2e55, 12)?;
        head(c);
        c.cmp(B, b8 as u16, 0xff);
        c.cmp(B, p as u16, 0);
        if p != 0 {
            return c.ret();
        }
        return super::ui::ui_head_animate_up(c.m, 0x17e6);
    }
    let mode = c.byte(0x11c9);
    if mode != 0 {
        c.run(0x2e55, 12)?;
        head(c);
        c.cmp(B, b8 as u16, 0xff);
        c.cmp(B, mode as u16, 0);
        return c.ret();
    }
    let si = c.word(0x47aa);
    c.run(0x2e55, if si == 0 { 15 } else { 18 })?;
    head(c);
    c.cmp(B, b8 as u16, 0xff);
    c.cmp(B, 0, 0);
    c.set_reg(SI, si);
    c.or(W, si, si);
    if si == 0 {
        return c.ret();
    }
    c.xor(W, 0, 0);
    let v = c.byte(si.wrapping_add(0x0e)) as u16;
    c.set_reg(AX, v);
    c.set_word(0x47c4, v);
    c.call(0x978e, 0x2e97)?;
    c.run(0x2e97, 1)?;
    c.ret()
}

/// `loc_03723`: a travel step (AL = [4735h], bit 7 then dropped): from
/// 94h, every 10h, the view redrawn (`loc_037ad`) and a wait of
/// ((AL - 94h) / 10h + 1, at most 0Ah) * 100h; before 37h the arrival
/// (`loc_02db1`); else the fade-out, the handlers 1A0Fh and 401Fh, and
/// the move to the place's room ([4], DL 2 / 0Ah for the sietches and
/// palaces) scheduled at 4057h before `scene_start`.
fn travel_step(c: &mut Ctx) -> Result<(), Bail> {
    let al = c.reg8(r8::AL);
    let s = c.byte(0x4735);
    let a = al.wrapping_sub(0x94);
    let n = if al < 0x94 { 4 } else if a & 0x0f != 0 { 6 } else { 7 };
    c.run(0x3723, n)?;
    let v = c.and(B, s as u16, 0x7f);
    c.set_byte(0x4735, v as u8);
    c.sub(B, al as u16, 0x94);
    c.set_reg8(r8::AL, a);
    if al < 0x94 {
        return c.ret();
    }
    c.test(B, a as u16, 0x0f);
    if a & 0x0f != 0 {
        return c.ret();
    }
    let ax = c.reg(AX);
    c.push(ax);
    c.call(0xdbb2, 0x3734)?;
    for (at, to) in [(0x3734u16, 0xc07cu16), (0x3737, 0x37ad), (0x373a, 0xc4dd)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    let sp = c.reg(SP);
    let ax = c.word_in(SS, sp);
    let big = ((ax & 0xff) >> 4) + 1 >= 0x0a;
    c.run(0x373d, 9 + big as u32)?;
    c.pop();
    c.set_reg(AX, ax);
    c.xor(B, 0, 0);
    c.set_reg8(r8::CL, 4);
    let x = c.m.cpu.shift(5, W, ax as u32 & 0xff, 4) as u16;
    let x = c.inc(W, x);
    c.cmp(B, x & 0xff, 0x0a);
    let x = if big { 0x0a } else { x & 0xff };
    c.set_reg(AX, x << 8);
    c.call(0xe387, 0x3750)?;
    let s = c.byte(0x4735);
    c.run(0x3750, 3)?;
    c.cmp(B, s as u16, 0x37);
    if s < 0x37 {
        return room_arrive(c.m, 0x2db1);
    }
    c.call(0x0e77, 0x375a)?;
    c.run(0x375a, 2)?;
    c.set_reg(CX, 5);
    c.call(0x0fd9, 0x3760)?;
    c.run(0x3760, 3)?;
    c.set_reg(BP, 0xc0ad);
    c.set_reg8(r8::AL, 0x34);
    c.call(0xc108, 0x3768)?;
    c.run(0x3768, 1)?;
    c.call(0x0086, 0x376b)?;
    c.run(0x376b, 2)?;
    c.set_reg(BP, 0x1a0f);
    c.call(0xc097, 0x3771)?;
    c.run(0x3771, 4)?;
    let si = c.word(0x1150);
    c.set_reg(SI, si);
    c.set_byte(0x47a6, 0xff);
    c.set_reg(BP, 0x401f);
    c.call(0xc097, 0x3780)?;
    let dx = c.word(4);
    let bx = c.word(6);
    let dh = (dx >> 8) as u8;
    c.run(0x3780, 4 + if dh < 0x21 { 3 + (dh == 0x20) as u32 } else { 0 } + 4)?;
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.cmp(B, dh as u16, 0x21);
    if dh < 0x21 {
        c.set_reg8(r8::DL, 2);
        c.cmp(B, dh as u16, 0x20);
        if dh == 0x20 {
            c.set_reg8(r8::DL, 0x0a);
        }
    }
    c.set_reg(BP, 0x4057);
    c.set_reg8(r8::AL, 0x34);
    c.set_byte(0x47a6, 0xff);
    c.call(0xc108, 0x37a3)?;
    c.run(0x37a3, 2)?;
    let e = c.byte(0xe7);
    let e = c.inc(B, e as u16);
    c.set_byte(0xe7, e as u8);
    scene_start(c.m, 0x35ad)
}

/// `loc_02db1` / `loc_02dbf`: the arrival in a room (from `loc_02db1`
/// the handler D717h, `loc_0d95b` and [47A6h] FFh first): `loc_02d74`,
/// the speaker and its flags dropped, the handler 2EB2h; at night ([2Bh])
/// the night view (`loc_00acd`) and the head raised; else the room drawn
/// (`loc_05ba0`, `loc_037b2`, `loc_0c412` outside the map modes), with
/// the light changed ([46DFh] to [46E0h]) a fade (10h, F66h) unless the
/// palettes match, and the scene refreshed (`loc_02e52`).
pub fn room_arrive(m: &mut Machine, start: u16) -> Result<(), Bail> {
    let mut c = Ctx::new(m);
    match start {
        0x2db1 => {
            c.run(0x2db1, 2)?;
            c.set_reg(BP, 0xd717);
            c.call(0xc097, 0x2db7)?;
            c.run(0x2db7, 1)?;
            c.call(0xd95b, 0x2dba)?;
            c.run(0x2dba, 2)?;
            c.set_byte(0x47a6, 0xff);
        }
        0x2dbf => c.run(0x2dbf, 1)?,
        0x2dd3 => {}
        _ => return Err(Bail),
    }
    if start != 0x2dd3 {
        c.call(0x2d74, 0x2dc2)?;
        c.run(0x2dc2, 6)?;
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
        c.set_byte(0xdce6, 0);
        c.set_byte(0x47a4, 0);
        c.set_word(0x47aa, 0);
        c.set_reg(BP, 0x2eb2);
        c.call(0xc097, 0x2dd3)?;
    }
    let k = c.byte(0x2b);
    if k != 0 {
        c.run(0x2dd3, 4)?;
        c.cmp(B, k as u16, 0);
        c.set_byte(0x4732, 0);
        c.call(0x2d74, 0x2de2)?;
        c.run(0x2de2, 3)?;
        let v = c.byte(0x11bc);
        let v = c.or(B, v as u16, 1);
        c.set_byte(0x11bc, v as u8);
        c.set_byte(0x46df, 0);
        c.call(0x0acd, 0x2def)?;
        for (at, to) in [(0x2defu16, 0x1797u16), (0x2df2, 0xc4cd), (0x2df5, 0xc0f4)] {
            c.run(at, 1)?;
            c.call(to, at + 3)?;
        }
        c.run(0x2df8, 1)?;
        return super::ui::ui_head_animate_up(c.m, 0x17e6);
    }
    let f = c.byte(0x4732);
    c.run(0x2dd3, 4 + (f & 1 != 0) as u32)?;
    c.cmp(B, k as u16, 0);
    c.test(B, f as u16, 1);
    if f & 1 != 0 {
        c.call(0x488a, 0x2e05)?;
    }
    c.run(0x2e05, 4)?;
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.set_word(0x14, 0);
    c.set_byte(0x46df, 0);
    c.call(0xc07c, 0x2e10)?;
    for (at, to) in [(0x2e10u16, 0x5ba0u16), (0x2e13, 0x37b2)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    let mode = c.byte(0x11c9);
    c.run(0x2e16, 2 + (mode & 3 == 0) as u32)?;
    c.test(B, mode as u16, 3);
    if mode & 3 == 0 {
        c.call(0xc412, 0x2e20)?;
    }
    for (at, to) in [(0x2e20u16, 0xad5eu16), (0x2e23, 0x1834), (0x2e26, 0x1797)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    let a = c.byte(0x46df);
    let old = c.byte(0x46e0);
    let swap = |c: &mut Ctx| {
        c.set_reg8(r8::AL, a);
        c.set_reg8(r8::AH, a);
        c.set_byte(0x46e0, a);
        c.set_reg8(r8::AL, old);
        c.cmp(B, old as u16, a as u16);
    };
    if old == a {
        c.run(0x2e29, 6)?;
        swap(&mut c);
        c.call(0xc0f4, 0x2e4f)?;
        c.run(0x2e4f, 1)?;
        c.call(0xc4cd, 0x2e52)?;
        return refresh_at(&mut c);
    }
    let (x, y) = (c.word(0xdbd6), c.word(0xdbd8));
    c.run(0x2e29, if x == y { 8 } else { 11 })?;
    swap(&mut c);
    c.set_reg(AX, x);
    c.cmp(W, x, y);
    if x != y {
        c.set_reg8(r8::AL, 0x10);
        c.set_reg(BP, 0x0f66);
        c.call(0xc108, 0x2e47)?;
        c.run(0x2e47, 1)?;
        c.call(0xae04, 0x2e4a)?;
        c.run(0x2e4a, 1)?;
    }
    refresh_at(&mut c)
}

/// `loc_04057`: into the room DX (DL the room, DH the place type) at the
/// position BX ([`loc_040c3`] first): [4] / [6] set, the previous room in
/// [0Dh]; unless [46EBh] is negative the arrival (`loc_02dbf`). The room
/// 3002h (the palace's first room) goes on at `loc_016fc` (not ported:
/// bails there).
pub fn enter_room(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4057)?;
    let mut c = Ctx::new(m);
    enter_room_at(&mut c)
}

fn enter_room_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x4057, 1)?;
    c.call(0x40c3, 0x405a)?;
    let dx = c.reg(DX);
    let bx = c.reg(BX);
    let f = c.byte(0x46eb);
    if f & 0x80 == 0 && dx == 0x3002 {
        c.m.cpu.ip = 0x405a;
        return Err(Bail);
    }
    c.run(0x405a, if f & 0x80 != 0 { 8 } else { 10 })?;
    c.set_word(4, dx);
    let old = c.byte(0x0b);
    c.set_byte(0x0b, dx as u8);
    c.set_reg8(r8::AL, old);
    c.set_byte(0x0d, old);
    c.set_word(6, bx);
    c.cmp(B, f as u16, 0);
    if f & 0x80 != 0 {
        return c.ret();
    }
    c.cmp(W, dx, 0x3002);
    room_arrive(c.m, 0x2dbf)
}

/// `ui_click_room_up` / `_right` / `_down` (BP 1-3) and
/// `ui_click_move_room` (BP the direction): the panel and the cursor
/// reset (`loc_0d2bd`, `loc_0ac30`, `loc_0a7a5`). Outside a place ([6]
/// low byte not 80h) a travel step: [4735h] counts up (bit 7 set) and
/// [0F4h] to 14h. At a place the room's exit BP (`loc_03efe`): none
/// returns; a room of the place (positive) is entered, first visits
/// noted ([0Bh] 1 and the place's bit 10h: `loc_0b389` 2 / 3 with the
/// counter [25h]), with the scene change (`loc_0a1c4`, `loc_036d3`,
/// `loc_0a1e2`, `loc_0abd5`) before `loc_04057`; a negative exit leaves
/// the place in that direction ([114Eh] dropped). A move looks up the
/// neighbour (`loc_0b5cf`, `loc_0b532`, `loc_0409a`) and on a match
/// enters that place (`loc_0503c`, `loc_0425b`, `loc_040ae`,
/// `loc_02170`) before `loc_04057`.
pub fn room_move(m: &mut Machine, start: u16) -> Result<(), Bail> {
    let mut c = Ctx::new(m);
    match start {
        0x3f15 | 0x3f1a | 0x3f1f => {
            c.run(start, 2)?;
            c.set_reg(BP, (start - 0x3f15) / 5 + 1);
        }
        0x3f27 => {}
        _ => return Err(Bail),
    }
    c.run(0x3f27, 2)?;
    let bp = c.reg(BP);
    c.push(bp);
    c.call(0xd2bd, 0x3f2b)?;
    c.run(0x3f2b, 1)?;
    c.call(0xac30, 0x3f2e)?;
    c.run(0x3f2e, 1)?;
    c.call(0xa7a5, 0x3f31)?;
    let dx = c.word(4);
    let bx = c.word(6);
    let start_ops = |c: &mut Ctx| {
        let bp = c.pop();
        c.set_reg(BP, bp);
        c.set_byte(0x47a9, 0);
        c.set_byte(0x26, 0);
        c.set_reg(DX, dx);
        c.set_reg(BX, bx);
        c.cmp(B, bx & 0xff, 0x80);
    };
    if bx & 0xff != 0x80 {
        let s = c.byte(0x4735);
        let f4 = c.byte(0xf4);
        let a = (s & 0x7f).wrapping_add(1);
        let n = 7 + 4 + (a & 0x80 != 0) as u32 + 4 + (f4 < 0x14) as u32 + 1;
        c.run(0x3f31, n)?;
        start_ops(&mut c);
        c.set_reg8(r8::AL, s);
        let x = c.and(B, s as u16, 0x7f);
        let mut x = c.inc(B, x);
        if x & 0x80 != 0 {
            x = c.dec(B, x);
        }
        let x = c.or(B, x, 0x80);
        c.set_reg8(r8::AL, x as u8);
        c.set_byte(0x4735, x as u8);
        c.cmp(B, f4 as u16, 0x14);
        if f4 < 0x14 {
            let v = c.inc(B, f4 as u16);
            c.set_byte(0xf4, v as u8);
        }
        return step_3ff5(&mut c);
    }
    c.run(0x3f31, 8)?;
    start_ops(&mut c);
    c.call(0x3efe, 0x3f6a)?;
    let (bp, si) = (c.reg(BP), c.reg(SI));
    let dl = c.byte_in(SS, bp.wrapping_add(si));
    let pick = |c: &mut Ctx| {
        c.set_reg8(r8::DL, dl);
        c.or(B, dl as u16, dl as u16);
    };
    if dl == 0 {
        c.run(0x3f6a, 4)?;
        pick(&mut c);
        return c.ret();
    }
    if dl & 0x80 != 0 {
        c.run(0x3f6a, 4)?;
        pick(&mut c);
        return leave_place(&mut c);
    }
    let b = c.byte(0x0b);
    if b == 1 {
        c.run(0x3f6a, 7)?;
        pick(&mut c);
        c.cmp(B, 1, 1);
        c.call(0xe270, 0x3f7c)?;
        c.run(0x3f7c, 2)?;
        c.set_reg8(r8::CL, 2);
        c.call(0xb389, 0x3f81)?;
        c.run(0x3f81, 1)?;
        c.call(0xe283, 0x3f84)?;
    } else {
        c.run(0x3f6a, 6)?;
        pick(&mut c);
        c.cmp(B, b as u16, 1);
    }
    let si = c.word(0x114e);
    let t = c.byte(si.wrapping_add(0x0a));
    if t & 0x10 != 0 {
        c.run(0x3f84, 3)?;
        c.set_reg(SI, si);
        c.test(B, t as u16, 0x10);
    } else {
        c.run(0x3f84, 8)?;
        c.set_reg(SI, si);
        c.test(B, t as u16, 0x10);
        let v = c.or(B, t as u16, 0x10);
        c.set_byte(si.wrapping_add(0x0a), v as u8);
        let dh = c.reg8(r8::DH);
        c.cmp(B, dh as u16, 0x20);
        let k = c.byte(0x25);
        let k = c.adc(B, k as u16, 0);
        c.set_byte(0x25, k as u8);
        c.set_byte(0x26, 0xff);
        c.call(0xe270, 0x3fa2)?;
        c.run(0x3fa2, 2)?;
        c.set_reg8(r8::CL, 3);
        c.call(0xb389, 0x3fa7)?;
        c.run(0x3fa7, 1)?;
        c.call(0xe283, 0x3faa)?;
    }
    c.run(0x3faa, 3)?;
    let dl = c.reg8(r8::DL);
    c.set_byte(0x0c, dl);
    c.set_byte(0x23, 1);
    c.call(0xa1c4, 0x3fb6)?;
    c.run(0x3fb6, 3)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.call(0x36d3, 0x3fbb)?;
    c.run(0x3fbb, 3)?;
    let v = c.pop();
    c.set_reg(DX, v);
    let v = c.pop();
    c.set_reg(BX, v);
    c.call(0xa1e2, 0x3fc0)?;
    if !c.flag(pc::cpu::ZF) {
        c.run(0x3fc0, 2)?;
        return c.ret();
    }
    c.run(0x3fc0, 4)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.call(0xabd5, 0x3fc8)?;
    c.run(0x3fc8, 4)?;
    let v = c.pop();
    c.set_reg(DX, v);
    let v = c.pop();
    c.set_reg(BX, v);
    c.set_byte(0x23, 5);
    enter_room_at(&mut c)
}

/// `loc_03fd2`: out of the place by the exit -DL: [0E7h] 0, BP the
/// direction, the place [114Eh] taken (0 left) and its position and
/// map cell in DX / BX, [8] and [9] FFh; then the move (`loc_03ff5`).
fn leave_place(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x3fd2, 11)?;
    c.set_byte(0xe7, 0);
    let dl = c.reg8(r8::DL);
    let dh = c.reg8(r8::DH);
    c.xor(B, dh as u16, dh as u16);
    let n = c.neg(B, dl as u16);
    c.set_reg(DX, n);
    c.set_reg(BP, n);
    c.xor(W, 0, 0);
    let si = c.word(0x114e);
    c.set_word(0x114e, 0);
    c.set_reg(SI, si);
    let dx = c.word(si.wrapping_add(2));
    let bx = c.word(si.wrapping_add(4));
    c.set_reg(DX, dx);
    c.xor(B, bx >> 8, bx >> 8);
    c.set_reg(BX, bx & 0xff);
    c.set_byte(8, 0xff);
    c.set_byte(9, 0xff);
    step_3ff5(c)
}

/// `loc_03ff5`: the move BP from the position DX / BX: the target
/// (`loc_0b5cf` with the direction's word at 1454h); a place of the
/// map there (`loc_0b532` bit 40h, `loc_0409a`) at the same position
/// becomes the current one ([114Eh], [1150h], `loc_0503c`, the counts
/// [98h] / [9Ah] cleared, `loc_0425b`, `loc_040ae`; [8] / [9] its room
/// and position, its bit 10h for the sietches and palaces, `loc_02170`);
/// then `loc_04057`.
fn step_3ff5(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x3ff5, 3)?;
    let bp = c.reg(BP);
    let bp = c.shl1(W, bp, 1);
    c.set_reg(BP, bp);
    let ax = c.word_in(SS, bp.wrapping_add(0x1454));
    c.set_reg(AX, ax);
    c.call(0xb5cf, 0x3ffe)?;
    let bh = c.reg8(r8::BH);
    if bh != 0 {
        c.run(0x3ffe, 2)?;
        c.or(B, bh as u16, bh as u16);
        return enter_room_at(c);
    }
    c.run(0x3ffe, 6)?;
    c.or(B, 0, 0);
    let x = c.reg8(r8::BL) as i8 as i16 as u16;
    c.set_reg(AX, x);
    c.set_reg(BX, x);
    c.call(0xb532, 0x400a)?;
    let al = c.reg8(r8::AL);
    let bx = c.reg(BX);
    if al & 0x40 == 0 {
        c.run(0x400a, 3)?;
    } else {
        c.run(0x400a, 4)?;
    }
    c.xor(B, bx >> 8, bx >> 8);
    c.set_reg(BX, bx & 0xff);
    c.test(B, al as u16, 0x40);
    if al & 0x40 == 0 {
        return enter_room_at(c);
    }
    c.call(0x409a, 0x4013)?;
    if !c.flag(pc::cpu::ZF) {
        c.run(0x4013, 1)?;
        return enter_room_at(c);
    }
    let (dx, si) = (c.reg(DX), c.reg(SI));
    let w = c.word(si.wrapping_add(2));
    if dx != w {
        c.run(0x4013, 3)?;
        c.cmp(W, dx, w);
        return enter_room_at(c);
    }
    c.run(0x4013, 11)?;
    c.cmp(W, dx, w);
    let x = c.reg8(r8::BL) as i8 as i16 as u16;
    c.set_reg(AX, x);
    c.set_reg(BX, x);
    c.set_byte(0x4735, 0);
    c.set_word(0x114e, si);
    c.set_word(0x1150, si);
    c.set_reg(DI, si);
    c.call(0x503c, 0x4031)?;
    c.run(0x4031, 3)?;
    c.set_word(0x9a, 0);
    c.set_word(0x98, 0);
    c.call(0x425b, 0x4040)?;
    c.run(0x4040, 1)?;
    c.call(0x40ae, 0x4043)?;
    let dh = c.reg8(r8::DH);
    let bh = c.reg8(r8::BH);
    let si = c.reg(SI);
    c.run(0x4043, 4 + (dh >= 0x20) as u32 + 1)?;
    c.set_byte(8, dh);
    c.set_byte(9, bh);
    c.cmp(B, dh as u16, 0x20);
    if dh >= 0x20 {
        let v = c.byte(si.wrapping_add(0x0a));
        let v = c.or(B, v as u16, 0x10);
        c.set_byte(si.wrapping_add(0x0a), v as u8);
    }
    c.call(0x2170, 0x4057)?;
    enter_room_at(c)
}

/// `loc_037b2` / `loc_037b5`: the room drawn (from `loc_037b2` the
/// dialogue reset first): `loc_04d00`, [472Dh] 0, `loc_05ba8`,
/// `loc_0c432`; at a place the room's scene byte (`loc_03efe`): a
/// scene (positive) is drawn by `loc_039ec`. Else `loc_03ae9`, [47A4h]
/// bit 0; outside the map modes the place's view (`loc_0380c`,
/// `place_scene`, `loc_04d06`); in them the map ([4728h] 0, `loc_04988`,
/// `loc_04a5a`, the resource [487Eh] through the driver) and the sky
/// (`set_sky_palette`).
pub fn draw_room(m: &mut Machine, start: u16) -> Result<(), Bail> {
    let mut c = Ctx::new(m);
    match start {
        0x37b2 => {
            c.run(0x37b2, 1)?;
            c.call(0x98e6, 0x37b5)?;
        }
        0x37b5 => {}
        _ => return Err(Bail),
    }
    c.run(0x37b5, 1)?;
    c.call(0x4d00, 0x37b8)?;
    c.run(0x37b8, 2)?;
    c.set_word(0x472d, 0);
    c.call(0x5ba8, 0x37c1)?;
    c.run(0x37c1, 1)?;
    c.call(0xc432, 0x37c4)?;
    let b8 = c.byte(8);
    if b8 == 0xff {
        c.run(0x37c4, 6)?;
        c.set_reg(AX, 0xffff);
        c.cmp(B, b8 as u16, 0xff);
        c.or(W, 0xffff, 0xffff);
    } else {
        c.run(0x37c4, 5)?;
        c.set_reg(AX, 0xffff);
        c.cmp(B, b8 as u16, 0xff);
        let dx = c.word(4);
        c.set_reg(DX, dx);
        c.call(0x3efe, 0x37d4)?;
        let ah = c.reg8(r8::AH);
        let si = c.reg(SI);
        let al = c.byte(si);
        let ax = (ah as u16) << 8 | al as u16;
        c.run(0x37d4, 4)?;
        c.lods(B, DS);
        c.or(W, ax, ax);
        if ax & 0x8000 == 0 {
            return room_scene_39ec(&mut c);
        }
    }
    c.call(0x3ae9, 0x37df)?;
    let mode = c.byte(0x11c9);
    let v = c.byte(0x47a4);
    if mode & 3 == 0 {
        c.run(0x37df, 4)?;
        let v = c.or(B, v as u16, 1);
        c.set_byte(0x47a4, v as u8);
        c.test(B, mode as u16, 3);
        c.call(0x380c, 0x37ee)?;
        c.run(0x37ee, 1)?;
        c.call(0x4e12, 0x37f1)?;
        c.run(0x37f1, 1)?;
        return enemy_watch_4d06(&mut c);
    }
    c.run(0x37df, 5)?;
    let v = c.or(B, v as u16, 1);
    c.set_byte(0x47a4, v as u8);
    c.test(B, mode as u16, 3);
    c.set_byte(0x4728, 0);
    c.call(0x4988, 0x37fc)?;
    c.run(0x37fc, 1)?;
    c.call(0x4a5a, 0x37ff)?;
    c.run(0x37ff, 2)?;
    let ax = c.word(0x487e);
    c.set_reg(AX, ax);
    c.call(0xca1b, 0x3805)?;
    c.interp(0x3805, 0x3809)?;
    c.run(0x3809, 1)?;
    super::ui::set_sky_palette(c.m, 0x388d)
}

/// `loc_04d06`: [0F6h] 0; when the place [1150h] is here ([4], its +4
/// against [6]) and [4734h] is set and not below the position's high
/// byte, the troops' march shown ([0F6h] 1, `loc_04d57` with AH 1,
/// `loc_04bdf`, `loc_0c412`) and past [4733h] the alarm (`loc_0ab15`
/// with 5, `loc_04d57` with AH 0, the task 4BB9h every 10h).
fn enemy_watch_4d06(c: &mut Ctx) -> Result<(), Bail> {
    let di = c.word(0x1150);
    let dx = c.word(di.wrapping_add(2));
    let here = c.word(4);
    let ax = c.word(0x4733);
    let ah = (ax >> 8) as u8;
    let bx = c.word(6);
    let b4 = c.byte(di.wrapping_add(4));
    let bh = (bx >> 8) as u8;
    let n = if dx != here {
        5
    } else if ah == 0 {
        8
    } else if bx as u8 != b4 {
        11
    } else if bh > ah {
        13
    } else {
        18
    };
    c.run(0x4d06, n + (n < 18) as u32)?;
    c.set_byte(0xf6, 0);
    c.set_reg(DI, di);
    c.set_reg(DX, dx);
    c.cmp(W, dx, here);
    if n > 5 {
        c.set_reg(AX, ax);
        c.or(B, ah as u16, ah as u16);
    }
    if n > 8 {
        c.set_reg(BX, bx);
        c.cmp(B, bx & 0xff, b4 as u16);
    }
    if n > 11 {
        c.cmp(B, bh as u16, ah as u16);
    }
    if n < 18 {
        return c.ret();
    }
    c.push(ax);
    c.push(bx);
    let f = c.byte(0xf6);
    let f = c.inc(B, f as u16);
    c.set_byte(0xf6, f as u8);
    c.set_reg8(r8::AH, 1);
    c.call(0x4d57, 0x4d37)?;
    c.run(0x4d37, 1)?;
    c.call(0x4bdf, 0x4d3a)?;
    c.run(0x4d3a, 1)?;
    c.call(0xc412, 0x4d3d)?;
    let sp = c.reg(SP);
    let bx = c.word_in(SS, sp);
    let ax = c.word_in(SS, sp.wrapping_add(2));
    let (bh, al) = ((bx >> 8) as u8, ax as u8);
    c.run(0x4d3d, if bh > al { 5 } else { 6 })?;
    c.pop();
    c.pop();
    c.set_reg(BX, bx);
    c.set_reg(AX, ax);
    c.cmp(B, bh as u16, al as u16);
    if bh > al {
        return c.ret();
    }
    c.set_reg8(r8::AL, 5);
    c.call(0xab15, 0x4d48)?;
    c.run(0x4d48, 2)?;
    c.xor(B, 0, 0);
    c.set_reg8(r8::AH, 0);
    c.call(0x4d57, 0x4d4d)?;
    c.run(0x4d4d, 3)?;
    c.set_reg(SI, 0x4bb9);
    c.set_reg(BP, 0x10);
    c.call(0xda25, 0x4d56)?;
    c.run(0x4d56, 1)?;
    c.ret()
}

/// `loc_039ec`: the room's scene AX: [22E3h] 1, `loc_03ae9`; the room
/// [4] 2005h draws the sky (`draw_sky`), 1005h too without [22E3h];
/// the first room of a place ([4] low 1) with the place's view
/// (`loc_037eb`), in a palace (21h) with the place's own scene; then
/// the scene (`draw_SAL`). In a lit first room with [4731h] not FFh
/// and [46FFh] figures: [4731h] 0 and, unless [47A4h] & 81h, the figures
/// drawn (`loc_0c13e` 33h, `loc_03a95`, `loc_03aa9` each 46h / 0Ah on)
/// before the sky (`set_sky_palette`); else the room frame task removed
/// and in a sietch's room 4 the task C0B6h every 0Ch.
fn room_scene_39ec(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x39ec, 3)?;
    c.set_byte(0x22e3, 1);
    let ax = c.reg(AX);
    c.push(ax);
    c.call(0x3ae9, 0x39f5)?;
    let r = c.word(4);
    let mut n = 3u32;
    let mut redo = false;
    let mut sky = false;
    let mut eb = false;
    let mut clear = false;
    if r == 0x2005 {
        sky = true;
    } else {
        n += 2;
        if r == 0x1005 {
            clear = true;
            sky = true;
        } else {
            n += 2;
            if (r as u8).wrapping_sub(1) == 0 {
                n += 2;
                eb = true;
                if (r >> 8) as u8 == 0x21 {
                    n += 4;
                    redo = true;
                }
            }
        }
    }
    n += if eb { 1 } else if clear { 2 } else if sky { 1 } else { 0 };
    c.run(0x39f5, n)?;
    c.set_reg(AX, r);
    c.cmp(W, r, 0x2005);
    if r != 0x2005 {
        c.cmp(W, r, 0x1005);
        if r != 0x1005 {
            let al = c.dec(B, r & 0xff);
            c.set_reg8(r8::AL, al as u8);
            if al == 0 {
                c.cmp(B, r >> 8, 0x21);
            }
        }
    }
    if redo {
        let mut v = c.pop();
        let di = c.word(0x114e);
        c.set_reg(DI, di);
        v = (v & 0xff00) | c.byte(di) as u16;
        c.set_reg(AX, v);
        c.push(v);
    }
    if eb {
        c.call(0x37eb, 0x3a16)?;
        c.run(0x3a16, 1)?;
    } else if sky {
        if clear {
            c.set_byte(0x22e3, 0);
        }
        c.call(0x38b4, 0x3a20)?;
    }
    c.run(0x3a20, 2)?;
    let v = c.pop();
    c.set_reg(AX, v);
    c.call(0x3b59, 0x3a24)?;
    figures_3a24(c)
}

fn figures_3a24(c: &mut Ctx) -> Result<(), Bail> {
    let lit = c.byte(0x46df);
    let r = c.byte(4);
    if lit == 0 || r != 1 {
        c.run(0x3a24, if lit == 0 { 3 } else { 5 })?;
        c.cmp(B, lit as u16, 0);
        if lit != 0 {
            c.cmp(B, r as u16, 1);
        }
        c.call(0x39e6, 0x3a7f)?;
        let ax = c.word(4);
        let (al, ah) = (ax as u8, (ax >> 8) as u8);
        let go = al == 4 && ah < 0x20;
        c.run(0x3a7f, if al != 4 { 4 } else if !go { 6 } else { 8 })?;
        c.set_reg(AX, ax);
        c.cmp(B, al as u16, 4);
        if al == 4 {
            c.cmp(B, ah as u16, 0x20);
        }
        if !go {
            return c.ret();
        }
        c.set_reg(SI, 0xc0b6);
        c.set_reg(BP, 0x0c);
        c.call(0xda25, 0x3a94)?;
        c.run(0x3a94, 1)?;
        return c.ret();
    }
    let k = c.byte(0x4731);
    if k == 0xff {
        c.run(0x3a24, 7)?;
        c.cmp(B, lit as u16, 0);
        c.cmp(B, r as u16, 1);
        c.cmp(B, k as u16, 0xff);
        return c.ret();
    }
    let cl = c.byte(0x46ff);
    if cl == 0 {
        c.run(0x3a24, 10)?;
        c.cmp(B, lit as u16, 0);
        c.cmp(B, r as u16, 1);
        c.cmp(B, k as u16, 0xff);
        c.set_reg(CX, 0);
        c.xor(B, 0, 0);
        return c.ret();
    }
    let f = c.byte(0x47a4);
    let skip = f & 0x81 != 0;
    c.run(0x3a24, 6 + 3 + 5 + if skip { 1 } else { 2 })?;
    c.cmp(B, lit as u16, 0);
    c.cmp(B, r as u16, 1);
    c.cmp(B, k as u16, 0xff);
    c.set_reg(CX, cl as u16);
    c.xor(B, 0, 0);
    c.set_reg(AX, 0x388d);
    c.push(0x388d);
    c.set_byte(0x4731, 0);
    c.test(B, f as u16, 0x81);
    if skip {
        c.pop();
        return super::ui::set_sky_palette(c.m, 0x388d);
    }
    c.set_reg(AX, 0x33);
    c.call(0xc13e, 0x3a57)?;
    c.run(0x3a57, 1)?;
    c.call(0x3a95, 0x3a5a)?;
    c.run(0x3a5a, 6)?;
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let a = c.add(W, dx, 0x0c);
    c.set_word(0x472d, a);
    let a = c.add(W, bx, 8);
    c.set_reg(AX, a);
    c.set_word(0x472f, a);
    loop {
        c.run(0x3a6a, 4)?;
        let (cx, bx, dx) = (c.reg(CX), c.reg(BX), c.reg(DX));
        c.push(cx);
        c.push(bx);
        c.push(dx);
        c.call(0x3aa9, 0x3a70)?;
        let sp = c.reg(SP);
        let dx = c.word_in(SS, sp);
        let cx = c.word_in(SS, sp.wrapping_add(4));
        c.run(0x3a70, 6 + (cx == 1) as u32)?;
        let dx2 = c.pop();
        let bx = c.pop();
        let cx = c.pop();
        let _ = dx;
        let d = c.add(W, dx2, 0x46);
        c.set_reg(DX, d);
        let b = c.add(W, bx, 0x0a);
        c.set_reg(BX, b);
        let cx = cx.wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.pop();
    super::ui::set_sky_palette(c.m, 0x388d)
}

/// `loc_037eb`: the place's view (`loc_0380c`, `place_scene`) and the
/// troops' watch (`loc_04d06`).
pub fn place_view_37eb(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x37eb)?;
    let mut c = Ctx::new(m);
    c.run(0x37eb, 1)?;
    c.call(0x380c, 0x37ee)?;
    c.run(0x37ee, 1)?;
    c.call(0x4e12, 0x37f1)?;
    c.run(0x37f1, 1)?;
    enemy_watch_4d06(&mut c)
}

/// `loc_01b0d`: the clock's step: with no voice playing
/// (`is_voc_pcm_playing`), [2788h] clear and [2Ah] below C8h the time
/// moves (`loc_02b2a`); then `loc_01b23`.
pub fn time_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    let mut c = Ctx::new(m);
    if start == 0x1b23 {
        return new_time_1b23(&mut c);
    }
    entered(start, 0x1b0d)?;
    c.run(0x1b0d, 1)?;
    c.call(0xabcc, 0x1b10)?;
    if !c.flag(pc::cpu::ZF) {
        c.run(0x1b10, 2)?;
        return c.ret();
    }
    let k = c.byte(0x2788);
    let t = c.byte(0x2a);
    if k != 0 {
        c.run(0x1b10, 4)?;
        c.cmp(B, k as u16, 0);
        return c.ret();
    }
    if t >= 0xc8 {
        c.run(0x1b10, 6)?;
        c.cmp(B, 0, 0);
        c.cmp(B, t as u16, 0xc8);
        return c.ret();
    }
    c.run(0x1b10, 6)?;
    c.cmp(B, 0, 0);
    c.cmp(B, t as u16, 0xc8);
    c.call(0x2b2a, 0x1b23)?;
    new_time_1b23(&mut c)
}

/// `loc_01b23`: on a new time slot ([46DDh], cleared): [0F4h] counts
/// down (from 10h on; below it 0, and [0F5h] 0), the date panel and the
/// sky (`loc_01a0f`, `loc_038e1`); the hour [2] noted in [1174h], its
/// change ([46DEh]) handled (`loc_01c46`). Before the stage 7 ([0C2h])
/// the day's events: `loc_01d9f`, `loc_06c6f`, `loc_063f0`, the
/// handler for `loc_01ae0` (the word table 1DB3h), `loc_01c18`,
/// `loc_0331e` with the saved [11CEh] ([11F7h] kept), `loc_01bec`;
/// unless [46D9h] the map view refreshed ([46ECh]) and at a place
/// [473Bh] acted on: positive (and [0FBh] not negative, no [46DAh]) the
/// room left and redrawn (`loc_00b21`, `loc_02db1`); negative the map
/// refreshed, or the view rebuilt (`loc_0d2bd`, the gauges). [473Bh] 0.
fn new_time_1b23(c: &mut Ctx) -> Result<(), Bail> {
    let f = c.byte(0x46dd);
    if f == 0 {
        c.run(0x1b23, 3)?;
        c.cmp(B, 0, 0);
        return c.ret();
    }
    let a = c.byte(0xf4).wrapping_sub(1);
    let low = (a as i8) < 0x10;
    c.run(0x1b23, 7 + if low { 2 } else { 0 } + 2)?;
    c.cmp(B, f as u16, 0);
    c.set_byte(0x46dd, 0);
    let f4 = c.byte(0xf4);
    let a = c.dec(B, f4 as u16) as u8;
    c.cmp(B, a as u16, 0x10);
    let a = if low {
        c.xor(B, a as u16, a as u16);
        c.set_byte(0xf5, 0);
        0
    } else {
        a
    };
    c.set_reg8(r8::AL, a);
    c.set_byte(0xf4, a);
    c.call(0x1a0f, 0x1b43)?;
    c.run(0x1b43, 1)?;
    c.call(0x38e1, 0x1b46)?;
    let now = c.word(2);
    let old = c.word(0x1174);
    let d = (old as u8 & 0xf0).wrapping_sub(now as u8 & 0xf0);
    c.run(0x1b46, 8 + (d != 0) as u32)?;
    c.set_word(0x1174, now);
    c.and(B, old & 0xff, 0xf0);
    let cl = c.and(B, now & 0xff, 0xf0);
    c.set_reg(CX, (now & 0xff00) | cl);
    c.sub(B, (old & 0xf0) as u16, cl);
    c.set_reg(AX, (old & 0xff00) | d as u16);
    c.set_byte(0x46de, d);
    if d != 0 {
        c.call(0x1c46, 0x1b5e)?;
    }
    let st = c.byte(0xc2);
    if st >= 7 {
        c.run(0x1b5e, 2)?;
        c.cmp(B, st as u16, 7);
        return clear_473b(c);
    }
    c.run(0x1b5e, 3)?;
    c.cmp(B, st as u16, 7);
    c.call(0x1d9f, 0x1b68)?;
    c.run(0x1b68, 3)?;
    let (a, b) = (c.word(0x11f7), c.word(0x11ce));
    c.push(a);
    c.push(b);
    c.call(0x6c6f, 0x1b73)?;
    c.run(0x1b73, 1)?;
    c.call(0x63f0, 0x1b76)?;
    c.run(0x1b76, 1)?;
    c.call(0x1ae0, 0x1b79)?;
    c.run(0x1b79, 3)?;
    let ax = c.reg(AX);
    let si = c.shl1(W, ax, 1);
    c.set_reg(AX, si);
    c.set_reg(SI, si);
    let to = c.code_word(si.wrapping_add(0x1db3));
    c.call(to, 0x1b82)?;
    c.run(0x1b82, 1)?;
    c.call(0x1c18, 0x1b85)?;
    c.run(0x1b85, 2)?;
    let di = c.pop();
    c.set_reg(DI, di);
    c.call(0x331e, 0x1b89)?;
    c.run(0x1b89, 2)?;
    let v = c.pop();
    c.set_word(0x11f7, v);
    c.call(0x1bec, 0x1b90)?;
    let k = c.byte(0x46d9);
    if k != 0 {
        c.run(0x1b90, 2)?;
        c.cmp(B, k as u16, 0);
        return clear_473b(c);
    }
    let e = c.byte(0x46ec);
    c.run(0x1b90, 4 + (e != 0) as u32)?;
    c.cmp(B, 0, 0);
    c.cmp(B, e as u16, 0);
    if e != 0 {
        c.call(0x5d6d, 0x1ba1)?;
    }
    let di = c.word(0x114e);
    if di == 0 {
        c.run(0x1ba1, 3)?;
        c.set_reg(DI, 0);
        c.or(W, 0, 0);
        return clear_473b(c);
    }
    let v = c.byte(0x473b);
    c.run(0x1ba1, 3 + if v & 0x80 != 0 { 2 } else { 3 })?;
    c.set_reg(DI, di);
    c.or(W, di, di);
    c.cmp(B, v as u16, 0);
    if v == 0 {
        return clear_473b(c);
    }
    let fb = c.byte(0xfb);
    if v & 0x80 != 0 {
        c.run(0x1bd2, 1)?;
        c.call(0x1bb2, 0x1bd5)?;
        let fb = c.byte(0xfb);
        if fb & 0x80 != 0 {
            c.run(0x1bd5, 3)?;
            c.cmp(B, fb as u16, 0);
            return super::map::refresh_map_view(c.m, 0x5d6d);
        }
        let da = c.byte(0x46da);
        if da != 0 {
            c.run(0x1bd5, 4)?;
            c.cmp(B, fb as u16, 0);
            c.cmp(B, da as u16, 0);
            return clear_473b(c);
        }
        c.run(0x1bd5, 5)?;
        c.cmp(B, fb as u16, 0);
        c.cmp(B, 0, 0);
        c.call(0xd2bd, 0x1be6)?;
        c.run(0x1be6, 1)?;
        c.run(0x0fa7, 1)?;
        c.call(0xdbb2, 0x0faa)?;
        c.run(0x0faa, 2)?;
        c.set_reg8(r8::AL, 0x2a);
        c.call(0x189a, 0x0faf)?;
        c.run(0x0faf, 1)?;
        return super::screens::draw_gauges(c.m, 0xd763);
    }
    if fb & 0x80 != 0 {
        c.run(0x1bb8, 2)?;
        c.cmp(B, fb as u16, 0);
        return clear_473b(c);
    }
    let da = c.byte(0x46da);
    if da != 0 {
        c.run(0x1bb8, 4)?;
        c.cmp(B, fb as u16, 0);
        c.cmp(B, da as u16, 0);
        return clear_473b(c);
    }
    c.run(0x1bb8, 5)?;
    c.cmp(B, fb as u16, 0);
    c.cmp(B, 0, 0);
    c.call(0xdbb2, 0x1bc9)?;
    c.run(0x1bc9, 1)?;
    c.call(0x1bb2, 0x1bcc)?;
    c.run(0x1bcc, 1)?;
    c.call(0x0b21, 0x1bcf)?;
    c.run(0x1bcf, 1)?;
    room_arrive(c.m, 0x2db1)
}

/// `loc_01bb2` reached by a jump: [473Bh] 0, `ret`.
fn clear_473b(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x1bb2, 2)?;
    c.set_byte(0x473b, 0);
    c.ret()
}

/// `loc_03542` / `loc_03551`: the scene's record (from `loc_03542` the
/// events for [47C4h] at the place dropped, `loc_02a51`, and [0E7h] 0):
/// [47A7h] counts; stage 3 ([23h]) installs the list 1F92h (handler
/// 97CFh), stage 4 the list 1F9Eh (bit 40h dropped) and the place's
/// buttons, both with [1BEAh] / [1BF8h] 0; else outside [4774h] and
/// below stage 64h the phrase [47C4h] (`loc_093df`) and the fade.
pub fn scene_record(m: &mut Machine, start: u16) -> Result<(), Bail> {
    let mut c = Ctx::new(m);
    match start {
        0x3542 => scene_record_3542(&mut c),
        0x3551 => scene_record_3551(&mut c),
        _ => Err(Bail),
    }
}

fn scene_record_3542(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x3542, 3)?;
    let ax = c.word(0x47c4);
    c.set_reg(AX, ax);
    let di = c.word(0x114e);
    c.set_reg(DI, di);
    c.call(0x2a51, 0x354c)?;
    c.run(0x354c, 1)?;
    c.set_byte(0xe7, 0);
    scene_record_3551(c)
}

fn scene_record_3551(c: &mut Ctx) -> Result<(), Bail> {
    let n = c.byte(0x47a7);
    let st = c.byte(0x23);
    let count = |c: &mut Ctx| {
        let v = c.inc(B, n as u16);
        c.set_byte(0x47a7, v as u8);
        c.cmp(B, st as u16, 3);
    };
    if st == 3 {
        c.run(0x3551, 6)?;
        count(c);
        c.set_reg(BP, 0x1f92);
        c.set_reg(BX, 0x97cf);
        c.call(0xd323, 0x3565)?;
        c.run(0x3565, 3)?;
        c.set_word(0x1bea, 0);
        c.set_word(0x1bf8, 0);
        return c.ret();
    }
    if st == 4 {
        c.run(0x3551, 9)?;
        count(c);
        c.cmp(B, 4, 4);
        c.set_reg(BP, 0x1f9e);
        let v = c.byte_in(SS, 0x1fa9);
        let v = c.and(B, v as u16, 0xbf);
        c.m.cpu.write8(&mut c.m.hw, SS, 0x1fa9, v as u8);
        c.set_reg(BX, 0x97cf);
        c.call(0xd323, 0x3586)?;
        c.run(0x3586, 3)?;
        c.set_word(0x1bea, 0);
        c.set_word(0x1bf8, 0);
        return super::screens::panel_buttons(c.m, 0x2ffb);
    }
    let k = c.byte(0x4774);
    let go = k == 0 && st < 0x64;
    c.run(0x3551, 5 + if k != 0 { 3 } else if !go { 5 } else { 6 })?;
    count(c);
    c.cmp(B, st as u16, 4);
    c.cmp(B, k as u16, 0);
    if k == 0 {
        c.cmp(B, st as u16, 0x64);
    }
    if !go {
        return c.ret();
    }
    let ax = c.word(0x47c4);
    c.set_reg(AX, ax);
    c.call(0x93df, 0x35a9)?;
    c.run(0x35a9, 1)?;
    c.call(0xd280, 0x35ac)?;
    c.run(0x35ac, 1)?;
    c.ret()
}

/// `loc_02b2a`: the clock in the room view (nothing on [46D9h], [4774h]
/// or a map mode; the list 1F0Eh current, [0FBh] not negative): AX the
/// time [0CE7Ah]. Past the hour 14h ([2Ah]) with events ([1190h]) and
/// 32h since [0DC5Ah]: an event to tell (`loc_02ad8`, CF) starts its
/// scene (`loc_03542`, stage 0); else by day the light dims (28h from
/// 96h, 26h from 15Eh) and from 1C2h (or at night) the next event is
/// shown. At 14h exactly, outside a place and with [10h] 0, every 3E8h
/// the time moves on 3B6h (`loc_01071`, `loc_0b17a`) and the clock steps
/// again until [0EAh] is FFh. An event ([1191h]): its kind in [0EAh]
/// ([0EBh] for 1), its phrase below 10h (0Eh needs a speaker), the
/// speaker [1193h] placed (`loc_0331e`, `loc_02e98`; from phrase 0Eh
/// with a figure `loc_06906`), the talk (`loc_02c92`, `loc_09ef1`), the
/// list shifted, and the view reset before `loc_0c412`.
pub fn clock_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2b2a)?;
    let mut c = Ctx::new(m);
    let (a, b, d) = (c.byte(0x46d9), c.byte(0x4774), c.byte(0x11c9));
    if a | b | d != 0 {
        c.run(0x2b2a, 5)?;
        flags_2b2a(&mut c, a, b, d);
        return c.ret();
    }
    c.run(0x2b2a, 5)?;
    flags_2b2a(&mut c, a, b, d);
    c.call(0xd41b, 0x2b3a)?;
    let bp = c.reg(BP);
    if bp != 0x1f0e {
        c.run(0x2b3a, 3)?;
        c.cmp(W, bp, 0x1f0e);
        return c.ret();
    }
    let fb = c.byte(0xfb);
    if fb & 0x80 != 0 {
        c.run(0x2b3a, 5)?;
        c.cmp(W, bp, 0x1f0e);
        c.cmp(B, fb as u16, 0);
        return c.ret();
    }
    let t = c.word(0xce7a);
    let h = c.byte(0x2a);
    let pre = |c: &mut Ctx| {
        c.cmp(W, bp, 0x1f0e);
        c.cmp(B, fb as u16, 0);
        c.set_reg(AX, t);
        c.cmp(B, h as u16, 0x14);
    };
    if h < 0x14 {
        c.run(0x2b3a, 8)?;
        pre(&mut c);
        return c.ret();
    }
    let last = c.word(0xdc5a);
    if h == 0x14 {
        let w10 = c.word(0x10);
        let b8 = c.byte(8);
        let dt = t.wrapping_sub(last);
        let n = if w10 != 0 { 2 } else if b8 != 0xff { 4 } else if dt < 0x3e8 { 7 } else { 9 };
        c.run(0x2b3a, 8 + n + (n < 9) as u32)?;
        pre(&mut c);
        c.cmp(W, w10, 0);
        if w10 == 0 {
            c.cmp(B, b8 as u16, 0xff);
            if b8 == 0xff {
                c.sub(W, t, last);
                c.set_reg(AX, dt);
                c.cmp(W, dt, 0x3e8);
            }
        }
        if n < 9 {
            return c.ret();
        }
        let v = c.add(W, last, 0x3b6);
        c.set_word(0xdc5a, v);
        c.call(0x1071, 0x2bc1)?;
        c.run(0x2bc1, 1)?;
        c.call(0xb17a, 0x2bc4)?;
        loop {
            c.run(0x2bc4, 1)?;
            c.call(0x2b2a, 0x2bc7)?;
            let e = c.byte(0xea);
            c.run(0x2bc7, 2 + (e == 0xff) as u32)?;
            c.cmp(B, e as u16, 0xff);
            if e == 0xff {
                return c.ret();
            }
        }
    }
    let ev = c.byte(0x1190);
    if ev == 0 {
        c.run(0x2b3a, 11)?;
        pre(&mut c);
        c.cmp(B, 0, 0);
        return c.ret();
    }
    let dt = t.wrapping_sub(last);
    if dt < 0x32 {
        c.run(0x2b3a, 14)?;
        pre(&mut c);
        c.cmp(B, ev as u16, 0);
        c.sub(W, t, last);
        c.set_reg(AX, dt);
        c.cmp(W, dt, 0x32);
        return c.ret();
    }
    c.run(0x2b3a, 14)?;
    pre(&mut c);
    c.cmp(B, ev as u16, 0);
    c.sub(W, t, last);
    c.set_reg(AX, dt);
    c.cmp(W, dt, 0x32);
    c.call(0x2ad8, 0x2b66)?;
    if c.flag(CF) {
        c.run(0x2b66, 3)?;
        c.set_byte(0x23, 0);
        return scene_record(c.m, 0x3542);
    }
    let night = c.byte(0x2b);
    let ax = c.reg(AX);
    if night == 0 {
        // The dimming steps: 28h from 96h, 26h from 15Eh, both below
        // the next.
        let (n, bl) = if ax < 0x96 {
            (5, Some(0x28u8))
        } else if ax < 0xfa {
            (8, None)
        } else if ax < 0x15e {
            (10, Some(0x26))
        } else if ax >= 0x1c2 {
            (12, None)
        } else {
            (13, None)
        };
        let ret_here = n == 8 || n == 13;
        c.run(0x2b66, 1 + n)?;
        c.cmp(B, 0, 0);
        c.set_reg8(r8::BL, 0x28);
        c.cmp(W, ax, 0x96);
        if ax >= 0x96 {
            c.cmp(W, ax, 0xfa);
            if ax >= 0xfa {
                c.cmp(W, ax, 0x15e);
                c.set_reg8(r8::BL, 0x26);
                if ax >= 0x15e {
                    c.cmp(W, ax, 0x1c2);
                }
            }
        }
        if ret_here {
            return c.ret();
        }
        if let Some(bl) = bl {
            c.run(0x2b90, 2)?;
            let bx = (c.reg(BX) & 0xff00) | bl as u16;
            c.push(bx);
            c.call(0xdbb2, 0x2b94)?;
            c.run(0x2b94, 1)?;
            c.call(0xc49a, 0x2b97)?;
            c.run(0x2b97, 3)?;
            let v = c.pop();
            c.set_reg(AX, v);
            c.set_reg(BP, 0x0f66);
            c.call(0xc108, 0x2b9e)?;
            c.run(0x2b9e, 1)?;
            return super::mouse::draw_mouse(c.m, 0xdbec);
        }
    } else {
        c.run(0x2b66, 3)?;
        c.cmp(B, night as u16, 0);
    }
    next_event_2bd2(&mut c)
}

fn flags_2b2a(c: &mut Ctx, a: u8, b: u8, d: u8) {
    c.set_reg8(r8::AL, a);
    let x = c.or(B, a as u16, b as u16);
    let x = c.or(B, x, d as u16);
    c.set_reg8(r8::AL, x as u8);
}

/// `loc_02bd2`: the next event ([1191h]) shown; see [`clock_step`].
fn next_event_2bd2(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x2bd2, 1)?;
    c.call(0xdbb2, 0x2bd5)?;
    c.run(0x2bd5, 1)?;
    c.call(0xad5e, 0x2bd8)?;
    let di = c.word(0x1193);
    if di == 0 {
        c.run(0x2bd8, 3)?;
        c.set_reg(DI, 0);
        c.or(W, 0, 0);
    } else {
        let a = di.wrapping_sub(0x100);
        let far = a >= 0x7aa;
        c.run(0x2bd8, 7 + if far { 2 } else { 1 })?;
        c.set_reg(DI, di);
        c.or(W, di, di);
        c.set_reg(AX, di);
        c.sub(W, di, 0x100);
        c.set_reg(AX, a);
        c.cmp(W, a, 0x7aa);
        if far {
            c.set_byte(0x1193, 0);
        } else {
            c.call(0x331e, 0x2bf4)?;
        }
    }
    let ev = c.word(0x1191);
    let (al, ah) = (ev as u8, (ev >> 8) as u8);
    let di = c.word(0x1193);
    let skip = ah >= 0x10 || (ah == 0x0e && di == 0);
    let n = 4 + (al == 1) as u32 + 5 + if ah >= 0x10 { 0 } else if ah != 0x0e { 2 } else { 4 };
    c.run(0x2bf4, n + if skip { 1 } else if di != 0 { 4 } else { 3 })?;
    c.set_reg(AX, ev);
    c.set_byte(0xea, al);
    c.cmp(B, al as u16, 1);
    if al == 1 {
        c.set_byte(0xeb, al);
    }
    c.set_reg(AX, ev & 0xff00 | ah as u16);
    c.xor(B, ah as u16, ah as u16);
    let ax = ah as u16;
    c.set_reg(AX, ax);
    c.set_reg(DI, di);
    c.cmp(W, ax, 0x10);
    if ah < 0x10 {
        c.cmp(B, ax, 0x0e);
        if ah == 0x0e {
            c.or(W, di, di);
        }
    }
    if skip {
        return shift_list_1191(c.m, 0x2a34);
    }
    c.set_word(0x47c4, ax);
    c.or(W, di, di);
    if di != 0 {
        c.call(0x331e, 0x2c20)?;
        c.run(0x2c20, 1)?;
        c.call(0x2e98, 0x2c23)?;
        let p = c.word(0x47c4);
        if p < 0x0e {
            c.run(0x2c23, 2)?;
            c.cmp(W, p, 0x0e);
        } else {
            let k = c.byte(0xea);
            let di = c.reg(DI);
            let f = c.byte(di.wrapping_add(9));
            let call = k == 0x0e || f != 0;
            c.run(0x2c23, 5 + if k == 0x0e { 0 } else { 3 } + call as u32)?;
            c.cmp(W, p, 0x0e);
            c.set_reg8(r8::AL, 3);
            c.cmp(B, k as u16, 0x0e);
            if k != 0x0e {
                c.set_reg8(r8::AL, f);
                c.or(B, f as u16, f as u16);
            }
            if call {
                c.call(0x6906, 0x2c3d)?;
                c.run(0x2c3d, 2)?;
                c.set_word(0x47c4, 0x0e);
                let si = c.reg(SI);
                c.set_word(0x4756, si);
            }
        }
    }
    c.run(0x2c47, 1)?;
    c.call(0x2c92, 0x2c4a)?;
    c.run(0x2c4a, 2)?;
    c.set_reg8(r8::AL, 1);
    c.call(0x9ef1, 0x2c4f)?;
    c.run(0x2c4f, 1)?;
    c.call(0x2a34, 0x2c52)?;
    c.run(0x2c52, 4)?;
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.set_byte(0x1f0f, 0);
    c.set_word(0x1f10, 0);
    c.call(0xd397, 0x2c5d)?;
    c.run(0x2c5d, 1)?;
    c.call(0xb2b9, 0x2c60)?;
    c.run(0x2c60, 2)?;
    c.set_reg(AX, 0x0bb8);
    c.call(0xddb0, 0x2c66)?;
    c.run(0x2c66, 1)?;
    c.call(0xb2b3, 0x2c69)?;
    c.run(0x2c69, 2)?;
    c.set_reg(SI, 0x2cc7);
    c.call(0xda5f, 0x2c6f)?;
    c.run(0x2c6f, 1)?;
    c.call(0x98e6, 0x2c72)?;
    c.run(0x2c72, 8)?;
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.set_word(0x479e, 0);
    c.set_word(0x4540, 0);
    c.set_byte(0xea, 0xff);
    c.set_byte(0xe8, 0x0a);
    c.set_word(0xdc30, 0);
    c.set_reg8(r8::AL, 6);
    c.call(0x189a, 0x2c8f)?;
    c.run(0x2c8f, 1)?;
    super::gfx::copy_active_framebuffer_to_framebuffer_2(c.m, 0xc412)
}

/// `draw_gradient_section_2`: the outline of a scene polygon's edge
/// (BX, DX) from the pen ([47E8h] x, [47EAh] y) into the edge list
/// [22D9h] (every other word): one point per row it crosses (rows
/// change by the sign of BX). A flat edge (BX 0) moves x and notes its
/// left end; a vertical one (DX 0) notes x for each of its BX + 1
/// rows; else a midpoint walk along the longer axis notes x before
/// each row step. Bails with DF set.
pub fn edge_points(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3e80)?;
    let mut c = Ctx::new(m);
    if c.flag(pc::cpu::DF) {
        return Err(Bail);
    }
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    let (di0, si0) = (c.reg(DI), c.reg(SI));
    let x = c.word(0x47e8);
    if bx == 0 {
        let (s, cf) = x.overflowing_add(dx);
        c.run(0x3e80, 4 + 5 + cf as u32 + 8)?;
        c.push(di0);
        c.push(si0);
        c.or(W, 0, 0);
        let y = c.word(0x47ea);
        c.set_reg(BX, y);
        c.set_reg(CX, dx);
        c.add(W, x, dx);
        c.set_word(0x47e8, s);
        let mut left = x;
        if cf {
            left = c.add(W, x, dx);
        }
        c.set_reg(DX, left);
        let p = c.word(0x22d9);
        c.set_reg(DI, p);
        c.set_reg(AX, left);
        c.stos(W);
        let p = c.reg(DI);
        let p = c.add(W, p, 2);
        c.set_reg(DI, p);
        c.set_word(0x22d9, p);
        let v = c.pop();
        c.set_reg(SI, v);
        let v = c.pop();
        c.set_reg(DI, v);
        return c.ret();
    }
    let bneg = bx & 0x8000 != 0;
    let (bxa, ax) = if bneg { (bx.wrapping_neg(), 0xffffu16) } else { (bx, 1) };
    let head = |c: &mut Ctx| {
        c.push(di0);
        c.push(si0);
        c.or(W, bx, bx);
        c.set_reg(AX, 1);
        if bneg {
            let b = c.neg(W, bx);
            c.set_reg(BX, b);
            c.neg(W, 1);
            c.set_reg(AX, 0xffff);
        }
    };
    if dx == 0 {
        let y = c.word(0x47ea);
        c.run(0x3e80, 4 + 2 + 2 * bneg as u32 + 2 + 5 + if bneg { 3 } else { 1 } + 3)?;
        head(&mut c);
        c.or(W, 0, 0);
        c.set_reg(CX, bxa);
        c.set_reg(BX, y);
        c.set_reg(DX, x);
        c.or(W, ax, ax);
        if bneg {
            let v = c.sub(W, y, bxa);
            c.set_word(0x47ea, v);
            let b = c.sub(W, y, bxa);
            c.set_reg(BX, b);
        } else {
            let v = c.add(W, y, bxa);
            c.set_word(0x47ea, v);
        }
        let n = c.inc(W, bxa);
        c.set_reg(CX, n);
        let p = c.word(0x22d9);
        c.set_reg(DI, p);
        c.set_reg(AX, x);
        let mut cx = n;
        loop {
            c.run(0x3e73, 3)?;
            c.stos(W);
            let p = c.reg(DI);
            let p = c.add(W, p, 2);
            c.set_reg(DI, p);
            cx = cx.wrapping_sub(1);
            c.set_reg(CX, cx);
            if cx == 0 {
                break;
            }
        }
        c.run(0x3e79, 4)?;
        let p = c.reg(DI);
        c.set_word(0x22d9, p);
        let v = c.pop();
        c.set_reg(SI, v);
        let v = c.pop();
        c.set_reg(DI, v);
        return c.ret();
    }
    let dneg = dx & 0x8000 != 0;
    let (dxa, cx) = if dneg { (dx.wrapping_neg(), 0xffffu16) } else { (dx, 1) };
    let steep = dxa > bxa;
    c.run(0x3e80, 4 + 2 + 2 * bneg as u32 + 2 + 2 + 2 * dneg as u32 + 10 + if steep { 2 } else { 4 } + 3)?;
    head(&mut c);
    c.or(W, dx, dx);
    c.set_reg(CX, 1);
    if dneg {
        c.neg(W, 1);
        c.set_reg(CX, 0xffff);
        c.neg(W, dx);
        c.set_reg(DX, dxa);
    }
    c.push(ax);
    c.push(cx);
    c.push(ax);
    c.push(cx);
    let bp = c.reg(SP);
    c.set_reg(BP, bp);
    let (mut si, mut di) = (bxa, dxa);
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.cmp(W, dxa, bxa);
    if steep {
        c.set_word_in(SS, bp.wrapping_add(2), 0);
    } else {
        c.or(W, bxa, bxa);
        std::mem::swap(&mut si, &mut di);
        c.set_word_in(SS, bp, 0);
    }
    c.set_reg(SI, si);
    c.set_reg(DI, di);
    c.set_reg(AX, di);
    c.set_reg(CX, di);
    let mut acc = c.shr1(W, di, 1);
    c.set_reg(AX, acc);
    let mut n = di;
    loop {
        // 3EC0h
        let a = acc.wrapping_add(si);
        let below = a < di;
        let (sx, sy) = if below {
            (c.word_in(SS, bp), c.word_in(SS, bp.wrapping_add(2)))
        } else {
            (c.word_in(SS, bp.wrapping_add(4)), c.word_in(SS, bp.wrapping_add(6)))
        };
        let row = sy == 1;
        c.run(0x3ec0, 3 + if below { 2 } else { 4 } + 3 + if row { 9 } else { 0 } + 2)?;
        c.add(W, acc, si);
        c.cmp(W, a, di);
        acc = if below { a } else { c.sub(W, a, di) };
        c.set_reg(AX, acc);
        c.set_reg(DX, sx);
        c.set_reg(BX, sy);
        let x = c.word(0x47e8);
        let nx = c.add(W, sx, x);
        c.set_reg(DX, nx);
        c.cmp(W, sy, 1);
        if row {
            c.push(di);
            c.push(acc);
            let p = c.word(0x22d9);
            c.set_reg(DI, p);
            c.set_reg(AX, x);
            c.stos(W);
            let p = c.reg(DI);
            let p = c.add(W, p, 2);
            c.set_word(0x22d9, p);
            let v = c.pop();
            c.set_reg(AX, v);
            let v = c.pop();
            c.set_reg(DI, v);
        }
        c.set_word(0x47e8, nx);
        n = n.wrapping_sub(1);
        c.set_reg(CX, n);
        if n == 0 {
            break;
        }
    }
    c.run(0x3ef8, 4)?;
    let sp = c.reg(SP);
    let sp = c.add(W, sp, 8);
    c.set_reg(SP, sp);
    let v = c.pop();
    c.set_reg(SI, v);
    let v = c.pop();
    c.set_reg(DI, v);
    c.ret()
}

/// `draw_SAL` (3B59): scene AL of the SAL resources: its sprite sheet
/// (13h + (AL - 1) / 16, when not the first) opened, its list (from the
/// table BC6Eh) and the position markers read (`sal_read_position_markers`,
/// into 18h bytes on the stack, [47F6h]); then each entry drawn until
/// FFFFh: a sprite (index, x, y, colour override at CS:C21Ah;
/// `draw_sprite_clobbering_bx_dx`), a person's place (sprite 1: the next
/// marker's figure, `loc_03d2f`, unless [47A4h] & 81h), a polygon
/// (`SAL_polygon`) or a rectangle (C0xxh: far [3901h]).
pub fn draw_sal(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{BP, CS, DI, ES, SP};
    entered(start, 0x3b59)?;
    let mut c = Ctx::new(m);
    let ax = c.reg(AX);
    let a = (ax & 0xff).wrapping_sub(1);
    let sheet = a >> 4 != 0;
    c.run(0x3b59, 8 + if sheet { 2 } else { 0 })?;
    let sp = c.reg(SP);
    let sp = c.sub(W, sp, 0x18);
    c.set_reg(SP, sp);
    c.set_word(0x47f6, sp);
    let ah = ax >> 8;
    c.xor(B, ah, ah);
    let a = c.dec(W, ax & 0xff);
    c.set_reg(AX, a);
    c.push(a);
    c.set_reg8(r8::CL, 4);
    let s = c.m.cpu.shift(5, W, a as u32, 4) as u16;
    c.set_reg(AX, s);
    if sheet {
        let v = c.add(W, s, 0x13);
        c.set_reg(AX, v);
        c.call(0xc13e, 0x3b70)?;
    }
    c.run(0x3b70, 7)?;
    let a = c.pop();
    let a = c.and(W, a, 0xf);
    let a = c.shl1(W, a, 1);
    c.set_reg(AX, a);
    let si = c.add(W, 0xbc6e, a);
    let si = c.word(si);
    c.set_reg(SI, si);
    c.call(0x3d83, 0x3b80)?;
    loop {
        let si = c.reg(SI);
        let w = c.word(si);
        if w == 0xffff {
            c.run(0x3b80, 3 + 3)?;
            c.lods(W, DS);
            c.cmp(W, w, 0xffff);
            c.m.cpu.write8(&mut c.m.hw, CS, 0xc21a, 0);
            let sp = c.reg(SP);
            let sp = c.add(W, sp, 0x18);
            c.set_reg(SP, sp);
            return c.ret();
        }
        if w & 0x8000 != 0 {
            if w >> 8 == 0xc0 {
                c.run(0x3b80, 4 + 2 + 14)?;
                c.lods(W, DS);
                c.cmp(W, w, 0xffff);
                c.cmp(B, w >> 8, 0xc0);
                c.push(w);
                let es = c.word(0xdbda);
                c.set_sreg(ES, es);
                let dx = c.lods(W, DS);
                c.set_reg(DX, dx);
                let bx = c.lods(W, DS);
                c.set_reg(BX, bx);
                let di = c.lods(W, DS);
                c.set_reg(DI, di);
                let cx = c.lods(W, DS);
                c.set_reg(CX, cx);
                let a = c.pop();
                c.set_reg(AX, a);
                let si = c.reg(SI);
                c.push(si);
                c.set_reg(BP, 0xffff);
                c.set_reg(SI, 0x1470);
                c.interp(0x3be2, 0x3be6)?;
                c.run(0x3be6, 2)?;
                let v = c.pop();
                c.set_reg(SI, v);
                continue;
            }
            c.run(0x3b80, 4 + 3)?;
            c.lods(W, DS);
            c.cmp(W, w, 0xffff);
            c.cmp(B, w >> 8, 0xc0);
            c.call(0x3be9, 0x3bc7)?;
            c.run(0x3bc7, 1)?;
            continue;
        }
        let one = w & 0x1ff == 1;
        c.run(0x3b80, 4 + 17 + 1)?;
        c.lods(W, DS);
        c.cmp(W, w, 0xffff);
        c.set_reg(DI, w);
        let h = c.shr1(B, w >> 8, 1);
        let h = c.and(B, h, 1);
        c.set_reg(AX, (h << 8) | (w & 0xff));
        let b1 = c.lods(B, DS);
        let dx = (h << 8) | b1;
        c.set_reg(DX, dx);
        c.lods(B, DS);
        c.xor(B, h, h);
        let bx = c.reg(AX) & 0xff;
        c.set_reg(AX, bx);
        c.set_reg(BX, bx);
        let col = c.lods(B, DS) as u8;
        let si = c.reg(SI);
        c.push(si);
        c.m.cpu.write8(&mut c.m.hw, CS, 0xc21a, col);
        c.set_reg(AX, w);
        let a = c.and(W, w, 0xfdff);
        let a = c.dec(W, a);
        c.set_reg(AX, a);
        let d = c.and(W, w, 0x1ff);
        c.set_reg(DI, d);
        c.cmp(W, d, 1);
        if !one {
            c.call(0xc22f, 0x3bb2)?;
            c.run(0x3bb2, 2)?;
            let v = c.pop();
            c.set_reg(SI, v);
            continue;
        }
        let f = c.byte(0x47a4);
        let skip = f & 0x81 != 0;
        let p = c.word(0x47f6);
        let mark = c.byte(p);
        let none = !skip && mark == 0xff;
        c.run(0x3d12, if skip { 2 } else if none { 6 } else { 8 })?;
        c.test(B, f as u16, 0x81);
        if !skip {
            c.set_reg(DI, p);
            let q = c.dec(W, p);
            c.set_word(0x47f6, q);
            c.cmp(B, mark as u16, 0xff);
            if !none {
                c.set_reg8(r8::AL, mark);
                c.call(0x3d2f, 0x3d2b)?;
            }
        }
        c.run(0x3d2b, 2)?;
        let v = c.pop();
        c.set_reg(SI, v);
    }
}

/// `SAL_polygon` (3BE9): a filled polygon of the scene, SI past its
/// header word (colour [si-2] into [47EDh], style [si-1] into [47ECh]).
/// The gradient steps ([22DBh], [22DDh], each a signed byte * 16), the
/// first point ([47EEh], [47F0h]); the right side's points traced
/// (`loc_03e13`, into 4C60h) up to the one with bit 14, the left side's
/// (into 4C62h) up to a negative one (or none with bit 15), closing
/// back to the start. Then each line from the top drawn between its
/// two ends (far [3945h] into [0DBDAh], [22DFh] the step, BP 1 for a
/// dithered style), right to left (backwards) with style bit 0.
pub fn sal_polygon(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{BP, DF, DI, ES};
    entered(start, 0x3be9)?;
    let mut c = Ctx::new(m);
    c.run(0x3be9, 21 + 6)?;
    c.set_word(0x22d9, 0x4c60);
    let ds = c.sreg(DS);
    c.push(ds);
    let v = c.pop();
    c.set_sreg(ES, v);
    let si = c.reg(SI);
    let a = c.byte(si.wrapping_sub(2));
    c.set_reg8(r8::AL, a);
    c.set_byte(0x47ed, a);
    let a = c.byte(si.wrapping_sub(1));
    c.set_reg8(r8::AL, a);
    c.set_byte(0x47ec, a);
    for at in [0x22dbu16, 0x22dd] {
        let b = c.lods(B, DS) as u8;
        let a = b as i8 as i16 as u16;
        c.set_reg(AX, a);
        let a = c.shl1(W, a, 4);
        c.set_reg(AX, a);
        c.set_word(at, a);
    }
    let dx = c.lods(W, DS);
    c.set_reg(DX, dx);
    let bx = c.lods(W, DS);
    c.set_reg(BX, bx);
    c.set_word(0x47ee, dx);
    c.set_word(0x47f0, bx);
    let mut ax;
    loop {
        c.run(0x3c25, 7)?;
        ax = c.lods(W, DS);
        c.push(ax);
        let d = c.and(W, ax, 0x3fff);
        c.set_reg(AX, d);
        c.set_reg(DI, d);
        let cx = c.lods(W, DS);
        c.set_reg(CX, cx);
        c.call(0x3e13, 0x3c32)?;
        c.run(0x3c32, 5)?;
        let (di, cx) = (c.reg(DI), c.reg(CX));
        c.set_reg(DX, di);
        c.set_reg(BX, cx);
        ax = c.pop();
        c.set_reg(AX, ax);
        c.test(W, ax, 0x4000);
        if ax & 0x4000 != 0 {
            break;
        }
    }
    c.run(0x3c3c, 7)?;
    let (di, cx) = (c.reg(DI), c.reg(CX));
    c.set_word(0x47f2, di);
    c.set_word(0x47f4, cx);
    let dx = c.word(0x47ee);
    c.set_reg(DX, dx);
    let bx = c.word(0x47f0);
    c.set_reg(BX, bx);
    c.set_word(0x22d9, 0x4c62);
    c.test(W, ax, 0x8000);
    if ax & 0x8000 == 0 {
        loop {
            c.run(0x3c57, 7)?;
            ax = c.lods(W, DS);
            c.push(ax);
            let d = c.and(W, ax, 0x3fff);
            c.set_reg(AX, d);
            c.set_reg(DI, d);
            let cx = c.lods(W, DS);
            c.set_reg(CX, cx);
            c.call(0x3e13, 0x3c64)?;
            c.run(0x3c64, 5 + if ax & 0x8000 != 0 { 2 } else { 0 })?;
            let (di, cx) = (c.reg(DI), c.reg(CX));
            c.set_reg(DX, di);
            c.set_reg(BX, cx);
            ax = c.pop();
            c.set_reg(AX, ax);
            c.or(W, ax, ax);
            if ax & 0x8000 != 0 {
                c.set_reg(DX, di);
                c.set_reg(BX, cx);
                break;
            }
        }
    }
    c.run(0x3c71, 3)?;
    let di = c.word(0x47f2);
    c.set_reg(DI, di);
    let cx = c.word(0x47f4);
    c.set_reg(CX, cx);
    c.call(0x3e13, 0x3c7c)?;
    let style = c.byte(0x47ec);
    let dither = style & 0x3e != 0;
    c.run(0x3c7c, 11 + dither as u32 + 6)?;
    let es = c.word(0xdbda);
    c.set_sreg(ES, es);
    let si = c.reg(SI);
    c.push(si);
    let top = c.word(0x47f0);
    c.set_reg(BX, top);
    let bot = c.word(0x47f4);
    let n = c.sub(W, bot, top);
    c.set_reg(BP, n);
    c.set_reg(SI, 0x4c60);
    c.set_reg(CX, n);
    c.set_reg(BP, 0);
    let ah = c.and(B, style as u16, 0x3e);
    c.set_reg8(r8::AH, ah as u8);
    if dither {
        c.set_reg(BP, 1);
    }
    c.set_reg8(r8::AL, 2);
    let step = c.reg(AX);
    c.set_word(0x22df, step);
    let col = c.byte(0x47ed);
    c.set_reg8(r8::AH, col);
    c.xor(B, 2, 2);
    c.set_reg8(r8::AL, 0);
    c.test(B, style as u16, 1);
    let back = style & 1 != 0;
    let base = if back { 0x3ce0u16 } else { 0x3cb2 };
    let mut cx = n;
    loop {
        let si = c.reg(SI);
        let (p, q) = (c.word(si), c.word(si.wrapping_add(2)));
        let swap = if back { p < q } else { p >= q };
        let (lo, hi) = if swap { (q, p) } else { (p, q) };
        let len = if back { hi.wrapping_sub(1).wrapping_sub(lo) } else { hi.wrapping_add(1).wrapping_sub(lo) };
        c.run(base, 9 + swap as u32 + 3 + if len != 0 { if back { 6 } else { 4 } } else { 0 })?;
        c.push(cx);
        let ax = c.reg(AX);
        c.push(ax);
        c.lods(W, DS);
        c.set_reg(DX, p);
        c.lods(W, DS);
        c.set_reg(CX, q);
        let v = c.pop();
        c.set_reg(AX, v);
        if back {
            c.cmp(W, p, q);
        } else {
            c.cmp(W, p, q);
        }
        c.set_reg(DX, lo);
        let x = if back { c.dec(W, hi) } else { c.inc(W, hi) };
        let l = c.sub(W, x, lo);
        c.set_reg(CX, l);
        if l != 0 {
            let l = if back { c.neg(W, l) } else { l };
            c.set_reg(CX, l);
            let si = c.reg(SI);
            c.push(si);
            let bx = c.reg(BX);
            c.push(bx);
            let s = c.word(0x22df);
            c.set_reg(SI, s);
            let d = c.word(0x22db);
            c.set_reg(DI, d);
            if back {
                c.set_flag(DF, true);
                c.interp(0x3d01, 0x3d05)?;
                c.run(0x3d05, 3)?;
            } else {
                c.interp(0x3cd0, 0x3cd4)?;
                c.run(0x3cd4, 2)?;
            }
            let v = c.pop();
            c.set_reg(BX, v);
            let v = c.pop();
            c.set_reg(SI, v);
            if back {
                c.set_flag(DF, false);
            }
        }
        let at = if back { 0x3d08 } else { 0x3cd6 };
        let left = cx.wrapping_sub(1);
        c.run(at, 4 + if left == 0 { 2 } else { 0 })?;
        let ax = c.reg(AX);
        let g = c.word(0x22dd);
        let ax = c.add(W, ax, g);
        c.set_reg(AX, ax);
        let bx = c.reg(BX);
        let bx = c.inc(W, bx);
        c.set_reg(BX, bx);
        cx = c.pop();
        cx = cx.wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    let v = c.pop();
    c.set_reg(SI, v);
    c.ret()
}
