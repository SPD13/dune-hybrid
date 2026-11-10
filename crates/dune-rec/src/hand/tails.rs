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
