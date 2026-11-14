//! Saved games (their RLE, restoring the map's bits and the game's
//! state), particles, numbers in strings, reading resources.

use super::pure;
use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CS, CX, DI, DS, DX, ES, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const OPEN_ONMAP_RESOURCE: u16 = 0xc13b;
    pub const PARTICLES_UPDATE_DIRTY_RECT: u16 = 0xc6ad;
    pub const CENTER_SPRITE_COORDINATES: u16 = 0xc202;
    pub const GET_SUBRESOURCE_POINTER: u16 = 0xc1f4;
    pub const GET_PHRASE_OR_COMMAND_STRING_SI: u16 = 0xcf70;
    pub const FIND_LAST_NUMERIC_DIGIT: u16 = 0xd03c;
    pub const OPEN_RES_OR_FILE_OR_DIE: u16 = 0xf229;
    pub const READ_DUNE_DAT_CX_TO_ESDI: u16 = 0xf2ea;
    pub const FONT_SET_DRAW_POSITION: u16 = 0xd04e;
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
}

/// Particle records: 11h bytes each, after the count at [`g::PARTICLES`].
const PARTICLE_SIZE: u8 = 0x11;
/// Two pointers into the particle list, kept right when one is removed.
const PARTICLE_POINTERS: u16 = 0x4752;

/// `decompress_sav` (B4BB): the RLE of saved games, DS:SI to ES:DI: a
/// header (the escape byte, the packed size with the header); then bytes,
/// or escape, count, byte for a run. Exit: CX = the bytes written, DI
/// back at the start.
pub fn decompress_sav(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb4bb)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let esc = c.lods(W, DS) as u8;
    c.set_reg8(r8::BL, esc);
    let size = c.lods(W, DS);
    c.set_reg(CX, size);
    let left = c.sub(W, size, 4);
    c.set_reg(CX, left);
    let di = c.reg(DI);
    c.push(di);
    loop {
        let si = c.reg(SI);
        let b = c.byte_in(DS, si);
        let cx = c.reg(CX);
        if b != esc {
            let left = cx.wrapping_sub(1);
            c.run(0xb4c5, 5 + if left == 0 { 4 } else { 0 })?;
            c.lods(B, DS);
            c.cmp(B, b as u16, esc as u16);
            c.stos(B);
            c.set_reg(CX, left);
            if left == 0 {
                return finish(&mut c);
            }
            continue;
        }
        c.run(0xb4c5, 8)?;
        c.lods(B, DS);
        c.cmp(B, b as u16, esc as u16);
        let count = c.lods(B, DS);
        c.set_reg(DX, cx);
        c.set_reg8(r8::CL, count as u8);
        c.xor(B, 0, 0);
        c.set_reg8(r8::CH, 0);
        c.lods(B, DS);
        c.run(0xb4db, 1)?;
        c.rep(0xaa); // rep stosb
        // The token's 3 bytes: 2 here, 1 by the loop.
        let rest = cx.wrapping_sub(2);
        let done = rest == 0 || cx < 2;
        let looped_out = !done && rest == 1;
        let n = if rest == 0 {
            3
        } else if cx < 2 {
            4
        } else if looped_out {
            6
        } else {
            5
        };
        c.run(0xb4dd, n + if done || looped_out { 4 } else { 0 })?;
        c.set_reg(CX, cx);
        let rest = c.sub(W, cx, 2);
        c.set_reg(CX, rest);
        if done {
            return finish(&mut c);
        }
        let rest = rest.wrapping_sub(1);
        c.set_reg(CX, rest);
        if rest == 0 {
            return finish(&mut c);
        }
    }
}

/// B4CD: `pop cx; sub di, cx; xchg di, cx; ret`, counted by the caller.
fn finish(c: &mut Ctx) -> Result<(), Bail> {
    let start = c.pop();
    let di = c.reg(DI);
    let n = c.sub(W, di, start);
    c.set_reg(CX, n);
    c.set_reg(DI, start);
    c.ret()
}

/// `restore_from_save_memory` (B473): from the save at DS:SI: the map's
/// bits 4..5 (2 bits per cell, 4 cells a byte, into [DD00h]:0), then the
/// counters kept in the code segment (A2h bytes at CS:00AA), then the
/// game's state (11F8h bytes at AA76h, 1261h at 0).
pub fn restore_from_save_memory(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb473)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let map = c.word_in(SS, g::MAP_GFX_DATA + 2);
    c.set_sreg(ES, map);
    let di = c.reg(DI);
    let zero = c.xor(W, di, di);
    c.set_reg(DI, zero);
    let mut bytes = c.shr1(W, 0xc5fc, 2);
    c.set_reg(BX, bytes);
    loop {
        c.run(0xb481, 5)?;
        let packed = c.lods(B, DS) as u8;
        c.set_reg(CX, 4);
        let mut ah = packed as u16;
        ah = c.ror1(B, ah);
        ah = c.ror1(B, ah);
        c.set_reg8(r8::AH, ah as u8);
        for left in (0..4u16).rev() {
            let last = left == 0;
            c.run(0xb48b, 8 + if last { 2 } else { 0 })?;
            let di = c.reg(DI);
            let cell = c.byte_in(ES, di) as u16;
            let v = c.xor(B, cell, ah);
            let v = c.and(B, v, 0xcf);
            let v = c.xor(B, v, ah);
            c.set_reg8(r8::AL, v as u8);
            c.stos(B);
            ah = c.rol1(B, ah);
            ah = c.rol1(B, ah);
            c.set_reg8(r8::AH, ah as u8);
            c.set_reg(CX, left);
        }
        bytes = c.dec(W, bytes);
        c.set_reg(BX, bytes);
        if bytes == 0 {
            break;
        }
    }
    let copies = [(0xb49e, CS, 0x00aa, 0xa2, 4), (0xb4a8, SS, 0xaa76, 0x11f8, 4), (0xb4b2, SS, 0x0000, 0x1261, 2)];
    for (at, seg, to, n, insns) in copies {
        c.run(at, insns)?;
        if insns == 4 {
            let s = c.sreg(seg);
            c.push(s);
            let es = c.pop();
            c.set_sreg(ES, es);
        }
        c.set_reg(DI, to);
        c.set_reg(CX, n);
        c.run(if insns == 4 { at + 8 } else { at + 6 }, 1)?; // the rep
        c.rep(0xa4); // rep movsb
    }
    c.run(0xb4ba, 1)?;
    c.ret()
}

/// `particles_remove_particle` (C58A): removes the particle at DI (if it
/// is one): marked (bit 80h of +0Ch), its rectangle redrawn, the ones
/// after it moved down, the count and the two list pointers after it
/// adjusted.
pub fn particles_remove_particle(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc58a)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::OPEN_ONMAP_RESOURCE, 0xc58d)?;
    let count = c.word(g::PARTICLES);
    let end = (g::PARTICLES + 2).wrapping_add(((count as u8) as u16) * PARTICLE_SIZE as u16);
    let di = c.reg(DI);
    let n = if count == 0 { 5 } else if di >= end { 10 } else { 14 };
    c.run(0xc58d, n)?;
    c.set_reg(SI, g::PARTICLES);
    c.lods(W, DS);
    c.or(W, count, count);
    if count == 0 {
        return c.ret();
    }
    c.set_reg8(r8::AH, PARTICLE_SIZE);
    c.mul8(PARTICLE_SIZE);
    let si = c.reg(SI);
    let ax = c.reg(AX);
    let end = c.add(W, si, ax);
    c.set_reg(SI, end);
    c.cmp(W, di, end);
    if di >= end {
        return c.ret();
    }
    let f = c.byte(di + 0x0c);
    let f = c.or(B, f as u16, 0x80) as u8;
    c.set_byte(di + 0x0c, f);
    c.push(end);
    c.push(di);
    c.set_reg(SI, di);
    c.call(code::PARTICLES_UPDATE_DIRTY_RECT, 0xc5aa)?;
    let sp = c.reg(SP);
    let (di, end) = (c.word_in(SS, sp), c.word_in(SS, sp + 2));
    let after = end.wrapping_sub(di.wrapping_add(PARTICLE_SIZE as u16));
    c.run(0xc5aa, 6 + if after == 0 { 0 } else { 2 })?;
    let di = c.pop();
    c.set_reg(DI, di);
    let end = c.pop();
    c.push(di);
    let from = di.wrapping_add(PARTICLE_SIZE as u16);
    c.set_reg(SI, from);
    let n = c.sub(W, end, from);
    c.set_reg(CX, n);
    if n != 0 {
        let ds = c.sreg(DS);
        c.push(ds);
        let es = c.pop();
        c.set_sreg(ES, es);
        c.run(0xc5b6, 1)?;
        c.rep(0xa4); // rep movsb
    }
    c.run(0xc5b8, 4)?;
    let count = c.word(g::PARTICLES);
    let count = c.dec(W, count);
    c.set_word(g::PARTICLES, count);
    let di = c.pop();
    c.set_reg(DI, di);
    c.set_reg(SI, PARTICLE_POINTERS);
    c.set_reg(CX, 2);
    for left in [1u16, 0] {
        let si = c.reg(SI);
        let p = c.word(si);
        let below = p < di;
        c.run(0xc5c3, 3 + if below { 0 } else { 1 } + 1 + if left == 0 { 1 } else { 0 })?;
        c.lods(W, DS);
        c.cmp(W, p, di);
        if !below {
            let v = c.sub(W, p, PARTICLE_SIZE as u16);
            c.set_word(si, v);
        }
        c.set_reg(CX, left);
    }
    c.ret()
}

/// `particles_spawn_particle` (C60B): a new particle: sprite AX of the
/// on-map sheet, centred at (DX, BX), with its rectangle, SI and flags 0.
/// SI kept.
pub fn particles_spawn_particle(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc60b)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let ax = c.reg(AX);
    c.push(ax);
    c.call(code::OPEN_ONMAP_RESOURCE, 0xc60f)?;
    c.run(0xc60f, 2)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.call(code::CENTER_SPRITE_COORDINATES, 0xc613)?;
    spawn_at(&mut c)

}

/// From C613: a particle for sprite AX at (DX, BX), resource SI, appended
/// to the list; its rect from the sprite's size.
fn spawn_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xc613, 15)?;
    let si = c.reg(SI);
    c.push(si);
    c.set_reg(DI, g::PARTICLES);
    let sprite = c.reg(AX);
    c.set_reg(BP, sprite);
    let count = c.word(g::PARTICLES);
    let n = c.inc(W, count);
    c.set_word(g::PARTICLES, n);
    c.set_reg(AX, (PARTICLE_SIZE as u16) << 8 | count & 0xff);
    c.mul8(PARTICLE_SIZE);
    let off = c.reg(AX);
    c.set_reg(AX, sprite);
    c.set_reg(BP, off);
    let di = off.wrapping_add(g::PARTICLES).wrapping_add(2);
    c.set_reg(DI, di);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.set_word(di, dx);
    c.set_word(di + 2, bx);
    c.set_word(di + 8, sprite);
    c.set_word(di + 0x0a, si);
    c.set_byte(di + 0x0c, 0);
    c.call(code::GET_SUBRESOURCE_POINTER, 0xc637)?;
    c.run(0xc637, 9)?;
    let w = c.lods(W, ES);
    let w = c.and(B, w >> 8, 0x0f) << 8 | w & 0xff;
    c.set_reg(AX, w);
    let dx = c.reg(DX);
    let dx = c.add(W, dx, w);
    c.set_reg(DX, dx);
    let si = c.reg(SI);
    let h = c.byte_in(ES, si);
    let bx = c.reg(BX);
    let bl = c.add(B, bx & 0xff, h as u16);
    let bh = c.adc(B, bx >> 8, 0);
    let bx = bh << 8 | bl;
    c.set_reg(BX, bx);
    c.set_word(di + 4, dx);
    c.set_word(di + 6, bx);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_0c613`: [`spawn_at`] (`particles_spawn_particle` without opening
/// the resource and centring).
pub fn particles_add_particle(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc613)?;
    spawn_at(&mut Ctx::new(m))
}

/// From E2E3: writes AX (0..999, more is 999) as 3 digits ending at ES:SI,
/// leading zeros as spaces. BX, CX kept.
fn number(c: &mut Ctx) -> Result<(), Bail> {
    let ax = c.reg(AX);
    let big = ax >= 0x3e8;
    let v = if big { 0x3e7 } else { ax };
    let (h, r) = (v / 100, v % 100);
    let (tens, _) = (r / 10, r % 10);
    let n = 6 + big as u32 + 4 + if h == 0 { 2 } else { 0 } + 7 + if h == 0 { 2 + (tens == 0) as u32 } else { 0 } + 4;
    c.run(0xe2e3, n)?;
    let (bx, cx) = (c.reg(BX), c.reg(CX));
    c.push(bx);
    c.push(cx);
    c.set_reg(CX, 0x64);
    c.set_reg(BX, 0x64);
    c.cmp(W, ax, 0x3e8);
    c.set_reg(AX, v);
    c.m.cpu.mul_div(&mut c.m.hw, B, 6, 0x64); // div cl
    let al = c.reg8(r8::AL);
    let mut digit = c.add(B, al as u16, 0x30) as u8;
    c.cmp(B, digit as u16, 0x30);
    if h == 0 {
        digit = b' ';
        let zero = c.xor(W, 0x64, 0x64);
        c.set_reg(BX, zero);
    }
    c.set_reg8(r8::AL, digit);
    let si = c.reg(SI);
    c.m.cpu.write8(&mut c.m.hw, ES, si.wrapping_sub(3), digit);
    let ah = c.reg8(r8::AH);
    c.set_reg8(r8::AL, ah);
    c.aam(10);
    let ax = c.reg(AX);
    let ax = ax.rotate_left(8);
    let mut ax = c.add(W, ax, 0x3030);
    let bx = c.reg(BX);
    c.or(W, bx, bx);
    if bx == 0 {
        c.cmp(B, ax & 0xff, 0x30);
        if ax & 0xff == 0x30 {
            ax = ax & 0xff00 | b' ' as u16;
        }
    }
    c.set_reg(AX, ax);
    c.set_word_in(ES, si.wrapping_sub(2), ax);
    let cx = c.pop();
    c.set_reg(CX, cx);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.ret()
}

/// `string_replace_number_ending_at_es_si` (E2E3).
pub fn string_replace_number_ending_at_es_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe2e3)?;
    number(&mut Ctx::new(m))
}

/// `find_string_and_replace_digits` (E2DB): writes AX into the number of
/// string SI (`get_phrase_or_command_string_si`, its last digit).
pub fn find_string_and_replace_digits(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe2db)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let ax = c.reg(AX);
    c.push(ax);
    c.call(code::GET_PHRASE_OR_COMMAND_STRING_SI, 0xe2df)?;
    c.run(0xe2df, 1)?;
    c.call(code::FIND_LAST_NUMERIC_DIGIT, 0xe2e2)?;
    c.run(0xe2e2, 1)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    number(&mut c)
}

/// `read_resource_to_esdi` (F244): the resource named at DX read to ES:DI
/// (again until it reads, from DUNE.DAT; a file of its own read and
/// closed).
pub fn read_resource_to_esdi(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf244)?;
    let mut c = Ctx::new(m);
    loop {
        c.run(0xf244, 2)?;
        let dx = c.reg(DX);
        c.push(dx);
        c.call(code::OPEN_RES_OR_FILE_OR_DIE, 0xf248)?;
        let bx = c.reg(BX);
        let dat = c.word(g::DUNE_DAT_HANDLE);
        c.run(0xf248, if bx == dat { 4 } else { 3 })?;
        let dx = c.pop();
        c.set_reg(DX, dx);
        c.cmp(W, bx, dat);
        if bx != dat {
            return super::files::read_ffff_to_esdi_and_close(c.m, 0xf260);
        }
        c.call(code::READ_DUNE_DAT_CX_TO_ESDI, 0xf252)?;
        if !c.flag(pc::cpu::CF) {
            c.run(0xf252, 2)?;
            return c.ret();
        }
        c.run(0xf252, 1)?;
    }
}

/// From E31C: writes AX (0..65535) as 5 digits ending at ES:SI, leading
/// zeros as spaces (the last 3 through E2F6, [`number`]'s tail). BX, CX
/// kept; DX = 0.
fn number5(c: &mut Ctx) -> Result<(), Bail> {
    let ax = c.reg(AX);
    let (q, rem) = (ax / 1000, ax % 1000);
    let blank1 = q / 10 == 0;
    let blank2 = q == 0;
    let (h, tens) = (rem / 100, rem % 100 / 10);
    let blank_tens = blank2 && h == 0 && tens == 0;
    let n = 11
        + if blank1 { 3 + if blank2 { 2 } else { 0 } } else { 0 }
        + 8
        + if blank2 { 4 + if h == 0 { 2 } else { 0 } } else { 0 }
        + 7
        + if blank2 && h == 0 { 2 + blank_tens as u32 } else { 0 }
        + 4;
    c.run(0xe31c, n)?;
    let (bx, cx) = (c.reg(BX), c.reg(CX));
    c.push(bx);
    c.push(cx);
    c.set_reg(CX, 0x3e8);
    c.set_reg(BX, 0x3e8);
    let dx = c.reg(DX);
    let zero = c.xor(W, dx, dx);
    c.set_reg(DX, zero);
    c.m.cpu.mul_div(&mut c.m.hw, W, 6, 0x3e8); // div cx
    c.aam(10);
    let v = c.reg(AX).rotate_left(8);
    let mut v = c.add(W, v, 0x3030);
    c.cmp(B, v & 0xff, 0x30);
    if blank1 {
        v = v & 0xff00 | 0x20;
        c.cmp(B, v >> 8, 0x30);
        if blank2 {
            v = 0x2020;
            let bx = c.reg(BX);
            let zero = c.xor(W, bx, bx);
            c.set_reg(BX, zero);
        }
    }
    c.set_reg(AX, v);
    let si = c.reg(SI);
    c.set_word_in(pc::cpu::ES, si.wrapping_sub(5), v);
    let rest = c.reg(DX);
    c.set_reg(AX, rest);
    let zero = c.xor(W, rest, rest);
    c.set_reg(DX, zero);
    c.set_reg(CX, 0x64);
    c.m.cpu.mul_div(&mut c.m.hw, B, 6, 0x64); // div cl
    let al = c.reg8(r8::AL);
    let mut digit = c.add(B, al as u16, 0x30) as u8;
    let bx = c.reg(BX);
    c.or(W, bx, bx);
    if bx == 0 {
        let one = c.inc(W, bx);
        c.set_reg(BX, one);
        c.cmp(B, digit as u16, 0x30);
        if h == 0 {
            digit = b' ';
            let zero = c.xor(W, one, one);
            c.set_reg(BX, zero);
        }
    }
    c.set_reg8(r8::AL, digit);
    c.m.cpu.write8(&mut c.m.hw, pc::cpu::ES, si.wrapping_sub(3), digit);
    let ah = c.reg8(r8::AH);
    c.set_reg8(r8::AL, ah);
    c.aam(10);
    let ax = c.reg(AX).rotate_left(8);
    let mut ax = c.add(W, ax, 0x3030);
    let bx = c.reg(BX);
    c.or(W, bx, bx);
    if bx == 0 {
        c.cmp(B, ax & 0xff, 0x30);
        if ax & 0xff == 0x30 {
            ax = ax & 0xff00 | b' ' as u16;
        }
    }
    c.set_reg(AX, ax);
    c.set_word_in(pc::cpu::ES, si.wrapping_sub(2), ax);
    let cx = c.pop();
    c.set_reg(CX, cx);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.ret()
}

/// `loc_0e31c`: AX as 5 digits ending at ES:SI (see [`number5`]).
pub fn string_replace_5_digits_ending_at_es_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe31c)?;
    number5(&mut Ctx::new(m))
}

/// From E297 (the stretch starting at `at` with `prefix` instructions
/// before it, done by `ops`): draws AX (0..255 for each hundred) as 3
/// digits through the font's glyph routine ([`g::FONT_DRAW`]), leading
/// zeros as spaces. CX kept.
fn draw_number(c: &mut Ctx, at: u16, prefix: u32, ax: u16, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    if ax / 100 > 0xff {
        // `div cl` faults: the interpreter raises it.
        c.m.cpu.ip = at;
        return Err(Bail);
    }
    let zero = ax / 100 == 0;
    c.run(at, prefix + 6 + if zero { 2 } else { 0 } + 1)?;
    ops(c);
    let cx = c.reg(CX);
    c.push(cx);
    c.set_reg(CX, 0x64);
    c.set_reg(AX, ax);
    c.m.cpu.mul_div(&mut c.m.hw, B, 6, 0x64); // div cl
    let al = c.reg8(r8::AL);
    let digit = c.add(B, al as u16, 0x30) as u8;
    c.cmp(B, digit as u16, 0x30);
    c.set_reg8(r8::AL, digit);
    if zero {
        c.set_reg8(r8::AL, b' ');
        let ch = c.reg8(r8::CH);
        let ch = c.dec(B, ch as u16) as u8;
        c.set_reg8(r8::CH, ch);
    }
    let glyph = c.word(g::FONT_DRAW);
    c.call(glyph, 0xe2ab)?;
    let ah = c.reg8(r8::AH);
    let ch = c.reg8(r8::CH);
    let mut v = ((ah % 10) as u16) << 8 | (ah / 10) as u16;
    v = v.wrapping_add(0x3030);
    let blank = ch != 0 && v & 0xff == 0x30;
    c.run(0xe2ab, 6 + if ch != 0 { 2 + blank as u32 } else { 0 } + 1)?;
    c.set_reg8(r8::AL, ah);
    c.aam(10);
    let ax = c.reg(AX).rotate_left(8);
    let mut ax = c.add(W, ax, 0x3030);
    c.or(B, ch as u16, ch as u16);
    if ch != 0 {
        c.cmp(B, ax & 0xff, 0x30);
        if blank {
            ax = ax & 0xff00 | 0x20;
        }
    }
    c.set_reg(AX, ax);
    let glyph = c.word(g::FONT_DRAW);
    c.call(glyph, 0xe2c2)?;
    c.run(0xe2c2, 2)?;
    let ah = c.reg8(r8::AH);
    c.set_reg8(r8::AL, ah);
    let glyph = c.word(g::FONT_DRAW);
    c.call(glyph, 0xe2c8)?;
    c.run(0xe2c8, 2)?;
    let cx = c.pop();
    c.set_reg(CX, cx);
    c.ret()
}

/// `loc_0e295`: AL drawn as 3 digits at the font's position.
pub fn draw_number_al(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe295)?;
    let mut c = Ctx::new(m);
    let al = c.reg8(r8::AL) as u16;
    draw_number(&mut c, start, 1, al, |c| {
        let ah = c.reg8(r8::AH) as u16;
        let z = c.xor(B, ah, ah);
        c.set_reg8(r8::AH, z as u8);
    })
}

/// `loc_0e290`: AX drawn as 3 digits at DX, BX (`font_set_draw_position`).
pub fn draw_number_at(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe290)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::FONT_SET_DRAW_POSITION, 0xe293)?;
    let ax = c.reg(AX);
    draw_number(&mut c, 0xe293, 1, ax, |_| {})
}

/// From B2CD (the stretch from `at`, `prefix` instructions before, done by
/// `ops`): the label of a save slot: phrase AX (low 12 bits), its two last
/// numbers found (`find_last_numeric_digit_in_str_at_es_si`), the first
/// set to the save's day ((time + 3) / 16 + 1), and after it the 10
/// characters of phrase 117h for the time of day (time & 0Fh). Registers
/// and ES kept.
fn save_slot_label(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 1)?;
    ops(c);
    c.call(code::SAVE_REGS, 0xb2d0)?;
    c.run(0xb2d0, 4)?;
    let es = c.sreg(ES);
    c.push(es);
    let ax = c.reg(AX);
    let ax = c.and(W, ax, 0x0fff);
    c.set_reg(AX, ax);
    c.set_reg(SI, ax);
    c.call(code::GET_PHRASE_OR_COMMAND_STRING_SI, 0xb2d9)?;
    c.run(0xb2d9, 1)?;
    c.call(code::FIND_LAST_NUMERIC_DIGIT, 0xb2dc)?;
    c.run(0xb2dc, 1)?;
    c.call(code::FIND_LAST_NUMERIC_DIGIT, 0xb2df)?;
    c.run(0xb2df, 9)?;
    let t = c.word(SAVE_TIME);
    c.push(t);
    let v = c.add(W, t, 3);
    let v = c.shr1(W, v, 4);
    let v = c.inc(W, v);
    c.set_reg(AX, v);
    c.call(0xe2e3, 0xb2f2)?;
    c.run(0xb2f2, 3)?;
    let si = c.reg(SI);
    c.set_reg(DI, si.wrapping_add(3));
    c.set_reg(SI, 0x117);
    c.call(code::GET_PHRASE_OR_COMMAND_STRING_SI, 0xb2fb)?;
    c.run(0xb2fb, 7)?;
    let t = c.pop();
    let al = c.and(B, t & 0xff, 0x0f);
    c.set_reg(AX, 0x0a00 | al);
    c.mul8(0x0a);
    let off = c.reg(AX);
    let si = c.reg(SI);
    let si = c.add(W, si, off);
    c.set_reg(SI, si);
    c.set_reg(CX, 0x0a);
    c.m.cpu.string_op(&mut c.m.hw, 0xa4, Some(ES), 0xf3); // rep movsb es:
    c.run(0xb30a, 2)?;
    let es = c.pop();
    c.set_sreg(ES, es);
    c.call(code::RESTORE_REGS, 0xb30e)?;
    c.run(0xb30e, 1)?;
    c.ret()
}

/// The time read from a save's file (its first word).
const SAVE_TIME: u16 = 0xd816;

/// `loc_0b2cd`: see [`save_slot_label`].
pub fn save_slot_label_ax(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb2cd)?;
    save_slot_label(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_0b2c4`: [`save_slot_label`] of the slot at SI (its phrase the
/// first word), unless [38AFh] is above 32h.
pub fn save_slot_label_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb2c4)?;
    let mut c = Ctx::new(m);
    let v = c.byte(0x38af);
    if v > 0x32 {
        c.run(start, 3)?;
        c.cmp(B, v as u16, 0x32);
        return c.ret();
    }
    save_slot_label(&mut c, start, 3, |c| {
        c.cmp(B, v as u16, 0x32);
        let si = c.reg(SI);
        let ax = c.word(si);
        c.set_reg(AX, ax);
    })
}

/// `loc_0c5cf`: a particle for the spawn record at BP (SS: its resource
/// byte, then 3-byte variants to a 0): with variants, spawned
/// (`particles_spawn_particle`), the record noted in the particle (+0Fh),
/// one variant picked at random (+0Dh) and bit 0 of +0Ch set (SI kept).
pub fn particles_spawn_variant(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc5cf)?;
    let mut c = Ctx::new(m);
    let bp = c.reg(BP);
    let res = c.byte_in(SS, bp);
    let more = c.byte_in(SS, bp.wrapping_add(3));
    c.run(start, if more == 0 { 4 } else { 6 })?;
    let _ = c.reg8(r8::AH) as u16;
    c.set_reg(AX, res as u16);
    if more == 0 {
        return particles_spawn_particle(c.m, 0xc60b);
    }
    c.push(bp);
    c.call(0xc60b, 0xc5de)?;
    // The variants, purely: SI from BP by 3 while not 0.
    let mut k = 0u16;
    let mut p = bp;
    loop {
        p = p.wrapping_add(3);
        k += 1;
        if c.byte(p) == 0 {
            break;
        }
    }
    let count = k - 1;
    c.run(0xc5de, 5 + 5 * k as u32 + if count == 0 { 2 + 2 } else { 3 })?;
    let bp = c.pop();
    c.set_reg(BP, bp);
    let di = c.reg(DI);
    c.set_word(di.wrapping_add(0x0f), bp);
    let si = c.reg(SI);
    c.push(si);
    c.set_reg(SI, bp);
    let mut bx = 0xffffu16;
    let mut p = bp;
    loop {
        bx = pure::inc(W, bx);
        p = pure::add(W, p, 3);
        let v = c.byte(p);
        if v == 0 {
            break;
        }
    }
    c.set_reg(SI, p);
    c.set_reg(BX, bx);
    if count == 0 {
        let si = c.pop();
        c.set_reg(SI, si);
        return c.ret();
    }
    c.call(0xe3df, 0xc5fa)?;
    c.run(0xc5fa, 8)?;
    let ax = c.reg(AX);
    c.set_reg(BX, ax);
    let a2 = pure::shl1(W, ax, 1);
    let a3 = pure::add(W, a2, ax);
    c.set_reg(AX, a3);
    let bp = c.reg(BP);
    let bp = pure::add(W, bp, a3);
    c.set_reg(BP, bp);
    let di = c.reg(DI);
    c.set_word(di.wrapping_add(0x0d), bp);
    let f = c.byte(di.wrapping_add(0x0c));
    let f = pure::or(B, f as u16, 1);
    c.set_byte(di.wrapping_add(0x0c), f as u8);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_0c661`: the particle at DI (resource opened first,
/// `open_onmap_resource`) moved by (DX, BX), and the screen redrawn over
/// the old place and the new one (`particles_update_dirty_rect` on the
/// union, built on the stack).
pub fn particles_move_particle(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc661)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::OPEN_ONMAP_RESOURCE, 0xc664)?;
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.run(0xc664, 17 + if dx & 0x8000 != 0 { 2 } else { 3 } + 2 + if bx & 0x8000 != 0 { 2 } else { 3 } + 2)?;
    let si = c.reg(DI);
    c.set_reg(SI, si);
    let sp = c.reg(SP);
    let sp = c.sub(W, sp, 8);
    c.set_reg(SP, sp);
    c.set_reg(DI, sp);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    for _ in 0..4 {
        c.movs(W);
    }
    let s = c.reg(SI);
    let s = c.sub(W, s, 8);
    c.set_reg(SI, s);
    let d = c.reg(DI);
    let d = c.sub(W, d, 8);
    c.set_reg(DI, d);
    for (k, v) in [(0u16, dx), (2, bx), (4, dx), (6, bx)] {
        let w = c.word(s.wrapping_add(k));
        let w = c.add(W, w, v);
        c.set_word(s.wrapping_add(k), w);
    }
    c.or(W, dx, dx);
    let k = if dx & 0x8000 != 0 { 0 } else { 4 };
    let v = c.word(s.wrapping_add(k));
    c.set_reg(AX, v);
    c.set_word(d.wrapping_add(k), v);
    c.or(W, bx, bx);
    let k = if bx & 0x8000 != 0 { 2 } else { 6 };
    let v = c.word(s.wrapping_add(k));
    c.set_reg(AX, v);
    c.set_word(d.wrapping_add(k), v);
    c.set_reg(SI, d);
    c.call(code::PARTICLES_UPDATE_DIRTY_RECT, 0xc6a9)?;
    c.run(0xc6a9, 2)?;
    let sp = c.reg(SP);
    let sp = c.add(W, sp, 8);
    c.set_reg(SP, sp);
    c.ret()
}

/// `load_save_game_timestamp` (B30F): for each save slot (the file name
/// at 38A8h, its digit at +7 from '1'; the records from SI, 4 bytes
/// each, up to the word A3h) the file's first word read into [0D816h]
/// (`loc_0b2c4` then marks it): the record's top bits become CX's (or,
/// with CH 80h, their inverse) if the file was read, else they are
/// cleared (or set).
pub fn load_save_game_timestamp(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::CF;
    entered(start, 0xb30f)?;
    let mut c = Ctx::new(m);
    c.run(0xb30f, 2)?;
    c.set_reg(DI, 0x38a8);
    c.set_byte(0x38af, 0x31);
    loop {
        c.run(0xb316, 2)?;
        let di = c.reg(DI);
        c.set_reg(DX, di);
        c.call(0xf2fc, 0xb31b)?;
        c.run(0xb31b, 1)?;
        c.set_reg(AX, 0x3d00);
        c.interp(0xb31e, 0xb320)?;
        if c.flag(CF) {
            c.run(0xb320, 1)?;
        } else {
            c.run(0xb320, 6)?;
            let ax = c.reg(AX);
            c.set_reg(BX, ax);
            c.set_reg(DX, 0xd816);
            let cx = c.reg(CX);
            c.push(cx);
            c.set_reg(CX, 2);
            c.set_reg8(r8::AH, 0x3f);
            c.interp(0xb32d, 0xb32f)?;
            let ax = c.reg(AX);
            c.run(0xb32f, 3 + if ax < 2 { 0 } else { 1 })?;
            c.cmp(W, ax, 2);
            let v = c.pop();
            c.set_reg(CX, v);
            if ax >= 2 {
                c.set_reg8(r8::AH, 0x3e);
                c.interp(0xb336, 0xb338)?;
                c.run(0xb338, 1)?;
                c.call(0xb2c4, 0xb33b)?;
                c.run(0xb33b, 1)?;
                c.set_flag(CF, false);
            }
        }
        let (ax, cx, si) = (c.reg(AX), c.reg(CX), c.reg(SI));
        let ch = cx >> 8;
        let next = c.word(si.wrapping_add(4));
        c.run(0xb33c, 3 + (ch == 0x80) as u32 + 7 + (next == 0xa3) as u32)?;
        let mut ax = c.sbb(W, ax, ax);
        c.cmp(B, ch, 0x80);
        if ch == 0x80 {
            ax = !ax;
        }
        let ax = c.and(W, ax, cx);
        c.set_reg(AX, ax);
        let v = c.word(si);
        let v = c.and(W, v, 0x3fff);
        let v = c.or(W, v, ax);
        c.set_word(si, v);
        let si = c.add(W, si, 4);
        c.set_reg(SI, si);
        let di = c.reg(DI);
        let d = c.byte(di.wrapping_add(7));
        let d = c.inc(B, d as u16);
        c.set_byte(di.wrapping_add(7), d as u8);
        let next = c.word(si);
        c.cmp(W, next, 0xa3);
        if next == 0xa3 {
            return c.ret();
        }
    }
}

/// `compress_sav` (B4EA): CX bytes from DS:SI packed to ES:DI + 4: a
/// run of up to FEh equal bytes as F7h, count, byte (and any F7h byte
/// so), runs of 1 or 2 bytes as they are. Then a 0 word, and the header
/// at ES:DI: DX (DL F7h, DH the last run's count) and the packed size
/// (from DI + 4, the end word included).
pub fn compress_sav(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb4ea)?;
    let mut c = Ctx::new(m);
    c.run(0xb4ea, 3)?;
    c.set_reg8(r8::DL, 0xf7);
    let di0 = c.reg(DI);
    c.push(di0);
    let d = pure::add(W, di0, 4);
    c.set_reg(DI, d);
    loop {
        c.run(0xb4f0, 1)?;
        let mut dh = 0u8;
        c.set_reg8(r8::DH, 0);
        let mut al;
        loop {
            let si = c.reg(SI);
            let a = c.byte(si);
            let nx = c.byte(si.wrapping_add(1));
            let cx = c.reg(CX);
            let ne = a != nx;
            let full = !ne && dh.wrapping_add(1) == 0xff;
            let end = !ne && !full && cx.wrapping_sub(1) == 0;
            c.run(0xb4f2, if ne { 4 } else if full { 6 } else { 9 + end as u32 })?;
            c.lods(B, DS);
            al = a;
            dh = pure::inc(B, dh as u16) as u8;
            c.set_reg8(r8::DH, dh);
            if ne {
                break;
            }
            if full {
                break;
            }
            let cx = pure::dec(W, cx);
            c.set_reg(CX, cx);
            if cx == 0 {
                let cx = pure::inc(W, 0);
                c.set_reg(CX, cx);
                break;
            }
        }
        let dl = c.reg8(r8::DL);
        let esc = al == dl || (dh != 1 && dh != 2);
        let cmps = if al == dl { 2 } else if dh == 1 { 4 } else { 6 };
        let body = if esc { 6 } else if dh == 2 { 2 } else { 0 };
        let left = c.reg(CX).wrapping_sub(1);
        c.run(0xb504, cmps + body + 2 + if left == 0 { 8 } else { 0 })?;
        if esc {
            c.set_reg8(r8::AH, al);
            c.set_reg8(r8::AL, dl);
            c.stos(B);
            c.set_reg8(r8::AL, dh);
            c.stos(B);
            c.set_reg8(r8::AL, al);
        } else if dh == 2 {
            c.stos(B);
        }
        c.stos(B);
        c.set_reg(CX, left);
        if left == 0 {
            break;
        }
    }
    let di = c.reg(DI);
    c.set_reg(CX, di);
    c.set_reg(AX, 0);
    c.stos(W);
    let d0 = c.pop();
    c.set_reg(DI, d0);
    let n = pure::sub(W, di, d0);
    c.set_reg(CX, n);
    let dx = c.reg(DX);
    c.set_word_in(ES, d0, dx);
    c.set_word_in(ES, d0.wrapping_add(2), n);
    c.ret()
}

/// `unrle_image_to_tmp_buffer` (C2A1): the RLE image at DS:SI ([SI-2]'s
/// low byte the row count) unpacked into SS:4C60h, each row
/// (DI mod 200h + 3) / 4 * 2 bytes: a byte n < 80h copies n + 1 bytes,
/// else the next byte is repeated 1 - n times. Returns DS:SI = SS:4C60h;
/// AX, BX, CX, DI, ES, BP kept.
pub fn unrle_image_to_tmp_buffer(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc2a1)?;
    let mut c = Ctx::new(m);
    c.run(0xc2a1, 17)?;
    for r in [AX, BX, CX, DI] {
        let v = c.reg(r);
        c.push(v);
    }
    let es = c.sreg(ES);
    c.push(es);
    let bp = c.reg(BP);
    c.push(bp);
    let ss = c.sreg(SS);
    c.push(ss);
    let v = c.pop();
    c.set_sreg(ES, v);
    let di0 = c.reg(DI);
    c.set_reg(BP, di0);
    c.set_reg(DI, 0x4c60);
    let w = c.and(W, di0, 0x1ff);
    let w = c.add(W, w, 3);
    let w = c.shr1(W, w, 2);
    let w = c.shl1(W, w, 1);
    c.set_reg(BP, w);
    let si = c.reg(SI);
    let n = c.word(si.wrapping_sub(2));
    c.xor(B, n >> 8, n >> 8);
    let mut rows = n & 0xff;
    c.set_reg(CX, rows);
    loop {
        c.run(0xc2c0, 2)?;
        c.push(rows);
        let mut bx = w;
        c.set_reg(BX, bx);
        loop {
            let si = c.reg(SI);
            let al = c.byte(si);
            let lit = al & 0x80 == 0;
            let len = if lit { 1 + al as u16 } else { 1u8.wrapping_sub(al) as u16 };
            let left = bx.wrapping_sub(len);
            c.run(0xc2c3, 3 + if lit { 5 } else { 6 })?;
            c.lods(B, DS);
            c.test(B, al as u16, 0x80);
            if lit {
                c.add(B, 1, al as u16);
            } else {
                c.sub(B, 1, al as u16);
            }
            c.set_reg8(r8::CL, len as u8);
            c.xor(B, 0, 0);
            c.set_reg(CX, len);
            bx = c.sub(W, bx, len);
            c.set_reg(BX, bx);
            if lit {
                c.rep(0xa4);
                c.run(0xc2d2, 1 + (left == 0) as u32)?;
            } else {
                c.lods(B, DS);
                c.rep(0xaa);
                c.run(0xc2e1, 1)?;
            }
            if bx == 0 {
                break;
            }
        }
        c.run(0xc2e3, 2)?;
        rows = c.pop().wrapping_sub(1);
        c.set_reg(CX, rows);
        if rows == 0 {
            break;
        }
    }
    c.run(0xc2e6, 10)?;
    c.set_reg(SI, 0x4c60);
    let ss = c.sreg(SS);
    c.push(ss);
    let v = c.pop();
    c.set_sreg(DS, v);
    let v = c.pop();
    c.set_reg(BP, v);
    let v = c.pop();
    c.set_sreg(ES, v);
    for r in [DI, CX, BX, AX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// `particles_update_dirty_rect` (C6AD): the rectangle at SI (x0 with
/// flags in its top bits, y0, x1, y1) redrawn on screen. With the mouse
/// shown over it, it is hidden first (`loc_0dbb2`) and drawn again on
/// return (the return goes through DBECh). Clipped to the view at
/// D83Ch (into D834h; nothing left: done). Then, on the background
/// (`loc_0c443`), the live particles (3CC0h, [3CBEh] of 11h bytes)
/// meeting it listed on the stack (up to 100h) and drawn in the order
/// the sorter [2786h] gives (`loc_0c30d` each, taken from the list);
/// the frieze (`loc_01797`) when the rectangle reaches it outside the
/// intro; the two overlays [0DBE0h], [0DBE2h] (`loc_0c7d4`); and the
/// rectangle copied to the screen (`loc_0c51e`).
pub fn particles_update_dirty_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc6ad)?;
    let mut c = Ctx::new(m);
    c.run(0xc6ad, 1)?;
    c.call(0xc13b, 0xc6b0)?;
    let si = c.reg(SI);
    let k = c.byte(0xdc46);
    let (my, mx) = (c.word(0xdc44), c.word(0xdc42));
    let r: Vec<u16> = (0..4).map(|i| c.word(si.wrapping_add(2 * i))).collect();
    let bx0 = r[0] & 0x0fff;
    let lt = |a: u16, b: u16| (a as i16) < (b as i16);
    let mut n = 2;
    let mut hit = false;
    if k & 0x80 == 0 {
        n += 3;
        if lt(my, r[3]) {
            n += 3;
            if lt(r[1], my.wrapping_add(0x10)) {
                n += 3;
                if lt(mx, r[2]) {
                    n += 5;
                    if lt(bx0, mx.wrapping_add(0x10)) {
                        n += 4;
                        hit = true;
                    }
                }
            }
        }
    }
    c.run(0xc6b0, n)?;
    c.cmp(B, k as u16, 0);
    if k & 0x80 == 0 {
        c.set_reg(AX, my);
        c.cmp(W, my, r[3]);
        if lt(my, r[3]) {
            let a = c.add(W, my, 0x10);
            c.set_reg(AX, a);
            c.cmp(W, a, r[1]);
            if lt(r[1], a) {
                c.set_reg(AX, mx);
                c.cmp(W, mx, r[2]);
                if lt(mx, r[2]) {
                    let a = c.add(W, mx, 0x10);
                    c.set_reg(AX, a);
                    c.set_reg(BX, r[0]);
                    let bh = c.and(B, r[0] >> 8, 0xf);
                    c.set_reg(BX, (bh << 8) | (r[0] & 0xff));
                    c.cmp(W, a, bx0);
                    if hit {
                        c.set_reg(AX, 0xdbec);
                        c.push(0xdbec);
                        c.push(si);
                        c.call(0xdbb2, 0xc6e3)?;
                        c.run(0xc6e3, 1)?;
                        let v = c.pop();
                        c.set_reg(SI, v);
                    }
                }
            }
        }
    }
    // Clip to the view.
    let si = c.reg(SI);
    let r: Vec<u16> = (0..4).map(|i| c.word(si.wrapping_add(2 * i))).collect();
    let v: Vec<u16> = (0..4).map(|i| c.word(0xd83c + 2 * i)).collect();
    let x0 = if lt(r[0], v[0]) { v[0] } else { r[0] };
    let y0 = if lt(r[1], v[1]) { v[1] } else { r[1] };
    let x1 = if lt(v[2], r[2]) { v[2] } else { r[2] };
    let y1 = if lt(v[3], r[3]) { v[3] } else { r[3] };
    let mut n = 4 + 3 + lt(r[0], v[0]) as u32 + 1 + 3 + lt(r[1], v[1]) as u32 + 1;
    n += 3 + lt(v[2], r[2]) as u32 + 2;
    let empty_x = !lt(x0, x1);
    let empty_y = !empty_x && !lt(y0, y1);
    if empty_x {
        n += 1;
    } else {
        n += 1 + 3 + lt(v[3], r[3]) as u32 + 2;
        n += if empty_y { 1 } else { 2 };
    }
    c.run(0xc6e4, n)?;
    let ds = c.sreg(DS);
    c.set_reg(AX, ds);
    c.set_sreg(ES, ds);
    c.set_reg(DI, 0xd834);
    c.set_reg(BX, 8);
    let ops: [(u16, u16, bool); 4] = [(r[0], v[0], true), (r[1], v[1], true), (r[2], v[2], false), (r[3], v[3], false)];
    for (i, (a, b, low)) in ops.into_iter().enumerate() {
        c.lods(W, DS);
        c.cmp(W, a, b);
        let take = if low { lt(a, b) } else { lt(b, a) };
        let val = if take { b } else { a };
        c.set_reg(AX, val);
        if i >= 2 {
            let lo = if i == 2 { x0 } else { y0 };
            c.cmp(W, val, lo);
            if !lt(lo, val) {
                c.ret()?;
                return ret_mouse_tail(&mut c);
            }
        }
        c.stos(W);
    }
    c.call(0xc443, 0xc71b)?;
    // The particles meeting the rectangle, listed on the stack.
    let count = c.word(0x3cbe);
    c.run(0xc71b, 4)?;
    let sp = c.reg(SP);
    let sp = c.sub(W, sp, 0x200);
    c.set_reg(SP, sp);
    c.set_reg(DI, sp);
    c.set_reg(CX, count);
    if count != 0 {
        c.run(0xc727, 9)?;
        c.set_reg(SI, 0xd834);
        let (dx, bx, bp, ax) = (c.word(0xd834), c.word(0xd836), c.word(0xd838), c.word(0xd83a));
        c.set_reg(DX, dx);
        c.set_reg(BX, bx);
        c.set_reg(BP, bp);
        c.set_reg(AX, ax);
        c.set_reg(SI, 0xd83a);
        let mut si = 0x3cc0u16;
        c.set_reg(SI, si);
        let mut di = sp;
        for k in 0..count {
            let f = c.byte(si.wrapping_add(0xc));
            let p: Vec<u16> = (0..4).map(|i| c.word(si.wrapping_add(2 * i))).collect();
            let mut n = 2;
            let mut meets = false;
            c.check(0xc738, 16)?;
            c.cmp(B, f as u16, 0);
            if f & 0x80 == 0 {
                n += 2;
                c.cmp(W, p[0], bp);
                if lt(p[0], bp) {
                    n += 2;
                    c.cmp(W, p[1], ax);
                    if lt(p[1], ax) {
                        n += 2;
                        c.cmp(W, p[2], dx);
                        if lt(dx, p[2]) {
                            n += 2;
                            c.cmp(W, p[3], bx);
                            if lt(bx, p[3]) {
                                n += 2;
                                meets = true;
                            }
                        }
                    }
                }
            }
            if meets {
                c.set_word(di, si);
                di = c.add(W, di, 2);
                c.set_reg(DI, di);
            }
            si = c.add(W, si, 0x11);
            c.set_reg(SI, si);
            c.set_reg(CX, count - 1 - k);
            c.count(n + 2);
        }
        let len = di.wrapping_sub(sp) >> 1;
        c.run(0xc75b, 4)?;
        c.set_reg(CX, di);
        let d = c.sub(W, di, sp);
        let h = c.shr1(W, d, 1);
        c.set_reg(CX, h);
        if len != 0 {
            loop {
                c.run(0xc763, 3)?;
                let sp0 = c.reg(SP);
                c.set_reg(SI, sp0);
                let cx = c.reg(CX);
                c.push(cx);
                let to = c.word(0x2786);
                c.call(to, 0xc76a)?;
                if c.flag(pc::cpu::SF) {
                    c.run(0xc76a, 2)?;
                    let v = c.pop();
                    c.set_reg(CX, v);
                    break;
                }
                c.run(0xc76a, 7)?;
                let bx = c.reg(BX);
                let s0 = c.reg(SI);
                c.xor(W, s0, s0);
                let s = c.word(bx.wrapping_sub(2));
                c.set_word(bx.wrapping_sub(2), 0);
                c.set_reg(SI, s);
                let ax = c.word(s.wrapping_add(8));
                c.set_reg(AX, ax);
                let dx = c.word(s);
                c.set_reg(DX, dx);
                let b = c.word(s.wrapping_add(2));
                c.set_reg(BX, b);
                c.call(0xc30d, 0xc77c)?;
                c.run(0xc77c, 2)?;
                let v = c.pop();
                c.set_reg(CX, v);
            }
        }
    }
    // The frieze, the overlays, the copy.
    let f = c.byte(0x227d);
    let (b, h, l) = (c.word(0xd83a), c.word(0xd838), c.word(0xd834));
    let n = if f != 0 {
        2
    } else if lt(b, 0x89) {
        4
    } else if lt(h, 0x7e) {
        6
    } else if !lt(l, 0xc2) {
        8
    } else {
        9
    };
    c.run(0xc780, n)?;
    c.cmp(B, f as u16, 0);
    if f == 0 {
        c.cmp(W, b, 0x89);
        if n > 4 {
            c.cmp(W, h, 0x7e);
        }
        if n > 6 {
            c.cmp(W, l, 0xc2);
        }
        if n == 9 {
            c.call(0x1797, 0xc7a2)?;
        }
    }
    for (at, ov, ret) in [(0xc7a2u16, 0xdbe0u16, 0xc7b0u16), (0xc7b0, 0xdbe2, 0xc7be)] {
        let s = c.word(ov);
        c.run(at, 3 + if s != 0 { 2 } else { 0 })?;
        c.set_reg(SI, s);
        c.or(W, s, s);
        if s == 0 {
            break;
        }
        c.set_reg(DI, 0xd834);
        c.call(0xc7d4, ret)?;
    }
    c.run(0xc7be, 6)?;
    c.set_reg(SI, 0xd834);
    let (dx, bx, bp, ax) = (c.word(0xd834), c.word(0xd836), c.word(0xd838), c.word(0xd83a));
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.set_reg(BP, bp);
    c.set_reg(AX, ax);
    c.call(0xc51e, 0xc7cf)?;
    c.run(0xc7cf, 2)?;
    let sp = c.reg(SP);
    let s = c.add(W, sp, 0x200);
    c.set_reg(SP, s);
    c.ret()?;
    ret_mouse_tail(&mut c)
}

/// After a `ret`: landed on DBECh (pushed when the mouse was hidden),
/// the mouse is drawn again there.
fn ret_mouse_tail(c: &mut Ctx) -> Result<(), Bail> {
    if c.m.cpu.ip == 0xdbec {
        return super::mouse::draw_mouse(c.m, 0xdbec);
    }
    Ok(())
}
