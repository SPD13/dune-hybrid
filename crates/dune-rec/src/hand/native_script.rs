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
fn say_entry(w: &mut World, list: u16) -> Option<bool> {
    speaker_mood(w);
    w.set_word(conv::APPEND_AT, TEXT_BUFFER);
    let mut e = list;
    for _ in 0..4000 {
        let head = w.word(e);
        if head == 0xffff {
            return Some(false);
        }
        let [flags, cond] = head.to_le_bytes();
        let said_once = flags & 0x80 != 0 && flags & 0x40 == 0;
        if said_once && flags & w.byte(conv::MODE) != 0 {
            e = e.wrapping_add(4);
            continue;
        }
        let high = w.byte(e.wrapping_add(2)).rotate_left(2) & 3;
        if condition(w, (high as u16) << 8 | cond as u16)? {
            entry_said(w, e)?;
            after_entry(w)?;
            return Some(true);
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
        // `loc_0a1d0`, `loc_0a1dc`: [47A5h] FFh, 80h; `loc_0a1d6`: 0.
        1 => w.set_byte(conv::FLAG_47A5, 0xff),
        2 => w.set_byte(conv::FLAG_47A5, 0),
        8 => reward(w)?,
        // `loc_0a172`: Jessica's remark counted; Duncan's (`loc_024a3`)
        // not modelled.
        0x0f => match w.word(conv::TALKER) {
            1 => w.update::<Situation>(|s| s.desert_remarks = s.desert_remarks.wrapping_add(1)),
            3 => return nm("Duncan's remark"),
            _ => {}
        },
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
        // `loc_0a1f7`: the sequence for the phase started (12F8h before
        // phase 14h, 134Fh before 18h, 1370h before 30h, else 12DBh).
        3 => {
            let p = w.byte(g::GAME_PHASE);
            let s = if p < 0x14 { 0x12f8 } else if p < 0x18 { 0x134f } else if p < 0x30 { 0x1370 } else { 0x12db };
            super::native_seq::start_sequence(w, s);
        }
        // `loc_0a244`, `loc_0a248`: the answers offered (the mode 0 or 1,
        // no answer yet; the screen pushed, the voice's numbers: display).
        4 | 5 => {
            w.set_byte(g::ANSWER_MODE, action - 4);
            w.update::<Economy>(|t| t.answer = 0);
        }
        // `loc_0a157`: Duncan's, Stilgar's and the smuggler's own ends of a
        // talk (`loc_024ee`, `loc_02d2c`, `loc_02419`): not modelled.
        9 => match w.word(conv::TALKER) {
            3 => return nm("Duncan's end of a talk (action 9)"),
            5 => return nm("Stilgar's end of a talk (action 9)"),
            0x0d => return nm("the smuggler's end of a talk (action 9)"),
            _ => {}
        },
        // `loc_0a25b`: the voice's numbers for the scene (sound).
        0x0a => {}
        // `loc_0a28e`: the person shown on the map (its mark, the map
        // drawn and put back: display); unless in the book or at the
        // player's place, the line's state 0.
        0x0d => {
            if w.byte(g::TALK_MODE) != 1 && w.word(g::NOTED_RECORD) != w.word(g::PLACE) {
                w.set_byte(0x47a4, 0);
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
        // `loc_0102f`: sequence 1321h started.
        0x0c => {
            w.set_byte(0x124a, w.byte(0x124a) & 0x7f);
            w.set_byte(0x1247, w.byte(0x1247) & 0x7f);
            w.set_word(0x121d, 0xffff);
            super::native_seq::start_sequence(w, 0x1321);
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
        // `loc_01139`: 10 charisma, sequence 1313h started, record
        // 1048h's flags (10h on, 2 off), [1178h] the people's count + 2,
        // places discovered.
        0x48 => {
            super::native_raids::raise_charisma(w, 0x0a);
            super::native_seq::start_sequence(w, 0x1313);
            w.set_byte(0x1057, (w.byte(0x1057) | 0x10) & 0xfd);
            w.set_byte(0x1178, w.byte(0x28).wrapping_add(2));
            for p in [0x3f4, 0x410, 0x3bc, 0x88c] {
                discover(w, p)?;
            }
        }
        // `loc_01196`: event flag 20h, sequence 12FBh started, places
        // discovered.
        0x58 => {
            w.set_byte(0x0a, w.byte(0x0a) | 0x20);
            super::native_seq::start_sequence(w, 0x12fb);
            for p in [0x7e4, 0x790, 0x7ac, 0x854, 0x81c] {
                discover(w, p)?;
            }
        }
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
        // `loc_01f13`: of the places from 138h (while the next's byte 0 is
        // below 8), the one furthest down (its y, signed, above -64h) not
        // skipped and without an attacker of yours; none: 100h. Character
        // 7 there in room 3, its name the event's place ([0F2h]), message
        // 2B0Ah.
        0x64 => {
            let (mut bx, mut si) = (0xff9cu16, 0u16);
            let mut di = 0x138u16;
            loop {
                if di + 2 * 0x1c > super::world::STORY_END {
                    return nm("a walk past the tables");
                }
                let y = w.word(di + 4);
                if (y as i16) > (bx as i16) && !super::native_day::skipped(w, di) && super::native_raids::sides(w, di)?.1 == 0 {
                    bx = y;
                    si = di;
                }
                di += 0x1c;
                if w.byte(di) >= 8 {
                    break;
                }
            }
            if bx == 0xff9c {
                si = 0x100;
            }
            let n = ((si.wrapping_sub(0x100) / 0x1c + 1) & 0xff) << 8 | 0x80;
            let dx = (w.byte(si + 8) as u16) << 8 | 3;
            w.set_word(0x1048, dx);
            w.set_word(0x104a, n);
            w.set_word(0xf2, (w.byte(si) as u16) << 8 | w.byte(si + 1) as u16);
            add_message(w, 0x2b0a);
        }
        _ => return nm("a phase's handler"),
    }
    Some(())
}

/// `loc_0425b`: the place `p` discovered (flag 80h off, byte 0Bh 0); a
/// sietch (below kind 20h) one more held, and place 603h's discovery
/// raises the phase to 10h.
pub fn discover(w: &mut World, p: u16) -> Option<()> {
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

/// The speakers by number (`characters::who`).
const JESSICA: u16 = 1;
const THUFIR: u16 = 2;
const DUNCAN: u16 = 3;
const STILGAR: u16 = 5;
const HARKONNEN: u16 = 0x0c;
const SMUGGLER: u16 = 0x0d;

/// The dialogue entry's hook ([227Eh]: F66h, nothing; Stilgar's reward
/// sets 2CCFh).
const HOOK: u16 = g::DIALOGUE_ENTRY_HOOK;
/// The map view's mode (46EBh).
const MAP_VIEW_MODE: u16 = 0x46eb;

/// `loc_09761`: the speaker `talker` starts its dialogue: its list (entry
/// 8 x the speaker + 7 of the table at DS:AA76h), its record
/// (`loc_09f40`), mode 80h, the first entry said (`loc_09f9e`). The troop
/// told about (0Eh) and Thufir's sums at the endgame's stage 4 are not
/// modelled. The text box's margins, the portrait and the voice are
/// display.
pub fn speaker_dialogue(w: &mut World, talker: u16) -> Option<()> {
    w.set_word(conv::TALKER, talker);
    if talker as u8 == 0x0e {
        return nm("the troop told about");
    }
    let group = talker << 3 | 7;
    let list = w.word(0xaa76u16.wrapping_add(group << 1));
    text_setup(w)?;
    w.set_byte(conv::MODE, 0x80);
    say_entry(w, list).map(|_| ())
}

/// `loc_09f40`: the speaker's record (Thufir's sums at the endgame's
/// stage 4 not modelled; the text box's margins and font: display).
fn text_setup(w: &mut World) -> Option<()> {
    let talker = w.word(conv::TALKER);
    if talker == THUFIR && w.get::<Economy>().endgame_stage == 4 {
        return nm("Thufir's sums");
    }
    w.set_word(0x47a2, (talker & 0xff) * 0x10 + g::CHARACTERS);
    Some(())
}

/// `loc_096d8` then `loc_09702` with 10h: `talker` says its sentence group
/// 10h's list 4 (entry 84h of the table at DS:AA76h) in mode 20h (the
/// mode given back after).
fn talk_as(w: &mut World, talker: u16) -> Option<()> {
    w.set_word(conv::TALKER, talker);
    say_sentence(w, 0x10).map(|_| ())
}

/// `loc_09702`: the speaker says sentence `n` (the list n x 8 + 4 of the
/// table at DS:AA76h) in mode 20h (given back after).
fn say_sentence(w: &mut World, n: u16) -> Option<bool> {
    text_setup(w)?;
    let list = w.word(0xaa76u16.wrapping_add((n << 3 | 4) << 1));
    let mode = w.word(conv::MODE);
    w.set_byte(conv::MODE, 0x20);
    let said = say_entry(w, list)?;
    w.set_word(conv::MODE, mode);
    Some(said)
}

/// `loc_096f1`: speaker `n` says its sentence `n` (the troop told about,
/// 0Eh, not modelled). Whether it said a line.
pub fn say_numbered(w: &mut World, n: u16) -> Option<bool> {
    w.set_word(conv::TALKER, n);
    if n & 0xff == 0x0e {
        return nm("the troop told about");
    }
    say_sentence(w, n)
}

/// `loc_07bb9` with AL `reason`: the troop `t` commented on: described
/// (`loc_031f6`), the talk's reason [23h], nothing pending, speaker 0Fh's
/// sentence 0Fh (`loc_096f1`). (The voice and the text box: display.)
pub fn comment_on_troop(w: &mut World, t: u16, reason: u8) -> Option<()> {
    describe_troop(w, t)?;
    w.set_byte(0x23, reason);
    w.set_word(0x47ba, 0);
    w.set_word(conv::TALKER, 0x0f);
    say_sentence(w, 0x0f).map(|_| ())
}

/// `loc_031f6`: the troop `t` talked about: the record of the troop
/// talked about filled (its place, number, state, job, age, rates, words
/// 10h and 12h, home, motivation, skills, equipment, size, days at its
/// job) and the phrases for it (the subject: its job + 18h; the detail:
/// its age's, 70h..74h; its home; its skills' levels, D1h + the skill /
/// 16; the item it carries), then its place described.
pub fn describe_troop(w: &mut World, t: u16) -> Option<()> {
    use super::phrase_args::{self as pa, PhraseArgs};
    use super::topic_troop::TopicTroop;
    use super::world::troop;
    let p = w.word(t + troop::LOCATION);
    let state = w.byte(t + troop::STATE);
    let job = state & 0x0f;
    let id = w.byte(t);
    w.update::<TopicTroop>(|r| {
        r.place = p;
        r.id = id;
        r.state = state;
        r.job = job;
    });
    w.update::<PhraseArgs>(|a| a.slots[pa::SUBJECT] = job as u16 + 0x18);
    // `loc_032c7`: its age and the detail's phrase.
    let age = w.time().wrapping_sub(w.word(t + troop::CHANGED_AT));
    let detail = if state & 0x10 != 0 {
        0x74
    } else if age < 3 {
        0x70
    } else if age < 0x10 {
        0x71
    } else if age < 0x20 {
        0x72
    } else {
        0x73
    };
    w.update::<TopicTroop>(|r| {
        r.age = age;
        r.age_days = (age >> 4) as u8;
    });
    w.update::<PhraseArgs>(|a| a.slots[pa::DETAIL] = detail);
    // `loc_0329d`: the average rate over its age, then its rate now.
    let rate = if state != 0 {
        w.set_word(t + troop::WORD_10, w.word(t + troop::WORD_10) & !0x0c);
        0
    } else {
        let average = if age == 0 {
            0
        } else {
            let w0e = w.word(t + troop::WORD_0E);
            w0e / age + (age < (w0e % age) << 1) as u16
        };
        w.update::<TopicTroop>(|r| r.average = average);
        super::native_troops::rate(w, t, p)
    };
    let (w10, w12) = (w.word(t + troop::WORD_10), w.word(t + troop::WORD_12));
    w.update::<TopicTroop>(|r| {
        r.rate = rate;
        r.word_10 = w10;
        r.word_12 = w12;
        r.home = (w12 & 0x0f) as u8;
    });
    w.update::<PhraseArgs>(|a| a.slots[pa::TROOP_HOME] = w12 & 0x0f);
    let m = super::native_troops::motivation(w, t);
    w.update::<TopicTroop>(|r| r.motivation = m);
    let mut phrases = [0u16; 3];
    for k in 0..3u16 {
        let b = w.byte(t + troop::SKILLS + k);
        w.update::<TopicTroop>(|r| r.skills[k as usize] = b);
        phrases[k as usize] = (b as u16 >> 4) + 0xd1;
    }
    w.update::<PhraseArgs>(|a| {
        for k in 0..3 {
            a.slots[pa::SKILLS + k] = phrases[k];
        }
    });
    let (w0c, w0e) = (w.word(t + troop::RATE), w.word(t + troop::WORD_0E));
    let activity = ((w.byte(t + troop::STATE) & 0x0f) >> 2) as u16;
    let job_skill = w.byte(t + troop::SKILLS + activity);
    let (equipment, size) = (w.byte(t + troop::EQUIPMENT), w.byte(t + troop::SIZE_FIELD));
    let days = ((w.time() >> 4) as u8).wrapping_sub(w.byte(t + troop::DAY));
    w.update::<TopicTroop>(|r| {
        r.word_0c = w0c;
        r.word_0e = w0e;
        r.job_skill = job_skill;
        r.equipment = equipment;
        r.size = size;
        r.days = days;
    });
    w.update::<PhraseArgs>(|a| a.slots[pa::SMUGGLER_ITEM] = (w0e & 0xff) + 0xe8);
    super::native_step::describe_place(w, p)
}

/// `loc_06a89`, the orders menu: the troop `t` given job `job` unless it
/// has it (`loc_06acb`), the order allowed ([47A5h] FFh), the troop's
/// comment (`loc_07bb9`, reason 0Ah); an entry of it may forbid the
/// order (action 2): then its word 12h and its job are given back; else
/// a troop with an activity loses its harvester (bit 80h of byte 19h).
pub fn order_job(w: &mut World, t: u16, job: u8) -> Option<()> {
    use super::world::troop;
    let state = w.byte(t + troop::STATE);
    if state & 0x0f == job {
        return Some(());
    }
    let w12 = w.word(t + troop::WORD_12);
    super::native_jobs::give_job_unless_same(w, t, job);
    w.set_byte(conv::FLAG_47A5, 0xff);
    comment_on_troop(w, t, 0x0a)?;
    if w.byte(conv::FLAG_47A5) == 0xff {
        if (w.byte(t + troop::STATE) & 0x0f) >> 2 != 0 {
            w.set_byte(t + troop::EQUIPMENT, w.byte(t + troop::EQUIPMENT) & 0x7f);
        }
        return Some(());
    }
    w.set_word(t + troop::WORD_12, w12);
    super::native_jobs::give_job_unless_same(w, t, state);
    Some(())
}


/// `loc_02ad8`: the next event waiting told when its teller is present
/// (its bit of [12h]; teller 0Fh only at its place): its place described
/// (when it has one), the event's kind noted ([0EAh], then FFh), the
/// teller's sentence, the place talked about described again. Whether it
/// was told.
pub fn tell_event(w: &mut World) -> Option<bool> {
    let (kind, teller, place) = (w.byte(g::NEXT_EVENT), w.byte(g::NEXT_EVENT + 1), w.word(g::NEXT_EVENT + 2));
    let present = w.get::<Progress>().present;
    let sh = (teller & 0x1f) as u32;
    let bit = sh < 16 && (present >> sh) & 1 != 0;
    if !bit || (teller == 0x0f && place != w.word(g::PLACE)) {
        return Some(false);
    }
    tell_found(w, kind as u16 | (teller as u16) << 8, place)?;
    Some(true)
}

/// `loc_02b00`: the event `ev` (kind, teller) at `place` told: its place
/// described (when it has one), the event's kind noted ([0EAh], then
/// FFh), the teller's sentence, the place talked about described again.
pub fn tell_found(w: &mut World, ev: u16, place: u16) -> Option<()> {
    let keep = w.word(g::TOPIC_PLACE_RECORD);
    if place != 0 {
        super::native_step::describe_place(w, place)?;
    }
    w.update::<Situation>(|s| s.event_kind = ev as u8);
    talk_as(w, ev >> 8)?;
    w.update::<Situation>(|s| s.event_kind = 0xff);
    super::native_step::describe_place(w, keep)
}

/// `loc_02a51`: the events of the teller `teller` dropped (teller 0Fh's
/// only at the place `at`), the others kept in order, two words of 0
/// after when any went.
pub fn drop_events(w: &mut World, teller: u8, at: u16) {
    let n = w.byte(g::PENDING_EVENTS);
    let mut kept: Vec<(u16, u16)> = Vec::new();
    for i in 0..n as u16 {
        let e = g::NEXT_EVENT + 4 * i;
        let (ev, p) = (w.word(e), w.word(e + 2));
        let drop = (ev >> 8) as u8 == teller && (teller != 0x0f || p == at);
        if !drop {
            kept.push((ev, p));
        }
    }
    for (i, (ev, p)) in kept.iter().enumerate() {
        let e = g::NEXT_EVENT + 4 * i as u16;
        w.set_word(e, *ev);
        w.set_word(e + 2, *p);
    }
    w.set_byte(g::PENDING_EVENTS, kept.len() as u8);
    if kept.len() as u8 != n {
        let e = g::NEXT_EVENT + 4 * kept.len() as u16;
        w.set_word(e, 0);
        w.set_word(e + 2, 0);
    }
}

/// `loc_094f3`: for a person (speaker below 10h), its flags and the time
/// since it was met (word 8 in the party, else word 0Ah) noted; before
/// phase 64h, for the first eight, the attack's target noted as the
/// place (`loc_02e98`: the sentence's record, the place's name in the
/// phrase slots).
fn speaker_mood(w: &mut World) {
    let talker = w.word(conv::TALKER);
    if talker >= 0x10 {
        return;
    }
    let record = w.word(0x47a2);
    let flags = w.byte(record.wrapping_add(0x0f));
    let then = w.word(record.wrapping_add(if flags & 0x40 == 0 { 0x0a } else { 8 }));
    let since = w.time().wrapping_sub(then);
    w.update::<Progress>(|t| {
        t.speaker_flags = flags;
        t.speaker_time_since = since;
    });
    if w.get::<Progress>().phase >= 0x64 || talker >= 9 {
        return;
    }
    let place = w.word(g::ATTACK_TARGET);
    if place == 0 {
        return;
    }
    w.set_word(g::NOTED_RECORD, place);
    let x = w.byte(place) as u16;
    let y = (w.byte(place.wrapping_add(1)) as u16).wrapping_add(0x0c);
    w.update::<super::phrase_args::PhraseArgs>(|t| {
        t.slots[super::phrase_args::PLACE] = x;
        t.slots[super::phrase_args::PLACE + 1] = y;
    });
}

/// After an entry said, for a person outside the map views: unless the
/// event's kind is positive, the dialogue entry's hook run once (reset to
/// F66h; Stilgar's, 2CCFh, its answer 1 not modelled). The scene and the voice are
/// display.
fn after_entry(w: &mut World) -> Option<()> {
    if w.byte(MAP_VIEW_MODE) != 0 || w.word(conv::TALKER) >= 0x10 {
        return Some(());
    }
    if (w.get::<Situation>().event_kind as i8) > 0 {
        return Some(());
    }
    let hook = w.word(HOOK);
    w.set_word(HOOK, 0xf66);
    match hook {
        0xf66 => Some(()),
        // `loc_02ccf`, Stilgar's: event flag 8; with the answer 1 (yes) his
        // offer goes on (not modelled).
        0x2ccf => {
            w.set_byte(0x0a, w.byte(0x0a) | 8);
            if w.get::<Economy>().answer == 1 {
                return nm("Stilgar's offer taken");
            }
            Some(())
        }
        _ => nm("a dialogue entry's hook"),
    }
}

/// `loc_0a125`, action 8: the talk's reward by the speaker. Jessica:
/// with event flag 2, 40 charisma and the reach from FFCEh, else at reach
/// 1 10 charisma and the reach from 0Ah, else the reach on
/// (`loc_0a1b3`); Stilgar: his hook; the Harkonnen: the place talked
/// about discovered; the smuggler's deal opened (`loc_02388`); Duncan's
/// (`loc_02239`) not modelled.
fn reward(w: &mut World) -> Option<()> {
    match w.word(conv::TALKER) {
        JESSICA => {
            let from = if w.get::<Progress>().event_flags & 2 != 0 {
                super::native_raids::raise_charisma(w, 0x28);
                0xffce
            } else {
                let reach = w.word(g::REACH);
                if reach == 1 {
                    super::native_raids::raise_charisma(w, 0x0a);
                    0x0a
                } else {
                    reach
                }
            };
            let reach = from.wrapping_add(0x14);
            w.set_word(g::REACH, reach);
            let d5 = if reach < 0x64 { 0x80u8.wrapping_sub((reach / 6) as u8) } else { 0 };
            w.update::<Situation>(|s| s.jessica_days = d5);
        }
        DUNCAN => return nm("Duncan's reward"),
        STILGAR => w.set_word(HOOK, 0x2ccf),
        t => {
            if t == HARKONNEN {
                let p = w.word(g::TOPIC_PLACE_RECORD);
                w.set_byte(p.wrapping_add(0x0a), w.byte(p.wrapping_add(0x0a)) & 0x7f);
            }
            if t == SMUGGLER {
                return super::native_player::open_deal(w).map(|_| ());
            }
        }
    }
    Some(())
}

/// For the check of `loc_09f9e`: the dialogue step from the list `list`.
pub fn say_entry_check(w: &mut World, list: u16) -> Option<()> {
    say_entry(w, list).map(|_| ())
}

/// `loc_093df`: the speaker `talker`'s talk started: its group (talker x
/// 8), met and talked to here ([0Eh], [14h]), its record, nothing pending
/// or resumed, mode 80h, no line said yet. (Its menu item: display.)
fn start_speaker(w: &mut World, talker: u16) {
    let speaker = talker as u8;
    w.set_word(0x47be, talker & 0xff00 | (speaker as u16) << 3);
    let shift = speaker & 0x1f;
    let bit = if shift < 16 { 1u16 << shift } else { 0 };
    for at in [g::MET, g::TALKED_HERE] {
        w.set_word(at, w.word(at) | bit);
    }
    w.set_word(0x47a2, 0x10 * speaker as u16 + g::CHARACTERS);
    w.set_word(0x47ba, 0);
    w.set_word(conv::PHRASE_RESUME, 0);
    w.set_byte(conv::MODE, 0x80);
    w.update::<Progress>(|t| t.speaker_said_line = 0);
}

/// The simulator's idle player (it hears and leaves): where the clock's
/// step (`loc_02b2a`) tells the next event to a player idle on the main
/// screen (no call, no sequence, no travel; from phase 14h; events
/// waiting), it is told (`loc_02ad8`); told, [23h] 0 and its scene
/// (`loc_03542`: the teller's events at the place dropped, no rescues, the
/// scene counted, the teller's talk started outside a sequence before
/// talk 64h); the player then leaves the scene (display). At most one
/// event between two steps. Phase 14h's own idle start (`loc_02ba1`) is
/// not modelled.
pub fn idle_player(w: &mut World) -> Option<()> {
    if w.byte(g::COMM_CALLER) | w.byte(0x4774) | w.byte(0x11c9) != 0 {
        return Some(());
    }
    let phase = w.get::<Progress>().phase;
    if phase < 0x14 {
        return Some(());
    }
    if phase == 0x14 {
        return phase_14_start(w);
    }
    if w.byte(g::PENDING_EVENTS) == 0 {
        return Some(());
    }
    if !tell_event(w)? {
        return Some(());
    }
    w.set_byte(0x23, 0);
    let talker = w.word(conv::TALKER);
    let place = w.word(g::PLACE);
    drop_events(w, talker as u8, place);
    w.update::<Situation>(|s| s.desert_rescues = 0);
    w.set_byte(0x47a7, w.byte(0x47a7).wrapping_add(1));
    if w.byte(0x4774) == 0 && w.byte(0x23) < 0x64 {
        start_speaker(w, talker);
    }
    Some(())
}

/// `loc_02ba1`: in phase 14h, a player with no party ([10h]) at no place
/// ([8] FFh) idle 1000 ticks (the simulator's idle player is): phase 15h
/// set up (`loc_01071`: its days 0, [0FDBh] 1, record 1018h's room and
/// place, [0FE8h] 0Ah, [0D5h] FFh, the first demand today, event flag 1,
/// event 1), the phase change's step, then the clock's step until an
/// event is told ([0EAh] FFh): the idle player hears it.
fn phase_14_start(w: &mut World) -> Option<()> {
    if w.word(g::PARTY) != 0 || w.byte(8) != 0xff {
        return Some(());
    }
    w.set_byte(0xff, 0);
    w.set_byte(g::GAME_PHASE, 0x15);
    w.set_byte(0xfdb, 1);
    w.set_word(0x1018, 0x200b);
    w.set_word(0x101a, 0x180);
    w.set_byte(0xfe8, 0x0a);
    w.set_byte(0xd5, 0xff);
    super::native_step::demand_today(w);
    w.set_byte(0x0a, w.byte(0x0a) | 1);
    add_pending_event(w, 1, 0);
    on_phase_change(w)?;
    idle_player(w)
}

/// For the check of `loc_06a89`: the troop shown (in the zoomed map, mode
/// 80h and up: troop [1954h]; else not modelled) ordered job `job`.
pub fn order_shown_check(w: &mut World, job: u8) -> Option<()> {
    if w.byte(MAP_VIEW_MODE) < 0x80 {
        return nm("the shown troop from the character list");
    }
    let t = super::native_troops::troop_number(w.byte(g::SHOWN_TROOP));
    order_job(w, t, job)
}

/// [`start_speaker`] for the travel's scenes.
pub fn start_speaker_pub(w: &mut World, talker: u16) {
    start_speaker(w, talker)
}
