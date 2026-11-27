//! The raids natively (the native mode, Stage C of
//! `docs/readable-port-plan.md`): troops meeting at a place
//! (`loc_07429`), a place raided (`loc_07443` and its troops' callbacks
//! `loc_075af`, `loc_075ea`, `loc_0762a`), a place abandoned
//! (`loc_074b6`), the charisma's changes (`loc_06f78`, `loc_06fb0`) and a
//! troop started (`loc_0668f`), as rules on a [`World`].
//!
//! The walks over a place's chain read each troop's next after its
//! callback, as `loc_06603` does: a callback that relinks or resets the
//! troop (its next then 0) ends the walk there.

use super::game::{location_flag, troop_state};
use super::globals as g;
use super::native_day::add_pending_event;
use super::native_jobs::give_job_unless_same;
use super::native_moves::{link, reset, unlink};
use super::native_troops::{add_skill, circle, lower_motivation, set_held, troop_number};
use super::world::{nm, place, troop, World, STORY_END};
use super::{economy::Economy, harkonnen, progress::Progress};

const SLOT: u16 = 2;

/// `loc_06603`'s walk of the place `p`'s chain: `f` for each troop (with
/// whether it is yours: byte 3 below 80h), the next read after it.
fn each_troop(w: &mut World, p: u16, mut f: impl FnMut(&mut World, u16, bool) -> Option<()>) -> Option<()> {
    let mut n = w.byte(p + place::FIRST_TROOP);
    let mut k = 0;
    while n != 0 {
        let t = troop_number(n);
        if t + troop::SIZE > STORY_END || k > 0x100 {
            return nm("a walk past the tables");
        }
        k += 1;
        let yours = w.byte(t + troop::STATE) < 0x80;
        f(w, t, yours)?;
        n = w.byte(t + troop::NEXT);
    }
    Some(())
}

/// `loc_06f56`: every troop not started nor not a troop (bits A0h) gains
/// `by` motivation, at most 100 (the byte wrapping first).
fn add_motivation_to_all(w: &mut World, by: u8) {
    let mut t = troop::FIRST;
    while t < g::TROOPS_END {
        if w.byte(t + troop::STATE) & 0xa0 == 0 {
            let v = w.byte(t + troop::MOTIVATION).wrapping_add(by);
            w.set_byte(t + troop::MOTIVATION, v.min(100));
        }
        t += troop::SIZE;
    }
}

/// `increase_charisma_by_al` (6F78h): charisma up by `by` (the byte
/// wrapping, then at most C8h); each step of 4 it rises through, every
/// troop free (bits A0h of byte 3 clear) gains that much motivation.
pub fn raise_charisma(w: &mut World, by: u8) {
    let old = w.byte(g::CHARISMA);
    let new = by.wrapping_add(old).min(0xc8);
    w.set_byte(g::CHARISMA, new);
    let steps = (new & 0xfc).wrapping_sub(old & 0xfc) >> 2;
    if steps != 0 {
        add_motivation_to_all(w, steps);
    }
}

/// `loc_06fb0`: charisma down by `by`, to at least 1; each step of 4 it
/// drops through, every troop free loses that much motivation
/// (`decrease_troop_motivation`).
fn lose_charisma(w: &mut World, by: u8) {
    let before = w.byte(g::CHARISMA);
    let after = match before.checked_sub(by) {
        Some(v) if v > 0 => v,
        _ => 1,
    };
    let steps = (before & 0xfc).wrapping_sub(after & 0xfc) >> 2;
    w.set_byte(g::CHARISMA, after);
    if steps == 0 {
        return;
    }
    let mut t = troop::FIRST;
    while t < g::TROOPS_END {
        if w.byte(t + troop::STATE) & 0xa0 == 0 {
            lower_motivation(w, t, steps);
        }
        t += troop::SIZE;
    }
}

/// `loc_0668f`: the troop started (a revolt), unless it is already or
/// the Harkonnens': no equipment, its time of change now, 4 charisma
/// lost.
pub fn start_troop(w: &mut World, t: u16) {
    let s = w.byte(t + troop::STATE);
    if s & troop_state::STARTED != 0 || w.byte(t + troop::BYTE_10) & 0x80 != 0 {
        return;
    }
    w.set_byte(t + troop::STATE, s | troop_state::STARTED);
    w.set_byte(t + troop::EQUIPMENT, 0);
    w.set_word(t + troop::CHANGED_AT, w.time());
    lose_charisma(w, 4);
}

/// `loc_075af` for a troop of yours at the raided place `p`: troop 3 is
/// no longer started (its held state by the place, `loc_06b8a`); a
/// started troop becomes 22h; others are rallied (bit 400h of word 10h,
/// 20h too at a place of kind 28h or more), 4 motivation, 3 of skill 1,
/// to military training.
fn rally(w: &mut World, t: u16, p: u16) {
    let state = w.byte(t + troop::STATE);
    if t == troop_number(3) {
        w.set_byte(t + troop::STATE, state & !troop_state::STARTED);
        let held = w.byte(p + place::FLAGS) & 0x41 != 0;
        set_held(w, t, held);
        return;
    }
    if state & troop_state::STARTED != 0 {
        w.set_byte(t + troop::STATE, 0x22);
        return;
    }
    let far = w.byte(p + place::KIND) >= 0x28;
    w.set_word(t + troop::WORD_10, w.word(t + troop::WORD_10) | 0x400 | if far { 0x20 } else { 0 });
    let v = w.byte(t + troop::MOTIVATION).wrapping_add(4);
    w.set_byte(t + troop::MOTIVATION, v.min(100));
    add_skill(w, t, 1, 3);
    give_job_unless_same(w, t, super::game::troop_job::MILITARY_TRAINING);
}

/// `loc_075ea` for a Harkonnen troop at the raided place `p`: relinked
/// as not the Harkonnens'; past slot 7 it is lost (reset), else a new
/// troop of the place (state A0h, its men and motivation, skills drawn
/// from `rand_masked`, no equipment).
fn renew(w: &mut World, t: u16, p: u16) -> Option<()> {
    unlink(w, t)?;
    w.set_byte(t + troop::BYTE_10, w.byte(t + troop::BYTE_10) & !0x80);
    link(w, t, p)?;
    if w.byte(t + SLOT) >= 8 {
        return reset(w, t);
    }
    w.set_byte(t + troop::STATE, 0xa0);
    let v = w.rand_masked(0x0f7f).wrapping_add(0x1464);
    w.set_byte(t + troop::SIZE_FIELD, v as u8);
    w.set_byte(t + troop::MOTIVATION, (v >> 8) as u8);
    let v = w.rand_masked(0x1f1f).wrapping_add(0x0a0a);
    w.set_byte(t + troop::SKILLS, v as u8);
    w.set_byte(t + troop::SKILLS + 1, (v >> 8) as u8);
    w.set_byte(t + troop::EQUIPMENT, 0);
    w.set_byte(t + troop::BYTE_10, w.byte(t + troop::BYTE_10) & !0x10);
    Some(())
}

/// `loc_0747c`: the Harkonnen troops left at the place: walked until no
/// more than the bound ([`harkonnen::garrison_bound`]) are counted, each
/// beyond the bound lost (reset), the others captured (`loc_0762a`:
/// state ACh, byte 10h bit 10h, no men, rate nor equipment); then when at
/// most one place is left in the fields (`loc_01cda`'s count), the
/// endgame's first stage (records FF7h and 1007h lose bit 2, the
/// locations' byte 19h summed into the place's, `loc_0765e`).
fn garrison(w: &mut World, p: u16) -> Option<()> {
    let bound = harkonnen::garrison_bound(w.word(g::RANDOM_BITS));
    let mut rounds = 0;
    loop {
        rounds += 1;
        if rounds > 0x100 {
            return nm("a raid's garrison not settling");
        }
        let mut cx = 0u16;
        each_troop(w, p, |w, t, yours| {
            if yours || w.byte(t + troop::BYTE_10) & 0x80 == 0 {
                return Some(());
            }
            if cx >= bound {
                reset(w, t)?;
            } else {
                w.set_byte(t + troop::STATE, 0xac);
                w.set_byte(t + troop::BYTE_10, w.byte(t + troop::BYTE_10) | 0x10);
                w.set_byte(t + troop::SIZE_FIELD, 0);
                w.set_word(t + troop::RATE, 0);
                w.set_byte(t + troop::EQUIPMENT, 0);
            }
            cx += 1;
            Some(())
        })?;
        if cx <= bound {
            break;
        }
    }
    let counted = super::native_day::estimate_field_spice(w)?;
    if counted as u8 <= 1 {
        w.update::<Economy>(|e| e.endgame_stage = 1);
        for r in [0xff7u16, 0x1007] {
            w.set_byte(r, w.byte(r) & 0xfd);
        }
        let mut sum = 0u8;
        for q in super::native_day::places(w)? {
            sum = sum.wrapping_add(w.byte(q + 0x19));
        }
        if sum >= 10 {
            w.set_byte(p + 0x19, w.byte(p + 0x19).wrapping_add(sum - 10));
        }
    }
    Some(())
}

/// `loc_07443`: the place raided: radius 5 and its circle (state 20h),
/// the raid's end (two days on, byte 0Bh), 4 charisma, a point of
/// motivation for all, flag 08h, its region to unmark, your troops there
/// rallied, the Harkonnens' renewed, the garrison.
pub fn raided(w: &mut World, p: u16) -> Option<()> {
    w.set_byte(p + place::RADIUS, harkonnen::RAID_RADIUS);
    circle(w, p, harkonnen::RAID_RADIUS as u16, 0x20)?;
    w.set_byte(p + place::RADIUS, harkonnen::raid_ends((w.time() >> 4) as u8));
    raise_charisma(w, 4);
    add_motivation_to_all(w, 1);
    w.set_byte(p + place::FLAGS, w.byte(p + place::FLAGS) | 8);
    w.set_word(g::REGIONS_TO_UNMARK, harkonnen::region_mark(w.byte(p)));
    each_troop(w, p, |w, t, yours| {
        if yours {
            rally(w, t, p);
        }
        Some(())
    })?;
    each_troop(w, p, |w, t, yours| {
        if !yours && w.byte(t + troop::BYTE_10) & 0x80 != 0 {
            renew(w, t, p)?;
        }
        Some(())
    })?;
    garrison(w, p)
}

/// `loc_07429`: your troop meets the troops at the place `p`: away from
/// the player, event 0F07h; at a place of the raided kinds
/// ([`harkonnen::meeting_raids`]) the raid, else the Harkonnens' mark
/// lifted, your troops rallied and the garrison.
pub fn troops_meet(w: &mut World, p: u16) -> Option<()> {
    if p != w.word(g::PLACE) {
        add_pending_event(w, 0x0f07, p);
    }
    if harkonnen::meeting_raids(w.byte(p + place::KIND)) {
        return raided(w, p);
    }
    w.set_byte(p + place::FLAGS, w.byte(p + place::FLAGS) & !location_flag::HARKONNEN);
    each_troop(w, p, |w, t, yours| {
        if yours {
            rally(w, t, p);
        }
        Some(())
    })?;
    garrison(w, p)
}

/// `loc_074b6`: the place abandoned. The player's own: the Harkonnens'
/// mark lifted, the comm's caller 6. Else the mark lifted; a sietch
/// (below kind 28h) becomes abandoned (kind 28h + its low 3 bits, one
/// sietch less held, the people there in room 3 of it); its troops: the
/// Harkonnens' held (byte 10h bit 10h), others started; the circle of 5
/// (state 30h); flags 09h cleared.
pub fn abandon(w: &mut World, p: u16) -> Option<()> {
    let f = w.byte(p + place::FLAGS);
    w.set_byte(p + place::FLAGS, f & 0xfd);
    if p == w.word(g::PLACE) {
        w.set_byte(g::COMM_CALLER, 6);
        return Some(());
    }
    let kind = w.byte(p + place::KIND);
    if kind < 0x28 {
        let k = (kind & 7) + 0x28;
        w.set_byte(p + place::KIND, k);
        w.update::<Progress>(|t| t.sietches_held = t.sietches_held.wrapping_sub(1));
        let off = p.wrapping_sub(place::FIRST);
        if off / place::SIZE > 0xff {
            return nm("a division fault");
        }
        let bx = (((off / place::SIZE + 1) & 0xff) << 8) | 0x80;
        for i in 0..9u16 {
            let s = g::CHARACTERS + 0x10 * i;
            if w.word(s + 2) == bx {
                w.set_word(s, (k as u16) << 8 | 3);
            }
        }
    }
    each_troop(w, p, |w, t, _| {
        let b = w.byte(t + troop::BYTE_10);
        if b & 0x80 != 0 {
            w.set_byte(t + troop::BYTE_10, b | 0x10);
        } else {
            w.set_byte(t + troop::STATE, w.byte(t + troop::STATE) | troop_state::STARTED);
        }
        Some(())
    })?;
    circle(w, p, 5, 0x30)?;
    w.set_byte(p + place::FLAGS, w.byte(p + place::FLAGS) & 0xf6);
    Some(())
}

/// `loc_06e02`: the Harkonnen troops at the place `p` lost (reset), the
/// walk again until none is left.
pub fn lose_all_harkonnen(w: &mut World, p: u16) -> Option<()> {
    for _ in 0..0x100 {
        let mut cx = 0;
        each_troop(w, p, |w, t, yours| {
            if !yours && w.byte(t + troop::BYTE_10) & 0x80 != 0 {
                reset(w, t)?;
                cx += 1;
            }
            Some(())
        })?;
        if cx == 0 {
            return Some(());
        }
    }
    nm("Harkonnen troops not all lost")
}

/// `loc_05098`'s counts at the place `p`: the Harkonnen troops (CX) and
/// your attackers (state 6, DX), of the troops not started.
pub fn sides(w: &World, p: u16) -> Option<(u16, u16)> {
    let (mut cx, mut dx) = (0u16, 0u16);
    let mut n = w.byte(p + place::FIRST_TROOP);
    let mut k = 0;
    while n != 0 {
        let t = troop_number(n);
        if t + troop::SIZE > STORY_END || k > 0x100 {
            return nm("a walk past the tables");
        }
        k += 1;
        let s = w.byte(t + troop::STATE);
        if s & 0x20 == 0 {
            if w.byte(t + troop::BYTE_10) & 0x80 != 0 {
                cx += 1;
            } else if s == 6 {
                dx += 1;
            }
        }
        n = w.byte(t + troop::NEXT);
    }
    Some((cx, dx))
}

/// `loc_0758d`: the blow `dx` takes from the troop `t`: times FFh less
/// twice its skill 1 (a byte), over 100h (FFh past it), at most its men.
fn blow(w: &World, t: u16, dx: u16) -> u8 {
    let d = 0xffu8.wrapping_sub(w.byte(t + 0x17)).wrapping_sub(w.byte(t + 0x17)) as u32;
    let p = dx as u32 * d;
    let al = if p >> 16 != 0 { 0xff } else { (p >> 8) as u8 };
    al.min(w.byte(t + troop::SIZE_FIELD))
}

/// `loc_0739e`, job 6: the attack. At the Harkonnens' palace (11Ch): the
/// endgame's next stage, your troops there to training (state 4), the
/// Harkonnens there lost, the map's cells of state 30h to 20h, event
/// 0F0Ah. Elsewhere (the takeover skipped once, [11BCh]): the place's
/// forces measured; no Harkonnen strength: the troops meet
/// (`loc_07429`). Else a random byte against the strike odds: below, the
/// attacker's yield over the Harkonnen troops there (plus one, at most
/// FFh) strikes each Harkonnen troop ([`blow`]; a troop left without
/// men is started and held, one time in four losing its equipment
/// bit 4), the blows added to the attacker's word 0Ch, and with none
/// left standing the troops meet; not below, the Harkonnens' strength
/// over your troops there strikes the attacker (word 0Eh), and left
/// without men it gets 1Eh..9Dh new, is started, and with no attacker
/// left the place is abandoned.
pub fn attack(w: &mut World, t: u16, p: u16) -> Option<()> {
    w.set_byte(g::SKIP_HARKONNEN_TAKE, w.byte(g::SKIP_HARKONNEN_TAKE) | 1);
    if p == 0x11c {
        w.update::<Economy>(|e| e.endgame_stage = e.endgame_stage.wrapping_add(1));
        for x in super::native_day::your_troops(w, p)? {
            w.set_byte(x + troop::STATE, 4);
        }
        lose_all_harkonnen(w, p)?;
        for o in 0..0xc5f9u16 {
            let b = w.cell(o);
            if b & 0x30 == 0x30 {
                w.set_cell(o, b & 0xef);
            }
        }
        add_pending_event(w, 0x0f0a, 0x11c);
        return Some(());
    }
    super::native_step::measure_forces(w, p)?;
    if w.word(g::TOPIC_PLACE_HARKONNEN_STRENGTH) == 0 {
        return troops_meet(w, p);
    }
    let r = w.rand() as u8;
    if r < w.byte(g::TOPIC_PLACE_STRIKE_ODDS) {
        let (cx, _) = sides(w, p)?;
        let mut ax = super::native_step::troop_yield(w, t);
        if cx != 0 {
            ax /= cx;
        }
        let dl = (ax as u8).wrapping_add(1);
        let dx = if dl == 0 { 0xff } else { dl } as u16;
        let (mut total, mut standing) = (0u16, 0u16);
        each_troop(w, p, |w, x, _| {
            if w.byte(x + troop::BYTE_10) & 0x80 == 0 {
                return Some(());
            }
            standing += 1;
            let al = blow(w, x, dx);
            total = total.wrapping_add(al as u16);
            let size = w.byte(x + troop::SIZE_FIELD);
            w.set_byte(x + troop::SIZE_FIELD, size.wrapping_sub(al));
            if size > al {
                return Some(());
            }
            w.set_byte(x + troop::STATE, w.byte(x + troop::STATE) | 0x20);
            w.set_byte(x + troop::BYTE_10, w.byte(x + troop::BYTE_10) | 0x10);
            standing -= 1;
            if w.rand_masked(3) == 0 {
                w.set_byte(x + troop::EQUIPMENT, w.byte(x + troop::EQUIPMENT) & 0xfb);
                let at = w.word(x + troop::LOCATION);
                let bits = w.byte(x + troop::EQUIPMENT);
                for k in (0..8u16).filter(|&k| bits & (0x80 >> k) != 0) {
                    let v = w.byte(at + place::EQUIPMENT + k);
                    if v != 0 {
                        w.set_byte(at + place::EQUIPMENT + k, v - 1);
                    }
                }
            }
            Some(())
        })?;
        w.set_word(t + troop::RATE, w.word(t + troop::RATE).wrapping_add(total));
        if standing == 0 {
            return troops_meet(w, p);
        }
        return Some(());
    }
    let mut ax = w.word(g::TOPIC_PLACE_HARKONNEN_STRENGTH);
    let cl = w.byte(g::TOPIC_PLACE_OUR_TROOPS) as u16;
    if cl != 0 {
        ax /= cl;
    }
    let al = blow(w, t, ax);
    w.set_word(t + troop::WORD_0E, w.word(t + troop::WORD_0E).wrapping_add(al as u16));
    let size = w.byte(t + troop::SIZE_FIELD);
    w.set_byte(t + troop::SIZE_FIELD, size.wrapping_sub(al));
    if size > al {
        return Some(());
    }
    let men = (w.rand_masked(0x7f) as u8).wrapping_add(0x1e);
    w.set_byte(t + troop::SIZE_FIELD, men);
    start_troop(w, t);
    let (_, ours) = sides(w, p)?;
    if ours == 0 {
        abandon(w, p)?;
    }
    Some(())
}
