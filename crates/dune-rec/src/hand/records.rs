//! Records: locations (sietches, palaces) and the objects on the map.
//!
//! A location record's byte at +8 is its kind: below 20h a plain place,
//! 20h, 21h..27h, 28h..2Fh and 30h and up the kinds that
//! `get_location_type_string_offset` and `calc_SAL_index` tell apart.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BX, CX, DI, DX, SI},
    rec::Bail,
};

/// Location kind thresholds (byte +8 of a location record).
const KIND_THRESHOLDS: [u8; 4] = [0x20, 0x21, 0x28, 0x30];

/// `calc_SAL_index` (5E4F): AX += the number of [`KIND_THRESHOLDS`] that
/// the kind of the location at SI reaches.
///
/// Exit: CL = the kind; flags from the last `cmp`, or the last `inc` when
/// all are reached.
pub fn calc_sal_index(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5e4f)?;
    let mut c = Ctx::new(m);
    c.check(start, 14)?;
    let si = c.reg(SI);
    let kind = c.byte(si.wrapping_add(8));
    c.set_reg8(r8::CL, kind);
    let mut ax = c.reg(AX);
    let mut n = 2; // mov cl, [si+8]; ret
    for t in KIND_THRESHOLDS {
        c.cmp(B, kind as u16, t as u16);
        n += 2;
        if kind < t {
            break;
        }
        ax = c.inc(W, ax);
        n += 1;
    }
    c.set_reg(AX, ax);
    c.count(n);
    c.ret()
}

/// `get_location_type_string_offset` (6231): AX = the string index for the
/// kind of the location at DI: 0 below 20h, 1 for 20h, 2 for 21h..27h, 3 for
/// 28h..2Fh, and 1 again from 30h on.
///
/// Exit: BX kept; flags from the last `cmp`, or the `sub` for 30h and up.
pub fn get_location_type_string_offset(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6231)?;
    let mut c = Ctx::new(m);
    c.check(start, 17)?;
    let bx = c.reg(BX);
    c.push(bx);
    let di = c.reg(DI);
    let kind = c.byte(di.wrapping_add(8));
    c.set_reg8(r8::BL, kind);
    let mut ax = c.xor(W, 0, 0);
    let mut n = 5; // push, mov, xor; pop, ret
    for (i, t) in KIND_THRESHOLDS.into_iter().enumerate() {
        if i > 0 {
            ax = c.inc(W, ax);
            n += 1;
        }
        c.cmp(B, kind as u16, t as u16);
        n += 2;
        if kind < t {
            break;
        }
        if i == KIND_THRESHOLDS.len() - 1 {
            ax = c.sub(B, ax, 2); // sub al, 2
            n += 1;
        }
    }
    c.set_reg(AX, ax);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.count(n);
    c.ret()
}

/// `sub_01e0`: stores DI, DX, BX at +4, +6, +8 of the record at SI, then
/// sets its byte +12h from the kind (low nibble of [DI]): bits 4..6 kept,
/// bit 7 flipped once for each of 3, 5 and 9 the kind is above, and the
/// kind in the low nibble.
///
/// Exit: AX = (bits 4..6 and the flips) << 8 | the new byte. Flags from the
/// final `or`.
pub fn sub_01e0(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x01e0)?;
    let mut c = Ctx::new(m);
    c.check(start, 18)?;
    let (si, di, dx, bx) = (c.reg(SI), c.reg(DI), c.reg(DX), c.reg(BX));
    c.set_word(si.wrapping_add(4), di);
    c.set_word(si.wrapping_add(6), dx);
    c.set_word(si.wrapping_add(8), bx);
    let kind = c.byte(di);
    let old = c.byte(si.wrapping_add(0x12));
    let ax = c.and(W, (old as u16) << 8 | kind as u16, 0x700f);
    let kind = ax & 0xff;
    let mut high = ax >> 8;
    let mut n = 6 + 3; // stores, loads, and; or, mov, ret
    for t in [3, 5, 9] {
        c.cmp(B, kind, t);
        n += 2;
        if kind <= t {
            break;
        }
        high = c.xor(B, high, 0x80);
        n += 1;
    }
    let byte = c.or(B, kind, high);
    c.set_reg(AX, high << 8 | byte);
    c.set_byte(si.wrapping_add(0x12), byte as u8);
    c.count(n);
    c.ret()
}

/// `get_orni_position` (3A95): the ornithopter's screen position (DX, BX):
/// (95h, 39h), or (CAh, 49h) when byte 5 is 20h or more.
/// Flags from the `cmp`.
pub fn get_orni_position(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3a95)?;
    let mut c = Ctx::new(m);
    let x_high = c.byte(g::BYTE_0005);
    let far = x_high >= 0x20;
    c.run(start, if far { 7 } else { 5 })?;
    c.cmp(B, x_high as u16, 0x20);
    let (dx, bx) = if far { (0xca, 0x49) } else { (0x95, 0x39) };
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.ret()
}

/// `get_map_position` (407E): a map position in (DX, BX): the words at 4
/// and 6, BX sign-extended from its low byte. When that byte is 80h, the
/// position of the record at [`g::MAP_POSITION_RECORD`] instead (and SI
/// points to it).
/// Flags from the `cmp`.
pub fn get_map_position(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x407e)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    let x = c.word(g::WORD_0004);
    let y = c.word(g::WORD_0006);
    c.cmp(B, y & 0xff, 0x80);
    if y & 0xff == 0x80 {
        let si = c.word(g::MAP_POSITION_RECORD);
        c.set_reg(SI, si);
        let x = c.word(si.wrapping_add(2));
        let y = c.word(si.wrapping_add(4));
        c.set_reg(DX, x);
        c.set_reg(BX, y);
    } else {
        c.set_reg(DX, x);
        c.set_reg(BX, y as u8 as i8 as u16);
    }
    c.ret()
}

/// `sub_40c9`: moves the object at SI to (DX, BX) if its flag 40h (byte
/// +0Fh) is set. Flags from the `test`.
pub fn sub_40c9(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x40c9)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let flags = c.byte(si.wrapping_add(0x0f));
    let movable = flags & 0x40 != 0;
    c.run(start, if movable { 5 } else { 3 })?;
    c.test(B, flags as u16, 0x40);
    if movable {
        let (dx, bx) = (c.reg(DX), c.reg(BX));
        c.set_word(si, dx);
        c.set_word(si.wrapping_add(2), bx);
    }
    c.ret()
}

/// `sub_5082`: counts the location at SI: CX += 1 if its bit 80h at +10h
/// is set, else DX += 1 if its byte +3 is 6; nothing if bit 20h of +3 is
/// set. Flags from the last test, compare or `inc`.
pub fn sub_5082(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5082)?;
    let mut c = Ctx::new(m);
    c.check(start, 8)?;
    let si = c.reg(SI);
    let b3 = c.byte(si.wrapping_add(3));
    c.test(B, b3 as u16, 0x20);
    if b3 & 0x20 != 0 {
        c.count(3);
        return c.ret();
    }
    let b10 = c.byte(si.wrapping_add(0x10));
    c.test(B, b10 as u16, 0x80);
    if b10 & 0x80 != 0 {
        let cx = c.reg(CX);
        let cx = c.inc(W, cx);
        c.set_reg(CX, cx);
        c.count(6);
        return c.ret();
    }
    c.cmp(B, b3 as u16, 6);
    if b3 == 6 {
        let dx = c.reg(DX);
        let dx = c.inc(W, dx);
        c.set_reg(DX, dx);
        c.count(8);
    } else {
        c.count(7);
    }
    c.ret()
}

/// Location records: 1Ch bytes each from 100h (the slot before them at
/// 0E4h), FFFFh after the last.
const LOCATIONS_BEFORE: u16 = 0x00e4;
const LOCATION_SIZE: u16 = 0x1c;

/// `loc_0409a`: SI = the location whose word 6 is DI (ZF set), or past
/// the last (ZF clear, from `or si, si`).
pub fn find_location_by_word_6(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x409a)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let mut si = LOCATIONS_BEFORE;
    let mut n = 1;
    loop {
        si = si.wrapping_add(LOCATION_SIZE);
        n += 3;
        if c.word(si) == 0xffff {
            n += 2;
            break;
        }
        n += 2;
        if c.word(si.wrapping_add(6)) == di {
            n += 1;
            break;
        }
    }
    c.run(start, n)?;
    let mut si = LOCATIONS_BEFORE;
    loop {
        si = c.add(W, si, LOCATION_SIZE);
        let w = c.word(si);
        c.cmp(W, w, 0xffff);
        if w == 0xffff {
            c.or(W, si, si);
            break;
        }
        let v = c.word(si.wrapping_add(6));
        c.cmp(W, di, v);
        if v == di {
            break;
        }
    }
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_040ae`: for the location at DI: BH = its number (from 1), BL =
/// 80h, DH = its kind (byte 8), DL = 1.
pub fn location_coords(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x40ae)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let off = di.wrapping_sub(0x100);
    if off / LOCATION_SIZE > 0xff {
        // `div bl` faults: the interpreter raises it.
        c.m.cpu.ip = start;
        return Err(Bail);
    }
    c.run(start, 10)?;
    let off = c.sub(W, di, 0x100);
    c.set_reg(AX, off);
    c.m.cpu.mul_div(&mut c.m.hw, B, 6, LOCATION_SIZE as u32); // div bl
    let n = c.reg8(r8::AL);
    let n = c.inc(B, n as u16) as u8;
    let kind = c.byte(di.wrapping_add(8));
    c.set_reg(BX, (n as u16) << 8 | 0x80);
    c.set_reg(DX, (kind as u16) << 8 | 1);
    c.ret()
}

/// `loc_040e6`: for the record at SI with bits 40h and 2 of byte 0Fh:
/// `loc_09556`, `loc_09655`.
pub fn update_flagged_record(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x40e6)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let f = c.byte(si.wrapping_add(0x0f));
    let both = f & 0x40 != 0 && f & 2 != 0;
    c.run(start, if f & 0x40 == 0 { 3 } else { 5 })?;
    c.test(B, f as u16, 0x40);
    if f & 0x40 != 0 {
        c.test(B, f as u16, 2);
    }
    if both {
        c.call(0x9556, 0x40f5)?;
        c.run(0x40f5, 1)?;
        c.call(0x9655, 0x40f8)?;
        c.run(0x40f8, 1)?;
    }
    c.ret()
}

/// `loc_03efe`: SI = the 5-byte entry DL (from 1) of the list DH (word
/// table at 13C4h).
pub fn list_entry(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3efe)?;
    let mut c = Ctx::new(m);
    c.run(start, 11)?;
    let dx = c.reg(DX);
    c.xor(B, 0, 0);
    let i = c.shl1(W, dx >> 8, 1);
    let list = c.word(0x13c4u16.wrapping_add(i));
    let k = c.dec(B, dx & 0xff);
    c.set_reg(AX, 5 << 8 | k);
    c.mul8(5);
    let ax = c.reg(AX);
    let si = c.add(W, list, ax);
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_02e98`: the point at DI (two bytes, y + 0Ch) to [11EDh], [11EFh];
/// DI noted at [47E6h].
pub fn note_point(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x2e98)?;
    let mut c = Ctx::new(m);
    c.run(start, 10)?;
    let di = c.reg(DI);
    c.set_word(0x47e6, di);
    c.xor(B, 0, 0);
    let x = c.byte(di) as u16;
    let x = c.add(W, x, 0);
    c.set_word(0x11ed, x);
    let y = c.byte(di.wrapping_add(1)) as u16;
    c.xor(B, 0, 0);
    let y = c.add(W, y, 0x0c);
    c.set_reg(AX, y);
    c.set_word(0x11ef, y);
    c.ret()
}

/// `loc_033ad`: when the record at SI has word 2 = BX, the bit of its
/// byte 0Eh set at [0F7h].
pub fn mark_record_bit(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x33ad)?;
    let mut c = Ctx::new(m);
    let (si, bx) = (c.reg(SI), c.reg(BX));
    let w = c.word(si.wrapping_add(2));
    c.run(start, if w == bx { 7 } else { 3 })?;
    c.cmp(W, bx, w);
    if w == bx {
        let cl = c.byte(si.wrapping_add(0x0e));
        c.set_reg8(r8::CL, cl);
        let bit = c.shl(B, 1, cl) as u8;
        c.set_reg8(r8::AL, bit);
        let v = c.byte(0xf7);
        let v = c.or(B, v as u16, bit as u16);
        c.set_byte(0xf7, v as u8);
    }
    c.ret()
}
