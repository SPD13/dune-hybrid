//! Drawing sprites (plain and scaled), UI elements, the screen update
//! with the talking head, and small string helpers.

use super::pure;
use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CS, CX, DI, DS, DX, ES, SI, SS},
    rec::Bail,
};

mod code {
    pub const SET_SCREEN_AS_ACTIVE_FRAMEBUFFER: u16 = 0xc08e;
    pub const DRAW_SPRITE_CLOBBERING_BX_DX: u16 = 0xc22f;
    pub const UNRLE_IMAGE_TO_TMP_BUFFER: u16 = 0xc2a1;
    pub const CALL_RESTORE_CURSOR: u16 = 0xdbb2;
    pub const DRAW_HEAD: u16 = 0x1797;
}

/// A colour that replaces a sprite's (0: none), in the code segment.
const SPRITE_COLOUR_OVERRIDE: u16 = 0xc21a;
/// The scale factors (divisors), by the sprite's scale bits.
const SCALE_DIVISORS: u16 = 0x2774;

/// `load_icones_sprites` (C137): opens resource 0, the icons.
pub fn load_icones_sprites(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc137)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let ax = c.reg(AX);
    let zero = c.xor(W, ax, ax);
    c.set_reg(AX, zero);
    super::resources::open(&mut c)
}

/// `draw_sprite_clobbering_bx_dx` (C22F): sprite AX & 1FFh of the loaded
/// sheet at (DX, BX) into the active framebuffer (driver function 5); AX
/// bits 13..14 flip it, bits 10..12 scale it down (function 32, by the
/// divisor for the scale, from the image unpacked to a buffer first when
/// it is packed). A colour override at CS:C21A replaces the sprite's
/// palette byte. Exit: DS = SS.
pub fn draw_sprite_clobbering_bx_dx(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc22f)?;
    let mut c = Ctx::new(m);
    let ax = c.reg(AX);
    let colour = c.m.cpu.read8(&mut c.m.hw, CS, SPRITE_COLOUR_OVERRIDE);
    c.run(start, 16 + if colour != 0 { 1 } else { 0 } + 3)?;
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.set_sreg(ES, fb);
    let (sheet, seg) = (c.word(g::SPRITE_SHEET), c.word(g::SPRITE_SHEET + 2));
    c.set_reg(SI, sheet);
    c.set_sreg(DS, seg);
    let index = c.and(W, ax, 0x1ff);
    let index = c.shl1(W, index, 1);
    c.set_reg(BP, index);
    let off = c.word_in(DS, index.wrapping_add(sheet));
    let si = c.add(W, sheet, off);
    c.set_reg(SI, si);
    c.set_reg(CX, ax);
    c.push(ax);
    let width = c.lods(W, DS);
    let flip = c.and(B, ax >> 8, 0x60);
    c.set_reg8(r8::CH, flip as u8);
    let high = c.or(B, width >> 8, flip);
    let di = high << 8 | width & 0xff;
    c.set_reg(AX, di);
    c.set_reg(DI, di);
    let lines = c.lods(W, DS);
    c.set_reg(CX, lines);
    c.cmp(B, colour as u16, 0);
    if colour != 0 {
        c.set_reg8(r8::CH, colour);
    }
    let ax = c.pop();
    let scale = c.and(W, ax, 0x1c00);
    c.set_reg(AX, scale);
    if scale == 0 {
        c.interp(0xc263, 0xc268)?;
        return ds_from_ss(&mut c);
    }

    // C26B: scaled, by the divisor for the scale bits.
    let slot = (scale >> 8 | scale << 8) >> 1;
    let divisor = c.word_in(SS, slot.wrapping_add(SCALE_DIVISORS));
    let cx = c.reg(CX);
    let (w, h) = ((di & 0x1ff).rotate_left(8), (cx & 0xff) << 8);
    if divisor == 0 || (w as u32) / (divisor as u32) > 0xffff || (h as u32) / (divisor as u32) > 0xffff {
        c.m.cpu.ip = 0xc26b; // the division faults: the interpreter's
        return Err(Bail);
    }
    let packed = di & 0x8000 != 0;
    c.run(0xc26b, 23 + packed as u32)?;
    let ax = scale.rotate_left(8);
    c.set_reg(AX, ax);
    let bp = c.shr1(W, ax, 1);
    c.set_reg(BP, bp);
    c.set_reg(BP, divisor);
    let a = c.and(W, di, 0x1ff);
    let dx = c.reg(DX);
    c.push(dx);
    c.set_reg(AX, a.rotate_left(8));
    let zero = c.xor(W, dx, dx);
    c.set_reg(DX, zero);
    c.m.cpu.mul_div(&mut c.m.hw, W, 6, divisor as u32); // div bp: the width
    let dx = c.pop();
    c.set_reg(DX, dx);
    let width = c.reg(AX);
    c.push(width);
    let ah0 = c.xor(B, 0, 0);
    c.set_reg(AX, (cx & 0xff) << 8 | ah0);
    c.push(dx);
    let zero = c.xor(W, dx, dx);
    c.set_reg(DX, zero);
    c.m.cpu.mul_div(&mut c.m.hw, W, 6, divisor as u32); // div bp: the height
    let dx = c.pop();
    c.set_reg(DX, dx);
    let al = c.reg8(r8::AL);
    c.set_reg8(r8::CL, al);
    let width = c.pop();
    c.set_reg(AX, width);
    c.or(W, di, di);
    if packed {
        c.call(code::UNRLE_IMAGE_TO_TMP_BUFFER, 0xc299)?;
    }
    c.interp(0xc299, 0xc29e)?;
    ds_from_ss(&mut c)
}

/// `push ss; pop ds; ret`, counted here.
fn ds_from_ss(c: &mut Ctx) -> Result<(), Bail> {
    let at = c.m.cpu.ip; // where `interp` came back
    c.run(at, 3)?;
    let ss = c.sreg(SS);
    c.push(ss);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}

/// `draw_ui_element` (D200): the UI element at SI on the screen: its
/// rectangle filled first (driver function 9) with bit 40h of its flags
/// (+8), then its sprite (+0Ah, none for FFFFh) at its position, unless
/// bit 20h. SI and the framebuffer kept.
pub fn draw_ui_element(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd200)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.push(fb);
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0xd207)?;
    let si = c.reg(SI);
    let flags = c.byte(si.wrapping_add(8));
    if flags & 0x40 != 0 {
        c.run(0xd207, 5)?;
        c.push(si);
        c.test(B, flags as u16, 0x40);
        let fb = c.word(g::FRAMEBUFFER_ACTIVE);
        c.set_sreg(ES, fb);
        c.push(si);
        c.interp(0xd213, 0xd217)?;
        c.run(0xd217, 1)?;
        let si = c.pop();
        c.set_reg(SI, si);
    } else {
        c.run(0xd207, 3)?;
        c.push(si);
        c.test(B, flags as u16, 0x40);
    }
    let si = c.reg(SI);
    let flags = c.byte(si.wrapping_add(8));
    let sprite = c.word(si.wrapping_add(0x0a));
    let n = if flags & 0x20 != 0 {
        2
    } else if sprite == 0xffff {
        14
    } else {
        16
    };
    c.run(0xd218, n + if n == 16 { 0 } else { 3 })?;
    c.test(B, flags as u16, 0x20);
    if flags & 0x20 == 0 {
        for r in [DX, BX, DI, CX] {
            let v = c.lods(W, DS);
            c.set_reg(r, v);
        }
        c.lods(W, DS);
        let s = c.lods(W, DS);
        let ax = c.inc(W, s);
        c.set_reg(AX, ax);
        if s != 0xffff {
            let ax = c.dec(W, ax);
            c.set_reg(AX, ax);
            c.call(code::DRAW_SPRITE_CLOBBERING_BX_DX, 0xd233)?;
            c.run(0xd233, 3)?;
        }
    }
    let si = c.pop();
    c.set_reg(SI, si);
    let fb = c.pop();
    c.set_word(g::FRAMEBUFFER_ACTIVE, fb);
    c.ret()
}

/// From C4FB: the rectangle (DX, BX) to (BP, AX) to the screen
/// (`particles_update_screen_rect`), the talking head redrawn first
/// (`sub_1797`) outside the map view when the rectangle reaches it (from
/// x < C2h, to x ≥ 7Eh, y ≥ 89h).
fn update_with_head(c: &mut Ctx, at: u16) -> Result<(), Bail> {
    let view = c.byte(g::MAP_VIEW_FLAG);
    let (dx, bp, ax) = (c.reg(DX), c.reg(BP), c.reg(AX));
    let lt = |a: u16, b: u16| (a as i16) < (b as i16);
    let n = if view != 0 {
        2
    } else if lt(ax, 0x89) {
        4
    } else if lt(bp, 0x7e) {
        6
    } else if !lt(dx, 0xc2) {
        8
    } else {
        13
    };
    c.run(at, n)?;
    c.cmp(B, view as u16, 0);
    if n >= 4 {
        c.cmp(W, ax, 0x89);
    }
    if n >= 6 {
        c.cmp(W, bp, 0x7e);
    }
    if n >= 8 {
        c.cmp(W, dx, 0xc2);
    }
    if n == 13 {
        for r in [AX, BX, DX, BP] {
            let v = c.reg(r);
            c.push(v);
        }
        c.call(code::DRAW_HEAD, 0xc51a)?;
        c.run(0xc51a, 4)?;
        for r in [BP, DX, BX, AX] {
            let v = c.pop();
            c.set_reg(r, v);
        }
    }
    super::screen::particles_update_screen_rect(c.m, 0xc51e)
}

/// Loads the rectangle at SI (C4F0; `rect` sets SI first, in `extra`
/// instructions from `at`) and goes on at C4FB.
fn update_rect_at_si(c: &mut Ctx, at: u16, extra: u32, rect: Option<u16>) -> Result<(), Bail> {
    c.run(at, extra + 4)?;
    if let Some(r) = rect {
        c.set_reg(SI, r);
    }
    let si = c.reg(SI);
    let (l, t, r, b) = (c.word(si), c.word(si + 2), c.word(si + 4), c.word(si + 6));
    c.set_reg(DX, l);
    c.set_reg(BX, t);
    c.set_reg(BP, r);
    c.set_reg(AX, b);
    update_with_head(c, 0xc4fb)
}

/// `draw_head_if_needed_and_update_screen_rect` (C4FB).
pub fn draw_head_if_needed_and_update_screen_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc4fb)?;
    update_with_head(&mut Ctx::new(m), 0xc4fb)
}

/// `draw_head_if_needed_and_update_screen_rect_at_si` (C4F0).
pub fn draw_head_if_needed_and_update_screen_rect_at_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc4f0)?;
    update_rect_at_si(&mut Ctx::new(m), 0xc4f0, 0, None)
}

/// `update_screen_at_sprite_rect_updating_head` (C4ED): the clip
/// rectangle.
pub fn update_screen_at_sprite_rect_updating_head(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc4ed)?;
    update_rect_at_si(&mut Ctx::new(m), 0xc4ed, 1, Some(g::RECT_D834))
}

/// `loc_0c4dd`: the game area to the screen (with the talking head),
/// the cursor hidden first unless it is below the game area.
pub fn sub_c4dd(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc4dd)?;
    let mut c = Ctx::new(m);
    let y = c.word(g::MOUSE_Y);
    let above = y < 0x98;
    c.run(start, 3 + above as u32)?;
    c.set_reg(AX, y);
    c.cmp(W, y, 0x98);
    if above {
        c.call(code::CALL_RESTORE_CURSOR, 0xc4e8)?;
    }
    update_rect_at_si(&mut c, 0xc4e8, 2, Some(g::GAME_AREA_RECT))
}

/// `res_store_in_lookup_table` (E75B): a lookup entry at ES:DI: AX, DL,
/// then 2 + 1 bytes from SI + 10h, and 4 from SI + 14h. SI kept.
pub fn res_store_in_lookup_table(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe75b)?;
    let mut c = Ctx::new(m);
    c.run(start, 12)?;
    let si = c.reg(SI);
    c.push(si);
    c.stos(W);
    let dl = c.reg8(r8::DL);
    c.set_reg8(r8::AL, dl);
    c.stos(B);
    let si = pure::add(W, si, 0x10);
    c.set_reg(SI, si);
    c.movs(W);
    c.movs(B);
    let si = c.reg(SI);
    let si = pure::inc(W, si);
    c.set_reg(SI, si);
    c.movs(W);
    c.movs(W);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `string_update_ordinal_suffix` (BFA7): the ordinal suffix for the
/// number ending at ES:SI ("st", "nd", "rd", "th" in the language's table
/// at 251Ah, 8 bytes per language), written at ES:SI. Teens and units
/// from 4 take the first; in the table at 2522h, a 1 after a space too.
pub fn string_update_ordinal_suffix(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbfa7)?;
    let mut c = Ctx::new(m);
    c.check(start, 25)?;
    let language = c.byte(g::LANGUAGE);
    c.set_reg8(r8::BL, language);
    let bx = pure::shl1(W, language as u16, 3);
    let mut table = pure::add(W, bx, 0x251a);
    let si = c.reg(SI);
    let two = c.word_in(ES, si.wrapping_sub(2));
    let (tens, units) = ((two & 0xff) as u8, two >> 8);
    let units = pure::and(B, units, 0x0f);
    c.set_reg(AX, (tens as u16) << 8 | units);
    let mut n = 11;
    let mut indexed = false;
    if tens != 0x31 {
        n += 2;
        if units < 4 {
            n += 2;
            if units != 1 {
                indexed = true;
            } else {
                n += 2;
                if table != 0x2522 {
                    indexed = true;
                } else {
                    n += 2;
                    indexed = tens == 0x20;
                }
            }
        }
    }
    if indexed {
        let ah = pure::xor(B, 0, 0);
        let ax = pure::shl1(W, units | ah << 8, 1);
        c.set_reg(AX, ax);
        table = pure::add(W, table, ax);
        n += 3;
    }
    c.set_reg(BX, table);
    let suffix = c.word(table);
    c.set_reg(AX, suffix);
    c.set_word_in(ES, si, suffix);
    c.count(n + 3);
    c.ret()
}

/// `blit_repeated_x`: sprite AX of the current sheet ([0DBB0h]) tiled
/// over the rectangle at SI (x0, y0, x1, y1): clipped by the driver
/// (far [38D9h]), the first copy drawn (far [38C9h]), then copies to
/// the right along the first row and, for each further row, down
/// (far [3931h], the last ones cut to the rectangle), through a frame
/// on the stack (x, y, width, height, x, y; then the sprite's height
/// and width, the rectangle's height, width, y and x). DS kept.
pub fn blit_repeated_x(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::SP;
    entered(start, 0xc370)?;
    let mut c = Ctx::new(m);
    c.run(0xc370, 4)?;
    let ds = c.sreg(DS);
    c.push(ds);
    c.push(ds);
    let ax = c.reg(AX);
    c.push(ax);
    let si = c.reg(SI);
    c.push(si);
    c.interp(0xc374, 0xc378)?;
    c.run(0xc378, 25)?;
    let si = c.pop();
    c.set_reg(SI, si);
    let ax = c.pop();
    c.set_reg(AX, ax);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let x = c.word(si);
    c.set_reg(DX, x);
    c.push(x);
    let y = c.word(si.wrapping_add(2));
    c.set_reg(BX, y);
    c.push(y);
    let x2 = c.word(si.wrapping_add(4));
    let w = c.sub(W, x2, x);
    c.set_reg(CX, w);
    c.push(w);
    let y2 = c.word(si.wrapping_add(6));
    let h = c.sub(W, y2, y);
    c.set_reg(CX, h);
    c.push(h);
    let (s0, sg) = (c.word(0xdbb0), c.word(0xdbb2));
    c.set_reg(SI, s0);
    c.set_sreg(DS, sg);
    let bp = c.shl1(W, ax, 1);
    c.set_reg(BP, bp);
    let o = c.word(bp.wrapping_add(s0));
    let s1 = c.add(W, s0, o);
    c.set_reg(SI, s1);
    let a = c.lods(W, DS);
    c.set_reg(DI, a);
    let sw = c.and(W, a, 0x1ff);
    c.set_reg(AX, sw);
    c.push(sw);
    let a = c.lods(W, DS);
    c.set_reg(CX, a);
    c.xor(B, a >> 8, a >> 8);
    let sh = a & 0xff;
    c.set_reg(AX, sh);
    c.push(sh);
    c.interp(0xc3a6, 0xc3ab)?;
    c.run(0xc3ab, 14)?;
    let es = c.sreg(ES);
    c.push(es);
    let v = c.pop();
    c.set_sreg(DS, v);
    let sp = c.reg(SP);
    c.set_reg(BP, sp);
    let x = c.word_in(SS, sp.wrapping_add(0xa));
    c.set_reg(DX, x);
    let y = c.word_in(SS, sp.wrapping_add(8));
    c.set_reg(BX, y);
    let f0 = c.word_in(SS, sp);
    let f2 = c.word_in(SS, sp.wrapping_add(2));
    for v in [y, x, f0, f2, y, x] {
        c.push(v);
    }
    let bp = c.reg(SP);
    c.set_reg(BP, bp);
    let w = c.word_in(SS, bp.wrapping_add(0x12));
    let t = c.word_in(SS, bp.wrapping_add(4));
    let dx = c.sub(W, w, t);
    c.set_reg(DX, dx);
    tile_run(&mut c, DX, 4, 8, 0xc3c7)?;
    c.run(0xc3e0, 7)?;
    let bp = c.reg(BP);
    let w = c.word_in(SS, bp.wrapping_add(0x12));
    c.set_reg(AX, w);
    c.set_word_in(SS, bp.wrapping_add(4), w);
    let x = c.word_in(SS, bp);
    c.set_reg(AX, x);
    c.set_word_in(SS, bp.wrapping_add(8), x);
    let h = c.word_in(SS, bp.wrapping_add(0x10));
    let t = c.word_in(SS, bp.wrapping_add(6));
    let bx = c.sub(W, h, t);
    c.set_reg(BX, bx);
    if bx != 0 {
        tile_run(&mut c, BX, 6, 0xa, 0xc3f4)?;
    }
    c.run(0xc40d, 3)?;
    let sp = c.reg(SP);
    let s = c.add(W, sp, 0x18);
    c.set_reg(SP, s);
    let v = c.pop();
    c.set_sreg(DS, v);
    c.ret()
}

/// `blit_repeated_x`'s loops (C3C7h / C3F4h): while the register `r`
/// (the length left) stays positive, the frame's position [bp+pos]
/// stepped by the size [bp+size] (cut to what is left) and a copy
/// drawn (far [3931h]).
fn tile_run(c: &mut Ctx, r: usize, size: u16, pos: u16, at: u16) -> Result<(), Bail> {
    loop {
        let bp = c.reg(BP);
        let left = c.reg(r);
        let a = c.word_in(SS, bp.wrapping_add(size));
        let borrow = left < a;
        c.run(at, 5 + borrow as u32)?;
        c.set_reg(AX, a);
        let v = c.word_in(SS, bp.wrapping_add(pos));
        let v = c.add(W, v, a);
        c.set_word_in(SS, bp.wrapping_add(pos), v);
        let left = c.sub(W, left, a);
        c.set_reg(r, left);
        if borrow {
            let v = c.add(W, a, left);
            c.set_word_in(SS, bp.wrapping_add(size), v);
        }
        c.push(left);
        let call = at + 0xe;
        c.interp(call, call + 5)?;
        c.run(call + 5, 3)?;
        let left = c.pop();
        c.set_reg(r, left);
        c.cmp(W, left, 0);
        if left as i16 <= 0 {
            return Ok(());
        }
    }
}
