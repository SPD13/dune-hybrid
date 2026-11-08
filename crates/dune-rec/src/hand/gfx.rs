//! Calls into the graphics driver (DNVGA, through its far-pointer table at
//! DS:38B5, Cryogenic's `gfxVtable`), the sprite drawing wrappers, HNM
//! frame steps and UI drawing.

use super::{Ctx, W, entered, globals as g};
use pc::{
    Machine,
    cpu::{AX, BP, CF, CX, DS, ES, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const SET_FB1_AS_ACTIVE_FRAMEBUFFER: u16 = 0xc07c;
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
