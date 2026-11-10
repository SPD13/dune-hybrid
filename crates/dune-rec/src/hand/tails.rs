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

/// `loc_0189a`: the room redrawn: directly (`draw_room_game_screen`)
/// with [46D9h]; else by transition AL (DX 0), the screen copied
/// (`loc_0c07c`), `loc_0ae04`, [0DC5Ah] the clock, then 17E6h.
pub fn room_189a(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x189a)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0x46d9);
    c.run(0x189a, 3 + if f != 0 { 1 } else { 2 })?;
    c.set_reg(BP, 0x2db1);
    c.cmp(B, f as u16, 0);
    if f != 0 {
        return c.jump(0x2db1);
    }
    let dx = c.reg(DX);
    c.xor(W, dx, dx);
    c.set_reg(DX, 0);
    c.call(0xc108, 0x18ab)?;
    c.run(0x18ab, 1)?;
    c.call(0xc07c, 0x18ae)?;
    c.run(0x18ae, 1)?;
    c.call(0xae04, 0x18b1)?;
    c.run(0x18b1, 3)?;
    let t = c.word(0xce7a);
    c.set_reg(AX, t);
    c.set_word(0xdc5a, t);
    c.jump(0x17e6)
}

/// `intro_kynes` (07A3): sheet 2Eh, the sprite list 1500h, sprite 1 at
/// 54h,0Bh, `loc_0c412`, then scene 6 with BP 12h (099Dh).
pub fn intro_kynes(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x07a3)?;
    let mut c = Ctx::new(m);
    c.run(0x07a3, 2)?;
    c.set_reg(AX, 0x2e);
    c.call(0xc13e, 0x07a9)?;
    c.run(0x07a9, 2)?;
    c.set_reg(SI, 0x1500);
    c.call(0xc21b, 0x07af)?;
    c.run(0x07af, 4)?;
    c.set_reg(AX, 1);
    c.set_reg(DX, 0x54);
    c.set_reg(BX, 0xb);
    c.call(0xc22f, 0x07bb)?;
    c.run(0x07bb, 1)?;
    c.call(0xc412, 0x07be)?;
    c.run(0x07be, 3)?;
    c.set_reg8(r8::AL, 6);
    c.set_reg(BP, 0x12);
    c.jump(0x099d)
}

/// `loc_009f5`: `loc_009ef`, `loc_0c0f4`, the music on (`loc_0ad50`)
/// unless the intro's voice has it ([227Dh] clear and [3810h] bit 0);
/// the frame task A16h added (`loc_0da25`, BP 0). CF clear.
pub fn intro_09f5(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x09f5)?;
    let mut c = Ctx::new(m);
    c.run(0x09f5, 1)?;
    c.call(0x09ef, 0x09f8)?;
    c.run(0x09f8, 1)?;
    c.call(0xc0f4, 0x09fb)?;
    let f = c.byte(0x227d);
    let v = c.byte(0x3810);
    let skip = f == 0 && v & 1 != 0;
    c.run(0x09fb, 2 + if f == 0 { 2 } else { 0 } + (!skip) as u32)?;
    c.cmp(B, f as u16, 0);
    if f == 0 {
        c.test(B, v as u16, 1);
    }
    if !skip {
        c.call(0xad50, 0x0a0c)?;
    }
    c.run(0x0a0c, 3)?;
    c.set_reg(SI, 0xa16);
    let bp = c.reg(BP);
    c.xor(W, bp, bp);
    c.set_reg(BP, 0);
    c.call(0xda25, 0x0a14)?;
    c.run(0x0a14, 2)?;
    c.set_flag(CF, false);
    c.ret()
}

/// `loc_0094a`: `loc_038b4`, sheet 2Dh, then its sprite 0 at 0,3Ch.
pub fn intro_094a(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x094a)?;
    let mut c = Ctx::new(m);
    c.run(0x094a, 1)?;
    c.call(0x38b4, 0x094d)?;
    c.run(0x094d, 2)?;
    c.set_reg(AX, 0x2d);
    c.call(0xc13e, 0x0953)?;
    c.run(0x0953, 4)?;
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    c.set_reg(AX, 0);
    let dx = c.reg(DX);
    c.xor(W, dx, dx);
    c.set_reg(DX, 0);
    c.set_reg(BX, 0x3c);
    c.jump(0xc22f)
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

/// `loc_047fb`: the counter [4731h] stepped by CL each 14h ticks
/// (`loc_0e353`, BP 4821h) up to 21h; at 1Ah going up the sound
/// `loc_0ac30`; `loc_0ae04` each step.
pub fn steps_47fb(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x47fb)?;
    let mut c = Ctx::new(m);
    loop {
        c.run(0x47fb, 3)?;
        c.set_reg(BP, 0x4821);
        c.set_reg(AX, 0x14);
        c.call(0xe353, 0x4804)?;
        let cl = c.reg8(r8::CL);
        let v = c.byte(0x4731).wrapping_add(cl);
        let sound = v == 0x1a && cl & 0x80 == 0;
        c.run(0x4804, 4 + if v == 0x1a { 2 } else { 0 } + 1)?;
        let old = c.byte(0x4731);
        let v = c.add(B, old as u16, cl as u16);
        c.set_byte(0x4731, v as u8);
        c.set_reg8(r8::AL, v as u8);
        c.cmp(B, v, 0x1a);
        if v == 0x1a {
            c.or(B, cl as u16, cl as u16);
            if sound {
                c.call(0xac30, 0x4816)?;
            }
        }
        if v == 0x1a && sound {
            c.run(0x4816, 1)?;
        }
        c.call(0xae04, 0x4819)?;
        let k = c.byte(0x4731);
        c.run(0x4819, 2 + (k >= 0x21) as u32)?;
        c.cmp(B, k as u16, 0x21);
        if k >= 0x21 {
            return c.ret();
        }
    }
}

/// `loc_0aed6`: unless [11C9h], the book opened: `loc_018ba`,
/// `loc_0181e`, `clear_some_mouse_rect`, [0C6h] 1, `loc_0b2b9`,
/// `loc_0d95b`, the music stopped, transition 34h with BP AF26h, then
/// `loc_0ae04`.
pub fn book_aed6(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xaed6)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0x11c9);
    c.run(0xaed6, 3)?;
    c.cmp(B, f as u16, 0);
    if f != 0 {
        return c.ret();
    }
    for (at, to, ret) in [(0xaede, 0x18ba, 0xaee1), (0xaee1, 0x181e, 0xaee4), (0xaee4, 0xdaa3, 0xaee7)] {
        if at != 0xaede {
            c.run(at, 1)?;
        }
        c.call(to, ret)?;
    }
    c.run(0xaee7, 2)?;
    c.set_byte(0xc6, 1);
    c.call(0xb2b9, 0xaeef)?;
    c.run(0xaeef, 1)?;
    c.call(0xd95b, 0xaef2)?;
    c.run(0xaef2, 1)?;
    c.call(0xad5e, 0xaef5)?;
    c.run(0xaef5, 3)?;
    c.set_reg8(r8::AL, 0x34);
    c.set_reg(BP, 0xaf26);
    c.call(0xc108, 0xaefd)?;
    c.run(0xaefd, 1)?;
    c.jump(0xae04)
}

/// `loc_009c7`: talking head AL ([47C4h]) set up (`loc_091a0`),
/// [478Ch] 1Eh; DX (when not past the head's x [1BF0h]) moves the head
/// right, its right edge [1BF4h] kept within 140h.
pub fn head_09c7(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x09c7)?;
    let mut c = Ctx::new(m);
    c.run(0x09c7, 4)?;
    let dx = c.reg(DX);
    c.push(dx);
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    c.set_reg8(r8::AH, 0);
    let ax = c.reg(AX);
    c.set_word(0x47c4, ax);
    c.call(0x91a0, 0x09d0)?;
    let sp = c.reg(pc::cpu::SP);
    let d = c.word_in(pc::cpu::SS, sp);
    let x = c.word(0x1bf0);
    if x >= d {
        c.run(0x09d0, 5 + 1)?;
    } else {
        let r = c.word(0x1bf4).wrapping_add(d);
        let clip = (r as i16) > 0x140;
        c.run(0x09d0, 5 + 4 + clip as u32 + 1)?;
    }
    c.set_byte(0x478c, 0x1e);
    c.set_reg(DI, 0x1bf0);
    let d = c.pop();
    c.set_reg(AX, d);
    c.cmp(W, x, d);
    if x >= d {
        return c.ret();
    }
    let v = c.add(W, x, d);
    c.set_word(0x1bf0, v);
    let r = c.word(0x1bf4);
    let r = c.add(W, r, d);
    c.set_word(0x1bf4, r);
    c.cmp(W, r, 0x140);
    if (r as i16) > 0x140 {
        c.set_word(0x1bf4, 0x140);
    }
    c.ret()
}

/// `intro_25_sting_start` (0886): sheet 30h, the frame (far [38DDh],
/// colour DEh, 1470h), the sprite lists 154Eh and 155Ch (`loc_0c21b`,
/// `loc_05ba8`, `loc_0c32f`), `loc_0c412`, scene 0Ah at DX 3Ah
/// (`loc_009c7`), on at 978Eh.
pub fn intro_25_sting_start(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::ES;
    entered(start, 0x0886)?;
    let mut c = Ctx::new(m);
    c.run(0x0886, 2)?;
    c.set_reg(AX, 0x30);
    c.call(0xc13e, 0x088c)?;
    c.run(0x088c, 3)?;
    c.set_reg(SI, 0x1470);
    c.set_reg8(r8::AL, 0xde);
    let es = c.word(0xdbda);
    c.set_sreg(ES, es);
    c.interp(0x0895, 0x0899)?;
    c.run(0x0899, 2)?;
    c.set_reg(SI, 0x154e);
    c.call(0xc21b, 0x089f)?;
    c.run(0x089f, 1)?;
    c.call(0x5ba8, 0x08a2)?;
    c.run(0x08a2, 2)?;
    c.set_reg(SI, 0x155c);
    c.call(0xc32f, 0x08a8)?;
    c.run(0x08a8, 1)?;
    c.call(0xc412, 0x08ab)?;
    c.run(0x08ab, 3)?;
    c.set_reg8(r8::AL, 0xa);
    c.set_reg(DX, 0x3a);
    c.call(0x09c7, 0x08b3)?;
    c.run(0x08b3, 1)?;
    c.jump(0x978e)
}

/// `loc_028b5`: sheet 15h, sound 0Ah; the device's caller [47A9h]: its
/// position (225Dh table: x low byte, y high) kept at 47F8h + 4 * it,
/// then its sprite 1Eh + it drawn there (`loc_0c22f`).
pub fn comm_28b5(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x28b5)?;
    let mut c = Ctx::new(m);
    c.run(0x28b5, 2)?;
    c.set_reg(AX, 0x15);
    c.call(0xc13e, 0x28bb)?;
    c.run(0x28bb, 2)?;
    c.set_reg8(r8::AL, 0xa);
    c.call(0xab15, 0x28c0)?;
    c.run(0x28c0, 12)?;
    let k = c.byte(0x47a9) as u16;
    c.set_reg8(r8::AL, k as u8);
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    c.set_reg(AX, k);
    let si = c.shl1(W, k, 1);
    c.set_reg(SI, si);
    let p = c.word(si.wrapping_add(0x225d));
    let bx = c.reg(BX);
    c.xor(W, bx, bx);
    let dx = p & 0xff;
    let bx = p >> 8;
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    let a = c.add(W, k, 0x1e);
    c.set_reg(AX, a);
    let si = c.shl1(W, si, 1);
    c.set_reg(SI, si);
    c.set_word(si.wrapping_add(0x47f8), dx);
    c.set_word(si.wrapping_add(0x47fa), bx);
    c.jump(0xc22f)
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

/// `sub_0ed0`: the palace plan's screen: sheet 3Bh's palette
/// (`loc_0c2f2`) and sprite 1, `loc_00f08`, its sprite 2 at 0,0, the
/// frieze, the panel's buttons 1D1Eh (`loc_0d72b`), `loc_098f5`,
/// [1C06h] 80h; then the screen pushed (BP 20C2h, BX EB9h: D338h).
pub fn plan_0ed0(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0ed0)?;
    let mut c = Ctx::new(m);
    c.run(0x0ed0, 2)?;
    c.set_reg8(r8::AL, 0x3b);
    c.call(0xc2f2, 0x0ed5)?;
    c.run(0x0ed5, 2)?;
    c.set_reg(AX, 1);
    c.call(0xc22f, 0x0edb)?;
    c.run(0x0edb, 1)?;
    c.call(0x0f08, 0x0ede)?;
    c.run(0x0ede, 2)?;
    c.set_reg(AX, 0x3b);
    c.call(0xc13e, 0x0ee4)?;
    c.run(0x0ee4, 4)?;
    c.set_reg(AX, 2);
    let dx = c.reg(DX);
    c.xor(W, dx, dx);
    c.set_reg(DX, 0);
    let bx = c.reg(BX);
    c.xor(W, bx, bx);
    c.set_reg(BX, 0);
    c.call(0xc22f, 0x0eee)?;
    c.run(0x0eee, 1)?;
    c.call(0x1797, 0x0ef1)?;
    c.run(0x0ef1, 2)?;
    c.set_reg(SI, 0x1d1e);
    c.call(0xd72b, 0x0ef7)?;
    c.run(0x0ef7, 1)?;
    c.call(0x98f5, 0x0efa)?;
    c.run(0x0efa, 4)?;
    c.set_byte(0x1c06, 0x80);
    c.set_reg(BP, 0x20c2);
    c.set_reg(BX, 0xeb9);
    c.jump(0xd338)
}

/// `loc_082da`: the troop at SI moved to DI: unless it is the player's
/// (8E0h) or already there (AL 10h), its place [si+4] set to DI for
/// `loc_031f6` and `loc_02e98` and put back (AL 0Bh); then
/// `loc_09f82`, `loc_0a1c4`, `loc_07bbe` (SI, DI kept), and A1E2h.
pub fn troop_82da(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x82da)?;
    let mut c = Ctx::new(m);
    let (si, di) = (c.reg(SI), c.reg(DI));
    let bp = c.word(si.wrapping_add(4));
    let player = si == 0x8e0;
    let same = !player && di == bp;
    c.run(0x82da, 5 + if player { 0 } else { 3 } + if same { 1 } else { 3 })?;
    c.push(di);
    c.push(si);
    c.set_reg(BP, bp);
    c.cmp(W, si, 0x8e0);
    if !player {
        c.set_reg8(r8::AL, 0x10);
        c.cmp(W, di, bp);
    }
    if !same {
        c.push(bp);
        c.set_word(si.wrapping_add(4), di);
        c.call(0x31f6, 0x82f2)?;
        c.run(0x82f2, 1)?;
        c.call(0x2e98, 0x82f5)?;
        c.run(0x82f5, 3)?;
        let v = c.pop();
        let s = c.reg(SI);
        c.set_word(s.wrapping_add(4), v);
        c.set_reg8(r8::AL, 0xb);
    }
    c.call(0x9f82, 0x82fd)?;
    c.run(0x82fd, 1)?;
    c.call(0xa1c4, 0x8300)?;
    c.run(0x8300, 1)?;
    c.call(0x7bbe, 0x8303)?;
    c.run(0x8303, 3)?;
    let v = c.pop();
    c.set_reg(SI, v);
    let v = c.pop();
    c.set_reg(DI, v);
    c.jump(0xa1e2)
}

/// `results_draw_text_and_icones` (BE1D): sheet 1, the sprite list
/// 2506h, `loc_0bed7`, `loc_0bdfa`; DD17h's three words cleared, [115Ch]
/// the day ([2] & FFF0h); `loc_0d075`, the text 2494h (`loc_0d1a6`), the
/// frame task BE57h every 0Ch (`loc_0da25`), then BE57h.
pub fn results_be1d(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbe1d)?;
    let mut c = Ctx::new(m);
    c.run(0xbe1d, 2)?;
    c.set_reg(AX, 1);
    c.call(0xc13e, 0xbe23)?;
    c.run(0xbe23, 2)?;
    c.set_reg(SI, 0x2506);
    c.call(0xc21b, 0xbe29)?;
    c.run(0xbe29, 1)?;
    c.call(0xbed7, 0xbe2c)?;
    c.run(0xbe2c, 1)?;
    c.call(0xbdfa, 0xbe2f)?;
    c.run(0xbe2f, 9)?;
    c.set_reg(SI, 0xdd17);
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    c.set_reg(AX, 0);
    c.set_word(0xdd17, 0);
    c.set_word(0xdd19, 0);
    c.set_word(0xdd1b, 0);
    let d = c.word(2);
    let d = c.and(W, d, 0xfff0);
    c.set_reg(AX, d);
    c.set_word(0x115c, d);
    c.call(0xd075, 0xbe48)?;
    c.run(0xbe48, 2)?;
    c.set_reg(SI, 0x2494);
    c.call(0xd1a6, 0xbe4e)?;
    c.run(0xbe4e, 3)?;
    c.set_reg(SI, 0xbe57);
    c.set_reg(BP, 0xc);
    c.call(0xda25, 0xbe57)?;
    c.jump(0xbe57)
}

/// `sub_0826`: the intro's next picture once the last has shown
/// ([46D7h] counted down elsewhere): [46D6h] + 1; at 0Bh sheet 9
/// instead, at 0Eh on at 0867h; else shown for 0Ah (0Dh) or 1Eh
/// steps: `loc_00820`, the driver's far [394Dh] (CX F0h, BX 180h).
/// Then 391Dh.
pub fn intro_0826(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0826)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x46d7);
    if k != 0 {
        c.run(0x0826, 3)?;
        c.cmp(B, k as u16, 0);
        return c.jump(0x391d);
    }
    let al = c.byte(0x46d6);
    let ax = (c.reg(AX) & 0xff00 | al as u16).wrapping_add(1);
    let n = ax as u8;
    if n == 0xb {
        c.run(0x0826, 2 + 4 + 2)?;
    } else if n == 0xe {
        c.run(0x0826, 2 + 4 + 2)?;
    } else {
        c.run(0x0826, 2 + 4 + 2 + 3 + (n != 0xd) as u32 + 3)?;
    }
    c.cmp(B, 0, 0);
    c.set_reg8(r8::AL, al);
    let a0 = c.reg(AX);
    let ax = c.inc(W, a0);
    c.set_reg(AX, ax);
    c.cmp(B, n as u16, 0xb);
    if n == 0xb {
        c.set_reg(AX, 9);
        return c.jump(0xc13e);
    }
    c.cmp(B, n as u16, 0xe);
    if n == 0xe {
        return c.jump(0x0867);
    }
    c.set_reg8(r8::AH, 0xa);
    c.cmp(B, n as u16, 0xd);
    if n != 0xd {
        c.set_reg8(r8::AH, 0x1e);
    }
    let ah = c.reg8(r8::AH);
    c.set_byte(0x46d7, ah);
    c.set_reg8(r8::BL, n);
    c.call(0x0820, 0x084a)?;
    c.run(0x084a, 2)?;
    c.set_reg(CX, 0xf0);
    c.set_reg(BX, 0x180);
    c.interp(0x0850, 0x0854)?;
    c.run(0x0854, 1)?;
    c.jump(0x391d)
}

/// `loc_09719`: unless [4Ch] is negative, the speaker's place moved to
/// (`loc_02a51` AL 0Fh, DI [si+4] of [46EFh]); `loc_09f82`; the dialogue
/// set ([47C4h] 0Fh, [47A2h] 10C8h, `loc_0a1c4`); the next line from
/// [47BAh] (or [0AB6Ah] when 0 or FFFFh) with [47C2h] 20h
/// (`loc_09f9e`), kept in [47BAh] (0 when it is the last).
pub fn dialogue_9719(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9719)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0x4c);
    let neg = f & 0x80 != 0;
    c.run(0x9719, 2 + if neg { 1 } else { 4 })?;
    c.cmp(B, f as u16, 0);
    if !neg {
        let si = c.word(0x46ef);
        c.set_reg(SI, si);
        let di = c.word(si.wrapping_add(4));
        c.set_reg(DI, di);
        c.set_reg8(r8::AL, 0xf);
        c.call(0x2a51, 0x972c)?;
        c.run(0x972c, 1)?;
    }
    c.call(0x9f82, 0x972f)?;
    c.run(0x972f, 3)?;
    c.set_word(0x47c4, 0xf);
    c.set_word(0x47a2, 0x10c8);
    c.call(0xa1c4, 0x973e)?;
    let v = c.word(0x47ba);
    let n = if v == 0xffff { 3 + 1 } else if v != 0 { 3 + 2 } else { 3 + 2 + 1 };
    c.run(0x973e, n + 2)?;
    c.set_reg(SI, v);
    let s = c.inc(W, v);
    c.set_reg(SI, s);
    if s != 0 {
        let s = c.dec(W, s);
        c.set_reg(SI, s);
    }
    if v == 0 || v == 0xffff {
        let w = c.word(0xab6a);
        c.set_reg(SI, w);
    }
    c.set_byte(0x47c2, 0x20);
    c.call(0x9f9e, 0x9754)?;
    let cf = c.flag(CF);
    c.run(0x9754, 2 + if cf { 1 } else { 0 } + 1)?;
    let si = c.reg(SI);
    c.set_word(0x47ba, si);
    if cf {
        c.set_word(0x47ba, 0);
    }
    c.ret()
}

/// `loc_0b96b`: the globe's decorations slid open (`globe_slide_decorations_open`),
/// [0DD02h] down, `loc_0b98b`, then B941h.
pub fn globe_b96b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb96b)?;
    let mut c = Ctx::new(m);
    c.run(0xb96b, 1)?;
    c.call(0xb8f3, 0xb96e)?;
    c.run(0xb96e, 2)?;
    let v = c.byte(0xdd02);
    let v = c.dec(B, v as u16);
    c.set_byte(0xdd02, v as u8);
    c.call(0xb98b, 0xb975)?;
    c.run(0xb975, 1)?;
    c.jump(0xb941)
}

/// `loc_0c02e`: `map_func`; the spice stock [0A6h] capped at [0A0h] +
/// [1172h] - [1170h] (0 below); the troop records from 8AAh (1Bh bytes
/// each, up to a 0) summed by their +1Ah byte: free ones (not bit 5 of
/// +3) into [0ACh] when working (+10h bit 7), else into [0AAh] unless
/// +3 bit 7.
pub fn totals_c02e(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc02e)?;
    let mut c = Ctx::new(m);
    c.run(0xc02e, 1)?;
    c.call(0xbfe3, 0xc031)?;
    let (a0, b1, b0, a6) = (c.word(0xa0), c.word(0x1172), c.word(0x1170), c.word(0xa6));
    let s = a0.wrapping_add(b1);
    let (d, borrow) = s.overflowing_sub(b0);
    let cap = if borrow { 0 } else { d };
    let below = cap < a6;
    c.run(0xc031, 4 + borrow as u32 + 2 + (!below) as u32 + 4)?;
    c.set_reg(AX, a0);
    let v = c.add(W, a0, b1);
    let v = c.sub(W, v, b0);
    let v = if borrow { c.xor(W, v, v) } else { v };
    c.set_reg(AX, v);
    c.cmp(W, v, a6);
    if !below {
        c.set_word(0xa6, v);
    }
    c.xor(W, v, v);
    c.set_reg(AX, 0);
    c.set_word(0xaa, 0);
    c.set_word(0xac, 0);
    let mut si = 0x8aau16;
    c.set_reg(SI, si);
    loop {
        let al = c.byte(si.wrapping_add(0x1a));
        let k3 = c.byte(si.wrapping_add(3));
        let k10 = c.byte(si.wrapping_add(0x10));
        let next = c.byte(si.wrapping_add(0x1b));
        let mut n = 3;
        if k3 & 0x20 == 0 {
            n += 2;
            if k10 & 0x80 != 0 {
                n += 2;
            } else {
                n += 2 + (k3 & 0x80 == 0) as u32;
            }
        }
        n += 3 + (next == 0) as u32;
        c.run(0xc054, n)?;
        c.set_reg8(r8::AL, al);
        c.test(B, k3 as u16, 0x20);
        if k3 & 0x20 == 0 {
            c.test(B, k10 as u16, 0x80);
            if k10 & 0x80 != 0 {
                let v = c.word(0xac);
                let v = c.add(W, v, al as u16);
                c.set_word(0xac, v);
            } else {
                c.test(B, k3 as u16, 0x80);
                if k3 & 0x80 == 0 {
                    let v = c.word(0xaa);
                    let v = c.add(W, v, al as u16);
                    c.set_word(0xaa, v);
                }
            }
        }
        si = c.add(W, si, 0x1b);
        c.set_reg(SI, si);
        c.cmp(B, next as u16, 0);
        if next == 0 {
            return c.ret();
        }
    }
}

/// `intro_28_night_attack_start` (0ACD): the particle sorter C827h, the
/// sheet byte at CS:C13Ch 2Bh (`open_onmap_resource`), the screen saved;
/// sprites 2 and 3 tiled over 1582h and 158Ah, the sprite list 11DDh,
/// `loc_05ba0`; 4856h's 8 bytes cleared; the frame task B45h replaced
/// (every 3), `loc_0c412`, then sound 3 (`loc_0ab15`).
pub fn intro_night_attack(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{CS, DS, ES};
    entered(start, 0x0acd)?;
    let mut c = Ctx::new(m);
    c.run(0x0acd, 3)?;
    c.set_word(0x2786, 0xc827);
    c.m.cpu.write8(&mut c.m.hw, CS, 0xc13c, 0x2b);
    c.call(0xc13b, 0x0adc)?;
    c.run(0x0adc, 1)?;
    c.call(0xc07c, 0x0adf)?;
    c.run(0x0adf, 4)?;
    c.set_reg(SI, 0x1582);
    let es = c.word(0xdbda);
    c.set_sreg(ES, es);
    c.set_reg(AX, 2);
    c.call(0xc370, 0x0aec)?;
    c.run(0x0aec, 3)?;
    c.set_reg(SI, 0x158a);
    c.set_reg(AX, 3);
    c.call(0xc370, 0x0af5)?;
    c.run(0x0af5, 2)?;
    c.set_reg(SI, 0x11dd);
    c.call(0xc21b, 0x0afb)?;
    c.run(0x0afb, 1)?;
    c.call(0x5ba0, 0x0afe)?;
    c.run(0x0afe, 6)?;
    c.set_reg(DI, 0x4856);
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    c.set_reg(AX, 0);
    let ds = c.sreg(DS);
    c.push(ds);
    let v = c.pop();
    c.set_sreg(ES, v);
    c.set_reg(CX, 8);
    c.rep(0xaa);
    c.run(0x0b0a, 2)?;
    c.set_reg(SI, 0xb45);
    c.call(0xda5f, 0x0b10)?;
    c.run(0x0b10, 3)?;
    c.set_reg(SI, 0xb45);
    c.set_reg(BP, 3);
    c.call(0xda25, 0x0b19)?;
    c.run(0x0b19, 1)?;
    c.call(0xc412, 0x0b1c)?;
    c.run(0x0b1c, 2)?;
    c.set_reg8(r8::AL, 3);
    c.jump(0xab15)
}

/// `loc_06dbb`: the place BX's people (`loc_040ae`; the 0Ch records at
/// FD8h, 10h bytes, and the player's at [4]) given DH as their state,
/// their kind 2 unless it is 1 (the player's also in [0Bh], [8], and
/// [473Bh] bit 7); then `loc_06603` with BP 6E0Fh and on at 6E02h. SI
/// kept.
pub fn people_6dbb(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6dbb)?;
    let mut c = Ctx::new(m);
    c.run(0x6dbb, 2)?;
    let si = c.reg(SI);
    c.push(si);
    c.call(0x40ae, 0x6dbf)?;
    c.run(0x6dbf, 2)?;
    c.set_reg(SI, 0xfd8);
    c.set_reg(CX, 0xc);
    let (bx, dh) = (c.reg(BX), c.reg8(r8::DH));
    for k in 0..0xcu16 {
        let s = 0xfd8 + 0x10 * k;
        let p = c.word(s.wrapping_add(2));
        let hit = p == bx;
        let v = c.word(s);
        let one = v & 0xff == 1;
        c.run(0x6dc5, 2 + if hit { 4 + (!one) as u32 + 1 } else { 0 } + 2)?;
        c.cmp(W, bx, p);
        if hit {
            c.set_reg(AX, v);
            c.set_reg8(r8::AH, dh);
            c.cmp(B, v & 0xff, 1);
            if !one {
                c.set_reg8(r8::AL, 2);
            }
            let ax = c.reg(AX);
            c.set_word(s, ax);
        }
        let s2 = c.add(W, s, 0x10);
        c.set_reg(SI, s2);
        c.set_reg(CX, 0xb - k);
    }
    let p = c.word(6);
    let hit = p == bx;
    let v = c.word(4);
    let one = v & 0xff == 1;
    c.run(0x6ddb, 2 + if hit { 5 + (!one) as u32 + 3 } else { 0 } + 3)?;
    c.cmp(W, bx, p);
    if hit {
        let f = c.byte(0x473b);
        let f = c.or(B, f as u16, 0x80);
        c.set_byte(0x473b, f as u8);
        c.set_reg(AX, v);
        c.set_reg8(r8::AH, dh);
        c.cmp(B, v & 0xff, 1);
        if !one {
            c.set_reg8(r8::AL, 2);
        }
        let ax = c.reg(AX);
        c.set_word(4, ax);
        c.set_byte(0xb, ax as u8);
        c.set_byte(8, (ax >> 8) as u8);
    }
    let v = c.pop();
    c.set_reg(SI, v);
    c.set_reg(BP, 0x6e0f);
    c.call(0x6603, 0x6e02)?;
    c.jump(0x6e02)
}

/// `sub_afc7`: the book's next page (unless [0C6h] bit 1 forbids it):
/// with bit 2 set, two pages on ([11BFh]) and the music back
/// (`loc_00a3e`, `loc_0ad5e`); the page found from [11BFh] (`loc_0b150`,
/// CX 1, DX FFFEh, BX [2406h]): shown (`ui_draw_book_turning_page`,
/// B02Ch with BP B039h, `loc_09901`, D397h); past the end, unless
/// already, bit 2 set and the book's frame drawn (`loc_009f5`,
/// B02Ch with BP 1797h); a step back is B024h's.
pub fn book_afc7(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::ZF;
    entered(start, 0xafc7)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0xc6);
    if f & 2 != 0 {
        c.run(0xafc7, 3)?;
        c.test(B, f as u16, 2);
        return c.ret();
    }
    c.run(0xafc7, 4 + if f & 4 != 0 { 2 } else { 5 })?;
    c.test(B, f as u16, 2);
    c.test(B, f as u16, 4);
    if f & 4 != 0 {
        let v = c.word(0x11bf);
        let v = c.add(W, v, 2);
        c.set_word(0x11bf, v);
        c.call(0x0a3e, 0xafdd)?;
        c.run(0xafdd, 1)?;
        c.call(0xad5e, 0xafe0)?;
        c.run(0xafe0, 5)?;
    }
    c.set_reg(CX, 1);
    c.set_reg(DX, 0xfffe);
    let bx = c.word(0x2406);
    c.set_reg(BX, bx);
    let si = c.word(0x11bf);
    c.set_reg(SI, si);
    c.call(0xb150, 0xaff1)?;
    if !c.flag(ZF) {
        c.run(0xaff1, 3)?;
        let di = c.reg(DI);
        c.set_word(0x11bf, di);
        c.call(0xb1af, 0xaffa)?;
        c.run(0xaffa, 2)?;
        c.set_reg(BP, 0xb039);
        c.call(0xb02c, 0xb000)?;
        c.run(0xb000, 1)?;
        c.call(0x9901, 0xb003)?;
        c.run(0xb003, 1)?;
        return c.jump(0xd397);
    }
    let dx = c.reg(DX);
    if dx & 0x8000 != 0 {
        c.run(0xaff1, 3)?;
        c.or(W, dx, dx);
        return c.jump(0xb024);
    }
    let f = c.byte(0xc6);
    if f & 4 != 0 {
        c.run(0xaff1, 5 + 1)?;
        c.or(W, dx, dx);
        c.test(B, f as u16, 4);
        return c.ret();
    }
    c.run(0xaff1, 5 + 2)?;
    c.or(W, dx, dx);
    c.test(B, f as u16, 4);
    let f = c.or(B, f as u16, 4);
    c.set_byte(0xc6, f as u8);
    c.call(0x09f5, 0xb019)?;
    c.run(0xb019, 2)?;
    let dx = c.reg(DX);
    c.xor(W, dx, dx);
    c.set_reg(DX, 0);
    c.call(0xb1af, 0xb01e)?;
    c.run(0xb01e, 2)?;
    c.set_reg(BP, 0x1797);
    c.jump(0xb02c)
}

/// `sub_941d`: a click in the dialogue: with the communication device
/// active ([47A9h]) on at 2993h; else after `loc_0d41b`, by the screen
/// BP: the palace plan's (20C2h) closed (`loc_0d2ea`, 0EB9h); the
/// book's (with [11C9h] & 3, unless [11CAh]) at 4AADh; with a dialogue
/// open ([479Eh]) `loc_0d2e2`; else the room redrawn (`draw_room_game_screen`,
/// BX and DX kept) and the click handled (`loc_09215`).
pub fn dialogue_941d(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x941d)?;
    let mut c = Ctx::new(m);
    let comm = c.byte(0x47a9);
    if comm != 0 {
        c.run(0x941d, 3)?;
        c.cmp(B, comm as u16, 0);
        return c.jump(0x2993);
    }
    c.run(0x941d, 3)?;
    c.cmp(B, 0, 0);
    c.call(0xd41b, 0x942a)?;
    let bp = c.reg(BP);
    if bp == 0x20c2 {
        c.run(0x942a, 3)?;
        c.cmp(W, bp, 0x20c2);
        c.call(0xd2ea, 0x9433)?;
        c.run(0x9433, 1)?;
        return c.jump(0x0eb9);
    }
    let f = c.byte(0x11c9);
    let g = c.byte(0x11ca);
    let book = f & 3 != 0 && g == 0;
    let open = c.word(0x479e);
    c.run(0x942a, 2 + 2 + if f & 3 != 0 { 2 } else { 0 } + if book { 1 } else { 2 + if open != 0 { 1 } else { 3 } })?;
    c.cmp(W, bp, 0x20c2);
    c.test(B, f as u16, 3);
    if f & 3 != 0 {
        c.cmp(B, g as u16, 0);
    }
    if book {
        return c.jump(0x4aad);
    }
    c.cmp(W, open, 0);
    if open != 0 {
        return c.jump(0xd2e2);
    }
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.call(0x2db1, 0x9453)?;
    c.run(0x9453, 3)?;
    let v = c.pop();
    c.set_reg(DX, v);
    let v = c.pop();
    c.set_reg(BX, v);
    c.jump(0x9215)
}

/// `loc_00f08`: without [10h] bit 7 on at 0960h; else Jessica's head
/// (7: `setup_lip_sync_data_from_current`, [478Ch] 0, 0Fh lower),
/// `loc_0978e`, `loc_0998e`, [22A6h] none, [47C3h] up, [47C6h] 1,
/// `loc_0c412`, then 0965h with DX 2Dh.
pub fn jessica_0f08(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0f08)?;
    let mut c = Ctx::new(m);
    let f = c.word(0x10);
    if f & 0x80 == 0 {
        c.run(0x0f08, 3)?;
        c.test(W, f, 0x80);
        return c.jump(0x0960);
    }
    c.run(0x0f08, 3)?;
    c.test(W, f, 0x80);
    c.call(0xc412, 0x0f16)?;
    c.run(0x0f16, 2)?;
    c.set_word(0x47c4, 7);
    c.call(0x9197, 0x0f1f)?;
    c.run(0x0f1f, 3)?;
    c.set_byte(0x478c, 0);
    let v = c.word(0x1bf2);
    let v = c.add(W, v, 0xf);
    c.set_word(0x1bf2, v);
    c.call(0x978e, 0x0f2c)?;
    c.run(0x0f2c, 1)?;
    c.call(0x998e, 0x0f2f)?;
    c.run(0x0f2f, 4)?;
    c.set_word(0x22a6, 0xffff);
    let v = c.byte(0x47c3);
    let v = c.inc(B, v as u16);
    c.set_byte(0x47c3, v as u8);
    c.set_word(0x47c6, 1);
    c.call(0xc412, 0x0f42)?;
    c.run(0x0f42, 2)?;
    c.set_reg(DX, 0x2d);
    c.jump(0x0965)
}

/// `loc_01707`: the intro script's next step. On the screen 1FAEh a
/// click on 1B56h (`loc_0d6fe`) goes to 9ED5h. Else [0CE9Dh] 0 and the
/// next byte of the script at CS:[477Ah] runs its step (the table at
/// CS:1475h); at its end (FFh) the task 176Bh removed, the place and
/// state from [4776h], [4774h] 0, the music changed unless phase 48h,
/// `loc_0b2be`, and on at 0FA7h (or, with [0FBh] negative, the music
/// stopped, `loc_068eb`, `loc_0780a`).
pub fn script_1707(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::CS;
    entered(start, 0x1707)?;
    let mut c = Ctx::new(m);
    let scr = c.word(0x2220);
    if scr == 0x1fae {
        c.run(0x1707, 4)?;
        c.cmp(W, scr, 0x1fae);
        c.set_reg(DI, 0x1b56);
        c.call(0xd6fe, 0x1715)?;
        if c.flag(CF) {
            c.run(0x1715, 2)?;
            return c.jump(0x9ed5);
        }
        c.run(0x1715, 1)?;
    } else {
        c.run(0x1707, 2)?;
        c.cmp(W, scr, 0x1fae);
    }
    let si = c.word(0x477a);
    let b = c.byte_in(CS, si);
    c.run(0x171a, 5 + if b == 0xff { 0 } else { 4 })?;
    c.set_byte(0xce9d, 0);
    c.set_reg(SI, si);
    c.lods(B, CS);
    c.cmp(B, b as u16, 0xff);
    if b != 0xff {
        let si = c.reg(SI);
        c.set_word(0x477a, si);
        let ah = c.reg8(r8::AH);
        c.xor(B, ah as u16, ah as u16);
        c.set_reg(AX, b as u16);
        c.set_reg(BX, b as u16);
        let to = c.code_word(0x1475u16.wrapping_add(b as u16));
        return c.jump(to);
    }
    c.run(0x1736, 2)?;
    c.set_reg(SI, 0x176b);
    c.call(0xda5f, 0x173c)?;
    let ph = c.byte(0x2a);
    c.run(0x173c, 7 + (ph != 0x48) as u32)?;
    let ax = c.word(0x4776);
    c.set_reg(AX, ax);
    c.set_byte(4, ax as u8);
    c.set_byte(0x46e0, (ax >> 8) as u8);
    c.xor(B, ax & 0xff, ax & 0xff);
    c.set_reg8(r8::AL, 0);
    c.set_byte(0x4774, 0);
    c.cmp(B, ph as u16, 0x48);
    if ph != 0x48 {
        c.call(0xadbe, 0x1755)?;
    }
    c.run(0x1755, 1)?;
    c.call(0xb2be, 0x1758)?;
    let f = c.byte(0xfb);
    if f & 0x80 == 0 {
        c.run(0x1758, 3)?;
        c.cmp(B, f as u16, 0);
        return c.jump(0x0fa7);
    }
    c.run(0x1758, 3)?;
    c.cmp(B, f as u16, 0);
    c.call(0xad5e, 0x1765)?;
    c.run(0x1765, 1)?;
    c.call(0x68eb, 0x1768)?;
    c.run(0x1768, 1)?;
    c.jump(0x780a)
}

/// `ui_draw_palace_plan` (18EE): after `loc_0d41b`, unless the plan is
/// shown (BP 2012h: `loc_0d2e2`), in the palace ([4] 20xxh but room 1):
/// the screen 1ABAh's buttons (`loc_0d95e`, `loc_0d2bd`), the screen
/// saved, the frame (far [38DDh] at 143Ch, colour F1h), `loc_05b6e`
/// (1444h, F7h), sheet 21h, the sprite list 120Bh, `loc_01948`, the
/// rect 143Ch shown (`loc_0c0d5` AL 10h), then the screen pushed (BP
/// 2012h, BX 19FCh).
pub fn palace_plan_18ee(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::ES;
    entered(start, 0x18ee)?;
    let mut c = Ctx::new(m);
    c.run(0x18ee, 1)?;
    c.call(0xd41b, 0x18f1)?;
    let bp = c.reg(BP);
    if bp == 0x2012 {
        c.run(0x18f1, 3)?;
        c.cmp(W, bp, 0x2012);
        return c.jump(0xd2e2);
    }
    let ax = c.word(4);
    let (al, ah) = (ax as u8, (ax >> 8) as u8);
    let go = ah == 0x20 && al != 1;
    c.run(0x18f1, 2 + 3 + if ah == 0x20 { 2 } else { 0 } + if go { 2 } else { 1 })?;
    c.cmp(W, bp, 0x2012);
    c.set_reg(AX, ax);
    c.cmp(B, ah as u16, 0x20);
    if ah == 0x20 {
        c.cmp(B, al as u16, 1);
    }
    if !go {
        return c.ret();
    }
    c.set_reg(AX, 0x1aba);
    c.call(0xd95e, 0x190c)?;
    c.run(0x190c, 1)?;
    c.call(0xd2bd, 0x190f)?;
    c.run(0x190f, 1)?;
    c.call(0xc07c, 0x1912)?;
    c.run(0x1912, 3)?;
    c.set_reg(SI, 0x143c);
    c.set_reg8(r8::AL, 0xf1);
    let es = c.word(0xdbda);
    c.set_sreg(ES, es);
    c.interp(0x191b, 0x191f)?;
    c.run(0x191f, 3)?;
    c.set_reg(SI, 0x1444);
    c.set_reg8(r8::AL, 0xf7);
    c.call(0x5b6e, 0x1927)?;
    c.run(0x1927, 2)?;
    c.set_reg(AX, 0x21);
    c.call(0xc13e, 0x192d)?;
    c.run(0x192d, 2)?;
    c.set_reg(SI, 0x120b);
    c.call(0xc21b, 0x1933)?;
    c.run(0x1933, 1)?;
    c.call(0x1948, 0x1936)?;
    c.run(0x1936, 3)?;
    c.set_reg(SI, 0x143c);
    c.set_reg8(r8::AL, 0x10);
    c.call(0xc0d5, 0x193e)?;
    c.run(0x193e, 3)?;
    c.set_reg(BP, 0x2012);
    c.set_reg(BX, 0x19fc);
    c.jump(0xd323)
}

/// `loc_07c02`: the place SI chosen (`loc_07ba3`, `loc_031f6`, the
/// travel to [si+4]: `loc_02e98`); past the shown count ([1176h])
/// [4Ch] FFh and the marker [4752h] reset (its +0Dh, +0Fh 1916h). Then
/// the place's lines told (`loc_09719` until CF clear, SI [46EFh]),
/// `loc_09efd`; [46F4h] 0, or for [47A5h] 80h 1 with `loc_07efb` and
/// `loc_07e1e`; `loc_09901` without a talking head; then `loc_0c07c`.
pub fn place_7c02(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7c02)?;
    let mut c = Ctx::new(m);
    c.run(0x7c02, 1)?;
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
    let shown = c.word(0x1176);
    if ax <= shown {
        c.run(0x7c11, 2 + 1)?;
        c.cmp(W, ax, shown);
    } else {
        let d = c.word(0x4752);
        c.run(0x7c11, 2 + 4 + if d != 0 { 3 } else { 0 } + 1)?;
        c.cmp(W, ax, shown);
        c.set_byte(0x4c, 0xff);
        c.set_reg(DI, d);
        c.or(W, d, d);
        if d != 0 {
            c.set_reg(AX, 0x1916);
            c.set_word(d.wrapping_add(0xd), 0x1916);
            c.set_word(d.wrapping_add(0xf), 0x1916);
        }
    }
    loop {
        c.call(0x9719, 0x7c30)?;
        let cf = c.flag(CF);
        c.run(0x7c30, 2 + if cf { 1 } else { 0 })?;
        let si = c.word(0x46ef);
        c.set_reg(SI, si);
        if !cf {
            break;
        }
    }
    c.run(0x7c36, 2)?;
    let si = c.reg(SI);
    c.push(si);
    c.call(0x9efd, 0x7c3a)?;
    let k = c.byte(0x47a5);
    c.run(0x7c3a, 4 + if k == 0x80 { 2 } else { 0 })?;
    let v = c.pop();
    c.set_reg(SI, v);
    c.set_byte(0x46f4, 0);
    c.cmp(B, k as u16, 0x80);
    if k == 0x80 {
        let v = c.byte(0x46f4);
        let v = c.inc(B, v as u16);
        c.set_byte(0x46f4, v as u8);
        c.call(0x7efb, 0x7c4e)?;
        c.run(0x7c4e, 2)?;
        let si = c.reg(SI);
        c.push(si);
        c.call(0x7e1e, 0x7c52)?;
        c.run(0x7c52, 2)?;
        let v = c.pop();
        c.set_reg(SI, v);
    }
    let f = c.byte(0x46eb);
    c.run(0x7c56, 2 + if f & 0x80 != 0 { 1 } else { 1 })?;
    c.cmp(B, f as u16, 0);
    if f & 0x80 != 0 {
        c.call(0x9901, 0x7c60)?;
        c.run(0x7c60, 1)?;
    }
    c.jump(0xc07c)
}

/// `loc_0bed7`: the results' figures: [115Ch] the day, the totals
/// (`loc_0c02e`, `loc_0bf26`), then six bytes at DD11h: [0A4h] / 2 + 1,
/// [0A2h] / 2 + 1, [0A8h] / 16 + 1, [0A6h] / 16 + 1, [0ACh] / 256 + 1,
/// [0AAh] / 256 + 1.
pub fn results_bed7(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{DS, ES};
    entered(start, 0xbed7)?;
    let mut c = Ctx::new(m);
    c.run(0xbed7, 4)?;
    let d = c.word(2);
    c.set_reg(AX, d);
    let d = c.and(W, d, 0xfff0);
    c.set_reg(AX, d);
    c.set_word(0x115c, d);
    c.call(0xc02e, 0xbee3)?;
    c.run(0xbee3, 1)?;
    c.call(0xbf26, 0xbee6)?;
    c.run(0xbee6, 34)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let v = c.pop();
    c.set_sreg(ES, v);
    c.set_reg(DI, 0xdd11);
    for (at, sh, word) in [(0xa4u16, 1u32, true), (0xa2, 1, true), (0xa8, 4, false), (0xa6, 4, false)] {
        let v = c.word(at);
        c.set_reg(AX, v);
        let v = c.shr1(W, v, sh);
        c.set_reg(AX, v);
        if word {
            let v = c.inc(W, v);
            c.set_reg(AX, v);
        } else {
            let l = c.inc(B, v & 0xff);
            c.set_reg8(r8::AL, l as u8);
        }
        c.stos(B);
    }
    for at in [0xacu16, 0xaa] {
        let v = c.word(at);
        c.set_reg(AX, v);
        c.set_reg8(r8::AL, (v >> 8) as u8);
        let l = c.inc(B, v >> 8);
        c.set_reg8(r8::AL, l as u8);
        c.stos(B);
    }
    c.ret()
}

/// `loc_0380c`: the room's sky ([22E3h] 1, `loc_0388d`) and its palette
/// (`loc_0c2f2` AL): the place [1150h]'s entry from the table 1972h
/// (`loc_05e4f`, BX); in the room shown ([6] not 80h, or the place's
/// own) a light by the room's number, past the entry's +5 the place's
/// own light: 0Ah + ([4] ^ [6] swapped + 1) mod 0Ah ... from 88h for
/// the palace (+0Ah bit 0) or where a troop is (`loc_0407e`,
/// `loc_0b58b`: a cell of kind 10h), else 42h + mod 13h. AL 7Fh mixes
/// in the place's +0 byte.
pub fn sky_380c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::ES;
    entered(start, 0x380c)?;
    let mut c = Ctx::new(m);
    c.run(0x380c, 2)?;
    c.set_byte(0x22e3, 1);
    c.call(0x388d, 0x3814)?;
    c.run(0x3814, 3)?;
    let si = c.word(0x1150);
    c.set_reg(SI, si);
    c.set_reg(AX, 0x1972);
    c.call(0x5e4f, 0x381e)?;
    let ax = c.reg(AX);
    let (dx, w6) = (c.word(4), c.word(6));
    let si = c.reg(SI);
    let p2 = c.word(si.wrapping_add(2));
    let al6 = w6 as u8;
    let ah6 = (w6 >> 8) as u8;
    let lit_path = al6 == 0x80 || dx == p2;
    if lit_path {
        let a = if al6 == 0x80 { 0u8 } else { ah6 };
        let lim = c.byte(ax.wrapping_add(5));
        let in_range = a < lim;
        let n = 6 + if al6 == 0x80 { 0 } else { 2 + 1 } + 2;
        if in_range {
            let b0 = c.byte(ax);
            let v = a.wrapping_add(b0);
            let mix = v == 0x7f;
            c.run(0x381e, n + 3 + if mix { 4 } else { 0 } + 1)?;
            c.set_reg(BX, ax);
            c.set_reg(DX, dx);
            c.set_reg(AX, w6);
            c.cmp(B, al6 as u16, 0x80);
            c.set_reg8(r8::AL, 0);
            if al6 != 0x80 {
                c.cmp(W, dx, p2);
                c.set_reg8(r8::AL, ah6);
            }
            c.cmp(B, a as u16, lim as u16);
            let v = c.add(B, a as u16, b0 as u16);
            c.set_reg8(r8::AL, v as u8);
            c.cmp(B, v, 0x7f);
            if mix {
                let h = c.byte(si);
                c.set_reg8(r8::AH, h);
                let h = c.shr1(B, h as u16, 1);
                c.set_reg8(r8::AH, h as u8);
                let v = c.add(B, v, h);
                let v = c.sub(B, v, 5);
                c.set_reg8(r8::AL, v as u8);
            }
            return c.jump(0xc2f2);
        }
        c.run(0x381e, n)?;
        c.set_reg(BX, ax);
        c.set_reg(DX, dx);
        c.set_reg(AX, w6);
        c.cmp(B, al6 as u16, 0x80);
        c.set_reg8(r8::AL, 0);
        if al6 != 0x80 {
            c.cmp(W, dx, p2);
            c.set_reg8(r8::AL, ah6);
        }
        c.cmp(B, a as u16, lim as u16);
    } else {
        c.run(0x381e, 6 + 2)?;
        c.set_reg(BX, ax);
        c.set_reg(DX, dx);
        c.set_reg(AX, w6);
        c.cmp(B, al6 as u16, 0x80);
        c.set_reg8(r8::AL, 0);
        c.cmp(W, dx, p2);
    }
    // 384Ah: the light's range by the place.
    let di = c.word(0x1150);
    let f = c.byte(di.wrapping_add(0xa));
    let (bx, cx) = if f & 1 != 0 {
        c.run(0x384a, 3 + 2)?;
        c.set_reg(DI, di);
        c.test(B, f as u16, 1);
        (0xa, 0x88)
    } else {
        c.run(0x384a, 3 + 1)?;
        c.set_reg(DI, di);
        c.test(B, f as u16, 1);
        c.call(0x407e, 0x3857)?;
        c.run(0x3857, 1)?;
        c.call(0xb58b, 0x385a)?;
        c.run(0x385a, 2)?;
        let di = c.reg(DI);
        let mut di = c.dec(W, di);
        c.set_reg(DI, di);
        c.set_reg(CX, 4);
        let mut hit = false;
        for k in 0..4u16 {
            let v = c.byte_in(ES, di);
            let found = v & 0x30 == 0x10;
            c.run(0x385e, 5 + if found { 0 } else { 1 })?;
            c.set_reg8(r8::AL, v);
            di = c.inc(W, di);
            c.set_reg(DI, di);
            let a = c.and(B, v as u16, 0x30);
            c.set_reg8(r8::AL, a as u8);
            c.cmp(B, a, 0x10);
            if found {
                hit = true;
                break;
            }
            c.set_reg(CX, 3 - k);
        }
        if hit {
            c.run(0x3872, 2)?;
            (0xa, 0x88)
        } else {
            c.run(0x386a, 3)?;
            (0x13, 0x42)
        }
    };
    c.set_reg(BX, bx);
    c.set_reg(CX, cx);
    c.run(0x3878, 9)?;
    let w6 = c.word(6);
    c.set_reg(AX, w6);
    let a = w6.swap_bytes();
    let w4 = c.word(4);
    let a = c.xor(W, a, w4);
    let a = c.inc(W, a);
    c.set_reg(AX, a);
    c.xor(W, 0, 0);
    c.set_reg(DX, 0);
    c.m.cpu.mul_div(&mut c.m.hw, W, 6, bx as u32);
    let d = c.reg(DX);
    let a = c.add(W, d, cx);
    c.set_reg(AX, a);
    c.jump(0xc2f2)
}

/// `loc_01c46`: a day passes. The days in the same phase [0FFh] (reset
/// when [2Ah] changed, [0FEh] the last); `loc_01d66`, `loc_01e43`; [0D5h]
/// up from 1; the spice: [0A6h] = [0A0h] + the harvest [1172h] - the
/// sold [1170h] (both taken, 0 below), its change against [0AEh] (kept)
/// as a gain [0B2h] or a loss [0B0h]; `loc_01cda`, the totals
/// (`loc_0c02e`, `loc_0bf26`); then with a random word (`loc_0e3cc`)
/// each troop record from 10D8h (11h bytes, below kind 14h) with bit 3
/// of +2 gets, for its five slots without a value but with +9 negative,
/// the next 2 random bits.
pub fn day_1c46(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::SS;
    entered(start, 0x1c46)?;
    let mut c = Ctx::new(m);
    let ph = c.byte(0x2a);
    let last = c.byte(0xfe);
    c.run(0x1c46, 5 + (ph != last) as u32 + 2)?;
    c.set_reg8(r8::AL, ph);
    c.set_reg8(r8::AH, ph);
    c.set_byte(0xfe, ph);
    c.set_reg8(r8::AL, last);
    c.cmp(B, last as u16, ph as u16);
    if ph != last {
        c.set_byte(0xff, 0);
    }
    let v = c.byte(0xff);
    let v = c.inc(B, v as u16);
    c.set_byte(0xff, v as u8);
    c.call(0x1d66, 0x1c5f)?;
    c.run(0x1c5f, 1)?;
    c.call(0x1e43, 0x1c62)?;
    let d = c.byte(0xd5).wrapping_add(1);
    let (harvest, a0, sold, ae) = (c.word(0x1172), c.word(0xa0), c.word(0x1170), c.word(0xae));
    let s = harvest.wrapping_add(a0);
    let (stock, borrow) = s.overflowing_sub(sold);
    let stock = if borrow { 0 } else { stock };
    let (diff, loss) = ae.overflowing_sub(stock);
    c.run(0x1c62, 4 + (d >= 2) as u32 + 7 + borrow as u32 + 5 + if loss { 2 } else { 0 } + 3)?;
    c.set_reg8(r8::AL, d.wrapping_sub(1));
    let a = c.inc(B, d.wrapping_sub(1) as u16);
    c.set_reg8(r8::AL, a as u8);
    c.cmp(B, a, 2);
    if d >= 2 {
        c.set_byte(0xd5, d);
    }
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    c.set_word(0x1172, 0);
    c.set_reg(BX, a0);
    let v = c.add(W, harvest, a0);
    c.set_word(0x1170, a0);
    let v = c.sub(W, v, sold);
    let v = if borrow { c.xor(W, v, v) } else { v };
    c.set_word(0xa6, v);
    c.set_word(0xae, v);
    c.xor(W, 0, 0);
    let r = c.sub(W, ae, v);
    let (gain, lost) = if loss {
        let n = c.neg(W, r);
        (0u16, n)
    } else {
        (r, 0)
    };
    let _ = diff;
    c.set_reg(AX, gain);
    c.set_reg(BX, lost);
    c.set_word(0xb2, gain);
    c.set_word(0xb0, lost);
    c.call(0x1cda, 0x1ca0)?;
    c.run(0x1ca0, 1)?;
    c.call(0xc02e, 0x1ca3)?;
    c.run(0x1ca3, 1)?;
    c.call(0xbf26, 0x1ca6)?;
    c.run(0x1ca6, 1)?;
    c.call(0xe3cc, 0x1ca9)?;
    c.run(0x1ca9, 2)?;
    let mut bx = c.reg(AX);
    c.set_reg(BX, bx);
    let mut si = 0x10d8u16;
    c.set_reg(SI, si);
    loop {
        let f = c.byte(si.wrapping_add(2));
        c.run(0x1cae, 2)?;
        c.test(B, f as u16, 8);
        if f & 8 != 0 {
            c.run(0x1cb4, 1)?;
            c.set_reg(BP, 4);
            let mut bp = 4u16;
            loop {
                let v = c.byte_in(SS, bp.wrapping_add(si).wrapping_add(4));
                let w = c.byte_in(SS, bp.wrapping_add(si).wrapping_add(9));
                let fill = v == 0 && w & 0x80 != 0;
                let n = 2 + if v == 0 { 2 } else { 0 } + if fill { 5 } else { 0 } + 2;
                c.run(0x1cb7, n)?;
                c.cmp(B, v as u16, 0);
                if v == 0 {
                    c.cmp(B, w as u16, 0);
                }
                if fill {
                    bx = c.rol1(W, bx);
                    bx = c.rol1(W, bx);
                    c.set_reg(BX, bx);
                    let a = c.and(B, bx & 0xff, 3);
                    c.set_reg8(r8::AL, a as u8);
                    let p = bp.wrapping_add(si).wrapping_add(4);
                    c.m.cpu.write8(&mut c.m.hw, SS, p, a as u8);
                }
                bp = c.dec(W, bp);
                c.set_reg(BP, bp);
                if bp & 0x8000 != 0 {
                    break;
                }
            }
        }
        si = c.add(W, si, 0x11);
        c.set_reg(SI, si);
        let k = c.byte(si);
        c.run(0x1cd1, 3 + (k >= 0x14) as u32)?;
        c.cmp(B, k as u16, 0x14);
        if k >= 0x14 {
            return c.ret();
        }
    }
}

/// `create_save_cl` (B389): save slot CL ('1' + CL into the name at
/// 38A8h): the image made (`create_save_in_memory`), the day [2] put
/// before it, packed (`compress_sav`) into ES:2, the file named
/// (`loc_0f2fc`), then written (F27Ch, CX + 2 bytes from 0).
pub fn create_save_cl(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{DS, ES};
    entered(start, 0xb389)?;
    let mut c = Ctx::new(m);
    c.run(0xb389, 3)?;
    let cl = c.reg8(r8::CL);
    let cl = c.add(B, cl as u16, 0x31);
    c.set_reg8(r8::CL, cl as u8);
    c.set_byte(0x38af, cl as u8);
    c.call(0xb427, 0xb393)?;
    c.run(0xb393, 8)?;
    let d = c.word(2);
    c.set_reg(AX, d);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.sreg(ES);
    c.push(es);
    let v = c.pop();
    c.set_sreg(DS, v);
    let di = c.reg(DI);
    c.set_reg(SI, di);
    c.xor(W, di, di);
    c.set_reg(DI, 0);
    c.stos(W);
    c.call(0xb4ea, 0xb3a1)?;
    c.run(0xb3a1, 3)?;
    let v = c.pop();
    c.set_sreg(DS, v);
    c.set_reg(DX, 0x38a8);
    c.call(0xf2fc, 0xb3a8)?;
    c.run(0xb3a8, 3)?;
    let di = c.reg(DI);
    c.xor(W, di, di);
    c.set_reg(DI, 0);
    let cx = c.reg(CX);
    let cx = c.add(W, cx, 2);
    c.set_reg(CX, cx);
    c.jump(0xf27c)
}

/// `loc_0a03f`: a dialogue line's effects (`loc_0c85b`, unless the
/// dialogue is being skipped [47B6h]): its action (low nibble of [si],
/// the table at CS:A107h); its sentence noted for the log (with +2 bits
/// 2-3, once: [si] bit 7; the entry (SI - AA78h) / 4 with the speaker
/// [47C4h] * 8 in the high byte, at CS:[11BDh]); [19h] FFh, SI to the
/// next line (or FFFFh when [47A8h] was set). Then, with no talking
/// head and a speaker, the head (`loc_0978e`), the screen `loc_02ebf`
/// while [4774h], and unless [0EAh] is positive the next step: the
/// routine [227Eh] (taken, F66h left) between `loc_09efd` and
/// `loc_0e270`/`loc_0e283`. [28E7h] from [28E8h] unless [0FBh] is
/// negative. CF clear.
pub fn line_a03f(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{CS, SS};
    let _ = SS;
    entered(start, 0xa03f)?;
    let mut c = Ctx::new(m);
    c.run(0xa03f, 1)?;
    c.call(0xc85b, 0xa042)?;
    let skip = c.word(0x47b6);
    if skip != 0 {
        c.run(0xa042, 2)?;
        c.cmp(W, skip, 0);
    } else {
        let si = c.reg(SI);
        let a = c.byte(si) & 0xf;
        c.run(0xa042, 2 + 3 + if a != 0 { 7 } else { 0 })?;
        c.cmp(W, 0, 0);
        let v = c.byte(si);
        c.set_reg8(r8::AL, v);
        let a = c.and(B, v as u16, 0xf);
        c.set_reg8(r8::AL, a as u8);
        if a != 0 {
            let ah = c.reg8(r8::AH);
            c.xor(B, ah as u16, ah as u16);
            c.set_reg8(r8::AH, 0);
            c.push(si);
            let ax = c.dec(W, a);
            let ax = c.shl1(W, ax, 1);
            c.set_reg(AX, ax);
            c.set_reg(BX, 0xa107);
            let bx = c.add(W, 0xa107, ax);
            c.set_reg(BX, bx);
            let to = c.code_word(bx);
            c.call(to, 0xa05d)?;
            c.run(0xa05d, 1)?;
            let v = c.pop();
            c.set_reg(SI, v);
        }
        let si = c.reg(SI);
        let b2 = c.byte(si.wrapping_add(2));
        let b0 = c.byte(si);
        let log = b2 & 0xc != 0 && b0 & 0x80 == 0;
        let flag = c.byte(0x47a8);
        c.run(0xa05e, 3 + if b2 & 0xc != 0 { 2 } else { 0 } + if log { 13 } else { 0 } + 7 + (flag != 0) as u32)?;
        c.set_reg8(r8::AL, b2);
        let a = c.and(B, b2 as u16, 0xc);
        c.set_reg8(r8::AL, a as u8);
        if b2 & 0xc != 0 {
            c.test(B, b0 as u16, 0x80);
        }
        if log {
            let ax = c.sub(W, si, 0xaa78);
            let ax = c.shr1(W, ax, 2);
            let sp = c.byte(0x47c4);
            let bl = c.shl1(B, sp as u16, 3);
            c.set_reg8(r8::BL, bl as u8);
            let ah = c.or(B, ax >> 8, bl);
            let ax = (ah << 8) | (ax & 0xff);
            c.set_reg(AX, ax);
            let bp = c.word(0x11bd);
            c.set_reg(BP, bp);
            c.set_word_in(CS, bp, ax);
            c.set_word_in(CS, bp.wrapping_add(2), 0);
            let v = c.word(0x11bd);
            let v = c.add(W, v, 2);
            c.set_word(0x11bd, v);
        }
        c.set_byte(0x19, 0xff);
        let v = c.or(B, b0 as u16, 0x80);
        c.set_byte(si, v as u8);
        let s = c.add(W, si, 4);
        c.set_reg(SI, s);
        c.xor(B, 0, 0);
        c.set_byte(0x47a8, 0);
        c.set_reg8(r8::AL, flag);
        c.or(B, flag as u16, flag as u16);
        if flag != 0 {
            c.set_reg(SI, 0xffff);
        }
    }
    let head = c.byte(0x46eb);
    let spk = c.word(0x47c4);
    if head != 0 || spk >= 0x10 {
        c.run(0xa0aa, 2 + if head == 0 { 2 } else { 0 })?;
        c.cmp(B, head as u16, 0);
        if head == 0 {
            c.cmp(W, spk, 0x10);
        }
    } else {
        c.run(0xa0aa, 4 + 2)?;
        c.cmp(B, 0, 0);
        c.cmp(W, spk, 0x10);
        let si = c.reg(SI);
        c.push(si);
        c.call(0x978e, 0xa0bc)?;
        let k = c.byte(0x4774);
        c.run(0xa0bc, 3 + if k != 0 { 2 } else { 0 })?;
        let v = c.pop();
        c.set_reg(SI, v);
        c.cmp(B, k as u16, 0);
        if k != 0 {
            let si = c.reg(SI);
            c.push(si);
            c.call(0x2ebf, 0xa0c8)?;
            c.run(0xa0c8, 1)?;
            let v = c.pop();
            c.set_reg(SI, v);
        }
        let e = c.byte(0xea);
        let pos = (e as i8) > 0;
        c.run(0xa0c9, 2 + if pos { 0 } else { 1 })?;
        c.cmp(B, e as u16, 0);
        if !pos {
            c.call(0xe270, 0xa0d3)?;
            c.run(0xa0d3, 1)?;
            c.call(0x9efd, 0xa0d6)?;
            c.run(0xa0d6, 3)?;
            let to = c.word(0x227e);
            c.set_word(0x227e, 0xf66);
            c.set_reg(AX, to);
            c.call(to, 0xa0df)?;
            c.run(0xa0df, 1)?;
            c.call(0xe283, 0xa0e2)?;
        }
    }
    let f = c.byte(0xfb);
    c.run(0xa0e2, 2 + if f & 0x80 != 0 { 0 } else { 2 } + 2)?;
    c.cmp(B, f as u16, 0);
    if f & 0x80 == 0 {
        let v = c.byte(0x28e8);
        c.set_reg8(r8::AL, v);
        c.set_byte(0x28e7, v);
    }
    c.set_flag(CF, false);
    c.ret()
}

/// `loc_079ee`: the panel of place SI ([46EFh]): its frame 18E9h (for an
/// unvisited place, `loc_06917` ZF, sized by its height and the map
/// centre [4710h] set) as the overlay [0DBE0h] (`loc_07b0f` AL 2); the
/// text box from the frame (2244h; margins 0, 5, 0, 1; the picture box
/// 18F3h 3Dh square), `loc_07b1b`. Then the speaker: the troop leader
/// (0Ch) for a sietch (+3 bit 5) whose people are not all gone
/// (+0Ah bit 1, or `loc_05d36` CF clear), else the place's own (0Fh,
/// [4758h] SI, [476Ch] 0, by the head [22A6h] and pose [47D0h]); its
/// head set up (`loc_091a0`, `loc_0c0f4`), its colours [46D2h]/[46D4h]
/// from 22B9h, its face drawn in 47D4h (the sprite list [47CAh],
/// `loc_09d6a`), the rect shown (`loc_0c4aa`), then `open_onmap_resource`.
pub fn place_panel_79ee(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{CF as CARRY, SS, ZF};
    entered(start, 0x79ee)?;
    let mut c = Ctx::new(m);
    c.run(0x79ee, 2)?;
    let si = c.reg(SI);
    c.set_word(0x46ef, si);
    c.call(0x6917, 0x79f5)?;
    let z = c.flag(ZF);
    if z {
        let di = c.reg(DI);
        let h = c.word(di.wrapping_add(2));
        let low = (h as i16) < 0x4c;
        c.run(0x79f5, 2 + 4 + if low { 2 } else { 0 } + 5 + 3)?;
        c.set_reg(SI, 0x18e9);
        c.set_reg(AX, 0x1e);
        c.set_reg(BX, 5);
        c.cmp(W, h, 0x4c);
        let (ax, bx) = if low { (0xe, 0x50) } else { (0x1e, 5) };
        c.set_reg(AX, ax);
        c.set_word(0x18eb, bx);
        let b = c.add(W, bx, 0x43);
        c.set_reg(BX, b);
        c.set_word(0x18ef, b);
        c.set_word(0x4710, 0x5c);
        c.set_word(0x4712, ax);
    } else {
        c.run(0x79f5, 2 + 3)?;
        c.set_reg(SI, 0x18e9);
    }
    c.set_word(0xdbe0, 0x18e9);
    c.set_reg8(r8::AL, 2);
    c.call(0x7b0f, 0x7a27)?;
    c.run(0x7a27, 23)?;
    c.set_reg(DI, 0x18e9);
    c.set_reg(SI, 0x18f3);
    let dx = c.word(0x18e9);
    let bx = c.word(0x18eb);
    c.set_reg(AX, dx);
    let a = c.add(W, dx, 0x49);
    c.set_reg(AX, a);
    c.set_word(0x2244, a);
    let bx = c.add(W, bx, 3);
    c.set_reg(BX, bx);
    c.set_word(0x2246, bx);
    c.xor(W, 0, 0);
    c.set_word(0x4784, 0);
    c.set_word(0x4786, 5);
    c.set_word(0x4788, 0);
    let a = c.inc(W, 0);
    c.set_reg(AX, a);
    c.set_word(0x478a, 1);
    let dx = c.add(W, dx, 4);
    c.set_word(0x18f3, dx);
    c.set_word(0x18f5, bx);
    let dx = c.add(W, dx, 0x3d);
    c.set_reg(DX, dx);
    let bx = c.add(W, bx, 0x3d);
    c.set_reg(BX, bx);
    c.set_word(0x18f7, dx);
    c.set_word(0x18f9, bx);
    c.call(0x7b1b, 0x7a6a)?;
    let si = c.word(0x46ef);
    let k = c.byte(si.wrapping_add(3));
    let sietch = k & 0x20 != 0;
    let mut troop = false;
    if sietch {
        let di = c.word(si.wrapping_add(4));
        let f = c.byte(di.wrapping_add(0xa));
        if f & 2 != 0 {
            c.run(0x7a6a, 3 + 3)?;
            c.set_reg(SI, si);
            c.test(B, k as u16, 0x20);
            c.set_reg(DI, di);
            c.test(B, f as u16, 2);
            troop = true;
        } else {
            c.run(0x7a6a, 3 + 3 + 1)?;
            c.set_reg(SI, si);
            c.test(B, k as u16, 0x20);
            c.set_reg(DI, di);
            c.test(B, f as u16, 2);
            c.call(0x5d36, 0x7a80)?;
            let cf = c.flag(CARRY);
            c.run(0x7a80, 1)?;
            troop = !cf;
        }
    } else {
        c.run(0x7a6a, 3)?;
        c.set_reg(SI, si);
        c.test(B, k as u16, 0x20);
    }
    let (ax, bp) = if troop {
        c.run(0x7a82, 3)?;
        c.set_reg(AX, 0xc);
        c.set_word(0x47c4, 0xc);
        c.call(0x91a0, 0x7a8b)?;
        c.run(0x7a8b, 1)?;
        c.call(0xc0f4, 0x7a8e)?;
        c.run(0x7a8e, 3)?;
        c.set_reg(AX, 0xc);
        c.set_reg(BP, 0);
        (0xcu16, 0u16)
    } else {
        c.run(0x7a96, 5)?;
        c.set_reg(AX, 0xf);
        c.set_word(0x47c4, 0xf);
        let si = c.reg(SI);
        c.set_word(0x4758, si);
        c.set_byte(0x476c, 0);
        c.call(0x91a0, 0x7aa8)?;
        c.run(0x7aa8, 1)?;
        c.call(0xc0f4, 0x7aab)?;
        c.run(0x7aab, 9)?;
        let h = c.word(0x22a6);
        let a = c.sub(W, h, 0xe);
        let a = c.shl1(W, a, 2);
        c.set_reg(AX, a);
        let p = c.byte(0x47d0);
        let b = c.dec(B, p as u16);
        c.set_reg8(r8::BL, b as u8);
        c.xor(B, 0, 0);
        let bx = c.shl1(W, b, 1);
        c.set_reg(BX, bx);
        c.set_reg(BP, bx);
        (a, bx)
    };
    c.run(0x7ac1, 21)?;
    let s = c.add(W, 0x22b9, ax);
    c.set_reg(SI, s);
    let a = c.lods(W, pc::cpu::DS);
    c.set_word(0x46d2, a);
    let a = c.lods(W, pc::cpu::DS);
    c.set_word(0x46d4, a);
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let base = c.word_in(SS, 0x47ca);
    c.set_reg(SI, base);
    let seg = c.word_in(SS, 0xdbb2);
    c.set_sreg(pc::cpu::DS, seg);
    let off = c.word_at(seg, bp.wrapping_add(base));
    let s = c.add(W, base, off);
    c.set_reg(SI, s);
    let dx = c.word_in(SS, 0x18f3);
    let bx = c.word_in(SS, 0x18f5);
    let bx = c.inc(W, bx);
    let dx = c.inc(W, dx);
    c.set_word_in(SS, 0x47d4, dx);
    c.set_word_in(SS, 0x47d6, bx);
    let dx = c.add(W, dx, 0x3b);
    c.set_reg(DX, dx);
    let bx = c.add(W, bx, 0x3b);
    c.set_reg(BX, bx);
    c.set_word_in(SS, 0x47d8, dx);
    c.set_word_in(SS, 0x47da, bx);
    c.call(0x9d6a, 0x7b05)?;
    c.run(0x7b05, 3)?;
    let v = c.pop();
    c.set_sreg(pc::cpu::DS, v);
    c.set_reg(SI, 0x47d4);
    c.call(0xc4aa, 0x7b0c)?;
    c.run(0x7b0c, 1)?;
    c.jump(0xc13b)
}
