//! The intro sequence's steps: videos, frame tasks, the talking head.
//! Most end in `clc; ret`: CF clear tells the intro script to go on.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DX, SI, ZF},
    rec::Bail,
};

/// Code offsets the intro calls.
mod code {
    pub const CLEAR_GLOBAL_Y_OFFSET: u16 = 0x0579;
    pub const OPEN_SKY_PALETTE_AND_DRAW: u16 = 0x0802;
    /// Plays the talking head (with BX the head's frames).
    pub const TALKING_HEAD: u16 = 0x9efd;
    pub const SUB_3901: u16 = 0x3901;
    pub const SET_SCREEN_AS_ACTIVE_FRAMEBUFFER: u16 = 0xc08e;
    pub const UPDATE_SCREEN_PALETTE: u16 = 0xc0f4;
    pub const DRAW_SPRITE_CLOBBERING_BX_DX: u16 = 0xc22f;
    pub const SUB_C868: u16 = 0xc868;
    pub const HNM_DO_FRAME_SKIPPABLE: u16 = 0xc9e8;
    pub const HNM_DO_FRAME: u16 = 0xca60;
    pub const CHECK_IF_HNM_COMPLETE: u16 = 0xcc85;
    pub const ADD_FRAME_TASK: u16 = 0xda25;
    /// The frame task that plays the next HNM frame ([`super::sub_070c`]).
    pub const HNM_FRAME_TASK: u16 = 0x070c;
    /// The desert sky animation's frame task.
    pub const DESERT_SKY_TASK: u16 = 0x0826;
}

/// `clc; ret`, the end of most steps.
fn go_on(c: &mut Ctx, at: u16) -> Result<(), Bail> {
    c.run(at, 2)?;
    c.set_flag(CF, false);
    c.ret()
}

/// `clear_global_y_offset` (0579): the screen's vertical offset to 0
/// (graphics driver function 33). Exit: AX = 0.
pub fn clear_global_y_offset(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0579)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let ax = c.reg(AX);
    let zero = c.xor(W, ax, ax);
    c.set_reg(AX, zero);
    c.interp(0x057b, 0x057f)?; // call far [gfxVtable33]
    c.run(0x057f, 1)?;
    c.ret()
}

/// Start an HNM video on screen: no vertical offset, draw to the screen.
fn hnm_start(c: &mut Ctx, at: u16) -> Result<(), Bail> {
    c.run(at, 1)?;
    c.call(code::CLEAR_GLOBAL_Y_OFFSET, at + 3)?;
    c.run(at + 3, 1)?;
    c.call(code::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, at + 6)
}

/// `intro_play_hnm_86_frames` (06AA): plays the current video up to its
/// frame 86 (56h), or until a key or click (CF set then).
pub fn intro_play_hnm_86_frames(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x06aa)?;
    let mut c = Ctx::new(m);
    hnm_start(&mut c, 0x06aa)?;
    loop {
        c.run(0x06b0, 1)?;
        c.call(code::HNM_DO_FRAME_SKIPPABLE, 0x06b3)?;
        if c.flag(CF) {
            c.run(0x06b3, 2)?; // jb; ret
            return c.ret();
        }
        let frame = c.word(g::HNM_FRAME);
        c.run(0x06b3, if frame == 0x56 { 4 } else { 3 })?;
        c.cmp(W, frame, 0x56);
        if frame == 0x56 {
            return c.ret();
        }
    }
}

/// `intro_play_hnm_skippable` (06BD): plays the current video to its end,
/// or until a key or click (CF set then).
pub fn intro_play_hnm_skippable(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x06bd)?;
    hnm_skippable(&mut Ctx::new(m))
}

/// `play_PRESENT_HNM` (0684): `jmp intro_play_hnm_skippable`.
pub fn play_present_hnm(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0684)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    hnm_skippable(&mut c)
}

fn hnm_skippable(c: &mut Ctx) -> Result<(), Bail> {
    hnm_start(c, 0x06bd)?;
    loop {
        c.run(0x06c3, 1)?;
        c.call(code::HNM_DO_FRAME_SKIPPABLE, 0x06c6)?;
        if c.flag(CF) {
            c.run(0x06c6, 2)?; // jb 06BC; ret
            return c.ret();
        }
        c.run(0x06c6, 2)?;
        c.call(code::CHECK_IF_HNM_COMPLETE, 0x06cb)?;
        if !c.flag(ZF) {
            c.run(0x06cb, 2)?; // je; ret
            return c.ret();
        }
        c.run(0x06cb, 1)?;
    }
}

/// Play the current video from a frame task: register [`code::HNM_FRAME_TASK`]
/// (BP = 0: every frame), then run it once.
fn hnm_with_frame_task(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x0704, 3)?;
    let bp = c.reg(BP);
    let zero = c.xor(W, bp, bp);
    c.set_reg(BP, zero);
    c.set_reg(SI, code::HNM_FRAME_TASK);
    c.call(code::ADD_FRAME_TASK, 0x070c)?;
    hnm_frame_task(c)
}

/// The HNM frame task (070C): the next frame.
fn hnm_frame_task(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x070c, 1)?;
    c.call(code::HNM_DO_FRAME, 0x070f)?;
    go_on(c, 0x070f)
}

/// `intro_play_hnm_with_frame_task` (0704).
pub fn intro_play_hnm_with_frame_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0704)?;
    hnm_with_frame_task(&mut Ctx::new(m))
}

/// `sub_070c`: the HNM frame task.
pub fn sub_070c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x070c)?;
    hnm_frame_task(&mut Ctx::new(m))
}

/// `sub_06fc`: frame tasks to 3Fh, `sub_3901`, then the video from a frame
/// task.
pub fn sub_06fc(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x06fc)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(g::FRAME_TASKS_ACTIVE, 0x3f);
    c.call(code::SUB_3901, 0x0704)?;
    hnm_with_frame_task(&mut c)
}

/// The talking head, 400 (190h) ticks.
fn talking_head(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x0798, 2)?;
    c.set_word(g::TALKING_HEAD_TIME, 0x190);
    c.call(code::TALKING_HEAD, 0x07a1)?;
    go_on(c, 0x07a1)
}

/// `intro_talking_head_play` (0798).
pub fn intro_talking_head_play(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0798)?;
    talking_head(&mut Ctx::new(m))
}

/// `sub_078d`: the screen palette, then `sub_c868` with [DBE6h] = 6, then
/// the talking head.
pub fn sub_078d(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x078d)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::UPDATE_SCREEN_PALETTE, 0x0790)?;
    c.run(0x0790, 2)?;
    c.set_byte(g::BYTE_DBE6, 6);
    c.call(code::SUB_C868, 0x0798)?;
    talking_head(&mut c)
}

/// The desert sky animation as a frame task, every `every` frames.
fn desert_sky(m: &mut Machine, start: u16, entry: u16, every: u16) -> Result<(), Bail> {
    entered(start, entry)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_reg(SI, code::DESERT_SKY_TASK);
    c.set_reg(BP, every);
    c.call(code::ADD_FRAME_TASK, entry + 9)?;
    go_on(&mut c, entry + 9)
}

/// `intro_play_desert_sky_animation` (085D): every 9 frames.
pub fn intro_play_desert_sky_animation(m: &mut Machine, start: u16) -> Result<(), Bail> {
    desert_sky(m, start, 0x085d, 9)
}

/// `sub_087b`: the desert sky animation every 16 frames.
pub fn sub_087b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    desert_sky(m, start, 0x087b, 0x10)
}

/// `intro_midnight` (0868): the sky at midnight (palette 0Bh), then
/// sprite 7 at (13h, 4Ah).
pub fn intro_midnight(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0868)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg8(super::r8::BL, 0x0b);
    c.call(code::OPEN_SKY_PALETTE_AND_DRAW, 0x086d)?;
    c.run(0x086d, 4)?;
    c.set_reg(AX, 7);
    c.set_reg(DX, 0x13);
    c.set_reg(BX, 0x4a);
    c.call(code::DRAW_SPRITE_CLOBBERING_BX_DX, 0x0879)?;
    go_on(&mut c, 0x0879)
}

/// More code offsets.
mod more {
    pub const SET_FB1_AS_ACTIVE_FRAMEBUFFER: u16 = 0xc07c;
    pub const GFX_CLEAR_ACTIVE_FRAMEBUFFER: u16 = 0xc0ad;
    pub const OPEN_RESOURCE_BY_INDEX: u16 = 0xc13e;
    pub const COPY_ACTIVE_FRAMEBUFFER_TO_FRAMEBUFFER_2: u16 = 0xc412;
    pub const COPY_GAME_AREA_TO_SCREEN_FB2_TO_FB1: u16 = 0xc43e;
    pub const GFX_COPY_WHOLE_FRAMEBUF_TO_SCREEN: u16 = 0xc4cd;
    pub const SUB_C4DD: u16 = 0xc4dd;
    pub const DRAW_SPRITE_CLIPPED: u16 = 0xc305;
    pub const HNM_DO_FRAME_AND_CHECK_IF_FRAME_ADVANCED: u16 = 0xc9f4;
    pub const COPY_GAME_AREA_RECT_TO_CLIP_RECT: u16 = 0x5ba8;
    pub const MIDI_FUNC_2_0: u16 = 0xaeb7;
    pub const PLAY_PCM_AL: u16 = 0xabdb;
    pub const STC_ON_USER_INPUT: u16 = 0xdd63;
    pub const OPEN_SKYRS_PALETTE: u16 = 0x0820;
    pub const ROOM_SETUP: u16 = 0x08f0;
    pub const ROOM_DRAW: u16 = 0x37b2;
}

/// The company logo videos: from the front buffer, frame by frame until
/// the end, each new frame copied to the screen; CF set if a key or click
/// skipped it. Virgin's starts the music once the MIDI driver is past its
/// set-up (function 5 gave 8 or more and [DBCBh] is set).
fn play_logo(c: &mut Ctx, entry: u16, music: bool) -> Result<(), Bail> {
    // The offsets after the entry's: VIRGIN at 0625, CRYO at 0661.
    let at = |o: u16| entry + o;
    c.run(entry, 1)?;
    c.call(more::SET_FB1_AS_ACTIVE_FRAMEBUFFER, at(3))?;
    loop {
        c.run(at(3), 1)?;
        c.call(more::STC_ON_USER_INPUT, at(6))?;
        if c.flag(CF) {
            c.run(at(6), 2)?; // jb 064Ch; ret
            return c.ret();
        }
        c.run(at(6), 2)?;
        c.call(more::HNM_DO_FRAME_AND_CHECK_IF_FRAME_ADVANCED, at(0xb))?;
        if c.flag(ZF) {
            c.run(at(0xb), 1)?; // no new frame: again
            continue;
        }
        c.run(at(0xb), 2)?;
        c.call(more::GFX_COPY_WHOLE_FRAMEBUF_TO_SCREEN, at(0x10))?;
        let check = if music {
            let ready = c.word(g::MIDI_FUNC5_BX);
            if ready < 8 {
                c.run(0x0635, 2)?;
                c.cmp(W, ready, 8);
            } else {
                let flag = c.byte(g::CURRENT_MUSIC);
                c.run(0x0635, if flag == 0 { 4 } else { 5 })?;
                c.cmp(W, ready, 8);
                c.cmp(super::B, flag as u16, 0);
                if flag != 0 {
                    c.call(more::MIDI_FUNC_2_0, 0x0646)?;
                }
            }
            0x0646
        } else {
            at(0x10)
        };
        c.run(check, 1)?;
        c.call(code::CHECK_IF_HNM_COMPLETE, check + 3)?;
        if c.flag(ZF) {
            c.run(check + 3, 1)?;
            continue;
        }
        c.run(check + 3, 3)?; // je; clc; ret
        c.set_flag(CF, false);
        return c.ret();
    }
}

/// `play_VIRGIN_HNM` (0625).
pub fn play_virgin_hnm(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0625)?;
    play_logo(&mut Ctx::new(m), 0x0625, true)
}

/// `play_CRYO_OR_CRYO2_HNM` (0661).
pub fn play_cryo_hnm(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0661)?;
    play_logo(&mut Ctx::new(m), 0x0661, false)
}

/// C2F2: opens resource AL and draws its first sprite at (0, 0), keeping
/// BX = DX = 0. The jumps that led here are counted by the caller.
fn draw_first_sprite(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xc2f2, 2)?;
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    c.set_reg8(r8::AH, 0);
    c.call(more::OPEN_RESOURCE_BY_INDEX, 0xc2f7)?;
    c.run(0xc2f7, 6)?;
    for r in [AX, BX, DX] {
        let v = c.reg(r);
        let zero = c.xor(W, v, v);
        c.set_reg(r, zero);
    }
    c.push(0);
    c.push(0);
    c.call(code::DRAW_SPRITE_CLOBBERING_BX_DX, 0xc302)?;
    c.run(0xc302, 3)?;
    c.pop();
    c.set_reg(DX, 0);
    c.pop();
    c.set_reg(BX, 0);
    c.ret()
}

/// The intro's still pictures: `mov al, resource; jmp 0739h` (0739:
/// `jmp C2F2h`), some after clearing the active framebuffer.
fn still(m: &mut Machine, start: u16, entry: u16, resource: u8, clear: bool, jumps: u32) -> Result<(), Bail> {
    entered(start, entry)?;
    let mut c = Ctx::new(m);
    let mut at = entry;
    if clear {
        c.run(at, 1)?;
        c.call(more::GFX_CLEAR_ACTIVE_FRAMEBUFFER, at + 3)?;
        at += 3;
    }
    c.run(at, 1 + jumps)?;
    c.set_reg8(r8::AL, resource);
    draw_first_sprite(&mut c)
}

/// `sub_0737`: picture 56h.
pub fn sub_0737(m: &mut Machine, start: u16) -> Result<(), Bail> {
    still(m, start, 0x0737, 0x56, false, 1)
}
/// `sub_073c`: picture 57h.
pub fn sub_073c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    still(m, start, 0x073c, 0x57, false, 2)
}
/// `sub_0740`: picture 58h on a cleared screen.
pub fn sub_0740(m: &mut Machine, start: u16) -> Result<(), Bail> {
    still(m, start, 0x0740, 0x58, true, 2)
}
/// `sub_0747`: picture 59h.
pub fn sub_0747(m: &mut Machine, start: u16) -> Result<(), Bail> {
    still(m, start, 0x0747, 0x59, false, 2)
}
/// `sub_0752`: picture 5Bh.
pub fn sub_0752(m: &mut Machine, start: u16) -> Result<(), Bail> {
    still(m, start, 0x0752, 0x5b, false, 2)
}
/// `sub_0756`: picture 5Ch.
pub fn sub_0756(m: &mut Machine, start: u16) -> Result<(), Bail> {
    still(m, start, 0x0756, 0x5c, false, 2)
}
/// `sub_075a`: picture 5Dh.
pub fn sub_075a(m: &mut Machine, start: u16) -> Result<(), Bail> {
    still(m, start, 0x075a, 0x5d, false, 2)
}
/// `sub_075e`: picture 5Eh.
pub fn sub_075e(m: &mut Machine, start: u16) -> Result<(), Bail> {
    still(m, start, 0x075e, 0x5e, false, 2)
}
/// `intro_outside_of_palace` (09A5): picture 62h on a cleared screen.
pub fn intro_outside_of_palace(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x09a5)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(more::GFX_CLEAR_ACTIVE_FRAMEBUFFER, 0x09a8)?;
    c.run(0x09a8, 2)?;
    c.set_reg8(r8::AL, 0x62);
    draw_first_sprite(&mut c)
}

/// 0981: draws the room (DX, with BX), and copies it to the second
/// framebuffer.
fn draw_room(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x0981, 1)?;
    c.call(more::ROOM_SETUP, 0x0984)?;
    c.run(0x0984, 1)?;
    c.call(more::ROOM_DRAW, 0x0987)?;
    c.run(0x0987, 1)?; // jmp C412h
    super::gfx::copy_active_framebuffer_to_framebuffer_2(c.m, more::COPY_ACTIVE_FRAMEBUFFER_TO_FRAMEBUFFER_2)
}

/// `intro_inside_sietch` (077C).
pub fn intro_inside_sietch(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x077c)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(more::GFX_CLEAR_ACTIVE_FRAMEBUFFER, 0x077f)?;
    c.run(0x077f, 3)?; // mov bx; mov dx; jmp 0981h
    c.set_reg(BX, 0x1080);
    c.set_reg(DX, 0x0803);
    draw_room(&mut c)
}

/// `intro_water_ripples_in_cave` (07E0): with PCM sound 4.
pub fn intro_water_ripples_in_cave(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x07e0)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg8(r8::AL, 4);
    c.call(more::PLAY_PCM_AL, 0x07e5)?;
    c.run(0x07e5, 3)?;
    c.set_reg(DX, 0x0804);
    c.set_reg(BX, 0x1080);
    draw_room(&mut c)
}

/// `intro_palace_equipment_room` (0972).
pub fn intro_palace_equipment_room(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0972)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(more::GFX_CLEAR_ACTIVE_FRAMEBUFFER, 0x0975)?;
    c.run(0x0975, 3)?; // mov dx; mov word [12h], 0; mov bx
    c.set_reg(DX, 0x2002);
    c.set_word(g::INTRO_0012, 0);
    c.set_reg(BX, 0x0180);
    draw_room(&mut c)
}

/// `intro_lady_jessica_1` (098A).
pub fn intro_lady_jessica_1(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x098a)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?; // mov word [12h], 2; mov dx; jmp 097Eh; mov bx
    c.set_word(g::INTRO_0012, 2);
    c.set_reg(DX, 0x2004);
    c.set_reg(BX, 0x0180);
    draw_room(&mut c)
}

/// `intro_load_desert_sky_animation` (07FD): the desert sky (palette 8,
/// then driver function 2 with CX = F0h, BX = 180h), then the sky
/// animation's icons (resource 2Eh, list at DS:1500).
pub fn intro_load_desert_sky_animation(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x07fd)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(more::GFX_CLEAR_ACTIVE_FRAMEBUFFER, 0x0800)?;
    c.run(0x0800, 2)?;
    c.set_reg8(r8::BL, 8);
    c.call(more::OPEN_SKYRS_PALETTE, 0x0805)?;
    c.run(0x0805, 2)?;
    c.set_reg(CX, 0xf0);
    c.set_reg(BX, 0x180);
    c.interp(0x080b, 0x080f)?;
    c.run(0x080f, 3)?;
    c.set_byte(g::FRAME_TASKS_ACTIVE, 0);
    c.set_reg(AX, 0x2e);
    c.call(more::OPEN_RESOURCE_BY_INDEX, 0x081a)?;
    c.run(0x081a, 2)?; // mov si, 1500h; jmp C21Bh
    c.set_reg(SI, 0x1500);
    super::sprites::draw_icons_list_at_si(c.m, 0xc21b)
}

/// `intro_27_baron_guards_play` (08B6): the guards (sprites 5 and 6 of
/// resource 30h) slide in from the left, 20h pixels a frame, to x = 0.
pub fn intro_27_baron_guards_play(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x08b6)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(AX, 0x30);
    c.call(more::OPEN_RESOURCE_BY_INDEX, 0x08bc)?;
    c.run(0x08bc, 1)?;
    c.call(more::COPY_ACTIVE_FRAMEBUFFER_TO_FRAMEBUFFER_2, 0x08bf)?;
    c.run(0x08bf, 1)?;
    c.call(more::COPY_GAME_AREA_RECT_TO_CLIP_RECT, 0x08c2)?;
    c.run(0x08c2, 1)?;
    c.set_reg(DX, 0xffa0);
    loop {
        c.run(0x08c5, 2)?;
        let x = c.reg(DX);
        c.push(x);
        c.call(more::COPY_GAME_AREA_TO_SCREEN_FB2_TO_FB1, 0x08c9)?;
        c.run(0x08c9, 4)?;
        let x = c.pop();
        c.set_reg(DX, x);
        c.set_reg(BX, 0x0d);
        c.set_reg(AX, 5);
        c.call(more::DRAW_SPRITE_CLIPPED, 0x08d3)?;
        c.run(0x08d3, 2)?;
        c.set_reg(AX, 6);
        c.call(more::DRAW_SPRITE_CLIPPED, 0x08d9)?;
        c.run(0x08d9, 2)?;
        let x = c.reg(DX);
        c.push(x);
        c.call(more::SUB_C4DD, 0x08dd)?;
        c.check(0x08dd, 5)?;
        let x = c.pop();
        let next = c.add(W, x, 0x20);
        c.set_reg(DX, next);
        if (x as i16 as i32) + 0x20 <= 0 {
            c.count(3); // pop; add; jle
            continue;
        }
        c.count(5);
        c.set_flag(CF, false);
        return c.ret();
    }
}

/// From 097E (the stretch from `at`, `prefix` instructions before):
/// `loc_008f0` with BX = 180h, `loc_037b2`, then the active framebuffer
/// copied to the second.
fn setup_and_copy(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 2)?;
    ops(c);
    c.set_reg(BX, 0x180);
    c.call(0x08f0, 0x0984)?;
    c.run(0x0984, 1)?;
    c.call(0x37b2, 0x0987)?;
    c.run(0x0987, 1)?;
    super::gfx::copy_active_framebuffer_to_framebuffer_2(c.m, 0xc412)
}

/// `loc_0097e`: see [`setup_and_copy`].
pub fn setup_and_copy_097e(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x097e)?;
    setup_and_copy(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_00978`: [12h] cleared, then [`setup_and_copy`].
pub fn setup_and_copy_0978(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0978)?;
    setup_and_copy(&mut Ctx::new(m), start, 1, |c| c.set_word(g::INTRO_0012, 0))
}

/// `loc_00a16`: `loc_00a23` with the active framebuffer kept; CF clear.
pub fn keep_framebuffer_0a23(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0a16)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let fb = c.word(g::FRAMEBUFFER_ACTIVE);
    c.push(fb);
    c.call(0x0a23, 0x0a1d)?;
    c.run(0x0a1d, 3)?;
    let fb = c.pop();
    c.set_word(g::FRAMEBUFFER_ACTIVE, fb);
    c.set_flag(CF, false);
    c.ret()
}

/// `loc_04937`: at frame 11 of the video, the PCM driver's function 5;
/// then `check_if_hnm_complete`.
pub fn sound_at_frame_11(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4937)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let ax = c.word(g::HNM_FRAME);
    c.set_reg(AX, ax);
    if ax & 0xff == 0x0b {
        c.call(0xac30, 0x4941)?;
        c.run(0x4941, 1)?;
    }
    super::video::check_if_hnm_complete(c.m, 0xcc85)
}

/// `loc_0105b`: `loc_0425b` for each word (DI) of the 0-ended list at
/// CS:SI.
pub fn for_each_in_code_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x105b)?;
    let mut c = Ctx::new(m);
    let (mut at, mut jmp) = (start, 0);
    loop {
        let si = c.reg(SI);
        let w = c.code_word(si);
        c.run(at, jmp + if w == 0 { 4 } else { 5 })?;
        c.lods(W, pc::cpu::CS);
        if w == 0 {
            return c.ret();
        }
        c.set_reg(pc::cpu::DI, w);
        c.call(0x425b, 0x1066)?;
        (at, jmp) = (0x1066, 1);
    }
}

/// `play_intro2` (021C): the intro's second part ([289Eh] 8Ch, [28E7h]
/// set). Unless entered with ZF set (skipped): the credits, eight
/// pictures (from 1, `copy_pal_and_transition` BP 2C1h, each with its
/// sound, `loc_0ab4f`), each shown for FA0h ticks (`wait_interruptable`)
/// unless a key ends it (`loc_0de54`, or the wait); then a transition
/// (BP 301h), a pause, the title (`loc_038f1` BL 0Ch, `loc_03950`,
/// [46DFh] meanwhile) and a fade out (BP C0ADh). Then the screen is
/// cleared (far [38D5h] on [0DBD8h]), the sound and frame tasks
/// stopped, the game set up (day counter [115h] up, [0DBE6h] 6) and the
/// player put in the palace (`loc_008f0`, DX 200Ah, BX 180h).
pub fn play_intro2(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{ES, SP, SS};
    entered(start, 0x021c)?;
    let mut c = Ctx::new(m);
    let z = c.flag(ZF);
    c.run(0x021c, 3 + if z { 0 } else { 1 })?;
    c.set_byte(0x289e, 0x8c);
    c.set_byte(0x28e7, 1);
    if !z {
        c.call(0xad50, 0x022b)?;
        c.run(0x022b, 2)?;
        c.set_reg(AX, 0x3e8);
        let mut si = 1u16;
        c.set_reg(SI, 1);
        let skipped = loop {
            c.run(0x0231, 3)?;
            c.push(si);
            c.set_reg(BP, 0x2c1);
            c.call(0xc102, 0x0238)?;
            c.run(0x0238, 1)?;
            c.call(0xade0, 0x023b)?;
            c.run(0x023b, 3)?;
            let a = c.pop();
            c.set_reg(AX, a);
            c.push(a);
            c.call(0xab4f, 0x0240)?;
            c.run(0x0240, 2)?;
            si = c.pop();
            c.set_reg(SI, si);
            c.call(0xde54, 0x0244)?;
            if c.flag(ZF) {
                c.run(0x0244, 1)?;
                break true;
            }
            c.run(0x0244, 5)?;
            c.push(si);
            c.set_reg(AX, 0x320);
            c.set_reg(AX, 0xfa0);
            c.call(0xddb0, 0x0250)?;
            c.run(0x0250, 2)?;
            let f = c.m.cpu.flags();
            c.push(f);
            c.call(0x0911, 0x0254)?;
            c.run(0x0254, 1)?;
            c.call(0xac14, 0x0257)?;
            c.run(0x0257, 1)?;
            c.call(0xaded, 0x025a)?;
            let sp = c.reg(SP);
            let f = c.word_in(SS, sp);
            let zf = f & 0x40 != 0;
            c.run(0x025a, 3 + if zf { 0 } else { 3 })?;
            let f = c.pop();
            c.m.cpu.set_flags(f);
            si = c.pop();
            c.set_reg(SI, si);
            if zf {
                break true;
            }
            si = c.inc(W, si);
            c.set_reg(SI, si);
            c.cmp(W, si, 8);
            if si > 8 {
                break false;
            }
        };
        if !skipped {
            c.run(0x0264, 3)?;
            c.set_reg(BP, 0x301);
            c.set_reg8(r8::AL, 0x10);
            c.call(0xc108, 0x026c)?;
            c.run(0x026c, 2)?;
            c.set_reg(AX, 0xc8);
            c.call(0xddb0, 0x0272)?;
            c.run(0x0272, 2)?;
            c.set_reg8(r8::BL, 0x0c);
            c.call(0x38f1, 0x0277)?;
            c.run(0x0277, 3)?;
            c.set_byte(0x46df, 1);
            c.set_reg(AX, 0x4b0);
            c.call(0xddb0, 0x0282)?;
            c.run(0x0282, 1)?;
            c.call(0x3950, 0x0285)?;
            c.run(0x0285, 4)?;
            c.set_byte(0x46df, 0);
            c.set_reg(BP, 0xc0ad);
            c.set_reg8(r8::AL, 0x10);
            c.call(0xc108, 0x0292)?;
        }
    }
    c.run(0x0292, 1)?;
    let es = c.word(0xdbd8);
    c.set_sreg(ES, es);
    c.interp(0x0296, 0x029a)?;
    c.run(0x029a, 1)?;
    c.call(0xac14, 0x029d)?;
    c.run(0x029d, 3)?;
    c.set_byte(0x227d, 0);
    c.set_byte(0xc5, 0);
    c.call(0x0911, 0x02aa)?;
    c.run(0x02aa, 6)?;
    c.set_byte(0x28e7, 0);
    c.set_byte(0xdbe6, 6);
    let v = c.byte(0x115);
    let v = c.inc(B, v as u16);
    c.set_byte(0x115, v as u8);
    c.set_reg(DX, 0x200a);
    c.set_reg(BX, 0x180);
    c.jump(0x08f0)
}
