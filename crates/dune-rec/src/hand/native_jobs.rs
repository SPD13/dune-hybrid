//! The troops' jobs natively (the native mode, Stage C of
//! `docs/readable-port-plan.md`): the job handlers the troops' tick calls
//! (the table at CS:6C26h) and the job changes, as rules on a [`World`].
//! The decisions are `troop_jobs`' and `troop_moves`'; the walks are the
//! original's (`loc_06fe5`, `loc_070cc`, `loc_071ef`, `loc_072b0`,
//! `loc_07693`, `loc_07711`, `loc_0767d`, `loc_06ad4`, ...). `None`: a
//! case not modelled yet (a troop setting out, water spreading on the
//! map, the places' forces measured, a troop started), the world then
//! partly changed.

use super::game::{location_flag, troop_job};
use super::globals as g;
use super::native_day::{add_pending_event, your_troops};
use super::native_troops::{add_skill, day_update, event_roll, held_by_state, motivation, rate, set_held, stamp};
use super::world::{place, troop, World};
use super::{economy::Economy, situation::Situation, troop_jobs, troop_moves};

/// The spice harvested, in tenths not counted yet (46E1h, not saved).
const SPICE_TENTHS: u16 = 0x46e1;
/// The troop selected on the map (46EFh, the map's selection).
const SELECTED: u16 = 0x46ef;

/// A location's other fields the jobs use.
mod field {
    pub const X: u16 = 2;
    pub const Y: u16 = 4;
    pub const WORD_0C: u16 = 0x0c;
    pub const WORD_0E: u16 = 0x0e;
    pub const BYTE_0E: u16 = 0x0e;
    pub const IRRIGATION: u16 = 0x0f;
    pub const PER_SPICE: u16 = 0x11;
    pub const LEFT: u16 = 0x13;
    pub const BYTE_1A: u16 = 0x1a;
    pub const WATER: u16 = 0x1b;
}

/// The job handler for job `job` of the troop `t` at the place `p`
/// (CS:6C26h; jobs 2, 3, 6, 7 and 0Bh up have none here: 6, the attacks,
/// has no port yet).
pub fn work(w: &mut World, job: u8, t: u16, p: u16) -> Option<()> {
    match job {
        troop_job::SPICE_MINING => harvest(w, t, p),
        troop_job::SPICE_PROSPECTING => prospecting(w, t, p),
        4 => training(w, t, p),
        5 => espionage(w, t, p),
        6 => super::world::nm("the attack job"),
        troop_job::IRRIGATION => irrigation(w, t, p),
        9 => wind_trap(w, t, p),
        0x0a => bulbs(w, t, p),
        _ => Some(()),
    }
}

/// `loc_06f48`: motivation up by `by`, at most 100 (the byte wrapping
/// first).
fn add_motivation(w: &mut World, t: u16, by: u8) {
    let v = w.byte(t + troop::MOTIVATION).wrapping_add(by);
    w.set_byte(t + troop::MOTIVATION, v.min(100));
}

/// `loc_0348a`: the troop's strength: twice its motivation plus its
/// skill 0 (the first add's carry lost, the second saturating), times its
/// size, over 1000h; 1 for any men.
fn strength(w: &World, t: u16) -> u8 {
    let m = motivation(w, t);
    let (sum, carry) = m.wrapping_add(m).overflowing_add(w.byte(t + troop::SKILLS));
    let sum = if carry { 0xff } else { sum };
    let men = w.byte(t + troop::SIZE_FIELD);
    let s = ((sum as u16 * men as u16) >> 12) as u8;
    if s == 0 { (men != 0) as u8 } else { s }
}

/// `loc_06b8a`: held while the place is prospected or started (flags
/// 41h).
fn held_by_location(w: &mut World, t: u16, p: u16) -> bool {
    let held = w.byte(p + place::FLAGS) & 0x41 != 0;
    set_held(w, t, held);
    held
}

/// `loc_06bd7`: held unless free (bits 30h of word 12h clear), with water
/// at the place, its wind-trap built and the troop's equipment bit 2.
fn held_by_skill(w: &mut World, t: u16, p: u16) -> bool {
    let free = w.word(t + troop::WORD_12) & 0x30 == 0
        && w.byte(p + field::WATER) >= 1
        && w.byte(p + place::FLAGS) & location_flag::WIND_TRAP != 0
        && w.byte(t + troop::EQUIPMENT) & 2 != 0;
    set_held(w, t, !free);
    !free
}

/// `loc_06c15`: the check of the troop's job (CS:6BF5h): mining by its
/// state, prospecting by its place, irrigation by its skill; others free.
fn job_check(w: &mut World, t: u16) -> bool {
    let p = w.word(t + troop::LOCATION);
    match w.byte(t + troop::STATE) & 0x0f {
        0 => held_by_state(w, t, p),
        1 => held_by_location(w, t, p),
        8 => held_by_skill(w, t, p),
        _ => false,
    }
}

/// `loc_06ad4`: the troop takes job `job` ([`troop_moves::job_given`]):
/// its state the job, bits 30h of byte 12h and 100h of word 10h cleared,
/// 100h again unless its job check holds it, stamped, its activity
/// marked in word 12h ([`troop_moves::activity_mark`]). (The orders menu
/// redrawn for the troop shown is display.)
pub fn set_job(w: &mut World, t: u16, job: u8) {
    let at = w.word(t + troop::LOCATION);
    let given = troop_moves::job_given(job, at, || w.byte(at + field::BYTE_1A));
    w.set_byte(t + troop::STATE, given);
    w.set_byte(t + troop::BYTE_12, w.byte(t + troop::BYTE_12) & !0x30);
    w.set_word(t + troop::WORD_10, w.word(t + troop::WORD_10) & !0x100);
    if !job_check(w, t) {
        w.set_word(t + troop::WORD_10, w.word(t + troop::WORD_10) | 0x100);
    }
    stamp(w, t);
    let state = w.byte(t + troop::STATE);
    if let Some(bit) = troop_moves::activity_mark(state, (state & 0x0f) >> 2) {
        w.set_word(t + troop::WORD_12, w.word(t + troop::WORD_12) | bit);
    }
}

/// `loc_06ac5`: the troop's job made its state's bits FCh, unless the
/// same.
pub fn set_activity(w: &mut World, t: u16) {
    let state = w.byte(t + troop::STATE);
    if state & 0x0f != state & 0xfc {
        set_job(w, t, state & 0xfc);
    }
}

/// `loc_06acb`: the troop given job `job` unless it has it.
pub fn give_job_unless_same(w: &mut World, t: u16, job: u8) {
    if w.byte(t + troop::STATE) & 0x0f != job {
        set_job(w, t, job);
    }
}

/// `loc_06fe5`, job 0: spice mining. The troop's day; in its slot of the
/// time ([`troop_jobs::harvest_slot`]) or, with none, after the events'
/// roll, unless held: the harvest (bit 100h), its rate tallied (a skill
/// point each 80h), the spice counted in tenths, the place's spice taken
/// ([`troop_jobs::taken`]).
fn harvest(w: &mut World, t: u16, p: u16) -> Option<()> {
    day_update(w, t, p)?;
    let w10 = w.word(t + troop::WORD_10);
    match troop_jobs::harvest_slot(w10, w.time(), w.byte(t + troop::ID)) {
        Some(false) => {
            w.set_word(t + troop::RATE, 0);
            w.set_byte(t + troop::STATE, w.byte(t + troop::STATE) | 0x10);
            return Some(());
        }
        Some(true) => {
            w.set_word(t + troop::WORD_10, w10 & !0x200);
            if held_by_state(w, t, p) {
                return Some(());
            }
        }
        None => {
            if held_by_state(w, t, p) {
                w.set_word(t + troop::RATE, 0);
                w.set_word(t + troop::WORD_0E, 0);
                w.set_byte(t + troop::STATE, w.byte(t + troop::STATE) | 0x10);
                return Some(());
            }
            event_roll(w, t, p);
        }
    }
    w.set_word(t + troop::WORD_10, w.word(t + troop::WORD_10) | 0x100);
    let r = rate(w, t, p);
    let (total, crossed) = troop_jobs::tally(w.word(t + troop::WORD_0E), r);
    w.set_word(t + troop::WORD_0E, total);
    if crossed {
        add_skill(w, t, 0, 1);
    }
    let (whole, tenths) = troop_jobs::in_tenths(r, w.word(SPICE_TENTHS));
    w.set_word(SPICE_TENTHS, tenths);
    w.update::<Economy>(|e| e.spice = e.spice.wrapping_add(whole));
    let taken = troop_jobs::taken(r, w.byte(p + field::LEFT), w.byte(p + field::PER_SPICE), w.byte(p + place::SPICE))?;
    w.set_byte(p + field::LEFT, taken.left);
    w.set_byte(p + place::SPICE, taken.spice);
    Some(())
}

/// `loc_070cc`, job 1: spice prospecting, at a place not the Harkonnens':
/// not prospected yet, unless held, its rate set once
/// ([`troop_jobs::prospect_rate`]), the share done in hundredths
/// ([`troop_jobs::prospect_share`]) until found: 2 skill points, the
/// place prospected. Then the share 100, and unless the troop is the
/// selected one, held, or setting out (not modelled), event 0F0Eh.
fn prospecting(w: &mut World, t: u16, p: u16) -> Option<()> {
    let a = w.byte(p + place::FLAGS);
    if a & location_flag::HARKONNEN != 0 {
        return Some(());
    }
    if a & location_flag::PROSPECTED == 0 {
        if held_by_location(w, t, p) {
            return Some(());
        }
        let mut r = w.word(t + troop::RATE);
        if r == 0 {
            r = troop_jobs::prospect_rate(w.byte(p + field::PER_SPICE), w.byte(t + troop::MOTIVATION), w.byte(t + troop::SKILLS))?;
            w.set_word(t + troop::RATE, r);
        }
        let elapsed = w.time().wrapping_sub(w.word(t + troop::CHANGED_AT));
        if let Some(share) = troop_jobs::prospect_share(r, elapsed) {
            if elapsed != 0 {
                w.set_word(t + troop::WORD_0E, share);
            }
            return Some(());
        }
        add_skill(w, t, 0, 2);
        w.set_byte(p + place::FLAGS, w.byte(p + place::FLAGS) | location_flag::PROSPECTED);
    }
    let st = w.byte(t + troop::STATE);
    held_by_location(w, t, p);
    w.set_word(t + troop::WORD_0E, 0x64);
    if t == w.word(SELECTED) {
        return Some(());
    }
    if w.word(g::FOLLOWED_TROOP_PLACE) != 0 {
        return super::native_moves::set_out(w, t, p);
    }
    if st & 0x10 == 0 {
        add_pending_event(w, 0x0f0e, p);
    }
    Some(())
}

/// `loc_0727d`: the place's bit 04h cleared, and each troop of yours there
/// with bit 40h of word 12h loses it (`and word, 0BFh`: the immediate
/// sign-extended, FFBFh): 100h if any.
fn clear_location_4(w: &mut World, p: u16) -> Option<u16> {
    w.set_byte(p + place::FLAGS, w.byte(p + place::FLAGS) & !location_flag::BIT_4);
    let mut cx = 0;
    for x in your_troops(w, p)? {
        let v = w.word(x + troop::WORD_12);
        if v & 0x40 != 0 {
            w.set_word(x + troop::WORD_12, v & !0x40);
            cx = 0x100;
        }
    }
    Some(cx)
}

/// `loc_071ef`, job 4: military training. The troop's day; at a place
/// with bit 04h, an exercise counting down ([`troop_jobs::exercise`]),
/// over: the bit cleared for all; else the drill's countdown (word 0Ch)
/// down one, and at its end a new drill: the trainers' strength (0A0h at
/// the tick's own place, else the mean of the trainees' skill 1 there),
/// its countdown ([`troop_jobs::drill_countdown`]), a point of skill 1.
fn training(w: &mut World, t: u16, p: u16) -> Option<()> {
    day_update(w, t, p)?;
    if w.byte(p + place::FLAGS) & location_flag::BIT_4 != 0 {
        let (next, over) = troop_jobs::exercise(w.word(t + troop::WORD_0E), w.byte(t + troop::SKILLS + 1));
        w.set_word(t + troop::WORD_10, w.word(t + troop::WORD_10) & 0xfdff);
        if !over {
            w.set_word(t + troop::WORD_0E, next);
            return Some(());
        }
        let cx = clear_location_4(w, p)?;
        w.set_word(t + troop::WORD_12, w.word(t + troop::WORD_12) | cx);
        w.set_word(t + troop::WORD_0E, 0);
        return Some(());
    }
    let n = w.word(t + troop::RATE).wrapping_sub(1);
    w.set_word(t + troop::WORD_10, w.word(t + troop::WORD_10) & 0xfdff);
    w.set_word(t + troop::RATE, n);
    if n & 0x8000 == 0 {
        return Some(());
    }
    let ax = if p == w.word(g::TICK_LOCATION) {
        w.set_word(t + troop::WORD_10, w.word(t + troop::WORD_10) | 0x800);
        0xa0u16
    } else {
        let (mut cx, mut dx) = (0u16, 0u16);
        for x in your_troops(w, p)? {
            if w.byte(x + troop::STATE) == 4 {
                cx = cx.wrapping_add(1);
                dx = dx.wrapping_add(w.byte(x + troop::SKILLS + 1) as u16);
            }
        }
        if cx != 0 {
            let cl = cx as u8 as u16;
            if cl == 0 || dx / cl > 0xff {
                return super::world::nm("a division fault");
            }
            (dx % cl) << 8 | dx / cl
        } else {
            dx
        }
    };
    let gain = troop_jobs::drill_gain(ax as u8, w.byte(t + troop::SKILLS + 1));
    let m = motivation(w, t);
    w.set_word(t + troop::RATE, troop_jobs::drill_countdown(w.byte(t + troop::EQUIPMENT), gain, m));
    add_skill(w, t, 1, 1);
    Some(())
}

/// `loc_072b0`, job 5: espionage. Once a quarter of its time past
/// ([`troop_jobs::spy_times`]): its rate (the flagged troops' count when
/// 0: not modelled), past half the report (the Harkonnens' strength
/// there per its rate, `loc_033be`), then done (bit 40h of byte 10h). Unless the place's own
/// troop, a spy may be found out ([`troop_jobs::spy_found_out`]) and
/// started (not modelled when it is).
fn espionage(w: &mut World, t: u16, p: u16) -> Option<()> {
    let elapsed = w.time().wrapping_sub(w.word(t + troop::CHANGED_AT));
    let skill = w.byte(t + troop::SKILLS + 1);
    if w.byte(t + troop::BYTE_10) & 0x40 == 0 {
        let (half, quarter) = troop_jobs::spy_times(skill);
        if troop_jobs::past(quarter, elapsed) {
            if w.word(t + troop::RATE) == 0 {
                return super::world::nm("the flagged troops counted");
            }
            if !troop_jobs::past(half, elapsed) {
                return spy_found_out(w, t, p, elapsed, skill);
            }
            super::native_step::measure_forces(w, p)?;
            let rate = w.word(t + troop::RATE);
            let Some(report) = troop_jobs::spy_report(w.word(g::TOPIC_PLACE_HARKONNEN_STRENGTH), rate) else {
                return super::world::nm("a division fault");
            };
            w.set_word(t + troop::WORD_0E, report);
            w.set_byte(t + troop::BYTE_10, w.byte(t + troop::BYTE_10) | 0x40);
        }
    }
    spy_found_out(w, t, p, elapsed, skill)
}

/// `loc_072f7`: the spy, unless the place's own troop, found out on a
/// draw ([`troop_jobs::spy_found_out`]): started (not modelled).
fn spy_found_out(w: &mut World, t: u16, p: u16, elapsed: u16, skill: u8) -> Option<()> {
    if w.byte(t + troop::ID) == w.byte(p + place::FIRST_TROOP) {
        return Some(());
    }
    if troop_jobs::spy_found_out(skill, elapsed, || 0).is_none() {
        return Some(());
    }
    let r = w.rand();
    if troop_jobs::spy_found_out(skill, elapsed, || r) != Some(true) {
        return Some(());
    }
    super::native_raids::start_troop(w, t);
    Some(())
}

/// `loc_07693`, job 8: irrigation, unless held. A place not started is
/// started (spice 0, its centre, radius 4) and its water's spread drawn
/// (not modelled); else the troop's day, a step of skill 2 on byte 0Fh and
/// on its carry a point of skill 2 and the water's growth
/// ([`troop_jobs::irrigated`]): dried up (the troop marked) or grown (the
/// spread drawn: not modelled).
fn irrigation(w: &mut World, t: u16, p: u16) -> Option<()> {
    if w.byte(t + troop::STATE) & 0x10 != 0 || held_by_skill(w, t, p) {
        return Some(());
    }
    w.set_byte(g::TROOPS_TICK_RECHECK, 0xff);
    w.set_word(t + troop::WORD_10, w.word(t + troop::WORD_10) | 0x100);
    let a = w.byte(p + place::FLAGS);
    if a & location_flag::STARTED == 0 {
        w.set_byte(p + place::FLAGS, a | location_flag::STARTED);
        w.set_byte(p + place::SPICE, 0);
        w.set_word(p + field::WORD_0C, w.word(p + field::X));
        w.set_word(p + field::WORD_0E, w.word(p + field::Y) & 0xff);
        w.set_byte(p + place::RADIUS, 4);
        return super::world::nm("water spread on the map");
    }
    day_update(w, t, p)?;
    let step = troop_jobs::irrigation_step(w.byte(t + troop::SKILLS + 2));
    let f = w.byte(p + field::IRRIGATION);
    w.set_byte(p + field::IRRIGATION, f.wrapping_add(step));
    if f as u16 + step as u16 <= 0xff {
        return Some(());
    }
    add_skill(w, t, 2, 1);
    let b = w.byte(p + place::RADIUS);
    match troop_jobs::irrigated(w.byte(p + field::WATER), b, w.byte(p + field::BYTE_0E)) {
        troop_jobs::Irrigated::DriedUp => {
            w.set_byte(p + field::WATER, 0);
            w.set_byte(t + troop::STATE, w.byte(t + troop::STATE) | 0x10);
            Some(())
        }
        troop_jobs::Irrigated::Grows { water, radius, byte_0e, .. } => {
            w.set_byte(p + field::WATER, water);
            if radius != b {
                w.set_byte(p + place::RADIUS, radius);
            }
            if let Some(v) = byte_0e {
                w.set_byte(p + field::BYTE_0E, v);
            }
            super::world::nm("water spread on the map")
        }
    }
}

/// `loc_07711`, job 9: wind-trap assembly. Built already: the troop's
/// activity. Else a point of motivation, its strength added to byte 1Bh
/// ([`troop_jobs::assembled`]); on its carry the wind-trap built (flag
/// 20h, kind bit 8, water 5) and the troop's activity.
fn wind_trap(w: &mut World, t: u16, p: u16) -> Option<()> {
    if w.byte(p + place::FLAGS) & location_flag::WIND_TRAP != 0 {
        set_activity(w, t);
        return Some(());
    }
    add_motivation(w, t, 1);
    let s = strength(w, t);
    let (r, carry) = troop_jobs::assembled(w.byte(p + field::WATER), s);
    w.set_byte(p + field::WATER, r);
    if carry {
        w.set_byte(p + place::FLAGS, w.byte(p + place::FLAGS) | location_flag::WIND_TRAP);
        w.set_byte(p + place::KIND, w.byte(p + place::KIND) | 8);
        w.set_byte(p + field::WATER, troop_jobs::WIND_TRAP_START);
        set_activity(w, t);
    }
    Some(())
}

/// `loc_0767d`, job 0Ah: bulb growing. While byte 1Ah is 0, the counter
/// ([`troop_jobs::bulb_count`]); its wrap grows the bulbs (byte 1Ah 10h).
/// Grown, the troop goes back to irrigation.
fn bulbs(w: &mut World, t: u16, p: u16) -> Option<()> {
    if w.byte(p + field::BYTE_1A) == 0 {
        let (k, wraps) = troop_jobs::bulb_count(w.get::<Situation>().job10_counter);
        w.update::<Situation>(|s| s.job10_counter = k);
        if !wraps {
            return Some(());
        }
        w.set_byte(p + field::BYTE_1A, troop_jobs::BULBS_GROWN);
    }
    give_job_unless_same(w, t, troop_job::IRRIGATION);
    Some(())
}

/// For the checks of `loc_06ac5` (`job` None: the troop's activity) and
/// `loc_06acb` (the job given unless the same).
pub fn set_job_check(w: &mut World, t: u16, job: Option<u8>) {
    match job {
        None => set_activity(w, t),
        Some(j) => give_job_unless_same(w, t, j),
    }
}
