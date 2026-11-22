//! Story progress, the intro script, and one-line flag setters.

use super::dialogue;
use super::waits;
use super::video;
use super::text;
use super::tasks;
use super::tails;
use super::screen;
use super::resources;
use super::mouse;
use super::frame;
use super::files;
use super::audio;
use super::time;
use super::math;
use super::globe;
use super::cursor;
use super::troops;
use super::sound;
use super::records;
use super::map;
use super::gfx;
use super::entry;
use super::game::{Location, location_flag};
use super::pure;
use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DS, DX, SI, SP, SS, Width, ZF},
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
    pub const GET_INGAME_DAY_Q: u16 = 0x1ac5;
    pub const LOCATION_COORDS: u16 = 0x40ae;
    /// BP's routine for the record at DI.
    pub const FOR_RECORD: u16 = 0x661d;
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
    pub const SET_GAME_PHASE: u16 = 0x121f;
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
    /// `sub_10b2`: the handler of phase 20h (and 21h): character 2's location
    /// byte ([0FFBh], its +3) = 1.
    on_phase_20, 0x10b2, g::FLAG_0FFB, 1
);
set_flag!(
    /// `sub_1127`: the handler of phase 38h (and 39h): character 0's location
    /// byte ([0FDBh], its +3) = FFh.
    on_phase_38, 0x1127, g::CHARACTER_0_LOCATION, 0xff
);
set_flag!(
    /// `sub_a5aa`: [`g::SETTINGS_HOVER`] = 0: the cursor is on no slider or
    /// switch.
    clear_settings_hover, 0xa5aa, g::SETTINGS_HOVER, 0
);

/// `sub_112d`: the handler of phase 40h (and 41h): bit 2 of character 8's
/// flags ([1067h], its +0Fh). Flags from the `or`.
pub fn on_phase_40(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x112d)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let bits = c.character(8).flags;
    let bits = pure::or(B, bits as u16, 2) as u8;
    c.update_character(8, |t| t.flags = bits);
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
    let current = c.progress().phase;
    c.run(0x121f, 2)?; // cmp al, [2Ah]; jbe
    if new <= current {
        c.run(0x1242, 1)?;
        return c.ret();
    }
    c.run(0x1225, 3)?; // mov [2Ah], al; mov byte [0FFh], 0; call
    c.update_progress(|t| t.phase = new);
    c.update_situation(|t| t.days_in_phase = 0);
    c.call(code::ON_PHASE_CHANGE, 0x1230)?;

    // The hook may have moved the phase on: read it again.
    let phase = c.progress().phase;
    c.run(0x1230, 3)?; // mov bl, [2Ah]; cmp bl, 6Ch; ja
    c.set_reg8(super::r8::BL, phase);
    if phase > code::LAST_HANDLED_PHASE {
        c.run(0x1242, 1)?;
        return c.ret();
    }
    c.run(0x1239, 3)?; // xor bh, bh; shr bx, 1; call cs:[bx+11E7h]
    let index = (phase >> 1) as u16;
    // The handler is called through a table: the flags of `shr bx, 1` go
    // with it.
    let bx = c.shr1(Width::W, phase as u16, 1);
    c.set_reg(BX, bx);
    let handler = c.code_word(code::PHASE_HANDLERS.wrapping_add(index));
    c.call(handler, 0x1242)?;
    c.run(0x1242, 1)?;
    c.ret()
}

/// [`set_game_phase`] called: the game phase raised to AL (never lowered).
pub(super) fn raise_phase(c: &mut Ctx, phase: u8, ret: u16) -> Result<(), Bail> {
    c.set_reg8(r8::AL, phase);
    c.call(entry::SET_GAME_PHASE, ret)
}

/// `on_phase_04`: [122Ah] -= 1, `sub_105b` with SI = 1021h, then [0FE8h] = 9.
pub fn on_phase_04(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1011)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let v = c.byte(g::PALACE_ROOM_2_LABEL);
    let v = pure::dec(B, v as u16) as u8;
    c.set_byte(g::PALACE_ROOM_2_LABEL, v);
    c.set_reg(SI, 0x1021);
    c.call(entry::FOR_EACH_IN_CODE_LIST, 0x101b)?;
    c.run(0x101b, 2)?;
    set_0fe8(&mut c)
}

/// `mov byte [0FE8h], 9; ret` (101B).
fn set_0fe8(c: &mut Ctx) -> Result<(), Bail> {
    c.update_character(1, |t| t.x = t.x & 0xff00 | 9);
    c.ret()
}

/// `on_phase_50`: sets bit 40h of [0Ah], charisma += 28h, then [0FE8h] = 9.
pub fn on_phase_50(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x117b)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let bits = c.progress().event_flags;
    let bits = pure::or(B, bits as u16, 0x40) as u8;
    c.update_progress(|t| t.event_flags = bits);
    raise_charisma(&mut c, 0x28, 0x1185)?;
    c.run(0x1185, 3)?; // jmp 101Bh; mov; ret
    set_0fe8(&mut c)
}

/// `on_phase_5c`: [1038h] = 5, [11D0h] += 0Ch, [1156h] = the day + 3,
/// [1141h] += 1. Flags from the `inc`; AX = the day + 3.
pub fn on_phase_5c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x11b3)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.update_character(6, |t| t.x = t.x & 0xff00 | 5);
    let v = c.byte(g::BYTE_11D0);
    let v = pure::add(B, v as u16, 0x0c) as u8;
    c.set_byte(g::BYTE_11D0, v);
    c.call(code::GET_INGAME_DAY_Q, 0x11c0)?;
    c.run(0x11c0, 4)?;
    let day = c.reg(AX);
    let day = pure::add(W, day, 3);
    c.set_reg(AX, day);
    c.set_word(g::ATTACK_DAY, day);
    let v = c.byte(g::ITEMS_OFFERED);
    let v = pure::inc(B, v as u16) as u8;
    c.set_byte(g::ITEMS_OFFERED, v);
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
    let old = c.progress().charisma;
    c.set_reg8(AH, old);
    let gain = c.reg8(AL);
    let mut new = pure::add(B, gain as u16, old as u16) as u8;
    let mut n = 4 + 6; // mov; add; cmp; jbe; mov [29h]; and; sub; shr; shr; jne
    if new > MAX_CHARISMA {
        new = MAX_CHARISMA;
        n += 1;
    }
    c.update_progress(|t| t.charisma = new);
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

/// [`increase_charisma_by_al`] called: charisma up by AL (at most C8h), motivation with it.
pub(super) fn raise_charisma(c: &mut Ctx, points: u8, ret: u16) -> Result<(), Bail> {
    c.set_reg8(r8::AL, points);
    c.call(entry::INCREASE_CHARISMA_BY_AL, ret)
}

/// `loc_04ab8`: the flight started ([4727h] FFh).
pub fn set_flying(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4ab8)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.update_flight(|t| t.flying = 0xff);
    c.ret()
}

/// `loc_04ac4`: the travel no longer held ([11CAh] 0).
pub fn release_travel(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4ac4)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.update_travel(|t| t.held = 0);
    c.ret()
}

/// `loc_04aca`: the travel held ([11CAh] 1).
pub fn hold_travel(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4aca)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.update_travel(|t| t.held = 1);
    c.ret()
}

/// `loc_050be`: the flight steers again ([11CBh] 0).
pub fn stop_keeping_heading(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x50be)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.update_travel(|t| t.keep_heading = 0);
    c.ret()
}

/// `sub_a1d6`: [47A5h] 0: the action asked of the scripts (set FFh by
/// `allow_scripted_action`) is not to go ahead.
pub fn forbid_scripted_action(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa1d6)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.update_conversation(|t| t.flag_47a5 = 0);
    c.ret()
}

/// `loc_0a1c4`: [47A5h] FFh before the scripts are asked about an action
/// (they may clear it); `is_scripted_action_allowed` checks it after.
pub fn allow_scripted_action(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa1c4)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.update_conversation(|t| t.flag_47a5 = 0xff);
    c.ret()
}

/// `loc_0a1e2`: ZF set when [47A5h] is FFh, CF when it is not (the
/// flags of `cmp [47A5h], FFh`; its callers read those two).
pub fn is_scripted_action_allowed(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa1e2)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let v = c.conversation().flag_47a5;
    c.set_flag(pc::cpu::ZF, v == 0xff);
    c.set_flag(CF, v != 0xff);
    c.ret()
}

/// [`is_scripted_action_allowed`] called: whether [47A5h] is FFh.
pub(super) fn scripted_action_allowed(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    c.call_zf(0xa1e2, ret)
}

/// `loc_0a1e8`: one more at `last_entry` ([47A8h]).
pub fn mark_last_entry(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa1e8)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let v = c.conversation().last_entry;
    let v = pure::inc(B, v as u16);
    c.update_conversation(|t| t.last_entry = v as u8);
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
    /// `loc_0100b`: at phase 1 (`next_phase_unless_done`): character 3's
    /// location byte ([100Bh], its +3) = 1.
    on_phase_01, 0x100b, 0x100b, 1
);
set_flag!(
    /// `loc_0101b`: [0FE8h] = 9.
    set_0fe8_to_9, 0x101b, g::BYTE_0FE8, 9
);
set_flag!(
    /// `loc_01bb2`: [`g::REDRAW`] = 0: nothing left to redraw.
    clear_redraw, 0x1bb2, 0x473b, 0
);
set_flag!(
    /// `loc_0215f`: [46D9h] = 7.
    set_46d9_to_7, 0x215f, g::COMM_CALLER, 7
);
set_word!(
    /// `loc_01392`: the UI list [2220h] = 1FBAh, as a sequence step shows a
    /// room or a scene (13C8h, 140Bh, 1422h, 1442h).
    select_scene_ui_list, 0x1392, 0x2220, 0x1fba
);
set_word!(
    /// `loc_01399`: the UI list [2220h] = 1FAEh, as a sequence step shows a
    /// talk (13A0h, 13E4h); `run_script_next_step` then takes a click on 1B56h.
    select_talk_ui_list, 0x1399, 0x2220, 0x1fae
);
/// `loc_09901`: no text box shown ([`super::text_box::TextBox::shown`]
/// 0).
pub fn hide_text_box(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9901)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.update_text_box(|t| t.shown = 0);
    c.ret()
}

/// `loc_011cb`: the phase to 60h (unchanged-flag cleared), and the
/// position of location 11Ch (`loc_040ae`) noted at 1048h with DL = 2.
pub fn go_to_phase_60(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x11cb)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.update_situation(|t| t.days_in_phase = 0);
    c.update_progress(|t| t.phase = 0x60);
    c.set_reg(DI, 0x011c);
    c.call(code::LOCATION_COORDS, 0x11db)?;
    c.run(0x11db, 4)?;
    c.set_reg8(r8::DL, 2);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.update_character(7, |t| t.x = dx);
    c.update_character(7, |t| t.y = bx);
    c.ret()
}

/// `loc_01243`: the sum (BX, from 0) over the three records from 138h
/// (`loc_01258`, with BP = 1269h); 1000 or more counts one at [`g::ENDGAME_STAGE`].
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
    c.call(entry::FOR_THREE_RECORDS, 0x124d)?;
    let bx = c.reg(BX);
    let big = bx >= 0x3e8;
    c.run(0x124d, if big { 4 } else { 3 })?;
    if big {
        let v = c.economy().endgame_stage;
        let v = pure::inc(B, v as u16);
        c.update_economy(|t| t.endgame_stage = v as u8);
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
    let phase = c.progress().phase;
    c.run(start, if al != 4 { 4 } else { 6 })?;
    // Its callers read CF alone.
    c.ret_cf(al == 4 && (0x15..0x20).contains(&phase))
}

/// [`is_4_in_phases_15_to_1f`] called: whether AL is 4 and the phase is 15h..1Fh.
pub(super) fn four_in_phases_15_to_1f(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    c.call_cf(0x127c, ret)
}

/// `loc_01d9f`: unless bit 80h of [12h], `loc_01e01` on the record at
/// 1048h, then `loc_01eda` when it sets CF.
pub fn check_character_7_attack(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1d9f)?;
    let mut c = Ctx::new(m);
    let w = c.progress().present;
    if w & 0x80 != 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    c.run(start, 4)?;
    c.set_reg(SI, 0x1048);
    let character_at_flagged_troop = troops::character_at_flagged_troop(&mut c, 0x1dad)?;
    c.run(0x1dad, 2)?;
    if character_at_flagged_troop {
        c.call(entry::ATTACK_CLOCK_TICK, 0x1db2)?;
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
    let phase = c.progress().phase;
    c.run(start, 7)?;
    let d = pure::sub(B, phase as u16, 0x60);
    c.set_reg8(r8::AL, d as u8);
    if d >= 4 {
        return c.ret();
    }
    c.call(code::SAVE_REGS, 0x1ed1)?;
    c.run(0x1ed1, 2)?;
    raise_phase(&mut c, 0x64, 0x1ed6)?;
    c.run(0x1ed6, 1)?;
    c.call(code::RESTORE_REGS, 0x1ed9)?;
    c.run(0x1ed9, 1)?;
    c.ret()
}

/// `loc_023d5`: the haggle: [`g::SMUGGLER_PRICE`] down by an eighth of it.
pub fn haggle_smuggler_price(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x23d5)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    let v = c.economy().smuggler_price as u16;
    let e = pure::shr1(B, v, 3);
    let v = pure::sub(B, v, e);
    c.set_reg(AX, e << 8 | v);
    c.update_economy(|t| t.smuggler_price = v as u8);
    c.ret()
}

/// `loc_02524`: AX spice paid (to the Emperor or a smuggler): taken from
/// [`g::SPICE`], added to [`g::SPICE_PAID`].
pub fn pay_spice(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2524)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let ax = c.reg(AX);
    let v = c.economy().spice;
    let v = pure::sub(W, v, ax);
    c.update_economy(|t| t.spice = v);
    let w = c.word(g::SPICE_PAID);
    let w = pure::add(W, w, ax);
    c.set_word(g::SPICE_PAID, w);
    c.ret()
}

/// `loc_03310`: AX = D1h + AL / 16.
pub fn skill_level_phrase(m: &mut Machine, start: u16) -> Result<(), Bail> {
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
pub fn measure_place_forces(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x33be)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let ax = c.reg(AX);
    let z = pure::xor(W, ax, ax);
    c.set_reg(AX, z);
    for at in [0x94, 0x96, 0x5c, 0x5e] {
        c.set_word(at, 0);
    }
    troops::run_for_each_troop(&mut c, entry::ADD_TROOP_STRENGTH, 0x33d2)?;
    c.run(0x33d2, 1)?;
    c.call(entry::STRIKE_ODDS, 0x33d5)?;
    c.run(0x33d5, 2)?;
    let al = c.reg8(r8::AL);
    c.update_place_forces(|t| t.strike_odds = al);
    c.ret()
}

/// `loc_0366f`: AX = a byte of [1152h] (the low one with bit 80h of [0],
/// else the high one; the low one when the high is FFh; FFFFh as is).
/// Flags from `or ax, ax`.
pub fn pick_one_of_two_speakers(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x366f)?;
    let mut c = Ctx::new(m);
    let ax = c.word(g::SPEAKER_SLOTS);
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
    let mut v = ax;
    if ax != 0xffff {
        if ax >> 8 != 0xff {
            if w0 & 0x80 == 0 {
                v = v.rotate_left(8);
            }
        }
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
    let v = c.progress().talk_reason;
    if v == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    c.run(start, 3)?;
    c.call(entry::CLOSE_DIALOGUE_TEXT, 0x36dd)?;
    c.run(0x36dd, 3)?;
    c.update_conversation(|t| t.scene_count = 0);
    run_for_records_here(&mut c, entry::SCENE_SENTENCE, 0x36e8)?;
    c.run(0x36e8, 2)?;
    c.update_progress(|t| t.talk_reason = 0);
    c.ret()
}

/// `loc_0390a`: `loc_03916` while [46D7h] is set.
pub fn finish_sky_fade(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x390a)?;
    let mut c = Ctx::new(m);
    let (mut at, mut jmp) = (start, 0);
    loop {
        let v = c.byte(g::FRAME_TASKS_ACTIVE);
        c.run(at, jmp + 3)?;
        if v == 0 {
            return c.ret();
        }
        c.call(entry::SKY_FADE_TASK, 0x3914)?;
        (at, jmp) = (0x3914, 1);
    }
}

/// `loc_0395f`: BL = the table at 2280h (by AL's low 4 bits) plus AL's
/// bits 2..4 (in AH as 0..1Ch).
pub fn sky_palette_for_time(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x395f)?;
    let mut c = Ctx::new(m);
    c.run(start, 9)?;
    let al = c.reg8(r8::AL) as u16;
    let ah = pure::shr1(B, al, 2);
    let ax = pure::and(W, ah << 8 | al, 0x1c0f);
    let v = c.byte(0x2280u16.wrapping_add(ax & 0xff));
    let s = pure::add(B, v as u16, ax >> 8);
    c.set_reg(AX, ax & 0xff00 | s);
    c.set_reg(BX, 0x2200 | s);
    c.ret()
}

/// `loc_03ae9`: the 2Eh words at 47F8h to FFFFh. AX kept.
pub fn clear_speaker_places(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3ae9)?;
    let mut c = Ctx::new(m);
    c.run(start, 7 + 0x2e + 2)?; // the `rep` iterations too
    let ax = c.reg(AX);
    c.push(ax);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.set_reg(CX, 0x2e);
    c.set_reg(AX, 0xffff);
    c.set_reg(DI, 0x47f8);
    c.rep_counted(0xab);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// From 41CC (the stretch from `at`, `prefix` instructions before it done
/// by `ops`): [21FDh] = AL, and [1F11h] too when [1F12h] is 4FFBh.
fn note_21fd(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    let w = c.word(g::ACTION_LIST_FIRST_HANDLER);
    c.run(at, prefix + if w == 0x4ffb { 5 } else { 4 })?;
    ops(c);
    let al = c.reg8(r8::AL);
    c.set_byte(g::MAP_ACTION_FLAGS, al);
    c.cmp(W, w, 0x4ffb);
    if w == 0x4ffb {
        c.set_byte(g::ACTION_LIST_FIRST_FLAGS, al);
    }
    c.ret()
}

/// `loc_041cc`: [21FDh] = AL (see [`note_21fd`]).
pub fn note_21fd_al(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x41cc)?;
    note_21fd(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_041c5`: [4726h] and [21FDh] cleared (see [`note_21fd`]).
pub fn reset_desert_countdown(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x41c5)?;
    note_21fd(&mut Ctx::new(m), start, 2, |c| {
        c.set_byte(g::DESERT_COUNTDOWN, 0);
        let al = c.reg8(r8::AL) as u16;
        let z = pure::xor(B, al, al);
        c.set_reg8(r8::AL, z as u8);
    })
}

/// `loc_0469b`: when [473Fh] is set, cleared and the frame task at 46B5h
/// removed (registers kept).
pub fn stop_typing_message(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x469b)?;
    let mut c = Ctx::new(m);
    let v = c.typed_message().next[0];
    if v == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    c.run(start, 4)?;
    c.update_typed_message(|t| t.next[0] = 0);
    c.call(code::SAVE_REGS, 0x46ab)?;
    c.run(0x46ab, 2)?;
    tasks::remove_task(&mut c, entry::MESSAGE_TASK, 0x46b1)?;
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
    let (v, al) = (c.travel().heading, c.reg8(r8::AL));
    let v = c.add(B, v as u16, al as u16);
    c.update_travel(|t| t.heading = v as u8);
    c.update_travel(|t| t.carry = 0x80);
    c.ret()
}

/// `loc_05119`: see [`add_11c7`].
pub fn turn_heading_by_al(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5119)?;
    add_11c7(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_04965`: `loc_05124`, then [11C5h] = DI, [11C8h], [11C7h] = 0 and
/// [`add_11c7`].
pub fn fly_to_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4965)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(entry::HEAD_FOR_LOCATION, 0x4968)?;
    add_11c7(&mut c, 0x4968, 5, |c| {
        let cx = c.reg(CX);
        let z = pure::xor(W, cx, cx);
        c.set_reg(CX, z);
        let di = c.reg(DI);
        c.update_travel(|t| t.target = di);
        c.update_travel(|t| t.no_target = 0);
        c.update_travel(|t| t.heading = 0);
    })
}

/// `loc_04b3b`: one more tick at [472Bh] (every 16th `loc_00fd9` with
/// CX = 1); the map position (`get_map_position`, `loc_05206`,
/// `loc_040c3`) to [4], [6].
pub fn advance_flight_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4b3b)?;
    let mut c = Ctx::new(m);
    let n = c.flight().steps.wrapping_add(1);
    let tick = n & 0x0f == 0;
    c.run(start, if tick { 5 } else { 4 })?;
    let v = c.flight().steps;
    let v = pure::inc(W, v);
    c.update_flight(|t| t.steps = v);
    if tick {
        time::run_clock(&mut c, 1, 0x4b4d)?;
        c.run(0x4b4d, 1)?;
    }
    c.call(code::GET_MAP_POSITION, 0x4b50)?;
    c.run(0x4b50, 1)?;
    c.call(entry::HEADING_STEP, 0x4b53)?;
    c.run(0x4b53, 1)?;
    c.call(entry::MOVE_PARTY_HERE, 0x4b56)?;
    c.run(0x4b56, 3)?;
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.update_player(|p| p.x = dx);
    c.update_player(|p| p.y = bx);
    c.ret()
}

/// `loc_04ded`: `loc_0661d` with BP = 4E04h, CX = 0 (SI kept); then
/// [4733h] = CX with CH = the record's byte 14h, and `loc_07f27` (ES
/// kept).
pub fn note_place_troops(m: &mut Machine, start: u16) -> Result<(), Bail> {
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
    c.set_word(g::PLACE_TROOP_COUNT, cx);
    let es = c.sreg(pc::cpu::ES);
    c.push(es);
    c.call(entry::NOTED_LOCATION_SKILL_NEEDS, 0x4e01)?;
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
    let v = c.economy().book;
    c.set_reg8(r8::AL, v);
    let ax = c.reg(AX);
    c.push(ax);
    let v2 = pure::or(B, v as u16, 0x80) as u8;
    c.set_reg8(r8::AL, v2);
    c.update_economy(|t| t.book = v2);
    c.call(code::DIALOGUE_STEP_AB84, 0xb186)?;
    c.run(0xb186, 3)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.update_economy(|t| t.book = ax as u8);
    c.ret()
}

/// `loc_0b254`: [243Eh] = 24h less what is left after AX in the 0Ch words
/// at 2426h (so 25h - 0Ch + its index), or 0 when it is not there. AX, SI
/// kept.
pub fn find_chapter_video(m: &mut Machine, start: u16) -> Result<(), Bail> {
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
        v = 0x24u16.wrapping_sub(c.reg(CX));
    }
    c.set_reg(AX, v);
    c.set_word(g::CHAPTER_VIDEO, v);
    let si = c.pop();
    c.set_reg(SI, si);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// `loc_01bec`: at a place the Harkonnens hold ([2Bh]): the place
/// entered again (`loc_0503c` on location [114Eh]); [46D9h] to 6 when
/// set; and when it is no longer held, `loc_00b21` and bit 0 of [473Bh].
pub fn revisit_harkonnen_place(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1bec)?;
    let mut c = Ctx::new(m);
    let v = c.progress().harkonnen_place;
    if v == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    c.run(start, 4)?;
    let di = c.word(g::PLACE);
    enter_location_at(&mut c, di, 0x1bfa)?;
    let f = c.byte(g::COMM_CALLER);
    let still = c.progress().harkonnen_place;
    c.run(0x1bfa, 2 + (f != 0) as u32 + 2 + if still != 0 { 1 } else { 2 })?;
    if f != 0 {
        c.set_byte(g::COMM_CALLER, 6);
    }
    if still != 0 {
        return c.ret();
    }
    let di = c.reg(DI);
    c.push(di);
    c.call(entry::STOP_SCENE_VOICE_AND_PARTICLES, 0x1c11)?;
    c.run(0x1c11, 3)?;
    let di = c.pop();
    c.set_reg(DI, di);
    let b = c.byte(g::REDRAW);
    let b = pure::or(B, b as u16, 1);
    c.set_byte(g::REDRAW, b as u8);
    c.ret()
}

/// `loc_01c18`: by [46EBh]: negative, the map's resource opened and the
/// marks of [46FAh] (`loc_078e9`) and [46F8h] (`loc_0600e`) redrawn; 0
/// with [0FBh] negative, the stats screen (`loc_0bdbb`).
pub fn refresh_map_overlays(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1c18)?;
    let mut c = Ctx::new(m);
    let k = c.map_view().mode;
    if k & 0x80 == 0 {
        let map_ui = c.situation().map_interface;
        let n = if k != 0 { 4 } else if map_ui & 0x80 == 0 { 6 } else { 6 };
        c.run(start, n)?;
        if k != 0 {
            return c.ret();
        }
        if map_ui & 0x80 == 0 {
            return c.ret();
        }
        c.call(entry::STATS_SCREEN_MARKS, 0x1c45)?;
        c.run(0x1c45, 1)?;
        return c.ret();
    }
    c.run(start, 3)?;
    c.call(entry::OPEN_ONMAP_RESOURCE, 0x1c22)?;
    let si = c.map_selection().panel_dirty;
    c.run(0x1c22, 3 + (si != 0) as u32)?;
    c.set_reg(SI, si);
    if si != 0 {
        c.call(entry::TROOP_PANEL, 0x1c2d)?;
    }
    let di = c.map_selection().caption;
    c.run(0x1c2d, if di != 0 { 4 } else { 4 })?;
    c.set_reg(DI, di);
    if di == 0 {
        return c.ret();
    }
    c.call(entry::DRAW_LOCATION_CAPTION, 0x1c38)?;
    c.run(0x1c38, 1)?;
    c.ret()
}

/// `loc_0235f`: the smuggler at SI ([`g::SMUGGLERS`]) made current (at
/// [10B4h]): its bytes 2, 1 to [`g::SMUGGLER_FLAGS`],
/// [`g::SMUGGLER_PATIENCE`], its debt (word 0Eh) to [`g::SMUGGLER_DEBT`],
/// and [`g::DEBT_DAYS`] the days since its byte 10h (0 without a debt).
pub fn select_smuggler(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x235f)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let w = c.word(si.wrapping_add(0x0e));
    c.run(start, if w == 0 { 8 } else { 9 })?;
    c.update_character(13, |t| t.word_0c = si);
    let b2 = c.byte(si.wrapping_add(2));
    c.set_reg8(r8::AL, b2);
    c.update_progress(|t| t.smuggler_flags = b2);
    c.set_reg(AX, w);
    c.update_progress(|t| t.smuggler_debt = w);
    c.update_progress(|t| t.debt_days = 0);
    if w != 0 {
        c.call(code::GET_INGAME_DAY_Q, 0x237b)?;
        c.run(0x237b, 2)?;
        let al = c.reg8(r8::AL);
        let si = c.reg(SI);
        let b = c.byte(si.wrapping_add(0x10));
        let d = pure::sub(B, al as u16, b as u16) as u8;
        c.set_reg8(r8::AL, d);
        c.update_progress(|t| t.debt_days = d);
    }
    c.run(0x2381, 3)?;
    let si = c.reg(SI);
    let b1 = c.byte(si.wrapping_add(1));
    c.set_reg8(r8::AL, b1);
    c.update_progress(|t| t.smuggler_patience = b1);
    c.ret()
}

/// `loc_02318`: a visit to a smuggler: the one of [`g::SMUGGLERS`] whose
/// byte 0 is the byte at DI (the location's) made current (`loc_0235f`);
/// [`g::SMUGGLER_DAYS_AWAY`] the days since its byte 3 (1 the first
/// time, marking bit 8 of byte 2); the price and [9Fh] cleared;
/// [`g::SMUGGLER_ITEM`] = 0E8h + the low 3 bits of [0] modulo
/// [`g::ITEMS_OFFERED`].
pub fn visit_smuggler(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2318)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let key = c.byte(di);
    let mut si = g::SMUGGLERS - 0x11;
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
    let mut s = g::SMUGGLERS - 0x11;
    loop {
        s = pure::add(W, s, 0x11);
        let v = c.byte(s);
        if v == key {
            break;
        }
    }
    c.set_reg(SI, s);
    c.call(entry::SELECT_SMUGGLER, 0x2327)?;
    c.run(0x2327, 1)?;
    c.call(code::GET_INGAME_DAY_Q, 0x232a)?;
    let si = c.reg(SI);
    let f = c.byte(si.wrapping_add(2));
    let first = f & 8 == 0;
    let m0 = c.word(0) & 7;
    let modulus = c.byte(g::ITEMS_OFFERED) as u16;
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
    c.update_progress(|t| t.smuggler_days_away = d);
    c.update_economy(|t| t.smuggler_price = 0);
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
    c.set_word(g::SMUGGLER_ITEM, ax);
    c.update_economy(|t| t.answer = 0);
    c.ret()
}

/// `loc_023e6`: the item bought from the smuggler at DI, on credit: its
/// byte 2 loses bits 60h; the price (taken, cleared) added to
/// [`g::SMUGGLER_DEBT`] and its word 0Eh (one more of [`g::DEBTS`] when
/// that was 0); its byte 10h = the day; and the item moves: the
/// smuggler's byte 4 + i one down, the location [114Eh]'s byte 14h + i
/// one up (i = [`g::SMUGGLER_ITEM`] - 0E8h).
pub fn buy_from_smuggler_on_credit(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x23e6)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let price = c.economy().smuggler_price as u16;
    let w = c.word(di.wrapping_add(0x0e)).wrapping_add(price);
    let new = w == price;
    c.run(start, 7 + new as u32 + 1)?;
    let f = c.byte(di.wrapping_add(2));
    let f = pure::and(B, f as u16, 0x9f);
    c.set_byte(di.wrapping_add(2), f as u8);
    c.update_economy(|t| t.smuggler_price = 0);
    c.set_reg(AX, price);
    let v = c.progress().smuggler_debt;
    let v = pure::add(W, v, price);
    c.update_progress(|t| t.smuggler_debt = v);
    let v = c.word(di.wrapping_add(0x0e));
    let v = pure::add(W, v, price);
    c.set_word(di.wrapping_add(0x0e), v);
    if new {
        let b = c.progress().debts;
        let b = pure::inc(B, b as u16);
        c.update_progress(|t| t.debts = b as u8);
    }
    c.call(code::GET_INGAME_DAY_Q, 0x2403)?;
    c.run(0x2403, 8)?;
    let di = c.reg(DI);
    let al = c.reg8(r8::AL);
    c.set_byte(di.wrapping_add(0x10), al);
    let i = c.word(g::SMUGGLER_ITEM);
    let i = pure::sub(W, i, 0xe8);
    c.set_reg(AX, i);
    c.set_reg(BX, i);
    let at = i.wrapping_add(di).wrapping_add(4);
    let b = c.byte(at);
    let b = pure::dec(B, b as u16);
    c.set_byte(at, b as u8);
    let si = c.word(g::PLACE);
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
    let value = c.economy().shipment_ratio;
    // `cmp al, t; adc ah, cl` for each threshold.
    let grade = [1u8, 0x40, 0x80, 0x90, 0xff].iter().filter(|&&t| value < t).count() as u8;
    c.set_reg8(r8::AL, value);
    c.set_reg8(r8::CL, 0);
    c.set_reg8(r8::AH, grade);
    c.ret()
}

/// `loc_0272f`: the oldest of the [`g::MESSAGES`] dropped (the 9 after
/// it moved down, a 0 after), when [`g::MESSAGE_COUNT`] counts any.
pub fn drop_oldest_message(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x272f)?;
    let mut c = Ctx::new(m);
    let n = c.economy().message_count;
    if n == 0 {
        c.run(start, 4)?;
        c.set_reg(CX, 0);
        return c.ret();
    }
    c.run(start, 11 + 9 + 3)?; // the `rep` iterations too
    c.set_reg(CX, n as u16);
    c.set_reg(SI, g::MESSAGES);
    let v = pure::dec(B, n as u16);
    c.update_economy(|t| t.message_count = v as u8);
    c.set_reg(DI, g::MESSAGES);
    let si = pure::add(W, g::MESSAGES, 2);
    c.set_reg(SI, si);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.set_reg(CX, 9);
    c.rep_counted(0xa5);
    c.set_reg(AX, 0);
    c.stos(W);
    c.ret()
}

/// `loc_02a34`: the oldest of [`g::PENDING_EVENTS`] dropped: the count
/// one down (at 0, [118Fh] cleared), the entries shifted down one, zeros
/// after.
pub fn drop_oldest_event(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2a34)?;
    let mut c = Ctx::new(m);
    let v = c.byte(g::PENDING_EVENTS).wrapping_sub(1);
    c.run(start, (3 + (v == 0) as u32 + 7) + 0x12)?; // the `rep` iterations too
    c.set_reg(SI, g::PENDING_EVENTS);
    let b = c.byte(g::PENDING_EVENTS);
    let v = pure::dec(B, b as u16);
    c.set_byte(g::PENDING_EVENTS, v as u8);
    if v == 0 {
        c.set_byte(g::PENDING_EVENTS_FLAG, 0);
    }
    let si = pure::inc(W, g::PENDING_EVENTS);
    c.set_reg(DI, si);
    let si = pure::add(W, si, 4);
    c.set_reg(SI, si);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.set_reg(CX, 0x12);
    c.rep_counted(0xa5);
    c.run(0x2a4c, 4)?;
    c.set_reg(AX, 0);
    c.stos(W);
    c.stos(W);
    c.ret()
}

/// `loc_028e1`: at hour 0Ch [46D9h] = 7; when [47A9h] is set: the screen
/// shown, transition 8 (BP = 2DD3h), the flag cleared and the glow faded
/// out (`comm_fade_out_glow`); [47A6h] = FFh.
pub fn close_message(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x28e1)?;
    let mut c = Ctx::new(m);
    let h = c.progress().message_param;
    c.run(start, if h == 0x0c { 3 } else { 2 })?;
    if h == 0x0c {
        c.call(entry::SET_46D9_TO_7, 0x28eb)?;
    }
    let f = c.conversation().caller;
    c.run(0x28eb, 3)?;
    if f == 0 {
        return c.ret();
    }
    c.call(entry::GFX_COPY_FRAMEBUFFER_TO_SCREEN, 0x28f5)?;
    c.run(0x28f5, 4)?;
    c.update_conversation(|t| t.scene_flags = 0xff);
    screen::transition_drawn(&mut c, 0x2dd3, 8, 0x2902)?;
    c.run(0x2902, 2)?;
    c.update_conversation(|t| t.caller = 0);
    c.call(entry::COMM_FADE_OUT_GLOW, 0x290a)?;
    c.run(0x290a, 1)?;
    c.ret()
}

/// `loc_02b00`: a message: with DI, `loc_0331e` first (AX kept); [0EAh] =
/// AL, the phrase AH said (`loc_096d8`, then `loc_09945`); [0EAh] = FFh,
/// `loc_09ef1` with AL = 1; `loc_0331e` on [11CEh] (as it was). CF set.
pub fn speak_event_line(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2b00)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    c.run(start, if di == 0 { 3 } else { 5 })?;
    let w = c.word(g::TOPIC_PLACE_RECORD);
    c.push(w);
    if di != 0 {
        let ax = c.reg(AX);
        c.push(ax);
        c.call(entry::DESCRIBE_PLACE, 0x2b0c)?;
        c.run(0x2b0c, 1)?;
        let ax = c.pop();
        c.set_reg(AX, ax);
    }
    c.run(0x2b0d, 4)?;
    let ax = c.reg(AX);
    c.update_situation(|t| t.event_kind = ax as u8);
    dialogue::speak_as(&mut c, ax >> 8, 0x2b17)?;
    c.run(0x2b17, 1)?;
    c.call(entry::ADD_TWO_HEADS_TEXT_TASK, 0x2b1a)?;
    c.run(0x2b1a, 3)?;
    c.update_situation(|t| t.event_kind = 0xff);
    dialogue::talk_text_voice(&mut c, 1, 0x2b24)?;
    c.run(0x2b24, 2)?;
    let di = c.pop();
    talk_about_location(&mut c, di, 0x2b28)?;
    c.run(0x2b28, 2)?;
    c.set_flag(pc::cpu::CF, true);
    c.ret()
}

/// `loc_02a7f`: the spent entries (kind 1) dropped from [`g::PENDING_EVENTS`],
/// the count fixed and a zero entry after when it shrank.
pub fn drop_told_events(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2a7f)?;
    let mut c = Ctx::new(m);
    let n = c.byte(g::PENDING_EVENTS) as u16;
    if n == 0 {
        c.run(start, 7)?;
        c.set_reg(SI, g::PENDING_EVENTS);
        c.set_reg(CX, 0);
        c.set_reg(BP, 0);
        c.lods(B, pc::cpu::DS);
        return c.ret();
    }
    let mut kept = 0u16;
    let mut count = 9;
    for i in 0..n {
        let k = c.byte(g::PENDING_EVENTS + 1 + 4 * i);
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
    c.set_reg(SI, g::PENDING_EVENTS);
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
    let old = c.byte(g::PENDING_EVENTS);
    c.set_byte(g::PENDING_EVENTS, bp as u8);
    c.set_reg(CX, (bp & 0xff) << 8 | old as u16);
    if shrank {
        c.set_reg(AX, 0);
        c.stos(W);
        c.stos(W);
    }
    c.ret()
}

/// `loc_02aaf`: the entry of [`g::PENDING_EVENTS`] for kind AL (for kind 0Fh, the
/// one whose word is DI): CF set, AX, DI = its words. SI kept.
pub fn find_pending_event(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2aaf)?;
    let mut c = Ctx::new(m);
    let n = c.byte(g::PENDING_EVENTS) as u16;
    let (al, di) = (c.reg8(r8::AL), c.reg(DI));
    let mut count = 5;
    let mut found = None;
    if n == 0 {
        count += 3;
    } else {
        count += 1;
        for i in 0..n {
            let e = g::PENDING_EVENTS + 1 + 4 * i;
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
    // The exit's flags are the last test's (`loop` keeps them), then CF.
    match found {
        None if n == 0 => {
            c.set_reg(SI, g::PENDING_EVENTS);
            c.set_reg(CX, 0);
            c.xor(W, 0, 0); // `xor cx, cx`
        }
        None => {
            let last = g::PENDING_EVENTS + 1 + 4 * (n - 1);
            c.set_reg(SI, last + 4);
            c.set_reg(CX, 0);
            c.add(W, last, 4); // `add si, 4`
        }
        Some(i) => {
            let e = g::PENDING_EVENTS + 1 + 4 * i;
            let (event, place) = (c.word(e), c.word(e.wrapping_add(2)));
            c.set_reg(SI, e);
            c.set_reg(CX, n - i);
            if al != 0x0f {
                c.cmp(B, al as u16, 0x0f);
            } else {
                c.cmp(W, di, place);
            }
            c.set_reg(AX, event);
            c.set_reg(DI, place);
        }
    }
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret_cf(found.is_some())
}

/// [`find_pending_event`] called: whether kind AL has a pending event (AX, DI = its words).
pub(super) fn pending_event_found(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    c.call_cf(0x2aaf, ret)
}

/// `loc_03127`: the positions of characters 0Ch..0Fh reset (7F80h),
/// [476Ah], [476Bh] cleared; at a location ([6] low byte 80h):
/// `loc_06603` with BP = 316Eh, and in a palace (kind 21h) character
/// 0Dh's place and position set to the player's and made current
/// (`loc_02318`); then `loc_0331e`.
pub fn reset_visitor_positions(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3127)?;
    let mut c = Ctx::new(m);
    let bx = c.player().y;
    let at = bx & 0xff == 0x80;
    c.run(start, if at { 14 } else { 11 })?;
    c.set_byte(g::CHARACTER_15_SPECIAL_SLOT, 0);
    c.set_byte(g::CHARACTER_15_TROOP_COUNT, 0);
    c.set_reg(AX, 0x7f80);
    for i in [15, 14, 13, 12] {
        c.update_character(i, |t| t.y = 0x7f80);
    }
    c.set_reg(BX, bx);
    if !at {
        return c.ret();
    }
    let di = c.word(g::PLACE);
    c.set_reg(DI, di);
    let dx = c.player().x;
    c.set_reg(DX, dx);
    troops::run_for_each_troop(&mut c, entry::PLACE_TROOP_MARKER, 0x3157)?;
    let di = c.reg(DI);
    let kind = c.byte(di.wrapping_add(8));
    c.run(0x3157, if kind != 0x21 { 3 } else { 6 })?;
    if kind == 0x21 {
        let (dx, bx) = (c.reg(DX), c.reg(BX));
        c.update_character(13, |t| (t.x, t.y) = (dx, bx));
        c.push(di);
        c.call(entry::VISIT_SMUGGLER, 0x3169)?;
        c.run(0x3169, 2)?;
        let di = c.pop();
        c.set_reg(DI, di);
    }
    c.call(entry::DESCRIBE_PLACE, 0x316d)?;
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
    c.call(entry::RESET_VISITOR_POSITIONS, 0x3096)?;
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
    c.update_progress(|t| t.present = 0);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    run_for_records_here(c, entry::ADD_SPEAKER, 0x30af)?;
    c.run(0x30af, 2)?;
    run_for_records_here(c, entry::ADD_SPEAKER_40, 0x30b5)?;
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
pub fn reset_dialogue_then_set_up_place(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3090)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(entry::RESET_DIALOGUE, 0x3093)?;
    setup_3093(&mut c, 0x3093, 0, |_| Ok(()))
}

/// `loc_03385`: [0F7h] = the bits (byte 0Eh) of the records 1018h, 1028h,
/// 1048h at the location DI, unless it is [1150h].
pub fn characters_at_place(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3385)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let skip = di == c.word(g::LAST_PLACE);
    if skip {
        c.run(start, 4)?;
        c.update_situation(|t| t.place_characters = 0);
        return c.ret();
    }
    let off = di.wrapping_sub(0x100);
    if off / 0x1c > 0xff {
        c.m.cpu.ip = start;
        return Err(Bail);
    }
    c.run(start, 12)?;
    c.update_situation(|t| t.place_characters = 0);
    let off = pure::sub(W, di, 0x100);
    c.set_reg(AX, off);
    c.set_reg8(r8::BL, 0x1c);
    c.m.cpu.mul_div(&mut c.m.hw, B, 6, 0x1c); // div bl
    let ax = c.reg(AX);
    let ax = pure::inc(W, ax);
    c.set_reg(AX, ax);
    c.set_reg(BX, (ax & 0xff) << 8 | 0x80);
    c.set_reg(SI, 0x1018);
    c.call(entry::MARK_RECORD_BIT, 0x33a4)?;
    c.run(0x33a4, 2)?;
    c.set_reg(SI, 0x1028);
    c.call(entry::MARK_RECORD_BIT, 0x33aa)?;
    let bx = c.reg(BX);
    let si = 0x1048u16;
    let w = c.word(si.wrapping_add(2));
    c.run(0x33aa, 1 + if w == bx { 7 } else { 3 })?;
    c.set_reg(SI, si);
    if w == bx {
        let cl = c.byte(si.wrapping_add(0x0e));
        c.set_reg8(r8::CL, cl);
        let bit = pure::shl(B, 1, cl) as u8;
        c.set_reg8(r8::AL, bit);
        let v = c.situation().place_characters;
        let v = pure::or(B, v as u16, bit as u16);
        c.update_situation(|t| t.place_characters = v as u8);
    }
    c.ret()
}

/// `loc_033d9`: AL = the odds in 256 that our side strikes: 80h x the
/// larger of [`g::TOPIC_PLACE_HARKONNEN_STRENGTH`] and
/// [`g::TOPIC_PLACE_OUR_STRENGTH`] / the smaller, at most FCh (also
/// when the smaller is 0); negated when ours is the smaller.
pub fn strike_odds(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x33d9)?;
    let mut c = Ctx::new(m);
    let (a, d) = (c.place_forces().our_strength, c.place_forces().harkonnen_strength);
    let below = a < d;
    let (big, small) = if below { (d, a) } else { (a, d) };
    // DX:AX = big · 100h / small (`div cx`), halved, at most FCh.
    let mut cost = 5 + below as u32 + 2;
    let (ratio, dx) = if small == 0 {
        cost += 1;
        (0xfc, 0)
    } else if big >> 8 >= small {
        cost += 5 + 1;
        (0xfc, big >> 8)
    } else {
        let wide = (big as u32) << 8;
        let q = (wide / small as u32) as u16 >> 1;
        cost += 5 + 4 + (q >= 0xfc) as u32;
        (q.min(0xfc), (wide % small as u32) as u16)
    };
    cost += 2 + below as u32 + 1;
    c.run(start, cost)?;
    // `pushf` / `popf` around it: the word stays below SP.
    c.cmp(W, a, d);
    let kept = c.m.cpu.flags();
    c.push(kept);
    c.pop();
    c.set_reg(CX, small);
    c.set_reg(DX, dx);
    let al = if below { (ratio as u8).wrapping_neg() } else { ratio as u8 };
    c.set_reg(AX, ratio & 0xff00 | al as u16);
    c.ret()
}

/// `loc_0348a`: from `loc_06efd`'s AL: (2 AL + the troop's byte 16h, at
/// most FFh) x its byte 1Ah / 1000h, as AL; at least 1 when byte 1Ah is
/// set.
pub fn troop_strength(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x348a)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let motivation = troops::effective_motivation(&mut c, 0x348d)?;
    let troop = c.reg(SI);
    let [base, men] = [0x16u16, 0x1a].map(|k| c.byte(troop.wrapping_add(k)));
    // `add al, al` drops its carry; the second `add` saturates.
    let (sum, carry) = motivation.wrapping_add(motivation).overflowing_add(base);
    let sum = if carry { 0xff } else { sum };
    let strength = ((sum as u16 * men as u16) >> 12) as u8;
    c.run(0x348d, 4 + carry as u32 + 6 + 4 + if strength == 0 { 4 } else { 1 })?;
    let strength = if strength == 0 { (men != 0) as u8 } else { strength };
    c.set_reg(AX, strength as u16);
    c.ret()
}

/// `loc_034a5`: the 33h bytes at 60h cleared, `loc_06639` with BP =
/// 34D0h; then [91h] = [60h] + [7Eh], [92h] = [61h] + [7Fh]. SI kept.
pub fn count_place_troops(m: &mut Machine, start: u16) -> Result<(), Bail> {
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
    c.set_reg(DI, g::TOPIC_PLACE_OUR_TROOPS);
    c.set_reg(CX, 0x33);
    c.set_reg8(r8::AL, 0);
    c.rep(0xaa); // rep stosb
    c.run(0x34b3, 3)?;
    let di = c.pop();
    c.set_reg(DI, di);
    troops::run_for_location_troops(&mut c, entry::COUNT_TROOP_AT_PLACE, 0x34ba)?;
    c.run(0x34ba, 8)?;
    for (a, b, to) in [
        (g::TOPIC_PLACE_OUR_TROOPS, g::TOPIC_PLACE_OUR_INBOUND, g::TOPIC_PLACE_OUR_TOTAL),
        (g::TOPIC_PLACE_HARKONNEN_TROOPS, g::TOPIC_PLACE_HARKONNEN_INBOUND, g::TOPIC_PLACE_HARKONNEN_TOTAL),
    ] {
        let x = c.byte(a);
        let y = c.byte(b);
        let s = pure::add(B, x as u16, y as u16) as u8;
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
    let (ax, cx, ring) = (c.reg(AX), c.reg(CX), c.reg(DI));
    let [size, offset] = cx.to_le_bytes();
    if size == 0 {
        return Err(Bail);
    }
    // (AL + CH) mod CL by repeated `sub` until it borrows.
    let mut at = (ax as u8).wrapping_add(offset);
    let mut rounds = 0;
    loop {
        rounds += 1;
        let (r, borrow) = at.overflowing_sub(size);
        at = r;
        if borrow {
            break;
        }
    }
    let wanted = ax & 0xff00 | at.wrapping_add(size) as u16;
    let mut cost = 2 + 2 * rounds + 3;
    let (bx, slot) = if c.byte(wanted.wrapping_add(ring)) == 0xff {
        cost += 2;
        (wanted, Some(wanted))
    } else {
        // Else the first free one.
        cost += 1;
        let mut k = 0u16;
        loop {
            cost += 3;
            if k as u8 >= size {
                cost += 1;
                break (k, None);
            }
            cost += 2;
            if c.byte(k.wrapping_add(ring)) == 0xff {
                cost += 2;
                break (k, Some(k));
            }
            k += 1;
        }
    };
    c.run(start, cost)?;
    c.set_reg(BX, bx);
    if let Some(k) = slot {
        c.set_byte(k.wrapping_add(ring), ax as u8);
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
    c.set_reg(SI, g::CHARACTERS);
    c.set_reg(CX, 0x10);
    let (y, x) = (c.player().y, c.player().x);
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
            let (y, x) = (c.player().y, c.player().x);
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

/// [`for_records_here_bp`] called: BP's routine for each record here.
pub(super) fn run_for_records_here(c: &mut Ctx, routine: u16, ret: u16) -> Result<(), Bail> {
    c.set_reg(BP, routine);
    c.call(entry::FOR_RECORDS_HERE_BP, ret)
}

/// `loc_040c3`: [`for_records_here`] with `move_following_character`.
pub fn move_party_here(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x40c3)?;
    for_records_here(&mut Ctx::new(m), start, 2, |c| c.set_reg(BP, 0x40c9))
}

/// `loc_04944`: `loc_050be`; with DI from FFF0h up (a screen point) the
/// map point under it (`loc_0b5f9`) made the place ([11CBh] one down,
/// `get_map_position`, `loc_05133`), DI = [1150h], CL = 1; else
/// `loc_05124` and CL = 0. Then [11C5h] = DI, [11C8h] = CL and
/// [`add_11c7`].
pub fn set_up_flight(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4944)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(entry::STOP_KEEPING_HEADING, 0x4947)?;
    let di = c.reg(DI);
    if di < 0xfff0 {
        c.run(0x4947, 3)?;
        c.call(entry::HEAD_FOR_LOCATION, 0x4968)?;
        return add_11c7(&mut c, 0x4968, 5, |c| {
            let cx = c.reg(CX);
            let z = pure::xor(W, cx, cx);
            c.set_reg(CX, z);
            let di = c.reg(DI);
            c.update_travel(|t| t.target = di);
            c.update_travel(|t| t.no_target = 0);
            c.update_travel(|t| t.heading = 0);
        });
    }
    c.run(0x4947, 4)?;
    let v = c.travel().keep_heading;
    let v = pure::dec(B, v as u16);
    c.update_travel(|t| t.keep_heading = v as u8);
    c.call(entry::SCREEN_TO_MAP, 0x4953)?;
    c.run(0x4953, 3)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.set_reg(CX, bx);
    c.set_reg(DI, dx);
    c.call(code::GET_MAP_POSITION, 0x495a)?;
    c.run(0x495a, 1)?;
    c.call(entry::DIRECTION_BETWEEN, 0x495d)?;
    add_11c7(&mut c, 0x495d, 7, |c| {
        let di = c.word(g::LAST_PLACE);
        c.set_reg(DI, di);
        c.set_reg8(r8::CL, 1);
        c.update_travel(|t| t.target = di);
        c.update_travel(|t| t.no_target = 1);
        c.update_travel(|t| t.heading = 0);
    })
}

/// `loc_05124`: `loc_05133` for the map position and the place at DI
/// (its words 2, 4 as DI, CX). DI kept.
pub fn head_for_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
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
    c.call(entry::DIRECTION_BETWEEN, 0x5131)?;
    c.run(0x5131, 2)?;
    let di = c.pop();
    c.set_reg(DI, di);
    c.ret()
}

/// [`head_for_location`] called: whether the map position is the place at DI itself (no direction to it).
pub(super) fn already_there(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    c.call_cf(0x5124, ret)
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

/// [`place_phrase`] called: the phrase number of the place DI (AX).
pub(super) fn place_phrase_number(c: &mut Ctx, ret: u16) -> Result<u16, Bail> {
    c.call(entry::PLACE_PHRASE, ret)?;
    Ok(c.reg(AX))
}

/// `loc_04658`: unless a message runs ([473Fh]): phrase 57h shown as a
/// message (its far pointer at 473Fh, 55h at 4743h, the box at 4745h by
/// [473Eh]) by the frame task 46B5h every 18h ticks. Registers kept.
pub fn start_message_57(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4658)?;
    let mut c = Ctx::new(m);
    let v = c.typed_message().next[0];
    c.run(start, 3)?;
    if v != 0 {
        return c.ret();
    }
    c.call(code::SAVE_REGS, 0x4662)?;
    c.run(0x4662, 2)?;
    text::string_ptr(&mut c, 0x57, 0x4668)?;
    let f = c.byte(g::GAME_FRAME);
    c.run(0x4668, 7 + if f != 0 { 2 } else { 0 } + 5)?;
    let es = c.sreg(pc::cpu::ES);
    c.update_typed_message(|t| t.next[1] = es);
    let si = c.reg(SI);
    c.update_typed_message(|t| t.next[0] = si);
    c.update_typed_message(|t| t.pen[0] = 0x55);
    let mut cx = 0xf561u16;
    let mut ax = 0x22u16;
    if f != 0 {
        ax = pure::add(B, ax, 4);
        cx = 0x2000 | cx & 0xff;
    }
    c.set_reg(CX, cx);
    c.set_reg(AX, ax);
    c.update_typed_message(|t| t.pen[1] = ax);
    c.update_typed_message(|t| t.colour = cx);
    tasks::add_task(&mut c, entry::MESSAGE_TASK, 0x18, 0x4697)?;
    c.run(0x4697, 1)?;
    c.call(code::RESTORE_REGS, 0x469a)?;
    c.run(0x469a, 1)?;
    c.ret()
}

/// `loc_0368b`: by the screen kind (bits 3 of [11C9h]): 2, `loc_04aeb` and
/// the game rect copied; 1, picture 34h with message AX (`loc_0978e`,
/// unless negative), the screen copied ([473Eh], [47A4h] set). [4728h]
/// bit 0 set. Registers kept.
pub fn show_speaker_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x368b)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SAVE_REGS, 0x368e)?;
    let k = c.travel().screen_kind & 3;
    c.run(0x368e, if k == 2 { 6 } else if k == 1 { 12 } else { 8 })?;
    let v = c.flight().marker;
    let v = pure::or(B, v as u16, 1);
    c.update_flight(|t| t.marker = v as u8);
    let bl = c.travel().screen_kind;
    let bl = pure::and(B, bl as u16, 3) as u8;
    c.set_reg8(r8::BL, bl);
    if k == 2 {
        c.call(entry::SCENE_39, 0x36ce)?;
        c.run(0x36ce, 1)?;
        c.call(entry::COPY_GAME_RECT_FROM_FB2_TO_FB1, 0x36d1)?;
        c.run(0x36d1, 2)?;
    } else {
        let bl = pure::dec(B, bl as u16) as u8;
        c.set_reg8(r8::BL, bl);
        if k == 1 {
            c.set_byte(g::GAME_FRAME, 1);
            c.update_conversation(|t| t.line_state = 1);
            let ax = c.reg(AX);
            c.push(ax);
            gfx::draw_resource_sprite_0(&mut c, 0x34, 0x36b3)?;
            c.run(0x36b3, 1)?;
            c.call(entry::UPDATE_SCREEN_PALETTE, 0x36b6)?;
            c.run(0x36b6, 1)?;
            c.call(entry::COPY_ACTIVE_FRAMEBUFFER_TO_FRAMEBUFFER_2, 0x36b9)?;
            let sp = c.reg(pc::cpu::SP);
            let ax = c.word_in(pc::cpu::SS, sp);
            let neg = ax & 0x8000 != 0;
            c.run(0x36b9, if neg { 4 } else { 5 })?;
            c.pop();
            c.set_reg(AX, ax);
            if !neg {
                c.update_conversation(|t| t.talker = ax);
                c.call(entry::REDRAW_DIALOGUE_SCENE, 0x36c4)?;
                c.run(0x36c4, 1)?;
            }
            c.call(entry::UPDATE_GAME_AREA_ON_SCREEN, 0x36c7)?;
            c.run(0x36c7, 1)?;
        }
    }
    c.call(code::RESTORE_REGS, 0x36ca)?;
    c.run(0x36ca, 1)?;
    c.ret()
}

/// `loc_0503c`: entering the location at DI: [0FDh], [2Bh] cleared; a
/// place the Harkonnens hold or occupy ([2Bh] set, `loc_06144`, [11DDh] = its picture 2Fh..33h
/// by kind) when it has bit 2 of byte 0Ah or is of a kind not passed by
/// `location_kind_check` with troops counted (`loc_05098`, DX); else with
/// CX, [46D9h] = 4.
pub fn enter_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x503c)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let f = Location(di).flags(&mut c);
    c.run(start, if f & location_flag::HARKONNEN != 0 { 4 } else { 5 })?;
    c.update_situation(|t| t.battle_balance = 0);
    c.update_progress(|t| t.harkonnen_place = 0);
    if f & location_flag::HARKONNEN == 0 {
        if map::location_skipped(&mut c, 0x504f)? {
            c.run(0x504f, 2)?;
            return c.ret();
        }
        c.run(0x504f, 2)?;
        let (cx, dx) = troops::troop_sides(&mut c, 0x5054)?;
        if dx == 0 {
            c.run(0x5054, if cx == 0 { 4 } else { 5 })?;
            if cx != 0 {
                c.set_byte(g::COMM_CALLER, 4);
            }
            return c.ret();
        }
        c.run(0x5054, 2)?;
    }
    c.run(0x5058, 2)?;
    let v = c.progress().harkonnen_place;
    let v = pure::inc(B, v as u16);
    c.update_progress(|t| t.harkonnen_place = v as u8);
    c.call(entry::NOTE_BATTLE_BALANCE, 0x505f)?;
    let di = c.reg(DI);
    let kind = c.byte(di.wrapping_add(8));
    let n = if kind < 0x20 { 4 } else if kind == 0x30 { 7 } else if kind < 0x28 { 9 } else { 10 };
    c.run(0x505f, n + 2)?;
    c.set_reg8(r8::AL, kind);
    let mut ah = 0x2fu16;
    let mut al = kind as u16;
    if kind >= 0x20 {
        ah = pure::inc(B, ah);
        if kind != 0x30 {
            al = pure::sub(B, kind as u16, 0x28);
            if kind >= 0x28 {
                ah = pure::add(B, ah, 3);
            }
        }
    }
    c.set_reg(AX, ah << 8 | al);
    c.set_byte(g::HARKONNEN_PLACE_ICONS, ah as u8);
    c.ret()
}

/// [`enter_location`] called: entering the location at DI.
pub(super) fn enter_location_at(c: &mut Ctx, location: u16, ret: u16) -> Result<(), Bail> {
    c.set_reg(DI, location);
    c.call(entry::ENTER_LOCATION, ret)
}

/// `sub_4415`: leaving the map's hotspot mode (when [46EBh] was set): the
/// markers cleared, the mouse rect, the frame task 44ABh and the message
/// gone, the rects and panel put back (`loc_05ba0`, `loc_043e3`, the
/// palette, the buttons, `loc_02ffb`; [11C9h] cleared without a place;
/// `loc_049d4` with [4728h] positive); then [11CAh] cleared.
pub fn leave_hotspot_mode(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4415)?;
    let mut c = Ctx::new(m);
    let k = c.map_view().mode;
    c.run(start, if k == 0 { 5 } else { 6 })?;
    c.update_map_view(|t| t.mode = 0);
    c.set_reg8(r8::AL, k);
    if k == 0 {
        return c.ret();
    }
    c.set_word(g::MAP_MARKERS, 0);
    c.call(entry::CLEAR_SOME_MOUSE_RECT, 0x4429)?;
    c.run(0x4429, 2)?;
    tasks::remove_task(&mut c, entry::BLINK_MAP_POINTER, 0x442f)?;
    let mut at = 0x442f;
    for t in [0x469b, 0x5ba0, 0x43e3, 0xc0f4] {
        c.run(at, 1)?;
        c.call(t, at + 3)?;
        at += 3;
    }
    let place = c.travel().target;
    c.run(0x443b, 2 + (place == 0) as u32 + 1)?;
    if place == 0 {
        c.update_travel(|t| t.screen_kind = 0);
    }
    c.call(entry::SET_MAP_CLICK_HANDLER, 0x444a)?;
    c.run(0x444a, 1)?;
    c.call(entry::SET_BUTTONS_FOR_SCREEN, 0x444d)?;
    c.run(0x444d, 1)?;
    c.call(entry::PANEL_BUTTONS, 0x4450)?;
    let f = c.flight().marker;
    let pos = (f as i8) > 0;
    c.run(0x4450, 2 + pos as u32 + if pos { 0 } else { 1 })?;
    if pos {
        c.call(entry::OPEN_ZOOMED_MAP_WITH_POINTS, 0x445a)?;
        c.run(0x445a, 1)?;
    }
    release_travel(c.m, 0x4ac4)
}

/// `sub_46b5` (a frame task): the message's next character drawn (small
/// font, colour and place at 4743h..4747h, on the screen), again for a
/// space; a negative byte ends it.
pub fn message_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x46b5)?;
    let mut c = Ctx::new(m);
    loop {
        let [si, seg] = c.typed_message().next;
        let ch = c.byte_at_seg(seg, si);
        let end = ch & 0x80 != 0;
        c.run(0x46b5, if end { 5 } else { 7 })?;
        c.set_reg(SI, si);
        c.set_sreg(pc::cpu::ES, seg);
        c.set_reg8(r8::AL, ch);
        if end {
            return c.ret();
        }
        let p = c.typed_message().next[0];
        let p = pure::inc(W, p);
        c.update_typed_message(|t| t.next[0] = p);
        mouse::hide_mouse_over(&mut c, 0x14a4, 0x46ca)?;
        c.run(0x46ca, 2)?;
        let fb = c.screen_buffers().active;
        c.push(fb);
        c.call(entry::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0x46d1)?;
        c.run(0x46d1, 3)?;
        let [x, y] = c.typed_message().pen;
        video::set_draw_position(&mut c, x, y, 0x46dc)?;
        c.run(0x46dc, 3)?;
        let colour = c.typed_message().colour;
        c.set_reg(CX, colour);
        c.set_word(g::TEXT_COLOUR, colour);
        c.call(entry::FONT_SELECT_SMALL_FONT, 0x46e7)?;
        c.run(0x46e7, 2)?;
        let ax = c.reg(AX);
        c.push(ax);
        c.call(entry::SMALL_FONT_GLYPH, 0x46eb)?;
        c.run(0x46eb, 1)?;
        c.call(entry::FONT_GET_DRAW_POSITION, 0x46ee)?;
        c.run(0x46ee, 5)?;
        let (dx, bx) = (c.reg(DX), c.reg(BX));
        c.update_typed_message(|t| t.pen[0] = dx);
        c.update_typed_message(|t| t.pen[1] = bx);
        let ax = c.pop();
        c.set_reg(AX, ax);
        let fb = c.pop();
        c.update_screen_buffers(|t| t.active = fb);
        c.call(entry::DRAW_MOUSE_CURSOR_IF_NEEDED, 0x46fe)?;
        let al = c.reg8(r8::AL);
        c.run(0x46fe, if al == 0x20 { 2 } else { 3 })?;
        if al != 0x20 {
            return c.ret();
        }
    }
}

/// `loc_08347`: the 3 words at 11D5h moved down to 11D3h. SI, DI kept; ES
/// = DS.
pub fn shift_followed_troop_note(m: &mut Machine, start: u16) -> Result<(), Bail> {
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
    c.call(entry::SWAP_BYTE_ARRAYS, 0x999a)?;
    c.run(0x999a, 3)?;
    c.set_reg(SI, 0x22a6);
    c.set_reg(CX, 1);
    c.call(entry::SWAP_BYTE_ARRAYS, 0x99a3)?;
    c.run(0x99a3, 3)?;
    c.set_reg(SI, 0x4540);
    c.set_reg(CX, 0xc9);
    c.call(entry::SWAP_BYTE_ARRAYS, 0x99ac)?;
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
    let si = c.word(g::CS_SCRIPT_PTR);
    let b = c.byte_in(pc::cpu::CS, si);
    c.run(start, if b == 0xff { 8 } else { 5 })?;
    if b == 0xff {
        c.set_reg(SI, si);
        let s = c.code_word(0x167);
        let v = c.code_word(s);
        let s = pure::add(W, s, v);
        c.set_reg(SI, s);
        c.set_word(g::CS_SCRIPT_PTR, s);
    } else {
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
            c.set_word(g::CS_SCRIPT_PTR, si);
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
        c.call(entry::DRAW_CS_SPRITE_LIST, 0x4d96)?;
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
    let es = c.screen_buffers().front;
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
        c.set_reg(BX, ax);
        let dx = c.lods(W, pc::cpu::DS);
        c.set_reg(DX, dx);
        let ax = c.lods(W, pc::cpu::DS);
        let bx = c.reg(BX);
        c.set_reg(BX, ax);
        c.set_reg(AX, bx);
        c.call(entry::DRAW_SPRITE_CLOBBERING_BX_DX, 0x26cb)?;
        c.run(0x26cb, 5)?;
    } else {
        let _ = step;
        c.run(0x26bb, 9)?;
        let s = c.pop();
        c.set_reg(SI, s);
        c.lods(W, pc::cpu::DS);
    }
    c.set_reg(DX, 0x7e);
    c.set_reg(BX, 0x4c);
    c.set_reg(BP, 0xc2);
    c.set_reg(AX, 0x4c);
    let h = c.word(g::MAP_CLICK_HANDLER);
    if h == 0x1ad6 {
        c.run(0xc526, 3)?;
        return c.ret();
    }
    c.run(0xc526, 5)?;
    c.cmp(W, h, 0x1ad6); // through the driver's far call (reads none); not shown unread after
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.screen_buffers().screen;
    let nds = c.screen_buffers().front;
    c.set_sreg(pc::cpu::ES, es);
    c.set_sreg(pc::cpu::DS, nds);
    c.interp(0xc537, 0xc53c)?;
    c.run(0xc53c, 2)?;
    let ds = c.pop();
    c.set_sreg(pc::cpu::DS, ds);
    c.ret()
}

/// `loc_0e851`: AX = [39B9h] + 2F13h compared with [0CE68h] (flags).
pub fn check_allocator_room(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe851)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let a = c.word(g::ALLOCATOR_NEXT_SEGMENT);
    let a = pure::add(W, a, 0x2f13);
    c.set_reg(AX, a);
    let b = c.word(g::ALLOCATOR_LAST_SEGMENT);
    c.cmp(W, a, b);
    c.ret()
}

/// [`check_allocator_room`] called: whether [39B9h] + 2F13h is below [0CE68h].
pub(super) fn allocator_below_limit(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    c.call_cf(0xe851, ret)
}

/// `loc_00d45`: the sky by the time of day redrawn, then three times
/// darkened by 28h with a wait for [46D7h] to run from 20h below 10h;
/// then a wait for it to reach 0. [227Dh] counted up meanwhile.
pub fn sky_flash(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0d45)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(entry::SKY_PALETTE_FOR_GAME_TIME, 0x0d48)?;
    c.run(0x0d48, 1)?;
    c.call(entry::OPEN_SKY_OR_SKYDN_PALETTE_AL_SUB_BL, 0x0d4b)?;
    c.run(0x0d4b, 1)?;
    c.call(entry::SEND_SKY_PALETTE_ROWS, 0x0d4e)?;
    c.run(0x0d4e, 2)?;
    let v = c.byte(g::CINEMATIC);
    let v = pure::inc(B, v as u16);
    c.set_byte(g::CINEMATIC, v as u8);
    c.set_reg(CX, 3);
    loop {
        c.run(0x0d55, 3)?;
        let cx = c.reg(CX);
        c.push(cx);
        c.set_reg8(r8::BL, 0x28);
        c.call(entry::OPEN_SKY_OR_SKYDN_PALETTE_AL_SUB_BL, 0x0d5b)?;
        c.run(0x0d5b, 1)?;
        c.call(entry::SEND_SKY_PALETTE_ROWS_DEFAULT_DRIVER, 0x0d5e)?;
        c.run(0x0d5e, 1)?;
        c.call(entry::UPDATE_SCREEN_PALETTE, 0x0d61)?;
        c.run(0x0d61, 4)?;
        c.set_byte(g::FRAME_TASKS_ACTIVE, 0x20);
        loop {
            frame::draw_then_wait(&mut c, entry::SKY_FADE_STEP, 3, 0x0d6f)?;
            let k = c.byte(g::FRAME_TASKS_ACTIVE);
            if k >= 0x10 {
                c.run(0x0d6f, 5)?;
                continue;
            }
            c.run(0x0d6f, 4)?;
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
        frame::draw_then_wait(&mut c, entry::SKY_FADE_STEP, 3, 0x0d82)?;
        let k = c.byte(g::FRAME_TASKS_ACTIVE);
        if k != 0 {
            c.run(0x0d82, 5)?;
            continue;
        }
        c.run(0x0d82, 4)?;
        break;
    }
    let v = c.byte(g::CINEMATIC);
    let v = pure::dec(B, v as u16);
    c.set_byte(g::CINEMATIC, v as u8);
    c.ret()
}

/// `loc_04ec6`: the flight's next video ([DC02h], `video_open`) from
/// [`g::FLIGHT_VIDEO`] (below 2 itself), the terrain's low nibble in AL
/// and the video asked for ([`g::HNM_VIDEO`], AX): nibble 8 or more
/// (rough) gives 3 (4 for AX past 2 but not 5); below, AX up to 2 keeps
/// [`g::FLIGHT_VIDEO`], up to 4 gives 5, past 4 gives 2 (also into
/// [`g::FLIGHT_VIDEO`]). BX kept.
pub fn next_flight_video(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4ec6)?;
    let mut c = Ctx::new(m);
    let bx0 = c.reg(BX);
    let b = c.word(g::FLIGHT_VIDEO);
    let al = c.reg8(r8::AL);
    let ax = c.video_state().video;
    if b < 2 {
        c.run(start, 4 + 3)?;
        c.push(bx0);
        c.update_video_state(|t| t.video_open = b);
    } else if al & 0x0f < 8 {
        let n = if ax <= 2 { 2 } else if ax <= 4 { 5 } else { 7 };
        c.run(start, 8 + n + 3)?;
        c.push(bx0);
        let _ = pure::and(B, al as u16, 0x0f);
        c.set_reg(AX, ax);
        let mut v = b;
        if ax > 2 {
            v = 5;
            if ax > 4 {
                v = 2;
                c.set_word(g::FLIGHT_VIDEO, 2);
            }
        }
        c.update_video_state(|t| t.video_open = v);
    } else {
        let n = if ax <= 2 { 3 } else if ax == 5 { 5 } else { 6 };
        c.run(start, 8 + n + 3)?;
        c.push(bx0);
        let _ = pure::and(B, al as u16, 0x0f);
        c.set_reg(AX, ax);
        let mut v = 3;
        if ax > 2 {
            if ax != 5 {
                v = 4;
            }
        }
        c.update_video_state(|t| t.video_open = v);
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
    let ph = c.progress().phase;
    let al = ph.wrapping_sub(0x2d);
    let w = c.progress().party;
    let di = c.word(g::TICK_LOCATION);
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
    talk_about_location(&mut c, di, 0x6c62)?;
    let k = c.place_forces().counts[6];
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
pub fn arrive_and_update_party(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x40d5)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.update_progress(|t| t.talk_reason = 7);
    c.call(entry::HANDLE_FLAG_23, 0x40dd)?;
    c.run(0x40dd, 1)?;
    c.call(entry::RELEASE_TRAVEL, 0x40e0)?;
    c.run(0x40e0, 2)?;
    c.set_reg(BP, 0x40e6);
    for_records_here_bp(c.m, 0x36ee)
}

/// `loc_04182`: in mode 1 of [11C9h]: unless [11CBh] is clear with the
/// location [11C5h] past `location_kind_check` (CF), at a map place
/// (`get_map_position`, `loc_0b532`) with bits 30h, `note_21fd_al` 40h
/// ([23h] 4 the first time), and after the [4726h] countdown by 20h
/// [46D9h] = 2; else [4726h] cleared and [`reset_desert_countdown`].
pub fn map_place_watch(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4182)?;
    let mut c = Ctx::new(m);
    let mode = c.travel().screen_kind;
    let al = (mode & 3).wrapping_sub(1);
    if al != 0 {
        c.run(start, 5)?;
        c.set_reg8(r8::AL, mode);
        let a = pure::and(B, mode as u16, 3);
        let a = pure::dec(B, a);
        c.set_reg8(r8::AL, a as u8);
        return c.ret();
    }
    let k = c.travel().keep_heading;
    c.run(start, if k == 0 { 8 } else { 7 })?;
    c.set_reg8(r8::AL, mode);
    let a = pure::and(B, mode as u16, 3);
    let a = pure::dec(B, a);
    c.set_reg8(r8::AL, a as u8);
    let drop_it = |c: &mut Ctx, at: u16, n: u32| -> Result<(), Bail> {
        c.run(at, n)?;
        c.set_byte(g::DESERT_COUNTDOWN, 0);
        c.set_reg8(r8::AL, 0);
        note_21fd_al(c.m, 0x41cc)
    };
    if k == 0 {
        let di = c.travel().target;
        c.set_reg(DI, di);
        if map::location_skipped(&mut c, 0x4199)? {
            return drop_it(&mut c, 0x4199, 3);
        }
        c.run(0x4199, 2)?;
    }
    c.call(entry::GET_MAP_POSITION, 0x419e)?;
    c.run(0x419e, 1)?;
    let cell = globe::globe_map_byte(&mut c, 0x41a1)?;
    let a = cell & 0x30;
    if a < 0x30 {
        c.run(0x41a1, 3)?;
        let a = pure::and(B, cell as u16, 0x30);
        c.set_reg8(r8::AL, a as u8);
        return drop_it(&mut c, 0x41c5, 2);
    }
    let t = c.byte(g::DESERT_COUNTDOWN);
    c.run(0x41a1, 5 + (t == 0) as u32 + 2)?;
    let a = pure::and(B, cell as u16, 0x30);
    c.set_reg8(r8::AL, a as u8);
    if t == 0 {
        c.update_progress(|t| t.talk_reason = 4);
    }
    c.set_reg8(r8::AL, 0x40);
    c.call(entry::NOTE_21FD_AL, 0x41b8)?;
    let t = c.byte(g::DESERT_COUNTDOWN);
    let r = t.wrapping_sub(0x20);
    c.run(0x41b8, if r != 0 { 3 } else { 4 })?;
    let r = pure::sub(B, t as u16, 0x20);
    c.set_byte(g::DESERT_COUNTDOWN, r as u8);
    if r != 0 {
        return c.ret();
    }
    c.set_byte(g::COMM_CALLER, 2);
    c.ret()
}

/// `loc_026da`: the message AX (a phrase) added to the
/// [`g::MESSAGES`] unless there (registers kept): with 10 already, the
/// oldest dropped (`loc_0272f`) first; [`g::MESSAGE_COUNT`] and
/// [`g::UNREAD_MESSAGES`] counted; from phase 38h, out of room 8
/// ([0Bh]), `loc_029ee` with AX 201h.
pub fn add_message(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x26da)?;
    let mut c = Ctx::new(m);
    add_message_in(&mut c)
}

fn add_message_in(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x26da, 1)?;
    c.call(entry::SAVE_REGS, 0x26dd)?;
    let ax = c.reg(AX);
    let n0 = c.economy().message_count as u16;
    let mut found = false;
    let mut k = 0u16;
    while k < n0 {
        if c.word(g::MESSAGES + 2 * k) == ax {
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
        c.set_reg(SI, g::MESSAGES);
        for i in 0..=k.min(n0.saturating_sub(1)) {
            let p = g::MESSAGES + 2 * i;
            let w = c.word(p);
            c.cmp(W, ax, w);
            if found && i == k {
                c.set_reg(SI, p);
                c.set_reg(CX, n0 - k);
                c.call(entry::RESTORE_REGS, 0x272e)?;
                c.run(0x272e, 1)?;
                return c.ret();
            }
            c.add(W, p, 2);
            c.set_reg(SI, p + 2);
        }
        c.set_reg(CX, 0);
    }
    let cl = c.economy().message_count;
    let full = cl >= 0x0a;
    c.run(0x26f1, 4 + if full { 2 } else { 0 })?;
    c.set_reg8(r8::CL, cl);
    c.set_reg(SI, g::MESSAGES);
    c.cmp(B, cl as u16, 0x0a);
    let mut cl = cl;
    if full {
        c.push(ax);
        c.call(entry::DROP_OLDEST_MESSAGE, 0x2701)?;
        c.run(0x2701, 3)?;
        cl = 9;
        c.set_reg8(r8::CL, 9);
        c.set_reg(SI, g::MESSAGES);
        let a = c.pop();
        c.set_reg(AX, a);
    }
    let ph = c.progress().phase;
    let room = c.progress().room;
    let call = ph >= 0x38 && room != 8;
    c.run(0x2707, 8 + if ph >= 0x38 { 2 + if call { 2 } else { 0 } } else { 0 } + if call { 0 } else { 1 })?;
    let d = c.add(B, cl as u16, cl as u16);
    c.xor(B, 0, 0);
    c.set_reg(CX, d);
    let si = c.add(W, g::MESSAGES, d);
    c.set_reg(SI, si);
    let ax = c.reg(AX);
    c.set_word(si, ax);
    let v = c.economy().message_count;
    let v = c.inc(B, v as u16);
    c.update_economy(|t| t.message_count = v as u8);
    let v = c.economy().unread_messages;
    let v = c.inc(B, v as u16);
    c.update_economy(|t| t.unread_messages = v as u8);
    c.cmp(B, ph as u16, 0x38);
    if ph >= 0x38 {
        c.cmp(B, room as u16, 8);
    }
    if call {
        add_event(c, 0x201, 0x272b)?;
        c.run(0x272b, 1)?;
    }
    c.call(entry::RESTORE_REGS, 0x272e)?;
    c.run(0x272e, 1)?;
    c.ret()
}

/// `sub_1045`: record FD8h's byte 0 = 5, `loc_0101b`, then message 10Bh
/// ([`add_message`]).
pub fn event_10b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1045)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.update_character(0, |t| t.x = t.x & 0xff00 | 5);
    c.call(entry::SET_0FE8_TO_9, 0x104d)?;
    c.run(0x104d, 2)?;
    c.set_reg(AX, 0x10b);
    add_message_in(&mut c)
}

/// `sub_1103`: `loc_029ee` with AX 4, then message 1409h ([`add_message`]).
pub fn on_phase_30(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1103)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    add_event(&mut c, 4, 0x1109)?;
    c.run(0x1109, 2)?;
    c.set_reg(AX, 0x1409);
    add_message_in(&mut c)
}

/// `loc_03950`: [46D7h] = 0 and the frame task 3916h removed.
pub fn stop_sky_fade(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3950)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_byte(g::FRAME_TASKS_ACTIVE, 0);
    c.set_reg(SI, 0x3916);
    super::tasks::remove_frame_task(c.m, 0xda5f)
}

/// `loc_029f0`: with bit 1 of [0Ah], the event (AX, DI) added to
/// [`g::PENDING_EVENTS`] unless there; with 10
/// already, the oldest dropped ([`drop_oldest_event`]) first.
pub fn add_pending_event(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x29f0)?;
    let mut c = Ctx::new(m);
    let f = c.progress().event_flags;
    if f & 1 == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let (ax, di) = (c.reg(AX), c.reg(DI));
    let n0 = c.byte(g::PENDING_EVENTS) as u16;
    let mut n = 2 + 4;
    let mut found = None;
    if n0 != 0 {
        n += 1;
        for i in 0..n0 {
            let p = g::NEXT_EVENT + 4 * i;
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
    if let Some(i) = found {
        // Already there.
        let at = g::NEXT_EVENT + 4 * i;
        c.set_reg(SI, at);
        c.set_reg(CX, n0 - i);
        return c.ret();
    }
    c.set_reg(SI, g::PENDING_EVENTS);
    c.set_reg(CX, n0);
    if full {
        // The oldest dropped first.
        c.push(ax);
        c.push(g::PENDING_EVENTS);
        c.push(di);
        c.call(entry::DROP_OLDEST_EVENT, 0x2a1f)?;
        c.run(0x2a1f, 4)?;
        let d = c.pop();
        c.set_reg(DI, d);
        let s = c.pop();
        c.set_reg(SI, s);
        let a = c.pop();
        c.set_reg(AX, a);
        c.set_reg(CX, 9);
    }
    // Added at the end.
    let at = (if full { 9 } else { n0 }) * 4;
    c.run(0x2a25, 8)?;
    let count = c.byte(g::PENDING_EVENTS).wrapping_add(1);
    c.set_byte(g::PENDING_EVENTS, count);
    c.set_reg(CX, at);
    c.set_reg(SI, g::NEXT_EVENT + at);
    let (ax, di) = (c.reg(AX), c.reg(DI));
    c.set_word(g::NEXT_EVENT + at, ax);
    c.set_word(g::NEXT_EVENT_PLACE + at, di);
    c.ret()
}

/// `loc_02ad8`: the [`g::NEXT_EVENT`] (AH a bit of [`g::PRESENT`], DI its
/// place [`g::NEXT_EVENT_PLACE`]): when that bit is set (and, for bit 0Fh, it is the
/// current location) it is told: `loc_0dbb2`, `loc_0331e` for its place
/// ([11CEh] kept), the sentence AH by `loc_096d8`, `loc_09945`, `loc_09ef1`
/// with AL 1 ([0EAh] AL meanwhile, then FFh); CF set. Else CF clear.
pub fn tell_pending_event(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2ad8)?;
    let mut c = Ctx::new(m);
    let ev = c.word(g::NEXT_EVENT);
    let present = c.progress().present;
    let cl = (ev >> 8) as u8;
    let sh = (cl & 0x1f) as u32;
    let bit = sh < 16 && (present >> sh) & 1 != 0;
    let di = c.word(g::NEXT_EVENT_PLACE);
    let here = c.word(g::PLACE);
    let tell = bit && (cl != 0x0f || di == here);
    let n = 7 + if bit { 3 + if cl == 0x0f { 2 } else { 0 } } else { 0 };
    c.run(start, n + if tell { 2 } else { 3 })?;
    let ax0 = c.reg(AX);
    c.push(ax0);
    c.set_reg(AX, ev);
    c.set_reg8(r8::CL, cl);
    // The event's bit of [12h] (`shr bx, cl; shr bx, 1`: CF).
    let bits = if sh < 16 { present >> sh } else { 0 };
    c.set_reg(BX, bits >> 1);
    if bit {
        c.set_reg(DI, di);
    }
    if !tell {
        let a = c.pop();
        c.set_reg(AX, a);
        return c.ret_cf(false);
    }
    let sp = c.reg(SP).wrapping_add(2);
    c.set_reg(SP, sp);
    c.call(entry::CALL_RESTORE_CURSOR, 0x2b00)?;
    let di = c.reg(DI);
    let keep = c.word(g::TOPIC_PLACE_RECORD);
    c.run(0x2b00, 3 + if di != 0 { 2 } else { 0 })?;
    c.push(keep);
    if di != 0 {
        let ax = c.reg(AX);
        c.push(ax);
        c.call(entry::DESCRIBE_PLACE, 0x2b0c)?;
        c.run(0x2b0c, 1)?;
        let a = c.pop();
        c.set_reg(AX, a);
    }
    c.run(0x2b0d, 4)?;
    let ax = c.reg(AX);
    c.update_situation(|t| t.event_kind = ax as u8);
    dialogue::speak_as(&mut c, ax >> 8, 0x2b17)?;
    c.run(0x2b17, 1)?;
    c.call(entry::ADD_TWO_HEADS_TEXT_TASK, 0x2b1a)?;
    c.run(0x2b1a, 3)?;
    c.update_situation(|t| t.event_kind = 0xff);
    dialogue::talk_text_voice(&mut c, 1, 0x2b24)?;
    c.run(0x2b24, 2)?;
    let d = c.pop();
    talk_about_location(&mut c, d, 0x2b28)?;
    c.run(0x2b28, 2)?;
    c.ret_cf(true)
}

/// [`tell_pending_event`] called: whether the next pending event was told.
pub(super) fn event_told(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    c.call_cf(0x2ad8, ret)
}

/// `loc_02a51`: the [`g::PENDING_EVENTS`] for the bit AL (for 0Fh only
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
    let cnt = c.byte(g::PENDING_EVENTS);
    if cnt == 0 {
        c.run(start, 9)?;
        c.set_reg(SI, g::NEXT_EVENT);
        c.set_reg8(r8::DL, dl);
        c.set_reg(BX, bx);
        c.set_reg(CX, 0);
        c.set_reg(BP, 0);
        c.set_reg8(r8::AL, 0);
        return c.ret();
    }
    let mut n = 8 + 3;
    let mut kept = 0u16;
    for i in 0..cnt as u16 {
        let p = g::NEXT_EVENT + 4 * i;
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
    c.set_reg(SI, g::PENDING_EVENTS);
    c.set_reg8(r8::DL, dl);
    c.set_reg(BX, bx);
    c.set_reg(BP, 0);
    c.lods(B, DS);
    c.set_reg(CX, cnt as u16);
    let si = c.reg(SI);
    c.set_reg(DI, si);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    // The events kept moved down (`stosw`, `movsw`), the others skipped.
    let mut left = 0u16;
    for _ in 0..cnt {
        let event = c.lods(W, DS);
        let at = c.reg(SI);
        let drop = (event >> 8) as u8 == dl && (dl != 0x0f || c.word(at) == bx);
        if drop {
            c.set_reg(SI, at.wrapping_add(2));
        } else {
            c.stos(W);
            c.movs(W);
            left += 1;
        }
    }
    c.set_reg(BP, left);
    c.set_byte(g::PENDING_EVENTS, left as u8);
    c.set_reg(CX, (left & 0xff) << 8 | cnt as u16);
    if !changed {
        return c.ret();
    }
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
    let t = c.game_time();
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
    c.update_topic_troop(|t| t.age = ax);
    c.set_reg(DX, ax);
    c.set_reg8(r8::CL, 4);
    let q = c.m.cpu.shift(5, W, ax as u32, 4) as u16;
    c.update_topic_troop(|t| t.age_days = q as u8);
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
        text::string_ptr(&mut c, ax, 0x32fe)?;
        c.run(0x32fe, 1)?;
        c.call(entry::FIND_LAST_NUMERIC_DIGIT, 0x3301)?;
        c.run(0x3301, 4)?;
        let dx = c.reg(DX);
        c.set_reg(AX, dx);
        c.set_reg8(r8::CL, 4);
        let q = c.m.cpu.shift(5, W, dx as u32, 4) as u16;
        c.set_reg(AX, q);
        c.call(entry::STRING_REPLACE_NUMBER_ENDING_AT_ES_SI, 0x330a)?;
        c.run(0x330a, 4)?;
        let s = c.pop();
        c.set_reg(SI, s);
        let a = c.pop();
        c.set_reg(AX, a);
        ax = a;
    }
    c.set_word(g::PHRASE_ARG_DETAIL, ax);
    c.ret()
}

/// `loc_01771`: [477Ah] = AX, [4778h] = 0, [4774h] counted, `loc_0b2b9`,
/// `loc_0ad5e`, [4776h] = ([46E0h], [4]); then the frame task 176Bh every
/// 64h ticks.
pub fn start_sequence(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1771)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let ax = c.reg(AX);
    c.update_sequence(|t| t.next = ax);
    c.update_sequence(|t| t.markers = 0);
    let v = c.sequence().playing;
    let v = pure::inc(B, v as u16);
    c.update_sequence(|t| t.playing = v as u8);
    c.call(entry::STOP_CLOCK, 0x1781)?;
    c.run(0x1781, 1)?;
    c.call(entry::PICK_MUSIC, 0x1784)?;
    c.run(0x1784, 6)?;
    let lo = c.player().x as u8;
    let hi = c.byte(g::SKY_PALETTE_SHOWN);
    let ax = (hi as u16) << 8 | lo as u16;
    c.set_reg(AX, ax);
    c.update_sequence(|t| t.return_to = ax);
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
    let ax = pure::sub(W, ax, 2);
    let ax = pure::shl1(W, ax, 1);
    c.set_reg(AX, ax);
    c.set_reg(BP, ax);
    let off = c.code_word(ax.wrapping_add(0x169));
    let si = pure::add(W, 0x169, off);
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

/// `loc_00a3e`: the frame task 0A16h (`keep_framebuffer_0a23`) removed.
pub fn stop_keep_framebuffer_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0a3e)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(SI, 0x0a16);
    super::tasks::remove_frame_task(c.m, 0xda5f)
}

/// `loc_04d00`: the frame task 4BB9h (the troops' alarm, started by
/// `enemy_watch`) removed.
pub fn stop_alarm_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
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
    c.update_conversation(|t| t.line_state = 0);
    c.set_byte(g::SKY_PALETTE_SET, 0);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.update_player(|p| p.x = dx);
    c.update_player(|p| p.y = bx);
    c.update_progress(|t| t.place_kind = (dx >> 8) as u8);
    c.set_reg8(r8::AL, 0x1c);
    c.mul8((bx >> 8) as u8);
    let ax = c.reg(AX);
    let ax = pure::add(W, ax, 0xe4);
    c.set_reg(AX, ax);
    c.set_word(g::PLACE, ax);
    super::ui::open_sal_resource(c.m, 0x2d74)
}

/// `sub_110f`: record FD8h's byte 10h set to 0Ah (word 12h 180h) unless
/// it is 8; then message 280Ah ([`add_message`]).
pub fn on_phase_34(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x110f)?;
    let mut c = Ctx::new(m);
    let k = c.character(1).x as u8;
    c.run(start, if k == 8 { 4 } else { 6 })?;
    if k != 8 {
        c.update_character(1, |t| t.x = t.x & 0xff00 | 0x0a);
        c.update_character(1, |t| t.y = 0x180);
    }
    c.set_reg(AX, 0x280a);
    add_message_in(&mut c)
}

/// `loc_029ee`: [`add_pending_event`] with DI 0.
pub fn note_pair_0(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x29ee)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.set_reg(DI, 0);
    add_pending_event(c.m, 0x29f0)
}

/// [`note_pair_0`] called: the pending event pair AX added, with DI 0.
pub(super) fn add_event(c: &mut Ctx, event: u16, ret: u16) -> Result<(), Bail> {
    c.set_reg(AX, event);
    c.call(entry::NOTE_PAIR_0, ret)
}

/// `sub_1166`: [`g::ITEMS_OFFERED`] counted, record FD8h's byte 10h 2 (word 12h
/// 180h), then pair 105h ([`note_pair_0`]).
pub fn event_105(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1166)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let v = c.byte(g::ITEMS_OFFERED);
    let v = pure::inc(B, v as u16);
    c.set_byte(g::ITEMS_OFFERED, v as u8);
    c.update_character(1, |t| t.x = t.x & 0xff00 | 2);
    c.update_character(1, |t| t.y = 0x180);
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
    let bx = c.economy().demanded;
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
    c.set_reg(BP, g::SHIPMENT_OFFERS);
    let f = c.economy().shipment_flags;
    let f = pure::and(B, f as u16, 0xf9);
    c.update_economy(|t| t.shipment_flags = f as u8);
    c.set_reg(BX, bx);
    let x = pure::shr1(W, bx, 1);
    let x = pure::add(W, x, bx);
    c.set_reg(CX, x);
    let d = pure::add(W, bx, bx);
    c.set_reg(DX, d);
    let s = pure::shr1(W, ax, 1);
    c.set_reg(SI, s);
    let t = pure::shr1(W, ax, 2);
    let t = pure::add(W, t, s);
    c.set_reg(DI, t);
    let put = |c: &mut Ctx, i: u16, v: u16| c.set_word_in(SS, g::SHIPMENT_OFFERS + 2 * i, v);
    if ax < bx {
        put(&mut c, 0, ax);
        put(&mut c, 1, di);
        put(&mut c, 2, si);
        let d2 = pure::sub(W, di, si);
        c.set_reg(DI, d2);
        put(&mut c, 3, d2);
        return c.ret();
    }
    put(&mut c, 0, bx);
    put(&mut c, 1, ax);
    let bits = if ax < cx {
        put(&mut c, 2, di);
        put(&mut c, 3, si);
        2
    } else {
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
    let v = pure::or(B, f, bits);
    c.update_economy(|t| t.shipment_flags = v as u8);
    c.ret()
}

/// `sub_102f`: bit 80h of [124Ah] and [1247h] cleared, [121Dh] = FFFFh,
/// then [`start_sequence`] with AX 1321h.
pub fn on_phase_0c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x102f)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    for p in [0x124au16, 0x1247] {
        let v = c.byte(p);
        let v = pure::and(B, v as u16, 0x7f);
        c.set_byte(p, v as u8);
    }
    c.set_word(g::PALACE_PLAN_SPRITE_4, 0xffff);
    c.set_reg(AX, 0x1321);
    start_sequence(c.m, 0x1771)
}

/// `loc_01071`: the game set to phase 15h with record FD8h's state
/// ([0FFh] 0, [0FDBh] 1, [1018h] 200Bh, [101Ah] 180h, [0FE8h] 0Ah,
/// [0D5h] FFh), `loc_02090`, bit 1 of [0Ah], then pair 1
/// ([`note_pair_0`]).
pub fn start_phase_15(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1071)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    c.update_situation(|t| t.days_in_phase = 0);
    c.update_progress(|t| t.phase = 0x15);
    c.update_character(0, |t| t.y = t.y & 0x00ff | 1 << 8);
    c.update_character(4, |t| t.x = 0x200b);
    c.update_character(4, |t| t.y = 0x180);
    c.update_character(1, |t| t.x = t.x & 0xff00 | 0x0a);
    c.update_situation(|t| t.character_1_days = 0xff);
    c.call(entry::NEXT_DEMAND, 0x1099)?;
    c.run(0x1099, 3)?;
    let v = c.progress().event_flags;
    let v = pure::or(B, v as u16, 1);
    c.update_progress(|t| t.event_flags = v as u8);
    c.set_reg(AX, 1);
    note_pair_0(c.m, 0x29ee)
}

/// `loc_0331e`: the location DI becomes the one talked about ([11CEh]):
/// its fields copied to the condition variables (4Dh..54h, its 7 needs
/// to 55h, the name's region by 1141h), `loc_033be`, `loc_034a5`, the
/// needs with troops' skills (`noted_location_skill_needs`) as a bit set in [53h],
/// `loc_03385`, `loc_05274`. SI, DI kept.
pub fn describe_place(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x331e)?;
    let mut c = Ctx::new(m);
    c.run(start, (24) + 7)?; // the `rep` iterations too
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.set_word(g::TOPIC_PLACE_RECORD, di);
    c.push(si);
    c.push(di);
    let (b0, b1) = (c.byte(di), c.byte(di.wrapping_add(1)));
    let ax = (b0 as u16) << 8 | b1 as u16;
    c.update_topic_place(|t| t.name = ax);
    c.set_reg(BX, g::CHANCES);
    let r = c.byte(g::CHANCES.wrapping_add(b1 as u16));
    c.update_topic_place(|t| t.chance = r);
    let mut al = r;
    for (src, dst) in [
        (0x0au16, g::TOPIC_PLACE_STATUS),
        (0x12, g::TOPIC_PLACE_SPICE),
        (0x1b, g::TOPIC_PLACE_BYTE_1B),
        (8, g::TOPIC_PLACE_KIND),
    ] {
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
    c.set_reg(DI, g::TOPIC_PLACE_NEEDS);
    c.set_reg(CX, 7);
    c.rep_counted(0xa4);
    c.run(0x3359, 2)?;
    let d = c.pop();
    c.set_reg(DI, d);
    c.call(entry::MEASURE_PLACE_FORCES, 0x335d)?;
    c.run(0x335d, 1)?;
    c.call(entry::COUNT_PLACE_TROOPS, 0x3360)?;
    c.run(0x3360, 1)?;
    c.call(entry::NOTED_LOCATION_SKILL_NEEDS, 0x3363)?;
    // The needs the troops' skills meet (`cmp [di], 1; rcl al, 1`: a bit
    // per need of 0, inverted), as bits 1-7 of [53h].
    c.run(0x3363, 3 + 28 + 6)?;
    let mut bits = 0xffu8;
    for i in 0..7u16 {
        bits = bits << 1 | (c.byte(0x46fe + i) == 0) as u8;
    }
    let met = !bits << 1;
    c.set_reg(DI, 0x46fe + 7);
    c.set_reg(CX, 1);
    c.set_reg8(r8::AL, met);
    c.update_topic_place(|t| t.unmet_needs = met);
    let d = c.pop();
    c.set_reg(DI, d);
    c.call(entry::CHARACTERS_AT_PLACE, 0x3380)?;
    c.run(0x3380, 1)?;
    c.call(entry::NEAREST_PLACES, 0x3383)?;
    c.run(0x3383, 2)?;
    let s = c.pop();
    c.set_reg(SI, s);
    c.ret()
}

/// [`describe_place`] called: the location DI becomes the one talked about.
pub(super) fn talk_about_location(c: &mut Ctx, location: u16, ret: u16) -> Result<(), Bail> {
    c.set_reg(DI, location);
    c.call(entry::DESCRIBE_PLACE, ret)
}

/// `ui_comm`: the sietch to call: of the sietches from 100h (below 20h,
/// without bits 82h of byte 0Ah, west of BX, from 64h; `loc_01e24`
/// clear) whose state (`describe_place`) changed ([60h] - [63h]) and
/// whose troop leader BP ([0E4h], or [0DEh] past 1Eh of [0E2h] with [0DCh]
/// below 1Eh; not 11Ch) has troops but none counted by `loc_05098`'s
/// DX, the westmost. DI it ([0D816h], 0 for none, flags), BP [0D818h].
pub fn ui_comm(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2017)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_reg(DI, 0x100);
    c.set_word(g::COMM_STEP, 0);
    c.set_reg(BX, 0x64);
    let mut di = 0x100u16;
    loop {
        let bx = c.reg(BX);
        let b8 = c.byte(di.wrapping_add(8));
        let f = Location(di).flags(&mut c);
        let x = c.word(di.wrapping_add(4));
        'one: {
            // A sietch (below 20h, not 82h) west of BX.
            if b8 >= 0x20 {
                c.run(0x2023, 2)?;
                break 'one;
            }
            if f & (location_flag::UNDISCOVERED | location_flag::HARKONNEN) != 0 {
                c.run(0x2023, 4)?;
                break 'one;
            }
            if (bx as i16) <= x as i16 {
                c.run(0x2023, 6)?;
                break 'one;
            }
            c.run(0x2023, 7)?;
            let location = c.reg(DI);
            if troops::troop_with_400_here(&mut c, location, 0x2037)? {
                c.run(0x2037, 1)?;
                break 'one;
            }
            c.run(0x2037, 3)?;
            c.push(bx);
            c.call(entry::DESCRIBE_PLACE, 0x203d)?;
            let (a, b) = (c.place_forces().counts[0], c.place_forces().counts[3]);
            if a == b {
                c.run(0x203d, 4)?;
                let v = c.pop();
                c.set_reg(BX, v);
                c.set_reg8(r8::AL, 0);
                break 'one;
            }
            let (e4, e2, dc, de) = (c.situation().nearest_high_unmapped.record, c.situation().nearest_high_unmapped.distance, c.situation().nearest_high_mapped.distance, c.situation().nearest_high_mapped.record);
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
            c.set_reg8(r8::AL, a.wrapping_sub(b));
            // Its troop leader.
            c.set_reg(BP, e4);
            if e2 >= 0x1e {
                if skip {
                    break 'one;
                }
                c.set_reg(BP, de);
            }
            if bp == 0x11c {
                break 'one;
            }
            c.push(di);
            c.set_reg(DI, bp);
            let (harkonnen, unstarted) = troops::troop_sides(&mut c, 0x2069)?;
            let (cx, dx, d2) = (harkonnen, unstarted, c.reg(DI));
            let take = cx != 0 && dx == 0;
            c.run(0x2069, 3 + if cx == 0 { 0 } else { 2 } + if take { 3 } else { 0 })?;
            c.set_reg(BP, d2);
            let d = c.pop();
            c.set_reg(DI, d);
            if cx == 0 {
                break 'one;
            }
            if take {
                let x = c.word(d.wrapping_add(4));
                c.set_reg(BX, x);
                c.set_word(g::COMM_STEP, d);
                c.set_word(g::SPAN_ROW, d2);
            }
        }
        di = c.reg(DI);
        let nd = di.wrapping_add(0x1c);
        let nb = c.byte(nd);
        let more = nb < 8;
        c.run(0x207d, 3 + if more { 0 } else { 4 })?;
        c.set_reg(DI, nd);
        di = nd;
        if !more {
            break;
        }
    }
    let bp = c.word(g::SPAN_ROW);
    c.set_reg(BP, bp);
    let di = c.word(g::COMM_STEP);
    c.set_reg(DI, di);
    c.set_flag(ZF, di == 0); // the exit's flags
    c.ret()
}

/// [`ui_comm`] called: whether there is a sietch to call (DI = it).
pub(super) fn sietch_to_call(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    Ok(!c.call_zf(0x2017, ret)?)
}

/// `sal_read_position_markers`: the 17h markers at [47F6h] cleared (FFh),
/// then filled: from the CS list [4778h] while [4774h] runs, else (with
/// a count at SI) `loc_03df4` for each person present ([12h] xor [10h],
/// AX the bit) and the extra ones of [476Ah] (10h on); [47F6h] then
/// moves on by the byte at SI - 1.
pub fn sal_read_position_markers(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3d83)?;
    let mut c = Ctx::new(m);
    c.run(start, 6 + 0x17)?; // the `rep` iterations too
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(pc::cpu::ES, es);
    c.set_reg(AX, 0xffff);
    c.set_reg(CX, 0x17);
    let di = c.word(g::SAL_MARKERS_PTR);
    c.set_reg(DI, di);
    c.rep_counted(0xaa);
    let k = c.sequence().playing;
    let list = c.sequence().markers;
    let di = c.word(g::SAL_MARKERS_PTR);
    if k != 0 && list != 0 {
        c.run(0x3d91, 3 + 3 + 5)?;
        c.set_reg(DI, di);
        c.set_reg(AX, list);
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
        let (present, party) = (c.progress().present, c.progress().party);
        let pre = if k == 0 { 3 } else { 6 };
        c.run(0x3d91, pre + 5 + if cl != 0 { 3 } else { 0 })?;
        c.set_reg(DI, di);
        if k != 0 {
            c.set_reg(AX, list);
        }
        let dx = pure::xor(W, present, party);
        c.set_reg(DX, dx);
        c.set_reg8(r8::CL, cl);
        if cl != 0 {
            let ch = c.economy().room_seat_shift;
            let ch = pure::and(B, ch as u16, 0x0f);
            c.set_reg8(r8::CH, ch as u8);
            c.set_reg(AX, 0xffff);
            loop {
                let dx = c.reg(DX);
                let cf = dx & 1 != 0;
                c.run(0x3dc8, 3 + cf as u32)?;
                let ax = c.reg(AX);
                let ax = pure::inc(W, ax);
                c.set_reg(AX, ax);
                let d = pure::shr1(W, dx, 1);
                c.set_reg(DX, d);
                if cf {
                    c.call(entry::PUT_IN_RING, 0x3dd0)?;
                }
                let dx = c.reg(DX);
                c.run(0x3dd0, 2)?;
                if dx == 0 {
                    break;
                }
            }
            let dl = c.byte(g::CHARACTER_15_TROOP_COUNT);
            let dx = c.reg(DX) & 0xff00 | dl as u16;
            let d = dx.wrapping_sub(1);
            let more = (d as i16) > 0;
            c.run(0x3dd4, 3 + more as u32)?;
            c.set_reg(DX, dx);
            let d = pure::dec(W, dx);
            c.set_reg(DX, d);
            if more {
                c.set_reg(AX, 0x0f);
                loop {
                    c.run(0x3dde, 2)?;
                    let ax = c.reg(AX);
                    let ax = pure::inc(W, ax);
                    c.set_reg(AX, ax);
                    c.call(entry::PUT_IN_RING, 0x3de2)?;
                    let dx = c.reg(DX);
                    let d = dx.wrapping_sub(1);
                    c.run(0x3de2, 2)?;
                    let d2 = pure::dec(W, dx);
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
    let di = c.word(g::SAL_MARKERS_PTR);
    let ax = pure::dec(W, b);
    c.set_reg(AX, ax);
    let di = pure::add(W, di, ax);
    c.set_reg(DI, di);
    c.set_word(g::SAL_MARKERS_PTR, di);
    c.ret()
}

/// `sub_11e6`: of the locations from 138h with a kind past
/// `location_kind_check` and no troops counted by `loc_05098` (DX), the
/// one with the largest y (word 4, above -64h; 100h for none) becomes
/// record 1048h's place (its coordinates, `location_coords`, DL 3) and
/// [0F2h] its name; then message 2B0Ah ([`add_message`]).
pub fn event_place_message(m: &mut Machine, start: u16) -> Result<(), Bail> {
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
                break 'one;
            }
            c.run(0x1f19, 4)?;
            c.set_reg(AX, ax);
            if map::location_skipped(&mut c, 0x1f23)? {
                c.run(0x1f23, 1)?;
                break 'one;
            }
            c.run(0x1f23, 2)?;
            let (_, unstarted) = troops::troop_sides(&mut c, 0x1f28)?;
            if unstarted != 0 {
                c.run(0x1f28, 2)?;
                break 'one;
            }
            let (di, bx) = (c.reg(DI), c.reg(BX));
            let ax = c.word(di.wrapping_add(4));
            let more = (ax as i16) > bx as i16;
            c.run(0x1f28, 5 + if more { 2 } else { 0 })?;
            c.set_reg(AX, ax);
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
        let d = pure::add(W, di, 0x1c);
        c.set_reg(DI, d);
        if more {
            continue;
        }
        if bx == 0xff9c {
            c.set_reg(SI, 0x100);
        }
        let si = c.reg(SI);
        c.set_reg(DI, si);
        break;
    }
    c.call(entry::LOCATION_COORDS, 0x1f4c)?;
    c.run(0x1f4c, 8)?;
    let (dx, bx, di) = (c.reg(DX), c.reg(BX), c.reg(DI));
    let dx = dx & 0xff00 | 3;
    c.set_reg(DX, dx);
    c.update_character(7, |t| t.x = dx);
    c.update_character(7, |t| t.y = bx);
    let ax = (c.byte(di) as u16) << 8 | c.byte(di.wrapping_add(1)) as u16;
    c.update_situation(|t| t.event_place_name = ax);
    c.set_reg(AX, 0x2b0a);
    add_message_in(&mut c)
}

/// `loc_00a23`: one step of the scene's video: in a cinematic ([227Dh])
/// straight to the screen and [`hnm_do_frame`]; else on the front buffer
/// `hnm_do_frame_and_check_if_frame_advanced`, and on a new frame
/// `update_game_area_on_screen` and the mouse ([`draw_mouse`]); CF clear when none.
pub fn video_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0a23)?;
    let mut c = Ctx::new(m);
    let k = c.byte(g::CINEMATIC);
    if k != 0 {
        c.run(start, 3)?;
        c.call(entry::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0x0a2d)?;
        c.run(0x0a2d, 1)?;
        return super::frame::hnm_do_frame(c.m, 0xca60);
    }
    c.run(start, 3)?;
    c.call(entry::SET_FB1_AS_ACTIVE_FRAMEBUFFER, 0x0a33)?;
    c.run(0x0a33, 1)?;
    if !gfx::frame_advanced(&mut c, 0x0a36)? {
        c.run(0x0a36, 3)?;
        c.set_flag(CF, false);
        return c.ret();
    }
    c.run(0x0a36, 2)?;
    c.call(entry::UPDATE_GAME_AREA_ON_SCREEN, 0x0a3b)?;
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
        c.call(entry::RAND, 0x0c5b)?;
        let k = c.byte(g::CREATURES_STILL);
        c.run(0x0c5b, 2 + (k != 0) as u32 + 3)?;
        let mut ax = c.reg(AX);
        if k != 0 {
            let al = pure::and(B, ax & 0xff, 0xef);
            ax = ax & 0xff00 | al;
            c.set_reg(AX, ax);
        }
        c.set_reg(CX, ax);
        let random = math::random_masked(&mut c, 7, 0x0c6c)?;
        let hi = (random as u8) >= 4;
        c.run(0x0c6c, 3 + hi as u32 + 1)?;
        let si = c.reg(SI);
        c.set_byte(si.wrapping_add(1), random as u8);
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
        let (_, step_y) = map::step_for_direction(&mut c, 0x0ccf)?;
        c.run(0x0ccf, 3)?;
        let (bx, dx) = (step_y, c.reg(DX));
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
    c.call(entry::PARTICLES_SPAWN_PARTICLE, 0x0ce4)?;
    c.run(0x0ce4, 2)?;
    let di = c.reg(DI);
    c.set_word(di.wrapping_add(0x0d), 0);
    c.ret()
}

/// `loc_031f6`: the troop SI described in the condition variables
/// (registers kept): its location (2Ch), id, activity, the age phrase
/// (`record_age_phrase`) and rate (`troop_rate_by_age`, [48h]), words
/// 10h, 12h, motivation (`troop_motivation`), its three skills with
/// their phrases (`skill_level_phrase`), words 0Ch, 0Eh, the skill of
/// its activity (`loc_0693b`), byte 19h, size, the days since byte 14h,
/// and its location (`describe_place`).
pub fn describe_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x31f6)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(entry::SAVE_REGS, 0x31f9)?;
    c.run(0x31f9, 11)?;
    let si = c.reg(SI);
    let di = c.word(si.wrapping_add(4));
    c.set_reg(DI, di);
    c.update_topic_troop(|t| t.place = di);
    let id = c.byte(si);
    c.update_topic_troop(|t| t.id = id);
    let b3 = c.byte(si.wrapping_add(3));
    c.update_topic_troop(|t| t.state = b3);
    let ax = c.reg(AX) & 0xff00 | b3 as u16;
    let a = pure::and(W, ax, 0x0f);
    c.update_topic_troop(|t| t.job = a as u8);
    let a = pure::add(W, a, 0x18);
    c.set_reg(AX, a);
    c.set_word(g::PHRASE_ARG_SUBJECT, a);
    c.call(entry::RECORD_AGE_PHRASE, 0x321a)?;
    c.run(0x321a, 1)?;
    c.call(entry::TROOP_RATE_BY_AGE, 0x321d)?;
    c.run(0x321d, 10)?;
    let si = c.reg(SI);
    let ax = c.reg(AX);
    c.update_topic_troop(|t| t.rate = ax);
    let w = c.word(si.wrapping_add(0x10));
    c.update_topic_troop(|t| t.word_10 = w);
    let w = c.word(si.wrapping_add(0x12));
    c.update_topic_troop(|t| t.word_12 = w);
    let a = pure::and(W, w, 0x0f);
    c.update_topic_troop(|t| t.word_12_low = a as u8);
    let a = pure::add(W, a, 0);
    c.set_reg(AX, a);
    c.set_word(g::PHRASE_ARG_8A, a);
    c.call(entry::TROOP_MOTIVATION, 0x323b)?;
    for (at, src, dst, word, ret) in [
        (0x323bu16, 0x16u16, g::TOPIC_TROOP_SKILLS, g::PHRASE_ARG_SKILLS, 0x3247u16),
        (0x3247, 0x17, g::TOPIC_TROOP_SKILLS + 1, g::PHRASE_ARG_SKILLS + 2, 0x3253),
        (0x3253, 0x18, g::TOPIC_TROOP_SKILLS + 2, g::PHRASE_ARG_SKILLS + 4, 0x325f),
    ] {
        c.run(at, 4)?;
        let ax = c.reg(AX);
        if at == 0x323b {
            c.update_topic_troop(|t| t.motivation = ax as u8);
        } else {
            let prev = if at == 0x3247 { g::PHRASE_ARG_SKILLS } else { g::PHRASE_ARG_SKILLS + 2 };
            c.set_word(prev, ax);
        }
        let si = c.reg(SI);
        let b = c.byte(si.wrapping_add(src));
        c.set_reg8(r8::AL, b);
        c.set_byte(dst, b);
        let _ = word;
        c.call(entry::SKILL_LEVEL_PHRASE, ret)?;
    }
    c.run(0x325f, 9)?;
    let ax = c.reg(AX);
    c.set_word(g::PHRASE_ARG_SKILLS + 4, ax);
    let si = c.reg(SI);
    let w = c.word(si.wrapping_add(0x0c));
    c.update_topic_troop(|t| t.word_0c = w);
    let w = c.word(si.wrapping_add(0x0e));
    c.update_topic_troop(|t| t.word_0e = w);
    let a = pure::add(W, w & 0xff, 0xe8);
    c.set_reg(AX, a);
    c.set_word(g::SMUGGLER_ITEM, a);
    c.call(entry::TROOP_ACTIVITY, 0x3279)?;
    c.run(0x3279, 8)?;
    let ax = c.reg(AX);
    c.set_reg(BP, ax);
    let si = c.reg(SI);
    let s = c.byte_in(SS, ax.wrapping_add(si).wrapping_add(0x16));
    c.update_topic_troop(|t| t.job_skill = s);
    let b = c.byte(si.wrapping_add(0x19));
    c.update_topic_troop(|t| t.needs = b);
    let z = c.byte(si.wrapping_add(0x1a));
    c.update_topic_troop(|t| t.size = z);
    c.set_reg(AX, ax & 0xff00 | z as u16);
    c.call(entry::GET_INGAME_DAY_Q, 0x3290)?;
    c.run(0x3290, 3)?;
    let al = c.reg8(r8::AL);
    let si = c.reg(SI);
    let d = c.byte(si.wrapping_add(0x14));
    let v = pure::sub(B, al as u16, d as u16);
    c.set_reg8(r8::AL, v as u8);
    c.update_topic_troop(|t| t.days = v as u8);
    c.call(entry::DESCRIBE_PLACE, 0x3299)?;
    c.run(0x3299, 1)?;
    c.call(entry::RESTORE_REGS, 0x329c)?;
    c.run(0x329c, 1)?;
    c.ret()
}

/// [`describe_troop`] called: the troop SI described in the condition variables.
pub(super) fn tell_about_troop(c: &mut Ctx, troop: u16, ret: u16) -> Result<(), Bail> {
    c.set_reg(SI, troop);
    c.call(entry::DESCRIBE_TROOP, ret)
}

/// `sub_1027`: bit 80h of [122Eh] cleared, then the panel's buttons
/// (`panel_buttons`).
pub fn on_phase_08(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1027)?;
    let mut c = Ctx::new(m);
    super::screens::panel_buttons_at(&mut c, start, 2, |c| {
        let v = c.byte(g::PALACE_ROOM_2_BUTTON_4);
        let v = pure::and(B, v as u16, 0x7f);
        c.set_byte(g::PALACE_ROOM_2_BUTTON_4, v as u8);
    })
}

/// `sub_10a4`: bit 80h of [1245h] cleared, [1217h] FFFFh, then the panel's
/// buttons.
pub fn on_phase_1c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x10a4)?;
    let mut c = Ctx::new(m);
    super::screens::panel_buttons_at(&mut c, start, 3, |c| {
        let v = c.byte(g::PALACE_ROOM_7_BUTTON_2);
        let v = pure::and(B, v as u16, 0x7f);
        c.set_byte(g::PALACE_ROOM_7_BUTTON_2, v as u8);
        c.set_word(g::PALACE_PLAN_SPRITE_3, 0xffff);
    })
}

/// `loc_01188`: bit 80h of [1259h] cleared, [1211h] FFFFh, then the panel's
/// buttons.
pub fn on_phase_54(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1188)?;
    let mut c = Ctx::new(m);
    super::screens::panel_buttons_at(&mut c, start, 3, |c| {
        let v = c.byte(g::PALACE_ROOM_11_BUTTON_2);
        let v = pure::and(B, v as u16, 0x7f);
        c.set_byte(g::PALACE_ROOM_11_BUTTON_2, v as u8);
        c.set_word(g::PALACE_PLAN_SPRITE_2, 0xffff);
    })
}

/// `loc_02090`: the next spice demand: [118Dh] = the day; [0BCh] =
/// ([0C3h], counted, ·96h + 64h) · (E0h + a random 0..3Fh) / 256, for
/// [0BEh] below 80h scaled by (100h + ~(2·[0BEh]))/256; FFFFh on any
/// overflow. [0CFh] 0, [0BFh] |= 90h, then message 20Bh (30Bh for [`g::SHIPMENT_RATIO`]
/// below 80h) ([`add_message`]).
pub fn next_demand(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2090)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let ax0 = time::game_day(&mut c, 0x2093)?;
    let k = c.economy().demands;
    let be = c.economy().shipment_ratio;
    let pos = be & 0x80 == 0;
    let tail_n = |out: bool| out as u32 + 7 + pos as u32;
    let p = ((ax0 & 0xff00) | k as u16) as u32 * 0x96;
    let over1 = p >> 16 != 0;
    let (s, carry) = (p as u16).overflowing_add(0x64);
    let head = |c: &mut Ctx| {
        c.set_word(g::SHIPMENT_DUE, ax0);
        c.set_reg8(r8::AL, k);
        let v = pure::inc(B, k as u16);
        c.update_economy(|t| t.demands = v as u8);
        c.set_reg(BX, 0x96);
        c.mul16(0x96);
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
        let a = pure::add(W, ax, 0x64);
        c.set_reg(AX, a);
        return demand_tail(&mut c, true);
    }
    c.run(0x2093, 13)?;
    head(&mut c);
    let ax = c.reg(AX);
    let a = pure::add(W, ax, 0x64);
    c.set_reg(AX, a);
    c.set_reg(CX, s);
    let random = math::random_masked(&mut c, 0x3f, 0x20ef)?;
    let r = random.wrapping_add(0xe0);
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
    let a = pure::add(W, ax, 0xe0);
    c.set_reg(AX, a);
    c.mul16(s);
    let dx = c.reg(DX);
    if dx >> 8 == 0 {
        let ax = c.reg(AX);
        let d = (dx & 0xff) << 8 | ax >> 8;
        c.set_reg(DX, d);
        let sh = pure::shl1(B, be as u16, 1);
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
    c.update_economy(|t| t.demanded = dx);
    c.update_situation(|t| t.shipment_days_left = 0);
    let v = c.economy().shipment_flags;
    let v = c.or(B, v as u16, 0x90);
    c.update_economy(|t| t.shipment_flags = v as u8);
    c.set_reg(AX, 0x20b);
    let be = c.economy().shipment_ratio;
    c.cmp(B, be as u16, 0);
    if be & 0x80 == 0 {
        let ah = c.inc(B, 2);
        c.set_reg(AX, ah << 8 | 0x0b);
    }
    add_message_in(c)
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
    c.call(entry::SAVE_REGS, 0x2173)?;
    c.run(0x2173, 2)?;
    c.set_reg(SI, 0xfd8);
    c.set_reg(CX, 9);
    for i in 0..9u16 {
        let si = 0xfd8 + 0x10 * i;
        let f = c.byte(si.wrapping_add(0x0f));
        'one: {
            if f & 0x40 != 0 {
                c.run(0x2179, 2)?;
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
                c.set_reg(DX, dx);
                c.set_reg(BX, bx);
                if si == 0x1008 {
                    if home {
                        c.set_word(si, dx);
                    }
                }
            };
            if bx & 0xff != 0x80 {
                c.run(0x2179, n0 + 1)?;
                ops0(&mut c);
                c.call(entry::TROOP_TO_PLACE, 0x21f1)?;
                break 'one;
            }
            let dl = dx as u8;
            let (dh, bh) = ((dx >> 8) as u8, (bx >> 8) as u8);
            if dl != 1 {
                let fewer = si < 0x1028;
                if fewer || bh != 1 {
                    c.run(0x2179, n0 + 2 + 2 + if fewer { 0 } else { 2 })?;
                    ops0(&mut c);
                    break 'one;
                }
                c.run(0x2179, n0 + 2 + 5)?;
                ops0(&mut c);
                let roll = troops::roll_2_to_12(&mut c, 0x21ea)?;
                c.run(0x21ea, 2)?;
                c.set_byte(si, roll);
                break 'one;
            }
            let cur = c.progress().place_index;
            let lf = Location::number(bh).flags(&mut c);
            let mut n = n0 + 2 + 2;
            let stage = if dh >= 0x21 {
                0
            } else if bh == cur {
                n += 2;
                1
            } else if lf & location_flag::HARKONNEN != 0 {
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
            let nx = c.byte(g::PEOPLE_NEXT_STATE.wrapping_add(e as u16));
            let ph = c.progress().phase;
            if stage == 4 && nx == 6 {
                n += 2 + if ph < 0x24 { 2 } else { 0 };
            }
            c.run(0x2179, n)?;
            ops0(&mut c);
            if stage == 0 {
                break 'one;
            }
            if stage == 1 {
                break 'one;
            }
            c.set_reg8(r8::AL, 0x1c);
            c.mul8(bh);
            let a = c.reg(AX);
            let a = pure::add(W, a, 0xe4);
            c.set_reg(AX, a);
            c.set_reg(DI, a);
            if stage == 2 {
                break 'one;
            }
            let v = c.byte(si);
            let v = pure::inc(B, v as u16);
            c.set_byte(si, v as u8);
            if stage == 3 {
                break 'one;
            }
            c.set_reg(BX, 0x144d);
            c.set_reg8(r8::AL, nx);
            c.set_byte(si, nx);
            let g = c.byte(si.wrapping_add(0x0f));
            let g = pure::and(B, g as u16, 0xfb);
            c.set_byte(si.wrapping_add(0x0f), g as u8);
            if nx == 6 {
                if ph < 0x24 {
                    c.set_byte(si, 0x0a);
                }
            }
        }
        let last = i == 8;
        c.run(0x21f1, 2 + if last { 1 } else { 0 })?;
        let s = pure::add(W, si, 0x10);
        c.set_reg(SI, s);
        c.set_reg(CX, 8 - i);
    }
    c.call(entry::RESTORE_REGS, 0x21f9)?;
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
    let fb = c.screen_buffers().active;
    c.push(fb);
    c.call(entry::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0x45e5)?;
    c.run(0x45e5, 1)?;
    c.call(entry::CALL_RESTORE_CURSOR, 0x45e8)?;
    c.run(0x45e8, 1)?;
    c.call(entry::FONT_SELECT_SMALL_FONT, 0x45eb)?;
    let f = c.byte(g::GAME_FRAME);
    let di = c.reg(DI);
    c.run(0x45eb, 5 + if f != 0 { 2 } else { 0 } + 3)?;
    c.set_reg(DX, 0x55);
    c.set_reg(BX, 0x22);
    c.set_reg(CX, 0xf5fe);
    if f != 0 {
        c.set_reg(BX, 0x26);
        c.set_reg(CX, 0x20fe);
    }
    if di == 0 {
        c.call(entry::START_MESSAGE_57, 0x462d)?;
        c.run(0x462d, 2)?;
        let cx = c.reg(CX);
        c.set_word(g::TEXT_COLOUR, cx);
        c.call(entry::FONT_SET_DRAW_POSITION, 0x4634)?;
        c.run(0x4634, 1)?;
    } else {
        c.call(entry::STOP_TYPING_MESSAGE, 0x4607)?;
        let di = c.reg(DI);
        c.run(0x4607, 4)?;
        if di < 0xfff0 {
            let cx = c.reg(CX);
            c.push(cx);
            c.call(entry::DRAW_STRING_LOCATION_TYPE, 0x463a)?;
            c.run(0x463a, 1)?;
            c.call(entry::FONT_GET_DRAW_POSITION, 0x463d)?;
            c.run(0x463d, 2)?;
            let x = c.pop();
            c.set_reg(CX, x);
            c.call(entry::DRAW_LOCATION_NAME, 0x4641)?;
        } else {
            c.set_reg(AX, 0xa4);
            c.call(entry::FONT_DRAW_STRING_WITH_COLOR_AT_POS, 0x4612)?;
            let di = c.reg(DI);
            let d = di.wrapping_sub(0xfff0);
            c.run(0x4612, if d >= 8 { 3 } else { 5 })?;
            c.set_reg(DI, d);
            c.cmp(W, d, 8); // the character writer sees these flags
            if d < 8 {
                c.set_reg8(r8::AL, 0x20);
                let to = c.word(g::FONT_DRAW);
                c.call(to, 0x4620)?;
                c.run(0x4620, 3)?;
                let phrase = c.reg(DI).wrapping_add(0xda);
                c.set_reg(AX, phrase);
                c.call(entry::FONT_DRAW_PHRASE_OR_COMMAND_STRING, 0x4628)?;
                c.run(0x4628, 1)?;
            }
        }
    }
    let mut i = 0;
    loop {
        let v = c.font_pen().at[0];
        i += 1;
        if i > 1000 {
            c.m.cpu.ip = 0x4641;
            return Err(Bail);
        }
        c.run(0x4641, 4)?;
        // The character writer and `loc_0dbec` after the loop see these.
        c.cmp(W, v, 0xed);
        if v > 0xed {
            break;
        }
        c.set_reg8(r8::AL, 0x20);
        let to = c.word(g::FONT_DRAW);
        c.call(to, 0x464f)?;
        c.run(0x464f, 1)?;
    }
    let fb = c.pop();
    c.update_screen_buffers(|t| t.active = fb);
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
    let k = c.conversation().scene_count;
    if k != 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    c.run(start, 6)?;
    let si = c.reg(SI);
    let al = c.byte(si.wrapping_add(0x0e));
    c.set_reg(AX, al as u16);
    c.push(si);
    c.call(entry::SAY_NUMBERED, 0x3530)?;
    let cf = c.flag(CF);
    c.run(0x3530, if cf { 5 } else { 2 })?;
    if cf {
        let s = c.pop();
        c.set_reg(SI, s);
        let (ax, di) = (c.conversation().talker, c.word(g::PLACE));
        c.set_reg(AX, ax);
        c.set_reg(DI, di);
        if !pending_event_found(&mut c, 0x353d)? {
            c.run(0x353d, 2)?;
            return c.ret();
        }
        c.run(0x353d, 2)?;
        c.call(entry::SPEAK_EVENT_LINE, 0x3542)?;
    } else {
        let s = c.pop();
        c.set_reg(SI, s);
    }
    c.run(0x3542, 3)?;
    let (ax, di) = (c.conversation().talker, c.word(g::PLACE));
    c.set_reg(AX, ax);
    c.set_reg(DI, di);
    c.call(entry::DROP_EVENTS, 0x354c)?;
    let st = c.progress().talk_reason;
    let ops = |c: &mut Ctx| {
        c.update_situation(|t| t.desert_rescues = 0);
        let v = c.conversation().scene_count;
        let v = pure::inc(B, v as u16);
        c.update_conversation(|t| t.scene_count = v as u8);
    };
    if st == 3 {
        c.run(0x354c, 7)?;
        ops(&mut c);
        c.set_reg(BP, 0x1f92);
        c.set_reg(BX, 0x97cf);
        c.call(entry::PUSH_SCREEN, 0x3565)?;
        c.run(0x3565, 3)?;
        c.set_word(g::TEXT_BOX_FLAGS, 0);
        c.set_word(g::HEAD_BOX_FLAGS, 0);
        return c.ret();
    }
    if st == 4 {
        c.run(0x354c, 10)?;
        ops(&mut c);
        c.set_reg(BP, 0x1f9e);
        let v = c.byte_in(SS, 0x1fa9);
        let v = pure::and(B, v as u16, 0xbf);
        c.m.cpu.write8(&mut c.m.hw, SS, 0x1fa9, v as u8);
        c.set_reg(BX, 0x97cf);
        c.call(entry::PUSH_SCREEN, 0x3586)?;
        c.run(0x3586, 3)?;
        c.set_word(g::TEXT_BOX_FLAGS, 0);
        c.set_word(g::HEAD_BOX_FLAGS, 0);
        return super::screens::panel_buttons(c.m, 0x2ffb);
    }
    let w = c.sequence().playing;
    if w != 0 {
        c.run(0x354c, 9)?;
        ops(&mut c);
        return c.ret();
    }
    if st >= 0x64 {
        c.run(0x354c, 11)?;
        ops(&mut c);
        return c.ret();
    }
    c.run(0x354c, 12)?;
    ops(&mut c);
    let ax = c.conversation().talker;
    dialogue::start_speaking(&mut c, ax, 0x35a9)?;
    c.run(0x35a9, 1)?;
    c.call(entry::FADE_PANEL, 0x35ac)?;
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
    let mode = c.travel().screen_kind;
    if mode & 0x0f == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    c.run(start, 5)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.call(entry::MESSAGE_SUBJECT, 0x451a)?;
    let di = c.map_selection().subject;
    let mode = c.travel().screen_kind;
    let cur = c.word(g::PLACE);
    let ops = |c: &mut Ctx| {
        let d = c.pop();
        c.set_reg(DX, d);
        let b = c.pop();
        c.set_reg(BX, b);
        c.set_reg(DI, di);
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
            return c.ret();
        }
        c.run(0x451a, if mode & 3 != 0 { 8 } else { 10 } + 4)?;
        ops(&mut c);
    } else {
        c.run(0x451a, 6 + 4)?;
        ops(&mut c);
    }
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.push(di);
    c.call(entry::PLACE_PHRASE, 0x453a)?;
    c.run(0x453a, 1)?;
    c.call(entry::PLAY_VOICE_LOWERING_MUSIC, 0x453d)?;
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
        files::wait_ticks_with_tasks(&mut c, 0x14, 0x4549)?;
        c.run(0x4549, 3)?;
        let di = c.reg(DI);
        c.push(di);
        c.set_reg(DI, 0);
        c.call(entry::REDRAW_MESSAGE_LINE, 0x454f)?;
        c.run(0x454f, 3)?;
        let d = c.pop();
        c.set_reg(DI, d);
        files::wait_ticks_with_tasks(&mut c, 0x0a, 0x4556)?;
        c.run(0x4556, 1)?;
        c.call(entry::REDRAW_MESSAGE_LINE, 0x4559)?;
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
    c.call(entry::WAIT_VOICE_DONE, 0x4561)?;
    c.run(0x4561, 5)?;
    let d = c.pop();
    c.set_reg(DI, d);
    let d = c.pop();
    c.set_reg(DX, d);
    let b = c.pop();
    c.set_reg(BX, b);
    c.set_byte(g::SAL_FLAGS, 0x80);
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
    c.call(entry::SET_UP_FLIGHT, 0x4706)?;
    c.run(0x4706, 1)?;
    c.call(entry::SKY_IF_CHANGED, 0x4709)?;
    c.run(0x4709, 6)?;
    let m0 = c.travel().screen_kind;
    c.set_reg8(r8::AL, m0);
    let ax = c.reg(AX);
    c.push(ax);
    let a = c.shr1(B, m0 as u16, 2);
    c.set_reg8(r8::AL, a as u8);
    let v = c.or(B, m0 as u16, a);
    c.update_travel(|t| t.screen_kind = v as u8);
    c.call(entry::PICK_MUSIC, 0x4718)?;
    let t = c.progress().harkonnen_place;
    c.run(0x4718, if t != 0 { 4 } else { 3 })?;
    c.cmp(B, t as u16, 0);
    if t != 0 {
        c.update_progress(|t| t.harkonnen_place = 0);
        c.call(entry::STOP_SCENE_VOICE_AND_PARTICLES, 0x4727)?;
        c.run(0x4727, 1)?;
    }
    c.call(entry::LEAVE_SCREEN, 0x472a)?;
    c.run(0x472a, 1)?;
    c.call(entry::STOP_ALARM_TASK, 0x472d)?;
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
    c.update_flight(|t| t.steps = 0);
    c.push(a);
    c.call(entry::RESET_DESERT_COUNTDOWN, 0x473c)?;
    let m1 = c.travel().screen_kind;
    let d = (m1 & 3).wrapping_sub(1);
    c.run(0x473c, 5)?;
    let a = c.and(B, m1 as u16, 3);
    let a = c.dec(B, a);
    c.set_reg8(r8::AL, a as u8);
    if d == 0 {
        c.call(entry::UI_HEAD_ANIMATE_DOWN, 0x4748)?;
        c.run(0x4748, 1)?;
    }
    c.call(entry::COPY_GAME_RECT_FROM_FB2_TO_FB1, 0x474b)?;
    c.run(0x474b, 1)?;
    c.call(entry::ARRIVE_AND_UPDATE_PARTY, 0x474e)?;
    c.run(0x474e, 4)?;
    let a = c.pop();
    c.set_reg(AX, a);
    let h = c.travel().heading;
    c.set_reg8(r8::BL, h);
    let bx = c.reg(BX);
    c.push(bx);
    c.call(entry::RUN_NIGHT_SEQUENCE, 0x4757)?;
    c.run(0x4757, 4)?;
    let a = c.pop();
    c.set_reg(AX, a);
    c.update_travel(|t| t.heading = a as u8);
    c.update_progress(|t| t.place_kind = 0xff);
    c.call(entry::ADVANCE_FLIGHT_STEP, 0x4763)?;
    let v = c.map_view().mode;
    let neg = v & 0x80 != 0;
    c.run(0x4763, if neg { 5 } else { 5 })?;
    c.set_word(g::PLACE, 0);
    c.update_flight(|t| t.last_move = 0);
    c.cmp(B, v as u16, 0);
    if !neg {
        c.call(0x2dbf, 0x4779)?;
        c.run(0x4779, 1)?;
    }
    c.call(entry::SET_FLYING, 0x477c)?;
    let m2 = c.travel().screen_kind;
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
    let di = c.word(g::LAST_PLACE);
    c.set_reg(DI, di);
    let v = c.byte(di.wrapping_add(0x15));
    let v = c.dec(B, v as u16);
    c.set_byte(di.wrapping_add(0x15), v as u8);
    super::audio::pcm_stop_voc(c.m, 0xac14)
}

/// From 478Fh: `loc_02eb2`, then the palette.
fn palette_after_2eb2(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x478f, 1)?;
    c.call(entry::PLACE_PANEL_BACK, 0x4792)?;
    c.run(0x4792, 1)?;
    super::gfx::update_screen_palette(c.m, 0xc0f4)
}

/// `sub_1dd7` (`loc_01f64`): the Harkonnen take a location: from phase
/// 3Ch, or 70h days after [1154h], unless [2] bit 10h or [11BCh] was set
/// (cleared), on the carry out of rotating [0] left: the target
/// (`ui_comm`, ZF: none); [0C4h] up; twice the troops at the location BP
/// (+9, chained by +1) with bit 80h of +10h are taken (+3 8Dh, `loc_084a6`,
/// bit 10h of +10h cleared, `loc_08357`); the location DI flagged 2, its
/// troops (`alert_location_troops`), the place records at 0FD8h (9 of
/// 10h) of the place BX moved to DX, the event 0Ch (0Dh for the troop
/// 8E0h's location) noted (`call_29f0_f`); for the current place [6]
/// its new id DX in [4] and [0Bh], [473Bh], [2Bh] and `note_battle_balance`, else
/// the shown locations counted.
pub fn harkonnen_take(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1dd7)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let ph = c.progress().phase;
    if ph < 0x3c {
        let d = c.game_time();
        let t = c.word(g::WORD_1154);
        let a = d.wrapping_sub(t);
        if d < t || a < 0x70 {
            c.run(0x1f64, if d < t { 6 } else { 8 })?;
            let a = pure::sub(W, d, t);
            c.set_reg(AX, a);
            return c.ret();
        }
        c.run(0x1f64, 7)?;
        let a = pure::sub(W, d, t);
        c.set_reg(AX, a);
    } else {
        c.run(0x1f64, 2)?;
    }
    let d = c.game_time();
    if d & 0x10 != 0 {
        c.run(0x1f79, 3)?;
        return c.ret();
    }
    let k = c.byte(g::SKIP_HARKONNEN_TAKE);
    if k != 0 {
        c.run(0x1f79, 7)?;
        c.set_byte(g::SKIP_HARKONNEN_TAKE, 0);
        c.set_reg8(r8::AL, k);
        return c.ret();
    }
    let w = c.word(0);
    let carry = w & 0x8000 != 0;
    c.run(0x1f79, if carry { 9 } else { 9 })?;
    c.set_byte(g::SKIP_HARKONNEN_TAKE, 0);
    c.set_reg8(r8::AL, 0);
    let r = pure::rol1(W, w);
    c.set_word(0, r);
    if !carry {
        return c.ret();
    }
    if !sietch_to_call(&mut c, 0x1f95)? {
        c.run(0x1f95, 2)?;
        return c.ret();
    }
    c.run(0x1f95, 3)?;
    let v = c.economy().harkonnen_takeovers;
    let v = pure::inc(B, v as u16);
    c.update_economy(|t| t.harkonnen_takeovers = v as u8);
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
                break;
            }
            c.run(0x1fa1, 3)?;
            let troop_ = troops::troop_record(&mut c, 0x1fa8)?;
            let si = troop_.0;
            let nx = c.byte(si.wrapping_add(1));
            let f = c.byte(si.wrapping_add(0x10));
            c.run(0x1fa8, if f & 0x80 != 0 { 9 } else { 3 })?;
            c.set_reg8(r8::AL, nx);
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
        c.call(entry::TROOP_SET_OUT, 0x1fbc)?;
        c.run(0x1fbc, 5)?;
        let d = c.pop();
        c.set_reg(DI, d);
        let s = c.pop();
        c.set_reg(SI, s);
        let f = c.byte(s.wrapping_add(0x10));
        let f = pure::and(B, f as u16, 0xef);
        c.set_byte(s.wrapping_add(0x10), f as u8);
        c.push(d);
        c.call(entry::TROOP_ARRIVED, 0x1fc6)?;
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
    let f = Location(di).flags(&mut c);
    let f = pure::or(B, f as u16, 2);
    Location(di).set_flags(&mut c, f as u8);
    c.call(entry::ALERT_LOCATION_TROOPS, 0x1fd2)?;
    c.run(0x1fd2, 1)?;
    c.call(entry::LOCATION_COORDS, 0x1fd5)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    let mut n = 2;
    for i in 0..9u16 {
        let p = 0xfd8 + 0x10 * i;
        n += 4 + (c.word(p + 2) == bx) as u32;
    }
    let di = c.reg(DI);
    let own = c.word(g::FOLLOWED_TROOP_LOCATION) == di;
    n += 4 + own as u32 + 3;
    c.run(0x1fd5, n)?;
    c.set_reg(SI, 0xfd8);
    c.set_reg(CX, 9);
    for i in 0..9u16 {
        let p = 0xfd8 + 0x10 * i;
        let v = c.word(p + 2);
        if v == bx {
            c.set_word(p, dx);
        }
        let s = pure::add(W, p, 0x10);
        c.set_reg(SI, s);
        c.set_reg(CX, 8 - i);
    }
    c.set_reg8(r8::AL, 0x0c);
    c.set_reg(SI, 0x8e0);
    if own {
        let a = pure::inc(B, 0x0c);
        c.set_reg8(r8::AL, a as u8);
    }
    c.push(bx);
    c.push(dx);
    c.call(entry::CALL_29F0_F, 0x1ff8)?;
    let cur = c.player().y;
    let bx = c.word_in(SS, c.reg(SP).wrapping_add(2));
    if bx != cur {
        c.run(0x1ff8, 5)?;
        let d = c.pop();
        c.set_reg(DX, d);
        let b = c.pop();
        c.set_reg(BX, b);
        return super::map::count_shown_location(c.m, 0x5d50);
    }
    c.run(0x1ff8, 10)?;
    let d = c.pop();
    c.set_reg(DX, d);
    let b = c.pop();
    c.set_reg(BX, b);
    c.update_player(|p| p.x = d);
    let al = d as u8;
    c.set_reg8(r8::AL, al);
    c.update_progress(|t| t.room = al);
    let v = c.byte(g::REDRAW);
    let v = pure::or(B, v as u16, al as u16);
    c.set_byte(g::REDRAW, v as u8);
    c.update_progress(|t| t.harkonnen_place = al);
    c.call(entry::NOTE_BATTLE_BALANCE, 0x2013)?;
    c.run(0x2013, 1)?;
    c.ret()
}

/// `sub_1dd4` (`loc_020a4`): the spice demand's day check, with bit 7 of
/// [0BFh]. On the day [118Dh] (the day count from `get_ingame_day_q`):
/// in the endgame ([`g::ENDGAME_STAGE`]) or otherwise, the days left in [0CFh]
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
    let f = c.economy().shipment_flags;
    if f & 0x80 == 0 {
        c.run(0x20a4, 3)?;
        return c.ret();
    }
    c.run(0x20a4, 3)?;
    let ax = time::game_day(&mut c, 0x20ae)?;
    let day = c.word(g::SHIPMENT_DUE);
    let d = ax.wrapping_sub(day);
    let pend = c.economy().endgame_stage;
    let f = c.economy().shipment_flags;
    let late = c.byte(g::SPICE_DEMAND_LAPSED);
    if pend != 0 || f & 0x10 == 0 && late == 0 {
        // 2098h / 20C6h: the days left.
        let n = if pend != 0 { 2 } else { 6 } + 2 + if d != 0 { 3 } else { 0 };
        if pend == 0 && d == 0 {
            return new_demand(&mut c, ax, day, f);
        }
        c.run(0x20ae, n + if d == 0 { 1 } else { 0 })?;
        let a = pure::sub(W, ax, day);
        c.set_reg(AX, a);
        if d != 0 {
            let a = pure::neg(W, a);
            c.set_reg(AX, a);
            c.update_situation(|t| t.shipment_days_left = a as u8);
        }
        return c.ret();
    }
    if f & 0x10 == 0 {
        // [11BBh] set: 215Fh.
        c.run(0x20ae, 7 + 2)?;
        c.set_byte(g::COMM_CALLER, 7);
        return c.ret();
    }
    // 2131h: reminders.
    if d == 0 {
        c.run(0x20ae, 6 + 1)?;
        let a = pure::sub(W, ax, day);
        c.set_reg(AX, a);
        return c.ret();
    }
    if d >= 4 {
        c.run(0x20ae, 8 + 2)?;
        let a = pure::sub(W, ax, day);
        c.set_reg(AX, a);
        c.set_byte(g::COMM_CALLER, 7);
        return c.ret();
    }
    c.run(0x20ae, 12)?;
    let a = pure::sub(W, ax, day);
    c.set_reg(AX, a);
    let a = pure::shl1(B, a & 0xff, 2);
    c.set_reg(AX, d & 0xff00 | a);
    c.set_reg8(r8::BL, a as u8);
    c.call(entry::GRADE_BE, 0x2145)?;
    let ah = (c.reg(AX) >> 8) as u8;
    let g = if ah < 3 { ah } else { 2 };
    let bl = c.reg8(r8::BL);
    let idx = g.wrapping_add(bl);
    let code = c.byte_in(pc::cpu::CS, 0x2161u16.wrapping_add(idx as u16));
    c.run(0x2145, 3 + if ah >= 3 { 1 } else { 0 } + 5 + if code == 0 { 2 } else { 3 })?;
    c.set_reg8(r8::AL, ah);
    let a = pure::add(B, g as u16, bl as u16);
    c.set_reg(BX, 0x2161);
    c.set_reg8(r8::AL, code);
    let _ = a;
    if code == 0 {
        c.set_byte(g::COMM_CALLER, 7);
        return c.ret();
    }
    c.set_reg(AX, (code as u16) << 8 | 0x0b);
    add_message(c.m, 0x26da)
}

/// `loc_020d2` (after the day test, at 20AEh): the new demand.
fn new_demand(c: &mut Ctx, ax: u16, day: u16, f: u8) -> Result<(), Bail> {
    c.run(0x20ae, 6 + 2 + 11)?;
    c.cmp(B, 0, 0);
    c.test(B, f as u16, 0x10);
    c.cmp(B, 0, 0);
    let a = c.sub(W, ax, day);
    c.set_reg(AX, a);
    let k = c.economy().demands;
    c.set_reg8(r8::AL, k);
    let v = c.inc(B, k as u16);
    c.update_economy(|t| t.demands = v as u8);
    c.set_reg(BX, 0x96);
    c.mul16(0x96);
    let (ax, dx) = (c.reg(AX), c.reg(DX));
    c.or(W, dx, dx);
    let cx = c.add(W, ax, 0x64);
    c.set_reg(AX, cx);
    c.set_reg(CX, cx);
    let random = math::random_masked(c, 0x3f, 0x20ef)?;
    let r = random.wrapping_add(0xe0);
    let p = r as u32 * cx as u32;
    let (pdx, pax) = ((p >> 16) as u16, p as u16);
    let be = c.economy().shipment_ratio;
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
    c.update_economy(|t| t.demanded = dx);
    c.update_situation(|t| t.shipment_days_left = 0);
    let v = c.economy().shipment_flags;
    let v = c.or(B, v as u16, 0x90);
    c.update_economy(|t| t.shipment_flags = v as u8);
    let be2 = c.economy().shipment_ratio;
    c.cmp(B, be2 as u16, 0);
    let mut ax = 0x20bu16;
    if be2 & 0x80 == 0 {
        let h = c.inc(B, 2);
        ax = h << 8 | 0x0b;
    }
    c.set_reg(AX, ax);
    add_message(c.m, 0x26da)
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
    let t = c.byte(g::SCENE_TIMER).wrapping_sub(1);
    let day = c.situation().event_kind;
    let pic = !((day as i8) > 0) && t <= 0x10;
    c.run(start, 4 + if (day as i8) > 0 { 0 } else { 3 } + if pic { 2 } else { 0 })?;
    c.set_reg(SI, 0x4856);
    c.set_byte(g::SCENE_TIMER, t);
    if (day as i8) <= 0 {
        c.set_reg8(r8::BL, t);
        if pic {
            // `loc_00d0d` reads ZF of the `cmp bl, 10h`.
            c.cmp(B, t as u16, 0x10);
            c.push(0x4856);
            c.call(entry::SCENE_PICTURE, 0x0b5e)?;
            c.run(0x0b5e, 1)?;
            let s = c.pop();
            c.set_reg(SI, s);
        }
    }
    let a = c.text_box().shown;
    let b = c.map_view().mode;
    let v = a & 0xff00 | ((a as u8) | b) as u16;
    if v != 0 {
        c.run(0x0b5f, 5)?;
        c.set_reg(AX, v);
        return c.ret();
    }
    let si = c.reg(SI);
    let w4 = c.word(si.wrapping_add(4)).wrapping_sub(1);
    let neg4 = w4 & 0x8000 != 0;
    let b6 = c.byte(si.wrapping_add(6)).wrapping_sub(1);
    let neg6 = b6 & 0x80 != 0;
    c.run(0x0b5f, 4 + 2 + if neg4 { 3 } else { 0 })?;
    c.set_reg(AX, v);
    c.set_word(si.wrapping_add(4), w4);
    if neg4 {
        c.set_byte(si.wrapping_add(6), b6);
        let r = math::random_word(&mut c, if neg6 { 0x0b78 } else { 0x0b89 })?;
        if neg6 {
            // A new wait.
            c.run(0x0b78, 6)?;
            c.set_byte(si.wrapping_add(6), (r & 0x7f) as u8);
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
            // A particle: kind 1Ch + 3 bits, y 30h..5Fh, x below 140h.
            c.set_reg(DX, r);
            c.set_reg(BX, bl as u16);
            if (0x30..0x60).contains(&bl) {
                c.set_reg(DX, dxv);
                if dxv < 0x140 {
                    c.set_reg(AX, (bl & 7) as u16 + 0x1c);
                    c.push(si);
                    c.set_reg(SI, 0);
                    c.call(entry::PARTICLES_SPAWN_PARTICLE, 0x0bb6)?;
                    c.run(0x0bb6, 1)?;
                    let s = c.pop();
                    c.set_reg(SI, s);
                }
            }
        }
    }
    let si = c.reg(SI);
    let k = c.byte(si).wrapping_sub(1);
    c.run(0x0bb7, if k & 0x80 != 0 { 3 } else { 2 })?;
    c.set_byte(si, k);
    if k & 0x80 != 0 {
        c.call(entry::CREATURE_STEP, 0x0bbe)?;
    }
    let n = c.word(g::PARTICLES);
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
            // Two random bits from the seed (`rol word [0], 1`; `rcl ax, 1`).
            let mut a = ax >> 2;
            for _ in 0..2 {
                let seed = c.word(0);
                c.set_word(0, seed.rotate_left(1));
                a = a << 1 | seed >> 15;
            }
            c.set_reg(AX, a);
        } else if al >= 0x1c {
            let a1 = ax.wrapping_add(1);
            let past = (a1 as u8) > 0x2d;
            c.run(0x0bc6, if past { 11 } else { 10 })?;
            c.set_reg(AX, ax);
            c.set_reg(DX, dx);
            c.push(cx);
            c.set_reg(AX, a1);
            remove = past;
        } else {
            c.run(0x0bc6, 9)?;
            c.set_reg(AX, ax);
            c.set_reg(DX, dx);
            c.push(cx);
            let b = c.word(di.wrapping_add(0x0d));
            c.set_reg(BX, b);
            c.call(entry::UNPACK_SIGNED, 0x0be2)?;
            c.run(0x0be2, 1)?;
            c.call(entry::UNPACK_SIGNED, 0x0be5)?;
            c.run(0x0be5, 8)?;
            let bx = c.reg(BX);
            c.set_word(di.wrapping_add(0x0d), bx);
            let seed = c.word(0).rotate_left(3);
            c.set_reg8(r8::CL, 3);
            c.set_word(0, seed);
            c.set_reg(AX, (seed & 7) + 0x14);
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
            c.call(entry::PARTICLES_MOVE_PARTICLE, 0x0c1c)?;
            let di = c.word_in(SS, c.reg(SP));
            let (x0, y0, y1) = (c.word(di), c.word(di.wrapping_add(4)), c.word(di.wrapping_add(6)));
            let off = x0 >= 0x140 || y0 & 0x8000 != 0 || y1 & 0x8000 != 0;
            let n = if x0 >= 0x140 { 3 } else if y0 & 0x8000 != 0 { 6 } else { 8 };
            c.run(0x0c1c, n)?;
            let d = c.pop();
            c.set_reg(DI, d);
            if x0 < 0x140 {
                c.set_reg(AX, 0);
            }
            remove = off;
        }
        if remove {
            c.run(0x0c2f, 2)?;
            let di = c.reg(DI);
            c.push(di);
            c.call(entry::PARTICLES_REMOVE_PARTICLE, 0x0c33)?;
            c.run(0x0c33, 1)?;
            let d = c.pop();
            c.set_reg(DI, d);
        }
        let cx = c.word_in(SS, c.reg(SP)).wrapping_sub(1);
        c.run(0x0c34, if cx == 0 { 4 } else { 3 })?;
        c.pop();
        let di = c.reg(DI).wrapping_add(0x11);
        c.set_reg(DI, di);
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
/// and for a stage the picked record (`pick_one_of_two_speakers`) shown with its voice
/// after a wait of 4Bh, its sentence (`loc_096d8`) and record
/// (`loc_03551`); without one the watch, and for a stage the scene 24h
/// with the phrase BFh, the panel 1F9Eh (bit 40h) and its buttons.
pub fn scene_start(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x35ad)?;
    let mut c = Ctx::new(m);
    let mode = c.travel().screen_kind;
    let was = c.conversation().scene_flags;
    if mode == 0 {
        if was != 0 {
            c.run(start, 9)?;
            scene_flags(&mut c, was, false);
            return c.ret();
        }
        let k = c.progress().room;
        let f = c.economy().shipment_chosen & c.word(g::SHIPMENT_READY);
        let t = c.progress().present;
        let call = k == 8 && f != 0 && t & 8 != 0;
        let n = 11 + if k != 8 { 0 } else if f == 0 { 3 } else { 5 } + call as u32;
        c.run(start, n)?;
        scene_flags(&mut c, was, false);
        let v = c.progress().answer_count;
        c.update_progress(|t| t.answer_count = v.wrapping_add(1));
        if k == 8 {
            c.set_reg(AX, f);
        }
        if call {
            c.call(entry::SHIP_SPICE_TO_EMPEROR, 0x35e3)?;
        }
        c.run(0x35e3, 2)?;
        c.set_reg(BP, 0x3520);
        return for_records_here_bp(c.m, 0x36ee);
    }
    if was != 0 {
        c.run(start, 10)?;
        scene_flags(&mut c, was, true);
        return c.ret();
    }
    let p = c.word(g::SPEAKER_SLOTS);
    c.run(start, 12)?;
    scene_flags(&mut c, was, true);
    if p != 0xffff {
        c.call(entry::SCAN_ENCOUNTERS, 0x3606)?;
        c.run(0x3606, 1)?;
        c.call(entry::MAP_PLACE_WATCH, 0x3609)?;
        let st = c.progress().talk_reason;
        if st == 0 {
            c.run(0x3609, 3)?;
            c.set_flag(CF, false);
            return c.ret();
        }
        c.run(0x3609, 3)?;
        c.call(entry::PICK_ONE_OF_TWO_SPEAKERS, 0x3613)?;
        if c.flag(pc::cpu::SF) {
            c.run(0x3613, 2)?;
            return c.ret();
        }
        c.run(0x3613, 2)?;
        c.call(entry::CALL_RESTORE_CURSOR, 0x3618)?;
        c.run(0x3618, 1)?;
        c.call(entry::SHOW_SPEAKER_SCREEN, 0x361b)?;
        c.run(0x361b, 3)?;
        let ax = c.reg(AX);
        c.push(ax);
        waits::wait_ticks(&mut c, 0x4b, 0x3622)?;
        c.run(0x3622, 3)?;
        let a = c.pop();
        c.set_reg(AX, a);
        c.push(a);
        c.call(entry::TALK_AS, 0x3627)?;
        let cf = c.flag(CF);
        c.run(0x3627, if cf { 3 } else { 7 })?;
        let a = c.pop();
        c.set_reg(AX, a);
        if cf {
            return c.ret();
        }
        c.set_reg8(r8::CL, 0x10);
        c.mul8(0x10);
        let a = c.reg(AX).wrapping_add(0xfd8);
        c.set_reg(AX, a);
        c.set_reg(SI, a);
        c.call(entry::SCENE_RECORD, 0x3636)?;
        c.run(0x3636, 1)?;
        return c.ret();
    }
    c.call(entry::MAP_PLACE_WATCH, 0x363a)?;
    let st = c.progress().talk_reason;
    if st == 0 {
        c.run(0x363a, 3)?;
        c.set_flag(CF, false);
        return c.ret();
    }
    c.run(0x363a, 3)?;
    c.call(entry::CALL_RESTORE_CURSOR, 0x3644)?;
    c.run(0x3644, 1)?;
    c.call(entry::SCENE_24, 0x3647)?;
    c.run(0x3647, 5)?;
    text::draw_string_at(&mut c, 0xbf, 0x66, 0x4e, 0x200c, 0x3656)?;
    for (at, to) in [(0x3656u16, 0xc0f4u16), (0x3659, 0xc4dd), (0x365c, 0x4aca)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    c.run(0x365f, 4)?;
    c.set_reg(BP, 0x1f9e);
    let v = c.byte_in(SS, 0x1fa9);
    c.m.cpu.write8(&mut c.m.hw, SS, 0x1fa9, v | 0x40);
    c.set_reg(BX, 0x4abe);
    c.call(entry::PUSH_SCREEN, 0x366c)?;
    c.run(0x366c, 1)?;
    super::screens::panel_buttons(c.m, 0x2ffb)
}

/// `loc_035ad`'s start: [1Ah], [47A7h] (and in a map mode [23h]) 0,
/// [47A6h] taken.
fn scene_flags(c: &mut Ctx, was: u8, map: bool) {
    c.set_reg(AX, 0);
    c.update_progress(|t| t.answer_count = 0);
    c.update_conversation(|t| t.scene_count = 0);
    if map {
        c.update_progress(|t| t.talk_reason = 0);
    }
    c.update_conversation(|t| t.scene_flags = 0);
    c.set_reg8(r8::AL, was);
    c.or(B, was as u16, was as u16); // the flags of the early exits
}

/// `loc_02e52`: the scene refreshed after a move: `scene_start`, the time
/// noted ([`g::LAST_ACTION_TIME`] = [`g::TIMER`]); unless the scene took over ([47A7h]):
/// travelling ([`g::WALK_STEPS`] bit 7) a travel step (`loc_03723`); else outside a
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
    c.call(entry::SCENE_START, 0x2e55)?;
    let t = c.timer();
    let k = c.conversation().scene_count;
    let note = |c: &mut Ctx| {
        c.set_reg(AX, t);
        c.set_word(g::LAST_ACTION_TIME, t);
        c.cmp(B, k as u16, 0);
    };
    if k != 0 {
        c.run(0x2e55, 5)?;
        note(c);
        return c.ret();
    }
    let al = c.byte(g::WALK_STEPS);
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
    let kind = c.progress().place_kind;
    if kind != 0xff {
        let p = c.sequence().playing;
        c.run(0x2e55, 12)?;
        head(c);
        c.cmp(B, kind as u16, 0xff);
        c.cmp(B, p as u16, 0);
        if p != 0 {
            return c.ret();
        }
        return super::ui::ui_head_animate_up(c.m, 0x17e6);
    }
    let mode = c.travel().screen_kind;
    if mode != 0 {
        c.run(0x2e55, 12)?;
        head(c);
        c.cmp(B, kind as u16, 0xff);
        c.cmp(B, mode as u16, 0);
        return c.ret();
    }
    let si = c.conversation().first_speaker;
    c.run(0x2e55, if si == 0 { 15 } else { 18 })?;
    head(c);
    c.cmp(B, kind as u16, 0xff);
    c.cmp(B, 0, 0);
    c.set_reg(SI, si);
    c.or(W, si, si);
    if si == 0 {
        return c.ret();
    }
    c.xor(W, 0, 0);
    let v = c.byte(si.wrapping_add(0x0e)) as u16;
    c.set_reg(AX, v);
    c.update_conversation(|t| t.talker = v);
    c.call(entry::REDRAW_DIALOGUE_SCENE, 0x2e97)?;
    c.run(0x2e97, 1)?;
    c.ret()
}

/// `loc_03723`: a travel step (AL = [`g::WALK_STEPS`], bit 7 then dropped): from
/// 94h, every 10h, the view redrawn (`loc_037ad`) and a wait of
/// ((AL - 94h) / 10h + 1, at most 0Ah) * 100h; before 37h the arrival
/// (`loc_02db1`); else the fade-out, the handlers 1A0Fh and 401Fh, and
/// the move to the place's room ([4], DL 2 / 0Ah for the sietches and
/// palaces) scheduled at 4057h before `scene_start`.
fn travel_step(c: &mut Ctx) -> Result<(), Bail> {
    let al = c.reg8(r8::AL);
    let s = c.byte(g::WALK_STEPS);
    let a = al.wrapping_sub(0x94);
    let n = if al < 0x94 { 4 } else if a & 0x0f != 0 { 6 } else { 7 };
    c.run(0x3723, n)?;
    let v = c.and(B, s as u16, 0x7f);
    c.set_byte(g::WALK_STEPS, v as u8);
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
    c.call(entry::CALL_RESTORE_CURSOR, 0x3734)?;
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
    waits::wait_ticks(c, x << 8, 0x3750)?;
    let s = c.byte(g::WALK_STEPS);
    c.run(0x3750, 3)?;
    c.cmp(B, s as u16, 0x37);
    if s < 0x37 {
        return room_arrive(c.m, 0x2db1);
    }
    c.call(entry::PLAY_WORMSUIT_VIDEO, 0x375a)?;
    c.run(0x375a, 2)?;
    time::run_clock(c, 5, 0x3760)?;
    c.run(0x3760, 3)?;
    screen::transition_drawn(c, entry::GFX_CLEAR_ACTIVE_FRAMEBUFFER, 0x34, 0x3768)?;
    c.run(0x3768, 1)?;
    c.call(entry::DRAW_HEAD_PANEL, 0x376b)?;
    c.run(0x376b, 2)?;
    gfx::draw_on_front_buffer(c, entry::REDRAW_DATE_PANEL, 0x3771)?;
    c.run(0x3771, 4)?;
    let si = c.word(g::LAST_PLACE);
    c.set_reg(SI, si);
    c.update_conversation(|t| t.scene_flags = 0xff);
    gfx::draw_on_front_buffer(c, 0x401f, 0x3780)?;
    let dx = c.player().x;
    let bx = c.player().y;
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
    c.update_conversation(|t| t.scene_flags = 0xff);
    c.call(entry::TRANSITION_AL, 0x37a3)?;
    c.run(0x37a3, 2)?;
    let e = c.situation().desert_rescues;
    let e = c.inc(B, e as u16);
    c.update_situation(|t| t.desert_rescues = e as u8);
    scene_start(c.m, 0x35ad)
}

/// `loc_02db1` / `loc_02dbf`: the arrival in a room (from `loc_02db1`
/// the handler D717h, `loc_0d95b` and [47A6h] FFh first): `loc_02d74`,
/// the speaker and its flags dropped, the handler 2EB2h; at a place the
/// Harkonnens hold ([2Bh]) its view (`loc_00acd`) and the head raised;
/// else the room drawn
/// (`loc_05ba0`, `loc_037b2`, `loc_0c412` outside the map modes), with
/// the light changed ([46DFh] to [46E0h]) a fade (10h, F66h) unless the
/// palettes match, and the scene refreshed (`loc_02e52`).
pub fn room_arrive(m: &mut Machine, start: u16) -> Result<(), Bail> {
    let mut c = Ctx::new(m);
    match start {
        0x2db1 => {
            c.run(0x2db1, 2)?;
            gfx::draw_on_front_buffer(&mut c, entry::SET_BUTTONS_FOR_SCREEN, 0x2db7)?;
            c.run(0x2db7, 1)?;
            c.call(entry::SET_MAP_CLICK_HANDLER, 0x2dba)?;
            c.run(0x2dba, 2)?;
            c.update_conversation(|t| t.scene_flags = 0xff);
        }
        0x2dbf => c.run(0x2dbf, 1)?,
        0x2dd3 => {}
        _ => return Err(Bail),
    }
    if start != 0x2dd3 {
        c.call(entry::OPEN_SAL_RESOURCE, 0x2dc2)?;
        c.run(0x2dc2, 6)?;
        c.set_reg(AX, 0);
        c.set_byte(g::TRANSITION_ACTIVE, 0);
        c.update_conversation(|t| t.line_state = 0);
        c.update_conversation(|t| t.first_speaker = 0);
        gfx::draw_on_front_buffer(&mut c, entry::PLACE_PANEL_BACK, 0x2dd3)?;
    }
    let k = c.progress().harkonnen_place;
    if k != 0 {
        c.run(0x2dd3, 4)?;
        c.set_byte(g::SAL_FLAGS, 0);
        c.call(entry::OPEN_SAL_RESOURCE, 0x2de2)?;
        c.run(0x2de2, 3)?;
        let v = c.byte(g::SKIP_HARKONNEN_TAKE);
        c.set_byte(g::SKIP_HARKONNEN_TAKE, v | 1);
        c.set_byte(g::SKY_PALETTE_SET, 0);
        c.call(entry::INTRO_NIGHT_ATTACK, 0x2def)?;
        for (at, to) in [(0x2defu16, 0x1797u16), (0x2df2, 0xc4cd), (0x2df5, 0xc0f4)] {
            c.run(at, 1)?;
            c.call(to, at + 3)?;
        }
        c.run(0x2df8, 1)?;
        return super::ui::ui_head_animate_up(c.m, 0x17e6);
    }
    let f = c.byte(g::SAL_FLAGS);
    c.run(0x2dd3, 4 + (f & 1 != 0) as u32)?;
    if f & 1 != 0 {
        c.call(entry::PLAY_SAL_SCENE, 0x2e05)?;
    }
    c.run(0x2e05, 4)?;
    c.set_reg(AX, 0);
    c.update_progress(|t| t.talked_here = 0);
    c.set_byte(g::SKY_PALETTE_SET, 0);
    c.call(entry::SET_FB1_AS_ACTIVE_FRAMEBUFFER, 0x2e10)?;
    for (at, to) in [(0x2e10u16, 0x5ba0u16), (0x2e13, 0x37b2)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    let mode = c.travel().screen_kind;
    c.run(0x2e16, 2 + (mode & 3 == 0) as u32)?;
    if mode & 3 == 0 {
        c.call(entry::COPY_ACTIVE_FRAMEBUFFER_TO_FRAMEBUFFER_2, 0x2e20)?;
    }
    for (at, to) in [(0x2e20u16, 0xad5eu16), (0x2e23, 0x1834), (0x2e26, 0x1797)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    let a = c.byte(g::SKY_PALETTE_SET);
    let old = c.byte(g::SKY_PALETTE_SHOWN);
    let swap = |c: &mut Ctx| {
        c.set_reg8(r8::AL, a);
        c.set_reg8(r8::AH, a);
        c.set_byte(g::SKY_PALETTE_SHOWN, a);
        c.set_reg8(r8::AL, old);
    };
    if old == a {
        c.run(0x2e29, 6)?;
        swap(&mut c);
        c.call(entry::UPDATE_SCREEN_PALETTE, 0x2e4f)?;
        c.run(0x2e4f, 1)?;
        c.call(entry::GFX_COPY_WHOLE_FRAMEBUF_TO_SCREEN, 0x2e52)?;
        return refresh_at(&mut c);
    }
    let (x, y) = (c.screen_buffers().front, c.screen_buffers().screen);
    c.run(0x2e29, if x == y { 8 } else { 11 })?;
    swap(&mut c);
    c.set_reg(AX, x);
    if x != y {
        screen::transition_drawn(&mut c, entry::NOTHING, 0x10, 0x2e47)?;
        c.run(0x2e47, 1)?;
        c.call(entry::MUSIC_STEP_IF_ENABLED, 0x2e4a)?;
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
    c.call(entry::MOVE_PARTY_HERE, 0x405a)?;
    let dx = c.reg(DX);
    let bx = c.reg(BX);
    let f = c.map_view().mode;
    if f & 0x80 == 0 && dx == 0x3002 {
        c.m.cpu.ip = 0x405a;
        return Err(Bail);
    }
    c.run(0x405a, if f & 0x80 != 0 { 8 } else { 10 })?;
    c.update_player(|p| p.x = dx);
    let old = c.progress().room;
    c.update_progress(|t| t.room = dx as u8);
    c.set_reg8(r8::AL, old);
    c.update_progress(|t| t.previous_room = old);
    c.update_player(|p| p.y = bx);
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
/// low byte not 80h) a travel step: [`g::WALK_STEPS`] counts up (bit 7 set) and
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
    c.call(entry::LEAVE_SCREENS, 0x3f2b)?;
    c.run(0x3f2b, 1)?;
    c.call(entry::CALL_PCM_AUDIO_VTABLE_FUNC_5, 0x3f2e)?;
    c.run(0x3f2e, 1)?;
    c.call(entry::LIP_SYNC_STOP, 0x3f31)?;
    let dx = c.player().x;
    let bx = c.player().y;
    let start_ops = |c: &mut Ctx| {
        let bp = c.pop();
        c.set_reg(BP, bp);
        c.update_conversation(|t| t.caller = 0);
        c.update_progress(|t| t.first_time_inside = 0);
        c.set_reg(DX, dx);
        c.set_reg(BX, bx);
    };
    if bx & 0xff != 0x80 {
        let s = c.byte(g::WALK_STEPS);
        let f4 = c.situation().desert_fatigue;
        let a = (s & 0x7f).wrapping_add(1);
        let n = 7 + 4 + (a & 0x80 != 0) as u32 + 4 + (f4 < 0x14) as u32 + 1;
        c.run(0x3f31, n)?;
        start_ops(&mut c);
        // The travel steps counted up to 7Fh (bit 7 set).
        let x = (if a & 0x80 != 0 { a - 1 } else { a }) | 0x80;
        c.set_reg8(r8::AL, x);
        c.set_byte(g::WALK_STEPS, x);
        if f4 < 0x14 {
            c.update_situation(|t| t.desert_fatigue = f4 + 1);
        }
        return step_3ff5(&mut c);
    }
    c.run(0x3f31, 8)?;
    start_ops(&mut c);
    let entry_ = records::list_entry_ptr(&mut c, 0x3f6a)?;
    let (bp, si) = (c.reg(BP), entry_);
    let dl = c.byte_in(SS, bp.wrapping_add(si));
    let pick = |c: &mut Ctx| {
        c.set_reg8(r8::DL, dl);
        c.or(B, dl as u16, dl as u16); // the flags of the exit without one
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
    let b = c.progress().room;
    c.run(0x3f6a, if b == 1 { 7 } else { 6 })?;
    if b == 1 {
        pick(&mut c);
        c.call(entry::SAVE_REGS, 0x3f7c)?;
        c.run(0x3f7c, 2)?;
        tails::save_to_slot(&mut c, 2, 0x3f81)?;
        c.run(0x3f81, 1)?;
        c.call(entry::RESTORE_REGS, 0x3f84)?;
    } else {
        pick(&mut c);
    }
    let si = c.word(g::PLACE);
    let t = Location(si).flags(&mut c);
    c.run(0x3f84, if t & location_flag::VISITED != 0 { 3 } else { 8 })?;
    if t & location_flag::VISITED != 0 {
        c.set_reg(SI, si);
    } else {
        c.set_reg(SI, si);
        // A first visit: noted, and counted when DH is below 20h.
        Location(si).set_flags(&mut c, t | location_flag::VISITED);
        let dh = c.reg8(r8::DH);
        let k = c.progress().sietches_entered.wrapping_add((dh < 0x20) as u8);
        c.update_progress(|t| t.sietches_entered = k);
        c.update_progress(|t| t.first_time_inside = 0xff);
        c.call(entry::SAVE_REGS, 0x3fa2)?;
        c.run(0x3fa2, 2)?;
        tails::save_to_slot(&mut c, 3, 0x3fa7)?;
        c.run(0x3fa7, 1)?;
        c.call(entry::RESTORE_REGS, 0x3faa)?;
    }
    c.run(0x3faa, 3)?;
    let dl = c.reg8(r8::DL);
    c.update_progress(|t| t.entering_room = dl);
    c.update_progress(|t| t.talk_reason = 1);
    c.call(entry::ALLOW_SCRIPTED_ACTION, 0x3fb6)?;
    c.run(0x3fb6, 3)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.call(entry::HANDLE_FLAG_23, 0x3fbb)?;
    c.run(0x3fbb, 3)?;
    let v = c.pop();
    c.set_reg(DX, v);
    let v = c.pop();
    c.set_reg(BX, v);
    if !scripted_action_allowed(&mut c, 0x3fc0)? {
        c.run(0x3fc0, 2)?;
        return c.ret();
    }
    c.run(0x3fc0, 4)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.call(entry::WHILE_VOICE_PLAYING, 0x3fc8)?;
    c.run(0x3fc8, 4)?;
    let v = c.pop();
    c.set_reg(DX, v);
    let v = c.pop();
    c.set_reg(BX, v);
    c.update_progress(|t| t.talk_reason = 5);
    enter_room_at(&mut c)
}

/// `loc_03fd2`: out of the place by the exit -DL: [0E7h] 0, BP the
/// direction, the place [114Eh] taken (0 left) and its position and
/// map cell in DX / BX, [8] and [9] FFh; then the move (`loc_03ff5`).
fn leave_place(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x3fd2, 11)?;
    c.update_situation(|t| t.desert_rescues = 0);
    let dl = c.reg8(r8::DL);
    let dh = c.reg8(r8::DH);
    c.xor(B, dh as u16, dh as u16);
    let n = c.neg(B, dl as u16);
    c.set_reg(DX, n);
    c.set_reg(BP, n);
    c.xor(W, 0, 0);
    let si = c.word(g::PLACE);
    c.set_word(g::PLACE, 0);
    c.set_reg(SI, si);
    let dx = c.word(si.wrapping_add(2));
    let bx = c.word(si.wrapping_add(4));
    c.set_reg(DX, dx);
    c.xor(B, bx >> 8, bx >> 8);
    c.set_reg(BX, bx & 0xff);
    c.update_progress(|t| t.place_kind = 0xff);
    c.update_progress(|t| t.place_index = 0xff);
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
    c.call(entry::STEP_GLOBE_POSITION, 0x3ffe)?;
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
    let cell = globe::globe_map_byte(c, 0x400a)?;
    let bx = c.reg(BX);
    c.run(0x400a, if cell & 0x40 == 0 { 3 } else { 4 })?;
    c.xor(B, bx >> 8, bx >> 8);
    c.set_reg(BX, bx & 0xff);
    c.test(B, cell as u16, 0x40);
    if cell & 0x40 == 0 {
        return enter_room_at(c);
    }
    if !records::location_found(c, 0x4013)? {
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
    c.set_byte(g::WALK_STEPS, 0);
    c.set_word(g::PLACE, si);
    c.set_word(g::LAST_PLACE, si);
    enter_location_at(c, si, 0x4031)?;
    c.run(0x4031, 3)?;
    c.update_place_forces(|t| t.our_loss = 0);
    c.update_place_forces(|t| t.harkonnen_loss = 0);
    c.call(entry::DISCOVER_LOCATION, 0x4040)?;
    c.run(0x4040, 1)?;
    c.call(entry::LOCATION_COORDS, 0x4043)?;
    let dh = c.reg8(r8::DH);
    let bh = c.reg8(r8::BH);
    let si = c.reg(SI);
    c.run(0x4043, 4 + (dh >= 0x20) as u32 + 1)?;
    c.update_progress(|t| t.place_kind = dh);
    c.update_progress(|t| t.place_index = bh);
    c.cmp(B, dh as u16, 0x20);
    if dh >= 0x20 {
        let v = Location(si).flags(c);
        let v = c.or(B, v as u16, 0x10);
        Location(si).set_flags(c, v as u8);
    }
    c.call(entry::UPDATE_PEOPLE, 0x4057)?;
    enter_room_at(c)
}

/// `loc_037b2` / `loc_037b5`: the room drawn (from `loc_037b2` the
/// dialogue reset first): `loc_04d00`, [472Dh] 0, `loc_05ba8`,
/// `loc_0c432`; at a place the room's scene byte (`loc_03efe`): a
/// scene (positive) is drawn by `loc_039ec`. Else `loc_03ae9`, [47A4h]
/// bit 0; outside the map modes the place's view (`loc_0380c`,
/// `place_scene`, `loc_04d06`); in them the map ([4728h] 0, `loc_04988`,
/// `loc_04a5a`, the resource [`g::FLIGHT_VIDEO`] through the driver) and the sky
/// (`set_sky_palette`).
pub fn draw_room(m: &mut Machine, start: u16) -> Result<(), Bail> {
    let mut c = Ctx::new(m);
    match start {
        0x37b2 => {
            c.run(0x37b2, 1)?;
            c.call(entry::RESET_DIALOGUE, 0x37b5)?;
        }
        0x37b5 => {}
        _ => return Err(Bail),
    }
    c.run(0x37b5, 1)?;
    c.call(entry::STOP_ALARM_TASK, 0x37b8)?;
    c.run(0x37b8, 2)?;
    c.set_word(g::ROOM_FIGURES_X, 0);
    c.call(entry::COPY_GAME_AREA_RECT_TO_CLIP_RECT, 0x37c1)?;
    c.run(0x37c1, 1)?;
    c.call(entry::CLEAR_GAME_AREA, 0x37c4)?;
    let kind = c.progress().place_kind;
    c.run(0x37c4, if kind == 0xff { 6 } else { 5 })?;
    if kind == 0xff {
        c.set_reg(AX, 0xffff);
    } else {
        c.set_reg(AX, 0xffff);
        let dx = c.player().x;
        c.set_reg(DX, dx);
        let si = records::list_entry_ptr(&mut c, 0x37d4)?;
        let ah = c.reg8(r8::AH);
        let al = c.byte(si);
        let ax = (ah as u16) << 8 | al as u16;
        c.run(0x37d4, 4)?;
        c.lods(B, DS);
        if ax & 0x8000 == 0 {
            return room_scene_39ec(&mut c);
        }
    }
    c.call(entry::CLEAR_SPEAKER_PLACES, 0x37df)?;
    let mode = c.travel().screen_kind;
    let v = c.conversation().line_state;
    if mode & 3 == 0 {
        c.run(0x37df, 4)?;
        c.update_conversation(|t| t.line_state = v | 1);
        c.call(entry::SET_ROOM_LIGHT_PALETTE, 0x37ee)?;
        c.run(0x37ee, 1)?;
        c.call(entry::PLACE_SCENE, 0x37f1)?;
        c.run(0x37f1, 1)?;
        return enemy_watch(&mut c);
    }
    c.run(0x37df, 5)?;
    c.update_conversation(|t| t.line_state = v | 1);
    c.update_flight(|t| t.marker = 0);
    c.call(entry::OPEN_ZOOMED_MAP, 0x37fc)?;
    c.run(0x37fc, 1)?;
    c.call(entry::DRAW_MAP_POINTS, 0x37ff)?;
    c.run(0x37ff, 2)?;
    let ax = c.word(g::FLIGHT_VIDEO);
    c.set_reg(AX, ax);
    c.call(entry::HNM_LOAD_FIRST_FRAME, 0x3805)?;
    c.interp(0x3805, 0x3809)?;
    c.run(0x3809, 1)?;
    super::ui::set_sky_palette(c.m, 0x388d)
}

/// `loc_04d06`: [0F6h] 0; when the place [1150h] is here ([4], its +4
/// against [6]) and [4734h] is set and not below the position's high
/// byte, the troops' march shown ([0F6h] 1, `loc_04d57` with AH 1,
/// `loc_04bdf`, `loc_0c412`) and past [4733h] the alarm (`loc_0ab15`
/// with 5, `loc_04d57` with AH 0, the task 4BB9h every 10h).
fn enemy_watch(c: &mut Ctx) -> Result<(), Bail> {
    let di = c.word(g::LAST_PLACE);
    let dx = c.word(di.wrapping_add(2));
    let here = c.player().x;
    let ax = c.word(g::PLACE_TROOP_COUNT);
    let ah = (ax >> 8) as u8;
    let bx = c.player().y;
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
    c.update_situation(|t| t.march_shown = 0);
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
    let f = c.situation().march_shown;
    let f = c.inc(B, f as u16);
    c.update_situation(|t| t.march_shown = f as u8);
    c.set_reg8(r8::AH, 1);
    c.call(entry::SCROLL_SCENE_31, 0x4d37)?;
    c.run(0x4d37, 1)?;
    c.call(entry::DRAW_SCROLL_SCENE, 0x4d3a)?;
    c.run(0x4d3a, 1)?;
    c.call(entry::COPY_ACTIVE_FRAMEBUFFER_TO_FRAMEBUFFER_2, 0x4d3d)?;
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
    audio::start_voc(c, 5, 0x4d48)?;
    c.run(0x4d48, 2)?;
    c.xor(B, 0, 0);
    c.set_reg8(r8::AH, 0);
    c.call(entry::SCROLL_SCENE_31, 0x4d4d)?;
    c.run(0x4d4d, 3)?;
    tasks::add_task(c, 0x4bb9, 0x10, 0x4d56)?;
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
    c.set_byte(g::SKY_VARIANT, 1);
    let ax = c.reg(AX);
    c.push(ax);
    c.call(entry::CLEAR_SPEAKER_PLACES, 0x39f5)?;
    let r = c.player().x;
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
        let di = c.word(g::PLACE);
        c.set_reg(DI, di);
        v = (v & 0xff00) | c.byte(di) as u16;
        c.set_reg(AX, v);
        c.push(v);
    }
    if eb {
        c.call(entry::SHOW_PLACE_VIEW, 0x3a16)?;
        c.run(0x3a16, 1)?;
    } else if sky {
        if clear {
            c.set_byte(g::SKY_VARIANT, 0);
        }
        c.call(entry::DRAW_SKY, 0x3a20)?;
    }
    c.run(0x3a20, 2)?;
    let v = c.pop();
    c.set_reg(AX, v);
    c.call(entry::DRAW_SAL, 0x3a24)?;
    figures_3a24(c)
}

fn figures_3a24(c: &mut Ctx) -> Result<(), Bail> {
    let lit = c.byte(g::SKY_PALETTE_SET);
    let r = c.player().x as u8;
    if lit == 0 || r != 1 {
        c.run(0x3a24, if lit == 0 { 3 } else { 5 })?;
        c.cmp(B, lit as u16, 0);
        if lit != 0 {
            c.cmp(B, r as u16, 1);
        }
        c.call(entry::REMOVE_ROOM_FRAME_TASK, 0x3a7f)?;
        let ax = c.player().x;
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
        tasks::add_task(c, entry::ROOM_FRAME_TASK, 0x0c, 0x3a94)?;
        c.run(0x3a94, 1)?;
        return c.ret();
    }
    let k = c.byte(g::ORNI_SPEED);
    if k == 0xff {
        c.run(0x3a24, 7)?;
        c.cmp(B, lit as u16, 0);
        c.cmp(B, r as u16, 1);
        c.cmp(B, k as u16, 0xff);
        return c.ret();
    }
    let cl = c.byte(g::LOCATION_SKILL_NEED_1);
    if cl == 0 {
        c.run(0x3a24, 10)?;
        c.cmp(B, lit as u16, 0);
        c.cmp(B, r as u16, 1);
        c.cmp(B, k as u16, 0xff);
        c.set_reg(CX, 0);
        c.xor(B, 0, 0);
        return c.ret();
    }
    let f = c.conversation().line_state;
    let skip = f & 0x81 != 0;
    c.run(0x3a24, 6 + 3 + 5 + if skip { 1 } else { 2 })?;
    c.cmp(B, lit as u16, 0);
    c.cmp(B, r as u16, 1);
    c.cmp(B, k as u16, 0xff);
    c.set_reg(CX, cl as u16);
    c.xor(B, 0, 0);
    c.set_reg(AX, 0x388d);
    c.push(0x388d);
    c.set_byte(g::ORNI_SPEED, 0);
    c.test(B, f as u16, 0x81);
    if skip {
        c.pop();
        return super::ui::set_sky_palette(c.m, 0x388d);
    }
    resources::open_resource(c, 0x33, 0x3a57)?;
    c.run(0x3a57, 1)?;
    let (dx, bx) = records::orni_position(c, 0x3a5a)?;
    c.run(0x3a5a, 6)?;
    let a = c.add(W, dx, 0x0c);
    c.set_word(g::ROOM_FIGURES_X, a);
    let a = c.add(W, bx, 8);
    c.set_reg(AX, a);
    c.set_word(g::ROOM_FIGURES_Y, a);
    loop {
        c.run(0x3a6a, 4)?;
        let (cx, bx, dx) = (c.reg(CX), c.reg(BX), c.reg(DX));
        c.push(cx);
        c.push(bx);
        c.push(dx);
        c.call(entry::DRAW_ORNI, 0x3a70)?;
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
pub fn show_place_view(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x37eb)?;
    let mut c = Ctx::new(m);
    c.run(0x37eb, 1)?;
    c.call(entry::SET_ROOM_LIGHT_PALETTE, 0x37ee)?;
    c.run(0x37ee, 1)?;
    c.call(entry::PLACE_SCENE, 0x37f1)?;
    c.run(0x37f1, 1)?;
    enemy_watch(&mut c)
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
    if sound::voc_playing(&mut c, 0x1b10)? {
        c.run(0x1b10, 2)?;
        return c.ret();
    }
    let k = c.clock().stopped;
    let t = c.progress().phase;
    if k != 0 {
        c.run(0x1b10, 4)?;
        return c.ret();
    }
    if t >= 0xc8 {
        c.run(0x1b10, 6)?;
        return c.ret();
    }
    c.run(0x1b10, 6)?;
    c.call(entry::CLOCK_STEP, 0x1b23)?;
    new_time_1b23(&mut c)
}

/// `loc_01b23`: on a new time slot ([46DDh], cleared): [0F4h] counts
/// down (from 10h on; below it 0, and [0F5h] 0), the date panel and the
/// sky (`loc_01a0f`, `loc_038e1`); the hour [2] noted in [1174h], its
/// change ([46DEh]) handled (`loc_01c46`). Before the stage 7 ([`g::ENDGAME_STAGE`])
/// the day's events: `loc_01d9f`, `loc_06c6f`, `loc_063f0`, the
/// handler for `loc_01ae0` (the word table 1DB3h), `loc_01c18`,
/// `loc_0331e` with the saved [11CEh] ([11F7h] kept), `loc_01bec`;
/// unless [46D9h] the map view refreshed ([46ECh]) and at a place
/// [473Bh] acted on: positive (and [0FBh] not negative, no [46DAh]) the
/// room left and redrawn (`loc_00b21`, `loc_02db1`); negative the map
/// refreshed, or the view rebuilt (`loc_0d2bd`, the gauges). [473Bh] 0.
fn new_time_1b23(c: &mut Ctx) -> Result<(), Bail> {
    // The timer's handler sets [46DDh]: it is read and cleared each right
    // after a stretch of its instruction alone (see `ticks.rs`).
    c.run(0x1b23, 1)?;
    let f = c.clock().steps_pending;
    c.cmp(B, f as u16, 0);
    if f == 0 {
        c.run(0x1b28, 2)?;
        return c.ret();
    }
    c.run(0x1b28, 1)?;
    c.run(0x1b2a, 1)?;
    c.update_clock(|k| k.steps_pending = 0);
    let a = c.situation().desert_fatigue.wrapping_sub(1);
    let low = (a as i8) < 0x10;
    c.run(0x1b2f, 4 + if low { 2 } else { 0 } + 2)?;
    let f4 = c.situation().desert_fatigue;
    let a = c.dec(B, f4 as u16) as u8;
    c.cmp(B, a as u16, 0x10);
    let a = if low {
        c.xor(B, a as u16, a as u16);
        c.update_situation(|t| t.desert_remarks = 0);
        0
    } else {
        a
    };
    c.set_reg8(r8::AL, a);
    c.update_situation(|t| t.desert_fatigue = a);
    c.call(entry::REDRAW_DATE_PANEL, 0x1b43)?;
    c.run(0x1b43, 1)?;
    c.call(entry::SKY_IF_CHANGED, 0x1b46)?;
    let now = c.game_time();
    let old = c.word(g::LAST_TIME);
    let d = (old as u8 & 0xf0).wrapping_sub(now as u8 & 0xf0);
    c.run(0x1b46, 8 + (d != 0) as u32)?;
    c.set_word(g::LAST_TIME, now);
    c.and(B, old & 0xff, 0xf0);
    let cl = c.and(B, now & 0xff, 0xf0);
    c.set_reg(CX, (now & 0xff00) | cl);
    c.sub(B, (old & 0xf0) as u16, cl);
    c.set_reg(AX, (old & 0xff00) | d as u16);
    c.set_byte(g::DAY_CHANGED, d);
    if d != 0 {
        c.call(entry::NEW_DAY, 0x1b5e)?;
    }
    let st = c.economy().endgame_stage;
    if st >= 7 {
        c.run(0x1b5e, 2)?;
        c.cmp(B, st as u16, 7);
        return clear_473b(c);
    }
    c.run(0x1b5e, 3)?;
    c.cmp(B, st as u16, 7);
    c.call(entry::CHECK_CHARACTER_7_ATTACK, 0x1b68)?;
    c.run(0x1b68, 3)?;
    let (a, b) = (c.word(g::PHRASE_ARG_SKILLS), c.word(g::TOPIC_PLACE_RECORD));
    c.push(a);
    c.push(b);
    c.call(entry::TROOPS_TICK, 0x1b73)?;
    c.run(0x1b73, 1)?;
    c.call(entry::GROW_SPICE, 0x1b76)?;
    c.run(0x1b76, 1)?;
    c.call(entry::GET_INGAME_TIME_OF_DAY, 0x1b79)?;
    c.run(0x1b79, 3)?;
    let ax = c.reg(AX);
    let si = c.shl1(W, ax, 1);
    c.set_reg(AX, si);
    c.set_reg(SI, si);
    let to = c.code_word(si.wrapping_add(0x1db3));
    c.call(to, 0x1b82)?;
    c.run(0x1b82, 1)?;
    c.call(entry::REFRESH_MAP_OVERLAYS, 0x1b85)?;
    c.run(0x1b85, 2)?;
    let di = c.pop();
    talk_about_location(c, di, 0x1b89)?;
    c.run(0x1b89, 2)?;
    let v = c.pop();
    c.set_word(g::PHRASE_ARG_SKILLS, v);
    c.call(entry::REVISIT_HARKONNEN_PLACE, 0x1b90)?;
    let k = c.byte(g::COMM_CALLER);
    if k != 0 {
        c.run(0x1b90, 2)?;
        c.cmp(B, k as u16, 0);
        return clear_473b(c);
    }
    let e = c.map_view().changes;
    c.run(0x1b90, 4 + (e != 0) as u32)?;
    c.cmp(B, 0, 0);
    c.cmp(B, e as u16, 0);
    if e != 0 {
        c.call(entry::REFRESH_MAP_VIEW, 0x1ba1)?;
    }
    let di = c.word(g::PLACE);
    if di == 0 {
        c.run(0x1ba1, 3)?;
        c.set_reg(DI, 0);
        c.or(W, 0, 0);
        return clear_473b(c);
    }
    let v = c.byte(g::REDRAW);
    c.run(0x1ba1, 3 + if v & 0x80 != 0 { 2 } else { 3 })?;
    c.set_reg(DI, di);
    c.or(W, di, di);
    c.cmp(B, v as u16, 0);
    if v == 0 {
        return clear_473b(c);
    }
    let map_ui = c.situation().map_interface;
    if v & 0x80 != 0 {
        c.run(0x1bd2, 1)?;
        c.call(entry::CLEAR_REDRAW, 0x1bd5)?;
        let map_ui = c.situation().map_interface;
        if map_ui & 0x80 != 0 {
            c.run(0x1bd5, 3)?;
            c.cmp(B, map_ui as u16, 0);
            return super::map::refresh_map_view(c.m, 0x5d6d);
        }
        let da = c.byte(g::CLOCK_TICKING);
        if da != 0 {
            c.run(0x1bd5, 4)?;
            c.cmp(B, map_ui as u16, 0);
            c.cmp(B, da as u16, 0);
            return clear_473b(c);
        }
        c.run(0x1bd5, 5)?;
        c.cmp(B, map_ui as u16, 0);
        c.cmp(B, 0, 0);
        c.call(entry::LEAVE_SCREENS, 0x1be6)?;
        c.run(0x1be6, 1)?;
        c.run(0x0fa7, 1)?;
        c.call(entry::CALL_RESTORE_CURSOR, 0x0faa)?;
        c.run(0x0faa, 2)?;
        tails::redraw_room(c, 0x2a, 0x0faf)?;
        c.run(0x0faf, 1)?;
        return super::screens::draw_gauges(c.m, 0xd763);
    }
    if map_ui & 0x80 != 0 {
        c.run(0x1bb8, 2)?;
        c.cmp(B, map_ui as u16, 0);
        return clear_473b(c);
    }
    let da = c.byte(g::CLOCK_TICKING);
    if da != 0 {
        c.run(0x1bb8, 4)?;
        c.cmp(B, map_ui as u16, 0);
        c.cmp(B, da as u16, 0);
        return clear_473b(c);
    }
    c.run(0x1bb8, 5)?;
    c.cmp(B, map_ui as u16, 0);
    c.cmp(B, 0, 0);
    c.call(entry::CALL_RESTORE_CURSOR, 0x1bc9)?;
    c.run(0x1bc9, 1)?;
    c.call(entry::CLEAR_REDRAW, 0x1bcc)?;
    c.run(0x1bcc, 1)?;
    c.call(entry::STOP_SCENE_VOICE_AND_PARTICLES, 0x1bcf)?;
    c.run(0x1bcf, 1)?;
    room_arrive(c.m, 0x2db1)
}

/// `loc_01bb2` reached by a jump: [473Bh] 0, `ret`.
fn clear_473b(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x1bb2, 2)?;
    c.set_byte(g::REDRAW, 0);
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
    let ax = c.conversation().talker;
    c.set_reg(AX, ax);
    let di = c.word(g::PLACE);
    c.set_reg(DI, di);
    c.call(entry::DROP_EVENTS, 0x354c)?;
    c.run(0x354c, 1)?;
    c.update_situation(|t| t.desert_rescues = 0);
    scene_record_3551(c)
}

fn scene_record_3551(c: &mut Ctx) -> Result<(), Bail> {
    let n = c.conversation().scene_count;
    let st = c.progress().talk_reason;
    let count = |c: &mut Ctx| {
        let v = c.inc(B, n as u16);
        c.update_conversation(|t| t.scene_count = v as u8);
        c.cmp(B, st as u16, 3);
    };
    if st == 3 {
        c.run(0x3551, 6)?;
        count(c);
        c.set_reg(BP, 0x1f92);
        c.set_reg(BX, 0x97cf);
        c.call(entry::PUSH_SCREEN, 0x3565)?;
        c.run(0x3565, 3)?;
        c.set_word(g::TEXT_BOX_FLAGS, 0);
        c.set_word(g::HEAD_BOX_FLAGS, 0);
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
        c.call(entry::PUSH_SCREEN, 0x3586)?;
        c.run(0x3586, 3)?;
        c.set_word(g::TEXT_BOX_FLAGS, 0);
        c.set_word(g::HEAD_BOX_FLAGS, 0);
        return super::screens::panel_buttons(c.m, 0x2ffb);
    }
    let k = c.sequence().playing;
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
    let ax = c.conversation().talker;
    dialogue::start_speaking(c, ax, 0x35a9)?;
    c.run(0x35a9, 1)?;
    c.call(entry::FADE_PANEL, 0x35ac)?;
    c.run(0x35ac, 1)?;
    c.ret()
}

/// `loc_02b2a`: the clock in the room view (nothing on [46D9h], [4774h]
/// or a map mode; the list 1F0Eh current, [0FBh] not negative): AX the
/// time [0CE7Ah]. Past the hour 14h ([2Ah]) with [`g::PENDING_EVENTS`] and
/// 32h since [`g::LAST_ACTION_TIME`]: an event to tell (`loc_02ad8`, CF) starts its
/// scene (`loc_03542`, stage 0); else (not at a Harkonnen place, [2Bh])
/// the light dims (28h from 96h, 26h from 15Eh) and from 1C2h (or at
/// one) the next event is
/// shown. At 14h exactly, outside a place and with [10h] 0, every 3E8h
/// the time moves on 3B6h (`loc_01071`, `loc_0b17a`) and the clock steps
/// again until [0EAh] is FFh. An event ([`g::NEXT_EVENT`]): its kind in [0EAh]
/// ([0EBh] for 1), its phrase below 10h (0Eh needs a speaker), the
/// speaker at [`g::NEXT_EVENT_PLACE`] placed (`loc_0331e`, `loc_02e98`; from phrase 0Eh
/// with a figure `loc_06906`), the talk (`loc_02c92`, `loc_09ef1`), the
/// list shifted, and the view reset before `loc_0c412`.
pub fn clock_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2b2a)?;
    let mut c = Ctx::new(m);
    let (a, b, d) = (c.byte(g::COMM_CALLER), c.sequence().playing, c.travel().screen_kind);
    // Not in a scene, a talk or with the head held.
    c.run(0x2b2a, 5)?;
    c.set_reg8(r8::AL, a | b | d);
    if a | b | d != 0 {
        return c.ret();
    }
    let ui_list = cursor::current_ui_list(&mut c, 0x2b3a)?;
    if ui_list != 0x1f0e {
        c.run(0x2b3a, 3)?;
        return c.ret();
    }
    let map_ui = c.situation().map_interface;
    if map_ui & 0x80 != 0 {
        c.run(0x2b3a, 5)?;
        return c.ret();
    }
    // The ticks, read right after a stretch of their `mov` alone (see
    // `ticks.rs`).
    c.run(0x2b3a, 4)?;
    c.cmp(B, map_ui as u16, 0);
    c.run(0x2b47, 1)?;
    let t = c.word(super::globals::TIMER);
    c.set_reg(AX, t);
    let h = c.progress().phase;
    if h < 0x14 {
        c.run(0x2b4a, 3)?;
        return c.ret();
    }
    let last = c.word(g::LAST_ACTION_TIME);
    if h == 0x14 {
        let party = c.progress().party;
        let kind = c.progress().place_kind;
        let dt = t.wrapping_sub(last);
        let n = if party != 0 { 2 } else if kind != 0xff { 4 } else if dt < 0x3e8 { 7 } else { 9 };
        c.run(0x2b4a, 3 + n + (n < 9) as u32)?;
        c.set_reg(AX, if n >= 7 { dt } else { t });
        if n < 9 {
            return c.ret();
        }
        // The time moved on.
        c.set_word(g::LAST_ACTION_TIME, last.wrapping_add(0x3b6));
        c.call(entry::START_PHASE_15, 0x2bc1)?;
        c.run(0x2bc1, 1)?;
        c.call(entry::ON_PHASE_CHANGE, 0x2bc4)?;
        loop {
            c.run(0x2bc4, 1)?;
            c.call(entry::CLOCK_STEP, 0x2bc7)?;
            let e = c.situation().event_kind;
            c.run(0x2bc7, 2 + (e == 0xff) as u32)?;
            if e == 0xff {
                return c.ret();
            }
        }
    }
    let ev = c.byte(g::PENDING_EVENTS);
    if ev == 0 {
        c.run(0x2b4a, 6)?;
        return c.ret();
    }
    let dt = t.wrapping_sub(last);
    c.run(0x2b4a, 9)?;
    c.set_reg(AX, dt);
    if dt < 0x32 {
        return c.ret();
    }
    if event_told(&mut c, 0x2b66)? {
        c.run(0x2b66, 3)?;
        c.update_progress(|t| t.talk_reason = 0);
        return scene_record(c.m, 0x3542);
    }
    let harkonnen = c.progress().harkonnen_place;
    let ax = c.reg(AX);
    if harkonnen == 0 {
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
        c.set_reg8(r8::BL, if ax >= 0xfa { 0x26 } else { 0x28 });
        if ret_here {
            return c.ret();
        }
        if let Some(bl) = bl {
            c.run(0x2b90, 2)?;
            let bx = (c.reg(BX) & 0xff00) | bl as u16;
            c.push(bx);
            c.call(entry::CALL_RESTORE_CURSOR, 0x2b94)?;
            c.run(0x2b94, 1)?;
            c.call(entry::GFX_COPY_FRAMEBUFFER_TO_SCREEN, 0x2b97)?;
            c.run(0x2b97, 3)?;
            let v = c.pop();
            c.set_reg(AX, v);
            c.set_reg(BP, 0x0f66);
            c.call(entry::TRANSITION_AL, 0x2b9e)?;
            c.run(0x2b9e, 1)?;
            return super::mouse::draw_mouse(c.m, 0xdbec);
        }
    } else {
        c.run(0x2b66, 3)?;
    }
    next_event_2bd2(&mut c)
}

/// `loc_02bd2`: the [`g::NEXT_EVENT`] shown; see [`clock_step`].
fn next_event_2bd2(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x2bd2, 1)?;
    c.call(entry::CALL_RESTORE_CURSOR, 0x2bd5)?;
    c.run(0x2bd5, 1)?;
    c.call(entry::PICK_MUSIC, 0x2bd8)?;
    let di = c.word(g::NEXT_EVENT_PLACE);
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
            c.set_byte(g::NEXT_EVENT_PLACE, 0);
        } else {
            c.call(entry::DESCRIBE_PLACE, 0x2bf4)?;
        }
    }
    let ev = c.word(g::NEXT_EVENT);
    let (al, ah) = (ev as u8, (ev >> 8) as u8);
    let di = c.word(g::NEXT_EVENT_PLACE);
    let skip = ah >= 0x10 || (ah == 0x0e && di == 0);
    let n = 4 + (al == 1) as u32 + 5 + if ah >= 0x10 { 0 } else if ah != 0x0e { 2 } else { 4 };
    c.run(0x2bf4, n + if skip { 1 } else if di != 0 { 4 } else { 3 })?;
    c.set_reg(AX, ev);
    c.update_situation(|t| t.event_kind = al);
    c.cmp(B, al as u16, 1);
    if al == 1 {
        c.update_situation(|t| t.message_number = al);
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
        return drop_oldest_event(c.m, 0x2a34);
    }
    c.update_conversation(|t| t.talker = ax);
    c.or(W, di, di);
    if di != 0 {
        c.call(entry::DESCRIBE_PLACE, 0x2c20)?;
        c.run(0x2c20, 1)?;
        c.call(entry::NOTE_POINT, 0x2c23)?;
        let p = c.conversation().talker;
        if p < 0x0e {
            c.run(0x2c23, 2)?;
            c.cmp(W, p, 0x0e);
        } else {
            let k = c.situation().event_kind;
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
                let troop = troops::troop_record(c, 0x2c3d)?;
                c.run(0x2c3d, 2)?;
                c.update_conversation(|t| t.talker = 0x0e);
                c.set_word(g::CHARACTER_14_TROOP, troop.0);
            }
        }
    }
    c.run(0x2c47, 1)?;
    c.call(entry::TRANSITION_TO_TALK_SCREEN, 0x2c4a)?;
    c.run(0x2c4a, 2)?;
    dialogue::talk_text_voice(c, 1, 0x2c4f)?;
    c.run(0x2c4f, 1)?;
    c.call(entry::DROP_OLDEST_EVENT, 0x2c52)?;
    c.run(0x2c52, 4)?;
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.set_byte(g::ACTION_LIST, 0);
    c.set_word(g::ACTION_LIST_FIRST, 0);
    c.call(entry::REDRAW_UI_SLOTS, 0x2c5d)?;
    c.run(0x2c5d, 1)?;
    c.call(entry::STOP_CLOCK, 0x2c60)?;
    c.run(0x2c60, 2)?;
    waits::wait_or_input(c, 0x0bb8, 0x2c66)?;
    c.run(0x2c66, 1)?;
    c.call(entry::RELEASE_CLOCK, 0x2c69)?;
    c.run(0x2c69, 2)?;
    tasks::remove_task(c, 0x2cc7, 0x2c6f)?;
    c.run(0x2c6f, 1)?;
    c.call(entry::RESET_DIALOGUE, 0x2c72)?;
    c.run(0x2c72, 8)?;
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.update_text_box(|t| t.shown = 0);
    c.set_word(g::HEAD_SPRITES_DRAWN, 0);
    c.update_situation(|t| t.event_kind = 0xff);
    c.update_situation(|t| t.head_animation_step = 0x0a);
    c.set_word(g::NEXT_PHRASE, 0);
    tails::redraw_room(c, 6, 0x2c8f)?;
    c.run(0x2c8f, 1)?;
    super::gfx::copy_active_framebuffer_to_framebuffer_2(c.m, 0xc412)
}

/// `draw_gradient_section_2`: the outline of a scene polygon's edge
/// (BX, DX) from the pen ([47E8h] x, [47EAh] y) into the edge list
/// [`g::POLYGON_EDGES`] (every other word): one point per row it crosses (rows
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
    let x = c.word(g::GRADIENT_X);
    if bx == 0 {
        let (s, cf) = x.overflowing_add(dx);
        c.run(0x3e80, 4 + 5 + cf as u32 + 8)?;
        c.push(di0);
        c.push(si0);
        let y = c.word(g::GRADIENT_Y);
        c.set_reg(BX, y);
        c.set_reg(CX, dx);
        c.set_word(g::GRADIENT_X, s);
        let mut left = x;
        if cf {
            left = pure::add(W, x, dx);
        }
        c.set_reg(DX, left);
        let p = c.word(g::POLYGON_EDGES);
        c.set_reg(DI, p);
        c.set_reg(AX, left);
        c.stos(W);
        let p = c.reg(DI);
        let p = pure::add(W, p, 2);
        c.set_reg(DI, p);
        c.set_word(g::POLYGON_EDGES, p);
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
        c.set_reg(AX, 1);
        if bneg {
            let b = pure::neg(W, bx);
            c.set_reg(BX, b);
            c.set_reg(AX, 0xffff);
        }
    };
    if dx == 0 {
        let y = c.word(g::GRADIENT_Y);
        c.run(0x3e80, 4 + 2 + 2 * bneg as u32 + 2 + 5 + if bneg { 3 } else { 1 } + 3)?;
        head(&mut c);
        c.set_reg(CX, bxa);
        c.set_reg(BX, y);
        c.set_reg(DX, x);
        if bneg {
            let v = pure::sub(W, y, bxa);
            c.set_word(g::GRADIENT_Y, v);
            let b = pure::sub(W, y, bxa);
            c.set_reg(BX, b);
        } else {
            let v = pure::add(W, y, bxa);
            c.set_word(g::GRADIENT_Y, v);
        }
        let n = pure::inc(W, bxa);
        c.set_reg(CX, n);
        let p = c.word(g::POLYGON_EDGES);
        c.set_reg(DI, p);
        c.set_reg(AX, x);
        let mut cx = n;
        loop {
            c.run(0x3e73, 3)?;
            c.stos(W);
            let p = c.reg(DI);
            let p = pure::add(W, p, 2);
            c.set_reg(DI, p);
            cx = cx.wrapping_sub(1);
            c.set_reg(CX, cx);
            if cx == 0 {
                break;
            }
        }
        c.run(0x3e79, 4)?;
        let p = c.reg(DI);
        c.set_word(g::POLYGON_EDGES, p);
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
    c.set_reg(CX, 1);
    if dneg {
        c.set_reg(CX, 0xffff);
        c.set_reg(DX, dxa);
    }
    c.push(ax);
    c.push(cx);
    c.push(ax);
    c.push(cx);
    let bp = c.reg(SP);
    c.set_reg(BP, bp);
    let (mut si, mut di) = (bxa, dxa);
    c.set_reg(AX, 0);
    if steep {
        c.set_word_in(SS, bp.wrapping_add(2), 0);
    } else {
        std::mem::swap(&mut si, &mut di);
        c.set_word_in(SS, bp, 0);
    }
    c.set_reg(SI, si);
    c.set_reg(DI, di);
    c.set_reg(AX, di);
    c.set_reg(CX, di);
    let mut acc = pure::shr1(W, di, 1);
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
        acc = if below { a } else { pure::sub(W, a, di) };
        c.set_reg(AX, acc);
        c.set_reg(DX, sx);
        c.set_reg(BX, sy);
        let x = c.word(g::GRADIENT_X);
        let nx = pure::add(W, sx, x);
        c.set_reg(DX, nx);
        if row {
            c.push(di);
            c.push(acc);
            let p = c.word(g::POLYGON_EDGES);
            c.set_reg(DI, p);
            c.set_reg(AX, x);
            c.stos(W);
            let p = c.reg(DI);
            let p = pure::add(W, p, 2);
            c.set_word(g::POLYGON_EDGES, p);
            let v = c.pop();
            c.set_reg(AX, v);
            let v = c.pop();
            c.set_reg(DI, v);
        }
        c.set_word(g::GRADIENT_X, nx);
        n = n.wrapping_sub(1);
        c.set_reg(CX, n);
        if n == 0 {
            break;
        }
    }
    c.run(0x3ef8, 4)?;
    let sp = c.reg(SP);
    let sp = pure::add(W, sp, 8);
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
    // 18h bytes on the stack for the markers.
    let sp = c.reg(SP).wrapping_sub(0x18);
    c.set_reg(SP, sp);
    c.set_word(g::SAL_MARKERS_PTR, sp);
    let scene = (ax & 0xff).wrapping_sub(1);
    c.push(scene);
    c.set_reg8(r8::CL, 4);
    c.set_reg(AX, scene >> 4);
    if sheet {
        let resource = (scene >> 4) + 0x13;
        resources::open_resource(&mut c, resource, 0x3b70)?;
    }
    c.run(0x3b70, 7)?;
    let scene = c.pop();
    let slot = (scene & 0xf) << 1;
    c.set_reg(AX, slot);
    let list = c.word(0xbc6e + slot);
    c.set_reg(SI, list);
    c.call(entry::SAL_READ_POSITION_MARKERS, 0x3b80)?;
    loop {
        let si = c.reg(SI);
        let w = c.word(si);
        if w == 0xffff {
            c.run(0x3b80, 3 + 3)?;
            c.lods(W, DS);
            c.m.cpu.write8(&mut c.m.hw, CS, 0xc21a, 0);
            let sp = c.reg(SP).wrapping_add(0x18);
            c.set_reg(SP, sp);
            return c.ret();
        }
        if w & 0x8000 != 0 {
            if w >> 8 == 0xc0 {
                // A rectangle (far [3901h], which sees the `cmp ah, 0C0h`
                // flags).
                c.run(0x3b80, 4 + 2 + 14)?;
                c.lods(W, DS);
                c.cmp(B, w >> 8, 0xc0);
                c.push(w);
                let es = c.screen_buffers().active;
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
            c.call(entry::SAL_POLYGON, 0x3bc7)?;
            c.run(0x3bc7, 1)?;
            continue;
        }
        let one = w & 0x1ff == 1;
        // A sprite: index (9 bits; bit 9 the x's high bit), x, y, colour.
        c.run(0x3b80, 4 + 17 + 1)?;
        c.lods(W, DS);
        c.set_reg(DI, w);
        let h = (w >> 9) & 1;
        c.set_reg(AX, (h << 8) | (w & 0xff));
        let b1 = c.lods(B, DS);
        let dx = (h << 8) | b1;
        c.set_reg(DX, dx);
        c.lods(B, DS);
        let bx = c.reg(AX) & 0xff;
        c.set_reg(AX, bx);
        c.set_reg(BX, bx);
        let col = c.lods(B, DS) as u8;
        let si = c.reg(SI);
        c.push(si);
        c.m.cpu.write8(&mut c.m.hw, CS, 0xc21a, col);
        c.set_reg(AX, (w & 0xfdff).wrapping_sub(1));
        c.set_reg(DI, w & 0x1ff);
        if !one {
            c.call(entry::DRAW_SPRITE_CLOBBERING_BX_DX, 0x3bb2)?;
            c.run(0x3bb2, 2)?;
            let v = c.pop();
            c.set_reg(SI, v);
            continue;
        }
        let f = c.conversation().line_state;
        let skip = f & 0x81 != 0;
        let p = c.word(g::SAL_MARKERS_PTR);
        let mark = c.byte(p);
        let none = !skip && mark == 0xff;
        c.run(0x3d12, if skip { 2 } else if none { 6 } else { 8 })?;
        if !skip {
            c.set_reg(DI, p);
            c.set_word(g::SAL_MARKERS_PTR, p.wrapping_sub(1));
            if !none {
                c.set_reg8(r8::AL, mark);
                c.call(entry::DRAW_CHARACTER, 0x3d2b)?;
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
    c.set_word(g::POLYGON_EDGES, 0x4c60);
    let ds = c.sreg(DS);
    c.push(ds);
    let v = c.pop();
    c.set_sreg(ES, v);
    let si = c.reg(SI);
    let a = c.byte(si.wrapping_sub(2));
    c.set_reg8(r8::AL, a);
    c.set_byte(g::POLYGON_COLOUR, a);
    let a = c.byte(si.wrapping_sub(1));
    c.set_reg8(r8::AL, a);
    c.set_byte(g::POLYGON_STYLE, a);
    for at in [g::POLYGON_SHADE_X, g::POLYGON_SHADE_Y] {
        let b = c.lods(B, DS) as u8;
        let step = (b as i8 as i16 as u16) << 4;
        c.set_reg(AX, step);
        c.set_word(at, step);
    }
    let dx = c.lods(W, DS);
    c.set_reg(DX, dx);
    let bx = c.lods(W, DS);
    c.set_reg(BX, bx);
    c.set_word(g::POLYGON_START_X, dx);
    c.set_word(g::POLYGON_START_Y, bx);
    let mut ax;
    loop {
        c.run(0x3c25, 7)?;
        ax = c.lods(W, DS);
        c.push(ax);
        c.set_reg(AX, ax & 0x3fff);
        c.set_reg(DI, ax & 0x3fff);
        let cx = c.lods(W, DS);
        c.set_reg(CX, cx);
        c.call(entry::DRAW_GRADIENT_SECTION, 0x3c32)?;
        c.run(0x3c32, 5)?;
        let (di, cx) = (c.reg(DI), c.reg(CX));
        c.set_reg(DX, di);
        c.set_reg(BX, cx);
        ax = c.pop();
        c.set_reg(AX, ax);
        if ax & 0x4000 != 0 {
            break;
        }
    }
    c.run(0x3c3c, 7)?;
    let (di, cx) = (c.reg(DI), c.reg(CX));
    c.set_word(g::POLYGON_END_X, di);
    c.set_word(g::POLYGON_END_Y, cx);
    let dx = c.word(g::POLYGON_START_X);
    c.set_reg(DX, dx);
    let bx = c.word(g::POLYGON_START_Y);
    c.set_reg(BX, bx);
    c.set_word(g::POLYGON_EDGES, 0x4c62);
    if ax & 0x8000 == 0 {
        loop {
            c.run(0x3c57, 7)?;
            ax = c.lods(W, DS);
            c.push(ax);
            c.set_reg(AX, ax & 0x3fff);
            c.set_reg(DI, ax & 0x3fff);
            let cx = c.lods(W, DS);
            c.set_reg(CX, cx);
            c.call(entry::DRAW_GRADIENT_SECTION, 0x3c64)?;
            c.run(0x3c64, 5 + if ax & 0x8000 != 0 { 2 } else { 0 })?;
            let (di, cx) = (c.reg(DI), c.reg(CX));
            c.set_reg(DX, di);
            c.set_reg(BX, cx);
            ax = c.pop();
            c.set_reg(AX, ax);
            if ax & 0x8000 != 0 {
                c.set_reg(DX, di);
                c.set_reg(BX, cx);
                break;
            }
        }
    }
    c.run(0x3c71, 3)?;
    let di = c.word(g::POLYGON_END_X);
    c.set_reg(DI, di);
    let cx = c.word(g::POLYGON_END_Y);
    c.set_reg(CX, cx);
    c.call(entry::DRAW_GRADIENT_SECTION, 0x3c7c)?;
    let style = c.byte(g::POLYGON_STYLE);
    let dither = style & 0x3e != 0;
    c.run(0x3c7c, 11 + dither as u32 + 6)?;
    let es = c.screen_buffers().active;
    c.set_sreg(ES, es);
    let si = c.reg(SI);
    c.push(si);
    let top = c.word(g::POLYGON_START_Y);
    c.set_reg(BX, top);
    // The lines from the top: their count, the step word (style bits
    // and 2), BP 1 for a dithered style.
    let n = c.word(g::POLYGON_END_Y).wrapping_sub(top);
    c.set_reg(SI, 0x4c60);
    c.set_reg(CX, n);
    c.set_reg(BP, dither as u16);
    let step = ((style & 0x3e) as u16) << 8 | 2;
    c.set_word(g::POLYGON_LINE_STEP, step);
    let col = c.byte(g::POLYGON_COLOUR);
    c.set_reg(AX, (col as u16) << 8);
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
        c.set_reg(DX, lo);
        let x = if back { hi.wrapping_sub(1) } else { hi.wrapping_add(1) };
        // The flags of the `sub` (forward) or the `neg` (backwards) pass
        // through the driver's far call (it reads none) and are not shown
        // unread after it.
        let l = c.sub(W, x, lo);
        c.set_reg(CX, l);
        if l != 0 {
            let l = if back { c.neg(W, l) } else { l };
            c.set_reg(CX, l);
            let si = c.reg(SI);
            c.push(si);
            let bx = c.reg(BX);
            c.push(bx);
            let s = c.word(g::POLYGON_LINE_STEP);
            c.set_reg(SI, s);
            let d = c.word(g::POLYGON_SHADE_X);
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
        let ax = c.reg(AX).wrapping_add(c.word(g::POLYGON_SHADE_Y));
        c.set_reg(AX, ax);
        let bx = c.reg(BX).wrapping_add(1);
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

/// `loc_0a219` (a callback, called through a register; SI an entry): for
/// an entry without bit 7, the next game phase ([`g::GAME_PHASE`] one
/// up, [0FFh] 0, `loc_0b17a`), and at phase 1 `loc_0100b`. Flags exact
/// (its callers are not known).
pub fn next_phase_unless_done(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa219)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let f = c.byte(si);
    if f & 0x80 != 0 {
        c.run(start, 3)?; // test; jne; ret
        return c.ret();
    }
    c.run(start, 5)?; // test; jne; inc [2Ah]; mov [0FFh], 0; call
    let p = c.byte(g::GAME_PHASE);
    let p = pure::inc(B, p as u16) as u8;
    c.set_byte(g::GAME_PHASE, p);
    c.set_byte(0xff, 0);
    c.call(entry::ON_PHASE_CHANGE, 0xa22a)?;
    let p = c.byte(g::GAME_PHASE);
    c.run(0xa22a, 3)?; // cmp; jne; ret | call
    if p != 1 {
        return c.ret();
    }
    c.call(entry::ON_PHASE_01, 0xa234)?;
    c.run(0xa234, 1)?;
    c.ret()
}

/// `loc_0a235` (a callback, called through a register; SI an entry): for
/// an entry without bit 7, the game phase set to the next multiple of 4
/// (`set_game_phase` with AL ([2Ah] and FCh) + 4). Flags exact (its
/// callers are not known).
pub fn next_phase_group_unless_done(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa235)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let f = c.byte(si);
    if f & 0x80 != 0 {
        c.run(start, 3)?; // test; jne A234h; ret
        return c.ret();
    }
    let p = c.byte(g::GAME_PHASE);
    c.run(start, 6)?; // test; jne; mov al; and; add; jmp
    let al = pure::and(B, p as u16, 0xfc);
    let al = pure::add(B, al, 4) as u8;
    c.set_reg8(r8::AL, al);
    c.jump(code::SET_GAME_PHASE)
}

/// `loc_0316e` (the callback `loc_03127` has `loc_06603` run per troop:
/// SI the troop, DI its location, DX/BX the player's place, DL a kind):
/// AH the troop's kind (2; 3 when its state has 20h and the location is
/// a kind 28h on; 1 when not and [`g::HARKONNEN_PLACE`]), unless it is DL
/// or (without 20h) byte 10h has bit 7... (see below) nothing more.
/// Otherwise the troop's marker: with byte 10h bit 7, character 12 (DI
/// 4768h; its flags' bit 4 from the state's, the leader's motivation FFh
/// or byte 15h, asked 0); else for a state 80h on character 14 (DI
/// 4756h), else character 15 and the next of the marker slots 4758h
/// ([476Ah] modulo 8, then one up; [476Bh] for troop 8E0h). The slot
/// gets SI, the character DX/BX. Flags dropped (its caller reads none).
pub fn place_troop_marker(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use super::r8::DL;
    entered(start, 0x316e)?;
    let mut c = Ctx::new(m);
    let (si, di, dx, bx) = (c.reg(SI), c.reg(DI), c.reg(DX), c.reg(BX));
    let state = c.byte(si.wrapping_add(3));
    let leader = c.byte(si.wrapping_add(0x10)) & 0x80 != 0;
    let mut n = 4; // mov al; mov ah, 2; test; je
    let mut ah: u8 = 2;
    if state & 0x20 != 0 {
        let kind = c.byte(di.wrapping_add(8));
        n += 2; // cmp; jb
        if kind >= 0x28 {
            ah = 3;
            n += 2; // inc ah; jmp
        }
    } else {
        n += 2; // test; jne
        if leader {
            c.run(start, n + 1)?; // ret
            c.set_reg(AX, (ah as u16) << 8 | state as u16);
            return c.ret();
        }
        let h = c.byte(g::HARKONNEN_PLACE);
        n += 2; // cmp; je
        if h != 0 {
            ah = 1;
            n += 1; // dec ah
        }
    }
    n += 2; // cmp ah, dl; jne
    if ah != c.reg8(DL) {
        c.run(start, n + 1)?; // ret
        c.set_reg(AX, (ah as u16) << 8 | state as u16);
        return c.ret();
    }
    n += 2; // test; jne
    // (character, slot, AX, the writes of the marker slots / the leader)
    let (who, slot, ax);
    let mut slots: Option<(u8, bool)> = None;
    let mut asked: Option<(u8, u8)> = None;
    if !leader {
        n += 4; // mov bp; mov di; cmp al, 80h; jnb
        if state >= 0x80 {
            (who, slot, ax) = (14, 0x4756u16, (ah as u16) << 8 | state as u16);
        } else {
            let v = c.byte(g::CHARACTER_15_TROOP_COUNT) & 7;
            n += 8; // mov bp; mov di; and; mov al; inc; xor ah; cmp si; jne
            let first = si == 0x8e0;
            if first {
                n += 3; // inc ax; mov [476Bh], al; dec ax
            }
            n += 3; // shl; add; jmp
            slots = Some((v, first));
            (who, slot, ax) = (15, g::CHARACTER_15_TROOPS.wrapping_add((v as u16) << 1), (v as u16) << 1);
        }
    } else {
        n += 10; // mov bp; mov di; mov al; and; and; or; mov [0EEh]; or; mov al; jne
        let bit = state & 0x10;
        let al = if bit != 0 {
            0xff
        } else {
            n += 1; // mov al, [si+15h]
            c.byte(si.wrapping_add(0x15))
        };
        n += 1; // mov [0EDh], al
        asked = Some((bit, al));
        (who, slot, ax) = (12, 0x4768u16, (ah as u16) << 8 | al as u16);
    }
    n += 4; // mov [di], si; mov [bp], dx; mov [bp+2], bx; ret
    c.run(start, n)?;
    if let Some((v, first)) = slots {
        c.set_byte(g::CHARACTER_15_TROOP_COUNT, v + 1);
        if first {
            c.set_byte(g::CHARACTER_15_SPECIAL_SLOT, v + 1);
        }
    }
    if let Some((bit, al)) = asked {
        c.update_character(12, |t| t.flags = (t.flags & 0xef) | bit);
        c.update_situation(|t| {
            t.leader_asked = 0;
            t.leader_motivation = al;
        });
    }
    c.set_word(slot, si);
    c.update_character(who, |t| (t.x, t.y) = (dx, bx));
    c.set_reg(AX, ax);
    c.set_reg(BP, super::characters::record(who as u16));
    c.set_reg(DI, slot);
    c.ret()
}
