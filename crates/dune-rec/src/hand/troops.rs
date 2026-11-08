//! Troops: 1Bh-byte records from [`g::TROOPS`], numbered from 1; and the
//! per-location byte 19h, its bits spread over bytes and back.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, CF, CX, DF, DI, DS, ES, SI, ZF},
    rec::Bail,
};

mod code {
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
    pub const PARTICLES_REMOVE_PARTICLE: u16 = 0xc58a;
    pub const FOR_RECORD: u16 = 0x661d;
    pub const FOR_EACH_TROOP: u16 = 0x6603;
}

const TROOP_SIZE: u8 = 0x1b;

/// `loc_06906`: SI = troop AL (AX kept); ZF, CF from `cmp` of its byte 3
/// with 80h.
pub fn troop_ptr(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6906)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    let ax = c.reg(AX);
    let n = c.dec(B, ax & 0xff);
    c.set_reg(AX, (TROOP_SIZE as u16) << 8 | n);
    c.mul8(TROOP_SIZE);
    let off = c.reg(AX);
    let si = c.add(W, off, g::TROOPS);
    c.set_reg(SI, si);
    c.set_reg(AX, ax);
    let b = c.byte(si.wrapping_add(3));
    c.cmp(B, b as u16, 0x80);
    c.ret()
}

/// `loc_0693b`: AX = bits 2..3 of the troop's byte 3 (its activity).
pub fn troop_activity(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x693b)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let si = c.reg(SI);
    let b = c.byte(si.wrapping_add(3));
    let ax = c.reg(AX) & 0xff00 | b as u16;
    let ax = c.and(W, ax, 0x0f);
    let ax = c.shr1(W, ax, 2);
    c.set_reg(AX, ax);
    c.ret()
}

/// `loc_066b1`: `loc_0858c`; the particle of the troop (`loc_06917`, ZF)
/// removed; then the troop at SI reset (word 4 = 0FBCh, byte 3 = A0h,
/// byte 1Ah = 0).
pub fn reset_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x66b1)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x858c, 0x66b4)?;
    c.run(0x66b4, 3)?;
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.push(si);
    c.push(di);
    c.call(0x6917, 0x66b9)?;
    let zf = c.flag(ZF);
    if zf {
        c.run(0x66b9, 2)?;
        c.call(code::PARTICLES_REMOVE_PARTICLE, 0x66be)?;
        c.run(0x66be, 6)?;
    } else {
        c.run(0x66b9, 7)?;
    }
    let di = c.pop();
    c.set_reg(DI, di);
    let si = c.pop();
    c.set_reg(SI, si);
    c.set_word(si.wrapping_add(4), 0x0fbc);
    c.set_byte(si.wrapping_add(3), 0xa0);
    c.set_byte(si.wrapping_add(0x1a), 0);
    c.ret()
}

/// `loc_06757`: troop AX (`troop_ptr`); unless `loc_0686e` sets CF,
/// `loc_06770`, and with BP 1 or more `loc_0c5cf` (SI kept), CF clear.
pub fn troop_6757(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6757)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x6906, 0x675a)?;
    c.run(0x675a, 1)?;
    c.call(0x686e, 0x675d)?;
    if c.flag(CF) {
        c.run(0x675d, 2)?;
        return c.ret();
    }
    c.run(0x675d, 2)?;
    c.call(0x6770, 0x6762)?;
    let bp = c.reg(BP);
    if bp < 1 {
        c.run(0x6762, 3)?;
        c.cmp(W, bp, 1);
        return c.ret();
    }
    c.run(0x6762, 4)?;
    c.cmp(W, bp, 1);
    let si = c.reg(SI);
    c.push(si);
    c.call(0xc5cf, 0x676b)?;
    c.run(0x676b, 3)?;
    let si = c.pop();
    c.set_reg(SI, si);
    c.set_flag(CF, false);
    c.ret()
}

/// `loc_069a3`: the particle at [4752h] (if any) removed and forgotten (DI
/// kept).
pub fn remove_particle_4752(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x69a3)?;
    let mut c = Ctx::new(m);
    let p = c.word(0x4752);
    c.run(start, if p == 0 { 7 } else { 6 })?;
    let di = c.reg(DI);
    c.push(di);
    c.xor(W, di, di);
    c.set_word(0x4752, 0);
    c.set_reg(DI, p);
    c.or(W, p, p);
    if p != 0 {
        c.call(code::PARTICLES_REMOVE_PARTICLE, 0x69b1)?;
        c.run(0x69b1, 2)?;
    }
    let di = c.pop();
    c.set_reg(DI, di);
    c.ret()
}

/// `loc_06b25`: the record at SI stamped with the game time (word 0Ah),
/// words 0Ch, 0Eh cleared.
pub fn stamp_record(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6b25)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let si = c.reg(SI);
    let t = c.word(g::GAME_TIME);
    c.set_word(si.wrapping_add(0x0a), t);
    let z = c.xor(W, t, t);
    c.set_reg(AX, z);
    c.set_word(si.wrapping_add(0x0c), 0);
    c.set_word(si.wrapping_add(0x0e), 0);
    c.ret()
}

/// `loc_06e02`: `loc_06603` with BP = 764Dh (CX = 0) again while it leaves
/// CX set.
pub fn repeat_764d(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6e02)?;
    let mut c = Ctx::new(m);
    let (mut at, mut extra) = (start, 0);
    loop {
        c.run(at, extra + 3)?;
        let cx = c.reg(CX);
        c.xor(W, cx, cx);
        c.set_reg(CX, 0);
        c.set_reg(BP, 0x764d);
        c.call(code::FOR_EACH_TROOP, 0x6e0a)?;
        let cx = c.reg(CX);
        if cx == 0 {
            c.run(0x6e0a, 3)?;
            c.or(W, cx, cx);
            return c.ret();
        }
        c.or(W, cx, cx);
        (at, extra) = (0x6e0a, 2);
    }
}

/// `loc_06ebf`: `loc_0661d` with BP = 6ECBh on the record at the troop's
/// word 4 (DI kept).
pub fn for_troop_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6ebf)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let di = c.reg(DI);
    c.push(di);
    let si = c.reg(SI);
    let loc = c.word(si.wrapping_add(4));
    c.set_reg(DI, loc);
    c.set_reg(BP, 0x6ecb);
    c.call(code::FOR_RECORD, 0x6ec9)?;
    c.run(0x6ec9, 2)?;
    let di = c.pop();
    c.set_reg(DI, di);
    c.ret()
}

/// `loc_06f48`: AL more at the troop's byte 15h, at most 100.
pub fn add_to_troop_15(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6f48)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let at = si.wrapping_add(0x15);
    let (v, al) = (c.byte(at), c.reg8(r8::AL));
    let sum = v.wrapping_add(al);
    c.run(start, if sum <= 0x64 { 4 } else { 5 })?;
    let sum = c.add(B, v as u16, al as u16);
    c.set_byte(at, sum as u8);
    c.cmp(B, sum, 0x64);
    if sum > 0x64 {
        c.set_byte(at, 0x64);
    }
    c.ret()
}

/// From 7085 (`prefix` instructions before, done by `ops`): bit 10h of the
/// troop's byte 3.
fn mark_troop(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 2)?;
    ops(c);
    let si = c.reg(SI);
    let b = c.byte(si.wrapping_add(3));
    let b = c.or(B, b as u16, 0x10);
    c.set_byte(si.wrapping_add(3), b as u8);
    c.ret()
}

/// `loc_07085`: see [`mark_troop`].
pub fn mark_troop_10(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7085)?;
    mark_troop(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_0719c`: bit 200h of the troop's word 10h, then [`mark_troop`].
pub fn mark_troop_200_10(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x719c)?;
    mark_troop(&mut Ctx::new(m), start, 2, |c| {
        let si = c.reg(SI);
        let w = c.word(si.wrapping_add(0x10));
        let w = c.or(W, w, 0x200);
        c.set_word(si.wrapping_add(0x10), w);
    })
}

/// `loc_071b2`: `loc_029f0` with AH = 0Fh (SI, DI kept).
pub fn call_29f0_f(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x71b2)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_reg8(r8::AH, 0x0f);
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.push(si);
    c.push(di);
    c.call(0x29f0, 0x71b9)?;
    c.run(0x71b9, 3)?;
    let di = c.pop();
    c.set_reg(DI, di);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_0765e`: the sum of byte 19h over the locations (from 100h, up to
/// the FFh one), less 10 when that does not borrow, added to the
/// location at DI's (registers kept).
pub fn add_location_sum(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x765e)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SAVE_REGS, 0x7661)?;
    let mut si = 0x100u16;
    let mut k = 0;
    let mut sum = 0u8;
    loop {
        sum = sum.wrapping_add(c.byte(si.wrapping_add(0x19)));
        si = si.wrapping_add(0x1c);
        k += 1;
        if c.byte(si) == 0xff {
            break;
        }
    }
    let borrow = sum < 0x0a;
    c.run(0x7661, 2 + 4 * k + if borrow { 3 } else { 4 })?;
    c.set_reg(SI, 0x100);
    let cx = c.reg(CX);
    c.xor(W, cx, cx);
    let mut si = 0x100u16;
    let mut cl = 0u16;
    loop {
        let v = c.byte(si.wrapping_add(0x19));
        cl = c.add(B, cl, v as u16);
        si = c.add(W, si, 0x1c);
        let b = c.byte(si);
        c.cmp(B, b as u16, 0xff);
        if b == 0xff {
            break;
        }
    }
    c.set_reg(SI, si);
    let cl = c.sub(B, cl, 0x0a);
    c.set_reg(CX, cl);
    if !borrow {
        let di = c.reg(DI);
        let v = c.byte(di.wrapping_add(0x19));
        let v = c.add(B, v as u16, cl);
        c.set_byte(di.wrapping_add(0x19), v as u8);
    }
    c.call(code::RESTORE_REGS, 0x767c)?;
    c.run(0x767c, 1)?;
    c.ret()
}

/// The bits of a location's byte 19h, one per byte (bit 7 first).
const SPREAD_BITS: u16 = 0x4705;
const SPREAD_BITS_END: u16 = 0x470c;

/// `loc_07efb`: the location's byte 19h spread over [`SPREAD_BITS`] (DI
/// kept).
pub fn spread_location_bits(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7efb)?;
    let mut c = Ctx::new(m);
    if c.flag(DF) {
        return Err(Bail);
    }
    let n = (SPREAD_BITS_END - SPREAD_BITS) as u32;
    c.run(start, 5 + 5 * n + 2)?;
    let di = c.reg(DI);
    c.push(di);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(DI, SPREAD_BITS);
    let si = c.reg(SI);
    let b = c.byte(si.wrapping_add(0x19));
    let mut ax = (b as u16) << 8 | c.reg(AX) & 0xff;
    loop {
        ax &= 0xff00;
        c.xor(B, 0, 0);
        ax = c.rol1(W, ax);
        c.set_reg(AX, ax);
        c.stos(B);
        let di = c.reg(DI);
        c.cmp(W, di, SPREAD_BITS_END);
        if di >= SPREAD_BITS_END {
            break;
        }
    }
    let di = c.pop();
    c.set_reg(DI, di);
    c.ret()
}

/// `loc_07f11`: [`SPREAD_BITS`] packed back into the location's byte 19h.
pub fn pack_location_bits(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7f11)?;
    let mut c = Ctx::new(m);
    let n = (SPREAD_BITS_END - SPREAD_BITS) as u32;
    c.run(start, 4 + 4 * n + 4)?;
    let si = c.reg(SI);
    c.push(si);
    c.set_reg(SI, SPREAD_BITS_END - 1);
    c.set_flag(DF, true);
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    c.set_reg(AX, 0);
    loop {
        c.lods(B, DS);
        let ax = c.reg(AX);
        let ax = c.ror1(W, ax);
        c.set_reg(AX, ax);
        let s = c.reg(SI);
        c.cmp(W, s, SPREAD_BITS);
        if s < SPREAD_BITS {
            break;
        }
    }
    let si = c.pop();
    c.set_reg(SI, si);
    c.set_flag(DF, false);
    let ah = c.reg8(r8::AH);
    c.set_byte(si.wrapping_add(0x19), ah);
    c.ret()
}

/// `loc_07f5f`: for each set bit of the location's byte 19h (bit 7
/// first), one more at the matching byte from DI + 14h. DI kept.
pub fn count_location_bits(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7f5f)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let bits = c.byte(si.wrapping_add(0x19));
    // The path: 7F66 shl; jae; then from 7F6A/7F6C as the bits go.
    let mut n = 3 + 2;
    let mut al = bits;
    let mut cf = al & 0x80 != 0;
    al <<= 1;
    loop {
        if cf {
            n += 1; // inc byte [di]
        }
        n += 3; // inc di; shl al, 1; jb
        cf = al & 0x80 != 0;
        al <<= 1;
        if cf {
            continue;
        }
        n += 1; // jne
        if al == 0 {
            break;
        }
    }
    n += 2;
    c.run(start, n)?;
    c.set_reg8(r8::AL, bits);
    let di = c.reg(DI);
    c.push(di);
    let mut d = c.add(W, di, 0x14);
    let mut al = c.shl1(B, bits as u16, 1);
    let mut cf = c.flag(CF);
    loop {
        if cf {
            let v = c.byte(d);
            let v = c.inc(B, v as u16);
            c.set_byte(d, v as u8);
        }
        d = c.inc(W, d);
        al = c.shl1(B, al, 1);
        cf = c.flag(CF);
        if cf {
            continue;
        }
        if al == 0 {
            break;
        }
    }
    c.set_reg8(r8::AL, al as u8);
    let di = c.pop();
    c.set_reg(DI, di);
    c.ret()
}

/// Set while [`redraw_7b36`] runs.
const REDRAWING: u16 = 0x46d8;

/// `loc_07b2b`: unless [`REDRAWING`], driver function 30 (`loc_0c0e8`).
pub fn gfx_call_30_unless_redrawing(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7b2b)?;
    let mut c = Ctx::new(m);
    let v = c.byte(REDRAWING);
    c.run(start, 3)?;
    c.cmp(B, v as u16, 0);
    if v != 0 {
        return c.ret();
    }
    super::gfx::gfx_call_30(c.m, 0xc0e8)
}

/// `loc_07b36`: `loc_08770`, `loc_05f79`, `loc_079de` with [`REDRAWING`]
/// set and the transition flag at 80h; then both cleared, and [46F4h]
/// (SI, DI kept).
pub fn redraw_7b36(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7b36)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.push(si);
    c.push(di);
    c.set_byte(REDRAWING, 1);
    c.set_byte(g::TRANSITION_ACTIVE, 0x80);
    c.call(0x8770, 0x7b45)?;
    c.run(0x7b45, 1)?;
    c.call(0x5f79, 0x7b48)?;
    c.run(0x7b48, 1)?;
    c.call(0x79de, 0x7b4b)?;
    c.run(0x7b4b, 5)?;
    c.set_byte(g::TRANSITION_ACTIVE, 0);
    c.set_byte(0x46f4, 0);
    let di = c.pop();
    c.set_reg(DI, di);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}
