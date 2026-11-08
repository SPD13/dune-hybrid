//! Dialogue state: the talking character's mode bytes (47C2h, 47C4h,
//! 47DCh...), the phrase pointers, and small steps of the conversation.

use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CX, DI, DX, SI, SP, SS, ZF},
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
