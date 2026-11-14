//! Dialogue state: the talking character's mode bytes (47C2h, 47C4h,
//! 47DCh...), the phrase pointers, and small steps of the conversation.

use super::pure;
use super::{B, Ctx, W, entered, game, r8};
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
/// The character talking ([47C4h]); its record is FD8h + 10h · it
/// ([`SPEAKER`]).
const TALKER: u16 = 0x47c4;
const TALKING: u16 = 0x47dc;
/// The text being shown (cleared when done).
const TEXT_PTR: u16 = 0x1bea;

/// `loc_096b5`: a dialogue step ([`code::DIALOGUE_STEP`]) on the phrase
/// at [AB84h], with the mode 80h and character 10h for the while (both kept).
pub fn step_phrase_ab84(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x96b5)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let (talker, mode) = (c.word(TALKER), c.word(MODE));
    c.push(talker);
    c.push(mode);
    c.set_word(TALKER, 0x10);
    c.set_byte(MODE, 0x80);
    let phrase = c.word(0xab84);
    c.set_reg(SI, phrase);
    c.call(code::DIALOGUE_STEP, 0x96cf)?;
    c.run(0x96cf, 3)?;
    let mode = c.pop();
    c.set_word(MODE, mode);
    let talker = c.pop();
    c.set_word(TALKER, talker);
    c.ret()
}

/// `loc_096d8`: the character AX talks: `say_sentence` 10h while [`TALKING`] is one up;
/// then the text pointer and [`TALKING`] cleared.
pub fn talk_as(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x96d8)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let talker = c.reg(AX);
    c.set_word(TALKER, talker);
    let talking = c.byte(TALKING).wrapping_add(1);
    c.set_byte(TALKING, talking);
    c.set_reg(AX, 0x10);
    c.call(0x9702, 0x96e5)?;
    c.run(0x96e5, 3)?;
    c.set_word(TEXT_PTR, 0);
    c.set_byte(TALKING, 0);
    c.ret()
}

/// `loc_0970b`: the phrase AX (words from [`PHRASES`]) set up
/// (`speaker_text_setup`), then [`code::DIALOGUE_STEP`] with the mode 20h
/// for the while.
pub fn say_phrase(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x970b)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let phrase = c.word(PHRASES.wrapping_add(c.reg(AX) << 1));
    c.set_reg(SI, phrase);
    say_with_mode_20(&mut c)
}

/// From 9716h (the end of `say_phrase` and `say_sentence`): SI's text set
/// up (`speaker_text_setup`), then a dialogue step with the mode 20h for
/// the while.
fn say_with_mode_20(c: &mut Ctx) -> Result<(), Bail> {
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

/// The phrases: words at -558Ah (AA76h), by number.
const PHRASES: u16 = 0xaa76;

/// `loc_098f5`: [1C06h], [1BF8h] and the text pointer cleared (AX 0).
pub fn clear_text_state(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x98f5)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
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

/// `loc_09985`: `two_heads_text_step` until the low 3 bits of [47CEh]
/// are 0.
pub fn finish_99be(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9985)?;
    let mut c = Ctx::new(m);
    loop {
        let done = c.word(TEXT_LEFT) & 7 == 0;
        c.run(0x9985, 3)?;
        if done {
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

/// `loc_09f1c`: the lip sync set up (`setup_lip_sync_data_from_current`),
/// bit 10h of [47D1h], a random sentence (`random_sentence`), then its
/// strings skipped (`skip_strings` with AH 0), SI noted at [47C6h].
pub fn start_9f1c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9f1c)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x9197, 0x9f1f)?;
    c.run(0x9f1f, 2)?;
    let b = c.byte(TEXT_FLAGS);
    c.set_byte(TEXT_FLAGS, b | 0x10);
    c.call(0x9a7b, 0x9f27)?;
    c.run(0x9f27, 2)?;
    c.set_reg8(r8::AH, 0);
    c.call(0x9a60, 0x9f2c)?;
    c.run(0x9f2c, 2)?;
    let at = c.reg(SI);
    c.set_word(TEXT_AT, at);
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

/// `loc_09556`: the record SI loses bit 40h of byte 0Fh, its time 2
/// stamped (`stamp_record_time` with BX 2), and its bit (number in byte
/// 0Eh) cleared in [10h] (AX the mask, CL the number).
pub fn release_record(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9556)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let record = c.reg(SI);
    let f = c.byte(record.wrapping_add(0x0f));
    c.set_byte(record.wrapping_add(0x0f), f & !0x40);
    c.set_reg(BX, 2);
    c.call(0x956d, 0x9560)?;
    c.run(0x9560, 5)?;
    let record = c.reg(SI);
    let bit = c.byte(record.wrapping_add(0x0e));
    c.set_reg8(r8::CL, bit);
    // `rol ax,cl` of FFFEh (the 8086 rotates by the count mod 16).
    let mask = 0xfffeu16.rotate_left((bit & 0x1f) as u32 % 16);
    c.set_reg(AX, mask);
    let v = c.word(0x10) & mask;
    c.set_word(0x10, v);
    c.ret()
}

/// `loc_0956d`: the record SI's time BX (word 8 + BX, written in DS) set
/// to the game time, unless its other time (BX xor 2, read in SS) is less
/// than 2 ago. AX how long ago, or now; BP the other's index.
pub fn stamp_record_time(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x956d)?;
    let mut c = Ctx::new(m);
    let (record, which) = (c.reg(SI), c.reg(BX));
    let other = which ^ 2;
    let now = game::game_time(&mut c);
    let then = c.word_in(SS, other.wrapping_add(record).wrapping_add(8));
    let ago = now.wrapping_sub(then);
    c.run(start, if ago < 2 { 7 } else { 9 })?;
    c.set_reg(BP, other);
    c.set_reg(AX, ago);
    if ago >= 2 {
        c.set_reg(AX, now);
        c.set_word(which.wrapping_add(record).wrapping_add(8), now);
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

/// `loc_0af00`: the dialogue buttons 4, 8 and 0Ch (the UI elements at
/// 2034h + them) off (bit 4000h) when there is no dialogue of that kind
/// (`scan_dialogues_a8` with BH the kind, BL 1Ch), else on. CX the last
/// kind.
pub fn mark_dialogue_buttons(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xaf00)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_reg8(r8::BH, 4);
    c.set_reg8(r8::BL, 0x1c);
    c.call(0xb147, 0xaf07)?;
    loop {
        let none = c.flag(ZF);
        let kind = c.reg8(r8::BH);
        let again = kind.wrapping_add(4) <= 0x0c;
        c.run(0xaf07, 11 + !none as u32 + if again { 2 } else { 1 })?;
        let off = if none { 0x4000 } else { 0 };
        c.set_reg(AX, off);
        c.set_reg(CX, kind as u16);
        let button = 0x2034 + kind as u16;
        c.set_reg(SI, button);
        let w = c.word(button) & !0x4000 | off;
        c.set_word(button, w);
        c.set_reg8(r8::BH, kind.wrapping_add(4));
        if !again {
            return c.ret();
        }
        c.set_reg8(r8::BL, 0x1c);
        c.call(0xb147, 0xaf07)?;
    }
}

/// `loc_0b126`: phrase AX (`get_phrase_or_command_string_si`) appended at
/// [`APPEND_AT`] (`expand_phrase`; with `format_interpolated_string` and
/// a space after unless it starts with one). SI kept.
pub fn append_phrase(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb126)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.save(&[SI]);
    let phrase = c.reg(AX);
    c.set_reg(SI, phrase);
    c.call(0xcf70, 0xb12c)?;
    c.run(0xb12c, 3)?;
    let at = c.word(APPEND_AT);
    c.set_reg(DI, at);
    c.save(&[DI]);
    c.call(0x88f1, 0xb134)?;
    let spaced = c.byte(c.reg(SI)) == 0x20;
    c.run(0xb134, if spaced { 3 } else { 4 })?;
    c.restore(&[DI]);
    if !spaced {
        c.call(0x8944, 0xb13d)?;
        c.run(0xb13d, 3)?;
        let di = c.reg(DI).wrapping_sub(1);
        c.set_reg(DI, di);
        c.set_reg8(r8::AL, 0x20);
        c.stos(B);
    }
    c.run(0xb141, 3)?;
    let end = c.reg(DI);
    c.set_word(APPEND_AT, end);
    c.restore(&[SI]);
    c.ret()
}

/// Where `append_phrase` writes next ([47BCh]).
const APPEND_AT: u16 = 0x47bc;

/// `loc_09702`: the sentence AX·8 + 4 (AX that) of [`PHRASES`], said as
/// in `say_phrase`.
pub fn say_sentence(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9702)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    let sentence = c.reg(AX) << 3 | 4;
    c.set_reg(AX, sentence);
    let phrase = c.word(PHRASES.wrapping_add(sentence << 1));
    c.set_reg(SI, phrase);
    say_with_mode_20(&mut c)
}

/// `loc_0994f`: BP = the sentence offset for [47D0h]: 0 gives a random
/// one of 6 (`loc_0e3b7`) + [0F0h], else ([47D0h] - 1)·2 (AX too).
pub fn sentence_offset(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x994f)?;
    let mut c = Ctx::new(m);
    let k = c.byte(SENTENCE_KIND);
    if k == 0 {
        c.run(start, 5)?;
        c.set_reg8(r8::AL, 0);
        c.set_reg(BX, 6);
        c.call(0xe3b7, 0x995c)?;
        c.run(0x995c, 3)?;
        let bp = c.reg(AX).wrapping_add(c.word(0xf0));
        c.set_reg(BP, bp);
        return c.ret();
    }
    c.run(start, 8)?;
    let offset = (k - 1) as u16 * 2;
    c.set_reg(AX, offset);
    c.set_reg(BP, offset);
    c.ret()
}

/// `loc_0996c`: with [`SENTENCE_KIND`] set, SI past 20h zero-ended strings
/// at ES:SI (DS = ES for the while, then SS; AL, CX 0).
pub fn skip_32_strings(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x996c)?;
    let mut c = Ctx::new(m);
    if c.byte(SENTENCE_KIND) == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let step = c.string_step(B);
    let mut at = c.reg(SI);
    // `lodsb; or al,al; jne` per byte, `loop` per string.
    let mut cost = 8;
    for _ in 0..0x20 {
        loop {
            let b = c.byte_in(ES, at);
            at = at.wrapping_add(step);
            cost += 3;
            if b == 0 {
                break;
            }
        }
        cost += 1;
    }
    c.run(start, cost)?;
    let es = c.sreg(ES);
    c.push(es);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.set_reg(SI, at);
    c.set_reg8(r8::AL, 0);
    c.set_reg(CX, 0);
    let ss = c.sreg(SS);
    c.push(ss);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}

/// Which sentence of a set ([47D0h]): 0 a random one.
const SENTENCE_KIND: u16 = 0x47d0;

/// `loc_09a60`: SI past AX zero-ended strings at ES:SI (`repne scasb`
/// each; none for AL 0), [47CEh] = 8. BX 0, AL 0. Callers read the flags
/// (its last `or` or `dec`).
pub fn skip_strings(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9a60)?;
    let mut c = Ctx::new(m);
    let count = c.reg(AX);
    if count & 0xff == 0 {
        c.run(start, 4)?;
        c.or(B, 0, 0);
        c.set_word(TEXT_LEFT, 8);
        return c.ret();
    }
    c.run(start, 7)?;
    c.set_reg(BX, count);
    c.set_reg(AX, count & 0xff00);
    c.set_reg(CX, 0xffff);
    let at = c.reg(SI);
    c.set_reg(DI, at);
    for left in (1..=count).rev() {
        // The emulator counts each of its iterations itself.
        c.m.cpu.string_op(&mut c.m.hw, 0xae, None, 0xf2);
        c.run(0x9a6f, if left == 1 { 5 } else { 3 })?;
        c.set_reg(BX, left - 1);
    }
    c.dec(W, 1);
    let end = c.reg(DI);
    c.set_reg(SI, end);
    c.set_word(TEXT_LEFT, 8);
    c.ret()
}

/// `loc_0908c`: when [0D83Ah] (AX) is past [4782h] and [479Eh] is 223Ch,
/// the driver's blit (far [38CDh]) of [22FCh] at x 140h to the back
/// buffer ([0DBDAh]), 92h - [4793h] rows (CH FFh, BP D834h).
pub fn blit_dialogue_frame(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x908c)?;
    let mut c = Ctx::new(m);
    let at = c.word(0xd83a);
    let cost = if at <= c.word(0x4782) {
        4
    } else if c.word(BOX_SHOWN) != 0x223c {
        6
    } else {
        14
    };
    c.run(start, cost)?;
    c.set_reg(AX, at);
    if cost != 14 {
        return c.ret();
    }
    let rows = c.word(0x4793);
    c.set_reg(BX, 0x92u16.wrapping_sub(rows));
    c.set_reg(DX, 0);
    c.set_reg(CX, 0xff00 | rows & 0xff);
    c.set_reg(DI, 0x140);
    let sheet = c.word(0x22fc);
    c.set_reg(SI, sheet);
    let back = c.word(0xdbda);
    c.set_sreg(ES, back);
    c.set_reg(BP, 0xd834);
    c.interp(0x90b8, 0x90bc)?;
    c.run(0x90bc, 1)?;
    c.ret()
}

/// `loc_08df0`: with bit 1 of [4799h], it is cleared when one of the
/// [`ENTRIES`] (count first, then 6-byte entries) has a first word and a
/// second of 1Eh or more. SI kept; CX the entries left, AX the last first
/// word.
pub fn check_dialogue_entries(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8df0)?;
    let mut c = Ctx::new(m);
    let flags = c.byte(0x4799);
    if flags & 1 == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let step = c.string_step(W);
    let count = c.word(ENTRIES);
    if count == 0 {
        // `loop` from 0 would go 65536 times: left to the interpreter.
        return Err(Bail);
    }
    let mut at = ENTRIES.wrapping_add(step);
    let mut cost = 6;
    let (mut found, mut left, mut first) = (false, 0, 0);
    for k in 0..count {
        first = c.word(at);
        at = at.wrapping_add(step);
        cost += 3;
        if first != 0 {
            cost += 2;
            if c.word(at) >= 0x1e {
                cost += 3;
                found = true;
                left = count - k;
                break;
            }
        }
        at = at.wrapping_add(4);
        cost += 2;
    }
    c.run(start, cost + if found { 0 } else { 2 })?;
    c.save(&[SI]);
    if found {
        c.set_byte(0x4799, flags & !1);
    }
    c.set_reg(AX, first);
    c.set_reg(CX, left);
    c.restore(&[SI]);
    c.ret()
}

/// Dialogue entries ([0A9D0h]): a count, then 6 bytes each.
const ENTRIES: u16 = 0xa9d0;

/// `loc_093df`: the speaker AL (CL) starts talking: [47BEh] = AX with
/// AL·8; its bit in [0Eh] and [14h]; its record (FD8h + 10h·AL) at
/// [`SPEAKER`] and SI for `speaker_list_item`; [47BAh], [47B6h], [19h]
/// 0, the mode 80h.
pub fn start_speaker(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x93df)?;
    let mut c = Ctx::new(m);
    c.run(start, 16)?;
    let ax = c.reg(AX);
    let speaker = ax as u8;
    c.set_reg8(r8::CL, speaker);
    c.set_word(0x47be, ax & 0xff00 | (speaker << 3) as u16);
    // `shl ax,cl` of 1 (the count mod 32).
    let shift = speaker & 0x1f;
    let bit = if shift < 16 { 1u16 << shift } else { 0 };
    for at in [0x0e, 0x14] {
        let v = c.word(at) | bit;
        c.set_word(at, v);
    }
    let record = 0x10 * speaker as u16 + 0x0fd8;
    c.set_reg(AX, record);
    c.set_word(SPEAKER, record);
    c.set_reg(SI, record);
    c.set_word(0x47ba, 0);
    c.call(0x90bd, 0x940c)?;
    c.run(0x940c, 4)?;
    c.set_word(0x47b6, 0);
    c.set_byte(MODE, 0x80);
    c.set_byte(0x19, 0);
    c.ret()
}

/// The speaker's record ([47A2h]).
const SPEAKER: u16 = 0x47a2;

/// `loc_096f1`: the character AX talks; for sentence 0Eh first the troop [4756h]
/// described (`loc_031f6`); then [`say_sentence`].
pub fn say_numbered(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x96f1)?;
    let mut c = Ctx::new(m);
    let ax = c.reg(AX);
    let described = ax & 0xff == 0x0e;
    c.run(start, if described { 5 } else { 3 })?;
    c.set_word(TALKER, ax);
    if described {
        let troop = c.word(0x4756);
        c.set_reg(SI, troop);
        c.call(0x31f6, 0x96ff)?;
        c.run(0x96ff, 1)?;
        c.set_reg(AX, 0x0e);
    }
    say_sentence(c.m, 0x9702)
}

/// `loc_09d6a`: the phrase list at SI run up to its 0 (ES the back
/// buffer, SS:[0DBD8h]): each entry (1 takes the next byte as AL, AH 1)
/// is the word AX - 2 of the table at SS:[47CCh], whose text goes to
/// `draw_dialogue_sprites` (SI kept, BP and AX the index). Callers read
/// the flags: `or al,al` on the 0.
pub fn say_phrase_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9d6a)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let back = c.word_in(SS, 0xdbd8);
    c.set_sreg(ES, back);
    loop {
        let entry = c.byte(c.reg(SI));
        if entry == 0 {
            c.run(0x9d6f, 5)?;
            c.lods(B, DS);
            c.set_reg(AX, 0);
            return c.ret();
        }
        c.run(0x9d6f, if entry == 1 { 15 } else { 13 })?;
        c.lods(B, DS);
        c.set_reg(AX, entry as u16);
        if entry == 1 {
            c.set_reg8(r8::AH, 1);
            c.lods(B, DS);
        }
        c.save(&[SI]);
        let index = c.reg(AX).wrapping_sub(2) << 1;
        c.set_reg(AX, index);
        c.set_reg(BP, index);
        let table = c.word_in(SS, 0x47cc);
        let text = table.wrapping_add(c.word(index.wrapping_add(table)));
        c.set_reg(SI, text);
        c.call(0x9d94, 0x9d90)?;
        c.run(0x9d90, 2)?;
        c.restore(&[SI]);
    }
}

/// `loc_09046`: in the picture [22FCh] ([4793h] × [2240h] bytes; ES = DS)
/// each 0 next to a 0Fh (left, right, above, below; 140h a row) becomes
/// F0h (08h with [0EAh] above 0). The 0Fh found by `repne scasb`, whose
/// flags are returned at the end. Bails at entry for an empty picture.
pub fn outline_picture(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9046)?;
    let mut c = Ctx::new(m);
    let (rows, width) = (c.word(0x4793), c.word(0x2240));
    let size = rows as u32 * width as u32;
    if size as u16 == 0 {
        return Err(Bail);
    }
    let thin = c.byte(0xea) as i8 > 0;
    c.run(start, 11 + thin as u32)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(DX, (size >> 16) as u16);
    c.set_reg(CX, size as u16);
    let picture = c.word(0x22fc);
    c.set_reg(DI, picture);
    let colour: u8 = if thin { 8 } else { 0xf0 };
    c.set_reg(AX, (colour as u16) << 8 | 0x0f);
    c.set_reg(BX, 0);
    loop {
        // The next 0Fh (the emulator counts its iterations itself).
        c.m.cpu.string_op(&mut c.m.hw, 0xae, None, 0xf2);
        if !c.flag(ZF) {
            c.run(0x9065, 2)?;
            return c.ret();
        }
        let at = c.reg(DI);
        let around = [at.wrapping_sub(2), at, at.wrapping_sub(0x141), at.wrapping_add(0x13f)];
        let zero = around.map(|p| c.byte_in(ES, p) == 0);
        let cost = 1 + (0..3).map(|i| 2 + zero[i] as u32).sum::<u32>() + 3 + if zero[3] { 2 } else { 0 };
        c.run(0x9065, cost)?;
        for (p, z) in around.into_iter().zip(zero) {
            if z {
                c.m.cpu.write8(&mut c.m.hw, ES, p, colour);
            }
        }
    }
}

/// `loc_09bac`: the lip sync set up (`setup_lip_sync_data_from_current`;
/// SI kept), then [`dialogue_frame_rect`].
pub fn redraw_dialogue_frame(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9bac)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.save(&[SI]);
    c.call(0x9197, 0x9bb0)?;
    c.run(0x9bb0, 1)?;
    c.restore(&[SI]);
    super::map::dialogue_frame_rect(c.m, 0x9bb1)
}

/// `loc_09f40`: for the character [`TALKER`] (number 2 with [0C2h] 4
/// first `sum_three_records`; AX, SI kept): its record (FD8h + 10h·it)
/// at [`SPEAKER`] (AX); with the map not up ([`MAP_MODE`] 0) the front
/// buffer active and the text box at (28h, 10h), 10h × 10h; then
/// [`tall_font_colour_f0`]. CL 10h.
pub fn speaker_text_setup(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9f40)?;
    let mut c = Ctx::new(m);
    let talker = c.word(TALKER);
    let k = c.byte(0xc2);
    let sums = talker == 2 && k == 4;
    c.run(start, if talker != 2 { 3 } else if !sums { 5 } else { 8 })?;
    c.set_reg(AX, talker);
    if sums {
        c.save(&[AX, SI]);
        c.call(0x1243, 0x9f54)?;
        c.run(0x9f54, 2)?;
        c.restore(&[AX, SI]);
    }
    let map_up = c.byte(MAP_MODE) != 0;
    c.run(0x9f56, if map_up { 6 } else { 7 })?;
    c.set_reg8(r8::CL, 0x10);
    let record = (c.reg(AX) & 0xff) * 0x10 + 0x0fd8;
    c.set_reg(AX, record);
    c.set_word(SPEAKER, record);
    if !map_up {
        c.call(0xc07c, 0x9f6a)?;
        c.run(0x9f6a, 4)?;
        c.set_word(0x4784, 0x28);
        c.set_word(0x4786, 0x10);
        c.set_word(0x4788, 0x10);
        c.set_word(0x478a, 0x10);
    }
    tall_font_colour_f0(c.m, 0x9f82)
}

/// The map's mode ([46EBh]): 0 when not up.
const MAP_MODE: u16 = 0x46eb;

/// `loc_09908`: the dialogue text: SI = [47CAh] + the entry at
/// `sentence_offset`'s BP of the table at ES = [0DBB2h], past
/// [`SENTENCE_KIND`]'s strings (`skip_32_strings`); [47D1h] C0h, [47CEh]
/// [478Ch]·4 (AX). [47C8h] = SI, swapped into [47C6h]: when that was
/// empty (SI 0), with the map not up ([0EAh] at most 0), `loc_0127c` for
/// [`TALKER`] (AX), and unless CF the frame task `two_heads_text_step`
/// (BP 10h). Callers read the flags of its exits.
pub fn start_dialogue_text(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9908)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let table = c.word(0x47ca);
    c.set_reg(SI, table);
    let texts = c.word(0xdbb2);
    c.set_sreg(ES, texts);
    c.call(0x994f, 0x9913)?;
    c.run(0x9913, 8)?;
    c.set_byte(TEXT_FLAGS, 0xc0);
    let n = (c.byte(0x478c) as u16) << 2;
    c.set_reg(AX, n);
    c.set_word(TEXT_LEFT, n);
    let (si, bp) = (c.reg(SI), c.reg(BP));
    let text = si.wrapping_add(c.word_in(ES, bp.wrapping_add(si)));
    c.set_reg(SI, text);
    c.call(0x996c, 0x992a)?;
    let text = c.reg(SI);
    let previous = c.word(TEXT_AT);
    if previous != 0 {
        c.run(0x992a, 5)?;
        c.set_word(0x47c8, text);
        c.set_word(TEXT_AT, text);
        c.set_reg(SI, previous);
        c.or(W, previous, previous);
        return c.ret();
    }
    let k = c.byte(0xea);
    let map_up = k as i8 > 0;
    c.run(0x992a, if map_up { 7 } else { 8 })?;
    c.set_word(0x47c8, text);
    c.set_word(TEXT_AT, text);
    c.set_reg(SI, 0);
    if map_up {
        c.cmp(B, k as u16, 0);
        return c.ret();
    }
    let talker = c.word(TALKER);
    c.set_reg(AX, talker);
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

/// `loc_094f3`: for a [`TALKER`] below 10h: [18h] = byte 0Fh of its
/// record [`SPEAKER`], [16h] (AX) = the game time - its word 8 (0Ah
/// without bit 40h of that byte); SI kept. Then before phase 64h, for one
/// below 9, `loc_02e98` for the location [11DBh] if any (DI).
pub fn speaker_mood(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x94f3)?;
    let mut c = Ctx::new(m);
    let talker = c.word(TALKER);
    if talker >= 0x10 {
        c.run(start, 3)?;
        return c.ret();
    }
    let record = c.word(SPEAKER);
    let flags = c.byte(record.wrapping_add(0x0f));
    let near = flags & 0x40 == 0;
    c.run(start, 13 + near as u32)?;
    c.save(&[SI]);
    c.set_reg(SI, record);
    c.set_byte(0x18, flags);
    let then = c.word(record.wrapping_add(if near { 0x0a } else { 8 }));
    let since = game::game_time(&mut c).wrapping_sub(then);
    c.set_reg(AX, since);
    c.set_word(0x16, since);
    c.restore(&[SI]);
    if c.byte(super::globals::GAME_PHASE) >= 0x64 {
        c.run(0x9519, 3)?;
        return c.ret();
    }
    if talker >= 9 {
        c.run(0x9519, 5)?;
        return c.ret();
    }
    let place = c.word(0x11db);
    c.run(0x9519, 8)?;
    c.set_reg(DI, place);
    if place == 0 {
        return c.ret();
    }
    c.call(0x2e98, 0x9532)?;
    c.run(0x9532, 1)?;
    c.ret()
}

/// `loc_09f31`: SI = the sentence AX of the speaker [47BEh] (its group of
/// 8) from [`PHRASES`], then [`speaker_text_setup`].
pub fn speaker_sentence(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9f31)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let sentence = (c.word(0x47be) & 0xfff8).wrapping_add(c.reg(AX));
    let text = c.word(PHRASES.wrapping_add(sentence << 1));
    c.set_reg(SI, text);
    speaker_text_setup(c.m, 0x9f40)
}

/// `loc_09b09`: for a sentence record [`NUMBER_RECORD`] of 38h or more:
/// the digits of [20h] (of [9Dh] when the record is not exactly 38h) by
/// `decimal_digits` (queued in BL, DH, DL, CH, CL, FFh after the last),
/// each stored at [0DBB0h] (DI, on) as the word 5 + it (6 + it from the
/// first non-zero one on) of the table there (ES = [0DBB2h]).
pub fn number_words(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9b09)?;
    let mut c = Ctx::new(m);
    let record = c.word(NUMBER_RECORD);
    let kind = c.word(record);
    if kind < 0x38 {
        c.run(start, 4)?;
        c.set_reg(SI, record);
        return c.ret();
    }
    c.run(start, if kind == 0x38 { 6 } else { 8 })?;
    c.set_reg(SI, record);
    let number = if kind == 0x38 { c.word(0x20) } else { c.byte(0x9d) as u16 };
    c.set_reg(AX, number);
    c.call(0x8a23, 0x9b1f)?;
    c.run(0x9b1f, 4)?;
    let ax = c.reg(AX);
    c.set_reg(CX, ax);
    let (table, texts) = (c.word(0xdbb0), c.word(0xdbb2));
    c.set_reg(SI, table);
    c.set_sreg(ES, texts);
    c.set_reg(DI, table);
    let mut base = 5u16;
    c.set_reg(BP, base);
    loop {
        let digit = c.reg(BX) & 0x0f;
        c.run(0x9b2a, if digit != 0 { 14 } else { 13 })?;
        if digit != 0 {
            base = 6;
            c.set_reg(BP, base);
        }
        let index = (digit + base) << 1;
        let word = c.word_in(ES, index.wrapping_add(table));
        c.set_reg(AX, word);
        c.stos(W);
        // The next digit: BL ← DH ← DL ← CH ← CL ← FFh.
        let (cx, dx) = (c.reg(CX), c.reg(DX));
        let next = dx >> 8;
        c.set_reg(BX, index & 0xff00 | next);
        c.set_reg(DX, (dx & 0xff) << 8 | cx >> 8);
        c.set_reg(CX, (cx & 0xff) << 8 | 0xff);
        if next == 0xff {
            break;
        }
    }
    c.run(0x9b48, 1)?;
    c.ret()
}

/// The record of the sentence's number part ([47E4h]): its word 0 the
/// kind (38h and up: a number to say), then four words for
/// `font_draw_interpolated_string`.
const NUMBER_RECORD: u16 = 0x47e4;
/// The number part's state ([47E1h]): 0 none, bit 80h begun, bit 1 to
/// draw; [47E2h] its entry.
const NUMBER_PART: u16 = 0x47e1;

/// `loc_09ab4`: the sentence's number part ([`NUMBER_PART`], AL): when set
/// but not yet begun, it begins (bit 80h): `number_words`, SI the entry
/// [47E2h] (AX, BP) of the text [47CAh] (table ES), [47CEh] 14h, CF set.
/// Begun with bit 1: [`NUMBER_PART`] = 80h, `loc_0d075`, and for a record
/// below 38h its four words to `font_draw_interpolated_string` (CX, DX,
/// BX; AX the first), the tall font, `loc_0dbb2`, `loc_0c4dd`. CF clear
/// but where it begins (callers read CF).
pub fn sentence_number_part(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9ab4)?;
    let mut c = Ctx::new(m);
    let part = c.byte(NUMBER_PART);
    if part == 0 {
        c.run(start, 4)?;
        c.set_reg8(r8::AL, 0);
        c.set_flag(pc::cpu::CF, false);
        return c.ret();
    }
    if part & 0x80 == 0 {
        c.run(start, 6)?;
        c.set_reg8(r8::AL, part);
        c.set_byte(NUMBER_PART, part | 0x80);
        c.call(0x9b09, 0x9ac5)?;
        c.run(0x9ac5, 8)?;
        let entry = c.byte(NUMBER_PART + 1) as u16;
        c.set_reg(AX, entry);
        c.set_reg(BP, entry);
        let base = c.word(0x47ca);
        let text = base.wrapping_add(c.word_in(ES, entry.wrapping_add(base)));
        c.set_reg(SI, text);
        c.set_word(TEXT_LEFT, 0x14);
        c.set_flag(pc::cpu::CF, true);
        return c.ret();
    }
    let draws = part & 1 != 0;
    c.run(start, if draws { 8 } else { 7 })?;
    c.set_reg8(r8::AL, part >> 1);
    if !draws {
        c.set_flag(pc::cpu::CF, false);
        return c.ret();
    }
    c.set_byte(NUMBER_PART, 0x80);
    c.call(0xd075, 0x9ae7)?;
    let record = c.word(NUMBER_RECORD);
    let kind = c.word(record);
    c.run(0x9ae7, if kind >= 0x38 { 4 } else { 12 })?;
    c.set_reg(SI, record);
    if kind >= 0x38 {
        c.set_flag(pc::cpu::CF, false);
        return c.ret();
    }
    // Its four words (`lodsw` each) after the kind.
    let mut words = [0u16; 4];
    for w in &mut words {
        *w = c.lods(W, DS);
    }
    c.set_reg(CX, words[3]);
    c.set_reg(DX, words[1]);
    c.set_reg(BX, words[2]);
    c.set_reg(AX, words[0]);
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

/// `loc_09b49`: when the number part has begun ([`NUMBER_PART`] 80h): the
/// lip sync set up (`setup_lip_sync_data_from_current`; SI kept), SI the
/// entry [47E2h] + 2 (AX, BP) of the text [47CAh] (table ES =
/// [0DBB2h]), [`NUMBER_PART`] = 81h, and the frame tasks run
/// (`wait_processing_frame_tasks_interruptable`, BP 99F6h, AX 0Ch; ES
/// kept) until the text at [47C6h] ends (FFh). [`NUMBER_PART`] cleared
/// (with [47E2h]).
pub fn speak_number_part(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9b49)?;
    let mut c = Ctx::new(m);
    let part = c.word(NUMBER_PART);
    if part & 0xff != 0x80 {
        c.run(start, 5)?;
        c.set_reg(AX, part);
        c.set_word(NUMBER_PART, 0);
        return c.ret();
    }
    c.run(start, 6)?;
    c.set_reg(AX, part);
    c.save(&[SI]);
    c.push(part);
    c.call(0x9197, 0x9b55)?;
    c.run(0x9b55, 10)?;
    let part = c.pop();
    let entry = (part >> 8).wrapping_add(2);
    c.set_reg(AX, entry);
    c.set_reg(BP, entry);
    let base = c.word(0x47ca);
    let texts = c.word(0xdbb2);
    c.set_sreg(ES, texts);
    let text = base.wrapping_add(c.word_in(ES, entry.wrapping_add(base)));
    c.set_reg(SI, text);
    c.set_byte(NUMBER_PART, 0x81);
    loop {
        c.run(0x9b6e, 4)?;
        let es = c.sreg(ES);
        c.push(es);
        c.set_reg(BP, 0x99f6);
        c.set_reg(AX, 0x0c);
        c.call(0xe353, 0x9b78)?;
        let es = c.word_in(SS, c.reg(SP));
        let at = c.word(TEXT_AT);
        let ended = c.byte_at_seg(es, at) == 0xff;
        c.run(0x9b78, 4)?;
        c.pop();
        c.set_sreg(ES, es);
        c.set_reg(SI, at);
        if ended {
            break;
        }
    }
    c.run(0x9b83, 3)?;
    c.restore(&[SI]);
    c.set_word(NUMBER_PART, 0);
    c.ret()
}

/// `loc_09a7b`: SI the text of the sentence [`SENTENCE_KIND`] - 1 (else
/// 5, 6 for character 7 from phase C8h, with BX 0F38h for 0F18h) plus
/// [0F0h] (BP the entry, AX its offset) of [47CAh] in ES = [0DBB2h]; then
/// `rand_masked` with BX.
pub fn random_sentence(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9a7b)?;
    let mut c = Ctx::new(m);
    let kind = c.byte(SENTENCE_KIND);
    let talker = c.word(TALKER);
    let late = c.byte(super::globals::GAME_PHASE) >= 0xc8;
    let cost = if kind != 0 {
        4
    } else if talker != 7 {
        8
    } else if !late {
        10
    } else {
        11
    };
    c.run(start, cost + 9)?;
    let sentence = if kind != 0 {
        c.set_reg(BX, 0x0f18);
        kind
    } else {
        c.set_reg(BX, 0x0f38);
        if talker == 7 && late { 6 } else { 5 }
    };
    let offset = (sentence - 1) as u16 * 2;
    c.set_reg(AX, offset);
    let entry = offset.wrapping_add(c.word(0xf0));
    c.set_reg(BP, entry);
    let base = c.word(0x47ca);
    let texts = c.word(0xdbb2);
    c.set_sreg(ES, texts);
    let text = base.wrapping_add(c.word_in(ES, entry.wrapping_add(base)));
    c.set_reg(SI, text);
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
    let shown = c.word(BOX_SHOWN);
    if shown < 2 {
        c.run(0x8c8a, 5)?;
        c.set_word(BOX_SHOWN, 0);
        c.set_reg(AX, shown);
        // `cmp ax,2`: its flags are returned (callers read them).
        c.cmp(W, shown, 2);
        return c.ret();
    }
    let on_map = c.byte(MAP_SCREEN) != 0;
    c.run(0x8c8a, if on_map { 10 } else { 8 })?;
    c.set_word(BOX_SHOWN, 0);
    c.set_reg(AX, shown);
    c.set_reg(SI, 0x1470);
    if on_map {
        // The map picture back under the box (the driver's far [391Dh]).
        c.set_reg(BP, 0x1be2);
        c.set_reg(SI, 0x4c60);
        let es = c.word(0xdbde);
        c.set_sreg(ES, es);
        c.interp(0x8ca9, 0x8cad)?;
        c.run(0x8cad, 3)?;
        c.set_reg(SI, 0x1be2);
        c.set_word(TEXT_PTR, 0);
    }
    c.call(0xc446, 0x8cb8)?;
    let text = c.word(0x47c8);
    c.run(0x8cb8, if text != 0 { 5 } else { 4 })?;
    c.set_reg(SI, text);
    if text != 0 {
        c.set_word(0x4540, 0);
        c.call(0x9bac, 0x8cc9)?;
        c.run(0x8cc9, 1)?;
    }
    c.call(0xc4dd, 0x8ccc)?;
    c.run(0x8ccc, 1)?;
    c.ret()
}

/// The dialogue box up ([479Eh]): 2 or more when drawn.
const BOX_SHOWN: u16 = 0x479e;
/// The map screen's mode ([28E7h]): 0 off.
const MAP_SCREEN: u16 = 0x28e7;

/// `loc_0a0f1`: on the map screen in mode 2, a record SI with bit 10h of
/// byte 2 makes it mode 1; one without closes the dialogue box
/// ([`close_dialogue_box`]). Callers read the flags of its tests.
pub fn map_mode_2_record(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa0f1)?;
    let mut c = Ctx::new(m);
    let mode = c.byte(MAP_SCREEN);
    if mode != 2 {
        c.run(start, 3)?;
        c.cmp(B, mode as u16, 2);
        return c.ret();
    }
    let b = c.byte(c.reg(SI).wrapping_add(2));
    if b & 0x10 != 0 {
        c.run(start, 6)?;
        c.test(B, b as u16, 0x10);
        c.set_byte(MAP_SCREEN, 1);
        return c.ret();
    }
    c.run(start, 5)?;
    close_box_at(&mut c)
}

/// `loc_08e9e`: a line's spacing stored at ES:DI (3 words): DL (its gaps
/// + 1), then BX / (DL - 1) + 6 and the remainder (BX + 6 and 0 for DL 1;
/// 0, 0 for DL 0). DL added to [478Ch]; DH one more, DL 0, BX = [478Fh].
pub fn store_spacing(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8e9e)?;
    let mut c = Ctx::new(m);
    let (dx, width) = (c.reg(DX), c.reg(BX));
    let count = dx & 0xff;
    let (each, rest) = if count == 0 {
        (0, 0)
    } else if count == 1 {
        (width.wrapping_add(6), 0)
    } else {
        let gaps = count - 1;
        ((width / gaps).wrapping_add(6), width % gaps)
    };
    c.run(start, if count == 0 { 11 } else if count == 1 { 22 } else { 23 })?;
    for w in [count, each, rest] {
        c.set_reg(AX, w);
        c.stos(W);
    }
    let n = c.byte(0x478c).wrapping_add(count as u8);
    c.set_byte(0x478c, n);
    c.set_reg(DX, (((dx >> 8) as u8).wrapping_add(1) as u16) << 8);
    let next = c.word(0x478f);
    c.set_reg(BX, next);
    if count != 0 {
        // The `push dx` / `pop dx` around the division leaves its word.
        c.below_sp(1, dx);
    }
    c.ret()
}

/// `loc_09b8b`: the dialogue text dropped: `loc_0a7a5`, [47C3h], [47CEh]
/// 0, bit 80h of [47D1h] cleared, [47C6h] taken (cleared, AX); when there
/// was one, its frame task `two_heads_text_step` removed. Callers read the
/// flags (`or ax,ax`).
pub fn drop_dialogue_text(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9b8b)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xa7a5, 0x9b8e)?;
    let text = c.word(TEXT_AT);
    c.run(0x9b8e, if text != 0 { 9 } else { 8 })?;
    c.set_byte(0x47c3, 0);
    c.set_word(TEXT_LEFT, 0);
    let b = c.byte(TEXT_FLAGS);
    c.set_byte(TEXT_FLAGS, b & 0x7f);
    c.set_word(TEXT_AT, 0);
    c.set_reg(AX, text);
    // `or ax,ax`: its flags are returned, also through
    // `remove_frame_task` when that finds no task.
    c.or(W, text, text);
    if text == 0 {
        return c.ret();
    }
    c.set_reg(SI, 0x99be);
    super::tasks::remove_frame_task(c.m, 0xda5f)
}

/// `loc_09655`: the speaker SI's number (byte 0Eh, CL) taken off the two
/// gauges at [`GAUGES`]: the second becomes FFh, or the first takes the
/// second's and that becomes FFh; its marker (+10D0h) cleared and the
/// gauges redrawn ([`draw_gauges`]). DI the gauge, AL its new value.
pub fn drop_speaker_gauge(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9655)?;
    let mut c = Ctx::new(m);
    let speaker = c.byte(c.reg(SI).wrapping_add(0x0e));
    let (first, second) = (c.byte(GAUGES), c.byte(GAUGES + 1));
    c.set_reg8(r8::CL, speaker);
    if second != speaker && first != speaker {
        c.run(start, 9)?;
        c.set_reg(DI, GAUGES);
        c.set_reg8(r8::AL, 0xff);
        return c.ret();
    }
    let in_second = second == speaker;
    c.run(start, if in_second { 8 } else { 12 })?;
    let (gauge, value) = if in_second {
        (GAUGES + 1, 0xff)
    } else {
        c.set_byte(GAUGES + 1, 0xff);
        (GAUGES, second)
    };
    c.set_reg(DI, gauge);
    c.set_reg8(r8::AL, value);
    c.set_byte(gauge, value);
    c.set_byte(gauge.wrapping_add(0x10d0), 0);
    super::screens::draw_gauges(c.m, 0xd763)
}

/// The two speakers' gauges ([1152h], [1153h]; FFh none).
const GAUGES: u16 = 0x1152;

/// `loc_098e6`: `clear_text_state` (AX 0) into [47C8h], [47AAh] and
/// [`BOX_SHOWN`], then [`drop_dialogue_text`].
pub fn reset_dialogue(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x98e6)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x98f5, 0x98e9)?;
    c.run(0x98e9, 4)?;
    let ax = c.reg(AX);
    for at in [0x47c8u16, 0x47aa, BOX_SHOWN] {
        c.set_word(at, ax);
    }
    drop_dialogue_text(c.m, 0x9b8b)
}

/// `sub_30b9`: for a speaker record SI without bit 40h of byte 0Fh (the
/// first such becomes [47AAh]): [`speaker_entries_at`].
pub fn add_speaker(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x30b9)?;
    let mut c = Ctx::new(m);
    if c.byte(c.reg(SI).wrapping_add(0x0f)) & 0x40 != 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let first = c.word(0x47aa) == 0;
    speaker_entries_at(&mut c, start, 4 + first as u32, |c| {
        if first {
            let record = c.reg(SI);
            c.set_word(0x47aa, record);
        }
    })
}

/// `sub_3120`: [`speaker_entries_at`] for a speaker SI with bit 40h of
/// byte 0Fh.
pub fn add_speaker_40(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3120)?;
    let mut c = Ctx::new(m);
    if c.byte(c.reg(SI).wrapping_add(0x0f)) & 0x40 == 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    speaker_entries_at(&mut c, start, 2, |_| {})
}

/// From `loc_030ca` (`pre` instructions before, done by `head`): the
/// speaker SI's entries stored at ES:DI, 2 words each: its phrase (78h +
/// its number, byte 0Eh; AL, CL the number) and its place (word 4); its
/// bit in [12h]. For speaker 0Fh also [476Ah] - 1 more (phrases 88h on,
/// the same place; CX the count), and from phase 5 the entry [476Bh] - 1
/// - [476Ah] (signed, from DI) made phrase 8Fh.
fn speaker_entries_at(c: &mut Ctx, at: u16, pre: u32, head: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    let record = c.reg(SI);
    let speaker = c.byte(record.wrapping_add(0x0e));
    let place = c.word(record.wrapping_add(4));
    let (k, last) = (c.byte(0x476a), c.byte(0x476b));
    let phase = c.byte(super::globals::GAME_PHASE);
    let more = (k as u16).wrapping_sub(1);
    let extra = if (more as i16) > 0 { more } else { 0 };
    let mut cost = pre + 12;
    if speaker != 0x0f {
        cost += 1;
    } else {
        cost += 6 + if extra > 0 { 4 + 6 * extra as u32 } else { 0 };
        cost += if phase < 5 { 1 } else if last == 0 { 4 } else { 13 };
    }
    c.run(at, cost)?;
    head(c);
    c.set_reg8(r8::AL, speaker);
    c.set_reg8(r8::CL, speaker);
    c.set_reg(AX, 0x78 + speaker as u16);
    c.stos(W);
    // `shl ax,cl` of 1 (the count mod 32).
    let shift = speaker & 0x1f;
    let bit = if shift < 16 { 1u16 << shift } else { 0 };
    let v = c.word(0x12) | bit;
    c.set_word(0x12, v);
    c.set_reg(AX, place);
    c.stos(W);
    if speaker != 0x0f {
        return c.ret();
    }
    c.set_reg(CX, more);
    if extra > 0 {
        c.save(&[SI]);
        c.set_reg(SI, place);
        for i in 0..extra {
            let phrase = 0x88 + i;
            c.set_reg(AX, phrase);
            c.stos(W);
            c.set_reg(AX, place);
            c.stos(W);
            c.set_reg(AX, phrase);
            c.set_reg(CX, extra - 1 - i);
        }
        c.restore(&[SI]);
    }
    if phase < 5 {
        return c.ret();
    }
    c.set_reg8(r8::AL, last);
    if last == 0 {
        return c.ret();
    }
    c.save(&[DI]);
    let slot = ((last.wrapping_sub(1).wrapping_sub(k) as i8 as i16) << 2) as u16;
    c.set_reg(AX, slot);
    let di = c.reg(DI);
    c.set_word(di.wrapping_add(slot), 0x8f);
    c.restore(&[DI]);
    c.ret()
}

/// `loc_09cc6`: the sprite entry at SI (sprite, x, y bytes; placed at
/// 1BF0h's x, y) added to the bounding rect at SS:[`RECT_D834`]; its size
/// from the sprite table [0DBB0h] in [0DBB2h] (width 9 bits, height a
/// byte). DX, BX its far corner; AX the height. SI, DS kept.
pub fn grow_dialogue_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9cc6)?;
    let mut c = Ctx::new(m);
    let entry = c.reg(SI);
    let step = c.string_step(B);
    let sprite = c.byte(entry);
    let x = (c.byte(entry.wrapping_add(step)) as u16).wrapping_add(c.word(0x1bf0));
    let y = (c.byte(entry.wrapping_add(step.wrapping_mul(2))) as u16).wrapping_add(c.word(0x1bf2));
    let (table, seg) = (c.word(0xdbb0), c.word(0xdbb2));
    let index = (sprite as u16).wrapping_sub(1).wrapping_shl(1);
    let header = table.wrapping_add(c.word_at(seg, index.wrapping_add(table)));
    let width = c.word_at(seg, header) & 0x1ff;
    let height = c.word_at(seg, header.wrapping_add(c.string_step(W))) & 0xff;
    let (x1, y1) = (x.wrapping_add(width), y.wrapping_add(height));
    let rect: [u16; 4] = std::array::from_fn(|k| c.word_in(SS, RECT_D834 + 2 * k as u16));
    let grows = [rect[0] >= x, rect[1] >= y, rect[2] < x1, rect[3] < y1];
    c.run(start, 33 + grows.iter().map(|&g| g as u32).sum::<u32>())?;
    c.save(&[SI]);
    let ds = c.sreg(DS);
    c.push(ds);
    for (k, v) in [x, y, x1, y1].into_iter().enumerate() {
        if grows[k] {
            c.set_word_in(SS, RECT_D834 + 2 * k as u16, v);
        }
    }
    c.set_reg(DX, x1);
    c.set_reg(BX, y1);
    c.set_reg(AX, height);
    c.set_reg(BP, RECT_D834);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.restore(&[SI]);
    c.ret()
}

/// The dialogue's bounding rect: x0, y0, x1, y1 words in SS.
const RECT_D834: u16 = 0xd834;

/// `loc_09673`: the speaker SI's number (byte 0Eh, CL) put on a gauge
/// ([`GAUGES`]; the first free, DI). Nothing when it has one already.
/// With both taken by others, the first one's speaker leaves: [23h] its
/// number + 64h (AL), `release_record`, and the second moves down. Its
/// marker (+10D0h) 10h and the gauges redrawn ([`draw_gauges`]).
pub fn add_speaker_gauge(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9673)?;
    let mut c = Ctx::new(m);
    let speaker = c.byte(c.reg(SI).wrapping_add(0x0e));
    let (first, second) = (c.byte(GAUGES), c.byte(GAUGES + 1));
    c.set_reg8(r8::CL, speaker);
    if first == speaker {
        c.run(start, 5)?;
        c.set_reg(DI, GAUGES);
        return c.ret();
    }
    let gauge = if first == 0xff {
        c.run(start, 9)?;
        GAUGES
    } else if second == speaker {
        c.run(start, 10)?;
        c.set_reg(DI, GAUGES + 1);
        return c.ret();
    } else if second == 0xff {
        c.run(start, 14)?;
        GAUGES + 1
    } else {
        c.run(start, 22)?;
        c.set_reg(DI, GAUGES);
        c.save(&[CX]);
        c.set_reg8(r8::CL, first);
        let leaving = 0x10 * first as u16 + 0x0fd8;
        c.set_reg(SI, leaving);
        let note = c.byte(leaving.wrapping_add(0x0e)).wrapping_add(0x64);
        c.set_reg(AX, leaving & 0xff00 | note as u16);
        c.set_byte(0x23, note);
        c.call(0x9556, 0x96a4)?;
        c.run(0x96a4, 7)?;
        c.restore(&[CX]);
        let moved = c.byte(GAUGES + 1);
        c.set_reg8(r8::AL, moved);
        c.set_byte(GAUGES, moved);
        GAUGES + 1
    };
    c.set_reg(DI, gauge);
    let speaker = c.reg8(r8::CL);
    c.set_byte(gauge, speaker);
    c.set_byte(gauge.wrapping_add(0x10d0), 0x10);
    super::screens::draw_gauges(c.m, 0xd763)
}

/// `loc_08e16`: the text at SI broken into lines of the width [478Fh]
/// (BX the room left, DL the words on the line, DH the lines): each line's
/// spacing stored from [`ENTRIES`] + 2 on (`store_spacing`). CR ends a
/// line (its entry then 6, 0), spaces are skipped, a word takes its width
/// (`word_width`, CX) + 6; one that doesn't fit goes on the next line, or
/// with the room only short by its trailing 6, ends this one. At the end
/// (a byte with bit 7) the last line, an empty entry, and the line count
/// at [`ENTRIES`]; C8h there for a word that fits no line. ES = DS.
pub fn break_lines(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8e16)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_byte(0x478c, 0);
    c.set_reg(DI, ENTRIES + 2);
    let width = c.word(0x478f);
    c.set_reg(BX, width);
    c.set_reg(DX, 0);
    let words_on_line = |c: &Ctx| c.reg(DX) & 0xff;
    let add_word = |c: &mut Ctx| {
        let dx = c.reg(DX);
        c.set_reg(DX, dx & 0xff00 | (dx as u8).wrapping_add(1) as u16);
    };
    loop {
        let ch = c.byte(c.reg(SI));
        if ch & 0x80 != 0 {
            c.run(0x8e28, 3)?;
            c.set_reg8(r8::AL, ch);
            break;
        }
        if ch == 0x0d {
            c.run(0x8e28, 6)?;
            c.set_reg8(r8::AL, ch);
            c.call(0x8e9e, 0x8e3c)?;
            c.run(0x8e3c, 5)?;
            let di = c.reg(DI);
            c.set_word(di.wrapping_sub(4), 6);
            c.set_word(di.wrapping_sub(2), 0);
            let dx = c.reg(DX);
            c.set_reg(DX, dx & 0xff00);
            let si = c.reg(SI).wrapping_add(1);
            c.set_reg(SI, si);
            continue;
        }
        if ch == 0x20 {
            c.run(0x8e28, 9)?;
            c.set_reg8(r8::AL, ch);
            let si = c.reg(SI).wrapping_add(1);
            c.set_reg(SI, si);
            continue;
        }
        c.run(0x8e28, 8)?;
        c.set_reg8(r8::AL, ch);
        c.call(0x8ed3, 0x8e4e)?;
        let word = c.reg(CX);
        if word == 0 {
            c.run(0x8e4e, 2)?;
            continue;
        }
        c.run(0x8e4e, 3)?;
        let need = word.wrapping_add(6);
        c.set_reg(CX, need);
        // Place it: on this line, or end the line and try again.
        loop {
            let room = c.reg(BX);
            if room >= need {
                c.run(0x8e55, 4)?;
                c.set_reg(BX, room - need);
                add_word(&mut c);
                break;
            }
            let short = room.wrapping_sub(need).wrapping_add(6);
            if short & 0x8000 == 0 {
                // Only its trailing space doesn't fit: it ends the line.
                c.run(0x8e55, 6)?;
                c.set_reg(BX, short);
                add_word(&mut c);
                c.call(0x8e9e, 0x8e67)?;
                c.run(0x8e67, 1)?;
                break;
            }
            c.run(0x8e55, 8)?;
            if words_on_line(&c) == 0 {
                c.set_reg(BX, short);
                c.set_word(ENTRIES, 0xc8);
                return c.ret();
            }
            c.set_reg(BX, short.wrapping_add(need));
            c.call(0x8e9e, 0x8e72)?;
            c.run(0x8e72, 1)?;
        }
    }
    let last_line = words_on_line(&c) != 0;
    c.run(0x8e74, if last_line { 3 } else { 2 })?;
    if last_line {
        c.call(0x8e9e, 0x8e7b)?;
    }
    c.run(0x8e7b, 8)?;
    let di = c.reg(DI);
    for (k, v) in [(-4i16, 6u16), (-2, 0), (0, 0), (2, 0)] {
        c.set_word(di.wrapping_add(k as u16), v);
    }
    let lines = c.reg(DX) >> 8;
    c.set_reg(DX, lines);
    c.set_word(ENTRIES, lines);
    c.ret()
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
    c.set_byte(SENTENCE_KIND, ah);
    c.set_reg(AX, al as u16);
    c.ret()
}

/// `open_talking_head_resource`: resource AX + 2 opened
/// (`open_resource_by_index`).
pub fn open_talking_head_resource(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x920f)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let resource = c.reg(AX).wrapping_add(2);
    c.set_reg(AX, resource);
    super::resources::open_resource_by_index(c.m, 0xc13e)
}

/// `loc_098b2`: unless [47C3h], the dialogue text closed: [4540h] 0, bits
/// C0h of [47D1h] cleared, [47C8h] taken (AX); when there was one, the
/// text frame 1BF0h emptied (its word 8 and [1C06h] 0) and redrawn
/// (`loc_0c446`, `draw_head_if_needed_and_update_screen_rect_at_si`),
/// then [`drop_dialogue_text`].
pub fn close_dialogue_text(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x98b2)?;
    let mut c = Ctx::new(m);
    if c.byte(0x47c3) != 0 {
        c.run(start, 3)?;
        return c.ret();
    }
    let text = c.word(0x47c8);
    c.run(start, if text == 0 { 9 } else { 12 })?;
    c.set_word(0x4540, 0);
    let b = c.byte(TEXT_FLAGS);
    c.set_byte(TEXT_FLAGS, b & 0x3f);
    c.set_word(0x47c8, 0);
    c.set_reg(AX, text);
    if text == 0 {
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

/// `loc_0978e`: the dialogue scene redrawn: `loc_04aca`; for a character
/// talking ([`TALKER`] not FFFFh, AX): `loc_091a0`, `start_dialogue_text`,
/// the box 1BE2h (`loc_0c477`) while [`BOX_SHOWN`], the text frame
/// ([47C8h]: [4540h] 0, `redraw_dialogue_frame`; `loc_09025` for the box
/// 223Ch); then the palette (`loc_0c0f4`) and [`sub_c4dd`]. Callers read
/// the flags.
pub fn redraw_dialogue_scene(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x978e)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x4aca, 0x9791)?;
    let talker = c.word(TALKER);
    c.run(0x9791, 4)?;
    c.set_reg(AX, talker);
    if talker == 0xffff {
        c.cmp(W, talker, 0xffff);
        return c.ret();
    }
    c.call(0x91a0, 0x979c)?;
    c.run(0x979c, 1)?;
    c.call(0x9908, 0x979f)?;
    let boxed = c.word(BOX_SHOWN) != 0;
    c.run(0x979f, if boxed { 4 } else { 2 })?;
    if boxed {
        c.set_reg(SI, 0x1be2);
        c.call(0xc477, 0x97ac)?;
    }
    let text = c.word(0x47c8);
    c.run(0x97ac, if text != 0 { 5 } else { 4 })?;
    c.set_reg(SI, text);
    if text != 0 {
        c.set_word(0x4540, 0);
        c.call(0x9bac, 0x97bd)?;
        let big = c.word(BOX_SHOWN) == 0x223c;
        c.run(0x97bd, 3)?;
        if big {
            c.call(0x9025, 0x97c8)?;
            c.run(0x97c8, 1)?;
        }
    }
    c.call(0xc0f4, 0x97cb)?;
    c.run(0x97cb, 1)?;
    super::draw::sub_c4dd(c.m, 0xc4dd)
}

/// `loc_099da` (a frame task): the dialogue text's next step, after the
/// lip sync is set up (`setup_lip_sync_data_from_current`). Only while the
/// text runs (bit 80h of [`TEXT_FLAGS`], AL). Its phrases are at
/// [`TEXT_AT`] in [0DBB2h], [`TEXT_LEFT`] of them to go:
/// - a phrase is framed (`dialogue_frame_rect`; [47C8h] the one shown),
///   and unless the frame is closed (13Fh) its box drawn
///   (`blit_dialogue_frame`, `restore_mouse_if_rect_intersects` over the
///   rect, `draw_head_if_needed_and_update_screen_rect_at_si`), then
///   [`draw_mouse_cursor_if_needed`];
/// - at a list's end (FFh) the next sentence of [47CAh]
///   (`sentence_offset`, `skip_32_strings`), or, the count done, bit 10h:
/// - then while the count is not above 0 the number part
///   (`sentence_number_part`, CF: a phrase to show) or a new random
///   sentence (`random_sentence`; AH set: done) whose strings it skips
///   (`skip_strings`).
pub fn dialogue_text_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x99da)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x9197, 0x99dd)?;
    let flags = c.byte(TEXT_FLAGS);
    if flags & 0x80 == 0 {
        c.run(0x99dd, 4)?;
        c.set_reg8(r8::AL, flags);
        return c.ret();
    }
    enum Step {
        Phrase,
        Show,
        ListEnd,
        Counted,
    }
    let mut step = if flags & 0x10 != 0 {
        c.run(0x99dd, 5)?;
        Step::Counted
    } else {
        c.run(0x99dd, 7)?;
        let at = c.word(TEXT_AT);
        c.set_reg(SI, at);
        let texts = c.word(0xdbb2);
        c.set_sreg(ES, texts);
        Step::Phrase
    };
    c.set_reg8(r8::AL, flags);
    loop {
        step = match step {
            Step::Phrase => {
                let ended = c.byte_in(ES, c.reg(SI)) == 0xff;
                c.run(0x99f0, 2)?;
                if ended { Step::ListEnd } else { Step::Show }
            }
            Step::Show => {
                c.run(0x99f6, 3)?;
                let left = c.word(TEXT_LEFT).wrapping_sub(1);
                c.set_word(TEXT_LEFT, left);
                let at = c.reg(SI);
                c.set_word(0x47c8, at);
                c.call(0x9bb1, 0x9a01)?;
                let closed = c.word(RECT_D834) == 0x13f;
                c.run(0x9a01, 4)?;
                let at = c.reg(SI);
                c.set_word(TEXT_AT, at);
                if closed {
                    return c.ret();
                }
                c.call(0x908c, 0x9a10)?;
                c.run(0x9a10, 2)?;
                c.set_reg(SI, RECT_D834);
                c.call(0xdb74, 0x9a16)?;
                c.run(0x9a16, 1)?;
                c.call(0xc4f0, 0x9a19)?;
                c.run(0x9a19, 1)?;
                return super::mouse::draw_mouse_cursor_if_needed(c.m, 0xdb67);
            }
            Step::ListEnd => {
                if c.word(TEXT_LEFT) & 0x8000 != 0 {
                    c.run(0x9a1d, 3)?;
                    let b = c.byte(TEXT_FLAGS);
                    c.set_byte(TEXT_FLAGS, b | 0x10);
                    Step::Counted
                } else {
                    c.run(0x9a1d, 5)?;
                    let table = c.word(0x47ca);
                    c.set_reg(SI, table);
                    let texts = c.word(0xdbb2);
                    c.set_sreg(ES, texts);
                    c.call(0x994f, 0x9a2f)?;
                    c.run(0x9a2f, 2)?;
                    let (si, bp) = (c.reg(SI), c.reg(BP));
                    let next = si.wrapping_add(c.word_in(ES, bp.wrapping_add(si)));
                    c.set_reg(SI, next);
                    c.call(0x996c, 0x9a35)?;
                    c.run(0x9a35, 2)?;
                    let at = c.reg(SI);
                    c.set_word(TEXT_AT, at);
                    Step::Phrase
                }
            }
            Step::Counted => {
                let more = c.word(TEXT_LEFT) as i16 > 0;
                let (at, texts) = (c.word(TEXT_AT), c.word(0xdbb2));
                c.run(0x9a40, if more { 4 } else { 5 })?;
                c.set_reg(SI, at);
                c.set_sreg(ES, texts);
                if more {
                    Step::Show
                } else {
                    c.call(0x9ab4, 0x9a52)?;
                    if c.flag(pc::cpu::CF) {
                        c.run(0x9a52, 1)?;
                    } else {
                        c.run(0x9a52, 2)?;
                        c.call(0x9a7b, 0x9a57)?;
                        let done = c.reg8(r8::AH) != 0;
                        c.run(0x9a57, 3)?;
                        if done {
                            return c.ret();
                        }
                        c.call(0x9a60, 0x9a5e)?;
                        c.run(0x9a5e, 1)?;
                    }
                    Step::Show
                }
            }
        };
    }
}

/// The dialogue text's state ([47D1h]): bit 80h running, 10h its count
/// done.
const TEXT_FLAGS: u16 = 0x47d1;
/// The next phrase of the text ([47C6h]).
const TEXT_AT: u16 = 0x47c6;
/// The phrases left ([47CEh]).
const TEXT_LEFT: u16 = 0x47ce;

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
    c.save(&[AX]);
    c.call(0xc08e, 0x80e3)?;
    c.run(0x80e3, 1)?;
    c.call(0x8fd1, 0x80e6)?;
    // The box 2244h raised for the while: its y and height pushed.
    let (y, height) = (c.word(MAP_BOX + 2), c.word(MAP_BOX + 6));
    let raised = y >= 0x32;
    c.run(0x80e6, if raised { 12 } else { 11 })?;
    let phrase = c.pop();
    c.set_reg(BX, phrase);
    c.set_reg(SI, MAP_BOX);
    let new_y = if raised { y + 0x26 } else { y };
    c.set_reg(AX, new_y);
    c.push(y);
    c.push(height);
    c.push(phrase);
    c.set_word(MAP_BOX + 2, new_y);
    c.set_word(MAP_BOX + 6, 0x19);
    c.call(0x9f82, 0x8105)?;
    c.run(0x8105, 2)?;
    let phrase = c.pop();
    c.set_reg(AX, phrase);
    c.call(0x88af, 0x8109)?;
    let timed = c.byte(0x4774) == 0;
    c.run(0x8109, if timed { 6 } else { 5 })?;
    if timed {
        let more = 0x10au16.wrapping_add(c.word(0xd810));
        c.set_reg(AX, more);
        let t = c.word(0x4780).wrapping_add(more);
        c.set_word(0x4780, t);
        c.call(0x9efd, 0x811e)?;
        c.run(0x811e, 3)?;
    }
    let height = c.pop();
    c.set_word(MAP_BOX + 6, height);
    let y = c.pop();
    c.set_word(MAP_BOX + 2, y);
    c.call(0xc07c, 0x8129)?;
    c.run(0x8129, 3)?;
    c.set_word(0x4720, 0x18f3);
    c.set_byte(0x4722, 0);
    c.call(0x541f, 0x8137)?;
    c.run(0x8137, 2)?;
    c.set_word(TEXT_PTR, 0);
    c.ret()
}

/// The box a phrase over the map is drawn in (2244h: x, y, width,
/// height words).
const MAP_BOX: u16 = 0x2244;

/// `loc_088af`: the phrase AX (none for 0) shown: [4780h] = it, [47E0h]
/// 0. Over the map (bit 40h of [`MAP_MODE`], then dropped):
/// [`show_phrase_on_map`]. Else it is fetched
/// (`get_phrase_or_command_string_si`) and expanded (`expand_phrase`,
/// `format_interpolated_string` into A6B0h). On the map screen
/// ([`MAP_SCREEN`] 2 and up) out of [46D9h] the box 1BE2h is reset (its
/// flag at 1C06h from [0FBh]); else, unless it starts with a control
/// code, laid out (`layout_text`). Callers read the flags.
pub fn show_phrase(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x88af)?;
    let mut c = Ctx::new(m);
    let phrase = c.reg(AX);
    if phrase == 0 {
        c.run(start, 3)?;
        c.or(W, 0, 0);
        return c.ret();
    }
    let mode = c.byte(MAP_MODE);
    let note = move |c: &mut Ctx| {
        c.set_word(0x4780, phrase);
        c.set_byte(0x47e0, 0);
    };
    if mode & 0x40 != 0 {
        return phrase_on_map_at(&mut c, start, 8, |c| {
            note(c);
            c.set_byte(MAP_MODE, mode & !0x40);
        });
    }
    c.run(start, 8)?;
    note(&mut c);
    c.set_reg(SI, phrase);
    c.call(0xcf70, 0x88cf)?;
    c.run(0x88cf, 1)?;
    c.call(0x88f1, 0x88d2)?;
    c.run(0x88d2, 3)?;
    c.set_reg(DI, 0xa6b0);
    c.push(0xa6b0);
    c.call(0x8944, 0x88d9)?;
    let map_screen = c.byte(MAP_SCREEN) >= 2;
    if map_screen && c.byte(0x46d9) == 0 {
        c.run(0x88d9, 21)?;
        let text = c.pop();
        c.set_reg(SI, text);
        c.set_word(BOX_SHOWN, 1);
        let flag = !c.byte(0xfb) & 0x80;
        c.set_byte(0x1c06, flag);
        let ds = c.sreg(DS);
        c.push(ds);
        let es = c.pop();
        c.set_sreg(ES, es);
        c.set_reg(DI, 0x1be2);
        c.set_reg(AX, 0);
        for _ in 0..4 {
            c.stos(W);
        }
        c.set_reg8(r8::AL, 0x80);
        c.stos(W);
        c.xor(W, 0, 0); // `xor ax,ax` above: its flags are returned
        return c.ret();
    }
    let text = c.word_in(SS, c.reg(SP));
    let first = c.byte(text);
    let control = first & 0x80 != 0;
    c.run(0x88d9, 3 + if map_screen { 2 } else { 0 } + if control { 4 } else { 5 })?;
    let text = c.pop();
    c.set_reg(SI, text);
    c.lods(B, DS);
    if control {
        c.or(B, first as u16, first as u16);
        return c.ret();
    }
    let si = c.reg(SI).wrapping_sub(1);
    c.set_reg(SI, si);
    c.call(0x8b11, 0x88ea)?;
    let b = c.byte(c.reg(SI));
    c.run(0x88ea, if b >= 0xfe { 3 } else { 4 })?;
    c.cmp(B, b as u16, 0xfe);
    c.ret()
}

/// `loc_09efd`: the voice for the text: [47DDh] = [`TALKING`]; when
/// `load_voc_and_lipsync_data` (AX [4780h], BX [`TALKER`]) starts one
/// (CF): `start_9f1c` for a character below 10h, the lip sync set up
/// (`setup_lip_sync_data_from_current`), the lip-sync task A7C2h
/// (`add_frame_task`, BP 0), [0DC2Bh] 1, the driver's voice start (far
/// [3991h] with SI 3811h), [0DC2Ch] the time (AX), [0DC2Eh] 8000h, and
/// `loc_0a82e` when `loc_0a814` says so (CF).
pub fn start_text_voice(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9efd)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let talking = c.byte(TALKING);
    c.set_reg8(r8::AL, talking);
    c.set_byte(0x47dd, talking);
    let phrase = c.word(0x4780);
    c.set_reg(AX, phrase);
    let talker = c.word(TALKER);
    c.set_reg(BX, talker);
    c.call(0xa6cc, 0x9f0d)?;
    if !c.flag(pc::cpu::CF) {
        c.run(0x9f0d, 2)?;
        return c.ret();
    }
    if c.word(TALKER) < 0x10 {
        c.run(0x9f0d, 4)?;
        c.call(0x9f1c, 0x9f19)?;
        c.run(0x9f19, 2)?;
    } else {
        c.run(0x9f0d, 5)?;
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
    let time = c.word(0xce7a);
    c.set_reg(AX, time);
    c.set_word(0xdc2c, time);
    c.set_word(0xdc2e, 0x8000);
    c.call(0xa814, 0xa783)?;
    let again = c.flag(pc::cpu::CF);
    c.run(0xa783, 2)?;
    if !again {
        return c.ret();
    }
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
        // A bail leaves CS:IP at the stretch's first instruction.
        let Ok((n, call)) = r else {
            c.m.cpu.ip = at;
            return Err(Bail);
        };
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
    let ax = c.word(TALKER);
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

/// `loc_099be`: with [47C3h] set, the talking heads 7 and 2Dh opened, each
/// in the other game state (`swap_game_state` around), a text step
/// between; then the text step ([`dialogue_text_step`]).
pub fn two_heads_text_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x99be)?;
    let mut c = Ctx::new(m);
    if c.byte(0x47c3) == 0 {
        c.run(start, 2)?;
        return dialogue_text_step(c.m, 0x99da);
    }
    c.run(start, 3)?;
    c.call(0x998e, 0x99c8)?;
    c.run(0x99c8, 2)?;
    c.set_reg(AX, 7);
    c.call(0x920f, 0x99ce)?;
    c.run(0x99ce, 1)?;
    c.call(0x99da, 0x99d1)?;
    c.run(0x99d1, 1)?;
    c.call(0x998e, 0x99d4)?;
    c.run(0x99d4, 2)?;
    c.set_reg(AX, 0x2d);
    c.call(0x920f, 0x99da)?;
    dialogue_text_step(c.m, 0x99da)
}

/// `loc_097cf`: the end of a dialogue line (lip sync stopped). With a
/// speaker ([47C4h] not FFFFh): the troop [47A2h] marked talked to (+0Fh
/// bit 20h set, 4 cleared), [47E1h] 0, the dialogue box closed; in a map
/// mode the panel redrawn on the front buffer and its buttons ([11CAh]
/// 0 meanwhile), `loc_037f4`, `loc_0c4dd`, [11CAh] 0. Else by [47A4h]:
/// bit 7 ends the dialogue (gauge dropped, state cleared, the text
/// dropped; before stage 74h the view and head back up, from stage 64h
/// to 73h `loc_036d3` and a wait, again); else the speaker's gauge (off
/// with bit 40h of +0Fh), and in a dialogue the text redrawn, or with
/// bit 0 of [47A4h] clear the view and head back up.
pub fn line_done(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x97cf)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xa7a5, 0x97d2)?;
    let sp = c.word(TALKER);
    if sp == 0xffff {
        c.run(0x97d2, 3)?;
        c.cmp(W, sp, 0xffff);
        return c.ret();
    }
    let mode = c.byte(0x11c9);
    c.run(0x97d2, 9)?;
    c.cmp(W, sp, 0xffff);
    let si = c.word(SPEAKER);
    c.set_reg(SI, si);
    let f = c.byte(si.wrapping_add(0x0f));
    let f = c.or(B, f as u16, 0x20);
    let f = c.and(B, f, 0xfb);
    c.set_byte(si.wrapping_add(0x0f), f as u8);
    c.set_word(0x47e1, 0);
    c.cmp(B, mode as u16, 0);
    c.call(0x8c8a, if mode != 0 { 0x97f5 } else { 0x980f })?;
    if mode != 0 {
        c.run(0x97f5, 2)?;
        c.set_reg(BP, 0x98b2);
        c.call(0xc097, 0x97fb)?;
        c.run(0x97fb, 4)?;
        c.xor(B, 0, 0);
        let old = c.byte(0x11ca);
        c.set_byte(0x11ca, 0);
        let ax = c.reg(AX) & 0xff00 | old as u16;
        c.set_reg(AX, ax);
        c.push(ax);
        c.call(0x2ffb, 0x9805)?;
        c.run(0x9805, 4)?;
        let a = c.pop();
        c.set_reg(AX, a);
        c.set_byte(0x11ca, a as u8);
        c.call(0x37f4, 0x4ac1)?;
        c.run(0x4ac1, 1)?;
        c.call(0xc4dd, 0x4ac4)?;
        c.run(0x4ac4, 2)?;
        c.set_byte(0x11ca, 0);
        return c.ret();
    }
    let st = c.byte(0x47a4);
    let si = c.word(SPEAKER);
    if st & 0x80 != 0 {
        let gauge = c.byte(si.wrapping_add(0x0f)) & 0x40 != 0;
        c.run(0x980f, 3 + 3 + gauge as u32)?;
        c.cmp(B, st as u16, 0);
        c.set_reg(SI, si);
        c.set_word(0x1c06, 0);
        c.test(B, if gauge { 0x40 } else { 0 }, 0x40);
        if gauge {
            c.call(0x9673, 0x9858)?;
        }
        return line_end_9858(&mut c);
    }
    let talk = c.byte(0x2b);
    let gauge = c.byte(si.wrapping_add(0x0f)) & 0x40 != 0;
    c.run(0x980f, 5 + if talk != 0 { 2 } else { 0 } + 3)?;
    c.cmp(B, st as u16, 0);
    c.set_reg(SI, si);
    c.cmp(B, talk as u16, 0);
    if talk != 0 {
        c.set_reg(AX, 0x9840);
        c.push(0x9840);
    }
    c.test(B, if gauge { 0x40 } else { 0 }, 0x40);
    if gauge {
        if talk == 0 {
            return add_speaker_gauge(c.m, 0x9673);
        }
        // The jump with 9840h pushed: its return lands there.
        c.pop();
        c.call(0x9673, 0x9840)?;
        return text_redraw_9840(&mut c);
    }
    c.call(0x9655, 0x9831)?;
    let t2 = c.byte(0x2b);
    let b0 = c.byte(0x47a4) & 1 != 0;
    if t2 != 0 || b0 {
        c.run(0x9831, if t2 != 0 { 3 } else { 5 })?;
        c.cmp(B, t2 as u16, 0);
        if t2 == 0 {
            c.test(B, b0 as u16, 1);
        }
        if talk == 0 {
            return c.ret();
        }
        c.pop();
        return text_redraw_9840(&mut c);
    }
    if talk != 0 {
        c.m.cpu.ip = 0x9831;
        return Err(Bail);
    }
    c.run(0x9831, 4)?;
    c.cmp(B, 0, 0);
    c.test(B, 0, 1);
    view_back_9879(&mut c)
}

/// `loc_09840`: the text dropped, the game area to the screen,
/// `loc_0c4dd`.
fn text_redraw_9840(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x9840, 1)?;
    c.call(0x9b8b, 0x9843)?;
    c.run(0x9843, 1)?;
    c.call(0xc43e, 0x9846)?;
    c.run(0x9846, 1)?;
    super::draw::sub_c4dd(c.m, 0xc4dd)
}

/// From 9858h: the dialogue state cleared and its text dropped; from
/// stage 64h to 73h `loc_036d3` and a wait of C8h (again), else the view
/// back ([`view_back_9879`]).
fn line_end_9858(c: &mut Ctx) -> Result<(), Bail> {
    let mut i = 0;
    loop {
        i += 1;
        if i > 100 {
            c.m.cpu.ip = 0x9858;
            return Err(Bail);
        }
        c.run(0x9858, 7)?;
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
        c.set_word(0x4540, 0);
        c.set_word(BOX_SHOWN, 0);
        let v = c.byte(TEXT_FLAGS);
        let v = c.and(B, v as u16, 0x3f);
        c.set_byte(TEXT_FLAGS, v as u8);
        c.set_word(0x47c8, 0);
        let v = c.byte(0x47a4);
        let v = c.and(B, v as u16, 0x7f);
        c.set_byte(0x47a4, v as u8);
        c.call(0x9b8b, 0x9870)?;
        let st = c.byte(0x23);
        let d = st.wrapping_sub(0x64);
        c.run(0x9870, 4)?;
        c.set_reg8(r8::AL, st);
        let a = c.sub(B, st as u16, 0x64);
        c.set_reg8(r8::AL, a as u8);
        c.cmp(B, a, 0x10);
        if d >= 0x10 {
            return view_back_9879(c);
        }
        c.run(0x9898, 2)?;
        c.set_reg(BP, 0x37b2);
        c.call(0xc097, 0x989e)?;
        c.run(0x989e, 1)?;
        c.call(0x36d3, 0x98a1)?;
        c.run(0x98a1, 2)?;
        c.set_reg(AX, 0xc8);
        c.call(0xe3a0, 0x98a7)?;
        c.run(0x98a7, 2)?;
        c.set_word(0x1c06, 0);
    }
}

/// From 9879h: the view back: `loc_02efb`, outside the map modes
/// `setup_3090`, `loc_037b2`, the buffers and palette, `gfx_call_25`,
/// `loc_0c4dd`, the head up.
fn view_back_9879(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x9879, 1)?;
    c.call(0x2efb, 0x987c)?;
    let k = c.byte(0x11c9);
    c.run(0x987c, 3)?;
    c.cmp(B, k as u16, 0);
    if k == 0 {
        c.call(0x3090, 0x9886)?;
        c.run(0x9886, 1)?;
    }
    c.call(0x37b2, 0x9889)?;
    for (at, to) in [(0x9889u16, 0xc412u16), (0x988c, 0xc0f4), (0x988f, 0x1834), (0x9892, 0xc4dd)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    c.run(0x9895, 1)?;
    super::ui::ui_head_animate_up(c.m, 0x17e6)
}

/// `loc_09f9e`: say the first usable entry of the dialogue list SI (4
/// bytes each, FFFFh ends it: CF): one not yet said (bit 7 of its flags,
/// unless bit 6 or it matches the mask [47C2h]) whose condition holds
/// (`evaluate_condition_ax`, condition id from bytes 1 and 2). Its flags
/// word goes to [47DEh] and its phrase (word 2, bytes swapped, low 10
/// bits + 800h) is shown: through the text buffer [47BCh] (a phrase
/// starting with a code of 80h or more cuts the buffer back one entry)
/// or directly ([`show_phrase`], not with [0C6h]). Then `loc_0c85b`;
/// unless a phrase runs ([47B6h]) its action (low 4 bits: the table at
/// CS:A107h), for bits 0Ch the entry noted in the CS list [11BDh], the
/// entry marked said, [19h] FFh, and SI past it (FFFFh when [47A8h] was
/// set). Outside the map views for a speaker below 10h: the portrait
/// brought (before the phrase) and the scene redrawn, `loc_02ebf` with
/// [4774h], and with no [0EAh] above 0 the voice started and the hook
/// [227Eh] (reset to F66h) run. [28E7h] = [28E8h] unless [0FBh] is
/// negative. CF clear.
pub fn say_dialogue_entry(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9f9e)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let si = c.reg(SI);
    c.set_word(0x477c, si);
    c.call(0x94f3, 0x9fa5)?;
    c.run(0x9fa5, 1)?;
    c.set_word(0x47bc, 0xa6b0);
    // The list: the first entry to say.
    let mut i = 0;
    loop {
        i += 1;
        if i > 4000 {
            c.m.cpu.ip = 0x9fab;
            return Err(Bail);
        }
        let si = c.reg(SI);
        let ax = c.word(si);
        if ax == 0xffff {
            c.run(0x9fab, 5)?;
            c.set_reg(AX, ax);
            c.cmp(W, ax, 0xffff);
            c.set_flag(pc::cpu::CF, true);
            return c.ret();
        }
        let al = ax as u8;
        let mask = c.byte(MODE);
        let said = al & 0x80 != 0 && al & 0x40 == 0;
        let skip = said && al & mask != 0;
        let n = 5 + if al & 0x80 == 0 { 0 } else if al & 0x40 != 0 { 2 } else { 4 };
        if skip {
            c.run(0x9fab, n + 2)?;
        } else {
            c.run(0x9fab, n + 7)?;
        }
        c.set_reg(AX, ax);
        c.cmp(W, ax, 0xffff);
        c.test(B, al as u16, 0x80);
        let mut ax = ax;
        if al & 0x80 != 0 {
            c.test(B, al as u16, 0x40);
            if al & 0x40 == 0 {
                let a = c.and(B, al as u16, mask as u16);
                ax = ax & 0xff00 | a;
                c.set_reg(AX, ax);
            }
        }
        if skip {
            let s = c.add(W, si, 4);
            c.set_reg(SI, s);
            continue;
        }
        c.push(si);
        let b2 = c.byte(si.wrapping_add(2));
        let h = c.rol1(B, b2 as u16);
        let h = c.rol1(B, h);
        let h = c.and(B, h, 3);
        c.set_reg(AX, h << 8 | ax >> 8);
        c.call(0xa396, 0x9fd0)?;
        let zf = c.flag(ZF);
        c.run(0x9fd0, if zf { 4 } else { 2 })?;
        let s = c.pop();
        c.set_reg(SI, s);
        if zf {
            let s = c.add(W, s, 4);
            c.set_reg(SI, s);
            continue;
        }
        break;
    }
    // The portrait first, outside the map views.
    let v = c.byte(0x46eb);
    let sp = c.word(TALKER);
    if v != 0 {
        c.run(0x9fd8, 2)?;
        c.cmp(B, v as u16, 0);
    } else if sp >= 0x10 {
        c.run(0x9fd8, 5)?;
        c.cmp(B, 0, 0);
        c.set_reg(AX, sp);
        c.cmp(W, sp, 0x10);
    } else {
        c.run(0x9fd8, 8)?;
        c.cmp(B, 0, 0);
        c.set_reg(AX, sp);
        c.cmp(W, sp, 0x10);
        let si = c.reg(SI);
        c.push(si);
        c.push(sp);
        c.call(0xa0f1, 0x9fec)?;
        c.run(0x9fec, 1)?;
        c.call(0x1803, 0x9fef)?;
        c.run(0x9fef, 1)?;
        c.call(0x3af9, 0x9ff2)?;
        c.run(0x9ff2, 2)?;
        let a = c.pop();
        c.set_reg(AX, a);
        c.call(0x91a0, 0x9ff6)?;
        c.run(0x9ff6, 1)?;
        let s = c.pop();
        c.set_reg(SI, s);
    }
    // The phrase.
    let si = c.reg(SI);
    let di = c.word(0x47bc);
    c.run(0x9ff7, 10)?;
    c.push(si);
    let f = c.lods(W, DS);
    c.set_word(0x47de, f);
    let p = c.lods(W, DS).swap_bytes();
    let p = c.and(W, p, 0x3ff);
    let p = c.or(W, p, 0x800);
    c.set_reg(AX, p);
    c.set_reg(DI, di);
    c.cmp(W, di, 0xa6b0);
    if di != 0xa6b0 {
        c.run(0xa00f, 2)?;
        c.set_reg(SI, p);
        c.call(0xcf70, 0xa014)?;
        let si = c.reg(SI);
        let b = c.byte_in(ES, si);
        if b >= 0x80 {
            c.run(0xa014, 7)?;
            c.cmp(B, b as u16, 0x80);
            let a = c.pop();
            let a = c.sub(W, a, 4);
            c.set_reg(AX, a);
            c.push(a);
            let di = c.reg(DI);
            c.set_byte(di.wrapping_sub(1), 0xff);
        } else {
            c.run(0xa014, 3)?;
            c.cmp(B, b as u16, 0x80);
            c.call(0x88f1, 0xa029)?;
            c.run(0xa029, 1)?;
            c.call(0x8944, 0xa02c)?;
        }
        c.run(0xa02c, 2)?;
        c.set_reg(SI, 0xa6b0);
        c.call(0x8b11, 0xa032)?;
        c.run(0xa032, 1)?;
    } else {
        let k = c.byte(0xc6);
        c.run(0xa034, if k != 0 { 2 } else { 3 })?;
        c.cmp(B, k as u16, 0);
        if k == 0 {
            c.call(0x88af, 0xa03e)?;
        }
    }
    c.run(0xa03e, 2)?;
    let s = c.pop();
    c.set_reg(SI, s);
    c.call(0xc85b, 0xa042)?;
    // The entry's action and marks, unless a phrase runs.
    let busy = c.word(0x47b6);
    if busy != 0 {
        c.run(0xa042, 2)?;
        c.cmp(W, busy, 0);
    } else {
        let si = c.reg(SI);
        let act = c.byte(si) & 0x0f;
        c.run(0xa042, if act != 0 { 12 } else { 5 })?;
        c.cmp(W, 0, 0);
        let a = c.and(B, act as u16, 0x0f);
        c.set_reg8(r8::AL, a as u8);
        if act != 0 {
            c.xor(B, 0, 0);
            c.set_reg(AX, a);
            c.push(si);
            let a = c.dec(W, a);
            let a = c.shl1(W, a, 1);
            c.set_reg(AX, a);
            let bx = c.add(W, 0xa107, a);
            c.set_reg(BX, bx);
            let to = c.word_in(pc::cpu::CS, bx);
            c.call(to, 0xa05d)?;
            c.run(0xa05d, 1)?;
            let s = c.pop();
            c.set_reg(SI, s);
        }
        let si = c.reg(SI);
        let b2 = c.byte(si.wrapping_add(2));
        let fl = c.byte(si);
        let note = b2 & 0x0c != 0 && fl & 0x80 == 0;
        let n = 3 + if b2 & 0x0c == 0 { 0 } else if fl & 0x80 != 0 { 2 } else { 15 };
        let had = c.byte(0x47a8);
        c.run(0xa05e, n + 7 + if had != 0 { 1 } else { 0 })?;
        let a = c.and(B, b2 as u16, 0x0c);
        c.set_reg8(r8::AL, a as u8);
        if b2 & 0x0c != 0 {
            c.test(B, fl as u16, 0x80);
        }
        if note {
            let a = c.sub(W, si, 0xaa78);
            let a = c.shr1(W, a, 2);
            let bl = c.byte(TALKER);
            let b = c.shl1(B, bl as u16, 3);
            c.set_reg8(r8::BL, b as u8);
            let h = c.or(B, a >> 8, b);
            let a = h << 8 | a & 0xff;
            c.set_reg(AX, a);
            let bp = c.word(0x11bd);
            c.set_reg(BP, bp);
            c.set_word_in(pc::cpu::CS, bp, a);
            c.set_word_in(pc::cpu::CS, bp.wrapping_add(2), 0);
            let v = c.add(W, bp, 2);
            c.set_word(0x11bd, v);
        }
        c.set_byte(0x19, 0xff);
        let fl = c.byte(si);
        let fl = c.or(B, fl as u16, 0x80);
        c.set_byte(si, fl as u8);
        let s = c.add(W, si, 4);
        c.set_reg(SI, s);
        c.xor(B, 0, 0);
        c.set_byte(0x47a8, 0);
        c.set_reg8(r8::AL, had);
        c.or(B, had as u16, had as u16);
        if had != 0 {
            c.set_reg(SI, 0xffff);
        }
    }
    // Outside the map views: the scene and the voice.
    let v = c.byte(0x46eb);
    let sp = c.word(TALKER);
    if v != 0 || sp >= 0x10 {
        c.run(0xa0aa, if v != 0 { 2 } else { 4 })?;
        c.cmp(B, v as u16, 0);
        if v == 0 {
            c.cmp(W, sp, 0x10);
        }
    } else {
        c.run(0xa0aa, 6)?;
        c.cmp(B, 0, 0);
        c.cmp(W, sp, 0x10);
        let si = c.reg(SI);
        c.push(si);
        c.call(0x978e, 0xa0bc)?;
        let w = c.byte(0x4774);
        c.run(0xa0bc, if w != 0 { 5 } else { 3 })?;
        let s = c.pop();
        c.set_reg(SI, s);
        c.cmp(B, w as u16, 0);
        if w != 0 {
            c.push(s);
            c.call(0x2ebf, 0xa0c8)?;
            c.run(0xa0c8, 1)?;
            let s = c.pop();
            c.set_reg(SI, s);
        }
        let e = c.byte(0xea);
        let up = (e as i8) > 0;
        c.run(0xa0c9, if up { 2 } else { 3 })?;
        c.cmp(B, e as u16, 0);
        if !up {
            c.call(0xe270, 0xa0d3)?;
            c.run(0xa0d3, 1)?;
            c.call(0x9efd, 0xa0d6)?;
            c.run(0xa0d6, 3)?;
            let hook = c.word(0x227e);
            c.set_word(0x227e, 0xf66);
            c.set_reg(AX, hook);
            c.call(hook, 0xa0df)?;
            c.run(0xa0df, 1)?;
            c.call(0xe283, 0xa0e2)?;
        }
    }
    let fb = c.byte(0xfb);
    let neg = fb & 0x80 != 0;
    c.run(0xa0e2, if neg { 4 } else { 6 })?;
    c.cmp(B, fb as u16, 0);
    if !neg {
        let v = c.byte(0x28e8);
        c.set_reg8(r8::AL, v);
        c.set_byte(0x28e7, v);
    }
    c.set_flag(pc::cpu::CF, false);
    c.ret()
}

/// `loc_09761`: the speaker AX ([47C4h]; 0Eh describes the troop
/// [4756h] first) starts its dialogue: its list (entry 8 * AX + 7 of
/// the table at AA76h), the text set up, the mask [47C2h] 80h, then the
/// first entry said ([`say_dialogue_entry`]).
pub fn start_speaker_dialogue(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9761)?;
    let mut c = Ctx::new(m);
    let ax = c.reg(AX);
    c.run(start, if ax as u8 == 0x0e { 5 } else { 3 })?;
    c.set_word(TALKER, ax);
    c.cmp(B, ax & 0xff, 0x0e);
    if ax as u8 == 0x0e {
        let si = c.word(0x4756);
        c.set_reg(SI, si);
        c.call(0x31f6, 0x976f)?;
        c.run(0x976f, 1)?;
        c.set_reg(AX, 0x0e);
    }
    c.run(0x9772, 8)?;
    let ax = c.reg(AX);
    let a = c.shl1(W, ax, 3);
    let a = c.or(W, a, 7);
    c.set_reg(AX, a);
    let s = c.shl1(W, a, 1);
    let si = c.word(s.wrapping_sub(0x558a));
    c.set_reg(SI, si);
    c.call(0x9f40, 0x9786)?;
    c.run(0x9786, 2)?;
    c.set_byte(MODE, 0x80);
    say_dialogue_entry(c.m, 0x9f9e)
}

/// `sub_9468`: the dialogue's next line, unless the cursor's UI is the
/// list 1FFEh (`loc_0d41b`): the text set up, [226Dh] 0Ah, [1Bh] 0; from
/// the pending list [47BAh] (else entry [47BEh] of the table AA76h) the
/// first sayable entry ([`say_dialogue_entry`]; the speaker's number
/// told first), the list kept in [47BAh] and the panel faded; when a
/// list runs out the next of its group of 4 ([47BEh] up); with none left
/// the screen left (speaker 0Dh: from the group of 8 again, mask 20h).
/// Bails while a phrase runs ([47B6h]).
pub fn next_dialogue_line(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9468)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xd41b, 0x946b)?;
    let bp = c.reg(BP);
    if bp == 0x1ffe {
        c.run(0x946b, 3)?;
        c.cmp(W, bp, 0x1ffe);
        return c.ret();
    }
    c.run(0x946b, 2)?;
    c.cmp(W, bp, 0x1ffe);
    next_line_from_9472(&mut c)
}

/// `sub_9468` from 9472h (see [`next_dialogue_line`]).
pub(super) fn next_line_from_9472(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x9472, 1)?;
    c.call(0x9f40, 0x9475)?;
    let busy = c.word(0x47b6);
    if busy != 0 {
        c.m.cpu.ip = 0x9475;
        return Err(Bail);
    }
    let si = c.word(0x47ba);
    c.run(0x9475, 7 + if si == 0 { 1 } else { 0 })?;
    c.set_byte(0x226d, 0x0a);
    c.set_byte(0x1b, 0);
    c.cmp(W, 0, 0);
    c.set_reg(SI, si);
    c.or(W, si, si);
    // 0: 9492h (the list of entry SI), 1: 949Ah (the list SI).
    let mut at = 1;
    if si == 0 {
        let e = c.word(0x47be);
        c.set_reg(SI, e);
        at = 0;
    }
    let mut i = 0;
    loop {
        i += 1;
        if i > 1000 {
            c.m.cpu.ip = if at == 0 { 0x9492 } else { 0x949a };
            return Err(Bail);
        }
        if at == 0 {
            c.run(0x9492, 3)?;
            let si = c.reg(SI);
            c.set_reg(AX, si);
            let s = c.shl1(W, si, 1);
            let l = c.word(s.wrapping_sub(0x558a));
            c.set_reg(SI, l);
        }
        let si = c.reg(SI);
        if si != 0xffff {
            c.run(0x949a, 3)?;
            c.cmp(W, si, 0xffff);
            c.call(0x9b49, 0x94a2)?;
            c.run(0x94a2, 1)?;
            c.call(0x9f9e, 0x94a5)?;
            let si = c.reg(SI);
            if !c.flag(pc::cpu::CF) {
                c.run(0x94a5, 3)?;
                c.set_word(0x47ba, si);
                return fade_panel_tail(c.m);
            }
            let e = c.word(0x47be).wrapping_add(1);
            c.run(0x94a5, 8)?;
            c.set_word(0x47ba, si);
            c.set_reg(AX, e.wrapping_sub(1));
            let e = c.inc(W, e.wrapping_sub(1));
            c.set_word(0x47be, e);
            c.set_reg(SI, e);
            let a = c.and(W, e, 3);
            c.set_reg(AX, a);
            if a != 0 {
                at = 0;
                continue;
            }
        } else {
            c.run(0x949a, 2)?;
            c.cmp(W, si, 0xffff);
        }
        // 94B9h: the group ran out.
        let sp = c.word(TALKER);
        if sp != 0x0d {
            c.run(0x94b9, 3)?;
            c.cmp(W, sp, 0x0d);
            return super::screens::leave_and_fade(c.m, 0xd2e2);
        }
        let si = c.reg(SI);
        c.run(0x94b9, 2 + 2 + if si == 0xffff { 1 } else { 0 } + 4)?;
        c.cmp(W, sp, 0x0d);
        c.cmp(W, si, 0xffff);
        let s = if si == 0xffff { c.word(0x47be) } else { si };
        let s = c.and(W, s, 0xfff8);
        c.set_reg(SI, s);
        c.set_word(0x47be, s);
        c.set_byte(MODE, 0x20);
        at = 0;
    }
}

/// `jmp loc_0d280` from 94DAh.
fn fade_panel_tail(m: &mut Machine) -> Result<(), Bail> {
    super::screens::fade_panel(m, 0xd280)
}

/// `sub_945b`: with [479Eh] the next line ([`next_dialogue_line`]); else
/// the speaker [47C4h] started afresh (`loc_093aa`: lip sync, portrait,
/// text box, its dialogue group by `loc_093df`) and its line from 9472h.
pub fn dialogue_line(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x945b)?;
    let mut c = Ctx::new(m);
    let k = c.word(BOX_SHOWN);
    if k != 0 {
        c.run(start, 2)?;
        c.cmp(W, k, 0);
        return next_dialogue_line(c.m, 0x9468);
    }
    c.run(start, 4 + 5)?;
    c.cmp(W, 0, 0);
    let sp = c.word(TALKER);
    c.set_reg(AX, sp);
    c.xor(B, 0, 0);
    let ax = sp & 0xff;
    c.set_reg(AX, ax);
    c.set_word(0x47e1, 0);
    c.push(ax);
    c.set_word(TALKER, ax);
    c.call(0x91a0, 0x93b9)?;
    for (at, to) in [(0x93b9u16, 0x3af9u16), (0x93bc, 0x9197), (0x93bf, 0x9908)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    c.run(0x93c2, 3)?;
    let si = c.word(0x47c8);
    c.set_reg(SI, si);
    c.set_word(0x4540, 0);
    c.call(0x9bac, 0x93cf)?;
    for (at, to) in [(0x93cfu16, 0x1834u16), (0x93d2, 0xc0f4), (0x93d5, 0xc4dd)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    c.run(0x93d8, 2)?;
    let a = c.pop();
    c.set_reg(AX, a);
    c.call(0x93df, 0x93dc)?;
    c.run(0x93dc, 1)?;
    next_line_from_9472(&mut c)
}

/// `loc_09f8b`: [`say_dialogue_entry`] with the mask [47C2h] 20h
/// meanwhile.
pub fn say_entry_mask_20(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9f8b)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let v = c.word(MODE);
    c.push(v);
    c.set_byte(MODE, 0x20);
    c.call(0x9f9e, 0x9f97)?;
    c.run(0x9f97, 2)?;
    let v = c.pop();
    c.set_word(MODE, v);
    c.ret()
}

/// `loc_0b039`: the book page for the entry at CS:[11BFh]: [0C6h] bits 1,
/// 2 cleared; into the text buffer A6B0h ([47BCh]) the heading
/// (`find_in_2426` code's top 5 bits + F3h) and the entry's phrase
/// (its record at AA7Ah + 4 * low 11 bits, kept in [477Ch]); the text
/// ended and shown, its frame (2408h), the portrait; a mark (sprite 4 of
/// 32h) when the text ends above 8Ah; except on the first page (AAh) the
/// heading phrase 103h + bits 2..3 of the record; the page number
/// ((entry - AAh) / 2 + 1, a leading 0 blank) at (132h, 3) in colour
/// 53h; and with [243Eh] the icons 2412h at it - 0Bh ([1C30h] 80h, else
/// 0).
pub fn book_page(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xb039)?;
    let mut c = Ctx::new(m);
    c.run(start, 8)?;
    let v = c.byte(0xc6);
    let v = c.and(B, v as u16, 0xf9);
    c.set_byte(0xc6, v as u8);
    c.set_reg(SI, 0xa6b0);
    c.set_word(0x47bc, 0xa6b0);
    c.push(0xa6b0);
    let p = c.word(0x11bf);
    let e = c.word_in(pc::cpu::CS, p);
    c.set_reg(SI, e);
    c.set_reg(AX, e);
    c.call(0xb254, 0xb052)?;
    c.run(0xb052, 7)?;
    let ax = c.reg(AX);
    let h = c.shr1(B, ax >> 8, 3);
    c.xor(B, 0, 0);
    let a = c.add(W, h, 0xf3);
    c.set_reg(AX, a);
    c.call(0xb126, 0xb062)?;
    c.run(0xb062, 12)?;
    let si = c.reg(SI);
    let s = c.and(W, si, 0x7ff);
    let s = c.shl1(W, s, 2);
    let s = c.add(W, s, 0xaa7a);
    c.set_reg(SI, s);
    c.set_word(0x477c, s);
    let r = c.lods(W, DS);
    let s = c.pop();
    c.set_reg(SI, s);
    c.push(r);
    let x = r.swap_bytes();
    let h = c.and(B, x >> 8, 3);
    let h = c.or(B, h, 8);
    c.set_reg(AX, h << 8 | x & 0xff);
    c.call(0xb126, 0xb080)?;
    c.run(0xb080, 3)?;
    let di = c.word(0x47bc);
    c.set_reg(DI, di);
    c.set_byte(di.wrapping_sub(1), 0xff);
    c.call(0x88e1, 0xb08b)?;
    c.run(0xb08b, 2)?;
    c.set_reg(SI, 0x2408);
    c.call(0xc551, 0xb091)?;
    c.run(0xb091, 1)?;
    c.call(0x1797, 0xb094)?;
    let end = c.word(0xd82e);
    let mid = 0x8cu16.wrapping_sub(end) >> 1;
    let b = mid.wrapping_add(end);
    let mark = b < 0x8a;
    c.run(0xb094, 7 + if mark { 3 } else { 0 })?;
    c.set_reg(AX, end);
    let bx = c.sub(W, 0x8c, end);
    let bx = c.shr1(W, bx, 1);
    let bx = c.add(W, bx, end);
    c.set_reg(BX, bx);
    c.cmp(W, bx, 0x8a);
    if mark {
        c.set_reg(DX, 0x93);
        c.set_reg(AX, 0x32);
        c.call(0xc13e, 0xb0af)?;
        c.run(0xb0af, 2)?;
        c.set_reg(AX, 4);
        c.call(0xc22f, 0xb0b5)?;
    }
    let page = c.word(0x11bf);
    let first = page == 0xaa;
    c.run(0xb0b5, 3 + if first { 0 } else { 8 })?;
    let r = c.pop();
    c.set_reg(AX, r);
    c.cmp(W, page, 0xaa);
    if !first {
        let a = c.and(W, r, 0x0c);
        let a = c.shr1(W, a, 2);
        let a = c.add(W, a, 0x103);
        c.set_reg(AX, a);
        c.set_reg(BX, 0x8b);
        c.set_reg(DX, 0xfa);
        c.set_reg(CX, 0x64);
        c.call(0xd194, 0xb0d4)?;
    }
    c.run(0xb0d4, 3)?;
    c.set_reg(BX, 3);
    c.set_reg(DX, 0x132);
    c.call(0xd04e, 0xb0dd)?;
    let page = c.word(0x11bf);
    let n = (page.wrapping_sub(0xaa) >> 1).wrapping_add(1) as u8;
    let tens = n / 10;
    c.run(0xb0dd, 10 + if tens == 0 { 1 } else { 0 } + 2)?;
    c.set_word(0xdbe4, 0x53);
    let a = c.sub(W, page, 0xaa);
    let a = c.shr1(W, a, 1);
    let a = c.inc(W, a);
    c.set_reg(AX, a);
    c.aam(10);
    let ax = c.reg(AX);
    let ax = c.add(W, ax, 0x3030).swap_bytes();
    c.cmp(B, ax & 0xff, 0x30);
    let ax = if ax & 0xff == 0x30 { ax & 0xff00 | 0x20 } else { ax };
    c.set_reg(AX, ax);
    c.push(ax);
    c.call(0xd12f, 0xb0fd)?;
    c.run(0xb0fd, 3)?;
    let ax = c.pop();
    c.set_reg(AX, ax & 0xff00 | ax >> 8);
    c.call(0xd12f, 0xb103)?;
    let icons = c.word(0x243e);
    c.run(0xb103, 4 + if icons != 0 { 7 } else { 1 })?;
    c.set_byte(0x1c30, 0);
    c.set_reg(AX, icons);
    c.or(W, icons, icons);
    if icons == 0 {
        return c.ret();
    }
    c.set_byte(0x1c30, 0x80);
    c.set_reg(SI, 0x2412);
    let a = c.add(W, icons, 0xfff5);
    c.set_reg(AX, a);
    c.set_word(0x2412, a);
    c.set_word(0x2418, a);
    c.set_word(0x241e, a);
    c.call(0xc21b, 0xb125)?;
    c.run(0xb125, 1)?;
    c.ret()
}

/// `loc_08ccd`: the text box for the phrase SI: [4799h] 9, colour F0h,
/// then the box (BP, on the stack segment) and its margins [4784h..478Ah]
/// by the situation: the map views 2244h (the troop's description
/// `loc_079ee` first unless [46EFh]); no speaker 224Ch; [0C6h] 2265h in
/// colour 64h (`loc_0d082`); at night 2275h in colour 6, no margins;
/// without subtitles ([28E7h] 0) 223Ch in colour 0Fh ([4799h] 1); else
/// the first of the 3 boxes from 2224h whose height takes the broken
/// lines (`break_lines`, [0A9D0h] lines of 0Ah plus the margins), a
/// smaller one kept by a coin toss, and with none fitting (the last one,
/// its margins dropped, ZF from the old [478Ah]) returning broken. The
/// lines broken for the box's width less the margins ([478Fh]); CF clear.
pub fn text_box_for(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8ccd)?;
    let mut c = Ctx::new(m);
    let v = c.byte(0x46eb);
    let head = |c: &mut Ctx| {
        c.set_byte(0x4799, 9);
        c.set_word(0xdbe4, 0xf0);
    };
    if v != 0 {
        let t = c.word(0x46ef);
        c.run(start, 6 + if t == 0 { 1 } else { 0 })?;
        head(&mut c);
        c.cmp(B, v as u16, 0);
        c.cmp(W, t, 0);
        if t == 0 {
            c.call(0xe270, 0x8ce9)?;
            c.run(0x8ce9, 3)?;
            let es = c.sreg(ES);
            c.push(es);
            let si = c.word(0x46f1);
            c.set_reg(SI, si);
            c.call(0x79ee, 0x8cf1)?;
            c.run(0x8cf1, 2)?;
            let es = c.pop();
            c.set_sreg(ES, es);
            c.call(0xe283, 0x8cf5)?;
        }
        c.run(0x8cf5, 2)?;
        c.set_reg(BP, 0x2244);
        return box_8ddb(&mut c);
    }
    let sp = c.word(TALKER);
    if sp == 0xffff {
        c.run(start, 15)?;
        head(&mut c);
        c.cmp(B, 0, 0);
        c.cmp(W, sp, 0xffff);
        margins(&mut c, 0x48, 0x10, 8, 8);
        c.set_reg(BP, 0x224c);
        return box_8ddb(&mut c);
    }
    let k = c.byte(0xc6);
    if k != 0 {
        c.run(start, 18)?;
        head(&mut c);
        c.cmp(B, 0, 0);
        c.cmp(W, sp, 0xffff);
        c.cmp(B, k as u16, 0);
        c.set_reg(BP, 0x2265);
        margins(&mut c, 0x3c, 0x32, 0x0a, 0x0a);
        c.set_byte(0xdbe4, 0x64);
        c.call(0xd082, 0x8d40)?;
        c.run(0x8d40, 1)?;
        return box_8ddb(&mut c);
    }
    let night = c.byte(0x227d);
    if night != 0 {
        c.run(start, 18)?;
        head(&mut c);
        c.cmp(B, 0, 0);
        c.cmp(W, sp, 0xffff);
        c.cmp(B, 0, 0);
        c.cmp(B, night as u16, 0);
        c.set_byte(0xdbe4, 6);
        c.set_reg(BP, 0x2275);
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
        for a in [0x4788u16, 0x478a, 0x4784, 0x4786] {
            c.set_word(a, 0);
        }
        return box_8ddb(&mut c);
    }
    let sub = c.byte(0x28e7);
    let pre = |c: &mut Ctx| {
        head(c);
        c.cmp(B, 0, 0);
        c.cmp(W, sp, 0xffff);
        c.cmp(B, 0, 0);
        c.cmp(B, 0, 0);
        c.cmp(B, sub as u16, 0);
    };
    if sub == 0 {
        c.run(start, 23)?;
        pre(&mut c);
        c.set_byte(0x4799, 1);
        c.set_byte(0xdbe4, 0x0f);
        c.set_reg(BP, 0x223c);
        c.xor(W, 0, 0);
        c.set_word(0x478a, 0);
        let a = c.inc(W, 0);
        c.set_word(0x4788, a);
        c.set_reg(AX, 0x10);
        c.set_word(0x4784, 0x10);
        c.set_word(0x4786, 0x10);
        return box_8ddb(&mut c);
    }
    c.run(start, 14)?;
    pre(&mut c);
    c.set_reg(BP, 0x2224);
    c.set_reg(CX, 3);
    let mut at = 0x8d90u16;
    loop {
        let bp = c.reg(BP);
        c.run(at, 7)?;
        width_478f(&mut c, bp);
        let (si, cx) = (c.reg(SI), c.reg(CX));
        c.push(si);
        c.push(cx);
        c.call(0x8e16, 0x8da3)?;
        let lines = c.word(0xa9d0) as u8;
        let h = (lines as u16) * 10;
        let (m1, m2) = (c.word(0x4788), c.word(0x478a));
        let need = h.wrapping_add(m1).wrapping_add(m2);
        let bp = c.reg(BP);
        let cap = c.word_in(SS, bp.wrapping_add(6));
        let fits = need < cap;
        let cx = c.word_in(SS, c.reg(SP));
        let last = !fits && cx.wrapping_sub(1) == 0;
        c.run(0x8da3, 9 + if fits { 1 } else { 2 } + if last { 6 } else { 0 })?;
        let cx = c.pop();
        c.set_reg(CX, cx);
        let s = c.pop();
        c.set_reg(SI, s);
        c.set_reg(AX, 0x0a00 | lines as u16);
        c.mul8(0x0a);
        let a = c.add(W, h, m1);
        let a = c.add(W, a, m2);
        c.set_reg(AX, a);
        c.cmp(W, a, cap);
        if fits {
            let cx = c.dec(W, cx);
            c.set_reg(CX, cx);
            if cx == 0 {
                c.run(0x8dce, 3)?;
                c.set_flag(pc::cpu::CF, false);
                return c.ret();
            }
            c.run(0x8dce, 3)?;
            c.set_reg(BX, 1);
            c.call(0xe3b7, 0x8dd6)?;
            if c.flag(pc::cpu::ZF) {
                c.run(0x8dd6, 3)?;
                c.set_flag(pc::cpu::CF, false);
                return c.ret();
            }
            c.run(0x8dd6, 2)?;
            let bp = c.reg(BP);
            let b = c.add(W, bp, 8);
            c.set_reg(BP, b);
            return box_8ddb(&mut c);
        }
        let b = c.add(W, bp, 8);
        c.set_reg(BP, b);
        let cx = cx.wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            let b = c.sub(W, b, 8);
            c.set_reg(BP, b);
            c.xor(W, 0, 0);
            c.set_word(0x4788, 0);
            let old = c.word(0x478a);
            c.set_word(0x478a, 0);
            c.set_reg(AX, old);
            c.or(W, old, old);
            return c.ret();
        }
        at = 0x8d90;
    }
}

/// The margins [4784h], [4786h], [4788h], [478Ah] (the `mov al` steps).
fn margins(c: &mut Ctx, a: u16, b: u16, d: u16, e: u16) {
    c.set_reg(AX, a);
    c.set_word(0x4784, a);
    let ax = c.reg(AX) & 0xff00 | b;
    c.set_reg(AX, ax);
    c.set_word(0x4786, ax);
    let ax = ax & 0xff00 | d;
    c.set_reg(AX, ax);
    c.set_word(0x4788, ax);
    let _ = e;
    c.set_word(0x478a, ax);
}

/// The width for the box BP less the margins into [478Fh].
fn width_478f(c: &mut Ctx, bp: u16) {
    let w = c.word_in(SS, bp.wrapping_add(4));
    let (a, b) = (c.word(0x4784), c.word(0x4786));
    let x = c.sub(W, w, a);
    let x = c.sub(W, x, b);
    c.set_reg(AX, x);
    c.set_word(0x478f, x);
}

/// From 8DDBh: the lines broken for the box BP; CF clear.
fn box_8ddb(c: &mut Ctx) -> Result<(), Bail> {
    let bp = c.reg(BP);
    c.run(0x8ddb, 6)?;
    width_478f(c, bp);
    let si = c.reg(SI);
    c.push(si);
    c.call(0x8e16, 0x8ded)?;
    c.run(0x8ded, 3)?;
    let s = c.pop();
    c.set_reg(SI, s);
    c.set_flag(pc::cpu::CF, false);
    c.ret()
}

/// `setup_lip_sync_data_from_current`: [`setup_lip_sync_data_from_sprite_sheet`]
/// for the speaker [47C4h], if any.
pub fn setup_lip_sync_data_from_current(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9197)?;
    let mut c = Ctx::new(m);
    let sp = c.word(TALKER);
    if sp == 0xffff {
        c.run(start, 4)?;
        c.set_reg(AX, sp);
        c.cmp(W, sp, 0xffff);
        return c.ret();
    }
    if c.flag(pc::cpu::DF) {
        return Err(Bail);
    }
    c.run(start, 3)?;
    c.set_reg(AX, sp);
    c.cmp(W, sp, 0xffff);
    lip_sync_setup_at(&mut c)
}

/// `setup_lip_sync_data_from_sprite_sheet`: the talking head for the
/// speaker AX ([0F0h] 0, 0Ah for 0Ch with bit 10h of [10A7h]): its
/// sentence set (`pick_sentence`); a new head ([22A6h]) closes the text
/// first and opens its resource ([`open_talking_head_resource`]), its
/// text width (22A8h table) into the three boxes 2224h.., the speaker
/// box 1BF0h from the sheet's first sprite, and the lip sync tables
/// ([47CCh], [47CAh], [47D2h]). Bails with DF set.
pub fn setup_lip_sync_data_from_sprite_sheet(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x91a0)?;
    let mut c = Ctx::new(m);
    if c.flag(pc::cpu::DF) {
        return Err(Bail);
    }
    lip_sync_setup_at(&mut c)
}

fn lip_sync_setup_at(c: &mut Ctx) -> Result<(), Bail> {
    let ax = c.reg(AX);
    let f = c.byte(0x10a7);
    let n = if ax != 0x0c { 3 } else if f & 0x10 == 0 { 5 } else { 6 };
    c.run(0x91a0, n + 1)?;
    c.set_word(0xf0, 0);
    c.cmp(W, ax, 0x0c);
    if ax == 0x0c {
        c.test(B, f as u16, 0x10);
        if f & 0x10 != 0 {
            c.set_word(0xf0, 0x0a);
        }
    }
    c.call(0x9123, 0x91bb)?;
    let ax = c.reg(AX);
    let cur = c.word(0x22a6);
    if ax == cur {
        c.run(0x91bb, 2)?;
        c.cmp(W, ax, cur);
        return open_talking_head_resource(c.m, 0x920f);
    }
    c.run(0x91bb, 4)?;
    c.cmp(W, ax, cur);
    c.push(ax);
    c.call(0x98b2, 0x91c5)?;
    c.run(0x91c5, 4)?;
    let a = c.pop();
    c.set_reg(AX, a);
    c.set_word(0x22a6, a);
    c.set_reg(SI, a);
    c.call(0x920f, 0x91ce)?;
    c.run(0x91ce, 28)?;
    let si = c.reg(SI);
    let w = c.byte(si.wrapping_add(0x22a8));
    c.xor(B, 0, 0);
    c.set_reg(AX, w as u16);
    for a in [0x2224u16, 0x222c, 0x2234] {
        c.set_word(a, w as u16);
    }
    let ds = c.sreg(DS);
    c.push(ds);
    c.push(ds);
    let e = c.pop();
    c.set_sreg(ES, e);
    let (p, s) = (c.word(0xdbb0), c.word(0xdbb2));
    c.set_reg(SI, p);
    c.set_sreg(DS, s);
    let bx = c.word(p);
    c.set_reg(BX, bx);
    let o = c.word(bx.wrapping_add(p).wrapping_sub(2));
    let si = c.add(W, p, o);
    let si = c.add(W, si, 4);
    c.set_reg(SI, si);
    c.set_reg(DI, 0x1bf0);
    for _ in 0..4 {
        c.movs(W);
    }
    let si = c.reg(SI);
    c.set_reg(AX, si);
    let a = c.add(W, si, 2);
    c.set_reg(AX, a);
    c.set_word_in(SS, 0x47cc, a);
    let o = c.word(si);
    let si = c.add(W, si, o);
    c.set_reg(SI, si);
    let bx = c.word(si);
    c.set_reg(BX, bx);
    let o = c.word(bx.wrapping_add(si).wrapping_sub(2));
    let di = c.add(W, si, o);
    c.set_reg(DI, di);
    let d = c.pop();
    c.set_sreg(DS, d);
    c.set_word(0x47ca, si);
    c.set_word(0x47d2, di);
    c.ret()
}

/// `loc_090bd`: the dialogue list 1F7Eh's second item (+6 the phrase BX,
/// +8 its handler DX) for the speaker SI (its number in byte 0Eh, AL):
/// with bit 1000h of [12h] (not for 0Ch) and bit 10h of [10A7h]: 9Ch,
/// 9584h. Else 0Fh: 93h, 5A03h; 0Eh: 96h (97h with bit 10h of [0Ah]),
/// 95C1h; others by their flags (byte 0Fh, CL): 4091h moving (bit 80h,
/// with 5A03h), else 91h, 95E2h, or with bit 40h 92h, 9533h. Then a
/// redraw asked (`loc_0d316`, BP the list) and the list installed with
/// the handler 97CFh.
pub fn speaker_list_item(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x90bd)?;
    let mut c = Ctx::new(m);
    let record = c.reg(SI);
    let speaker = c.byte(record.wrapping_add(0x0e));
    let flags = c.byte(record.wrapping_add(0x0f));
    let special = speaker != 0x0c && c.word(0x12) & 0x1000 != 0;
    let (mut phrase, mut handler) = (c.reg(BX), c.reg(DX));
    let mut by_flags = false;
    let mut cost = 3 + if speaker != 0x0c { 2 } else { 0 };
    if special {
        cost += 4;
        (phrase, handler) = (0x9c, 0x9584);
    }
    if !(special && c.byte(0x10a7) & 0x10 == 0) {
        cost += 4;
        (phrase, handler) = (0x93, 0x5a03);
        if speaker != 0x0f {
            cost += 2;
            if speaker == 0x0e {
                cost += 4;
                (phrase, handler) = (0x96, 0x95c1);
                if c.byte(0x0a) & 0x10 != 0 {
                    cost += 2;
                    phrase = 0x97;
                }
            } else {
                cost += 4;
                by_flags = true;
                phrase = 0x4091;
                if flags & 0x80 == 0 {
                    cost += 4;
                    (phrase, handler) = (0x91, 0x95e2);
                    if flags & 0x40 != 0 {
                        cost += 2;
                        (phrase, handler) = (0x92, 0x9533);
                    }
                }
            }
        }
    }
    c.run(start, cost + 4)?;
    if by_flags {
        c.set_reg8(r8::CL, flags);
    }
    c.set_reg8(r8::AL, speaker);
    c.set_reg(BX, phrase);
    c.set_reg(DX, handler);
    c.set_reg(BP, 0x1f7e);
    c.set_word_in(SS, 0x1f84, phrase);
    c.set_word_in(SS, 0x1f86, handler);
    c.call(0xd316, 0x911d)?;
    c.run(0x911d, 2)?;
    c.set_reg(BX, 0x97cf);
    super::cursor::install_ui_list_d338(c.m, 0xd338)
}

/// `loc_09215`: a click in the dialogue: in the action list 1F0Eh (room
/// mode) on a speaker (`loc_09285`, CF; CL its record) its record's
/// handler (+4 of the record FD8h + 10h * CL), from 0Fh the phrase
/// CL - 0Fh (`loc_09381`); 2Fh goes on at `loc_042e9` (not ported:
/// bails). Off the speakers in the first room of a place (not a
/// palace's, by day) below y 98h the move up (`ui_click_room_up`). In the
/// speaker list 1F7Eh (`loc_092c9`): the current phrase [47C4h] again
/// goes to the next line (`sub_945b`); another the panel reset and its
/// handler.
pub fn dialogue_click(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9215)?;
    let mut c = Ctx::new(m);
    c.run(0x9215, 1)?;
    c.call(0xd41b, 0x9218)?;
    let bp = c.reg(BP);
    if bp == 0x1f0e {
        let mode = c.byte(0x11c9);
        c.run(0x9218, 5)?;
        c.cmp(W, bp, 0x1f0e);
        c.cmp(B, mode as u16, 0);
        if mode != 0 {
            return c.ret();
        }
        c.call(0x9285, 0x9228)?;
        if !c.flag(pc::cpu::CF) {
            let b = c.byte(0x0b);
            let bx = c.reg(BX);
            let b8 = c.byte(8);
            let night = c.byte(0x2b);
            let n = if b != 1 {
                4
            } else if bx >= 0x98 {
                6
            } else if b8 == 0x21 {
                8
            } else {
                10
            };
            c.run(0x9228, n)?;
            c.cmp(B, b as u16, 1);
            if b == 1 {
                c.cmp(W, bx, 0x98);
                if bx < 0x98 {
                    c.cmp(B, b8 as u16, 0x21);
                    if b8 != 0x21 {
                        c.cmp(B, night as u16, 0);
                        if night == 0 {
                            return super::story::room_move(c.m, 0x3f15);
                        }
                    }
                }
            }
            return c.ret();
        }
        let cl = c.reg8(r8::CL);
        if cl == 0x2f {
            c.m.cpu.ip = 0x9228;
            return Err(Bail);
        }
        if cl < 0x0f {
            c.run(0x9228, 5)?;
            c.cmp(B, cl as u16, 0x2f);
            c.cmp(B, cl as u16, 0x0f);
            return record_handler_9234(&mut c);
        }
        c.run(0x9228, 8)?;
        c.cmp(B, cl as u16, 0x2f);
        c.cmp(B, cl as u16, 0x0f);
        let v = c.sub(B, cl as u16, 0x0f) as u8;
        c.set_reg8(r8::CL, v);
        c.set_reg8(r8::AL, v);
        return phrase_9381(&mut c);
    }
    if bp != 0x1f7e {
        c.run(0x9218, 5)?;
        c.cmp(W, bp, 0x1f0e);
        c.cmp(W, bp, 0x1f7e);
        return c.ret();
    }
    c.run(0x9218, 5)?;
    c.cmp(W, bp, 0x1f0e);
    c.cmp(W, bp, 0x1f7e);
    c.call(0x92c9, 0x9251)?;
    if !c.flag(pc::cpu::CF) {
        c.run(0x9251, 2)?;
        return c.ret();
    }
    let cx = c.reg(CX);
    let p = c.word(TALKER);
    if cx == p {
        c.run(0x9251, 4)?;
        c.cmp(W, cx, p);
        return dialogue_line(c.m, 0x945b);
    }
    c.run(0x9251, 5)?;
    c.cmp(W, cx, p);
    c.push(cx);
    c.call(0xd2bd, 0x9260)?;
    c.run(0x9260, 2)?;
    let cx = c.pop();
    c.set_reg(CX, cx);
    record_handler_9234(&mut c)
}

/// `loc_09234`: the handler of the record CL (+4 of FD8h + 10h * CL).
fn record_handler_9234(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x9234, 5)?;
    let cl = c.reg8(r8::CL);
    c.set_reg8(r8::AL, 0x10);
    c.mul8(cl);
    let ax = c.reg(AX);
    let si = c.add(W, ax, 0x0fd8);
    c.set_reg(AX, si);
    c.set_reg(SI, si);
    let to = c.word(si.wrapping_add(4));
    c.jump(to)
}

/// `loc_09381`: the phrase AL (0 from 9; 8 is the one before [476Bh],
/// 0 when none) of the table 4758h: [476Ch] noted, its record's phase
/// (`loc_01ebe`) and the troop described (`loc_031f6`), then the
/// phrase 0Fh said (`loc_093aa`).
fn phrase_9381(c: &mut Ctx) -> Result<(), Bail> {
    let al = c.reg8(r8::AL);
    let a1 = if al >= 9 { 0 } else { al };
    let k = c.byte(0x476b);
    let v = k.wrapping_sub(1);
    let a2 = if a1 != 8 { a1 } else if v & 0x80 != 0 { 0 } else { v };
    let n = 2 + (al >= 9) as u32 + 2 + if a1 == 8 { 3 + (v & 0x80 != 0) as u32 } else { 0 } + 7;
    c.run(0x9381, n)?;
    c.cmp(B, al as u16, 9);
    if al >= 9 {
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
    }
    c.cmp(B, a1 as u16, 8);
    if a1 == 8 {
        c.set_reg8(r8::AL, k);
        c.dec(B, k as u16);
        c.set_reg8(r8::AL, v);
        if v & 0x80 != 0 {
            c.xor(W, 0, 0);
            c.set_reg(AX, 0);
        }
    }
    c.set_byte(0x476c, a2);
    c.xor(B, 0, 0);
    let ax = a2 as u16;
    let ax2 = c.add(W, ax, ax);
    c.set_reg(AX, ax2);
    let si = c.add(W, 0x4758, ax2);
    let si = c.word(si);
    c.set_reg(SI, si);
    c.call(0x1ebe, 0x93a5)?;
    c.run(0x93a5, 1)?;
    c.call(0x31f6, 0x93a8)?;
    c.run(0x93a8, 1)?;
    c.set_reg8(r8::AL, 0x0f);
    phrase_93aa(c)
}

/// `loc_093aa`: the phrase AL said: [47E1h] 0, [47C4h] the phrase, the
/// speaker's lips and head (`setup_lip_sync_data_from_sprite_sheet`,
/// `loc_03af9`, `..._from_current`, `loc_09908`), the text [47C8h]
/// (`loc_09bac`, [4540h] 0), the frame (`loc_01834`, `loc_0c0f4`,
/// `loc_0c4dd`), the phrase's line (`loc_093df`); then the next line.
fn phrase_93aa(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x93aa, 5)?;
    c.xor(B, 0, 0);
    let ax = c.reg8(r8::AL) as u16;
    c.set_reg(AX, ax);
    c.set_word(0x47e1, 0);
    c.push(ax);
    c.set_word(TALKER, ax);
    c.call(0x91a0, 0x93b9)?;
    for (at, to) in [(0x93b9u16, 0x3af9u16), (0x93bc, 0x9197), (0x93bf, 0x9908)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    c.run(0x93c2, 3)?;
    let si = c.word(0x47c8);
    c.set_reg(SI, si);
    c.set_word(0x4540, 0);
    c.call(0x9bac, 0x93cf)?;
    for (at, to) in [(0x93cfu16, 0x1834u16), (0x93d2, 0xc0f4), (0x93d5, 0xc4dd)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    c.run(0x93d8, 2)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.call(0x93df, 0x93dc)?;
    c.run(0x93dc, 1)?;
    next_line_from_9472(c)
}

/// `loc_08b11`: the text SI laid out in its box: the old box closed
/// (`loc_08c8a`), the new one placed (`loc_08ccd`; CF: no room, done),
/// drawn (`draw_speech_bubble`, `loc_08df0`) and the pen at [4791h] /
/// [4793h]. The layout A9D0h (line count, then per line its width
/// count, offset and spacing) is placed by [4799h]: bits 2-3 spread the
/// lines (4: the line height [479Ah] from the room [478Dh]; 8 / 0Ch:
/// centred, 8 halved), bit 1 centres the first line (once). Each line:
/// its words (space / CR runs end a word, a negative byte the line),
/// 1 the colour pair [0DBE4h] swapped, 6 / 8 the font switches
/// (`loc_0d075` / `loc_0d068`), others drawn ([2518h]); words spaced by
/// DX ([479Ch] of them one wider). Lines step by [479Ah] (taken from
/// [478Dh]); at the end the pen kept, SI back on the last byte, and for
/// the picture 223Ch ([479Eh]) its outline (`loc_09046`) and the back
/// buffer made current.
pub fn layout_text(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8b11)?;
    let mut c = Ctx::new(m);
    c.run(0x8b11, 2)?;
    let si = c.reg(SI);
    c.push(si);
    c.call(0x8c8a, 0x8b15)?;
    c.run(0x8b15, 2)?;
    let si = c.pop();
    c.set_reg(SI, si);
    c.call(0x8ccd, 0x8b19)?;
    if c.flag(pc::cpu::CF) {
        c.run(0x8b19, 2)?;
        return c.ret();
    }
    c.run(0x8b19, 2)?;
    c.call(0x8f28, 0x8b1e)?;
    c.run(0x8b1e, 1)?;
    c.call(0x8df0, 0x8b21)?;
    c.run(0x8b21, 3)?;
    let (dx, bx) = (c.word(0x4791), c.word(0x4793));
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.call(0xd04e, 0x8b2c)?;
    let f = c.byte(0x4799);
    let a = f & 0x0c;
    let w0 = c.word_in(SS, 0xa9d0);
    let room = c.word(0x478d);
    let head = |c: &mut Ctx| {
        c.set_reg(BP, 0xa9d0);
        c.set_word(0x479a, 0x0a);
        c.set_reg8(r8::AL, f);
        c.and(B, f as u16, 0x0c);
        c.set_reg8(r8::AL, a);
    };
    if a == 0 {
        c.run(0x8b2c, 5)?;
        head(&mut c);
    } else if a < 8 {
        let b1 = w0.wrapping_sub(1);
        c.run(0x8b2c, 5 + 2 + 6 + if b1 != 0 { 3 } else { 5 })?;
        head(&mut c);
        c.cmp(B, a as u16, 8);
        c.set_reg(BX, w0);
        c.xor(W, 0, 0);
        c.set_reg(DX, 0);
        c.set_reg(AX, room);
        let ax = c.sub(W, room, 8);
        c.set_reg(AX, ax);
        c.dec(W, w0);
        c.set_reg(BX, b1);
        if b1 != 0 {
            c.m.cpu.mul_div(&mut c.m.hw, W, 6, b1 as u32);
            let q = c.reg(AX);
            c.set_word(0x479a, q);
        } else {
            let h = c.shr1(W, ax, 1);
            c.set_reg(AX, h);
            let x = c.word(0xd82c);
            c.set_reg(DX, x);
            let y = c.word(0xd82e);
            let b = c.add(W, h, y);
            c.set_reg(BX, b);
            c.call(0xd04e, 0x8b64)?;
            c.run(0x8b64, 1)?;
        }
    } else {
        let prod = (w0 & 0xff) * 10;
        let under = room < prod;
        c.run(0x8b2c, 5 + 2 + 7 + under as u32 + 3 + (a == 8) as u32 + 3)?;
        head(&mut c);
        c.cmp(B, a as u16, 8);
        let fl = c.m.cpu.flags();
        c.push(fl);
        c.set_reg(AX, w0);
        c.set_reg8(r8::AH, 0x0a);
        c.mul8(0x0a);
        c.set_reg(BX, room);
        let mut bx = c.sub(W, room, prod);
        if under {
            c.xor(W, bx, bx);
            bx = 0;
        }
        c.set_reg(BX, bx);
        c.set_word(0x478d, prod);
        let fl = c.pop();
        c.m.cpu.set_flags(fl);
        if a == 8 {
            bx = c.shr1(W, bx, 1);
            c.set_reg(BX, bx);
        }
        let x = c.word(0xd82c);
        c.set_reg(DX, x);
        let y = c.word(0xd82e);
        let b = c.add(W, bx, y);
        c.set_reg(BX, b);
        c.call(0xd04e, 0x8b8b)?;
    }
    c.run(0x8b8b, 2)?;
    let bp = c.reg(BP);
    let n = c.word_in(SS, bp);
    c.set_reg(DX, n);
    let bp = c.add(W, bp, 2);
    c.set_reg(BP, bp);
    loop {
        // 8B91h: a line.
        let bp = c.reg(BP);
        let cx = c.word_in(SS, bp);
        let dx2 = c.word_in(SS, bp.wrapping_add(2));
        let bx2 = c.word_in(SS, bp.wrapping_add(4));
        c.run(0x8b91, 7 + (cx == 0) as u32)?;
        let dx = c.reg(DX);
        c.push(dx);
        c.set_reg(CX, cx);
        let b = c.add(W, bp, 2);
        c.set_reg(DX, dx2);
        c.set_reg(BX, bx2);
        let b = c.add(W, b, 4);
        c.set_reg(BP, b);
        if cx != 0 {
            line_8ba3(&mut c, cx, dx2, bx2)?;
        }
        // 8C47h: the next line.
        let step = c.word(0x479a);
        let room = c.word(0x478d);
        let under = room < step;
        c.run(0x8c47, 6 + under as u32 + 1)?;
        let x = c.word(0xd830);
        c.set_reg(DX, x);
        let y = c.word(0xd832);
        c.set_reg(BX, y);
        c.set_reg(AX, step);
        let b = c.add(W, y, step);
        c.set_reg(BX, b);
        let r = c.sub(W, room, step);
        c.set_word(0x478d, if under { 0 } else { r });
        c.call(0xd04e, 0x8c63)?;
        let sp = c.reg(SP);
        let left = c.word_in(SS, sp).wrapping_sub(1);
        c.run(0x8c63, 3 + (left != 0) as u32)?;
        c.pop();
        c.dec(W, left.wrapping_add(1));
        c.set_reg(DX, left);
        if left == 0 {
            break;
        }
    }
    let k = c.word(BOX_SHOWN);
    c.run(0x8c6a, 7 + 1)?;
    let (x, y) = (c.word(0xd830), c.word(0xd832));
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    c.set_word(0x4791, x);
    c.set_word(0x4793, y);
    let si = c.reg(SI);
    let si = c.dec(W, si);
    c.set_reg(SI, si);
    c.cmp(W, k, 0x223c);
    if k != 0x223c {
        return c.ret();
    }
    c.call(0x9046, 0x8c86)?;
    c.run(0x8c86, 1)?;
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `loc_08ba3`: a line of CX words with the offset DX / spacing BX (see
/// [`layout_text`]).
fn line_8ba3(c: &mut Ctx, cx: u16, dx2: u16, bx2: u16) -> Result<(), Bail> {
    let f = c.byte(0x4799);
    if f & 2 != 0 {
        let cl = cx as u8;
        let d = cl.wrapping_sub(1);
        c.run(0x8ba3, 2 + 4 + (d != 0) as u32 + 12)?;
        c.test(B, f as u16, 2);
        c.set_reg8(r8::AL, dx2 as u8);
        c.set_reg8(r8::DL, cl);
        c.dec(B, cl as u16);
        c.set_reg8(r8::DL, d);
        if d != 0 {
            c.mul8(d);
        }
        let al = c.reg8(r8::AL);
        let s = c.add(B, al as u16, bx2 & 0xff);
        c.set_reg8(r8::AL, s as u8);
        c.set_reg8(r8::DL, s as u8);
        c.set_reg8(r8::AL, cl);
        let a3 = c.dec(B, cl as u16);
        c.xor(B, 0, 0);
        let ax = a3;
        let ax = c.shl1(W, ax, 1);
        let dx = c.reg(DX);
        let dx = c.sub(W, dx, ax);
        let ax = c.shl1(W, ax, 1);
        c.set_reg(AX, ax);
        let dx = c.sub(W, dx, ax);
        let dx = c.shr1(W, dx, 1);
        c.set_reg(DX, dx);
        let v = c.word(0xd82c);
        let v = c.add(W, v, dx);
        c.set_word(0xd82c, v);
        let g = c.and(B, f as u16, 0xfe);
        c.set_byte(0x4799, g as u8);
    } else {
        c.run(0x8ba3, 2)?;
        c.test(B, f as u16, 2);
    }
    let sp = c.reg(SP);
    let lines = c.word_in(SS, sp);
    let f = c.byte(0x4799);
    let plain = lines == 1 || f & 1 == 0;
    c.run(0x8bd1, 4 + if lines == 1 { 0 } else { 2 } + if plain { 2 } else { 0 } + 1)?;
    c.pop();
    c.set_reg(AX, lines);
    c.push(lines);
    c.cmp(W, lines, 1);
    if lines != 1 {
        c.test(B, f as u16, 1);
    }
    if plain {
        c.set_reg(DX, 6);
        c.set_reg(BX, 0);
    }
    let bx = c.reg(BX);
    c.set_word(0x479c, bx);
    // 8BE9h: the characters.
    loop {
        let si = c.reg(SI);
        let al = c.byte(si);
        let tests: &[u8] = &[0x20, 0x0d, 0x06, 0x08, 0x01];
        let hit = tests.iter().position(|&t| t == al);
        let cmps = |c: &mut Ctx, upto: usize| {
            c.lods(B, DS);
            c.or(B, al as u16, al as u16);
            for &t in &tests[..upto] {
                c.cmp(B, al as u16, t as u16);
            }
        };
        if al & 0x80 != 0 {
            c.run(0x8be9, 3)?;
            cmps(c, 0);
        } else {
            match hit {
                Some(0) | Some(1) => {
                    let k = hit.unwrap();
                    c.run(0x8be9, 3 + 2 * (k as u32 + 1))?;
                    cmps(c, k + 1);
                    skip_blanks_8c19(c)?;
                }
                Some(2) | Some(3) => {
                    let k = hit.unwrap();
                    c.run(0x8be9, 3 + 2 * (k as u32 + 1) + 1)?;
                    cmps(c, k + 1);
                    let (to, ret) = if k == 2 { (0xd075u16, 0x8c12u16) } else { (0xd068, 0x8c17) };
                    c.call(to, ret)?;
                    c.run(ret, 1)?;
                    continue;
                }
                Some(_) => {
                    c.run(0x8be9, 3 + 10 + 4)?;
                    cmps(c, 5);
                    let v = c.word(0xdbe4);
                    let v = v.rotate_left(8);
                    c.set_reg(AX, v);
                    c.set_word(0xdbe4, v);
                    continue;
                }
                None => {
                    c.run(0x8be9, 3 + 10 + 1)?;
                    cmps(c, 5);
                    let to = c.word(0x2518);
                    c.call(to, 0x8c45)?;
                    c.run(0x8c45, 1)?;
                    continue;
                }
            }
        }
        // 8C26h: a word ends.
        let cx = c.reg(CX).wrapping_sub(1);
        if cx == 0 {
            c.run(0x8c26, 2)?;
            c.dec(W, 1);
            c.set_reg(CX, 0);
            return Ok(());
        }
        let k = c.word(0x479c);
        c.run(0x8c26, 2 + 4 + if k != 0 { 2 } else { 0 } + 3)?;
        let cx0 = c.reg(CX);
        c.dec(W, cx0);
        c.set_reg(CX, cx);
        let dx = c.reg(DX);
        c.push(dx);
        let p = c.word(0xd82c);
        let mut x = c.add(W, dx, p);
        c.cmp(W, k, 0);
        if k != 0 {
            x = c.inc(W, x);
            let k2 = c.dec(W, k);
            c.set_word(0x479c, k2);
        }
        c.set_reg(DX, x);
        c.set_word(0xd82c, x);
        let dx = c.pop();
        c.set_reg(DX, dx);
    }
}

/// `loc_08c19`: the blanks after a word skipped (spaces and CRs), then
/// the word's end (`loc_08c26`, counted by the caller's loop).
fn skip_blanks_8c19(c: &mut Ctx) -> Result<(), Bail> {
    loop {
        let si = c.reg(SI);
        let b = c.byte(si);
        if b == 0x20 {
            c.run(0x8c19, 4)?;
            c.cmp(B, b as u16, 0x20);
        } else if b == 0x0d {
            c.run(0x8c19, 6)?;
            c.cmp(B, b as u16, 0x20);
            c.cmp(B, b as u16, 0x0d);
        } else {
            c.run(0x8c19, 4)?;
            c.cmp(B, b as u16, 0x20);
            c.cmp(B, b as u16, 0x0d);
            return Ok(());
        }
        let si = c.inc(W, si);
        c.set_reg(SI, si);
    }
}

/// `format_interpolated_string`: the phrase DS:SI copied to ES:DI (=
/// DS) with its codes expanded. Leading spaces skipped; bytes below 80h
/// copied. 80h + word and 81h-8Fh (the words at 11EBh) open another
/// phrase (`loc_08a3b`, `loc_0cf70`: its DS:SI, the current one kept on
/// a stack of 0Ch levels at SP); 90h-9Fh print the number at the byte's
/// address (92h a word, else its low byte; `loc_08acc`, `loc_08a23`,
/// leading zeros dropped); A0h-CFh are copied; D0h-EFh with their
/// arguments (D0h two bytes, D1h four, from D2h one); from F0h a
/// phrase ends: the enclosing one goes on, or at the top the end code
/// is stored and the phrase's end noted in [47B6h] / [47B8h] (0 for
/// FFh); with bit 4 of [47DEh] a wait and `loc_08ac3`. [477Fh] the last
/// code.
pub fn format_interpolated_string(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::ES;
    entered(start, 0x8944)?;
    let mut c = Ctx::new(m);
    c.run(0x8944, 4)?;
    let sp = c.reg(SP);
    let sp = pure::sub(W, sp, 0x32);
    c.set_reg(SP, sp);
    c.set_reg(BP, sp);
    let ds = c.sreg(DS);
    c.push(ds);
    let v = c.pop();
    c.set_sreg(ES, v);
    let mut at = 0x894bu16;
    loop {
        match at {
            0x894b => {
                let si = c.reg(SI);
                let b = c.byte(si);
                if b == 0x20 {
                    c.run(0x894b, 4)?;
                    let si = pure::inc(W, si);
                    c.set_reg(SI, si);
                } else {
                    c.run(0x894b, 2)?;
                    at = 0x8953;
                }
            }
            0x8953 => {
                let si = c.reg(SI);
                let al = c.byte(si);
                if al & 0x80 == 0 {
                    c.run(0x8953, 5)?;
                    c.lods(B, DS);
                    c.stos(B);
                } else {
                    c.run(0x8953, 3)?;
                    c.lods(B, DS);
                    at = 0x895b;
                }
            }
            0x895b => {
                let al = c.reg8(r8::AL);
                let tests: [u8; 4] = [0xf0, 0xd0, 0xa0, 0x90];
                let k = tests.iter().position(|&t| al >= t).unwrap_or(4);
                let n = 1 + 2 * (k as u32 + 1).min(4) + (k == 3) as u32;
                c.run(0x895b, n)?;
                c.set_byte(0x477f, al);
                for &_ in &tests[..(k + 1).min(4)] {
                }
                at = match k {
                    0 => 0x89b0,
                    1 => 0x899b,
                    2 => 0x89ad,
                    3 => 0x89e4,
                    _ => 0x8970,
                };
            }
            0x8970 => {
                let al = c.reg8(r8::AL);
                if al == 0x80 {
                    c.run(0x8970, 5)?;
                    let w = c.lods(W, DS);
                    c.set_reg(AX, w.rotate_left(8));
                } else {
                    c.run(0x8970, 6)?;
                    let ax = c.reg(AX);
                    let ax = pure::and(W, ax, 0x0f);
                    let bx = pure::shl1(W, ax, 1);
                    c.set_reg(BX, bx);
                    let v = c.word(bx.wrapping_add(0x11eb));
                    c.set_reg(AX, v);
                }
                c.run(0x8984, 5)?;
                let bp = c.reg(BP);
                let si = c.reg(SI);
                c.set_word_in(SS, bp, si);
                let ds = c.sreg(DS);
                c.set_word_in(SS, bp.wrapping_add(2), ds);
                let bp = pure::add(W, bp, 4);
                c.set_reg(BP, bp);
                let ax = c.reg(AX);
                c.set_reg(SI, ax);
                c.call(0x8a3b, 0x8992)?;
                c.run(0x8992, 2)?;
                let es = c.sreg(ES);
                c.push(es);
                c.call(0xcf70, 0x8996)?;
                c.run(0x8996, 4)?;
                let es = c.sreg(ES);
                c.push(es);
                let v = c.pop();
                c.set_sreg(DS, v);
                let v = c.pop();
                c.set_sreg(ES, v);
                at = 0x8953;
            }
            0x899b => {
                let al = c.reg8(r8::AL);
                if al >= 0xd2 {
                    c.run(0x899b, 5)?;
                } else {
                    c.run(0x899b, 4 + if al == 0xd0 { 4 } else { 5 })?;
                }
                c.stos(B);
                c.movs(B);
                if al < 0xd2 {
                    c.movs(B);
                    if al != 0xd0 {
                        c.movs(W);
                    }
                }
                at = 0x8953;
            }
            0x89ad => {
                c.run(0x89ad, 2)?;
                c.stos(B);
                at = 0x8953;
            }
            0x89b0 => {
                let sp = c.reg(SP);
                let bp = c.reg(BP);
                if bp != sp {
                    c.run(0x89b0, 7)?;
                    c.set_reg(BX, sp);
                    let bp = pure::sub(W, bp, 4);
                    c.set_reg(BP, bp);
                    let si = c.word_in(SS, bp);
                    c.set_reg(SI, si);
                    let ds = c.word_in(SS, bp.wrapping_add(2));
                    c.set_sreg(DS, ds);
                    at = 0x8953;
                    continue;
                }
                let al = c.reg8(r8::AL);
                let f = c.byte(0x47de);
                let wait = f & 0x10 != 0;
                c.run(0x89b0, 3 + 3 + (al == 0xff) as u32 + 5 + if wait { 2 } else { 1 })?;
                c.set_reg(BX, sp);
                c.stos(B);
                let mut si = c.reg(SI);
                if al == 0xff {
                    si = 0;
                    c.set_reg(SI, 0);
                }
                c.set_word(0x47b6, si);
                let ds = c.sreg(DS);
                c.set_word(0x47b8, ds);
                let sp = pure::add(W, sp, 0x32);
                c.set_reg(SP, sp);
                if !wait {
                    return c.ret();
                }
                c.set_reg(BX, 3);
                c.call(0xe3b7, 0x89e0)?;
                c.run(0x89e0, 1)?;
                c.call(0x8ac3, 0x89e3)?;
                c.run(0x89e3, 1)?;
                return c.ret();
            }
            0x89e4 => {
                let al = c.reg8(r8::AL);
                let si = c.reg(SI);
                let p = c.byte(si) as u16;
                c.run(0x89e4, 8 + (al != 0x92) as u32 + 2)?;
                let bp = c.reg(BP);
                c.push(bp);
                c.set_reg8(r8::BL, al);
                c.lods(B, DS);
                c.set_reg(AX, p);
                c.set_reg(BP, p);
                let mut v = c.word_in(SS, p);
                c.set_reg(AX, v);
                if al != 0x92 {
                    v &= 0xff;
                    c.set_reg(AX, v);
                }
                c.push(v);
                c.call(0x8acc, 0x89fb)?;
                c.run(0x89fb, 2)?;
                let v = c.pop();
                c.set_reg(AX, v);
                c.call(0x8a23, 0x89ff)?;
                c.run(0x89ff, 3)?;
                let (ax, bx) = (c.reg(AX), c.reg(BX));
                c.set_reg(AX, bx);
                c.set_reg(BX, ax);
                c.set_reg(CX, 5);
                // Leading zeros: AL, DH, DL, BH, BL shift down.
                let shift = |c: &mut Ctx| {
                    let dx = c.reg(DX);
                    let bx = c.reg(BX);
                    c.set_reg8(r8::AL, (dx >> 8) as u8);
                    c.set_reg(DX, (dx << 8) | (bx >> 8));
                    c.set_reg(BX, (bx << 8) | (bx & 0xff));
                };
                loop {
                    c.run(0x8a0d, 2)?;
                    let al = c.reg8(r8::AL);
                    let cx = c.reg(CX).wrapping_sub(1);
                    c.set_reg(CX, cx);
                    if !(cx != 0 && al == 0) {
                        break;
                    }
                    c.run(0x8a05, 4)?;
                    shift(&mut c);
                }
                c.run(0x8a11, 1)?;
                let cx = c.reg(CX);
                let cx = pure::inc(W, cx);
                c.set_reg(CX, cx);
                loop {
                    c.run(0x8a12, 7)?;
                    let al = c.reg8(r8::AL);
                    let d = pure::add(B, al as u16, 0x30);
                    c.set_reg8(r8::AL, d as u8);
                    c.stos(B);
                    shift(&mut c);
                    let cx = c.reg(CX).wrapping_sub(1);
                    c.set_reg(CX, cx);
                    if cx == 0 {
                        break;
                    }
                }
                c.run(0x8a1f, 2)?;
                let bp = c.pop();
                c.set_reg(BP, bp);
                at = 0x8953;
            }
            _ => unreachable!(),
        }
    }
}

/// `loc_088d2`: the phrase SI formatted into the text buffer A6B0h
/// (`format_interpolated_string`); in the subtitle modes (from 2 of
/// [28E7h]) outside a dialogue scene ([46D9h] 0) the speech record
/// 1BE2h reset instead ([479Eh] 1, [1C06h] 80h unless [0FBh] is
/// negative); else a phrase not starting with a code shown
/// (`loc_08b11`). Bails with DF set.
pub fn format_and_show(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x88d2)?;
    let mut c = Ctx::new(m);
    if c.flag(pc::cpu::DF) {
        return Err(Bail);
    }
    c.run(start, 3)?;
    c.set_reg(DI, 0xa6b0);
    c.push(0xa6b0);
    c.call(0x8944, 0x88d9)?;
    let mode = c.byte(0x28e7);
    let scene = c.byte(0x46d9);
    if mode >= 2 && scene == 0 {
        if c.flag(pc::cpu::DF) {
            c.m.cpu.ip = 0x88d9;
            return Err(Bail);
        }
        c.run(0x88d9, 21)?;
        let s = c.pop();
        c.set_reg(SI, s);
        c.cmp(B, mode as u16, 2);
        c.cmp(B, 0, 0);
        c.set_word(BOX_SHOWN, 1);
        let f = c.byte(0xfb);
        c.set_reg8(r8::AL, !f);
        let a = c.and(B, (!f) as u16, 0x80);
        c.set_byte(0x1c06, a as u8);
        let ds = c.sreg(DS);
        c.push(ds);
        let v = c.pop();
        c.set_sreg(ES, v);
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
    let b = c.byte(si);
    let pre = 3 + if mode >= 2 { 2 } else { 0 };
    if b & 0x80 != 0 {
        c.run(0x88d9, pre + 4)?;
    } else {
        c.run(0x88d9, pre + 5)?;
    }
    let s = c.pop();
    c.set_reg(SI, s);
    c.cmp(B, mode as u16, 2);
    if mode >= 2 {
        c.cmp(B, scene as u16, 0);
    }
    c.lods(B, DS);
    c.or(B, b as u16, b as u16);
    if b & 0x80 != 0 {
        return c.ret();
    }
    let si = c.reg(SI);
    let si = c.dec(W, si);
    c.set_reg(SI, si);
    c.call(0x8b11, 0x88ea)?;
    let si = c.reg(SI);
    let v = c.byte(si);
    c.run(0x88ea, if v >= 0xfe { 3 } else { 4 })?;
    c.cmp(B, v as u16, 0xfe);
    c.ret()
}

/// `loc_088f1`: the phrase ES:SI expanded into SS:A840h (DS and ES swapped
/// for it): bytes below E0h and FEh copied; E0h-FDh with their word take
/// a dictionary entry (offset: the word's low 14 bits from SS:[47B4h];
/// length: its low nibble plus the word's top two bits · 10h; `rep
/// movsb`) and a space after it unless the phrase ends there; FFh (or the
/// buffer's end at A9CFh) ends it with FFh. Then SI the buffer and DS SS.
/// Bails with DF set.
pub fn expand_phrase(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::ES;
    entered(start, 0x88f1)?;
    let mut c = Ctx::new(m);
    if c.flag(pc::cpu::DF) {
        return Err(Bail);
    }
    c.run(0x88f1, 5)?;
    let (ds, es) = (c.sreg(DS), c.sreg(ES));
    c.push(ds);
    c.push(es);
    let v = c.pop();
    c.set_sreg(DS, v);
    let v = c.pop();
    c.set_sreg(ES, v);
    c.set_reg(DI, 0xa840);
    loop {
        let b = c.byte(c.reg(SI));
        if b == 0xff {
            c.run(0x88f8, 8)?;
            c.lods(B, DS);
            return end_893d(&mut c);
        }
        if b == 0xfe || b < 0xe0 {
            let full = c.reg(DI).wrapping_add(1) >= 0xa9cf;
            c.run(0x88f8, 9 + if b == 0xfe { 0 } else { 2 } + if full { 5 } else { 1 })?;
            c.lods(B, DS);
            c.stos(B);
            c.set_reg8(r8::AL, 0xff);
            if full {
                return end_893d(&mut c);
            }
            continue;
        }
        // A dictionary word.
        c.run(0x88f8, 21)?;
        c.lods(B, DS);
        let w = c.lods(W, DS);
        let offset = w & 0x3fff;
        c.set_reg(AX, offset);
        let length = ((w >> 8) >> 2) & 0x30 | (b & 0x0f) as u16;
        c.set_reg(CX, length);
        c.save(&[SI]);
        let at = c.word_in(SS, 0x47b4).wrapping_add(offset);
        c.set_reg(SI, at);
        c.rep(0xa4);
        let after = c.word_in(SS, c.reg(SP));
        let next = c.byte(after);
        c.run(0x892f, if next == 0xff { 9 } else { 6 })?;
        c.restore(&[SI]);
        if next == 0xff {
            c.lods(B, DS);
            return end_893d(&mut c);
        }
        let di = c.reg(DI);
        c.m.cpu.write8(&mut c.m.hw, ES, di, 0x20);
        c.set_reg(DI, di.wrapping_add(1));
    }
}

/// `loc_0893d` (counted by the caller): FFh stored, SI the buffer
/// A840h, DS = SS.
fn end_893d(c: &mut Ctx) -> Result<(), Bail> {
    c.stos(B);
    c.set_reg(SI, 0xa840);
    let ss = c.sreg(SS);
    c.push(ss);
    let v = c.pop();
    c.set_sreg(DS, v);
    c.ret()
}

/// `copy_non_pcm_lip_sync_data_and_draw_talking_head`: the head's sprite
/// list [`HEAD_SPRITES`] kept at [`HEAD_SPRITES_KEPT`], and each sprite
/// drawn at (x + [1BF0h], y + [1BF2h]) from the sprite sheet [0DBB0h]
/// in [0DBB2h] into the back buffer [0DBDAh] (the driver's far [38CDh],
/// clipped to [`RECT_D834`]). Bails with DF set or no sprites.
pub fn draw_talking_head(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::ES;
    entered(start, 0x9d16)?;
    let mut c = Ctx::new(m);
    let count = c.word(HEAD_SPRITES);
    if c.flag(pc::cpu::DF) || count == 0 {
        return Err(Bail);
    }
    c.run(start, 11)?;
    // The list kept (`rep movsb` of its count word and 3 bytes a sprite).
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(DI, HEAD_SPRITES_KEPT);
    c.set_reg(SI, HEAD_SPRITES);
    c.push(HEAD_SPRITES);
    c.set_reg(AX, count);
    c.set_reg(CX, count.wrapping_mul(3).wrapping_add(2));
    c.rep(0xa4);
    c.run(0x9d2c, 3)?;
    let list = c.pop();
    c.set_reg(SI, list);
    let count = c.lods(W, DS);
    c.set_reg(CX, count);
    loop {
        c.run(0x9d30, 22)?;
        c.save(&[CX]);
        let sprite = c.lods(B, DS);
        c.set_reg(AX, sprite);
        let x = c.lods(B, DS).wrapping_add(c.word(0x1bf0));
        c.set_reg(DX, x);
        let y = c.lods(B, DS).wrapping_add(c.word(0x1bf2));
        c.set_reg(BX, y);
        c.save(&[SI]);
        let back = c.word(0xdbda);
        c.set_sreg(ES, back);
        let (table, sheet) = (c.word(0xdbb0), c.word(0xdbb2));
        c.set_sreg(DS, sheet);
        let header = table.wrapping_add(c.word(sprite.wrapping_sub(1).wrapping_shl(1).wrapping_add(table)));
        c.set_reg(SI, header);
        let at = c.lods(W, DS);
        c.set_reg(DI, at);
        let size = c.lods(W, DS) & 0xff;
        c.set_reg(AX, size);
        c.set_reg(CX, size);
        c.set_reg(BP, RECT_D834);
        // The driver's blit (far [38CDh]).
        c.interp(0x9d5e, 0x9d63)?;
        let left = c.word_in(SS, c.reg(SP).wrapping_add(2)).wrapping_sub(1);
        c.run(0x9d63, if left == 0 { 6 } else { 5 })?;
        let ss = c.sreg(SS);
        c.push(ss);
        let ds = c.pop();
        c.set_sreg(DS, ds);
        c.restore(&[SI]);
        c.pop();
        c.set_reg(CX, left);
        if left == 0 {
            return c.ret();
        }
    }
}

/// The talking head's sprites (a count, then sprite, x, y bytes).
const HEAD_SPRITES: u16 = 0x4608;
/// Where they are kept once drawn ([`rects_missing`] compares the two).
const HEAD_SPRITES_KEPT: u16 = 0x4540;

/// `draw_speech_bubble` (8F28): the text box for the rectangle at SS:BP
/// (x, y, width, height; [`BOX_SHOWN`] BP): kept at 1BE2h (x, y, right
/// edge up to 140h, bottom), its text area from the margins
/// [4784h..478Ah] ([4791h..4797h] the corner, [478Fh] / [478Dh] the
/// size; DX, BX the far corner). Then drawn by kind: with a talking head
/// shown ([`MAP_MODE`]) the dialogue frame ([`frame_8fd1`]); for the book
/// ([0C6h]) its page ([`book_8ff5`]); nothing in the intro ([227Dh]) or
/// with [46D9h]; with [`MAP_SCREEN`] a parchment (from 4C60h, far [3919h]
/// into [0DBDEh]) tiled with sprite 1Ch (`loc_0c137`, `loc_0c370`; SI
/// kept); else the text buffer cleared ([`clear_900b`]).
pub fn draw_speech_bubble(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{BP, ES, SS};
    entered(start, 0x8f28)?;
    let mut c = Ctx::new(m);
    let rect = c.reg(BP);
    let [x, y, width, height] = [0u16, 2, 4, 6].map(|k| c.word_in(SS, rect.wrapping_add(k)));
    let (right, bottom) = (x.wrapping_add(width), y.wrapping_add(height));
    let wide = right >= 0x140;
    c.run(start, if wide { 33 } else { 32 })?;
    c.set_word(BOX_SHOWN, rect);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    let [left_margin, right_margin, top_margin, bottom_margin] = [0x4784u16, 0x4786, 0x4788, 0x478a].map(|a| c.word(a));
    let text_x = x.wrapping_add(left_margin);
    let text_y = y.wrapping_add(top_margin);
    for (a, v) in [(0x4791u16, text_x), (0x4795, text_x), (0x4793, text_y), (0x4797, text_y)] {
        c.set_word(a, v);
    }
    c.set_word(0x478f, width.wrapping_sub(left_margin).wrapping_sub(right_margin));
    c.set_word(0x478d, height.wrapping_sub(top_margin).wrapping_sub(bottom_margin));
    let edge = if wide { 0x140 } else { right };
    c.set_reg(DI, 0x1be2);
    for v in [x, y, edge, bottom] {
        c.set_reg(AX, v);
        c.stos(W);
    }
    c.set_reg(DX, right);
    c.set_reg(BX, bottom);
    let [head, book, intro, busy, map_screen] = [MAP_MODE, 0xc6, 0x227d, 0x46d9, MAP_SCREEN].map(|a| c.byte(a));
    if head != 0 {
        c.run(0x8f7f, 2)?;
        return frame_8fd1(&mut c);
    }
    if book != 0 {
        c.run(0x8f7f, 4)?;
        return book_8ff5(&mut c);
    }
    if intro != 0 || busy != 0 {
        c.run(0x8f7f, if intro != 0 { 7 } else { 9 })?;
        return c.ret();
    }
    if map_screen == 0 {
        c.run(0x8f7f, 10)?;
        return clear_900b(&mut c);
    }
    c.run(0x8f7f, 18)?;
    c.save(&[SI]);
    c.set_reg(SI, 0x4c60);
    c.set_reg(BP, 0x1be2);
    c.set_reg(AX, 0x80);
    c.set_word_in(SS, 0x1bea, 0x80);
    c.set_word(0x1c06, 0x80);
    c.set_word_in(SS, 0x1bee, 0x9468);
    let parchment = c.word(0xdbde);
    c.set_sreg(ES, parchment);
    c.interp(0x8fbb, 0x8fbf)?;
    c.run(0x8fbf, 1)?;
    c.call(0xc137, 0x8fc2)?;
    c.run(0x8fc2, 4)?;
    c.set_reg(SI, 0x1be2);
    let screen = c.word(0xdbd6);
    c.set_sreg(ES, screen);
    c.set_reg(AX, 0x1c);
    c.call(0xc370, 0x8fcf)?;
    c.run(0x8fcf, 2)?;
    c.restore(&[SI]);
    c.ret()
}

/// 8FD1h: the dialogue frame around the box (far [38DDh] into
/// [0DBDAh], colour [18F2h]), drawn one line higher.
fn frame_8fd1(c: &mut Ctx) -> Result<(), Bail> {
    use pc::cpu::ES;
    c.run(0x8fd1, 7)?;
    let si = c.reg(SI);
    c.push(si);
    c.set_reg(SI, 0x1be2);
    c.set_word(0x1bea, 0x80);
    c.set_word(0x1bee, 0x7bed);
    let v = c.word(0x1be4);
    let v = c.sub(W, v, 2);
    c.set_word(0x1be4, v);
    let al = c.byte(0x18f2);
    c.set_reg8(r8::AL, al);
    let es = c.word(0xdbda);
    c.set_sreg(ES, es);
    c.interp(0x8fea, 0x8fee)?;
    c.run(0x8fee, 3)?;
    let v = c.word(0x1be4);
    let v = c.add(W, v, 2);
    c.set_word(0x1be4, v);
    let v = c.pop();
    c.set_reg(SI, v);
    c.ret()
}

/// 8FF5h: the book's page: sheet 32h, sprite 3 tiled over 2265h.
fn book_8ff5(c: &mut Ctx) -> Result<(), Bail> {
    use pc::cpu::ES;
    c.run(0x8ff5, 3)?;
    let si = c.reg(SI);
    c.push(si);
    c.set_reg(AX, 0x32);
    c.call(0xc13e, 0x8ffc)?;
    c.run(0x8ffc, 4)?;
    c.set_reg(AX, 3);
    c.set_reg(SI, 0x2265);
    let es = c.word(0xdbd6);
    c.set_sreg(ES, es);
    c.call(0xc370, 0x9009)?;
    c.run(0x9009, 2)?;
    let v = c.pop();
    c.set_reg(SI, v);
    c.ret()
}

/// 900Bh: the text buffer 4C60h (5960h bytes) cleared, [22FCh] 4C60h,
/// `loc_0c085`; then the box at 1BE2h reset (8895h: [1C06h] the
/// inverse of [0FBh]'s bit 7).
fn clear_900b(c: &mut Ctx) -> Result<(), Bail> {
    use pc::cpu::ES;
    c.run(0x900b, 6)?;
    c.set_reg(DI, 0x4c60);
    c.set_reg(CX, 0x5960);
    let ds = c.sreg(DS);
    c.push(ds);
    let v = c.pop();
    c.set_sreg(ES, v);
    let al = c.reg8(r8::AL);
    c.xor(B, al as u16, al as u16);
    c.set_reg8(r8::AL, 0);
    c.rep(0xaa);
    c.run(0x9017, 4)?;
    c.set_reg(AX, 0x4c6f);
    let a = c.and(B, 0x6f, 0xf0);
    c.set_reg(AX, 0x4c00 | a);
    c.set_word(0x22fc, 0x4c00 | a);
    c.call(0xc085, 0x9022)?;
    // The `jmp`, then 8895h to its `ret`: 15 instructions.
    c.run(0x9022, 1 + 15)?;
    let f = c.byte(0xfb);
    let a = !f;
    let a = c.and(B, a as u16, 0x80);
    c.set_reg8(r8::AL, a as u8);
    c.set_byte(0x1c06, a as u8);
    let ds = c.sreg(DS);
    c.push(ds);
    let v = c.pop();
    c.set_sreg(ES, v);
    c.set_reg(DI, 0x1be2);
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    c.set_reg(AX, 0);
    for _ in 0..4 {
        c.stos(W);
    }
    c.set_reg8(r8::AL, 0x80);
    c.stos(W);
    c.ret()
}

/// `loc_0a172`: by [`TALKER`] (AX): 1 counts at [0F5h]; 3 goes on into
/// `loc_024a3`; others return.
pub fn dialogue_state_a172(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa172)?;
    let mut c = Ctx::new(m);
    let talker = c.word(TALKER);
    c.run(start, if talker == 1 { 7 } else { 6 })?;
    c.set_reg(AX, talker);
    if talker == 1 {
        let n = c.byte(0xf5).wrapping_add(1);
        c.set_byte(0xf5, n);
    }
    if talker == 3 {
        return c.jump(0x24a3);
    }
    c.ret()
}

/// `loc_0a28e`: a character's portrait brought up (SI kept). For
/// [`TALKER`] 0Eh and up [0DC30h] = `loc_0456c` for the person [47E6h].
/// Unless the map screen is in mode 1 or the person is where the player
/// is: [47A4h] 0, resource 21h opened, `loc_0c21b` with 22E4h, the
/// picture's 2A80h bytes moved up to end at A5BFh ([22FCh] their new
/// start), the person's mark (`loc_05b55`), then with [`MAP_MODE`] 1
/// meanwhile `loc_05b99` (22F4h, 46E3h), `loc_05b93`, `loc_0b6c3`,
/// `loc_0c137`, `loc_05dce`, `loc_062fe` (SI the person); last
/// `loc_0c477` with 22ECh.
pub fn portrait_a28e(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa28e)?;
    let mut c = Ctx::new(m);
    let late = c.word(TALKER) >= 0x0e;
    c.run(start, if late { 5 } else { 3 })?;
    c.save(&[SI]);
    if late {
        let person = c.word(0x47e6);
        c.set_reg(DI, person);
        c.call(0x456c, 0xa29d)?;
        c.run(0xa29d, 1)?;
        let v = c.reg(AX);
        c.set_word(0xdc30, v);
    }
    let book = c.byte(MAP_SCREEN) == 1;
    let person = c.word(0x47e6);
    if book || person == game::here(&mut c) {
        c.run(0xa2a0, if book { 4 } else { 7 })?;
        if !book {
            c.set_reg(AX, person);
        }
        c.restore(&[SI]);
        return c.ret();
    }
    c.run(0xa2a0, 8)?;
    c.set_byte(0x47a4, 0);
    c.set_reg(AX, 0x21);
    c.call(0xc13e, 0xa2bb)?;
    c.run(0xa2bb, 2)?;
    c.set_reg(SI, 0x22e4);
    c.call(0xc21b, 0xa2c1)?;
    c.run(0xa2c1, 8)?;
    // `std; rep movsb`: the picture moved up, from its end down.
    let picture = c.word(0x22fc);
    c.set_reg(SI, picture.wrapping_add(0x2a80));
    c.set_reg(CX, 0x2a80);
    c.set_reg(DI, 0xa5bf);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_flag(pc::cpu::DF, true);
    c.rep(0xa4);
    c.run(0xa2d2, 6)?;
    c.set_flag(pc::cpu::DF, false);
    let moved = c.reg(DI).wrapping_add(1);
    c.set_reg(DI, moved);
    c.set_word(0x22fc, moved);
    let person = c.word(0x47e6);
    c.set_reg(DI, person);
    c.push(person);
    c.call(0x5b55, 0xa2e0)?;
    c.run(0xa2e0, 4)?;
    c.set_byte(MAP_MODE, 1);
    c.set_reg(SI, 0x22f4);
    c.set_reg(DI, 0x46e3);
    c.call(0x5b99, 0xa2ee)?;
    for (to, back) in [(0x5b93u16, 0xa2f1u16), (0xb6c3, 0xa2f4), (0xc137, 0xa2f7), (0x5dce, 0xa2fa)] {
        c.run(back - 3, 1)?;
        c.call(to, back)?;
    }
    c.run(0xa2fa, 2)?;
    let person = c.pop();
    c.set_reg(SI, person);
    c.call(0x62fe, 0xa2fe)?;
    c.run(0xa2fe, 3)?;
    c.set_byte(MAP_MODE, 0);
    c.set_reg(SI, 0x22ec);
    c.call(0xc477, 0xa309)?;
    c.run(0xa309, 2)?;
    c.restore(&[SI]);
    c.ret()
}

/// `loc_0a125`: by [`TALKER`] (AX): 1, the charisma step below; 3, on
/// into `loc_02239`; 5, [227Eh] 2CCFh; 0Ch, bit 80h of byte 0Ah of the
/// location [11CEh] cleared (DI it); 0Dh, on into `loc_02388`. The
/// charisma step: with bit 2 of [0Ah], charisma 28h more (6F78h, AL)
/// and AX -32h; else with [`REACH`] 1, charisma 0Ah more and AX 0Ah;
/// else AX [`REACH`]. Then [`charisma_tail`].
pub fn dialogue_state_a125(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa125)?;
    let mut c = Ctx::new(m);
    let talker = c.word(TALKER);
    match talker {
        1 => {
            c.run(start, 3)?;
            c.set_reg(AX, talker);
            if c.byte(0x0a) & 2 != 0 {
                c.run(0xa186, 4)?;
                c.set_reg8(r8::AL, 0x28);
                c.call(0x6f78, 0xa192)?;
                c.run(0xa192, 2)?;
                return charisma_tail(&mut c, 0xffce);
            }
            let reach = c.word(REACH);
            if reach == 1 {
                c.run(0xa186, 8)?;
                c.set_reg(AX, reach + 0x0a);
                c.set_reg8(r8::AL, 0x0a);
                c.call(0x6f78, 0xa1a7)?;
                c.run(0xa1a7, 1)?;
                return charisma_tail(&mut c, 0x0a);
            }
            c.run(0xa186, 5)?;
            c.set_reg(AX, reach);
            charisma_tail(&mut c, reach)
        }
        3 => {
            c.run(start, 6)?;
            c.set_reg(AX, talker);
            c.jump(0x2239)
        }
        5 => {
            c.run(start, 9)?;
            c.set_reg(AX, talker);
            c.set_word(0x227e, 0x2ccf);
            c.ret()
        }
        _ => {
            c.run(start, 12 + if talker == 0x0c { 2 } else { 0 })?;
            c.set_reg(AX, talker);
            if talker == 0x0c {
                let place = c.word(0x11ce);
                c.set_reg(DI, place);
                let b = c.byte(place.wrapping_add(0x0a));
                c.set_byte(place.wrapping_add(0x0a), b & 0x7f);
            }
            if talker == 0x0d {
                return c.jump(0x2388);
            }
            c.ret()
        }
    }
}

/// How far a caption shows, raised by the charisma step ([1176h]).
const REACH: u16 = 0x1176;

/// A1AAh-A1C3h of `loc_0a125`, with AX `ax` there: [`REACH`] (AX) = it
/// + 14h, and [0D5h] (BL) 0 from 64h on, else 80h less a sixth of it (AX
/// the `div`'s remainder and quotient).
fn charisma_tail(c: &mut Ctx, ax: u16) -> Result<(), Bail> {
    let reach = ax.wrapping_add(0x14);
    c.run(0xa1aa, if reach < 0x64 { 11 } else { 7 })?;
    c.set_reg(AX, reach);
    c.set_word(REACH, reach);
    let mut d5 = 0u8;
    if reach < 0x64 {
        let (q, r) = (reach / 6, reach % 6);
        c.set_reg(AX, r << 8 | q);
        d5 = 0x80u8.wrapping_sub(q as u8);
    }
    c.set_reg8(r8::BL, d5);
    c.set_byte(0xd5, d5);
    c.ret()
}

/// `loc_0a244`: [`dialogue_scene_a25b`]'s set-up with AL 0.
pub fn dialogue_scene_a244(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa244)?;
    let mut c = Ctx::new(m);
    c.run(0xa244, 2 + 5)?;
    c.set_reg8(r8::AL, 0);
    scene_setup_a24a(&mut c)
}

/// `loc_0a248`: [`dialogue_scene_a25b`]'s set-up with AL 1.
pub fn dialogue_scene_a248(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa248)?;
    let mut c = Ctx::new(m);
    c.run(0xa248, 1 + 5)?;
    c.set_reg8(r8::AL, 1);
    scene_setup_a24a(&mut c)
}

/// A24Ah-A258h (counted by the caller): [476Dh] AL, [9Fh] 0,
/// `loc_0d323` with BP 1FFEh and BX 0F66h, then on into
/// [`dialogue_scene_a25b`] (its return address).
fn scene_setup_a24a(c: &mut Ctx) -> Result<(), Bail> {
    let al = c.reg8(r8::AL);
    c.set_byte(0x476d, al);
    c.set_byte(0x9f, 0);
    c.set_reg(BP, 0x1ffe);
    c.set_reg(BX, 0x0f66);
    c.call(0xd323, 0xa25b)?;
    scene_at_a25b(c)
}

/// `loc_0a25b`: the entry of the table 197Ch (8 bytes each, its word at
/// +8) for the scene [4780h], searched while the scene is above it; when
/// found, [47E1h] = 0A01h (BH 10h for an entry whose next word is 38h)
/// with BH up by 4 x [47D0h] - 2 for [47D0h] from 1, and [47E4h] the
/// entry's next word's place. SI kept, ES = DS.
pub fn dialogue_scene_a25b(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa25b)?;
    scene_at_a25b(&mut Ctx::new(m))
}

fn scene_at_a25b(c: &mut Ctx) -> Result<(), Bail> {
    let ax = c.word(0x4780);
    let step = c.string_step(W);
    let mut di = 0x197cu16;
    let mut k = 0u32;
    let w = loop {
        di = di.wrapping_add(8);
        let w = c.word(di);
        di = di.wrapping_add(step);
        k += 1;
        if ax <= w {
            break w;
        }
        if k > 10_000 {
            c.m.cpu.ip = 0xa25b;
            return Err(Bail);
        }
    };
    let found = ax == w;
    let next = c.word(di);
    let al0 = c.byte(SENTENCE_KIND);
    // `dec al; js` jumps for AL 0 and 81h-FFh.
    let n = 6 + 3 * k + 1
        + if found { 2 + (next == 0x38) as u32 + 3 + if al0 == 0 || al0 > 0x80 { 0 } else { 4 } + 2 } else { 0 }
        + 2;
    c.run(0xa25b, n)?;
    let si = c.reg(SI);
    c.push(si);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(AX, ax);
    c.set_reg(BX, 0x0a01);
    let mut d = 0x197cu16;
    for _ in 0..k {
        d = c.add(W, d, 8);
        let w = c.word(d);
        c.cmp(W, ax, w);
        d = d.wrapping_add(step);
    }
    c.set_reg(DI, d);
    if found {
        c.cmp(W, next, 0x38);
        let mut bx = 0x0a01u16;
        if next == 0x38 {
            bx = 0x1001;
        }
        c.set_reg8(r8::AL, al0);
        let al = c.dec(B, al0 as u16);
        c.set_reg8(r8::AL, al as u8);
        if al & 0x80 == 0 {
            let al = c.shl1(B, al, 1);
            c.set_reg8(r8::AL, al as u8);
            let ax = c.reg(AX);
            let ax = c.inc(W, ax);
            c.set_reg(AX, ax);
            let al = c.shl1(B, ax & 0xff, 1);
            c.set_reg8(r8::AL, al as u8);
            let bh = c.add(B, bx >> 8, al);
            bx = (bh << 8) | (bx & 0xff);
        }
        c.set_reg(BX, bx);
        c.set_word(0x47e1, bx);
        c.set_word(0x47e4, d);
    }
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}
