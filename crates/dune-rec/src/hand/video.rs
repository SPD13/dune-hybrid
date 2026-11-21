//! Video: framebuffers, fonts, HNM playback state, sprite resources, UI.

use super::pure;
use super::{B, Ctx, W, entered, globals as g};
use pc::{
    Machine,
    cpu::{AX, BX, CF, DI, DX, ES, SI, ZF},
    rec::Bail,
};

/// `blink` (176B): flips the blink phase. Flags from the `xor`.
pub fn blink(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x176b)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let phase = c.sequence().blink;
    let phase = pure::xor(B, phase as u16, 0xff) as u8;
    c.update_sequence(|t| t.blink = phase);
    c.ret()
}

/// `settings_ui_sub_global_offset` (A453): (DX, BX) from screen to settings
/// menu coordinates. Flags from the second `sub`.
pub fn settings_ui_sub_global_offset(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa453)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let (ox, oy) = (c.word(g::SETTINGS_UI_ORIGIN_X), c.word(g::SETTINGS_UI_ORIGIN_Y));
    let (x, y) = (c.reg(DX), c.reg(BX));
    let x = pure::sub(W, x, ox);
    let y = pure::sub(W, y, oy);
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    c.ret()
}

/// `settings_ui_add_global_offset` (A45C): the reverse of
/// [`settings_ui_sub_global_offset`]. Flags from the second `add`.
pub fn settings_ui_add_global_offset(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa45c)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let (ox, oy) = (c.word(g::SETTINGS_UI_ORIGIN_X), c.word(g::SETTINGS_UI_ORIGIN_Y));
    let (x, y) = (c.reg(DX), c.reg(BX));
    let x = pure::add(W, x, ox);
    let y = pure::add(W, y, oy);
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    c.ret()
}

/// `push [from]; pop [FRAMEBUFFER_ACTIVE]; ret`: draw into another buffer.
fn select_framebuffer(m: &mut Machine, start: u16, entry: u16, from: u16) -> Result<(), Bail> {
    entered(start, entry)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let segment = c.word(from);
    c.below_sp(1, segment);
    c.update_screen_buffers(|t| t.active = segment);
    c.ret()
}

/// `set_fb1_as_active_framebuffer` (C07C): draw into the front buffer.
pub fn set_fb1_as_active_framebuffer(m: &mut Machine, start: u16) -> Result<(), Bail> {
    select_framebuffer(m, start, 0xc07c, g::FRAMEBUFFER_FRONT)
}

/// `set_backbuffer_as_frame_buffer` (C085): draw into the back buffer.
pub fn set_backbuffer_as_frame_buffer(m: &mut Machine, start: u16) -> Result<(), Bail> {
    select_framebuffer(m, start, 0xc085, g::BACKBUFFER)
}

/// `set_screen_as_active_framebuffer` (C08E): draw to the screen.
pub fn set_screen_as_active_framebuffer(m: &mut Machine, start: u16) -> Result<(), Bail> {
    select_framebuffer(m, start, 0xc08e, g::SCREEN_BUFFER)
}

/// `get_subresource_ax_pointer_to_dssi` (C1F4): ES:SI = sub-resource AX of
/// the loaded sprite sheet (its word offset table is at the sheet's start).
/// Exit: BX kept; flags from the `add`.
pub fn get_subresource_pointer(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc1f4)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let bx = c.reg(BX);
    c.push(bx);
    let sheet = c.word(g::SPRITE_SHEET);
    let sheet_seg = c.word(g::SPRITE_SHEET + 2);
    c.set_sreg(ES, sheet_seg);
    let ax = c.reg(AX);
    let index2 = pure::shl1(W, ax, 1);
    let offset = c.word_in(ES, index2.wrapping_add(sheet));
    let si = pure::add(W, sheet, offset);
    c.set_reg(SI, si);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.ret()
}

/// [`get_subresource_pointer`] called: the offset of sub-resource AX of the sprite sheet (SI; ES its segment).
pub(super) fn subresource(c: &mut Ctx, ret: u16) -> Result<u16, Bail> {
    c.call(0xc1f4, ret)?;
    Ok(c.reg(SI))
}

/// `get_hnm_resource_flag_name_ptr_by_index_ax_to_bx` (C921): BX = the
/// flag name of video AX. Flags from the second `add`.
pub fn get_hnm_resource_flag_name_ptr(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc921)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let ax = c.reg(AX);
    let p = pure::add(W, g::HNM_FLAG_NAMES, ax);
    let p = pure::add(W, p, ax);
    let name = c.word(p);
    c.set_reg(BX, name);
    c.ret()
}

/// `check_if_hnm_complete` (CC85): ZF set while a video plays (state 0 or
/// 1). Flags from the last `cmp`.
pub fn check_if_hnm_complete(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcc85)?;
    let mut c = Ctx::new(m);
    let state = c.video_state().playing;
    c.run(start, if state == 0 { 3 } else { 4 })?;
    c.set_flag(ZF, state == 0);
    c.set_flag(CF, false);
    if state != 0 {
        c.set_flag(ZF, state == 1);
        c.set_flag(CF, state < 1);
    }
    c.ret()
}

/// [`check_if_hnm_complete`] called: whether no video plays (its state past 1).
pub(super) fn video_done(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    Ok(!c.call_zf(0xcc85, ret)?)
}

/// `hnm_advance_read_ptr` (CDF7): both HNM read pointers += AX. CF clear;
/// the other flags from the second `add`.
pub fn hnm_advance_read_ptr(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcdf7)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let ax = c.reg(AX);
    for p in [g::HNM_READ_PTR, g::HNM_READ_PTR_2] {
        let v = c.word(p);
        let v = c.add(W, v, ax);
        c.set_word(p, v);
    }
    c.set_flag(CF, false);
    c.ret()
}


/// `hnm_reset_frame_counter` (CE01): all four frame counters.
pub fn hnm_reset_frame_counter(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xce01)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    c.update_video_state(|t| (t.frame, t.fetched, t.last_frame, t.stop_frame) = (0, 0, 0xffff, 0xffff));
    c.ret()
}

/// `hnm_reset_counters` (CE07): the frame counters but the first.
pub fn hnm_reset_counters(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xce07)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.update_video_state(|t| (t.fetched, t.last_frame, t.stop_frame) = (0, 0xffff, 0xffff));
    c.ret()
}

/// `hnm_reset_buffers` (CE1A): HNM buffers to the second framebuffer, read
/// pointers to 0. Exit: AX = [CE74h]; flags from `xor ax, ax`.
pub fn hnm_reset_buffers(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xce1a)?;
    let mut c = Ctx::new(m);
    c.run(start, 12)?;
    let fb2 = c.screen_buffers().second;
    let zero = pure::xor(W, fb2, fb2);
    let v = c.word(g::HNM_CE74);
    c.set_reg(AX, v);
    c.update_video_state(|t| {
        t.fill = [zero, fb2];
        t.next_chunk = [zero, fb2];
        t.buffered = zero;
        t.part_size = zero;
        t.pending_frame[1] = zero;
        t.wrapped_at = v;
    });
    c.ret()
}

/// `font_set_draw_position` (D04E): text goes on at (DX, BX), which is
/// also where its next line starts.
pub fn font_set_draw_position(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd04e)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let (x, y) = (c.reg(DX), c.reg(BX));
    c.update_font_pen(|t| t.at[0] = x);
    c.update_font_pen(|t| t.at[1] = y);
    c.update_font_pen(|t| t.line[0] = x);
    c.update_font_pen(|t| t.line[1] = y);
    c.ret()
}

/// `font_get_draw_position` (D05F): (DX, BX) = where text goes on.
pub fn font_get_draw_position(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd05f)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let (x, y) = (c.font_pen().at[0], c.font_pen().at[1]);
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    c.ret()
}

/// [`font_get_draw_position`] called: where text goes on (DX, BX).
pub(super) fn draw_position(c: &mut Ctx, ret: u16) -> Result<(u16, u16), Bail> {
    c.call(0xd05f, ret)?;
    Ok((c.reg(DX), c.reg(BX)))
}

/// Select a font: its glyph routine and width table.
fn select_font(m: &mut Machine, start: u16, entry: u16, draw: u16, widths: u16) -> Result<(), Bail> {
    entered(start, entry)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_word(g::FONT_DRAW, draw);
    c.set_word(g::FONT_WIDTHS, widths);
    c.ret()
}

/// `font_select_tall_font` (D068).
pub fn font_select_tall_font(m: &mut Machine, start: u16) -> Result<(), Bail> {
    select_font(m, start, 0xd068, 0xd096, 0xceec)
}

/// `font_select_small_font` (D075).
pub fn font_select_small_font(m: &mut Machine, start: u16) -> Result<(), Bail> {
    select_font(m, start, 0xd075, 0xd12f, 0xcf6c)
}

/// The cursor images with a UI element table, in table order.
const CURSORS_WITH_UI: [u16; 4] = [0x260c, 0x2650, 0x2694, 0x26d8];

/// `set_di_to_ui_elements_ptr_based_on_cursor_image` (D694): DI = the UI
/// element table of the current cursor image; ZF set if it has one (else
/// DI points past the last table). Exit: AX = the cursor image, CF set.
pub fn ui_elements_for_cursor(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd694)?;
    let mut c = Ctx::new(m);
    c.check(start, 14)?;
    let cursor = c.word(g::MOUSE_CURSOR_IMAGE);
    c.set_reg(AX, cursor);
    let mut di = g::ARROW_CURSOR_BUTTONS;
    let mut n = 2 + 2; // mov ax; mov di; stc; ret
    for (i, image) in CURSORS_WITH_UI.into_iter().enumerate() {
        if i > 0 {
            di = pure::add(W, di, 0x0e);
            n += 1;
        }
        c.cmp(W, cursor, image);
        n += 1;
        if i < CURSORS_WITH_UI.len() - 1 {
            n += 1; // je
            if cursor == image {
                break;
            }
        }
    }
    c.set_reg(DI, di);
    c.set_flag(CF, true);
    c.count(n);
    c.ret()
}

/// [`ui_elements_for_cursor`] called: whether the cursor image has a UI element table (DI = it).
pub(super) fn cursor_has_elements(c: &mut Ctx, ret: u16) -> Result<bool, Bail> {
    c.call_zf(0xd694, ret)
}

/// `clear_frame_tasks` (DA53): no frame tasks.
pub fn clear_frame_tasks(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xda53)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_word(g::FRAME_TASKS, 0);
    c.set_byte(g::FRAME_TASKS_ACTIVE, 0);
    c.ret()
}

/// `strcpy_to_filename_buf` (F2FC): copies the string at DX (with its 0)
/// to the filename buffer. Exit: DX = 3826h, AL = 0, SI and DI kept; flags
/// from `or al, al`.
pub fn strcpy_to_filename_buf(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf2fc)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.push(si);
    c.push(di);
    let from = c.reg(DX);
    c.set_reg(SI, from);
    let to = c.word(g::FILENAME_BUF_PTR);
    c.set_reg(DI, to);
    loop {
        c.run(0xf304, 6)?;
        let (si, di) = (c.reg(SI), c.reg(DI));
        let ch = c.byte(si);
        c.set_reg8(super::r8::AL, ch);
        let si = pure::inc(W, si);
        c.set_reg(SI, si);
        c.set_byte(di, ch);
        let di = pure::inc(W, di);
        c.set_reg(DI, di);
        c.or(B, ch as u16, ch as u16);
        if ch == 0 {
            break;
        }
    }
    c.run(0xf30e, 4)?;
    let di = c.pop();
    c.set_reg(DI, di);
    let si = c.pop();
    c.set_reg(SI, si);
    c.set_reg(DX, g::FILENAME_BUF);
    c.ret()
}

/// `swap_byte_arrays` (99B2): swaps CX words between SI and DI (DS).
/// Exit: SI, DI past them, CX = 0, AX = the last word from SI's side;
/// flags from the last `add di, 2`.
pub fn swap_byte_arrays(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x99b2)?;
    let mut c = Ctx::new(m);
    loop {
        c.run(0x99b2, 5)?;
        let ours = c.lods(W, pc::cpu::DS);
        let di = c.reg(DI);
        let theirs = c.word(di);
        c.set_word(di, ours);
        c.set_reg(AX, theirs);
        let si = c.reg(SI);
        c.set_word(si.wrapping_sub(2), theirs);
        let di = pure::add(W, di, 2);
        c.set_reg(DI, di);
        let cx = c.reg(pc::cpu::CX).wrapping_sub(1);
        c.set_reg(pc::cpu::CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.run(0x99bd, 1)?;
    c.ret()
}

/// Copies the 4-word rectangle at SI to DI (`push ds; pop es; movsw` × 4).
fn copy_rect(c: &mut Ctx) {
    let ds = c.sreg(pc::cpu::DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    for _ in 0..4 {
        c.movs(W);
    }
}

/// `copy_rect_at_si_to_di` (5B99). Exit: ES = DS, SI and DI past it.
pub fn copy_rect_at_si_to_di(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5b99)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    copy_rect(&mut c);
    c.ret()
}

/// `copy_game_area_rect_to_unknown_rect` (5BA0): the game area's rectangle
/// to D83Ch.
pub fn copy_game_area_rect_to_unknown_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5ba0)?;
    let mut c = Ctx::new(m);
    c.run(start, 3 + 7)?;
    c.set_reg(SI, g::GAME_AREA_RECT);
    c.set_reg(DI, g::RECT_D83C);
    copy_rect(&mut c);
    c.ret()
}

/// `copy_game_area_rect_to_clip_rect` (5BA8): the game area's rectangle
/// to the clip rectangle (D834h).
pub fn copy_game_area_rect_to_clip_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5ba8)?;
    let mut c = Ctx::new(m);
    c.run(start, 3 + 7)?;
    c.set_reg(SI, g::GAME_AREA_RECT);
    c.set_reg(DI, g::DIRTY_RECT);
    copy_rect(&mut c);
    c.ret()
}

/// `loc_0d082`: the font with glyphs drawn by `sub_d0ff` (the tall font's
/// sizes).
pub fn font_select_tall_font_with_sprites(m: &mut Machine, start: u16) -> Result<(), Bail> {
    select_font(m, start, 0xd082, 0xd0ff, 0xceec)
}

/// `loc_0d08f`: glyphs drawn by `font_draw_glyph_func`, the sizes kept.
pub fn font_use_plain_glyphs(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd08f)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_word(g::FONT_DRAW, 0xd16a);
    c.ret()
}
