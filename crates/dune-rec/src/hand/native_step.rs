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
