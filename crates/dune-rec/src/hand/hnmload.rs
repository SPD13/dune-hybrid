//! Opening HNM videos: the file, its palette, its first frame; the intro's
//! video loaders; removing all frame tasks.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DX, ES, SI},
    rec::Bail,
};

mod code {
    pub const HNM_CLOSE_RESOURCE: u16 = 0xca01;
    pub const HNM_RESET_BUFFERS: u16 = 0xce1a;
    pub const HNM_RESET_FRAME_COUNTER: u16 = 0xce01;
    pub const HNM_FLAG_NAME_PTR: u16 = 0xc921;
    pub const OPEN_RES_OR_FILE_OR_DIE: u16 = 0xf229;
    pub const HNM_READ_HEADER_SIZE: u16 = 0xcd8f;
    pub const HNM_READ_BYTES: u16 = 0xcdbf;
    pub const APPLY_PALETTE: u16 = 0xc1ba;
    pub const HNM_OPEN_AND_LOAD_PALETTE: u16 = 0xc92b;
    pub const HNM_OPEN_AT_BODY_START: u16 = 0xcda0;
    pub const HNM_DECODE_TYPED_CHUNK: u16 = 0xccf4;
    pub const DECODE_SD_BLOCK: u16 = 0xaa0f;
    pub const HNM_DECODE_VIDEO_FRAME: u16 = 0xcc96;
    pub const HNM_PREFETCH: u16 = 0xcb1a;
    pub const PLAY_MUSIC_MORNING: u16 = 0xad57;
    pub const PLAY_MUSIC: u16 = 0xad95;
    pub const GFX_CLEAR_ACTIVE_FRAMEBUFFER: u16 = 0xc0ad;
    pub const CLEAR_GLOBAL_Y_OFFSET: u16 = 0x0579;
    pub const SET_FB1_AS_ACTIVE_FRAMEBUFFER: u16 = 0xc07c;
}

/// Frames read ahead after the first (unless the video's flag 40h).
const PREFETCH_FRAMES: u16 = 0x32;

/// `hnm_open_and_load_palette` (C92B): opens video AX (closing the one
/// open): its flags, the file (from DUNE.DAT or its own), the header with
/// its palette (applied); then notes where the body starts in the file
/// ([DBF6h]) and its size ([DBFAh]). CF set if it could not be read.
pub fn hnm_open_and_load_palette(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc92b)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let video = c.reg(AX);
    c.set_word(g::HNM_VIDEO, video);
    c.call(code::HNM_CLOSE_RESOURCE, 0xc931)?;
    c.run(0xc931, 1)?;
    c.call(code::HNM_RESET_BUFFERS, 0xc934)?;
    c.run(0xc934, 2)?;
    c.set_byte(g::HNM_STATE, 0);
    c.call(code::HNM_RESET_FRAME_COUNTER, 0xc93c)?;
    c.run(0xc93c, 3)?;
    let video = c.word(g::HNM_VIDEO);
    c.set_reg(AX, video);
    c.set_word(g::HNM_VIDEO_2, video);
    c.call(code::HNM_FLAG_NAME_PTR, 0xc945)?;
    c.run(0xc945, 4)?;
    let bx = c.reg(BX);
    let flags = c.word(bx);
    c.set_reg(AX, flags);
    c.set_word(g::HNM_FLAGS, flags);
    c.set_reg(DX, bx.wrapping_add(2));
    c.call(code::OPEN_RES_OR_FILE_OR_DIE, 0xc950)?;
    c.run(0xc950, 8)?;
    let (bx, ax, dx, cx, bp) = (c.reg(BX), c.reg(AX), c.reg(DX), c.reg(CX), c.reg(BP));
    c.set_word(g::HNM_FILE_HANDLE, bx);
    c.set_word(g::HNM_FILE_OFFSET, ax);
    c.set_word(g::HNM_FILE_OFFSET + 2, dx);
    c.set_word(g::HNM_FILE_REMAIN, cx);
    c.set_word(g::HNM_FILE_REMAIN + 2, bp);
    let p2 = c.word(g::HNM_READ_PTR_2);
    c.push(p2);
    let p = c.word(g::HNM_READ_PTR);
    c.push(p);
    c.call(code::HNM_READ_HEADER_SIZE, 0xc96e)?;

    // The header (its size first) read into the buffer, from 0 if it would
    // not fit after the read pointer.
    let failed = if c.flag(CF) {
        c.run(0xc96e, 4)?; // jb; pop; pop; jb
        true
    } else {
        let (si, ax) = (c.reg(SI), c.reg(AX));
        let end = c.word(g::HNM_CE74);
        let (sum, carry) = si.overflowing_add(ax);
        let reset = carry || sum > end;
        c.run(0xc96e, 1 + if carry { 3 } else if reset { 5 } else { 4 } + 3)?;
        let si = c.add(W, si, ax);
        c.set_reg(SI, si);
        if !carry {
            c.cmp(W, si, end);
        }
        if reset {
            c.set_word(g::HNM_READ_PTR, 0);
        }
        let n = c.sub(W, ax, 2);
        c.set_reg(AX, n);
        c.set_reg(CX, n);
        c.call(code::HNM_READ_BYTES, 0xc988)?;
        let failed = c.flag(CF);
        c.run(0xc988, 3)?;
        failed
    };
    let p = c.pop();
    c.set_word(g::HNM_READ_PTR, p);
    let p2 = c.pop();
    c.set_word(g::HNM_READ_PTR_2, p2);
    if failed {
        c.run(0xc9e7, 1)?; // ret
        return c.ret();
    }

    // C992: the palette at the read pointer.
    let (si, es) = (c.word(g::HNM_READ_PTR), c.word(g::HNM_READ_PTR + 2));
    let size = c.word_at(es, si);
    let after = si.wrapping_add(2);
    let (sum, carry) = size.overflowing_add(after);
    let end = c.word(g::HNM_CE74);
    let reset = carry || sum > end;
    c.run(0xc992, 4 + if carry { 1 } else if reset { 3 } else { 2 } + 2)?;
    c.set_reg(SI, si);
    c.set_sreg(ES, es);
    c.lods(W, ES);
    let s = c.add(W, size, after);
    c.set_reg(AX, s);
    if !carry {
        c.cmp(W, s, end);
    }
    if reset {
        let si = c.reg(SI);
        let zero = c.xor(W, si, si);
        c.set_reg(SI, zero);
    }
    c.set_byte(g::PALETTE_RESOURCE, 0xff);
    c.call(code::APPLY_PALETTE, 0xc9ac)?;

    // Past the FFh padding.
    c.run(0xc9ac, 1)?;
    let si = c.reg(SI);
    let mut si = c.dec(W, si);
    c.set_reg(SI, si);
    loop {
        let pad = c.byte_in(ES, si.wrapping_add(1)) == 0xff;
        c.run(0xc9ad, 3)?;
        si = c.inc(W, si);
        c.set_reg(SI, si);
        let b = c.byte_in(ES, si);
        c.cmp(B, b as u16, 0xff);
        if !pad {
            break;
        }
    }

    // The body's offset in the header (at +10h with flag 4).
    let flags = c.byte(g::HNM_FLAGS);
    let wide = flags & 4 != 0;
    c.run(0xc9b4, 3 + wide as u32 + 16)?;
    c.xor(W, 0, 0);
    c.set_reg(BX, 0);
    c.test(B, flags as u16, 4);
    let off = if wide { 0x10 } else { 0 };
    c.set_reg8(r8::BL, off);
    let low = c.word_in(ES, si.wrapping_add(off as u16));
    let high = c.word_in(ES, si.wrapping_add(off as u16).wrapping_add(2));
    c.set_reg(CX, low);
    c.set_reg(BX, high);
    let pos = c.word(g::HNM_FILE_OFFSET);
    let v = c.add(W, pos, low);
    c.set_word(g::HNM_BODY_OFFSET, v);
    let pos = c.word(g::HNM_FILE_OFFSET + 2);
    let v = c.adc(W, pos, high);
    c.set_word(g::HNM_BODY_OFFSET + 2, v);
    let left = c.word(g::HNM_FILE_REMAIN);
    let v = c.sub(W, left, low);
    c.set_word(g::HNM_BODY_SIZE, v);
    let left = c.word(g::HNM_FILE_REMAIN + 2);
    let v = c.sbb(W, left, high);
    c.set_reg(AX, v);
    c.set_word(g::HNM_BODY_SIZE + 2, v);
    c.set_flag(CF, false);
    c.ret()
}

/// From CA1B: opens video AX and shows its first frame (decoding its
/// chunks into the second framebuffer, then the frame), reads 50 frames
/// ahead unless the video's flag 40h, and starts its timer. On failure,
/// the video is closed.
pub(super) fn load_first_frame(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xca1b, 1)?;
    c.call(code::HNM_OPEN_AND_LOAD_PALETTE, 0xca1e)?;
    if c.flag(CF) {
        c.run(0xca1e, 1)?;
        return super::hnm::hnm_close_resource(c.m, 0xca01);
    }
    c.run(0xca1e, 2)?;
    c.call(code::HNM_OPEN_AT_BODY_START, 0xca23)?;
    if c.flag(CF) {
        c.run(0xca23, 1)?;
        return super::hnm::hnm_close_resource(c.m, 0xca01);
    }
    c.run(0xca23, 6)?;
    c.set_byte(g::TRANSITION_ACTIVE, 0);
    let (si, es) = (c.word(g::HNM_DC10), c.word(g::HNM_DC10 + 2));
    c.set_reg(SI, si);
    c.set_sreg(ES, es);
    c.lods(W, ES);
    let fb2 = c.word(g::FRAMEBUFFER_2);
    c.set_reg(BP, fb2);
    c.call(code::HNM_DECODE_TYPED_CHUNK, 0xca37)?;
    c.run(0xca37, 1)?;
    c.call(code::DECODE_SD_BLOCK, 0xca3a)?;
    c.run(0xca3a, 1)?;
    c.call(code::HNM_DECODE_VIDEO_FRAME, 0xca3d)?;
    c.run(0xca3d, 1)?;
    c.call(code::HNM_RESET_BUFFERS, 0xca40)?;
    let flags = c.byte(g::HNM_FLAGS);
    let no_prefetch = flags & 0x40 != 0;
    c.run(0xca40, 4 + if no_prefetch { 3 } else { 3 })?;
    for p in [g::HNM_FRAME, g::HNM_FRAME_2] {
        let v = c.word(p);
        let v = c.inc(W, v);
        c.set_word(p, v);
    }
    c.test(B, flags as u16, 0x40);
    if !no_prefetch {
        c.set_reg(CX, PREFETCH_FRAMES);
        loop {
            let cx = c.reg(CX);
            c.push(cx);
            c.call(code::HNM_PREFETCH, 0xca56)?;
            let sp = c.reg(pc::cpu::SP);
            let left = c.word_in(pc::cpu::SS, sp).wrapping_sub(1);
            c.run(0xca56, 2 + if left == 0 { 3 } else { 2 })?;
            c.pop();
            c.set_reg(CX, left);
            if left == 0 {
                break;
            }
        }
    }
    let now = c.word(g::TIMER);
    c.set_reg(AX, now);
    c.set_word(g::HNM_FRAME_TICK, now);
    c.ret()
}

/// `hnm_load_first_frame` (CA1B).
pub fn hnm_load_first_frame(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xca1b)?;
    load_first_frame(&mut Ctx::new(m))
}

/// `hnm_open_at_body_start` (CDA0): the read buffer set up at the end of
/// the second framebuffer for the body's first frame (its size, from the
/// header), then it is read. CF set on failure.
pub fn hnm_open_at_body_start(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcda0)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::HNM_RESET_BUFFERS, 0xcda3)?;
    c.run(0xcda3, 1)?;
    c.call(code::HNM_READ_HEADER_SIZE, 0xcda6)?;
    if c.flag(CF) {
        c.run(0xcda6, 2)?;
        return c.ret();
    }
    c.run(0xcda6, 9)?;
    let size = c.reg(AX);
    let end = c.word(g::HNM_CE74);
    let di = c.sub(W, end, size);
    let di = c.sub(W, di, 2);
    c.set_reg(DI, di);
    c.set_word(g::HNM_DC10, di);
    c.stos(W);
    let di = c.reg(DI);
    c.set_word(g::HNM_READ_PTR, di);
    c.set_reg(CX, size);
    let cx = c.sub(W, size, 2);
    c.set_reg(CX, cx);
    super::hnm::hnm_read_bytes(c.m, 0xcdbf)
}

/// An intro video loader: what it does first (`before`, at most two
/// calls), then `mov ax, video; jmp CA1Bh`.
fn loader(c: &mut Ctx, entry: u16, before: &[u16], video: u16) -> Result<(), Bail> {
    let mut at = entry;
    for &target in before {
        c.run(at, 1)?;
        c.call(target, at + 3)?;
        at += 3;
    }
    c.run(at, 2)?;
    c.set_reg(AX, video);
    load_first_frame(c)
}

macro_rules! loaders {
    ($($(#[$doc:meta])* $name:ident, $entry:expr, [$($before:expr),*], $video:expr;)*) => {$(
        $(#[$doc])*
        pub fn $name(m: &mut Machine, start: u16) -> Result<(), Bail> {
            entered(start, $entry)?;
            loader(&mut Ctx::new(m), $entry, &[$($before),*], $video)
        }
    )*};
}

loaders! {
    /// `load_VIRGIN_HNM` (061C): the morning music, video 15h.
    load_virgin_hnm, 0x061c, [code::PLAY_MUSIC_MORNING], 0x15;
    /// `load_CRYO2_HNM` (0658): a cleared screen, video 17h.
    load_cryo2_hnm, 0x0658, [code::GFX_CLEAR_ACTIVE_FRAMEBUFFER], 0x17;
    /// `load_PRESENT_HNM` (0678): video 18h.
    load_present_hnm, 0x0678, [code::CLEAR_GLOBAL_Y_OFFSET, code::GFX_CLEAR_ACTIVE_FRAMEBUFFER], 0x18;
    /// `load_INTRO_HNM` (069E): video 0Fh into the front buffer.
    load_intro_hnm, 0x069e, [code::SET_FB1_AS_ACTIVE_FRAMEBUFFER, code::CLEAR_GLOBAL_Y_OFFSET], 0x0f;
    /// `play_CREDITS_HNM` (09EF): video 14h.
    play_credits_hnm, 0x09ef, [], 0x14;
}

/// `load_CRYO_HNM` (064D): music 0Ah, video 16h.
pub fn load_cryo_hnm(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x064d)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg8(r8::AL, 0x0a);
    c.call(code::PLAY_MUSIC, 0x0652)?;
    c.run(0x0652, 2)?;
    c.set_reg(AX, 0x16);
    load_first_frame(&mut c)
}

/// `remove_all_frame_tasks` (0911): every frame task the game uses gone
/// (the room's, the globe's, ...), the sky to its night variant.
pub fn remove_all_frame_tasks(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0911)?;
    let mut c = Ctx::new(m);
    let mut at = start;
    for target in [0x39e6, 0xb930, 0x0b21, 0x9985, 0x98e6] {
        c.run(at, 1)?;
        c.call(target, at + 3)?;
        at += 3;
    }
    c.run(0x0920, 4)?;
    c.set_byte(g::SKY_VARIANT, 1);
    c.set_byte(g::FRAME_TASKS_ACTIVE, 0);
    c.set_reg(SI, 0x070c);
    c.call(0xda5f, 0x0930)?;
    c.run(0x0930, 2)?;
    c.set_reg(SI, 0x3916);
    c.call(0xda5f, 0x0936)?;
    c.run(0x0936, 1)?;
    c.call(0x0a3e, 0x0939)?;
    c.run(0x0939, 2)?;
    c.set_reg(SI, 0x0826);
    super::tasks::remove_frame_task(c.m, 0xda5f)
}
