//! The condition scripts and the phase changes natively (the native mode,
//! Stage C of `docs/readable-port-plan.md`): `evaluate_condition_ax`
//! (A396h) on the scripts of CONDIT.HSQ, the dialogue step the phase
//! change runs for the pseudo-speaker 10h in the book's mode (`loc_096b5`,
//! `loc_09f9e` without the display), the entries' actions it meets, and
//! `set_game_phase` (121Fh) with its phases' handlers (CS:11E7h). `None`:
//! a case not modelled yet (an action, a phase's handler), named.

use super::globals as g;
use super::native_day::add_pending_event;
use super::native_step::add_message;
use super::world::{nm, World};
use super::{economy::Economy, progress::Progress, situation::Situation};

/// The conversation record (DS:47A2h) fields the step uses.
mod conv {
    pub const FLAG_47A5: u16 = 0x47a5;
    pub const LAST_ENTRY: u16 = 0x47a8;
    pub const PHRASE_RESUME: u16 = 0x47b6;
    pub const APPEND_AT: u16 = 0x47bc;
    pub const MODE: u16 = 0x47c2;
    pub const TALKER: u16 = 0x47c4;
}

/// The dialogue entries' start (the dialogue table's lists, DS:AA78h).
const DIALOGUES: u16 = 0xaa78;
/// The text buffer the step points the conversation at.
const TEXT_BUFFER: u16 = 0xa6b0;
/// The book's next free word ([11BDh], in the code segment).
const NOTED: u16 = 0x11bd;
/// The story's book flag ([0C6h]).
const BOOK: u16 = 0xc6;

/// The operators by their byte offset in the table at CS:A376h (16
/// words: the first ten operators, then six `false`), as the code word
/// there reads.
fn operator(idx: u16, dx: u16, ax: u16) -> Option<u16> {
    const TABLE: [u16; 16] = [0xa348, 0xa34f, 0xa356, 0xa35d, 0xa364, 0xa36b, 0xa33c, 0xa33f, 0xa342, 0xa345, 0xa36f, 0xa36f, 0xa36f, 0xa36f, 0xa36f, 0xa36f];
    if idx & 1 != 0 {
        return nm("a condition's operator at an odd offset");
    }
    let t = *TABLE.get(idx as usize / 2)?;
    let b = |t: bool| if t { 0xffff } else { 0 };
    Some(match t {
        0xa348 => b(dx == ax),
        0xa34f => b(dx < ax),
        0xa356 => b(dx > ax),
        0xa35d => b(dx != ax),
        0xa364 => b((dx as i16) <= (ax as i16)),
        0xa36b => b((dx as i16) >= (ax as i16)),
        0xa33c => dx.wrapping_add(ax),
        0xa33f => dx.wrapping_sub(ax),
        0xa342 => dx & ax,
        0xa345 => dx | ax,
        _ => 0,
    })
}

/// `read_condit_operand_into_ax` (A30Bh): an operand at `si`: below 80h a
/// story variable (its next byte; kind 1 a byte, else a word), 80h a
/// byte, else a word.
fn operand(w: &World, si: &mut u16) -> Option<u16> {
    let kind = w.condit(*si)?;
    *si = si.wrapping_add(1);
    if kind < 0x80 {
        let var = w.condit(*si)? as u16;
        *si = si.wrapping_add(1);
        if kind == 1 {
            Some(w.byte(var) as u16)
        } else {
            Some(w.byte(var) as u16 | (w.byte(var.wrapping_add(1)) as u16) << 8)
        }
    } else if kind == 0x80 {
        let v = w.condit(*si)? as u16;
        *si = si.wrapping_add(1);
        Some(v)
    } else {
        let v = w.condit(*si)? as u16 | (w.condit(si.wrapping_add(1))? as u16) << 8;
        *si = si.wrapping_add(2);
        Some(v)
    }
}

/// `evaluate_condition_ax` (A396h): whether condition `n` holds: its
/// script (the word before entry `n` of the table at [AA72h]) read as
/// terms of operands and operators, a term closed by an operator with
/// bit 80h (kept to combine), FFh ending; the terms then combined left to
/// right; it holds when not 0.
pub fn condition(w: &World, n: u16) -> Option<bool> {
    let table = w.conditions.base;
    let at = table.wrapping_add(n << 1).wrapping_sub(2);
    let mut si = w.condit(at)? as u16 | (w.condit(at.wrapping_add(1))? as u16) << 8;
    let mut terms: Vec<(u16, u16)> = Vec::new();
    let mut dx = operand(w, &mut si)?;
    loop {
        if terms.len() > 12 {
            return nm("a condition of too many terms");
        }
        let op = w.condit(si)?;
        si = si.wrapping_add(1);
        if op == 0xff {
            break;
        }
        if op & 0x80 != 0 {
            terms.push((dx, op as u16));
            dx = operand(w, &mut si)?;
            continue;
        }
        let ax = operand(w, &mut si)?;
        dx = operator(op as u16 & 0x1f, dx, ax)?;
    }
    if !terms.is_empty() {
        let last = dx;
        dx = terms[0].0;
        for i in 0..terms.len() {
            let next = terms.get(i + 1).map_or(last, |t| t.0);
            dx = operator(terms[i].1 & 0x1f, dx, next)?;
        }
    }
    Some(dx != 0)
}

/// `loc_096b5` (from `loc_0b17a`, the book's flag 80h meanwhile): the
/// phase change's dialogue step for the pseudo-speaker 10h, mode 80h
/// (both given back after).
fn phase_change_step(w: &mut World) -> Option<()> {
    let (talker, mode) = (w.word(conv::TALKER), w.word(conv::MODE));
    w.set_word(conv::TALKER, 0x10);
    w.set_byte(conv::MODE, 0x80);
    let list = w.word(g::PHASE_CHANGE_PHRASE);
    say_entry(w, list)?;
    w.set_word(conv::MODE, mode);
    w.set_word(conv::TALKER, talker);
    Some(())
}

/// `loc_09f9e` for a speaker of 10h or more, in the book's mode (nothing
/// shown): from the list `list`, the first entry not said once (flags
/// 80h without 40h, under the mode's mask) whose condition holds; none,
/// nothing. Its action (`loc_0a03f`, the table at CS:A107h), then
/// marked said and noted in the book.
fn say_entry(w: &mut World, list: u16) -> Option<()> {
    w.set_word(conv::APPEND_AT, TEXT_BUFFER);
    let mut e = list;
    for _ in 0..4000 {
        let head = w.word(e);
        if head == 0xffff {
            return Some(());
        }
        let [flags, cond] = head.to_le_bytes();
        let said_once = flags & 0x80 != 0 && flags & 0x40 == 0;
        if said_once && flags & w.byte(conv::MODE) != 0 {
            e = e.wrapping_add(4);
            continue;
        }
        let high = w.byte(e.wrapping_add(2)).rotate_left(2) & 3;
        if condition(w, (high as u16) << 8 | cond as u16)? {
            return entry_said(w, e);
        }
        e = e.wrapping_add(4);
    }
    nm("a dialogue list without its end")
}

/// `loc_0a03f` for the entry `e` (no phrase resumed): its action, then,
/// for an entry of kind bits 0Ch not said before, its number and speaker
/// noted in the book; marked said; the speaker's line said.
fn entry_said(w: &mut World, e: u16) -> Option<()> {
    if w.word(conv::PHRASE_RESUME) != 0 {
        return Some(());
    }
    let action = w.byte(e) & 0x0f;
    if action != 0 {
        entry_action(w, action, e)?;
    }
    let kind = w.byte(e.wrapping_add(2)) & 0x0c;
    let flags = w.byte(e);
    if kind != 0 && flags & 0x80 == 0 {
        let talker = (w.byte(conv::TALKER)) << 3;
        let number = e.wrapping_sub(DIALOGUES) >> 2 | (talker as u16) << 8;
        let noted = w.word(NOTED);
        let k = noted.wrapping_sub(super::book::AT) as usize;
        if k + 4 > w.save.book.len() {
            return nm("the book full");
        }
        w.save.book[k..k + 2].copy_from_slice(&number.to_le_bytes());
        w.save.book[k + 2..k + 4].copy_from_slice(&[0, 0]);
        w.set_word(NOTED, noted.wrapping_add(2));
    }
    w.update::<Progress>(|t| t.speaker_said_line = 0xff);
    w.set_byte(e, flags | 0x80);
    w.set_byte(conv::LAST_ENTRY, 0);
    Some(())
}

/// The entries' actions (CS:A107h by the action - 1) the phase change
/// meets.
fn entry_action(w: &mut World, action: u8, e: u16) -> Option<()> {
    let said = w.byte(e) & 0x80 != 0;
    match action {
        // `loc_0a1d0`, `loc_0a1dc`: [47A5h] FFh, 80h.
        1 => w.set_byte(conv::FLAG_47A5, 0xff),
        7 => w.set_byte(conv::FLAG_47A5, 0x80),
        // `loc_0a1e8`: the last entry counted.
        6 => w.set_byte(conv::LAST_ENTRY, w.byte(conv::LAST_ENTRY).wrapping_add(1)),
        // `loc_0a219`: unless said, the next phase (its days 0, the phase
        // change's step again; phase 1's own start not modelled).
        0x0b => {
            if !said {
                let p = w.byte(g::GAME_PHASE).wrapping_add(1);
                w.set_byte(g::GAME_PHASE, p);
                w.set_byte(0xff, 0);
                on_phase_change(w)?;
                if w.byte(g::GAME_PHASE) == 1 {
                    return nm("phase 1's start");
                }
            }
        }
        // `loc_0a235`: unless said, the next group of four phases.
        0x0c => {
            if !said {
                let p = (w.byte(g::GAME_PHASE) & 0xfc).wrapping_add(4);
                set_game_phase(w, p)?;
            }
        }
        // `loc_0a1ed`: unless said, the endgame's next stage.
        0x0e => {
            if !said {
                w.update::<Economy>(|t| t.endgame_stage = t.endgame_stage.wrapping_add(1));
            }
        }
        _ => return nm("a dialogue entry's action"),
    }
    Some(())
}

/// `loc_0b17a`: the phase change's step with the book's flag 80h.
pub fn on_phase_change(w: &mut World) -> Option<()> {
    let book = w.byte(BOOK);
    w.set_byte(BOOK, book | 0x80);
    phase_change_step(w)?;
    w.set_byte(BOOK, book);
    Some(())
}

/// `set_game_phase` (121Fh): a later phase `new` set (its days 0), the
/// phase change's step, then the phase's handler (CS:11E7h by the phase
/// over 2, as a byte offset: only phases that are multiples of 4 meet a
/// handler's start; phases past 6Ch none).
pub fn set_game_phase(w: &mut World, new: u8) -> Option<()> {
    if new <= w.byte(g::GAME_PHASE) {
        return Some(());
    }
    w.set_byte(g::GAME_PHASE, new);
    w.update::<Situation>(|s| s.days_in_phase = 0);
    on_phase_change(w)?;
    let p = w.byte(g::GAME_PHASE);
    if p > 0x6c {
        return Some(());
    }
    phase_handler(w, p)
}

/// The phase `p`'s handler (CS:11E7h + `p` / 2: phases 2 mod 4 read a
/// word across two entries, not a handler's start: not modelled). The
/// display's parts (the panel's buttons, `loc_02ffb`) are left out.
fn phase_handler(w: &mut World, p: u8) -> Option<()> {
    if p & 2 != 0 {
        return nm("a phase's handler between two");
    }
    let fe8 = |w: &mut World, v: u8| w.set_byte(0xfe8, v);
    match p & !3 {
        // `loc_010b7`: nothing.
        0x18 | 0x24 | 0x3c | 0x68 | 0x6c => {}
        // `loc_01011`.
        0x04 => {
            w.set_byte(0x122a, w.byte(0x122a).wrapping_sub(1));
            for p in [0x218, 0x2dc] {
                discover(w, p)?;
            }
            fe8(w, 9);
        }
        // `loc_01027`.
        0x08 => w.set_byte(0x122e, w.byte(0x122e) & 0x7f),
        // `loc_0102f`: a sequence started (the clock stopped meanwhile).
        0x0c => {
            w.set_byte(0x124a, w.byte(0x124a) & 0x7f);
            w.set_byte(0x1247, w.byte(0x1247) & 0x7f);
            w.set_word(0x121d, 0xffff);
            return nm("a sequence started");
        }
        // `loc_01045`.
        0x10 => {
            w.set_byte(0xfd8, 5);
            fe8(w, 9);
            add_message(w, 0x10b);
        }
        // `loc_01053`.
        0x14 => {
            w.set_byte(0xfd8, 0x0a);
            for p in [0x34c, 0x368, 0x384] {
                discover(w, p)?;
            }
        }
        // `loc_010a4`.
        0x1c => {
            w.set_byte(0x1245, w.byte(0x1245) & 0x7f);
            w.set_word(0x1217, 0xffff);
        }
        // `loc_010b2`.
        0x20 => w.set_byte(0xffb, 1),
        // `loc_010b8`.
        0x28 => discover(w, 0x800)?,
        // `loc_010be`.
        0x2c => {
            w.set_word(0x1154, w.time());
            w.set_word(0x1018, 0x2006);
            w.set_word(0xff8, 0x2008);
            w.set_word(0xffa, 0x180);
            fe8(w, 0x0a);
            w.set_word(0xfea, 0x180);
            w.set_word(0x1201, 0x109);
            super::native_raids::raise_charisma(w, 0x14);
            w.set_byte(0x0a, w.byte(0x0a) | 0x10);
            for p in [0x5ec, 0x5d0, 0x608, 0x640, 0x65c] {
                discover(w, p)?;
            }
        }
        // `loc_01103`: event 4, message 1409h.
        0x30 => {
            add_pending_event(w, 4, 0);
            add_message(w, 0x1409);
        }
        // `loc_0110f`.
        0x34 => {
            if w.byte(0xfe8) != 8 {
                fe8(w, 0x0a);
                w.set_word(0xfea, 0x180);
            }
            add_message(w, 0x280a);
        }
        // `loc_01127`.
        0x38 => w.set_byte(0xfdb, 0xff),
        // `loc_0112d`.
        0x40 => w.set_byte(0x1067, w.byte(0x1067) | 2),
        // `loc_01133`.
        0x44 => discover(w, 0x3d8)?,
        // `loc_01139`, `loc_01196`: a sequence started.
        0x48 | 0x58 => return nm("a sequence started"),
        // `loc_01166`.
        0x4c => {
            w.set_byte(0x1141, w.byte(0x1141).wrapping_add(1));
            fe8(w, 2);
            w.set_word(0xfea, 0x180);
            add_pending_event(w, 0x105, 0);
        }
        // `loc_0117b`.
        0x50 => {
            w.set_byte(0x0a, w.byte(0x0a) | 0x40);
            super::native_raids::raise_charisma(w, 0x28);
            fe8(w, 9);
        }
        // `loc_01188`.
        0x54 => {
            w.set_byte(0x1259, w.byte(0x1259) & 0x7f);
            w.set_word(0x1211, 0xffff);
        }
        // `loc_011b3` (the listing reads it as data).
        0x5c => {
            w.set_byte(0x1038, 5);
            w.set_byte(0x11d0, w.byte(0x11d0).wrapping_add(0x0c));
            w.set_word(0x1156, (w.time() >> 4).wrapping_add(3));
            w.set_byte(0x1141, w.byte(0x1141).wrapping_add(1));
        }
        // `loc_011cb`: phase 60h's start.
        0x60 => super::native_step::go_to_phase_60(w),
        _ => return nm("a phase's handler"),
    }
    Some(())
}

/// `loc_0425b`: the place `p` discovered (flag 80h off, byte 0Bh 0); a
/// sietch (below kind 20h) one more held, and place 603h's discovery
/// raises the phase to 10h.
fn discover(w: &mut World, p: u16) -> Option<()> {
    let f = w.byte(p + 0x0a);
    if f & 0x80 == 0 {
        return Some(());
    }
    w.set_byte(p + 0x0a, f & 0x7f);
    w.set_byte(p + 0x0b, 0);
    if w.byte(p + 8) >= 0x20 {
        return Some(());
    }
    w.update::<Progress>(|t| t.sietches_held = t.sietches_held.wrapping_add(1));
    if w.word(p) == 0x0603 {
        set_game_phase(w, 0x10)?;
    }
    Some(())
}
