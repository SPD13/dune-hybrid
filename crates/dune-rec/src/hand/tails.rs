//! Short entries: a few instructions that set up registers (or call a
//! helper first) and go on into another routine by a jump or by falling
//! into the next one. The routine they go on to runs as its own port
//! when it has one, else in the original from there.

use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DX, SI},
    rec::Bail,
};

/// `intro_part_2` (02DE): CX 0, then `draw_stars`.
pub fn intro_part_2(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x02de)?;
    let mut c = Ctx::new(m);
    c.run(0x02de, 1)?;
    let cx = c.reg(CX);
    c.xor(W, cx, cx);
    c.set_reg(CX, 0);
    c.jump(0x02e0)
}

/// `draw_stars` (02E0): on at 0A44h.
pub fn draw_stars(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x02e0)?;
    let mut c = Ctx::new(m);
    c.run(0x02e0, 1)?;
    c.jump(0x0a44)
}

/// `intro_load_desert_flyover_video` (06CE) and
/// `intro_desert_flight_to_sietch` (06D3): video 10h / 11h opened at
/// 06F3h.
pub fn intro_video_06f3(m: &mut Machine, start: u16) -> Result<(), Bail> {
    let v = match start {
        0x06ce => 0x10,
        0x06d3 => 0x11,
        _ => return Err(Bail),
    };
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(AX, v);
    c.jump(0x06f3)
}

/// `intro_chani_in_sietch` (0788): scene AL 7 (099Dh).
pub fn intro_chani_in_sietch(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0788)?;
    let mut c = Ctx::new(m);
    c.run(0x0788, 2)?;
    c.set_reg8(r8::AL, 7);
    c.jump(0x099d)
}

/// `ui_click_room_left` (3F24): BP 4 (left), then `ui_click_move_room`.
pub fn ui_click_room_left(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3f24)?;
    let mut c = Ctx::new(m);
    c.run(0x3f24, 1)?;
    c.set_reg(BP, 4);
    c.jump(0x3f27)
}

/// `loc_0d239`: CH 2, on at D23Fh.
pub fn ch2_d23f(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd239)?;
    let mut c = Ctx::new(m);
    c.run(0xd239, 2)?;
    c.set_reg8(r8::CH, 2);
    c.jump(0xd23f)
}

/// `loc_0b2aa`: `loc_0b2b9`, then the screen B2B3h pushed (`loc_0d323`).
pub fn screen_b2aa(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb2aa)?;
    let mut c = Ctx::new(m);
    c.run(0xb2aa, 1)?;
    c.call(0xb2b9, 0xb2ad)?;
    c.run(0xb2ad, 2)?;
    c.set_reg(BX, 0xb2b3);
    c.jump(0xd323)
}

/// `loc_01860`: unless [11C9h], `loc_01843` then `loc_0186b`.
pub fn frieze_1860(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1860)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0x11c9);
    c.run(0x1860, 3)?;
    c.cmp(B, f as u16, 0);
    if f != 0 {
        return c.ret();
    }
    c.call(0x1843, 0x186b)?;
    c.jump(0x186b)
}

/// `loc_06acb`: the record at SI of kind CL ([si+3] low nibble): on at
/// 6B24h, else at 6AD4h.
pub fn kind_6acb(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6acb)?;
    let mut c = Ctx::new(m);
    c.run(0x6acb, 4)?;
    let si = c.reg(SI);
    let k = c.byte(si.wrapping_add(3));
    c.set_reg8(r8::AL, k);
    let a = c.and(B, k as u16, 0xf);
    c.set_reg8(r8::AL, a as u8);
    let cl = c.reg8(r8::CL);
    c.cmp(B, a, cl as u16);
    c.jump(if a == cl as u16 { 0x6b24 } else { 0x6ad4 })
}

/// `loc_06c15`: by the kind of the record at SI ([si+3] low nibble), DI
/// its +4 word, CF clear: on through the table at CS:6BF5h.
pub fn kind_6c15(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6c15)?;
    let mut c = Ctx::new(m);
    c.run(0x6c15, 6)?;
    let si = c.reg(SI);
    let di = c.word(si.wrapping_add(4));
    c.set_reg(DI, di);
    let k = c.byte(si.wrapping_add(3));
    c.set_reg8(r8::BL, k);
    let bx = c.reg(BX);
    let bx = c.and(W, bx, 0xf);
    let bx = c.shl1(W, bx, 1);
    c.set_reg(BX, bx);
    c.set_flag(CF, false);
    let to = c.code_word(0x6bf5u16.wrapping_add(bx));
    c.jump(to)
}

/// `loc_05406`: the map's centre [4710h]/[4712h] from [11C1h]/[11C3h];
/// `loc_05beb`, `loc_05f79`, `loc_079de`, then `loc_0541f`.
pub fn map_5406(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5406)?;
    let mut c = Ctx::new(m);
    c.run(0x5406, 5)?;
    let dx = c.word(0x11c1);
    c.set_reg(DX, dx);
    let bx = c.word(0x11c3);
    c.set_reg(BX, bx);
    c.set_word(0x4710, dx);
    c.set_word(0x4712, bx);
    c.call(0x5beb, 0x5419)?;
    c.run(0x5419, 1)?;
    c.call(0x5f79, 0x541c)?;
    c.run(0x541c, 1)?;
    c.call(0x79de, 0x541f)?;
    c.jump(0x541f)
}

/// `loc_0bc81`: `loc_0ba9e`, `loc_05b5d`, the mouse hidden over A0h,4Fh
/// (`loc_0dbb2`), `loc_0bc99`, `loc_0adbe`, then 5A3Dh.
pub fn globe_bc81(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbc81)?;
    let mut c = Ctx::new(m);
    c.run(0xbc81, 1)?;
    c.call(0xba9e, 0xbc84)?;
    c.run(0xbc84, 1)?;
    c.call(0x5b5d, 0xbc87)?;
    c.run(0xbc87, 3)?;
    c.set_reg(DX, 0xa0);
    c.set_reg(BX, 0x4f);
    c.call(0xdbb2, 0xbc90)?;
    c.run(0xbc90, 1)?;
    c.call(0xbc99, 0xbc93)?;
    c.run(0xbc93, 1)?;
    c.call(0xadbe, 0xbc96)?;
    c.run(0xbc96, 1)?;
    c.jump(0x5a3d)
}

/// `sub_06d8`: the music stopped (`loc_0ac14`), [46DFh] 1, sound 12h
/// (`loc_038a2`), then video 12h into the buffer + 1E0h (`loc_0ca1b`).
pub fn intro_06d8(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x06d8)?;
    let mut c = Ctx::new(m);
    c.run(0x06d8, 1)?;
    c.call(0xac14, 0x06db)?;
    c.run(0x06db, 4)?;
    c.set_reg8(r8::AL, 1);
    c.set_byte(0x46df, 1);
    c.set_reg8(r8::BL, 0x12);
    c.call(0x38a2, 0x06e5)?;
    c.run(0x06e5, 4)?;
    c.set_reg(AX, 0x12);
    let v = c.word(0xdbda);
    let v = c.add(W, v, 0x1e0);
    c.set_word(0xdbda, v);
    c.jump(0xca1b)
}

/// `sub_b18b`: the music stopped (`loc_0adbe`), [2788h], [0C6h], [1C30h]
/// cleared, `loc_0ca01`, `loc_00a3e`, `loc_09901`, sheet 26h, then
/// 1877h.
pub fn b18b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb18b)?;
    let mut c = Ctx::new(m);
    c.run(0xb18b, 1)?;
    c.call(0xadbe, 0xb18e)?;
    c.run(0xb18e, 4)?;
    c.set_byte(0x2788, 0);
    c.set_byte(0xc6, 0);
    c.set_byte(0x1c30, 0);
    c.call(0xca01, 0xb1a0)?;
    c.run(0xb1a0, 1)?;
    c.call(0x0a3e, 0xb1a3)?;
    c.run(0xb1a3, 1)?;
    c.call(0x9901, 0xb1a6)?;
    c.run(0xb1a6, 2)?;
    c.set_reg(AX, 0x26);
    c.call(0xc13e, 0xb1ac)?;
    c.run(0xb1ac, 1)?;
    c.jump(0x1877)
}

/// `loc_0586e`: on through the table at CS:588Bh by BL, CH, CL being
/// set (bits 0, 1, 2 of the index).
pub fn flags_586e(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x586e)?;
    let mut c = Ctx::new(m);
    let (bl, ch, cl) = (c.reg8(r8::BL), c.reg8(r8::CH), c.reg8(r8::CL));
    c.run(0x586e, 3 + (bl != 0) as u32 + 2 + (ch != 0) as u32 + 2 + (cl != 0) as u32 + 3)?;
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    let mut al = 0u16;
    c.or(B, bl as u16, bl as u16);
    if bl != 0 {
        al = 1;
    }
    c.or(B, ch as u16, ch as u16);
    if ch != 0 {
        al = c.add(B, al, 2);
    }
    c.or(B, cl as u16, cl as u16);
    if cl != 0 {
        al = c.add(B, al, 4);
    }
    let ax = c.shl1(W, al, 1);
    c.set_reg(AX, ax);
    c.set_reg(SI, ax);
    let to = c.code_word(0x588bu16.wrapping_add(ax));
    c.jump(to)
}

/// `loc_0cfe4`: the sprites for the screen kind [0CEEBh]: BBh (C7h for
/// kind 6) read to CEECh, then C0h + kind into [47ACh] (far);
/// `loc_00098`, on at D01Ah.
pub fn sheets_cfe4(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::ES;
    entered(start, 0xcfe4)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0xceeb);
    c.run(0xcfe4, 4 + (k == 6) as u32 + 4)?;
    c.set_reg8(r8::AL, k);
    c.set_reg(SI, 0xbb);
    c.cmp(B, k as u16, 6);
    if k == 6 {
        c.set_reg(SI, 0xc7);
    }
    c.set_reg(DI, 0xceec);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let v = c.pop();
    c.set_sreg(ES, v);
    c.call(0xf0b9, 0xcff9)?;
    c.run(0xcff9, 6)?;
    let k = c.byte(0xceeb);
    let a = c.add(B, 0xc0, k as u16);
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    c.set_reg(AX, a);
    c.set_reg(SI, a);
    let (d, e) = (c.word(0x47ac), c.word(0x47ae));
    c.set_reg(DI, d);
    c.set_sreg(ES, e);
    c.call(0xf0b9, 0xd00a)?;
    c.run(0xd00a, 1)?;
    c.call(0x0098, 0xd00d)?;
    c.run(0xd00d, 1)?;
    c.jump(0xd01a)
}
