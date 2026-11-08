//! HSQ, the game's LZ compression: the checksum check, the in-place set-up
//! and the decompressor.
//!
//! A compressed resource starts with a 6-byte header: the decompressed
//! size (word), 0, the compressed size (word, header included), and a
//! byte that makes the six sum to ABh. Then a stream of control bits (16 at
//! a time, low bit first, read as needed) and data bytes:
//! - 1: a literal byte;
//! - 0 then 0, then two bits n: copy n + 2 bytes from 256 or fewer back
//!   (the distance's low byte follows);
//! - 0 then 1: a word with a 13-bit distance back and a 3-bit count (copy
//!   count + 2); a count of 0 means the count is in the next byte (+ 2),
//!   and a 0 there ends the data.

use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DS, DX, ES, SI, SP},
    rec::Bail,
};

/// The header's bytes sum to this.
const CHECKSUM: u8 = 0xab;
/// Room between the decompressed data and the compressed copy moved
/// behind it (the copy must stay ahead of the output).
const SLACK: u16 = 0x40;

/// The next control bit (left in CF) from the queue in BP: `shr bp, 1`;
/// when that empties it (the marker bit went out), 16 more bits from DS:SI
/// (`lodsw; mov bp, ax; stc; rcr bp, 1`, the marker on top). That is 2
/// instructions with their jump, or 6 with the reload ([`will_reload`]).
fn next_bit(c: &mut Ctx) -> bool {
    let bp = c.reg(BP);
    let rest = c.shr1(W, bp, 1);
    c.set_reg(BP, rest);
    if rest != 0 {
        return bp & 1 != 0;
    }
    let bits = c.lods(W, DS);
    c.set_reg(BP, bits);
    c.set_flag(CF, true);
    let bp = c.rcr1(W, bits);
    c.set_reg(BP, bp);
    bits & 1 != 0
}

/// Whether [`next_bit`] will reload, without changing anything.
fn will_reload(c: &Ctx) -> bool {
    c.reg(BP) >> 1 == 0
}

/// The decompressor's loop (F435) to its end: DS:SI the stream (its
/// control bits in BP), ES:DI the output, DS = ES while copying.
fn decompress(c: &mut Ctx) -> Result<(), Bail> {
    loop {
        // F435: shr bp, 1; je F43Eh (reload: 5 more); jae F446h / jb F43Bh.
        let reload = will_reload(c);
        c.run(0xf435, if reload { 7 } else { 3 })?;
        let literal = next_bit(c);
        if literal {
            c.run(0xf43b, 2)?; // movsb; jmp F435h
            c.movs(B);
            continue;
        }

        // F446: xor cx, cx; the next bit: a short or a long copy.
        let reload = will_reload(c);
        c.run(0xf446, 1 + if reload { 6 } else { 2 } + 1)?;
        let cx = c.reg(CX);
        let zero = c.xor(W, cx, cx);
        c.set_reg(CX, zero);
        let long = next_bit(c);
        if !long {
            // Two bits of count, then the distance byte.
            let mut count = 0u16;
            for at in [0xf454, 0xf460] {
                let reload = will_reload(c);
                c.run(at, if reload { 6 } else { 2 } + 1)?;
                next_bit(c);
                count = c.rcl1(W, count);
                c.set_reg(CX, count);
            }
            c.run(0xf46c, 2)?; // lodsb; mov ah, FFh
            let back = c.lods(B, DS);
            c.set_reg(AX, 0xff00 | back);
        } else {
            // F482: a word: distance (13 bits) and count (3 bits).
            let si = c.reg(SI);
            let word = c.word_in(DS, si);
            let short_count = word & 7;
            if short_count != 0 {
                c.run(0xf482, 8)?;
            } else {
                let next = c.byte_in(DS, si.wrapping_add(2));
                c.run(0xf482, if next != 0 { 14 } else { 14 + 7 })?;
            }
            let word = c.lods(W, DS);
            c.set_reg8(r8::CL, word as u8);
            let ax = c.shr1(W, word, 3);
            let ax = c.or(B, ax >> 8, 0xe0) << 8 | ax & 0xff;
            c.set_reg(AX, ax);
            let cl = c.and(B, word & 0xff, 7) as u8;
            c.set_reg8(r8::CL, cl);
            if cl == 0 {
                c.set_reg(BX, ax);
                let count = c.lods(B, DS) as u8;
                c.set_reg8(r8::CL, count);
                c.set_reg(AX, ax);
                c.or(B, count as u16, count as u16);
                if count == 0 {
                    return finish(c);
                }
            }
        }

        // F46F: copy CX + 2 bytes from DI + AX (negative) in the output.
        c.run(0xf46f, 7)?;
        let (ax, di) = (c.reg(AX), c.reg(DI));
        let from = c.add(W, ax, di);
        let si = c.reg(SI);
        c.set_reg(AX, si);
        c.set_reg(SI, from);
        let (ds, es) = (c.sreg(DS), c.sreg(ES));
        c.set_reg(BX, ds);
        c.set_reg(DX, es);
        c.set_sreg(DS, es);
        let cx = c.reg(CX);
        let cx = c.inc(W, cx);
        let cx = c.inc(W, cx);
        c.set_reg(CX, cx);
        c.run(0xf47a, 1)?;
        c.rep(0xa4); // rep movsb
        c.run(0xf47c, 3)?; // mov ds, bx; mov si, ax; jmp F435h
        let ds = c.reg(BX);
        c.set_sreg(DS, ds);
        let si = c.reg(AX);
        c.set_reg(SI, si);
    }
}

/// The end (F49E): CX = the bytes written (flags from that `sub`); DI, DS
/// as at the start, the saved CX dropped.
fn finish(c: &mut Ctx) -> Result<(), Bail> {
    c.set_flag(CF, true);
    let end = c.reg(DI);
    c.set_reg(CX, end);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let start = c.pop();
    c.set_reg(DI, start);
    let sp = c.reg(SP);
    let sp = c.add(W, sp, 2);
    c.set_reg(SP, sp);
    let written = c.sub(W, end, start);
    c.set_reg(CX, written);
    c.ret()
}

/// `hsq_decomp_skip_header_dssi_to_esdi` (F403): decompresses the HSQ
/// data at DS:SI (header skipped) to ES:DI. Exit: CX = the size.
pub fn hsq_decomp_skip_header(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf403)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let (cx, di, ds) = (c.reg(CX), c.reg(DI), c.sreg(DS));
    c.push(cx);
    c.push(di);
    c.push(ds);
    let si = c.reg(SI);
    let si = c.add(W, si, 6);
    c.set_reg(SI, si);
    let bp = c.reg(BP);
    let zero = c.xor(W, bp, bp);
    c.set_reg(BP, zero);
    decompress(&mut c)
}

/// `check_hsq_checksum` (F3D3): if the CX bytes at ES:DI are HSQ data (the
/// header's checksum, and its 0), decompresses them in place: moves the
/// compressed data behind where the output ends, then decompresses to
/// ES:DI: CX = the size then. CF set if not HSQ; nothing when HSQ is
/// disabled (flags from the `cmp`).
pub fn check_hsq_checksum(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf3d3)?;
    let mut c = Ctx::new(m);
    let disabled = c.byte(g::DISABLE_HSQ);
    if disabled != 0 {
        c.run(start, 3)?;
        c.cmp(B, disabled as u16, 0);
        return c.ret();
    }
    c.run(start, 12)?;
    c.cmp(B, 0, 0);
    let (cx, di, ds) = (c.reg(CX), c.reg(DI), c.sreg(DS));
    c.push(cx);
    c.push(di);
    c.push(ds);
    let es = c.sreg(ES);
    c.push(es);
    let es = c.pop();
    c.set_sreg(DS, es);
    let end = c.add(W, di, cx);
    c.set_reg(DX, end);
    c.set_reg(CX, 6);
    c.set_reg(SI, di);
    let ax = c.reg(AX);
    let zero = c.xor(W, ax, ax);
    c.set_reg(AX, zero);
    // The header's sum (lodsb; add ah, al; loop) × 6.
    c.run(0xf3ea, 18)?;
    let mut sum = 0u16;
    for i in 0..6 {
        let b = c.lods(B, DS);
        sum = c.add(B, sum, b);
        c.set_reg8(r8::AH, sum as u8);
        c.set_reg(CX, 5 - i);
    }
    let header_zero = c.byte_in(DS, di.wrapping_add(2)) == 0;
    if sum as u8 != CHECKSUM || !header_zero {
        c.run(0xf3ef, if sum as u8 != CHECKSUM { 2 } else { 8 } + 5)?;
        c.cmp(B, sum, CHECKSUM as u16);
        if sum as u8 == CHECKSUM {
            c.set_reg(SI, di);
            let size = c.lods(W, DS);
            c.set_reg(DI, size);
            let zero = c.lods(B, DS);
            c.or(B, zero, zero);
        }
        c.set_flag(CF, true);
        let ds = c.pop();
        c.set_sreg(DS, ds);
        let di = c.pop();
        c.set_reg(DI, di);
        let cx = c.pop();
        c.set_reg(CX, cx);
        return c.ret();
    }
    c.run(0xf3ef, 8)?;
    c.cmp(B, sum, CHECKSUM as u16);
    c.set_reg(SI, di);
    let size = c.lods(W, DS);
    c.set_reg(DI, size);
    let zero = c.lods(B, DS);
    c.or(B, zero, zero);
    decomp_no_header(&mut c)
}

/// From F40D (SI on the header's compressed size, DI = the decompressed
/// size): moves the compressed data and decompresses it.
fn decomp_no_header(c: &mut Ctx) -> Result<(), Bail> {
    let si = c.reg(SI);
    let packed = c.word_in(DS, si);
    let odd = packed.wrapping_sub(6) & 1 != 0;
    c.run(0xf40d, 13 + odd as u32)?;
    c.lods(W, DS);
    c.set_reg(CX, packed);
    let si = c.reg(SI);
    let start = c.sub(W, si, 5);
    c.set_reg(SI, start);
    c.set_reg(BP, start);
    let di = c.reg(DI);
    let di = c.add(W, di, start);
    let di = c.add(W, di, SLACK);
    let si = c.add(W, start, packed);
    let si = c.dec(W, si);
    let di = c.dec(W, di);
    c.set_reg(SI, si);
    c.set_reg(DI, di);
    let words = c.sub(W, packed, 6);
    c.set_flag(pc::cpu::DF, true);
    let words = c.shr1(W, words, 1);
    c.set_reg(CX, words);
    if odd {
        c.movs(B);
    }
    c.run(0xf427, 2)?;
    let (si, di) = (c.reg(SI), c.reg(DI));
    let si = c.dec(W, si);
    let di = c.dec(W, di);
    c.set_reg(SI, si);
    c.set_reg(DI, di);
    c.run(0xf429, 1)?;
    c.rep(0xa5); // rep movsw, backwards
    c.run(0xf42b, 5)?;
    c.set_flag(pc::cpu::DF, false);
    let di = c.reg(DI);
    let si = c.add(W, di, 2);
    c.set_reg(SI, si);
    let out = c.reg(BP);
    c.set_reg(DI, out);
    let zero = c.xor(W, out, out);
    c.set_reg(BP, zero);
    decompress(c)
}

/// `hsq_decomp_no_header` (F40D), as jumped to from `check_hsq_checksum`.
pub fn hsq_decomp_no_header(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf40d)?;
    decomp_no_header(&mut Ctx::new(m))
}
