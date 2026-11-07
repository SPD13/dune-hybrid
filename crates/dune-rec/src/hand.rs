//! Hand-written ports: readable Rust for routines of DNCDPRG.EXE, written
//! from the listing (`DNCDPRG.lst`) and checked against the original by
//! shadow mode exactly like the generated code (`dune-run --rec shadow`).
//! Where one exists it replaces the generated routine of the same entry.
//!
//! A port must leave the machine as the original does: the registers and
//! flags it returns (callers may rely on any of them, so the exit state is
//! spelled out), the memory it writes, and its instruction count, which is
//! the game's virtual time ([`Ctx::run`]). The meaning goes in names and
//! comments; the bookkeeping stays in [`Ctx`].

use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CS, DS, SP, SS, Width},
    rec::{Bail, Routine},
};

/// Data-segment offsets of the globals used here.
mod ds {
    /// Game time: the low nibble is the time of day (16 slots), bits 4..6
    /// select the sky's palette group.
    pub const GAME_TIME: u16 = 0x0002;
    /// Game phase (story progress), raised by `set_game_phase`.
    pub const GAME_PHASE: u16 = 0x002a;
    /// Cleared when the game phase changes.
    pub const PHASE_CHANGED_FLAG: u16 = 0x00ff;
    /// Sky palette per time-of-day slot (16 bytes).
    pub const SKY_PALETTE_BY_SLOT: u16 = 0x2280;
    /// Sine/cosine table: 4-byte entries (value, scale), indexed by angle.
    pub const TRIG_TABLE: u16 = 0x4948;
}

/// Code offsets the ports call or return to.
mod code {
    /// Called when the phase changes, before its handler.
    pub const ON_PHASE_CHANGE: u16 = 0xb17a;
    /// Phase handlers: a word table in the code segment, one per 4 phases.
    pub const PHASE_HANDLERS: u16 = 0x11e7;
    /// The last phase with a handler.
    pub const LAST_HANDLED_PHASE: u8 = 0x6c;
}

/// The ports, by entry offset.
pub static ROUTINES: &[(u16, Routine, &str)] = &[
    (0x121f, set_game_phase, "set_game_phase"),
    (0x395c, sky_palette_for_game_time, "get_sky_palette_id_from_game_time_in_bl"),
    (0xb5a0, trig_lookup, "tablat_lookup_from_bx_to_ax_bp"),
];

/// Access to the machine for ports: data-segment globals, registers, calls,
/// and the instruction count.
struct Ctx<'a> {
    m: &'a mut Machine,
}

impl Ctx<'_> {
    /// Start a straight-line stretch of `insns` original instructions from
    /// offset `at`. Bails there, before doing anything, if an interrupt (or
    /// the end of a shadow check) falls inside it, so the interpreter takes
    /// over at the exact instruction; else counts them.
    fn run(&mut self, at: u16, insns: u32) -> Result<(), Bail> {
        if self.m.rec_tick(insns) {
            self.m.cpu.ip = at;
            return Err(Bail);
        }
        self.m.cpu.instructions += insns as u64;
        Ok(())
    }

    fn byte(&mut self, off: u16) -> u8 {
        self.m.cpu.read8(&mut self.m.hw, DS, off)
    }
    fn word(&mut self, off: u16) -> u16 {
        self.m.cpu.read16(&mut self.m.hw, DS, off)
    }
    fn set_byte(&mut self, off: u16, v: u8) {
        self.m.cpu.write8(&mut self.m.hw, DS, off, v);
    }
    fn code_word(&mut self, off: u16) -> u16 {
        self.m.cpu.read16(&mut self.m.hw, CS, off)
    }

    /// `CALL target` from the instruction ending at `ret` (the original's
    /// return address goes on the stack: the callee may look at it).
    fn call(&mut self, target: u16, ret: u16) -> Result<(), Bail> {
        self.m.cpu.push(&mut self.m.hw, ret);
        self.m.rec_call(target, ret)
    }

    /// `RET`.
    fn ret(&mut self) -> Result<(), Bail> {
        self.m.cpu.ip = self.m.cpu.pop(&mut self.m.hw);
        Ok(())
    }
}

/// `set_game_phase` (121F): raise the game phase to AL. Phases only go up:
/// a lower or equal AL changes nothing. On a change the phase-change hook
/// runs, then the new phase's handler if it has one (phases up to 6Ch, one
/// handler per 4 phases).
///
/// Exit: AL as given; BX = the handler index if one ran. Flags: from the
/// comparison that ended it, or as the handler left them.
fn set_game_phase(m: &mut Machine, start: u16) -> Result<(), Bail> {
    if start != 0x121f {
        return Err(Bail);
    }
    let mut g = Ctx { m };
    let new = g.m.cpu.reg8(0);
    let current = g.byte(ds::GAME_PHASE);
    g.run(0x121f, 2)?; // cmp al, [2Ah]; jbe
    g.m.cpu.sub(Width::B, new as u32, current as u32, false);
    if new <= current {
        g.run(0x1242, 1)?;
        return g.ret();
    }
    g.run(0x1225, 3)?; // mov [2Ah], al; mov byte [0FFh], 0; call
    g.set_byte(ds::GAME_PHASE, new);
    g.set_byte(ds::PHASE_CHANGED_FLAG, 0);
    g.call(code::ON_PHASE_CHANGE, 0x1230)?;

    // The hook may have moved the phase on: read it again.
    let phase = g.byte(ds::GAME_PHASE);
    g.run(0x1230, 3)?; // mov bl, [2Ah]; cmp bl, 6Ch; ja
    g.m.cpu.set_reg8(3, phase);
    g.m.cpu.sub(Width::B, phase as u32, code::LAST_HANDLED_PHASE as u32, false);
    if phase > code::LAST_HANDLED_PHASE {
        g.run(0x1242, 1)?;
        return g.ret();
    }
    g.run(0x1239, 3)?; // xor bh, bh; shr bx, 1; call cs:[bx+11E7h]
    let index = (phase >> 1) as u16;
    g.m.cpu.regs[BX] = phase as u16;
    g.m.cpu.logic(Width::B, 0); // xor bh, bh
    g.m.cpu.regs[BX] = g.m.cpu.shift(5, Width::W, phase as u32, 1) as u16;
    let handler = g.code_word(code::PHASE_HANDLERS.wrapping_add(index));
    g.call(handler, 0x1242)?;
    g.run(0x1242, 1)?;
    g.ret()
}

/// `get_sky_palette_id_from_game_time_in_bl` (395C): the sky palette for
/// the current game time: the time-of-day slot's base palette plus 4 × the
/// palette group.
///
/// Exit: BL = AL = the palette id, AH = 4 × group, BH = 22h (the table's
/// page). Flags from the final addition.
fn sky_palette_for_game_time(m: &mut Machine, start: u16) -> Result<(), Bail> {
    if start != 0x395c {
        return Err(Bail);
    }
    let mut g = Ctx { m };
    g.run(0x395c, 10)?;
    let time = g.word(ds::GAME_TIME);
    let slot = time as u8 & 0x0f;
    let group4 = (time as u8 >> 2) & 0x1c;
    let base = g.byte(ds::SKY_PALETTE_BY_SLOT + slot as u16);
    let id = g.m.cpu.add(Width::B, base as u32, group4 as u32, false) as u8;
    g.m.cpu.regs[AX] = (group4 as u16) << 8 | id as u16;
    g.m.cpu.regs[BX] = ds::SKY_PALETTE_BY_SLOT & 0xff00 | id as u16;
    g.ret()
}

/// `tablat_lookup_from_bx_to_ax_bp` (B5A0): sine/cosine lookup. BX is a
/// signed angle index; a negative one reads the mirrored entry and negates
/// the value.
///
/// Exit: AX = value, BP = 2 × scale, BX unchanged. Flags from `shl bp, 1`.
fn trig_lookup(m: &mut Machine, start: u16) -> Result<(), Bail> {
    if start != 0xb5a0 {
        return Err(Bail);
    }
    let mut g = Ctx { m };
    let bx = g.m.cpu.regs[BX];
    let offset = bx.wrapping_mul(8);
    let negative = offset & 0x8000 != 0;
    g.run(0xb5a0, if negative { 12 } else { 10 })?;
    // push bx: the stack slot is written (and left below SP) as in the original.
    let sp = g.m.cpu.regs[SP].wrapping_sub(2);
    g.m.cpu.write16(&mut g.m.hw, SS, sp, bx);
    // The flags go through the original's operations (a shift leaves some
    // of them as the previous operation set them).
    let mut x = bx as u32;
    for _ in 0..3 {
        x = g.m.cpu.shift(4, Width::W, x, 1); // shl bx, 1
    }
    let entry = if negative {
        let e = g.m.cpu.sub(Width::W, 0, offset as u32, false) as u16; // neg bx
        g.m.cpu.set_flag(CF, offset != 0);
        e
    } else {
        offset
    };
    let value = g.word(ds::TRIG_TABLE.wrapping_add(entry));
    let scale = g.word(ds::TRIG_TABLE.wrapping_add(entry).wrapping_add(2));
    g.m.cpu.regs[AX] = if negative {
        let v = g.m.cpu.sub(Width::W, 0, value as u32, false) as u16; // neg ax
        g.m.cpu.set_flag(CF, value != 0);
        v
    } else {
        value
    };
    g.m.cpu.regs[BP] = g.m.cpu.shift(4, Width::W, scale as u32, 1) as u16;
    g.ret()
}
