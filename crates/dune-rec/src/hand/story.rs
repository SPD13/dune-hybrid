//! Story progress, the intro script, and one-line flag setters.

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

/// `loc_01bec`: in a place ([2Bh]): `loc_0503c` on location [114Eh];
/// [46D9h] to 6 when set; and when the place is left by it, `loc_00b21`
/// and bit 0 of [473Bh].
pub fn place_1bec(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1bec)?;
    let mut c = Ctx::new(m);
    let v = c.byte(0x2b);
    if v == 0 {
        c.run(start, 3)?;
        c.cmp(B, 0, 0);
        return c.ret();
    }
    c.run(start, 4)?;
    c.cmp(B, v as u16, 0);
    let di = c.word(0x114e);
    c.set_reg(DI, di);
    c.call(0x503c, 0x1bfa)?;
    let f = c.byte(0x46d9);
    let still = c.byte(0x2b);
    c.run(0x1bfa, 2 + (f != 0) as u32 + 2 + if still != 0 { 1 } else { 2 })?;
    c.cmp(B, f as u16, 0);
    if f != 0 {
        c.set_byte(0x46d9, 6);
    }
    c.cmp(B, still as u16, 0);
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
    let b = c.or(B, b as u16, 1);
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
        c.cmp(B, k as u16, 0);
        if k != 0 {
            return c.ret();
        }
        c.cmp(B, fb as u16, 0);
        if fb & 0x80 == 0 {
            return c.ret();
        }
        c.call(0xbdbb, 0x1c45)?;
        c.run(0x1c45, 1)?;
        return c.ret();
    }
    c.run(start, 3)?;
    c.cmp(B, k as u16, 0);
    c.call(0xc13b, 0x1c22)?;
    let si = c.word(0x46fa);
    c.run(0x1c22, 3 + (si != 0) as u32)?;
    c.set_reg(SI, si);
    c.or(W, si, si);
    if si != 0 {
        c.call(0x78e9, 0x1c2d)?;
    }
    let di = c.word(0x46f8);
    c.run(0x1c2d, if di != 0 { 4 } else { 4 })?;
    c.set_reg(DI, di);
    c.or(W, di, di);
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
    c.or(W, w, w);
    if w != 0 {
        c.call(code::GET_INGAME_DAY_Q, 0x237b)?;
        c.run(0x237b, 2)?;
        let al = c.reg8(r8::AL);
        let si = c.reg(SI);
        let b = c.byte(si.wrapping_add(0x10));
        let d = c.sub(B, al as u16, b as u16) as u8;
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
        s = c.add(W, s, 0x11);
        let v = c.byte(s);
        c.cmp(B, key as u16, v as u16);
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
    let mut d = c.sub(B, al as u16, b3 as u16) as u8;
    c.test(B, f as u16, 8);
    if first {
        d = 1;
        let f = c.or(B, f as u16, 8);
        c.set_byte(si.wrapping_add(2), f as u8);
    }
    c.set_reg8(r8::AL, d);
    c.set_byte(0x1e, d);
    c.set_byte(0x9d, 0);
    let w = c.word(0);
    let mut ax = c.and(W, w, 7);
    loop {
        c.cmp(B, ax & 0xff, modulus);
        if ax & 0xff < modulus {
            break;
        }
        let a = c.sub(B, ax & 0xff, modulus);
        ax = ax & 0xff00 | a;
    }
    let ax = c.add(W, ax, 0xe8);
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
    let f = c.and(B, f as u16, 0x9f);
    c.set_byte(di.wrapping_add(2), f as u8);
    c.xor(W, 0, 0);
    c.set_byte(0x9d, 0);
    c.set_reg(AX, gain);
    let v = c.word(0x20);
    let v = c.add(W, v, gain);
    c.set_word(0x20, v);
    let v = c.word(di.wrapping_add(0x0e));
    let v = c.add(W, v, gain);
    c.set_word(di.wrapping_add(0x0e), v);
    c.cmp(W, gain, v);
    if new {
        let b = c.byte(0x22);
        let b = c.inc(B, b as u16);
        c.set_byte(0x22, b as u8);
    }
    c.call(code::GET_INGAME_DAY_Q, 0x2403)?;
    c.run(0x2403, 8)?;
    let di = c.reg(DI);
    let al = c.reg8(r8::AL);
    c.set_byte(di.wrapping_add(0x10), al);
    let i = c.word(0x11f1);
    let i = c.sub(W, i, 0xe8);
    c.set_reg(AX, i);
    c.set_reg(BX, i);
    let at = i.wrapping_add(di).wrapping_add(4);
    let b = c.byte(at);
    let b = c.dec(B, b as u16);
    c.set_byte(at, b as u8);
    let si = c.word(0x114e);
    c.set_reg(SI, si);
    let at = i.wrapping_add(si).wrapping_add(0x14);
    let b = c.byte(at);
    let b = c.inc(B, b as u16);
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
        c.xor(B, 0, 0);
        return c.ret();
    }
    c.run(start, 11)?;
    c.set_reg(CX, n as u16);
    c.xor(B, 0, 0);
    c.set_reg(SI, 0x1179);
    let v = c.dec(B, n as u16);
    c.set_byte(0xc8, v as u8);
    c.set_reg(DI, 0x1179);
    let si = c.add(W, 0x1179, 2);
    c.set_reg(SI, si);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.set_reg(CX, 9);
    c.rep(0xa5);
    c.run(0x274a, 3)?;
    c.xor(W, 0, 0);
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
    let v = c.dec(B, b as u16);
    c.set_byte(0x1190, v as u8);
    if v == 0 {
        c.set_byte(0x118f, 0);
    }
    let si = c.inc(W, 0x1190);
    c.set_reg(DI, si);
    let si = c.add(W, si, 4);
    c.set_reg(SI, si);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.set_reg(CX, 0x12);
    c.rep(0xa5);
    c.run(0x2a4c, 4)?;
    c.xor(W, 0, 0);
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
        c.xor(W, 0, 0);
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
    c.xor(W, 0, 0);
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
        c.cmp(B, w & 0xff, 1);
        if w & 0xff == 1 {
            let si = c.reg(SI);
            let si = c.add(W, si, 2);
            c.set_reg(SI, si);
        } else {
            c.stos(W);
            c.movs(W);
            bp = c.inc(W, bp);
            c.set_reg(BP, bp);
        }
        c.set_reg(CX, n - 1 - i);
    }
    c.set_reg(CX, bp);
    let old = c.byte(LIST_1190);
    c.set_byte(LIST_1190, bp as u8);
    c.set_reg(CX, (bp & 0xff) << 8 | old as u16);
    c.cmp(B, old as u16, bp & 0xff);
    if shrank {
        c.xor(W, 0, 0);
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
    c.cmp(B, bx & 0xff, 0x80);
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
        c.cmp(B, kind as u16, 0x21);
    } else {
        c.run(0x3157, 6)?;
        c.cmp(B, kind as u16, 0x21);
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
    c.cmp(B, di >> 8, 0xff);
    let mut ax = di;
    if !direct {
        let w = c.word(di);
        let w = c.dec(W, w);
        let al = c.shl1(B, w & 0xff, 4);
        let al = c.or(B, al, w >> 8);
        c.xor(B, 0, 0);
        ax = al;
    }
    let ax = c.add(W, ax, 0x2bc);
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
    c.cmp(W, v, 0);
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
    c.cmp(B, f as u16, 0);
    if f != 0 {
        ax = c.add(B, ax, 4);
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
        c.or(B, ch as u16, ch as u16);
        if end {
            return c.ret();
        }
        let p = c.word(0x473f);
        let p = c.inc(W, p);
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
        c.cmp(B, al as u16, 0x20);
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
        c.cmp(B, b as u16, 0xff);
        let s = c.code_word(0x167);
        let v = c.code_word(s);
        let s = c.add(W, s, v);
        c.set_reg(SI, s);
        c.set_word(0xaa6e, s);
    } else {
        c.run(start, 5)?;
        c.set_reg(SI, si);
        c.cmp(B, b as u16, 0xff);
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
            let ah = c.reg8(r8::AH);
            c.xor(B, ah as u16, ah as u16);
            c.set_reg8(r8::AH, 0);
            c.or(B, 0, 0);
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
        let ah = c.reg8(r8::AH);
        c.xor(B, ah as u16, ah as u16);
        c.set_reg8(r8::AH, 0);
        c.or(B, al as u16, al as u16);
        c.cmp(B, al as u16, 1);
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
        let al = c.sub(B, ph as u16, 0x2d);
        c.set_reg8(r8::AL, al as u8);
        c.cmp(B, al, 3);
        if al >= 3 {
            return c.ret();
        }
        c.test(W, w, 0x10);
        if w & 0x10 != 0 {
            return c.ret();
        }
        c.set_reg(DI, di);
        c.or(W, di, di);
        return c.ret();
    }
    c.run(start, 10)?;
    c.set_reg8(r8::AL, ph);
    let al = c.sub(B, ph as u16, 0x2d);
    c.set_reg8(r8::AL, al as u8);
    c.cmp(B, al, 3);
    c.test(W, w, 0x10);
    c.set_reg(DI, di);
    c.or(W, di, di);
    c.call(0x331e, 0x6c62)?;
    let k = c.byte(0x66);
    if k == 0 {
        c.run(0x6c62, 3)?;
        c.cmp(B, 0, 0);
        return c.ret();
    }
    c.run(0x6c62, 4)?;
    c.cmp(B, k as u16, 0);
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
    let ax = c.sub(W, t, a);
    c.set_word(0x42, ax);
    c.set_reg(DX, ax);
    c.set_reg8(r8::CL, 4);
    let q = c.m.cpu.shift(5, W, ax as u32, 4) as u16;
    c.set_byte(0x41, q as u8);
    c.test(B, b3 as u16, 0x10);
    let mut ax = 0x74u16;
    if b3 & 0x10 == 0 {
        ax = 0x70;
        for lim in [3u16, 0x10, 0x20] {
            c.cmp(W, age, lim);
            if age < lim {
                break;
            }
            ax = c.inc(W, ax);
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
    let v = c.inc(B, v as u16);
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
    c.xor(B, 0, 0);
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
    let ax = c.add(W, ax, 0xe4);
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
    let di = c.reg(DI);
    c.xor(W, di, di);
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
    let v = c.inc(B, v as u16);
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
        let v = c.and(B, v as u16, 0x7f);
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
    let v = c.or(B, v as u16, 1);
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
        let v = c.dec(B, t as u16);
        c.set_byte(si.wrapping_add(1), v as u8);
        c.test(W, seed, 3);
        if a {
            c.set_byte(si.wrapping_add(7), 0x0b);
            c.test(W, seed, 0x0c);
            if b {
                c.set_byte(si.wrapping_add(7), 0x11);
            }
        }
        c.call(0xe3cc, 0x0c5b)?;
        let k = c.byte(0x473a);
        c.run(0x0c5b, 2 + (k != 0) as u32 + 3)?;
        c.cmp(B, k as u16, 0);
        let mut ax = c.reg(AX);
        if k != 0 {
            let al = c.and(B, ax & 0xff, 0xef);
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
        c.cmp(B, al as u16, 4);
        let mut cx = c.reg(CX);
        if hi {
            let ch = c.or(B, cx >> 8, 0x40);
            cx = ch << 8 | cx & 0xff;
            c.set_reg(CX, cx);
        }
        c.set_word(si.wrapping_add(2), cx);
    } else {
        c.run(start, 2)?;
        let v = c.dec(B, t as u16);
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
        let a = c.and(W, ax, 0x10);
        let bx = c.and(W, bx, 6);
        let s = c.word(bx.wrapping_add(0x15a2));
        c.set_reg(SI, s);
        let bx = c.add(W, bx, bx);
        c.set_reg(BX, bx);
        let a = c.add(W, a, bx);
        let l = c.add(B, a & 0xff, 4);
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
        c.and(W, ax, 0x10);
        let mut ah = c.and(B, b3 as u16, 0xc0);
        let mut al = c.and(B, b3 as u16, 0x3f);
        c.test(B, ah, 0x40);
        if turn {
            let s = c.rol1(W, seed);
            c.set_word(0, s);
            if roll {
                let mut cl = 0x0au16;
                c.or(B, ah, ah);
                if ah & 0x80 != 0 {
                    cl = c.neg(B, cl);
                }
                c.set_reg8(r8::CL, cl as u8);
                al = c.add(B, al, cl);
                if al & 0x80 != 0 {
                    ah = c.xor(B, ah, 0x80);
                    al = c.xor(B, 0, 0);
                }
                c.cmp(B, al, 0x40);
                if al >= 0x40 {
                    al = 0x3f;
                    ah = c.xor(B, ah, 0x80);
                }
                let v = c.or(B, ah, al);
                ah = v;
                c.set_byte(si.wrapping_add(3), v as u8);
            }
        }
        let a = c.add(B, al, 0xe0);
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
    let di = c.and(W, di, 0x0c);
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
    let a = c.and(W, ax, 0x0f);
    c.set_byte(0x2f, a as u8);
    let a = c.add(W, a, 0x18);
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
    let a = c.and(W, w, 0x0f);
    c.set_byte(0x31, a as u8);
    let a = c.add(W, a, 0);
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
    c.xor(B, 0, 0);
    let a = c.add(W, w & 0xff, 0xe8);
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
    let v = c.sub(B, al as u16, d as u16);
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
