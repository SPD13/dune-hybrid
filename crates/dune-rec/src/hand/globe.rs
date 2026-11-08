//! The globe view (its background, decorations sliding open and shut, the
//! rotation table) and the frieze sides of the game screen.

use super::{Ctx, W, entered, globals as g};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CX, DI, DS, DX, ES, SI},
    rec::Bail,
};

mod code {
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
