//! Video scenes: routines that play an HNM video (or wait on one) from
//! start to end: the worm-suit and night videos, the SAL scenes, the
//! video with a callback in BP (`loc_0c8fb`), and the steps around them.
//! They run for seconds: they are checked with `REC_BUDGET` raised (see
//! `docs/decompile.md`, section 3).

use super::waits;
use super::screen;
use super::resources;
use super::math;
use super::frame;
use super::audio;
use super::records;
use super::intro;
use super::video;
use super::gfx;
use super::entry;
use super::globals as g;
use super::pure;
use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CX, DS, DX, ES, SI, SS, ZF},
    rec::Bail,
};

/// `loc_00e66`: `loc_0ad5e`, then [`play_video_lowering_head`] with AX 0Ch.
pub fn play_comm_device_video(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0e66)?;
    let mut c = Ctx::new(m);
    c.run(0x0e66, 1)?;
    c.call(entry::PICK_MUSIC, 0x0e69)?;
    c.run(0x0e69, 1)?;
    c.set_reg(AX, 0x0c);
    video_at_0e6c(&mut c)
}

/// `loc_00e6c`: `lip_sync_stop` (AX kept), BP 181Eh, then on into
/// `loc_0c8fb`.
pub fn play_video_lowering_head(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0e6c)?;
    video_at_0e6c(&mut Ctx::new(m))
}

fn video_at_0e6c(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x0e6c, 2)?;
    let ax = c.reg(AX);
    c.push(ax);
    c.call(entry::LIP_SYNC_STOP, 0x0e70)?;
    c.run(0x0e70, 3)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.set_reg(BP, 0x181e);
    c.jump(entry::PLAY_VIDEO_WITH_CALLBACK)
}

/// `loc_04abe`: `loc_037f4`, `loc_0c4dd`, then [11CAh] 0.
pub fn redraw_flight_room(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4abe)?;
    let mut c = Ctx::new(m);
    c.run(0x4abe, 1)?;
    c.call(entry::DRAW_FLIGHT_ROOM, 0x4ac1)?;
    c.run(0x4ac1, 1)?;
    c.call(entry::UPDATE_GAME_AREA_ON_SCREEN, 0x4ac4)?;
    c.run(0x4ac4, 2)?;
    c.update_travel(|t| t.held = 0);
    c.ret()
}

/// `loc_0491c`: `loc_0c08e`, `loc_0c0f4`, sound 8 (`loc_0ab15`),
/// `loc_0ae04`; then `loc_0c9e8` until it gives ZF, with `loc_04937`
/// after each other (ending too when that gives NZ); on into
/// `loc_0c07c`.
#[allow(dead_code)]
pub fn play_night_video(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x491c)?;
    let mut c = Ctx::new(m);
    c.run(0x491c, 1)?;
    c.call(entry::SET_SCREEN_AS_ACTIVE_FRAMEBUFFER, 0x491f)?;
    c.run(0x491f, 1)?;
    c.call(entry::UPDATE_SCREEN_PALETTE, 0x4922)?;
    c.run(0x4922, 2)?;
    audio::start_voc(&mut c, 8, 0x4927)?;
    c.run(0x4927, 1)?;
    c.call(entry::MUSIC_STEP_IF_ENABLED, 0x492a)?;
    loop {
        c.run(0x492a, 1)?;
        c.call(entry::HNM_DO_FRAME_SKIPPABLE, 0x492d)?;
        c.run(0x492d, 2)?;
        if c.flag(ZF) {
            return c.jump(entry::SET_FB1_AS_ACTIVE_FRAMEBUFFER);
        }
        if intro::video_done_after_sound(&mut c, 0x4932)? {
            c.run(0x4932, 2)?;
            return c.jump(entry::SET_FB1_AS_ACTIVE_FRAMEBUFFER);
        }
        c.run(0x4932, 1)?;
    }
}

/// `loc_037f4`: [4728h] 0, `loc_04988`, `loc_04a5a`, the first frame of
/// HNM [`g::FLIGHT_VIDEO`] (`hnm_load_first_frame`), the far routine at [3959h], then
/// on into `set_sky_palette`.
pub fn draw_flight_room(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x37f4)?;
    let mut c = Ctx::new(m);
    c.run(0x37f4, 2)?;
    c.update_flight(|t| t.marker = 0);
    c.call(entry::OPEN_ZOOMED_MAP, 0x37fc)?;
    c.run(0x37fc, 1)?;
    c.call(entry::DRAW_MAP_POINTS, 0x37ff)?;
    c.run(0x37ff, 2)?;
    let ax = c.word(g::FLIGHT_VIDEO);
    c.set_reg(AX, ax);
    c.call(entry::HNM_LOAD_FIRST_FRAME, 0x3805)?;
    c.interp(0x3805, 0x3809)?;
    c.run(0x3809, 1)?;
    c.jump(entry::SET_SKY_PALETTE)
}

/// `loc_01566`: with [227Dh] up by one meanwhile, `loc_0e353` with BP
/// 1556h and AX 10h until [46D7h], counted down each time, reaches 0.
#[allow(dead_code)]
pub fn cinematic_fade_loop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1566)?;
    let mut c = Ctx::new(m);
    c.run(0x1566, 1)?;
    let v = c.byte(g::CINEMATIC);
    let v = pure::inc(B, v as u16);
    c.set_byte(g::CINEMATIC, v as u8);
    loop {
        c.run(0x156a, 3)?;
        frame::draw_then_wait(&mut c, 0x1556, 0x10, 0x1573)?;
        let n = c.byte(g::FRAME_TASKS_ACTIVE);
        let last = n == 1;
        c.run(0x1573, if last { 4 } else { 2 })?;
        let n = pure::dec(B, n as u16);
        c.set_byte(g::FRAME_TASKS_ACTIVE, n as u8);
        if last {
            let v = c.byte(g::CINEMATIC);
            let v = pure::dec(B, v as u16);
            c.set_byte(g::CINEMATIC, v as u8);
            return c.ret();
        }
    }
}

/// `loc_00e77`: the worm-suit music, video 0Bh opened on the back
/// buffer (`loc_0c4dd`, the palette), then five times: a frame advanced,
/// then transition 3Ch to 1797h; the video closed, the frame copied, on
/// into `ui_head_animate_down`.
pub fn play_wormsuit_video(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0e77)?;
    let mut c = Ctx::new(m);
    c.run(0x0e77, 1)?;
    c.call(entry::PLAY_MUSIC_WORMSUIT, 0x0e7a)?;
    c.run(0x0e7a, 1)?;
    c.call(entry::SET_FB1_AS_ACTIVE_FRAMEBUFFER, 0x0e7d)?;
    c.run(0x0e7d, 2)?;
    c.set_reg(AX, 0x0b);
    c.call(entry::HNM_LOAD_FIRST_FRAME, 0x0e83)?;
    c.run(0x0e83, 1)?;
    c.call(entry::UPDATE_GAME_AREA_ON_SCREEN, 0x0e86)?;
    c.run(0x0e86, 1)?;
    c.call(entry::UPDATE_SCREEN_PALETTE, 0x0e89)?;
    c.run(0x0e89, 1)?;
    c.set_reg(CX, 5);
    loop {
        c.run(0x0e8c, 2)?;
        let cx = c.reg(CX);
        c.push(cx);
        while !gfx::frame_advanced(&mut c, 0x0e90)? {
            c.run(0x0e90, 2)?;
        }
        c.run(0x0e90, 4)?;
        screen::transition_drawn(&mut c, 0x1797, 0x3c, 0x0e9a)?;
        c.run(0x0e9a, 2)?;
        let cx = c.pop().wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.run(0x0e9d, 1)?;
    c.call(entry::HNM_CLOSE_RESOURCE, 0x0ea0)?;
    c.run(0x0ea0, 1)?;
    c.call(entry::COPY_ACTIVE_FRAMEBUFFER_TO_FRAMEBUFFER_2, 0x0ea3)?;
    c.run(0x0ea3, 1)?;
    c.jump(0x1803)
}

/// `loc_0c8fb`: video AX opened on the back buffer (`loc_0c4dd`, the
/// palette), the callback BP called, then played to its end: each frame
/// that advanced shown (`loc_0c4dd`, `loc_0ace6`); the frame copied, on
/// into `hnm_close_resource`.
pub fn play_video_with_callback(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc8fb)?;
    let mut c = Ctx::new(m);
    c.run(0xc8fb, 1)?;
    c.call(entry::SET_FB1_AS_ACTIVE_FRAMEBUFFER, 0xc8fe)?;
    c.run(0xc8fe, 2)?;
    let bp = c.reg(BP);
    c.push(bp);
    c.call(entry::HNM_LOAD_FIRST_FRAME, 0xc902)?;
    c.run(0xc902, 1)?;
    c.call(entry::UPDATE_GAME_AREA_ON_SCREEN, 0xc905)?;
    c.run(0xc905, 1)?;
    c.call(entry::UPDATE_SCREEN_PALETTE, 0xc908)?;
    c.run(0xc908, 2)?;
    let bp = c.pop();
    c.set_reg(BP, bp);
    c.call(bp, 0xc90b)?;
    c.run(0xc90b, 1)?;
    loop {
        if !gfx::frame_advanced(&mut c, 0xc90e)? {
            c.run(0xc90e, 2)?;
            continue;
        }
        c.run(0xc90e, 2)?;
        c.call(entry::UPDATE_GAME_AREA_ON_SCREEN, 0xc913)?;
        c.run(0xc913, 1)?;
        c.call(entry::MUSIC_STEP, 0xc916)?;
        c.run(0xc916, 1)?;
        if video::video_done(&mut c, 0xc919)? {
            break;
        }
        c.run(0xc919, 2)?;
    }
    c.run(0xc919, 2)?;
    c.call(entry::COPY_ACTIVE_FRAMEBUFFER_TO_FRAMEBUFFER_2, 0xc91e)?;
    c.run(0xc91e, 1)?;
    c.jump(entry::HNM_CLOSE_RESOURCE)
}

/// `loc_0c8c1`: the video's frame BP drawn at DX, BX less its offset
/// (2796h/2798h by BP, not below 0) by the far routine at SS:[3949h] (ES
/// [0DBD8h], DS [0DBD6h]); then a wait until [0CE7Ah] has moved
/// [0DBE6h] ticks on from its value at the start. BX and DX kept.
pub fn draw_video_sprite_and_wait(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc8c1)?;
    let mut c = Ctx::new(m);
    let (bx, dx, bp) = (c.reg(BX), c.reg(DX), c.reg(BP));
    let at = bp.wrapping_shl(2);
    let ox = c.word(at.wrapping_add(0x2796));
    let oy = c.word(at.wrapping_add(0x2798));
    c.run(0xc8c1, 8 + (dx < ox) as u32 + 2 + (bx < oy) as u32 + 3)?;
    c.push(bx);
    c.push(dx);
    let t0 = c.timer();
    c.push(t0);
    c.set_reg(SI, at);
    c.set_reg(DX, dx.saturating_sub(ox));
    let mut y = pure::sub(W, bx, oy);
    if bx < oy {
        y = pure::xor(W, y, y);
    }
    c.set_reg(BX, y);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.screen_buffers().screen;
    let ds2 = c.screen_buffers().front;
    c.set_sreg(ES, es);
    c.set_sreg(DS, ds2);
    c.interp(0xc8e6, 0xc8eb)?;
    c.run(0xc8eb, 2)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let t0 = c.pop();
    c.set_reg(BX, t0);
    loop {
        c.run(0xc8ed, 4)?;
        let t = c.timer();
        // Until [0DBE6h] ticks have passed.
        let d = t.wrapping_sub(t0);
        c.set_reg(AX, d);
        if d as u8 >= c.video_state().delay {
            break;
        }
    }
    c.run(0xc8f8, 3)?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.ret()
}

/// `loc_0488a`: the SAL scene of the location [114Eh] (`calc_SAL_index`
/// with AX 6). Below 8: the cursor restored, [4728h] 80h, frames played
/// (`hnm_do_frame`, `loc_04ec6` with AX 0) until video 2 is at frame 3Ch
/// (scene 6) or 16h, `loc_0ce4b`, then played to its end. 9: video
/// `loc_0c8fb` with BP C4CDh. Else the still scene: [4731h] FFh while the
/// room is drawn on the back buffer and copied, voice 7, `loc_047fb` with
/// CL FFh, [4731h] 0. Then (not for the still scene) [46E0h] down by one;
/// [4732h] 0, on into `open_SAL_resource`.
pub fn play_sal_scene(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x488a)?;
    let mut c = Ctx::new(m);
    c.run(0x488a, 3)?;
    c.set_reg(AX, 6);
    let loc = c.word(g::PLACE);
    c.set_reg(SI, loc);
    let base = c.reg(AX);
    let (sal, _) = records::sal_index(&mut c, base, 0x4894)?;
    let al = sal & 0xff;
    if al < 8 {
        c.run(0x4894, 4)?;
        let bx = c.reg(AX);
        c.set_reg(BX, bx);
        c.call(entry::CALL_RESTORE_CURSOR, 0x489d)?;
        c.run(0x489d, 3)?;
        c.update_flight(|t| t.marker = 0x80);
        loop {
            let bx = c.reg(BX);
            c.push(bx);
            c.call(entry::HNM_DO_FRAME, 0x48a6)?;
            c.run(0x48a6, 2)?;
            c.set_reg(AX, 0);
            c.call(entry::NEXT_FLIGHT_VIDEO, 0x48ab)?;
            let bx = c.word_in(SS, c.reg(pc::cpu::SP));
            let video = c.video_state().video;
            if video != 2 {
                c.run(0x48ab, 5)?;
                c.pop();
                c.set_reg(BX, bx);
                continue;
            }
            let want = if bx == 6 { 0x3c } else { 0x16 };
            let frame = c.video_state().fetched;
            let n = 3 + 3 + (bx != 6) as u32 + 2;
            c.run(0x48ab, if want != frame { n + 2 } else { n + 1 })?;
            c.pop();
            c.set_reg(BX, bx);
            c.set_reg(AX, 0x3c);
            c.set_reg(AX, want);
            if want == frame {
                break;
            }
        }
        c.call(entry::HNM_NOTE_VIDEO_AND_FRAME, 0x48c9)?;
        loop {
            c.run(0x48c9, 1)?;
            c.call(entry::HNM_DO_FRAME, 0x48cc)?;
            c.run(0x48cc, 1)?;
            if video::video_done(&mut c, 0x48cf)? {
                break;
            }
            c.run(0x48cf, 1)?;
        }
        c.run(0x48cf, 4)?;
        return sal_end(&mut c);
    }
    c.run(0x4894, 4)?;
    if al == 9 {
        c.run(0x48dd, 2)?;
        c.set_reg(BP, 0xc4cd);
        c.call(entry::PLAY_VIDEO_WITH_CALLBACK, 0x48e3)?;
        c.run(0x48e3, 4)?;
        return sal_end(&mut c);
    }
    c.run(0x48e9, 2)?;
    c.set_byte(g::ORNI_SPEED, 0xff);
    c.call(entry::SET_FB1_AS_ACTIVE_FRAMEBUFFER, 0x48f1)?;
    c.run(0x48f1, 1)?;
    c.call(entry::COPY_GAME_AREA_RECT_TO_UNKNOWN_RECT, 0x48f4)?;
    c.run(0x48f4, 1)?;
    c.call(entry::DRAW_ROOM, 0x48f7)?;
    c.run(0x48f7, 1)?;
    c.call(entry::UPDATE_SCREEN_PALETTE, 0x48fa)?;
    c.run(0x48fa, 1)?;
    c.call(entry::COPY_ACTIVE_FRAMEBUFFER_TO_FRAMEBUFFER_2, 0x48fd)?;
    c.run(0x48fd, 3)?;
    c.set_byte(g::ORNI_SPEED, 0x1f);
    audio::start_voc(&mut c, 7, 0x4907)?;
    c.run(0x4907, 2)?;
    c.set_reg8(r8::CL, 0xff);
    c.call(entry::STEP_ORNI_ROTOR, 0x490c)?;
    c.run(0x490c, 4)?;
    c.set_byte(g::ORNI_SPEED, 0);
    c.set_byte(g::SAL_FLAGS, 0);
    c.jump(entry::OPEN_SAL_RESOURCE)
}

/// 48D1h-48DAh of `loc_0488a` (counted by the caller): [46E0h] down by
/// one, [4732h] 0, on into `open_SAL_resource`.
fn sal_end(c: &mut Ctx) -> Result<(), Bail> {
    let v = c.byte(g::SKY_PALETTE_SHOWN);
    let v = c.dec(B, v as u16);
    c.set_byte(g::SKY_PALETTE_SHOWN, v as u8);
    c.set_byte(g::SAL_FLAGS, 0);
    c.jump(entry::OPEN_SAL_RESOURCE)
}

/// `loc_02566`: the spice shipped to the Emperor (AX, the amount
/// chosen, [`g::SHIPMENT_CHOSEN`]; called at 35E0h). The location 1008h's
/// state (`loc_040e6`, AX kept), `loc_02524` taking it from
/// [`g::SPICE`] into [`g::SPICE_PAID`]; [`g::SHIPMENT_RATIO`] the
/// amount's share of [`g::SPICE_DEMANDED`] (AX x 100h / it, below 200h,
/// halved, at least 1); the days BX (7 from C0h, 6 above 80h, 5 at 80h,
/// 4 below; at 4, the ratio 0 when bit 8 of [`g::SHIPMENT_FLAGS`])
/// added to [`g::SHIPMENT_DUE`] with `loc_0e3b7`'s value for bits 1-2 of
/// [`g::DEMANDS`]; [`g::SHIPMENT_DAYS_LEFT`] the difference from the day
/// (`loc_01ac5`), [`g::SHIPMENT_READY`] cleared; then the shipment's
/// scene: transitions, waits, the fading frames (`loc_0e353` with BP
/// A44h and 26ACh), voices and music, on into `loc_02773`.
pub fn ship_spice_to_emperor(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{DX, ES};
    entered(start, 0x2566)?;
    let mut c = Ctx::new(m);
    c.run(0x2566, 3)?;
    c.set_reg(SI, 0x1008);
    let ax = c.reg(AX);
    c.push(ax);
    c.call(entry::UPDATE_FLAGGED_RECORD, 0x256d)?;
    c.run(0x256d, 3)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.set_reg(BX, ax);
    c.call(entry::PAY_SPICE, 0x2573)?;
    // The amount x 100h over the demand: a divide error (by 0, or a quotient
    // over FFFFh) is left to the interpreter.
    let bx = c.reg(BX);
    let num = (bx as u32) << 8;
    let den = c.economy().demanded as u32;
    if den == 0 || num / den > 0xffff {
        c.m.cpu.ip = 0x2573;
        return Err(Bail);
    }
    let (q, r) = ((num / den) as u16, (num % den) as u16);
    let big = q >= 0x200;
    c.run(0x2573, 7 + big as u32)?;
    c.set_reg(AX, bx);
    c.set_reg(DX, bx >> 8);
    c.set_reg(AX, bx << 8);
    // `div word ptr [0BCh]` (flags overwritten by the `cmp`).
    c.set_reg(AX, q);
    c.set_reg(DX, r);
    let mut ax = if big { 0x1ff } else { q };
    let half = ax >> 1;
    c.run(0x2587, 2 + (half == 0) as u32)?;
    ax = pure::shr1(W, ax, 1);
    if ax == 0 {
        ax = (ax & 0xff00) | 1;
    }
    c.set_reg(AX, ax);
    let al = ax & 0xff;
    let flags_bf = c.economy().shipment_flags;
    let (rating, ah, n) = if al >= 0xc0 {
        (7u16, 0x40u16, 5)
    } else if al > 0x80 {
        (6, 0x40, 8)
    } else if al == 0x80 {
        (5, 0, 12)
    } else {
        (4, 8, 16 + (flags_bf & 8 != 0) as u32)
    };
    c.run(0x258d, n)?;
    c.update_economy(|t| t.shipment_ratio = al as u8);
    c.set_reg(AX, (0x40 << 8) | al);
    c.set_reg(BX, 7);
    if rating < 5 {
        if flags_bf & 8 != 0 {
            c.update_economy(|t| t.shipment_ratio = 0);
        }
    }
    c.set_reg(BX, rating);
    c.set_reg(AX, (ah << 8) | al);
    c.run(0x25b3, 8)?;
    let ah = pure::or(B, ah, 0x80);
    c.set_reg(AX, (ah << 8) | al);
    c.update_economy(|t| t.shipment_flags = ah as u8);
    c.set_reg(SI, g::SHIPMENT_DUE);
    let v = c.word(g::SHIPMENT_DUE);
    let v = pure::add(W, v, rating);
    c.set_word(g::SHIPMENT_DUE, v);
    let bl = c.economy().demands;
    c.set_reg(BX, (rating & 0xff00) | bl as u16);
    let bl = pure::shr1(B, bl as u16, 1);
    let bx = pure::and(W, bl, 3);
    math::random_masked(&mut c, bx, 0x25cb)?;
    c.run(0x25cb, 2)?;
    let ax = c.reg(AX);
    let v = c.word(g::SHIPMENT_DUE);
    let v = pure::add(W, v, ax);
    c.set_word(g::SHIPMENT_DUE, v);
    c.call(entry::GET_INGAME_DAY_Q, 0x25d0)?;
    c.run(0x25d0, 5)?;
    let ax = c.reg(AX);
    let v = c.word(g::SHIPMENT_DUE);
    let d = pure::sub(W, ax, v);
    let d = pure::neg(W, d);
    c.set_reg(AX, d);
    c.update_situation(|t| t.shipment_days_left = d as u8);
    c.set_word(g::SHIPMENT_READY, 0);
    c.call(entry::COMM_RING_INCOMING_CALL, 0x25e0)?;
    c.run(0x25e0, 1)?;
    c.call(entry::COMM_FADE_IN_GLOW, 0x25e3)?;
    c.run(0x25e3, 1)?;
    c.call(entry::GFX_COPY_FRAMEBUFFER_TO_SCREEN, 0x25e6)?;
    c.run(0x25e6, 2)?;
    resources::open_resource(&mut c, 0x2c, 0x25ec)?;
    c.run(0x25ec, 1)?;
    c.call(entry::UPDATE_SCREEN_PALETTE, 0x25ef)?;
    c.run(0x25ef, 3)?;
    screen::transition_drawn(&mut c, 0x2555, 8, 0x25f7)?;
    c.run(0x25f7, 2)?;
    waits::wait_or_input(&mut c, 0x64, 0x25fd)?;
    c.run(0x25fd, 4)?;
    c.set_reg(CX, 0x18);
    screen::transition_drawn(&mut c, 0x0a44, 6, 0x2608)?;
    c.run(0x2608, 1)?;
    c.set_reg(CX, 0x18);
    let mut cx = 0x18u16;
    loop {
        c.run(0x260b, 5)?;
        c.push(cx);
        c.set_reg(BP, 0x0a44);
        c.set_reg(AX, 0x0c);
        let d = pure::dec(W, cx);
        c.set_reg(CX, d);
        c.call(entry::WAIT_PROCESSING_FRAME_TASKS_INTERRUPTABLE, 0x2616)?;
        c.run(0x2616, 2)?;
        cx = c.pop().wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.run(0x2619, 2)?;
    c.set_reg(AX, 0x27);
    c.call(entry::PLAY_VOICE_19, 0x261f)?;
    c.run(0x261f, 3)?;
    c.set_reg(SI, 0x4c60);
    c.set_reg(BP, 0x15aa);
    let es = c.screen_buffers().front;
    c.set_sreg(ES, es);
    c.interp(0x2629, 0x262d)?;
    c.run(0x262d, 2)?;
    waits::wait_or_input(&mut c, 0xc8, 0x2633)?;
    c.run(0x2633, 1)?;
    c.call(entry::WAIT_VOICE_DONE, 0x2636)?;
    c.run(0x2636, 2)?;
    audio::start_voc(&mut c, 1, 0x263b)?;
    c.run(0x263b, 1)?;
    c.set_reg(CX, 1);
    let mut cx = 1u16;
    loop {
        c.run(0x263e, 2)?;
        c.push(cx);
        c.set_reg(SI, 0x15b2);
        let mut si = 0x15b2u16;
        loop {
            c.run(0x2642, 4)?;
            c.push(si);
            frame::draw_then_wait(&mut c, 0x26ac, 0x0c, 0x264c)?;
            let last = si == 0x161e;
            c.run(0x264c, 3 + last as u32)?;
            si = c.pop();
            c.set_reg(SI, si);
            if last {
                c.call(entry::CALL_PCM_AUDIO_VTABLE_FUNC_5, 0x2656)?;
            }
            let next = si.wrapping_add(6);
            let b = c.byte(next);
            c.run(0x2656, 3)?;
            si = pure::add(W, si, 6);
            c.set_reg(SI, si);
            if b == 0xff {
                break;
            }
        }
        c.run(0x265e, 2)?;
        cx = c.pop().wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.run(0x2661, 1)?;
    c.call(entry::DRAW_SCENE_PICTURE, 0x2664)?;
    c.run(0x2664, 1)?;
    let mut cx = 0u16;
    loop {
        c.run(0x2666, 5)?;
        cx = pure::inc(W, cx);
        c.push(cx);
        c.set_reg(CX, cx);
        frame::draw_then_wait(&mut c, 0x0a44, 0x0c, 0x2671)?;
        c.run(0x2671, 3)?;
        cx = c.pop();
        c.set_reg(CX, cx);
        if cx >= 0x19 {
            break;
        }
    }
    c.run(0x2677, 3)?;
    c.update_conversation(|t| t.caller = 1);
    gfx::draw_on_front_buffer(&mut c, 0x2db1, 0x2682)?;
    c.run(0x2682, 2)?;
    resources::open_resource(&mut c, 0x2c, 0x2688)?;
    c.run(0x2688, 3)?;
    screen::transition_drawn(&mut c, 0x2555, 6, 0x2690)?;
    c.run(0x2690, 2)?;
    gfx::draw_on_front_buffer(&mut c, 0x2db1, 0x2696)?;
    c.run(0x2696, 4)?;
    c.update_conversation(|t| t.caller = 0);
    screen::transition_drawn(&mut c, 0x26a6, 8, 0x26a3)?;
    c.run(0x26a3, 1)?;
    c.jump(entry::COMM_FADE_OUT_GLOW)
}
