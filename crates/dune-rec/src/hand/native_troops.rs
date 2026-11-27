//! The troops natively (the native mode, Stage C of
//! `docs/readable-port-plan.md`): a troop's day, its motivation and rate,
//! its skills' decay, the events that befall it, as rules on a
//! [`World`]. The rules are the frames' own (`troop_day`, `troop_work`);
//! the walks are the original's (`loc_06e20`, `loc_06efd`, `loc_0708a`,
//! `loc_06d7b`, `loc_0714c` and what they call). `None`: a case not
//! modelled yet (the location's circle drawn into the map, a Harkonnen
//! troop reset), the world then partly changed.
//!
//! The display the original keeps on the way (the troops' particles on
//! the map, the redraw flags) is not state and is left out.

use super::game::{location_flag, troop_state};
use super::globals as g;
use super::native_day::{add_pending_event, your_troops};
use super::world::{place, troop, World, STORY_END};
use super::{player::PlayerAt, progress::Progress, situation::Situation, troop_day, troop_work};

/// The troop numbered `n`'s record.
pub fn troop_number(n: u8) -> u16 {
    troop::FIRST.wrapping_add((n.wrapping_sub(1) as u16) * troop::SIZE)
}

/// Every troop of the place `p`'s chain, by record, with whether it is
/// yours (byte 3 below 80h: `loc_06603`'s CF).
fn chain(w: &World, p: u16) -> Option<Vec<(u16, bool)>> {
    let mut out = Vec::new();
    let mut n = w.byte(p + place::FIRST_TROOP);
    while n != 0 {
        let t = troop_number(n);
        if t.wrapping_add(troop::SIZE) > STORY_END || out.len() > 0x100 {
            return super::world::nm("a walk past the tables");
        }
        out.push((t, w.byte(t + troop::STATE) < 0x80));
        n = w.byte(t + troop::NEXT);
    }
    Some(out)
}

/// `loc_07085`: the troop marked (state 10h).
pub fn mark(w: &mut World, t: u16) {
    w.set_byte(t + troop::STATE, w.byte(t + troop::STATE) | troop_state::MARKED);
}

/// `loc_06efd`: the troop's motivation for its work
/// ([`troop_work::effective_motivation`]).
pub fn motivation(w: &World, t: u16) -> u8 {
    let job = w.byte(t + troop::STATE) & 0x0f;
    let encouraged = w.get::<Situation>().troops_recheck != 0;
    let at_home = job == super::game::troop_job::ATTACKS && w.word(t + troop::LOCATION) == w.word(g::PLACE);
    troop_work::effective_motivation(w.byte(t + troop::MOTIVATION), job, encouraged, at_home, w.get::<Progress>().phase).value
}

/// `loc_0708a`: the troop's production rate at the place `p`
/// ([`troop_work::production_rate`]) and its trend in word 10h.
pub fn rate(w: &mut World, t: u16, p: u16) -> u16 {
    let m = motivation(w, t);
    let armed = w.byte(t + troop::EQUIPMENT) & 0x80 != 0;
    let (rate, _) = troop_work::production_rate(m, w.byte(t + troop::SKILLS), w.byte(t + troop::SIZE_FIELD), armed, w.byte(p + place::SPICE));
    let old = w.word(t + troop::RATE);
    w.set_word(t + troop::RATE, rate);
    if let Some(way) = troop_work::rate_trend(old, rate) {
        let v = w.word(t + troop::WORD_10);
        w.set_word(t + troop::WORD_10, v & 0xfff3 | way as u16);
    }
    rate
}

/// `loc_06edd`: skill `k` up by `gain` (at most 5Fh); a step of 10h noted
/// in word 10h's low two bits (the skill's number from 1).
pub fn add_skill(w: &mut World, t: u16, k: u16, gain: u8) {
    let old = w.byte(t + troop::SKILLS + k);
    let new = old.wrapping_add(gain).min(0x5f);
    w.set_byte(t + troop::SKILLS + k, new);
    if (new ^ old) & 0xf0 != 0 {
        let v = w.word(t + troop::WORD_10);
        let lo = (v as u8 & 0xfc) | (k.wrapping_add(1) as u8);
        w.set_word(t + troop::WORD_10, v & 0xff00 | lo as u16);
    }
}

/// `decrease_troop_motivation` (6F93h): motivation down by `by` (not
/// below 0); under 5 it is 4, the troop marked and bit 20h of word 12h set.
pub fn lower_motivation(w: &mut World, t: u16, by: u8) {
    let old = w.byte(t + troop::MOTIVATION);
    let v = old.saturating_sub(by);
    w.set_byte(t + troop::MOTIVATION, v);
    if v < 5 {
        w.set_byte(t + troop::MOTIVATION, 4);
        mark(w, t);
        w.set_word(t + troop::WORD_12, w.word(t + troop::WORD_12) | 0x20);
    }
}

/// `loc_06d7b`: every 40h ticks, the skills of the troop's activity (its
/// job / 4) whose bits of word 12h are set lose a point.
pub fn skills_decay(w: &mut World, t: u16) {
    if w.time() & 0x3f != 0 {
        return;
    }
    let activity = (w.byte(t + troop::STATE) & 0x0f) >> 2;
    let decays = 0xc000u16.rotate_left(activity as u32 & 0x0f) & w.word(t + troop::WORD_12);
    for (i, k) in [2u16, 1, 0].into_iter().enumerate() {
        if decays & (0x8000 >> i) != 0 {
            let v = w.byte(t + troop::SKILLS + k);
            if v != 0 {
                w.set_byte(t + troop::SKILLS + k, v - 1);
            }
        }
    }
}

/// `loc_06b96`: whether the troop is held (bit 200h of word 10h, bits 30h
/// of word 12h, no spice at the place `p`, or the place not prospected or
/// started: flags 40h clear or 01h set); bit 10h of its state made so
/// (`loc_06bb6`); released, it is stamped (`loc_06b25`).
pub fn held_by_state(w: &mut World, t: u16, p: u16) -> bool {
    let free = w.word(t + troop::WORD_10) & 0x200 == 0
        && w.word(t + troop::WORD_12) & 0x30 == 0
        && w.byte(p + place::SPICE) >= 1
        && (w.byte(p + place::FLAGS) ^ 0x40) & 0x41 == 0;
    set_held(w, t, !free);
    !free
}

/// `loc_06bb6`: bit 10h of the troop's state set to `held`; a troop
/// released is stamped (`loc_06b25`).
pub fn set_held(w: &mut World, t: u16, held: bool) {
    let s = w.byte(t + troop::STATE);
    if (s & 0x10 != 0) == held {
        return;
    }
    w.set_byte(t + troop::STATE, s ^ 0x10);
    if s & 0x10 != 0 {
        stamp(w, t);
    }
}

/// `loc_06b25`: the record stamped with the time (word 0Ah), words 0Ch and
/// 0Eh cleared.
pub fn stamp(w: &mut World, t: u16) {
    w.set_word(t + troop::CHANGED_AT, w.time());
    w.set_word(t + troop::RATE, 0);
    w.set_word(t + troop::WORD_0E, 0);
}

/// `loc_071bc`: from phase 35h, a troop with bit 40h of word 12h, one
/// time in eight (the random bits [0] rotated by 3): marked (bit 200h of
/// word 10h, then 8000h), the place's bit 04h, event 0F03h there.
fn maybe_mark(w: &mut World, t: u16, p: u16) {
    if w.get::<Progress>().phase < 0x35 || w.word(t + troop::WORD_12) & 0x40 == 0 {
        return;
    }
    let seed = w.word(g::RANDOM_BITS).rotate_left(3);
    w.set_word(g::RANDOM_BITS, seed);
    if seed & 7 != 0 {
        return;
    }
    w.set_word(t + troop::WORD_10, w.word(t + troop::WORD_10) | 0x200);
    mark(w, t);
    w.set_word(t + troop::WORD_10, w.word(t + troop::WORD_10) | 0x8000);
    w.set_byte(p + place::FLAGS, w.byte(p + place::FLAGS) | location_flag::BIT_4);
    add_pending_event(w, 0x0f03, p);
}

/// `loc_0714c`: with events on, an equipped troop (byte 19h bit 80h), in
/// its slot of the time (its number's low nibble): [`maybe_mark`], then a
/// random roll against the place's chance ([1141h] by its name): befallen
/// (bit 4000h); unless immune (bit 40h): 1 losses (2000h, its size down
/// 2 above 2), 2 stopped (200h, marked), 3 its equipment lost (1000h, the
/// place's stock down one, event 0F06h).
pub fn event_roll(w: &mut World, t: u16, p: u16) {
    if w.get::<Progress>().event_flags & 1 == 0 || w.byte(t + troop::EQUIPMENT) & 0x80 == 0 {
        return;
    }
    if w.time() as u8 & 0x0f != w.byte(t + troop::ID) & 0x0f {
        return;
    }
    maybe_mark(w, t, p);
    let roll = (w.rand() >> 8) as u8;
    let chance = w.byte(g::CHANCES.wrapping_add(w.byte(p + place::NAME_FIRST) as u16));
    if chance < roll {
        return;
    }
    let bits = w.byte(t + troop::EQUIPMENT);
    let v = w.word(t + troop::WORD_10) | 0x4000;
    w.set_word(t + troop::WORD_10, v);
    if bits & 0x40 != 0 {
        return;
    }
    match roll & 3 {
        0 => {}
        1 => {
            w.set_word(t + troop::WORD_10, v | 0x2000);
            let size = w.byte(t + troop::SIZE_FIELD);
            if size > 2 {
                w.set_byte(t + troop::SIZE_FIELD, size - 2);
            }
        }
        2 => {
            w.set_word(t + troop::WORD_10, v | 0x200);
            mark(w, t);
        }
        _ => {
            w.set_word(t + troop::WORD_10, v | 0x1000);
            w.set_byte(t + troop::EQUIPMENT, bits & !0x80);
            w.set_byte(p + place::EQUIPMENT, w.byte(p + place::EQUIPMENT).wrapping_sub(1));
            add_pending_event(w, 0x0f06, p);
        }
    }
}

/// `loc_06cfc`: a place below kind 20h, radius below 0Ch and not started
/// grows: its radius one more and its circle drawn into the map
/// (`loc_0644e`, state 20h).
fn grow_location(w: &mut World, p: u16) -> Option<()> {
    let r = w.byte(p + place::RADIUS);
    if w.byte(p + place::KIND) >= 0x20 || r >= 0x0c || w.byte(p + place::FLAGS) & location_flag::STARTED != 0 {
        return Some(());
    }
    w.set_byte(p + place::RADIUS, r + 1);
    circle(w, p, r as u16 + 1, 0x20)
}

/// What a circle's spans do to the cells (the handler at CS:64EDh).
#[derive(Clone, Copy)]
pub enum Span {
    /// `loc_0646f`: each cell not spreading (state 10h) made this state.
    Colour(u8),
    /// `loc_0653a`: a moving record's trail (the irrigation's water).
    Trail,
}

/// `loc_06447`'s circle of radius `radius` around the place `p`'s map
/// point, in `colour` (bits 4..5).
pub fn circle(w: &mut World, p: u16, radius: u16, colour: u8) -> Option<()> {
    let (x, y) = (w.word(p + 2), w.word(p + 4));
    walk(w, radius, x, y, Span::Colour(colour))
}

/// The midpoint walk at `loc_064b2` (`map::circle_sim`) of radius
/// `radius` around the map point (`x`, row `y`): each of its stops two
/// spans (`loc_064ef`: the rows BX and SI, from DI, DX - DI + 1 cells).
pub fn walk(w: &mut World, radius: u16, x: u16, y: u16, span: Span) -> Option<()> {
    let mut r = [0, 0, radius, 0, 0, 0, 0];
    let mut label = 0x64b2;
    loop {
        let Ok((_, end, out, _)) = super::map::circle_sim(label, r) else {
            return super::world::nm("a circle's walk too long");
        };
        let [_, bx, _, dx, si, di, _] = out;
        let Some(back) = end else {
            return Some(());
        };
        let count = dx.wrapping_sub(di).wrapping_add(1);
        circle_span(w, bx, di, count, x, y, span)?;
        circle_span(w, si, di, count, x, y, span)?;
        r = out;
        label = back;
    }
}

/// A span handler: row `row` of the circle (from its centre's row `y`;
/// rows past 5Dh, for a trail 56h, either way left out): the map row's
/// cells (`loc_0b58b`: the latitude table's row, its length BP twice its
/// scale, the centre `x` scaled into it) from `start` on, `count` of them
/// wrapping around the row.
fn circle_span(w: &mut World, row: u16, start: u16, count: u16, x: u16, y: u16, span: Span) -> Option<()> {
    let limit: i16 = if matches!(span, Span::Trail) { 0x56 } else { 0x5d };
    let row = row.wrapping_add(y);
    if (row as i16) > limit || (row as i16) < -limit {
        return Some(());
    }
    if count == 0 {
        return super::world::nm("a circle's span of 0 cells");
    }
    let off = row.wrapping_mul(8);
    let neg = off & 0x8000 != 0;
    let entry = if neg { off.wrapping_neg() } else { off };
    let value = w.word(g::LATITUDE_TABLE.wrapping_add(entry));
    let scale = w.word(g::LATITUDE_TABLE.wrapping_add(entry).wrapping_add(2));
    let ax = if neg { value.wrapping_neg() } else { value };
    let bp = scale.wrapping_shl(1);
    let pr = bp as u32 * x as u32;
    let hi = ((pr >> 16) as u16).wrapping_add((pr as u16) >> 15);
    let mut di = w.word(0xdcfe).wrapping_add(ax).wrapping_add(hi).wrapping_add(start);
    let mut dx = hi.wrapping_add(start);
    let mut guard = 0;
    while dx & 0x8000 != 0 {
        di = di.wrapping_add(bp);
        dx = dx.wrapping_add(bp);
        guard += 1;
        if guard > 10_000 {
            return super::world::nm("a circle's span past its row");
        }
    }
    for _ in 0..count {
        let old = w.cell(di);
        let bits = old & 0x30;
        match span {
            Span::Colour(colour) => {
                if bits != 0x10 {
                    w.set_cell(di, (old ^ bits) | colour);
                }
            }
            Span::Trail => {
                if bits != 0x10 {
                    if old & 0x40 != 0 {
                        trail_place(w, di)?;
                    }
                    let v = trail_cell(w, old);
                    w.set_cell(di, v);
                }
            }
        }
        di = di.wrapping_add(1);
        dx = dx.wrapping_add(1);
        if dx >= bp {
            dx = dx.wrapping_sub(bp);
            di = di.wrapping_sub(bp);
        }
    }
    Some(())
}

/// The trail's new byte for a cell `b` (`loc_0653a`'s recolour): bits
/// ECFh of the byte twice kept, state 20h; for a terrain (bits 1..3)
/// below 8, the trail's pattern ([4736h]) rotated, its carry making it
/// state 10h.
fn trail_cell(w: &mut World, b: u8) -> u8 {
    let ax = ((b as u16) << 8 | b as u16) & 0x0ecf;
    let mut al = (ax as u8) | 0x20;
    if ((ax >> 8) as u8) < 8 {
        let v = w.byte(g::MOVE_TRAIL_PATTERN);
        w.set_byte(g::MOVE_TRAIL_PATTERN, v.rotate_left(1));
        if v & 0x80 != 0 {
            al = (al & 0xcf) | 0x10;
        }
    }
    al
}

/// A trail over a place's marker (bit 40h): the place whose cell (word
/// 6) it is (`loc_0409a`) has its spice taken (byte 12h 0); not skipped
/// and from 138h on, it loses bit 80h and is raided (`loc_07443`).
fn trail_place(w: &mut World, cell: u16) -> Option<()> {
    let mut si = place::FIRST;
    loop {
        if si + place::SIZE > STORY_END {
            return super::world::nm("a walk past the tables");
        }
        if w.word(si) == 0xffff {
            return Some(());
        }
        if w.word(si + 6) == cell {
            break;
        }
        si += place::SIZE;
    }
    w.set_byte(si + place::SPICE, 0);
    if super::native_day::skipped(w, si) || si < 0x138 {
        return Some(());
    }
    w.set_byte(si + place::FLAGS, w.byte(si + place::FLAGS) & 0x7f);
    super::native_raids::raided(w, si)
}

/// `loc_06dbb`: the people at the place `p` (its number in the high byte,
/// 80h in the low: `loc_040ae`) take its kind as their room (high byte),
/// room 2 unless 1; the player too (the room and the place's kind); each
/// of its troops with bit 20h of word 10h moves it to 1000h of word 12h;
/// its Harkonnen troops are lost (not modelled yet: `None` if any).
fn set_people_state(w: &mut World, p: u16) -> Option<()> {
    let off = p.wrapping_sub(place::FIRST);
    if off / place::SIZE > 0xff {
        return super::world::nm("a division fault");
    }
    let bx = ((off / place::SIZE + 1) as u8 as u16) << 8 | 0x80;
    let dh = w.byte(p + place::KIND) as u16;
    for k in 0..0xcu16 {
        let s = g::CHARACTERS + 0x10 * k;
        if w.word(s + 2) == bx {
            let v = w.word(s);
            w.set_word(s, dh << 8 | if v & 0xff == 1 { 1 } else { 2 });
        }
    }
    let pl = w.get::<PlayerAt>();
    if pl.y == bx {
        let ax = dh << 8 | if pl.x & 0xff == 1 { 1 } else { 2 };
        w.update::<PlayerAt>(|t| t.x = ax);
        w.update::<Progress>(|t| {
            t.room = ax as u8;
            t.place_kind = (ax >> 8) as u8;
        });
    }
    for (t, _) in chain(w, p)? {
        let v = w.word(t + troop::WORD_10);
        if v & 0x20 != 0 {
            w.set_word(t + troop::WORD_10, v & !0x20);
            w.set_word(t + troop::WORD_12, w.word(t + troop::WORD_12) | 0x1000);
        }
    }
    super::native_raids::lose_all_harkonnen(w, p)
}

/// `loc_06e82`'s mark for a troop of yours at the place `p`: idle
/// elsewhere (not the player's place, below kind 21h, motivation below
/// 28h, no job bits 2Fh): 2 with bit 80h of byte 12h, else 1.
fn idle_elsewhere(w: &World, t: u16, p: u16) -> u8 {
    if p == w.word(g::PLACE) || w.byte(p + place::KIND) >= 0x21 || w.byte(t + troop::MOTIVATION) >= 0x28 || w.byte(t + troop::STATE) & 0x2f != 0 {
        return 0;
    }
    if w.byte(t + troop::BYTE_12) & 0x80 != 0 { 2 } else { 1 }
}

/// `loc_06ea8`: a troop of yours idle (motivation below 28h, no job bits
/// 2Bh) is marked, with bit 10h of byte 12h.
fn mark_idle(w: &mut World, t: u16) {
    if w.byte(t + troop::MOTIVATION) >= 0x28 || w.byte(t + troop::STATE) & 0x2b != 0 {
        return;
    }
    mark(w, t);
    w.set_byte(t + troop::BYTE_12, w.byte(t + troop::BYTE_12) | 0x10);
}

/// `loc_06e20`: on a new day, for the troop `t` at the place `p`: a
/// raided place whose raid is over is freed ([`troop_day::freed`], one
/// more sietch held, its people, radius 5); the place grows; a troop
/// restless after 8 days loses a point of motivation; and the place's
/// first troop, when 3 of yours there are idle elsewhere
/// ([`troop_day::idle_report`]), has them marked and event 302h told.
pub fn day_update(w: &mut World, t: u16, p: u16) -> Option<()> {
    if w.byte(g::DAY_CHANGED) == 0 {
        return Some(());
    }
    let flags = w.byte(p + place::FLAGS);
    if flags & location_flag::RAIDED != 0 {
        let day = (w.time() >> 4) as u8;
        if !troop_day::raid_over(day, w.byte(p + place::RADIUS)) {
            return Some(());
        }
        let (f, k) = troop_day::freed(flags, w.byte(p + place::KIND));
        w.set_byte(p + place::FLAGS, f);
        w.set_byte(p + place::KIND, k);
        w.set_byte(g::SIETCHES_HELD, w.byte(g::SIETCHES_HELD).wrapping_add(1));
        set_people_state(w, p)?;
        w.set_byte(p + place::RADIUS, troop_day::FREED_RADIUS);
    }
    grow_location(w, p)?;
    let day = (w.time() >> 4) as u8;
    if troop_day::restless(day, w.byte(t + troop::DAY)) {
        lower_motivation(w, t, 1);
    }
    if w.byte(p + place::FIRST_TROOP) != w.byte(t + troop::ID) {
        return Some(());
    }
    let yours = your_troops(w, p)?;
    let idle = yours.iter().fold(0u8, |dl, &x| dl | idle_elsewhere(w, x, p));
    if !troop_day::idle_report(idle) {
        return Some(());
    }
    for x in yours {
        mark_idle(w, x);
    }
    add_pending_event(w, 0x302, p);
    Some(())
}
