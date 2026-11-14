//! Troops: 1Bh-byte records from [`g::TROOPS`], numbered from 1; and the
//! per-location byte 19h, its bits spread over bytes and back.

use super::pure;
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

/// `loc_06757`: for troop AX (`troop_ptr`), unless `troop_screen_place`
/// sets CF (off screen): its sprite list (`troop_sprite_list`), and with
/// one (BP 1 or more) a particle for it (`loc_0c5cf`; SI kept), CF clear.
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
    // `cmp bp,1`: CF for no list.
    let none = c.reg(BP) < 1;
    c.run(0x6762, if none { 3 } else { 4 })?;
    c.set_flag(CF, none);
    if none {
        return c.ret();
    }
    c.save(&[SI]);
    c.call(0xc5cf, 0x676b)?;
    c.run(0x676b, 3)?;
    c.restore(&[SI]);
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
    // `or`: its flags are returned (callers read them).
    let s = c.or(B, s as u16, troop_state::MARKED as u16);
    troop.set_state(c, s as u8);
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
    // The flags of its last `shl al,1` (0 in, no carry: the walk's end).
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
    let redrawing = c.byte(REDRAWING);
    c.run(start, 3)?;
    if redrawing != 0 {
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
/// `ops`): CF set when one of the troops of the location DI's chain has
/// bit 400h of word 12h. SI kept.
fn location_has_400(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 5)?;
    ops(c);
    let first = Location(c.reg(DI)).first_troop(c);
    c.set_reg8(r8::AL, first);
    if first == 0 {
        c.set_flag(CF, false);
        return c.ret();
    }
    c.save(&[SI]);
    c.call(code::TROOP_PTR, 0x1e2f)?;
    loop {
        let troop = Troop(c.reg(SI));
        if troop.word_12(c) & 0x400 != 0 {
            c.run(0x1e2f, 5)?;
            c.restore(&[SI]);
            c.set_flag(CF, true);
            return c.ret();
        }
        let next = troop.next(c);
        c.run(0x1e2f, if next == 0 { 8 } else { 6 })?;
        c.set_reg8(r8::AL, next);
        if next == 0 {
            c.restore(&[SI]);
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

/// `loc_01e01`: in phase 5Dh, for the record SI of kind 7 (byte 0Eh) at
/// a location (byte 2 80h, byte 3 its number; BX word 2): byte 0 2, and CF
/// from [`location_has_400`] on that location (DI, AX); CF clear
/// otherwise.
pub fn troop_check_1e01(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1e01)?;
    let mut c = Ctx::new(m);
    let record = c.reg(SI);
    let phase_ok = c.byte(g::GAME_PHASE) == 0x5d;
    let kind_ok = phase_ok && c.byte(record.wrapping_add(0x0e)) == 7;
    let w2 = c.word(record.wrapping_add(2));
    if !(kind_ok && w2 & 0xff == 0x80) {
        c.run(start, if !phase_ok { 4 } else if !kind_ok { 6 } else { 9 })?;
        if kind_ok {
            c.set_reg(BX, w2);
        }
        c.set_flag(CF, false);
        return c.ret();
    }
    location_has_400(&mut c, start, 13, |c| {
        c.set_reg(BX, w2);
        c.set_byte(record, 2);
        let place = Location::number((w2 >> 8) as u8).offset();
        c.set_reg(AX, place);
        c.set_reg(DI, place);
    })
}

/// `loc_01eda`: every 32nd call ([`TICK_F9`] up by 8 to 0): `loc_01eb1`
/// for each troop (`for_each_troop`), the message pair 709h (`loc_029f0`),
/// [`COUNT_F8`] one down, and [`FIRST_400`] = the first location with a
/// troop of bit 400h of word 12h (`location_has_troop_400`); when none,
/// 0 and `loc_011cb`. CX, SI kept.
pub fn troops_1eda(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1eda)?;
    let mut c = Ctx::new(m);
    let tick = c.byte(TICK_F9).wrapping_add(8);
    c.run(start, if tick != 0 { 3 } else { 6 })?;
    c.set_byte(TICK_F9, tick);
    if tick != 0 {
        return c.ret();
    }
    c.save(&[CX, SI]);
    c.set_reg(BP, 0x1eb1);
    c.call(code::FOR_EACH_TROOP, 0x1ee9)?;
    c.run(0x1ee9, 2)?;
    c.set_reg(AX, 0x709);
    c.call(0x29f0, 0x1eef)?;
    c.run(0x1eef, 3)?;
    let n = c.byte(COUNT_F8).wrapping_sub(1);
    c.set_byte(COUNT_F8, n);
    c.set_reg(DI, Location::FIRST);
    c.call(0x1e24, 0x1ef9)?;
    // Each location in turn until `location_has_troop_400` gives CF.
    let found = loop {
        let place = c.reg(DI);
        if c.flag(CF) {
            c.run(0x1ef9, 7)?;
            break place;
        }
        let next = place.wrapping_add(Location::SIZE);
        if c.byte(next) == 0xff {
            c.run(0x1ef9, 9)?;
            c.set_reg(DI, 0);
            break 0;
        }
        c.run(0x1ef9, 5)?;
        c.set_reg(DI, next);
        c.call(0x1e24, 0x1ef9)?;
    };
    c.set_word(FIRST_400, found);
    if found == 0 {
        c.call(0x11cb, 0x1f10)?;
        c.run(0x1f10, 3)?;
    }
    c.restore(&[CX, SI]);
    c.ret()
}

/// Up by 8 each call of `troops_1eda` ([0F9h]).
const TICK_F9: u16 = 0xf9;
/// One down every 32nd call of `troops_1eda` ([0F8h]).
const COUNT_F8: u16 = 0xf8;
/// The first location with a troop of bit 400h, or 0 ([11DBh]).
const FIRST_400: u16 = 0x11db;

/// `loc_0221d`: the record SI moved to the place of `loc_05344` (from its
/// word 0): words 0 and 2 = `loc_040ae`'s DX, BX; bit 4 of byte 0Fh when
/// BH is the location [`PLAYER_PLACE`]. CX, SI kept.
pub fn troop_to_place(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x221d)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.save(&[CX, SI]);
    let w = c.word(c.reg(SI));
    c.set_reg(DX, w);
    c.call(0x5344, 0x2224)?;
    c.run(0x2224, 1)?;
    c.call(0x40ae, 0x2227)?;
    let at_player = c.reg8(r8::BH) == c.byte(PLAYER_PLACE);
    c.run(0x2227, if at_player { 8 } else { 7 })?;
    c.restore(&[CX, SI]);
    let record = c.reg(SI);
    let (x, y) = (c.reg(DX), c.reg(BX));
    c.set_word(record, x);
    c.set_word(record.wrapping_add(2), y);
    if at_player {
        let b = c.byte(record.wrapping_add(0x0f));
        c.set_byte(record.wrapping_add(0x0f), b | 4);
    }
    c.ret()
}

/// The number of the location the player is at ([9]).
const PLAYER_PLACE: u16 = 9;

/// `loc_01d66`: for the 12 10h-byte records from [`RECORDS_FD8`] at a
/// location (byte 2 80h, byte 3 its number, not FFh): word 0 = (the
/// location's byte 8, 1) when its byte 1 differs from that, or its byte 0
/// is above the limit for it (CS:[`LIMITS_1D35`]). The registers as the
/// last record left them.
pub fn refresh_records_fd8(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1d66)?;
    let mut c = Ctx::new(m);
    let (mut ax, mut bx, mut di) = (c.reg(AX), c.reg(BX), c.reg(DI));
    let mut cost = 3;
    let mut writes = Vec::new();
    for i in 0..12 {
        let record = RECORDS_FD8 + 0x10 * i;
        let w2 = c.word(record + 2);
        ax = w2;
        cost += 5;
        let (at_place, number) = (w2 as u8 == 0x80, (w2 >> 8) as u8);
        if !at_place {
            continue;
        }
        cost += 2;
        if number == 0xff {
            continue;
        }
        let place = Location::number(number);
        let w = c.word(record);
        let kind = place.byte_08(&mut c);
        di = place.offset();
        ax = w;
        bx = bx & 0xff00 | kind as u16;
        cost += 8;
        let changed = if (w >> 8) as u8 != kind {
            true
        } else {
            bx = kind as u16;
            let limit = c.byte_in(CS, (kind as u16).wrapping_add(LIMITS_1D35));
            cost += 3;
            w as u8 > limit
        };
        if changed {
            cost += 3;
            ax = (kind as u16) << 8 | 1;
            writes.push((record, ax));
        }
    }
    c.run(start, cost)?;
    for (record, v) in writes {
        c.set_word(record, v);
    }
    c.set_reg(AX, ax);
    c.set_reg(BX, bx);
    c.set_reg(DI, di);
    c.set_reg(CX, 0);
    c.set_reg(SI, RECORDS_FD8 + 0x10 * 12);
    c.ret()
}

/// Twelve 10h-byte records ([0FD8h]), see `refresh_records_fd8`.
const RECORDS_FD8: u16 = 0x0fd8;
/// Bytes in the code segment: a limit per location kind.
const LIMITS_1D35: u16 = 0x1d35;

/// `loc_01cda`: [`SUM_A8`] = the sum of byte 12h / 8 over the locations
/// `loc_05d36` doesn't skip (CF), plus a random part (`loc_0e3df`) of a
/// sixteenth of it. DX = how many counted.
pub fn sum_locations_a8(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1cda)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_reg(DI, Location::FIRST);
    c.set_reg(CX, 0);
    c.set_reg(DX, 0);
    c.call(0x5d36, 0x1ce4)?;
    loop {
        let skipped = c.flag(CF);
        let place = Location(c.reg(DI));
        let next = place.offset().wrapping_add(Location::SIZE);
        let last = c.byte(next) == 0xff;
        c.run(0x1ce4, if skipped { 1 } else { 8 } + 3 + if last { 6 } else { 1 })?;
        if !skipped {
            let dx = c.reg(DX).wrapping_add(1);
            c.set_reg(DX, dx);
            let part = (place.byte_12(&mut c) >> 3) as u16;
            c.set_reg(AX, part);
            let cx = c.reg(CX).wrapping_add(part);
            c.set_reg(CX, cx);
        }
        c.set_reg(DI, next);
        if last {
            break;
        }
        c.call(0x5d36, 0x1ce4)?;
    }
    let sum = c.reg(CX);
    c.set_reg(BX, sum >> 4);
    c.call(0xe3df, 0x1d09)?;
    c.run(0x1d09, 3)?;
    let total = c.reg(CX).wrapping_add(c.reg(AX));
    c.set_reg(CX, total);
    c.set_word(SUM_A8, total);
    c.ret()
}

/// Written by `sum_locations_a8` ([0A8h]).
const SUM_A8: u16 = 0xa8;

/// `loc_021fa`: AL = a random number from 2 (`loc_0e3df` below 10, + 2),
/// drawn again while it is 3 before phase 54h, or 0Bh or 6 before phase
/// 24h.
pub fn random_2_to_12(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x21fa)?;
    let mut c = Ctx::new(m);
    loop {
        c.run(start, 2)?;
        c.set_reg(BX, 0x0a);
        c.call(0xe3df, 0x2200)?;
        let v = c.reg8(r8::AL).wrapping_add(2);
        let phase = c.byte(g::GAME_PHASE);
        // The instructions to the `ret` or the `jmp` back.
        let (cost, again) = if phase >= 0x54 {
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
        c.run(0x2200, cost)?;
        c.set_reg8(r8::AL, v);
        if !again {
            return c.ret();
        }
    }
}

/// From 6603 (the stretch from `at`, `prefix` instructions before, done by
/// `ops`): BP's routine for each troop of the location DI's chain (SI the
/// troop; SI, DI and BP kept for it). SI kept.
fn each_troop(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    let first = Location(c.reg(DI)).first_troop(c);
    c.run(at, prefix + if first == 0 { 6 } else { 5 })?;
    ops(c);
    c.save(&[SI]);
    c.set_reg8(r8::AL, first);
    if first == 0 {
        c.restore(&[SI]);
        return c.ret();
    }
    loop {
        c.call(code::TROOP_PTR, 0x660e)?;
        c.run(0x660e, 4)?;
        let next = call_for_troop(c, 0x6613)?;
        c.run(0x6613, if next == 0 { 9 } else { 8 })?;
        c.restore(&[SI, DI, BP]);
        c.set_reg8(r8::AL, next);
        if next == 0 {
            c.restore(&[SI]);
            return c.ret();
        }
    }
}

/// In [`each_troop`] and [`for_each_troop_below_80`]: BP's routine for the
/// troop SI (SI, DI and BP pushed), returning to `back`; the next troop of
/// the chain, from the troop as pushed (the routine may move SI, or unlink
/// the troop).
fn call_for_troop(c: &mut Ctx, back: u16) -> Result<u8, Bail> {
    c.save(&[SI, DI, BP]);
    let callback = c.reg(BP);
    c.call(callback, back)?;
    let pushed = c.word_in(SS, c.reg(SP).wrapping_add(4));
    Ok(Troop(pushed).next(c))
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
        c.set_reg(CX, 0);
        c.set_reg(DX, 0);
    })
}

/// `loc_0661d`: as [`for_each_troop`], for the troops `troop_ptr` gives
/// CF for (byte 3 below 80h: yours), with that CF.
pub fn for_each_troop_below_80(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x661d)?;
    let mut c = Ctx::new(m);
    let first = Location(c.reg(DI)).first_troop(&mut c);
    c.run(start, if first == 0 { 6 } else { 5 })?;
    c.save(&[SI]);
    c.set_reg8(r8::AL, first);
    if first == 0 {
        c.restore(&[SI]);
        return c.ret();
    }
    loop {
        c.call(code::TROOP_PTR, 0x6628)?;
        let next = if c.flag(CF) {
            c.run(0x6628, 5)?;
            let next = call_for_troop(&mut c, 0x662f)?;
            c.run(0x662f, if next == 0 { 9 } else { 8 })?;
            c.restore(&[SI, DI, BP]);
            next
        } else {
            let next = Troop(c.reg(SI)).next(&mut c);
            c.run(0x6628, if next == 0 { 7 } else { 6 })?;
            next
        };
        c.set_reg8(r8::AL, next);
        if next == 0 {
            c.restore(&[SI]);
            return c.ret();
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
    // The flags of the tests it returns on (callers read them).
    if state & troop_state::STARTED != 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let b10 = troop.byte_10(&mut c);
    if b10 & 0x80 != 0 {
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

/// `loc_068eb`: SI = the troop shown: in the zoomed map ([`MAP_MODE`]
/// 80h or more) troop [`SHOWN_TROOP`] (`troop_ptr`, AX kept but AL that
/// number), else the one at [`SHOWN_LIST`] by [`SHOWN_INDEX`] (AX twice
/// the index, AL the troop's number).
pub fn shown_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x68eb)?;
    let mut c = Ctx::new(m);
    let number = c.byte(SHOWN_TROOP);
    if c.byte(MAP_MODE) >= 0x80 {
        c.run(start, 3 + 8)?;
        c.set_reg8(r8::AL, number);
        c.set_reg(SI, Troop::number(number).offset());
        return c.ret();
    }
    c.run(start, 12)?;
    let index = c.byte(SHOWN_INDEX) as u16 * 2;
    let troop = Troop(c.word(SHOWN_LIST.wrapping_add(index)));
    c.set_reg(SI, troop.offset());
    let id = troop.id(&mut c);
    c.set_reg(AX, index & 0xff00 | id as u16);
    c.ret()
}

/// The map's mode ([46EBh]): 80h and up zoomed in, below in hotspot mode.
const MAP_MODE: u16 = 0x46eb;
/// The list of troops `shown_troop` picks from by [`SHOWN_INDEX`].
const SHOWN_LIST: u16 = 0x4758;
const SHOWN_INDEX: u16 = 0x476c;

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

/// `loc_06946`: the particle under (DX, BX), last first (signed: word 0 <
/// DX < word 4, word 2 < BX < word 6, not hidden by bit 40h of byte 0Ch):
/// SI = it, DI = its troop (word 0Ah), CF for a troop of yours (byte 3
/// below 80h). CF clear when none. CX the particles left to look at, AX
/// 11h × the count.
pub fn particle_at(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6946)?;
    let mut c = Ctx::new(m);
    if c.flag(DF) {
        return Err(Bail);
    }
    let (x, y) = (c.reg(DX) as i16, c.reg(BX) as i16);
    let count = c.word(PARTICLES);
    let first = (PARTICLES + 2).wrapping_add(0x11 * (count & 0xff));
    let mut cost = if count == 0 { 6 } else { 7 };
    let mut found = None;
    let mut at = first;
    for left in (1..=count).rev() {
        at = at.wrapping_sub(0x11);
        let word = |c: &mut Ctx, k: u16| c.word(at.wrapping_add(k)) as i16;
        // Each test passed costs 2 more; a hit 3, a miss 1 (`loop`).
        cost += 3;
        let hit = if word(&mut c, 0) >= x {
            false
        } else if {
            cost += 2;
            word(&mut c, 2) >= y
        } {
            false
        } else if {
            cost += 2;
            x >= word(&mut c, 4)
        } {
            false
        } else if {
            cost += 2;
            c.byte(at.wrapping_add(0x0c)) & 0x40 != 0
        } {
            false
        } else {
            cost += 2;
            y < word(&mut c, 6)
        };
        if hit {
            cost += 3;
            found = Some((at, left));
            break;
        }
        cost += 1;
    }
    if count != 0 && found.is_none() {
        cost += 2;
    }
    c.run(start, cost)?;
    if count == 0 {
        c.set_reg(SI, PARTICLES + 2);
        c.set_reg(AX, 0);
        c.set_reg(CX, 0);
        c.set_flag(CF, false);
        return c.ret();
    }
    c.set_reg(AX, 0x11 * (count & 0xff));
    c.set_reg(SI, at);
    match found {
        Some((particle, left)) => {
            c.set_reg(CX, left);
            let troop = Troop(c.word(particle.wrapping_add(0x0a)));
            c.set_reg(DI, troop.offset());
            let yours = troop.state(&mut c) < 0x80;
            c.set_flag(CF, yours);
        }
        None => {
            c.set_reg(CX, 0);
            c.set_flag(CF, false);
        }
    }
    c.ret()
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

/// `loc_068d2`: [`each_troop`] with `troop_68da`, CX = 0.
pub fn for_each_troop_68da(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x68d2)?;
    each_troop(&mut Ctx::new(m), start, 3, |c| {
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
    // The flags of its `shr al,1` twice are returned (callers read them).
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
/// travelling or at a location `loc_05ed0` accepts (ZF; DI the troop's
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
    if troop.state(&mut c) & troop_state::TRAVELLING != 0 {
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

/// `loc_07ee2`: DI = the first of 7 ranges (word pairs from SI; a start of
/// 0 skips it) with DX at or past its start and below its end; 7 when
/// none. SI after the start word of the range found (or past the last),
/// AX the last start word read, CF set when found (callers read the
/// flags: those of its last `cmp`).
pub fn find_range(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7ee2)?;
    let mut c = Ctx::new(m);
    let x = c.reg(DX);
    let step = c.string_step(W);
    let mut at = c.reg(SI);
    let mut cost = 1;
    let mut found = None;
    let mut first = 0;
    for k in 0..7u16 {
        first = c.word(at);
        at = at.wrapping_add(step);
        cost += 3;
        if first != 0 && x >= first {
            cost += 4;
            let end = c.word(at);
            if x < end {
                cost += 1;
                found = Some((k, end));
                break;
            }
        } else if first != 0 {
            cost += 2;
        }
        at = at.wrapping_add(2);
        cost += 4 + (k == 6) as u32;
    }
    c.run(start, cost)?;
    c.set_reg(AX, first);
    c.set_reg(SI, at);
    match found {
        Some((k, end)) => {
            c.set_reg(DI, k);
            c.cmp(W, x, end);
        }
        None => {
            c.set_reg(DI, 7);
            c.cmp(W, 7, 7);
        }
    }
    c.ret()
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

/// `loc_0846c`: the troop SI's sprites redrawn where it stands. DI = its
/// location: where it goes when travelling, else one `loc_05ed0` accepts
/// (ZF). `troop_6757` with its number (CF: off the map), then the sprite
/// list's screen rect marked dirty (`loc_0c6ad` with SI the list; DI
/// kept). DI 0 when there is none (`xor di,di`: its flags returned). SI
/// kept.
pub fn troop_location_dirty(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x846c)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let travelling = troop.state(&mut c) & troop_state::TRAVELLING != 0;
    c.run(start, 5)?;
    c.save(&[SI]);
    let none = |c: &mut Ctx, at: u16| -> Result<(), Bail> {
        c.run(at, 5)?;
        c.set_reg(DI, 0);
        c.restore(&[SI]);
        c.ret()
    };
    if !travelling {
        let place = troop.location(&mut c);
        c.set_reg(DI, place);
        c.call(0x5ed0, 0x8479)?;
        if !c.flag(ZF) {
            return none(&mut c, 0x8479);
        }
        c.run(0x8479, 3)?;
    }
    let id = Troop(c.reg(SI)).id(&mut c);
    c.set_reg8(r8::AL, id);
    c.call(0x6757, 0x8480)?;
    if c.flag(CF) {
        return none(&mut c, 0x8480);
    }
    c.run(0x8480, 4)?;
    let list = c.reg(DI);
    c.set_reg(SI, list);
    c.save(&[DI]);
    c.call(0xc6ad, 0x8488)?;
    c.run(0x8488, 3)?;
    c.restore(&[DI]);
    c.restore(&[SI]);
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

/// `loc_077d7`: the troop shown (`shown_troop`) takes the skill BX at its
/// location: the needs worked out (`skill_needs_46fe`; BX, SI kept), and
/// when the need for BX is at least 1 (2 for skill 1 at the location
/// [`PLACE_1150`]): [`LAST_SKILLS`] = its byte 19h, which gets bit 80h >>
/// BX; CF clear. Else CF set. AL that least need, or the bit.
pub fn troop_take_skill(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x77d7)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x68eb, 0x77da)?;
    c.run(0x77da, 4)?;
    let place = Troop(c.reg(SI)).location(&mut c);
    c.set_reg(DI, place);
    c.save(&[BX, SI]);
    c.call(0x7f27, 0x77e2)?;
    c.restore(&[BX, SI]);
    let (troop, skill, place) = (Troop(c.reg(SI)), c.reg(BX), c.reg(DI));
    let two = skill == 1 && place == c.word(PLACE_1150);
    let least = 1 + two as u8;
    let ok = c.byte(NEEDS_46FE.wrapping_add(skill)) >= least;
    c.run(0x77e2, 8 + if skill == 1 { 2 + two as u32 } else { 0 } + if ok { 7 } else { 0 })?;
    c.set_reg8(r8::AL, least);
    if !ok {
        c.set_flag(CF, true);
        return c.ret();
    }
    let bits = troop.byte_19(&mut c);
    c.set_byte(LAST_SKILLS, bits);
    c.set_reg8(r8::CL, skill as u8);
    let shift = skill & 0x1f;
    let bit = if shift < 8 { 0x80u8 >> shift } else { 0 };
    c.set_reg8(r8::AL, bit);
    troop.set_byte_19(&mut c, bits | bit);
    c.set_flag(CF, false);
    c.ret()
}

/// A location `troop_take_skill` asks two of skill 1 for ([1150h]).
const PLACE_1150: u16 = 0x1150;
/// Where `skill_needs_46fe` puts a location's needs.
const NEEDS_46FE: u16 = 0x46fe;
/// The troop's skill bits before `troop_take_skill` changed them ([3Dh]).
const LAST_SKILLS: u16 = 0x3d;

/// `loc_07b58`: [1BEAh], [46F4h] and [4Ch] cleared, and the troop
/// [`SELECTED`] dropped (0): its state reset (`loc_01ebe`; word 10h kept
/// to bits 3F0h, word 12h without bits 1A00h, its day today), `loc_0a7a5`,
/// the panel 18E9h redrawn ([0DBE0h] 0, `loc_0c6ad`), [47BAh] 0, and
/// `gfx_call_30_unless_redrawing` with AL 4. SI the troop.
pub fn drop_selected_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7b58)?;
    let mut c = Ctx::new(m);
    let selected = c.word(SELECTED);
    c.run(start, if selected == 0 { 8 } else { 10 })?;
    c.set_word(0x1bea, 0);
    c.set_byte(0x46f4, 0);
    c.set_byte(0x4c, 0);
    c.set_word(SELECTED, 0);
    c.set_reg(SI, selected);
    if selected == 0 {
        return c.ret();
    }
    c.call(0x1ebe, 0x7b7c)?;
    c.run(0x7b7c, 3)?;
    let troop = Troop(c.reg(SI));
    let w = troop.word_10(&mut c);
    troop.set_word_10(&mut c, w & 0x3f0);
    let w = troop.word_12(&mut c);
    troop.set_word_12(&mut c, w & 0xe5ff);
    c.call(0x1ac5, 0x7b89)?;
    c.run(0x7b89, 2)?;
    let today = c.reg8(r8::AL);
    Troop(c.reg(SI)).set_day(&mut c, today);
    c.call(0xa7a5, 0x7b8f)?;
    c.run(0x7b8f, 5)?;
    c.set_reg(SI, 0x18e9);
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

/// The troop selected on the map ([46EFh]), 0 for none.
const SELECTED: u16 = 0x46ef;

/// `loc_083bc`: the troop SI relinked (`link_troop`), no longer at its
/// travelling, `reset_location_troop`, `count_location_bits`; for a
/// troop of yours, its job set again (`set_job_6ad4`), and when it is at
/// the location the player is at with the hotspot 1F0Eh up (`loc_0d41b`,
/// BP) and [0Bh] 1 (2 with [2Bh] 0), [473Bh] gets bit 1. CL its byte 3.
pub fn update_troop_83bc(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x83bc)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x851f, 0x83bf)?;
    c.run(0x83bf, 2)?;
    let troop = Troop(c.reg(SI));
    let s = troop.state(&mut c);
    troop.set_state(&mut c, s & !troop_state::TRAVELLING);
    c.call(0x85cc, 0x83c6)?;
    c.run(0x83c6, 1)?;
    c.call(0x7f5f, 0x83c9)?;
    let state = Troop(c.reg(SI)).state(&mut c);
    let yours = state & troop_state::NOT_A_TROOP == 0;
    c.run(0x83c9, if yours { 5 } else { 4 })?;
    if !yours {
        c.set_reg8(r8::CL, state);
        return c.ret();
    }
    c.set_reg8(r8::CL, state & 0x0f);
    c.call(0x6ad4, 0x83d6)?;
    let place = Troop(c.reg(SI)).location(&mut c);
    c.run(0x83d6, 4)?;
    c.set_reg(DI, place);
    // The flags of each exit's last test are returned (callers read them).
    let here = game::here(&mut c);
    if place != here {
        return c.ret();
    }
    c.call(0xd41b, 0x83e2)?;
    let bp = c.reg(BP);
    if bp != 0x1f0e {
        c.run(0x83e2, 3)?;
        return c.ret();
    }
    let want = 1 + (c.byte(0x2b) == 0) as u8;
    let have = c.byte(0x0b);
    let hit = have == want;
    c.run(0x83e2, 8 + hit as u32)?;
    c.set_reg8(r8::AL, want);
    if hit {
        let b = c.byte(0x473b);
        let b = pure::or(B, b as u16, 1);
        c.set_byte(0x473b, b as u8);
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
            c.or(B, 0, 0); // `or al,al` on the chain's 0: its flags are returned
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

/// `loc_085cc`: for a troop SI without bit 80h of word 10h in a slot
/// past 8: taken out of its chain (`unlink_troop`), then the first troop
/// of the location DI's chain without bit 80h of byte 10h that is started
/// is reset (`reset_troop`, registers kept); SI linked again
/// (`link_troop`).
pub fn reset_location_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x85cc)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    if troop.word_10(&mut c) & 0x80 != 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let slot = c.byte(troop.offset().wrapping_add(2));
    c.run(start, 5)?;
    if slot <= 8 {
        return c.ret();
    }
    c.call(0x858c, 0x85dc)?;
    c.run(0x85dc, 2)?;
    c.save(&[SI]);
    let mut id = Location(c.reg(DI)).first_troop(&mut c);
    c.set_reg8(r8::AL, id);
    loop {
        if id == 0 {
            c.run(0x85e0, 4)?;
            break;
        }
        c.run(0x85e0, 3)?;
        c.call(0x6906, 0x85e7)?;
        let other = Troop(c.reg(SI));
        id = other.next(&mut c);
        let apart = other.byte_10(&mut c) & 0x80 != 0;
        let started = other.state(&mut c) & troop_state::STARTED != 0;
        c.run(0x85e7, if apart { 3 } else if !started { 5 } else { 6 })?;
        c.set_reg8(r8::AL, id);
        if apart || !started {
            continue;
        }
        c.call(code::SAVE_REGS, 0x85f9)?;
        c.run(0x85f9, 1)?;
        c.call(0x66b1, 0x85fc)?;
        c.run(0x85fc, 1)?;
        c.call(code::RESTORE_REGS, 0x85ff)?;
        c.run(0x85ff, 2)?;
        break;
    }
    c.restore(&[SI]);
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

/// `loc_06715`: `troop_6757` for every troop of the chains of the
/// locations listed at SS:[`SHOWN_PLACES`] (6 bytes each, to a 0), then
/// for every record of the troop table travelling and without
/// bit 10h of byte 10h.
pub fn refresh_all_troops(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6715)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let mut entry = SHOWN_PLACES;
    c.set_reg(BP, entry);
    loop {
        let place = c.word_in(SS, entry);
        if place == 0 {
            c.run(0x6718, 4)?;
            c.set_reg(DI, 0);
            break;
        }
        let first = Location(place).first_troop(&mut c);
        c.run(0x6718, 8)?;
        c.set_reg(DI, place);
        c.set_reg8(r8::AL, first);
        if first != 0 {
            // Along the chain: `troop_6757` takes the troop number in AL.
            loop {
                c.save(&[BP]);
                c.call(0x6757, 0x672a)?;
                let next = Troop(c.reg(SI)).next(&mut c);
                c.run(0x672a, 6)?;
                c.restore(&[BP]);
                c.set_reg8(r8::AL, next);
                if next == 0 {
                    break;
                }
            }
        }
        entry += 6;
        c.set_reg(BP, entry);
    }
    // On through the table from the troop as `troop_6757` leaves SI.
    let mut at = g::TROOPS.wrapping_sub(Troop::SIZE);
    loop {
        let troop = Troop(at.wrapping_add(Troop::SIZE));
        if troop.offset() >= g::TROOPS_END {
            c.run(0x673a, 4)?;
            c.set_reg(SI, troop.offset());
            return c.ret();
        }
        let marked = troop.byte_10(&mut c) & 0x10 != 0;
        let travelling = troop.state(&mut c) & troop_state::TRAVELLING != 0;
        c.run(0x673a, if marked { 5 } else if !travelling { 7 } else { 9 })?;
        c.set_reg(SI, troop.offset());
        if !marked && travelling {
            let id = troop.id(&mut c);
            c.set_reg8(r8::AL, id);
            c.call(0x6757, 0x6754)?;
            c.run(0x6754, 1)?;
        }
        at = c.reg(SI);
    }
}

/// The locations whose troops `refresh_all_troops` draws: words in the
/// stack segment, 6 bytes apart, ending with 0.
const SHOWN_PLACES: u16 = 0xa5c0;

/// `loc_06d19`: a plain troop SI (no bits E3h of byte 3, none of bit 80h
/// of byte 10h, not troop 3) merges into the troop BX that `loc_06d5f`
/// picks among those of its location (`for_each_troop_below_80`; CL the
/// complement of its size, DX it): its size added there, the skill bits
/// (byte 19h) shared (its own keeps those both have), bit 200h of word
/// 12h there; then it is reset (`reset_troop`). CF set when merged.
pub fn merge_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6d19)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let cost = if troop.state(&mut c) & 0xe3 != 0 {
        3
    } else if troop.byte_10(&mut c) & 0x80 != 0 {
        5
    } else if troop == Troop::number(3) {
        7
    } else {
        13
    };
    c.run(start, cost)?;
    if cost != 13 {
        c.set_flag(CF, false);
        return c.ret();
    }
    let place = troop.location(&mut c);
    c.set_reg(DI, place);
    c.set_reg(BX, 0);
    let size = troop.size(&mut c);
    c.set_reg8(r8::CL, !size);
    c.set_reg(DX, troop.offset());
    c.set_reg(BP, 0x6d5f);
    c.call(0x661d, 0x6d3d)?;
    let into = c.reg(BX);
    if into == 0 {
        c.run(0x6d3d, 3)?;
        c.set_flag(CF, false);
        return c.ret();
    }
    c.run(0x6d3d, 11)?;
    let (troop, other) = (Troop(c.reg(SI)), Troop(into));
    let sum = other.size(&mut c).wrapping_add(troop.size(&mut c));
    other.set_size(&mut c, sum);
    let mine = troop.byte_19(&mut c);
    let both = mine & other.byte_19(&mut c);
    c.set_reg(AX, (mine as u16) << 8 | both as u16);
    troop.set_byte_19(&mut c, both);
    let theirs = other.byte_19(&mut c);
    other.set_byte_19(&mut c, theirs | mine);
    let w = other.word_12(&mut c);
    other.set_word_12(&mut c, w | 0x200);
    c.call(0x66b1, 0x6d5d)?;
    c.run(0x6d5d, 2)?;
    c.set_flag(CF, true);
    c.ret()
}

/// `loc_079de`: with [`PANEL_DIRTY`] set (then cleared), the panel 18DFh
/// redrawn on the front buffer ([0DBE0h] 0, `loc_0c6ad`) and
/// `gfx_call_30_unless_redrawing` with AL 8; else only the front buffer
/// made active. AX the flag.
pub fn redraw_panel_18df(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x79de)?;
    let mut c = Ctx::new(m);
    let dirty = c.word(PANEL_DIRTY);
    if dirty == 0 {
        c.run(start, 5)?;
        c.set_reg(AX, 0);
        // `or ax,ax`: its flags pass through to the callers, who read them.
        return super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c);
    }
    c.run(start, 7)?;
    c.set_word(PANEL_DIRTY, 0);
    c.set_reg(AX, dirty);
    c.set_reg(SI, 0x18df);
    c.call(0xc07c, 0x5fa2)?;
    c.run(0x5fa2, 2)?;
    c.set_word(0xdbe0, 0);
    c.call(0xc6ad, 0x5fab)?;
    c.run(0x5fab, 2)?;
    c.set_reg8(r8::AL, 8);
    gfx_call_30_unless_redrawing(c.m, 0x7b2b)
}

/// The troop whose panel is to be drawn ([46FAh]), 0 for none.
const PANEL_DIRTY: u16 = 0x46fa;

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

/// `loc_08018`: the need BX at [`NEEDS_4C60`] one lower (unless 0 or 80h
/// and up) for each troop travelling to DI and moving (job bits 3)
/// to elsewhere (word 0Ch) with BL in byte 0Eh; then, at the location
/// [`PLACE_1150`] before phase 50h, need 1 one lower the same way. AL as
/// the last troop's tests left it.
pub fn lower_needs_by_movers(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8018)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let (place, skill) = (c.reg(DI), c.reg(BX));
    let need_at = NEEDS_4C60.wrapping_add(skill);
    for troop in Troop::all() {
        let state = troop.state(&mut c);
        let need = c.byte(need_at);
        // How far its tests go (each `jne` a step), and the cost of each.
        let travelling = state & troop_state::TRAVELLING != 0;
        let moving = state & 3 == 3;
        let lowers = travelling
            && moving
            && troop.location(&mut c) == place
            && c.word(troop.offset().wrapping_add(0x0c)) != place
            && c.byte(troop.offset().wrapping_add(0x0e)) == skill as u8;
        let cost = 6 + if !travelling {
            0
        } else if !moving {
            3
        } else if troop.location(&mut c) != place {
            5
        } else if c.word(troop.offset().wrapping_add(0x0c)) == place {
            7
        } else if !lowers {
            9
        } else {
            11 + if need.wrapping_sub(1) & 0x80 != 0 { 2 } else { 0 }
        };
        c.run(0x801b, cost)?;
        c.set_reg(SI, troop.offset());
        c.set_reg8(r8::AL, if travelling { state & 3 } else { state });
        if lowers && need.wrapping_sub(1) & 0x80 == 0 {
            c.set_byte(need_at, need - 1);
        }
    }
    c.run(0x801b, 3)?;
    c.set_reg(SI, g::TROOPS_END);
    if place != c.word(PLACE_1150) {
        c.run(0x804c, 3)?;
        return c.ret();
    }
    if c.byte(g::GAME_PHASE) >= 0x50 {
        c.run(0x804c, 5)?;
        return c.ret();
    }
    let need = c.byte(NEEDS_4C60 + 1);
    let stays = need.wrapping_sub(1) & 0x80 != 0;
    c.run(0x804c, 8 + stays as u32)?;
    if !stays {
        c.set_byte(NEEDS_4C60 + 1, need - 1);
    }
    c.ret()
}

/// The needs `lower_needs_by_movers` lowers ([4C60h], one byte a skill).
const NEEDS_4C60: u16 = 0x4c60;

/// `loc_0708a`: the troop SI's production rate into word 0Ch (AX): its
/// effective motivation (`troop_motivation`) plus the top of skill 0,
/// times its size (a quarter without bit 80h of byte 19h), times the top
/// of the location DI's byte 12h + 1 (by the bytes of that +1), /128, 9
/// bits. DX = the old rate less the new. When it changes, bits Ch of
/// word 10h (CX) say which way: 8 up, 4 down (BL). Callers read the flags
/// (its last `sub` or `or`).
pub fn troop_rate(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x708a)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x6efd, 0x708d)?;
    let (troop, place) = (Troop(c.reg(SI)), Location(c.reg(DI)));
    let motivation = c.reg8(r8::AL);
    let base = motivation.wrapping_add(troop.skill(&mut c, 0) & 0xf0);
    let mut product = base as u16 * troop.size(&mut c) as u16;
    let quartered = troop.byte_19(&mut c) & 0x80 == 0;
    if quartered {
        product >>= 2;
    }
    // `inc ax` over (product's high byte, the location's top bits), then
    // `mul ah`: those two bytes multiplied.
    let pair = (product & 0xff00 | (place.byte_12(&mut c) & 0xf0) as u16).wrapping_add(1);
    let rate = ((pair & 0xff) * (pair >> 8)).swap_bytes().rotate_left(1) & 0x01ff;
    let old = troop.rate(&mut c);
    let changed = old != rate;
    let up = old < rate;
    c.run(0x708d, 18 + if quartered { 2 } else { 0 } + if changed { 6 + !up as u32 } else { 0 })?;
    c.set_reg(AX, rate);
    troop.set_rate(&mut c, rate);
    let less = pure::sub(W, old, rate);
    c.set_reg(DX, less);
    if !changed {
        return c.ret();
    }
    let way: u8 = if up { 8 } else { 4 };
    c.set_reg8(r8::BL, way);
    let w = troop.word_10(&mut c);
    let low = pure::or(B, w & 0xf3, way as u16);
    let w = w & 0xff00 | low;
    c.set_reg(CX, w);
    troop.set_word_10(&mut c, w);
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

/// `loc_06639`: BP's routine for each troop of the location DI's chain
/// (`for_each_troop`), then for each record of the troop table that is at
/// travelling (bit 40h of byte 3) to DI (word 4, or word
/// 0Ch while moving: job bits 3), with the flags of byte 3 against 80h
/// (CF: yours). SI kept.
pub fn for_troops_at_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6639)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::FOR_EACH_TROOP, 0x663c)?;
    c.run(0x663c, 2)?;
    c.save(&[SI]);
    for troop in Troop::all() {
        c.set_reg(SI, troop.offset());
        let state = troop.state(&mut c);
        if state & troop_state::TRAVELLING == 0 {
            c.run(0x6640, 3)?;
            c.set_reg8(r8::AL, state);
        } else {
            let moving = state & 3 == 3;
            // While moving, word 0Ch holds where it goes.
            let place = if moving { c.word(troop.offset().wrapping_add(0x0c)) } else { troop.location(&mut c) };
            let here = place == c.reg(DI);
            c.run(0x6640, 11 + moving as u32 + if here { 5 } else { 0 })?;
            c.set_reg8(r8::AL, state & 3);
            c.save(&[BX]);
            c.restore(&[BX]);
            if here {
                c.save(&[SI, DI, BP]);
                // The routine gets the flags of `cmp byte 3, 80h`.
                c.cmp(B, state as u16, 0x80);
                let callback = c.reg(BP);
                c.call(callback, 0x6662)?;
                c.run(0x6662, 3)?;
                c.restore(&[SI, DI, BP]);
            }
        }
        let last = troop.offset().wrapping_add(Troop::SIZE) >= g::TROOPS_END;
        c.run(0x6665, if last { 5 } else { 3 })?;
        if last {
            c.set_reg(SI, troop.offset().wrapping_add(Troop::SIZE));
            c.restore(&[SI]);
            return c.ret();
        }
    }
    unreachable!("the loop returns at the last troop")
}

/// `loc_0858c`: the troop SI taken out of its location's chain (unless at
/// travelling): the location's first troop or the troop before it
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
    if state & troop_state::TRAVELLING != 0 {
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

/// `loc_07847`: the troop SI's orders menu: its items (bit 40h of the
/// bytes at [`ORDER_ITEMS`] + 1, + 5, + 9) all on (`menu_start`), then
/// off for what it cannot do now. A troop held (bit 400h of word 12h)
/// keeps them all. Job 2: item 1 off and [`ORDER_ITEMS`] 56h. Else item 1
/// off but for job 1, item 9 off from phase 5; and before phase 4, for a
/// troop with bit 200h of word 10h, or at a location (DI) not taken (bit
/// 8 of byte 0Ah) of byte 8 28h or more, that's all. Otherwise item 5 off
/// when its location still needs a skill or the troop has any
/// (`skill_needs_46fe`; the needs ORed with its byte 19h, AL). Callers
/// read the flags: those of its last test.
pub fn troop_orders_menu(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7847)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let w12 = troop.word_12(&mut c);
    if w12 & 0x400 != 0 {
        c.run(start, 6)?;
        menu_start(&mut c);
        return c.ret();
    }
    let job = troop.job(&mut c);
    if job == 2 {
        c.run(start, 14)?;
        menu_start(&mut c);
        c.set_reg8(r8::AL, job);
        clear_bit_40(&mut c, ORDER_ITEMS + 1);
        c.set_byte(ORDER_ITEMS, 0x56);
        return c.ret();
    }
    let phase = c.byte(g::GAME_PHASE);
    let w10 = troop.word_10(&mut c);
    let place = Location(troop.location(&mut c));
    let taken = place.byte_0a(&mut c) & 8 != 0;
    let kind = place.byte_08(&mut c);
    // The instructions to the test it stops at.
    let mut cost = 15 + (job != 1) as u32 + (phase >= 5) as u32;
    let stop = if phase < 4 {
        cost += 1;
        Some(1)
    } else if w10 & 0x200 != 0 {
        cost += 3;
        Some(2)
    } else if !taken && kind >= 0x28 {
        cost += 8;
        Some(3)
    } else {
        cost += 7 + if taken { 0 } else { 2 };
        None
    };
    c.run(start, cost)?;
    menu_start(&mut c);
    c.set_reg8(r8::AL, job);
    if job != 1 {
        clear_bit_40(&mut c, ORDER_ITEMS + 1);
    }
    if phase >= 5 {
        clear_bit_40(&mut c, ORDER_ITEMS + 9);
    }
    match stop {
        Some(1) => {
            return c.ret();
        }
        Some(2) => {
            return c.ret();
        }
        _ => {}
    }
    c.set_reg(DI, place.offset());
    if stop.is_some() {
        return c.ret();
    }
    c.save(&[SI]);
    c.call(0x7f27, 0x78a3)?;
    let mut needed = 0u8;
    for k in 0..7 {
        needed |= c.byte(NEEDS_46FE + k);
    }
    let troop = Troop(c.word_in(SS, c.reg(SP)));
    let bits = troop.byte_19(&mut c);
    let any = needed | bits;
    // `or al,[si]; inc si; loop` over the 7 needs, then `or al,[si+19h]`.
    c.run(0x78a3, 27 + if any != 0 { 2 } else { 1 })?;
    c.set_reg(SI, NEEDS_46FE + 7);
    c.set_reg(CX, 0);
    c.restore(&[SI]);
    c.set_reg8(r8::AL, any);
    if any != 0 {
        let v = c.byte(ORDER_ITEMS + 5);
        let v = pure::and(B, v as u16, 0xbf);
        c.set_byte(ORDER_ITEMS + 5, v as u8);
    }
    c.ret()
}

/// The orders menu's items: bit 40h of a byte per item on.
const ORDER_ITEMS: u16 = 0x2110;

/// The first three of `loc_07847`: all the menu items on.
fn menu_start(c: &mut Ctx) {
    c.set_word(ORDER_ITEMS, 0x404f);
    for item in [ORDER_ITEMS + 5, ORDER_ITEMS + 9] {
        let v = c.byte(item);
        c.set_byte(item, v | 0x40);
    }
}

/// An item of the orders menu off.
fn clear_bit_40(c: &mut Ctx, item: u16) {
    let v = c.byte(item);
    c.set_byte(item, v & !0x40);
}

/// `loc_0686e`: the troop SI's place on the screen; CF set when it has
/// none. Only with the map zoomed in ([`MAP_MODE`] 80h or more). A troop
/// travelling: its map point (words 6, 8; `map_to_screen`) in
/// DX, BX, CF when off the view (signed: x -10h or less or 148h or more,
/// y -10h or less or A0h or more). Else the place at SS:[BP+2], [BP+4]
/// moved by the offset for its slot (byte 2 - 1, the other side for a
/// location with bit 2 of byte 0Ah) from [`SLOT_OFFSETS`]: DX x + the x
/// offset, AX the y offset; BX the y + that offset, CF clear, when the y
/// word is 8000h or more (its high byte as a flag), else BX the y word
/// and CF set.
pub fn troop_screen_place(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x686e)?;
    let mut c = Ctx::new(m);
    if c.byte(MAP_MODE) < 0x80 {
        c.run(start, 3)?;
        c.set_flag(CF, true);
        return c.ret();
    }
    let troop = Troop(c.reg(SI));
    if troop.state(&mut c) & troop_state::TRAVELLING != 0 {
        c.run(start, 7)?;
        let (x, y) = (c.word(troop.offset().wrapping_add(6)), c.word(troop.offset().wrapping_add(8)));
        c.set_reg(DX, x);
        c.set_reg(BX, y);
        c.call(0xb647, 0x68b8)?;
        let (sx, sy) = (c.reg(DX) as i16, c.reg(BX) as i16);
        // Which of its four tests fails first (each `cmp`, `jle`/`jge`).
        let failed = [sx <= -16, sy <= -16, sx >= 0x148, sy >= 0xa0].iter().position(|&f| f);
        c.run(0x68b8, failed.map_or(10, |k| 2 * k as u32 + 4))?;
        c.set_flag(CF, failed.is_some());
        return c.ret();
    }
    let slot = c.byte(troop.offset().wrapping_add(2));
    let place = Location(troop.location(&mut c));
    let other_side = place.byte_0a(&mut c) & 2 != 0;
    let frame = c.reg(BP);
    let (px, py) = (c.word_in(SS, frame.wrapping_add(2)), c.word_in(SS, frame.wrapping_add(4)));
    let flagged = py >= 0x8000;
    c.run(start, 22 + other_side as u32 + if flagged { 4 } else { 1 })?;
    c.save(&[DI]);
    c.set_reg(DI, place.offset());
    c.restore(&[DI]);
    let index = (slot.wrapping_sub(1) ^ if other_side { 8 } else { 0 }) as u16 & 0x0f;
    let offset = |c: &mut Ctx, k: u16| c.byte(SLOT_OFFSETS.wrapping_add(2 * index + k)) as i8 as u16;
    let (ox, oy) = (offset(&mut c, 0), offset(&mut c, 1));
    c.set_reg(AX, oy);
    c.set_reg(DX, ox.wrapping_add(px));
    if !flagged {
        c.set_reg(BX, py);
        c.set_flag(CF, true);
        return c.ret();
    }
    c.set_reg(BX, (py & 0xff).wrapping_add(oy));
    c.set_flag(CF, false);
    c.ret()
}

/// The offsets of a troop's slot around its place (byte pairs, signed).
const SLOT_OFFSETS: u16 = 0x1672;

/// `loc_01e43`: in phase 5Ch, once the day reaches [`ATTACK_DAY`], unless
/// the player is at [`PALACE_7C8`]: of the locations below kind 28h
/// (byte 8), without bit 80h of byte 0Ah and other than [`PLACE_2C0`],
/// the one where `loc_01ea1` counts the most troops (DX, over its chain)
/// becomes [`FIRST_400`] ([`COUNT_F8`] one more), then `loc_01ea9` for
/// its troops and the message pair 0F08h (`call_29f0_f`). CX the count,
/// BX that location.
pub fn pick_attack_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1e43)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x1ac5, 0x1e46)?;
    let cost = if c.reg(AX) < c.word(ATTACK_DAY) {
        3
    } else if c.byte(g::GAME_PHASE) != 0x5c {
        5
    } else if game::here(&mut c) == PALACE_7C8 {
        7
    } else {
        8
    };
    c.run(0x1e46, cost)?;
    if cost != 8 {
        return c.ret();
    }
    c.set_reg(DI, Location::FIRST);
    c.set_reg(CX, 0);
    let mut place = Location(Location::FIRST);
    loop {
        if place.byte_08(&mut c) >= 0x28 {
            c.run(0x1e60, 2)?;
        } else if place.byte_0a(&mut c) & 0x80 != 0 {
            c.run(0x1e60, 4)?;
        } else if place.offset() == PLACE_2C0 {
            c.run(0x1e60, 6)?;
        } else {
            c.run(0x1e60, 9)?;
            c.set_reg(DX, 0);
            c.set_reg(BP, 0x1ea1);
            c.call(0x661d, 0x1e7a)?;
            let (count, best) = (c.reg(DX), c.reg(CX));
            let more = count > best;
            c.run(0x1e7a, if more { 4 } else { 2 })?;
            place = Location(c.reg(DI));
            if more {
                c.set_reg(CX, count);
                c.set_reg(BX, place.offset());
            }
        }
        let next = place.offset().wrapping_add(Location::SIZE);
        let last = c.byte(next) == 0xff;
        let none = c.reg(CX) == 0;
        c.run(0x1e82, 3 + if last { 1 + none as u32 } else { 0 })?;
        c.set_reg(DI, next);
        place = Location(next);
        if last {
            if none {
                return c.ret();
            }
            break;
        }
    }
    c.run(0x1e8c, 5)?;
    let target = c.reg(BX);
    c.set_reg(DI, target);
    c.set_word(FIRST_400, target);
    let n = c.byte(COUNT_F8).wrapping_add(1);
    c.set_byte(COUNT_F8, n);
    c.set_reg(BP, 0x1ea9);
    c.call(0x661d, 0x1e9c)?;
    c.run(0x1e9c, 2)?;
    c.set_reg8(r8::AL, 8);
    call_29f0_f(c.m, 0x71b2)
}

/// The day from which `pick_attack_location` picks ([1156h]).
const ATTACK_DAY: u16 = 0x1156;
/// Location 63 (7C8h): no attack picked while the player is there; job 8
/// there can become 0Ah (`set_job_6ad4`).
const PALACE_7C8: u16 = 0x07c8;
/// Location 17 (2C0h), never picked by `pick_attack_location`.
const PLACE_2C0: u16 = 0x02c0;

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

/// `loc_07f90`: for the troop SI, the nearest other location from location
/// 3 on (no bit 80h of byte 0Ah, byte 8 below 28h) that still needs skill
/// BX once its own troops' skills and the movers are counted
/// (`skill_needs` into [`NEEDS_4C60`], `lower_needs_by_movers`):
/// [`NEAREST`] it, [`NEAREST_DISTANCE`] its distance (FFFFh while none).
/// The distance: the larger of the x delta's high byte and the y delta
/// (by their low bytes), too far from 32h (by its low byte), a quarter
/// of it for a location with byte 8 from 21h. Its own location, x and y
/// stay pushed (BP over them) while it looks; AX, DX the last deltas.
pub fn nearest_needing_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7f90)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let home = Location(Troop(c.reg(SI)).location(&mut c));
    let (hx, hy) = (c.word(home.offset().wrapping_add(2)), c.word(home.offset().wrapping_add(4)));
    c.set_reg(DI, home.offset());
    c.push(home.offset());
    c.push(hy);
    c.push(hx);
    let sp = c.reg(SP);
    c.set_reg(BP, sp);
    c.set_word(NEAREST_DISTANCE, 0xffff);
    let mut place = Location::number(3);
    c.set_reg(DI, place.offset());
    let abs = |v: u16| if v & 0x8000 != 0 { v.wrapping_neg() } else { v };
    loop {
        if c.word(place.offset()) == 0xffff {
            c.run(0x7fa5, 4)?;
            c.set_reg(SP, sp.wrapping_add(6));
            return c.ret();
        }
        let kind = place.byte_08(&mut c);
        if place.byte_0a(&mut c) & 0x80 != 0 {
            c.run(0x7fa5, 4)?;
        } else if kind >= 0x28 {
            c.run(0x7fa5, 6)?;
        } else if place == home {
            c.run(0x7fa5, 8)?;
        } else {
            let across = c.word(place.offset().wrapping_add(2)).wrapping_sub(hx);
            let down = c.word(place.offset().wrapping_add(4)).wrapping_sub(hy);
            let coarse = abs(across) >> 8;
            let take_down = (coarse as u8) < (abs(down) as u8);
            let mut distance = if take_down { abs(down) } else { coarse };
            let far = distance as u8 >= 0x32;
            let quartered = kind >= 0x21;
            let best = c.word(NEAREST_DISTANCE);
            let negs = (across & 0x8000 != 0) as u32 + (down & 0x8000 != 0) as u32;
            let mut cost = 20 + negs + take_down as u32;
            if !far {
                if quartered {
                    distance >>= 2;
                }
                cost += 4 + if quartered { 2 } else { 0 } + if distance < best { 3 } else { 0 };
            }
            c.run(0x7fa5, cost)?;
            c.set_reg(AX, abs(down));
            c.set_reg(DX, distance);
            if !far && distance < best {
                // The needs there: its own troops' skills off, then movers.
                c.save(&[BX]);
                c.set_reg(BX, NEEDS_4C60);
                c.call(0x7f2a, 0x7ff5)?;
                let skill = c.word_in(SS, c.reg(SP));
                let need = c.byte(NEEDS_4C60.wrapping_add(skill));
                c.run(0x7ff5, 3 + (need != 0) as u32)?;
                c.restore(&[BX]);
                if need != 0 {
                    c.call(0x8018, 0x8000)?;
                    let need = c.byte(NEEDS_4C60.wrapping_add(c.reg(BX)));
                    c.run(0x8000, if need != 0 { 4 } else { 2 })?;
                    if need != 0 {
                        let (d, at) = (c.reg(DX), c.reg(DI));
                        c.set_word(NEAREST_DISTANCE, d);
                        c.set_word(NEAREST, at);
                    }
                }
            }
        }
        let at = c.reg(DI);
        c.run(0x800f, 2)?;
        place = Location(at.wrapping_add(Location::SIZE));
        c.set_reg(DI, place.offset());
    }
}

/// The location `nearest_needing_location` found ([470Eh]).
const NEAREST: u16 = 0x470e;
/// Its distance, FFFFh for none ([470Ch]).
const NEAREST_DISTANCE: u16 = 0x470c;

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

/// `loc_06c6f`: the troops' tick. [`TICK_PLACE`] = the location of record
/// 1018h when its byte 2 is 80h (else 0), `phase_30_check`; then each
/// troop in turn:
/// - one with bits 430h of word 12h waits: travelling, it is
///   looked at as below; else, while [`TICK_AGAIN`] is set, bits 30h of
///   byte 12h are cleared and it is looked at again from the start unless
///   bit 400h holds it;
/// - a small one (size below 14h) may merge (`merge_troop`, CF: done);
/// - one not yours or started (bits A0h of byte 3) is skipped;
/// - one travelling moves (`troop_move`) unless it is the troop
///   shown;
/// - else its job's handler (the table at CS:[`JOB_HANDLERS`], DI its
///   location; SI kept), then `troop_skills_decay`.
///
/// [`TICK_AGAIN`] = [`TICK_NEXT`] at the end (AL).
pub fn troops_tick(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6c6f)?;
    let mut c = Ctx::new(m);
    let w = c.word(0x101a);
    let at_place = w & 0xff == 0x80;
    c.run(start, if at_place { 11 } else { 6 })?;
    // Location (number - 1) × 1Ch + 100h, the number wrapping (0: 1CE4h).
    let place = if at_place { ((w >> 8) as u8).wrapping_sub(1) as u16 * Location::SIZE + Location::FIRST } else { 0 };
    c.set_reg(AX, if at_place { place } else { w });
    c.set_reg(SI, place);
    c.set_word(TICK_PLACE, place);
    c.call(0x6c46, 0x6c8a)?;
    c.run(0x6c8a, 2)?;
    c.set_byte(TICK_NEXT, 0);
    c.set_reg(SI, g::TROOPS);

    enum Step {
        Start,
        Act,
        Waiting,
        AtOwn,
        Next,
    }
    let mut step = Step::Start;
    loop {
        let troop = Troop(c.reg(SI));
        step = match step {
            Step::Start => {
                if troop.word_12(&mut c) & 0x430 != 0 {
                    c.run(0x6c92, 2)?;
                    Step::Waiting
                } else if troop.size(&mut c) >= 0x14 {
                    c.run(0x6c92, 4)?;
                    Step::Act
                } else {
                    c.run(0x6c92, 5)?;
                    c.call(0x6d19, 0x6ca2)?;
                    let merged = c.flag(CF);
                    c.run(0x6ca2, 1)?;
                    if merged { Step::Next } else { Step::Act }
                }
            }
            Step::Act => {
                let state = troop.state(&mut c);
                if state & (troop_state::NOT_A_TROOP | troop_state::STARTED) != 0 {
                    c.run(0x6ca4, 3)?;
                    c.set_reg8(r8::AL, state);
                    Step::Next
                } else if state & troop_state::TRAVELLING != 0 {
                    c.run(0x6ca4, 5)?;
                    c.set_reg8(r8::AL, state);
                    Step::AtOwn
                } else {
                    c.run(0x6ca4, 11)?;
                    let job = (state & 0x0f) as u16;
                    c.set_reg(AX, job);
                    c.set_reg(BX, job * 2);
                    c.save(&[SI]);
                    let place = troop.location(&mut c);
                    c.set_reg(DI, place);
                    let handler = c.code_word(JOB_HANDLERS + job * 2);
                    c.call(handler, 0x6cbf)?;
                    c.run(0x6cbf, 2)?;
                    c.restore(&[SI]);
                    c.call(0x6d7b, 0x6cc3)?;
                    Step::Next
                }
            }
            Step::Waiting => {
                let tick_again = c.byte(TICK_AGAIN);
                if troop.state(&mut c) & troop_state::TRAVELLING != 0 {
                    c.run(0x6cd3, 2)?;
                    Step::AtOwn
                } else if tick_again == 0 {
                    c.run(0x6cd3, 4)?;
                    Step::Next
                } else {
                    let held = troop.word_12(&mut c) & 0x400 != 0;
                    c.run(0x6cd3, if held { 7 } else { 8 })?;
                    let b = troop.byte_12(&mut c);
                    troop.set_byte_12(&mut c, b & !0x30);
                    if held { Step::Next } else { Step::Start }
                }
            }
            Step::AtOwn => {
                let id = troop.id(&mut c);
                if id == c.byte(SHOWN_TROOP) {
                    c.run(0x6ced, 3)?;
                } else {
                    c.run(0x6ced, 5)?;
                    c.save(&[SI]);
                    c.call(0x8308, 0x6cf9)?;
                    c.run(0x6cf9, 2)?;
                    c.restore(&[SI]);
                }
                c.set_reg8(r8::AL, id);
                Step::Next
            }
            Step::Next => {
                let next = troop.offset().wrapping_add(Troop::SIZE);
                let more = next < g::TROOPS_END;
                c.run(0x6cc3, if more { 3 } else { 6 })?;
                c.set_reg(SI, next);
                if !more {
                    let again = c.byte(TICK_NEXT);
                    c.set_reg8(r8::AL, again);
                    c.set_byte(TICK_AGAIN, again);
                    return c.ret();
                }
                Step::Start
            }
        };
    }
}

/// The location of record 1018h for this tick, or 0 ([473Ch]).
const TICK_PLACE: u16 = 0x473c;
/// Set during a tick for the next one ([4737h]).
const TICK_NEXT: u16 = 0x4737;
/// While set, waiting troops are looked at again ([0FAh]).
const TICK_AGAIN: u16 = 0xfa;
/// The job handlers: words in the code segment, by job.
const JOB_HANDLERS: u16 = 0x6c26;

/// `loc_07be0`: for the selected troop ([`SELECTED`] = SI): [47BAh] 0.
/// With its skills panel up ([46F4h], [46F5h]), a click on a need (the
/// rect 1936h, `loc_0d6fe`; which one by `find_range` over 4C7Ch) moves
/// one point from the spread bits at [`SPREAD_BITS`] to [`NEEDS_46FE`]
/// and redraws the panel (`loc_07d0c`). Else the full refresh:
/// `loc_07ba3`, `loc_031f6`, its location (`loc_02e98`); when it is
/// farther than [1176h] (`troop_map_distance`), [4Ch] FFh and the marked
/// particle [`PARTICLE_4752`] gets 1916h in words 0Dh and 0Fh;
/// `loc_09719` until CF clear; `loc_09efd`; [46F4h] 0, or 1 for an order
/// panel ([47A5h] 80h) with the bits spread (`spread_location_bits`) and
/// the skill bars (`loc_07e1e`); with bit 80h of [`MAP_MODE`]
/// `loc_09901`; the front buffer made active.
pub fn refresh_selected_troop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7be0)?;
    let mut c = Ctx::new(m);
    let selected = c.word(SELECTED);
    if c.reg(SI) != selected {
        c.run(start, 3)?;
        return c.ret();
    }
    let panel = c.byte(0x46f4) != 0;
    if panel && c.byte(0x46f5) != 0 {
        c.run(start, 12)?;
        c.set_word(0x47ba, 0);
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
        let k = c.reg(DI);
        let from = c.byte(SPREAD_BITS.wrapping_add(k)).wrapping_sub(1);
        c.set_byte(SPREAD_BITS.wrapping_add(k), from);
        let to = c.byte(NEEDS_46FE.wrapping_add(k)).wrapping_add(1);
        c.set_byte(NEEDS_46FE.wrapping_add(k), to);
        c.call(0x7d0c, 0x7ee1)?;
        c.run(0x7ee1, 1)?;
        return c.ret();
    }
    c.run(start, if panel { 9 } else { 7 })?;
    c.set_word(0x47ba, 0);
    c.set_reg(SI, selected);
    c.call(0x7ba3, 0x7c05)?;
    c.run(0x7c05, 1)?;
    c.call(0x31f6, 0x7c08)?;
    c.run(0x7c08, 2)?;
    let place = Troop(c.reg(SI)).location(&mut c);
    c.set_reg(DI, place);
    c.call(0x2e98, 0x7c0e)?;
    c.run(0x7c0e, 1)?;
    c.call(0x7c63, 0x7c11)?;
    if c.reg(AX) <= c.word(0x1176) {
        c.run(0x7c11, 3)?;
    } else {
        let marked = c.word(PARTICLE_4752);
        c.run(0x7c11, if marked != 0 { 10 } else { 7 })?;
        c.set_byte(0x4c, 0xff);
        c.set_reg(DI, marked);
        if marked != 0 {
            c.set_reg(AX, 0x1916);
            c.set_word(marked.wrapping_add(0x0d), 0x1916);
            c.set_word(marked.wrapping_add(0x0f), 0x1916);
        }
    }
    loop {
        c.call(0x9719, 0x7c30)?;
        let again = c.flag(CF);
        c.run(0x7c30, if again { 3 } else { 4 })?;
        let selected = c.word(SELECTED);
        c.set_reg(SI, selected);
        if !again {
            break;
        }
    }
    c.save(&[SI]);
    c.call(0x9efd, 0x7c3a)?;
    let orders = c.byte(0x47a5) == 0x80;
    c.run(0x7c3a, if orders { 6 } else { 4 })?;
    c.restore(&[SI]);
    c.set_byte(0x46f4, orders as u8);
    if orders {
        c.call(0x7efb, 0x7c4e)?;
        c.run(0x7c4e, 2)?;
        c.save(&[SI]);
        c.call(0x7e1e, 0x7c52)?;
        c.run(0x7c52, 2)?;
        c.restore(&[SI]);
    }
    let zoomed = c.byte(MAP_MODE) & 0x80 != 0;
    c.run(0x7c56, 3)?;
    if zoomed {
        c.call(0x9901, 0x7c60)?;
        c.run(0x7c60, 1)?;
    }
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `loc_08604`: the troop SI's step toward its location. The x distance
/// counts in units of the scale for its y (SS:[4880h + |2·y|], BP); the
/// step goes along the larger of that and the y distance. Close (below
/// 7) it stops (DX = BX = 0). Else DX is the x step (± the scale) and BX
/// the y step (±1); the step along the smaller axis comes only on every
/// other tick of the seed at [`SEED_0`] (rotated left, its carry), and
/// none where that distance halves to 0. AX and DI as its distances (the
/// larger halved); SI kept. Bails at entry for a zero scale (the
/// original's `div` faults).
pub fn troop_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8604)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let place = troop.location(&mut c);
    let (lx, ly) = (c.word(place.wrapping_add(2)), c.word(place.wrapping_add(4)));
    let (tx, ty) = (c.word(troop.offset().wrapping_add(6)), c.word(troop.offset().wrapping_add(8)));
    let neg = |v: u16| v & 0x8000 != 0;
    let abs = |v: u16| if neg(v) { v.wrapping_neg() } else { v };
    let row = ty.wrapping_shl(1);
    let scale = c.word_in(SS, abs(row).wrapping_add(0x4880));
    if scale == 0 {
        return Err(Bail);
    }
    let dy = ly.wrapping_sub(ty);
    let dx = lx.wrapping_sub(tx);
    let y_dir: u16 = if neg(dy) { 0xffff } else { 1 };
    let across = abs(dx) / scale;
    let down = abs(dy);
    let x_step = if neg(dx) { scale.wrapping_neg() } else { scale };
    let seed = c.word(SEED_0);
    let carry = neg(seed);
    let mut cost = 22 + neg(row) as u32 + 2 * neg(dy) as u32 + neg(dx) as u32;
    // The results: AX, DI, DX, BX, BP, and whether the seed turns.
    let (mut ax, mut di, mut out_dx, mut bx, mut bp, mut turns) = (across, down, dx, y_dir, scale, false);
    if across >= down {
        if across < 7 {
            cost += 6;
            (out_dx, bx) = (0, 0);
        } else {
            cost += 8 + (down == 0) as u32 + neg(dx) as u32;
            if down == 0 {
                bx = 0;
            }
            bp = x_step;
            ax = across >> 1;
            if ax == 0 {
                cost += 2;
            } else {
                cost += 5 + !carry as u32;
                out_dx = x_step;
                turns = true;
                if !carry {
                    bx = 0;
                }
            }
        }
    } else if down < 7 {
        cost += 6;
        (out_dx, bx) = (0, 0);
    } else if across == 0 {
        cost += 6;
    } else {
        cost += 8 + neg(dx) as u32;
        bp = x_step;
        di = down >> 1;
        if di == 0 {
            cost += 2;
        } else {
            cost += 3 + if carry { 2 } else { 3 };
            out_dx = if carry { x_step } else { 0 };
            turns = true;
        }
    }
    c.run(start, cost)?;
    c.save(&[SI]);
    if turns {
        c.set_word(SEED_0, seed.rotate_left(1));
    }
    c.set_reg(AX, ax);
    c.set_reg(DI, di);
    c.set_reg(DX, out_dx);
    c.set_reg(BX, bx);
    c.set_reg(BP, bp);
    c.restore(&[SI]);
    c.ret()
}

/// `loc_06770`: BP = the sprite list for the troop SI (see
/// [`sprite_list_of`]); a travelling troop's goes on into
/// `troop_facing_sprite`.
pub fn troop_sprite_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6770)?;
    let mut c = Ctx::new(m);
    let troop = Troop(c.reg(SI));
    let list = sprite_list_of(&mut c, troop);
    c.run(start, list.cost)?;
    c.set_reg(AX, list.ax);
    c.set_reg(BP, list.bp);
    if list.facing {
        return troop_facing_sprite(c.m, 0x6827);
    }
    c.ret()
}

/// What `loc_06770` picks: BP the sprite list (words in the stack
/// segment's tables), AX as it leaves it, its instructions, and whether
/// it goes on into `troop_facing_sprite` (a travelling troop).
struct SpriteList {
    bp: u16,
    ax: u16,
    cost: u32,
    facing: bool,
}

/// The sprite list of `loc_06770`:
/// - hidden (bit 10h of byte 10h): none (0);
/// - not a troop of yours and without bit 80h of byte 10h: 181Fh where
///   its location has bit 10h of byte 0Ah (read in the stack segment),
///   else none;
/// - travelling: by its facing (`troop_facing_sprite`);
/// - busy (bits 30h of byte 3): by its job from [`BUSY_SPRITES`]; for
///   job 0 by bits C0h of byte 19h (1813h for 80h, 1817h for 40h, 181Bh
///   for both);
/// - with no job and bits C0h of byte 19h: 17BCh (80h), 17C9h (40h),
///   1806h (both);
/// - else by its job from [`JOB_SPRITES`] (1732h becomes 16AAh at a
///   location with bit 2 of byte 0Ah), and the lists 1774h and 1732h by
///   its heading (slot - 1, its low 3 bits: 3, 4, 5 and up are 0Ah, 14h,
///   1Eh further).
fn sprite_list_of(c: &mut Ctx, troop: Troop) -> SpriteList {
    let mut ax = c.reg(AX);
    let mut bp = c.reg(BP);
    let done = |bp: u16, ax: u16, cost: u32| SpriteList { bp, ax, cost: cost + 1, facing: false };
    let b10 = troop.byte_10(c);
    if b10 & 0x10 != 0 {
        return done(0, ax, 3);
    }
    let state = troop.state(c);
    ax = ax & 0xff00 | state as u16;
    let mut cost = 5;
    if b10 & 0x80 == 0 {
        cost += 2;
        if state & troop_state::NOT_A_TROOP != 0 {
            cost += 4;
            let place = troop.location(c);
            let flags = c.byte_in(SS, place.wrapping_add(0x0a));
            return if flags & 0x10 != 0 { done(0x181f, ax, cost) } else { done(0, ax, cost + 1) };
        }
    }
    cost += 2;
    if state & troop_state::TRAVELLING != 0 {
        return SpriteList { bp, ax, cost: cost + 1, facing: true };
    }
    cost += 2;
    let job = (state & 0x0f) as u16;
    if state & 0x30 != 0 {
        cost += 6;
        bp = c.word_in(SS, BUSY_SPRITES + job * 2);
        if job != 0 {
            return done(bp, job, cost);
        }
        cost += 3;
        let bits = (troop.byte_19(c) & 0xc0) as u16;
        if bits == 0 {
            return done(bp, bits, cost);
        }
        cost += 3;
        if bits == 0x80 {
            return done(0x1813, bits, cost);
        }
        cost += 3;
        if bits == 0x40 {
            return done(0x1817, bits, cost);
        }
        return done(0x181b, bits, cost + 1);
    }
    cost += 2;
    ax = job;
    if job == 0 {
        cost += 2;
        let bits = (troop.byte_19(c) & 0xc0) as u16;
        if bits != 0 {
            cost += 5;
            if bits == 0x80 {
                return done(0x17bc, bits, cost);
            }
            cost += 3;
            if bits == 0x40 {
                return done(0x17c9, bits, cost);
            }
            return done(0x1806, bits, cost + 1);
        }
    }
    cost += 5;
    bp = c.word_in(SS, JOB_SPRITES + job * 2);
    if bp != 0x1774 {
        cost += 2;
        if bp != 0x1732 {
            return done(bp, ax, cost);
        }
        cost += 4;
        let place = troop.location(c);
        if c.byte_in(SS, place.wrapping_add(0x0a)) & 2 != 0 {
            return done(0x16aa, ax, cost);
        }
        cost += 1;
    }
    cost += 5;
    let heading = c.byte(troop.offset().wrapping_add(2)).wrapping_sub(1) & 7;
    let further = match heading {
        0..=2 => 0,
        3 => 1,
        4 => 2,
        _ => 3,
    };
    cost += 3 * further.min(2) + (heading > 4) as u32;
    done(bp.wrapping_add(0x0a * further as u16), heading as u16, cost)
}

/// The sprite lists of busy troops by job (words, stack segment).
const BUSY_SPRITES: u16 = 0x16b6;
/// The sprite lists by job (words, stack segment).
const JOB_SPRITES: u16 = 0x179c;

/// `loc_05ce4`: a click on the map's troops: the sprite list dropped
/// (`loc_05beb`), the map resource opened (`loc_0c13b`). Unless a second
/// panel is up ([0DBE2h]) or the first one ([0DBE0h], DI) is another than
/// the troop panel 18DFh: a click inside the troop panel (`loc_0d6fe`,
/// CF) redraws it (`redraw_panel_18df`); else the troop of the particle
/// under the cursor (`particle_at`, CF): the one shown already
/// ([`PANEL_DIRTY`]) is redrawn the same way, another is drawn first
/// (`redraw_7b36`; SI kept), then its caption and panel
/// (`troop_caption`).
pub fn map_troop_click(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5ce4)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x5beb, 0x5ce7)?;
    c.run(0x5ce7, 1)?;
    c.call(0xc13b, 0x5cea)?;
    if c.word(0xdbe2) != 0 {
        c.run(0x5cea, 3)?;
        return c.ret();
    }
    let panel = c.word(0xdbe0);
    if panel != 0 {
        c.run(0x5cea, 8)?;
        c.set_reg(DI, panel);
        if panel != 0x18df {
            return c.ret();
        }
        c.call(0xd6fe, 0x5d02)?;
        let inside = c.flag(CF);
        c.run(0x5d02, 2)?;
        if inside {
            return redraw_panel_18df(c.m, 0x79de);
        }
    } else {
        c.run(0x5cea, 6)?;
        c.set_reg(DI, 0);
    }
    c.call(0x6946, 0x5d07)?;
    if !c.flag(CF) {
        c.run(0x5d07, 2)?;
        return c.ret();
    }
    let troop = c.word(c.reg(SI).wrapping_add(0x0a));
    let same = troop == c.word(PANEL_DIRTY);
    c.run(0x5d07, if same { 5 } else { 6 })?;
    c.set_reg(SI, troop);
    if same {
        return redraw_panel_18df(c.m, 0x79de);
    }
    c.save(&[SI]);
    c.call(0x7b36, 0x5d16)?;
    c.run(0x5d16, 2)?;
    c.restore(&[SI]);
    troop_caption(c.m, 0x78bc)
}

/// `loc_078bc`: for the troop SI within reach of the map position
/// ([`troop_map_distance`] below [`REACH`]) and shown (`troop_particle`,
/// ZF; DI its particle): on the screen (`loc_0c08e`), it becomes the
/// panel's troop ([`PANEL_DIRTY`]), the panel box ([`PANEL`], 64h wide)
/// is placed at its particle (`loc_05f25`; SI kept), [1955h] = its
/// number, then its panel ([`troop_panel`]). Callers read the flags of
/// its exits.
pub fn troop_caption(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x78bc)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let place = Troop(c.reg(SI)).location(&mut c);
    c.set_reg(DI, place);
    c.call(0x7c63, 0x78c2)?;
    let (distance, reach) = (c.reg(AX), c.word(REACH));
    c.run(0x78c2, 3)?;
    if distance >= reach {
        return c.ret();
    }
    c.call(0x6917, 0x78cb)?;
    let shown = c.flag(ZF);
    c.run(0x78cb, 2)?;
    if !shown {
        return c.ret();
    }
    c.call(0xc08e, 0x78d0)?;
    c.run(0x78d0, 7)?;
    let troop = Troop(c.reg(SI));
    c.set_word(PANEL_DIRTY, troop.offset());
    c.save(&[SI]);
    c.set_reg(SI, PANEL);
    let particle = c.reg(DI);
    let (x, y) = (c.word(particle), c.word(particle.wrapping_add(2)));
    c.set_reg(BX, y);
    c.set_reg(DX, x);
    c.set_reg(CX, 0x64);
    c.call(0x5f25, 0x78e3)?;
    c.run(0x78e3, 3)?;
    c.restore(&[SI]);
    let id = troop.id(&mut c);
    c.set_reg8(r8::AL, id);
    c.set_byte(0x1955, id);
    troop_panel_at(&mut c)
}

/// How far from the map position a troop's caption shows ([1176h]).
const REACH: u16 = 0x1176;
/// The troop panel's box: x, y words, the bars' sprite at +6, its colour
/// at +9.
const PANEL: u16 = 0x18df;

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
    let shown = c.flag(ZF);
    c.run(0x78ec, 2)?;
    if !shown {
        return redraw_panel_18df(c.m, 0x79de);
    }
    c.call(0xc08e, 0x78f4)?;
    c.run(0x78f4, 1)?;
    c.call(0x31f6, 0x78f7)?;
    c.run(0x78f7, 3)?;
    let bottom = c.word(0x11f3).wrapping_add(0x0c);
    c.set_word(0x11f3, bottom);
    c.set_reg(SI, PANEL);
    c.call(0x7b1b, 0x7902)?;
    c.run(0x7902, 1)?;
    c.call(0xd075, 0x7905)?;
    // The title: 3Ah, 3Bh for a troop moving (bit 40h of [30h]).
    let moving = c.byte(DESCRIBED) & 0x40 != 0;
    c.run(0x7905, 10 + moving as u32)?;
    c.set_reg8(r8::CL, 0x9a);
    let colour = c.byte(PANEL + 9);
    c.set_reg8(r8::CH, colour);
    let (px, py) = (c.word(PANEL), c.word(PANEL + 2));
    c.set_reg(DX, px.wrapping_add(0x0c));
    c.set_reg(BX, py.wrapping_add(4));
    c.set_reg(AX, 0x3a + moving as u16);
    c.call(0x8865, 0x7927)?;
    // The place's name, 9 lower.
    c.run(0x7927, 5)?;
    c.set_reg8(r8::CL, 0x96);
    let (dx, bx) = (c.reg(DX).wrapping_sub(8), c.reg(BX).wrapping_add(9));
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    let place = c.word(0x2c);
    c.set_reg(DI, place);
    c.call(0x62a6, 0x7936)?;
    // Its job, 0Ah lower: 41h, 42h for the specialised (20h..22h).
    let kind = c.byte(DESCRIBED);
    if kind & 0x20 != 0 {
        c.run(0x7936, if kind != 0x22 { 8 } else { 10 })?;
        c.set_reg8(r8::CL, 0x9a);
        let bx = c.reg(BX).wrapping_add(0x0a);
        c.set_reg(BX, bx);
        c.set_reg(AX, 0x41 + (kind == 0x22) as u16);
        return status_79b6(c);
    }
    c.run(0x7936, 7)?;
    c.set_reg8(r8::CL, 0x9a);
    let bx = c.reg(BX).wrapping_add(0x0a);
    c.set_reg(BX, bx);
    c.set_reg(AX, 0x3c);
    c.call(0x8865, 0x7952)?;
    if c.byte(DESCRIBED) == 2 {
        c.run(0x7952, 3)?;
        let bx = c.reg(BX).wrapping_add(0x0f);
        c.set_reg(BX, bx);
        return skills_79bc(c);
    }
    // The activity's phrase (by [2Fh]), 0Fh lower.
    c.run(0x7952, 10)?;
    let bx = c.reg(BX).wrapping_add(0x0f);
    c.set_reg(BX, bx);
    let activity = c.byte(0x2f);
    let at = 0x11f7 + ((activity >> 1) & 6) as u16;
    c.set_reg(SI, at);
    let phrase = c.word(at);
    c.set_reg(AX, phrase);
    c.call(0xd194, 0x796e)?;
    // Then a status, 0Ah lower, the first that holds: none while moving;
    // 3Fh (bit 200h of [32h]); 40h (without 100h there, or with bits 30h
    // of [34h]); 3Dh ([30h] 0); 43h ([2Fh] 1); 3Eh ([30h] 6).
    let (kind, w32, w34, activity) = (c.byte(DESCRIBED), c.word(0x32), c.word(0x34), c.byte(0x2f));
    let tests: [(u16, bool); 6] = [
        (0x3f, w32 & 0x200 != 0),
        (0x40, w32 & 0x100 == 0),
        (0x40, w34 & 0x30 != 0),
        (0x3d, kind == 0),
        (0x43, activity == 1),
        (0x3e, kind == 6),
    ];
    // Each test costs 3 (2 for the one after 40h's first), and the one
    // that holds 1 more for its jump.
    let costs = [3u32, 3, 2, 3, 3, 3];
    let mut cost = 3;
    let mut phrase = None;
    if kind & 0x40 == 0 {
        for (k, &(p, holds)) in tests.iter().enumerate() {
            cost += costs[k];
            if k != 2 {
                c.set_reg(AX, p);
            }
            if holds {
                phrase = Some(p);
                break;
            }
        }
        if phrase.is_none() {
            cost += 1;
        }
    }
    c.run(0x796e, cost)?;
    let bx = c.reg(BX).wrapping_add(0x0a);
    c.set_reg(BX, bx);
    match phrase {
        Some(_) => status_79b6(c),
        None => skills_79bc(c),
    }
}

/// The described troop's kind ([30h], by `loc_031f6`): bit 40h moving,
/// 20h..22h the specialised, 2, 6, 0 as the job.
const DESCRIBED: u16 = 0x30;

/// From 79B6h: the status phrase AX, then 11h lower the skills.
fn status_79b6(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x79b6, 1)?;
    c.call(0x8865, 0x79b9)?;
    c.run(0x79b9, 1)?;
    let bx = c.reg(BX).wrapping_add(0x11);
    c.set_reg(BX, bx);
    skills_79bc(c)
}

/// From 79BCh: the skills heading 6Eh 4 lower, 8 lower the panel troop's
/// skill bits spread (`spread_location_bits`) and their bars (`loc_07e3d`
/// over [`SPREAD_BITS`], BP the bars' sprite); back to the front buffer.
fn skills_79bc(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x79bc, 4)?;
    let bx = c.reg(BX).wrapping_add(4);
    c.set_reg(BX, bx);
    c.set_reg(AX, 0x6e);
    c.set_reg8(r8::CL, 0x96);
    c.call(0xd194, 0x79c7)?;
    c.run(0x79c7, 3)?;
    let bx = c.reg(BX).wrapping_add(8);
    c.set_reg(BX, bx);
    let troop = c.word(PANEL_DIRTY);
    c.set_reg(SI, troop);
    c.call(0x7efb, 0x79d1)?;
    c.run(0x79d1, 3)?;
    c.set_reg(SI, SPREAD_BITS);
    let bars = c.word(PANEL + 6);
    c.set_reg(BP, bars);
    c.call(0x7e3d, 0x79db)?;
    c.run(0x79db, 1)?;
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `loc_08308`: the troop SI moves: 4 steps (8 when fast, bit 40h of byte
/// 19h), see [`steps_8313`].
pub fn troop_move(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8308)?;
    let mut c = Ctx::new(m);
    let fast = Troop(c.reg(SI)).byte_19(&mut c) & 0x40 != 0;
    c.run(start, if fast { 4 } else { 3 })?;
    c.set_reg(CX, if fast { 8 } else { 4 });
    steps_8313(&mut c)
}

/// `loc_08313`: the troop SI's CX steps (`troop_step`, CX kept) added to
/// its position (words 6, 8); a still step means it has arrived
/// ([`troop_arrived_at`]). Then its sprite: on the map
/// (`troop_screen_place`, no CF) updated where it is shown
/// (`troop_particle`, ZF: `loc_0c653`) or placed (`troop_facing_sprite`,
/// `loc_0c5cf`, the rect marked dirty: `loc_0c6ad` with SI the
/// particle); off it removed where shown (`particles_remove_particle`).
fn steps_8313(c: &mut Ctx) -> Result<(), Bail> {
    loop {
        c.run(0x8313, 2)?;
        c.save(&[CX]);
        c.call(0x8604, 0x8317)?;
        let (dy, dx) = (c.reg(BX), c.reg(DX));
        let still = dy | dx == 0;
        c.run(0x8317, if still { 4 } else { 7 })?;
        c.restore(&[CX]);
        c.set_reg(AX, dy | dx);
        if still {
            return troop_arrived_at(c);
        }
        let troop = Troop(c.reg(SI));
        let at = troop.offset();
        let x = c.word(at.wrapping_add(6)).wrapping_add(dx);
        c.set_word(at.wrapping_add(6), x);
        let y = c.word(at.wrapping_add(8)).wrapping_add(dy);
        c.set_word(at.wrapping_add(8), y);
        let left = c.reg(CX).wrapping_sub(1);
        c.set_reg(CX, left);
        if left == 0 {
            break;
        }
    }
    c.run(0x8326, 1)?;
    c.call(0x686e, 0x8329)?;
    let off_map = c.flag(CF);
    c.run(0x8329, 2)?;
    if off_map {
        c.call(0x6917, 0x8336)?;
        let shown = c.flag(ZF);
        c.run(0x8336, 2)?;
        if shown {
            return super::saves::particles_remove_particle(c.m, 0xc58a);
        }
        return c.ret();
    }
    c.call(0x6917, 0x832e)?;
    let shown = c.flag(ZF);
    c.run(0x832e, 2)?;
    if shown {
        return c.jump(0xc653);
    }
    c.call(0x6827, 0x833f)?;
    c.run(0x833f, 1)?;
    c.call(0xc5cf, 0x8342)?;
    c.run(0x8342, 2)?;
    let particle = c.reg(DI);
    c.set_reg(SI, particle);
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
    let troop = Troop(c.reg(SI));
    let target = troop.location(c);
    if troop != Troop::number(3) {
        c.run(0x8357, 2)?;
    } else {
        // Troop 3, followed: its note shifted when it reaches the place.
        let followed = c.word(0x11d3);
        c.run(0x8357, if followed == target { 6 } else { 5 })?;
        c.set_reg(AX, followed);
        if followed == target {
            c.call(0x8347, 0x8368)?;
        }
    }
    let troop = Troop(c.reg(SI));
    let order = troop.state(c);
    let place = Location(troop.location(c));
    let (x, y) = (c.word(place.offset().wrapping_add(2)), c.word(place.offset().wrapping_add(4)));
    let secret = place.byte_0a(c) & 2 == 0;
    c.run(0x8368, if secret { 10 } else { 9 })?;
    let job = order & 0x0f;
    c.set_reg8(r8::AL, job);
    c.set_reg(DI, place.offset());
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    c.set_word(troop.offset().wrapping_add(6), x);
    c.set_word(troop.offset().wrapping_add(8), y);
    let mut found = true;
    if secret {
        c.call(0x5d36, 0x8385)?;
        found = !c.flag(CF);
        c.run(0x8385, 1)?;
    }
    if found {
        c.run(0x8387, 2)?;
        c.save(&[AX]);
        c.call(0x83bc, 0x838b)?;
        let ax = c.word_in(SS, c.reg(SP));
        let job = ax as u8;
        let (troop, place) = (Troop(c.reg(SI)), Location(c.reg(DI)));
        if job == 5 {
            // Harvesting: a place newly found is shown.
            let flags = place.byte_0a(c);
            let new = flags & 0x80 != 0;
            c.run(0x838b, if new { 7 } else { 5 })?;
            c.restore(&[AX]);
            if new {
                place.set_byte_0a(c, flags & 0x7f);
                return super::map::note_shown_location(c.m, 0x5d44);
            }
            return tell_troops_83fd(c);
        }
        let own = troop.id(c);
        let meets = own == place.first_troop(c);
        c.run(0x838b, if meets { 7 } else { 6 })?;
        c.restore(&[AX]);
        c.set_reg8(r8::AL, own);
        if meets {
            return c.jump(0x7429);
        }
        return tell_troops_83fd(c);
    }
    // Not found: orders 5 and 6 drop their figure (`set_troop_activity`;
    // DI kept, AL 0); a move (job bits 3) goes on to its next place.
    let job = c.reg8(r8::AL);
    let drop = (5..=6).contains(&job);
    c.run(0x83a7, 2 + if job < 5 { 0 } else { 2 } + if drop { 2 } else { 0 })?;
    if drop {
        c.save(&[DI]);
        c.call(0x6ac5, 0x83b3)?;
        c.run(0x83b3, 2)?;
        c.restore(&[DI]);
        c.set_reg8(r8::AL, 0);
    }
    c.run(0x83b6, 3)?;
    let moving = c.reg8(r8::AL) & 3;
    c.set_reg8(r8::AL, moving);
    if moving == 3 {
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

/// `loc_0841f`: the troop SI goes on to its next place (word 0Ch). At it
/// already: no longer moving (job bits 3 cleared), `update_troop_83bc`.
/// Else the needs of the old place worked out (`skill_needs_46fe`; SI
/// kept) and the skill it carries (word 0Eh: the need's index, and its
/// bit in the high byte) handed over where still needed: that need of the
/// old place one down and the bit into its byte 19h; word 0Eh keeps the
/// index only then. Its location the next place, then its sprite: where
/// shown (`troop_particle`, ZF) removed, and unless travelling
/// or at one `loc_05ed0` refuses, placed again as in
/// `troop_location_dirty`.
fn next_place_841f(c: &mut Ctx) -> Result<(), Bail> {
    let troop = Troop(c.reg(SI));
    let place = troop.location(c);
    let next = c.word(troop.offset().wrapping_add(0x0c));
    c.run(0x841f, 5)?;
    c.set_reg(DI, place);
    if place == next {
        let s = troop.state(c);
        troop.set_state(c, s & !3);
        return update_troop_83bc(c.m, 0x83bc);
    }
    c.save(&[SI]);
    c.call(0x7f27, 0x842b)?;
    let troop = Troop(c.word_in(SS, c.reg(SP)));
    let carried = c.word(troop.offset().wrapping_add(0x0e));
    let need = carried & 0xff;
    let needed = c.byte(NEEDS_46FE.wrapping_add(need)) != 0;
    c.run(0x842b, if needed { 13 } else { 10 })?;
    c.restore(&[SI]);
    c.set_reg(BX, need);
    let mut carried = carried;
    if needed {
        let at = need.wrapping_add(c.reg(DI)).wrapping_add(0x14);
        let v = c.byte(at).wrapping_sub(1);
        c.set_byte(at, v);
        let bits = troop.byte_19(c) | (carried >> 8) as u8;
        troop.set_byte_19(c, bits);
        carried = need;
    }
    c.set_reg(CX, carried);
    c.set_word(troop.offset().wrapping_add(0x0e), carried);
    let next = c.word(troop.offset().wrapping_add(0x0c));
    c.set_reg(DI, next);
    troop.set_location(c, next);
    c.run(0x8461, 1)?;
    c.call(0x6917, 0x8464)?;
    if !c.flag(ZF) {
        c.run(0x8464, 2)?;
        return c.ret();
    }
    c.run(0x8464, 3)?;
    c.save(&[SI]);
    c.call(0xc58a, 0x846b)?;
    let troop = Troop(c.word_in(SS, c.reg(SP)));
    let travelling = troop.state(c) & troop_state::TRAVELLING != 0;
    c.run(0x846b, if travelling { 4 } else { 6 })?;
    // `pop si; push si`: the troop again, and kept.
    c.restore(&[SI]);
    c.save(&[SI]);
    if !travelling {
        let place = troop.location(c);
        c.set_reg(DI, place);
        c.call(0x5ed0, 0x8479)?;
        if !c.flag(ZF) {
            c.run(0x8479, 3)?;
            return figure_gone_848b(c);
        }
        c.run(0x8479, 1)?;
    }
    c.run(0x847b, 2)?;
    let id = Troop(c.reg(SI)).id(c);
    c.set_reg8(r8::AL, id);
    c.call(0x6757, 0x8480)?;
    if c.flag(CF) {
        c.run(0x8480, 3)?;
        return figure_gone_848b(c);
    }
    c.run(0x8480, 4)?;
    let list = c.reg(DI);
    c.set_reg(SI, list);
    c.save(&[DI]);
    c.call(0xc6ad, 0x8488)?;
    c.run(0x8488, 3)?;
    c.restore(&[DI]);
    c.restore(&[SI]);
    c.ret()
}

/// `loc_0848b` (counted by the caller with its jump): DI 0 (`xor`: its
/// flags returned), SI back.
fn figure_gone_848b(c: &mut Ctx) -> Result<(), Bail> {
    c.xor(W, 0, 0);
    c.set_reg(DI, 0);
    c.run(0x8489, 2)?;
    c.restore(&[SI]);
    c.ret()
}

/// `loc_084a6`: the troop SI sets out for the place DI. Troop 3 stops
/// here while it is followed (`loc_0848f`, ZF). Already travelling it
/// only takes the new target (no longer moving, job bits 3
/// cleared) and is shown (`drop_troop_particle`). Else it leaves: its
/// location's troops redo their jobs (`for_troop_location`), it is taken
/// out of the chain (`unlink_troop`), and a harvester (byte 3 exactly 6)
/// at a taken place (bit 2 of byte 0Ah) leaves it to the enemy when it
/// was the last there (`for_each_troop_5082`: CX, DX; `loc_074b6`); one
/// going to a place of byte 8 below 28h loses motivation
/// (`decrease_troop_motivation` 3), unless that place is taken: then it
/// stays. Then on its way: the target its location (DI the old one), at
/// travelling, slot 0, `lower_by_bits`, `drop_troop_particle`,
/// and unless it waits (bit 10h of byte 10h) seven steps
/// ([`steps_8313`]). Callers read the flags: those of its last test.
pub fn troop_set_out(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x84a6)?;
    let mut c = Ctx::new(m);
    if Troop(c.reg(SI)) == Troop::number(3) {
        c.run(start, 3)?;
        c.call(0x848f, 0x84af)?;
        if c.flag(ZF) {
            c.run(0x84af, 2)?;
            return c.ret();
        }
        c.run(0x84af, 1)?;
    } else {
        c.run(start, 2)?;
    }
    let troop = Troop(c.reg(SI));
    let state = troop.state(&mut c);
    if state & troop_state::TRAVELLING != 0 {
        let moving = state & 3 == 3;
        c.run(0x84b2, 8 + moving as u32)?;
        let target = c.reg(DI);
        troop.set_location(&mut c, target);
        c.set_reg8(r8::AL, state & 3);
        if moving {
            troop.set_state(&mut c, state & !3);
        }
        return drop_troop_particle(c.m, 0x8461);
    }
    c.run(0x84b2, 3)?;
    c.call(0x6ebf, 0x84cd)?;
    c.run(0x84cd, 2)?;
    c.save(&[DI]);
    c.call(0x858c, 0x84d1)?;
    let troop = Troop(c.reg(SI));
    let harvester = troop.state(&mut c) == 6;
    c.run(0x84d1, 3)?;
    c.restore(&[DI]);
    if harvester {
        let home = Location(troop.location(&mut c));
        let taken = home.byte_0a(&mut c) & 2 != 0;
        c.run(0x84d8, if taken { 5 } else { 4 })?;
        c.save(&[DI]);
        c.set_reg(DI, home.offset());
        if taken {
            c.call(0x5098, 0x84e5)?;
            let (count, dx) = (c.reg(CX), c.reg(DX));
            let last = count != 0 && dx.wrapping_sub(1) as i16 <= 0;
            c.run(0x84e5, if count == 0 { 1 } else if last { 5 } else { 3 })?;
            if count != 0 {
                c.set_reg(DX, dx.wrapping_sub(1));
            }
            if last {
                c.save(&[SI]);
                c.call(0x74b6, 0x84ee)?;
                c.run(0x84ee, 1)?;
                c.restore(&[SI]);
            }
        }
        let target = Location(c.word_in(SS, c.reg(SP)));
        let kind = target.byte_08(&mut c);
        let flags = target.byte_0a(&mut c);
        let stays = kind < 0x28 && flags & 2 != 0;
        c.run(0x84ef, if kind >= 0x28 { 3 } else if stays { 6 } else { 7 })?;
        c.restore(&[DI]);
        if stays {
            return c.ret();
        }
        if kind < 0x28 {
            c.set_reg8(r8::AL, 3);
            c.call(0x6f93, 0x8501)?;
        }
    }
    c.run(0x8501, 4)?;
    let troop = Troop(c.reg(SI));
    let target = c.reg(DI);
    let old = troop.location(&mut c);
    troop.set_location(&mut c, target);
    c.set_reg(DI, old);
    let s = troop.state(&mut c);
    troop.set_state(&mut c, s | troop_state::TRAVELLING);
    c.set_byte(troop.offset().wrapping_add(2), 0);
    c.call(0x7f75, 0x850f)?;
    c.run(0x850f, 1)?;
    c.call(0x8461, 0x8512)?;
    let b10 = Troop(c.reg(SI)).byte_10(&mut c);
    if b10 & 0x10 != 0 {
        c.run(0x8512, 3)?;
        return c.ret();
    }
    c.run(0x8512, 4)?;
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
/// base DX of 7Fh (a troop travelling) or 61h. One more at the
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
    let travelling = state & troop_state::TRAVELLING != 0;
    let base: u16 = if travelling { 0x7f } else { 0x61 };
    let apart = troop.byte_10(&mut c) & 0x80 != 0;
    let mut cost = 9 + travelling as u32 + if apart { 0 } else { 3 };
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
    let shown = c.byte(SHOWN_TROOP);
    if id != shown {
        c.cmp(B, id as u16, shown as u16);
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
        6 => {
            c.ret()
        }
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
    let state = troop.state(&mut c);
    let counted = state == 4;
    c.run(start, if counted { 6 } else { 3 })?;
    // Exact: the flags of its last instruction are returned.
    if !counted {
        c.cmp(B, state as u16, 4);
        return c.ret();
    }
    let cx = c.reg(CX).wrapping_add(1);
    c.set_reg(CX, cx);
    // `add dl, skill 1; adc dh, 0`
    let (dx, skill) = (c.reg(DX), troop.skill(&mut c, 1));
    let dl = c.add(B, dx & 0xff, skill as u16);
    let dh = c.adc(B, dx >> 8, 0);
    c.set_reg(DX, dh << 8 | dl);
    c.ret()
}
