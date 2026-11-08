//! Saved games (their RLE, restoring the map's bits and the game's
//! state), particles, numbers in strings, reading resources.

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
