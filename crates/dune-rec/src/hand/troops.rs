//! Troops: 1Bh-byte records from [`g::TROOPS`], numbered from 1; and the
//! per-location byte 19h, its bits spread over bytes and back.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CS, CX, DF, DI, DS, DX, ES, SI, ZF},
    rec::Bail,
};

mod code {
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
    pub const PARTICLES_REMOVE_PARTICLE: u16 = 0xc58a;
    pub const FOR_RECORD: u16 = 0x661d;
    pub const FOR_EACH_TROOP: u16 = 0x6603;
    pub const TROOP_PTR: u16 = 0x6906;
}

const TROOP_SIZE: u8 = 0x1b;

/// `loc_06906`: SI = troop AL (AX kept); ZF, CF from `cmp` of its byte 3
/// with 80h.
pub fn troop_ptr(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6906)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    troop_ptr_ops(&mut c);
    c.ret()
}

/// [`troop_ptr`]'s instructions (counted by the caller).
fn troop_ptr_ops(c: &mut Ctx) {
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

/// From 1E24 (the stretch from `at`, `prefix` instructions before, done by
/// `ops`): CF set when one of the troops at the location at DI (its byte
/// 9 the first, then each troop's byte 1) has bit 400h of word 12h. SI
/// kept.
fn location_has_400(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.check(at, prefix + 5)?;
    ops(c);
    let di = c.reg(DI);
    let first = c.byte(di.wrapping_add(9));
    if first == 0 {
        c.count(prefix + 5);
        c.set_reg8(r8::AL, 0);
        c.or(B, 0, 0);
        c.set_flag(CF, false);
        return c.ret();
    }
    c.count(prefix + 5);
    c.set_reg8(r8::AL, first);
    c.or(B, first as u16, first as u16);
    let si = c.reg(SI);
    c.push(si);
    c.call(code::TROOP_PTR, 0x1e2f)?;
    loop {
        let si = c.reg(SI);
        let w = c.word(si.wrapping_add(0x12));
        if w & 0x400 != 0 {
            c.run(0x1e2f, 5)?;
            c.test(W, w, 0x400);
            let si = c.pop();
            c.set_reg(SI, si);
            c.set_flag(CF, true);
            return c.ret();
        }
        let next = c.byte(si.wrapping_add(1));
        c.run(0x1e2f, if next == 0 { 8 } else { 6 })?;
        c.test(W, w, 0x400);
        c.set_reg8(r8::AL, next);
        c.or(B, next as u16, next as u16);
        if next == 0 {
            let si = c.pop();
            c.set_reg(SI, si);
            c.set_flag(CF, false);
            return c.ret();
        }
        c.call(code::TROOP_PTR, 0x1e2f)?;
    }
}

/// `loc_01e24`: see [`location_has_400`].
pub fn location_has_troop_400(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1e24)?;
    location_has_400(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_01e01`: in phase 5Dh, for the troop at SI of kind 7 at a location
/// (word 2 = 80h, location BH): marked 2, CF from [`location_has_400`]
/// on its location; CF clear otherwise.
pub fn troop_check_1e01(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1e01)?;
    let mut c = Ctx::new(m);
    let phase = c.byte(g::GAME_PHASE);
    let si = c.reg(SI);
    let kind = c.byte(si.wrapping_add(0x0e));
    let w2 = c.word(si.wrapping_add(2));
    let ok = phase == 0x5d && kind == 7 && w2 & 0xff == 0x80;
    if !ok {
        let n = if phase != 0x5d { 4 } else if kind != 7 { 6 } else { 9 };
        c.run(start, n)?;
        c.cmp(B, phase as u16, 0x5d);
        if phase == 0x5d {
            c.cmp(B, kind as u16, 7);
            if kind == 7 {
                c.set_reg(BX, w2);
                c.cmp(B, w2 & 0xff, 0x80);
            }
        }
        c.set_flag(CF, false);
        return c.ret();
    }
    location_has_400(&mut c, start, 13, |c| {
        c.cmp(B, phase as u16, 0x5d);
        c.cmp(B, kind as u16, 7);
        c.set_reg(BX, w2);
        c.cmp(B, w2 & 0xff, 0x80);
        c.set_byte(si, 2);
        c.set_reg(AX, 0x1c);
        c.mul8((w2 >> 8) as u8);
        let ax = c.reg(AX);
        let di = c.add(W, ax, 0xe4);
        c.set_reg(AX, di);
        c.set_reg(DI, di);
    })
}

/// `loc_01eda`: every 32 calls ([0F9h] by 8): `loc_06603` with BP =
/// 1EB1h, `loc_029f0` with AX = 709h, [0F8h] one down, and [11DBh] = the
/// first location with a troop of bit 400h (else 0, and phase 60h,
/// `loc_011cb`). CX, SI kept.
pub fn troops_1eda(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1eda)?;
    let mut c = Ctx::new(m);
    let v = c.byte(0xf9);
    let n = v.wrapping_add(8);
    c.run(start, if n != 0 { 3 } else { 6 })?;
    let n = c.add(B, v as u16, 8);
    c.set_byte(0xf9, n as u8);
    if n != 0 {
        return c.ret();
    }
    let (cx, si) = (c.reg(CX), c.reg(SI));
    c.push(cx);
    c.push(si);
    c.set_reg(BP, 0x1eb1);
    c.call(code::FOR_EACH_TROOP, 0x1ee9)?;
    c.run(0x1ee9, 2)?;
    c.set_reg(AX, 0x709);
    c.call(0x29f0, 0x1eef)?;
    c.run(0x1eef, 3)?;
    let f8 = c.byte(0xf8);
    let f8 = c.dec(B, f8 as u16);
    c.set_byte(0xf8, f8 as u8);
    c.set_reg(DI, 0x100);
    c.call(0x1e24, 0x1ef9)?;
    loop {
        let found = c.flag(CF);
        let di = c.reg(DI);
        if found {
            c.run(0x1ef9, 4 + if di != 0 { 3 } else { 1 })?;
            break;
        }
        let next = di.wrapping_add(0x1c);
        let end = c.byte(next) == 0xff;
        if !end {
            c.run(0x1ef9, 5)?;
            let d = c.add(W, di, 0x1c);
            c.set_reg(DI, d);
            let b = c.byte(d);
            c.cmp(B, b as u16, 0xff);
            c.call(0x1e24, 0x1ef9)?;
            continue;
        }
        c.run(0x1ef9, 5 + 4)?;
        c.add(W, di, 0x1c);
        c.cmp(B, 0xff, 0xff);
        c.xor(W, 0, 0);
        c.set_reg(DI, 0);
        break;
    }
    let di = c.reg(DI);
    c.set_word(0x11db, di);
    c.or(W, di, di);
    if di == 0 {
        c.call(0x11cb, 0x1f10)?;
        c.run(0x1f10, 3)?;
    }
    let si = c.pop();
    c.set_reg(SI, si);
    let cx = c.pop();
    c.set_reg(CX, cx);
    c.ret()
}

/// `loc_0221d`: the troop at SI moved to the place of `loc_05344` (from
/// its word 0): word 0, 2 = `loc_040ae`'s DX, BX; bit 4 of byte 0Fh when
/// that is the player's location ([9]). CX, SI kept.
pub fn troop_to_place(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x221d)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let (cx, si) = (c.reg(CX), c.reg(SI));
    c.push(cx);
    c.push(si);
    let w = c.word(si);
    c.set_reg(DX, w);
    c.call(0x5344, 0x2224)?;
    c.run(0x2224, 1)?;
    c.call(0x40ae, 0x2227)?;
    let bh = c.reg8(r8::BH);
    let here = c.byte(9);
    c.run(0x2227, if bh == here { 8 } else { 7 })?;
    let si = c.pop();
    c.set_reg(SI, si);
    let cx = c.pop();
    c.set_reg(CX, cx);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.set_word(si, dx);
    c.set_word(si.wrapping_add(2), bx);
    c.cmp(B, bh as u16, here as u16);
    if bh == here {
        let f = c.byte(si.wrapping_add(0x0f));
        let f = c.or(B, f as u16, 4);
        c.set_byte(si.wrapping_add(0x0f), f as u8);
    }
    c.ret()
}

/// `loc_01d66`: for the 12 16-byte records from 0FD8h at a location
/// (byte 2 = 80h, byte 3 its number): word 0 = (that location's kind, 1)
/// when its kind changed, or its count (byte 0) is above the kind's limit
/// (CS:1D35h).
pub fn refresh_records_fd8(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1d66)?;
    let mut c = Ctx::new(m);
    let mut n = 2 + 1;
    let mut si = 0x0fd8u16;
    for _ in 0..12 {
        let ax = c.word(si.wrapping_add(2));
        n += 3;
        if ax & 0xff == 0x80 {
            n += 2;
            if ax >> 8 != 0xff {
                let di = (0x1cu16 * (ax >> 8)).wrapping_add(0xe4);
                let w = c.word(si);
                let bl = c.byte(di.wrapping_add(8));
                n += 8;
                if (w >> 8) as u8 != bl {
                    n += 3;
                } else {
                    let lim = c.byte_in(CS, (bl as u16).wrapping_add(0x1d35));
                    n += 3;
                    if (w as u8) > lim {
                        n += 3;
                    }
                }
            }
        }
        n += 2;
        si = si.wrapping_add(0x10);
    }
    c.run(start, n)?;
    c.set_reg(CX, 0x0c);
    let mut si = 0x0fd8u16;
    for i in 0..12u16 {
        c.set_reg(SI, si);
        let ax = c.word(si.wrapping_add(2));
        c.set_reg(AX, ax);
        c.cmp(B, ax & 0xff, 0x80);
        if ax & 0xff == 0x80 {
            c.cmp(B, ax >> 8, 0xff);
            if ax >> 8 != 0xff {
                c.set_reg(AX, ax & 0xff00 | 0x1c);
                c.mul8((ax >> 8) as u8);
                let a = c.reg(AX);
                let di = c.add(W, a, 0xe4);
                c.set_reg(DI, di);
                let w = c.word(si);
                c.set_reg(AX, w);
                let bl = c.byte(di.wrapping_add(8));
                c.set_reg8(r8::BL, bl);
                c.cmp(B, w >> 8, bl as u16);
                let mut write = (w >> 8) as u8 != bl;
                if !write {
                    c.set_reg8(r8::BH, 0);
                    c.xor(B, 0, 0);
                    let lim = c.byte_in(CS, (bl as u16).wrapping_add(0x1d35));
                    c.cmp(B, w & 0xff, lim as u16);
                    write = (w as u8) > lim;
                }
                if write {
                    let v = (bl as u16) << 8 | 1;
                    c.set_reg(AX, v);
                    c.set_word(si, v);
                }
            }
        }
        si = c.add(W, si, 0x10);
        c.set_reg(CX, 0x0b - i);
    }
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_01cda`: [0A8h] = the sum over the locations not of `location_kind_
/// check` of byte 12h / 8, plus a random part of a sixteenth of it. DX =
/// how many counted.
pub fn sum_locations_a8(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1cda)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_reg(DI, 0x100);
    c.xor(W, 0, 0);
    c.set_reg(CX, 0);
    c.set_reg(DX, 0);
    c.call(0x5d36, 0x1ce4)?;
    loop {
        let skip = c.flag(CF);
        let di = c.reg(DI);
        let next = di.wrapping_add(0x1c);
        let end = c.byte(next) == 0xff;
        c.run(0x1ce4, if skip { 1 } else { 8 } + 3 + if end { 6 } else { 1 })?;
        if !skip {
            let dx = c.reg(DX);
            let dx = c.inc(W, dx);
            c.set_reg(DX, dx);
            let b = c.byte(di.wrapping_add(0x12)) as u16;
            let b = c.shr1(B, b, 3);
            c.xor(B, 0, 0);
            c.set_reg(AX, b);
            let cx = c.reg(CX);
            let cx = c.add(W, cx, b);
            c.set_reg(CX, cx);
        }
        let d = c.add(W, di, 0x1c);
        c.set_reg(DI, d);
        let v = c.byte(d);
        c.cmp(B, v as u16, 0xff);
        if end {
            break;
        }
        c.call(0x5d36, 0x1ce4)?;
    }
    let cx = c.reg(CX);
    let bx = c.shr1(W, cx, 4);
    c.set_reg(BX, bx);
    c.call(0xe3df, 0x1d09)?;
    c.run(0x1d09, 3)?;
    let (cx, ax) = (c.reg(CX), c.reg(AX));
    let cx = c.add(W, cx, ax);
    c.set_reg(CX, cx);
    c.set_word(0xa8, cx);
    c.ret()
}

/// `loc_021fa`: a random number 2..0Ch, not 3 before phase 54h and not 0Bh
/// or 6 before phase 24h.
pub fn random_2_to_12(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x21fa)?;
    let mut c = Ctx::new(m);
    let (mut at, mut extra) = (start, 0);
    loop {
        c.run(at, extra + 2)?;
        c.set_reg(BX, 0x0a);
        c.call(0xe3df, 0x2200)?;
        let al = c.reg8(r8::AL);
        let v = al.wrapping_add(2);
        let phase = c.byte(g::GAME_PHASE);
        let (n, again) = if phase >= 0x54 {
            (4, false)
        } else if v == 3 {
            (5, true)
        } else if phase >= 0x24 {
            (8, false)
        } else if v == 0x0b {
            (9, true)
        } else if v == 6 {
            (11, true)
        } else {
            (12, false)
        };
        c.run(0x2200, n)?;
        let v = c.add(B, al as u16, 2) as u8;
        c.set_reg8(r8::AL, v);
        c.cmp(B, phase as u16, 0x54);
        if phase < 0x54 {
            c.cmp(B, v as u16, 3);
            if v != 3 {
                c.cmp(B, phase as u16, 0x24);
                if phase < 0x24 {
                    c.cmp(B, v as u16, 0x0b);
                    if v != 0x0b {
                        c.cmp(B, v as u16, 6);
                    }
                }
            }
        }
        if !again {
            return c.ret();
        }
        (at, extra) = (0x21fa, 0);
    }
}

/// From 6603 (the stretch from `at`, `prefix` instructions before, done by
/// `ops`): BP's routine for each troop at the location DI (its byte 9 the
/// first, each troop's byte 1 the next; SI the troop, DI and BP kept for
/// it). SI kept.
fn each_troop(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    let di = c.reg(DI);
    let first = c.byte(di.wrapping_add(9));
    c.run(at, prefix + if first == 0 { 6 } else { 5 })?;
    ops(c);
    let si = c.reg(SI);
    c.push(si);
    c.set_reg8(r8::AL, first);
    c.or(B, first as u16, first as u16);
    if first == 0 {
        let si = c.pop();
        c.set_reg(SI, si);
        return c.ret();
    }
    loop {
        c.call(code::TROOP_PTR, 0x660e)?;
        c.run(0x660e, 4)?;
        for r in [SI, DI, BP] {
            let v = c.reg(r);
            c.push(v);
        }
        let bp = c.reg(BP);
        c.call(bp, 0x6613)?;
        let sp = c.reg(pc::cpu::SP);
        let si = c.word_in(pc::cpu::SS, sp.wrapping_add(4));
        let next = c.byte(si.wrapping_add(1));
        c.run(0x6613, 7 + if next == 0 { 2 } else { 1 })?;
        for r in [BP, DI, SI] {
            let v = c.pop();
            c.set_reg(r, v);
        }
        c.set_reg8(r8::AL, next);
        c.or(B, next as u16, next as u16);
        if next == 0 {
            let si = c.pop();
            c.set_reg(SI, si);
            return c.ret();
        }
    }
}

/// `loc_06603`: see [`each_troop`].
pub fn for_each_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6603)?;
    each_troop(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_05098`: [`each_troop`] with `sub_5082`, CX = DX = 0 first.
pub fn for_each_troop_5082(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5098)?;
    each_troop(&mut Ctx::new(m), start, 4, |c| {
        c.set_reg(BP, 0x5082);
        c.xor(W, 0, 0);
        c.set_reg(CX, 0);
        c.set_reg(DX, 0);
    })
}

/// `loc_0661d`: as [`for_each_troop`], for the troops `troop_ptr` gives
/// CF for (byte 3 below 80h).
pub fn for_each_troop_below_80(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x661d)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let first = c.byte(di.wrapping_add(9));
    c.run(start, if first == 0 { 6 } else { 5 })?;
    let si = c.reg(SI);
    c.push(si);
    c.set_reg8(r8::AL, first);
    c.or(B, first as u16, first as u16);
    if first == 0 {
        let si = c.pop();
        c.set_reg(SI, si);
        return c.ret();
    }
    loop {
        c.call(code::TROOP_PTR, 0x6628)?;
        let take = c.flag(CF);
        let si = c.reg(SI);
        if take {
            c.run(0x6628, 5)?;
            for r in [SI, DI, BP] {
                let v = c.reg(r);
                c.push(v);
            }
            let bp = c.reg(BP);
            c.call(bp, 0x662f)?;
            let sp = c.reg(pc::cpu::SP);
            let si = c.word_in(pc::cpu::SS, sp.wrapping_add(4));
            let next = c.byte(si.wrapping_add(1));
            c.run(0x662f, 7 + if next == 0 { 2 } else { 1 })?;
            for r in [BP, DI, SI] {
                let v = c.pop();
                c.set_reg(r, v);
            }
            c.set_reg8(r8::AL, next);
            c.or(B, next as u16, next as u16);
            if next == 0 {
                let si = c.pop();
                c.set_reg(SI, si);
                return c.ret();
            }
        } else {
            let next = c.byte(si.wrapping_add(1));
            c.run(0x6628, 5 + if next == 0 { 2 } else { 1 })?;
            c.set_reg8(r8::AL, next);
            c.or(B, next as u16, next as u16);
            if next == 0 {
                let si = c.pop();
                c.set_reg(SI, si);
                return c.ret();
            }
        }
    }
}

/// `loc_0668f`: the troop at SI started (bit 20h of byte 3, unless set or
/// bit 80h of byte 10h): byte 19h cleared, stamped with the game time,
/// `loc_06fb0` with AL = 4 (DI kept).
pub fn start_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x668f)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let (b3, b10) = (c.byte(si.wrapping_add(3)), c.byte(si.wrapping_add(0x10)));
    let n = if b3 & 0x20 != 0 { 3 } else if b10 & 0x80 != 0 { 5 } else { 11 };
    c.run(start, n)?;
    c.test(B, b3 as u16, 0x20);
    if b3 & 0x20 != 0 {
        return c.ret();
    }
    c.test(B, b10 as u16, 0x80);
    if b10 & 0x80 != 0 {
        return c.ret();
    }
    let v = c.or(B, b3 as u16, 0x20);
    c.set_byte(si.wrapping_add(3), v as u8);
    c.set_byte(si.wrapping_add(0x19), 0);
    let t = c.word(g::GAME_TIME);
    c.set_reg(AX, t);
    c.set_word(si.wrapping_add(0x0a), t);
    let di = c.reg(DI);
    c.push(di);
    c.set_reg8(r8::AL, 4);
    c.call(0x6fb0, 0x66af)?;
    c.run(0x66af, 2)?;
    let di = c.pop();
    c.set_reg(DI, di);
    c.ret()
}

/// `loc_068eb`: SI = the troop shown: in the zoomed map the one of
/// [1954h] (`troop_ptr`), else the one picked from the list at 4758h by
/// [476Ch]. Flags from its byte 3 against 80h.
pub fn shown_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x68eb)?;
    let mut c = Ctx::new(m);
    let al = c.byte(0x1954);
    let k = c.byte(0x46eb);
    if k >= 0x80 {
        c.run(start, 3 + 8)?;
        c.set_reg8(r8::AL, al);
        c.cmp(B, k as u16, 0x80);
        troop_ptr_ops(&mut c);
        return c.ret();
    }
    c.run(start, 12)?;
    c.set_reg8(r8::AL, al);
    c.cmp(B, k as u16, 0x80);
    let i = c.byte(0x476c) as u16;
    c.xor(B, 0, 0);
    let i2 = c.add(W, i, i);
    c.set_reg(AX, i2);
    let si = c.word(i2.wrapping_add(0x4758));
    c.set_reg(SI, si);
    let n = c.byte(si);
    c.set_reg8(r8::AL, n);
    let b = c.byte(si.wrapping_add(3));
    c.cmp(B, b as u16, 0x80);
    c.ret()
}

/// The particles: a count at 3CBEh, 11h-byte records after it (word 0Ah
/// the troop, byte 0Ch flags).
const PARTICLES: u16 = 0x3cbe;

/// `loc_06917`: DI = the particle of the troop SI (not hidden, bit 40h),
/// ZF set; ZF clear (from `or di, di`) when none or not in hotspot mode.
pub fn troop_particle(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6917)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x46eb);
    let si = c.reg(SI);
    // The path, purely.
    let n_all = c.word(PARTICLES);
    let mut n = 3;
    let mut di = 0x3cafu16;
    let mut cx = n_all;
    let mut found = false;
    if k & 0x80 == 0 {
        n += 2;
    } else {
        n += 1;
        loop {
            n += 1;
            if cx == 0 {
                n += 2;
                break;
            }
            let mut zf;
            loop {
                di = di.wrapping_add(0x11);
                zf = c.word(di.wrapping_add(0x0a)) == si;
                cx = cx.wrapping_sub(1);
                n += 3;
                if cx == 0 || zf {
                    break;
                }
            }
            n += 1;
            if !zf {
                n += 1;
                break;
            }
            n += 2;
            if c.byte(di.wrapping_add(0x0c)) & 0x40 == 0 {
                n += 1;
                found = true;
                break;
            }
        }
    }
    c.run(start, n)?;
    c.cmp(B, k as u16, 0);
    c.set_reg(DI, 0x3caf);
    if k & 0x80 == 0 {
        c.or(W, 0x3caf, 0x3caf);
        return c.ret();
    }
    let mut cx = n_all;
    c.set_reg(CX, cx);
    let mut di = 0x3cafu16;
    loop {
        if cx == 0 {
            c.or(W, di, di);
            return c.ret();
        }
        loop {
            di = c.add(W, di, 0x11);
            let w = c.word(di.wrapping_add(0x0a));
            c.cmp(W, w, si);
            cx = cx.wrapping_sub(1);
            if cx == 0 || w == si {
                break;
            }
        }
        c.set_reg(DI, di);
        c.set_reg(CX, cx);
        if c.word(di.wrapping_add(0x0a)) != si {
            return c.ret();
        }
        let f = c.byte(di.wrapping_add(0x0c));
        c.test(B, f as u16, 0x40);
        if f & 0x40 == 0 {
            let _ = found;
            return c.ret();
        }
    }
}

/// `loc_06946`: the particle under (DX, BX) (not hidden), last first: SI
/// = it, DI = its troop (flags from its byte 3 against 80h); CF clear when
/// none.
pub fn particle_at(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6946)?;
    let mut c = Ctx::new(m);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let count = c.word(PARTICLES);
    let first = PARTICLES + 2 + (0x11u16 * (count & 0xff));
    let mut n = 4;
    let mut hit = None;
    if count == 0 {
        n += 2;
    } else {
        n += 3;
        let mut si = first;
        for k in 0..count {
            si = si.wrapping_sub(0x11);
            let r = [c.word(si), c.word(si.wrapping_add(2)), c.word(si.wrapping_add(4)), c.word(si.wrapping_add(6))];
            let f = c.byte(si.wrapping_add(0x0c));
            n += 3;
            let ok = if (r[0] as i16) >= (dx as i16) {
                false
            } else {
                n += 2;
                if (r[1] as i16) >= (bx as i16) {
                    false
                } else {
                    n += 2;
                    if (dx as i16) >= (r[2] as i16) {
                        false
                    } else {
                        n += 2;
                        if f & 0x40 != 0 {
                            false
                        } else {
                            n += 2;
                            (bx as i16) < (r[3] as i16)
                        }
                    }
                }
            };
            if ok {
                n += 3;
                hit = Some((si, k));
                break;
            }
            n += 1;
        }
        if hit.is_none() {
            n += 2;
        }
    }
    c.run(start, n)?;
    c.set_reg(SI, PARTICLES);
    let w = c.lods(W, pc::cpu::DS);
    c.set_reg(CX, w);
    if count == 0 {
        c.set_flag(CF, false);
        return c.ret();
    }
    c.set_reg(AX, w & 0xff00 | 0x11);
    c.mul8(count as u8);
    let off = c.reg(AX);
    let mut si = c.add(W, PARTICLES + 2, off);
    let mut cx = count;
    loop {
        si = c.sub(W, si, 0x11);
        c.set_reg(SI, si);
        let r0 = c.word(si);
        c.cmp(W, r0, dx);
        let mut ok = false;
        if (r0 as i16) < (dx as i16) {
            let r1 = c.word(si.wrapping_add(2));
            c.cmp(W, r1, bx);
            if (r1 as i16) < (bx as i16) {
                let r2 = c.word(si.wrapping_add(4));
                c.cmp(W, dx, r2);
                if (dx as i16) < (r2 as i16) {
                    let f = c.byte(si.wrapping_add(0x0c));
                    c.test(B, f as u16, 0x40);
                    if f & 0x40 == 0 {
                        let r3 = c.word(si.wrapping_add(6));
                        c.cmp(W, bx, r3);
                        ok = (bx as i16) < (r3 as i16);
                    }
                }
            }
        }
        if ok {
            let di = c.word(si.wrapping_add(0x0a));
            c.set_reg(DI, di);
            let b = c.byte(di.wrapping_add(3));
            c.cmp(B, b as u16, 0x80);
            return c.ret();
        }
        cx -= 1;
        c.set_reg(CX, cx);
        if cx == 0 {
            c.set_flag(CF, false);
            let _ = hit;
            return c.ret();
        }
    }
}

/// `loc_06cfc`: one more at the location DI's byte 0Bh (a place below
/// kind 20h, under 0Ch, without bit 1 of byte 0Ah), and `loc_0644e`. SI,
/// DI kept.
pub fn grow_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6cfc)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let (kind, b, f) = (c.byte(di.wrapping_add(8)), c.byte(di.wrapping_add(0x0b)), c.byte(di.wrapping_add(0x0a)));
    let n = if kind >= 0x20 { 3 } else if b >= 0x0c { 5 } else if f & 1 != 0 { 7 } else { 10 };
    c.run(start, n)?;
    c.cmp(B, kind as u16, 0x20);
    if kind >= 0x20 {
        return c.ret();
    }
    c.cmp(B, b as u16, 0x0c);
    if b >= 0x0c {
        return c.ret();
    }
    c.test(B, f as u16, 1);
    if f & 1 != 0 {
        return c.ret();
    }
    let v = c.inc(B, b as u16);
    c.set_byte(di.wrapping_add(0x0b), v as u8);
    let si = c.reg(SI);
    c.push(si);
    c.push(di);
    c.call(0x644e, 0x6d16)?;
    c.run(0x6d16, 3)?;
    let di = c.pop();
    c.set_reg(DI, di);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_06edd`: AL more at the troop SI's skill BX (byte 16h + BX, at most
/// 5Fh); when it crosses a 16 boundary, its word 10h's low bits = BX + 1.
pub fn add_skill(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6edd)?;
    let mut c = Ctx::new(m);
    let (si, bx, al) = (c.reg(SI), c.reg(BX), c.reg8(r8::AL));
    let at = bx.wrapping_add(si).wrapping_add(0x16);
    let dl = c.byte(at);
    let s = al.wrapping_add(dl);
    let v = if s > 0x5f { 0x5f } else { s };
    let crossed = (v ^ dl) & 0xf0 != 0;
    c.run(start, 4 + (s > 0x5f) as u32 + 4 + if crossed { 6 } else { 1 })?;
    c.set_reg8(r8::DL, dl);
    let s = c.add(B, al as u16, dl as u16);
    c.cmp(B, s, 0x5f);
    c.set_byte(at, v);
    let x = c.xor(B, v as u16, dl as u16);
    let x = c.and(B, x, 0xf0);
    c.set_reg8(r8::AL, x as u8);
    if !crossed {
        return c.ret();
    }
    let w = c.word(si.wrapping_add(0x10));
    let lo = c.and(B, w & 0xff, 0xfc);
    let b = c.inc(W, bx);
    c.set_reg(BX, b);
    let lo = c.or(B, lo, b & 0xff);
    let w = w & 0xff00 | lo;
    c.set_reg(AX, w);
    c.set_word(si.wrapping_add(0x10), w);
    c.ret()
}

/// `loc_06f56`: AL more at every troop's byte 15h (at most 100), but the
/// ones with bits A0h of byte 3. SI kept.
pub fn add_to_all_troops_15(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6f56)?;
    let mut c = Ctx::new(m);
    let al = c.reg8(r8::AL);
    let mut n = 2;
    let mut si = g::TROOPS;
    while si < g::TROOPS_END {
        let f = c.byte(si.wrapping_add(3));
        n += 2;
        if f & 0xa0 == 0 {
            let v = c.byte(si.wrapping_add(0x15)).wrapping_add(al);
            n += 3 + (v > 0x64) as u32;
        }
        n += 3;
        si += TROOP_SIZE as u16;
    }
    c.run(start, n + 2)?;
    let s0 = c.reg(SI);
    c.push(s0);
    let mut si = g::TROOPS;
    while si < g::TROOPS_END {
        let f = c.byte(si.wrapping_add(3));
        c.test(B, f as u16, 0xa0);
        if f & 0xa0 == 0 {
            let at = si.wrapping_add(0x15);
            let v = c.byte(at);
            let v = c.add(B, v as u16, al as u16);
            c.set_byte(at, v as u8);
            c.cmp(B, v, 0x64);
            if v > 0x64 {
                c.set_byte(at, 0x64);
            }
        }
        si = c.add(W, si, TROOP_SIZE as u16);
        c.cmp(W, si, g::TROOPS_END);
    }
    c.set_reg(SI, si);
    let s0 = c.pop();
    c.set_reg(SI, s0);
    c.ret()
}

/// `loc_068d2`: [`each_troop`] with `loc_068da`, CX = 0.
pub fn for_each_troop_68da(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x68d2)?;
    each_troop(&mut Ctx::new(m), start, 3, |c| {
        c.xor(W, 0, 0);
        c.set_reg(CX, 0);
        c.set_reg(BP, 0x68da);
    })
}

/// `loc_06efd`: AL = the troop SI's effective motivation: byte 15h (+14h
/// with [0FAh]); at its own location for activity 6 +1Eh, at most 100 (100
/// for activity 8, 9); in phases 64h..67h 28h less, at least 10.
pub fn troop_motivation(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6efd)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let act = c.byte(si.wrapping_add(3)) & 0x0f;
    let base = c.byte(si.wrapping_add(0x15));
    let bonus = c.byte(0xfa) != 0;
    let mut al = if bonus { base.wrapping_add(0x14) } else { base };
    let mut n = 5 + bonus as u32 + 2;
    if act == 6 {
        let home = c.word(si.wrapping_add(4)) == c.word(0x114e);
        n += 5;
        if home {
            al = al.wrapping_add(0x1e);
            n += 2 + 2;
            if al >= 0x64 {
                al = 0x64;
                n += 1;
            }
        }
    } else if act & 0xfe == 8 {
        n += 3 + 1;
        al = 0x64;
    } else {
        n += 3 + 2;
        if al >= 0x64 {
            al = 0x64;
            n += 1;
        }
    }
    let phase = c.byte(g::GAME_PHASE);
    let late = (0x64..0x68).contains(&phase);
    n += if phase < 0x64 { 3 } else if phase >= 0x68 { 5 } else { 0 };
    let mut floor = false;
    if late {
        let v = al.wrapping_sub(0x28);
        floor = (v as i8) < 0x0a;
        n += 7 + floor as u32 + 1;
    }
    c.run(start, n)?;
    let b3 = c.byte(si.wrapping_add(3));
    let ah = c.and(B, b3 as u16, 0x0f);
    let mut a = base as u16;
    c.set_reg(AX, ah << 8 | a);
    let f = c.byte(0xfa);
    c.cmp(B, f as u16, 0);
    if bonus {
        a = c.add(B, a, 0x14);
    }
    c.cmp(B, ah, 6);
    let mut ah2 = ah;
    if act == 6 {
        let di = c.reg(DI);
        c.push(di);
        let loc = c.word(si.wrapping_add(4));
        c.set_reg(DI, loc);
        let here = c.word(0x114e);
        c.cmp(W, loc, here);
        let di = c.pop();
        c.set_reg(DI, di);
        if loc == here {
            a = c.add(B, a, 0x1e);
            c.cmp(B, a, 0x64);
            if a >= 0x64 {
                a = 0x64;
            }
        }
    } else {
        ah2 = c.and(B, ah, 0xfe);
        c.cmp(B, ah2, 8);
        if ah2 == 8 {
            a = 0x64;
        } else {
            c.cmp(B, a, 0x64);
            if a >= 0x64 {
                a = 0x64;
            }
        }
    }
    c.cmp(B, phase as u16, 0x64);
    if phase >= 0x64 {
        c.cmp(B, phase as u16, 0x68);
        if phase < 0x68 {
            a = c.sub(B, a, 0x28);
            c.cmp(B, a, 0x0a);
            if floor {
                a = 0x0a;
            }
        }
    }
    let _ = al;
    c.set_reg(AX, ah2 << 8 | a);
    c.ret()
}

/// `loc_06fb0`: charisma ([29h]) down by AL (to at least 1); when that
/// drops a 4-step, every troop (but bits A0h of byte 3) loses
/// motivation (`decrease_troop_motivation`) once per step. SI kept.
pub fn lose_charisma(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6fb0)?;
    let mut c = Ctx::new(m);
    let d = c.reg8(r8::AL);
    let ch = c.byte(g::CHARISMA);
    let (v, borrow) = ch.overflowing_sub(d);
    let new = if borrow || v == 0 { 1 } else { v };
    let steps = ((ch & 0xfc).wrapping_sub(new & 0xfc)) >> 2;
    c.run(start, 4 + (borrow || v == 0) as u32 + 7 + if steps == 0 { 1 } else { 2 })?;
    c.set_reg8(r8::AH, d);
    let r = c.sub(B, ch as u16, d as u16);
    c.set_reg8(r8::AL, r as u8);
    c.set_reg(AX, (new as u16) << 8 | ch as u16);
    c.set_byte(g::CHARISMA, new);
    let ax = c.and(W, (new as u16) << 8 | ch as u16, 0xfcfc);
    let s = c.sub(B, ax & 0xff, ax >> 8);
    let s = c.shr1(B, s, 2);
    c.set_reg(AX, ax & 0xff00 | s);
    if s == 0 {
        return c.ret();
    }
    let si = c.reg(SI);
    c.push(si);
    let mut p = g::TROOPS;
    loop {
        c.set_reg(SI, p);
        let f = c.byte(p.wrapping_add(3));
        if f & 0xa0 == 0 {
            c.run(0x6fd1, 3)?;
            c.test(B, f as u16, 0xa0);
            c.call(0x6f93, 0x6fda)?;
        } else {
            c.run(0x6fd1, 2)?;
            c.test(B, f as u16, 0xa0);
        }
        let s = c.reg(SI);
        let next = s.wrapping_add(TROOP_SIZE as u16);
        let more = next < g::TROOPS_END;
        c.run(0x6fda, 3 + if more { 0 } else { 2 })?;
        let next = c.add(W, s, TROOP_SIZE as u16);
        c.cmp(W, next, g::TROOPS_END);
        c.set_reg(SI, next);
        if !more {
            let si = c.pop();
            c.set_reg(SI, si);
            return c.ret();
        }
        p = next;
    }
}

/// From 6BB6 (CF from the caller): the troop SI's bit 10h of byte 3 made
/// CF; when it changes, `loc_08461` (DI kept) and, when it was set,
/// `stamp_record`. Flags come back as given.
fn set_troop_bit_10(c: &mut Ctx) -> Result<(), Bail> {
    let cf = c.flag(CF);
    let si = c.reg(SI);
    let b3 = c.byte(si.wrapping_add(3));
    let want = if cf { 0x10 } else { 0 };
    let have = b3 & 0x10;
    if have == want {
        c.run(0x6bb6, 8)?;
    } else {
        c.run(0x6bb6, 10)?;
    }
    let f = c.m.cpu.flags();
    c.push(f);
    let ah = c.sbb(B, 0, 0);
    let ax = c.and(W, ah << 8 | b3 as u16, 0x1010);
    c.set_reg(AX, ax);
    c.cmp(B, ax & 0xff, ax >> 8);
    if have != want {
        let v = c.xor(B, b3 as u16, 0x10);
        c.set_byte(si.wrapping_add(3), v as u8);
        c.push(ax);
        let di = c.reg(DI);
        c.push(di);
        c.call(0x8461, 0x6bcc)?;
        c.run(0x6bcc, if have == 0 { 6 } else { 5 })?;
        let di = c.pop();
        c.set_reg(DI, di);
        let ax = c.pop();
        c.set_reg(AX, ax);
        c.test(B, ax & 0xff, 0x10);
        if ax & 0x10 != 0 {
            c.call(0x6b25, 0x6bd5)?;
            c.run(0x6bd5, 2)?;
        }
    }
    let f = c.pop();
    c.m.cpu.set_flags(f);
    c.ret()
}

/// `loc_06b8a`: [`set_troop_bit_10`] with CF set when the location DI has
/// either of bits 41h of byte 0Ah.
pub fn troop_bit_10_by_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6b8a)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let di = c.reg(DI);
    let a = c.byte(di.wrapping_add(0x0a));
    let ah = c.and(B, a as u16, 0x41);
    c.set_reg8(r8::AH, ah as u8);
    c.cmp(B, ah, 1);
    let cf = c.flag(CF);
    c.set_flag(CF, !cf);
    set_troop_bit_10(&mut c)
}

/// `loc_06b96`: [`set_troop_bit_10`] with CF set for a troop with bit
/// 200h of word 10h or bits 30h of word 12h, or at a location with byte
/// 12h 0, or with bits 41h of byte 0Ah other than 40h alone.
pub fn troop_bit_10_by_state(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6b96)?;
    let mut c = Ctx::new(m);
    let (si, di) = (c.reg(SI), c.reg(DI));
    let w10 = c.word(si.wrapping_add(0x10));
    let w12 = c.word(si.wrapping_add(0x12));
    let b12 = c.byte(di.wrapping_add(0x12));
    let a = c.byte(di.wrapping_add(0x0a));
    let x = (a ^ 0x40) & 0x41;
    let n = if w10 & 0x200 != 0 {
        3
    } else if w12 & 0x30 != 0 {
        6
    } else if b12 < 1 {
        8
    } else if x == 0 {
        12
    } else {
        13
    };
    c.run(start, n)?;
    c.test(W, w10, 0x200);
    c.set_flag(CF, true);
    if w10 & 0x200 == 0 {
        c.test(W, w12, 0x30);
        c.set_flag(CF, true);
        if w12 & 0x30 == 0 {
            c.cmp(B, b12 as u16, 1);
            if b12 >= 1 {
                c.set_reg8(r8::AL, a);
                let v = c.xor(B, a as u16, 0x40);
                let v = c.and(B, v, 0x41);
                c.set_reg8(r8::AL, v as u8);
                if v != 0 {
                    c.set_flag(CF, true);
                }
            }
        }
    }
    set_troop_bit_10(&mut c)
}
