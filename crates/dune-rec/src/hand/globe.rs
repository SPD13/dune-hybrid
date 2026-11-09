//! The globe view (its background, decorations sliding open and shut, the
//! rotation table) and the frieze sides of the game screen.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DS, DX, ES, SI, SS},
    rec::Bail,
};

mod code {
    pub const MAP_CELL_AT: u16 = 0xb58b;
    pub const SET_FB1_AS_ACTIVE_FRAMEBUFFER: u16 = 0xc07c;
    pub const SET_SCREEN_AS_ACTIVE_FRAMEBUFFER: u16 = 0xc08e;
    pub const GFX_CALL_BP_WITH_FRONT_BUFFER_AS_SCREEN: u16 = 0xc097;
    pub const OPEN_RESOURCE_BY_INDEX: u16 = 0xc13e;
    pub const DRAW_SPRITE_CLOBBERING_BX_DX: u16 = 0xc22f;
    pub const DRAW_SPRITE_CLIPPED: u16 = 0xc305;
    pub const DRAW_SPRITE_CLIPPED_CLOBBERING: u16 = 0xc30d;
    pub const SET_SOME_MOUSE_RECT: u16 = 0xdaaa;
    pub const COPY_RECT_AT_SI_TO_DI: u16 = 0x5b99;
    pub const COPY_GAME_AREA_RECT_TO_CLIP_RECT: u16 = 0x5ba8;
    pub const COPY_GAME_RECT_FROM_FB2_TO_FB1: u16 = 0xc474;
    pub const COPY_GAME_AREA_TO_SCREEN_FB2_TO_FB1: u16 = 0xc43e;
    pub const SUB_C4DD: u16 = 0xc4dd;
    pub const LOAD_RESOURCE_TO: u16 = 0xf0b9;
    pub const SET_GLOBE_TILT_AND_ROTATION: u16 = 0xba75;
    pub const DRAW_GLOBE_SIDE_DECORATIONS: u16 = 0xb87e;
    pub const GLOBE_FILL_BLUE_BACKGROUND: u16 = 0xb84a;
    pub const REMOVE_GLOBE_TASK_BE57: u16 = 0xb93b;
    pub const UI_DRAW_DATE_AND_TIME_INDICATOR: u16 = 0x1a34;
    pub const FRIEZE_SIDES: u16 = 0xd795;
    pub const DRAW_UI_ELEMENTS_LIST: u16 = 0xd1f2;
}

/// The globe's frame (sprites of resource 1) and its rectangles.
const GLOBE_SHEET: u16 = 1;
const GLOBE_MOUSE_RECT: u16 = 0x2448;
const GLOBE_CLIP_RECT: u16 = 0x2440;
/// The globe's frame tasks.
const GLOBE_TASK: u16 = 0xb9ae;
const GLOBE_TASK_2: u16 = 0xbe57;
/// Steps of the globe's rotation (one turn).
const ROTATION_STEPS: u16 = 0x18e;
/// Rows of the rotation table.
const ROWS: u16 = 0x62;

/// From B85A: the globe with its atmosphere: the frame's sprite 2 at
/// (5Bh, 14h), the mouse and clip rectangles, then the globe itself.
fn globe_with_atmosphere(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xb85a, 2)?;
    c.set_reg(AX, GLOBE_SHEET);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0xb860)?;
    c.run(0xb860, 4)?;
    c.set_reg(DX, 0x5b);
    c.set_reg(BX, 0x14);
    c.set_reg(AX, 2);
    c.call(code::DRAW_SPRITE_CLOBBERING_BX_DX, 0xb86c)?;
    c.run(0xb86c, 2)?;
    c.set_reg(SI, GLOBE_MOUSE_RECT);
    c.call(code::SET_SOME_MOUSE_RECT, 0xb872)?;
    c.run(0xb872, 3)?;
    c.set_reg(SI, GLOBE_CLIP_RECT);
    c.set_reg(DI, g::RECT_D834);
    c.call(code::COPY_RECT_AT_SI_TO_DI, 0xb87b)?;
    c.run(0xb87b, 1)?; // jmp B977h
    super::gfx::map_func_gfx(c.m, 0xb977)
}

/// `draw_globe_with_atmosphere` (B85A).
pub fn draw_globe_with_atmosphere(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb85a)?;
    globe_with_atmosphere(&mut Ctx::new(m))
}

/// `globe_fill_blue_background_to_front_buffer` (B84A): the game area of
/// the front buffer in colour F0h (driver function 10), then the globe.
pub fn globe_fill_blue_background(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb84a)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SET_FB1_AS_ACTIVE_FRAMEBUFFER, 0xb84d)?;
    c.run(0xb84d, 3)?;
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.set_sreg(ES, fb);
    c.set_reg(SI, g::GAME_AREA_RECT);
    c.set_reg8(super::r8::AL, 0xf0);
    c.interp(0xb856, 0xb85a)?;
    globe_with_atmosphere(&mut c)
}

/// From B87E: the side decorations at [DD0Fh] from the left and
/// mirrored on the right, clipped to the game area; the clip rectangle
/// back to the globe's.
fn side_decorations(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xb87e, 1)?;
    c.call(code::COPY_GAME_AREA_RECT_TO_CLIP_RECT, 0xb881)?;
    c.run(0xb881, 2)?;
    c.set_reg(AX, GLOBE_SHEET);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0xb887)?;
    c.run(0xb887, 4)?;
    let x = c.word(g::GLOBE_DECORATIONS_X);
    c.set_reg(DX, x);
    for r in [BX, AX] {
        let v = c.reg(r);
        let zero = c.xor(W, v, v);
        c.set_reg(r, zero);
    }
    c.call(code::DRAW_SPRITE_CLIPPED, 0xb892)?;
    c.run(0xb892, 4)?;
    let dx = c.reg(DX);
    let dx = c.sub(W, dx, 0xd6);
    let dx = c.neg(W, dx);
    c.set_reg(DX, dx);
    c.set_reg(AX, 1);
    c.call(code::DRAW_SPRITE_CLIPPED_CLOBBERING, 0xb89e)?;
    c.run(0xb89e, 3)?;
    c.set_reg(SI, GLOBE_CLIP_RECT);
    c.set_reg(DI, g::RECT_D834);
    super::video::copy_rect_at_si_to_di(c.m, 0x5b99)
}

/// `draw_globe_side_decorations` (B87E).
pub fn draw_globe_side_decorations(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb87e)?;
    side_decorations(&mut Ctx::new(m))
}

/// `setup_globe_draw` (B8A7): loads the globe's map (resource 92h to
/// DS:4C60), sets its tilt and rotation from [197Ch], [197Eh], opens the
/// frame's sheet and updates the palette.
pub fn setup_globe_draw(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb8a7)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(DI, g::GLOBE_MAP);
    c.set_reg(SI, 0x92);
    c.call(code::LOAD_RESOURCE_TO, 0xb8b2)?;
    c.run(0xb8b2, 3)?;
    let (dx, bx) = (c.word(g::GLOBE_TILT_START), c.word(g::GLOBE_TILT_START + 2));
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.call(code::SET_GLOBE_TILT_AND_ROTATION, 0xb8bd)?;
    c.run(0xb8bd, 2)?;
    c.set_reg(AX, GLOBE_SHEET);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0xb8c3)?;
    c.run(0xb8c3, 1)?; // jmp C0F4h
    super::gfx::update_screen_palette(c.m, 0xc0f4)
}

/// `globe_slide_decorations_open` (B8F3): the globe drawn (with BE1Dh in
/// the front buffer), then the side decorations slide out, 10h a frame,
/// to -6Ah.
pub fn globe_slide_decorations_open(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb8f3)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::GLOBE_FILL_BLUE_BACKGROUND, 0xb8f6)?;
    c.run(0xb8f6, 2)?;
    c.set_reg(BP, 0xbe1d);
    c.call(code::GFX_CALL_BP_WITH_FRONT_BUFFER_AS_SCREEN, 0xb8fc)?;
    c.run(0xb8fc, 1)?;
    c.call(code::COPY_GAME_RECT_FROM_FB2_TO_FB1, 0xb8ff)?;
    loop {
        c.run(0xb8ff, 1)?;
        c.call(code::COPY_GAME_AREA_TO_SCREEN_FB2_TO_FB1, 0xb902)?;
        c.run(0xb902, 1)?;
        c.call(code::DRAW_GLOBE_SIDE_DECORATIONS, 0xb905)?;
        c.run(0xb905, 1)?;
        c.call(code::SUB_C4DD, 0xb908)?;
        let x = c.word(g::GLOBE_DECORATIONS_X).wrapping_sub(0x10);
        let more = (x as i16) > -0x6a;
        c.run(0xb908, if more { 3 } else { 4 })?;
        let x0 = c.word(g::GLOBE_DECORATIONS_X);
        let x = c.sub(W, x0, 0x10);
        c.set_word(g::GLOBE_DECORATIONS_X, x);
        c.cmp(W, x, 0xff96);
        if !more {
            return c.ret();
        }
    }
}

/// `globe_slide_decorations_close` (B915): the second globe task removed;
/// on the screen the side decorations slide in, 10h a frame, to 0; then
/// once more into the front buffer.
pub fn globe_slide_decorations_close(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb915)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::REMOVE_GLOBE_TASK_BE57, 0xb918)?;
    c.run(0xb918, 1)?;
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0xb91b)?;
    c.run(0xb91b, 2)?;
    loop {
        let x = c.word(g::GLOBE_DECORATIONS_X);
        let x = c.add(W, x, 0x10);
        c.set_word(g::GLOBE_DECORATIONS_X, x);
        c.call(code::DRAW_GLOBE_SIDE_DECORATIONS, 0xb923)?;
        let x = c.word(g::GLOBE_DECORATIONS_X);
        if x != 0 {
            c.run(0xb923, 4)?; // cmp; jne; add; call
            c.cmp(W, x, 0);
            continue;
        }
        c.run(0xb923, 3)?;
        c.cmp(W, x, 0);
        c.call(code::SET_FB1_AS_ACTIVE_FRAMEBUFFER, 0xb92d)?;
        c.run(0xb92d, 1)?; // jmp B87Eh
        return side_decorations(&mut c);
    }
}

/// `remove_globe_frame_tasks` (B930): no globe tasks (and [DD03h] = 0).
pub fn remove_globe_frame_tasks(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb930)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_byte(g::GLOBE_DD03, 0);
    c.set_reg(SI, GLOBE_TASK);
    c.call(0xda5f, 0xb93b)?;
    remove_task_2(&mut c)
}

/// B93B: removes the second globe task.
fn remove_task_2(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xb93b, 2)?;
    c.set_reg(SI, GLOBE_TASK_2);
    super::tasks::remove_frame_task(c.m, 0xda5f)
}

/// `loc_0b93b`.
pub fn sub_b93b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb93b)?;
    remove_task_2(&mut Ctx::new(m))
}

/// From B9F6: the rotation table: the step 8000h / 18Eh (with DX the
/// rotation as the division's high word), then for each of the 62h rows
/// at SI + 6, 6 bytes apart, the row's word × the step × 2, as a 32-bit
/// value after it.
fn rotation_table(c: &mut Ctx) -> Result<(), Bail> {
    if c.reg(DX) >= ROTATION_STEPS {
        // The division overflows: the interpreter takes it from here.
        c.m.cpu.ip = 0xb9f6;
        return Err(Bail);
    }
    c.run(0xb9f6, 5)?;
    c.set_reg(AX, 0x8000);
    c.set_reg(CX, ROTATION_STEPS);
    c.m.cpu.mul_div(&mut c.m.hw, W, 6, ROTATION_STEPS as u32); // div cx
    c.set_reg(CX, ROWS);
    let step = c.reg(AX);
    c.set_reg(BX, step);
    loop {
        let left = c.reg(CX).wrapping_sub(1);
        c.run(0xba03, if left == 0 { 9 } else { 8 })?;
        let si = c.reg(SI);
        let si = c.add(W, si, 6);
        c.set_reg(SI, si);
        c.lods(W, DS);
        c.mul16(step);
        let (ax, dx) = (c.reg(AX), c.reg(DX));
        let ax = c.add(W, ax, ax);
        let dx = c.adc(W, dx, dx);
        c.set_reg(AX, ax);
        c.set_reg(DX, dx);
        let si = c.reg(SI);
        c.set_word(si, dx);
        c.set_word(si.wrapping_add(2), ax);
        c.set_reg(CX, left);
        if left == 0 {
            return c.ret();
        }
    }
}

/// `recalculate_globe_rotation_table` (B9F6).
pub fn recalculate_globe_rotation_table(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb9f6)?;
    rotation_table(&mut Ctx::new(m))
}

/// `globe_rotation_increment_ax` (B9E0): the globe's rotation += AX
/// (wrapping in 0..18Eh), then its table.
pub fn globe_rotation_increment_ax(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb9e0)?;
    let mut c = Ctx::new(m);
    let rotation = c.word(g::GLOBE_ROTATION);
    let ax = c.reg(AX);
    let sum = rotation.wrapping_add(ax);
    let wrapped_low = sum & 0x8000 != 0;
    let r = if wrapped_low { sum.wrapping_add(ROTATION_STEPS) } else { sum };
    let wrapped_high = r.wrapping_sub(ROTATION_STEPS) & 0x8000 == 0;
    c.run(start, 5 + wrapped_low as u32 + 2 + wrapped_high as u32 + 1)?;
    c.set_reg(SI, g::GLOBE_ROTATION);
    c.set_reg(DX, rotation);
    c.set_reg(CX, ROTATION_STEPS);
    let mut dx = c.add(W, rotation, ax);
    if wrapped_low {
        dx = c.add(W, dx, ROTATION_STEPS);
    }
    c.cmp(W, dx, ROTATION_STEPS);
    if wrapped_high {
        dx = c.sub(W, dx, ROTATION_STEPS);
    }
    c.set_reg(DX, dx);
    c.set_word(g::GLOBE_ROTATION, dx);
    rotation_table(&mut c)
}

/// From D795: copies the 4 frieze sides at SI (4 bytes each) into the UI
/// elements at 1AEEh (0Eh apart), then draws the 3 elements at 1AE6h.
fn frieze_sides(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xd795, 4 + 4 * 4 + 3)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(DI, g::FRIEZE_ELEMENTS + 8);
    c.set_reg(CX, 4);
    for left in (0..4u16).rev() {
        c.movs(W);
        c.movs(W);
        let di = c.reg(DI);
        let di = c.add(W, di, 0x0a);
        c.set_reg(DI, di);
        c.set_reg(CX, left);
    }
    c.set_reg(SI, g::FRIEZE_ELEMENTS);
    c.set_reg(CX, 3);
    super::gfx::draw_ui_elements_list_at_ds_si(c.m, 0xd1f2)
}

/// `ui_set_and_draw_frieze_sides` (D795).
pub fn ui_set_and_draw_frieze_sides(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd795)?;
    frieze_sides(&mut Ctx::new(m))
}

/// A frieze: `mov si, sides` (and a `jmp`), then D795.
fn frieze(m: &mut Machine, start: u16, entry: u16, sides: u16, jumps: u32) -> Result<(), Bail> {
    entered(start, entry)?;
    let mut c = Ctx::new(m);
    c.run(start, 1 + jumps)?;
    c.set_reg(SI, sides);
    frieze_sides(&mut c)
}

/// `ui_set_and_draw_frieze_sides_map` (D792).
pub fn frieze_sides_map(m: &mut Machine, start: u16) -> Result<(), Bail> {
    frieze(m, start, 0xd792, 0x1c66, 0)
}

/// `ui_set_and_draw_frieze_sides_open_book` (D7AD).
pub fn frieze_sides_open_book(m: &mut Machine, start: u16) -> Result<(), Bail> {
    frieze(m, start, 0xd7ad, 0x1c56, 1)
}

/// `ui_set_and_draw_frieze_sides_globe` (D7B2).
pub fn frieze_sides_globe(m: &mut Machine, start: u16) -> Result<(), Bail> {
    frieze(m, start, 0xd7b2, 0x1c46, 1)
}

/// `ui_set_and_draw_frieze_sides_closed_book` (D75A): the closed book's
/// frieze, the date panel, then the book's two page counters: first both
/// at 40h, then at 41h + [1152h], [1153h].
pub fn frieze_sides_closed_book(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd75a)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(SI, 0x1c36);
    c.call(code::FRIEZE_SIDES, 0xd760)?;
    c.run(0xd760, 1)?;
    c.call(code::UI_DRAW_DATE_AND_TIME_INDICATOR, 0xd763)?;
    c.run(0xd763, 6)?;
    c.set_reg(SI, g::BOOK_COUNTERS);
    c.set_reg(AX, 0x40);
    c.set_word(g::BOOK_COUNTERS + 0x0a, 0x40);
    c.set_word(g::BOOK_COUNTERS + 0x18, 0x40);
    c.set_reg(CX, 2);
    c.call(code::DRAW_UI_ELEMENTS_LIST, 0xd775)?;
    c.run(0xd775, 11)?;
    c.set_reg(SI, g::BOOK_COUNTERS);
    for (from, to) in [(g::BOOK_PAGE, 0x0a), (g::BOOK_PAGE + 1, 0x18)] {
        let v = c.byte(from) as i8 as u16;
        c.set_reg(AX, v);
        let v = c.add(W, v, 0x41);
        c.set_reg(AX, v);
        c.set_word(g::BOOK_COUNTERS + to, v);
    }
    c.set_reg(CX, 2);
    super::gfx::draw_ui_elements_list_at_ds_si(c.m, 0xd1f2)
}

/// `loc_0bc4e`: whether the cursor (DX, BX) is on the globe, 80h x 6Dh at
/// (60h, 19h): CF set when it is; DX, BX made relative to it (as far as
/// the checks went).
pub fn on_globe(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbc4e)?;
    let mut c = Ctx::new(m);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let x = dx.wrapping_sub(0x60);
    let n = if dx < 0x60 {
        4
    } else if x >= 0x80 {
        6
    } else if bx < 0x19 {
        9
    } else {
        10
    };
    c.run(start, n)?;
    let x = c.sub(W, dx, 0x60);
    c.set_reg(DX, x);
    let cf = c.flag(CF);
    c.set_flag(CF, !cf);
    if dx < 0x60 {
        return c.ret();
    }
    c.cmp(W, x, 0x80);
    if x >= 0x80 {
        return c.ret();
    }
    let y = c.sub(W, bx, 0x19);
    c.set_reg(BX, y);
    let cf = c.flag(CF);
    c.set_flag(CF, !cf);
    if bx >= 0x19 {
        c.cmp(W, y, 0x6d);
    }
    c.ret()
}

/// `loc_0b532`: AL = the map byte at angle BX, distance DX (`map_func`
/// B58B: ES:DI to it). DX kept.
pub fn map_byte_at(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb532)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let dx = c.reg(DX);
    c.push(dx);
    c.call(code::MAP_CELL_AT, 0xb536)?;
    c.run(0xb536, 3)?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    let di = c.reg(DI);
    let v = c.byte_in(ES, di);
    c.set_reg8(r8::AL, v);
    c.ret()
}

/// `loc_0b5c5`: `map_func` (B58B), then DX = 10000h x DX / BP (DX:0 / BP).
pub fn map_cell_scaled(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb5c5)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::MAP_CELL_AT, 0xb5c8)?;
    let (dx, bp) = (c.reg(DX), c.reg(BP));
    if bp == 0 || dx >= bp {
        // `div bp` faults: the interpreter raises it.
        c.m.cpu.ip = 0xb5c8;
        return Err(Bail);
    }
    c.run(0xb5c8, 4)?;
    let ax = c.reg(AX);
    let z = c.xor(W, ax, ax);
    c.set_reg(AX, z);
    c.m.cpu.mul_div(&mut c.m.hw, W, 6, bp as u32); // div bp
    let q = c.reg(AX);
    c.set_reg(DX, q);
    c.ret()
}

/// The map's projection: the mark (centre) at 197Ch/197Eh and the
/// screen's centre at DCF6h/DCF8h; a second set at 1980h/DCFAh to swap
/// with.
const CENTRE: u16 = 0x197c;
const SCREEN_CENTRE: u16 = 0xdcf6;
const OTHER_CENTRE: u16 = 0x1980;
const OTHER_SCREEN_CENTRE: u16 = 0xdcfa;
/// The latitude table: 8-byte rows (SS for [BP+...]).
const LATITUDES: u16 = 0x4948;
/// Bit 7: the zoomed map (shifts by 2).
const HOTSPOT_KINDS: u16 = 0x46eb;

/// `loc_0b69a`: the two projection centres swapped.
pub fn swap_projection(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb69a)?;
    let mut c = Ctx::new(m);
    c.run(start, 13)?;
    let mut ax = 0;
    for (a, b) in [(CENTRE, OTHER_CENTRE), (CENTRE + 2, OTHER_CENTRE + 2), (SCREEN_CENTRE, OTHER_SCREEN_CENTRE), (SCREEN_CENTRE + 2, OTHER_SCREEN_CENTRE + 2)] {
        let (x, y) = (c.word(a), c.word(b));
        c.set_word(b, x);
        c.set_word(a, y);
        ax = y;
    }
    c.set_reg(AX, ax);
    c.ret()
}

/// `loc_0b647`: the screen point (DX, BX) of the map point DX (along),
/// BX (row): rows by the zoom, along scaled by the row's width (the
/// latitude table's word 2) about the centre.
pub fn map_to_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb647)?;
    let mut c = Ctx::new(m);
    let zoom = c.byte(HOTSPOT_KINDS) & 0x80 != 0;
    let bx = c.reg(BX);
    let row8 = bx.wrapping_mul(8);
    let neg = row8 & 0x8000 != 0;
    let cl = if zoom { 2 } else { 0 };
    c.run(start, 3 + zoom as u32 + 8 + neg as u32 + 6 + if zoom { 3 * 2 } else { 0 } + 2)?;
    c.xor(W, 0, 0);
    let k = c.byte(HOTSPOT_KINDS);
    c.test(B, k as u16, 0x80);
    c.set_reg(CX, cl);
    let cy = c.word(CENTRE + 2);
    let y = c.sub(W, bx, cy);
    let y = c.shl(W, y, cl as u8);
    let sy = c.word(SCREEN_CENTRE + 2);
    let y = c.add(W, y, sy);
    c.set_reg(BX, y);
    let mut bp = c.shl1(W, bx, 3);
    if neg {
        bp = c.neg(W, bp);
    }
    let w = c.word_in(SS, bp.wrapping_add(LATITUDES + 2));
    let w = c.add(W, w, w);
    c.set_reg(BP, w);
    let dx = c.reg(DX);
    let cx0 = c.word(CENTRE);
    let d = c.sub(W, dx, cx0);
    c.set_reg(AX, d);
    c.set_reg(DX, d);
    c.m.cpu.mul_div(&mut c.m.hw, W, 5, w as u32); // imul bp
    if zoom {
        for _ in 0..2 {
            let ax = c.reg(AX);
            let ax = c.shl1(W, ax, 1);
            c.set_reg(AX, ax);
            let dx = c.reg(DX);
            let dx = c.rcl1(W, dx);
            c.set_reg(DX, dx);
        }
        c.set_reg(CX, 0);
    }
    let dx = c.reg(DX);
    let sx = c.word(SCREEN_CENTRE);
    let dx = c.add(W, dx, sx);
    c.set_reg(DX, dx);
    c.ret()
}

/// `loc_0b683`: with SF from the caller: BP, DX made a span (DX the
/// larger part, at most -BP).
pub fn span_b683(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb683)?;
    let mut c = Ctx::new(m);
    let sf = c.flag(pc::cpu::SF);
    let (bp, dx) = (c.reg(BP), c.reg(DX));
    let (bp2, dx2) = if sf { (dx, dx.wrapping_add(bp)) } else { (dx.wrapping_sub(bp), dx) };
    let cx = bp2.wrapping_neg();
    let take = cx < dx2;
    c.run(start, if sf { 3 } else { 4 } + 4 + take as u32 + 1)?;
    if sf {
        c.set_reg(BP, dx);
        let d = c.add(W, bp, dx);
        c.set_reg(DX, d);
    } else {
        let b = c.sub(W, bp, dx);
        let b = c.neg(W, b);
        c.set_reg(BP, b);
    }
    let bp = c.reg(BP);
    let cx = c.neg(W, bp);
    c.set_reg(CX, cx);
    let dx = c.reg(DX);
    c.cmp(W, cx, dx);
    if take {
        c.set_reg(DX, bp);
    }
    c.ret()
}

/// `loc_0b5cf`: DX += AL (signed), BH += AH with the carry into BL (the
/// row), BL kept in 62h..9Eh (the step undone at the edge).
pub fn step_b5cf(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb5cf)?;
    let mut c = Ctx::new(m);
    let (ax, bx) = (c.reg(AX), c.reg(BX));
    let (ah, bh, bl) = ((ax >> 8) as u8, (bx >> 8) as u8, bx as u8);
    let n = if ah == 0 {
        5
    } else if ah & 0x80 == 0 {
        let (_, carry) = bh.overflowing_add(ah);
        if !carry { 8 } else if (bl.wrapping_add(1) as i8) < 0x62 { 11 } else { 14 }
    } else {
        let (_, carry) = bh.overflowing_add(ah);
        if carry { 8 } else if (bl.wrapping_sub(1) as i8) > (0x9e_u8 as i8) { 11 } else { 13 }
    };
    c.run(start, n)?;
    c.or(B, ah as u16, ah as u16);
    if ah != 0 {
        let (mut bh, mut bl) = (bh, bl);
        if ah & 0x80 == 0 {
            bh = c.add(B, bh as u16, ah as u16) as u8;
            if c.flag(pc::cpu::CF) {
                bl = c.inc(B, bl as u16) as u8;
                c.cmp(B, bl as u16, 0x62);
                if (bl as i8) >= 0x62 {
                    bl = c.dec(B, bl as u16) as u8;
                    bh = c.sub(B, bh as u16, ah as u16) as u8;
                }
            }
        } else {
            bh = c.add(B, bh as u16, ah as u16) as u8;
            if !c.flag(pc::cpu::CF) {
                bl = c.dec(B, bl as u16) as u8;
                c.cmp(B, bl as u16, 0x9e);
                if (bl as i8) <= (0x9e_u8 as i8) {
                    bl = c.inc(B, bl as u16) as u8;
                    bh = c.sub(B, bh as u16, ah as u16) as u8;
                }
            }
        }
        c.set_reg(BX, (bh as u16) << 8 | bl as u16);
    }
    let al = ax as u8 as i8 as u16;
    c.set_reg(AX, al);
    let dx = c.reg(DX);
    let dx = c.add(W, dx, al);
    c.set_reg(DX, dx);
    c.ret()
}

/// `loc_0b53b`: AX map bytes of the row BX centred on DX (wrapping at the
/// row's width BP) into 3-byte entries at SI (the byte, its offset); SI
/// after them. BX, CX, DX kept.
pub fn read_map_row(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb53b)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    for r in [BX, CX, DX, AX] {
        let v = c.reg(r);
        c.push(v);
    }
    c.call(code::MAP_CELL_AT, 0xb542)?;
    let sp = c.reg(pc::cpu::SP);
    let count = c.word_in(SS, sp);
    if count == 0 {
        // `loop` from 0: 65536 rounds; the interpreter does those.
        c.m.cpu.ip = 0xb542;
        return Err(Bail);
    }
    let (dx, di, bp) = (c.reg(DX), c.reg(DI), c.reg(BP));
    let half = count >> 1;
    let (dx1, borrow) = dx.overflowing_sub(half);
    // The loop's path, purely.
    let mut n = 6 + if borrow { 2 } else { 0 };
    let mut x = if borrow { dx1.wrapping_add(bp) } else { dx1 };
    for _ in 0..count {
        x = x.wrapping_add(1);
        n += 9;
        if x >= bp {
            x = x.wrapping_sub(bp);
            n += 2;
        }
    }
    n += 4;
    c.run(0xb542, n)?;
    let ax = c.pop();
    c.set_reg(CX, ax);
    let half = c.shr1(W, ax, 1);
    c.set_reg(AX, half);
    let mut di = c.sub(W, di, half);
    let mut dx = c.sub(W, dx, half);
    if borrow {
        dx = c.add(W, dx, bp);
        di = c.add(W, di, bp);
    }
    let mut si = c.reg(SI);
    for _ in 0..count {
        let v = c.byte_in(ES, di);
        c.set_reg8(r8::AL, v);
        c.m.cpu.write8(&mut c.m.hw, pc::cpu::DS, si, v);
        c.set_word(si.wrapping_add(1), di);
        si = c.add(W, si, 3);
        di = c.inc(W, di);
        dx = c.inc(W, dx);
        c.cmp(W, dx, bp);
        if dx >= bp {
            dx = c.sub(W, dx, bp);
            di = c.sub(W, di, bp);
        }
    }
    c.set_reg(SI, si);
    c.set_reg(DI, di);
    c.set_reg(CX, 0);
    for r in [DX, CX, BX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// `loc_0b56c`: CX rows (from BX - CX / 2, not above row -62h) of AX map
/// bytes each (`read_map_row`) into the buffer at 9E68h; SI = it.
pub fn read_map_rows(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb56c)?;
    let mut c = Ctx::new(m);
    let (cx, bx) = (c.reg(CX), c.reg(BX));
    if cx == 0 {
        return Err(Bail);
    }
    let top = bx.wrapping_sub(cx >> 1);
    let clamp = (top as i16) < -0x62;
    c.run(start, 6 + clamp as u32 + 1 + 2)?;
    c.push(cx);
    let half = c.shr1(W, cx, 1);
    c.set_reg(SI, half);
    let mut y = c.sub(W, bx, half);
    c.cmp(W, y, 0xff9e);
    if clamp {
        y = 0xff9e;
    }
    c.set_reg(BX, y);
    c.set_reg(SI, ROWS_BUFFER);
    loop {
        let ax = c.reg(AX);
        c.push(ax);
        c.call(0xb53b, 0xb582)?;
        let cx = c.reg(CX).wrapping_sub(1);
        c.run(0xb582, 3 + if cx == 0 { 3 } else { 2 })?;
        let ax = c.pop();
        c.set_reg(AX, ax);
        let bx = c.reg(BX);
        let bx = c.inc(W, bx);
        c.set_reg(BX, bx);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.set_reg(SI, ROWS_BUFFER);
    let cx = c.pop();
    c.set_reg(CX, cx);
    c.ret()
}

const ROWS_BUFFER: u16 = 0x9e68;

/// `loc_0b5f9`: the map point of the screen point (DX, BX), the reverse of
/// [`map_to_screen`]: BX the row, DX along it as a fraction of the row
/// (DX:0 / the row's width).
pub fn screen_to_map(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb5f9)?;
    let mut c = Ctx::new(m);
    let zoom = c.byte(HOTSPOT_KINDS) & 0x80 != 0;
    let cl = if zoom { 2u16 } else { 0 };
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let (sx, sy) = (c.word(SCREEN_CENTRE), c.word(SCREEN_CENTRE + 2));
    let row = ((bx.wrapping_sub(sy) as i16) >> cl) as u16;
    let row = row.wrapping_add(c.word(CENTRE + 2));
    let neg = row & 0x8000 != 0;
    let off = (if neg { row.wrapping_neg() } else { row }).wrapping_mul(8);
    let si = LATITUDES.wrapping_add(off);
    let d = dx.wrapping_sub(sx);
    // The rest of the path and the divisor.
    let (n2, div, num) = if zoom {
        let v = ((d as i16) >> cl) as u16;
        let v = v.wrapping_add(c.word(si.wrapping_add(4)));
        (3 + 1, cl, v.wrapping_add(cl))
    } else {
        let v = d.wrapping_add(c.word(si.wrapping_add(6)));
        let w = c.word(si.wrapping_add(2)).wrapping_shl(1);
        if v < w {
            (5, w, v)
        } else if v.wrapping_sub(w) & 0x8000 != 0 {
            (7, w, v.wrapping_add(w))
        } else {
            (8, w, v.wrapping_sub(w))
        }
    };
    if div == 0 || num >= div {
        // `div cx` faults: the interpreter raises it.
        return Err(Bail);
    }
    c.run(start, 5 + zoom as u32 + 4 + neg as u32 + 6 + n2 + 4)?;
    let dx = c.sub(W, dx, sx);
    let bx = c.sub(W, bx, sy);
    c.xor(W, 0, 0);
    let k = c.byte(HOTSPOT_KINDS);
    c.test(B, k as u16, 0x80);
    c.set_reg(CX, cl);
    let bx = c.m.cpu.shift(7, W, bx as u32, cl as u32) as u16; // sar bx, cl
    let cy = c.word(CENTRE + 2);
    let bx = c.add(W, bx, cy);
    c.set_reg(BX, bx);
    let mut ax = bx;
    if neg {
        ax = c.neg(W, ax);
    }
    let ax = c.shl1(W, ax, 3);
    let si = c.add(W, LATITUDES, ax);
    c.set_reg(SI, si);
    let mut dx = dx;
    if zoom {
        dx = c.m.cpu.shift(7, W, dx as u32, cl as u32) as u16; // sar dx, cl
        let a = c.word(si.wrapping_add(4));
        dx = c.add(W, dx, a);
        dx = c.add(W, dx, cl);
    } else {
        let a = c.word(si.wrapping_add(6));
        dx = c.add(W, dx, a);
        let w = c.word(si.wrapping_add(2));
        let w = c.shl1(W, w, 1);
        c.set_reg(CX, w);
        c.cmp(W, dx, w);
        if dx >= w {
            dx = if dx.wrapping_sub(w) & 0x8000 != 0 { c.add(W, dx, w) } else { c.sub(W, dx, w) };
        }
    }
    c.set_reg(DX, dx);
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.m.cpu.mul_div(&mut c.m.hw, W, 6, div as u32); // div cx
    let q = c.reg(AX);
    c.set_reg(DX, q);
    c.ret()
}

/// `loc_0b8ea`: the frame task B9AEh every tick.
pub fn add_task_b9ae(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb8ea)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_reg(SI, 0xb9ae);
    c.set_reg(BP, 1);
    super::tasks::add_frame_task(c.m, 0xda25)
}

/// `loc_0b98b`: one globe frame: `map_func_gfx`, `loc_0baf2`, the rect
/// D834h (`loc_0db74`, `loc_0c4ed`; BX, DX kept), `loc_0bc0c` when DX is
/// set, `loc_0db67`, then [`globe_rotation_increment_ax`] by 1.
pub fn globe_frame(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb98b)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xb977, 0xb98e)?;
    globe_frame_from_b98e(&mut c, 0xb98e, 0)
}

/// [`globe_frame`] from B98Eh, after `pre` instructions from `at`.
fn globe_frame_from_b98e(c: &mut Ctx, at: u16, pre: u32) -> Result<(), Bail> {
    c.run(at, pre + 1)?;
    c.call(0xbaf2, 0xb991)?;
    c.run(0xb991, 4)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.set_reg(SI, 0xd834);
    c.call(0xdb74, 0xb999)?;
    c.run(0xb999, 1)?;
    c.call(0xc4ed, 0xb99c)?;
    c.run(0xb99c, 5)?;
    let d = c.pop();
    c.set_reg(DX, d);
    let b = c.pop();
    c.set_reg(BX, b);
    c.or(W, d, d);
    if d != 0 {
        c.call(0xbc0c, 0xb9a5)?;
        c.run(0xb9a5, 1)?;
    }
    c.call(0xdb67, 0xb9a8)?;
    c.run(0xb9a8, 2)?;
    c.set_reg(AX, 1);
    globe_rotation_increment_ax(c.m, 0xb9e0)
}

/// `loc_0ba9e`: the globe turned toward the map position
/// (`get_map_position`, `loc_0b683`): steps of at most 20h around and
/// 18h up/down (`globe_rotation_increment_ax`, `loc_0ba15`,
/// [`globe_frame`]) until a step needed no clamping.
pub fn globe_turn_to(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xba9e)?;
    let mut c = Ctx::new(m);
    loop {
        c.run(0xba9e, 1)?;
        c.call(0x407e, 0xbaa1)?;
        c.run(0xbaa1, 6)?;
        c.set_reg(AX, 0x18e);
        let dx = c.reg(DX);
        c.mul16(dx);
        let b = c.word(0x494a);
        let bp = c.add(W, b, b);
        c.set_reg(BP, bp);
        let dx = c.reg(DX);
        let r = c.word(0x494c);
        let dx = c.sub(W, dx, r);
        c.set_reg(DX, dx);
        c.call(0xb683, 0xbab3)?;
        let (dx, bx) = (c.reg(DX), c.reg(BX));
        let top = c.word(0x2460);
        let y = bx.wrapping_sub(top);
        let (sx, sy) = (dx as i16, y as i16);
        let c1 = sx >= 0x20;
        let x1 = if c1 { 0x20 } else { sx };
        let c2 = x1 <= -0x20;
        let c3 = sy >= 0x18;
        let y1 = if c3 { 0x18 } else { sy };
        let c4 = y1 <= -0x18;
        let n = 4 + 2 * c1 as u32 + 3 + 2 * c2 as u32 + 4 + 2 * c3 as u32 + 3 + 2 * c4 as u32 + 4;
        c.run(0xbab3, n)?;
        let mut cx = 1u16;
        c.set_reg(AX, 0x20);
        let mut x = dx;
        c.cmp(W, x, 0x20);
        if c1 {
            x = 0x20;
            cx = c.inc(W, cx);
        }
        let a = c.neg(W, 0x20);
        c.cmp(W, x, a);
        if c2 {
            x = a;
            cx = c.inc(W, cx);
        }
        let mut yv = c.sub(W, bx, top);
        c.set_reg(AX, 0x18);
        c.cmp(W, yv, 0x18);
        if c3 {
            yv = 0x18;
            cx = c.inc(W, cx);
        }
        let a = c.neg(W, 0x18);
        c.cmp(W, yv, a);
        if c4 {
            yv = a;
            cx = c.inc(W, cx);
        }
        c.set_reg(DX, x);
        c.set_reg(BX, yv);
        c.set_reg(CX, cx);
        c.set_reg(AX, x);
        c.push(cx);
        c.push(yv);
        c.call(0xb9e0, 0xbae7)?;
        c.run(0xbae7, 2)?;
        let a = c.pop();
        c.set_reg(AX, a);
        c.call(0xba15, 0xbaeb)?;
        c.run(0xbaeb, 1)?;
        c.call(0xb98b, 0xbaee)?;
        let cx = c.word_in(SS, c.m.cpu.regs[pc::cpu::SP]).wrapping_sub(1);
        c.run(0xbaee, 2 + (cx == 0) as u32)?;
        c.pop();
        c.set_reg(CX, cx);
        if cx == 0 {
            return c.ret();
        }
    }
}

/// `globe_increment_tilt_by_ax`: the tilt [2460h] moved by AX (within
/// -62h..62h), then the row table at 8B77h (C4h words, ES = DS) rebuilt
/// for it.
pub fn globe_increment_tilt(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xba15)?;
    let mut c = Ctx::new(m);
    // A first pass counts (its flags undone), the second one writes.
    let saved = c.m.cpu.flags();
    let n = tilt_walk(&mut c, false, true);
    c.m.cpu.set_flags(saved);
    c.run(start, n?)?;
    tilt_walk(&mut c, true, true)?;
    c.ret()
}

/// The code of `globe_increment_tilt_by_ax` after its entry; with
/// `write`, the memory and registers too; without `head`, from BA2Dh
/// (the table for the tilt [2460h] as it is). Its instruction count.
fn tilt_walk(c: &mut Ctx, write: bool, head: bool) -> Result<u32, Bail> {
    let mut n = 0;
    let mut ax;
    let mut cx;
    if head {
        let ax0 = c.reg(AX);
        let t = c.word(0x2460);
        ax = c.add(W, ax0, t);
        cx = 0x62u16;
        c.cmp(W, ax, cx);
        n = 4;
        if (ax as i16) >= 0x62 {
            ax = cx;
            n += 1;
        }
        cx = c.neg(W, cx);
        c.cmp(W, ax, cx);
        n += 3 + 1;
        if (ax as i16) <= (cx as i16) {
            ax = cx;
            n += 1;
        }
        if write {
            c.set_word(0x2460, ax);
        }
    } else {
        ax = c.word(0x2460);
    }
    if write {
        let ds = c.sreg(DS);
        c.push(ds);
        let es = c.pop();
        c.set_sreg(ES, es);
    }
    let step = c.string_step(W);
    let mut di = 0x8b77u16;
    let mut bx = 0x62u16;
    cx = 0xc4;
    ax = c.add(W, ax, bx);
    c.cmp(W, ax, bx);
    n += 9;
    let lo = |ax: u16, v: u16| ax & 0xff00 | v & 0xff;
    let mut l = if (ax as i16) <= (bx as i16) { 0xba55u16 } else { 0xba41 };
    loop {
        if n > 400_000 {
            return Err(Bail);
        }
        let mut store = |c: &mut Ctx, ax: u16| {
            if write {
                c.set_word_in(ES, di, ax);
            }
            di = di.wrapping_add(step);
        };
        match l {
            0xba41 => {
                bx = c.neg(W, bx);
                let a = c.add(B, ax & 0xff, bx & 0xff);
                let a = c.add(B, a, bx & 0xff);
                ax = lo(ax, a);
                n += 3;
                l = 0xba47;
            }
            0xba47 => {
                let a = c.dec(B, ax & 0xff);
                ax = lo(ax, a);
                store(c, ax);
                c.cmp(B, a, bx & 0xff);
                n += 4;
                if (a as u8 as i8) <= (bx as u8 as i8) {
                    l = 0xba50;
                } else {
                    n += 1;
                    cx = cx.wrapping_sub(1);
                    l = if cx != 0 { 0xba47 } else { 0xba50 };
                }
            }
            0xba50 => {
                bx = c.neg(W, bx);
                ax = bx;
                cx = c.dec(W, cx);
                n += 3;
                l = 0xba55;
            }
            0xba55 => {
                store(c, ax);
                let a = c.dec(B, ax & 0xff);
                ax = lo(ax, a);
                n += 3;
                if a & 0x80 != 0 {
                    l = 0xba5c;
                } else {
                    n += 1;
                    cx = cx.wrapping_sub(1);
                    l = if cx != 0 { 0xba55 } else { 0xba5c };
                }
            }
            0xba5c | 0xba6a => {
                cx = c.dec(W, cx);
                n += 2;
                if (cx as i16) <= 0 {
                    n += 1;
                    break;
                }
                n += 1;
                if l == 0xba5c {
                    ax = c.neg(W, ax);
                    l = 0xba61;
                } else {
                    let a = c.neg(B, ax & 0xff);
                    ax = lo(ax, a);
                    l = 0xba6f;
                }
            }
            0xba61 => {
                store(c, ax);
                let a = c.inc(B, ax & 0xff);
                ax = lo(ax, a);
                c.cmp(B, a, bx & 0xff);
                n += 4;
                if (a as u8 as i8) > (bx as u8 as i8) {
                    l = 0xba6a;
                } else {
                    n += 1;
                    cx = cx.wrapping_sub(1);
                    l = if cx != 0 { 0xba61 } else { 0xba6a };
                }
            }
            _ => {
                let a = c.inc(B, ax & 0xff);
                ax = lo(ax, a);
                store(c, ax);
                n += 3;
                cx = cx.wrapping_sub(1);
                if cx == 0 {
                    n += 1;
                    break;
                }
            }
        }
    }
    if write {
        c.set_reg(AX, ax);
        c.set_reg(BX, bx);
        c.set_reg(CX, cx);
        c.set_reg(DI, di);
    }
    Ok(n)
}

/// `loc_0b9b9`, `loc_0b9c0`, `loc_0b9cc`, `loc_0b9d3`: the globe's
/// arrow buttons: with bit 1 of AX (the click), the tilt by 8 / -8
/// (`globe_increment_tilt_by_ax`) or the rotation by -20h / 20h
/// (`globe_rotation_increment_ax`), then a [`globe_frame`].
pub fn globe_tilt_down(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb9b9)?;
    globe_button(m, start, 3, 8, 0xba15, 0xb9c5)
}

pub fn globe_tilt_up(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb9c0)?;
    globe_button(m, start, 2, 0xfff8, 0xba15, 0xb9c5)
}

pub fn globe_turn_left(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb9cc)?;
    globe_button(m, start, 3, 0xffe0, 0xb9e0, 0xb9d8)
}

pub fn globe_turn_right(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb9d3)?;
    globe_button(m, start, 2, 0x20, 0xb9e0, 0xb9d8)
}

fn globe_button(m: &mut Machine, start: u16, pre: u32, step: u16, by: u16, jnb: u16) -> Result<(), Bail> {
    let mut c = Ctx::new(m);
    let ax = c.reg(AX);
    let click = ax & 1 != 0;
    c.run(start, pre + 1 + click as u32)?;
    c.shr1(W, ax, 1);
    c.set_reg(AX, step);
    if !click {
        c.run(0xb9df, 1)?;
        return c.ret();
    }
    c.call(by, jnb + 5)?;
    c.run(jnb + 5, 1)?;
    globe_frame(c.m, 0xb98b)
}

/// `set_globe_tilt_and_rotation`: the rotation [494Ch] = DX·18Eh / 64K
/// ([494Eh] 0) and the tilt [2460h] = BX (unsigned, 20h..FFE0h), then
/// `loc_0b9f6` and the row table for the tilt (as
/// [`globe_increment_tilt`]).
pub fn set_globe_tilt_and_rotation(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xba75)?;
    let mut c = Ctx::new(m);
    let bx = c.reg(BX);
    let lo = bx < 0x20;
    let a = if lo { 0x20 } else { bx };
    let hi = a >= 0xffe0;
    let a = if hi { 0xffe0 } else { a };
    c.run(start, 8 + lo as u32 + 2 + hi as u32 + 2)?;
    c.set_reg(SI, 0x494c);
    c.set_reg(AX, 0x18e);
    let dx = c.reg(DX);
    c.mul16(dx);
    let d = c.reg(DX);
    c.set_word(0x494c, d);
    c.set_word(0x494e, 0);
    c.cmp(W, bx, 0x20);
    let a1 = if lo { 0x20 } else { bx };
    c.cmp(W, a1, 0xffe0);
    c.set_reg(AX, a);
    c.set_word(0x2460, a);
    c.call(0xb9f6, 0xba9c)?;
    let saved = c.m.cpu.flags();
    let n = tilt_walk(&mut c, false, false);
    c.m.cpu.set_flags(saved);
    c.run(0xba9c, 1 + n?)?;
    tilt_walk(&mut c, true, false)?;
    c.ret()
}

/// `sub_b9ae`: the driver's globe drag (far [3915h], ES the front
/// buffer); when it moved (CF), a [`globe_frame`] without its first step.
pub fn globe_drag(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb9ae)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let es = c.word(0xdbd6);
    c.set_sreg(ES, es);
    c.interp(0xb9b2, 0xb9b6)?;
    if !c.flag(CF) {
        c.run(0xb9b6, 2)?;
        return c.ret();
    }
    globe_frame_from_b98e(&mut c, 0xb9b6, 1)
}

/// `draw_globe_cursor_at_dx_bx`: on the screen, the icon sprites loaded,
/// the cursor sprite 36h drawn at (DX, BX less its height byte).
pub fn draw_globe_cursor_at_dx_bx(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbc0c)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xc08e, 0xbc0f)?;
    c.run(0xbc0f, 1)?;
    c.call(0xc137, 0xbc12)?;
    c.run(0xbc12, 2)?;
    c.set_reg(AX, 0x36);
    c.call(0xc1f4, 0xbc18)?;
    c.run(0xbc18, 2)?;
    let si = c.reg(SI);
    let h = c.byte_in(ES, si.wrapping_add(2));
    let bl = c.reg8(r8::BL);
    let v = c.sub(B, bl as u16, h as u16);
    c.set_reg8(r8::BL, v as u8);
    super::draw::draw_sprite_clobbering_bx_dx(c.m, 0xc22f)
}

/// `sub_bc64`: a click at (DX, BX) on the globe ([`on_globe`]) that hits
/// a location (`loc_0bd25`, CF): its point (AX, BX) kept in [197Ch],
/// the globe frame drawn, `loc_0dbb2`, `loc_0bc99`, `loc_0adbe`, then
/// into the map view (`ui_transition_to_map_interface`). DX, BX kept.
pub fn globe_click(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbc64)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.call(0xbc4e, 0xbc69)?;
    let miss = |c: &mut Ctx, at: u16| -> Result<(), Bail> {
        c.run(at, 4)?;
        let d = c.pop();
        c.set_reg(DX, d);
        let b = c.pop();
        c.set_reg(BX, b);
        c.ret()
    };
    if !c.flag(CF) {
        return miss(&mut c, 0xbc69);
    }
    c.run(0xbc69, 2)?;
    c.call(0xbd25, 0xbc6e)?;
    if !c.flag(CF) {
        return miss(&mut c, 0xbc6e);
    }
    c.run(0xbc6e, 4)?;
    let (ax, bx) = (c.reg(AX), c.reg(BX));
    c.set_word(0x197c, ax);
    c.set_word(0x197e, bx);
    c.call(0xb98b, 0xbc7a)?;
    c.run(0xbc7a, 4)?;
    let d = c.pop();
    c.set_reg(DX, d);
    let b = c.pop();
    c.set_reg(BX, b);
    c.call(0xdbb2, 0xbc90)?;
    c.run(0xbc90, 1)?;
    c.call(0xbc99, 0xbc93)?;
    c.run(0xbc93, 1)?;
    c.call(0xadbe, 0xbc96)?;
    c.run(0xbc96, 1)?;
    super::ui::ui_transition_to_map_interface(c.m, 0x5a3d)
}

/// `sub_b8c6`: into the globe: `loc_0d2bd`, out of the map view, the
/// globe set up (`loc_0b8a7`), [0DD03h] up, the globe drawn by
/// `loc_0c108` (AL 0, DX FFFFh, BP B827h), `loc_0ae04`, the click handler
/// 2562h, `loc_017e6`, and the frame task B9AEh (every frame).
pub fn enter_globe(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb8c6)?;
    let mut c = Ctx::new(m);
    for (at, to) in [(start, 0xd2bdu16), (0xb8c9, 0x5adf), (0xb8cc, 0xb8a7)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    c.run(0xb8cf, 5)?;
    let v = c.byte(0xdd03);
    let v = c.inc(B, v as u16);
    c.set_byte(0xdd03, v as u8);
    c.xor(B, 0, 0);
    c.set_reg8(r8::AL, 0);
    c.set_reg(DX, 0xffff);
    c.set_reg(BP, 0xb827);
    c.call(0xc108, 0xb8de)?;
    c.run(0xb8de, 1)?;
    c.call(0xae04, 0xb8e1)?;
    c.run(0xb8e1, 2)?;
    c.set_reg(AX, 0x2562);
    c.call(0xd95e, 0xb8e7)?;
    c.run(0xb8e7, 1)?;
    c.call(0x17e6, 0xb8ea)?;
    c.run(0xb8ea, 3)?;
    c.set_reg(SI, 0xb9ae);
    c.set_reg(BP, 1);
    super::tasks::add_frame_task(c.m, 0xda25)
}

/// `map2_resource_func` (start-up): the map resource 3Ah's histogram of
/// its C5F9h cell bytes (256 words at 4C60h, from 7); each location
/// (from 100h, 1Ch apart, to an FFh) placed on the map (`map_cell_scaled`:
/// its x, its cell +6, the cell marked 40h), its terrain +10h (the cell
/// byte of [0DBB2h]) and the terrain's rarity +11h (its count / 16);
/// then for each location `loc_06603` with the callback 01E0h. Bails with
/// DF set.
pub fn map2_resource_func(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0169)?;
    let mut c = Ctx::new(m);
    if c.flag(pc::cpu::DF) {
        return Err(Bail);
    }
    c.run(start, 2)?;
    c.set_reg(AX, 0x3a);
    c.call(0xc13e, 0x016f)?;
    if c.flag(pc::cpu::DF) {
        c.m.cpu.ip = 0x016f;
        return Err(Bail);
    }
    c.run(0x016f, 7)?;
    let ds = c.sreg(DS);
    c.set_sreg(ES, ds);
    c.set_reg(DI, 0x4c60);
    c.push(0x4c60);
    c.set_reg(AX, 7);
    c.set_reg(CX, 0x100);
    c.rep(0xab);
    c.run(0x017d, 3)?;
    let d = c.pop();
    c.set_reg(DI, d);
    let (p, s) = (c.word(0xdbb0), c.word(0xdbb2));
    c.set_reg(SI, p);
    c.set_sreg(ES, s);
    c.set_reg(CX, 0xc5f9);
    c.run(0x0185, 5 * 0xc5f9)?;
    for i in 0..0xc5f9u16 {
        let v = c.lods(B, ES);
        let bx = c.shl1(W, v, 1);
        c.set_reg(BX, bx);
        let a = bx.wrapping_add(d);
        let w = c.word(a);
        let w = c.inc(W, w);
        c.set_word(a, w);
        c.set_reg(CX, 0xc5f8 - i);
    }
    c.run(0x018f, 1)?;
    c.set_reg(SI, 0x100);
    let mut k = 0;
    loop {
        k += 1;
        if k > 400 {
            c.m.cpu.ip = 0x0192;
            return Err(Bail);
        }
        c.run(0x0192, 3)?;
        let si = c.reg(SI);
        let (dx, bx) = (c.word(si.wrapping_add(2)), c.word(si.wrapping_add(4)));
        c.set_reg(DX, dx);
        c.set_reg(BX, bx);
        c.call(0xb5c5, 0x019b)?;
        let si = c.reg(SI);
        let nx = c.byte(si.wrapping_add(0x1c));
        c.run(0x019b, 16)?;
        let (dx, di) = (c.reg(DX), c.reg(DI));
        c.set_word(si.wrapping_add(2), dx);
        c.set_word(si.wrapping_add(6), di);
        let v = c.byte_in(ES, di);
        let v = c.or(B, v as u16, 0x40);
        c.m.cpu.write8(&mut c.m.hw, ES, di, v as u8);
        let es = c.word(0xdbb2);
        c.set_sreg(ES, es);
        let t = c.byte_in(ES, di);
        c.set_reg8(r8::AL, t);
        c.set_byte(si.wrapping_add(0x10), t);
        c.xor(W, 0, 0);
        let bx = c.shl1(W, t as u16, 1);
        c.set_reg(BX, bx);
        let n = c.word(bx.wrapping_add(0x4c60));
        c.set_reg8(r8::CL, 4);
        let n = c.m.cpu.shift(5, W, n as u32, 4) as u16;
        c.set_reg(AX, n);
        c.set_byte(si.wrapping_add(0x11), n as u8);
        let s = c.add(W, si, 0x1c);
        c.set_reg(SI, s);
        c.cmp(B, nx as u16, 0xff);
        if nx == 0xff {
            break;
        }
    }
    c.run(0x01c8, 1)?;
    c.set_reg(DI, 0x100);
    loop {
        c.run(0x01cb, 4)?;
        c.set_reg(BP, 0x01e0);
        let di = c.reg(DI);
        let (dx, bx) = (c.word(di.wrapping_add(2)), c.word(di.wrapping_add(4)));
        c.set_reg(DX, dx);
        c.set_reg(BX, bx);
        c.call(0x6603, 0x01d7)?;
        let di = c.reg(DI);
        let nx = c.byte(di.wrapping_add(0x1c));
        c.run(0x01d7, if nx == 0xff { 4 } else { 3 })?;
        let d = c.add(W, di, 0x1c);
        c.set_reg(DI, d);
        c.cmp(B, nx as u16, 0xff);
        if nx == 0xff {
            return c.ret();
        }
    }
}

/// `loc_0b941`: the globe's UI list 204Ah with its button B1h / B96Bh
/// (B2h / B961h with [0DD02h]) installed (handler D917h).
pub fn globe_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb941)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0xdd02);
    c.run(start, 5 + if k != 0 { 2 } else { 0 } + 4)?;
    c.set_reg(BP, 0x204a);
    let mut ax = 0xb1u16;
    let mut bx = 0xb96bu16;
    c.cmp(B, k as u16, 0);
    if k != 0 {
        ax = c.inc(W, ax);
        bx = 0xb961;
    }
    c.set_reg(AX, ax);
    c.set_word_in(SS, 0x2050, ax);
    c.set_word_in(SS, 0x2052, bx);
    c.set_reg(BX, 0xd917);
    super::cursor::install_ui_list_d338(c.m, 0xd338)
}

/// `sub_bc1f`: with the globe's list (204Ah) or the map list (2062h)
/// current (`loc_0d41b`), the one for the cursor ([`on_globe`], CF on
/// the globe) installed when it changes.
pub fn globe_or_map_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xbc1f)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xd41b, 0xbc22)?;
    let bp = c.reg(BP);
    if bp != 0x204a && bp != 0x2062 {
        c.run(0xbc22, 5)?;
        c.cmp(W, bp, 0x204a);
        c.cmp(W, bp, 0x2062);
        return c.ret();
    }
    c.run(0xbc22, if bp == 0x204a { 4 } else { 5 })?;
    c.cmp(W, bp, 0x204a);
    if bp != 0x204a {
        c.cmp(W, bp, 0x2062);
    }
    c.call(0xbc4e, 0xbc31)?;
    let bp = c.reg(BP);
    if c.flag(CF) {
        if bp == 0x2062 {
            c.run(0xbc31, 4)?;
            c.cmp(W, bp, 0x2062);
            return c.ret();
        }
        c.run(0xbc31, 7)?;
        c.cmp(W, bp, 0x2062);
        c.set_reg(BP, 0x2062);
        c.set_reg(BX, 0xd917);
        c.xor(W, 0, 0);
        c.set_reg(CX, 0);
        return super::cursor::install_ui_list(c.m, 0xd33a);
    }
    c.run(0xbc31, 4)?;
    c.cmp(W, bp, 0x204a);
    if bp == 0x204a {
        return c.ret();
    }
    globe_list(c.m, 0xb941)
}
