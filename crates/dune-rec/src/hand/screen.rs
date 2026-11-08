//! Screen updates through the graphics driver: rectangles between the
//! framebuffers, transitions, clipped sprites.
//!
//! A rectangle is 4 words: left, top, right, bottom. Empty ones (right
//! not after left, or bottom not after top) are skipped.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CX, DI, DS, DX, ES, SI, SS},
    rec::Bail,
};

mod code {
    pub const GFX_CALL_BP_WITH_FRONT_BUFFER_AS_SCREEN: u16 = 0xc097;
    pub const GFX_COPY_WHOLE_FRAMEBUF_TO_SCREEN: u16 = 0xc4cd;
    pub const GET_SUBRESOURCE_POINTER: u16 = 0xc1f4;
    pub const DRAW_SPRITE_CLIPPED_CLOBBERING: u16 = 0xc30d;
    /// Clears the map's click handler when this is it (`particles_...`).
    pub const NO_PARTICLES_HANDLER: u16 = 0x1ad6;
}

/// The timer the transitions pace themselves by.
const TRANSITION_TIMER: u16 = 0xce7a;

/// Loads the rectangle at SI: DX, BX = left, top; BP, AX = its width and
/// height (`sub`, flags from the last). Returns whether it is empty, and
/// the instructions run (4 loads, then 2 or 4).
fn load_rect(c: &mut Ctx, si: u16) -> (bool, u32) {
    let (l, t, r, b) = (c.word(si), c.word(si + 2), c.word(si + 4), c.word(si + 6));
    c.set_reg(DX, l);
    c.set_reg(BX, t);
    let width = c.sub(W, r, l);
    c.set_reg(BP, width);
    c.set_reg(AX, b);
    if r <= l {
        return (true, 6);
    }
    let height = c.sub(W, b, t);
    c.set_reg(AX, height);
    (b <= t, 8)
}

/// Whether the rectangle at SI is empty, and the instructions
/// [`load_rect`] runs on it, without changing anything.
fn rect_shape(c: &mut Ctx, si: u16) -> (bool, u32) {
    let (l, t, r, b) = (c.word(si), c.word(si + 2), c.word(si + 4), c.word(si + 6));
    if r <= l { (true, 6) } else { (b <= t, 8) }
}

/// C446: copies the rectangle at SI from the second framebuffer to the
/// front buffer (driver function 14). CX kept.
fn copy_rect_fb2_to_fb1(c: &mut Ctx) -> Result<(), Bail> {
    copy_rect_to_fb1(c, 0xc446, 1, g::FRAMEBUFFER_2)
}

/// The rectangle at SI from the buffer at `from` to the front buffer; the
/// stretch starts at `at`, `prefix` instructions before C449.
fn copy_rect_to_fb1(c: &mut Ctx, at: u16, prefix: u32, from: u16) -> Result<(), Bail> {
    let si = c.reg(SI);
    let (empty, n) = rect_shape(c, si);
    c.run(at, prefix + 2 + n + if empty { 2 } else { 4 })?;
    let fb2 = c.word(from);
    c.set_reg(AX, fb2);
    let cx = c.reg(CX);
    c.push(cx);
    c.set_reg(CX, fb2);
    let (empty, _) = load_rect(c, si);
    if !empty {
        let front = c.word(g::FRAMEBUFFER_FRONT);
        c.set_sreg(ES, front);
        c.push(si);
        let ds = c.sreg(DS);
        c.push(ds);
        c.set_reg(SI, fb2);
        c.interp(0xc467, 0xc46b)?;
        c.run(0xc46b, 4)?;
        let ds = c.pop();
        c.set_sreg(DS, ds);
        let si = c.pop();
        c.set_reg(SI, si);
    }
    let cx = c.pop();
    c.set_reg(CX, cx);
    c.ret()
}

/// `copy_rect_fb2_to_fb1` (C446).
pub fn copy_rect_fb2_to_fb1_at_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc446)?;
    copy_rect_fb2_to_fb1(&mut Ctx::new(m))
}

/// `loc_0c46f`: the rectangle at SI from the back buffer to the front
/// buffer.
pub fn copy_rect_backbuffer_to_fb1(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc46f)?;
    copy_rect_to_fb1(&mut Ctx::new(m), 0xc46f, 2, g::BACKBUFFER)
}

/// `copy_game_area_to_screen_fb2_to_fb1` (C43E): the game area.
pub fn copy_game_area_to_screen_fb2_to_fb1(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc43e)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?; // mov si; jmp
    c.set_reg(SI, g::GAME_AREA_RECT);
    copy_rect_fb2_to_fb1(&mut c)
}

/// `copy_clip_rect_to_screen_from_fb2` (C443): the clip rectangle.
pub fn copy_clip_rect_to_screen_from_fb2(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc443)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.set_reg(SI, g::RECT_D834);
    copy_rect_fb2_to_fb1(&mut c)
}

/// C477: copies the rectangle at SI from the front buffer to the second
/// framebuffer (driver function 12).
fn copy_rect_fb1_to_fb2(c: &mut Ctx) -> Result<(), Bail> {
    let si = c.reg(SI);
    let (empty, n) = rect_shape(c, si);
    c.run(0xc477, n + if empty { 1 } else { 3 })?;
    let (empty, _) = load_rect(c, si);
    if empty {
        return c.ret();
    }
    let fb2 = c.word(g::FRAMEBUFFER_2);
    c.set_sreg(ES, fb2);
    let ds = c.sreg(DS);
    c.push(ds);
    let front = c.word(g::FRAMEBUFFER_FRONT);
    c.set_sreg(DS, front);
    c.interp(0xc493, 0xc498)?;
    c.run(0xc498, 2)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}

/// `gfx_copy_rect_fb1_to_fb2` (C477).
pub fn gfx_copy_rect_fb1_to_fb2(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc477)?;
    copy_rect_fb1_to_fb2(&mut Ctx::new(m))
}

/// `copy_game_rect_from_fb2_to_fb1` (C474): despite the name, the game
/// area from the front buffer to the second framebuffer.
pub fn copy_game_rect_from_fb2_to_fb1(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc474)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.set_reg(SI, g::GAME_AREA_RECT);
    copy_rect_fb1_to_fb2(&mut c)
}

/// The tail of C4AA and C51E: copies the loaded rectangle from the screen
/// buffer to the front buffer (driver function 29), `push ds` first.
fn rect_to_screen(c: &mut Ctx, call_at: u16, front_first: bool) -> Result<(), Bail> {
    let ds = c.sreg(DS);
    c.push(ds);
    let (front, screen) = (c.word(g::FRAMEBUFFER_FRONT), c.word(g::SCREEN_BUFFER));
    // C4AA loads ES = front, DS = screen; C51E ES = screen, DS = front.
    let (es, ds) = if front_first { (front, screen) } else { (screen, front) };
    c.set_sreg(ES, es);
    c.set_sreg(DS, ds);
    c.interp(call_at, call_at + 5)?;
    c.run(call_at + 5, 2)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}

/// `gfx_copy_rect_to_screen` (C4AA): the rectangle at SI, from the screen
/// buffer to the front buffer.
pub fn gfx_copy_rect_to_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc4aa)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let (empty, n) = rect_shape(&mut c, si);
    c.run(start, n + if empty { 1 } else { 3 })?;
    let (empty, _) = load_rect(&mut c, si);
    if empty {
        return c.ret();
    }
    rect_to_screen(&mut c, 0xc4c6, true)
}

/// `particles_update_screen_rect` (C51E): the rectangle (DX, BX) to (BP,
/// AX), from the front buffer to the screen buffer, unless the map's click
/// handler is 1AD6h.
pub fn particles_update_screen_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc51e)?;
    let mut c = Ctx::new(m);
    let (l, t, r, b) = (c.reg(DX), c.reg(BX), c.reg(BP), c.reg(AX));
    let handler = c.word(g::MAP_CLICK_HANDLER);
    let n = if r <= l {
        3
    } else if b <= t {
        5
    } else if handler == code::NO_PARTICLES_HANDLER {
        7
    } else {
        9
    };
    c.run(start, n)?;
    let width = c.sub(W, r, l);
    c.set_reg(BP, width);
    if r <= l {
        return c.ret();
    }
    let height = c.sub(W, b, t);
    c.set_reg(AX, height);
    if b <= t {
        return c.ret();
    }
    c.cmp(W, handler, code::NO_PARTICLES_HANDLER);
    if handler == code::NO_PARTICLES_HANDLER {
        return c.ret();
    }
    rect_to_screen(&mut c, 0xc537, false)
}

/// From C108: the transition AL from the front buffer to the screen
/// (driver function 27, paced by the timer), then the screen palette.
fn transition(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xc108, 4)?;
    c.set_byte(g::TRANSITION_ACTIVE, 0x80);
    let (ax, dx) = (c.reg(AX), c.reg(DX));
    c.push(ax);
    c.push(dx);
    c.call(code::GFX_CALL_BP_WITH_FRONT_BUFFER_AS_SCREEN, 0xc112)?;
    c.run(0xc112, 7)?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    let ax = c.pop();
    c.set_reg(AX, ax);
    let ds = c.sreg(DS);
    c.push(ds);
    let fb2 = c.word(g::FRAMEBUFFER_2);
    c.set_reg(SI, fb2);
    let screen = c.word(g::SCREEN_BUFFER);
    c.set_sreg(ES, screen);
    let front = c.word(g::FRAMEBUFFER_FRONT);
    c.set_sreg(DS, front);
    c.set_reg(BP, TRANSITION_TIMER);
    c.interp(0xc124, 0xc129)?;
    c.run(0xc129, 2)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.call(code::GFX_COPY_WHOLE_FRAMEBUF_TO_SCREEN, 0xc12d)?;
    c.interp(0xc12d, 0xc131)?;
    c.run(0xc131, 2)?;
    c.set_byte(g::TRANSITION_ACTIVE, 0);
    c.ret()
}

/// `transition` (C108): draws BP into the front buffer, then transition
/// AL to the screen.
pub fn transition_al(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc108)?;
    transition(&mut Ctx::new(m))
}

/// Not in use yet: never called in the checking runs, so unverified.
#[allow(dead_code)]
/// `copy_pal_and_transition` (C102): copies palette 2 to palette 1 (driver
/// function 41), then transition 3Ah.
pub fn copy_pal_and_transition(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc102)?;
    let mut c = Ctx::new(m);
    c.interp(0xc102, 0xc106)?;
    c.run(0xc106, 1)?;
    c.set_reg8(r8::AL, 0x3a);
    transition(&mut c)
}

/// `center_sprite_coordinates` (C202): (DX, BX) -= half the size of
/// sprite AX (width: the low 12 bits of its first word). AX, SI kept; ES
/// = the sheet. Flags from the `sub bx`.
pub fn center_sprite_coordinates(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc202)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let (ax, si) = (c.reg(AX), c.reg(SI));
    c.push(ax);
    c.push(si);
    c.call(code::GET_SUBRESOURCE_POINTER, 0xc207)?;
    c.run(0xc207, 11)?;
    let size = c.lods(W, ES);
    let high = c.and(B, size >> 8, 0x0f);
    let width = c.shr1(W, high << 8 | size & 0xff, 1);
    let dx = c.reg(DX);
    let dx = c.sub(W, dx, width);
    c.set_reg(DX, dx);
    let height = c.lods(B, ES);
    let half = c.shr1(B, height, 1) as u8;
    let half = half as i8 as u16;
    let bx = c.reg(BX);
    let bx = c.sub(W, bx, half);
    c.set_reg(BX, bx);
    let si = c.pop();
    c.set_reg(SI, si);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// `draw_sprite_clipped_clobbering_bx_dx` (C30D): sprite AX of the loaded
/// sheet at (DX, BX) into the active framebuffer, clipped to the clip
/// rectangle (driver function 6). Exit: DS = SS; DI, CX = its size words.
pub fn draw_sprite_clipped_clobbering(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc30d)?;
    let mut c = Ctx::new(m);
    c.run(start, 11)?;
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.set_sreg(ES, fb);
    let (sheet, seg) = (c.word(g::SPRITE_SHEET), c.word(g::SPRITE_SHEET + 2));
    c.set_reg(SI, sheet);
    c.set_sreg(DS, seg);
    let ax = c.reg(AX);
    let index = c.shl1(W, ax, 1);
    c.set_reg(BP, index);
    let offset = c.word_in(DS, index.wrapping_add(sheet));
    let si = c.add(W, sheet, offset);
    c.set_reg(SI, si);
    let size = c.lods(W, DS);
    c.set_reg(DI, size);
    c.lods(W, DS);
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    c.set_reg8(r8::AH, 0);
    let ax = c.reg(AX);
    c.set_reg(CX, ax);
    c.set_reg(BP, g::RECT_D834);
    c.interp(0xc327, 0xc32c)?;
    c.run(0xc32c, 3)?;
    let ss = c.sreg(SS);
    c.push(ss);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}

/// `draw_sprite_list` (C32F): like `draw_icons_list_at_si`, clipped.
pub fn draw_sprite_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc32f)?;
    let mut c = Ctx::new(m);
    loop {
        let si = c.reg(SI);
        let sprite = c.word_in(DS, si);
        if sprite == 0xffff {
            c.run(0xc32f, 4)?;
            c.lods(W, DS);
            c.cmp(W, sprite, 0xffff);
            return c.ret();
        }
        c.run(0xc32f, 10)?;
        c.lods(W, DS);
        c.cmp(W, sprite, 0xffff);
        let x = c.lods(W, DS);
        let y = c.lods(W, DS);
        c.set_reg(AX, sprite);
        c.set_reg(DX, x);
        c.set_reg(BX, y);
        let si = c.reg(SI);
        c.push(si);
        c.call(code::DRAW_SPRITE_CLIPPED_CLOBBERING, 0xc340)?;
        c.run(0xc340, 2)?;
        let si = c.pop();
        c.set_reg(SI, si);
    }
}

/// `draw_all_ui_elements` (D1EF): the UI element list at SI (a count, then
/// the elements).
pub fn draw_all_ui_elements(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd1ef)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let count = c.lods(W, DS);
    c.set_reg(CX, count);
    super::gfx::draw_ui_elements_list_at_ds_si(c.m, 0xd1f2)
}

/// `loc_0c7d4`: the rect at SI (left, top, right, bottom, signed) clipped
/// to the rect at DI, copied from the screen buffer to the front buffer
/// (driver function 16); nothing when they do not meet. DS back as SS.
pub fn copy_rect_clipped(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc7d4)?;
    let mut c = Ctx::new(m);
    let (si, di) = (c.reg(SI), c.reg(DI));
    let r = [c.word(si), c.word(si + 2), c.word(si + 4), c.word(si + 6)];
    let k = [c.word(di), c.word(di + 2), c.word(di + 4), c.word(di + 6)];
    // Up to where the signed tests pass.
    let tests = [(r[0] as i16) < (k[2] as i16), (r[1] as i16) < (k[3] as i16), (r[2] as i16) > (k[0] as i16), (r[3] as i16) > (k[1] as i16)];
    let passed = tests.iter().take_while(|&&t| t).count();
    if passed < 4 {
        c.run(start, 4 * passed as u32 + 4)?;
        let mut v = 0;
        for i in 0..=passed {
            v = c.lods(W, DS);
            let lim = [k[2], k[3], k[0], k[1]][i];
            c.cmp(W, v, lim);
            if i == passed {
                break;
            }
            c.set_reg([DX, BX, BP, AX][i], v);
        }
        let _ = v;
        return c.ret();
    }
    let x0 = if r[0] < k[0] { k[0] } else { r[0] };
    let x1 = if r[2] >= k[2] { k[2] } else { r[2] };
    let y0 = if r[1] < k[1] { k[1] } else { r[1] };
    let y1 = if r[3] >= k[3] { k[3] } else { r[3] };
    let n = 15
        + 2 + (r[0] < k[0]) as u32
        + 2 + (r[2] >= k[2]) as u32
        + 2 + (r[1] < k[1]) as u32
        + 2 + (r[3] >= k[3]) as u32
        + 2;
    let empty = x1 <= x0 || y1 <= y0;
    if empty {
        c.run(start, n + if x1 <= x0 { 1 } else { 3 })?;
    } else {
        c.run(start, n + 2 + 2)?;
    }
    for (i, reg) in [DX, BX, BP, AX].into_iter().enumerate() {
        let v = c.lods(W, DS);
        c.cmp(W, v, [k[2], k[3], k[0], k[1]][i]);
        if reg != AX {
            c.set_reg(reg, v);
        }
    }
    c.cmp(W, r[0], k[0]);
    c.set_reg(DX, x0);
    c.cmp(W, r[2], k[2]);
    c.set_reg(BP, x1);
    c.cmp(W, r[1], k[1]);
    c.set_reg(BX, y0);
    c.cmp(W, r[3], k[3]);
    c.set_reg(AX, y1);
    let w = c.sub(W, x1, x0);
    c.set_reg(BP, w);
    if x1 <= x0 {
        return c.ret();
    }
    let h = c.sub(W, y1, y0);
    c.set_reg(AX, h);
    if y1 <= y0 {
        return c.ret();
    }
    let front = c.word(g::FRAMEBUFFER_FRONT);
    c.set_sreg(ES, front);
    let screen = c.word(g::SCREEN_BUFFER);
    c.set_sreg(DS, screen);
    c.interp(0xc81f, 0xc824)?;
    c.run(0xc824, 3)?;
    let ss = c.sreg(SS);
    c.push(ss);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}
