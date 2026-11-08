//! Game-phase handlers (called by `set_game_phase` through its table) and
//! the discovery of locations they trigger.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, CS, DI, SI},
    rec::Bail,
};

mod code {
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
    pub const SET_GAME_PHASE: u16 = 0x121f;
    pub const INCREASE_CHARISMA_BY_AL: u16 = 0x6f78;
    pub const SUB_1771: u16 = 0x1771;
}

/// Location record fields.
mod location {
    /// Bit 80h: not discovered yet.
    pub const STATE: u16 = 0x0a;
    pub const B: u16 = 0x0b;
    /// Below 20h: a sietch.
    pub const KIND: u16 = 0x08;
    pub const UNDISCOVERED: u8 = 0x80;
    /// The location whose discovery raises the phase to 10h.
    pub const PHASE_10_LOCATION: u16 = 0x0603;
}

/// `sub_425b`: discovers the location at DI: clears its undiscovered bit
/// and byte +0Bh; a sietch counts as found, and one of them (id 0603h)
/// raises the game phase to 10h. Flags from the last test or compare.
fn discover(c: &mut Ctx) -> Result<(), Bail> {
    let di = c.reg(DI);
    let state = c.byte(di.wrapping_add(location::STATE));
    if state & location::UNDISCOVERED == 0 {
        c.run(0x425b, 3)?; // test; je; ret
        c.test(B, state as u16, location::UNDISCOVERED as u16);
        return c.ret();
    }
    c.check(0x425b, 10)?;
    c.test(B, state as u16, location::UNDISCOVERED as u16);
    let state = c.and(B, state as u16, 0x7f) as u8;
    c.set_byte(di.wrapping_add(location::STATE), state);
    c.set_byte(di.wrapping_add(location::B), 0);
    let kind = c.byte(di.wrapping_add(location::KIND));
    c.cmp(B, kind as u16, 0x20);
    if kind >= 0x20 {
        c.count(7);
        return c.ret();
    }
    let found = c.byte(g::SIETCHES_FOUND);
    let found = c.inc(B, found as u16) as u8;
    c.set_byte(g::SIETCHES_FOUND, found);
    let id = c.word(di);
    c.cmp(W, id, location::PHASE_10_LOCATION);
    if id != location::PHASE_10_LOCATION {
        c.count(10);
        return c.ret();
    }
    c.count(10);
    c.call(code::SAVE_REGS, 0x427c)?;
    c.run(0x427c, 2)?;
    c.set_reg8(r8::AL, 0x10);
    c.call(code::SET_GAME_PHASE, 0x4281)?;
    c.run(0x4281, 1)?;
    c.call(code::RESTORE_REGS, 0x4284)?;
    c.run(0x4284, 1)?;
    c.ret()
}

/// `sub_10b8`: discovers the location at 800h.
pub fn sub_10b8(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x10b8)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?; // mov di; jmp 425Bh
    c.set_reg(DI, 0x0800);
    discover(&mut c)
}

/// `sub_1133`: discovers the location at 3D8h.
pub fn sub_1133(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1133)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(DI, 0x03d8);
    discover(&mut c)
}

/// From 105B: discovers the locations listed at CS:SI, up to a 0 word.
/// Exit: AX = 0, DI the last one; flags from `or ax, ax`.
fn discover_list(c: &mut Ctx) -> Result<(), Bail> {
    loop {
        let si = c.reg(SI);
        let next = c.code_word(si);
        if next == 0 {
            c.run(0x105b, 4)?; // lodsw cs:; or; je; ret
            c.lods(W, CS);
            c.or(W, 0, 0);
            return c.ret();
        }
        c.run(0x105b, 5)?;
        c.lods(W, CS);
        c.or(W, next, next);
        c.set_reg(DI, next);
        c.call(0x425b, 0x1066)?;
        c.run(0x1066, 1)?; // jmp 105Bh
    }
}

/// `sub_1053`: [0FD8h] = 0Ah, then the locations at CS:1069.
pub fn sub_1053(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1053)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(g::BYTE_0FD8, 0x0a);
    c.set_reg(SI, 0x1069);
    discover_list(&mut c)
}

/// `sub_10be`: a phase's set-up: game time noted, several story words
/// set, charisma + 14h, story bit 10h, then the locations at CS:10F7.
pub fn sub_10be(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x10be)?;
    let mut c = Ctx::new(m);
    c.run(start, 10)?;
    let time = c.word(g::GAME_TIME);
    c.set_reg(AX, time);
    c.set_word(g::WORD_1154, time);
    c.set_word(g::WORD_1018, 0x2006);
    c.set_word(g::WORD_0FF8, 0x2008);
    c.set_word(g::WORD_0FFA, 0x0180);
    c.set_byte(g::BYTE_0FE8, 0x0a);
    c.set_word(g::WORD_0FEA, 0x0180);
    c.set_word(g::WORD_1201, 0x0109);
    c.set_reg8(r8::AL, 0x14);
    c.call(code::INCREASE_CHARISMA_BY_AL, 0x10ec)?;
    c.run(0x10ec, 3)?;
    let bits = c.byte(g::STORY_BITS_000A);
    let bits = c.or(B, bits as u16, 0x10) as u8;
    c.set_byte(g::STORY_BITS_000A, bits);
    c.set_reg(SI, 0x10f7);
    discover_list(&mut c)
}

/// `sub_1139`: charisma + 0Ah, `sub_1771` with 1313h, [1057h] bit 10h on
/// and bit 2 off, [1178h] = [28h] + 2, then the locations at CS:115C.
pub fn sub_1139(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1139)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg8(r8::AL, 0x0a);
    c.call(code::INCREASE_CHARISMA_BY_AL, 0x113e)?;
    c.run(0x113e, 2)?;
    c.set_reg(AX, 0x1313);
    c.call(code::SUB_1771, 0x1144)?;
    c.run(0x1144, 9)?;
    let v = c.byte(g::BYTE_1057);
    let v = c.or(B, v as u16, 0x10);
    let v = c.and(B, v, 0xfd) as u8;
    c.set_byte(g::BYTE_1057, v);
    let b = c.byte(g::BYTE_0028);
    let b = c.add(B, b as u16, 2) as u8;
    c.set_reg8(r8::AL, b);
    c.set_byte(g::BYTE_1178, b);
    c.set_reg(SI, 0x115c);
    discover_list(&mut c)
}

/// `sub_1196`: story bit 20h, `sub_1771` with 12FBh, then the locations
/// at CS:11A7.
pub fn sub_1196(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1196)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let bits = c.byte(g::STORY_BITS_000A);
    let bits = c.or(B, bits as u16, 0x20) as u8;
    c.set_byte(g::STORY_BITS_000A, bits);
    c.set_reg(AX, 0x12fb);
    c.call(code::SUB_1771, 0x11a1)?;
    c.run(0x11a1, 2)?;
    c.set_reg(SI, 0x11a7);
    discover_list(&mut c)
}

/// `sub_425b` as a routine.
pub fn sub_425b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x425b)?;
    discover(&mut Ctx::new(m))
}
