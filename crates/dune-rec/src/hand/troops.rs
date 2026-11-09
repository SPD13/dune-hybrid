//! Troops: 1Bh-byte records from [`g::TROOPS`], numbered from 1; and the
//! per-location byte 19h, its bits spread over bytes and back.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CS, CX, DF, DI, DS, DX, ES, SI, SP, SS, ZF},
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

/// `loc_0697c`: for the troop SI with its particle (`loc_06917`, ZF), at
/// its own location (bit 40h of byte 3) or one `loc_05ed0` accepts: unless
/// `loc_0686e` sets CF, `loc_0c5cf` with BP 18FDh, then the location DI
/// gets bit 40h of byte 0Ch and becomes [4752h].
pub fn mark_troop_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x697c)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x6917, 0x697f)?;
    if !c.flag(ZF) {
        c.run(0x697f, 2)?;
        return c.ret();
    }
    let si = c.reg(SI);
    let b3 = c.byte(si.wrapping_add(3));
    if b3 & 0x40 != 0 {
        c.run(0x697f, 4)?;
        c.test(B, b3 as u16, 0x40);
    } else {
        c.run(0x697f, 5)?;
        c.test(B, b3 as u16, 0x40);
        let loc = c.word(si.wrapping_add(4));
        c.set_reg(DI, loc);
        c.call(0x5ed0, 0x698d)?;
        let zf = c.flag(ZF);
        c.run(0x698d, 2)?;
        if !zf {
            return c.ret();
        }
    }
    c.call(0x686e, 0x6992)?;
    if c.flag(CF) {
        c.run(0x6992, 2)?;
        return c.ret();
    }
    c.run(0x6992, 3)?;
    c.set_reg(BP, 0x18fd);
    c.call(0xc5cf, 0x699a)?;
    c.run(0x699a, 3)?;
    let di = c.reg(DI);
    let v = c.byte(di.wrapping_add(0x0c));
    let v = c.or(B, v as u16, 0x40);
    c.set_byte(di.wrapping_add(0x0c), v as u8);
    c.set_word(0x4752, di);
    c.ret()
}

/// `loc_0758d`: AL (AH 0) = DX scaled by 255 - 2·byte 17h of the troop SI
/// (/256, 255 on overflow), at most its byte 1Ah. DX kept.
pub fn scaled_by_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x758d)?;
    let mut c = Ctx::new(m);
    let (si, dx) = (c.reg(SI), c.reg(DX));
    let s = c.byte(si.wrapping_add(0x17));
    let cap = c.byte(si.wrapping_add(0x1a));
    let f = 0xffu8.wrapping_sub(s).wrapping_sub(s);
    let prod = dx as u32 * f as u32;
    let over = prod >> 16 != 0;
    let al = if over { 0xff } else { (prod >> 8) as u8 };
    c.run(start, 10 + over as u32 + 3 + (al > cap) as u32 + 1)?;
    c.push(dx);
    c.set_reg(AX, dx);
    let dl = c.sub(B, 0xff, s as u16);
    let dl = c.sub(B, dl, s as u16);
    c.set_reg(DX, dl);
    c.mul16(dl);
    let (ax, hi) = (c.reg(AX), c.reg(DX));
    let mut a = ax >> 8;
    c.xor(B, ax >> 8, ax >> 8);
    c.or(W, hi, hi);
    if hi != 0 {
        a = 0xff;
    }
    let d = c.pop();
    c.set_reg(DX, d);
    c.cmp(B, a, cap as u16);
    if a > cap as u16 {
        a = cap as u16;
    }
    c.set_reg(AX, a);
    c.ret()
}

/// `loc_07ee2`: DI = the first of 7 ranges (word pairs from SI; an end of
/// 0 skips it) with DX at or past its start and below the next word; 7
/// when none. SI after its start word.
pub fn find_range(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7ee2)?;
    let mut c = Ctx::new(m);
    let dx = c.reg(DX);
    let step = c.string_step(W);
    let mut si = c.reg(SI);
    let mut n = 1;
    let mut di = 0;
    loop {
        let ax = c.word(si);
        si = si.wrapping_add(step);
        n += 3;
        if ax != 0 {
            n += 2;
            if dx >= ax {
                n += 2;
                let w = c.word(si);
                if dx < w {
                    n += 1;
                    break;
                }
            }
        }
        si = si.wrapping_add(2);
        di += 1;
        n += 4;
        if di >= 7 {
            n += 1;
            break;
        }
    }
    c.run(start, n)?;
    c.xor(W, 0, 0);
    let mut di = 0u16;
    c.set_reg(DI, 0);
    loop {
        let ax = c.lods(W, DS);
        c.or(W, ax, ax);
        if ax != 0 {
            c.cmp(W, dx, ax);
            if dx >= ax {
                let s = c.reg(SI);
                let w = c.word(s);
                c.cmp(W, dx, w);
                if dx < w {
                    return c.ret();
                }
            }
        }
        let s = c.reg(SI);
        let s = c.add(W, s, 2);
        c.set_reg(SI, s);
        di = c.inc(W, di);
        c.set_reg(DI, di);
        c.cmp(W, di, 7);
        if di >= 7 {
            return c.ret();
        }
    }
}

/// `loc_07f75`: for each bit of byte 19h of the troop SI, from the top,
/// the byte from DI + 14h on one down (not below 0). DI kept, AL 0.
pub fn lower_by_bits(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7f75)?;
    let mut c = Ctx::new(m);
    let (si, di0) = (c.reg(SI), c.reg(DI));
    let al0 = c.byte(si.wrapping_add(0x19));
    let mut n = 5;
    let mut a = al0;
    let mut p = di0.wrapping_add(0x14);
    let mut at80 = a & 0x80 != 0;
    a <<= 1;
    loop {
        if at80 {
            n += 2 + (c.byte(p) == 0) as u32;
        }
        p = p.wrapping_add(1);
        let cf = a & 0x80 != 0;
        a <<= 1;
        n += 3;
        if cf {
            at80 = true;
            continue;
        }
        n += 1;
        if a != 0 {
            at80 = false;
            continue;
        }
        n += 2;
        break;
    }
    c.run(start, n)?;
    c.set_reg8(r8::AL, al0);
    c.push(di0);
    let mut p = c.add(W, di0, 0x14);
    let mut al = c.shl1(B, al0 as u16, 1);
    let mut at80 = c.flag(CF);
    loop {
        if at80 {
            let v = c.byte(p);
            let r = c.sub(B, v as u16, 1);
            c.set_byte(p, r as u8);
            if c.flag(CF) {
                let r = c.inc(B, r);
                c.set_byte(p, r as u8);
            }
        }
        p = c.inc(W, p);
        al = c.shl1(B, al, 1);
        if c.flag(CF) {
            at80 = true;
            continue;
        }
        if al != 0 {
            at80 = false;
            continue;
        }
        break;
    }
    c.set_reg8(r8::AL, al as u8);
    let d = c.pop();
    c.set_reg(DI, d);
    c.ret()
}

/// `loc_0846c`: DI = the location of the troop SI (byte 3 bit 40h: its
/// own, else one `loc_05ed0` accepts) by `loc_06757` with its byte 0, its
/// screen rect then marked dirty (`loc_0c6ad`); DI 0 when there is none.
/// SI kept.
pub fn troop_location_dirty(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x846c)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let b3 = c.byte(si.wrapping_add(3));
    c.run(start, 5)?;
    c.push(si);
    c.test(B, b3 as u16, 0x40);
    let none = |c: &mut Ctx, at: u16| -> Result<(), Bail> {
        c.run(at, 5)?;
        let di = c.reg(DI);
        c.xor(W, di, di);
        c.set_reg(DI, 0);
        let s = c.pop();
        c.set_reg(SI, s);
        c.ret()
    };
    if b3 & 0x40 == 0 {
        let loc = c.word(si.wrapping_add(4));
        c.set_reg(DI, loc);
        c.call(0x5ed0, 0x8479)?;
        if !c.flag(ZF) {
            return none(&mut c, 0x8479);
        }
        c.run(0x8479, 3)?;
    }
    let s = c.reg(SI);
    let al = c.byte(s);
    c.set_reg8(r8::AL, al);
    c.call(0x6757, 0x8480)?;
    if c.flag(CF) {
        return none(&mut c, 0x8480);
    }
    c.run(0x8480, 4)?;
    let di = c.reg(DI);
    c.set_reg(SI, di);
    c.push(di);
    c.call(0xc6ad, 0x8488)?;
    c.run(0x8488, 3)?;
    let d = c.pop();
    c.set_reg(DI, d);
    let s = c.pop();
    c.set_reg(SI, s);
    c.ret()
}

/// `loc_0727d`: bit 4 of byte 0Ah of the location DI cleared; `loc_07289`
/// for each of its troops (`loc_0661d`), CX 0.
pub fn clear_location_4(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x727d)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let di = c.reg(DI);
    let v = c.byte(di.wrapping_add(0x0a));
    let v = c.and(B, v as u16, 0xfb);
    c.set_byte(di.wrapping_add(0x0a), v as u8);
    c.set_reg(BP, 0x7289);
    let cx = c.reg(CX);
    c.xor(W, cx, cx);
    c.set_reg(CX, 0);
    for_each_troop_below_80(c.m, 0x661d)
}

/// `loc_07298`: `loc_072a2` for each troop of the location DI, CX, DX 0.
pub fn for_location_troops_72a2(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7298)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_reg(BP, 0x72a2);
    c.xor(W, 0, 0);
    c.set_reg(CX, 0);
    c.set_reg(DX, 0);
    for_each_troop_below_80(c.m, 0x661d)
}

/// `loc_083fd`: `loc_08403` for each troop of the location DI.
pub fn for_location_troops_8403(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x83fd)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(BP, 0x8403);
    for_each_troop_below_80(c.m, 0x661d)
}

/// `loc_077d7`: the troops redrawn (`loc_068eb`), `loc_07f27` for the
/// troop SI (BX, SI kept); then, when the need [BX + 46FEh] reaches 1 (2
/// for BX 1 at location [1150h]): [3Dh] = its byte 19h, which gets bit
/// 80h >> BL, CF clear; else CF set.
pub fn troop_take_skill(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x77d7)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x68eb, 0x77da)?;
    c.run(0x77da, 4)?;
    let si = c.reg(SI);
    let loc = c.word(si.wrapping_add(4));
    c.set_reg(DI, loc);
    let bx = c.reg(BX);
    c.push(bx);
    c.push(si);
    c.call(0x7f27, 0x77e2)?;
    let si = c.pop();
    c.set_reg(SI, si);
    let bx = c.pop();
    c.set_reg(BX, bx);
    let di = c.reg(DI);
    let at = c.word(0x1150);
    let two = bx == 1 && di == at;
    let al: u8 = 1 + two as u8;
    let need = c.byte(bx.wrapping_add(0x46fe));
    let ok = need >= al;
    c.run(0x77e2, 5 + if bx == 1 { 2 + two as u32 } else { 0 } + 2 + if ok { 7 } else { 0 } + 1)?;
    c.set_reg8(r8::AL, 1);
    c.cmp(W, bx, 1);
    if bx == 1 {
        c.cmp(W, di, at);
        if two {
            c.inc(B, 1);
            c.set_reg8(r8::AL, 2);
        }
    }
    c.cmp(B, need as u16, al as u16);
    if !ok {
        return c.ret();
    }
    let b = c.byte(si.wrapping_add(0x19));
    c.set_byte(0x3d, b);
    c.set_reg8(r8::CL, bx as u8);
    let bit = c.m.cpu.shift(5, B, 0x80, (bx & 0x1f) as u32) as u16;
    c.set_reg8(r8::AL, bit as u8);
    let v = c.or(B, b as u16, bit);
    c.set_byte(si.wrapping_add(0x19), v as u8);
    c.set_flag(CF, false);
    c.ret()
}

/// `loc_07b58`: [1BEAh], [46F4h], [4Ch] cleared, the troop [46EFh] taken
/// (cleared): its state reset (`loc_01ebe`, word 10h & 3F0h, word 12h &
/// E5FFh, byte 14h the day), `loc_0a7a5`, the panel 18E9h redrawn with
/// [0DBE0h], [47BAh] 0, and `loc_07b2b` with AL 4.
pub fn drop_selected_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7b58)?;
    let mut c = Ctx::new(m);
    let sel = c.word(0x46ef);
    if sel == 0 {
        c.run(start, 8)?;
    } else {
        c.run(start, 10)?;
    }
    c.set_word(0x1bea, 0);
    c.set_byte(0x46f4, 0);
    let si = c.reg(SI);
    c.xor(W, si, si);
    c.set_byte(0x4c, 0);
    c.set_word(0x46ef, 0);
    c.set_reg(SI, sel);
    c.or(W, sel, sel);
    if sel == 0 {
        return c.ret();
    }
    let k = c.byte(0x4c);
    c.cmp(B, k as u16, 0);
    c.call(0x1ebe, 0x7b7c)?;
    c.run(0x7b7c, 3)?;
    let si = c.reg(SI);
    let v = c.word(si.wrapping_add(0x10));
    let v = c.and(W, v, 0x3f0);
    c.set_word(si.wrapping_add(0x10), v);
    let v = c.word(si.wrapping_add(0x12));
    let v = c.and(W, v, 0xe5ff);
    c.set_word(si.wrapping_add(0x12), v);
    c.call(0x1ac5, 0x7b89)?;
    c.run(0x7b89, 2)?;
    let si = c.reg(SI);
    let al = c.reg8(r8::AL);
    c.set_byte(si.wrapping_add(0x14), al);
    c.call(0xa7a5, 0x7b8f)?;
    c.run(0x7b8f, 5)?;
    c.set_reg(SI, 0x18e9);
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    c.set_reg(AX, 0);
    c.set_word(0xdbe0, 0);
    c.set_word(0x47ba, 0);
    c.call(0xc6ad, 0x7b9d)?;
    c.run(0x7b9d, 2)?;
    c.set_reg8(r8::AL, 4);
    c.call(0x7b2b, 0x7ba2)?;
    c.run(0x7ba2, 1)?;
    c.ret()
}

/// `loc_083bc`: the troop SI updated (`loc_0851f`, bit 40h of byte 3
/// cleared, `loc_085cc`, `loc_07f5f`); for a troop (byte 3 below 80h),
/// `loc_06ad4` with its activity; at the current location with the
/// hotspot 1F0Eh (`loc_0d41b`) and [0Bh] = 1 (2 with [2Bh] 0), [473Bh]
/// gets bit 1.
pub fn update_troop_83bc(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x83bc)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x851f, 0x83bf)?;
    c.run(0x83bf, 2)?;
    let si = c.reg(SI);
    let v = c.byte(si.wrapping_add(3));
    let v = c.and(B, v as u16, 0xbf);
    c.set_byte(si.wrapping_add(3), v as u8);
    c.call(0x85cc, 0x83c6)?;
    c.run(0x83c6, 1)?;
    c.call(0x7f5f, 0x83c9)?;
    let si = c.reg(SI);
    let cl = c.byte(si.wrapping_add(3));
    if cl & 0x80 != 0 {
        c.run(0x83c9, 4)?;
        c.set_reg8(r8::CL, cl);
        c.or(B, cl as u16, cl as u16);
        return c.ret();
    }
    c.run(0x83c9, 5)?;
    c.set_reg8(r8::CL, cl);
    c.or(B, cl as u16, cl as u16);
    let a = c.and(B, cl as u16, 0x0f);
    c.set_reg8(r8::CL, a as u8);
    c.call(0x6ad4, 0x83d6)?;
    let si = c.reg(SI);
    let di = c.word(si.wrapping_add(4));
    let here = c.word(0x114e);
    if di != here {
        c.run(0x83d6, 4)?;
        c.set_reg(DI, di);
        c.cmp(W, di, here);
        return c.ret();
    }
    c.run(0x83d6, 4)?;
    c.set_reg(DI, di);
    c.cmp(W, di, here);
    c.call(0xd41b, 0x83e2)?;
    let bp = c.reg(BP);
    if bp != 0x1f0e {
        c.run(0x83e2, 3)?;
        c.cmp(W, bp, 0x1f0e);
        return c.ret();
    }
    let b2b = c.byte(0x2b);
    let want = 1 + (b2b < 1) as u8;
    let b0b = c.byte(0x0b);
    let hit = b0b == want;
    c.run(0x83e2, 7 + hit as u32 + 1)?;
    c.cmp(W, bp, 0x1f0e);
    c.cmp(B, b2b as u16, 1);
    let al = c.adc(B, 1, 0);
    c.set_reg8(r8::AL, al as u8);
    c.cmp(B, b0b as u16, al);
    if hit {
        let v = c.byte(0x473b);
        let v = c.or(B, v as u16, 1);
        c.set_byte(0x473b, v as u8);
    }
    c.ret()
}

/// `loc_071bc`: past phase 35h, for a troop with bit 40h of word 12h, one
/// time in 8 (the seed at [0] rotated 3): `loc_0719c`, bit 8000h of word
/// 10h, bit 4 of byte 0Ah of the location DI, and `loc_071b2` with AL 3.
pub fn maybe_mark_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x71bc)?;
    let mut c = Ctx::new(m);
    let ph = c.byte(0x2a);
    if ph < 0x35 {
        c.run(start, 3)?;
        c.cmp(B, ph as u16, 0x35);
        return c.ret();
    }
    let si = c.reg(SI);
    let w = c.word(si.wrapping_add(0x12));
    if w & 0x40 == 0 {
        c.run(start, 5)?;
        c.cmp(B, ph as u16, 0x35);
        c.test(W, w, 0x40);
        return c.ret();
    }
    let s0 = c.word(0);
    let hit = s0.rotate_left(3) & 7 == 0;
    c.run(start, if hit { 10 } else { 10 })?;
    c.cmp(B, ph as u16, 0x35);
    c.test(W, w, 0x40);
    let mut s = s0;
    for _ in 0..3 {
        s = c.rol1(W, s);
        c.set_word(0, s);
    }
    c.test(W, s, 7);
    if !hit {
        return c.ret();
    }
    c.call(0x719c, 0x71e1)?;
    c.run(0x71e1, 4)?;
    let si = c.reg(SI);
    let v = c.word(si.wrapping_add(0x10));
    let v = c.or(W, v, 0x8000);
    c.set_word(si.wrapping_add(0x10), v);
    let di = c.reg(DI);
    let v = c.byte(di.wrapping_add(0x0a));
    let v = c.or(B, v as u16, 4);
    c.set_byte(di.wrapping_add(0x0a), v as u8);
    c.set_reg8(r8::AL, 3);
    call_29f0_f(c.m, 0x71b2)
}

/// `loc_066ce`: a troop SI (byte 3 bit 80h) without bit 80h of byte 10h
/// joins: [28h] counted (at [1178h] or more, `loc_0121f` with AL 4Ch),
/// charisma up 1, byte 3 to its bit 20h plus 2, `stamp_record`, byte 14h
/// the day; its location gets byte 0Bh 2 (`loc_0644e`) when that was 0.
pub fn troop_joins(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x66ce)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let b3 = c.byte(si.wrapping_add(3));
    if b3 & 0x80 == 0 {
        c.run(start, 3)?;
        c.test(B, b3 as u16, 0x80);
        return c.ret();
    }
    let b10 = c.byte(si.wrapping_add(0x10));
    if b10 & 0x80 != 0 {
        c.run(start, 5)?;
        c.test(B, b3 as u16, 0x80);
        c.test(B, b10 as u16, 0x80);
        return c.ret();
    }
    let k = c.byte(0x28).wrapping_add(1);
    let lim = c.byte(0x1178);
    let over = k >= lim;
    c.run(start, if over { 11 } else { 10 })?;
    c.test(B, b3 as u16, 0x80);
    c.test(B, b10 as u16, 0x80);
    let v = c.inc(B, k.wrapping_sub(1) as u16);
    c.set_byte(0x28, v as u8);
    c.set_reg8(r8::AL, k);
    c.cmp(B, k as u16, lim as u16);
    if over {
        c.push(si);
        c.set_reg8(r8::AL, 0x4c);
        c.call(0x121f, 0x66ed)?;
        c.run(0x66ed, 3)?;
        let s = c.pop();
        c.set_reg(SI, s);
    }
    c.set_reg8(r8::AL, 1);
    c.call(0x6f78, 0x66f3)?;
    c.run(0x66f3, 3)?;
    let si = c.reg(SI);
    let v = c.byte(si.wrapping_add(3));
    let v = c.and(B, v as u16, 0x20);
    let v = c.or(B, v, 2);
    c.set_byte(si.wrapping_add(3), v as u8);
    c.call(0x6b25, 0x66fe)?;
    c.run(0x66fe, 1)?;
    c.call(0x1ac5, 0x6701)?;
    let si = c.reg(SI);
    let di = c.word(si.wrapping_add(4));
    let b = c.byte(di.wrapping_add(0x0b));
    c.run(0x6701, if b != 0 { 5 } else { 6 })?;
    let al = c.reg8(r8::AL);
    c.set_byte(si.wrapping_add(0x14), al);
    c.set_reg(DI, di);
    c.cmp(B, b as u16, 0);
    if b != 0 {
        return c.ret();
    }
    c.set_byte(di.wrapping_add(0x0b), 2);
    c.call(0x644e, 0x6714)?;
    c.run(0x6714, 1)?;
    c.ret()
}

/// `loc_06d7b`: once in 40h ticks (word [2]), the troop SI's skills
/// (bytes 18h, 17h, 16h) down 1 (not below 0) for the bits of word 12h
/// that its activity's pair (C000h rotated by `loc_0693b`) selects.
pub fn troop_skills_decay(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6d7b)?;
    let mut c = Ctx::new(m);
    let t = c.word(2);
    if t & 0x3f != 0 {
        c.run(start, 3)?;
        c.test(W, t, 0x3f);
        return c.ret();
    }
    c.run(start, 3)?;
    c.test(W, t, 0x3f);
    c.call(0x693b, 0x6d87)?;
    let si = c.reg(SI);
    let cl = c.reg8(r8::AL);
    let w12 = c.word(si.wrapping_add(0x12));
    let a = 0xc000u16.rotate_left((cl & 0x1f) as u32 % 16) & w12;
    let mut n = 5;
    if a == 0 {
        n += 1;
    } else {
        for (i, off) in [0x18u16, 0x17, 0x16].into_iter().enumerate() {
            n += 2;
            if a & (0x8000 >> i) != 0 {
                n += 2;
                if c.byte(si.wrapping_add(off)) != 0 {
                    n += 1;
                }
            }
        }
        n += 1;
    }
    c.run(0x6d87, n)?;
    c.set_reg8(r8::CL, cl);
    let r = c.m.cpu.shift(0, W, 0xc000, (cl & 0x1f) as u32) as u16;
    let mut ax = c.and(W, r, w12);
    c.set_reg(AX, ax);
    if ax == 0 {
        return c.ret();
    }
    for off in [0x18u16, 0x17, 0x16] {
        ax = c.shl1(W, ax, 1);
        if c.flag(CF) {
            let v = c.byte(si.wrapping_add(off));
            c.cmp(B, v as u16, 0);
            if v != 0 {
                let v = c.dec(B, v as u16);
                c.set_byte(si.wrapping_add(off), v as u8);
            }
        }
    }
    c.set_reg(AX, ax);
    c.ret()
}

/// `loc_07f27`: [`skill_needs`] into 46FEh.
pub fn skill_needs_46fe(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7f27)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.set_reg(BX, 0x46fe);
    skill_needs_at(&mut c)
}

/// `loc_07f2a`: the 7 skill needs of the location DI (bytes 14h on)
/// copied to BX, then lowered by each of its troops' skills (byte 19h,
/// one bit per need). DI kept, ES = DS.
pub fn skill_needs(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7f2a)?;
    let mut c = Ctx::new(m);
    skill_needs_at(&mut c)
}

fn skill_needs_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x7f2a, 8)?;
    let (di, bx) = (c.reg(DI), c.reg(BX));
    c.push(di);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    let mut al = c.byte(di.wrapping_add(9));
    c.set_reg8(r8::AL, al);
    c.set_reg(SI, di.wrapping_add(0x14));
    c.set_reg(DI, bx);
    c.set_reg(CX, 7);
    c.rep(0xa4);
    loop {
        if al == 0 {
            c.run(0x7f3a, 4)?;
            c.or(B, 0, 0);
            let d = c.pop();
            c.set_reg(DI, d);
            return c.ret();
        }
        c.run(0x7f3a, 3)?;
        c.or(B, al as u16, al as u16);
        c.call(0x6906, 0x7f41)?;
        let (si, bx) = (c.reg(SI), c.reg(BX));
        let a0 = c.byte(si.wrapping_add(0x19));
        let mut n = 4;
        let mut a = a0;
        let mut p = bx;
        let mut at80 = a & 0x80 != 0;
        a <<= 1;
        loop {
            if at80 {
                n += 2 + (c.byte(p) == 0) as u32;
            }
            p = p.wrapping_add(1);
            let cf = a & 0x80 != 0;
            a <<= 1;
            n += 3;
            if cf {
                at80 = true;
                continue;
            }
            n += 1;
            if a != 0 {
                at80 = false;
                continue;
            }
            break;
        }
        c.run(0x7f41, n + 2)?;
        c.set_reg8(r8::AL, a0);
        let mut p = bx;
        let mut v = c.shl1(B, a0 as u16, 1);
        let mut at80 = c.flag(CF);
        loop {
            if at80 {
                let b = c.byte(p);
                let r = c.sub(B, b as u16, 1);
                c.set_byte(p, r as u8);
                if c.flag(CF) {
                    let r = c.inc(B, r);
                    c.set_byte(p, r as u8);
                }
            }
            p = c.inc(W, p);
            v = c.shl1(B, v, 1);
            if c.flag(CF) {
                at80 = true;
                continue;
            }
            if v != 0 {
                at80 = false;
                continue;
            }
            break;
        }
        c.set_reg(DI, p);
        al = c.byte(si.wrapping_add(1));
        c.set_reg8(r8::AL, al);
    }
}

/// `loc_085cc`: for a troop SI (word 10h without bit 80h, byte 2 past 8):
/// `loc_0858c`, then the first troop of the location DI without bit 80h
/// of byte 10h and with bit 20h of byte 3 reset (`reset_troop`,
/// registers kept); `loc_0851f` for SI.
pub fn reset_location_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x85cc)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let w10 = c.word(si.wrapping_add(0x10));
    if w10 & 0x80 != 0 {
        c.run(start, 3)?;
        c.test(W, w10, 0x80);
        return c.ret();
    }
    let b2 = c.byte(si.wrapping_add(2));
    c.run(start, 5)?;
    c.test(W, w10, 0x80);
    c.cmp(B, b2 as u16, 8);
    if b2 <= 8 {
        return c.ret();
    }
    c.call(0x858c, 0x85dc)?;
    c.run(0x85dc, 2)?;
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.push(si);
    let mut al = c.byte(di.wrapping_add(9));
    c.set_reg8(r8::AL, al);
    loop {
        if al == 0 {
            c.run(0x85e0, 4)?;
            c.or(B, 0, 0);
            break;
        }
        c.run(0x85e0, 3)?;
        c.or(B, al as u16, al as u16);
        c.call(0x6906, 0x85e7)?;
        let si = c.reg(SI);
        al = c.byte(si.wrapping_add(1));
        let b10 = c.byte(si.wrapping_add(0x10));
        let b3 = c.byte(si.wrapping_add(3));
        if b10 & 0x80 != 0 {
            c.run(0x85e7, 3)?;
            c.set_reg8(r8::AL, al);
            c.test(B, b10 as u16, 0x80);
            continue;
        }
        if b3 & 0x20 == 0 {
            c.run(0x85e7, 5)?;
            c.set_reg8(r8::AL, al);
            c.test(B, b10 as u16, 0x80);
            c.test(B, b3 as u16, 0x20);
            continue;
        }
        c.run(0x85e7, 6)?;
        c.set_reg8(r8::AL, al);
        c.test(B, b10 as u16, 0x80);
        c.test(B, b3 as u16, 0x20);
        c.call(code::SAVE_REGS, 0x85f9)?;
        c.run(0x85f9, 1)?;
        c.call(0x66b1, 0x85fc)?;
        c.run(0x85fc, 1)?;
        c.call(code::RESTORE_REGS, 0x85ff)?;
        c.run(0x85ff, 2)?;
        break;
    }
    let s = c.pop();
    c.set_reg(SI, s);
    c.call(0x851f, 0x8603)?;
    c.run(0x8603, 1)?;
    c.ret()
}

/// `loc_07c63`: AX = the larger of the troop SI's distance to the map
/// position (`get_map_position`, SI kept): |word 6 - DX| / the scale
/// [4880h + 2·|BX|] and |BX - word 8|.
pub fn troop_map_distance(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7c63)?;
    map_distance(m, start, 6)
}

/// `loc_07c8f`: [`troop_map_distance`] for the record SI with its place
/// in words 2 and 4.
pub fn record_map_distance(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7c8f)?;
    map_distance(m, start, 2)
}

/// The distance of `loc_07c63` / `loc_07c8f` (same code) for the place in
/// words `off`, `off` + 2 of SI.
fn map_distance(m: &mut Machine, start: u16, off: u16) -> Result<(), Bail> {
    let mut c = Ctx::new(m);
    let back = start.wrapping_add(4);
    c.run(start, 2)?;
    let si = c.reg(SI);
    c.push(si);
    c.call(0x407e, back)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    let si = c.word_in(SS, c.reg(SP));
    let b2 = bx.wrapping_shl(1);
    let neg_b = b2 & 0x8000 != 0;
    let idx = if neg_b { b2.wrapping_neg() } else { b2 };
    let bp = c.word_in(SS, idx.wrapping_add(0x4880));
    if bp == 0 {
        c.m.cpu.ip = back;
        return Err(Bail);
    }
    let w6 = c.word(si.wrapping_add(off));
    let d = w6.wrapping_sub(dx);
    let neg_d = d & 0x8000 != 0;
    let num = if neg_d { d.wrapping_neg() } else { d };
    let q = num / bp;
    let w8 = c.word(si.wrapping_add(off + 2));
    let e = bx.wrapping_sub(w8);
    let neg_e = e & 0x8000 != 0;
    let ab = if neg_e { e.wrapping_neg() } else { e };
    let n = 4 + neg_b as u32 + 1 + 3 + neg_d as u32 + 2 + 2 + neg_e as u32 + 2 + (q < ab) as u32 + 1;
    c.run(back, n)?;
    let si = c.pop();
    c.set_reg(SI, si);
    let p = c.shl1(W, bx, 1);
    let p = if neg_b { c.neg(W, p) } else { p };
    c.set_reg(BP, bp);
    let _ = p;
    let a = c.sub(W, w6, dx);
    let a = if neg_d { c.neg(W, a) } else { a };
    c.set_reg(AX, a);
    c.xor(W, dx, dx);
    c.set_reg(DX, 0);
    c.m.cpu.mul_div(&mut c.m.hw, W, 6, bp as u32);
    let b = c.sub(W, bx, w8);
    let b = if neg_e { c.neg(W, b) } else { b };
    c.set_reg(BX, b);
    let ax = c.reg(AX);
    c.cmp(W, ax, b);
    if ax < b {
        c.set_reg(AX, b);
    }
    c.ret()
}

/// `loc_08461`: for the troop SI with its particle (`loc_06917`, ZF):
/// the particle removed (SI kept), then [`troop_location_dirty`].
pub fn drop_troop_particle(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8461)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x6917, 0x8464)?;
    if !c.flag(ZF) {
        c.run(0x8464, 2)?;
        return c.ret();
    }
    c.run(0x8464, 3)?;
    let si = c.reg(SI);
    c.push(si);
    c.call(code::PARTICLES_REMOVE_PARTICLE, 0x846b)?;
    c.run(0x846b, 1)?;
    let si = c.pop();
    c.set_reg(SI, si);
    troop_location_dirty(c.m, 0x846c)
}

/// `loc_06715`: `troop_6757` for every troop of the locations listed at
/// SS:A5C0h (6 bytes each, to a 0), then for every troop from 8AAh on
/// with bit 40h of byte 3 and without bit 10h of byte 10h.
pub fn refresh_all_troops(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6715)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let mut bp = 0xa5c0u16;
    c.set_reg(BP, bp);
    loop {
        let di = c.word_in(SS, bp);
        if di == 0 {
            c.run(0x6718, 4)?;
            c.set_reg(DI, 0);
            c.or(W, 0, 0);
            break;
        }
        let al = c.byte(di.wrapping_add(9));
        c.run(0x6718, 8)?;
        c.set_reg(DI, di);
        c.or(W, di, di);
        c.set_reg8(r8::AL, al);
        c.or(B, al as u16, al as u16);
        if al == 0 {
            bp = c.add(W, bp, 6);
            c.set_reg(BP, bp);
            continue;
        }
        c.push(bp);
        c.call(0x6757, 0x672a)?;
        loop {
            let si = c.reg(SI);
            let al = c.byte(si.wrapping_add(1));
            c.run(0x672a, 6)?;
            bp = c.pop();
            c.set_reg(BP, bp);
            c.set_reg8(r8::AL, al);
            c.or(B, al as u16, al as u16);
            if al != 0 {
                c.push(bp);
                c.call(0x6757, 0x672a)?;
                continue;
            }
            bp = c.add(W, bp, 6);
            c.set_reg(BP, bp);
            break;
        }
    }
    let mut si = 0x88fu16;
    c.set_reg(SI, si);
    loop {
        let s2 = si.wrapping_add(0x1b);
        if s2 >= 0xfbb {
            c.run(0x673a, 4)?;
            let s = c.add(W, si, 0x1b);
            c.set_reg(SI, s);
            c.cmp(W, s, 0xfbb);
            return c.ret();
        }
        let b10 = c.byte(s2.wrapping_add(0x10));
        let b3 = c.byte(s2.wrapping_add(3));
        if b10 & 0x10 != 0 {
            c.run(0x673a, 5)?;
        } else if b3 & 0x40 == 0 {
            c.run(0x673a, 7)?;
        } else {
            c.run(0x673a, 9)?;
        }
        let s = c.add(W, si, 0x1b);
        c.set_reg(SI, s);
        c.cmp(W, s, 0xfbb);
        c.test(B, b10 as u16, 0x10);
        si = s;
        if b10 & 0x10 != 0 {
            continue;
        }
        c.test(B, b3 as u16, 0x40);
        if b3 & 0x40 == 0 {
            continue;
        }
        let al = c.byte(s);
        c.set_reg8(r8::AL, al);
        c.call(0x6757, 0x6754)?;
        c.run(0x6754, 1)?;
        si = c.reg(SI);
    }
}

/// `loc_06d19`: a plain troop SI (no bits E3h of byte 3, no bit 80h of
/// byte 10h, not 8E0h) merges into the troop BX that `loc_06d5f` picks
/// among those of its location (CL the complement of its size, DX it):
/// size added, skills shared, bit 200h of word 12h; SI reset
/// (`reset_troop`), CF set.
pub fn merge_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6d19)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let b3 = c.byte(si.wrapping_add(3));
    let b10 = c.byte(si.wrapping_add(0x10));
    if b3 & 0xe3 != 0 {
        c.run(start, 3)?;
        c.test(B, b3 as u16, 0xe3);
        return c.ret();
    }
    if b10 & 0x80 != 0 {
        c.run(start, 5)?;
        c.test(B, b3 as u16, 0xe3);
        c.test(B, b10 as u16, 0x80);
        return c.ret();
    }
    if si == 0x8e0 {
        c.run(start, 7)?;
        c.test(B, b3 as u16, 0xe3);
        c.test(B, b10 as u16, 0x80);
        c.cmp(W, si, 0x8e0);
        return c.ret();
    }
    c.run(start, 13)?;
    c.test(B, b3 as u16, 0xe3);
    c.test(B, b10 as u16, 0x80);
    c.cmp(W, si, 0x8e0);
    let di = c.word(si.wrapping_add(4));
    c.set_reg(DI, di);
    let bx = c.reg(BX);
    c.xor(W, bx, bx);
    c.set_reg(BX, 0);
    let cl = !c.byte(si.wrapping_add(0x1a));
    c.set_reg8(r8::CL, cl);
    c.set_reg(DX, si);
    c.set_reg(BP, 0x6d5f);
    c.call(0x661d, 0x6d3d)?;
    let bx = c.reg(BX);
    if bx == 0 {
        c.run(0x6d3d, 3)?;
        c.or(W, 0, 0);
        return c.ret();
    }
    c.run(0x6d3d, 11)?;
    c.or(W, bx, bx);
    let si = c.reg(SI);
    let a = c.byte(si.wrapping_add(0x1a));
    let t = c.byte(bx.wrapping_add(0x1a));
    let t = c.add(B, t as u16, a as u16);
    c.set_byte(bx.wrapping_add(0x1a), t as u8);
    let a = c.byte(si.wrapping_add(0x19));
    let o = c.byte(bx.wrapping_add(0x19));
    let v = c.and(B, a as u16, o as u16);
    c.set_reg(AX, (a as u16) << 8 | v);
    c.set_byte(si.wrapping_add(0x19), v as u8);
    let o = c.byte(bx.wrapping_add(0x19));
    let o = c.or(B, o as u16, a as u16);
    c.set_byte(bx.wrapping_add(0x19), o as u8);
    let w = c.word(bx.wrapping_add(0x12));
    let w = c.or(W, w, 0x200);
    c.set_word(bx.wrapping_add(0x12), w);
    c.call(0x66b1, 0x6d5d)?;
    c.run(0x6d5d, 2)?;
    c.set_flag(CF, true);
    c.ret()
}

/// `loc_079de`: with [46FAh] set (cleared), the panel 18DFh redrawn on
/// the front buffer ([0DBE0h] 0, `loc_0c6ad`) and `loc_07b2b` with AL 8;
/// else only the front buffer made active.
pub fn redraw_panel_18df(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x79de)?;
    let mut c = Ctx::new(m);
    let old = c.word(0x46fa);
    if old == 0 {
        c.run(start, 5)?;
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
        c.or(W, 0, 0);
        return super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c);
    }
    c.run(start, 7)?;
    c.xor(W, 0, 0);
    c.set_word(0x46fa, 0);
    c.set_reg(AX, old);
    c.or(W, old, old);
    c.set_reg(SI, 0x18df);
    c.call(0xc07c, 0x5fa2)?;
    c.run(0x5fa2, 2)?;
    c.set_word(0xdbe0, 0);
    c.call(0xc6ad, 0x5fab)?;
    c.run(0x5fab, 2)?;
    c.set_reg8(r8::AL, 8);
    gfx_call_30_unless_redrawing(c.m, 0x7b2b)
}

/// `loc_06bd7`: [`set_troop_bit_10`] with CF set for a troop with bits
/// 30h of word 12h, at a location with byte 1Bh 0 or without bit 20h of
/// byte 0Ah, or with bit 2 of its byte 19h clear.
pub fn troop_bit_10_by_skill(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6bd7)?;
    let mut c = Ctx::new(m);
    let (si, di) = (c.reg(SI), c.reg(DI));
    let w12 = c.word(si.wrapping_add(0x12));
    let b1b = c.byte(di.wrapping_add(0x1b));
    let b0a = c.byte(di.wrapping_add(0x0a));
    let n = if w12 & 0x30 != 0 {
        3
    } else if b1b < 1 {
        5
    } else if b0a & 0x20 == 0 {
        8
    } else {
        12
    };
    c.run(start, n)?;
    c.test(W, w12, 0x30);
    c.set_flag(CF, true);
    if n > 3 {
        c.cmp(B, b1b as u16, 1);
        if n > 5 {
            c.test(B, b0a as u16, 0x20);
            c.set_flag(CF, true);
            if n > 8 {
                let b = c.byte(si.wrapping_add(0x19));
                let a = c.and(B, b as u16, 2);
                c.set_reg8(r8::AL, a as u8);
                c.cmp(B, a, 1);
            }
        }
    }
    set_troop_bit_10(&mut c)
}

/// `loc_08018`: the skill needs at 4C60h lowered (not below 0) by each
/// troop from 8AAh on at its own location (bit 40h, bits 3 of byte 3) =
/// DI, moving (word 0Ch not DI) with byte 0Eh = BL (the need BX); at the
/// location [1150h] before phase 50h also need 1.
pub fn lower_needs_by_movers(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8018)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let (di, bx) = (c.reg(DI), c.reg(BX));
    let bl = bx as u8;
    let mut si = 0x88fu16;
    c.set_reg(SI, si);
    loop {
        let s = si.wrapping_add(0x1b);
        if s >= 0xfbb {
            c.run(0x801b, 3)?;
            let s = c.add(W, si, 0x1b);
            c.set_reg(SI, s);
            c.cmp(W, s, 0xfbb);
            break;
        }
        let b3 = c.byte(s.wrapping_add(3));
        let w4 = c.word(s.wrapping_add(4));
        let wc = c.word(s.wrapping_add(0x0c));
        let be = c.byte(s.wrapping_add(0x0e));
        let p = bx.wrapping_add(0x4c60);
        let need = c.byte(p);
        let mut n = 3 + 3;
        let stage = if b3 & 0x40 == 0 {
            0
        } else if b3 & 3 != 3 {
            n += 3;
            1
        } else if di != w4 {
            n += 5;
            2
        } else if di == wc {
            n += 7;
            3
        } else if bl != be {
            n += 9;
            4
        } else {
            n += 9 + 2 + if need.wrapping_sub(1) & 0x80 != 0 { 2 } else { 0 };
            5
        };
        c.run(0x801b, n)?;
        let s2 = c.add(W, si, 0x1b);
        c.set_reg(SI, s2);
        c.cmp(W, s2, 0xfbb);
        c.set_reg8(r8::AL, b3);
        c.test(B, b3 as u16, 0x40);
        si = s2;
        if stage == 0 {
            continue;
        }
        let a = c.and(B, b3 as u16, 3);
        c.set_reg8(r8::AL, a as u8);
        c.cmp(B, a, 3);
        if stage == 1 {
            continue;
        }
        c.cmp(W, di, w4);
        if stage == 2 {
            continue;
        }
        c.cmp(W, di, wc);
        if stage == 3 {
            continue;
        }
        c.cmp(B, bl as u16, be as u16);
        if stage == 4 {
            continue;
        }
        let v = c.dec(B, need as u16);
        c.set_byte(p, v as u8);
        if v & 0x80 != 0 {
            let v = c.inc(B, v);
            c.set_byte(p, v as u8);
        }
    }
    let at = c.word(0x1150);
    if di != at {
        c.run(0x804c, 3)?;
        c.cmp(W, di, at);
        return c.ret();
    }
    let ph = c.byte(0x2a);
    if ph >= 0x50 {
        c.run(0x804c, 5)?;
        c.cmp(W, di, at);
        c.cmp(B, ph as u16, 0x50);
        return c.ret();
    }
    let v = c.byte(0x4c61);
    let neg = v.wrapping_sub(1) & 0x80 != 0;
    c.run(0x804c, 7 + neg as u32 + 1)?;
    c.cmp(W, di, at);
    c.cmp(B, ph as u16, 0x50);
    let r = c.dec(B, v as u16);
    c.set_byte(0x4c61, r as u8);
    if neg {
        let r = c.inc(B, r);
        c.set_byte(0x4c61, r as u8);
    }
    c.ret()
}

/// `loc_0708a`: the troop SI's production rate into word 0Ch: its
/// motivation (`troop_motivation`) + the top of byte 16h, times its size
/// (byte 1Ah; /4 without bit 80h of byte 19h), times the top of byte
/// 12h of the location DI + 1, /128 (9 bits). When it changes, word 10h
/// gets 8 (down) or 4 (up) in its bits Ch.
pub fn troop_rate(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x708a)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x6efd, 0x708d)?;
    let (si, di) = (c.reg(SI), c.reg(DI));
    let al0 = c.reg8(r8::AL);
    let b16 = c.byte(si.wrapping_add(0x16));
    let b1a = c.byte(si.wrapping_add(0x1a));
    let b19 = c.byte(si.wrapping_add(0x19));
    let b12 = c.byte(di.wrapping_add(0x12));
    let old = c.word(si.wrapping_add(0x0c));
    let a = al0.wrapping_add(b16 & 0xf0);
    let mut ax = a as u16 * b1a as u16;
    let quarter = b19 & 0x80 == 0;
    if quarter {
        ax >>= 2;
    }
    let ax2 = (ax & 0xff00 | (b12 & 0xf0) as u16).wrapping_add(1);
    let p = (ax2 & 0xff) * (ax2 >> 8);
    let p = p.swap_bytes().rotate_left(1) & 0x01ff;
    let changed = old != p;
    let down = old < p;
    c.run(0x708d, 6 + if quarter { 2 } else { 0 } + 11 + if changed { 2 + (!down) as u32 + 4 } else { 0 } + 1)?;
    let ah = c.and(B, b16 as u16, 0xf0);
    let al = c.add(B, al0 as u16, ah);
    c.set_reg(AX, ah << 8 | al);
    c.mul8(b1a);
    c.test(B, b19 as u16, 0x80);
    let mut ax = c.reg(AX);
    if quarter {
        ax = c.shr1(W, ax, 2);
    }
    let al = c.and(B, b12 as u16, 0xf0);
    let ax = c.inc(W, ax & 0xff00 | al);
    c.set_reg(AX, ax);
    c.mul8((ax >> 8) as u8);
    let ax = c.reg(AX).swap_bytes();
    let ax = c.rol1(W, ax);
    let ah = c.and(B, ax >> 8, 1);
    let ax = ah << 8 | ax & 0xff;
    c.set_reg(AX, ax);
    c.set_word(si.wrapping_add(0x0c), ax);
    let d = c.sub(W, old, ax);
    c.set_reg(DX, d);
    if !changed {
        return c.ret();
    }
    let bl = if down { 8 } else { 4 };
    c.set_reg8(r8::BL, bl);
    let cx = c.word(si.wrapping_add(0x10));
    let cl = c.and(B, cx & 0xff, 0xf3);
    let cl = c.or(B, cl, bl as u16);
    let cx = cx & 0xff00 | cl;
    c.set_reg(CX, cx);
    c.set_word(si.wrapping_add(0x10), cx);
    c.ret()
}

/// `loc_06827`: BP = the sprite for the troop SI facing its location (the
/// direction of the larger of the two map deltas, by `loc_0693b`'s
/// group, from the table at SS:18BFh). BX, DX, DI kept.
pub fn troop_facing_sprite(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6827)?;
    let mut c = Ctx::new(m);
    let (si, bx0, dx0, di0) = (c.reg(SI), c.reg(BX), c.reg(DX), c.reg(DI));
    let loc = c.word(si.wrapping_add(4));
    let (lx, ly) = (c.word(loc.wrapping_add(2)), c.word(loc.wrapping_add(4)));
    let (tx, ty) = (c.word(si.wrapping_add(6)), c.word(si.wrapping_add(8)));
    let dxv = lx.wrapping_sub(tx);
    let s1 = dxv & 0x8000 != 0;
    let dx = (dxv >> 8) as u8 as i8 as i16 as u16;
    let ady = if s1 { dx.wrapping_neg() } else { dx };
    let by = ly.wrapping_sub(ty);
    let s2 = by & 0x8000 != 0;
    let acx = if s2 { by.wrapping_neg() } else { by };
    let below = ady < acx;
    let (bx, _) = if below { (by, dx) } else { (dx, by) };
    let s3 = bx & 0x8000 != 0;
    c.run(start, 12 + s1 as u32 + 3 + s2 as u32 + 3 + if below { 0 } else { 2 } + 2 + s3 as u32 + 1)?;
    c.push(bx0);
    c.push(dx0);
    c.push(di0);
    c.set_reg(DI, loc);
    let a = c.sub(W, lx, tx);
    c.set_reg(AX, dx);
    let _ = a;
    c.set_reg(DX, dx);
    let mut di = dx;
    if s1 {
        di = c.neg(W, di);
    }
    c.set_reg(DI, di);
    let b = c.sub(W, ly, ty);
    let mut cx = b;
    if s2 {
        cx = c.neg(W, cx);
    }
    c.set_reg(CX, cx);
    let mut bp = 2u16;
    c.cmp(W, di, cx);
    let (mut bxr, mut dxr) = (b, dx);
    if !below {
        bp = c.dec(W, bp);
        std::mem::swap(&mut bxr, &mut dxr);
    }
    c.set_reg(BX, bxr);
    c.set_reg(DX, dxr);
    c.or(W, bxr, bxr);
    if s3 {
        bp = c.xor(W, bp, 2);
    }
    c.set_reg(BP, bp);
    c.call(0x693b, 0x685e)?;
    c.run(0x685e, 9)?;
    let ax = c.reg(AX);
    let ax = c.shl1(W, ax, 2);
    c.set_reg(AX, ax);
    let bp = c.reg(BP);
    let bp = c.add(W, bp, ax);
    let bp = c.shl1(W, bp, 1);
    let v = c.word_in(SS, bp.wrapping_add(0x18bf));
    c.set_reg(BP, v);
    let d = c.pop();
    c.set_reg(DI, d);
    let d = c.pop();
    c.set_reg(DX, d);
    let b = c.pop();
    c.set_reg(BX, b);
    c.ret()
}

/// `loc_06639`: BP for each troop of the location DI (`each_troop`),
/// then for each troop from 8AAh with bit 40h of byte 3 at DI (its word
/// 4, or word 0Ch for those moving: bits 3 of byte 3), with flags of
/// byte 3 against 80h. SI kept.
pub fn for_troops_at_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6639)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::FOR_EACH_TROOP, 0x663c)?;
    c.run(0x663c, 2)?;
    let si = c.reg(SI);
    c.push(si);
    let mut si = 0x8aau16;
    c.set_reg(SI, si);
    loop {
        let al = c.byte(si.wrapping_add(3));
        let di = c.reg(DI);
        let mut hit = false;
        if al & 0x40 == 0 {
            c.run(0x6640, 3)?;
            c.set_reg8(r8::AL, al);
            c.test(B, al as u16, 0x40);
        } else {
            let moving = al & 3 == 3;
            let place = if moving { c.word(si.wrapping_add(0x0c)) } else { c.word(si.wrapping_add(4)) };
            hit = place == di;
            c.run(0x6640, 3 + 5 + moving as u32 + 3 + if hit { 5 } else { 0 })?;
            c.set_reg8(r8::AL, al);
            c.test(B, al as u16, 0x40);
            let bx = c.reg(BX);
            c.push(bx);
            let a = c.and(B, al as u16, 3);
            c.set_reg8(r8::AL, a as u8);
            c.cmp(B, a, 3);
            c.cmp(W, place, di);
            let bx = c.pop();
            c.set_reg(BX, bx);
            if hit {
                let bp = c.reg(BP);
                c.push(si);
                c.push(di);
                c.push(bp);
                c.cmp(B, al as u16, 0x80);
                c.call(bp, 0x6662)?;
                c.run(0x6662, 3)?;
                let bp = c.pop();
                c.set_reg(BP, bp);
                let d = c.pop();
                c.set_reg(DI, d);
                si = c.pop();
                c.set_reg(SI, si);
            }
        }
        let _ = hit;
        let s = si.wrapping_add(0x1b);
        let more = s < 0xfbb;
        c.run(0x6665, 3 + if more { 0 } else { 2 })?;
        let s = c.add(W, si, 0x1b);
        c.set_reg(SI, s);
        c.cmp(W, s, 0xfbb);
        si = s;
        if !more {
            let s = c.pop();
            c.set_reg(SI, s);
            return c.ret();
        }
    }
}

/// `loc_0858c`: the troop SI taken out of its location's troop chain (the
/// location's byte 9 or the previous troop's byte 1 skips it); its byte 1
/// cleared. BP = SI; DI kept.
pub fn unlink_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x858c)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let al = c.byte(si);
    let di = c.word(si.wrapping_add(4));
    let b3 = c.byte(si.wrapping_add(3));
    c.run(start, 6)?;
    c.set_reg(BP, si);
    c.set_reg8(r8::AL, al);
    c.set_reg(DI, di);
    c.push(di);
    c.test(B, b3 as u16, 0x40);
    let tail = |c: &mut Ctx, n: u32| -> Result<(), Bail> {
        c.run(0x85ba, n)?;
        let bp = c.reg(BP);
        c.set_reg(SI, bp);
        c.set_byte(bp.wrapping_add(1), 0);
        let d = c.pop();
        c.set_reg(DI, d);
        c.ret()
    };
    if b3 & 0x40 != 0 {
        return tail(&mut c, 4);
    }
    let first = c.byte(di.wrapping_add(9));
    if first == al {
        c.run(0x859a, 7)?;
        c.cmp(B, al as u16, first as u16);
        c.xor(B, 0, 0);
        let next = c.byte(si.wrapping_add(1));
        c.set_byte(si.wrapping_add(1), 0);
        c.set_reg8(r8::AH, next);
        let d = c.pop();
        c.set_reg(DI, d);
        c.set_byte(d.wrapping_add(9), next);
        return c.ret();
    }
    c.run(0x859a, 5)?;
    c.cmp(B, al as u16, first as u16);
    c.set_reg8(r8::CL, al);
    c.set_reg8(r8::AL, first);
    loop {
        c.call(0x6906, 0x85a7)?;
        let s = c.reg(SI);
        let nx = c.byte(s.wrapping_add(1));
        if nx == 0 {
            c.run(0x85a7, 4)?;
            c.set_reg8(r8::AL, 0);
            c.set_reg(DI, s);
            c.or(B, 0, 0);
            return tail(&mut c, 4);
        }
        if nx != al {
            c.run(0x85a7, 7)?;
            c.set_reg8(r8::AL, nx);
            c.set_reg(DI, s);
            c.or(B, nx as u16, nx as u16);
            c.cmp(B, nx as u16, al as u16);
            continue;
        }
        c.run(0x85a7, 8)?;
        c.set_reg8(r8::AL, nx);
        c.set_reg(DI, s);
        c.or(B, nx as u16, nx as u16);
        c.cmp(B, nx as u16, al as u16);
        let bp = c.reg(BP);
        let ah = c.byte_in(SS, bp.wrapping_add(1));
        c.set_reg8(r8::AH, ah);
        c.set_byte(s.wrapping_add(1), ah);
        return tail(&mut c, 4);
    }
}

/// `loc_07847`: the troop SI's orders menu (the item bits at 2110h..):
/// all on, then off for what it cannot do now (by activity, phase,
/// bit 200h of word 10h, its location's sietch state, and whether a
/// skill is still needed there by `skill_needs_46fe`).
pub fn troop_orders_menu(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7847)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let w12 = c.word(si.wrapping_add(0x12));
    if w12 & 0x400 != 0 {
        c.run(start, 6)?;
        menu_start(&mut c);
        c.test(W, w12, 0x400);
        return c.ret();
    }
    let al = c.byte(si.wrapping_add(3)) & 0x0f;
    let ph = c.byte(0x2a);
    if al == 2 {
        c.run(start, 5 + 4 + 1 + 4)?;
        menu_start(&mut c);
        c.test(W, w12, 0x400);
        let b = c.byte(si.wrapping_add(3));
        c.set_reg8(r8::AL, b);
        let a = c.and(B, b as u16, 0x0f);
        c.set_reg8(r8::AL, a as u8);
        c.cmp(B, a, 1);
        clear_bit_40(&mut c, 0x2111);
        c.cmp(B, a, 2);
        c.set_byte(0x2110, 0x56);
        return c.ret();
    }
    let w10 = c.word(si.wrapping_add(0x10));
    let di = c.word(si.wrapping_add(4));
    let b0a = c.byte(di.wrapping_add(0x0a));
    let b8 = c.byte(di.wrapping_add(8));
    let mut n = 5 + 4 + (al != 1) as u32 + 2 + 2 + (ph >= 5) as u32 + 2;
    let stage = if ph < 4 {
        n += 1;
        0
    } else if w10 & 0x200 != 0 {
        n += 3;
        1
    } else if b0a & 8 == 0 && b8 >= 0x28 {
        n += 2 + 3 + 3;
        2
    } else {
        n += 2 + 3 + if b0a & 8 == 0 { 2 } else { 0 } + 2;
        3
    };
    c.run(start, n)?;
    menu_start(&mut c);
    c.test(W, w12, 0x400);
    let b = c.byte(si.wrapping_add(3));
    c.set_reg8(r8::AL, b);
    let a = c.and(B, b as u16, 0x0f);
    c.set_reg8(r8::AL, a as u8);
    c.cmp(B, a, 1);
    if a != 1 {
        clear_bit_40(&mut c, 0x2111);
    }
    c.cmp(B, a, 2);
    c.cmp(B, ph as u16, 5);
    if ph >= 5 {
        clear_bit_40(&mut c, 0x2119);
    }
    c.cmp(B, ph as u16, 4);
    if stage == 0 {
        return c.ret();
    }
    c.test(W, w10, 0x200);
    if stage == 1 {
        return c.ret();
    }
    c.set_reg(DI, di);
    c.test(B, b0a as u16, 8);
    if b0a & 8 == 0 {
        c.cmp(B, b8 as u16, 0x28);
    }
    if stage == 2 {
        return c.ret();
    }
    c.push(si);
    c.call(0x7f27, 0x78a3)?;
    let mut acc = 0u16;
    let mut need = [0u8; 7];
    for (i, v) in need.iter_mut().enumerate() {
        *v = c.byte(0x46fe + i as u16);
        acc |= *v as u16;
    }
    let si0 = c.word_in(SS, c.reg(SP));
    let b19 = c.byte(si0.wrapping_add(0x19));
    let all = acc | b19 as u16;
    c.run(0x78a3, 3 + 21 + 3 + if all != 0 { 2 } else { 1 })?;
    c.set_reg(SI, 0x46fe);
    c.xor(B, 0, 0);
    let mut a = 0u16;
    for v in need {
        a = c.or(B, a, v as u16);
        let s = c.reg(SI);
        let s = c.inc(W, s);
        c.set_reg(SI, s);
    }
    c.set_reg(CX, 0);
    let s = c.pop();
    c.set_reg(SI, s);
    let a = c.or(B, a, b19 as u16);
    c.set_reg8(r8::AL, a as u8);
    if a != 0 {
        clear_bit_40(&mut c, 0x2115);
    }
    c.ret()
}

/// The first three of `loc_07847`: all the menu items on.
fn menu_start(c: &mut Ctx) {
    c.set_word(0x2110, 0x404f);
    for p in [0x2115u16, 0x2119] {
        let v = c.byte(p);
        let v = c.or(B, v as u16, 0x40);
        c.set_byte(p, v as u8);
    }
}

fn clear_bit_40(c: &mut Ctx, p: u16) {
    let v = c.byte(p);
    let v = c.and(B, v as u16, 0xbf);
    c.set_byte(p, v as u8);
}

/// `loc_0686e`: the troop SI's screen place: with the map up ([46EBh]
/// 80h or more), for one at its location (bit 40h) its map point
/// (`map_to_screen`), CF set when off the view (-10h..148h, -10h..A0h);
/// else the offset for its direction (byte 2 - 1, mirrored with bit 2 of
/// the location's byte 0Ah) from 1672h added to the place at SS:[BP+2],
/// CF set when [BP+4] is below 8000h. CF set out of the map.
pub fn troop_screen_place(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x686e)?;
    let mut c = Ctx::new(m);
    let mode = c.byte(0x46eb);
    if mode < 0x80 {
        c.run(start, 3)?;
        c.cmp(B, mode as u16, 0x80);
        return c.ret();
    }
    let si = c.reg(SI);
    let b3 = c.byte(si.wrapping_add(3));
    if b3 & 0x40 != 0 {
        c.run(start, 4 + 3)?;
        c.cmp(B, mode as u16, 0x80);
        c.test(B, b3 as u16, 0x40);
        let (x, y) = (c.word(si.wrapping_add(6)), c.word(si.wrapping_add(8)));
        c.set_reg(DX, x);
        c.set_reg(BX, y);
        c.call(0xb647, 0x68b8)?;
        let (dx, bx) = (c.reg(DX), c.reg(BX));
        let (sx, sy) = (dx as i16, bx as i16);
        let out = if sx <= -16 {
            2
        } else if sy <= -16 {
            4
        } else if sx >= 0x148 {
            6
        } else if sy >= 0xa0 {
            8
        } else {
            0
        };
        c.run(0x68b8, if out != 0 { out + 2 } else { 8 + 2 })?;
        c.cmp(W, dx, 0xfff0);
        if out == 0 || out > 2 {
            c.cmp(W, bx, 0xfff0);
            if out == 0 || out > 4 {
                c.cmp(W, dx, 0x148);
                if out == 0 || out > 6 {
                    c.cmp(W, bx, 0xa0);
                }
            }
        }
        c.set_flag(CF, out != 0);
        return c.ret();
    }
    let (bx0, di0, bp) = (c.reg(BX), c.reg(DI), c.reg(BP));
    let b2 = c.byte(si.wrapping_add(2));
    let loc = c.word(si.wrapping_add(4));
    let f = c.byte(loc.wrapping_add(0x0a));
    let flip = f & 2 != 0;
    let px = c.word_in(SS, bp.wrapping_add(2));
    let py = c.word_in(SS, bp.wrapping_add(4));
    let off = py >= 0x8000;
    c.run(start, 4 + 6 + flip as u32 + 12 + if off { 4 } else { 1 })?;
    c.cmp(B, mode as u16, 0x80);
    c.test(B, b3 as u16, 0x40);
    let bx = c.dec(W, bx0 & 0xff00 | b2 as u16);
    c.push(di0);
    c.set_reg(DI, loc);
    c.test(B, f as u16, 2);
    let mut bx = bx;
    if flip {
        let l = c.xor(B, bx & 0xff, 8);
        bx = bx & 0xff00 | l;
    }
    let d = c.pop();
    c.set_reg(DI, d);
    let bx = c.and(W, bx, 0x0f);
    let bx = c.add(W, bx, bx);
    let ox = c.byte(bx.wrapping_add(0x1672)) as i8 as i16 as u16;
    let oy = c.byte(bx.wrapping_add(0x1673)) as i8 as i16 as u16;
    c.set_reg(AX, oy);
    let dx = c.add(W, ox, px);
    c.set_reg(DX, dx);
    c.cmp(B, py >> 8, 0x80);
    if !off {
        c.set_reg(BX, py);
        return c.ret();
    }
    c.xor(B, 0, 0);
    let b = c.add(W, py & 0xff, oy);
    c.set_reg(BX, b);
    c.set_flag(CF, false);
    c.ret()
}

/// `loc_01e43`: in phase 5Ch, once the day reaches [1156h], away from
/// location 7C8h: the location from 100h (below 28h, without bit 80h of
/// byte 0Ah, not 2C0h) where `loc_01ea1` counts the most troops becomes
/// [11DBh] ([0F8h] counted), `loc_01ea9` for its troops, and pair 0F08h
/// ([`call_29f0_f`] with AL 8).
pub fn pick_attack_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1e43)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x1ac5, 0x1e46)?;
    let ax = c.reg(AX);
    let day = c.word(0x1156);
    let ph = c.byte(0x2a);
    let here = c.word(0x114e);
    if ax < day {
        c.run(0x1e46, 3)?;
        c.cmp(W, ax, day);
        return c.ret();
    }
    if ph != 0x5c {
        c.run(0x1e46, 5)?;
        c.cmp(W, ax, day);
        c.cmp(B, ph as u16, 0x5c);
        return c.ret();
    }
    if here == 0x7c8 {
        c.run(0x1e46, 7)?;
        c.cmp(W, ax, day);
        c.cmp(B, ph as u16, 0x5c);
        c.cmp(W, here, 0x7c8);
        return c.ret();
    }
    c.run(0x1e46, 8)?;
    c.cmp(W, ax, day);
    c.cmp(B, ph as u16, 0x5c);
    c.cmp(W, here, 0x7c8);
    c.set_reg(DI, 0x100);
    c.xor(W, 0, 0);
    c.set_reg(CX, 0);
    let mut di = 0x100u16;
    loop {
        let b8 = c.byte(di.wrapping_add(8));
        let f = c.byte(di.wrapping_add(0x0a));
        if b8 >= 0x28 {
            c.run(0x1e60, 2)?;
            c.cmp(B, b8 as u16, 0x28);
        } else if f & 0x80 != 0 {
            c.run(0x1e60, 4)?;
            c.cmp(B, b8 as u16, 0x28);
            c.test(B, f as u16, 0x80);
        } else if di == 0x2c0 {
            c.run(0x1e60, 6)?;
            c.cmp(B, b8 as u16, 0x28);
            c.test(B, f as u16, 0x80);
            c.cmp(W, di, 0x2c0);
        } else {
            c.run(0x1e60, 9)?;
            c.cmp(B, b8 as u16, 0x28);
            c.test(B, f as u16, 0x80);
            c.cmp(W, di, 0x2c0);
            c.xor(W, 0, 0);
            c.set_reg(DX, 0);
            c.set_reg(BP, 0x1ea1);
            c.call(0x661d, 0x1e7a)?;
            let (dx, cx) = (c.reg(DX), c.reg(CX));
            let more = dx > cx;
            c.run(0x1e7a, 2 + if more { 2 } else { 0 })?;
            c.cmp(W, dx, cx);
            if more {
                c.set_reg(CX, dx);
                let d = c.reg(DI);
                c.set_reg(BX, d);
            }
            di = c.reg(DI);
        }
        let nd = di.wrapping_add(0x1c);
        let nb = c.byte(nd);
        let cx = c.reg(CX);
        c.run(0x1e82, 3 + if nb == 0xff { 1 + if cx == 0 { 1 } else { 0 } } else { 0 })?;
        let d = c.add(W, di, 0x1c);
        c.set_reg(DI, d);
        c.cmp(B, nb as u16, 0xff);
        di = d;
        if nb != 0xff {
            continue;
        }
        if cx == 0 {
            return c.ret();
        }
        break;
    }
    c.run(0x1e8c, 5)?;
    let bx = c.reg(BX);
    c.set_reg(DI, bx);
    c.set_word(0x11db, bx);
    let v = c.byte(0xf8);
    let v = c.inc(B, v as u16);
    c.set_byte(0xf8, v as u8);
    c.set_reg(BP, 0x1ea9);
    c.call(0x661d, 0x1e9c)?;
    c.run(0x1e9c, 2)?;
    c.set_reg8(r8::AL, 8);
    call_29f0_f(c.m, 0x71b2)
}

/// `loc_06e20`: with [46DEh] set, for the troop SI at the location DI:
/// a location with bit 8 of byte 0Ah whose time (byte 0Bh) has come
/// (day - it below FEh) is freed (bit 8 off, byte 8 to its low 3 bits,
/// [27h] counted, `loc_06dbb`, byte 0Bh 5); then `loc_06cfc`, after 8
/// days (byte 14h) `loc_06f93` with AL 1; for the location's first troop
/// with 3 counted by `loc_06e82`, `loc_06ea8` for all and pair 302h.
pub fn troop_day_update(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6e20)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x46de);
    if k == 0 {
        c.run(start, 3)?;
        c.cmp(B, 0, 0);
        return c.ret();
    }
    let di = c.reg(DI);
    let f = c.byte(di.wrapping_add(0x0a));
    if f & 8 != 0 {
        c.run(start, 5)?;
        c.cmp(B, k as u16, 0);
        c.test(B, f as u16, 8);
        c.call(0x1ac5, 0x6e31)?;
        let di = c.reg(DI);
        let al = c.reg8(r8::AL);
        let b = c.byte(di.wrapping_add(0x0b));
        let d = al.wrapping_sub(b);
        if d >= 0xfe {
            c.run(0x6e31, 4)?;
            let d = c.sub(B, al as u16, b as u16);
            c.set_reg8(r8::AL, d as u8);
            c.cmp(B, d, 0xfe);
            return c.ret();
        }
        c.run(0x6e31, 7)?;
        let d = c.sub(B, al as u16, b as u16);
        c.set_reg8(r8::AL, d as u8);
        c.cmp(B, d, 0xfe);
        let v = c.byte(di.wrapping_add(0x0a));
        let v = c.and(B, v as u16, 0xf7);
        c.set_byte(di.wrapping_add(0x0a), v as u8);
        let v = c.byte(di.wrapping_add(8));
        let v = c.and(B, v as u16, 7);
        c.set_byte(di.wrapping_add(8), v as u8);
        let v = c.byte(0x27);
        let v = c.inc(B, v as u16);
        c.set_byte(0x27, v as u8);
        c.call(0x6dbb, 0x6e47)?;
        c.run(0x6e47, 2)?;
        let di = c.reg(DI);
        c.set_byte(di.wrapping_add(0x0b), 5);
    } else {
        c.run(start, 5)?;
        c.cmp(B, k as u16, 0);
        c.test(B, f as u16, 8);
    }
    c.call(0x6cfc, 0x6e4e)?;
    c.run(0x6e4e, 1)?;
    c.call(0x1ac5, 0x6e51)?;
    let si = c.reg(SI);
    let al = c.reg8(r8::AL);
    let b = c.byte(si.wrapping_add(0x14));
    let d = al.wrapping_sub(b);
    if d > 8 {
        c.run(0x6e51, 5)?;
        let d = c.sub(B, al as u16, b as u16);
        c.set_reg8(r8::AL, d as u8);
        c.cmp(B, d, 8);
        c.set_reg8(r8::AL, 1);
        c.call(0x6f93, 0x6e5d)?;
    } else {
        c.run(0x6e51, 3)?;
        let d = c.sub(B, al as u16, b as u16);
        c.set_reg8(r8::AL, d as u8);
        c.cmp(B, d, 8);
    }
    let (si, di) = (c.reg(SI), c.reg(DI));
    let id = c.byte(si);
    let first = c.byte(di.wrapping_add(9));
    if id != first {
        c.run(0x6e5d, 4)?;
        c.set_reg8(r8::AL, id);
        c.cmp(B, id as u16, first as u16);
        return c.ret();
    }
    c.run(0x6e5d, 6)?;
    c.set_reg8(r8::AL, id);
    c.cmp(B, id as u16, first as u16);
    c.xor(W, 0, 0);
    c.set_reg(DX, 0);
    c.set_reg(BP, 0x6e82);
    c.call(0x661d, 0x6e6c)?;
    let dl = c.reg8(r8::DL);
    if dl != 3 {
        c.run(0x6e6c, 3)?;
        c.cmp(B, dl as u16, 3);
        return c.ret();
    }
    c.run(0x6e6c, 4)?;
    c.cmp(B, 3, 3);
    c.set_reg(BP, 0x6ea8);
    c.call(0x661d, 0x6e77)?;
    c.run(0x6e77, 4)?;
    c.set_reg(AX, 0x302);
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.push(si);
    c.push(di);
    c.call(0x29f0, 0x6e7f)?;
    c.run(0x6e7f, 3)?;
    let d = c.pop();
    c.set_reg(DI, d);
    let s = c.pop();
    c.set_reg(SI, s);
    c.ret()
}

/// `loc_0329d`: for a troop SI with byte 3 set, bits Ch of word 10h
/// cleared (AX 0); else [4Ah] = its word 0Eh over the age [42h]
/// (rounded; the word itself for age 0) and [`troop_rate`].
pub fn troop_rate_by_age(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x329d)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let b3 = c.byte(si.wrapping_add(3));
    if b3 != 0 {
        c.run(start, 5)?;
        c.cmp(B, b3 as u16, 0);
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
        let w = c.word(si.wrapping_add(0x10));
        let w = c.and(W, w, 0xfff3);
        c.set_word(si.wrapping_add(0x10), w);
        return c.ret();
    }
    let age = c.word(0x42);
    c.run(start, 2 + 3 + if age != 0 { 7 } else { 0 } + 2)?;
    c.cmp(B, 0, 0);
    c.set_reg(AX, age);
    c.or(W, age, age);
    let mut ax = age;
    if age != 0 {
        c.set_reg(CX, age);
        let w = c.word(si.wrapping_add(0x0e));
        c.set_reg(AX, w);
        c.xor(W, 0, 0);
        c.set_reg(DX, 0);
        c.m.cpu.mul_div(&mut c.m.hw, W, 6, age as u32);
        let r = c.reg(DX);
        let r = c.shl1(W, r, 1);
        c.set_reg(DX, r);
        c.cmp(W, age, r);
        let q = c.reg(AX);
        ax = c.adc(W, q, 0);
        c.set_reg(AX, ax);
    }
    c.set_word(0x4a, ax);
    troop_rate(c.m, 0x708a)
}

/// `loc_0714c`: with bit 1 of [0Ah], for a troop SI with bit 80h of byte
/// 19h on its tick (low nibble of [2] = its id's): `maybe_mark_troop`,
/// then a random AH (`rand`) against the table 1141h by the location
/// DI's byte 0: when it passes, bit 4000h of word 10h, and unless bit
/// 40h of byte 19h, by AH & 3: 1 size down 2 (bit 2000h), 2 bit 200h
/// and `mark_troop_10`, 3 bit 1000h, skill bit 80h off, the location's
/// byte 14h down and pair 0F06h.
pub fn troop_event_roll(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x714c)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0x0a);
    let si = c.reg(SI);
    let b19 = c.byte(si.wrapping_add(0x19));
    let t = c.word(2);
    let id = c.byte(si);
    let ax = ((id as u16) << 8 | t & 0xff) & 0x0f0f;
    if f & 1 == 0 {
        c.run(start, 3)?;
        c.test(B, f as u16, 1);
        return c.ret();
    }
    if b19 & 0x80 == 0 {
        c.run(start, 5)?;
        c.test(B, f as u16, 1);
        c.test(B, b19 as u16, 0x80);
        return c.ret();
    }
    let hit = ax & 0xff == ax >> 8;
    c.run(start, 9 + if hit { 1 } else { 1 })?;
    c.test(B, f as u16, 1);
    c.test(B, b19 as u16, 0x80);
    let a = c.and(W, (id as u16) << 8 | t & 0xff, 0x0f0f);
    c.set_reg(AX, a);
    c.cmp(B, a & 0xff, a >> 8);
    if !hit {
        return c.ret();
    }
    c.call(0x71bc, 0x7168)?;
    c.run(0x7168, 1)?;
    c.call(0xe3cc, 0x716b)?;
    let (si, di) = (c.reg(SI), c.reg(DI));
    let ah = c.reg8(r8::AH);
    let b0 = c.byte(di);
    let lim = c.byte(0x1141u16.wrapping_add(b0 as u16));
    if lim < ah {
        c.run(0x716b, 6)?;
        c.set_reg8(r8::AL, lim);
        c.set_reg(BX, 0x1141);
        c.cmp(B, lim as u16, ah as u16);
        return c.ret();
    }
    let b19 = c.byte(si.wrapping_add(0x19));
    let kind = ah & 3;
    let n = 5 + 3 + if b19 & 0x40 != 0 { 1 } else { 2 + if kind == 0 { 1 } else { 2 + (kind <= 2) as u32 } };
    let size = c.byte(si.wrapping_add(0x1a));
    let tail = if b19 & 0x40 != 0 || kind == 0 {
        0
    } else if kind == 1 {
        3 + if size > 2 { 1 } else { 2 }
    } else if kind == 2 {
        2
    } else {
        4
    };
    c.run(0x716b, n + tail)?;
    c.set_reg8(r8::AL, lim);
    c.set_reg(BX, 0x1141);
    c.cmp(B, lim as u16, ah as u16);
    let w = c.word(si.wrapping_add(0x10));
    let w = c.or(W, w, 0x4000);
    c.set_word(si.wrapping_add(0x10), w);
    c.test(B, b19 as u16, 0x40);
    if b19 & 0x40 != 0 {
        return c.ret();
    }
    let k = c.and(B, ah as u16, 3);
    c.set_reg8(r8::AH, k as u8);
    if k == 0 {
        return c.ret();
    }
    c.cmp(B, k, 2);
    match k {
        1 => {
            let w = c.or(W, w, 0x2000);
            c.set_word(si.wrapping_add(0x10), w);
            let v = c.sub(B, size as u16, 2);
            c.set_byte(si.wrapping_add(0x1a), v as u8);
            if size > 2 {
                return c.ret();
            }
            let v = c.add(B, v, 2);
            c.set_byte(si.wrapping_add(0x1a), v as u8);
            c.ret()
        }
        2 => {
            let w = c.or(W, w, 0x200);
            c.set_word(si.wrapping_add(0x10), w);
            mark_troop_10(c.m, 0x7085)
        }
        _ => {
            let w = c.or(W, w, 0x1000);
            c.set_word(si.wrapping_add(0x10), w);
            let v = c.and(B, b19 as u16, 0x7f);
            c.set_byte(si.wrapping_add(0x19), v as u8);
            let d = c.byte(di.wrapping_add(0x14));
            let d = c.dec(B, d as u16);
            c.set_byte(di.wrapping_add(0x14), d as u8);
            c.set_reg8(r8::AL, 6);
            call_29f0_f(c.m, 0x71b2)
        }
    }
}

/// `loc_0342d`: AX = the troop SI's yield: (2·motivation + byte 17h,
/// at most FFh) × its size / 16, plus that ×2, ×4, ×8, ×16 for bits 5,
/// 4, 3, 2 of byte 19h (FFFFh on overflow), /256; 1 instead of 0 for a
/// troop of size 1 or more.
pub fn troop_yield(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x342d)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x6efd, 0x3430)?;
    let si = c.reg(SI);
    let al0 = c.reg8(r8::AL);
    let b17 = c.byte(si.wrapping_add(0x17));
    let size = c.byte(si.wrapping_add(0x1a));
    let b19 = c.byte(si.wrapping_add(0x19));
    let a2 = al0.wrapping_add(al0);
    let (s, cf) = a2.overflowing_add(b17);
    let a = if cf { 0xff } else { s };
    let base = (a as u16 * size as u16) >> 4;
    let mut n = 4 + cf as u32 + 12;
    let mut ax = base;
    let mut dx = base.wrapping_shl(1);
    let mut sat = false;
    for k in 0..4 {
        if k > 0 {
            dx = dx.wrapping_shl(1);
            n += 3;
        }
        if b19 & (0x20 >> k) != 0 {
            let (v, carry) = ax.overflowing_add(dx);
            ax = v;
            n += 2;
            if carry {
                sat = true;
                break;
            }
        }
    }
    if sat {
        ax = 0xffff;
        n += 1;
    }
    let hi = ax >> 8;
    n += 4 + if hi != 0 { 1 } else { 4 };
    c.run(0x3430, n)?;
    c.xor(B, 0, 0);
    let x = c.add(B, al0 as u16, al0 as u16);
    let x = c.add(B, x, b17 as u16);
    c.set_reg(AX, if cf { 0xff } else { x });
    c.mul8(size);
    let v = c.reg(AX);
    let v = c.shr1(W, v, 4);
    let mut ax = v;
    let mut dx = v;
    let mut bl = b19 as u16;
    bl = c.shl1(B, bl, 2);
    dx = c.shl1(W, dx, 1);
    for k in 0..4 {
        if k > 0 {
            dx = c.shl1(W, dx, 1);
        }
        bl = c.shl1(B, bl, 1);
        if c.flag(CF) {
            ax = c.add(W, ax, dx);
            if c.flag(CF) {
                ax = 0xffff;
                break;
            }
        }
    }
    c.set_reg(DX, dx);
    c.set_reg8(r8::BL, bl as u8);
    let a = ax >> 8;
    c.xor(B, 0, 0);
    c.set_reg(AX, a);
    c.or(W, a, a);
    if a != 0 {
        return c.ret();
    }
    c.cmp(B, size as u16, 1);
    let cf = c.flag(CF);
    c.set_flag(CF, !cf);
    let al = c.adc(B, 0, 0);
    c.set_reg(AX, al);
    c.ret()
}

/// `loc_06ac5`: the troop SI's activity set from its order (byte 3's top
/// bits; order 8 at an empty 7C8h becomes 0Ah) when they differ: bits 30h
/// of byte 12h and 100h of word 10h cleared (the latter back unless
/// `loc_06c15` sets CF), `drop_troop_particle`, `stamp_record`, its
/// skill bit (2000h << `loc_0693b`) for activities other than 2, and
/// `troop_orders_menu` for the troop shown ([1954h]).
pub fn set_troop_activity(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6ac5)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let b3 = c.byte(si.wrapping_add(3));
    let (cl, al) = (b3 & 0xfc, b3 & 0x0f);
    if al == cl {
        c.run(start, 7)?;
        c.set_reg8(r8::CL, cl);
        c.and(B, b3 as u16, 0xfc);
        let a = c.and(B, b3 as u16, 0x0f);
        c.set_reg8(r8::AL, a as u8);
        c.cmp(B, a, cl as u16);
        return c.ret();
    }
    let di = c.word(si.wrapping_add(4));
    let e = c.byte(di.wrapping_add(0x1a));
    let n8 = if cl != 8 { 0 } else if di != 0x7c8 { 2 } else if e != 0 { 4 } else { 5 };
    c.run(start, 6 + 3 + n8 + 4)?;
    c.and(B, b3 as u16, 0xfc);
    let a = c.and(B, b3 as u16, 0x0f);
    c.set_reg8(r8::AL, a as u8);
    c.cmp(B, a, cl as u16);
    c.set_reg(DI, di);
    c.cmp(B, cl as u16, 8);
    let mut v = cl;
    if cl == 8 {
        c.cmp(W, di, 0x7c8);
        if di == 0x7c8 {
            c.cmp(B, e as u16, 0);
            if e == 0 {
                v = 0x0a;
            }
        }
    }
    c.set_reg8(r8::CL, v);
    c.set_byte(si.wrapping_add(3), v);
    let b = c.byte(si.wrapping_add(0x12));
    let b = c.and(B, b as u16, 0xcf);
    c.set_byte(si.wrapping_add(0x12), b as u8);
    let w = c.word(si.wrapping_add(0x10));
    let w = c.and(W, w, 0xfeff);
    c.set_word(si.wrapping_add(0x10), w);
    c.call(0x6c15, 0x6af9)?;
    let cf = c.flag(CF);
    c.run(0x6af9, 2 + (!cf) as u32)?;
    if !cf {
        let si = c.reg(SI);
        let w = c.word(si.wrapping_add(0x10));
        let w = c.or(W, w, 0x100);
        c.set_word(si.wrapping_add(0x10), w);
    }
    c.call(0x8461, 0x6b03)?;
    c.run(0x6b03, 1)?;
    c.call(0x6b25, 0x6b06)?;
    let si = c.reg(SI);
    let b3 = c.byte(si.wrapping_add(3));
    if b3 != 2 {
        c.run(0x6b06, 3)?;
        c.cmp(B, b3 as u16, 2);
        c.call(0x693b, 0x6b0f)?;
        c.run(0x6b0f, 4)?;
        let al = c.reg8(r8::AL);
        c.set_reg8(r8::CL, al);
        let bit = c.shl(W, 0x2000, al);
        c.set_reg(AX, bit);
        let si = c.reg(SI);
        let w = c.word(si.wrapping_add(0x12));
        let w = c.or(W, w, bit);
        c.set_word(si.wrapping_add(0x12), w);
    } else {
        c.run(0x6b06, 2)?;
        c.cmp(B, 2, 2);
    }
    let si = c.reg(SI);
    let id = c.byte(si);
    let shown = c.byte(0x1954);
    c.run(0x6b19, 3 + if id == shown { 1 } else { 1 })?;
    c.set_reg8(r8::AL, id);
    c.cmp(B, id as u16, shown as u16);
    if id != shown {
        return c.ret();
    }
    c.call(0x7847, 0x6b24)?;
    c.run(0x6b24, 1)?;
    c.ret()
}

/// `loc_07f90`: for the troop SI, the nearest other location from 138h
/// (no bit 80h of byte 0Ah, below 28h; distance the larger of the
/// coordinate deltas' high bytes, under 32h, halved twice for sietches
/// from 21h) that still needs skill BX after this troop's own location
/// and the movers (`skill_needs` into 4C60h, `lower_needs_by_movers`):
/// [470Eh] it, [470Ch] its distance (FFFFh for none).
pub fn nearest_needing_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7f90)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let si = c.reg(SI);
    let home = c.word(si.wrapping_add(4));
    let (hx, hy) = (c.word(home.wrapping_add(2)), c.word(home.wrapping_add(4)));
    c.set_reg(DI, home);
    c.push(home);
    c.push(hy);
    c.push(hx);
    let bp = c.reg(SP);
    c.set_reg(BP, bp);
    c.set_word(0x470c, 0xffff);
    let mut di = 0x138u16;
    c.set_reg(DI, di);
    loop {
        let w = c.word(di);
        if w == 0xffff {
            c.run(0x7fa5, 4)?;
            c.cmp(W, w, 0xffff);
            let sp = c.reg(SP);
            let sp = c.add(W, sp, 6);
            c.set_reg(SP, sp);
            return c.ret();
        }
        let f = c.byte(di.wrapping_add(0x0a));
        let b8 = c.byte(di.wrapping_add(8));
        'one: {
            if f & 0x80 != 0 {
                c.run(0x7fa5, 4)?;
                c.cmp(W, w, 0xffff);
                c.test(B, f as u16, 0x80);
                break 'one;
            }
            if b8 >= 0x28 {
                c.run(0x7fa5, 6)?;
                c.cmp(W, w, 0xffff);
                c.test(B, f as u16, 0x80);
                c.cmp(B, b8 as u16, 0x28);
                break 'one;
            }
            if di == home {
                c.run(0x7fa5, 8)?;
                c.cmp(W, w, 0xffff);
                c.test(B, f as u16, 0x80);
                c.cmp(B, b8 as u16, 0x28);
                c.cmp(W, di, home);
                break 'one;
            }
            let (x, y) = (c.word(di.wrapping_add(2)), c.word(di.wrapping_add(4)));
            let dxr = x.wrapping_sub(hx);
            let n1 = dxr & 0x8000 != 0;
            let dxa = if n1 { dxr.wrapping_neg() } else { dxr };
            let axr = y.wrapping_sub(hy);
            let n2 = axr & 0x8000 != 0;
            let axa = if n2 { axr.wrapping_neg() } else { axr };
            let dl = (dxa >> 8) as u8;
            let take_ax = dl < axa as u8;
            let mut d = if take_ax { axa } else { dl as u16 };
            let far = (d as u8) >= 0x32;
            let sietch = b8 < 0x21;
            if !sietch && !far {
                d >>= 2;
            }
            let best = c.word(0x470c);
            let mut n = 8 + 3 + n1 as u32 + 3 + n2 as u32 + 4 + take_ax as u32 + 2;
            if far {
                c.run(0x7fa5, n)?;
            } else {
                n += 2 + if sietch { 0 } else { 2 } + 2;
                if d >= best {
                    c.run(0x7fa5, n)?;
                } else {
                    c.run(0x7fa5, n + 3)?;
                }
            }
            c.cmp(W, w, 0xffff);
            c.test(B, f as u16, 0x80);
            c.cmp(B, b8 as u16, 0x28);
            c.cmp(W, di, home);
            let a = c.sub(W, x, hx);
            let a = if n1 { c.neg(W, a) } else { a };
            let b = c.sub(W, y, hy);
            let b = if n2 { c.neg(W, b) } else { b };
            c.set_reg(AX, b);
            c.xor(B, 0, 0);
            let mut dx = a >> 8;
            c.cmp(B, dx, b & 0xff);
            if take_ax {
                dx = b;
            }
            c.cmp(B, dx & 0xff, 0x32);
            if far {
                c.set_reg(DX, dx);
                break 'one;
            }
            c.cmp(B, b8 as u16, 0x21);
            if !sietch {
                dx = c.shr1(W, dx, 2);
            }
            c.set_reg(DX, dx);
            c.cmp(W, dx, best);
            if dx >= best {
                break 'one;
            }
            let bx = c.reg(BX);
            c.push(bx);
            c.set_reg(BX, 0x4c60);
            c.call(0x7f2a, 0x7ff5)?;
            let bx = c.word_in(SS, c.reg(SP));
            let need = c.byte(bx.wrapping_add(0x4c60));
            c.run(0x7ff5, 3 + (need != 0) as u32)?;
            let b = c.pop();
            c.set_reg(BX, b);
            c.cmp(B, need as u16, 0);
            if need == 0 {
                break 'one;
            }
            c.call(0x8018, 0x8000)?;
            let bx = c.reg(BX);
            let need = c.byte(bx.wrapping_add(0x4c60));
            c.run(0x8000, 2 + if need != 0 { 2 } else { 0 })?;
            c.cmp(B, need as u16, 0);
            if need != 0 {
                let (dx, di) = (c.reg(DX), c.reg(DI));
                c.set_word(0x470c, dx);
                c.set_word(0x470e, di);
            }
        }
        di = c.reg(DI);
        c.run(0x800f, 2)?;
        di = c.add(W, di, 0x1c);
        c.set_reg(DI, di);
    }
}

/// `loc_0851f`: the troop SI linked into the location DI's chain (first,
/// or after the last) and given the first free slot (byte 2, from 1; from
/// 9 for those with bit 80h of word 10h), by a table of the slots taken
/// built on the stack. CX the slot.
pub fn link_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x851f)?;
    let mut c = Ctx::new(m);
    let (si, di) = (c.reg(SI), c.reg(DI));
    let id = c.byte(si);
    let first = c.byte(di.wrapping_add(9));
    let w10 = c.word(si.wrapping_add(0x10));
    let hi = w10 & 0x80 != 0;
    if first == 0 {
        c.run(start, 4 + 5 + if hi { 2 } else { 0 } + 1)?;
        c.set_reg(AX, (id as u16) << 8);
        c.or(B, 0, 0);
        c.set_byte(di.wrapping_add(9), id);
        c.set_reg(CX, 1);
        c.set_byte(si.wrapping_add(2), 1);
        c.test(W, w10, 0x80);
        if hi {
            let v = c.add(B, 1, 8);
            c.set_byte(si.wrapping_add(2), v as u8);
            let cl = c.add(B, 1, 8);
            c.set_reg(CX, cl);
        }
        return c.ret();
    }
    c.run(start, 4 + 9)?;
    let ax = (id as u16) << 8 | first as u16;
    c.set_reg(AX, ax);
    c.or(B, first as u16, first as u16);
    c.push(di);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    let sp = c.reg(SP);
    let sp = c.sub(W, sp, 0x1e);
    c.set_reg(SP, sp);
    c.set_reg(DI, sp);
    c.set_reg(CX, 0x1e);
    c.push(ax);
    c.xor(B, 0, 0);
    c.set_reg(AX, ax & 0xff00);
    c.rep(0xaa);
    c.run(0x8550, 5)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    let buf = c.reg(SP);
    c.set_reg(DI, buf);
    c.push(si);
    c.xor(W, 0, 0);
    c.set_reg(BX, 0);
    loop {
        c.call(0x6906, 0x8559)?;
        let s = c.reg(SI);
        let slot = c.byte(s.wrapping_add(2));
        let next = c.byte(s.wrapping_add(1));
        c.run(0x8559, 5 + if next == 0 { 5 } else { 1 })?;
        c.set_reg(BX, slot as u16);
        let di = c.reg(DI);
        c.set_byte((slot as u16).wrapping_add(di).wrapping_sub(1), 0xff);
        c.set_reg8(r8::AL, next);
        c.or(B, next as u16, next as u16);
        if next != 0 {
            continue;
        }
        let ah = c.reg8(r8::AH);
        c.set_byte(s.wrapping_add(1), ah);
        c.set_reg(CX, 0x1e);
        let si = c.pop();
        c.set_reg(SI, si);
        let w = c.word(si.wrapping_add(0x10));
        c.test(W, w, 0x80);
        if w & 0x80 != 0 {
            c.run(0x8575, 2)?;
            let cx = c.sub(W, 0x1e, 8);
            c.set_reg(CX, cx);
            let d = c.add(W, di, 8);
            c.set_reg(DI, d);
        }
        break;
    }
    c.run(0x857b, 2)?;
    c.xor(B, 0, 0);
    c.set_reg8(r8::AL, 0);
    c.m.cpu.string_op(&mut c.m.hw, 0xae, None, 0xf2); // repne scasb
    c.run(0x857f, 6)?;
    let cx = c.reg(CX);
    let cx = c.sub(W, cx, 0x1e);
    let cx = c.neg(W, cx);
    c.set_reg(CX, cx);
    let sp = c.reg(SP);
    let sp = c.add(W, sp, 0x1e);
    c.set_reg(SP, sp);
    let d = c.pop();
    c.set_reg(DI, d);
    let si = c.reg(SI);
    c.set_byte(si.wrapping_add(2), cx as u8);
    c.ret()
}

/// `loc_06c6f`: the troops' tick: [473Ch] the location of record 1018h
/// (when its byte 2 is 80h), `phase_30_check`; then for each troop from
/// 8AAh: one with bits 430h of word 12h waits (bits 30h of byte 12h
/// cleared while [0FAh] is set, then looked at again unless 400h);
/// small ones (size below 14h) may merge (`merge_troop`); one with bits
/// A0h of byte 3 is skipped; one with 40h (at its own location) and not
/// shown ([1954h]) gets `loc_08308`; else its activity's handler (the
/// table at CS:6C26h, DI its location) and `troop_skills_decay`.
/// [0FAh] = [4737h] at the end.
pub fn troops_tick(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6c6f)?;
    let mut c = Ctx::new(m);
    let v = c.word(0x101a);
    let is80 = v & 0xff == 0x80;
    c.run(start, 4 + if is80 { 5 } else { 0 } + 2)?;
    c.xor(W, 0, 0);
    let mut si = 0u16;
    c.set_reg(AX, v);
    c.cmp(B, v & 0xff, 0x80);
    if is80 {
        let ah = c.dec(B, v >> 8);
        c.set_reg(AX, ah << 8 | 0x1c);
        c.mul8(ah as u8);
        let a = c.reg(AX);
        si = c.add(W, a, 0x100);
        c.set_reg(AX, si);
    }
    c.set_reg(SI, si);
    c.set_word(0x473c, si);
    c.call(0x6c46, 0x6c8a)?;
    c.run(0x6c8a, 2)?;
    c.set_byte(0x4737, 0);
    c.set_reg(SI, 0x8aa);
    let mut label = 0x6c92u16;
    loop {
        let si = c.reg(SI);
        match label {
            0x6c92 => {
                let w = c.word(si.wrapping_add(0x12));
                if w & 0x430 != 0 {
                    c.run(0x6c92, 2)?;
                    c.test(W, w, 0x430);
                    label = 0x6cd3;
                    continue;
                }
                let b = c.byte(si.wrapping_add(0x1a));
                if b >= 0x14 {
                    c.run(0x6c92, 4)?;
                    c.test(W, w, 0x430);
                    c.cmp(B, b as u16, 0x14);
                    label = 0x6ca4;
                } else {
                    c.run(0x6c92, 5)?;
                    c.test(W, w, 0x430);
                    c.cmp(B, b as u16, 0x14);
                    c.call(0x6d19, 0x6ca2)?;
                    let cf = c.flag(CF);
                    c.run(0x6ca2, 1)?;
                    label = if cf { 0x6cc3 } else { 0x6ca4 };
                }
            }
            0x6ca4 => {
                let al = c.byte(si.wrapping_add(3));
                if al & 0xa0 != 0 {
                    c.run(0x6ca4, 3)?;
                    c.set_reg8(r8::AL, al);
                    c.test(B, al as u16, 0xa0);
                    label = 0x6cc3;
                } else if al & 0x40 != 0 {
                    c.run(0x6ca4, 5)?;
                    c.set_reg8(r8::AL, al);
                    c.test(B, al as u16, 0xa0);
                    c.test(B, al as u16, 0x40);
                    label = 0x6ced;
                } else {
                    c.run(0x6ca4, 11)?;
                    c.set_reg8(r8::AL, al);
                    c.test(B, al as u16, 0xa0);
                    c.test(B, al as u16, 0x40);
                    let ax = c.reg(AX);
                    let a = c.and(W, ax, 0x0f);
                    c.set_reg(AX, a);
                    let bx = c.shl1(W, a, 1);
                    c.set_reg(BX, bx);
                    c.push(si);
                    let di = c.word(si.wrapping_add(4));
                    c.set_reg(DI, di);
                    let t = c.code_word(bx.wrapping_add(0x6c26));
                    c.call(t, 0x6cbf)?;
                    c.run(0x6cbf, 2)?;
                    let s = c.pop();
                    c.set_reg(SI, s);
                    c.call(0x6d7b, 0x6cc3)?;
                    label = 0x6cc3;
                }
            }
            0x6cc3 => {
                let s2 = si.wrapping_add(0x1b);
                let more = s2 < 0xfbb;
                c.run(0x6cc3, 3 + if more { 0 } else { 3 })?;
                let s2 = c.add(W, si, 0x1b);
                c.cmp(W, s2, 0xfbb);
                c.set_reg(SI, s2);
                if more {
                    label = 0x6c92;
                    continue;
                }
                let al = c.byte(0x4737);
                c.set_reg8(r8::AL, al);
                c.set_byte(0xfa, al);
                return c.ret();
            }
            0x6cd3 => {
                let b3 = c.byte(si.wrapping_add(3));
                if b3 & 0x40 != 0 {
                    c.run(0x6cd3, 2)?;
                    c.test(B, b3 as u16, 0x40);
                    label = 0x6ced;
                    continue;
                }
                let fa = c.byte(0xfa);
                if fa == 0 {
                    c.run(0x6cd3, 4)?;
                    c.test(B, b3 as u16, 0x40);
                    c.cmp(B, 0, 0);
                    label = 0x6cc3;
                    continue;
                }
                let w = c.word(si.wrapping_add(0x12));
                let lo = (w & 0xff) & 0xcf;
                let w2 = w & 0xff00 | lo;
                let wait = w2 & 0x400 != 0;
                c.run(0x6cd3, if wait { 7 } else { 8 })?;
                c.test(B, b3 as u16, 0x40);
                c.cmp(B, fa as u16, 0);
                c.and(B, w & 0xff, 0xcf);
                c.set_byte(si.wrapping_add(0x12), lo as u8);
                c.test(W, w2, 0x400);
                label = if wait { 0x6cc3 } else { 0x6c92 };
            }
            _ => {
                let id = c.byte(si);
                let shown = c.byte(0x1954);
                if id == shown {
                    c.run(0x6ced, 3)?;
                    c.set_reg8(r8::AL, id);
                    c.cmp(B, id as u16, shown as u16);
                } else {
                    c.run(0x6ced, 5)?;
                    c.set_reg8(r8::AL, id);
                    c.cmp(B, id as u16, shown as u16);
                    c.push(si);
                    c.call(0x8308, 0x6cf9)?;
                    c.run(0x6cf9, 2)?;
                    let s = c.pop();
                    c.set_reg(SI, s);
                }
                label = 0x6cc3;
            }
        }
    }
}

/// `loc_07be0`: for the selected troop ([46EFh] = SI): [47BAh] 0; in its
/// skills panel ([46F4h], [46F5h]) a click on a need (rect 1936h,
/// `find_range` over 4C7Ch) moves one point from 4705h to 46FEh and
/// redraws the panel (`draw_skills_panel`). Else its full refresh:
/// `loc_07ba3`, `loc_031f6`, its location (`loc_02e98`), and when it is
/// farther than [1176h] ([4Ch] FFh) the marked location [4752h] gets
/// 1916h; `loc_09719` until CF clear, `loc_09efd`, and for an order panel
/// ([47A5h] 80h) `loc_07efb` and the skill bars (`skill_bars_4705`); in
/// the hotspot mode `loc_09901`; front buffer active.
pub fn refresh_selected_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7be0)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let sel = c.word(0x46ef);
    if si != sel {
        c.run(start, 3)?;
        c.cmp(W, si, sel);
        return c.ret();
    }
    let (p4, p5) = (c.byte(0x46f4), c.byte(0x46f5));
    if p4 != 0 && p5 != 0 {
        c.run(start, 2 + 3 + 2 + 1 + 2 + 2)?;
        c.cmp(W, si, sel);
        c.set_word(0x47ba, 0);
        c.cmp(B, p4 as u16, 0);
        c.cmp(B, p5 as u16, 0);
        c.cmp(B, p5 as u16, 0);
        c.set_reg(DI, 0x1936);
        c.call(0xd6fe, 0x7ea4)?;
        if !c.flag(CF) {
            c.run(0x7ea4, 2)?;
            return c.ret();
        }
        c.run(0x7ea4, 3)?;
        c.set_reg(SI, 0x4c7c);
        c.call(0x7ee2, 0x7eac)?;
        if !c.flag(CF) {
            c.run(0x7eac, 2)?;
            return c.ret();
        }
        c.run(0x7eac, 5)?;
        let di = c.reg(DI);
        let v = c.byte(di.wrapping_add(0x4705));
        let v = c.dec(B, v as u16);
        c.set_byte(di.wrapping_add(0x4705), v as u8);
        let v = c.byte(di.wrapping_add(0x46fe));
        let v = c.inc(B, v as u16);
        c.set_byte(di.wrapping_add(0x46fe), v as u8);
        c.call(0x7d0c, 0x7ee1)?;
        c.run(0x7ee1, 1)?;
        return c.ret();
    }
    c.run(start, 2 + 3 + if p4 == 0 { 0 } else { 2 } + 2)?;
    c.cmp(W, si, sel);
    c.set_word(0x47ba, 0);
    c.cmp(B, p4 as u16, 0);
    if p4 != 0 {
        c.cmp(B, 0, 0);
    }
    c.set_reg(SI, sel);
    c.call(0x7ba3, 0x7c05)?;
    c.run(0x7c05, 1)?;
    c.call(0x31f6, 0x7c08)?;
    c.run(0x7c08, 2)?;
    let si = c.reg(SI);
    let di = c.word(si.wrapping_add(4));
    c.set_reg(DI, di);
    c.call(0x2e98, 0x7c0e)?;
    c.run(0x7c0e, 1)?;
    c.call(0x7c63, 0x7c11)?;
    let ax = c.reg(AX);
    let lim = c.word(0x1176);
    if ax <= lim {
        c.run(0x7c11, 3)?;
        c.cmp(W, ax, lim);
    } else {
        let m4 = c.word(0x4752);
        c.run(0x7c11, 6 + if m4 != 0 { 3 } else { 0 } + 1)?;
        c.cmp(W, ax, lim);
        c.set_byte(0x4c, 0xff);
        c.set_reg(DI, m4);
        c.or(W, m4, m4);
        if m4 != 0 {
            c.set_reg(AX, 0x1916);
            c.set_word(m4.wrapping_add(0x0d), 0x1916);
            c.set_word(m4.wrapping_add(0x0f), 0x1916);
        }
    }
    loop {
        c.call(0x9719, 0x7c30)?;
        let cf = c.flag(CF);
        c.run(0x7c30, 2 + if cf { 1 } else { 2 })?;
        let s = c.word(0x46ef);
        c.set_reg(SI, s);
        if !cf {
            break;
        }
    }
    let si = c.reg(SI);
    c.push(si);
    c.call(0x9efd, 0x7c3a)?;
    let k = c.byte(0x47a5);
    c.run(0x7c3a, 4 + if k == 0x80 { 2 } else { 0 })?;
    let s = c.pop();
    c.set_reg(SI, s);
    c.set_byte(0x46f4, 0);
    c.cmp(B, k as u16, 0x80);
    if k == 0x80 {
        let v = c.inc(B, 0);
        c.set_byte(0x46f4, v as u8);
        c.call(0x7efb, 0x7c4e)?;
        c.run(0x7c4e, 2)?;
        let si = c.reg(SI);
        c.push(si);
        c.call(0x7e1e, 0x7c52)?;
        c.run(0x7c52, 2)?;
        let s = c.pop();
        c.set_reg(SI, s);
    }
    let h = c.byte(0x46eb);
    c.run(0x7c56, 2 + if h & 0x80 != 0 { 1 } else { 1 })?;
    c.cmp(B, h as u16, 0);
    if h & 0x80 != 0 {
        c.call(0x9901, 0x7c60)?;
        c.run(0x7c60, 1)?;
    }
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `loc_08604`: the troop SI's step toward its location: DX the x step
/// (± the scale [4880h + 2·|y|]) and BX the y step (±1), along the larger
/// of the scaled x distance and the y distance; 0 near (below 7) and on
/// alternate ticks of the seed [0] (rotated) for the smaller axis. SI
/// kept. Bails at entry for a zero scale.
pub fn troop_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8604)?;
    let mut c = Ctx::new(m);
    let si0 = c.reg(SI);
    let loc = c.word(si0.wrapping_add(4));
    let lx = c.word(loc.wrapping_add(2));
    let ly = c.word(loc.wrapping_add(4));
    let ty = c.word(si0.wrapping_add(8));
    let tx = c.word(si0.wrapping_add(6));
    let b2 = ty.wrapping_shl(1);
    let n1 = b2 & 0x8000 != 0;
    let idx = if n1 { b2.wrapping_neg() } else { b2 };
    let scale = c.word_in(SS, idx.wrapping_add(0x4880));
    if scale == 0 {
        return Err(Bail);
    }
    let by = ty.wrapping_sub(ly).wrapping_neg();
    let n2 = by & 0x8000 != 0;
    let ay = if n2 { by.wrapping_neg() } else { by };
    let dxv = lx.wrapping_sub(tx);
    let n3 = dxv & 0x8000 != 0;
    let ax0 = if n3 { dxv.wrapping_neg() } else { dxv };
    let q = ax0 / scale;
    let seed = c.word(0);
    let roll_cf = seed & 0x8000 != 0;
    let mut n = 7 + n1 as u32 + 5 + 2 * n2 as u32 + 3 + n3 as u32 + 3 + 4;
    // Paths: A (q >= |dy|) or B; see the listing at 863Dh.
    if q >= ay {
        n += 2;
        if q < 7 {
            n += 4;
        } else {
            n += 2 + (ay == 0) as u32 + 2 + n3 as u32 + 2;
            if q >> 1 == 0 {
                n += 2;
            } else {
                n += 3 + (!roll_cf) as u32 + 2;
            }
        }
    } else {
        n += 2;
        if ay < 7 {
            n += 4;
        } else {
            n += 2;
            if q == 0 {
                n += 2;
            } else {
                n += 2 + n3 as u32 + 2;
                if ay >> 1 == 0 {
                    n += 2;
                } else {
                    n += 3;
                    if roll_cf {
                        n += 2;
                    } else {
                        n += 3;
                    }
                }
            }
        }
    }
    c.run(start, n)?;
    c.push(si0);
    c.set_reg(DI, loc);
    let bp = c.shl1(W, ty, 1);
    let bp = if n1 { c.neg(W, bp) } else { bp };
    let _ = bp;
    let mut bp = scale;
    let bx = c.sub(W, ty, ly);
    let bx = c.neg(W, bx);
    let mut bx = bx;
    let mut di = 1u16;
    if n2 {
        di = c.neg(W, 1);
        bx = c.neg(W, bx);
    }
    let d = c.sub(W, lx, tx);
    let mut ax = d;
    if n3 {
        ax = c.neg(W, d);
    }
    c.set_reg(AX, ax);
    c.set_reg(DX, 0);
    c.m.cpu.mul_div(&mut c.m.hw, W, 6, scale as u32);
    let ax = c.reg(AX);
    let mut dx = d;
    // xchg di, bx
    let (d2, b2) = (bx, di);
    let mut bxr = b2;
    let dir = d2;
    c.cmp(W, ax, dir);
    let mut ax = ax;
    if ax >= dir {
        c.cmp(W, ax, 7);
        if ax < 7 {
            c.xor(W, 0, 0);
            bxr = 0;
            dx = 0;
        } else {
            c.or(W, dir, dir);
            if dir == 0 {
                c.xor(W, 0, 0);
                bxr = 0;
            }
            c.or(W, dx, dx);
            if n3 {
                bp = c.neg(W, bp);
            }
            ax = c.shr1(W, ax, 1);
            if ax != 0 {
                dx = bp;
                let s = c.rol1(W, seed);
                c.set_word(0, s);
                if !roll_cf {
                    c.xor(W, 0, 0);
                    bxr = 0;
                }
            }
        }
    } else {
        c.cmp(W, dir, 7);
        if dir < 7 {
            c.xor(W, 0, 0);
            bxr = 0;
            dx = 0;
        } else {
            c.or(W, ax, ax);
            if ax != 0 {
                c.or(W, dx, dx);
                if n3 {
                    bp = c.neg(W, bp);
                }
                let d3 = c.shr1(W, dir, 1);
                c.set_reg(DI, d3);
                if d3 != 0 {
                    dx = bp;
                    let s = c.rol1(W, seed);
                    c.set_word(0, s);
                    if !roll_cf {
                        c.xor(W, 0, 0);
                        dx = 0;
                    }
                }
            }
        }
    }
    if !(ax < dir && dir >= 7 && ax != 0) {
        c.set_reg(DI, dir);
    }
    c.set_reg(AX, ax);
    c.set_reg(BX, bxr);
    c.set_reg(DX, dx);
    c.set_reg(BP, bp);
    let s = c.pop();
    c.set_reg(SI, s);
    c.ret()
}

/// `loc_06770`: the sprite list BP for the troop SI: none (0) when
/// hidden (bit 10h of +10h) or a plain moving one whose location is not
/// flagged 10h (else 181Fh); moving (bit 40h of +3) by its facing
/// ([`troop_facing_sprite`]); busy (30h) the activity's (16B6h table, for
/// activity 0 by the bits C0h of +19h: 1813h, 1817h, 181Bh); else the
/// job's (179Ch table; 1732h becomes 16AAh at a location flagged 2) and
/// for 1774h / 1732h the one for its heading (+2: 0Ah apart), or with
/// no job by +19h (17BCh, 17C9h, 1806h).
pub fn troop_sprite_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6770)?;
    let mut c = Ctx::new(m);
    let saved = c.m.cpu.flags();
    let (n, _) = sprite_list_walk(&mut c, false);
    c.m.cpu.set_flags(saved);
    c.run(start, n)?;
    let (_, tail) = sprite_list_walk(&mut c, true);
    if tail {
        return troop_facing_sprite(c.m, 0x6827);
    }
    c.ret()
}

/// The code of `loc_06770` (with `write`, AX and BP set): its count, and
/// whether it ends in the jump to `troop_facing_sprite`.
fn sprite_list_walk(c: &mut Ctx, write: bool) -> (u32, bool) {
    let si = c.reg(SI);
    let mut ax = c.reg(AX);
    let mut bp = c.reg(BP);
    let mut n = 2u32;
    let mut tail = false;
    let f10 = c.byte(si.wrapping_add(0x10));
    c.test(B, f10 as u16, 0x10);
    // 0: 678Fh (none), 1: ret, 2: 6792h, 3: 67C5h, 4: 67CAh, 5: 67EDh.
    let mut at = if f10 & 0x10 != 0 { 0 } else { 9 };
    if at == 9 {
        let al = c.byte(si.wrapping_add(3));
        ax = ax & 0xff00 | al as u16;
        n += 3;
        c.test(B, f10 as u16, 0x80);
        at = if f10 & 0x80 != 0 {
            2
        } else {
            n += 2;
            c.or(B, al as u16, al as u16);
            if al & 0x80 == 0 {
                2
            } else {
                n += 4;
                let p = c.word(si.wrapping_add(4));
                let f = c.byte_in(SS, p.wrapping_add(0x0a));
                c.test(B, f as u16, 0x10);
                bp = 0x181f;
                if f & 0x10 != 0 { 1 } else { 0 }
            }
        };
    }
    loop {
        match at {
            0 => {
                n += 2;
                bp = c.xor(W, bp, bp);
                break;
            }
            1 => {
                n += 1;
                break;
            }
            2 => {
                n += 2;
                c.test(B, ax & 0xff, 0x40);
                if ax & 0x40 != 0 {
                    n += 1;
                    tail = true;
                    break;
                }
                n += 2;
                c.test(B, ax & 0xff, 0x30);
                if ax & 0x30 == 0 {
                    at = 3;
                    continue;
                }
                n += 6;
                ax = c.and(W, ax, 0x0f);
                let b = c.shl1(W, ax, 1);
                bp = c.word_in(SS, b.wrapping_add(0x16b6));
                c.or(W, ax, ax);
                if ax != 0 {
                    at = 1;
                    continue;
                }
                n += 3;
                let v = c.byte(si.wrapping_add(0x19));
                let a = c.and(B, v as u16, 0xc0);
                ax = ax & 0xff00 | a;
                if a == 0 {
                    at = 1;
                    continue;
                }
                n += 3;
                bp = 0x1813;
                c.cmp(B, a, 0x80);
                if a == 0x80 {
                    at = 1;
                    continue;
                }
                n += 3;
                bp = 0x1817;
                c.cmp(B, a, 0x40);
                if a == 0x40 {
                    at = 1;
                    continue;
                }
                n += 1;
                bp = 0x181b;
                at = 1;
            }
            3 => {
                n += 2;
                ax = c.and(W, ax, 0x0f);
                if ax != 0 {
                    at = 4;
                    continue;
                }
                n += 2;
                let v = c.byte(si.wrapping_add(0x19));
                c.test(B, v as u16, 0xc0);
                if v & 0xc0 == 0 {
                    at = 4;
                    continue;
                }
                n += 5;
                let a = c.and(B, v as u16, 0xc0);
                ax = ax & 0xff00 | a;
                bp = 0x17bc;
                c.cmp(B, a, 0x80);
                if a == 0x80 {
                    at = 1;
                    continue;
                }
                n += 3;
                bp = 0x17c9;
                c.cmp(B, a, 0x40);
                if a == 0x40 {
                    at = 1;
                    continue;
                }
                n += 1;
                bp = 0x1806;
                at = 1;
            }
            4 => {
                n += 5;
                let b = c.shl1(W, ax, 1);
                bp = c.word_in(SS, b.wrapping_add(0x179c));
                c.cmp(W, bp, 0x1774);
                if bp == 0x1774 {
                    at = 5;
                    continue;
                }
                n += 2;
                c.cmp(W, bp, 0x1732);
                if bp != 0x1732 {
                    at = 1;
                    continue;
                }
                n += 4;
                let p = c.word(si.wrapping_add(4));
                let f = c.byte_in(SS, p.wrapping_add(0x0a));
                c.test(B, f as u16, 2);
                bp = 0x16aa;
                if f & 2 != 0 {
                    at = 1;
                    continue;
                }
                n += 1;
                bp = 0x1732;
                at = 5;
            }
            _ => {
                n += 5;
                let h = c.byte(si.wrapping_add(2));
                let a = c.dec(B, h as u16);
                let a = c.and(B, a, 7);
                ax = ax & 0xff00 | a;
                c.cmp(B, a, 3);
                if a < 3 {
                    at = 1;
                    continue;
                }
                for k in [3u16, 4] {
                    n += 3;
                    bp = c.add(W, bp, 0x0a);
                    c.cmp(B, a, k);
                    if a == k {
                        break;
                    }
                }
                if a > 4 {
                    n += 1;
                    bp = c.add(W, bp, 0x0a);
                }
                at = 1;
            }
        }
    }
    if write {
        c.set_reg(AX, ax);
        c.set_reg(BP, bp);
    }
    (n, tail)
}
