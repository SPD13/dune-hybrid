//! Troops: 1Bh-byte records from [`g::TROOPS`], numbered from 1; and the
//! per-location byte 19h, its bits spread over bytes and back.

use super::game::{self, Location, Troop, troop_state};
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

/// `loc_0693b`: AX = the troop SI's job / 4 (its activity).
pub fn troop_activity(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x693b)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    c.run(start, 5)?;
    let job = troop.job(&mut c);
    c.set_reg(AX, (job >> 2) as u16);
    c.ret()
}

/// `loc_066b1`: the troop SI taken out of its location's chain
/// (`unlink_troop`), its particle removed if it has one
/// (`troop_particle`, ZF), then reset: at [`NOWHERE`], byte 3 A0h (not a
/// troop, started), size 0. SI, DI kept.
pub fn reset_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x66b1)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x858c, 0x66b4)?;
    c.run(0x66b4, 3)?;
    c.save(&[SI, DI]);
    c.call(0x6917, 0x66b9)?;
    if c.flag(ZF) {
        c.run(0x66b9, 2)?;
        c.call(code::PARTICLES_REMOVE_PARTICLE, 0x66be)?;
        c.run(0x66be, 6)?;
    } else {
        c.run(0x66b9, 7)?;
    }
    c.restore(&[SI, DI]);
    let troop = Troop(c.reg(SI));
    troop.set_location(&mut c, NOWHERE);
    troop.set_state(&mut c, troop_state::NOT_A_TROOP | troop_state::STARTED);
    troop.set_size(&mut c, 0);
    c.ret()
}

/// The location of a reset troop (`reset_troop`): just past the troops.
const NOWHERE: u16 = 0x0fbc;

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

/// `loc_069a3`: the particle at [`PARTICLE_4752`] (if any) removed and
/// forgotten. DI kept.
pub fn remove_particle_4752(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x69a3)?;
    let mut c = Ctx::new(m);
    let particle = c.word(PARTICLE_4752);
    c.run(start, if particle == 0 { 7 } else { 6 })?;
    c.save(&[DI]);
    c.set_word(PARTICLE_4752, 0);
    c.set_reg(DI, particle);
    if particle != 0 {
        c.call(code::PARTICLES_REMOVE_PARTICLE, 0x69b1)?;
        c.run(0x69b1, 2)?;
    }
    c.restore(&[DI]);
    c.ret()
}

/// A particle remembered at [4752h] (`remove_particle_4752`).
const PARTICLE_4752: u16 = 0x4752;

/// `loc_06b25`: the record at SI stamped with the game time (AX 0).
pub fn stamp_record(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6b25)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let (record, time) = (c.reg(SI), game::game_time(&mut c));
    game::stamp(&mut c, record, time);
    c.set_reg(AX, 0);
    c.ret()
}

/// `loc_06e02`: `lose_troop_764d` for each troop (`for_each_troop`, CX 0),
/// again while that leaves CX set (some troop was lost).
pub fn repeat_764d(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6e02)?;
    let mut c = Ctx::new(m);
    let (mut at, mut cost) = (start, 3);
    loop {
        c.run(at, cost)?;
        c.set_reg(CX, 0);
        c.set_reg(BP, 0x764d);
        c.call(code::FOR_EACH_TROOP, 0x6e0a)?;
        if c.reg(CX) == 0 {
            c.run(0x6e0a, 3)?;
            return c.ret();
        }
        (at, cost) = (0x6e0a, 5);
    }
}

/// `loc_06ebf`: `redo_job_6ecb` for each troop of the troop SI's location
/// (`for_each_troop_below_80` with DI that location). DI kept.
pub fn for_troop_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6ebf)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.save(&[DI]);
    let place = Troop(c.reg(SI)).location(&mut c);
    c.set_reg(DI, place);
    c.set_reg(BP, 0x6ecb);
    c.call(code::FOR_RECORD, 0x6ec9)?;
    c.run(0x6ec9, 2)?;
    c.restore(&[DI]);
    c.ret()
}

/// `loc_06f48`: the troop SI's motivation up by AL, to at most 100 (the
/// byte wraps first, as the original's `add` does).
pub fn add_motivation(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6f48)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let v = troop.motivation(&mut c).wrapping_add(c.reg8(r8::AL));
    c.run(start, if v <= 100 { 4 } else { 5 })?;
    troop.set_motivation(&mut c, v.min(100));
    c.ret()
}

/// From 7085 (`prefix` instructions before, done by `ops`): the troop SI
/// marked.
fn mark_troop(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx, Troop)) -> Result<(), Bail> {
    let troop = Troop(c.reg(SI));
    c.run(at, prefix + 2)?;
    ops(c, troop);
    let s = troop.state(c);
    troop.set_state(c, s | troop_state::MARKED);
    c.ret()
}

/// `loc_07085`: see [`mark_troop`].
pub fn mark_troop_10(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7085)?;
    mark_troop(&mut Ctx::new(m), start, 0, |_, _| {})
}

/// `loc_0719c`: bit 200h of the troop's word 10h, then [`mark_troop`].
pub fn mark_troop_200_10(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x719c)?;
    mark_troop(&mut Ctx::new(m), start, 2, |c, troop| {
        let w = troop.word_10(c);
        troop.set_word_10(c, w | 0x200);
    })
}

/// `loc_071b2`: `loc_029f0` with AH = 0Fh (SI, DI kept).
pub fn call_29f0_f(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x71b2)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_reg8(r8::AH, 0x0f);
    c.save(&[SI, DI]);
    c.call(0x29f0, 0x71b9)?;
    c.run(0x71b9, 3)?;
    c.restore(&[SI, DI]);
    c.ret()
}

/// `loc_0765e`: the sum of byte 19h over all the locations, less 10, added
/// to the location DI's byte 19h, unless the sum is below 10 (registers
/// kept).
pub fn add_location_sum(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x765e)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SAVE_REGS, 0x7661)?;
    let locations = Location::all(&mut c);
    let mut sum = 0u8;
    for l in &locations {
        sum = sum.wrapping_add(l.byte_19(&mut c));
    }
    let below = sum < 10;
    c.run(0x7661, 2 + 4 * locations.len() as u32 + if below { 3 } else { 4 })?;
    let extra = sum.wrapping_sub(10);
    let end = locations.last().map_or(Location::FIRST, |l| l.offset()).wrapping_add(Location::SIZE);
    c.set_reg(SI, end);
    c.set_reg(CX, extra as u16);
    if !below {
        let to = Location(c.reg(DI));
        let v = to.byte_19(&mut c).wrapping_add(extra);
        to.set_byte_19(&mut c, v);
    }
    c.call(code::RESTORE_REGS, 0x767c)?;
    c.run(0x767c, 1)?;
    c.ret()
}

/// The bits of a location's byte 19h, one per byte (bit 7 first).
const SPREAD_BITS: u16 = 0x4705;
const SPREAD_BITS_END: u16 = 0x470c;

/// `loc_07efb`: bits 7 to 1 of the record SI's byte 19h, one per byte at
/// [`SPREAD_BITS`] (ES = DS; AX as its last rotate left it). DI kept.
pub fn spread_location_bits(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7efb)?;
    let mut c = Ctx::new(m);
    if c.flag(DF) {
        return Err(Bail);
    }
    let n = SPREAD_BITS_END - SPREAD_BITS;
    c.run(start, 5 + 5 * n as u32 + 2)?;
    c.save(&[DI]);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    let bits = c.byte(c.reg(SI).wrapping_add(0x19));
    for i in 0..n {
        c.set_byte(SPREAD_BITS + i, bits >> (7 - i) & 1);
    }
    c.set_reg(AX, ((bits << n) as u16) << 8 | (bits >> (8 - n) & 1) as u16);
    c.restore(&[DI]);
    c.ret()
}

/// `loc_07f11`: [`SPREAD_BITS`] packed back into the record SI's byte 19h
/// (bits 7 to 1; bit 0 from what the rotates leave). SI kept.
pub fn pack_location_bits(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7f11)?;
    let mut c = Ctx::new(m);
    let n = (SPREAD_BITS_END - SPREAD_BITS) as u32;
    c.run(start, 4 + 4 * n + 4)?;
    c.save(&[SI]);
    // lodsb backwards, each byte's bit 0 rotated into the top of AX.
    let mut ax = 0u16;
    for at in (SPREAD_BITS..SPREAD_BITS_END).rev() {
        ax = (ax & 0xff00 | c.byte(at) as u16).rotate_right(1);
    }
    c.set_reg(AX, ax);
    c.restore(&[SI]);
    c.set_flag(DF, false);
    let si = c.reg(SI);
    c.set_byte(si.wrapping_add(0x19), (ax >> 8) as u8);
    c.ret()
}

/// `loc_07f5f`: for each set bit of the record SI's byte 19h (bit 7
/// first), one more in the matching need of the location DI. DI kept, AL
/// 0.
pub fn count_location_bits(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7f5f)?;
    let mut c = Ctx::new(m);
    let bits = c.byte(c.reg(SI).wrapping_add(0x19));
    let place = Location(c.reg(DI));
    let walk = BitWalk::of(bits);
    c.run(start, 5 + walk.cost + walk.set.len() as u32 + 2)?;
    c.save(&[DI]);
    for &k in &walk.set {
        let v = place.need(&mut c, k).wrapping_add(1);
        place.set_need(&mut c, k, v);
    }
    c.set_reg8(r8::AL, 0);
    c.restore(&[DI]);
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

/// `loc_0668f`: the troop SI started, unless it is already or bit 80h of
/// its word 10h is set: its byte 19h cleared, its time of change now (AX
/// that time), and charisma 4 less (`lose_charisma`; DI kept).
pub fn start_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x668f)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let state = troop.state(&mut c);
    if state & troop_state::STARTED != 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    if troop.word_10(&mut c) & 0x80 != 0 {
        c.run(start, 5)?;
        return c.ret();
    }
    c.run(start, 11)?;
    troop.set_state(&mut c, state | troop_state::STARTED);
    troop.set_byte_19(&mut c, 0);
    let now = game::game_time(&mut c);
    c.set_reg(AX, now);
    troop.set_changed_at(&mut c, now);
    c.save(&[DI]);
    c.set_reg8(r8::AL, 4);
    c.call(0x6fb0, 0x66af)?;
    c.run(0x66af, 2)?;
    c.restore(&[DI]);
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

/// `loc_06cfc`: the location DI's byte 0Bh one more, up to 0Ch, for a
/// location with byte 8 below 20h and without bit 1 of byte 0Ah; then
/// `loc_0644e`. SI, DI kept.
pub fn grow_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6cfc)?;
    let mut c = Ctx::new(m);
    let place = Location(c.reg(DI));
    let stage = place.byte_0b(&mut c);
    let cost = if place.byte_08(&mut c) >= 0x20 {
        3
    } else if stage >= 0x0c {
        5
    } else if place.byte_0a(&mut c) & 1 != 0 {
        7
    } else {
        10
    };
    c.run(start, cost)?;
    let grows = cost == 10;
    if !grows {
        return c.ret();
    }
    place.set_byte_0b(&mut c, stage + 1);
    c.save(&[SI, DI]);
    c.call(0x644e, 0x6d16)?;
    c.run(0x6d16, 3)?;
    c.restore(&[SI, DI]);
    c.ret()
}

/// `loc_06edd`: the troop SI's skill BX up by AL, to at most 5Fh. When it
/// reaches a new step of 16, the low bits of its word 10h say which skill
/// (BX + 1; BX then holds that). DL = the skill before, AL = the steps it
/// changed (old ^ new, top four bits).
pub fn add_skill(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6edd)?;
    let mut c = Ctx::new(m);
    let (troop, k, gain) = (Troop(c.reg(SI)), c.reg(BX), c.reg8(r8::AL));
    let old = troop.skill(&mut c, k);
    let sum = old.wrapping_add(gain);
    let new = sum.min(0x5f);
    let stepped = (new ^ old) & 0xf0;
    c.run(start, 8 + (sum > 0x5f) as u32 + if stepped != 0 { 6 } else { 1 })?;
    troop.set_skill(&mut c, k, new);
    c.set_reg8(r8::DL, old);
    c.set_reg8(r8::AL, stepped);
    if stepped != 0 {
        let which = k.wrapping_add(1);
        let w = troop.word_10(&mut c);
        let w = w & 0xff00 | (w & 0xfc | which & 0xff);
        troop.set_word_10(&mut c, w);
        c.set_reg(BX, which);
        c.set_reg(AX, w);
    }
    c.ret()
}

/// `loc_06f56`: every troop's motivation up by AL (at most 100; the byte
/// wraps first), but those not yours or started (bits A0h of byte 3).
pub fn add_motivation_to_all(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6f56)?;
    let mut c = Ctx::new(m);
    let gain = c.reg8(r8::AL);
    const SKIPPED: u8 = troop_state::NOT_A_TROOP | troop_state::STARTED;
    let mut new = Vec::new();
    let mut cost = 4;
    for troop in Troop::all() {
        cost += 5;
        if troop.state(&mut c) & SKIPPED == 0 {
            let v = troop.motivation(&mut c).wrapping_add(gain);
            cost += 3 + (v > 100) as u32;
            new.push((troop, v.min(100)));
        }
    }
    c.run(start, cost)?;
    c.save(&[SI]);
    for (troop, v) in new {
        troop.set_motivation(&mut c, v);
    }
    c.restore(&[SI]);
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

/// `loc_06efd`: AL = the troop SI's effective motivation, AH its job
/// (bit 0 cleared but for job 6): its motivation, 20 more while [0FAh] is
/// set; for job 6 at the location the player is at 30 more, at most 100;
/// jobs 8 and 9 100; others at most 100. In phases 64h to 67h 40 less, at
/// least 10 (signed: a result below 0 is 10 too).
pub fn troop_motivation(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6efd)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let job = troop.job(&mut c);
    let mut v = troop.motivation(&mut c);
    let bonus = c.byte(0xfa) != 0;
    let mut cost = 7;
    if bonus {
        v = v.wrapping_add(20);
        cost += 1;
    }
    let mut ah = job;
    if job == 6 {
        let home = troop.location(&mut c) == game::here(&mut c);
        cost += 5;
        if home {
            v = v.wrapping_add(30);
            cost += 4;
            if v >= 100 {
                v = 100;
                cost += 1;
            }
        }
    } else {
        ah = job & 0xfe;
        if ah == 8 {
            v = 100;
            cost += 4;
        } else {
            cost += 5;
            if v >= 100 {
                v = 100;
                cost += 1;
            }
        }
    }
    let phase = c.byte(g::GAME_PHASE);
    if phase < 0x64 {
        cost += 3;
    } else if phase >= 0x68 {
        cost += 5;
    } else {
        v = v.wrapping_sub(40);
        cost += 8;
        if (v as i8) < 10 {
            v = 10;
            cost += 1;
        }
    }
    c.run(start, cost)?;
    if job == 6 {
        // The location compared through DI (saved and given back).
        c.save(&[DI]);
        c.restore(&[DI]);
    }
    c.set_reg(AX, (ah as u16) << 8 | v as u16);
    c.ret()
}

/// `loc_06fb0`: charisma down by AL, to at least 1. For each step of 4 it
/// drops through, every troop but those not yours or started loses
/// motivation (`decrease_troop_motivation`, 6F93h). AH = the new charisma
/// rounded down to 4, AL the steps; SI kept.
pub fn lose_charisma(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6fb0)?;
    let mut c = Ctx::new(m);
    let loss = c.reg8(r8::AL);
    let before = game::charisma(&mut c);
    let floored = !matches!(before.checked_sub(loss), Some(v) if v > 0);
    let after = if floored { 1 } else { before - loss };
    let steps = ((before & 0xfc).wrapping_sub(after & 0xfc)) >> 2;
    c.run(start, 11 + floored as u32 + if steps == 0 { 1 } else { 2 })?;
    game::set_charisma(&mut c, after);
    c.set_reg(AX, ((after & 0xfc) as u16) << 8 | steps as u16);
    if steps == 0 {
        return c.ret();
    }
    const SKIPPED: u8 = troop_state::NOT_A_TROOP | troop_state::STARTED;
    c.save(&[SI]);
    let mut troop = Troop(g::TROOPS);
    loop {
        c.set_reg(SI, troop.offset());
        if troop.state(&mut c) & SKIPPED == 0 {
            c.run(0x6fd1, 3)?;
            c.call(0x6f93, 0x6fda)?;
        } else {
            c.run(0x6fd1, 2)?;
        }
        // On from SI as the call left it.
        let next = c.reg(SI).wrapping_add(Troop::SIZE);
        let more = next < g::TROOPS_END;
        c.run(0x6fda, if more { 3 } else { 5 })?;
        c.set_reg(SI, next);
        if !more {
            c.restore(&[SI]);
            return c.ret();
        }
        troop = Troop(next);
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

/// `loc_0697c`: for the troop SI with a particle (`troop_particle`, ZF),
/// at its own location or at one `loc_05ed0` accepts (ZF; DI the troop's
/// location): unless `troop_screen_place` sets CF, a particle made
/// (`loc_0c5cf` with BP 18FDh), given bit 40h of its byte 0Ch and
/// remembered at [`PARTICLE_4752`].
pub fn mark_troop_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x697c)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x6917, 0x697f)?;
    if !c.flag(ZF) {
        c.run(0x697f, 2)?;
        return c.ret();
    }
    let troop = Troop(c.reg(SI));
    if troop.state(&mut c) & troop_state::OWN_LOCATION != 0 {
        c.run(0x697f, 4)?;
    } else {
        c.run(0x697f, 5)?;
        let place = troop.location(&mut c);
        c.set_reg(DI, place);
        c.call(0x5ed0, 0x698d)?;
        let accepted = c.flag(ZF);
        c.run(0x698d, 2)?;
        if !accepted {
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
    let particle = c.reg(DI);
    let b = c.byte(particle.wrapping_add(0x0c));
    c.set_byte(particle.wrapping_add(0x0c), b | 0x40);
    c.set_word(PARTICLE_4752, particle);
    c.ret()
}

/// `loc_0758d`: AL (AH 0) = DX × (255 - 2 × the troop SI's skill 1) / 256
/// (255 on overflow), at most its size. DX kept.
pub fn scaled_by_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x758d)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let amount = c.reg(DX);
    let skill = troop.skill(&mut c, 1);
    let cap = troop.size(&mut c);
    let factor = 0xffu8.wrapping_sub(skill).wrapping_sub(skill);
    let product = amount as u32 * factor as u32;
    let over = product >> 16 != 0;
    let scaled = if over { 0xff } else { (product >> 8) as u8 };
    c.run(start, 14 + over as u32 + (scaled > cap) as u32)?;
    c.save(&[DX]);
    c.restore(&[DX]);
    c.set_reg(AX, scaled.min(cap) as u16);
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

/// `loc_07f75`: for each bit of the troop SI's byte 19h, from the top,
/// the need of the location DI it stands for one lower (not below 0).
/// DI kept, AL 0.
pub fn lower_by_bits(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7f75)?;
    let mut c = Ctx::new(m);
    let bits = Troop(c.reg(SI)).byte_19(&mut c);
    let place = Location(c.reg(DI));
    let walk = BitWalk::of(bits);
    let mut cost = 5 + walk.cost + 2;
    let mut lowered = Vec::new();
    for &k in &walk.set {
        let v = place.need(&mut c, k);
        cost += 2 + (v == 0) as u32;
        if v != 0 {
            lowered.push((k, v - 1));
        }
    }
    c.run(start, cost)?;
    c.save(&[DI]);
    for (k, v) in lowered {
        place.set_need(&mut c, k, v);
    }
    c.set_reg8(r8::AL, 0);
    c.restore(&[DI]);
    c.ret()
}

/// The walk `lower_by_bits`, `count_location_bits` and `skill_needs` make
/// over a byte of bits, one byte of needs per bit from the top, while set
/// bits are left (`shl al,1; jb; jnz` per step).
struct BitWalk {
    /// The needs (0 for bit 7) whose bit is set.
    set: Vec<u16>,
    /// The needs stepped over.
    steps: u16,
    /// Its instructions, but those for the set bits.
    cost: u32,
}

impl BitWalk {
    fn of(bits: u8) -> BitWalk {
        let has = |k: u16| k < 8 && bits & (0x80 >> k) != 0;
        let any_from = |k: u16| k < 8 && bits & (0xff >> k) != 0;
        let (mut set, mut cost, mut k) = (Vec::new(), 0, 0);
        loop {
            if has(k) {
                set.push(k);
            }
            cost += 3;
            if !has(k + 1) {
                cost += 1;
                if !any_from(k + 2) {
                    return BitWalk { set, steps: k + 1, cost };
                }
            }
            k += 1;
        }
    }
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

/// `loc_0727d`: bit 4 of the location DI's byte 0Ah cleared; then
/// `clear_bit_7289` for each of its troops (`for_each_troop_below_80`),
/// CX 0.
pub fn clear_location_4(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x727d)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let place = Location(c.reg(DI));
    let b = place.byte_0a(&mut c);
    place.set_byte_0a(&mut c, b & !4);
    c.set_reg(BP, 0x7289);
    c.set_reg(CX, 0);
    for_each_troop_below_80(c.m, 0x661d)
}

/// `loc_07298`: `tally_72a2` for each troop of the location DI
/// (`for_each_troop_below_80`), CX, DX 0.
pub fn for_location_troops_72a2(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7298)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_reg(BP, 0x72a2);
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

/// `loc_071bc`: from phase 35h on, for a troop SI with bit 40h of word
/// 12h, one time in 8 (the seed at [`SEED_0`] rotated left 3, its low
/// three bits zero): `mark_troop_200_10`, bit 8000h of word 10h, bit 4 of
/// the location DI's byte 0Ah, and the message pair 0F03h
/// (`call_29f0_f`).
pub fn maybe_mark_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x71bc)?;
    let mut c = Ctx::new(m);
    if c.byte(g::GAME_PHASE) < 0x35 {
        c.run(start, 3)?;
        return c.ret();
    }
    if Troop(c.reg(SI)).word_12(&mut c) & 0x40 == 0 {
        c.run(start, 5)?;
        return c.ret();
    }
    let seed = c.word(SEED_0).rotate_left(3);
    c.run(start, 10)?;
    c.set_word(SEED_0, seed);
    if seed & 7 != 0 {
        return c.ret();
    }
    c.call(0x719c, 0x71e1)?;
    c.run(0x71e1, 4)?;
    let troop = Troop(c.reg(SI));
    let w = troop.word_10(&mut c);
    troop.set_word_10(&mut c, w | 0x8000);
    let place = Location(c.reg(DI));
    let b = place.byte_0a(&mut c);
    place.set_byte_0a(&mut c, b | 4);
    c.set_reg8(r8::AL, 3);
    call_29f0_f(c.m, 0x71b2)
}

/// A seed `maybe_mark_troop` rotates ([0]).
const SEED_0: u16 = 0;

/// `loc_066ce`: a person SI not yours (byte 3 bit 80h) and without bit 80h
/// of word 10h joins you: one more joined ([28h]; reaching [1178h],
/// `loc_0121f` with AL 4Ch), charisma 1 more (6F78h), its job 2 (keeping
/// only its STARTED bit), stamped (`stamp_record`), its day the day
/// (`loc_01ac5`); its location's byte 0Bh 2 (and `loc_0644e`) if that was
/// 0. DI = its location.
pub fn troop_joins(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x66ce)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    if troop.state(&mut c) & troop_state::NOT_A_TROOP == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    if troop.word_10(&mut c) & 0x80 != 0 {
        c.run(start, 5)?;
        return c.ret();
    }
    let joined = c.byte(JOINED).wrapping_add(1);
    let enough = joined >= c.byte(JOINED_ENOUGH);
    c.run(start, if enough { 11 } else { 10 })?;
    c.set_byte(JOINED, joined);
    c.set_reg8(r8::AL, joined);
    if enough {
        c.save(&[SI]);
        c.set_reg8(r8::AL, 0x4c);
        c.call(0x121f, 0x66ed)?;
        c.run(0x66ed, 3)?;
        c.restore(&[SI]);
    }
    c.set_reg8(r8::AL, 1);
    c.call(0x6f78, 0x66f3)?;
    c.run(0x66f3, 3)?;
    let troop = Troop(c.reg(SI));
    let s = troop.state(&mut c);
    troop.set_state(&mut c, s & troop_state::STARTED | 2);
    c.call(0x6b25, 0x66fe)?;
    c.run(0x66fe, 1)?;
    c.call(0x1ac5, 0x6701)?;
    let troop = Troop(c.reg(SI));
    let place = Location(troop.location(&mut c));
    let first = place.byte_0b(&mut c) == 0;
    c.run(0x6701, if first { 6 } else { 5 })?;
    let day = c.reg8(r8::AL);
    troop.set_day(&mut c, day);
    c.set_reg(DI, place.offset());
    if !first {
        return c.ret();
    }
    place.set_byte_0b(&mut c, 2);
    c.call(0x644e, 0x6714)?;
    c.run(0x6714, 1)?;
    c.ret()
}

/// How many have joined you ([28h]).
const JOINED: u16 = 0x28;
/// The count of [`JOINED`] from which `troop_joins` calls 121Fh.
const JOINED_ENOUGH: u16 = 0x1178;

/// `loc_06d7b`: once every 40h ticks of the game time, the troop SI's
/// skills 2, 1, 0 (bytes 18h, 17h, 16h) each 1 lower (not below 0) where
/// its word 12h has the bit its activity selects (C000h rotated left by
/// the activity, `troop_activity`). AX = those bits shifted out, CL the
/// activity.
pub fn troop_skills_decay(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6d7b)?;
    let mut c = Ctx::new(m);
    let time = game::game_time(&mut c);
    c.run(start, 3)?;
    if time & 0x3f != 0 {
        return c.ret();
    }
    c.call(0x693b, 0x6d87)?;
    let (troop, activity) = (Troop(c.reg(SI)), c.reg8(r8::AL));
    let decays = 0xc000u16.rotate_left(activity as u32 & 0x0f) & troop.word_12(&mut c);
    let mut cost = 5;
    let mut lowered = Vec::new();
    if decays == 0 {
        cost += 1;
    } else {
        for (i, k) in [2u16, 1, 0].into_iter().enumerate() {
            cost += 2;
            if decays & (0x8000 >> i) != 0 {
                cost += 2;
                let v = troop.skill(&mut c, k);
                if v != 0 {
                    cost += 1;
                    lowered.push((k, v - 1));
                }
            }
        }
        cost += 1;
    }
    c.run(0x6d87, cost)?;
    c.set_reg8(r8::CL, activity);
    for (k, v) in lowered {
        troop.set_skill(&mut c, k, v);
    }
    c.set_reg(AX, decays << 3);
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
    if c.flag(DF) {
        return Err(Bail);
    }
    // 8 instructions, `rep movsb` counted once per byte as well.
    c.run(0x7f2a, 8 + 7)?;
    let place = Location(c.reg(DI));
    let out = c.reg(BX);
    c.save(&[DI]);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    // rep movsb of the 7 needs.
    for k in 0..7 {
        let v = place.need(c, k);
        c.set_byte(out + k, v);
    }
    c.set_reg(SI, place.offset() + 0x14 + 7);
    c.set_reg(DI, out + 7);
    c.set_reg(CX, 0);
    let mut id = place.first_troop(c);
    c.set_reg8(r8::AL, id);
    // Each troop of the chain lowers the needs its bits cover.
    loop {
        if id == 0 {
            c.run(0x7f3a, 4)?;
            c.restore(&[DI]);
            return c.ret();
        }
        c.run(0x7f3a, 3)?;
        c.call(0x6906, 0x7f41)?;
        let troop = Troop(c.reg(SI));
        let walk = BitWalk::of(troop.byte_19(c));
        let mut cost = 4 + walk.cost + 2;
        let mut lowered = Vec::new();
        for &k in &walk.set {
            let v = c.byte(out.wrapping_add(k));
            cost += 2 + (v == 0) as u32;
            if v != 0 {
                lowered.push((k, v - 1));
            }
        }
        c.run(0x7f41, cost)?;
        for (k, v) in lowered {
            c.set_byte(out.wrapping_add(k), v);
        }
        c.set_reg(DI, out.wrapping_add(walk.steps));
        id = troop.next(c);
        c.set_reg8(r8::AL, id);
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
/// words `off`, `off` + 2 of SI: `get_map_position` (DX, BX; SI kept),
/// then BP the scale at SS:[4880h + |2·BX|], AX the larger of |word off -
/// DX| / BP (DX the remainder) and BX = |BX - word off+2|.
fn map_distance(m: &mut Machine, start: u16, off: u16) -> Result<(), Bail> {
    let mut c = Ctx::new(m);
    let back = start.wrapping_add(4);
    c.run(start, 2)?;
    c.save(&[SI]);
    c.call(0x407e, back)?;
    let (x, y) = (c.reg(DX), c.reg(BX));
    let record = c.word_in(SS, c.reg(SP));
    let abs = |v: u16| if v & 0x8000 != 0 { v.wrapping_neg() } else { v };
    let row = y.wrapping_shl(1);
    let scale = c.word_in(SS, abs(row).wrapping_add(0x4880));
    if scale == 0 {
        // The original's `div` faults: the interpreter takes it from here.
        c.m.cpu.ip = back;
        return Err(Bail);
    }
    let across = c.word(record.wrapping_add(off)).wrapping_sub(x);
    let down = y.wrapping_sub(c.word(record.wrapping_add(off + 2)));
    let (q, r) = (abs(across) / scale, abs(across) % scale);
    let dy = abs(down);
    let negs = (row & 0x8000 != 0) as u32 + (across & 0x8000 != 0) as u32 + (down & 0x8000 != 0) as u32;
    c.run(back, 15 + negs + (q < dy) as u32)?;
    c.restore(&[SI]);
    c.set_reg(BP, scale);
    c.set_reg(DX, r);
    c.set_reg(BX, dy);
    c.set_reg(AX, q.max(dy));
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

/// `loc_06827`: BP = the sprite of the troop SI facing its location: the
/// direction (0 to 3) of the larger of the two deltas to it (the x delta
/// by its high byte), by its activity (`troop_activity`), from the table
/// at SS:[`FACING_SPRITES`]. BX, DX, DI kept.
pub fn troop_facing_sprite(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6827)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let place = troop.location(&mut c);
    let abs = |v: u16| if v & 0x8000 != 0 { v.wrapping_neg() } else { v };
    let (lx, ly) = (c.word(place.wrapping_add(2)), c.word(place.wrapping_add(4)));
    let (tx, ty) = (c.word(troop.offset().wrapping_add(6)), c.word(troop.offset().wrapping_add(8)));
    // `cbw` of the x delta's high byte.
    let across = (lx.wrapping_sub(tx) >> 8) as u8 as i8 as u16;
    let down = ly.wrapping_sub(ty);
    let vertical = abs(across) < abs(down);
    let (main, other) = if vertical { (down, across) } else { (across, down) };
    let backwards = main & 0x8000 != 0;
    let direction: u16 = (if vertical { 2 } else { 1 }) ^ (if backwards { 2 } else { 0 });
    let negs = (across & 0x8000 != 0) as u32 + (down & 0x8000 != 0) as u32 + backwards as u32;
    c.run(start, 21 + negs + if vertical { 0 } else { 2 })?;
    c.save(&[BX, DX, DI]);
    c.set_reg(AX, across);
    c.set_reg(DI, abs(across));
    c.set_reg(CX, abs(down));
    c.set_reg(BX, main);
    c.set_reg(DX, other);
    c.set_reg(BP, direction);
    c.call(0x693b, 0x685e)?;
    c.run(0x685e, 9)?;
    let activity = c.reg(AX);
    c.set_reg(AX, activity << 2);
    let index = c.reg(BP).wrapping_add(activity << 2) << 1;
    let sprite = c.word_in(SS, index.wrapping_add(FACING_SPRITES));
    c.set_reg(BP, sprite);
    c.restore(&[BX, DX, DI]);
    c.ret()
}

/// The sprites by activity and direction, words in the stack segment
/// (`troop_facing_sprite`).
const FACING_SPRITES: u16 = 0x18bf;

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

/// `loc_0858c`: the troop SI taken out of its location's chain (unless at
/// its own location): the location's first troop or the troop before it
/// then points past it; its next cleared. BP = SI; DI kept.
pub fn unlink_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x858c)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let id = troop.id(&mut c);
    let place = troop.location(&mut c);
    let state = troop.state(&mut c);
    c.run(start, 6)?;
    c.set_reg(BP, troop.offset());
    c.set_reg8(r8::AL, id);
    c.set_reg(DI, place);
    c.save(&[DI]);
    // The end: SI back from BP, its next cleared.
    let finish = |c: &mut Ctx| -> Result<(), Bail> {
        c.run(0x85ba, 4)?;
        let troop = Troop(c.reg(BP));
        c.set_reg(SI, troop.offset());
        c.set_byte(troop.offset().wrapping_add(1), 0);
        c.restore(&[DI]);
        c.ret()
    };
    if state & troop_state::OWN_LOCATION != 0 {
        return finish(&mut c);
    }
    let first = Location(place).first_troop(&mut c);
    if first == id {
        c.run(0x859a, 7)?;
        let next = troop.next(&mut c);
        c.set_byte(troop.offset().wrapping_add(1), 0);
        c.set_reg8(r8::AH, next);
        c.restore(&[DI]);
        let place = Location(c.reg(DI));
        c.set_byte(place.offset().wrapping_add(9), next);
        return c.ret();
    }
    c.run(0x859a, 5)?;
    c.set_reg8(r8::CL, id);
    c.set_reg8(r8::AL, first);
    // Along the chain (`troop_ptr` of AL) to the troop before it.
    loop {
        c.call(0x6906, 0x85a7)?;
        let before = Troop(c.reg(SI));
        let next = before.next(&mut c);
        if next == 0 {
            c.run(0x85a7, 4)?;
            c.set_reg8(r8::AL, 0);
            c.set_reg(DI, before.offset());
            return finish(&mut c);
        }
        c.run(0x85a7, if next == id { 8 } else { 7 })?;
        c.set_reg8(r8::AL, next);
        c.set_reg(DI, before.offset());
        if next == id {
            // `mov ah,[bp+1]`: BP addresses the stack segment.
            let past = c.byte_in(SS, c.reg(BP).wrapping_add(1));
            c.set_reg8(r8::AH, past);
            c.set_byte(before.offset().wrapping_add(1), past);
            return finish(&mut c);
        }
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

/// `loc_06e20`: with [`DAY_UPDATE`] set, for the troop SI at the location
/// DI: a taken location (bit 8 of byte 0Ah) whose day (byte 0Bh) is past
/// (today minus it below FEh) is freed: the bit cleared, byte 8 kept to
/// its low three bits, [`FREED`] counted, `loc_06dbb`, byte 0Bh 5. Then
/// `grow_location`; after 8 days with its job (byte 14h) a motivation
/// loss (`loc_06f93`, AL 1); and for the location's first troop, when
/// `loc_06e82` counts 3 of its troops, `loc_06ea8` for each and the
/// message pair 302h (`loc_029f0`; SI, DI kept).
pub fn troop_day_update(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6e20)?;
    let mut c = Ctx::new(m);
    if c.byte(DAY_UPDATE) == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let taken = Location(c.reg(DI)).byte_0a(&mut c) & 8 != 0;
    c.run(start, 5)?;
    if taken {
        c.call(0x1ac5, 0x6e31)?;
        let place = Location(c.reg(DI));
        let since = c.reg8(r8::AL).wrapping_sub(place.byte_0b(&mut c));
        c.set_reg8(r8::AL, since);
        if since >= 0xfe {
            c.run(0x6e31, 4)?;
            return c.ret();
        }
        c.run(0x6e31, 7)?;
        let b = place.byte_0a(&mut c);
        place.set_byte_0a(&mut c, b & !8);
        let b = place.byte_08(&mut c);
        place.set_byte_08(&mut c, b & 7);
        bump(&mut c, FREED);
        c.call(0x6dbb, 0x6e47)?;
        c.run(0x6e47, 2)?;
        Location(c.reg(DI)).set_byte_0b(&mut c, 5);
    }
    c.call(0x6cfc, 0x6e4e)?;
    c.run(0x6e4e, 1)?;
    c.call(0x1ac5, 0x6e51)?;
    let days = c.reg8(r8::AL).wrapping_sub(Troop(c.reg(SI)).day(&mut c));
    c.set_reg8(r8::AL, days);
    if days > 8 {
        c.run(0x6e51, 5)?;
        c.set_reg8(r8::AL, 1);
        c.call(0x6f93, 0x6e5d)?;
    } else {
        c.run(0x6e51, 3)?;
    }
    let id = Troop(c.reg(SI)).id(&mut c);
    let first = Location(c.reg(DI)).first_troop(&mut c) == id;
    c.run(0x6e5d, if first { 6 } else { 4 })?;
    c.set_reg8(r8::AL, id);
    if !first {
        return c.ret();
    }
    c.set_reg(DX, 0);
    c.set_reg(BP, 0x6e82);
    c.call(0x661d, 0x6e6c)?;
    if c.reg8(r8::DL) != 3 {
        c.run(0x6e6c, 3)?;
        return c.ret();
    }
    c.run(0x6e6c, 4)?;
    c.set_reg(BP, 0x6ea8);
    c.call(0x661d, 0x6e77)?;
    c.run(0x6e77, 4)?;
    c.set_reg(AX, 0x302);
    c.save(&[SI, DI]);
    c.call(0x29f0, 0x6e7f)?;
    c.run(0x6e7f, 3)?;
    c.restore(&[SI, DI]);
    c.ret()
}

/// Set while the day's updates run ([46DEh], `troop_day_update`).
const DAY_UPDATE: u16 = 0x46de;
/// The count of locations freed (`troop_day_update`, [27h]).
const FREED: u16 = 0x27;

/// `loc_0329d`: for a troop SI with byte 3 set, bits Ch of word 10h
/// cleared (AX 0); else [`RATE_BY_AGE`] = its word 0Eh over the age
/// [`AGE`] (rounded to nearest; the age itself when 0), and on into
/// [`troop_rate`].
pub fn troop_rate_by_age(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x329d)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    if troop.state(&mut c) != 0 {
        c.run(start, 5)?;
        c.set_reg(AX, 0);
        let w = troop.word_10(&mut c);
        troop.set_word_10(&mut c, w & !0x0c);
        return c.ret();
    }
    let age = c.word(AGE);
    c.run(start, if age != 0 { 14 } else { 7 })?;
    let mut rate = age;
    if age != 0 {
        let w = troop.word_0e(&mut c);
        let (q, r) = (w / age, w % age);
        let twice = r << 1;
        rate = q + (age < twice) as u16;
        c.set_reg(CX, age);
        c.set_reg(DX, twice);
    }
    c.set_reg(AX, rate);
    c.set_word(RATE_BY_AGE, rate);
    troop_rate(c.m, 0x708a)
}

/// Read by `troop_rate_by_age` ([42h]).
const AGE: u16 = 0x42;
/// Written by `troop_rate_by_age` ([4Ah]).
const RATE_BY_AGE: u16 = 0x4a;

/// `loc_0714c`: with bit 1 of [`EVENT_FLAGS`], for a troop SI with bit 80h
/// of byte 19h on its tick (the game time's low nibble = its number's):
/// `maybe_mark_troop`, then a random AH (`rand`) against the chance at
/// [`CHANCES`] by the location DI's byte 0. When it passes: bit 4000h of
/// word 10h, and unless bit 40h of byte 19h, by AH & 3: 1 its size 2
/// lower when above 2 (bit 2000h); 2 bit 200h and `mark_troop_10`; 3 bit
/// 1000h, bit 80h of byte 19h off, the location's need 0 one lower and
/// the message pair 0F06h (`call_29f0_f`).
pub fn troop_event_roll(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x714c)?;
    let mut c = Ctx::new(m);
    if c.byte(EVENT_FLAGS) & 1 == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let troop = Troop(c.reg(SI));
    let bits = troop.byte_19(&mut c);
    if bits & 0x80 == 0 {
        c.run(start, 5)?;
        return c.ret();
    }
    let tick = game::game_time(&mut c) as u8 & 0x0f;
    let id = troop.id(&mut c) & 0x0f;
    c.run(start, 10)?;
    c.set_reg(AX, (id as u16) << 8 | tick as u16);
    if tick != id {
        return c.ret();
    }
    c.call(0x71bc, 0x7168)?;
    c.run(0x7168, 1)?;
    c.call(0xe3cc, 0x716b)?;
    let (troop, place) = (Troop(c.reg(SI)), Location(c.reg(DI)));
    let roll = c.reg8(r8::AH);
    let k = place.byte_00(&mut c);
    let chance = c.byte(CHANCES.wrapping_add(k as u16));
    if chance < roll {
        c.run(0x716b, 6)?;
        c.set_reg8(r8::AL, chance);
        c.set_reg(BX, CHANCES);
        return c.ret();
    }
    let bits = troop.byte_19(&mut c);
    let size = troop.size(&mut c);
    let kind = roll & 3;
    let immune = bits & 0x40 != 0;
    let cost = 8 + match (immune, kind) {
        (true, _) => 1,
        (false, 0) => 3,
        (false, 1) => 5 + if size > 2 { 4 } else { 5 },
        (false, 2) => 5 + 2,
        _ => 4 + 4,
    };
    c.run(0x716b, cost)?;
    c.set_reg8(r8::AL, chance);
    c.set_reg(BX, CHANCES);
    let w = troop.word_10(&mut c) | 0x4000;
    troop.set_word_10(&mut c, w);
    if immune {
        return c.ret();
    }
    c.set_reg8(r8::AH, kind);
    match kind {
        0 => c.ret(),
        1 => {
            troop.set_word_10(&mut c, w | 0x2000);
            if size > 2 {
                troop.set_size(&mut c, size - 2);
            }
            c.ret()
        }
        2 => {
            troop.set_word_10(&mut c, w | 0x200);
            mark_troop_10(c.m, 0x7085)
        }
        _ => {
            troop.set_word_10(&mut c, w | 0x1000);
            troop.set_byte_19(&mut c, bits & !0x80);
            let need = place.need(&mut c, 0).wrapping_sub(1);
            place.set_need(&mut c, 0, need);
            c.set_reg8(r8::AL, 6);
            call_29f0_f(c.m, 0x71b2)
        }
    }
}

/// Bit 1: the troops' random events on (`troop_event_roll`, [0Ah]).
const EVENT_FLAGS: u16 = 0x0a;
/// The chance of an event, by the location's byte 0 (`troop_event_roll`).
const CHANCES: u16 = 0x1141;

/// `loc_0342d`: AX = the troop SI's yield. Its effective motivation
/// (`troop_motivation`) doubled plus skill 1 (at most FFh), times its
/// size, /16; plus that ×2, ×4, ×8, ×16 for bits 5, 4, 3, 2 of byte 19h
/// (FFFFh on overflow); /256. A troop of size 1 or more yields at least
/// 1. DX = the last multiple looked at, BL byte 19h shifted past it.
pub fn troop_yield(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x342d)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x6efd, 0x3430)?;
    let troop = Troop(c.reg(SI));
    let motivation = c.reg8(r8::AL);
    let size = troop.size(&mut c);
    let bits = troop.byte_19(&mut c);
    let (sum, over) = motivation.wrapping_add(motivation).overflowing_add(troop.skill(&mut c, 1));
    let base = (if over { 0xff } else { sum } as u16 * size as u16) >> 4;
    let mut cost = 16 + over as u32;
    let mut total = base;
    let mut step = base << 1;
    let mut looked = 0;
    let mut capped = false;
    for k in 0..4 {
        if k > 0 {
            step <<= 1;
            cost += 3;
        }
        looked += 1;
        if bits & (0x20 >> k) != 0 {
            cost += 2;
            match total.checked_add(step) {
                Some(v) => total = v,
                None => {
                    capped = true;
                    break;
                }
            }
        }
    }
    if capped {
        total = 0xffff;
        cost += 1;
    }
    let result = total >> 8;
    cost += 4 + if result != 0 { 1 } else { 4 };
    c.run(0x3430, cost)?;
    c.set_reg(DX, step);
    c.set_reg8(r8::BL, bits << (2 + looked));
    c.set_reg(AX, if result != 0 { result } else { (size >= 1) as u16 });
    c.ret()
}

/// `loc_06ac5`: the troop SI's byte 3 with its low two bits cleared as its
/// new job ([`set_job_6ad4`], CL), unless that is its job already (no bits
/// above the job and the low two clear). AL its job.
pub fn set_troop_activity(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6ac5)?;
    let mut c = Ctx::new(m);
    let state = Troop(c.reg(SI)).state(&mut c);
    let (new, job) = (state & 0xfc, state & 0x0f);
    let same = job == new;
    c.run(start, if same { 7 } else { 6 })?;
    c.set_reg8(r8::CL, new);
    c.set_reg8(r8::AL, job);
    if same {
        return c.ret();
    }
    set_job_at_6ad4(&mut c)
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
/// or after the last) and given the first free slot (byte 2): from 1, or
/// from 9 for a troop with bit 80h of word 10h. The slots taken are marked
/// in a table of 1Eh bytes on the stack, then searched (`repne scasb`).
/// CX the slot.
pub fn link_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x851f)?;
    let mut c = Ctx::new(m);
    let (troop, place) = (Troop(c.reg(SI)), Location(c.reg(DI)));
    let id = troop.id(&mut c);
    let first = place.first_troop(&mut c);
    let apart = troop.word_10(&mut c) & 0x80 != 0;
    let lowest: u8 = if apart { 9 } else { 1 };
    if first == 0 {
        c.run(start, 10 + if apart { 2 } else { 0 })?;
        c.set_reg(AX, (id as u16) << 8);
        c.set_byte(place.offset().wrapping_add(9), id);
        c.set_byte(troop.offset().wrapping_add(2), lowest);
        c.set_reg(CX, lowest as u16);
        return c.ret();
    }
    const TABLE: u16 = 0x1e;
    c.run(start, 13)?;
    let ax = (id as u16) << 8 | first as u16;
    c.save(&[DI]);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    let table = c.reg(SP).wrapping_sub(TABLE);
    c.set_reg(SP, table);
    c.set_reg(AX, ax);
    c.save(&[AX]);
    c.set_reg(AX, ax & 0xff00);
    c.set_reg(DI, table);
    c.set_reg(CX, TABLE);
    c.rep(0xaa); // rep stosb: the table cleared
    c.run(0x8550, 5)?;
    c.restore(&[AX]);
    c.set_reg(DI, table);
    c.save(&[SI]);
    c.set_reg(BX, 0);
    // Along the chain (`troop_ptr` of AL), each troop's slot marked FFh.
    let last = loop {
        c.call(0x6906, 0x8559)?;
        let other = Troop(c.reg(SI));
        let slot = c.byte(other.offset().wrapping_add(2));
        let next = other.next(&mut c);
        c.run(0x8559, if next == 0 { 10 } else { 6 })?;
        c.set_reg(BX, slot as u16);
        c.set_byte((slot as u16).wrapping_add(table).wrapping_sub(1), 0xff);
        c.set_reg8(r8::AL, next);
        if next == 0 {
            break other;
        }
    };
    c.set_byte(last.offset().wrapping_add(1), id);
    c.restore(&[SI]);
    c.set_reg(CX, TABLE);
    // Tested again: the marks can land anywhere (a slot past 1Eh writes
    // beyond the table), the troop's own record included.
    let apart = Troop(c.reg(SI)).word_10(&mut c) & 0x80 != 0;
    if apart {
        c.run(0x8575, 2)?;
        c.set_reg(CX, TABLE - 8);
        c.set_reg(DI, table + 8);
    }
    c.run(0x857b, 2)?;
    c.set_reg8(r8::AL, 0);
    c.m.cpu.string_op(&mut c.m.hw, 0xae, None, 0xf2); // repne scasb: the first free
    c.run(0x857f, 6)?;
    let slot = TABLE - c.reg(CX);
    c.set_reg(CX, slot);
    c.set_reg(SP, table.wrapping_add(TABLE));
    c.restore(&[DI]);
    let troop = Troop(c.reg(SI));
    c.set_byte(troop.offset().wrapping_add(2), slot as u8);
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

/// `loc_05ce4`: a click on the map's troops: the sprite list dropped,
/// the map resource opened; with no second place picked ([0DBE2h]) and
/// the first one none or the troop panel 18DFh: inside that panel
/// (`point_in_rect`, CF) the panel redrawn ([`redraw_panel_18df`]); else
/// the troop under the cursor (`particle_at`, CF): the one shown already
/// ([46FAh]) redrawn the same way, another one drawn first
/// (`redraw_7b36`); then its caption and panel ([`troop_caption`]).
pub fn map_troop_click(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5ce4)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x5beb, 0x5ce7)?;
    c.run(0x5ce7, 1)?;
    c.call(0xc13b, 0x5cea)?;
    let p2 = c.word(0xdbe2);
    if p2 != 0 {
        c.run(0x5cea, 3)?;
        c.cmp(W, p2, 0);
        return c.ret();
    }
    let di = c.word(0xdbe0);
    if di != 0 && di != 0x18df {
        c.run(0x5cea, 8)?;
        c.cmp(W, 0, 0);
        c.set_reg(DI, di);
        c.or(W, di, di);
        c.cmp(W, di, 0x18df);
        return c.ret();
    }
    if di != 0 {
        c.run(0x5cea, 8)?;
        c.cmp(W, 0, 0);
        c.set_reg(DI, di);
        c.or(W, di, di);
        c.cmp(W, di, 0x18df);
        c.call(0xd6fe, 0x5d02)?;
        if c.flag(CF) {
            c.run(0x5d02, 2)?;
            return redraw_panel_18df(c.m, 0x79de);
        }
        c.run(0x5d02, 2)?;
    } else {
        c.run(0x5cea, 6)?;
        c.cmp(W, 0, 0);
        c.set_reg(DI, 0);
        c.or(W, 0, 0);
    }
    c.call(0x6946, 0x5d07)?;
    if !c.flag(CF) {
        c.run(0x5d07, 2)?;
        return c.ret();
    }
    let si = c.reg(SI);
    let t = c.word(si.wrapping_add(0x0a));
    let cur = c.word(0x46fa);
    if t == cur {
        c.run(0x5d07, 5)?;
        c.set_reg(SI, t);
        c.cmp(W, t, cur);
        return redraw_panel_18df(c.m, 0x79de);
    }
    c.run(0x5d07, 6)?;
    c.set_reg(SI, t);
    c.cmp(W, t, cur);
    c.push(t);
    c.call(0x7b36, 0x5d16)?;
    c.run(0x5d16, 2)?;
    let s = c.pop();
    c.set_reg(SI, s);
    troop_caption(c.m, 0x78bc)
}

/// `loc_078bc`: for the troop SI within reach ([`troop_map_distance`] of
/// its location +4 below [1176h]) and shown ([`troop_particle`], ZF):
/// on the screen, [46FAh] = SI, its caption box 18DFh (64h wide) at the
/// location, [1955h] its id, then its panel ([`troop_panel`]).
pub fn troop_caption(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x78bc)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let si = c.reg(SI);
    let di = c.word(si.wrapping_add(4));
    c.set_reg(DI, di);
    c.call(0x7c63, 0x78c2)?;
    let ax = c.reg(AX);
    let lim = c.word(0x1176);
    if ax >= lim {
        c.run(0x78c2, 3)?;
        c.cmp(W, ax, lim);
        return c.ret();
    }
    c.run(0x78c2, 3)?;
    c.cmp(W, ax, lim);
    c.call(0x6917, 0x78cb)?;
    if !c.flag(ZF) {
        c.run(0x78cb, 2)?;
        return c.ret();
    }
    c.run(0x78cb, 2)?;
    c.call(0xc08e, 0x78d0)?;
    c.run(0x78d0, 7)?;
    let si = c.reg(SI);
    c.set_word(0x46fa, si);
    c.push(si);
    c.set_reg(SI, 0x18df);
    let di = c.reg(DI);
    let (y, x) = (c.word(di.wrapping_add(2)), c.word(di));
    c.set_reg(BX, y);
    c.set_reg(DX, x);
    c.set_reg(CX, 0x64);
    c.call(0x5f25, 0x78e3)?;
    c.run(0x78e3, 3)?;
    let s = c.pop();
    c.set_reg(SI, s);
    let id = c.byte(s);
    c.set_reg8(r8::AL, id);
    c.set_byte(0x1955, id);
    troop_panel_at(&mut c)
}

/// `loc_078e9`: the troop panel 18DFh for the troop SI shown
/// ([`troop_particle`], ZF; else [`redraw_panel_18df`]): on the screen,
/// its description (`describe_troop`), the box filled (its bottom
/// [11F3h] 0Ch lower), then at its corner: the title 3Ah (3Bh for a
/// troop moving, bit 6 of [30h]), the place's name, its job (41h / 42h
/// for the specialised ones 20h..22h, else 3Ch and the activity
/// [2Fh] phrase from 11F7h unless in place 2, and a status 3Fh / 40h /
/// 3Dh / 43h / 3Eh by [32h], [34h], [2Fh], [30h]), the skills heading
/// 6Eh, the troop's bits (`spread_location_bits`) and skill bars; back
/// to the front buffer.
pub fn troop_panel(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x78e9)?;
    let mut c = Ctx::new(m);
    troop_panel_at(&mut c)
}

fn troop_panel_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x78e9, 1)?;
    c.call(0x6917, 0x78ec)?;
    if !c.flag(ZF) {
        c.run(0x78ec, 2)?;
        return redraw_panel_18df(c.m, 0x79de);
    }
    c.run(0x78ec, 2)?;
    c.call(0xc08e, 0x78f4)?;
    c.run(0x78f4, 1)?;
    c.call(0x31f6, 0x78f7)?;
    c.run(0x78f7, 3)?;
    let v = c.word(0x11f3);
    let v = c.add(W, v, 0x0c);
    c.set_word(0x11f3, v);
    c.set_reg(SI, 0x18df);
    c.call(0x7b1b, 0x7902)?;
    c.run(0x7902, 1)?;
    c.call(0xd075, 0x7905)?;
    let f30 = c.byte(0x30);
    c.run(0x7905, 9 + if f30 & 0x40 != 0 { 1 } else { 0 } + 1)?;
    c.set_reg8(r8::CL, 0x9a);
    let ch = c.byte(0x18e8);
    c.set_reg8(r8::CH, ch);
    let (x, y) = (c.word(0x18df), c.word(0x18e1));
    let dx = c.add(W, x, 0x0c);
    c.set_reg(DX, dx);
    let bx = c.add(W, y, 4);
    c.set_reg(BX, bx);
    c.set_reg(AX, 0x3a);
    c.test(B, f30 as u16, 0x40);
    if f30 & 0x40 != 0 {
        let a = c.inc(W, 0x3a);
        c.set_reg(AX, a);
    }
    c.call(0x8865, 0x7927)?;
    c.run(0x7927, 5)?;
    c.set_reg8(r8::CL, 0x96);
    let dx = c.reg(DX);
    let dx = c.sub(W, dx, 8);
    c.set_reg(DX, dx);
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 9);
    c.set_reg(BX, bx);
    let di = c.word(0x2c);
    c.set_reg(DI, di);
    c.call(0x62a6, 0x7936)?;
    let al = c.byte(0x30);
    let spec = al & 0x20 != 0;
    // `status`: the status phrase by the flags (None: none).
    if spec {
        let n = if al != 0x22 { 8 } else { 10 };
        c.run(0x7936, n)?;
        c.set_reg8(r8::CL, 0x9a);
        let bx = c.reg(BX);
        let bx = c.add(W, bx, 0x0a);
        c.set_reg(BX, bx);
        c.set_reg8(r8::AL, al);
        c.test(B, al as u16, 0x20);
        c.cmp(B, al as u16, 0x22);
        let mut ax = 0x41u16;
        if al == 0x22 {
            ax = c.inc(W, ax);
        }
        c.set_reg(AX, ax);
        return status_79b6(c);
    }
    c.run(0x7936, 7)?;
    c.set_reg8(r8::CL, 0x9a);
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 0x0a);
    c.set_reg(BX, bx);
    c.set_reg8(r8::AL, al);
    c.test(B, al as u16, 0x20);
    c.set_reg(AX, 0x3c);
    c.call(0x8865, 0x7952)?;
    let p = c.byte(0x30);
    if p == 2 {
        c.run(0x7952, 3)?;
        let bx = c.reg(BX);
        let bx = c.add(W, bx, 0x0f);
        c.set_reg(BX, bx);
        c.cmp(B, 2, 2);
        return skills_79bc(c);
    }
    c.run(0x7952, 10)?;
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 0x0f);
    c.set_reg(BX, bx);
    c.cmp(B, p as u16, 2);
    let a2f = c.byte(0x2f);
    let ax = c.reg(AX) & 0xff00 | a2f as u16;
    c.set_reg(AX, ax);
    let a = c.shr1(B, a2f as u16, 1);
    let a = c.and(W, ax & 0xff00 | a, 6);
    let a = c.add(W, a, 0x11f7);
    c.set_reg(SI, a);
    let ph = c.word(a);
    c.set_reg(AX, ph);
    c.call(0xd194, 0x796e)?;
    let f30 = c.byte(0x30);
    let w32 = c.word(0x32);
    let w34 = c.word(0x34);
    let a2f = c.byte(0x2f);
    // The status chain from 7971h: (count, phrase or None).
    let (n, ph) = if f30 & 0x40 != 0 {
        (3, None)
    } else if w32 & 0x200 != 0 {
        (6, Some(0x3f))
    } else if w32 & 0x100 == 0 {
        (9, Some(0x40))
    } else if w34 & 0x30 != 0 {
        (11, Some(0x40))
    } else if f30 == 0 {
        (14, Some(0x3d))
    } else if a2f == 1 {
        (17, Some(0x43))
    } else if f30 == 6 {
        (20, Some(0x3e))
    } else {
        (21, None)
    };
    c.run(0x796e, n)?;
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 0x0a);
    c.set_reg(BX, bx);
    c.test(B, f30 as u16, 0x40);
    if n >= 4 {
        c.set_reg(AX, 0x3f);
        c.test(W, w32, 0x200);
    }
    if n >= 7 {
        c.set_reg(AX, 0x40);
        c.test(W, w32, 0x100);
    }
    if n >= 10 {
        c.test(W, w34, 0x30);
    }
    if n >= 12 {
        c.set_reg(AX, 0x3d);
        c.cmp(B, f30 as u16, 0);
    }
    if n >= 15 {
        c.set_reg(AX, 0x43);
        c.cmp(B, a2f as u16, 1);
    }
    if n >= 18 {
        c.set_reg(AX, 0x3e);
        c.cmp(B, f30 as u16, 6);
    }
    match ph {
        Some(_) => status_79b6(c),
        None => skills_79bc(c),
    }
}

/// From 79B6h: the status phrase AX, 11h lower, then the skills.
fn status_79b6(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x79b6, 1)?;
    c.call(0x8865, 0x79b9)?;
    c.run(0x79b9, 1)?;
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 0x11);
    c.set_reg(BX, bx);
    skills_79bc(c)
}

/// From 79BCh: the skills heading 6Eh, the troop [46FAh]'s bits and its
/// skill bars (4705h, [18E5h]); back to the front buffer.
fn skills_79bc(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x79bc, 4)?;
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 4);
    c.set_reg(BX, bx);
    c.set_reg(AX, 0x6e);
    c.set_reg8(r8::CL, 0x96);
    c.call(0xd194, 0x79c7)?;
    c.run(0x79c7, 3)?;
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 8);
    c.set_reg(BX, bx);
    let si = c.word(0x46fa);
    c.set_reg(SI, si);
    c.call(0x7efb, 0x79d1)?;
    c.run(0x79d1, 3)?;
    c.set_reg(SI, 0x4705);
    let bp = c.word(0x18e5);
    c.set_reg(BP, bp);
    c.call(0x7e3d, 0x79db)?;
    c.run(0x79db, 1)?;
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `loc_08308`: the troop SI moves: 4 steps (8 when fast, bit 40h of
/// +19h) of `loc_08604` added to its position (+6, +8); a still step
/// means it arrived (`loc_08357`). Then its sprite: on the map
/// (`loc_0686e`, no CF) shown (`loc_06917`; updated at `loc_0c653`, or
/// placed: `loc_06827`, `loc_0c5cf`, `particles_update_dirty_rect`);
/// off it removed when shown (`particles_remove_particle`).
pub fn troop_move(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8308)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let f = c.byte(si.wrapping_add(0x19));
    let fast = f & 0x40 != 0;
    c.run(0x8308, 3 + fast as u32)?;
    c.set_reg(CX, 4);
    c.test(B, f as u16, 0x40);
    if fast {
        c.set_reg8(r8::CL, 8);
    }
    steps_8313(&mut c)
}

/// `loc_08313`: the troop SI's CX steps (see [`troop_move`]).
fn steps_8313(c: &mut Ctx) -> Result<(), Bail> {
    loop {
        c.run(0x8313, 2)?;
        let cx = c.reg(CX);
        c.push(cx);
        c.call(0x8604, 0x8317)?;
        let (bx, dx) = (c.reg(BX), c.reg(DX));
        let sp = c.reg(SP);
        let cx = c.word_in(SS, sp);
        if bx | dx == 0 {
            c.run(0x8317, 4)?;
            c.pop();
            c.set_reg(CX, cx);
            c.set_reg(AX, bx);
            c.or(W, bx, dx);
            c.set_reg(AX, 0);
            return troop_arrived_at(c);
        }
        c.run(0x8317, 7)?;
        c.pop();
        c.set_reg(CX, cx);
        c.set_reg(AX, bx | dx);
        c.or(W, bx, dx);
        let si = c.reg(SI);
        let v = c.word(si.wrapping_add(6));
        let v = c.add(W, v, dx);
        c.set_word(si.wrapping_add(6), v);
        let v = c.word(si.wrapping_add(8));
        let v = c.add(W, v, bx);
        c.set_word(si.wrapping_add(8), v);
        let cx = cx.wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.run(0x8326, 1)?;
    c.call(0x686e, 0x8329)?;
    if c.flag(CF) {
        c.run(0x8329, 2)?;
        c.call(0x6917, 0x8336)?;
        if c.flag(ZF) {
            c.run(0x8336, 2)?;
            return super::saves::particles_remove_particle(c.m, 0xc58a);
        }
        c.run(0x8336, 2)?;
        return c.ret();
    }
    c.run(0x8329, 2)?;
    c.call(0x6917, 0x832e)?;
    if c.flag(ZF) {
        c.run(0x832e, 2)?;
        return c.jump(0xc653);
    }
    c.run(0x832e, 2)?;
    c.call(0x6827, 0x833f)?;
    c.run(0x833f, 1)?;
    c.call(0xc5cf, 0x8342)?;
    c.run(0x8342, 2)?;
    let di = c.reg(DI);
    c.set_reg(SI, di);
    c.jump(0xc6ad)
}

/// `loc_08357`: the troop SI arrived at its place (+4): the followed
/// troop's ([11D3h], troop 8E0h) note shifted (`loc_08347`); its
/// position the place's; then by its order (+3 low nibble): where the
/// place is not secret (bit 2 of +0Ah) or found (`loc_05d36`, CF) the
/// troop updated (`loc_083bc`); spice harvesting (5) shows a newly
/// found place (bit 7, `note_shown_location`), others meeting the
/// place's own troop (+9) go on at `loc_07429` (not ported: the
/// interpreter); otherwise the troops told (`loc_0661d`, BP 8403h).
/// Hidden: orders 5 / 6 drop their figure (`loc_06ac5`); a move (3)
/// goes on to the next place (`loc_0841f`), others update the troop.
pub fn troop_arrived(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8357)?;
    let mut c = Ctx::new(m);
    troop_arrived_at(&mut c)
}

fn troop_arrived_at(c: &mut Ctx) -> Result<(), Bail> {
    let si = c.reg(SI);
    let p = c.word(si.wrapping_add(4));
    if si != 0x08e0 {
        c.run(0x8357, 2)?;
        c.cmp(W, si, 0x08e0);
    } else {
        let f = c.word(0x11d3);
        c.run(0x8357, 5 + (f == p) as u32)?;
        c.cmp(W, si, 0x08e0);
        c.set_reg(AX, f);
        c.cmp(W, f, p);
        if f == p {
            c.call(0x8347, 0x8368)?;
        }
    }
    let si = c.reg(SI);
    let o = c.byte(si.wrapping_add(3));
    let di = c.word(si.wrapping_add(4));
    let dx = c.word(di.wrapping_add(2));
    let bx = c.word(di.wrapping_add(4));
    let fl = c.byte(di.wrapping_add(0x0a));
    let shown = fl & 2 != 0;
    c.run(0x8368, 9 + (!shown) as u32)?;
    c.set_reg8(r8::AL, o);
    let al = c.and(B, o as u16, 0x0f) as u8;
    c.set_reg8(r8::AL, al);
    c.set_reg(DI, di);
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.set_word(si.wrapping_add(6), dx);
    c.set_word(si.wrapping_add(8), bx);
    c.test(B, fl as u16, 2);
    let mut found = true;
    if !shown {
        c.call(0x5d36, 0x8385)?;
        found = !c.flag(CF);
        c.run(0x8385, 1)?;
    }
    if found {
        c.run(0x8387, 2)?;
        let ax = c.reg(AX);
        c.push(ax);
        c.call(0x83bc, 0x838b)?;
        let sp = c.reg(SP);
        let ax = c.word_in(SS, sp);
        let al = ax as u8;
        let (si, di) = (c.reg(SI), c.reg(DI));
        if al == 5 {
            let f = c.byte(di.wrapping_add(0x0a));
            c.run(0x838b, 3 + 2 + if f & 0x80 != 0 { 2 } else { 0 })?;
            c.pop();
            c.set_reg(AX, ax);
            c.cmp(B, 5, 5);
            c.test(B, f as u16, 0x80);
            if f & 0x80 != 0 {
                let v = c.and(B, f as u16, 0x7f);
                c.set_byte(di.wrapping_add(0x0a), v as u8);
                return super::map::note_shown_location(c.m, 0x5d44);
            }
            return tell_troops_83fd(c);
        }
        let own = c.byte(si);
        let place = c.byte(di.wrapping_add(9));
        c.run(0x838b, 3 + 3 + if own == place { 1 } else { 0 })?;
        c.pop();
        c.set_reg(AX, ax);
        c.cmp(B, al as u16, 5);
        c.set_reg8(r8::AL, own);
        c.cmp(B, own as u16, place as u16);
        if own == place {
            return c.jump(0x7429);
        }
        return tell_troops_83fd(c);
    }
    let al = c.reg8(r8::AL);
    let drop = (5..=6).contains(&al);
    let a = if drop { 0 } else { al } & 3;
    c.run(0x83a7, 2 + if al < 5 { 0 } else { 2 } + if drop { 2 } else { 0 })?;
    c.cmp(B, al as u16, 5);
    if al >= 5 {
        c.cmp(B, al as u16, 6);
    }
    if drop {
        let di = c.reg(DI);
        c.push(di);
        c.call(0x6ac5, 0x83b3)?;
        c.run(0x83b3, 2)?;
        let di = c.pop();
        c.set_reg(DI, di);
        c.xor(B, 0, 0);
        c.set_reg8(r8::AL, 0);
    }
    c.run(0x83b6, 3)?;
    let al = c.reg8(r8::AL);
    c.and(B, al as u16, 3);
    c.set_reg8(r8::AL, a);
    c.cmp(B, a as u16, 3);
    if a == 3 {
        return next_place_841f(c);
    }
    update_troop_83bc(c.m, 0x83bc)
}

/// `loc_083fd`: the troops told (`loc_0661d` with BP 8403h).
fn tell_troops_83fd(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x83fd, 2)?;
    c.set_reg(BP, 0x8403);
    for_each_troop_below_80(c.m, 0x661d)
}

/// `loc_0841f`: the troop SI goes on to its next place (+0Ch): the old
/// one left (`loc_07f27`; a carried item, +0Eh, given back to it: its
/// count +14h + item down, the item's flag into +19h); then shown on
/// the map (`loc_06917`): the old sprite removed and, unless hidden
/// (bit 6 of +3, or the place unseen: `loc_05ed0`), the troop's figure
/// (`loc_06757`) placed (`particles_update_dirty_rect`). The same place
/// clears the order's move bits and updates the troop.
fn next_place_841f(c: &mut Ctx) -> Result<(), Bail> {
    let si = c.reg(SI);
    let di = c.word(si.wrapping_add(4));
    let nx = c.word(si.wrapping_add(0x0c));
    if di == nx {
        c.run(0x841f, 5)?;
        c.set_reg(DI, di);
        c.cmp(W, di, nx);
        let v = c.byte(si.wrapping_add(3));
        let v = c.and(B, v as u16, 0xfc);
        c.set_byte(si.wrapping_add(3), v as u8);
        return update_troop_83bc(c.m, 0x83bc);
    }
    c.run(0x841f, 5)?;
    c.set_reg(DI, di);
    c.cmp(W, di, nx);
    c.push(si);
    c.call(0x7f27, 0x842b)?;
    let sp = c.reg(SP);
    let si = c.word_in(SS, sp);
    let cx = c.word(si.wrapping_add(0x0e));
    let bx = cx & 0xff;
    let k = c.byte(bx.wrapping_add(0x46fe));
    c.run(0x842b, 6 + if k != 0 { 3 } else { 0 } + 4)?;
    c.pop();
    c.set_reg(SI, si);
    c.set_reg(BX, cx);
    c.set_reg(CX, cx);
    c.xor(B, cx >> 8, cx >> 8);
    c.set_reg(BX, bx);
    c.cmp(B, k as u16, 0);
    let mut cx = cx;
    if k != 0 {
        let di = c.reg(DI);
        let at = bx.wrapping_add(di).wrapping_add(0x14);
        let v = c.byte(at);
        let v = c.dec(B, v as u16);
        c.set_byte(at, v as u8);
        let f = c.byte(si.wrapping_add(0x19));
        let f = c.or(B, f as u16, cx >> 8);
        c.set_byte(si.wrapping_add(0x19), f as u8);
        c.xor(B, cx >> 8, cx >> 8);
        cx &= 0xff;
        c.set_reg(CX, cx);
    }
    c.set_word(si.wrapping_add(0x0e), cx);
    let nx = c.word(si.wrapping_add(0x0c));
    c.set_reg(DI, nx);
    c.set_word(si.wrapping_add(4), nx);
    c.run(0x8461, 1)?;
    c.call(0x6917, 0x8464)?;
    if !c.flag(ZF) {
        c.run(0x8464, 2)?;
        return c.ret();
    }
    c.run(0x8464, 3)?;
    let si = c.reg(SI);
    c.push(si);
    c.call(0xc58a, 0x846b)?;
    let sp = c.reg(SP);
    let si = c.word_in(SS, sp);
    let o = c.byte(si.wrapping_add(3));
    if o & 0x40 != 0 {
        c.run(0x846b, 4)?;
        c.pop();
        c.set_reg(SI, si);
        c.push(si);
        c.test(B, o as u16, 0x40);
    } else {
        c.run(0x846b, 6)?;
        c.pop();
        c.set_reg(SI, si);
        c.push(si);
        c.test(B, o as u16, 0x40);
        let di = c.word(si.wrapping_add(4));
        c.set_reg(DI, di);
        c.call(0x5ed0, 0x8479)?;
        if !c.flag(ZF) {
            c.run(0x8479, 3)?;
            return figure_gone_848b(c);
        }
        c.run(0x8479, 1)?;
    }
    c.run(0x847b, 2)?;
    let si = c.reg(SI);
    let al = c.byte(si);
    c.set_reg8(r8::AL, al);
    c.call(0x6757, 0x8480)?;
    if c.flag(CF) {
        c.run(0x8480, 3)?;
        return figure_gone_848b(c);
    }
    c.run(0x8480, 4)?;
    let di = c.reg(DI);
    c.set_reg(SI, di);
    c.push(di);
    c.call(0xc6ad, 0x8488)?;
    c.run(0x8488, 3)?;
    let di = c.pop();
    c.set_reg(DI, di);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_0848b` (counted by the caller with its jump): DI 0, SI back.
fn figure_gone_848b(c: &mut Ctx) -> Result<(), Bail> {
    c.xor(W, 0, 0);
    c.set_reg(DI, 0);
    c.run(0x8489, 2)?;
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_084a6`: the troop SI sets out for the place DI: the followed
/// troop stops here unless still followed (`loc_0848f`). Already on its
/// way (bit 6 of +3) it just takes the new target (a move order, 3,
/// cleared) and is shown (`loc_08461`). Else it leaves its place
/// (`loc_06ebf`, `loc_0858c`); a harvesting troop (order 6) whose place
/// is known leaves it to the enemy when it was the last there
/// (`loc_05098`, `loc_074b6`), and a camp below 28h not secret loses
/// motivation (`decrease_troop_motivation` 3). Then on its way (+4 the
/// target, bit 6, +2 0, `loc_07f75`, `loc_08461`) and, unless it waits
/// (bit 4 of +10h), seven steps (`loc_08313`).
pub fn troop_set_out(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x84a6)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    if si == 0x08e0 {
        c.run(0x84a6, 3)?;
        c.cmp(W, si, 0x08e0);
        c.call(0x848f, 0x84af)?;
        if c.flag(ZF) {
            c.run(0x84af, 2)?;
            return c.ret();
        }
        c.run(0x84af, 1)?;
    } else {
        c.run(0x84a6, 2)?;
        c.cmp(W, si, 0x08e0);
    }
    let si = c.reg(SI);
    let o = c.byte(si.wrapping_add(3));
    if o & 0x40 != 0 {
        let a = o & 3;
        c.run(0x84b2, 2 + 5 + (a == 3) as u32 + 1)?;
        c.test(B, o as u16, 0x40);
        let di = c.reg(DI);
        c.set_word(si.wrapping_add(4), di);
        c.set_reg8(r8::AL, 3);
        c.and(B, 3, o as u16);
        c.set_reg8(r8::AL, a);
        c.cmp(B, a as u16, 3);
        if a == 3 {
            let v = c.and(B, o as u16, 0xfc);
            c.set_byte(si.wrapping_add(3), v as u8);
        }
        return drop_troop_particle(c.m, 0x8461);
    }
    c.run(0x84b2, 3)?;
    c.test(B, o as u16, 0x40);
    c.call(0x6ebf, 0x84cd)?;
    c.run(0x84cd, 2)?;
    let di = c.reg(DI);
    c.push(di);
    c.call(0x858c, 0x84d1)?;
    let sp = c.reg(SP);
    let di = c.word_in(SS, sp);
    let si = c.reg(SI);
    let o = c.byte(si.wrapping_add(3));
    c.run(0x84d1, 3)?;
    c.pop();
    c.set_reg(DI, di);
    c.cmp(B, o as u16, 6);
    if o == 6 {
        let p = c.word(si.wrapping_add(4));
        let f = c.byte(p.wrapping_add(0x0a));
        c.run(0x84d8, 4 + (f & 2 != 0) as u32)?;
        c.push(di);
        c.set_reg(DI, p);
        c.test(B, f as u16, 2);
        if f & 2 != 0 {
            c.call(0x5098, 0x84e5)?;
            let cx = c.reg(CX);
            let dx = c.reg(DX);
            let last = cx != 0 && (dx.wrapping_sub(1) as i16) <= 0;
            c.run(0x84e5, if cx == 0 { 1 } else { 3 + if last { 2 } else { 0 } })?;
            if cx != 0 {
                let d = c.dec(W, dx);
                c.set_reg(DX, d);
            }
            if last {
                let si = c.reg(SI);
                c.push(si);
                c.call(0x74b6, 0x84ee)?;
                c.run(0x84ee, 1)?;
                let si = c.pop();
                c.set_reg(SI, si);
            }
        }
        let sp = c.reg(SP);
        let di = c.word_in(SS, sp);
        let k = c.byte(di.wrapping_add(8));
        let f = c.byte(di.wrapping_add(0x0a));
        let n = if k >= 0x28 { 3 } else if f & 2 != 0 { 6 } else { 7 };
        c.run(0x84ef, n)?;
        c.pop();
        c.set_reg(DI, di);
        c.cmp(B, k as u16, 0x28);
        if k < 0x28 {
            c.test(B, f as u16, 2);
            if f & 2 != 0 {
                return c.ret();
            }
            c.set_reg8(r8::AL, 3);
            c.call(0x6f93, 0x8501)?;
        }
    }
    c.run(0x8501, 4)?;
    let si = c.reg(SI);
    let di = c.reg(DI);
    let old = c.word(si.wrapping_add(4));
    c.set_word(si.wrapping_add(4), di);
    c.set_reg(DI, old);
    let v = c.byte(si.wrapping_add(3));
    let v = c.or(B, v as u16, 0x40);
    c.set_byte(si.wrapping_add(3), v as u8);
    c.set_byte(si.wrapping_add(2), 0);
    c.call(0x7f75, 0x850f)?;
    c.run(0x850f, 1)?;
    c.call(0x8461, 0x8512)?;
    let si = c.reg(SI);
    let w = c.byte(si.wrapping_add(0x10));
    if w & 0x10 != 0 {
        c.run(0x8512, 3)?;
        c.test(B, w as u16, 0x10);
        return c.ret();
    }
    c.run(0x8512, 4)?;
    c.test(B, w as u16, 0x10);
    c.set_reg(CX, 7);
    steps_8313(&mut c)
}

/// `loc_03406` (BP of `loc_033be`, for each troop SI): unless started,
/// its yield (`troop_yield`, 342Dh, AX) is added to [`YIELD_APART`] for a
/// troop with bit 80h of byte 10h; else to [`YIELD`], with its words 10h
/// and 12h ORed into [`BITS_10`] and [`BITS_12`] (AX the last of them).
pub fn tally_yield(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3406)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    c.run(start, 3)?;
    if troop.state(&mut c) & troop_state::STARTED != 0 {
        return c.ret();
    }
    c.call(0x342d, 0x340f)?;
    let troop = Troop(c.reg(SI));
    let amount = c.reg(AX);
    let apart = troop.byte_10(&mut c) & 0x80 != 0;
    c.run(0x340f, if apart { 4 } else { 8 })?;
    let total = if apart { YIELD_APART } else { YIELD };
    let v = c.word(total).wrapping_add(amount);
    c.set_word(total, v);
    if apart {
        return c.ret();
    }
    let w10 = troop.word_10(&mut c);
    let w = c.word(BITS_10) | w10;
    c.set_word(BITS_10, w);
    let w12 = troop.word_12(&mut c);
    let w = c.word(BITS_12) | w12;
    c.set_word(BITS_12, w);
    c.set_reg(AX, w12);
    c.ret()
}

/// The yield of the troops with bit 80h of byte 10h (`tally_yield`).
const YIELD_APART: u16 = 0x94;
/// The yield of the other troops (`tally_yield`).
const YIELD: u16 = 0x96;
/// Their words 10h ORed together (`tally_yield`).
const BITS_10: u16 = 0x5c;
/// Their words 12h ORed together (`tally_yield`).
const BITS_12: u16 = 0x5e;

/// `loc_05728` (BP of `loc_06639`, for each troop SI, CF for byte 3 below
/// 80h): by its job (not 2), one more in BL for jobs 0-3, CH for 4-7, CL
/// for 8-15. AL = the job / 4 (the job itself when 2).
pub fn count_by_job(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5728)?;
    let mut c = Ctx::new(m);
    if !c.flag(CF) {
        c.run(start, 2)?;
        return c.ret();
    }
    let job = Troop(c.reg(SI)).job(&mut c);
    let group = job >> 2;
    c.run(start, if job == 2 { 6 } else if group == 0 { 10 } else { 12 })?;
    if job == 2 {
        c.set_reg8(r8::AL, job);
        return c.ret();
    }
    c.set_reg8(r8::AL, group);
    let counter = match group {
        0 => r8::BL,
        1 => r8::CH,
        _ => r8::CL,
    };
    let n = c.reg8(counter).wrapping_add(1);
    c.set_reg8(counter, n);
    c.ret()
}

/// `loc_04e04` (BP of `loc_04ded`, for each troop SI): one more in CX for
/// a troop with byte 3 zero and bit 80h of byte 19h.
pub fn count_4e04(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4e04)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    if troop.state(&mut c) != 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let counted = troop.byte_19(&mut c) & 0x80 != 0;
    c.run(start, if counted { 6 } else { 5 })?;
    if counted {
        let n = c.reg(CX).wrapping_add(1);
        c.set_reg(CX, n);
    }
    c.ret()
}

/// `loc_07289` (BP of `loc_0727d`, for each troop SI): bit 40h of word 12h
/// cleared, CX 100h when it was set.
pub fn clear_bit_7289(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7289)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let w = troop.word_12(&mut c);
    let set = w & 0x40 != 0;
    c.run(start, if set { 5 } else { 3 })?;
    if set {
        troop.set_word_12(&mut c, w & !0x40);
        c.set_reg(CX, 0x100);
    }
    c.ret()
}

/// `loc_07655`, the end of `loc_0762a` and `loc_0764d`: the troop SI reset
/// (`reset_troop`, 66B1h; CX, DX kept), one more in CX.
fn troop_lost_7655(c: &mut Ctx) -> Result<(), Bail> {
    c.save(&[CX, DX]);
    c.call(0x66b1, 0x765a)?;
    c.run(0x765a, 4)?;
    c.restore(&[CX, DX]);
    let n = c.reg(CX).wrapping_add(1);
    c.set_reg(CX, n);
    c.ret()
}

/// `loc_0762a` (BP of `loc_07443`, for each troop SI of a raided
/// location, CF for byte 3 below 80h): a troop with bit 80h of byte 10h
/// is captured while CX is below DX (byte 3 ACh, bit 10h of byte 10h, its
/// size, rate and byte 19h zero), lost from then on ([`troop_lost_7655`]).
/// One more in CX either way.
pub fn raid_troop_762a(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x762a)?;
    let mut c = Ctx::new(m);
    if c.flag(CF) {
        c.run(start, 2)?;
        return c.ret();
    }
    let troop = Troop(c.reg(SI));
    let b10 = troop.byte_10(&mut c);
    if b10 & 0x80 == 0 {
        c.run(start, 4)?;
        return c.ret();
    }
    let (count, limit) = (c.reg(CX), c.reg(DX));
    if count >= limit {
        c.run(start, 8)?;
        return troop_lost_7655(&mut c);
    }
    c.run(start, 12)?;
    troop.set_state(&mut c, 0xac);
    troop.set_byte_10(&mut c, b10 | 0x10);
    troop.set_size(&mut c, 0);
    troop.set_rate(&mut c, 0);
    troop.set_byte_19(&mut c, 0);
    c.set_reg(CX, count.wrapping_add(1));
    c.ret()
}

/// `loc_0764d` (BP of `loc_06e02`, for each troop SI, CF for byte 3 below
/// 80h): a troop with bit 80h of byte 10h is lost ([`troop_lost_7655`]).
pub fn lose_troop_764d(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x764d)?;
    let mut c = Ctx::new(m);
    if c.flag(CF) {
        c.run(start, 2)?;
        return c.ret();
    }
    if Troop(c.reg(SI)).byte_10(&mut c) & 0x80 == 0 {
        c.run(start, 4)?;
        return c.ret();
    }
    c.run(start, 6)?;
    troop_lost_7655(&mut c)
}

/// `loc_068da` (for each troop SI): with bit 10h of byte 10h,
/// [`troop_at_68e0`].
pub fn troop_68da(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x68da)?;
    let mut c = Ctx::new(m);
    if Troop(c.reg(SI)).byte_10(&mut c) & 0x10 == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    c.run(start, 2)?;
    troop_at_68e0(&mut c)
}

/// `loc_068e0`, the end of `loc_068da` and `loc_075ea`: bit 10h of byte
/// 10h of the troop SI cleared, one more in CX, `troop_location_dirty`
/// (846Ch; CX kept).
fn troop_at_68e0(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x68e0, 4)?;
    let troop = Troop(c.reg(SI));
    let b = troop.byte_10(c);
    troop.set_byte_10(c, b & !0x10);
    let n = c.reg(CX).wrapping_add(1);
    c.set_reg(CX, n);
    c.save(&[CX]);
    c.call(0x846c, 0x68e9)?;
    c.run(0x68e9, 2)?;
    c.restore(&[CX]);
    c.ret()
}

/// `loc_06e0f` (for each troop SI): bit 20h of word 10h moved to bit
/// 1000h of word 12h.
pub fn troop_6e0f(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6e0f)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let w = troop.word_10(&mut c);
    let set = w & 0x20 != 0;
    c.run(start, if set { 5 } else { 3 })?;
    if set {
        troop.set_word_10(&mut c, w & !0x20);
        let w12 = troop.word_12(&mut c);
        troop.set_word_12(&mut c, w12 | 0x1000);
    }
    c.ret()
}

/// `loc_075ea` (BP of `loc_07443`, for each troop SI of a raided
/// location, CF for byte 3 below 80h): a troop with bit 80h of byte 10h
/// leaves (`unlink_troop`, the bit cleared, `link_troop`); with CL 8 or
/// more it is lost ([`troop_lost_7655`]), else it starts over: byte 3
/// A0h, its size and motivation random from 1464h (mask F7Fh), skills 0
/// and 1 from 0A0Ah (mask 1F1Fh), byte 19h 0, then [`troop_at_68e0`].
pub fn raid_troop_75ea(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x75ea)?;
    let mut c = Ctx::new(m);
    if c.flag(CF) {
        c.run(start, 2)?;
        return c.ret();
    }
    if Troop(c.reg(SI)).byte_10(&mut c) & 0x80 == 0 {
        c.run(start, 4)?;
        return c.ret();
    }
    c.run(start, 4)?;
    c.call(0x858c, 0x75f5)?;
    c.run(0x75f5, 2)?;
    let troop = Troop(c.reg(SI));
    let b = troop.byte_10(&mut c);
    troop.set_byte_10(&mut c, b & !0x80);
    c.call(0x851f, 0x75fc)?;
    let lost = c.reg8(r8::CL) >= 8;
    c.run(0x75fc, 5)?;
    if lost {
        return troop_lost_7655(&mut c);
    }
    Troop(c.reg(SI)).set_state(&mut c, 0xa0);
    let v = random_from(&mut c, 0x0f7f, 0x1464, 0x760b)?;
    let troop = Troop(c.reg(SI));
    troop.set_size(&mut c, v as u8);
    troop.set_motivation(&mut c, (v >> 8) as u8);
    let v = random_from(&mut c, 0x1f1f, 0x0a0a, 0x761a)?;
    let troop = Troop(c.reg(SI));
    troop.set_skill(&mut c, 0, v as u8);
    troop.set_skill(&mut c, 1, (v >> 8) as u8);
    troop.set_byte_19(&mut c, 0);
    troop_at_68e0(&mut c)
}

/// In `raid_troop_75ea`: AX = `base` + a random number masked by `mask`
/// (`loc_0e3b7` with BX the mask), the call returning to `back`.
fn random_from(c: &mut Ctx, mask: u16, base: u16, back: u16) -> Result<u16, Bail> {
    c.set_reg(BX, mask);
    c.call(0xe3b7, back)?;
    c.run(back, 5)?;
    let v = c.reg(AX).wrapping_add(base);
    c.set_reg(AX, v);
    Ok(v)
}

/// `loc_075af` (BP of `loc_07443`, for each troop SI of a raided
/// location DI, CF for byte 3 below 80h): troop 3 is no longer started
/// and goes on into `troop_bit_10_by_location` (6B8Ah); a started troop
/// gets byte 3 22h; any other gets bit 400h of word 10h (and 20h for a
/// location with byte 8 28h or more), 4 motivation (`add_motivation`), 3
/// of skill 1 (`add_skill`), and goes on into `loc_06acb` with CL 4.
pub fn raid_troop_75af(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x75af)?;
    let mut c = Ctx::new(m);
    if !c.flag(CF) {
        c.run(start, 2)?;
        return c.ret();
    }
    let troop = Troop(c.reg(SI));
    let state = troop.state(&mut c);
    if troop == Troop::number(3) {
        c.run(start, 5)?;
        troop.set_state(&mut c, state & !troop_state::STARTED);
        return c.jump(0x6b8a);
    }
    if state & troop_state::STARTED != 0 {
        c.run(start, 7)?;
        troop.set_state(&mut c, 0x22);
        return c.ret();
    }
    let far = Location(c.reg(DI)).byte_08(&mut c) >= 0x28;
    c.run(start, if far { 11 } else { 10 })?;
    let w = troop.word_10(&mut c) | 0x400 | if far { 0x20 } else { 0 };
    troop.set_word_10(&mut c, w);
    c.set_reg8(r8::AL, 4);
    c.call(0x6f48, 0x75d1)?;
    c.run(0x75d1, 3)?;
    c.set_reg(BX, 1);
    c.set_reg8(r8::AL, 3);
    c.call(0x6edd, 0x75d9)?;
    c.run(0x75d9, 2)?;
    c.set_reg8(r8::CL, 4);
    c.jump(0x6acb)
}

/// `loc_034d0` (for each troop SI, unless started): counts bytes from a
/// base DX of 7Fh (a troop at its own location) or 61h. One more at the
/// base (the byte before it for a troop without bit 80h of byte 10h);
/// such a troop with byte 3 exactly 80h is counted at [`NOT_TROOPS`]
/// instead and stops there. Then one more at base + 1 + its job (the
/// low two bits dropped when both are set), and when base + that job is
/// below 7Fh also one more at 71h + the low nibble of word 12h. AL byte
/// 3, AX the last index, BX the last count's offset.
pub fn census_34d0(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x34d0)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let state = troop.state(&mut c);
    if state & troop_state::STARTED != 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let own_place = state & troop_state::OWN_LOCATION != 0;
    let base: u16 = if own_place { 0x7f } else { 0x61 };
    let apart = troop.byte_10(&mut c) & 0x80 != 0;
    let mut cost = 9 + own_place as u32 + if apart { 0 } else { 3 };
    if !apart && state == troop_state::NOT_A_TROOP {
        c.run(start, cost + 2)?;
        c.set_reg(DX, base);
        c.set_reg8(r8::AL, state);
        c.set_reg(BX, base - 1);
        bump(&mut c, NOT_TROOPS);
        return c.ret();
    }
    let job = state & 0x0f;
    let job = if state & 3 == 3 { job & !3 } else { job };
    let at = base + job as u16;
    let more = at < 0x7f;
    cost += 11 + (state & 3 == 3) as u32 + if more { 6 } else { 1 };
    c.run(start, cost)?;
    c.set_reg(DX, base);
    bump(&mut c, if apart { base } else { base - 1 });
    bump(&mut c, at + 1);
    c.set_reg(AX, job as u16);
    c.set_reg(BX, at);
    if more {
        let kind = troop.word_12(&mut c) & 0x0f;
        c.set_reg(AX, kind);
        c.set_reg(BX, 0x71 + kind);
        bump(&mut c, 0x71 + kind);
    }
    c.ret()
}

/// One more in the count byte at `at`.
fn bump(c: &mut Ctx, at: u16) {
    let v = c.byte(at).wrapping_add(1);
    c.set_byte(at, v);
}

/// The count of records not troops of yours (`census_34d0`, [90h]).
const NOT_TROOPS: u16 = 0x90;

/// `loc_06ad4`: the troop SI takes the job CL (0Ah instead of 8 at the
/// location 7C8h while its byte 1Ah is zero), with DI its location (word
/// 4): bits 30h of byte 12h and 100h of word 10h cleared, the latter set
/// again unless `loc_06c15` gives CF; `loc_08461`, `loc_06b25`; for a
/// job other than 2, bit `loc_0693b` (AL) of 2000h ORed into word 12h;
/// `loc_07847` for the troop shown ([1954h]).
pub fn set_job_6ad4(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6ad4)?;
    set_job_at_6ad4(&mut Ctx::new(m))
}

pub(super) fn set_job_at_6ad4(c: &mut Ctx) -> Result<(), Bail> {
    let troop = Troop(c.reg(SI));
    let place = troop.location(c);
    let job = c.reg8(r8::CL);
    // Job 8 at the location 7C8h becomes 0Ah while its byte 1Ah is zero.
    let special = job == 8 && place == SPECIAL_PLACE;
    let quiet = special && Location(place).byte_1a(c) == 0;
    let n = if job != 8 {
        3
    } else if !special {
        5
    } else if quiet {
        8
    } else {
        7
    };
    c.run(0x6ad4, n + 4)?;
    c.set_reg(DI, place);
    let job = if quiet { 0x0a } else { job };
    c.set_reg8(r8::CL, job);
    troop.set_state(c, job);
    let b = troop.byte_12(c);
    troop.set_byte_12(c, b & !0x30);
    let w = troop.word_10(c);
    troop.set_word_10(c, w & !0x100);
    c.call(0x6c15, 0x6af9)?;
    let troop = Troop(c.reg(SI));
    if c.flag(CF) {
        c.run(0x6af9, 2)?;
    } else {
        c.run(0x6af9, 3)?;
        let w = troop.word_10(c);
        troop.set_word_10(c, w | 0x100);
    }
    c.call(0x8461, 0x6b03)?;
    c.run(0x6b03, 1)?;
    c.call(0x6b25, 0x6b06)?;
    let troop = Troop(c.reg(SI));
    if troop.state(c) == 2 {
        c.run(0x6b06, 2)?;
    } else {
        c.run(0x6b06, 3)?;
        c.call(0x693b, 0x6b0f)?;
        c.run(0x6b0f, 4)?;
        let troop = Troop(c.reg(SI));
        let activity = c.reg8(r8::AL);
        c.set_reg8(r8::CL, activity);
        let bit = (0x2000u32 << (activity & 0x1f)) as u16;
        c.set_reg(AX, bit);
        let w = troop.word_12(c);
        troop.set_word_12(c, w | bit);
    }
    let troop = Troop(c.reg(SI));
    let id = troop.id(c);
    c.run(0x6b19, 4)?;
    c.set_reg8(r8::AL, id);
    if id != c.byte(SHOWN_TROOP) {
        return c.ret();
    }
    c.call(0x7847, 0x6b24)?;
    c.run(0x6b24, 1)?;
    c.ret()
}

/// The location where job 8 can become 0Ah (`set_job_6ad4`).
const SPECIAL_PLACE: u16 = 0x07c8;
/// The number of the troop the panel shows ([1954h]).
const SHOWN_TROOP: u16 = 0x1954;

/// `loc_06ecb` (for each troop SI): with bit 10h of byte 12h, its job
/// set again ([`set_job_6ad4`] with CL its job).
pub fn redo_job_6ecb(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6ecb)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    if troop.byte_12(&mut c) & 0x10 == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    c.run(start, 5)?;
    let job = troop.job(&mut c);
    c.set_reg8(r8::CL, job);
    set_job_at_6ad4(&mut c)
}

/// `loc_07506` (for each troop SI): bit 10h of byte 10h for a troop with
/// bit 80h there, else started.
pub fn mark_troop_7506(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7506)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    c.run(start, 4)?;
    let b10 = troop.byte_10(&mut c);
    if b10 & 0x80 != 0 {
        troop.set_byte_10(&mut c, b10 | 0x10);
    } else {
        let s = troop.state(&mut c);
        troop.set_state(&mut c, s | troop_state::STARTED);
    }
    c.ret()
}

/// `loc_06155` (for each troop SI): AX its size; a troop with bit 80h of
/// byte 10h adds it to [`SIZE_APART`]; one with byte 3 exactly 6 (job 6,
/// not started) adds it to DX, its rate to CX and its word 0Eh to BX.
pub fn sum_troop_6155(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6155)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let apart = troop.byte_10(&mut c) & 0x80 != 0;
    let job6 = !apart && troop.state(&mut c) == 6;
    c.run(start, if apart { 6 } else if !job6 { 7 } else { 12 })?;
    let size = troop.size(&mut c) as u16;
    c.set_reg(AX, size);
    if apart {
        let v = c.word(SIZE_APART).wrapping_add(size);
        c.set_word(SIZE_APART, v);
        return c.ret();
    }
    if !job6 {
        return c.ret();
    }
    // (Its test of bit 20h of byte 3 always finds it clear here.)
    let dx = c.reg(DX).wrapping_add(size);
    c.set_reg(DX, dx);
    let cx = c.reg(CX).wrapping_add(troop.rate(&mut c));
    c.set_reg(CX, cx);
    let bx = c.reg(BX).wrapping_add(troop.word_0e(&mut c));
    c.set_reg(BX, bx);
    c.ret()
}

/// The sizes of the troops with bit 80h of byte 10h (`sum_troop_6155`).
const SIZE_APART: u16 = 0xd81c;

/// `loc_08403`: for the troop SI unless started: job 1 goes on into
/// `mark_troop_10` (7085h); any other but 6 gets `loc_06acb` with CL 6.
/// AL the job.
pub fn troop_8403(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8403)?;
    let mut c = Ctx::new(m);
    let state = Troop(c.reg(SI)).state(&mut c);
    if state & troop_state::STARTED != 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let job = state & 0x0f;
    c.run(start, 6 + match job {
        1 => 1,
        6 => 3,
        _ => 4,
    })?;
    c.set_reg8(r8::AL, job);
    match job {
        1 => c.jump(0x7085),
        6 => c.ret(),
        _ => {
            c.set_reg8(r8::CL, 6);
            c.call(0x6acb, 0x841b)?;
            c.run(0x841b, 1)?;
            c.ret()
        }
    }
}

/// `loc_072a2` (a troop callback): for a troop SI with byte 3 exactly 4,
/// one more in CX and its skill 1 added to DX.
pub fn tally_72a2(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x72a2)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let counted = troop.state(&mut c) == 4;
    c.run(start, if counted { 6 } else { 3 })?;
    if counted {
        let cx = c.reg(CX).wrapping_add(1);
        c.set_reg(CX, cx);
        let dx = c.reg(DX).wrapping_add(troop.skill(&mut c, 1) as u16);
        c.set_reg(DX, dx);
    }
    c.ret()
}
