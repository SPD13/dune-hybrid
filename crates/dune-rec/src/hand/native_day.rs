//! The new day natively: `new_day` (1C46h) and everything it calls, as
//! rules on a [`World`] (the native mode, Stage C of
//! `docs/readable-port-plan.md`). No registers, no instruction counts:
//! only the state a save keeps and the random seeds. The display text the
//! original writes on the way (the statistics' digits in the phrase
//! file's string) is not state and is left out.
//!
//! The rules are those the 8086 frames use (`spice_accounts`,
//! `harkonnen`, `smuggler_trade`); the loops and record walks are the
//! original's, read from the listing (`loc_01c46`, `loc_01d66`,
//! `loc_01e43`, `loc_01cda`, `loc_0c02e`, `loc_0bfe3`, `loc_0bf26`,
//! `loc_029f0`, `loc_02a34`).

use super::globals as g;
use super::world::World;
use super::{economy::Economy, harkonnen, progress::Progress, situation::Situation, smuggler_trade, smugglers, spice_accounts};

use super::world::{place, troop, STORY_END};

/// The rooms' limits by the place's kind (CS:1D35h, `loc_01d66`).
const ROOM_LIMITS: [u8; 0x31] = [
    2, 2, 2, 2, 2, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 5, 2, 2, 2, 2, 2, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 0x0c, 1, 1, 1, 1, 1,
    1, 1, 3, 3, 3, 3, 3, 3, 3, 3, 2,
];

/// The new day on `w` (`None`: a case the rules don't model, the world
/// then partly changed).
pub fn new_day(w: &mut World) -> Option<()> {
    let ph = w.get::<Progress>().phase;
    let last = w.get::<Situation>().last_day_phase;
    w.update::<Situation>(|t| {
        t.last_day_phase = ph;
        if ph != last {
            t.days_in_phase = 0;
        }
        t.days_in_phase = t.days_in_phase.wrapping_add(1);
    });
    fix_character_rooms(w)?;
    pick_attack_location(w)?;
    let d = w.get::<Situation>().jessica_days.wrapping_add(1);
    if d >= 2 {
        w.update::<Situation>(|t| t.jessica_days = d);
    }
    let e = w.get::<Economy>();
    let (stock, _) = spice_accounts::day_income(e.spice, w.word(g::SPICE_PAID), w.word(g::SPICE_YESTERDAY));
    let (fell, rose) = spice_accounts::income_change(e.income_before, stock);
    w.set_word(g::SPICE_PAID, 0);
    w.set_word(g::SPICE_YESTERDAY, e.spice);
    w.update::<Economy>(|t| {
        t.income = stock;
        t.income_before = stock;
        t.income_fall = fell;
        t.income_rise = rose;
    });
    estimate_field_spice(w)?;
    update_totals(w)?;
    stats_numbers(w);
    restock_smugglers(w)
}

/// `loc_01d66`: each of the first 12 characters at a place (word 2: 80h,
/// the place's number) whose room (high byte of word 0) isn't the place's
/// kind, or whose number (low byte) is past the kind's limit, gets room
/// the kind, number 1.
fn fix_character_rooms(w: &mut World) -> Option<()> {
    for i in 0..12 {
        let record = g::CHARACTERS + 0x10 * i;
        let w2 = w.word(record + 2);
        let (at_place, number) = (w2 as u8 == 0x80, (w2 >> 8) as u8);
        if !at_place || number == 0xff {
            continue;
        }
        let p = (number as u16 * place::SIZE).wrapping_add(place::FIRST - place::SIZE);
        if p.wrapping_add(place::SIZE) > STORY_END {
            return super::world::nm("a walk past the tables");
        }
        let kind = w.byte(p + place::KIND);
        let v = w.word(record);
        let Some(&limit) = ROOM_LIMITS.get(kind as usize) else {
            return super::world::nm("a room limit past the table");
        };
        let changed = (v >> 8) as u8 != kind || v as u8 > limit;
        if changed {
            w.set_word(record, (kind as u16) << 8 | 1);
        }
    }
    Some(())
}

/// The troops of the place `p` whose byte 3 is below 80h (yours:
/// `loc_0661d`), by record.
pub(super) fn your_troops(w: &World, p: u16) -> Option<Vec<u16>> {
    let mut out = Vec::new();
    let mut n = w.byte(p + place::FIRST_TROOP);
    while n != 0 {
        let t = troop::FIRST.wrapping_add((n.wrapping_sub(1) as u16) * troop::SIZE);
        if t.wrapping_add(troop::SIZE) > STORY_END || out.len() > 0x100 {
            return super::world::nm("a walk past the tables");
        }
        if w.byte(t + troop::STATE) < 0x80 {
            out.push(t);
        }
        n = w.byte(t + troop::NEXT);
    }
    Some(out)
}

/// The places, by record, to the FFh after the last.
pub(super) fn places(w: &World) -> Option<Vec<u16>> {
    let mut out = vec![place::FIRST];
    loop {
        let next = out.last()?.wrapping_add(place::SIZE);
        if next >= STORY_END {
            return super::world::nm("a walk past the tables");
        }
        if w.byte(next) == 0xff {
            return Some(out);
        }
        out.push(next);
    }
}

/// `loc_01e43`: when an attack is due ([`harkonnen::attack_due`]), the
/// place not spared ([`harkonnen::not_a_target`]) with the most of your
/// troops below state 8 (the first of equals) is the target: noted, one
/// more attack pending, its troops of yours marked (word 12h bit 400h,
/// state 10h: `loc_01ea9`, `loc_07085`), and event 0F08h there.
fn pick_attack_location(w: &mut World) -> Option<()> {
    let day = w.time() >> 4;
    let due = harkonnen::attack_due(day, w.word(g::ATTACK_DAY), w.get::<Progress>().phase, w.word(g::PLACE));
    if due != harkonnen::AttackDue::Due {
        return Some(());
    }
    let (mut best, mut target) = (0u16, 0u16);
    for p in places(w)? {
        if harkonnen::not_a_target(w.byte(p + place::KIND), w.byte(p + place::FLAGS), p).is_some() {
            continue;
        }
        let count = your_troops(w, p)?.iter().filter(|&&t| w.byte(t + troop::STATE) < 8).count() as u16;
        if count > best {
            best = count;
            target = p;
        }
    }
    if best == 0 {
        return Some(());
    }
    w.set_word(g::ATTACK_TARGET, target);
    w.update::<Situation>(|t| t.attacks_pending = t.attacks_pending.wrapping_add(1));
    for t in your_troops(w, target)? {
        w.set_word(t + troop::WORD_12, w.word(t + troop::WORD_12) | 0x400);
        w.set_byte(t + troop::STATE, w.byte(t + troop::STATE) | super::game::troop_state::MARKED);
    }
    add_pending_event(w, 0x0f08, target);
    Some(())
}

/// `loc_029f0`: with bit 1 of the events' flags ([0Ah]), the event
/// (`word`, `at`) added unless waiting already; with 10 waiting the oldest
/// dropped first (`loc_02a34`).
pub(super) fn add_pending_event(w: &mut World, word: u16, at: u16) {
    if w.get::<Progress>().event_flags & 1 == 0 {
        return;
    }
    let n0 = w.byte(g::PENDING_EVENTS);
    for i in 0..n0 as u16 {
        let e = g::NEXT_EVENT + 4 * i;
        if e + 4 > STORY_END {
            break;
        }
        if w.word(e) == word && w.word(e + 2) == at {
            return;
        }
    }
    let slot = if n0 >= 0x0a {
        let v = n0.wrapping_sub(1);
        w.set_byte(g::PENDING_EVENTS, v);
        if v == 0 {
            w.set_byte(g::PENDING_EVENTS - 1, 0);
        }
        // `rep movsw` of 12h words down by 4, two words of 0 after.
        let from = g::NEXT_EVENT;
        for k in 0..0x24 {
            let b = w.byte(from + 4 + k);
            w.set_byte(from + k, b);
        }
        w.set_word(from + 0x24, 0);
        w.set_word(from + 0x26, 0);
        9
    } else {
        n0 as u16
    };
    let count = w.byte(g::PENDING_EVENTS).wrapping_add(1);
    w.set_byte(g::PENDING_EVENTS, count);
    let e = g::NEXT_EVENT + 4 * slot;
    w.set_word(e, word);
    w.set_word(e + 2, at);
}

/// Whether the location loops skip the place `p` (`loc_05d36`): of a
/// kind below 28h, or raided (flag 08h).
pub(super) fn skipped(w: &World, p: u16) -> bool {
    w.byte(p + place::KIND) < 0x28 || w.byte(p + place::FLAGS) & super::game::location_flag::RAIDED != 0
}

/// `loc_01cda`: the spice in the fields: an eighth of each place's not
/// skipped, plus a random part to a sixteenth of that; the places
/// counted.
pub fn estimate_field_spice(w: &mut World) -> Option<u16> {
    let (mut sum, mut count) = (0u16, 0u16);
    for p in places(w)? {
        if !skipped(w, p) {
            count = count.wrapping_add(1);
            sum = sum.wrapping_add((w.byte(p + place::SPICE) >> 3) as u16);
        }
    }
    let r = w.rand_below(sum >> 4);
    w.update::<Economy>(|t| t.field_spice_estimate = sum.wrapping_add(r));
    Some(count)
}

/// `loc_0bfe3`: the map's share of cells grown (states 10h, 20h) and of
/// state 30h, in hundredths, from the first C5F9h cells' states.
fn map_shares(w: &mut World) -> Option<()> {
    const CELLS: usize = 0xc5f9;
    let state = |j: usize| -> Option<u8> { Some((w.save.map.get(j / 4)? >> (2 * (3 - j % 4))) & 3) };
    let (mut grown, mut any) = (0u16, 0u16);
    for j in 0..CELLS {
        let s = state(j)?;
        if s != 0 {
            any = any.wrapping_add(1);
            if s != 3 {
                grown = grown.wrapping_add(1);
            }
        }
    }
    let cells = (CELLS as u16).wrapping_sub(0x188).wrapping_add(1);
    if any.wrapping_sub(grown) >= cells || grown >= cells {
        return super::world::nm("a division fault");
    }
    let share = |n: u16| -> (u16, u16) {
        let q = ((n as u32) << 16) / cells as u32;
        let p = q * 100;
        ((p >> 16) as u16 + ((p as u16) >> 15), (p as u16) << 1)
    };
    let (a_hi, _) = share(any.wrapping_sub(grown));
    let (b_hi, _) = share(grown);
    w.update::<Economy>(|t| {
        t.map_percent_10_20 = b_hi.wrapping_add(1);
        t.map_percent_30 = a_hi;
    });
    Some(())
}

/// `loc_0c02e`: the map's shares, the day's income raised to what it is
/// now, and the armies counted: each troop not started (state 20h) adds
/// its size to the Harkonnens' (byte 10h bit 80h) or, if yours (state
/// below 80h), to yours. The walk goes on while the next record's number
/// isn't 0.
fn update_totals(w: &mut World) -> Option<()> {
    map_shares(w)?;
    let e = w.get::<Economy>();
    let (now, _) = spice_accounts::day_income(e.spice, w.word(g::SPICE_PAID), w.word(g::SPICE_YESTERDAY));
    if let Some(v) = spice_accounts::raised_income(e.income, now) {
        w.update::<Economy>(|t| t.income = v);
    }
    let (mut ours, mut theirs) = (0u16, 0u16);
    let mut t = troop::FIRST;
    loop {
        if t + 2 * troop::SIZE > STORY_END {
            return super::world::nm("a walk past the tables");
        }
        let (size, state, b10) = (w.byte(t + troop::SIZE_FIELD), w.byte(t + troop::STATE), w.byte(t + troop::BYTE_10));
        if state & 0x20 == 0 {
            if b10 & 0x80 != 0 {
                theirs = theirs.wrapping_add(size as u16);
            } else if state & 0x80 == 0 {
                ours = ours.wrapping_add(size as u16);
            }
        }
        t += troop::SIZE;
        if w.byte(t) == 0 {
            break;
        }
    }
    w.update::<Economy>(|e| {
        e.army_size = ours;
        e.harkonnen_army_size = theirs;
    });
    Some(())
}

/// The 16-tick period the stats were last compared in.
const STATS_PERIOD: u16 = 0x115c;

/// `loc_0bf26`: the six statistics (the map's shares, the field spice,
/// the income, the armies) kept at [`super::variables::STATS_NUMBERS`],
/// each one's change marked at [`super::variables::STATS_LETTERS`]: 2
/// fallen, 1 risen, 3 the same in a new 16-tick period (else unmarked).
fn stats_numbers(w: &mut World) {
    let period = w.time() & 0xfff0;
    let saved = w.word(STATS_PERIOD);
    for (k, src) in [0xa4u16, 0xa2, 0xa8, 0xa6, 0xac, 0xaa].into_iter().enumerate() {
        let new = w.word(src);
        let at = super::variables::STATS_NUMBERS + 2 * k as u16;
        let old = w.word(at);
        w.set_word(at, new);
        let mark = if new == old {
            (saved != period).then_some(3)
        } else if new < old {
            Some(2)
        } else {
            Some(1)
        };
        if let Some(v) = mark {
            w.set_byte(super::variables::STATS_LETTERS + k as u16, v);
        }
    }
}

/// The smugglers' restock (`loc_01c46`'s end): from a random word, each
/// smuggler met refills each sold-out kind ([`smuggler_trade::restocks`],
/// [`smuggler_trade::restock_draw`]), kinds 4 to 0; the walk ends at a
/// record whose first byte is 14h or more.
fn restock_smugglers(w: &mut World) -> Option<()> {
    let mut bx = w.rand();
    let mut si = g::SMUGGLERS;
    loop {
        if si + 0x11 >= STORY_END {
            return super::world::nm("a walk past the tables");
        }
        if w.byte(si + 2) & smugglers::smuggler_flag::MET != 0 {
            for kind in (0..=4u16).rev() {
                let (v, price) = (w.byte(si + 4 + kind), w.byte(si + 9 + kind));
                if smuggler_trade::restocks(v, price) {
                    let (a, after) = smuggler_trade::restock_draw(bx);
                    bx = after;
                    w.set_byte(si + 4 + kind, a);
                }
            }
        }
        si += 0x11;
        if w.byte(si) >= 0x14 {
            return Some(());
        }
    }
}

/// `loc_063f0` (`map_func_qq`): on a new day, each wind-trap below its
/// most gains water by the cells spreading around it
/// ([`super::ecology::wind_trap_water`], `loc_0642e`'s six cells from its
/// cell's offset less one), then the day's growth over the map
/// ([`super::ecology::grow`], `loc_065b6`, from the seed CS:65B4h).
pub fn grow_water(w: &mut World) -> Option<()> {
    use super::ecology;
    if w.byte(g::DAY_CHANGED) == 0 {
        return Some(());
    }
    let mut p = place::FIRST;
    loop {
        if p + place::SIZE + 2 > STORY_END {
            return super::world::nm("a walk past the tables");
        }
        let water = w.byte(p + 0x1b);
        if w.byte(p + place::FLAGS) & super::game::location_flag::WIND_TRAP != 0 && ecology::wind_trap_water(water, 0).is_some() {
            let around = ecology::spreading_around(|o| w.cell(o), w.word(p + 6), 2);
            let v = ecology::wind_trap_water(water, around).expect("below the most");
            w.set_byte(p + 0x1b, v);
        }
        p += place::SIZE;
        if w.word(p) == 0xffff {
            break;
        }
    }
    let grown = ecology::grow(|o| w.cell(o), w.growth_seed);
    for (o, b) in grown.grown {
        w.set_cell(o, b);
    }
    w.growth_seed = grown.seed;
    Some(())
}
