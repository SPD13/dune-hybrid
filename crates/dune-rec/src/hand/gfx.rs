//! Calls into the graphics driver (DNVGA, through its far-pointer table at
//! DS:38B5, Cryogenic's `gfxVtable`), the sprite drawing wrappers, HNM
//! frame steps and UI drawing.

use super::pure;
use super::{B, Ctx, W, entered, globals as g};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CS, CX, DI, DS, DX, ES, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const SET_FB1_AS_ACTIVE_FRAMEBUFFER: u16 = 0xc07c;
    pub const OPEN_RESOURCE_BY_INDEX: u16 = 0xc13e;
    pub const DRAW_LINE: u16 = 0xc53e;
    pub const LOAD_RESOURCE_TO: u16 = 0xf0b9;
    pub const LOAD_ICONES_SPRITES: u16 = 0xc137;
    pub const APPLY_PALETTE: u16 = 0xc1ba;
    pub const DRAW_SPRITE_CLOBBERING_BX_DX: u16 = 0xc22f;
    pub const DRAW_SPRITE_CLIPPED_CLOBBERING: u16 = 0xc30d;
    pub const HNM_DO_FRAME: u16 = 0xca60;
    pub const HNM_READ_BYTES: u16 = 0xcdbf;
    pub const GET_PHRASE_OR_COMMAND_STRING_SI: u16 = 0xcf70;
    pub const FONT_DRAW_STRING: u16 = 0xd1bb;
    pub const DRAW_UI_ELEMENT: u16 = 0xd200;
    pub const STC_ON_USER_INPUT: u16 = 0xdd63;
    pub const WAIT_A_BIT: u16 = 0xe387;
}

/// Bytes per UI element record.
const UI_ELEMENT_SIZE: u16 = 0x0e;

/// `gfx_clear_active_framebuffer` (C0AD): driver function 8, ES = the
/// active framebuffer.
pub fn gfx_clear_active_framebuffer(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc0ad)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.set_sreg(ES, fb);
    c.interp(0xc0b1, 0xc0b5)?;
    c.run(0xc0b5, 1)?;
    c.ret()
}

/// `update_screen_palette` (C0F4): driver function 32, unless the front
/// buffer is the screen. Flags from the `cmp` (when it does nothing).
/// Exit: AX = the front buffer.
pub fn update_screen_palette(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc0f4)?;
    let mut c = Ctx::new(m);
    let (front, screen) = (c.word(g::FRAMEBUFFER_FRONT), c.word(g::SCREEN_BUFFER));
    c.run(start, if front == screen { 4 } else { 3 })?;
    c.set_reg(AX, front);
    c.cmp(W, front, screen);
    if front != screen {
        c.interp(0xc0fd, 0xc101)?;
        c.run(0xc101, 1)?;
    }
    c.ret()
}

/// `gfx_call_bp_with_front_buffer_as_screen` (C097): draw into the front
/// buffer and call BP with it as the screen, then the screen back.
pub fn gfx_call_bp_with_front_buffer_as_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc097)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SET_FB1_AS_ACTIVE_FRAMEBUFFER, 0xc09a)?;
    c.run(0xc09a, 4)?;
    let screen = c.word(g::SCREEN_BUFFER);
    c.push(screen);
    let front = c.word(g::FRAMEBUFFER_FRONT);
    c.push(front);
    let v = c.pop();
    c.set_word(g::SCREEN_BUFFER, v);
    let target = c.reg(BP);
    c.call(target, 0xc0a8)?;
    c.run(0xc0a8, 2)?;
    let screen = c.pop();
    c.set_word(g::SCREEN_BUFFER, screen);
    c.ret()
}

/// A sprite drawing routine that keeps BX and DX (C2FD, C305): pushes
/// them around the call of the one that does not.
fn keep_bx_dx(m: &mut Machine, start: u16, entry: u16, draw: u16) -> Result<(), Bail> {
    use pc::cpu::{BX, DX};
    entered(start, entry)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.call(draw, entry + 5)?;
    c.run(entry + 5, 3)?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.ret()
}

/// `draw_sprite` (C2FD): sprite AX at (DX, BX).
pub fn draw_sprite(m: &mut Machine, start: u16) -> Result<(), Bail> {
    keep_bx_dx(m, start, 0xc2fd, code::DRAW_SPRITE_CLOBBERING_BX_DX)
}

/// `draw_sprite_clipped` (C305).
pub fn draw_sprite_clipped(m: &mut Machine, start: u16) -> Result<(), Bail> {
    keep_bx_dx(m, start, 0xc305, code::DRAW_SPRITE_CLIPPED_CLOBBERING)
}

/// A framebuffer copy through the driver: `push ds; mov es, [to];
/// mov ds, [from]; call far ss:[fn]; pop ds; ret`.
fn copy_buffer(m: &mut Machine, start: u16, entry: u16, from: u16, to: u16) -> Result<(), Bail> {
    entered(start, entry)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let to = c.word(to);
    c.set_sreg(ES, to);
    let from = c.word(from);
    c.set_sreg(DS, from);
    c.interp(entry + 9, entry + 14)?;
    c.run(entry + 14, 2)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}

/// `copy_active_framebuffer_to_framebuffer_2` (C412): driver function 11.
pub fn copy_active_framebuffer_to_framebuffer_2(m: &mut Machine, start: u16) -> Result<(), Bail> {
    copy_buffer(m, start, 0xc412, g::FRAMEBUFFER_ACTIVE, g::FRAMEBUFFER_2)
}

/// `gfx_copy_framebuffer_to_screen` (C49A): driver function 15, from the
/// screen buffer to the front buffer.
pub fn gfx_copy_framebuffer_to_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    copy_buffer(m, start, 0xc49a, g::SCREEN_BUFFER, g::FRAMEBUFFER_FRONT)
}

/// `gfx_copy_whole_framebuf_to_screen` (C4CD): driver function 15, from the
/// front buffer to the screen buffer.
pub fn gfx_copy_whole_framebuf_to_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    copy_buffer(m, start, 0xc4cd, g::FRAMEBUFFER_FRONT, g::SCREEN_BUFFER)
}

/// `clear_game_area` (C432): driver function 9 on the game area's
/// rectangle (DS:1470) of the active framebuffer.
pub fn clear_game_area(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc432)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(SI, g::GAME_AREA_RECT);
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.set_sreg(ES, fb);
    c.interp(0xc439, 0xc43d)?;
    c.run(0xc43d, 1)?;
    c.ret()
}

/// `map_func_gfx` (B977): driver function 23 on the map: ES = the front
/// buffer, DS:SI = the map data, AL = [DD02h], BP = the trig table.
pub fn map_func_gfx(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb977)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_reg(BP, g::TRIG_TABLE);
    let front = c.word(g::FRAMEBUFFER_FRONT);
    c.set_sreg(ES, front);
    let al = c.byte(g::MAP_GFX_ARG);
    c.set_reg8(super::r8::AL, al);
    let (si, ds) = (c.word(g::MAP_GFX_DATA), c.word(g::MAP_GFX_DATA + 2));
    c.set_reg(SI, si);
    c.set_sreg(DS, ds);
    c.interp(0xb985, 0xb98a)?;
    c.run(0xb98a, 1)?;
    c.ret()
}

/// `hnm_do_frame_skippable` (C9E8): the next video frame; CF set (and the
/// key taken) on a key or click.
pub fn hnm_do_frame_skippable(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc9e8)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::HNM_DO_FRAME, 0xc9eb)?;
    c.run(0xc9eb, 1)?;
    c.call(code::STC_ON_USER_INPUT, 0xc9ee)?;
    if c.flag(CF) {
        c.run(0xc9ee, 4)?; // jb; jmp kb_clear_scancode; mov [CEE8h], 0; ret
        c.set_byte(g::KEY_HIT, 0);
    } else {
        c.run(0xc9ee, 2)?;
    }
    c.ret()
}

/// `hnm_do_frame_and_check_if_frame_advanced` (C9F4): the next video
/// frame. Exit: AX = the frame before; ZF clear if it advanced.
pub fn hnm_do_frame_and_check_if_frame_advanced(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc9f4)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let before = c.word(g::HNM_FRAME);
    c.push(before);
    c.call(code::HNM_DO_FRAME, 0xc9fb)?;
    c.run(0xc9fb, 3)?;
    let before = c.pop();
    c.set_reg(AX, before);
    let now = c.word(g::HNM_FRAME);
    c.cmp(W, before, now);
    c.ret()
}

/// `hnm_read_header_size` (CD8F): reads 2 bytes of the video; AX = them
/// (ES:SI past them), or CF set on failure.
pub fn hnm_read_header_size(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcd8f)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(CX, 2);
    c.call(code::HNM_READ_BYTES, 0xcd95)?;
    if c.flag(CF) {
        c.run(0xcd95, 2)?;
        return c.ret();
    }
    c.run(0xcd95, 4)?;
    let (si, es) = (c.word(g::HNM_READ_PTR), c.word(g::HNM_READ_PTR + 2));
    c.set_reg(SI, si);
    c.set_sreg(ES, es);
    let size = c.word_in(ES, si.wrapping_sub(2));
    c.set_reg(AX, size);
    c.ret()
}

/// `hnm_handle_pal_chunk` (CE3B): applies the palette chunk at the read
/// pointer's segment, offset [DC1Eh], then driver function 32.
pub fn hnm_handle_pal_chunk(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xce3b)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let es = c.word(g::HNM_READ_PTR + 2);
    c.set_sreg(ES, es);
    let si = c.word(g::HNM_PAL_CHUNK);
    c.set_reg(SI, si);
    c.call(code::APPLY_PALETTE, 0xce46)?;
    c.interp(0xce46, 0xce4a)?;
    c.run(0xce4a, 1)?;
    c.ret()
}

/// `font_draw_phrase_or_command_string` (D19B): draws string AX of the
/// phrase or command table. SI kept.
pub fn font_draw_phrase_or_command_string(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd19b)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let si = c.reg(SI);
    c.push(si);
    let ax = c.reg(AX);
    c.set_reg(SI, ax);
    c.call(code::GET_PHRASE_OR_COMMAND_STRING_SI, 0xd1a1)?;
    c.run(0xd1a1, 1)?;
    c.call(code::FONT_DRAW_STRING, 0xd1a4)?;
    c.run(0xd1a4, 2)?;
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `draw_ui_elements_list_at_ds_si` (D1F2): loads the icon sprites, then
/// draws the CX UI elements at SI. Exit: SI past them, CX = 0; flags from
/// the last `add si`.
pub fn draw_ui_elements_list_at_ds_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd1f2)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::LOAD_ICONES_SPRITES, 0xd1f5)?;
    loop {
        c.run(0xd1f5, 2)?;
        let cx = c.reg(CX);
        c.push(cx);
        c.call(code::DRAW_UI_ELEMENT, 0xd1f9)?;
        let sp = c.reg(SP);
        let left = c.word_in(SS, sp).wrapping_sub(1); // pop cx; add si; loop
        c.run(0xd1f9, if left == 0 { 4 } else { 3 })?;
        c.pop();
        c.set_reg(CX, left);
        let si = c.reg(SI);
        let si = c.add(W, si, UI_ELEMENT_SIZE);
        c.set_reg(SI, si);
        if left == 0 {
            return c.ret();
        }
    }
}

/// `ui_head_animate_down` (181E): steps the talking head's animation back
/// to 0, one step (`sub_17be`) every 8 ticks. Exit: flags from the `cmp`
/// with 0; AX = 8 if it moved.
pub fn ui_head_animate_down(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x181e)?;
    let mut c = Ctx::new(m);
    loop {
        let step = c.byte(g::HEAD_ANIMATION_STEP);
        if step == 0 {
            c.run(0x181e, 3)?; // cmp; je 181Dh; ret
            c.cmp(super::B, 0, 0);
            return c.ret();
        }
        c.run(0x181e, 4)?;
        c.cmp(super::B, step as u16, 0);
        let step = c.dec(super::B, step as u16) as u8;
        c.set_byte(g::HEAD_ANIMATION_STEP, step);
        c.call(0x17be, 0x182c)?;
        c.run(0x182c, 2)?;
        c.set_reg(AX, 8);
        c.call(code::WAIT_A_BIT, 0x1832)?;
        c.run(0x1832, 1)?; // jmp short 181Eh
    }
}

/// `loc_0c0e8`: driver function 30 on the screen buffer, BP = the timer.
pub fn gfx_call_30(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc0e8)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let screen = c.word(g::SCREEN_BUFFER);
    c.set_sreg(ES, screen);
    c.set_reg(BP, g::TIMER);
    c.interp(0xc0ef, 0xc0f3)?;
    c.run(0xc0f3, 1)?;
    c.ret()
}

/// `loc_0c343`: sprite AX of the sprite sheet centred on (DX, BX), into the
/// active framebuffer, clipped to the rect at D834h (driver function 6).
/// DS comes back as SS.
pub fn draw_sprite_centred(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc343)?;
    let mut c = Ctx::new(m);
    c.run(start, 16)?;
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.set_sreg(ES, fb);
    let (sheet, seg) = (c.word(g::SPRITE_SHEET), c.word(g::SPRITE_SHEET + 2));
    c.set_reg(SI, sheet);
    c.set_sreg(DS, seg);
    let ax = c.reg(AX);
    let bp = c.shl1(W, ax, 1);
    c.set_reg(BP, bp);
    let off = c.word(bp.wrapping_add(sheet));
    let si = c.add(W, sheet, off);
    c.set_reg(SI, si);
    let w = c.lods(W, DS);
    c.set_reg(DI, w);
    let hi = c.and(B, w >> 8, 0x0f);
    let half = c.shr1(W, hi << 8 | w & 0xff, 1);
    c.set_reg(AX, half);
    let dx = c.reg(DX);
    let dx = c.sub(W, dx, half);
    c.set_reg(DX, dx);
    let h = c.lods(W, DS);
    c.xor(B, h >> 8, h >> 8);
    let h = h & 0xff;
    c.set_reg(CX, h);
    let half = c.shr1(W, h, 1);
    c.set_reg(AX, half);
    let bx = c.reg(BX);
    let bx = c.sub(W, bx, half);
    c.set_reg(BX, bx);
    c.set_reg(BP, g::RECT_D834);
    c.interp(0xc368, 0xc36d)?;
    c.run(0xc36d, 3)?;
    let ss = c.sreg(SS);
    c.push(ss);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}

/// `loc_0c53e`: a line from (DX, BX) to (DI, CX) in the text colour, into
/// the active framebuffer (driver function 19; SI, BP its clip).
pub fn draw_line(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc53e)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    line(&mut c)?;
    c.run(0xc550, 1)?;
    c.ret()
}

fn line(c: &mut Ctx) -> Result<(), Bail> {
    c.set_reg(SI, LINE_CLIP);
    let bp = c.word(LINE_CLIP + 8);
    c.set_reg(BP, bp);
    let colour = c.byte(g::TEXT_COLOUR);
    c.set_reg8(super::r8::AL, colour);
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.set_sreg(ES, fb);
    c.interp(0xc54c, 0xc550)
}

/// The clip rect of lines, and a word after it.
const LINE_CLIP: u16 = 0x276a;

/// From C560: the outline of the rect (DX, BX) to (DI, CX) in colour AL:
/// top, left, bottom, right. The stretch from `at` has `prefix`
/// instructions before C560.
fn rect_outline(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 7)?;
    ops(c);
    let al = c.reg8(super::r8::AL);
    c.set_byte(g::TEXT_COLOUR, al);
    for r in [BX, CX, DX, DI] {
        let v = c.reg(r);
        c.push(v);
    }
    let bx = c.reg(BX);
    c.set_reg(CX, bx);
    c.call(code::DRAW_LINE, 0xc56c)?;
    c.run(0xc56c, 4)?;
    let sp = c.reg(SP);
    c.set_reg(BP, sp);
    let bottom = c.word_in(SS, sp.wrapping_add(4));
    c.set_reg(CX, bottom);
    c.set_reg(BX, bottom);
    c.call(code::DRAW_LINE, 0xc576)?;
    c.run(0xc576, 4)?;
    let sp = c.reg(SP);
    c.set_reg(BP, sp);
    let dx = c.reg(DX);
    c.set_reg(DI, dx);
    let top = c.word_in(SS, sp.wrapping_add(6));
    c.set_reg(BX, top);
    c.call(code::DRAW_LINE, 0xc580)?;
    c.run(0xc580, 3)?;
    let di = c.pop();
    c.set_reg(DI, di);
    c.set_reg(DX, di);
    c.call(code::DRAW_LINE, 0xc586)?;
    c.run(0xc586, 4)?;
    for r in [DX, CX, BX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// `draw_rect_outline` (C560).
pub fn draw_rect_outline(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc560)?;
    rect_outline(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_0c551`: the outline of the rect at SI (left, top, right, bottom,
/// exclusive; then the colour byte).
pub fn draw_rect_outline_at_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc551)?;
    rect_outline(&mut Ctx::new(m), start, 11, |c| {
        let l = c.lods(W, DS);
        c.set_reg(DX, l);
        let t = c.lods(W, DS);
        c.set_reg(BX, t);
        let r = c.lods(W, DS);
        let r = pure::dec(W, r);
        c.set_reg(DI, r);
        let b = c.lods(W, DS);
        let b = pure::dec(W, b);
        c.set_reg(CX, b);
        c.lods(B, DS);
    })
}

/// `loc_0c2f2`: opens resource AL and draws its sprite 0 at (0, 0).
pub fn open_resource_and_draw_sprite_0(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc2f2)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let ah = c.reg8(super::r8::AH) as u16;
    let z = c.xor(B, ah, ah);
    c.set_reg8(super::r8::AH, z as u8);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0xc2f7)?;
    c.run(0xc2f7, 6)?;
    for r in [AX, BX, DX] {
        let v = c.reg(r);
        let z = c.xor(W, v, v);
        c.set_reg(r, z);
    }
    c.push(0);
    c.push(0);
    c.call(code::DRAW_SPRITE_CLOBBERING_BX_DX, 0xc302)?;
    c.run(0xc302, 3)?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.ret()
}

/// `loc_01834`: driver function 25 on the front buffer, SI = CD9Eh,
/// BP = 1E76h.
pub fn gfx_call_25(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1834)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_reg(SI, 0xcd9e);
    c.set_reg(BP, 0x1e76);
    let front = c.word(g::FRAMEBUFFER_FRONT);
    c.set_sreg(ES, front);
    c.interp(0x183e, 0x1842)?;
    c.run(0x1842, 1)?;
    c.ret()
}

/// `loc_04b16`: unless the front buffer is the screen, driver function
/// 18 with ES = the screen buffer + 1E0h (paragraphs).
pub fn gfx_call_18(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4b16)?;
    let mut c = Ctx::new(m);
    let (screen, front) = (c.word(g::SCREEN_BUFFER), c.word(g::FRAMEBUFFER_FRONT));
    c.run(start, if screen == front { 5 } else { 6 })?;
    c.set_reg(AX, screen);
    c.set_reg(SI, front);
    c.cmp(W, screen, front);
    if screen == front {
        return c.ret();
    }
    let es = c.add(W, screen, 0x1e0);
    c.set_reg(AX, es);
    c.set_sreg(ES, es);
    c.interp(0x4b26, 0x4b2a)?;
    c.run(0x4b2a, 1)?;
    c.ret()
}

/// A ring of points (x, y words) in the code segment, E40Ch..E85Ch; the
/// next free slot at [`POINTS_NEXT`].
const POINTS_RING: u16 = 0xe40c;
const POINTS_RING_END: u16 = 0xe85c;
const POINTS_NEXT: u16 = 0x149a;

/// `loc_049ea`: [4728h] cleared, every point of the ring to (800h, 800h).
pub fn clear_points_ring(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x49ea)?;
    let mut c = Ctx::new(m);
    let n = ((POINTS_RING_END - POINTS_RING) / 4) as u32;
    c.run(start, 5 + 4 * n + 1)?;
    c.set_byte(0x4728, 0);
    let cs = c.sreg(pc::cpu::CS);
    c.push(cs);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(DI, POINTS_RING);
    c.set_reg(AX, 0x800);
    loop {
        c.stos(W);
        c.stos(W);
        let di = c.reg(DI);
        if di >= POINTS_RING_END {
            return c.ret();
        }
    }
}

/// `loc_04a00`: the point (DX, BX) into the ring's next slot.
pub fn add_point_to_ring(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4a00)?;
    let mut c = Ctx::new(m);
    let di = c.word(POINTS_NEXT);
    let step = c.string_step(W);
    let end = di.wrapping_add(step.wrapping_mul(2));
    let wrap = end >= POINTS_RING_END;
    c.run(start, 11 + wrap as u32)?;
    let cs = c.sreg(pc::cpu::CS);
    c.push(cs);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(DI, di);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.set_reg(AX, dx);
    c.stos(W);
    c.set_reg(AX, bx);
    c.stos(W);
    let next = if wrap { POINTS_RING } else { end };
    c.set_reg(DI, next);
    c.set_word(POINTS_NEXT, next);
    c.ret()
}

/// `loc_0157e`: a CX-byte block of zeros on the stack handed to the
/// driver (palette function at 38BDh, or 394Dh with BP set) at DX; AL
/// noted at [46D7h], CX, BX at D81Ah, D818h.
pub fn gfx_zero_block(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x157e)?;
    let mut c = Ctx::new(m);
    c.run(start, 10)?;
    let al = c.reg8(super::r8::AL);
    c.set_byte(0x46d7, al);
    let (cx, bx) = (c.reg(CX), c.reg(BX));
    c.set_word(0xd81a, cx);
    c.set_word(0xd818, bx);
    let sp = c.reg(SP);
    let sp = c.sub(W, sp, cx);
    c.set_reg(SP, sp);
    let ss = c.sreg(SS);
    c.push(ss);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(DI, sp);
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    c.set_reg(AX, 0);
    c.push(cx);
    c.m.cpu.string_op(&mut c.m.hw, 0xaa, None, 0xf3); // rep stosb
    c.run(0x1594, 4)?;
    let cx = c.pop();
    c.set_reg(CX, cx);
    let sp = c.reg(SP);
    c.set_reg(DX, sp);
    let bp = c.reg(BP);
    c.or(W, bp, bp);
    if bp == 0 {
        c.interp(0x159b, 0x159f)?;
        c.run(0x159f, 3)?;
    } else {
        c.interp(0x15a1, 0x15a5)?;
        c.run(0x15a5, 2)?;
    }
    let cx = c.reg(CX);
    let sp = c.reg(SP);
    let sp = c.add(W, sp, cx);
    c.set_reg(SP, sp);
    c.ret()
}

/// `loc_037ad`: resource 22h opened, its sprite 0 at (0, 0).
pub fn draw_resource_22(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x37ad)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg8(super::r8::AL, 0x22);
    open_resource_and_draw_sprite_0(c.m, 0xc2f2)
}

/// `loc_04a1a`: the last point of the ring (unless [4728h] is negative,
/// the cursor kept off the rect 148Ah): when on the map (row below 100h,
/// in the view by `loc_062d6`), sprite 2Fh drawn there, one up and left,
/// into the back buffer.
pub fn draw_last_ring_point(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4a1a)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0x4728);
    c.run(start, if f & 0x80 != 0 { 3 } else { 4 })?;
    if f & 0x80 != 0 {
        return c.ret();
    }
    c.set_reg(SI, 0x148a);
    c.call(0xdb74, 0x4a27)?;
    let p = c.word(POINTS_NEXT);
    let wrap = p == POINTS_RING;
    let at = (if wrap { POINTS_RING_END } else { p }).wrapping_sub(4);
    let y = c.code_word(at.wrapping_add(2));
    let off_map = (y >> 8).wrapping_sub(1) & 0x80 == 0;
    c.run(0x4a27, 3 + wrap as u32 + 7 + if off_map { 1 } else { 1 })?;
    c.set_reg(SI, p);
    let si = pure::sub(W, if wrap { POINTS_RING_END } else { p }, 4);
    c.set_reg(SI, si);
    let x = c.lods(W, CS);
    c.set_reg(DX, x);
    let y = c.lods(W, CS);
    c.set_reg(BX, y);
    let ah = pure::dec(B, y >> 8);
    c.set_reg(AX, ah << 8 | y & 0xff);
    if off_map {
        return c.ret();
    }
    c.call(0x62d6, 0x4a46)?;
    if c.flag(CF) {
        c.run(0x4a46, 2)?;
        return c.ret();
    }
    c.run(0x4a46, 4)?;
    let bx = c.reg(BX);
    let bx = pure::dec(W, bx);
    c.set_reg(BX, bx);
    let dx = c.reg(DX);
    let dx = pure::dec(W, dx);
    c.set_reg(DX, dx);
    c.call(code::LOAD_ICONES_SPRITES, 0x4a4d)?;
    c.run(0x4a4d, 2)?;
    c.set_reg(AX, 0x2f);
    c.call(0xc085, 0x4a53)?;
    c.run(0x4a53, 1)?;
    c.call(code::DRAW_SPRITE_CLOBBERING_BX_DX, 0x4a56)?;
    c.run(0x4a56, 1)?;
    c.call(code::SET_FB1_AS_ACTIVE_FRAMEBUFFER, 0x4a59)?;
    c.run(0x4a59, 1)?;
    c.ret()
}

/// `loc_04285`: the intro's script loaded once (resource BEh into the code
/// segment at 15Fh, its first 8 bytes to AA66h, the pointer moved on by
/// 167h), [487Eh] = 1, [473Eh] = 0, the screen kind 8, [AA6Eh] = the
/// script's start; resource 39h opened and the driver's function at
/// 3935h.
pub fn load_intro_script(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4285)?;
    let mut c = Ctx::new(m);
    let loaded = c.code_word(0x15f + 2) != 0;
    if loaded {
        c.run(start, 3 + 8)?;
        c.set_reg(DI, 0x15f);
        let v = c.code_word(0x161);
        c.cmp(W, v, 0);
    } else {
        c.run(start, 7)?;
        c.set_reg(DI, 0x15f);
        c.cmp(W, 0, 0);
        let cs = c.sreg(CS);
        c.push(cs);
        let es = c.pop();
        c.set_sreg(ES, es);
        c.set_reg(SI, 0xbe);
        c.call(code::LOAD_RESOURCE_TO, 0x4297)?;
        c.run(0x4297, 6)?;
        c.set_reg(SI, 0x15f);
        c.set_reg(DI, 0xaa66);
        let ds = c.sreg(DS);
        c.push(ds);
        let es = c.pop();
        c.set_sreg(ES, es);
        c.set_reg(CX, 4);
        c.m.cpu.string_op(&mut c.m.hw, 0xa5, Some(CS), 0xf3); // rep movsw cs:
        c.run(0x42a5, 1 + 8)?;
        let si = c.reg(SI);
        let v = c.word_in(CS, si);
        let v = c.add(W, v, 0x167);
        c.set_word_in(CS, si, v);
    }
    c.set_word(0x487e, 1);
    c.set_byte(0x473e, 0);
    c.set_byte(0x11c9, 8);
    let p = c.code_word(0x167);
    let q = c.code_word(p);
    let si = c.add(W, p, q);
    c.set_reg(SI, si);
    c.set_word(0xaa6e, si);
    c.set_reg(AX, 0x39);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0x42cc)?;
    c.interp(0x42cc, 0x42d0)?;
    c.run(0x42d0, 1)?;
    c.ret()
}

/// `loc_07b1b`: the driver's panel fill (far [38DDh], AL the colour byte
/// 9 of SI) on the screen [0DBDAh], then the rect SI outlined
/// ([`draw_rect_outline_at_si`]).
pub fn fill_and_outline(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7b1b)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let es = c.word(0xdbda);
    c.set_sreg(ES, es);
    let si = c.reg(SI);
    let al = c.byte(si.wrapping_add(9));
    c.m.cpu.set_reg8(super::r8::AL, al);
    c.push(si);
    c.interp(0x7b23, 0x7b27)?;
    c.run(0x7b27, 2)?;
    let s = c.pop();
    c.set_reg(SI, s);
    draw_rect_outline_at_si(c.m, 0xc551)
}

/// `loc_07b0f`: [46D8h] cleared, `loc_0c0e8` with SI = DI (SI kept, DI
/// the old SI), then [`fill_and_outline`].
pub fn redraw_panel_box(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7b0f)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_byte(0x46d8, 0);
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.push(si);
    c.set_reg(DI, si);
    c.set_reg(SI, di);
    c.call(0xc0e8, 0x7b1a)?;
    c.run(0x7b1a, 1)?;
    let s = c.pop();
    c.set_reg(SI, s);
    fill_and_outline(c.m, 0x7b1b)
}
