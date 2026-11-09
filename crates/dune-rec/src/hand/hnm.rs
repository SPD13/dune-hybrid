//! HNM video files: reading and closing.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DS, DX, ES, SI},
    rec::Bail,
};

mod code {
    pub const HNM_PREFETCH: u16 = 0xcb1a;
    pub const HNM_DECODE_TYPED_CHUNK: u16 = 0xccf4;
}

/// Where the ring buffer of video data ends.
const RING_END: u16 = g::HNM_CE74;
/// The chunk tag of a frame without video (mm).
const NO_VIDEO_TAG: u16 = 0x6d6d;

/// `hnm_close_resource` (CA01): closes the video file, unless it is
/// DUNE.DAT's handle (videos inside it), and resets the frame counters.
/// Exit: CX = 0; flags from `xor cx, cx`.
pub fn hnm_close_resource(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xca01)?;
    let mut c = Ctx::new(m);
    let handle = c.word(g::HNM_FILE_HANDLE);
    if handle == 0 {
        c.run(start, 6)?; // xor; xchg; or; je; xor cx; ret
    } else {
        c.run(start, 5)?;
    }
    let bx = c.reg(BX);
    c.xor(W, bx, bx);
    c.set_word(g::HNM_FILE_HANDLE, 0);
    c.set_reg(BX, handle);
    c.or(W, handle, handle);
    if handle != 0 {
        c.call(0xce01, 0xca0e)?; // hnm_reset_frame_counter
        let dat = c.word(g::DUNE_DAT_HANDLE);
        if handle == dat {
            c.run(0xca0e, 4)?; // cmp; je; xor cx; ret
            c.cmp(W, handle, dat);
        } else {
            c.run(0xca0e, 3)?;
            c.cmp(W, handle, dat);
            c.set_reg8(r8::AH, 0x3e);
            c.interp(0xca16, 0xca18)?; // DOS close
            c.run(0xca18, 2)?;
        }
    }
    let cx = c.reg(CX);
    let zero = c.xor(W, cx, cx);
    c.set_reg(CX, zero);
    c.ret()
}

/// `hnm_read_bytes` (CDBF): reads CX bytes of the video at its file
/// position into the read buffer (DOS seek and read, again until a read
/// gives them all), then moves the position, the bytes left and both read
/// pointers on. CF set if no video file is open.
pub fn hnm_read_bytes(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcdbf)?;
    let mut c = Ctx::new(m);
    let handle = c.word(g::HNM_FILE_HANDLE);
    c.run(start, if handle < 1 { 4 } else { 3 })?;
    c.set_reg(BX, handle);
    c.cmp(W, handle, 1);
    if handle < 1 {
        return c.ret();
    }
    let read = loop {
        c.run(0xcdc8, 4)?;
        let cx = c.reg(CX);
        c.push(cx);
        let (low, high) = (c.word(g::HNM_FILE_OFFSET), c.word(g::HNM_FILE_OFFSET + 2));
        c.set_reg(CX, high);
        c.set_reg(DX, low);
        c.set_reg(AX, 0x4200);
        c.interp(0xcdd4, 0xcdd6)?; // DOS seek
        c.run(0xcdd6, 4)?;
        let cx = c.pop();
        c.set_reg(CX, cx);
        let ds = c.sreg(DS);
        c.push(ds);
        let (buf, seg) = (c.word(g::HNM_READ_PTR), c.word(g::HNM_READ_PTR + 2));
        c.set_reg(DX, buf);
        c.set_sreg(DS, seg);
        c.set_reg8(r8::AH, 0x3f);
        c.interp(0xcdde, 0xcde0)?; // DOS read
        let (got, wanted) = (c.reg(AX), c.reg(CX));
        if got >= wanted {
            break got;
        }
        c.run(0xcde0, 3)?; // pop ds; cmp; jb
        let ds = c.pop();
        c.set_sreg(DS, ds);
        c.cmp(W, got, wanted);
    };
    c.run(0xcde0, 11)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let wanted = c.reg(CX);
    c.cmp(W, read, wanted);
    // The bytes left (32 bits) go down, the file offset up.
    let left = c.word(g::HNM_FILE_REMAIN);
    let left = c.sub(W, left, read);
    c.set_word(g::HNM_FILE_REMAIN, left);
    let left_high = c.word(g::HNM_FILE_REMAIN + 2);
    let left_high = c.sbb(W, left_high, 0);
    c.set_word(g::HNM_FILE_REMAIN + 2, left_high);
    let offset = c.word(g::HNM_FILE_OFFSET);
    let offset = c.add(W, offset, read);
    c.set_word(g::HNM_FILE_OFFSET, offset);
    let offset_high = c.word(g::HNM_FILE_OFFSET + 2);
    let offset_high = c.adc(W, offset_high, 0);
    c.set_word(g::HNM_FILE_OFFSET + 2, offset_high);
    for p in [g::HNM_READ_PTR, g::HNM_READ_PTR_2] {
        let v = c.word(p);
        let v = c.add(W, v, read);
        c.set_word(p, v);
    }
    c.set_flag(CF, false);
    c.ret()
}

/// `loc_0ca59`: the frame's tick from now.
pub fn hnm_reset_frame_tick(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xca59)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let t = c.word(g::TIMER);
    c.set_reg(AX, t);
    c.set_word(g::HNM_FRAME_TICK, t);
    c.ret()
}

/// `loc_0ca8f`: prefetches the video's data, the video flags' bit 7 noted
/// at [`g::HNM_FLAG_MSB`] for the while.
pub fn hnm_prefetch_flagged(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xca8f)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let flags = c.byte(g::HNM_FLAGS);
    let msb = c.and(B, flags as u16, 0x80) as u8;
    c.set_reg8(r8::AL, msb);
    c.set_byte(g::HNM_FLAG_MSB, msb);
    c.call(code::HNM_PREFETCH, 0xca9a)?;
    c.run(0xca9a, 2)?;
    c.set_byte(g::HNM_FLAG_MSB, 0);
    c.ret()
}

/// `loc_0ca9a`: clears [`g::HNM_FLAG_MSB`].
pub fn hnm_clear_flag_msb(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xca9a)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(g::HNM_FLAG_MSB, 0);
    c.ret()
}

/// `loc_0caa0`: decodes the next frame's video chunk when it is all in the
/// buffer (nothing left to fetch, and its size within what was read, or a
/// frame without video), into the front buffer (the back buffer with
/// flag 40h). CF set (AX unchanged) when it is not.
pub fn hnm_decode_frame_if_read(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcaa0)?;
    let mut c = Ctx::new(m);
    let pending = c.word(g::HNM_DC16);
    if pending > 0 {
        c.run(start, 3)?;
        c.cmp(W, pending, 0);
        return c.ret();
    }
    let read = c.word(g::HNM_READ_PTR_2);
    if read == 0 {
        c.run(start, 6)?;
        c.cmp(W, pending, 0);
        c.set_reg(CX, read);
        c.set_flag(CF, true);
        return c.ret();
    }
    let (si, seg) = (c.word(g::HNM_DC10), c.word(g::HNM_DC10 + 2));
    let size = c.word_at(seg, si);
    let tag = c.word_at(seg, si.wrapping_add(2));
    let no_video = tag == NO_VIDEO_TAG;
    if !no_video && read < size {
        c.run(start, 2 + 3 + 4 + 3)?;
    } else {
        let back = c.byte(g::HNM_FLAGS) & 0x40 != 0;
        c.run(start, 2 + 3 + 4 + if no_video { 0 } else { 2 } + 3 + back as u32 + 1)?;
    }
    c.cmp(W, pending, 0);
    c.set_reg(CX, read);
    c.set_flag(CF, true);
    c.set_reg(SI, si);
    c.set_sreg(ES, seg);
    c.lods(W, ES);
    let si = c.reg(SI);
    let tag = c.word_in(ES, si);
    c.cmp(W, tag, NO_VIDEO_TAG);
    if !no_video {
        c.cmp(W, read, size);
        if read < size {
            return c.ret();
        }
    }
    let front = c.word(g::FRAMEBUFFER_FRONT);
    c.set_reg(BP, front);
    let flags = c.byte(g::HNM_FLAGS);
    c.test(B, flags as u16, 0x40);
    if flags & 0x40 != 0 {
        let back = c.word(g::BACKBUFFER);
        c.set_reg(BP, back);
    }
    c.call(code::HNM_DECODE_TYPED_CHUNK, 0xcad1)?;
    c.run(0xcad1, 2)?;
    let ax = c.reg(AX);
    let z = c.xor(W, ax, ax);
    c.set_reg(AX, z);
    c.ret()
}

/// `loc_0cc0c`: SI (a chunk's start) on by AX, its size; past the ring's
/// end (or wrapping) the read pointer goes back to 0 and what it was is
/// noted at [`g::HNM_DC18`]. Then [`g::HNM_DC20`] = AX - 2 and one more
/// frame counted ([`g::HNM_FRAME_2`]).
pub fn hnm_advance_chunk(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcc0c)?;
    let mut c = Ctx::new(m);
    let (si, ax) = (c.reg(SI), c.reg(AX));
    let (end, carry) = si.overflowing_add(ax);
    let limit = c.word(RING_END);
    let wrap = carry || end > limit;
    c.run(start, if carry { 2 } else { 4 } + if wrap { 3 } else { 0 } + 4)?;
    let end = c.add(W, si, ax);
    c.set_reg(SI, end);
    if !carry {
        c.cmp(W, end, limit);
    }
    if wrap {
        let cx = c.reg(CX);
        c.xor(W, cx, cx);
        let old = c.word(g::HNM_READ_PTR);
        c.set_word(g::HNM_READ_PTR, 0);
        c.set_reg(CX, old);
        c.set_word(g::HNM_DC18, old);
    }
    let size = c.sub(W, ax, 2);
    c.set_reg(AX, size);
    c.set_word(g::HNM_DC20, size);
    let n = c.word(g::HNM_FRAME_2);
    let n = c.inc(W, n);
    c.set_word(g::HNM_FRAME_2, n);
    c.ret()
}

/// `loc_0cc2b`: whether CX more bytes fit in the ring buffer: CF set when
/// they do not (the flags of the comparison that decided).
pub fn hnm_ring_has_room(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcc2b)?;
    let mut c = Ctx::new(m);
    let (w, r, cx) = (c.word(g::HNM_READ_PTR), c.word(g::HNM_DC10), c.reg(CX));
    let below = w < r;
    if below {
        let need = w.wrapping_add(cx).wrapping_add(0x12);
        if r < need {
            c.run(start, 9)?;
            c.set_reg(AX, w);
            c.set_reg(BX, r);
            c.cmp(W, w, r);
            let a = c.add(W, w, cx);
            let a = c.add(W, a, 0x12);
            c.set_reg(AX, a);
            c.cmp(W, r, a);
            return c.ret();
        }
    }
    let read = c.word(g::HNM_READ_PTR_2);
    let (_, carry) = read.wrapping_add(0x0a).overflowing_add(cx);
    c.run(start, 4 + if below { 4 } else { 0 } + 4 + if carry { 1 } else { 2 })?;
    c.set_reg(AX, w);
    c.set_reg(BX, r);
    c.cmp(W, w, r);
    if below {
        let a = c.add(W, w, cx);
        let a = c.add(W, a, 0x12);
        c.cmp(W, r, a);
    }
    let a = c.add(W, read, 0x0a);
    let a = c.add(W, a, cx);
    c.set_reg(AX, a);
    if !carry {
        let v = c.word(g::HNM_DC18);
        c.cmp(W, v, a);
    }
    c.ret()
}

/// `loc_0cc4e`: past the chunk at the ring's read pointer: its size off
/// what was read, the pointer on by it (back to 0 past the ring's end);
/// the frame number on by one (to 1, and [`g::HNM_FRAME_3`] to FFFFh, after
/// the last).
pub fn hnm_consume_chunk(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcc4e)?;
    let mut c = Ctx::new(m);
    let (si, seg) = (c.word(g::HNM_DC10), c.word(g::HNM_DC10 + 2));
    let size = c.word_at(seg, si);
    let p = si.wrapping_add(2);
    let (end, carry) = p.overflowing_add(size);
    let limit = c.word(RING_END);
    let wrap = carry || end > limit;
    let frame = c.word(g::HNM_FRAME).wrapping_add(1);
    let last = c.word(g::HNM_FRAME_3);
    let over = frame > last;
    c.run(start, 5 + if carry { 0 } else { 2 } + if wrap { 3 } else { 0 } + 5 + if over { 2 } else { 0 } + 2)?;
    c.set_sreg(ES, seg);
    c.set_reg(SI, si);
    let size = c.lods(W, ES);
    let read = c.word(g::HNM_READ_PTR_2);
    let read = c.sub(W, read, size);
    c.set_word(g::HNM_READ_PTR_2, read);
    let end = c.add(W, p, size);
    c.set_reg(SI, end);
    if !carry {
        c.cmp(W, end, limit);
    }
    let mut ax = size;
    if wrap {
        let v = c.sub(W, size, 2);
        c.set_word(g::HNM_DC10, v);
        ax = c.xor(W, v, v);
    }
    let v = c.word(g::HNM_DC10);
    let v = c.add(W, v, ax);
    c.set_word(g::HNM_DC10, v);
    let f = c.word(g::HNM_FRAME);
    let mut f = c.inc(W, f);
    c.cmp(W, f, last);
    if over {
        f = 1;
        c.set_word(g::HNM_FRAME_3, 0xffff);
    }
    c.set_reg(AX, f);
    c.set_word(g::HNM_FRAME, f);
    c.ret()
}

/// `loc_0ce4b`: notes the video BX and frame AX.
pub fn hnm_note_video_and_frame(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xce4b)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let (bx, ax) = (c.reg(BX), c.reg(AX));
    c.set_word(g::HNM_VIDEO_2, bx);
    c.set_word(g::HNM_FRAME_4, ax);
    c.ret()
}

/// `IRULx_draw_or_clear_subtitle`: the subtitle entry SI ([3622h]) of the
/// table 35A8h (4 bytes each): an odd half-entry draws its sprite
/// ((SI - 35A8h) / 4) at (0, BEh); else the subtitle strip ED80h of the
/// buffer [0DBD8h] cleared (B40h words). Bails with DF set.
pub fn irulx_draw_or_clear_subtitle(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcf4b)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let d = si.wrapping_sub(0x35a8);
    let draw = d & 2 != 0;
    if !draw && c.flag(pc::cpu::DF) {
        return Err(Bail);
    }
    c.run(start, 6 + if draw { 3 } else { 5 })?;
    c.set_reg(AX, si);
    c.set_word(0x3622, si);
    let a = c.sub(W, si, 0x35a8);
    let a = c.shr1(W, a, 2);
    c.set_reg(AX, a);
    if draw {
        c.set_reg(BX, 0xbe);
        c.xor(W, 0, 0);
        c.set_reg(DX, 0);
        return super::draw::draw_sprite_clobbering_bx_dx(c.m, 0xc22f);
    }
    c.set_reg(DI, 0xed80);
    let es = c.word(0xdbd8);
    c.set_sreg(pc::cpu::ES, es);
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.set_reg(CX, 0xb40);
    c.rep(0xab);
    c.run(0xcf6f, 1)?;
    c.ret()
}
