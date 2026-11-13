//! The program's frame: `start`, the intro (`play_intro`) and the game's
//! main loop (`game_loop`). `start` and the main loop never return: the
//! shadow check can't see them; `--rec on` runs them, checked by whole-run
//! state hashes (`STATE_HASH=1`, against plain emulation).

use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CX, DI, DX, ES, IF, SI, ZF},
    rec::Bail,
};

/// `start` (0000): the command line (`parse_command_line`, AX DD1Dh), the
/// memory and drivers (`loc_0e594`, `loc_000b0`), interrupts on, the
/// intro (`play_intro`), the game's set-up (`loc_00309`, `loc_0021c`,
/// MIDI 2, [3810h] 0, the game time [2] = 2, `loc_00083`), the restart
/// save (`create_save_cl` with CL FFh: DUNE37S0.SAV), `loc_01860`, the
/// pause allowed ([0CE80h] FFh), `loc_0b2be`, then the main loop.
pub fn start(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0000)?;
    let mut c = Ctx::new(m);
    c.run(0x0000, 2)?;
    c.set_reg(AX, 0xdd1d);
    c.call(0xe4ad, 0x0006)?;
    c.run(0x0006, 1)?;
    c.call(0xe594, 0x0009)?;
    c.run(0x0009, 1)?;
    c.call(0x00b0, 0x000c)?;
    c.run(0x000c, 2)?;
    // STI; its one-instruction hold is over: the next instruction is
    // already in the `run` above.
    c.set_flag(IF, true);
    c.call(0x0580, 0x0010)?;
    c.run(0x0010, 1)?;
    c.call(0x0309, 0x0013)?;
    c.run(0x0013, 1)?;
    c.call(0x021c, 0x0016)?;
    c.run(0x0016, 1)?;
    c.call(0xaeb7, 0x0019)?;
    c.run(0x0019, 3)?;
    c.set_byte(0x3810, 0);
    c.set_word(2, 2);
    c.call(0x0083, 0x0027)?;
    c.run(0x0027, 2)?;
    c.set_reg8(r8::CL, 0xff);
    c.call(0xb389, 0x002c)?;
    c.run(0x002c, 1)?;
    c.call(0x1860, 0x002f)?;
    c.run(0x002f, 2)?;
    c.set_byte(0xce80, 0xff);
    c.call(0xb2be, 0x0037)?;
    c.run(0x0037, 1)?;
    c.call(0xd815, 0x003a)?;
    // The main loop does not return; past it is `exit_with_error`.
    c.jump(0x003a)
}

/// `play_intro` (0580): the intro script (`loc_00945` from 337h, each
/// word read by `loc_0093f`): per scene the far function [3939h] with
/// 18h, its video or still (`loc_0de0c`, `loc_00911`, the palette
/// [3959h], `loc_0c097` with the script's BP, `loc_039e6`), a
/// transition when the next word is not negative, the frame copied, then
/// the scene's routine (from the script, CF clear) and a wait or the
/// voice's end (`wait_for_pcm_voice_interruptable`). A key or click
/// (`loc_0de54` at the start, `loc_0de0c`, `loc_0dd63`, the routines'
/// CF) ends it: the music faded (`loc_09985`, the driver's [38D5h]),
/// `loc_00579`, the video closed, the game time [2] = 2, `loc_00911`.
/// CF on return: ended by the player. A script word FFFFh starts over.
pub fn play_intro(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0580)?;
    let mut c = Ctx::new(m);
    'top: loop {
        c.run(0x0580, 1)?;
        c.call(0xde54, 0x0583)?;
        if c.flag(ZF) {
            c.run(0x0583, 1)?;
            return intro_end(&mut c);
        }
        c.run(0x0583, 1)?;
        c.interp(0x0585, 0x0589)?;
        c.run(0x0589, 1)?;
        c.call(0xaeb7, 0x058c)?;
        c.run(0x058c, 2)?;
        c.set_reg(SI, 0x0337);
        c.call(0x0945, 0x0592)?;
        loop {
            c.run(0x0592, 1)?;
            c.set_reg(AX, 0x18);
            c.interp(0x0595, 0x0599)?;
            c.run(0x0599, 1)?;
            c.call(0x093f, 0x059c)?;
            let ax = c.reg(AX);
            c.run(0x059c, 3 + (ax == 0xffff) as u32)?;
            c.set_reg(BX, ax);
            let a1 = c.inc(W, ax);
            c.set_reg(AX, a1);
            if a1 == 0 {
                continue 'top;
            }
            c.run(0x05a3, 1)?;
            c.call(0xde0c, 0x05a6)?;
            if c.flag(CF) {
                c.run(0x05a6, 1)?;
                return intro_end(&mut c);
            }
            c.run(0x05a6, 2)?;
            c.call(0x0911, 0x05ab)?;
            c.interp(0x05ab, 0x05af)?;
            c.run(0x05af, 1)?;
            c.call(0x093f, 0x05b2)?;
            c.run(0x05b2, 2)?;
            let bp = c.reg(AX);
            c.set_reg(BP, bp);
            c.call(0xc097, 0x05b7)?;
            c.run(0x05b7, 2)?;
            let v = c.byte(0x47d1);
            let v = c.and(B, v as u16, 0x7f);
            c.set_byte(0x47d1, v as u8);
            c.call(0x39e6, 0x05bf)?;
            c.run(0x05bf, 1)?;
            c.call(0x093f, 0x05c2)?;
            c.run(0x05c2, 2)?;
            let bx = c.reg(AX);
            c.set_reg(BX, bx);
            c.call(0xde0c, 0x05c7)?;
            if c.flag(CF) {
                c.run(0x05c7, 1)?;
                return intro_end(&mut c);
            }
            c.run(0x05c7, 2)?;
            c.call(0x093f, 0x05cc)?;
            let ax = c.reg(AX);
            let neg = ax & 0x8000 != 0;
            c.run(0x05cc, 2 + if neg { 0 } else { 2 })?;
            c.or(W, ax, ax);
            if !neg {
                c.set_reg(BP, 0x0f66);
                c.call(0xc108, 0x05d6)?;
                c.run(0x05d6, 1)?;
                c.call(0xc0f4, 0x05d9)?;
                c.run(0x05d9, 1)?;
                c.call(0x3a7c, 0x05dc)?;
            }
            c.run(0x05dc, 1)?;
            c.call(0xc07c, 0x05df)?;
            c.run(0x05df, 2)?;
            let v = c.byte(0x47d1);
            let v = c.or(B, v as u16, 0x80);
            c.set_byte(0x47d1, v as u8);
            c.call(0xdd63, 0x05e7)?;
            if c.flag(CF) {
                c.run(0x05e7, 1)?;
                return intro_end(&mut c);
            }
            c.run(0x05e7, 2)?;
            c.call(0x093f, 0x05ec)?;
            c.run(0x05ec, 2)?;
            c.set_flag(CF, false);
            let f = c.reg(AX);
            c.call(f, 0x05ef)?;
            if c.flag(CF) {
                c.run(0x05ef, 1)?;
                return intro_end(&mut c);
            }
            c.run(0x05ef, 2)?;
            c.call(0x093f, 0x05f4)?;
            let ax = c.reg(AX);
            c.run(0x05f4, 2)?;
            c.or(W, ax, ax);
            if ax == 0 {
                continue;
            }
            c.run(0x05f8, 1)?;
            c.call(0xddf0, 0x05fb)?;
            c.run(0x05fb, 1)?;
            if !c.flag(CF) {
                continue;
            }
            return intro_end(&mut c);
        }
    }
}

/// 05FDh-061Bh of `play_intro` (the jump there counted by the caller).
fn intro_end(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x05fd, 2)?;
    let f = c.m.cpu.flags();
    c.push(f);
    c.call(0x9985, 0x0601)?;
    c.run(0x0601, 1)?;
    let es = c.word(0xdbd8);
    c.set_sreg(ES, es);
    c.interp(0x0605, 0x0609)?;
    c.run(0x0609, 3)?;
    let f = c.pop();
    c.m.cpu.set_flags(f);
    c.push(f);
    c.call(0x0579, 0x060e)?;
    c.run(0x060e, 1)?;
    c.call(0xca01, 0x0611)?;
    c.run(0x0611, 2)?;
    c.set_word(2, 2);
    c.call(0x0911, 0x061a)?;
    c.run(0x061a, 2)?;
    let f = c.pop();
    c.m.cpu.set_flags(f);
    c.ret()
}

/// `game_loop` (D815): the time [0DC68h] it started, [0DC4Bh] 0, then
/// [`main_loop_top`].
pub fn game_loop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd815)?;
    let mut c = Ctx::new(m);
    c.run(0xd815, 3)?;
    let t = c.word(0xce7a);
    c.set_reg(AX, t);
    c.set_word(0xdc68, t);
    c.set_byte(0xdc4b, 0);
    main_loop(&mut c)
}

/// The main loop's top (D820), a block leader the interpreter resumes the
/// loop at: each pass of the loop.
pub fn main_loop_top(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xd820)?;
    main_loop(&mut Ctx::new(m))
}

fn main_loop(c: &mut Ctx) -> Result<(), Bail> {
    loop {
        main_pass(c)?;
    }
}

/// One pass of the main loop from D820h to its `jmp` back (counted): the
/// Ctrl-V key (`loc_0b270`, [0CEE8h] 2Fh with [0CE9Eh] FFh); unless a
/// video plays ([46D9h]), `loc_0d7b7` and `loc_01b0d`; `loc_0d9d2`; with
/// the video, `loc_00d8e`; [0] a random number (`rand`), `loc_04f0c`; the
/// input (`loc_0d962` while [0DC4Bh], else `loc_0df1e` and
/// `loc_0db4c`); the mouse (`loc_0dc20`: AX buttons, DX/BX the place,
/// its moves DI/CX against [0DC62h]/[0DC64h]); then the dispatch through
/// the handler table at [2570h] by the buttons ([`dispatch`]).
fn main_pass(c: &mut Ctx) -> Result<(), Bail> {
    let k = c.byte(0xcee8);
    let ctrl_v = k == 0x2f && c.byte(0xce9e) == 0xff;
    c.run(0xd820, 2 + if k == 0x2f { 2 + ctrl_v as u32 } else { 0 })?;
    c.cmp(B, k as u16, 0x2f);
    if k == 0x2f {
        let v = c.byte(0xce9e);
        c.cmp(B, v as u16, 0xff);
        if ctrl_v {
            c.call(0xb270, 0xd831)?;
        }
    }
    let video = c.byte(0x46d9);
    c.run(0xd831, 2 + if video == 0 { 1 } else { 0 })?;
    c.cmp(B, video as u16, 0);
    if video == 0 {
        c.call(0xd7b7, 0xd83b)?;
        c.run(0xd83b, 1)?;
        c.call(0x1b0d, 0xd83e)?;
    }
    c.run(0xd83e, 1)?;
    c.call(0xd9d2, 0xd841)?;
    let video = c.byte(0x46d9);
    c.run(0xd841, 2 + (video != 0) as u32)?;
    c.cmp(B, video as u16, 0);
    if video != 0 {
        c.call(0x0d8e, 0xd84b)?;
    }
    c.run(0xd84b, 1)?;
    c.call(0xe3cc, 0xd84e)?;
    c.run(0xd84e, 2)?;
    let r = c.reg(AX);
    c.set_word(0, r);
    c.call(0x4f0c, 0xd854)?;
    let busy = c.byte(0xdc4b);
    c.run(0xd854, 3)?;
    c.cmp(B, busy as u16, 0);
    if busy != 0 {
        c.call(0xd962, 0xd85e)?;
        c.run(0xd85e, 1)?;
    } else {
        c.call(0xdf1e, 0xd863)?;
        c.run(0xd863, 1)?;
        c.call(0xdb4c, 0xd866)?;
    }
    c.run(0xd866, 1)?;
    c.call(0xdc20, 0xd869)?;
    let (ax, bx, dx) = (c.reg(AX), c.reg(BX), c.reg(DX));
    let buttons = ax & 0x0f;
    c.run(0xd869, 11)?;
    let old_x = c.word(0xdc62);
    c.set_word(0xdc62, dx);
    let old_y = c.word(0xdc64);
    c.set_word(0xdc64, bx);
    let di = c.sub(W, old_x, dx);
    let cx = c.sub(W, old_y, bx);
    let di = c.neg(W, di);
    let cx = c.neg(W, cx);
    c.set_reg(DI, di);
    c.set_reg(CX, cx);
    let si = c.word(0x2570);
    c.set_reg(SI, si);
    let ax = c.and(W, ax, 0x0f);
    c.set_reg(AX, ax);
    if buttons == 0 {
        // No button: the hover handler [SI].
        c.run(0xd886, 1)?;
        c.call(0xd50f, 0xd889)?;
        c.run(0xd889, 4)?;
        let (cx, di, si) = (c.reg(CX), c.reg(DI), c.reg(SI));
        c.set_reg(AX, cx);
        c.or(W, cx, di);
        c.set_reg(AX, cx | di);
        let h = c.word(si);
        c.call(h, 0xd891)?;
        c.run(0xd891, 1)?;
        return Ok(());
    }
    dispatch(c, ax, si, di, cx)
}

/// D893h on: a button is down (AX bits 0-3). [0DC5Ah] the time. With
/// the keyboard cursor ([4774h]) both buttons give `loc_01707`; else the
/// left button (bit 0 or 2: SI + 0, else SI + 2 with AX halved) or the
/// right one: the element under the cursor dispatched through the
/// handler table SI (+2, +6, +0Ah) or the element's own (+0Ch).
fn dispatch(c: &mut Ctx, ax: u16, si: u16, di: u16, cx: u16) -> Result<(), Bail> {
    let kb = c.byte(0x4774);
    c.run(0xd893, 4)?;
    let t = c.word(0xce7a);
    c.set_reg(BP, t);
    c.set_word(0xdc5a, t);
    c.cmp(B, kb as u16, 0);
    if kb != 0 {
        let both = (ax & 5) == 5;
        c.run(0xd8a2, 3)?;
        let al = c.and(B, ax & 0xff, 5);
        c.set_reg8(r8::AL, al as u8);
        c.cmp(B, al, 5);
        if both {
            c.run(0xd8a8, 1)?;
            c.call(0xdbb2, 0xd8ab)?;
            c.run(0xd8ab, 1)?;
            c.call(0x1707, 0xd8ae)?;
            c.run(0xd8ae, 1)?;
        } else {
            c.run(0xd8d7, 1)?;
        }
        return Ok(());
    }
    let left = ax & 5 != 0;
    c.run(0xd8b1, 2 + if left { 0 } else { 2 })?;
    c.test(B, ax & 0xff, 5);
    let (mut si, mut ax) = (si, ax);
    if !left {
        si = c.add(W, si, 2);
        c.set_reg(SI, si);
        ax = c.shr1(W, ax, 1);
        c.set_reg(AX, ax);
    }
    c.run(0xd8ba, 3)?;
    let al = c.and(B, ax & 0xff, 5);
    let al = c.dec(B, al);
    c.set_reg8(r8::AL, al as u8);
    if al == 0 {
        // A plain press.
        let held = c.word(0xdc5c);
        c.run(0xd8c0, 3)?;
        c.set_reg(BP, held);
        c.or(W, held, held);
        if held == 0 {
            let moved = (cx | di) != 0;
            c.run(0xd8c8, 3 + moved as u32)?;
            c.set_reg(AX, cx);
            c.or(W, cx, di);
            c.set_reg(AX, cx | di);
            if moved {
                c.call(0xdbb2, 0xd8d1)?;
                c.run(0xd8d1, 2)?;
                let a = c.byte(0xdc35);
                c.set_reg8(r8::AL, a);
                let si = c.reg(SI);
                let h = c.word(si.wrapping_add(0x0a));
                c.call(h, 0xd8d7)?;
            }
            c.run(0xd8d7, 1)?;
            return Ok(());
        }
        let t = c.word(0xce7a);
        let t0 = c.word(0xdc5e);
        c.run(0xd8da, 5)?;
        c.set_reg(AX, t);
        let d = c.sub(W, t, t0);
        c.set_reg(AX, d);
        c.cmp(W, d, 0x32);
        if d < 0x32 {
            return Ok(());
        }
        c.call(0xd6b7, 0xd8e9)?;
        if !c.flag(CF) {
            c.run(0xd8e9, 2)?;
            return Ok(());
        }
        let (di, held) = (c.reg(DI), c.reg(BP));
        c.run(0xd8e9, 4)?;
        c.cmp(W, di, held);
        if di != held {
            return Ok(());
        }
        c.call(0xdbb2, 0xd8f2)?;
        c.run(0xd8f2, 1)?;
        return release_at_d92b(c);
    }
    c.run(0xd8f4, 1)?;
    c.call(0xdbb2, 0xd8f7)?;
    c.run(0xd8f7, 1)?;
    c.call(0xe26f, 0xd8fa)?;
    let al = c.reg(AX) & 0xff;
    c.run(0xd8fa, 2)?;
    let al = c.sub(B, al, 3);
    c.set_reg8(r8::AL, al as u8);
    if al == 0 {
        c.run(0xd944, 4)?;
        let di = c.word(0xdc5c);
        c.set_reg(DI, di);
        c.set_word(0xdc5c, 0);
        c.or(W, di, di);
        if di != 0 {
            return release_at_d92b(c);
        }
        c.run(0xd952, 2)?;
        let a = c.byte(0xdc35);
        c.set_reg8(r8::AL, a);
        let si = c.reg(SI);
        let h = c.word(si.wrapping_add(6));
        c.call(h, 0xd958)?;
        c.run(0xd958, 1)?;
        return Ok(());
    }
    let top = c.word(0x2570);
    let si = c.reg(SI);
    c.run(0xd8fe, 2)?;
    c.cmp(W, si, top);
    if si == top {
        c.run(0xd904, 1)?;
        c.call(0xd6b7, 0xd907)?;
        if c.flag(CF) {
            c.run(0xd907, 1)?;
            let di = c.reg(DI);
            c.run(0xd918, 2)?;
            c.set_word(0xdc60, di);
            c.call(0xd65a, 0xd91f)?;
            let di = c.reg(DI);
            let b = c.byte(di.wrapping_add(9));
            c.run(0xd91f, 2)?;
            c.test(B, b as u16, 0x40);
            if b & 0x40 == 0 {
                return release_at_d92b(c);
            }
            c.run(0xd925, 2)?;
            c.set_word(0xdc5c, di);
            return press_at_d935(c);
        }
        c.run(0xd907, 3)?;
        let si = c.reg(SI);
        c.push(si);
        c.call(0x9215, 0xd90d)?;
        c.run(0xd90d, 1)?;
        let si = c.pop();
        c.set_reg(SI, si);
    }
    let si = c.reg(SI);
    c.run(0xd90e, 2)?;
    let a = c.byte(0xdc35);
    c.set_reg8(r8::AL, a);
    let h = c.word(si.wrapping_add(2));
    c.call(h, 0xd914)?;
    c.run(0xd914, 1)?;
    Ok(())
}

/// D92Bh: [0CE9Dh] and [0CEBAh] 0, then [`press_at_d935`].
fn release_at_d92b(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xd92b, 2)?;
    c.set_byte(0xce9d, 0);
    c.set_byte(0xceba, 0);
    press_at_d935(c)
}

/// D935h: [0DC5Eh] the time, the element DI's handler (+0Ch) with AL
/// [0DC35h], then back to the top.
fn press_at_d935(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xd935, 4)?;
    let t = c.word(0xce7a);
    c.set_reg(AX, t);
    c.set_word(0xdc5e, t);
    let a = c.byte(0xdc35);
    c.set_reg8(r8::AL, a);
    let di = c.reg(DI);
    let h = c.word(di.wrapping_add(0x0c));
    c.call(h, 0xd941)?;
    c.run(0xd941, 1)?;
    Ok(())
}

/// Where `parse_command_line` goes on.
#[derive(Clone, Copy)]
enum Parse {
    /// E4B7h: the next word.
    Word,
    /// E542h: the rest of this word skipped.
    Skip,
    /// E4E6h: the option at CS:DI matched.
    Option,
    /// E517h: after a character of a hex number (BX where it goes).
    Number,
    /// E552h: after a character of a path (stored at ES:DI).
    Path,
}

/// `parse_command_line` (E4AD): the DOS command tail (PSP:80h) read word
/// by word (`loc_0e56b` gives the next character: CF at the end, ZF at a
/// blank). A word's first two letters looked up in the option table at
/// CS:E40Ch (17h entries of 7 bytes: two letters, a third to match, the
/// byte offset from 2942h, the bits, then where its value goes): the bits
/// set in DS:[2942h + n], then hex numbers (each to the next word, up to
/// a character not above a blank) or, for 3826h, a path (up to 3898h,
/// with a trailing backslash; its end in [38A6h]).
pub fn parse_command_line(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{CS, DS};
    entered(start, 0xe4ad)?;
    let mut c = Ctx::new(m);
    c.run(0xe4ad, 5)?;
    c.set_reg(SI, 0x80);
    let n = c.lods(B, DS);
    let ah = c.reg(AX) >> 8;
    c.xor(B, ah, ah);
    c.set_reg(AX, n);
    let si = c.reg(SI);
    let bp = c.add(W, n, si);
    c.set_reg(BP, bp);
    let mut at = Parse::Word;
    loop {
        at = match at {
            Parse::Word => {
                c.run(0xe4b7, 3)?;
                let cs = c.sreg(CS);
                c.push(cs);
                let es = c.pop();
                c.set_sreg(ES, es);
                c.call(0xe56b, 0xe4bc)?;
                // Blanks before the word.
                loop {
                    if c.flag(CF) {
                        c.run(0xe4bc, 2)?;
                        return c.ret();
                    }
                    if !c.flag(ZF) {
                        break;
                    }
                    c.run(0xe4bc, 3)?;
                    c.call(0xe56b, 0xe4bc)?;
                }
                c.run(0xe4bc, 4)?;
                let al = c.reg8(r8::AL);
                c.set_reg8(r8::DL, al);
                c.call(0xe56b, 0xe4c5)?;
                if c.flag(CF) || c.flag(ZF) {
                    c.run(0xe4c5, 1)?;
                    Parse::Skip
                } else {
                    c.run(0xe4c5, 3)?;
                    let al = c.reg8(r8::AL);
                    c.set_reg8(r8::AH, al);
                    c.call(0xe56b, 0xe4cc)?;
                    if c.flag(CF) || c.flag(ZF) {
                        c.run(0xe4cc, 1)?;
                        Parse::Skip
                    } else {
                        look_up_option(&mut c)?
                    }
                }
            }
            Parse::Option => {
                let di = c.reg(DI);
                let to = c.word_in(CS, di.wrapping_add(3));
                c.run(0xe4e6, 10 + (to != 0) as u32)?;
                c.set_reg(AX, GAME_DS);
                c.set_sreg(ES, GAME_DS);
                let bl = c.byte_in(CS, di.wrapping_add(1));
                c.set_reg(BX, bl as u16);
                c.xor(B, 0, 0);
                let bx = c.add(W, bl as u16, 0x2942);
                c.set_reg(BX, bx);
                let bits = c.byte_in(CS, di.wrapping_add(2));
                c.set_reg8(r8::AL, bits);
                let v = c.byte_in(ES, bx);
                let v = c.or(B, v as u16, bits as u16);
                c.m.cpu.write8(&mut c.m.hw, ES, bx, v as u8);
                c.set_reg(BX, to);
                c.or(W, to, to);
                if to == 0 {
                    Parse::Skip
                } else {
                    c.call(0xe56b, 0xe507)?;
                    if c.flag(CF) {
                        c.run(0xe507, 2)?;
                        return c.ret();
                    }
                    if c.flag(ZF) {
                        c.run(0xe507, 2)?;
                        Parse::Skip
                    } else {
                        let path = to == 0x3826;
                        c.run(0xe507, 7)?;
                        let si = c.reg(SI);
                        let si = c.dec(W, si);
                        c.set_reg(SI, si);
                        c.cmp(W, to, 0x3826);
                        if path {
                            c.set_reg(DI, to);
                            c.call(0xe56b, 0xe552)?;
                            Parse::Path
                        } else {
                            c.xor(W, 0, 0);
                            c.set_reg(DX, 0);
                            c.call(0xe56b, 0xe517)?;
                            Parse::Number
                        }
                    }
                }
            }
            Parse::Number => {
                let al = c.reg8(r8::AL) as u16;
                let end = c.flag(CF) || c.flag(ZF);
                // The listing's tests: blank or end; below '0'; '0'-'9';
                // below 'A'; above 'F'.
                let (n, digit) = if end {
                    (2, None)
                } else if al < 0x30 {
                    (4, None)
                } else if al - 0x30 <= 9 {
                    (6, Some(al - 0x30))
                } else if al - 0x30 < 7 {
                    (8, None)
                } else if al - 0x37 > 0x0f {
                    (10, None)
                } else {
                    (10, Some(al - 0x37))
                };
                c.run(0xe517, n + if digit.is_some() { 7 } else { 0 })?;
                c.set_reg8(r8::AH, al as u8);
                if !end {
                    let mut v = c.sub(B, al, 0x30);
                    if al >= 0x30 {
                        c.cmp(B, v, 9);
                        if v > 9 {
                            v = c.sub(B, v, 7);
                            if al - 0x30 >= 7 {
                                c.cmp(B, v, 0x0f);
                            }
                        }
                    }
                    c.set_reg8(r8::AL, v as u8);
                }
                match digit {
                    Some(d) => {
                        let dx = c.reg(DX);
                        let dx = c.shl1(W, dx, 4);
                        let dl = c.or(B, dx & 0xff, d);
                        c.set_reg(DX, (dx & 0xff00) | dl);
                        c.call(0xe56b, 0xe517)?;
                        Parse::Number
                    }
                    None => {
                        let more = al > 0x20;
                        c.run(0xe537, 4 + if more { 2 } else { 0 })?;
                        let (bx, dx) = (c.reg(BX), c.reg(DX));
                        c.m.cpu.write16(&mut c.m.hw, ES, bx, dx);
                        let bx = c.add(W, bx, 2);
                        c.set_reg(BX, bx);
                        c.cmp(B, al, 0x20);
                        if more {
                            c.xor(W, 0, 0);
                            c.set_reg(DX, 0);
                            c.call(0xe56b, 0xe517)?;
                            Parse::Number
                        } else {
                            Parse::Skip
                        }
                    }
                }
            }
            Parse::Path => {
                if c.flag(CF) || c.flag(ZF) {
                    c.run(0xe552, 1)?;
                } else {
                    let di = c.reg(DI).wrapping_add(c.string_step(B));
                    let more = di < 0x3898;
                    c.run(0xe552, 4 + more as u32)?;
                    c.stos(B);
                    let di = c.reg(DI);
                    c.cmp(W, di, 0x3898);
                    if more {
                        c.call(0xe56b, 0xe552)?;
                        continue;
                    }
                }
                let di = c.reg(DI);
                let prev = c.byte_in(ES, di.wrapping_sub(1));
                c.run(0xe55b, 3 + (prev != 0x5c) as u32 + 2)?;
                c.set_reg8(r8::AL, 0x5c);
                c.cmp(B, prev as u16, 0x5c);
                if prev != 0x5c {
                    c.stos(B);
                }
                let di = c.reg(DI);
                c.m.cpu.write16(&mut c.m.hw, ES, 0x38a6, di);
                Parse::Skip
            }
            Parse::Skip => {
                c.run(0xe542, 2)?;
                let si = c.reg(SI);
                let si = c.dec(W, si);
                c.set_reg(SI, si);
                c.call(0xe56b, 0xe546)?;
                loop {
                    if c.flag(CF) {
                        c.run(0xe546, 2)?;
                        return c.ret();
                    }
                    c.run(0xe546, 3)?;
                    if c.flag(ZF) {
                        break;
                    }
                    c.call(0xe56b, 0xe546)?;
                }
                Parse::Word
            }
        };
    }
}

/// E4CEh-E4E3h of `parse_command_line`: AL and DL swapped (AX the word's
/// first two letters, DL its third), the option table at CS:E40Ch
/// searched (`scasw`, then the third letter).
fn look_up_option(c: &mut Ctx) -> Result<Parse, Bail> {
    use pc::cpu::CS;
    let (al, dl) = (c.reg8(r8::AL), c.reg8(r8::DL));
    let ax = (c.reg(AX) & 0xff00) | dl as u16;
    let step = c.string_step(W);
    // A first pass finds the entry and counts; the second does it.
    let mut di = 0xe40cu16;
    let mut n = 4u32;
    let mut found = false;
    for _ in 0..0x17 {
        let w = c.word_in(CS, di);
        di = di.wrapping_add(step);
        n += 2;
        if w == ax {
            n += 2;
            if c.byte_in(CS, di) == al {
                found = true;
                break;
            }
        }
        n += 2;
        di = di.wrapping_add(5);
    }
    if !found {
        n += 1;
    }
    c.run(0xe4ce, n)?;
    c.set_reg8(r8::AL, dl);
    c.set_reg8(r8::DL, al);
    c.set_reg(CX, 0x17);
    let mut di = 0xe40cu16;
    let mut cx = 0x17u16;
    loop {
        let w = c.word_in(CS, di);
        c.cmp(W, ax, w);
        di = di.wrapping_add(step);
        if w == ax {
            let b = c.byte_in(CS, di);
            c.cmp(B, al as u16, b as u16);
            if b == al {
                c.set_reg(DI, di);
                c.set_reg(CX, cx);
                return Ok(Parse::Option);
            }
        }
        di = c.add(W, di, 5);
        cx -= 1;
        if cx == 0 {
            break;
        }
    }
    c.set_reg(DI, di);
    c.set_reg(CX, 0);
    Ok(Parse::Skip)
}

/// The game's data segment.
const GAME_DS: u16 = 0x1f4b;
