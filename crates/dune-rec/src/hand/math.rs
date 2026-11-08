//! Arithmetic helpers: trig table, random numbers, register save/restore.

use super::{Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DX, SI, SP, SS, Width},
    rec::Bail,
};

/// `tablat_lookup_from_bx_to_ax_bp` (B5A0): sine/cosine lookup. BX is a
/// signed angle index; a negative one reads the mirrored entry and negates
/// the value.
///
/// Exit: AX = value, BP = 2 × scale, BX unchanged. Flags from `shl bp, 1`.
pub fn trig_lookup(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb5a0)?;
    let mut c = Ctx::new(m);
    let bx = c.reg(BX);
    let offset = bx.wrapping_mul(8);
    let negative = offset & 0x8000 != 0;
    c.run(0xb5a0, if negative { 12 } else { 10 })?;
    // push bx ... pop bx: the slot stays written below SP.
    c.below_sp(1, bx);
    // The flags go through the original's operations (a shift leaves some
    // of them as the previous operation set them).
    c.shl1(W, bx, 3);
    let entry = if negative {
        let e = c.neg(W, offset);
        c.set_flag(CF, offset != 0);
        e
    } else {
        offset
    };
    let value = c.word(g::TRIG_TABLE.wrapping_add(entry));
    let scale = c.word(g::TRIG_TABLE.wrapping_add(entry).wrapping_add(2));
    let ax = if negative {
        let v = c.neg(W, value);
        c.set_flag(CF, value != 0);
        v
    } else {
        value
    };
    c.set_reg(AX, ax);
    let bp = c.shl1(W, scale, 1);
    c.set_reg(BP, bp);
    c.ret()
}

/// `shl_cx_and_dx` (DB44): shifts CX and DX left by AL.
///
/// Exit: AX = CX = CX << AL, DX = DX << AL (counts mod 32). Flags from the
/// shift of DX (none changed for a zero count).
pub fn shl_cx_and_dx(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xdb44)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let count = c.reg8(r8::AL); // xchg cx, ax: CL is the old AL
    let cx = c.reg(CX);
    let shifted = c.shl(W, cx, count);
    let dx = c.reg(DX);
    let dx = c.shl(W, dx, count);
    c.set_reg(AX, shifted);
    c.set_reg(CX, shifted);
    c.set_reg(DX, dx);
    c.ret()
}

/// The shared tail of `sub_e16f` and its three variants (E173): given the
/// differences AX (along) and SI (across, before taking its absolute
/// value), a score for something ahead: AX + 2 × |SI| when AX ≥ 6 (signed)
/// and |SI| < 32h, else AX = SI = FFFFh (out of range).
///
/// `prefix` counts the variant's instructions before E173; `ops` does them
/// (with their flags) once the stretch is known not to bail; `along` and
/// `across` are what they leave in AX and SI.
fn nearness(
    c: &mut Ctx,
    entry: u16,
    prefix: u32,
    along: u16,
    across: u16,
    ops: impl FnOnce(&mut Ctx),
) -> Result<(), Bail> {
    let across_negative = across & 0x8000 != 0; // jns, on the last `sub si`
    let abs_across = if across_negative { across.wrapping_neg() } else { across };
    let too_close = (along as i16) < 6;
    let too_wide = abs_across >= 0x32;
    let n = prefix + 1 + across_negative as u32 + 2 + if too_close { 0 } else { 2 } + 3;
    c.run(entry, n)?;
    ops(c);
    if across_negative {
        c.neg(W, across);
    }
    c.cmp(W, along, 6);
    if !too_close {
        c.cmp(W, abs_across, 0x32);
    }
    if too_close || too_wide {
        c.set_reg(AX, 0xffff);
        c.set_reg(SI, 0xffff);
    } else {
        let a = c.add(W, along, abs_across);
        let a = c.add(W, a, abs_across);
        c.set_reg(AX, a);
        c.set_reg(SI, abs_across);
    }
    c.ret()
}

/// `sub_e16f`: along = AX - DX, across = SI - BX.
pub fn nearness_e16f(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe16f)?;
    let mut c = Ctx::new(m);
    let (ax, bx, dx, si) = (c.reg(AX), c.reg(BX), c.reg(DX), c.reg(SI));
    nearness(&mut c, start, 2, ax.wrapping_sub(dx), si.wrapping_sub(bx), |c| {
        c.sub(W, ax, dx);
        c.sub(W, si, bx);
    })
}

/// `sub_e18c`: along = DX - AX, across = SI - BX.
pub fn nearness_e18c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe18c)?;
    let mut c = Ctx::new(m);
    let (ax, bx, dx, si) = (c.reg(AX), c.reg(BX), c.reg(DX), c.reg(SI));
    nearness(&mut c, start, 4, dx.wrapping_sub(ax), si.wrapping_sub(bx), |c| {
        let d = c.sub(W, ax, dx);
        c.neg(W, d);
        c.sub(W, si, bx);
    })
}

/// `sub_e192`: along = BX - SI, across = AX - DX.
pub fn nearness_e192(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe192)?;
    let mut c = Ctx::new(m);
    let (ax, bx, dx, si) = (c.reg(AX), c.reg(BX), c.reg(DX), c.reg(SI));
    nearness(&mut c, start, 5, bx.wrapping_sub(si), ax.wrapping_sub(dx), |c| {
        let d = c.sub(W, si, bx);
        c.neg(W, d);
        c.sub(W, ax, dx);
    })
}

/// `sub_e19b`: along = SI - BX, across = AX - DX.
pub fn nearness_e19b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe19b)?;
    let mut c = Ctx::new(m);
    let (ax, bx, dx, si) = (c.reg(AX), c.reg(BX), c.reg(DX), c.reg(SI));
    nearness(&mut c, start, 4, si.wrapping_sub(bx), ax.wrapping_sub(dx), |c| {
        c.sub(W, si, bx);
        c.sub(W, ax, dx);
    })
}

/// `save_regs` (E270): pushes BX, CX, DX, SI, DI, BP for its caller, under
/// AX, which takes the place of its own return address. `restore_regs`
/// undoes it. Registers and flags unchanged.
pub fn save_regs(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe270)?;
    let mut c = Ctx::new(m);
    c.run(start, 12)?;
    let ret_slot = c.reg(SP);
    let ret = c.word_in(SS, ret_slot);
    for r in [BX, CX, DX, SI, DI, BP] {
        let v = c.reg(r);
        c.push(v);
    }
    let ax = c.reg(AX);
    c.set_word_in(SS, ret_slot, ax);
    c.push(ret); // push ax (the return address); ret
    c.ret()
}

/// `restore_regs` (E283): pops what `save_regs` pushed: BP, DI, SI, DX, CX,
/// BX, then AX from under them, and returns over it. Flags unchanged.
pub fn restore_regs(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe283)?;
    let mut c = Ctx::new(m);
    c.run(start, 10)?;
    let ret = c.pop();
    let ax_slot = c.reg(SP).wrapping_add(0x0c);
    let ax = c.word_in(SS, ax_slot);
    c.set_word_in(SS, ax_slot, ret);
    c.set_reg(AX, ax);
    for r in [BP, DI, SI, DX, CX, BX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// The game's linear congruential generator: seed = seed × `factor` + 1
/// (mod 2^16); returns AL = the new seed's high byte, AH = byte 2 of the
/// product. Flags from the `inc`.
fn lcg(c: &mut Ctx, seed_at: u16, factor: u16) {
    let dx = c.reg(DX);
    c.push(dx);
    let seed = c.word(seed_at);
    c.set_reg(AX, seed);
    c.mul16(factor); // DX:AX = seed × factor
    let ax = c.reg(AX);
    let seed = c.inc(W, ax);
    c.set_word(seed_at, seed);
    let dl = c.reg8(r8::DL);
    c.set_reg(AX, (dl as u16) << 8 | seed >> 8);
    let dx = c.pop();
    c.set_reg(DX, dx);
}

/// `rand` (E3CC): the next random word (see [`lcg`]); DX kept.
pub fn rand(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe3cc)?;
    let mut c = Ctx::new(m);
    c.run(start, 10)?;
    lcg(&mut c, g::RAND_SEED, 0xcbd1);
    c.ret()
}

/// `rand_masked` (E3B7): a random word from the second generator, ANDed
/// with BX. Flags from the `and`; DX kept.
pub fn rand_masked(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe3b7)?;
    let mut c = Ctx::new(m);
    c.run(start, 11)?;
    lcg(&mut c, g::RAND_MASKED_SEED, 0xe56d);
    let (ax, bx) = (c.reg(AX), c.reg(BX));
    let r = c.and(Width::W, ax, bx);
    c.set_reg(AX, r);
    c.ret()
}

/// `loc_0e3df`: a random number 0..=BX from the third generator (factor
/// CBD1h, as [`rand`]): drawn under the next power of two, again while
/// above BX. AX = it (0 when BX is 0); CX, DX kept. Flags from the last
/// `cmp ax, bx` (from `or ax, ax` when BX is 0).
pub fn rand_below(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe3df)?;
    let mut c = Ctx::new(m);
    let bx = c.reg(BX);
    if bx == 0 {
        c.run(start, 8)?;
        let (cx, dx) = (c.reg(CX), c.reg(DX));
        c.push(cx);
        c.push(dx);
        c.set_reg(AX, 0);
        c.or(W, 0, 0);
        c.pop();
        c.pop();
        return c.ret();
    }
    let bits = 16 - bx.leading_zeros();
    let mask = (0xffffu32 << bits) as u16 ^ 0xffff;
    // The draws, purely, to count them.
    let mut seed = c.word(g::RAND_BELOW_SEED);
    let mut draws = 0;
    loop {
        draws += 1;
        let p = seed as u32 * 0xcbd1;
        seed = (p as u16).wrapping_add(1);
        let v = ((p >> 16) as u16 & 0xff) << 8 | seed >> 8;
        if v & mask <= bx {
            break;
        }
    }
    c.run(start, 7 + 3 * bits + 10 * draws + 3)?;
    let (cx, dx) = (c.reg(CX), c.reg(DX));
    c.push(cx);
    c.push(dx);
    c.or(W, bx, bx);
    let mut ax = bx;
    let mut m = 0xffffu16;
    loop {
        m = c.shl1(W, m, 1);
        ax = c.shr1(W, ax, 1);
        if ax == 0 {
            break;
        }
    }
    let m = !m;
    c.set_reg(CX, m);
    let mut v;
    loop {
        let seed = c.word(g::RAND_BELOW_SEED);
        c.set_reg(AX, seed);
        c.set_reg(DX, 0xcbd1);
        c.mul16(0xcbd1);
        let lo = c.reg(AX);
        let seed = c.inc(W, lo);
        c.set_word(g::RAND_BELOW_SEED, seed);
        let dl = c.reg8(r8::DL);
        v = (dl as u16) << 8 | seed >> 8;
        v = c.and(W, v, m);
        c.cmp(W, v, bx);
        if v <= bx {
            break;
        }
    }
    c.set_reg(AX, v);
    let dx = c.pop();
    c.set_reg(DX, dx);
    let cx = c.pop();
    c.set_reg(CX, cx);
    c.ret()
}


/// `loc_00cf2`: AX (after `add al, bl`) rotated right by 5, AH then
/// shifted right by 3: BH = AH, DH = AL; BL, DL their old high bytes.
/// CL = 3.
pub fn unpack_0cf2(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0cf2)?;
    let mut c = Ctx::new(m);
    c.run(start, 10)?;
    unpack(&mut c);
    c.ret()
}

/// [`unpack_0cf2`]'s instructions (counted by the caller).
fn unpack(c: &mut Ctx) {
    let (ax, bx, dx) = (c.reg(AX), c.reg(BX), c.reg(DX));
    let al = c.add(Width::B, ax & 0xff, bx & 0xff);
    let ax = c.m.cpu.shift(1, Width::W, (ax & 0xff00 | al) as u32, 5) as u16; // ror ax, 5
    let ah = c.m.cpu.shift(5, Width::B, (ax >> 8) as u32, 3) as u16; // shr ah, 3
    let ax = ah << 8 | ax & 0xff;
    c.set_reg(AX, ax);
    c.set_reg8(r8::CL, 3);
    c.set_reg(BX, ah << 8 | bx >> 8);
    c.set_reg(DX, (ax & 0xff) << 8 | dx >> 8);
}

/// `loc_00cea`: [`unpack_0cf2`] of DL (signed: of -DL, DH negated after)
/// with AH = 0.
pub fn unpack_signed(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0cea)?;
    let mut c = Ctx::new(m);
    let dl = c.reg8(r8::DL);
    let neg = dl & 0x80 != 0;
    c.run(start, if neg { 6 } else { 14 })?;
    c.set_reg(AX, 0);
    c.xor(W, 0, 0);
    c.set_reg8(r8::AL, dl);
    c.or(Width::B, dl as u16, dl as u16);
    if !neg {
        unpack(&mut c);
        return c.ret();
    }
    let al = c.neg(Width::B, dl as u16);
    c.set_reg8(r8::AL, al as u8);
    c.call(0x0cf2, 0x0d0a)?;
    c.run(0x0d0a, 2)?;
    let dh = c.reg8(r8::DH);
    let dh = c.neg(Width::B, dh as u16);
    c.set_reg8(r8::DH, dh as u8);
    c.ret()
}
