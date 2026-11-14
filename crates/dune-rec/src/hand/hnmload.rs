//! Opening HNM videos: the file, its palette, its first frame; the intro's
//! video loaders; removing all frame tasks.

use super::pure;
use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DX, ES, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const COPY_RECT_AT_SI_TO_DI: u16 = 0x5b99;
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

/// `loc_0ceb0`: opens video AX and loads its palette; with flag 8 in its
/// entry, the header's 8 bytes from [`g::HNM_BODY_OFFSET`] noted just
/// before the entry. CF from the open.
pub fn hnm_open_noting_body(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xceb0)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::HNM_FLAG_NAME_PTR, 0xceb3)?;
    c.run(0xceb3, 2)?;
    let bx = c.reg(BX);
    c.push(bx);
    c.call(code::HNM_OPEN_AND_LOAD_PALETTE, 0xceb7)?;
    let failed = c.flag(CF);
    let sp = c.reg(SP);
    let di = c.word_in(SS, sp);
    let noted = !failed && c.byte(di) & 8 != 0;
    c.run(0xceb7, if failed { 3 } else if noted { 7 } else { 5 })?;
    c.pop();
    c.set_reg(DI, di);
    if !failed {
        if noted {
            let di = pure::sub(W, di, 8);
            c.set_reg(DI, di);
            c.set_reg(SI, g::HNM_BODY_OFFSET);
            c.call(code::COPY_RECT_AT_SI_TO_DI, 0xcec8)?;
            c.run(0xcec8, 1)?;
        }
    }
    c.ret()
}

/// `loc_0ce6c` (`hnm_initialize_memory_handler`): videos 2..8 lose flag 4
/// (kept in memory) unless memory is short (bit 2 of [2943h] or [39A9h]
/// at least 15Eh); with bits 3 of [2943h] all 25h lose flag 80h; videos
/// 2..7 opened (`loc_0ceb0`); then the video closed.
pub fn hnm_initialize_memory_handler(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xce6c)?;
    let mut c = Ctx::new(m);
    let mem = c.byte(0x2943);
    let big = c.word(0x39a9);
    let clear4 = mem & 2 != 0 || big < 0x15e;
    c.run(start, if mem & 2 != 0 { 2 } else { 4 } + if clear4 { 2 } else { 0 })?;
    if clear4 {
        c.set_reg(AX, 2);
        loop {
            c.call(code::HNM_FLAG_NAME_PTR, 0xce81)?;
            let ax = c.reg(AX).wrapping_add(1);
            c.run(0xce81, 4 + if ax < 9 { 1 } else { 0 })?;
            let bx = c.reg(BX);
            let f = c.byte(bx);
            let f = pure::and(B, f as u16, 0xfb);
            c.set_byte(bx, f as u8);
            let ax = pure::inc(W, ax.wrapping_sub(1));
            c.set_reg(AX, ax);
            if ax >= 9 {
                break;
            }
        }
    }
    let mem = c.byte(0x2943);
    let clear80 = mem & 3 != 0;
    c.run(0xce8a, 2 + if clear80 { 2 } else { 0 })?;
    if clear80 {
        c.set_reg(AX, 0);
        loop {
            c.call(code::HNM_FLAG_NAME_PTR, 0xce96)?;
            let ax = c.reg(AX).wrapping_add(1);
            c.run(0xce96, 4 + if ax < 0x25 { 1 } else { 0 })?;
            let bx = c.reg(BX);
            let f = c.byte(bx);
            let f = pure::and(B, f as u16, 0x7f);
            c.set_byte(bx, f as u8);
            let ax = pure::inc(W, ax.wrapping_sub(1));
            c.set_reg(AX, ax);
            if ax >= 0x25 {
                break;
            }
        }
    }
    c.run(0xce9f, 3)?;
    c.set_reg(AX, 2);
    c.push(2);
    c.call(0xceb0, 0xcea6)?;
    loop {
        let sp = c.reg(SP);
        let ax = c.word_in(SS, sp).wrapping_add(1);
        let more = ax < 8;
        c.run(0xcea6, 4 + if more { 2 } else { 1 })?;
        c.pop();
        let ax = pure::inc(W, ax.wrapping_sub(1));
        c.set_reg(AX, ax);
        if !more {
            return super::hnm::hnm_close_resource(c.m, 0xca01);
        }
        c.push(ax);
        c.call(0xceb0, 0xcea6)?;
    }
}

/// `hnm_read_header` (C93C): the HNM file [0DC00h] (kept in [0DC02h])
/// opened by name from the table entry `loc_0c921` returns (its flags
/// word in [0DBFEh], the name after it; `open_res_or_file_or_die`: the
/// handle in [35A6h], offset [0DC04h]:[0DC06h], size [0DC08h]:[0DC0Ah]).
/// The header read (`loc_0cd8f`, then the rest by `loc_0cdbf` into the
/// buffer [0DC0Ch], dropped to none when it would pass [0CE74h]); its
/// palette applied (`loc_0c1ba`, [0DBB4h] FFh); past the FFh padding,
/// the first frame's offset (the second entry when bit 2 of the flags
/// is set) gives the data's start [0DBF6h]:[0DBF8h] and the bytes left
/// [0DBFAh]:[0DBFCh]. CF set if a read failed.
pub fn hnm_read_header(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc93c)?;
    let mut c = Ctx::new(m);
    c.run(0xc93c, 3)?;
    let ax = c.word(0xdc00);
    c.set_reg(AX, ax);
    c.set_word(0xdc02, ax);
    c.call(0xc921, 0xc945)?;
    c.run(0xc945, 4)?;
    let bx = c.reg(BX);
    let ax = c.word(bx);
    c.set_reg(AX, ax);
    c.set_word(0xdbfe, ax);
    c.set_reg(DX, bx.wrapping_add(2));
    c.call(0xf229, 0xc950)?;
    c.run(0xc950, 8)?;
    for (r, at) in [(BX, 0x35a6u16), (AX, 0xdc04), (DX, 0xdc06), (CX, 0xdc08), (BP, 0xdc0a)] {
        let v = c.reg(r);
        c.set_word(at, v);
    }
    let a = c.word(0xdc1a);
    c.push(a);
    let b = c.word(0xdc0c);
    c.push(b);
    c.call(0xcd8f, 0xc96e)?;
    if !c.flag(CF) {
        let (si, ax) = (c.reg(SI), c.reg(AX));
        let (s, carry) = si.overflowing_add(ax);
        let lim = c.word(0xce74);
        let drop = carry || s > lim;
        c.run(0xc96e, 3 + if carry { 0 } else { 2 } + drop as u32 + 3)?;
        let s = c.add(W, si, ax);
        c.set_reg(SI, s);
        if !carry {
            c.cmp(W, s, lim);
        }
        if drop {
            c.set_word(0xdc0c, 0);
        }
        let a = c.sub(W, ax, 2);
        c.set_reg(AX, a);
        c.set_reg(CX, a);
        c.call(0xcdbf, 0xc988)?;
    } else {
        c.run(0xc96e, 1)?;
    }
    let cf = c.flag(CF);
    c.run(0xc988, 3 + cf as u32)?;
    let v = c.pop();
    c.set_word(0xdc0c, v);
    let v = c.pop();
    c.set_word(0xdc1a, v);
    if cf {
        return c.ret();
    }
    let (s0, e) = (c.word(0xdc0c), c.word(0xdc0e));
    let w = c.word_at(e, s0);
    let s1 = s0.wrapping_add(2);
    let (sum, carry) = w.overflowing_add(s1);
    let lim = c.word(0xce74);
    let reset = carry || sum > lim;
    c.run(0xc992, 4 + if carry { 0 } else { 2 } + reset as u32 + 2)?;
    c.set_reg(SI, s0);
    c.set_sreg(ES, e);
    let a = c.lods(W, ES);
    let a = c.add(W, a, s1);
    c.set_reg(AX, a);
    if !carry {
        c.cmp(W, a, lim);
    }
    if reset {
        c.xor(W, s1, s1);
        c.set_reg(SI, 0);
    }
    c.set_byte(0xdbb4, 0xff);
    c.call(0xc1ba, 0xc9ac)?;
    c.run(0xc9ac, 1)?;
    let si = c.reg(SI);
    let mut si = c.dec(W, si);
    c.set_reg(SI, si);
    loop {
        c.run(0xc9ad, 3)?;
        si = c.inc(W, si);
        c.set_reg(SI, si);
        let b = c.byte_in(ES, si);
        c.cmp(B, b as u16, 0xff);
        if b != 0xff {
            break;
        }
    }
    let f = c.byte(0xdbfe);
    let two = f & 4 != 0;
    c.run(0xc9b4, 3 + two as u32 + 16)?;
    let bx = c.reg(BX);
    c.xor(W, bx, bx);
    c.set_reg(BX, 0);
    c.test(B, f as u16, 4);
    let o = if two { 0x10u16 } else { 0 };
    c.set_reg(BX, o);
    let cx = c.word_in(ES, o.wrapping_add(si));
    c.set_reg(CX, cx);
    let bx = c.word_in(ES, o.wrapping_add(si).wrapping_add(2));
    c.set_reg(BX, bx);
    let a = c.word(0xdc04);
    let a = c.add(W, a, cx);
    c.set_word(0xdbf6, a);
    let a = c.word(0xdc06);
    let a = c.adc(W, a, bx);
    c.set_word(0xdbf8, a);
    let a = c.word(0xdc08);
    let a = c.sub(W, a, cx);
    c.set_word(0xdbfa, a);
    let a = c.word(0xdc0a);
    let a = c.sbb(W, a, bx);
    c.set_word(0xdbfc, a);
    c.set_reg(AX, a);
    c.set_flag(CF, false);
    c.ret()
}

/// `hnm_prefetch` (CB1A): more of the HNM file read ahead. While the
/// part wanted ([0DC20h]) is done, the next one is set up (`loc_0cd8f`,
/// `loc_0cc0c`) as long as frames are left ([0DBEAh] not [0DBEEh]) and
/// so is the file ([0DC08h]:[0DC0Ah]); else (the next video, from
/// CB61h) the original goes on. A part is read up to the next 2 KB
/// boundary of the file (unless [0DBFEh] bit 7), once the buffer has
/// room (`loc_0cc2b`; CF: not now), by `loc_0cdbf`.
pub fn hnm_prefetch(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcb1a)?;
    let mut c = Ctx::new(m);
    loop {
        let cx = c.word(0xdc20);
        if cx == 0 {
            c.run(0xcb1a, 2)?;
            c.set_reg(CX, 0);
            let (a, b) = (c.word(0xdbea), c.word(0xdbee));
            c.run(0xcb00, 3)?;
            c.set_reg(AX, a);
            c.cmp(W, a, b);
            if a == b {
                c.m.cpu.ip = 0xcb61;
                return Err(Bail);
            }
            let (a, b) = (c.word(0xdc08), c.word(0xdc0a));
            c.run(0xcb09, 3)?;
            c.set_reg(AX, a);
            let o = c.or(W, a, b);
            c.set_reg(AX, o);
            if o == 0 {
                c.m.cpu.ip = 0xcb61;
                return Err(Bail);
            }
            c.run(0xcb12, 1)?;
            c.call(0xcd8f, 0xcb15)?;
            c.run(0xcb15, 2)?;
            if c.flag(CF) {
                return c.ret();
            }
            c.call(0xcc0c, 0xcb1a)?;
            continue;
        }
        let f = c.byte(0xdbfe);
        let neg = f & 0x80 != 0;
        let a = (c.word(0xdc04).wrapping_neg() & 0x7ff) + 0x800;
        let clip = !neg && a < cx;
        c.run(0xcb1a, 2 + 2 + if neg { 0 } else { 6 + clip as u32 } + 1)?;
        c.set_reg(CX, cx);
        c.cmp(B, f as u16, 0);
        if !neg {
            let a = c.word(0xdc04);
            c.set_reg(AX, a);
            let a = c.neg(W, a);
            let a = c.and(W, a, 0x7ff);
            let h = c.add(B, a >> 8, 8);
            let a = (h << 8) | (a & 0xff);
            c.set_reg(AX, a);
            c.cmp(W, a, cx);
            if clip {
                c.set_reg(CX, a);
            }
        }
        c.call(0xcc2b, 0xcb3b)?;
        if c.flag(CF) {
            c.run(0xcb3b, 2)?;
            return c.ret();
        }
        c.run(0xcb3b, 3)?;
        let cx = c.reg(CX);
        let v = c.word(0xdc20);
        let v = c.sub(W, v, cx);
        c.set_word(0xdc20, v);
        return c.jump(0xcdbf);
    }
}

/// `hnm_decode_video_frame` (CC96): the frame waiting in segment
/// [0DC16h] (taken; none: nothing) at [0DC14h]. Kinds with [0DBFEh]
/// bits 4/5 (full-screen pictures) go on in the original (4AEBh /
/// 4AFDh). Else, unless [0DC24h] bit 10, its header (DI the screen
/// offset, CX the size: none when CL is 0, DX, BX) drawn into the
/// buffer [0DBDAh] by the driver: plain (far [38C9h]) for videos below
/// 19h ([0DC00h]), else by the decoder at CS:CC92h (its segment from
/// [38FBh]).
pub fn hnm_decode_video_frame(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{CS, DS};
    entered(start, 0xcc96)?;
    let mut c = Ctx::new(m);
    let seg = c.word(0xdc16);
    c.run(0xcc96, 6 + (seg == 0) as u32)?;
    let a = c.word(0x38fb);
    c.set_reg(AX, a);
    c.set_word_in(CS, 0xcc94, a);
    let b0 = c.reg(BP);
    c.xor(W, b0, b0);
    c.set_word(0xdc16, 0);
    c.set_reg(BP, seg);
    c.or(W, seg, seg);
    if seg == 0 {
        return c.ret();
    }
    let si = c.word(0xdc14);
    let f = c.byte(0xdbfe);
    c.run(0xcca7, 4)?;
    c.set_reg(SI, si);
    c.set_reg8(r8::AL, f);
    c.test(B, f as u16, 0x30);
    if f & 0x30 != 0 {
        c.run(0xccea, 3)?;
        c.test(B, f as u16, 0x20);
        return c.jump(if f & 0x20 != 0 { 0x4aeb } else { 0x4afd });
    }
    let k = c.word(0xdc24);
    c.run(0xccb2, 3)?;
    let ds = c.sreg(DS);
    c.push(ds);
    c.test(W, k, 0x400);
    if k & 0x400 != 0 {
        return pop_ds_ret_cce1(&mut c);
    }
    let w1 = c.word_at(seg, si.wrapping_add(2));
    c.run(0xccbb, 10)?;
    let es = c.word(0xdbda);
    c.set_sreg(ES, es);
    let vid = c.word(0xdc00);
    c.set_reg(BX, vid);
    c.set_sreg(DS, seg);
    let a = c.lods(W, DS);
    let h = c.and(B, a >> 8, 0xf9);
    let a = (h << 8) | (a & 0xff);
    c.set_reg(AX, a);
    c.set_reg(DI, a);
    let a = c.lods(W, DS);
    c.set_reg(CX, a);
    c.or(B, a & 0xff, a & 0xff);
    if w1 & 0xff == 0 {
        return pop_ds_ret_cce1(&mut c);
    }
    c.run(0xccd2, 6)?;
    let a = c.lods(W, DS);
    c.set_reg(DX, a);
    let a = c.lods(W, DS);
    c.set_reg(AX, vid);
    c.set_reg(BX, a);
    c.cmp(W, vid, 0x19);
    if vid >= 0x19 {
        c.interp(0xcce3, 0xcce8)?;
        c.run(0xcce8, 2)?;
    } else {
        c.interp(0xccdc, 0xcce1)?;
        c.run(0xcce1, 2)?;
    }
    let v = c.pop();
    c.set_sreg(DS, v);
    c.ret()
}

fn pop_ds_ret_cce1(c: &mut Ctx) -> Result<(), Bail> {
    use pc::cpu::DS;
    c.run(0xcce1, 2)?;
    let v = c.pop();
    c.set_sreg(DS, v);
    c.ret()
}

/// `hnm_decode_typed_chunk_video_to_bp` (CCF4): the frame at ES:SI (AX
/// bytes; none when it would pass [0CE74h]) split into its chunks: "sd"
/// a voice block ([0DC1Ch] its data, with PCM on: `check_pcm_enabled`),
/// "pl" a palette ([0DC1Eh]), "mm" a reference to an earlier frame
/// (followed when its tag matches, else the chunk is dropped:
/// `loc_0cc4e`). The picture (tag word AX) goes to segment BP, or to the
/// buffer [0DBDAh] with bit 10 ([0DC16h]:[0DC14h], [0DC24h] the tag):
/// packed (bit 10, or a size with bit 9) it is unpacked
/// (`hsq_decomp_skip_header_dssi_to_esdi`), else only its header copied
/// and the frame left in place ([0DC14h]/[0DC16h] at its data).
pub fn hnm_decode_typed_chunk(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{DS, ZF};
    entered(start, 0xccf4)?;
    let mut c = Ctx::new(m);
    let (ax, si) = (c.reg(AX), c.reg(SI));
    let (sum, carry) = ax.overflowing_add(si);
    let lim = c.word(0xce74);
    let zero = carry || sum > lim;
    c.run(0xccf4, 4 + if carry { 0 } else { 2 } + zero as u32)?;
    c.set_word(0xdc1c, 0xffff);
    c.set_word(0xdc1e, 0xffff);
    let s = pure::add(W, ax, si);
    c.set_reg(AX, s);
    if zero {
        c.set_reg(SI, 0);
    }
    let mut ax;
    loop {
        let si = c.reg(SI);
        let w = c.word_in(ES, si);
        if w == 0x6473 {
            c.run(0xcd0c, 4)?;
            c.lods(W, ES);
            c.call(0xae2f, 0xcd16)?;
            let z = c.flag(ZF);
            c.run(0xcd16, 1 + (!z) as u32 + 4)?;
            if !z {
                let si = c.reg(SI);
                c.set_word(0xdc1c, si);
            }
            let n = c.lods(W, ES);
            let n = pure::sub(W, n, 4);
            c.set_reg(AX, n);
            let si = c.reg(SI);
            let s = pure::add(W, si, n);
            c.set_reg(SI, s);
            ax = c.lods(W, ES);
        } else {
            c.run(0xcd0c, 3)?;
            ax = c.lods(W, ES);
        }
        if ax == 0x6c70 {
            c.run(0xcd25, 2 + 5)?;
            let n = c.lods(W, ES);
            let si = c.reg(SI);
            c.set_word(0xdc1e, si);
            let n = pure::sub(W, n, 4);
            c.set_reg(AX, n);
            let s = pure::add(W, si, n);
            c.set_reg(SI, s);
            continue;
        }
        c.run(0xcd25, 2)?;
        break;
    }
    if ax == 0x6d6d {
        let si = c.reg(SI);
        let bx = c.word_in(ES, si.wrapping_add(4));
        let (ns, ne) = (c.word_in(ES, si), c.word_in(ES, si.wrapping_add(2)));
        let a1 = c.word_at(ne, ns);
        let a2 = c.word_at(ne, ns.wrapping_add(2));
        let same = a1 == bx;
        c.run(0xcd37, 2 + 6 + (!same) as u32)?;
        c.set_reg(BX, bx);
        c.set_reg(SI, ns);
        c.set_sreg(ES, ne);
        c.lods(W, ES);
        c.lods(W, ES);
        if !same {
            return c.jump(0xcc4e);
        }
        ax = a2;
    } else {
        c.run(0xcd37, 2)?;
    }
    let big = ax & 0x400 != 0;
    c.run(0xcd4e, 6 + big as u32 + 9)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.sreg(ES);
    c.push(es);
    let bp = c.reg(BP);
    c.set_sreg(ES, bp);
    c.set_reg(DI, 0);
    if big {
        let b = c.word(0xdbda);
        c.set_sreg(ES, b);
    }
    let es = c.sreg(ES);
    c.set_word(0xdc16, es);
    c.set_word(0xdc14, 0);
    c.set_word(0xdc24, ax);
    let v = c.pop();
    c.set_sreg(DS, v);
    c.set_reg(CX, ax);
    let w = c.lods(W, DS);
    c.set_reg(CX, w);
    c.set_reg(AX, ax);
    if big {
        c.run(0xcd7c, 1)?;
        c.call(0xf403, 0xcd7f)?;
        return pop_ds_ret_cd7f(&mut c);
    }
    c.run(0xcd72, 4)?;
    c.stos(W);
    c.set_reg(AX, w);
    c.set_reg(CX, ax);
    c.stos(W);
    if ax == 0 {
        return pop_ds_ret_cd7f(&mut c);
    }
    let two = ax & 0x200 != 0;
    if two {
        c.run(0xcd77, 3)?;
        c.call(0xf403, 0xcd7f)?;
        return pop_ds_ret_cd7f(&mut c);
    }
    c.run(0xcd77, 2 + 6)?;
    let si = c.reg(SI);
    let s = pure::sub(W, si, 4);
    c.set_reg(SI, s);
    let seg = c.sreg(DS);
    c.set_reg(AX, seg);
    let v = c.pop();
    c.set_sreg(DS, v);
    c.set_word(0xdc14, s);
    c.set_word(0xdc16, seg);
    c.ret()
}

fn pop_ds_ret_cd7f(c: &mut Ctx) -> Result<(), Bail> {
    use pc::cpu::DS;
    c.run(0xcd7f, 2)?;
    let v = c.pop();
    c.set_sreg(DS, v);
    c.ret()
}
