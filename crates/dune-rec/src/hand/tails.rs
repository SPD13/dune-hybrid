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

/// `intro_lady_jessica_2` (0771): the background 2004h (`loc_00978`),
/// then scene AL 1 (099Dh).
pub fn intro_lady_jessica_2(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0771)?;
    let mut c = Ctx::new(m);
    c.run(0x0771, 2)?;
    c.set_reg(DX, 0x2004);
    c.call(0x0978, 0x0777)?;
    c.run(0x0777, 2)?;
    c.set_reg8(r8::AL, 1);
    c.jump(0x099d)
}

/// `intro_duke_leto` (0995): the background 200Ah (`loc_00978`), then
/// scene 0 at DX 0 (`loc_009c7`) and on at 978Eh.
pub fn intro_duke_leto(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0995)?;
    let mut c = Ctx::new(m);
    c.run(0x0995, 2)?;
    c.set_reg(DX, 0x200a);
    c.call(0x0978, 0x099b)?;
    c.run(0x099b, 3)?;
    let al = c.reg8(r8::AL);
    c.xor(B, al as u16, al as u16);
    c.set_reg8(r8::AL, 0);
    let dx = c.reg(DX);
    c.xor(W, dx, dx);
    c.set_reg(DX, 0);
    c.call(0x09c7, 0x09a2)?;
    c.run(0x09a2, 1)?;
    c.jump(0x978e)
}

/// `intro_paul_on_red_background` (07EE): sheet 30h, the sprite list
/// 1526h drawn (`loc_0c21b`), on at 0960h.
pub fn intro_paul_on_red_background(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x07ee)?;
    let mut c = Ctx::new(m);
    c.run(0x07ee, 2)?;
    c.set_reg(AX, 0x30);
    c.call(0xc13e, 0x07f4)?;
    c.run(0x07f4, 2)?;
    c.set_reg(SI, 0x1526);
    c.call(0xc21b, 0x07fa)?;
    c.run(0x07fa, 1)?;
    c.jump(0x0960)
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

/// `loc_0b024`: DH FFh, `loc_0b1af`, then BP AF43h into `loc_0b02c`.
pub fn b1af_b024(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb024)?;
    let mut c = Ctx::new(m);
    c.run(0xb024, 2)?;
    c.set_reg8(r8::DH, 0xff);
    c.call(0xb1af, 0xb029)?;
    c.run(0xb029, 1)?;
    c.set_reg(BP, 0xaf43);
    c.jump(0xb02c)
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

/// `loc_049d9`: without a talking head ([46EBh] negative) on through
/// [46EDh]; else `loc_049a0`, then 4A5Ah.
pub fn head_49d9(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x49d9)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0x46eb);
    if f & 0x80 != 0 {
        c.run(0x49d9, 3)?;
        c.cmp(B, f as u16, 0);
        let to = c.word(0x46ed);
        return c.jump(to);
    }
    c.run(0x49d9, 3)?;
    c.cmp(B, f as u16, 0);
    c.call(0x49a0, 0x49e3)?;
    c.run(0x49e3, 1)?;
    c.jump(0x4a5a)
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

/// `intro_somebody_in_sietch` (07C6): the background 802h at BX 1080h
/// (`loc_00981`, [12h] 100h meanwhile), then scene AL 5 (099Dh).
pub fn intro_somebody_in_sietch(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x07c6)?;
    let mut c = Ctx::new(m);
    c.run(0x07c6, 4)?;
    c.set_reg(BX, 0x1080);
    c.set_reg(DX, 0x802);
    c.set_word(0x12, 0x100);
    c.call(0x0981, 0x07d5)?;
    c.run(0x07d5, 3)?;
    c.set_word(0x12, 0);
    c.set_reg8(r8::AL, 5);
    c.jump(0x099d)
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

/// `loc_0d323`: `loc_0d316`, `loc_0d338`, `loc_0d280`, then D410h.
pub fn screen_d323(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd323)?;
    let mut c = Ctx::new(m);
    c.run(0xd323, 1)?;
    c.call(0xd316, 0xd326)?;
    c.run(0xd326, 1)?;
    c.call(0xd338, 0xd329)?;
    c.run(0xd329, 1)?;
    c.call(0xd280, 0xd32c)?;
    c.run(0xd32c, 1)?;
    c.jump(0xd410)
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

/// `loc_00802`: `loc_00820`, the driver's far [38BDh] (CX F0h, BX 180h),
/// [46D7h] 0, sheet 2Eh, then the sprite list 1500h (`loc_0c21b`).
pub fn intro_0802(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0802)?;
    let mut c = Ctx::new(m);
    c.run(0x0802, 1)?;
    c.call(0x0820, 0x0805)?;
    c.run(0x0805, 2)?;
    c.set_reg(CX, 0xf0);
    c.set_reg(BX, 0x180);
    c.interp(0x080b, 0x080f)?;
    c.run(0x080f, 3)?;
    c.set_byte(0x46d7, 0);
    c.set_reg(AX, 0x2e);
    c.call(0xc13e, 0x081a)?;
    c.run(0x081a, 2)?;
    c.set_reg(SI, 0x1500);
    c.jump(0xc21b)
}

/// `intro_26_baron` (09AD): sheet 30h, the sprite list 153Ah,
/// `loc_0c412`, scene 9 at DX 52h (`loc_009c7`), on at 978Eh.
pub fn intro_26_baron(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x09ad)?;
    let mut c = Ctx::new(m);
    c.run(0x09ad, 2)?;
    c.set_reg(AX, 0x30);
    c.call(0xc13e, 0x09b3)?;
    c.run(0x09b3, 2)?;
    c.set_reg(SI, 0x153a);
    c.call(0xc21b, 0x09b9)?;
    c.run(0x09b9, 1)?;
    c.call(0xc412, 0x09bc)?;
    c.run(0x09bc, 3)?;
    c.set_reg8(r8::AL, 9);
    c.set_reg(DX, 0x52);
    c.call(0x09c7, 0x09c4)?;
    c.run(0x09c4, 1)?;
    c.jump(0x978e)
}

/// `loc_07ba3`: `loc_0c08e`; a new SI (not [46EFh]) becomes [46F1h]
/// and is set up (`loc_079ee`, `loc_09f40`). SI kept.
pub fn select_7ba3(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7ba3)?;
    let mut c = Ctx::new(m);
    c.run(0x7ba3, 1)?;
    c.call(0xc08e, 0x7ba6)?;
    let si = c.reg(SI);
    let cur = c.word(0x46ef);
    if si == cur {
        c.run(0x7ba6, 3)?;
        c.cmp(W, si, cur);
        return c.ret();
    }
    c.run(0x7ba6, 5)?;
    c.cmp(W, si, cur);
    c.push(si);
    c.set_word(0x46f1, si);
    c.call(0x79ee, 0x7bb4)?;
    c.run(0x7bb4, 1)?;
    c.call(0x9f40, 0x7bb7)?;
    c.run(0x7bb7, 2)?;
    let v = c.pop();
    c.set_reg(SI, v);
    c.ret()
}

/// `loc_013c8`: an intro script at CS:SI: [4] its first byte, [4778h]
/// its list, [477Ah] past the list (its length byte); `loc_01392`,
/// `draw_room_game_screen`, then 1707h.
pub fn script_13c8(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::CS;
    entered(start, 0x13c8)?;
    let mut c = Ctx::new(m);
    c.run(0x13c8, 8)?;
    let a = c.lods(B, CS);
    c.set_byte(4, a as u8);
    let si = c.reg(SI);
    c.set_word(0x4778, si);
    let n = c.lods(B, CS);
    c.xor(B, 0, 0);
    c.set_reg(AX, n);
    let si = c.reg(SI);
    let s = c.add(W, si, n);
    c.set_reg(SI, s);
    c.set_word(0x477a, s);
    c.call(0x1392, 0x13de)?;
    c.run(0x13de, 1)?;
    c.call(0x2db1, 0x13e1)?;
    c.run(0x13e1, 1)?;
    c.jump(0x1707)
}
