//! Video scenes: routines that play an HNM video (or wait on one) from
//! start to end: the worm-suit and night videos, the SAL scenes, the
//! video with a callback in BP (`loc_0c8fb`), and the steps around them.
//! They run for seconds: they are checked with `REC_BUDGET` raised (see
//! `docs/decompile.md`, section 3).

use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DS, DX, ES, SI, SS, ZF},
    rec::Bail,
};

/// `loc_00e66`: `loc_0ad5e`, then [`video_0e6c`] with AX 0Ch.
pub fn video_0e66(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0e66)?;
    let mut c = Ctx::new(m);
    c.run(0x0e66, 1)?;
    c.call(0xad5e, 0x0e69)?;
    c.run(0x0e69, 1)?;
    c.set_reg(AX, 0x0c);
    video_at_0e6c(&mut c)
}

/// `loc_00e6c`: `lip_sync_stop` (AX kept), BP 181Eh, then on into
/// `loc_0c8fb`.
pub fn video_0e6c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0e6c)?;
    video_at_0e6c(&mut Ctx::new(m))
}

fn video_at_0e6c(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x0e6c, 2)?;
    let ax = c.reg(AX);
    c.push(ax);
    c.call(0xa7a5, 0x0e70)?;
    c.run(0x0e70, 3)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.set_reg(BP, 0x181e);
    c.jump(0xc8fb)
}

/// `loc_04abe`: `loc_037f4`, `loc_0c4dd`, then [11CAh] 0.
pub fn tail_4abe(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4abe)?;
    let mut c = Ctx::new(m);
    c.run(0x4abe, 1)?;
    c.call(0x37f4, 0x4ac1)?;
    c.run(0x4ac1, 1)?;
    c.call(0xc4dd, 0x4ac4)?;
    c.run(0x4ac4, 2)?;
    c.set_byte(0x11ca, 0);
    c.ret()
}

/// `loc_0491c`: `loc_0c08e`, `loc_0c0f4`, sound 8 (`loc_0ab15`),
/// `loc_0ae04`; then `loc_0c9e8` until it gives ZF, with `loc_04937`
/// after each other (ending too when that gives NZ); on into
/// `loc_0c07c`.
#[allow(dead_code)]
pub fn video_491c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x491c)?;
    let mut c = Ctx::new(m);
    c.run(0x491c, 1)?;
    c.call(0xc08e, 0x491f)?;
    c.run(0x491f, 1)?;
    c.call(0xc0f4, 0x4922)?;
    c.run(0x4922, 2)?;
    c.set_reg8(r8::AL, 8);
    c.call(0xab15, 0x4927)?;
    c.run(0x4927, 1)?;
    c.call(0xae04, 0x492a)?;
    loop {
        c.run(0x492a, 1)?;
        c.call(0xc9e8, 0x492d)?;
        c.run(0x492d, 2)?;
        if c.flag(ZF) {
            return c.jump(0xc07c);
        }
        c.call(0x4937, 0x4932)?;
        if !c.flag(ZF) {
            c.run(0x4932, 2)?;
            return c.jump(0xc07c);
        }
        c.run(0x4932, 1)?;
    }
}

/// `loc_037f4`: [4728h] 0, `loc_04988`, `loc_04a5a`, the first frame of
/// HNM [487Eh] (`hnm_load_first_frame`), the far routine at [3959h], then
/// on into `set_sky_palette`.
pub fn room_37f4(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x37f4)?;
    let mut c = Ctx::new(m);
    c.run(0x37f4, 2)?;
    c.set_byte(0x4728, 0);
    c.call(0x4988, 0x37fc)?;
    c.run(0x37fc, 1)?;
    c.call(0x4a5a, 0x37ff)?;
    c.run(0x37ff, 2)?;
    let ax = c.word(0x487e);
    c.set_reg(AX, ax);
    c.call(0xca1b, 0x3805)?;
    c.interp(0x3805, 0x3809)?;
    c.run(0x3809, 1)?;
    c.jump(0x388d)
}

/// `loc_01566`: with [227Dh] up by one meanwhile, `loc_0e353` with BP
/// 1556h and AX 10h until [46D7h], counted down each time, reaches 0.
#[allow(dead_code)]
pub fn repeat_1566(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x1566)?;
    let mut c = Ctx::new(m);
    c.run(0x1566, 1)?;
    let v = c.byte(0x227d);
    let v = c.inc(B, v as u16);
    c.set_byte(0x227d, v as u8);
    loop {
        c.run(0x156a, 3)?;
        c.set_reg(BP, 0x1556);
        c.set_reg(AX, 0x10);
        c.call(0xe353, 0x1573)?;
        let n = c.byte(0x46d7);
        let last = n == 1;
        c.run(0x1573, if last { 4 } else { 2 })?;
        let n = c.dec(B, n as u16);
        c.set_byte(0x46d7, n as u8);
        if last {
            let v = c.byte(0x227d);
            let v = c.dec(B, v as u16);
            c.set_byte(0x227d, v as u8);
            return c.ret();
        }
    }
}

/// `loc_00e77`: the worm-suit music, video 0Bh opened on the back
/// buffer (`loc_0c4dd`, the palette), then five times: a frame advanced,
/// then transition 3Ch to 1797h; the video closed, the frame copied, on
/// into `ui_head_animate_down`.
pub fn video_0e77(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0e77)?;
    let mut c = Ctx::new(m);
    c.run(0x0e77, 1)?;
    c.call(0xad50, 0x0e7a)?;
    c.run(0x0e7a, 1)?;
    c.call(0xc07c, 0x0e7d)?;
    c.run(0x0e7d, 2)?;
    c.set_reg(AX, 0x0b);
    c.call(0xca1b, 0x0e83)?;
    c.run(0x0e83, 1)?;
    c.call(0xc4dd, 0x0e86)?;
    c.run(0x0e86, 1)?;
    c.call(0xc0f4, 0x0e89)?;
    c.run(0x0e89, 1)?;
    c.set_reg(CX, 5);
    loop {
        c.run(0x0e8c, 2)?;
        let cx = c.reg(CX);
        c.push(cx);
        c.call(0xc9f4, 0x0e90)?;
        while c.flag(ZF) {
            c.run(0x0e90, 2)?;
            c.call(0xc9f4, 0x0e90)?;
        }
        c.run(0x0e90, 4)?;
        c.set_reg8(r8::AL, 0x3c);
        c.set_reg(BP, 0x1797);
        c.call(0xc108, 0x0e9a)?;
        c.run(0x0e9a, 2)?;
        let cx = c.pop().wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.run(0x0e9d, 1)?;
    c.call(0xca01, 0x0ea0)?;
    c.run(0x0ea0, 1)?;
    c.call(0xc412, 0x0ea3)?;
    c.run(0x0ea3, 1)?;
    c.jump(0x1803)
}

/// `loc_0c8fb`: video AX opened on the back buffer (`loc_0c4dd`, the
/// palette), the callback BP called, then played to its end: each frame
/// that advanced shown (`loc_0c4dd`, `loc_0ace6`); the frame copied, on
/// into `hnm_close_resource`.
pub fn video_c8fb(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc8fb)?;
    let mut c = Ctx::new(m);
    c.run(0xc8fb, 1)?;
    c.call(0xc07c, 0xc8fe)?;
    c.run(0xc8fe, 2)?;
    let bp = c.reg(BP);
    c.push(bp);
    c.call(0xca1b, 0xc902)?;
    c.run(0xc902, 1)?;
    c.call(0xc4dd, 0xc905)?;
    c.run(0xc905, 1)?;
    c.call(0xc0f4, 0xc908)?;
    c.run(0xc908, 2)?;
    let bp = c.pop();
    c.set_reg(BP, bp);
    c.call(bp, 0xc90b)?;
    c.run(0xc90b, 1)?;
    loop {
        c.call(0xc9f4, 0xc90e)?;
        if c.flag(ZF) {
            c.run(0xc90e, 2)?;
            continue;
        }
        c.run(0xc90e, 2)?;
        c.call(0xc4dd, 0xc913)?;
        c.run(0xc913, 1)?;
        c.call(0xace6, 0xc916)?;
        c.run(0xc916, 1)?;
        c.call(0xcc85, 0xc919)?;
        if !c.flag(ZF) {
            break;
        }
        c.run(0xc919, 2)?;
    }
    c.run(0xc919, 2)?;
    c.call(0xc412, 0xc91e)?;
    c.run(0xc91e, 1)?;
    c.jump(0xca01)
}

/// `loc_0c8c1`: the video's frame BP drawn at DX, BX less its offset
/// (2796h/2798h by BP, not below 0) by the far routine at SS:[3949h] (ES
/// [0DBD8h], DS [0DBD6h]); then a wait until [0CE7Ah] has moved
/// [0DBE6h] ticks on from its value at the start. BX and DX kept.
pub fn frame_wait_c8c1(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc8c1)?;
    let mut c = Ctx::new(m);
    let (bx, dx, bp) = (c.reg(BX), c.reg(DX), c.reg(BP));
    let at = bp.wrapping_shl(2);
    let ox = c.word(at.wrapping_add(0x2796));
    let oy = c.word(at.wrapping_add(0x2798));
    c.run(0xc8c1, 8 + (dx < ox) as u32 + 2 + (bx < oy) as u32 + 3)?;
    c.push(bx);
    c.push(dx);
    let t0 = c.word(0xce7a);
    c.push(t0);
    let si = c.shl1(W, bp, 2);
    c.set_reg(SI, si);
    let mut x = c.sub(W, dx, ox);
    if dx < ox {
        x = c.xor(W, x, x);
    }
    c.set_reg(DX, x);
    let mut y = c.sub(W, bx, oy);
    if bx < oy {
        y = c.xor(W, y, y);
    }
    c.set_reg(BX, y);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.word(0xdbd8);
    let ds2 = c.word(0xdbd6);
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
        let t = c.word(0xce7a);
        let d = c.sub(W, t, t0);
        c.set_reg(AX, d);
        let lim = c.byte(0xdbe6);
        c.cmp(B, d & 0xff, lim as u16);

        if !c.flag(CF) {
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
pub fn sal_scene_488a(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x488a)?;
    let mut c = Ctx::new(m);
    c.run(0x488a, 3)?;
    c.set_reg(AX, 6);
    let loc = c.word(0x114e);
    c.set_reg(SI, loc);
    c.call(0x5e4f, 0x4894)?;
    let al = c.reg(AX) & 0xff;
    if al < 8 {
        c.run(0x4894, 4)?;
        c.cmp(B, al, 8);
        let bx = c.reg(AX);
        c.set_reg(BX, bx);
        c.call(0xdbb2, 0x489d)?;
        c.run(0x489d, 3)?;
        c.set_byte(0x4728, 0x80);
        loop {
            let bx = c.reg(BX);
            c.push(bx);
            c.call(0xca60, 0x48a6)?;
            c.run(0x48a6, 2)?;
            let ax = c.reg(AX);
            c.xor(W, ax, ax);
            c.set_reg(AX, 0);
            c.call(0x4ec6, 0x48ab)?;
            let bx = c.word_in(SS, c.reg(pc::cpu::SP));
            let video = c.word(0xdc00);
            if video != 2 {
                c.run(0x48ab, 5)?;
                c.pop();
                c.set_reg(BX, bx);
                c.cmp(W, video, 2);
                continue;
            }
            let want = if bx == 6 { 0x3c } else { 0x16 };
            let frame = c.word(0xdbea);
            let n = 3 + 3 + (bx != 6) as u32 + 2;
            c.run(0x48ab, if want != frame { n + 2 } else { n + 1 })?;
            c.pop();
            c.set_reg(BX, bx);
            c.cmp(W, video, 2);
            c.set_reg(AX, 0x3c);
            c.cmp(W, bx, 6);
            c.set_reg(AX, want);
            c.cmp(W, want, frame);
            if want == frame {
                break;
            }
        }
        c.call(0xce4b, 0x48c9)?;
        loop {
            c.run(0x48c9, 1)?;
            c.call(0xca60, 0x48cc)?;
            c.run(0x48cc, 1)?;
            c.call(0xcc85, 0x48cf)?;
            if !c.flag(ZF) {
                break;
            }
            c.run(0x48cf, 1)?;
        }
        c.run(0x48cf, 4)?;
        return sal_end(&mut c);
    }
    c.run(0x4894, 4)?;
    c.cmp(B, al, 8);
    c.cmp(B, al, 9);
    if al == 9 {
        c.run(0x48dd, 2)?;
        c.set_reg(BP, 0xc4cd);
        c.call(0xc8fb, 0x48e3)?;
        c.run(0x48e3, 4)?;
        return sal_end(&mut c);
    }
    c.run(0x48e9, 2)?;
    c.set_byte(0x4731, 0xff);
    c.call(0xc07c, 0x48f1)?;
    c.run(0x48f1, 1)?;
    c.call(0x5ba0, 0x48f4)?;
    c.run(0x48f4, 1)?;
    c.call(0x37b2, 0x48f7)?;
    c.run(0x48f7, 1)?;
    c.call(0xc0f4, 0x48fa)?;
    c.run(0x48fa, 1)?;
    c.call(0xc412, 0x48fd)?;
    c.run(0x48fd, 3)?;
    c.set_byte(0x4731, 0x1f);
    c.set_reg8(r8::AL, 7);
    c.call(0xab15, 0x4907)?;
    c.run(0x4907, 2)?;
    c.set_reg8(r8::CL, 0xff);
    c.call(0x47fb, 0x490c)?;
    c.run(0x490c, 4)?;
    c.set_byte(0x4731, 0);
    c.set_byte(0x4732, 0);
    c.jump(0x2d74)
}

/// 48D1h-48DAh of `loc_0488a` (counted by the caller): [46E0h] down by
/// one, [4732h] 0, on into `open_SAL_resource`.
fn sal_end(c: &mut Ctx) -> Result<(), Bail> {
    let v = c.byte(0x46e0);
    let v = c.dec(B, v as u16);
    c.set_byte(0x46e0, v as u8);
    c.set_byte(0x4732, 0);
    c.jump(0x2d74)
}

/// `loc_02566`: the end of the game. The location 1008h's state
/// (`loc_040e6`, AX kept), `loc_02524` with BX the score AX; [0BEh] the
/// score's share of [0BCh] (AX x 100h / it, below 200h, halved, at least
/// 1); the rating BX (7 from C0h, 6 above 80h, 5 at 80h, 4 below; at 4,
/// [0BEh] 0 when bit 8 of [0BFh]) added to [118Dh] with `loc_0e3b7`'s
/// value for bits 1-2 of [0C3h]; [0CFh] the difference from the day
/// (`loc_01ac5`); then the closing scenes: transitions, waits, the
/// fading frames (`loc_0e353` with BP A44h and 26ACh), voices and music,
/// on into `loc_02773`.
pub fn endgame_2566(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{DX, ES};
    entered(start, 0x2566)?;
    let mut c = Ctx::new(m);
    c.run(0x2566, 3)?;
    c.set_reg(SI, 0x1008);
    let ax = c.reg(AX);
    c.push(ax);
    c.call(0x40e6, 0x256d)?;
    c.run(0x256d, 3)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.set_reg(BX, ax);
    c.call(0x2524, 0x2573)?;
    // The score x 100h over [0BCh]: a divide error (by 0, or a quotient
    // over FFFFh) is left to the interpreter.
    let bx = c.reg(BX);
    let num = (bx as u32) << 8;
    let den = c.word(0xbc) as u32;
    if den == 0 || num / den > 0xffff {
        c.m.cpu.ip = 0x2573;
        return Err(Bail);
    }
    let (q, r) = ((num / den) as u16, (num % den) as u16);
    let big = q >= 0x200;
    c.run(0x2573, 7 + big as u32)?;
    c.set_reg(AX, bx);
    c.xor(W, 0, 0);
    c.set_reg(DX, bx >> 8);
    c.set_reg(AX, bx << 8);
    // `div word ptr [0BCh]` (flags overwritten by the `cmp`).
    c.set_reg(AX, q);
    c.set_reg(DX, r);
    c.cmp(W, q, 0x200);
    let mut ax = if big { 0x1ff } else { q };
    let half = ax >> 1;
    c.run(0x2587, 2 + (half == 0) as u32)?;
    ax = c.shr1(W, ax, 1);
    if ax == 0 {
        ax = (ax & 0xff00) | 1;
    }
    c.set_reg(AX, ax);
    let al = ax & 0xff;
    let flags_bf = c.byte(0xbf);
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
    c.set_byte(0xbe, al as u8);
    c.set_reg(AX, (0x40 << 8) | al);
    c.set_reg(BX, 7);
    c.cmp(B, al, 0xc0);
    if rating < 7 {
        c.dec(W, 7);
        c.cmp(B, al, 0x80);
    }
    if rating < 6 {
        c.dec(W, 6);
        c.xor(B, 0x40, 0x40);
        c.cmp(B, al, 0x80);
    }
    if rating < 5 {
        c.dec(W, 5);
        c.test(B, flags_bf as u16, 8);
        if flags_bf & 8 != 0 {
            c.set_byte(0xbe, 0);
        }
    }
    c.set_reg(BX, rating);
    c.set_reg(AX, (ah << 8) | al);
    c.run(0x25b3, 8)?;
    let ah = c.or(B, ah, 0x80);
    c.set_reg(AX, (ah << 8) | al);
    c.set_byte(0xbf, ah as u8);
    c.set_reg(SI, 0x118d);
    let v = c.word(0x118d);
    let v = c.add(W, v, rating);
    c.set_word(0x118d, v);
    let bl = c.byte(0xc3);
    c.set_reg(BX, (rating & 0xff00) | bl as u16);
    let bl = c.shr1(B, bl as u16, 1);
    let bx = c.and(W, bl, 3);
    c.set_reg(BX, bx);
    c.call(0xe3b7, 0x25cb)?;
    c.run(0x25cb, 2)?;
    let ax = c.reg(AX);
    let v = c.word(0x118d);
    let v = c.add(W, v, ax);
    c.set_word(0x118d, v);
    c.call(0x1ac5, 0x25d0)?;
    c.run(0x25d0, 5)?;
    let ax = c.reg(AX);
    let v = c.word(0x118d);
    let d = c.sub(W, ax, v);
    let d = c.neg(W, d);
    c.set_reg(AX, d);
    c.set_byte(0xcf, d as u8);
    c.set_word(0x1158, 0);
    c.call(0x2806, 0x25e0)?;
    c.run(0x25e0, 1)?;
    c.call(0x2795, 0x25e3)?;
    c.run(0x25e3, 1)?;
    c.call(0xc49a, 0x25e6)?;
    c.run(0x25e6, 2)?;
    c.set_reg(AX, 0x2c);
    c.call(0xc13e, 0x25ec)?;
    c.run(0x25ec, 1)?;
    c.call(0xc0f4, 0x25ef)?;
    c.run(0x25ef, 3)?;
    c.set_reg(BP, 0x2555);
    c.set_reg8(r8::AL, 8);
    c.call(0xc108, 0x25f7)?;
    c.run(0x25f7, 2)?;
    c.set_reg(AX, 0x64);
    c.call(0xddb0, 0x25fd)?;
    c.run(0x25fd, 4)?;
    c.set_reg(CX, 0x18);
    c.set_reg(BP, 0x0a44);
    c.set_reg8(r8::AL, 6);
    c.call(0xc108, 0x2608)?;
    c.run(0x2608, 1)?;
    c.set_reg(CX, 0x18);
    let mut cx = 0x18u16;
    loop {
        c.run(0x260b, 5)?;
        c.push(cx);
        c.set_reg(BP, 0x0a44);
        c.set_reg(AX, 0x0c);
        let d = c.dec(W, cx);
        c.set_reg(CX, d);
        c.call(0xe353, 0x2616)?;
        c.run(0x2616, 2)?;
        cx = c.pop().wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    c.run(0x2619, 2)?;
    c.set_reg(AX, 0x27);
    c.call(0xab4f, 0x261f)?;
    c.run(0x261f, 3)?;
    c.set_reg(SI, 0x4c60);
    c.set_reg(BP, 0x15aa);
    let es = c.word(0xdbd6);
    c.set_sreg(ES, es);
    c.interp(0x2629, 0x262d)?;
    c.run(0x262d, 2)?;
    c.set_reg(AX, 0xc8);
    c.call(0xddb0, 0x2633)?;
    c.run(0x2633, 1)?;
    c.call(0xaba9, 0x2636)?;
    c.run(0x2636, 2)?;
    c.set_reg8(r8::AL, 1);
    c.call(0xab15, 0x263b)?;
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
            c.set_reg(BP, 0x26ac);
            c.set_reg(AX, 0x0c);
            c.call(0xe353, 0x264c)?;
            let last = si == 0x161e;
            c.run(0x264c, 3 + last as u32)?;
            si = c.pop();
            c.set_reg(SI, si);
            c.cmp(W, si, 0x161e);
            if last {
                c.call(0xac30, 0x2656)?;
            }
            let next = si.wrapping_add(6);
            let b = c.byte(next);
            c.run(0x2656, 3)?;
            si = c.add(W, si, 6);
            c.set_reg(SI, si);
            c.cmp(B, b as u16, 0xff);
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
    c.call(0x26ac, 0x2664)?;
    c.run(0x2664, 1)?;
    let cx0 = c.reg(CX);
    c.xor(W, cx0, cx0);
    let mut cx = 0u16;
    loop {
        c.run(0x2666, 5)?;
        cx = c.inc(W, cx);
        c.push(cx);
        c.set_reg(CX, cx);
        c.set_reg(BP, 0x0a44);
        c.set_reg(AX, 0x0c);
        c.call(0xe353, 0x2671)?;
        c.run(0x2671, 3)?;
        cx = c.pop();
        c.set_reg(CX, cx);
        c.cmp(W, cx, 0x19);
        if cx >= 0x19 {
            break;
        }
    }
    c.run(0x2677, 3)?;
    c.set_byte(0x47a9, 1);
    c.set_reg(BP, 0x2db1);
    c.call(0xc097, 0x2682)?;
    c.run(0x2682, 2)?;
    c.set_reg(AX, 0x2c);
    c.call(0xc13e, 0x2688)?;
    c.run(0x2688, 3)?;
    c.set_reg8(r8::AL, 6);
    c.set_reg(BP, 0x2555);
    c.call(0xc108, 0x2690)?;
    c.run(0x2690, 2)?;
    c.set_reg(BP, 0x2db1);
    c.call(0xc097, 0x2696)?;
    c.run(0x2696, 4)?;
    c.set_byte(0x47a9, 0);
    c.set_reg(BP, 0x26a6);
    c.set_reg8(r8::AL, 8);
    c.call(0xc108, 0x26a3)?;
    c.run(0x26a3, 1)?;
    c.jump(0x2773)
}
