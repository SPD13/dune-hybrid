//! Dialogue state: the talking character's mode bytes (47C2h, 47C4h,
//! 47DCh...), the phrase pointers, and small steps of the conversation.

use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CX, DI, DS, DX, ES, SI, SP, SS, ZF},
    rec::Bail,
};

mod code {
    pub const ADD_FRAME_TASK: u16 = 0xda25;
    pub const DIALOGUE_STEP: u16 = 0x9f9e;
}

const MODE: u16 = 0x47c2;
const SPEED: u16 = 0x47c4;
const TALKING: u16 = 0x47dc;
/// The text being shown (cleared when done).
const TEXT_PTR: u16 = 0x1bea;

/// `loc_096b5`: [`code::DIALOGUE_STEP`] on the phrase at [AB84h], with
/// the mode 80h and speed 10h for the while.
pub fn step_phrase_ab84(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x96b5)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let (speed, mode) = (c.word(SPEED), c.word(MODE));
    c.push(speed);
    c.push(mode);
    c.set_word(SPEED, 0x10);
    c.set_byte(MODE, 0x80);
    let si = c.word(0xab84);
    c.set_reg(SI, si);
    c.call(code::DIALOGUE_STEP, 0x96cf)?;
    c.run(0x96cf, 3)?;
    let mode = c.pop();
    c.set_word(MODE, mode);
    let speed = c.pop();
    c.set_word(SPEED, speed);
    c.ret()
}

/// `loc_096d8`: speed AX; `loc_09702` with AX = 10h while [`TALKING`] is
/// one up; then the text pointer and [`TALKING`] cleared.
pub fn talk_at_speed(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x96d8)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let ax = c.reg(AX);
    c.set_word(SPEED, ax);
    let t = c.byte(TALKING);
    let t = c.inc(B, t as u16);
    c.set_byte(TALKING, t as u8);
    c.set_reg(AX, 0x10);
    c.call(0x9702, 0x96e5)?;
    c.run(0x96e5, 3)?;
    c.set_word(TEXT_PTR, 0);
    c.set_byte(TALKING, 0);
    c.ret()
}

/// `loc_0970b`: phrase AX (the word table at AA76h), `loc_09f40`, then
/// [`code::DIALOGUE_STEP`] with the mode 20h for the while.
pub fn say_phrase(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x970b)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let ax = c.reg(AX);
    let i = c.shl1(W, ax, 1);
    let si = c.word(i.wrapping_sub(0x558a));
    c.set_reg(SI, si);
    c.call(0x9f40, 0x9716)?;
    c.run(0x9716, 4)?;
    let mode = c.word(MODE);
    c.push(mode);
    c.set_byte(MODE, 0x20);
    c.call(code::DIALOGUE_STEP, 0x9f97)?;
    c.run(0x9f97, 2)?;
    let mode = c.pop();
    c.set_word(MODE, mode);
    c.ret()
}

/// `loc_098f5`: [1C06h], [1BF8h] and the text pointer cleared.
pub fn clear_text_state(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x98f5)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    c.set_reg(AX, 0);
    for at in [0x1c06, 0x1bf8, TEXT_PTR] {
        c.set_word(at, 0);
    }
    c.ret()
}

/// `loc_09945`: the frame task `loc_099be` every 10h ticks.
pub fn add_task_99be(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9945)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_reg(SI, 0x99be);
    c.set_reg(BP, 0x10);
    c.call(code::ADD_FRAME_TASK, 0x994e)?;
    c.run(0x994e, 1)?;
    c.ret()
}

/// `loc_09985`: `loc_099be` until the low 3 bits of [47CEh] are 0.
pub fn finish_99be(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9985)?;
    let mut c = Ctx::new(m);
    loop {
        let w = c.word(0x47ce);
        c.run(0x9985, 3)?;
        c.test(W, w, 7);
        if w & 7 == 0 {
            return c.ret();
        }
        c.call(0x99be, 0x9985)?;
    }
}

/// `loc_09ef1`: `loc_09efd` with [`TALKING`] = AL for the while.
pub fn with_talking_9efd(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9ef1)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let al = c.reg8(r8::AL);
    c.set_byte(TALKING, al);
    c.call(0x9efd, 0x9ef7)?;
    c.run(0x9ef7, 2)?;
    c.set_byte(TALKING, 0);
    c.ret()
}

/// `loc_09f1c`: `loc_09197`; bit 10h of [47D1h]; `loc_09a7b`; then
/// `loc_09a60` with AH = 0, its SI noted at [47C6h].
pub fn start_9f1c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9f1c)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x9197, 0x9f1f)?;
    c.run(0x9f1f, 2)?;
    let v = c.byte(0x47d1);
    let v = c.or(B, v as u16, 0x10);
    c.set_byte(0x47d1, v as u8);
    c.call(0x9a7b, 0x9f27)?;
    c.run(0x9f27, 2)?;
    let ah = c.reg8(r8::AH) as u16;
    c.xor(B, ah, ah);
    c.set_reg8(r8::AH, 0);
    c.call(0x9a60, 0x9f2c)?;
    c.run(0x9f2c, 2)?;
    let si = c.reg(SI);
    c.set_word(0x47c6, si);
    c.ret()
}

/// `loc_09f82`: the text colour F0h and the tall font.
pub fn tall_font_colour_f0(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9f82)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_word(super::globals::TEXT_COLOUR, 0xf0);
    super::video::font_select_tall_font(c.m, 0xd068)
}

/// `loc_09556`: the record at SI loses bit 40h of byte 0Fh; its time 2
/// stamped (`loc_0956d` with BX = 2); its bit (byte 0Eh) cleared at [10h].
pub fn release_record(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9556)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let si = c.reg(SI);
    let f = c.byte(si.wrapping_add(0x0f));
    let f = c.and(B, f as u16, 0xbf);
    c.set_byte(si.wrapping_add(0x0f), f as u8);
    c.set_reg(BX, 2);
    c.call(0x956d, 0x9560)?;
    c.run(0x9560, 5)?;
    let si = c.reg(SI);
    let cl = c.byte(si.wrapping_add(0x0e));
    c.set_reg8(r8::CL, cl);
    let mask = c.m.cpu.shift(0, W, 0xfffe, (cl & 0x1f) as u32) as u16; // rol ax, cl
    c.set_reg(AX, mask);
    let v = c.word(0x10);
    let v = c.and(W, v, mask);
    c.set_word(0x10, v);
    c.ret()
}

/// `loc_0956d`: the record at SI's time BX (word 8 + BX) to the game time,
/// unless its other time (BX xor 2) is less than 2 ago.
pub fn stamp_record_time(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x956d)?;
    let mut c = Ctx::new(m);
    let (si, bx) = (c.reg(SI), c.reg(BX));
    let bp = bx ^ 2;
    let now = c.word(super::globals::GAME_TIME);
    let other = c.word_in(SS, bp.wrapping_add(si).wrapping_add(8));
    let ago = now.wrapping_sub(other);
    c.run(start, if ago < 2 { 7 } else { 9 })?;
    let bp = c.xor(W, bx, 2);
    c.set_reg(BP, bp);
    let ago = c.sub(W, now, other);
    c.set_reg(AX, ago);
    c.cmp(W, ago, 2);
    if ago >= 2 {
        c.set_reg(AX, now);
        c.set_word(bx.wrapping_add(si).wrapping_add(8), now);
    }
    c.ret()
}

/// The dialogue table: 4-byte entries from AA78h (SS), byte 2 its kinds.
const DIALOGUES: u16 = 0xaa78;

/// From B150: DI = the last of CX entries of the code-segment word list at
/// SI (from SI + DX, then every 2; an entry's low 11 bits index
/// [`DIALOGUES`], 0 ends) whose kind bits BL are 0 or BH; ZF from
/// `or di, di`. Counts the instructions (`apply` false) or does them.
fn scan_dialogues(c: &mut Ctx, apply: bool) -> u32 {
    let (mut si, mut dx, mut cx) = (c.reg(SI), c.reg(DX), c.reg(CX));
    let bx = c.reg(BX);
    let (bl, bh) = (bx as u8, (bx >> 8) as u8);
    let mut di = 0u16;
    let mut n = 1;
    if apply {
        c.xor(W, 0, 0);
        c.set_reg(DI, 0);
    }
    loop {
        si = if apply { c.add(W, si, dx) } else { si.wrapping_add(dx) };
        dx = if apply { c.or(W, dx, 2) } else { dx | 2 };
        let w = c.code_word(si);
        let bp = if apply { c.and(W, w, 0x7ff) } else { w & 0x7ff };
        if apply {
            c.set_reg(BP, bp);
        }
        n += 5;
        if bp == 0 {
            break;
        }
        let bp = if apply {
            let b = c.shl1(W, bp, 2);
            c.add(W, b, DIALOGUES)
        } else {
            (bp << 2).wrapping_add(DIALOGUES)
        };
        let kinds = c.byte_in(SS, bp.wrapping_add(2));
        let al = if apply { c.and(B, kinds as u16, bl as u16) as u8 } else { kinds & bl };
        n += 6;
        if apply {
            c.set_reg(BP, bp);
            c.set_reg8(r8::AL, al);
        }
        let take = if al == 0 {
            true
        } else {
            if apply {
                c.cmp(B, al as u16, bh as u16);
            }
            n += 2;
            al == bh
        };
        if take {
            di = si;
            cx = cx.wrapping_sub(1);
            n += 2;
            if apply {
                c.set_reg(DI, di);
                c.set_reg(CX, cx);
            }
            if cx == 0 {
                break;
            }
        }
    }
    if apply {
        c.set_reg(SI, si);
        c.set_reg(DX, dx);
        c.or(W, di, di);
    }
    n + 2
}

/// `loc_0b150`: see [`scan_dialogues`].
pub fn scan_dialogues_from(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb150)?;
    let mut c = Ctx::new(m);
    let n = scan_dialogues(&mut c, false);
    c.run(start, n)?;
    scan_dialogues(&mut c, true);
    c.ret()
}

/// `loc_0b147`: [`scan_dialogues`] of the list at A8h (from AAh), for
/// the first match.
pub fn scan_dialogues_a8(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb147)?;
    let mut c = Ctx::new(m);
    let (si, dx, cx) = (c.reg(SI), c.reg(DX), c.reg(CX));
    c.set_reg(SI, 0xa8);
    c.set_reg(DX, 2);
    c.set_reg(CX, 1);
    let n = scan_dialogues(&mut c, false);
    c.set_reg(SI, si);
    c.set_reg(DX, dx);
    c.set_reg(CX, cx);
    c.run(start, 3 + n)?;
    c.set_reg(SI, 0xa8);
    c.set_reg(DX, 2);
    c.set_reg(CX, 1);
    scan_dialogues(&mut c, true);
    c.ret()
}

/// `loc_0af00`: bit 4000h (off) of the UI elements at 2034h + 4, 8, 0Ch
/// set when there is no dialogue of kind 4, 8, 0Ch (bits 1Ch) to have.
pub fn mark_dialogue_buttons(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xaf00)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_reg8(r8::BH, 4);
    c.set_reg8(r8::BL, 0x1c);
    c.call(0xb147, 0xaf07)?;
    loop {
        let zf = c.flag(ZF);
        let bh = c.reg8(r8::BH);
        let next = bh.wrapping_add(4);
        let again = next <= 0x0c;
        c.run(0xaf07, 2 + if zf { 0 } else { 1 } + 9 + if again { 2 } else { 1 })?;
        let mut ax = 0x4000u16;
        if !zf {
            let ah = c.xor(B, 0x40, 0x40);
            ax = ah << 8;
        }
        c.set_reg(AX, ax);
        let cx = c.reg(CX);
        c.xor(W, cx, cx);
        c.set_reg(CX, bh as u16);
        let si = c.add(W, 0x2034, bh as u16);
        c.set_reg(SI, si);
        let w = c.word(si);
        let w = c.and(W, w, 0xbfff);
        let w = c.or(W, w, ax);
        c.set_word(si, w);
        let bh = c.add(B, bh as u16, 4) as u8;
        c.set_reg8(r8::BH, bh);
        c.cmp(B, bh as u16, 0x0c);
        if !again {
            return c.ret();
        }
        c.set_reg8(r8::BL, 0x1c);
        c.call(0xb147, 0xaf07)?;
    }
}

/// `loc_0b126`: phrase AX (`get_phrase_or_command_string_si`) appended at
/// [47BCh] (`loc_088f1`; with `loc_08944` and a space after unless it
/// starts with one). SI kept.
pub fn append_phrase(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb126)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let si = c.reg(SI);
    c.push(si);
    let ax = c.reg(AX);
    c.set_reg(SI, ax);
    c.call(0xcf70, 0xb12c)?;
    c.run(0xb12c, 3)?;
    let di = c.word(0x47bc);
    c.set_reg(DI, di);
    c.push(di);
    c.call(0x88f1, 0xb134)?;
    let sp = c.reg(SP);
    let di = c.word_in(SS, sp);
    let si = c.reg(SI);
    let first = c.byte(si);
    let space = first == 0x20;
    c.run(0xb134, if space { 3 } else { 4 })?;
    c.pop();
    c.set_reg(DI, di);
    c.cmp(B, first as u16, 0x20);
    if !space {
        c.call(0x8944, 0xb13d)?;
        c.run(0xb13d, 3)?;
        let di = c.reg(DI);
        let di = c.dec(W, di);
        c.set_reg(DI, di);
        c.set_reg8(r8::AL, 0x20);
        c.stos(B);
    }
    c.run(0xb141, 3)?;
    let di = c.reg(DI);
    c.set_word(0x47bc, di);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_09702`: the sentence AX·8 + 4 of the table at -558Ah (word ×2):
/// `loc_09f40` with it, then `dialogue_step` with [47C2h] 20h for the
/// time.
pub fn say_sentence(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9702)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    let ax = c.reg(AX);
    let ax = c.shl1(W, ax, 3);
    let ax = c.or(W, ax, 4);
    c.set_reg(AX, ax);
    let si = c.shl1(W, ax, 1);
    let si = c.word(si.wrapping_sub(0x558a));
    c.set_reg(SI, si);
    c.call(0x9f40, 0x9716)?;
    c.run(0x9716, 4)?;
    let keep = c.word(0x47c2);
    c.push(keep);
    c.set_byte(0x47c2, 0x20);
    c.call(code::DIALOGUE_STEP, 0x9f97)?;
    c.run(0x9f97, 2)?;
    let v = c.pop();
    c.set_word(0x47c2, v);
    c.ret()
}

/// `loc_0994f`: BP = the offset for [47D0h]: 0 gives a random one of 6
/// (`loc_0e3b7`) + [0F0h], else ([47D0h] - 1)·2.
pub fn sentence_offset(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x994f)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x47d0);
    if k == 0 {
        c.run(start, 5)?;
        c.set_reg8(r8::AL, k);
        c.or(B, 0, 0);
        c.set_reg(BX, 6);
        c.call(0xe3b7, 0x995c)?;
        c.run(0x995c, 3)?;
        let ax = c.reg(AX);
        let base = c.word(0xf0);
        let bp = c.add(W, ax, base);
        c.set_reg(BP, bp);
        return c.ret();
    }
    c.run(start, 8)?;
    c.or(B, k as u16, k as u16);
    let al = c.dec(B, k as u16);
    c.xor(B, 0, 0);
    let ax = c.shl1(W, al, 1);
    c.set_reg(AX, ax);
    c.set_reg(BP, ax);
    c.ret()
}

/// `loc_0996c`: with [47D0h] set, SI past 20h zero-ended strings at ES:SI.
pub fn skip_32_strings(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x996c)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x47d0);
    if k == 0 {
        c.run(start, 3)?;
        c.cmp(B, 0, 0);
        return c.ret();
    }
    let step = c.string_step(B);
    let mut p = c.reg(SI);
    let mut n = 5;
    for _ in 0..0x20 {
        loop {
            let b = c.byte_in(ES, p);
            p = p.wrapping_add(step);
            n += 3;
            if b == 0 {
                break;
            }
        }
        n += 1;
    }
    c.run(start, n + 3)?;
    c.cmp(B, k as u16, 0);
    let es = c.sreg(ES);
    c.push(es);
    let v = c.pop();
    c.set_sreg(DS, v);
    for i in 0..0x20u16 {
        loop {
            let b = c.lods(B, DS);
            c.or(B, b, b);
            if b == 0 {
                break;
            }
        }
        c.set_reg(CX, 0x1f - i);
    }
    let ss = c.sreg(SS);
    c.push(ss);
    let v = c.pop();
    c.set_sreg(DS, v);
    c.ret()
}

/// `loc_09a60`: SI past AX zero-ended strings at ES:SI (none for AL 0);
/// [47CEh] = 8.
pub fn skip_strings(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9a60)?;
    let mut c = Ctx::new(m);
    let ax = c.reg(AX);
    if ax & 0xff == 0 {
        c.run(start, 4)?;
        c.or(B, 0, 0);
        c.set_word(0x47ce, 8);
        return c.ret();
    }
    c.run(start, 7)?;
    c.or(B, ax & 0xff, ax & 0xff);
    c.set_reg(BX, ax);
    c.xor(B, 0, 0);
    c.set_reg(AX, ax & 0xff00);
    c.set_reg(CX, 0xffff);
    let si = c.reg(SI);
    c.set_reg(DI, si);
    let mut bx = ax;
    loop {
        c.m.cpu.string_op(&mut c.m.hw, 0xae, None, 0xf2); // repne scasb
        if bx == 1 {
            c.run(0x9a6f, 5)?;
        } else {
            c.run(0x9a6f, 3)?;
        }
        bx = c.dec(W, bx);
        c.set_reg(BX, bx);
        if bx == 0 {
            break;
        }
    }
    let di = c.reg(DI);
    c.set_reg(SI, di);
    c.set_word(0x47ce, 8);
    c.ret()
}

/// `loc_0908c`: when [0D83Ah] is past [4782h] and [479Eh] is 223Ch, the
/// driver's blit (far [38CDh]) of [22FCh] at x 140h to the back buffer,
/// 92h - [4793h] rows.
pub fn blit_dialogue_frame(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x908c)?;
    let mut c = Ctx::new(m);
    let ax = c.word(0xd83a);
    let lim = c.word(0x4782);
    let k = c.word(0x479e);
    if ax <= lim {
        c.run(start, 4)?;
        c.set_reg(AX, ax);
        c.cmp(W, ax, lim);
        return c.ret();
    }
    if k != 0x223c {
        c.run(start, 6)?;
        c.set_reg(AX, ax);
        c.cmp(W, ax, lim);
        c.cmp(W, k, 0x223c);
        return c.ret();
    }
    c.run(start, 14)?;
    c.set_reg(AX, ax);
    c.cmp(W, ax, lim);
    c.cmp(W, k, 0x223c);
    let cx = c.word(0x4793);
    let bx = c.sub(W, 0x92, cx);
    c.set_reg(BX, bx);
    c.xor(W, 0, 0);
    c.set_reg(DX, 0);
    c.set_reg(CX, 0xff00 | cx & 0xff);
    c.set_reg(DI, 0x140);
    let si = c.word(0x22fc);
    c.set_reg(SI, si);
    let es = c.word(0xdbda);
    c.set_sreg(ES, es);
    c.set_reg(BP, 0xd834);
    c.interp(0x90b8, 0x90bc)?;
    c.run(0x90bc, 1)?;
    c.ret()
}

/// `loc_08df0`: bit 1 of [4799h] cleared when one of the [0A9D0h]
/// entries (4 words from A9D2h) has a first word and a second of 1Eh or
/// more. SI kept.
pub fn check_dialogue_entries(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8df0)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0x4799);
    if f & 1 == 0 {
        c.run(start, 3)?;
        c.test(B, f as u16, 1);
        return c.ret();
    }
    let step = c.string_step(W);
    let cx = c.word(0xa9d0);
    if cx == 0 {
        return Err(Bail);
    }
    let mut p = 0xa9d0u16.wrapping_add(step);
    let mut n = 6;
    let mut found = false;
    for _ in 0..cx {
        let ax = c.word(p);
        p = p.wrapping_add(step);
        n += 3;
        if ax != 0 {
            n += 2;
            if c.word(p) >= 0x1e {
                n += 3;
                found = true;
                break;
            }
        }
        p = p.wrapping_add(4);
        n += 2;
    }
    c.run(start, n + if found { 0 } else { 2 })?;
    c.test(B, f as u16, 1);
    let si = c.reg(SI);
    c.push(si);
    c.set_reg(SI, 0xa9d0);
    c.lods(W, DS);
    c.set_reg(CX, cx);
    let mut cx = cx;
    loop {
        let ax = c.lods(W, DS);
        c.or(W, ax, ax);
        if ax != 0 {
            let s = c.reg(SI);
            let w = c.word(s);
            c.cmp(W, w, 0x1e);
            if w >= 0x1e {
                let v = c.and(B, f as u16, 0xfe);
                c.set_byte(0x4799, v as u8);
                break;
            }
        }
        let s = c.reg(SI);
        c.set_reg(SI, s.wrapping_add(4));
        c.add(W, s, 4);
        cx -= 1;
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    let s = c.pop();
    c.set_reg(SI, s);
    c.ret()
}

/// `loc_093df`: the speaker AL starts talking: [47BEh] = AX with AL·8,
/// its bit in [0Eh] and [14h], [47A2h] = SI its record (FD8h + 10h·AL)
/// for `loc_090bd`; [47BAh], [47B6h], [19h] 0, [47C2h] 80h.
pub fn start_speaker(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x93df)?;
    let mut c = Ctx::new(m);
    c.run(start, 16)?;
    let ax = c.reg(AX);
    let cl = ax as u8;
    c.set_reg8(r8::CL, cl);
    let al = c.shl1(B, cl as u16, 3);
    let ax = ax & 0xff00 | al;
    c.set_word(0x47be, ax);
    let bit = c.shl(W, 1, cl);
    let v = c.word(0x0e);
    let v = c.or(W, v, bit);
    c.set_word(0x0e, v);
    let v = c.word(0x14);
    let v = c.or(W, v, bit);
    c.set_word(0x14, v);
    c.set_reg(AX, bit & 0xff00 | 0x10);
    c.mul8(cl);
    let ax = c.reg(AX);
    let ax = c.add(W, ax, 0xfd8);
    c.set_reg(AX, ax);
    c.set_word(0x47a2, ax);
    c.set_reg(SI, ax);
    c.set_word(0x47ba, 0);
    c.call(0x90bd, 0x940c)?;
    c.run(0x940c, 4)?;
    c.set_word(0x47b6, 0);
    c.set_byte(0x47c2, 0x80);
    c.set_byte(0x19, 0);
    c.ret()
}

/// `loc_096f1`: [47C4h] = AX; for sentence 0Eh first `loc_031f6` with SI
/// [4756h]; then [`say_sentence`].
pub fn say_numbered(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x96f1)?;
    let mut c = Ctx::new(m);
    let ax = c.reg(AX);
    if ax & 0xff != 0x0e {
        c.run(start, 3)?;
        c.set_word(0x47c4, ax);
        c.cmp(B, ax & 0xff, 0x0e);
        return say_sentence(c.m, 0x9702);
    }
    c.run(start, 5)?;
    c.set_word(0x47c4, ax);
    c.cmp(B, ax & 0xff, 0x0e);
    let si = c.word(0x4756);
    c.set_reg(SI, si);
    c.call(0x31f6, 0x96ff)?;
    c.run(0x96ff, 1)?;
    c.set_reg(AX, 0x0e);
    say_sentence(c.m, 0x9702)
}

/// `loc_09d6a`: the phrase list at SI run up to its 0: each entry (1
/// takes the next byte as AL, AH 1) is word AX - 2 of the table at
/// [47CCh], whose text goes to `loc_09d94` (ES the back buffer).
pub fn say_phrase_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9d6a)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let es = c.word_in(SS, 0xdbd8);
    c.set_sreg(ES, es);
    loop {
        let p = c.reg(SI);
        let al = c.byte(p);
        if al == 0 {
            c.run(0x9d6f, 5)?;
            c.lods(B, DS);
            let ah = c.reg8(r8::AH);
            c.xor(B, ah as u16, ah as u16);
            c.set_reg8(r8::AH, 0);
            c.or(B, 0, 0);
            return c.ret();
        }
        c.run(0x9d6f, 6 + if al == 1 { 2 } else { 0 } + 7)?;
        c.lods(B, DS);
        let ah = c.reg8(r8::AH);
        c.xor(B, ah as u16, ah as u16);
        c.set_reg8(r8::AH, 0);
        c.or(B, al as u16, al as u16);
        c.cmp(B, al as u16, 1);
        if al == 1 {
            c.set_reg8(r8::AH, 1);
            c.lods(B, DS);
        }
        let si = c.reg(SI);
        c.push(si);
        let ax = c.reg(AX);
        let ax = c.sub(W, ax, 2);
        let ax = c.shl1(W, ax, 1);
        c.set_reg(AX, ax);
        c.set_reg(BP, ax);
        let base = c.word_in(SS, 0x47cc);
        let off = c.word_in(DS, ax.wrapping_add(base));
        let si = c.add(W, base, off);
        c.set_reg(SI, si);
        c.call(0x9d94, 0x9d90)?;
        c.run(0x9d90, 2)?;
        let s = c.pop();
        c.set_reg(SI, s);
    }
}

/// `loc_09046`: in the picture [22FCh] ([4793h]·[2240h] bytes, ES = DS),
/// each 0 next to a 0Fh (left, right, above, below; 140h a row) becomes
/// F0h (08h with [0EAh] above 0). Bails at entry for an empty picture.
pub fn outline_picture(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9046)?;
    let mut c = Ctx::new(m);
    let a = c.word(0x4793);
    let b = c.word(0x2240);
    if a.wrapping_mul(b) == 0 {
        return Err(Bail);
    }
    let k = c.byte(0xea);
    let up = (k as i8) > 0;
    c.run(start, 10 + up as u32 + 1)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(AX, a);
    c.mul16(b);
    let cx = c.reg(AX);
    c.set_reg(CX, cx);
    let di = c.word(0x22fc);
    c.set_reg(DI, di);
    c.set_reg(AX, 0xf00f);
    c.xor(W, 0, 0);
    c.set_reg(BX, 0);
    c.cmp(B, k as u16, 0);
    if up {
        c.set_reg8(r8::AH, 8);
    }
    let ah = c.reg8(r8::AH);
    loop {
        c.m.cpu.string_op(&mut c.m.hw, 0xae, None, 0xf2); // repne scasb
        if !c.flag(ZF) {
            c.run(0x9065, 2)?;
            return c.ret();
        }
        let di = c.reg(DI);
        let spots = [di.wrapping_sub(2), di, di.wrapping_sub(0x141), di.wrapping_add(0x13f)];
        let mut n = 1;
        let mut zs = [false; 4];
        for (i, &p) in spots.iter().enumerate() {
            zs[i] = c.byte_in(ES, p) == 0;
        }
        // Each test sees the writes before it; spots differ, so the reads
        // above hold.
        for i in 0..3 {
            n += 2 + zs[i] as u32;
        }
        n += 2 + if zs[3] { 2 } else { 0 } + 1;
        c.run(0x9065, n)?;
        for (i, &p) in spots.iter().enumerate() {
            let v = c.byte_in(ES, p);
            c.cmp(B, v as u16, 0);
            if zs[i] {
                c.m.cpu.write8(&mut c.m.hw, ES, p, ah);
            }
        }
    }
}

/// `loc_09bac`: `loc_09197` (SI kept), then [`dialogue_frame_rect`].
pub fn redraw_dialogue_frame(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9bac)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let si = c.reg(SI);
    c.push(si);
    c.call(0x9197, 0x9bb0)?;
    c.run(0x9bb0, 1)?;
    let si = c.pop();
    c.set_reg(SI, si);
    super::map::dialogue_frame_rect(c.m, 0x9bb1)
}

/// `loc_09f40`: the speaker [47C4h] (for 2 with [0C2h] 4 first
/// `sum_three_records`): [47A2h] = its record FD8h + 10h·AL; out of the
/// map's hotspot mode the front buffer active and the text box at (28h,
/// 10h) 10h·10h; then [`tall_font_colour_f0`].
pub fn speaker_text_setup(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9f40)?;
    let mut c = Ctx::new(m);
    let ax = c.word(0x47c4);
    let k = c.byte(0xc2);
    let call = ax == 2 && k == 4;
    if ax != 2 {
        c.run(start, 3)?;
        c.set_reg(AX, ax);
        c.cmp(W, ax, 2);
    } else if !call {
        c.run(start, 5)?;
        c.set_reg(AX, ax);
        c.cmp(W, ax, 2);
        c.cmp(B, k as u16, 4);
    } else {
        c.run(start, 8)?;
        c.set_reg(AX, ax);
        c.cmp(W, ax, 2);
        c.cmp(B, k as u16, 4);
        let si = c.reg(SI);
        c.push(ax);
        c.push(si);
        c.call(0x1243, 0x9f54)?;
        c.run(0x9f54, 2)?;
        let si = c.pop();
        c.set_reg(SI, si);
        let ax = c.pop();
        c.set_reg(AX, ax);
    }
    let h = c.byte(0x46eb);
    c.run(0x9f56, 6 + (h == 0) as u32)?;
    c.set_reg8(r8::CL, 0x10);
    c.mul8(0x10);
    let ax = c.reg(AX);
    let ax = c.add(W, ax, 0xfd8);
    c.set_reg(AX, ax);
    c.set_word(0x47a2, ax);
    c.cmp(B, h as u16, 0);
    if h == 0 {
        c.call(0xc07c, 0x9f6a)?;
        c.run(0x9f6a, 4)?;
        c.set_word(0x4784, 0x28);
        c.set_word(0x4786, 0x10);
        c.set_word(0x4788, 0x10);
        c.set_word(0x478a, 0x10);
    }
    tall_font_colour_f0(c.m, 0x9f82)
}

/// `loc_09908`: the dialogue text: SI [47CAh] + the entry BP
/// (`sentence_offset`) of the table at ES = [0DBB2h], past [47D0h]'s
/// strings (`skip_32_strings`); [47D1h] C0h, [47CEh] [478Ch]·4; [47C8h]
/// = SI, swapped into [47C6h]: when that was empty, out of the map
/// ([0EAh] at most 0) and `loc_0127c` for [47C4h] clear, the frame task
/// 99BEh (BP 10h).
pub fn start_dialogue_text(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9908)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let si = c.word(0x47ca);
    c.set_reg(SI, si);
    let es = c.word(0xdbb2);
    c.set_sreg(ES, es);
    c.call(0x994f, 0x9913)?;
    c.run(0x9913, 8)?;
    c.set_byte(0x47d1, 0xc0);
    let al = c.byte(0x478c);
    c.set_reg8(r8::AL, al);
    c.xor(B, 0, 0);
    let ax = c.shl1(W, al as u16, 2);
    c.set_reg(AX, ax);
    c.set_word(0x47ce, ax);
    let (si, bp) = (c.reg(SI), c.reg(BP));
    let off = c.word_in(ES, bp.wrapping_add(si));
    let si = c.add(W, si, off);
    c.set_reg(SI, si);
    c.call(0x996c, 0x992a)?;
    let si = c.reg(SI);
    let old = c.word(0x47c6);
    if old != 0 {
        c.run(0x992a, 5)?;
        c.set_word(0x47c8, si);
        c.set_word(0x47c6, si);
        c.set_reg(SI, old);
        c.or(W, old, old);
        return c.ret();
    }
    let k = c.byte(0xea);
    if (k as i8) > 0 {
        c.run(0x992a, 7)?;
    } else {
        c.run(0x992a, 8)?;
    }
    c.set_word(0x47c8, si);
    c.set_word(0x47c6, si);
    c.set_reg(SI, 0);
    c.or(W, 0, 0);
    c.cmp(B, k as u16, 0);
    if (k as i8) > 0 {
        return c.ret();
    }
    let ax = c.word(0x47c4);
    c.set_reg(AX, ax);
    c.call(0x127c, 0x9943)?;
    if c.flag(pc::cpu::CF) {
        c.run(0x9943, 2)?;
        return c.ret();
    }
    c.run(0x9943, 4)?;
    c.set_reg(SI, 0x99be);
    c.set_reg(BP, 0x10);
    c.call(code::ADD_FRAME_TASK, 0x994e)?;
    c.run(0x994e, 1)?;
    c.ret()
}

/// `loc_094f3`: for a speaker [47C4h] below 10h: [18h] = byte 0Fh of its
/// record [47A2h], [16h] = [2] - its word 8 (0Ah without bit 40h of
/// that byte); then before phase 64h for one below 9, `loc_02e98` for
/// the location [11DBh] if any. SI kept.
pub fn speaker_mood(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x94f3)?;
    let mut c = Ctx::new(m);
    let who = c.word(0x47c4);
    if who >= 0x10 {
        c.run(start, 3)?;
        c.cmp(W, who, 0x10);
        return c.ret();
    }
    let r = c.word(0x47a2);
    let f = c.byte(r.wrapping_add(0x0f));
    let near = f & 0x40 == 0;
    c.run(start, 9 + near as u32 + 4)?;
    c.cmp(W, who, 0x10);
    let si = c.reg(SI);
    c.push(si);
    c.set_reg(SI, r);
    c.set_byte(0x18, f);
    c.test(B, f as u16, 0x40);
    let mut ax = c.word(r.wrapping_add(8));
    if near {
        ax = c.word(r.wrapping_add(0x0a));
    }
    let t = c.word(2);
    let ax = c.sub(W, ax, t);
    let ax = c.neg(W, ax);
    c.set_reg(AX, ax);
    c.set_word(0x16, ax);
    let si = c.pop();
    c.set_reg(SI, si);
    let ph = c.byte(0x2a);
    if ph >= 0x64 {
        c.run(0x9519, 3)?;
        c.cmp(B, ph as u16, 0x64);
        return c.ret();
    }
    if who >= 9 {
        c.run(0x9519, 5)?;
        c.cmp(B, ph as u16, 0x64);
        c.cmp(W, who, 9);
        return c.ret();
    }
    let di = c.word(0x11db);
    c.run(0x9519, if di == 0 { 8 } else { 8 })?;
    c.cmp(B, ph as u16, 0x64);
    c.cmp(W, who, 9);
    c.set_reg(DI, di);
    c.or(W, di, di);
    if di == 0 {
        return c.ret();
    }
    c.call(0x2e98, 0x9532)?;
    c.run(0x9532, 1)?;
    c.ret()
}

/// `loc_09f31`: SI = sentence AX of the speaker [47BEh] (its group of 8)
/// from the table at -558Ah, then [`speaker_text_setup`].
pub fn speaker_sentence(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9f31)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let s = c.word(0x47be);
    let s = c.and(W, s, 0xfff8);
    let ax = c.reg(AX);
    let s = c.add(W, s, ax);
    let s = c.shl1(W, s, 1);
    let s = c.word(s.wrapping_sub(0x558a));
    c.set_reg(SI, s);
    speaker_text_setup(c.m, 0x9f40)
}

/// `loc_09b09`: for a sentence record [47E4h] of 38h or more: the
/// digits of [20h] (of [9Dh] when it is not exactly 38h) by
/// `decimal_digits`, each (to an FFh) stored as the word 5 + it (6 + it
/// from the first non-zero one on) of the table at [0DBB0h] over that
/// table's start.
pub fn number_words(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9b09)?;
    let mut c = Ctx::new(m);
    let si = c.word(0x47e4);
    let w = c.word(si);
    if w < 0x38 {
        c.run(start, 4)?;
        c.set_reg(SI, si);
        c.cmp(W, w, 0x38);
        return c.ret();
    }
    c.run(start, 5 + if w == 0x38 { 0 } else { 2 } + 1)?;
    c.set_reg(SI, si);
    c.cmp(W, w, 0x38);
    let ax = c.word(0x20);
    c.set_reg(AX, ax);
    if w != 0x38 {
        let al = c.byte(0x9d);
        c.xor(B, 0, 0);
        c.set_reg(AX, al as u16);
    }
    c.call(0x8a23, 0x9b1f)?;
    c.run(0x9b1f, 4)?;
    let ax = c.reg(AX);
    c.set_reg(CX, ax);
    let (si, es) = (c.word(0xdbb0), c.word(0xdbb2));
    c.set_reg(SI, si);
    c.set_sreg(ES, es);
    c.set_reg(DI, si);
    let mut bp = 5u16;
    c.set_reg(BP, bp);
    loop {
        let bx = c.reg(BX);
        let nz = bx & 0x0f != 0;
        c.run(0x9b2a, 2 + nz as u32 + 11)?;
        let b = c.and(W, bx, 0x0f);
        if nz {
            bp = 6;
            c.set_reg(BP, 6);
        }
        let b = c.add(W, b, bp);
        let b = c.shl1(W, b, 1);
        let v = c.word_in(ES, b.wrapping_add(si));
        c.set_reg(AX, v);
        c.stos(W);
        let (cx, dx) = (c.reg(CX), c.reg(DX));
        let bl = dx >> 8;
        c.set_reg(BX, b & 0xff00 | bl);
        c.set_reg(DX, (dx & 0xff) << 8 | cx >> 8);
        c.set_reg(CX, (cx & 0xff) << 8 | 0xff);
        c.cmp(B, bl, 0xff);
        if bl == 0xff {
            break;
        }
    }
    c.run(0x9b48, 1)?;
    c.ret()
}

/// `loc_09ab4`: the sentence's number part by [47E1h]: when set but not
/// yet begun (bit 80h added), `number_words`, SI the entry [47E2h] of the
/// text [47CAh] (table ES), [47CEh] 14h, CF set; when begun with bit 1,
/// [47E1h] = 80h, `loc_0d075`, and for a record [47E4h] below 38h its
/// four words to `loc_08865` (CX, DX, BX, AX), the tall font, `loc_0dbb2`,
/// `loc_0c4dd`, CF clear.
pub fn sentence_number_part(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9ab4)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x47e1);
    if k == 0 {
        c.run(start, 4)?;
        c.set_reg8(r8::AL, 0);
        c.or(B, 0, 0);
        return c.ret();
    }
    if k & 0x80 == 0 {
        c.run(start, 6)?;
        c.set_reg8(r8::AL, k);
        c.or(B, k as u16, k as u16);
        let v = c.or(B, k as u16, 0x80);
        c.set_byte(0x47e1, v as u8);
        c.call(0x9b09, 0x9ac5)?;
        c.run(0x9ac5, 8)?;
        let al = c.byte(0x47e2);
        c.set_reg8(r8::AL, al);
        c.xor(B, 0, 0);
        c.set_reg(AX, al as u16);
        c.set_reg(BP, al as u16);
        let base = c.word(0x47ca);
        let off = c.word_in(ES, (al as u16).wrapping_add(base));
        let si = c.add(W, base, off);
        c.set_reg(SI, si);
        c.set_word(0x47ce, 0x14);
        c.set_flag(pc::cpu::CF, true);
        return c.ret();
    }
    if k & 1 == 0 {
        c.run(start, 7)?;
        c.set_reg8(r8::AL, k);
        c.or(B, k as u16, k as u16);
        let a = c.shr1(B, k as u16, 1);
        c.set_reg8(r8::AL, a as u8);
        return c.ret();
    }
    c.run(start, 8)?;
    c.set_reg8(r8::AL, k);
    c.or(B, k as u16, k as u16);
    let a = c.shr1(B, k as u16, 1);
    c.set_reg8(r8::AL, a as u8);
    c.set_byte(0x47e1, 0x80);
    c.call(0xd075, 0x9ae7)?;
    let si = c.word(0x47e4);
    let w = c.word(si);
    if w >= 0x38 {
        c.run(0x9ae7, 4)?;
        c.set_reg(SI, si);
        c.cmp(W, w, 0x38);
        return c.ret();
    }
    c.run(0x9ae7, 3 + 9)?;
    c.set_reg(SI, si);
    c.cmp(W, w, 0x38);
    let a = c.lods(W, DS);
    c.set_reg(CX, a);
    let d = c.lods(W, DS);
    c.set_reg(DX, d);
    let b = c.lods(W, DS);
    c.set_reg(BX, b);
    let x = c.lods(W, DS);
    c.set_reg(CX, x);
    c.set_reg(AX, a);
    c.call(0x8865, 0x9afe)?;
    c.run(0x9afe, 1)?;
    c.call(0xd068, 0x9b01)?;
    c.run(0x9b01, 1)?;
    c.call(0xdbb2, 0x9b04)?;
    c.run(0x9b04, 1)?;
    c.call(0xc4dd, 0x9b07)?;
    c.run(0x9b07, 2)?;
    c.set_flag(pc::cpu::CF, false);
    c.ret()
}

/// `loc_09b49`: when [47E1h] is 80h: `loc_09197` (SI kept), SI the entry
/// [47E2h] + 2 of the text [47CAh] (table ES = [0DBB2h]), [47E1h] = 81h,
/// and `wait_processing_frame_tasks_interruptable` (BP 99F6h, AX 0Ch)
/// until the text at [47C6h] ends (FFh). [47E1h] cleared.
pub fn speak_number_part(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9b49)?;
    let mut c = Ctx::new(m);
    let ax = c.word(0x47e1);
    if ax & 0xff != 0x80 {
        c.run(start, 5)?;
        c.set_reg(AX, ax);
        c.cmp(B, ax & 0xff, 0x80);
        c.set_word(0x47e1, 0);
        return c.ret();
    }
    c.run(start, 6)?;
    c.set_reg(AX, ax);
    c.cmp(B, 0x80, 0x80);
    let si = c.reg(SI);
    c.push(si);
    c.push(ax);
    c.call(0x9197, 0x9b55)?;
    c.run(0x9b55, 10)?;
    let ax = c.pop();
    c.xor(B, 0, 0);
    let a = c.inc(W, ax >> 8);
    let a = c.inc(W, a);
    c.set_reg(AX, a);
    c.set_reg(BP, a);
    let base = c.word(0x47ca);
    let es = c.word(0xdbb2);
    c.set_sreg(ES, es);
    let off = c.word_in(ES, a.wrapping_add(base));
    let si = c.add(W, base, off);
    c.set_reg(SI, si);
    c.set_byte(0x47e1, 0x81);
    loop {
        c.run(0x9b6e, 4)?;
        let es = c.sreg(ES);
        c.push(es);
        c.set_reg(BP, 0x99f6);
        c.set_reg(AX, 0x0c);
        c.call(0xe353, 0x9b78)?;
        let es = c.word_in(SS, c.reg(SP));
        let si = c.word(0x47c6);
        let b = c.byte_at_seg(es, si);
        c.run(0x9b78, 4)?;
        c.pop();
        c.set_sreg(ES, es);
        c.set_reg(SI, si);
        c.cmp(B, b as u16, 0xff);
        if b == 0xff {
            break;
        }
    }
    c.run(0x9b83, 3)?;
    let s = c.pop();
    c.set_reg(SI, s);
    c.set_word(0x47e1, 0);
    c.ret()
}

/// `loc_09a7b`: SI the text of a random one of the sentences around
/// [47D0h] - 1 (else 4, 5 for speaker 7 from phase C8h; table 0F38h
/// then) of [47CAh] in ES = [0DBB2h], BP the entry; then `rand_masked`
/// with BX 0F18h (0F38h).
pub fn random_sentence(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9a7b)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x47d0);
    let who = c.word(0x47c4);
    let ph = c.byte(0x2a);
    let n = if k != 0 {
        4
    } else if who != 7 {
        8
    } else if ph < 0xc8 {
        10
    } else {
        11
    };
    c.run(start, n + 9)?;
    c.set_reg8(r8::AL, k);
    c.set_reg(BX, 0x0f18);
    c.or(B, k as u16, k as u16);
    let mut al = k;
    if k == 0 {
        al = 5;
        c.set_reg(BX, 0x0f38);
        c.cmp(W, who, 7);
        if who == 7 {
            c.cmp(B, ph as u16, 0xc8);
            if ph >= 0xc8 {
                al = c.inc(B, 5) as u8;
            }
        }
    }
    let a = c.dec(B, al as u16);
    c.xor(B, 0, 0);
    let ax = c.shl1(W, a, 1);
    c.set_reg(AX, ax);
    let base = c.word(0xf0);
    let bp = c.add(W, ax, base);
    c.set_reg(BP, bp);
    let si = c.word(0x47ca);
    let es = c.word(0xdbb2);
    c.set_sreg(ES, es);
    let off = c.word_in(ES, bp.wrapping_add(si));
    let si = c.add(W, si, off);
    c.set_reg(SI, si);
    super::math::rand_masked(c.m, 0xe3b7)
}

/// `loc_08c8a`: when [479Eh] (taken, cleared) is 2 or more: on the map
/// screen ([28E7h]) the picture 4C60h back to [0DBDEh] (far [391Dh], BP
/// 1BE2h; its word 8 then 0), the rect SI to the screen (`loc_0c446`),
/// with dialogue text [47C8h] the frame redrawn ([4540h] 0,
/// `redraw_dialogue_frame`), and `loc_0c4dd`.
pub fn close_dialogue_box(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8c8a)?;
    let mut c = Ctx::new(m);
    close_box_at(&mut c)
}

fn close_box_at(c: &mut Ctx) -> Result<(), Bail> {
    let old = c.word(0x479e);
    if old < 2 {
        c.run(0x8c8a, 5)?;
        c.xor(W, 0, 0);
        c.set_word(0x479e, 0);
        c.set_reg(AX, old);
        c.cmp(W, old, 2);
        return c.ret();
    }
    let map = c.byte(0x28e7);
    c.run(0x8c8a, 7 + if map != 0 { 3 } else { 1 })?;
    c.xor(W, 0, 0);
    c.set_word(0x479e, 0);
    c.set_reg(AX, old);
    c.cmp(W, old, 2);
    c.set_reg(SI, 0x1470);
    c.cmp(B, map as u16, 0);
    if map != 0 {
        c.set_reg(BP, 0x1be2);
        c.set_reg(SI, 0x4c60);
        let es = c.word(0xdbde);
        c.set_sreg(ES, es);
        c.interp(0x8ca9, 0x8cad)?;
        c.run(0x8cad, 3)?;
        c.set_reg(SI, 0x1be2);
        c.set_word(0x1bea, 0);
    }
    c.call(0xc446, 0x8cb8)?;
    let si = c.word(0x47c8);
    c.run(0x8cb8, 3 + if si != 0 { 2 } else { 1 })?;
    c.set_reg(SI, si);
    c.or(W, si, si);
    if si != 0 {
        c.set_word(0x4540, 0);
        c.call(0x9bac, 0x8cc9)?;
        c.run(0x8cc9, 1)?;
    }
    c.call(0xc4dd, 0x8ccc)?;
    c.run(0x8ccc, 1)?;
    c.ret()
}

/// `loc_0a0f1`: on the map screen in mode 2, a record SI with bit 10h of
/// byte 2 makes it mode 1; one without closes the dialogue box
/// ([`close_dialogue_box`]).
pub fn map_mode_2_record(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa0f1)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x28e7);
    if k != 2 {
        c.run(start, 3)?;
        c.cmp(B, k as u16, 2);
        return c.ret();
    }
    let si = c.reg(SI);
    let b = c.byte(si.wrapping_add(2));
    if b & 0x10 != 0 {
        c.run(start, 6)?;
        c.cmp(B, 2, 2);
        c.test(B, b as u16, 0x10);
        c.set_byte(0x28e7, 1);
        return c.ret();
    }
    c.run(start, 5)?;
    c.cmp(B, 2, 2);
    c.test(B, b as u16, 0x10);
    close_box_at(&mut c)
}

/// `loc_08e9e`: the words DL, then (unless DL is 0) BX / (DL - 1) + 6 and
/// its remainder (BX and 0 for DL 1), else 0, 0, stored at ES:DI; DL
/// added to [478Ch]; DH counted, DL 0, BX = [478Fh].
pub fn store_spacing(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8e9e)?;
    let mut c = Ctx::new(m);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let dl = dx & 0xff;
    if dl == 0 {
        c.run(start, 11)?;
        c.set_reg(AX, dx);
        c.xor(B, dx >> 8, dx >> 8);
        c.set_reg(AX, 0);
        c.stos(W);
        let v = c.byte(0x478c);
        let v = c.add(B, v as u16, 0);
        c.set_byte(0x478c, v as u8);
        c.or(W, 0, 0);
        c.stos(W);
        c.stos(W);
        let dh = c.inc(B, dx >> 8);
        c.set_reg(DX, dh << 8 | dl);
        let b = c.word(0x478f);
        c.set_reg(BX, b);
        return c.ret();
    }
    let d = dl - 1;
    c.run(start, 6 + 7 + (d != 0) as u32 + 9)?;
    c.set_reg(AX, dx);
    c.xor(B, dx >> 8, dx >> 8);
    c.set_reg(AX, dl);
    c.stos(W);
    let v = c.byte(0x478c);
    let v = c.add(B, v as u16, dl);
    c.set_byte(0x478c, v as u8);
    c.or(W, dl, dl);
    c.push(dx);
    c.set_reg(AX, bx);
    c.set_reg(BX, dx);
    c.xor(B, dx >> 8, dx >> 8);
    c.set_reg(BX, dl);
    c.xor(W, 0, 0);
    c.set_reg(DX, 0);
    let b = c.dec(W, dl);
    c.set_reg(BX, b);
    if d != 0 {
        c.m.cpu.mul_div(&mut c.m.hw, W, 6, b as u32);
    }
    let ax = c.reg(AX);
    let ax = c.add(W, ax, 6);
    c.set_reg(AX, ax);
    c.stos(W);
    let r = c.reg(DX);
    c.set_reg(AX, r);
    c.stos(W);
    let dx = c.pop();
    let dh = c.inc(B, dx >> 8);
    c.xor(B, 0, 0);
    c.set_reg(DX, dh << 8);
    let b = c.word(0x478f);
    c.set_reg(BX, b);
    c.ret()
}

/// `loc_09b8b`: the dialogue text dropped: `loc_0a7a5`, [47C3h], [47CEh]
/// 0, bit 80h of [47D1h] cleared, [47C6h] taken (cleared); when there
/// was one the frame task 99BEh removed.
pub fn drop_dialogue_text(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9b8b)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xa7a5, 0x9b8e)?;
    let old = c.word(0x47c6);
    c.run(0x9b8e, if old != 0 { 9 } else { 8 })?;
    c.xor(W, 0, 0);
    c.set_byte(0x47c3, 0);
    c.set_word(0x47ce, 0);
    let v = c.byte(0x47d1);
    let v = c.and(B, v as u16, 0x7f);
    c.set_byte(0x47d1, v as u8);
    c.set_word(0x47c6, 0);
    c.set_reg(AX, old);
    c.or(W, old, old);
    if old == 0 {
        return c.ret();
    }
    c.set_reg(SI, 0x99be);
    super::tasks::remove_frame_task(c.m, 0xda5f)
}

/// `loc_09655`: the speaker byte 0Eh of SI taken off the gauges at
/// 1152h/1153h (1153h becomes FFh; from 1152h the other moves down),
/// its marker at +10D0h cleared, the gauges redrawn ([`draw_gauges`]).
pub fn drop_speaker_gauge(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9655)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let cl = c.byte(si.wrapping_add(0x0e));
    let (g1, g2) = (c.byte(0x1152), c.byte(0x1153));
    if g2 != cl && g1 != cl {
        c.run(start, 9)?;
        c.set_reg8(r8::CL, cl);
        c.set_reg(DI, 0x1152);
        c.set_reg8(r8::AL, 0xff);
        c.cmp(B, g2 as u16, cl as u16);
        c.cmp(B, g1 as u16, cl as u16);
        return c.ret();
    }
    let first = g2 == cl;
    c.run(start, if first { 8 } else { 12 })?;
    c.set_reg8(r8::CL, cl);
    c.set_reg8(r8::AL, 0xff);
    c.cmp(B, g2 as u16, cl as u16);
    let (di, al) = if first {
        (0x1153u16, 0xffu8)
    } else {
        c.cmp(B, g1 as u16, cl as u16);
        c.set_byte(0x1153, 0xff);
        (0x1152, g2)
    };
    c.set_reg(DI, di);
    c.set_reg8(r8::AL, al);
    c.set_byte(di, al);
    c.set_byte(di.wrapping_add(0x10d0), 0);
    super::screens::draw_gauges(c.m, 0xd763)
}

/// `loc_098e6`: AX from `loc_098f5` into [47C8h], [47AAh], [479Eh], then
/// [`drop_dialogue_text`].
pub fn reset_dialogue(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x98e6)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x98f5, 0x98e9)?;
    c.run(0x98e9, 4)?;
    let ax = c.reg(AX);
    for p in [0x47c8u16, 0x47aa, 0x479e] {
        c.set_word(p, ax);
    }
    drop_dialogue_text(c.m, 0x9b8b)
}

/// `sub_30b9`: for a speaker record SI without bit 40h of byte 0Fh (the
/// first becomes [47AAh]): [`speaker_entries`].
pub fn add_speaker(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x30b9)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let f = c.byte(si.wrapping_add(0x0f));
    if f & 0x40 != 0 {
        c.run(start, 3)?;
        c.test(B, f as u16, 0x40);
        return c.ret();
    }
    let first = c.word(0x47aa);
    let pre = 4 + (first == 0) as u32;
    speaker_entries_at(&mut c, start, pre, |c| {
        c.test(B, f as u16, 0x40);
        c.cmp(W, first, 0);
        if first == 0 {
            let si = c.reg(SI);
            c.set_word(0x47aa, si);
        }
    })
}

/// `sub_3120`: [`speaker_entries`] for a speaker SI with bit 40h of byte
/// 0Fh.
pub fn add_speaker_40(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3120)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let f = c.byte(si.wrapping_add(0x0f));
    if f & 0x40 == 0 {
        c.run(start, 3)?;
        c.test(B, f as u16, 0x40);
        return c.ret();
    }
    speaker_entries_at(&mut c, start, 2, |c| c.test(B, f as u16, 0x40))
}

/// From `loc_030ca`: the speaker SI's entries stored at ES:DI (its
/// phrase 78h + byte 0Eh and its place, word 4; its bit in [12h]); for
/// speaker 0Fh also [476Ah] - 1 more (phrases 88h on, same place), and
/// from phase 5 the entry [476Bh] - 1 - [476Ah] (in 4 bytes) later made
/// phrase 8Fh.
fn speaker_entries_at(
    c: &mut Ctx,
    at: u16,
    pre: u32,
    head: impl FnOnce(&mut Ctx),
) -> Result<(), Bail> {
    let si = c.reg(SI);
    let id = c.byte(si.wrapping_add(0x0e));
    let place = c.word(si.wrapping_add(4));
    let mut n = pre + 12;
    let (k, more, ph, last) = (c.byte(0x476a), 0u16, c.byte(0x2a), c.byte(0x476b));
    let mut extra = more;
    if id != 0x0f {
        n += 1;
    } else {
        n += 4;
        let cnt = (k as u16).wrapping_sub(1);
        if (cnt as i16) > 0 {
            extra = cnt;
            n += 3 + 6 * cnt as u32 + 1;
        }
        n += 2;
        if ph < 5 {
            n += 1;
        } else {
            n += 3;
            n += if last == 0 { 1 } else { 10 };
        }
    }
    c.run(at, n)?;
    head(c);
    c.set_reg8(r8::AL, id);
    c.set_reg8(r8::CL, id);
    c.xor(B, 0, 0);
    let ax = c.add(W, id as u16, 0x78);
    c.set_reg(AX, ax);
    c.stos(W);
    let bit = c.shl(W, 1, id);
    let v = c.word(0x12);
    let v = c.or(W, v, bit);
    c.set_word(0x12, v);
    c.set_reg(AX, place);
    c.stos(W);
    c.cmp(B, id as u16, 0x0f);
    if id != 0x0f {
        return c.ret();
    }
    c.set_reg8(r8::CL, k);
    c.xor(B, 0, 0);
    let cx = c.dec(W, k as u16);
    c.set_reg(CX, cx);
    if extra > 0 {
        c.push(si);
        c.set_reg(SI, place);
        let mut ax = 0x87u16;
        for i in 0..extra {
            ax = c.inc(W, ax);
            c.set_reg(AX, ax);
            c.stos(W);
            c.set_reg(AX, place);
            c.stos(W);
            c.set_reg(AX, ax);
            c.set_reg(CX, extra - 1 - i);
        }
        let s = c.pop();
        c.set_reg(SI, s);
    }
    c.cmp(B, ph as u16, 5);
    if ph < 5 {
        return c.ret();
    }
    c.set_reg8(r8::AL, last);
    c.or(B, last as u16, last as u16);
    if last == 0 {
        return c.ret();
    }
    let di = c.reg(DI);
    c.push(di);
    let a = c.dec(B, last as u16);
    let a = c.sub(B, a, k as u16);
    let a = a as u8 as i8 as i16 as u16;
    let a = c.shl1(W, a, 2);
    c.set_reg(AX, a);
    let p = c.add(W, di, a);
    c.set_word(p, 0x8f);
    let d = c.pop();
    c.set_reg(DI, d);
    c.ret()
}

/// `loc_09cc6`: the sprite entry at SI (sprite, x, y; at 1BF0h's place)
/// added to the bounding rect at SS:D834h (its size from the sprite
/// table [0DBB0h]: width 9 bits, height a byte). SI, DS kept.
pub fn grow_dialogue_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9cc6)?;
    let mut c = Ctx::new(m);
    let (si, ds) = (c.reg(SI), c.sreg(DS));
    let step = c.string_step(B);
    let sp = c.byte(si);
    let x = c.byte(si.wrapping_add(step)) as u16;
    let y = c.byte(si.wrapping_add(step.wrapping_mul(2))) as u16;
    let (ox, oy) = (c.word(0x1bf0), c.word(0x1bf2));
    let (tab, seg) = (c.word(0xdbb0), c.word(0xdbb2));
    let dx = x.wrapping_add(ox);
    let bx = y.wrapping_add(oy);
    let bp = ((sp as u16).wrapping_sub(1)).wrapping_shl(1);
    let off = c.word_at(seg, bp.wrapping_add(tab));
    let p = tab.wrapping_add(off);
    let wstep = c.string_step(W);
    let w = c.word_at(seg, p) & 0x1ff;
    let h = c.word_at(seg, p.wrapping_add(wstep)) & 0xff;
    let r = [c.word_in(SS, 0xd834), c.word_in(SS, 0xd836), c.word_in(SS, 0xd838), c.word_in(SS, 0xd83a)];
    let x1 = dx.wrapping_add(w);
    let y1 = bx.wrapping_add(h);
    let n = 18 + (r[0] >= dx) as u32 + 2 + (r[1] >= bx) as u32 + 8 + (r[2] < x1) as u32 + 2 + (r[3] < y1) as u32 + 3;
    c.run(start, n)?;
    c.push(si);
    c.push(ds);
    c.xor(B, 0, 0);
    c.set_reg8(r8::AH, 0);
    c.lods(B, DS);
    c.lods(B, DS);
    let d = c.add(W, x, ox);
    c.set_reg(DX, d);
    c.lods(B, DS);
    let b = c.add(W, y, oy);
    c.set_reg(BX, b);
    c.set_sreg(DS, seg);
    c.set_reg(SI, tab);
    let bp2 = c.shl1(W, (sp as u16).wrapping_sub(1), 1);
    let _ = bp2;
    let s = c.add(W, tab, off);
    c.set_reg(SI, s);
    c.set_reg(BP, 0xd834);
    c.cmp(W, r[0], dx);
    if r[0] >= dx {
        c.set_word_in(SS, 0xd834, dx);
    }
    c.cmp(W, r[1], bx);
    if r[1] >= bx {
        c.set_word_in(SS, 0xd836, bx);
    }
    c.lods(W, DS);
    let a = c.and(W, w, 0x1ff);
    let d = c.add(W, dx, a);
    c.set_reg(DX, d);
    c.lods(W, DS);
    c.xor(B, 0, 0);
    c.set_reg(AX, h);
    let b = c.add(W, bx, h);
    c.set_reg(BX, b);
    c.cmp(W, r[2], x1);
    if r[2] < x1 {
        c.set_word_in(SS, 0xd838, x1);
    }
    c.cmp(W, r[3], y1);
    if r[3] < y1 {
        c.set_word_in(SS, 0xd83a, y1);
    }
    let d = c.pop();
    c.set_sreg(DS, d);
    let s = c.pop();
    c.set_reg(SI, s);
    c.ret()
}

/// `loc_09673`: the speaker byte 0Eh of SI put on a gauge (1152h, else
/// 1153h, when free; with both taken the first one's speaker leaves:
/// [23h] its byte 0Eh + 64h, `loc_09556`, and the second moves down),
/// its marker at +10D0h set to 10h, the gauges redrawn ([`draw_gauges`]).
/// Nothing when it already has one.
pub fn add_speaker_gauge(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9673)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI);
    let cl = c.byte(si.wrapping_add(0x0e));
    let (g1, g2) = (c.byte(0x1152), c.byte(0x1153));
    let di;
    if g1 == cl {
        c.run(start, 5)?;
        c.set_reg8(r8::CL, cl);
        c.set_reg(DI, 0x1152);
        c.cmp(B, g1 as u16, cl as u16);
        return c.ret();
    }
    if g1 == 0xff {
        c.run(start, 6 + 3)?;
        c.set_reg8(r8::CL, cl);
        c.set_reg(DI, 0x1152);
        c.cmp(B, g1 as u16, cl as u16);
        c.cmp(B, 0xff, 0xff);
        di = 0x1152u16;
    } else if g2 == cl {
        c.run(start, 10)?;
        c.set_reg8(r8::CL, cl);
        c.cmp(B, g1 as u16, cl as u16);
        c.cmp(B, g1 as u16, 0xff);
        let d = c.inc(W, 0x1152);
        c.set_reg(DI, d);
        c.cmp(B, g2 as u16, cl as u16);
        return c.ret();
    } else if g2 == 0xff {
        c.run(start, 11 + 3)?;
        c.set_reg8(r8::CL, cl);
        c.cmp(B, g1 as u16, cl as u16);
        c.cmp(B, g1 as u16, 0xff);
        let d = c.inc(W, 0x1152);
        c.set_reg(DI, d);
        c.cmp(B, g2 as u16, cl as u16);
        c.cmp(B, 0xff, 0xff);
        di = 0x1153;
    } else {
        c.run(start, 11 + 11)?;
        c.set_reg8(r8::CL, cl);
        c.cmp(B, g1 as u16, cl as u16);
        c.cmp(B, g1 as u16, 0xff);
        let d = c.inc(W, 0x1152);
        c.cmp(B, g2 as u16, cl as u16);
        c.cmp(B, g2 as u16, 0xff);
        let d = c.dec(W, d);
        c.set_reg(DI, d);
        let cx = c.reg(CX);
        c.push(cx);
        c.set_reg8(r8::CL, g1);
        c.set_reg8(r8::AL, 0x10);
        c.mul8(g1);
        let ax = c.reg(AX);
        let ax = c.add(W, ax, 0xfd8);
        c.set_reg(SI, ax);
        let b = c.byte(ax.wrapping_add(0x0e));
        let v = c.add(B, b as u16, 0x64);
        c.set_reg(AX, ax & 0xff00 | v);
        c.set_byte(0x23, v as u8);
        c.call(0x9556, 0x96a4)?;
        c.run(0x96a4, 4 + 3)?;
        let cx = c.pop();
        c.set_reg(CX, cx);
        let d = c.reg(DI);
        let d = c.inc(W, d);
        c.set_reg(DI, d);
        let a = c.byte(d);
        c.set_reg8(r8::AL, a);
        c.set_byte(d.wrapping_sub(1), a);
        di = d;
    }
    let cl = c.reg8(r8::CL);
    c.set_reg(DI, di);
    c.set_byte(di, cl);
    c.set_byte(di.wrapping_add(0x10d0), 0x10);
    super::screens::draw_gauges(c.m, 0xd763)
}

/// `loc_08e16`: the text at SI broken into lines of width [478Fh]: per
/// line its word count and spacing (`store_spacing`) from A9D2h on (CR
/// ends a line, words measured by `word_width` + 6); at the end (a byte
/// with bit 7) an empty entry and [0A9D0h] = the line count; C8h there
/// for a word that fits no line.
pub fn break_lines(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8e16)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_byte(0x478c, 0);
    c.set_reg(DI, 0xa9d2);
    let dx = c.reg(DX);
    c.xor(B, 0, 0);
    let w = c.word(0x478f);
    c.set_reg(BX, w);
    c.set_reg(DX, dx & 0xff & 0);
    let mut label = 0x8e28u16;
    loop {
        match label {
            0x8e28 => {
                let si = c.reg(SI);
                let al = c.byte(si);
                if al & 0x80 != 0 {
                    c.run(0x8e28, 3)?;
                    c.set_reg8(r8::AL, al);
                    c.or(B, al as u16, al as u16);
                    label = 0x8e74;
                } else if al == 0x0d {
                    c.run(0x8e28, 6)?;
                    c.set_reg8(r8::AL, al);
                    c.or(B, al as u16, al as u16);
                    c.cmp(B, al as u16, 0x0d);
                    c.call(0x8e9e, 0x8e3c)?;
                    c.run(0x8e3c, 5)?;
                    let di = c.reg(DI);
                    c.set_word(di.wrapping_sub(4), 6);
                    c.set_word(di.wrapping_sub(2), 0);
                    let dx = c.reg(DX);
                    c.xor(B, 0, 0);
                    c.set_reg(DX, dx & 0xff00);
                    let s = c.reg(SI);
                    let s = c.inc(W, s);
                    c.set_reg(SI, s);
                } else if al == 0x20 {
                    c.run(0x8e28, 9)?;
                    c.set_reg8(r8::AL, al);
                    c.or(B, al as u16, al as u16);
                    c.cmp(B, al as u16, 0x0d);
                    c.cmp(B, al as u16, 0x20);
                    let s = c.inc(W, si);
                    c.set_reg(SI, s);
                } else {
                    c.run(0x8e28, 8)?;
                    c.set_reg8(r8::AL, al);
                    c.or(B, al as u16, al as u16);
                    c.cmp(B, al as u16, 0x0d);
                    c.cmp(B, al as u16, 0x20);
                    c.call(0x8ed3, 0x8e4e)?;
                    let cx = c.reg(CX);
                    if cx == 0 {
                        c.run(0x8e4e, 2)?;
                        c.or(W, 0, 0);
                    } else {
                        c.run(0x8e4e, 3)?;
                        c.or(W, cx, cx);
                        let x = c.add(W, cx, 6);
                        c.set_reg(CX, x);
                        label = 0x8e55;
                    }
                }
            }
            0x8e55 => {
                let (bx, cx) = (c.reg(BX), c.reg(CX));
                let fits = bx >= cx;
                if fits {
                    c.run(0x8e55, 4)?;
                    let b = c.sub(W, bx, cx);
                    c.set_reg(BX, b);
                    let dx = c.reg(DX);
                    let dl = c.inc(B, dx & 0xff);
                    c.set_reg(DX, dx & 0xff00 | dl);
                    label = 0x8e28;
                    continue;
                }
                let b = bx.wrapping_sub(cx).wrapping_add(6);
                if b & 0x8000 == 0 {
                    c.run(0x8e55, 6)?;
                    let b = c.sub(W, bx, cx);
                    let b = c.add(W, b, 6);
                    c.set_reg(BX, b);
                    let dx = c.reg(DX);
                    let dl = c.inc(B, dx & 0xff);
                    c.set_reg(DX, dx & 0xff00 | dl);
                    c.call(0x8e9e, 0x8e67)?;
                    c.run(0x8e67, 1)?;
                    label = 0x8e28;
                    continue;
                }
                let dl = c.reg(DX) & 0xff;
                if dl == 0 {
                    c.run(0x8e55, 8)?;
                    let b = c.sub(W, bx, cx);
                    let b = c.add(W, b, 6);
                    c.set_reg(BX, b);
                    c.or(B, 0, 0);
                    c.set_word(0xa9d0, 0xc8);
                    return c.ret();
                }
                c.run(0x8e55, 8)?;
                let b = c.sub(W, bx, cx);
                let b = c.add(W, b, 6);
                c.or(B, dl, dl);
                let b = c.add(W, b, cx);
                c.set_reg(BX, b);
                c.call(0x8e9e, 0x8e72)?;
                c.run(0x8e72, 1)?;
                label = 0x8e55;
            }
            _ => {
                let dl = c.reg(DX) & 0xff;
                c.run(0x8e74, 2 + (dl != 0) as u32)?;
                c.or(B, dl, dl);
                if dl != 0 {
                    c.call(0x8e9e, 0x8e7b)?;
                }
                c.run(0x8e7b, 8)?;
                let di = c.reg(DI);
                c.set_word(di.wrapping_sub(4), 6);
                c.set_word(di.wrapping_sub(2), 0);
                c.set_word(di, 0);
                c.set_word(di.wrapping_add(2), 0);
                c.xor(B, 0, 0);
                let dx = c.reg(DX);
                let v = dx >> 8;
                c.set_reg(DX, v);
                c.set_word(0xa9d0, v);
                return c.ret();
            }
        }
    }
}

/// `loc_09123`: the sentence AL picked for a speaker: [47D0h] = its
/// variant (AL 0Dh by the current location's first byte; 0Eh and up by
/// the speaker's record, its byte split in threes into 0Fh or 11h
/// choices; from 11h by the time and [0F4h]), AL the sentence, AH 0.
pub fn pick_sentence(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9123)?;
    let mut c = Ctx::new(m);
    let al0 = c.reg8(r8::AL);
    let dx = c.reg(DX);
    let mut n;
    let (mut al, mut ah) = (al0, 0u8);
    let mut si = None;
    let mut di = None;
    let mut pushed = false;
    if al0 >= 0x11 {
        let t = c.word(2) << 2;
        ah = (t >> 8) as u8;
        n = 2 + 5;
        if ah >= 8 {
            ah = 8;
            n += 1;
        }
        ah <<= 1;
        if c.byte(0xf4) >= 0x10 {
            ah = ah.wrapping_add(1);
        }
        al = 0x2d;
        ah = ah.wrapping_add(1);
        n += 6 + 1 + 3;
    } else {
        n = 2 + 3;
        if al0 < 0x0d {
            n += 3;
        } else if al0 == 0x0d {
            let d = c.word(0x114e);
            di = Some(d);
            ah = (c.byte(d) >> 1).wrapping_add(1);
            n += 1 + 5 + 3;
        } else {
            let mut s = c.word(0x4756);
            n += 1 + 3;
            let mut go = true;
            if al0 != 0x0e {
                n += 2;
                if c.byte(0x2a) == 0xc8 {
                    n += 3;
                    go = false;
                } else {
                    let a = (c.byte(0x476c) as u16) << 1;
                    ah = (a >> 8) as u8;
                    s = c.word(a.wrapping_add(0x4758));
                    n += 4;
                }
            }
            si = Some(s);
            if go {
                pushed = true;
                let ax = (ah as u16) << 8 | c.byte(s) as u16;
                let (mut q, r) = ((ax / 3) as u8, (ax % 3) as u8);
                n += 7;
                let dl = if r == 0 { 0x0f } else { 0x11 };
                n += (r != 0) as u32;
                n += 2;
                while q >= dl {
                    q -= dl;
                    n += 4;
                }
                ah = q.wrapping_add(1);
                al = r.wrapping_add(0x0e);
                n += 3 + 1 + 3;
            }
        }
    }
    c.run(start, n)?;
    if let Some(d) = di {
        c.set_reg(DI, d);
    }
    if let Some(s) = si {
        c.set_reg(SI, s);
    }
    if pushed {
        c.below_sp(1, dx);
    }
    c.set_byte(0x47d0, ah);
    c.xor(B, 0, 0);
    c.set_reg(AX, al as u16);
    c.ret()
}

/// `open_talking_head_resource`: resource AX + 2 opened
/// (`open_resource_by_index`).
pub fn open_talking_head_resource(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x920f)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let ax = c.reg(AX);
    let ax = c.add(W, ax, 2);
    c.set_reg(AX, ax);
    super::resources::open_resource_by_index(c.m, 0xc13e)
}

/// `loc_098b2`: unless [47C3h], the dialogue text closed: [4540h] 0,
/// bits C0h of [47D1h] cleared, [47C8h] taken; when there was one the
/// frame 1BF0h emptied (its word 8 and [1C06h] 0) and redrawn
/// (`loc_0c446`, `draw_head_if_needed_and_update_screen_rect_at_si`),
/// then [`drop_dialogue_text`].
pub fn close_dialogue_text(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x98b2)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x47c3);
    if k != 0 {
        c.run(start, 3)?;
        c.cmp(B, k as u16, 0);
        return c.ret();
    }
    let old = c.word(0x47c8);
    c.run(start, 2 + 6 + if old == 0 { 1 } else { 4 })?;
    c.cmp(B, 0, 0);
    c.xor(W, 0, 0);
    c.set_word(0x4540, 0);
    let v = c.byte(0x47d1);
    let v = c.and(B, v as u16, 0x3f);
    c.set_byte(0x47d1, v as u8);
    c.set_word(0x47c8, 0);
    c.set_reg(AX, old);
    c.or(W, old, old);
    if old == 0 {
        return c.ret();
    }
    c.set_reg(SI, 0x1bf0);
    c.set_word(0x1bf8, 0);
    c.set_word(0x1c06, 0);
    c.call(0xc446, 0x98dc)?;
    c.run(0x98dc, 2)?;
    c.set_reg(SI, 0x1bf0);
    c.call(0xc4f0, 0x98e2)?;
    c.run(0x98e2, 1)?;
    drop_dialogue_text(c.m, 0x9b8b)
}

/// `loc_0978e`: the dialogue scene redrawn: `loc_04aca`; for a speaker
/// ([47C4h] not FFFFh) `loc_091a0`, `start_dialogue_text`, the box
/// 1BE2h (`loc_0c477`) when [479Eh], the text frame
/// (`redraw_dialogue_frame`, [4540h] 0; `loc_09025` for the box 223Ch);
/// the palette and [`sub_c4dd`].
pub fn redraw_dialogue_scene(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x978e)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x4aca, 0x9791)?;
    let who = c.word(0x47c4);
    c.run(0x9791, 3 + if who == 0xffff { 1 } else { 1 })?;
    c.set_reg(AX, who);
    c.cmp(W, who, 0xffff);
    if who == 0xffff {
        return c.ret();
    }
    c.call(0x91a0, 0x979c)?;
    c.run(0x979c, 1)?;
    c.call(0x9908, 0x979f)?;
    let b = c.word(0x479e);
    c.run(0x979f, 2 + if b != 0 { 2 } else { 0 })?;
    c.cmp(W, b, 0);
    if b != 0 {
        c.set_reg(SI, 0x1be2);
        c.call(0xc477, 0x97ac)?;
    }
    let si = c.word(0x47c8);
    c.run(0x97ac, 3 + if si != 0 { 2 } else { 1 })?;
    c.set_reg(SI, si);
    c.or(W, si, si);
    if si != 0 {
        c.set_word(0x4540, 0);
        c.call(0x9bac, 0x97bd)?;
        let b = c.word(0x479e);
        c.run(0x97bd, 2 + 1)?;
        c.cmp(W, b, 0x223c);
        if b == 0x223c {
            c.call(0x9025, 0x97c8)?;
            c.run(0x97c8, 1)?;
        }
    }
    c.call(0xc0f4, 0x97cb)?;
    c.run(0x97cb, 1)?;
    super::draw::sub_c4dd(c.m, 0xc4dd)
}

/// `loc_099da` (a frame task): the dialogue text's next step: with the
/// text running (bit 80h of [47D1h]) the next phrase at [47C6h] (in
/// [0DBB2h]) framed (`dialogue_frame_rect`, [47CEh] counted down; at the
/// list's end the next sentence of [47CAh] by `sentence_offset`,
/// `skip_32_strings`; at the count's end the number part:
/// `sentence_number_part`, `random_sentence`, `skip_strings`), its box
/// drawn (`blit_dialogue_frame`, `loc_0db74`,
/// `draw_head_if_needed_and_update_screen_rect_at_si`) unless the frame
/// is closed (13Fh); then [`draw_mouse_cursor_if_needed`].
pub fn dialogue_text_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x99da)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x9197, 0x99dd)?;
    let al = c.byte(0x47d1);
    if al & 0x80 == 0 {
        c.run(0x99dd, 4)?;
        c.set_reg8(r8::AL, al);
        c.or(B, al as u16, al as u16);
        return c.ret();
    }
    let mut label;
    if al & 0x10 != 0 {
        c.run(0x99dd, 5)?;
        c.set_reg8(r8::AL, al);
        c.or(B, al as u16, al as u16);
        c.test(B, al as u16, 0x10);
        label = 0x9a40u16;
    } else {
        c.run(0x99dd, 7)?;
        c.set_reg8(r8::AL, al);
        c.or(B, al as u16, al as u16);
        c.test(B, al as u16, 0x10);
        let si = c.word(0x47c6);
        c.set_reg(SI, si);
        let es = c.word(0xdbb2);
        c.set_sreg(ES, es);
        label = 0x99f0;
    }
    loop {
        match label {
            0x99f0 => {
                let si = c.reg(SI);
                let b = c.byte_in(ES, si);
                c.run(0x99f0, 2)?;
                c.cmp(B, b as u16, 0xff);
                label = if b == 0xff { 0x9a1d } else { 0x99f6 };
            }
            0x99f6 => {
                c.run(0x99f6, 3)?;
                let v = c.word(0x47ce);
                let v = c.dec(W, v);
                c.set_word(0x47ce, v);
                let si = c.reg(SI);
                c.set_word(0x47c8, si);
                c.call(0x9bb1, 0x9a01)?;
                let x = c.word(0xd834);
                if x == 0x13f {
                    c.run(0x9a01, 4)?;
                    let si = c.reg(SI);
                    c.set_word(0x47c6, si);
                    c.cmp(W, x, 0x13f);
                    return c.ret();
                }
                c.run(0x9a01, 4)?;
                let si = c.reg(SI);
                c.set_word(0x47c6, si);
                c.cmp(W, x, 0x13f);
                c.call(0x908c, 0x9a10)?;
                c.run(0x9a10, 2)?;
                c.set_reg(SI, 0xd834);
                c.call(0xdb74, 0x9a16)?;
                c.run(0x9a16, 1)?;
                c.call(0xc4f0, 0x9a19)?;
                c.run(0x9a19, 1)?;
                return super::mouse::draw_mouse_cursor_if_needed(c.m, 0xdb67);
            }
            0x9a1d => {
                let n = c.word(0x47ce);
                if n & 0x8000 != 0 {
                    c.run(0x9a1d, 3)?;
                    c.cmp(W, n, 0);
                    let v = c.byte(0x47d1);
                    let v = c.or(B, v as u16, 0x10);
                    c.set_byte(0x47d1, v as u8);
                    label = 0x9a40;
                    continue;
                }
                c.run(0x9a1d, 5)?;
                c.cmp(W, n, 0);
                let si = c.word(0x47ca);
                c.set_reg(SI, si);
                let es = c.word(0xdbb2);
                c.set_sreg(ES, es);
                c.call(0x994f, 0x9a2f)?;
                c.run(0x9a2f, 2)?;
                let (si, bp) = (c.reg(SI), c.reg(BP));
                let off = c.word_in(ES, bp.wrapping_add(si));
                let si = c.add(W, si, off);
                c.set_reg(SI, si);
                c.call(0x996c, 0x9a35)?;
                c.run(0x9a35, 2)?;
                let si = c.reg(SI);
                c.set_word(0x47c6, si);
                label = 0x99f0;
            }
            _ => {
                let n = c.word(0x47ce);
                let si = c.word(0x47c6);
                let es = c.word(0xdbb2);
                if (n as i16) > 0 {
                    c.run(0x9a40, 4)?;
                    c.set_reg(SI, si);
                    c.set_sreg(ES, es);
                    c.cmp(W, n, 0);
                    label = 0x99f6;
                    continue;
                }
                c.run(0x9a40, 5)?;
                c.set_reg(SI, si);
                c.set_sreg(ES, es);
                c.cmp(W, n, 0);
                c.call(0x9ab4, 0x9a52)?;
                if c.flag(pc::cpu::CF) {
                    c.run(0x9a52, 1)?;
                    label = 0x99f6;
                    continue;
                }
                c.run(0x9a52, 2)?;
                c.call(0x9a7b, 0x9a57)?;
                let ah = c.reg8(r8::AH);
                if ah != 0 {
                    c.run(0x9a57, 3)?;
                    c.or(B, ah as u16, ah as u16);
                    return c.ret();
                }
                c.run(0x9a57, 3)?;
                c.or(B, 0, 0);
                c.call(0x9a60, 0x9a5e)?;
                c.run(0x9a5e, 1)?;
                label = 0x99f6;
            }
        }
    }
}

/// `loc_080df`: a phrase AX shown over the map: on the back buffer
/// `gfx_draw_1be2`, the box 2244h raised (y + 26h from 32h, height 19h;
/// both restored after), the tall font colour, [`show_phrase`]; outside
/// [4774h] the text timer [4780h] += 10Ah + [0D810h] and `loc_09efd`;
/// front buffer, [4720h] 18F3h, [4722h] 0, `loc_0541f`, [1BEAh] 0.
pub fn show_phrase_on_map(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x80df)?;
    let mut c = Ctx::new(m);
    phrase_on_map_at(&mut c, start, 0, |_| {})
}

fn phrase_on_map_at(c: &mut Ctx, at: u16, pre: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, pre + 2)?;
    ops(c);
    let ax = c.reg(AX);
    c.push(ax);
    c.call(0xc08e, 0x80e3)?;
    c.run(0x80e3, 1)?;
    c.call(0x8fd1, 0x80e6)?;
    let y = c.word(0x2246);
    let big = y >= 0x32;
    c.run(0x80e6, 8 + big as u32 + 3)?;
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.set_reg(SI, 0x2244);
    c.set_reg(AX, y);
    c.push(y);
    let h = c.word(0x224a);
    c.push(h);
    c.push(bx);
    c.cmp(W, y, 0x32);
    let mut ny = y;
    if big {
        ny = c.add(W, y, 0x26);
        c.set_reg(AX, ny);
    }
    c.set_word(0x2246, ny);
    c.set_word(0x224a, 0x19);
    c.call(0x9f82, 0x8105)?;
    c.run(0x8105, 2)?;
    let a = c.pop();
    c.set_reg(AX, a);
    c.call(0x88af, 0x8109)?;
    let k = c.byte(0x4774);
    c.run(0x8109, 2 + if k == 0 { 4 } else { 3 })?;
    c.cmp(B, k as u16, 0);
    if k == 0 {
        let d = c.word(0xd810);
        let a = c.add(W, 0x10a, d);
        c.set_reg(AX, a);
        let t = c.word(0x4780);
        let t = c.add(W, t, a);
        c.set_word(0x4780, t);
        c.call(0x9efd, 0x811e)?;
        c.run(0x811e, 3)?;
    }
    let v = c.pop();
    c.set_word(0x224a, v);
    let v = c.pop();
    c.set_word(0x2246, v);
    c.call(0xc07c, 0x8129)?;
    c.run(0x8129, 3)?;
    c.set_word(0x4720, 0x18f3);
    c.set_byte(0x4722, 0);
    c.call(0x541f, 0x8137)?;
    c.run(0x8137, 2)?;
    c.set_word(0x1bea, 0);
    c.ret()
}

/// `loc_088af`: the phrase AX (none for 0) shown: [4780h] = it, [47E0h]
/// 0; over the map ([`show_phrase_on_map`], the hotspot bit 40h of
/// [46EBh] dropped); else fetched (`get_phrase_or_command_string_si`),
/// expanded (`loc_088f1`, `format_interpolated_string` into A6B0h): on
/// the map screen ([28E7h] 2 and up) out of [46D9h] the box 1BE2h reset
/// (its byte at 1C06h from [0FBh]); else, unless it starts with a
/// control code, `loc_08b11`.
pub fn show_phrase(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x88af)?;
    let mut c = Ctx::new(m);
    let ax = c.reg(AX);
    if ax == 0 {
        c.run(start, 3)?;
        c.or(W, 0, 0);
        return c.ret();
    }
    let h = c.byte(0x46eb);
    if h & 0x40 != 0 {
        return phrase_on_map_at(&mut c, start, 2 + 4 + 2, |c| {
            c.or(W, ax, ax);
            c.set_word(0x4780, ax);
            c.set_byte(0x47e0, 0);
            c.test(B, h as u16, 0x40);
            let v = c.and(B, h as u16, 0xbf);
            c.set_byte(0x46eb, v as u8);
        });
    }
    c.run(start, 2 + 4 + 2)?;
    c.or(W, ax, ax);
    c.set_word(0x4780, ax);
    c.set_byte(0x47e0, 0);
    c.test(B, h as u16, 0x40);
    c.set_reg(SI, ax);
    c.call(0xcf70, 0x88cf)?;
    c.run(0x88cf, 1)?;
    c.call(0x88f1, 0x88d2)?;
    c.run(0x88d2, 3)?;
    c.set_reg(DI, 0xa6b0);
    c.push(0xa6b0);
    c.call(0x8944, 0x88d9)?;
    let mapk = c.byte(0x28e7);
    let busy = c.byte(0x46d9);
    if mapk >= 2 && busy == 0 {
        c.run(0x88d9, 3 + 2 + 16)?;
        let s = c.pop();
        c.set_reg(SI, s);
        c.cmp(B, mapk as u16, 2);
        c.cmp(B, 0, 0);
        c.set_word(0x479e, 1);
        let fb = c.byte(0xfb);
        let al = c.and(B, !fb as u16, 0x80);
        c.set_byte(0x1c06, al as u8);
        let ds = c.sreg(DS);
        c.push(ds);
        let es = c.pop();
        c.set_sreg(ES, es);
        c.set_reg(DI, 0x1be2);
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
        for _ in 0..4 {
            c.stos(W);
        }
        c.set_reg8(r8::AL, 0x80);
        c.stos(W);
        return c.ret();
    }
    let si = c.word_in(SS, c.reg(SP));
    let al = c.byte(si);
    let pre = 3 + if mapk >= 2 { 2 } else { 0 };
    if al & 0x80 != 0 {
        c.run(0x88d9, pre + 4)?;
    } else {
        c.run(0x88d9, pre + 5)?;
    }
    let s = c.pop();
    c.set_reg(SI, s);
    c.cmp(B, mapk as u16, 2);
    if mapk >= 2 {
        c.cmp(B, busy as u16, 0);
    }
    let a = c.lods(B, DS);
    c.or(B, a, a);
    if a & 0x80 != 0 {
        return c.ret();
    }
    let s = c.dec(W, c.reg(SI));
    c.set_reg(SI, s);
    c.call(0x8b11, 0x88ea)?;
    let si = c.reg(SI);
    let b = c.byte(si);
    c.run(0x88ea, 2 + if b >= 0xfe { 1 } else { 2 })?;
    c.cmp(B, b as u16, 0xfe);
    c.ret()
}

/// `loc_09efd`: the voice for the text: [47DDh] = [47DCh]; when
/// `load_voc_and_lipsync_data` (AX [4780h], BX [47C4h]) starts one (CF),
/// `loc_09f1c` for a speaker below 10h, `loc_09197`, the lip-sync task
/// A7C2h (`add_frame_task`, BP 0), [0DC2Bh] 1, the driver's voice start
/// (far [3991h] with 3811h), [0DC2Ch] the time, [0DC2Eh] 8000h, and
/// `loc_0a82e` when `loc_0a814` says so.
pub fn start_text_voice(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9efd)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let al = c.byte(0x47dc);
    c.set_reg8(r8::AL, al);
    c.set_byte(0x47dd, al);
    let ax = c.word(0x4780);
    c.set_reg(AX, ax);
    let bx = c.word(0x47c4);
    c.set_reg(BX, bx);
    c.call(0xa6cc, 0x9f0d)?;
    if !c.flag(pc::cpu::CF) {
        c.run(0x9f0d, 2)?;
        return c.ret();
    }
    let who = c.word(0x47c4);
    if who < 0x10 {
        c.run(0x9f0d, 4)?;
        c.cmp(W, who, 0x10);
        c.call(0x9f1c, 0x9f19)?;
        c.run(0x9f19, 2)?;
    } else {
        c.run(0x9f0d, 5)?;
        c.cmp(W, who, 0x10);
    }
    c.call(0x9197, 0xa75f)?;
    c.run(0xa75f, 3)?;
    c.set_reg(BP, 0);
    c.set_reg(SI, 0xa7c2);
    c.call(code::ADD_FRAME_TASK, 0xa768)?;
    c.run(0xa768, 2)?;
    c.set_byte(0xdc2b, 1);
    c.set_reg(SI, 0x3811);
    c.interp(0xa770, 0xa774)?;
    c.run(0xa774, 4)?;
    let t = c.word(0xce7a);
    c.set_reg(AX, t);
    c.set_word(0xdc2c, t);
    c.set_word(0xdc2e, 0x8000);
    c.call(0xa814, 0xa783)?;
    if !c.flag(pc::cpu::CF) {
        c.run(0xa783, 2)?;
        return c.ret();
    }
    c.run(0xa783, 2)?;
    c.call(0xa82e, 0xa788)?;
    c.run(0xa788, 1)?;
    c.ret()
}

/// `loc_098af`: [`close_dialogue_box`], then [`close_dialogue_text`].
pub fn close_dialogue(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x98af)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x8c8a, 0x98b2)?;
    close_dialogue_text(c.m, 0x98b2)
}

/// `loc_09c2d`: the dialogue rect reset (13Fh, C7h, 0, 0) and grown by
/// the 3-byte entries of the lists 4540h and 4608h each missing from the
/// other ([`rects_missing`]), then clamped.
pub fn diff_rects(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9c2d)?;
    let mut c = Ctx::new(m);
    if c.flag(pc::cpu::DF) {
        return Err(Bail);
    }
    c.run(start, 10)?;
    c.set_word(0xd834, 0x13f);
    c.set_word(0xd836, 0xc7);
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.set_word(0xd838, 0);
    c.set_word(0xd83a, 0);
    let ds = c.sreg(DS);
    c.set_reg(AX, ds);
    c.set_sreg(ES, ds);
    c.set_reg(SI, 0x4540);
    c.set_reg(DI, 0x4608);
    c.call(0x9c54, 0x9c4e)?;
    c.run(0x9c4e, 2)?;
    c.set_reg(SI, 0x4608);
    c.set_reg(DI, 0x4540);
    rects_missing_at(&mut c, 0x9c54)
}

/// `loc_09c54`: each 3-byte entry of the list SI (a count word, then the
/// entries) not in the list ES:DI grows the dialogue rect
/// (`grow_dialogue_rect`); then the rect's bottom [0D83Ah] lowered to the
/// voice's line (unless one plays, or [47E1h] 81h, or speaker 9 / 0Ch;
/// to 4Ch for [47E1h] 80h), [0D834h] 13Fh when that is above its top.
/// Bails on an empty list (the loop would run 65536 times) or with DF.
pub fn rects_missing(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9c54)?;
    let mut c = Ctx::new(m);
    if c.flag(pc::cpu::DF) {
        return Err(Bail);
    }
    rects_missing_at(&mut c, start)
}

fn rects_missing_at(c: &mut Ctx, start: u16) -> Result<(), Bail> {
    let mut at = start;
    let mut resume = false;
    loop {
        // A first pass counts the stretch (registers, flags and the stack
        // below SP undone), the second one does it.
        let regs = c.m.cpu.regs;
        let flags = c.m.cpu.flags();
        let sp = c.reg(SP);
        // The slots the stretch can push to or pop and push again.
        let slots: Vec<(u16, u16)> = (0..8u16).map(|i| sp.wrapping_sub(8).wrapping_add(2 * i)).map(|a| (a, c.word_in(SS, a))).collect();
        let r = rects_walk(c, resume);
        c.m.cpu.regs = regs;
        c.m.cpu.set_flags(flags);
        for &(a, w) in &slots {
            c.set_word_in(SS, a, w);
        }
        let (n, call) = r?;
        c.run(at, n)?;
        rects_walk(c, resume)?;
        if !call {
            break;
        }
        c.call(0x9cc6, 0x9c72)?;
        at = 0x9c72;
        resume = true;
    }
    rects_tail(c)
}

/// The loops of `loc_09c54` from the entry (or after the call at 9C6Fh)
/// up to the next call (true) or their end: the count.
fn rects_walk(c: &mut Ctx, resume: bool) -> Result<(u32, bool), Bail> {
    let mut n = 0u32;
    // 0: the outer loop's top, 1: the inner loop, 2: past an entry.
    let mut at;
    if resume {
        let si = c.reg(SI);
        let s = c.add(W, si, 3);
        c.set_reg(SI, s);
        n += 1;
        at = 2;
    } else {
        let v = c.lods(W, DS);
        c.set_reg(CX, v);
        n += 2;
        if v == 0 {
            return Err(Bail);
        }
        at = 0;
    }
    loop {
        if n > 400_000 {
            return Err(Bail);
        }
        match at {
            0 => {
                let (cx, di) = (c.reg(CX), c.reg(DI));
                c.push(cx);
                c.push(di);
                let k = c.word(di);
                c.set_reg(CX, k);
                let d = c.add(W, di, 2);
                c.set_reg(DI, d);
                n += 4;
                if k == 0 {
                    return Err(Bail);
                }
                at = 1;
            }
            1 => {
                c.m.cpu.string_op(&mut c.m.hw, 0xa7, None, 0);
                let f1 = c.m.cpu.flags() as u8;
                c.m.cpu.string_op(&mut c.m.hw, 0xa6, None, 0);
                let f2 = c.m.cpu.flags() as u8;
                let al = c.and(B, f1 as u16, f2 as u16);
                c.set_reg(AX, (f2 as u16) << 8 | al);
                c.test(B, al, 0x40);
                n += 8;
                if al & 0x40 != 0 {
                    at = 2;
                    continue;
                }
                let si = c.reg(SI);
                let s = c.sub(W, si, 3);
                c.set_reg(SI, s);
                let cx = c.reg(CX).wrapping_sub(1);
                c.set_reg(CX, cx);
                n += 2;
                if cx == 0 {
                    return Ok((n + 1, true));
                }
            }
            _ => {
                let d = c.pop();
                c.set_reg(DI, d);
                let cx = c.pop().wrapping_sub(1);
                c.set_reg(CX, cx);
                n += 3;
                if cx == 0 {
                    return Ok((n, false));
                }
                at = 0;
            }
        }
    }
}

/// `loc_09c54` from 9C79h: the rect's bottom by the voice line.
fn rects_tail(c: &mut Ctx) -> Result<(), Bail> {
    let k = c.byte(0x47e1);
    if k == 0x81 {
        return rects_tail_9ca6(c, 0x9c79, 2, |c| c.cmp(B, 0x81, 0x81));
    }
    c.run(0x9c79, 3)?;
    c.cmp(B, k as u16, 0x81);
    c.call(0xabcc, 0x9c83)?;
    if c.flag(ZF) {
        return rects_tail_9ca6(c, 0x9c83, 1, |_| {});
    }
    let ax = c.word(0x47c4);
    let al = ax as u8;
    if al == 9 {
        c.run(0x9c83, 5)?;
        c.set_reg(AX, ax);
        c.cmp(B, 9, 9);
        return c.ret();
    }
    if al == 0x0c {
        c.run(0x9c83, 7)?;
        c.set_reg(AX, ax);
        c.cmp(B, 0x0c, 9);
        c.cmp(B, 0x0c, 0x0c);
        return c.ret();
    }
    let si = c.word(0xdc28);
    let v = c.word(si.wrapping_add(2));
    let bottom = c.word(0xd83a);
    let pre = |c: &mut Ctx| {
        c.set_reg(AX, ax);
        c.cmp(B, al as u16, 9);
        c.cmp(B, al as u16, 0x0c);
        c.set_reg(SI, si);
        c.set_reg(AX, v);
        c.cmp(W, v, bottom);
    };
    if v >= bottom {
        return rects_tail_9ca6(c, 0x9c83, 10, pre);
    }
    let top = c.word(0xd836);
    let le = (v as i16) <= (top as i16);
    let ops = |c: &mut Ctx| {
        pre(c);
        c.set_word(0xd83a, v);
        c.cmp(W, v, top);
    };
    if le {
        c.run(0x9c83, 13 + 2)?;
        ops(c);
        c.set_word(0xd834, 0x13f);
        return c.ret();
    }
    rects_tail_9ca6(c, 0x9c83, 13, ops)
}

/// From 9CA6h (after `pre` instructions from `at` with `ops`): for
/// [47E1h] 80h the bottom up to 4Ch.
fn rects_tail_9ca6(c: &mut Ctx, at: u16, pre: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    let k = c.byte(0x47e1);
    if k != 0x80 {
        c.run(at, pre + 3)?;
        ops(c);
        c.cmp(B, k as u16, 0x80);
        return c.ret();
    }
    let bottom = c.word(0xd83a);
    if 0x4c >= bottom {
        c.run(at, pre + 2 + 3 + 1)?;
        ops(c);
        c.cmp(B, 0x80, 0x80);
        c.set_reg(AX, 0x4c);
        c.cmp(W, 0x4c, bottom);
        return c.ret();
    }
    let top = c.word(0xd836);
    let gt = 0x4c > (top as i16);
    c.run(at, pre + 2 + 3 + 3 + if gt { 1 } else { 2 })?;
    ops(c);
    c.cmp(B, 0x80, 0x80);
    c.set_reg(AX, 0x4c);
    c.cmp(W, 0x4c, bottom);
    c.set_word(0xd83a, 0x4c);
    c.cmp(W, 0x4c, top);
    if !gt {
        c.set_word(0xd834, 0x13f);
    }
    c.ret()
}
