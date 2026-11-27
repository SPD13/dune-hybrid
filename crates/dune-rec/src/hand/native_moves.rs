//! The troops' tick and movement natively (the native mode, Stage C of
//! `docs/readable-port-plan.md`): `loc_06c6f` (each troop's tick: wait,
//! merge, work, travel) and what it calls, as rules on a [`World`]. The
//! decisions are `troop_moves`'; the walks are the original's
//! (`loc_06c6f`, `loc_06d19`, `loc_08308`, `loc_08604`, `loc_08357`,
//! `loc_083bc`, `loc_0851f`, `loc_0858c`, `loc_085cc`, `loc_066b1`,
//! `loc_07f27`, `loc_08403`). `None`: a case not modelled yet (troops
//! meeting at a place: the raids; the phase 30h's check talking), the
//! world then partly changed.
//!
//! The display on the way (the troops' figures on the map, the panels
//! redrawn, the orders menu) is not state and is left out.

use super::game::{location_flag, troop_job, troop_state};
use super::globals as g;
use super::native_day::{skipped, your_troops};
use super::native_jobs::{set_activity, set_job, work};
use super::native_troops::{mark, skills_decay, troop_number};
use super::world::{place, troop, World, STORY_END};
use super::{progress::Progress, situation::Situation, troop_moves};

/// The record reset troops point at (0FBCh).
const NOWHERE: u16 = super::variables::NOWHERE;
/// The followed troop's note (11D3h: the place it goes to, then two more).
const FOLLOWED: u16 = g::FOLLOWED_TROOP_PLACE;
/// The steps' scale by the row (DS:4880h, a table of the game's).
const SCALES: u16 = 0x4880;
/// A place's spare equipment, by kind (46FEh, `loc_07f27`'s).
const SPARE: u16 = g::LOCATION_SPARE_EQUIPMENT;
/// The troop's destination (word 0Ch) and what it carries (0Eh).
const DESTINATION: u16 = 0x0c;
const CARRIED: u16 = 0x0e;
const X: u16 = 6;
const Y: u16 = 8;
const SLOT: u16 = 2;

/// `loc_0851f`: the troop linked at the end of the place's chain, in its
/// first free slot (from 9 for the Harkonnens', else from 1: the slots
/// of the chain marked in a table of 1Eh; 1Eh when none is free).
fn link(w: &mut World, t: u16, p: u16) -> Option<()> {
    let id = w.byte(t + troop::ID);
    let lowest: u8 = if w.byte(t + troop::BYTE_10) & 0x80 != 0 { 9 } else { 1 };
    let first = w.byte(p + place::FIRST_TROOP);
    if first == 0 {
        w.set_byte(p + place::FIRST_TROOP, id);
        w.set_byte(t + SLOT, lowest);
        return Some(());
    }
    let mut used = [false; 0x1e];
    let mut n = first;
    let mut k = 0;
    let last = loop {
        let o = troop_number(n);
        if o + troop::SIZE > STORY_END || k > 0x100 {
            return None;
        }
        k += 1;
        let slot = w.byte(o + SLOT);
        if (1..=0x1e).contains(&slot) {
            used[slot as usize - 1] = true;
        }
        n = w.byte(o + troop::NEXT);
        if n == 0 {
            break o;
        }
    };
    w.set_byte(last + troop::NEXT, id);
    let from = lowest as usize - 1;
    let slot = (from..0x1e).find(|&i| !used[i]).map_or(0x1e, |i| i + 1);
    w.set_byte(t + SLOT, slot as u8);
    Some(())
}

/// `loc_0858c`: the troop taken out of its place's chain (a travelling
/// troop is in none), its next 0.
fn unlink(w: &mut World, t: u16) -> Option<()> {
    let id = w.byte(t + troop::ID);
    let p = w.word(t + troop::LOCATION);
    if w.byte(t + troop::STATE) & troop_state::TRAVELLING == 0 {
        let first = w.byte(p + place::FIRST_TROOP);
        if first == id {
            w.set_byte(p + place::FIRST_TROOP, w.byte(t + troop::NEXT));
        } else {
            let mut n = first;
            let mut k = 0;
            loop {
                let before = troop_number(n);
                if before + troop::SIZE > STORY_END || k > 0x100 {
                    return None;
                }
                k += 1;
                n = w.byte(before + troop::NEXT);
                if n == 0 {
                    break;
                }
                if n == id {
                    w.set_byte(before + troop::NEXT, w.byte(t + troop::NEXT));
                    break;
                }
            }
        }
    }
    w.set_byte(t + troop::NEXT, 0);
    Some(())
}

/// `loc_066b1`: the troop unlinked and reset: nowhere, state A0h (not a
/// troop, started), no men.
pub fn reset(w: &mut World, t: u16) -> Option<()> {
    unlink(w, t)?;
    w.set_word(t + troop::LOCATION, NOWHERE);
    w.set_byte(t + troop::STATE, troop_state::NOT_A_TROOP | troop_state::STARTED);
    w.set_byte(t + troop::SIZE_FIELD, 0);
    Some(())
}

/// `loc_085cc`: a troop of yours in a slot past 8 is taken out, the first
/// started troop of yours there reset, and linked again.
fn reset_location_troop(w: &mut World, t: u16, p: u16) -> Option<()> {
    if w.byte(t + troop::BYTE_10) & 0x80 != 0 || w.byte(t + SLOT) <= 8 {
        return Some(());
    }
    unlink(w, t)?;
    let mut n = w.byte(p + place::FIRST_TROOP);
    let mut k = 0;
    while n != 0 {
        let o = troop_number(n);
        if o + troop::SIZE > STORY_END || k > 0x100 {
            return None;
        }
        k += 1;
        n = w.byte(o + troop::NEXT);
        if w.byte(o + troop::BYTE_10) & 0x80 == 0 && w.byte(o + troop::STATE) & troop_state::STARTED != 0 {
            reset(w, o)?;
            break;
        }
    }
    link(w, t, p)
}

/// The kinds a byte of equipment holds (bit 7 kind 0, ...).
fn kinds(bits: u8) -> impl Iterator<Item = u16> {
    (0..8u16).filter(move |&k| bits & (0x80 >> k) != 0)
}

/// `loc_083bc`: the troop arrived at the place: linked there, no longer
/// travelling, a troop past slot 8 made room for, its equipment added to
/// the place's; a troop of yours (state below 80h) given its job again.
fn settle(w: &mut World, t: u16, p: u16) -> Option<()> {
    link(w, t, p)?;
    w.set_byte(t + troop::STATE, w.byte(t + troop::STATE) & !troop_state::TRAVELLING);
    reset_location_troop(w, t, p)?;
    // Kind 7 is the place's byte 1Bh, as the original's walk has it.
    for k in kinds(w.byte(t + troop::EQUIPMENT)) {
        w.set_byte(p + place::EQUIPMENT + k, w.byte(p + place::EQUIPMENT + k).wrapping_add(1));
    }
    let state = w.byte(t + troop::STATE);
    if state & troop_state::NOT_A_TROOP == 0 {
        set_job(w, t, state & 0x0f);
    }
    Some(())
}

/// `loc_08403` for each troop of yours at the place: unless started,
/// prospectors marked, others but attackers sent to attack.
fn alert(w: &mut World, p: u16) -> Option<()> {
    for x in your_troops(w, p)? {
        let state = w.byte(x + troop::STATE);
        if state & troop_state::STARTED != 0 {
            continue;
        }
        match state & 0x0f {
            troop_job::SPICE_PROSPECTING => mark(w, x),
            troop_job::ATTACKS => {}
            _ => {
                if w.byte(x + troop::STATE) & 0x0f != troop_job::ATTACKS {
                    set_job(w, x, troop_job::ATTACKS);
                }
            }
        }
    }
    Some(())
}

/// `loc_07f27`: the place's spare equipment of kind `kind`: its stock
/// less what each troop of its chain holds.
fn spare(w: &mut World, p: u16, kind: u16) -> Option<bool> {
    let mut out = [0u8; 7];
    for (k, v) in out.iter_mut().enumerate() {
        *v = w.byte(p + place::EQUIPMENT + k as u16);
    }
    let mut n = w.byte(p + place::FIRST_TROOP);
    let mut c = 0;
    while n != 0 {
        let o = troop_number(n);
        if o + troop::SIZE > STORY_END || c > 0x100 {
            return None;
        }
        c += 1;
        for k in kinds(w.byte(o + troop::EQUIPMENT)) {
            let v = *out.get(k as usize)?;
            if v != 0 {
                out[k as usize] = v - 1;
            }
        }
        n = w.byte(o + troop::NEXT);
    }
    for (k, v) in out.iter().enumerate() {
        w.set_byte(SPARE + k as u16, *v);
    }
    Some(*out.get(kind as usize)? != 0)
}

/// `loc_0841f`: a troop moving on: at its destination it stops there
/// (settles); else it takes what it carries from the place's spare
/// ([`troop_moves::take_carried`]) and heads for its destination.
fn move_on(w: &mut World, t: u16) -> Option<()> {
    let p = w.word(t + troop::LOCATION);
    let next = w.word(t + DESTINATION);
    if troop_moves::at_destination(p, next) {
        w.set_byte(t + troop::STATE, w.byte(t + troop::STATE) & !3);
        return settle(w, t, p);
    }
    let mut carried = w.word(t + CARRIED);
    let kind = carried & 0xff;
    let has = spare(w, p, kind)?;
    if let Some((bits, kept)) = troop_moves::take_carried(carried, has, w.byte(t + troop::EQUIPMENT)) {
        w.set_byte(p + place::EQUIPMENT + kind, w.byte(p + place::EQUIPMENT + kind).wrapping_sub(1));
        w.set_byte(t + troop::EQUIPMENT, bits);
        carried = kept;
    }
    w.set_word(t + CARRIED, carried);
    w.set_word(t + troop::LOCATION, next);
    Some(())
}

/// `loc_08357`: the troop at its place. The followed troop 3 reaching the
/// followed place shifts the note. At its place: found (a Harkonnen place
/// always, else one not skipped: [`troop_moves::place_found`]) it
/// settles; a spy discovers an undiscovered place, else the troops there
/// are alerted, or the troop meets them (not modelled). Not found: a
/// troop given up on goes idle, a moving one moves on, others settle.
fn arrived(w: &mut World, t: u16) -> Option<()> {
    let p = w.word(t + troop::LOCATION);
    if t == troop_number(3) && w.word(FOLLOWED) == p {
        for k in 0..6 {
            w.set_byte(FOLLOWED + k, w.byte(FOLLOWED + 2 + k));
        }
    }
    let job = w.byte(t + troop::STATE) & 0x0f;
    w.set_word(t + X, w.word(p + 2));
    w.set_word(t + Y, w.word(p + 4));
    let harkonnen = w.byte(p + place::FLAGS) & location_flag::HARKONNEN != 0;
    let found = troop_moves::place_found(harkonnen, || skipped(w, p));
    if found {
        settle(w, t, p)?;
        let flags = w.byte(p + place::FLAGS);
        let meets = w.byte(t + troop::ID) == w.byte(p + place::FIRST_TROOP);
        let a = troop_moves::arrival(job, flags, meets);
        if job == troop_job::ESPIONAGE {
            if a == troop_moves::Arrival::Discovered {
                w.set_byte(p + place::FLAGS, flags & !location_flag::UNDISCOVERED);
                return Some(());
            }
            return alert(w, p);
        }
        if meets {
            return None;
        }
        return alert(w, p);
    }
    let (drop, _) = troop_moves::not_found(job);
    let mut al = job;
    if drop {
        set_activity(w, t);
        al = 0;
    }
    if al & 3 == 3 {
        return move_on(w, t);
    }
    settle(w, t, p)
}

/// `loc_08604`: the troop's step towards its place
/// ([`troop_moves::step`], the row's scale from DS:4880h, the random bits'
/// top bit; a turn rotates them).
fn step(w: &mut World, t: u16) -> Option<(u16, u16)> {
    let p = w.word(t + troop::LOCATION);
    let (tx, ty) = (w.word(t + X), w.word(t + Y));
    let row = ty.wrapping_shl(1);
    let row = if row & 0x8000 != 0 { row.wrapping_neg() } else { row };
    if row >= 0x300 {
        return None;
    }
    let scale = w.word(SCALES + row);
    if scale == 0 {
        return None;
    }
    let seed = w.word(g::RANDOM_BITS);
    let s = troop_moves::step(w.word(p + 2).wrapping_sub(tx), w.word(p + 4).wrapping_sub(ty), scale, seed & 0x8000 != 0);
    if s.turns {
        w.set_word(g::RANDOM_BITS, seed.rotate_left(1));
    }
    Some((s.dx, s.dy))
}

/// `loc_08308`: the troop's steps of a tick
/// ([`troop_moves::steps_per_tick`]); still, it has arrived.
fn travel(w: &mut World, t: u16) -> Option<()> {
    for _ in 0..troop_moves::steps_per_tick(w.byte(t + troop::EQUIPMENT)) {
        let (dx, dy) = step(w, t)?;
        if dx | dy == 0 {
            return arrived(w, t);
        }
        w.set_word(t + X, w.word(t + X).wrapping_add(dx));
        w.set_word(t + Y, w.word(t + Y).wrapping_add(dy));
    }
    Some(())
}

/// `loc_06d19`: a small troop of yours, idle (no bits E3h), not the
/// Harkonnens' nor troop 3, merges into the largest of yours there it
/// fits beside (up to FFh men; the last of equals), not started nor
/// another's: the men and equipment joined, the other's bit 200h of word
/// 12h, this one reset.
fn merge(w: &mut World, t: u16) -> Option<bool> {
    if w.byte(t + troop::STATE) & 0xe3 != 0 || w.byte(t + troop::BYTE_10) & 0x80 != 0 || t == troop_number(3) {
        return Some(false);
    }
    let p = w.word(t + troop::LOCATION);
    let mut cl = !w.byte(t + troop::SIZE_FIELD);
    let mut into = 0;
    for x in your_troops(w, p)? {
        if w.byte(x + troop::STATE) & 0xa0 != 0 || x == troop_number(3) || x == t {
            continue;
        }
        let al = w.byte(x + troop::SIZE_FIELD);
        if cl >= al {
            into = x;
            cl = al;
        }
    }
    if into == 0 {
        return Some(false);
    }
    let sum = w.byte(into + troop::SIZE_FIELD).wrapping_add(w.byte(t + troop::SIZE_FIELD));
    w.set_byte(into + troop::SIZE_FIELD, sum);
    let mine = w.byte(t + troop::EQUIPMENT);
    w.set_byte(t + troop::EQUIPMENT, mine & w.byte(into + troop::EQUIPMENT));
    w.set_byte(into + troop::EQUIPMENT, w.byte(into + troop::EQUIPMENT) | mine);
    w.set_word(into + troop::WORD_12, w.word(into + troop::WORD_12) | 0x200);
    reset(w, t)?;
    Some(true)
}

/// `loc_06c46`: from phase 2Dh to 2Fh, the party without bit 10h, at a
/// place: the place talked about, and phase 30h set when the Harkonnens
/// are there (not modelled: `None`).
fn phase_30_check(w: &World) -> Option<()> {
    let p = w.get::<Progress>();
    if p.phase.wrapping_sub(0x2d) >= 3 || p.party & 0x10 != 0 || w.word(g::TICK_LOCATION) == 0 {
        return Some(());
    }
    None
}

/// `loc_06c6f`: the troops' tick. The place of character 4 (the tick's
/// location), the phase 30h's check, then each troop of the table: a
/// waiting one (bits 430h of word 12h) travels on if travelling, else,
/// when the troops are rechecked, is released (bits 30h of byte 12h) and
/// ticks again unless held; a small one may merge; then by its state
/// ([`troop_moves::tick_action`]) skipped, travelling (unless the troop
/// shown) or at work (its job's handler, its skills' decay). The recheck
/// asked for during the tick is kept for the next.
pub fn tick(w: &mut World) -> Option<()> {
    let c4 = w.word(g::CHARACTERS + 0x40 + 2);
    let at = if c4 & 0xff == 0x80 { ((c4 >> 8) as u8).wrapping_sub(1) as u16 * place::SIZE + place::FIRST } else { 0 };
    w.set_word(g::TICK_LOCATION, at);
    phase_30_check(w)?;
    w.set_byte(g::TROOPS_TICK_RECHECK, 0);
    enum Step {
        Start,
        Act,
        Waiting,
        Travel,
    }
    let mut t = troop::FIRST;
    while t < g::TROOPS_END {
        let mut s = Step::Start;
        loop {
            s = match s {
                Step::Start => {
                    if troop_moves::waits(w.word(t + troop::WORD_12)) {
                        Step::Waiting
                    } else if !troop_moves::may_merge(w.byte(t + troop::SIZE_FIELD)) {
                        Step::Act
                    } else if merge(w, t)? {
                        break;
                    } else {
                        Step::Act
                    }
                }
                Step::Act => match troop_moves::tick_action(w.byte(t + troop::STATE)) {
                    troop_moves::TickAction::Skip => break,
                    troop_moves::TickAction::Travel => Step::Travel,
                    troop_moves::TickAction::Work(job) => {
                        work(w, job, t, w.word(t + troop::LOCATION))?;
                        skills_decay(w, t);
                        break;
                    }
                },
                Step::Waiting => {
                    if w.byte(t + troop::STATE) & troop_state::TRAVELLING != 0 {
                        Step::Travel
                    } else if w.get::<Situation>().troops_recheck == 0 {
                        break;
                    } else {
                        let held = w.word(t + troop::WORD_12) & troop_moves::HELD != 0;
                        w.set_byte(t + troop::BYTE_12, w.byte(t + troop::BYTE_12) & !0x30);
                        if held {
                            break;
                        }
                        Step::Start
                    }
                }
                Step::Travel => {
                    if w.byte(t + troop::ID) != w.byte(g::SHOWN_TROOP) {
                        travel(w, t)?;
                    }
                    break;
                }
            };
        }
        t += troop::SIZE;
    }
    let again = w.byte(g::TROOPS_TICK_RECHECK);
    w.update::<Situation>(|s| s.troops_recheck = again);
    Some(())
}

/// For the checks of the move's ports: `loc_08308` (the troop's steps of
/// a tick), `loc_06d19` (a merge), `loc_066b1` (a reset).
pub fn travel_check(w: &mut World, t: u16) -> Option<()> {
    travel(w, t)
}
pub fn merge_check(w: &mut World, t: u16) -> Option<()> {
    merge(w, t).map(|_| ())
}
pub fn settle_check(w: &mut World, t: u16, p: u16) -> Option<()> {
    settle(w, t, p)
}
pub fn link_check(w: &mut World, t: u16, p: u16) -> Option<()> {
    link(w, t, p)
}
pub fn unlink_check(w: &mut World, t: u16) -> Option<()> {
    unlink(w, t)
}
