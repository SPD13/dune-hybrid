//! The time step's other pieces natively (the native mode, Stage C of
//! `docs/readable-port-plan.md`): what `loc_01b23` runs around the new
//! day and the troops' tick, as rules on a [`World`]: the character 7
//! check (`loc_01d9f`), the time-of-day handlers (CS:1DB3h: the Emperor's
//! demand `loc_020a4`, the Harkonnens' take `loc_01f64`, the shipment's
//! event `loc_01dda`, the Harkonnens' growth `loc_01dfe`). `None`: a case
//! not modelled yet, the world then partly changed.

use super::globals as g;
use super::native_day::{add_pending_event, places};
use super::native_troops::troop_number;
use super::world::{nm, place, troop, World, STORY_END};
use super::{economy::Economy, harkonnen, progress::Progress, situation::Situation};

/// The troops of the place `p`'s chain, by record (all of them).
fn chain(w: &World, p: u16) -> Option<Vec<u16>> {
    let mut out = Vec::new();
    let mut n = w.byte(p + place::FIRST_TROOP);
    while n != 0 {
        let t = troop_number(n);
        if t + troop::SIZE > STORY_END || out.len() > 0x100 {
            return nm("a walk past the tables");
        }
        out.push(t);
        n = w.byte(t + troop::NEXT);
    }
    Some(out)
}

/// `loc_01e24`: whether a troop of the place's chain has bit 400h of
/// word 12h (marked for the attack).
fn marked_troop_at(w: &World, p: u16) -> Option<bool> {
    Some(chain(w, p)?.iter().any(|&t| w.word(t + troop::WORD_12) & 0x400 != 0))
}

/// `loc_01d9f`: unless the party has bit 80h, character 7 (the record at
/// 1048h, kind 7 at byte 0Eh) at a place in phase 5Dh (`loc_01e01`: its
/// byte 0 then 2) with a marked troop there ticks the attack's clock
/// (`loc_01eda`: 8 more; wrapping, the troops there unmarked and noted
/// (bit 800h), event 709h, one attack less pending, the next place with a
/// marked troop the target; none: phase 60h, not modelled).
pub fn character_7_check(w: &mut World) -> Option<()> {
    if w.get::<Progress>().present & 0x80 != 0 {
        return Some(());
    }
    const RECORD: u16 = 0x1048;
    if w.get::<Progress>().phase != 0x5d || w.byte(RECORD + 0x0e) != 7 {
        return Some(());
    }
    let w2 = w.word(RECORD + 2);
    if w2 & 0xff != 0x80 {
        return Some(());
    }
    w.set_byte(RECORD, 2);
    let p = ((w2 >> 8) as u16 * place::SIZE).wrapping_add(place::FIRST - place::SIZE);
    if p + place::SIZE > STORY_END {
        return nm("a walk past the tables");
    }
    if !marked_troop_at(w, p)? {
        return Some(());
    }
    let tick = w.get::<Situation>().attack_clock.wrapping_add(8);
    w.update::<Situation>(|s| s.attack_clock = tick);
    if tick != 0 {
        return Some(());
    }
    for t in chain(w, p)? {
        let v = w.word(t + troop::WORD_12);
        w.set_word(t + troop::WORD_12, v & !0x400 | 0x800);
    }
    add_pending_event(w, 0x709, p);
    w.update::<Situation>(|s| s.attacks_pending = s.attacks_pending.wrapping_sub(1));
    let mut found = 0;
    for q in places(w)? {
        if marked_troop_at(w, q)? {
            found = q;
            break;
        }
    }
    w.set_word(g::ATTACK_TARGET, found);
    if found == 0 {
        return nm("phase 60h");
    }
    Some(())
}

/// The time of day's handler (CS:1DB3h by the time's low nibble): 3 the
/// Emperor's demand, 4 the Harkonnens' take, 8 the shipment's event, 0Fh
/// the Harkonnens' growth.
pub fn time_of_day(w: &mut World) -> Option<()> {
    match w.time() & 0x0f {
        3 => demand(w),
        4 => harkonnen_take(w),
        8 => shipment_event(w),
        0x0f => grow_harkonnen_troops(w),
        _ => Some(()),
    }
}

/// `loc_020a4`: with a demand going (bit 80h of the shipment's flags), the
/// days since it was made ([118Dh]) counted down into [0CFh]; its day
/// come, the next demand (not modelled, nor the calls it starts).
fn demand(w: &mut World) -> Option<()> {
    let flags = w.get::<Economy>().shipment_flags;
    if flags & 0x80 == 0 {
        return Some(());
    }
    let day = w.time() >> 4;
    let since = day.wrapping_sub(w.word(0x118d));
    if w.get::<Economy>().endgame_stage != 0 || (flags & 0x10 == 0 && w.byte(0x11bb) == 0) {
        if since == 0 {
            return if w.get::<Economy>().endgame_stage != 0 { Some(()) } else { nm("the Emperor's next demand") };
        }
        w.update::<Situation>(|s| s.shipment_days_left = since.wrapping_neg() as u8);
        return Some(());
    }
    nm("the Emperor's call")
}

/// `loc_01dd7` (`loc_01f64`): the Harkonnens' take of a sietch, once their
/// wait is over ([`harkonnen::take_window`]), not on a quiet day, nor
/// when skipped once ([11BCh]), on the random bits' draw
/// ([`harkonnen::take_draw`]); taken: not modelled.
pub fn harkonnen_take(w: &mut World) -> Option<()> {
    let now = w.time();
    let window = harkonnen::take_window(w.get::<Progress>().phase, now, w.word(g::WORD_1154));
    if !window.open() || harkonnen::quiet_day(now) {
        return Some(());
    }
    if w.byte(g::SKIP_HARKONNEN_TAKE) != 0 {
        w.set_byte(g::SKIP_HARKONNEN_TAKE, 0);
        return Some(());
    }
    let (carry, r) = harkonnen::take_draw(w.word(g::RANDOM_BITS));
    w.set_byte(g::SKIP_HARKONNEN_TAKE, 0);
    w.set_word(g::RANDOM_BITS, r);
    if carry {
        return nm("the Harkonnens take a sietch");
    }
    Some(())
}

/// `loc_01dda`: the shipment's event (30Bh) when one is due (bit 10h of
/// its flags), the party without bit 8, not in room 8, the endgame not
/// begun.
pub fn shipment_event(w: &mut World) -> Option<()> {
    let e = w.get::<Economy>();
    let p = w.get::<Progress>();
    if e.shipment_flags & 0x10 == 0 || p.party & 8 != 0 || p.room == 8 || e.endgame_stage != 0 {
        return Some(());
    }
    add_pending_event(w, 0x030b, 0);
    Some(())
}

/// `loc_01dfe`: on the random bits' top bit (rotated), each Harkonnen
/// troop (byte 10h bit 80h) of the table below 0FA0h grows by one man, up
/// to C8h.
pub fn grow_harkonnen_troops(w: &mut World) -> Option<()> {
    let bits = w.word(g::RANDOM_BITS);
    w.set_word(g::RANDOM_BITS, bits.rotate_left(1));
    if bits & 0x8000 == 0 {
        return Some(());
    }
    let mut t = troop::FIRST;
    loop {
        if w.byte(t + troop::BYTE_10) & 0x80 != 0 {
            let v = w.byte(t + troop::SIZE_FIELD);
            if v.wrapping_sub(1) < 0xc7 {
                w.set_byte(t + troop::SIZE_FIELD, v + 1);
            }
        }
        t += troop::SIZE;
        if t >= 0x0fa0 {
            return Some(());
        }
    }
}

/// `loc_0342d`: a troop's yield in a fight: twice its motivation plus
/// skill 1 (saturating), times its men, over 10h; for each of its
/// equipment bits 20h..04h, the step (doubling each time) added (capped
/// at FFFFh); over 100h, at least 1 with any men.
fn troop_yield(w: &World, t: u16) -> u16 {
    let m = super::native_troops::motivation(w, t);
    let size = w.byte(t + troop::SIZE_FIELD);
    let bits = w.byte(t + troop::EQUIPMENT);
    let (sum, over) = m.wrapping_add(m).overflowing_add(w.byte(t + troop::SKILLS + 1));
    let base = (if over { 0xff } else { sum } as u16 * size as u16) >> 4;
    let (mut total, mut step) = (base, base << 1);
    for k in 0..4 {
        if k > 0 {
            step <<= 1;
        }
        if bits & (0x20 >> k) != 0 {
            match total.checked_add(step) {
                Some(v) => total = v,
                None => {
                    total = 0xffff;
                    break;
                }
            }
        }
    }
    let r = total >> 8;
    if r != 0 { r } else { (size >= 1) as u16 }
}

/// The troops `loc_06639` walks for the place `p`: its chain, then every
/// troop of the table travelling there (to its location, or its
/// destination when moving on: state bits 3).
fn troops_for(w: &World, p: u16) -> Option<Vec<u16>> {
    let mut out = chain(w, p)?;
    let mut t = troop::FIRST;
    while t < g::TROOPS_END {
        let s = w.byte(t + troop::STATE);
        if s & 0x40 != 0 {
            let to = if s & 3 == 3 { w.word(t + 0x0c) } else { w.word(t + troop::LOCATION) };
            if to == p {
                out.push(t);
            }
        }
        t += troop::SIZE;
    }
    Some(out)
}

/// `loc_033be`: the place's forces: each troop of its chain not started
/// adds its yield to the Harkonnens' strength or to yours (yours: their
/// words 10h and 12h ORed into the place's), then the strike odds
/// (`loc_033d9`).
fn measure_forces(w: &mut World, p: u16) -> Option<()> {
    for at in [0x94u16, 0x96, 0x5c, 0x5e] {
        w.set_word(at, 0);
    }
    for t in chain(w, p)? {
        if w.byte(t + troop::STATE) & 0x20 != 0 {
            continue;
        }
        let amount = troop_yield(w, t);
        if w.byte(t + troop::BYTE_10) & 0x80 != 0 {
            w.set_word(g::TOPIC_PLACE_HARKONNEN_STRENGTH, w.word(g::TOPIC_PLACE_HARKONNEN_STRENGTH).wrapping_add(amount));
        } else {
            w.set_word(g::TOPIC_PLACE_OUR_STRENGTH, w.word(g::TOPIC_PLACE_OUR_STRENGTH).wrapping_add(amount));
            w.set_word(g::TOPIC_PLACE_TROOP_BITS_10, w.word(g::TOPIC_PLACE_TROOP_BITS_10) | w.word(t + troop::WORD_10));
            w.set_word(g::TOPIC_PLACE_TROOP_BITS_12, w.word(g::TOPIC_PLACE_TROOP_BITS_12) | w.word(t + troop::WORD_12));
        }
    }
    let (a, d) = (w.word(g::TOPIC_PLACE_OUR_STRENGTH), w.word(g::TOPIC_PLACE_HARKONNEN_STRENGTH));
    let below = a < d;
    let (big, small) = if below { (d, a) } else { (a, d) };
    let ratio = if small == 0 || big >> 8 >= small { 0xfc } else { ((((big as u32) << 8) / small as u32) as u16 >> 1).min(0xfc) };
    w.set_byte(g::TOPIC_PLACE_STRIKE_ODDS, if below { (ratio as u8).wrapping_neg() } else { ratio as u8 });
    Some(())
}

fn bump(w: &mut World, at: u16) {
    w.set_byte(at, w.byte(at).wrapping_add(1));
}

/// `loc_034a5`: the place's troops counted (60h..92h cleared first) by
/// `loc_034d0` for each troop `loc_06639` walks: idle ones, by side,
/// travelling or there, by job, and yours by kind; then the totals.
fn count_troops(w: &mut World, p: u16) -> Option<()> {
    for k in 0..0x33 {
        w.set_byte(g::TOPIC_PLACE_OUR_TROOPS + k, 0);
    }
    for t in troops_for(w, p)? {
        let state = w.byte(t + troop::STATE);
        if state & 0x20 != 0 {
            continue;
        }
        let base = if state & 0x40 != 0 { g::TOPIC_PLACE_HARKONNEN_INBOUND } else { g::TOPIC_PLACE_HARKONNEN_TROOPS };
        let harkonnen = w.byte(t + troop::BYTE_10) & 0x80 != 0;
        if !harkonnen && state == 0x80 {
            bump(w, g::TOPIC_PLACE_IDLE);
            continue;
        }
        let job = if state & 3 == 3 { state & 0x0f & !3 } else { state & 0x0f };
        let at = base + job as u16;
        bump(w, if harkonnen { base } else { base - 1 });
        bump(w, at + 1);
        if at < g::TOPIC_PLACE_HARKONNEN_INBOUND {
            bump(w, g::TOPIC_PLACE_TROOP_KINDS + (w.word(t + troop::WORD_12) & 0x0f));
        }
    }
    for (a, b, to) in [
        (g::TOPIC_PLACE_OUR_TROOPS, g::TOPIC_PLACE_OUR_INBOUND, g::TOPIC_PLACE_OUR_TOTAL),
        (g::TOPIC_PLACE_HARKONNEN_TROOPS, g::TOPIC_PLACE_HARKONNEN_INBOUND, g::TOPIC_PLACE_HARKONNEN_TOTAL),
    ] {
        w.set_byte(to, w.byte(a).wrapping_add(w.byte(b)));
    }
    Some(())
}

/// `loc_05133` / `loc_0514e`: the direction from (`x0`, `y0`) to (`x1`,
/// `y1`), 0..FFh (the differences halved when the rows are 80h or more
/// apart; the rows' difference in the high byte, by `imul` / `idiv`).
fn direction(x0: u16, y0: u16, x1: u16, y1: u16) -> u8 {
    let by = y1.wrapping_sub(y0);
    let half = (by as i16) < -0x80 || (by as i16) >= 0x80;
    let (mut b, mut d) = (by, x1.wrapping_sub(x0));
    if half {
        b = ((b as i16) >> 1) as u16;
        d = ((d as i16) >> 1) as u16;
    }
    let (bx, dx) = ((b & 0xff) << 8, d);
    let (nb, nd) = (bx & 0x8000 != 0, dx & 0x8000 != 0);
    let ax = if nb { bx.wrapping_neg() } else { bx };
    let cx = if nd { dx.wrapping_neg() } else { dx };
    if cx >= ax {
        if cx < 1 {
            return ax as u8;
        }
        let q = (32 * bx as i16 as i32) / dx as i16 as i32;
        (q as u8).wrapping_add(if nd { 0xc0 } else { 0x40 })
    } else {
        let q = (32 * dx as i16 as i32) / bx as i16 as i32;
        let al = q as u8;
        (if nb { al } else { al.wrapping_sub(0x80) }).wrapping_neg()
    }
}

/// `loc_05323`: the octant (0..7) of the place in the slot whose record
/// is at `bp`, seen from (`x`, `y`), into [`bp` + 2].
fn octant(w: &mut World, bp: u16, x: u16, y: u16) -> u16 {
    let rec = w.word(bp);
    let (x1, y1) = (w.word(rec.wrapping_add(2)), w.word(rec.wrapping_add(4)));
    let v = (direction(x, y, x1, y1).wrapping_add(0x10).rotate_left(3) & 7) as u16;
    w.set_byte(bp + 2, v as u8);
    v
}

/// `loc_05274`: the places nearest to the place `p` (the larger of the x
/// distance's high byte and the y distance), into the five slots (the
/// nearest of all; low or high kinds, mapped or not: an undiscovered low
/// one before its phase is left out), each with its octant; the
/// direction's phrase from the low unmapped one's.
fn nearest_places(w: &mut World, p: u16) -> Option<()> {
    let (dx, bx) = (w.word(p + 2), w.word(p + 4));
    for a in [g::NEAREST_PLACE, g::NEAREST_LOW_MAPPED, g::NEAREST_LOW_UNMAPPED, g::NEAREST_HIGH_MAPPED, g::NEAREST_HIGH_UNMAPPED] {
        w.set_word(a, 0xffff);
    }
    let phase = w.get::<Progress>().phase;
    let mut si = place::FIRST;
    loop {
        if si + place::SIZE > STORY_END {
            return nm("a walk past the tables");
        }
        if w.word(si) == 0xffff {
            break;
        }
        if si != p {
            let (x, y) = (w.word(si + 2).wrapping_sub(dx), w.word(si + 4).wrapping_sub(bx));
            let xa = if x & 0x8000 != 0 { x.wrapping_neg() } else { x };
            let ya = if y & 0x8000 != 0 { y.wrapping_neg() } else { y };
            let cl = (xa >> 8) as u8;
            let d = if cl < ya as u8 { ya } else { cl as u16 };
            let (kind, f) = (w.byte(si + place::KIND), w.byte(si + place::FLAGS));
            let undiscovered = f & super::game::location_flag::UNDISCOVERED != 0;
            let slot = if kind >= 0x28 {
                Some(if undiscovered { g::NEAREST_HIGH_UNMAPPED } else { g::NEAREST_HIGH_MAPPED })
            } else if !undiscovered {
                Some(g::NEAREST_LOW_MAPPED)
            } else if phase < w.byte(si + place::RADIUS) {
                None
            } else {
                Some(g::NEAREST_LOW_UNMAPPED)
            };
            if let Some(bp) = slot {
                if d < w.word(bp) {
                    w.set_word(bp, d);
                    w.set_word(bp + 2, si);
                }
                if d < w.word(g::NEAREST_PLACE) {
                    w.set_word(g::NEAREST_PLACE, d);
                    w.set_word(g::NEAREST_PLACE + 2, si);
                }
            }
        }
        si += place::SIZE;
    }
    let mut ax = 0;
    for bp in [0xdeu16, 0xe4, 0xd8] {
        ax = octant(w, bp, dx, bx);
    }
    w.update::<super::phrase_args::PhraseArgs>(|t| t.slots[super::phrase_args::DIRECTION] = ax.wrapping_add(0xda));
    octant(w, 0xcc, dx, bx);
    octant(w, 0xd2, dx, bx);
    Some(())
}

/// `loc_0331e`: the place `p` talked about: noted ([11CEh]), its name,
/// chance, status, spice, water, kind and equipment copied, its forces
/// measured, its troops counted, its spare kinds, the characters there
/// (`loc_03385`), the places nearest.
pub fn describe_place(w: &mut World, p: u16) -> Option<()> {
    w.set_word(g::TOPIC_PLACE_RECORD, p);
    let (b0, b1) = (w.byte(p), w.byte(p.wrapping_add(1)));
    w.set_word(g::TOPIC_PLACE_NAME, (b0 as u16) << 8 | b1 as u16);
    w.set_byte(g::TOPIC_PLACE_CHANCE, w.byte(g::CHANCES.wrapping_add(b1 as u16)));
    for (src, dst) in [(0x0au16, g::TOPIC_PLACE_STATUS), (0x12, g::TOPIC_PLACE_SPICE), (0x1b, g::TOPIC_PLACE_WATER), (8, g::TOPIC_PLACE_KIND)] {
        w.set_byte(dst, w.byte(p.wrapping_add(src)));
    }
    for k in 0..7 {
        w.set_byte(g::TOPIC_PLACE_EQUIPMENT + k, w.byte(p.wrapping_add(0x14 + k)));
    }
    measure_forces(w, p)?;
    count_troops(w, p)?;
    super::native_moves::spare_check(w, p)?;
    let mut bits = 0xffu8;
    for i in 0..7u16 {
        bits = bits << 1 | (w.byte(g::LOCATION_SPARE_EQUIPMENT + i) == 0) as u8;
    }
    w.set_byte(g::TOPIC_PLACE_SPARE_KINDS, !bits << 1);
    if p == w.word(g::LAST_PLACE) {
        w.set_byte(g::TOPIC_PLACE_CHARACTERS, 0);
    } else {
        let off = p.wrapping_sub(place::FIRST);
        if off / place::SIZE > 0xff {
            return nm("a division fault");
        }
        w.set_byte(g::TOPIC_PLACE_CHARACTERS, 0);
        let bx = (((off / place::SIZE + 1) & 0xff) << 8) | 0x80;
        for si in [0x1018u16, 0x1028, 0x1048] {
            if w.word(si + 2) == bx {
                let bit = super::pure::shl(pc::cpu::Width::B, 1, w.byte(si + 0x0e)) as u8;
                w.set_byte(g::TOPIC_PLACE_CHARACTERS, w.byte(g::TOPIC_PLACE_CHARACTERS) | bit);
            }
        }
    }
    nearest_places(w, p)
}

/// The steps of the clock waiting (46DDh, the clock's).
const STEPS_PENDING: u16 = 0x46dd;

/// `loc_01b23`: the time step, when the clock has stepped: the desert's
/// fatigue ([`super::clock::fatigue_after`]), the day changed
/// ([`super::clock::day_change`]: the new day), and unless the endgame is
/// past stage 6: the character 7 check, the troops' tick, the water's
/// growth, the time of day's handler, the place talked about described
/// again (the phrase of the skills kept), the Harkonnens' place revisited
/// (not modelled). The date panel, the sky, the map's overlays and the
/// room redrawn are display.
pub fn time_step(w: &mut World) -> Option<()> {
    if w.byte(STEPS_PENDING) == 0 {
        return Some(());
    }
    w.set_byte(STEPS_PENDING, 0);
    let (after, low) = super::clock::fatigue_after(w.get::<Situation>().desert_fatigue);
    w.update::<Situation>(|s| {
        if low {
            s.desert_remarks = 0;
        }
        s.desert_fatigue = after;
    });
    let (now, old) = (w.time(), w.word(g::LAST_TIME));
    let d = super::clock::day_change(old, now);
    w.set_word(g::LAST_TIME, now);
    w.set_byte(g::DAY_CHANGED, d);
    if d != 0 {
        super::native_day::new_day(w)?;
    }
    if w.get::<Economy>().endgame_stage >= 7 {
        return Some(());
    }
    character_7_check(w)?;
    let skills = w.get::<super::phrase_args::PhraseArgs>().slots[super::phrase_args::SKILLS];
    let topic = w.word(g::TOPIC_PLACE_RECORD);
    super::native_moves::tick(w)?;
    super::native_day::grow_water(w)?;
    time_of_day(w)?;
    describe_place(w, topic)?;
    w.update::<super::phrase_args::PhraseArgs>(|t| t.slots[super::phrase_args::SKILLS] = skills);
    if w.get::<Progress>().harkonnen_place != 0 {
        return nm("the Harkonnens' place revisited");
    }
    Some(())
}
