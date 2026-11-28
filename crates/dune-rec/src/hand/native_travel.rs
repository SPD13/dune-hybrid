//! The player's travel natively (the native mode, Stage C of
//! `docs/readable-port-plan.md`; instant, as decided with the user: the
//! flight's time is not simulated). The arrival at a place as
//! `loc_03ff5` runs it once the player is there: the place noted, the
//! battle checked (`loc_0503c`), the losses cleared, the place
//! discovered, its kind and number noted (visited for kinds 20h and up),
//! the people moved (`loc_02170`), the party following, room 1 entered
//! (`loc_0405a`), the room's arrival and scene (`loc_02dbf`, `loc_035ad`:
//! each person there says its greeting, or tells its waiting event).
//! Arriving at a Harkonnen place (the night attack) is not modelled.

use super::globals as g;
use super::world::{nm, place, World, STORY_END};
use super::{economy::Economy, place_forces::PlaceForces, player::PlayerAt, progress::Progress, situation::Situation};

/// The table of the people's next state, by their number (DS:144Dh).
const PEOPLE_NEXT_STATE: u16 = 0x144d;
/// The conversation record's scene flags and count, its first speaker.
const SCENE_FLAGS: u16 = 0x47a6;
const SCENE_COUNT: u16 = 0x47a7;
const FIRST_SPEAKER: u16 = 0x47aa;

/// The player travels to the place `p` and arrives there.
pub fn travel(w: &mut World, p: u16) -> Option<()> {
    if p < place::FIRST || p + place::SIZE > STORY_END {
        return nm("a travel to no place");
    }
    w.set_word(g::PLACE, p);
    w.set_word(g::LAST_PLACE, p);
    enter_location(w, p)?;
    w.update::<PlaceForces>(|t| {
        t.our_loss = 0;
        t.harkonnen_loss = 0;
    });
    super::native_script::discover(w, p)?;
    let n = ((p - place::FIRST) / place::SIZE + 1) as u8;
    let kind = w.byte(p + place::KIND);
    w.update::<Progress>(|t| {
        t.place_kind = kind;
        t.place_index = n;
    });
    if kind >= 0x20 {
        w.set_byte(p + place::FLAGS, w.byte(p + place::FLAGS) | 0x10);
    }
    update_people(w)?;
    let (dx, bx) = ((kind as u16) << 8 | 1, (n as u16) << 8 | 0x80);
    move_party_here(w, dx, bx);
    let mode = w.byte(0x46eb);
    if mode & 0x80 == 0 && dx == 0x3002 {
        return nm("room 3002h entered");
    }
    let old = w.get::<Progress>().room;
    w.update::<PlayerAt>(|t| {
        t.x = dx;
        t.y = bx;
    });
    w.update::<Progress>(|t| {
        t.room = dx as u8;
        t.previous_room = old;
    });
    if mode & 0x80 != 0 {
        return Some(());
    }
    room_arrive(w)
}

/// `loc_0503c`: the battle checked on arriving: no balance, no Harkonnen
/// place; a place not theirs and not skipped with no attacker of yours:
/// the comm's caller 4 when they have troops there; else (theirs, or
/// your attackers there) the Harkonnen place noted and the balance.
fn enter_location(w: &mut World, p: u16) -> Option<()> {
    w.update::<Situation>(|s| s.battle_balance = 0);
    w.update::<Progress>(|t| t.harkonnen_place = 0);
    if w.byte(p + place::FLAGS) & super::game::location_flag::HARKONNEN == 0 {
        if super::native_day::skipped(w, p) {
            return Some(());
        }
        let (cx, dx) = super::native_raids::sides(w, p)?;
        if dx == 0 {
            if cx != 0 {
                w.set_byte(g::COMM_CALLER, 4);
            }
            return Some(());
        }
    }
    w.update::<Progress>(|t| t.harkonnen_place = t.harkonnen_place.wrapping_add(1));
    let b = super::native_step::location_balance_check(w, p)? | 1;
    w.update::<Situation>(|s| s.battle_balance = b);
    Some(())
}

/// `loc_05344`: the discovered place nearest to (`x`, `y`) (the larger
/// of the x distance's high byte and the y distance, as the code compares
/// them).
fn nearest_location(w: &World, x: u16, y: u16) -> Option<u16> {
    let (mut best, mut di) = (0xffffu16, None);
    let mut si = place::FIRST;
    while si + place::SIZE <= STORY_END && w.word(si) != 0xffff {
        if w.byte(si + place::FLAGS) & super::game::location_flag::UNDISCOVERED == 0 {
            let dxd = w.word(si + 2).wrapping_sub(x);
            let dyd = w.word(si + 4).wrapping_sub(y);
            let cxa = if dxd & 0x8000 != 0 { dxd.wrapping_neg() } else { dxd };
            let ax = if dyd & 0x8000 != 0 { dyd.wrapping_neg() } else { dyd };
            let mut cx = cxa >> 8;
            if (cx as u8) < ax as u8 {
                cx = ax;
            }
            if cx < best {
                best = cx;
                di = Some(si);
            }
        }
        si += place::SIZE;
    }
    di
}

/// `loc_021fa`: a roll of 2..12 (`rand_below` 10, + 2), rolled again for
/// 3 (before phase 54h), and for 11 or 6 before phase 24h.
fn roll_2_to_12(w: &mut World) -> u8 {
    loop {
        let v = (w.rand_below(0x0a) as u8).wrapping_add(2);
        let ph = w.get::<Progress>().phase;
        if ph >= 0x54 {
            return v;
        }
        if v == 3 {
            continue;
        }
        if ph >= 0x24 || (v != 0x0b && v != 6) {
            return v;
        }
    }
}

/// `loc_02170`: the first nine people not in the party moved on: one
/// away from a place goes to the place nearest it (`loc_0221d`, marked
/// there when it is the player's); at a place, one not in room 1 (from
/// record 1028h, at place 1) gets a room 2..12; one in room 1: left at a
/// place of kind 21h up or the player's, or a Harkonnen place; elsewhere
/// its room on one, and at place 1 its next state (the table 144Dh; 6
/// becomes 0Ah before phase 24h, the record's bit 4 cleared). Record
/// 1008h at 180h stays in room 4.
fn update_people(w: &mut World) -> Option<()> {
    for i in 0..9u16 {
        let si = g::CHARACTERS + 0x10 * i;
        if w.byte(si + 0x0f) & 0x40 != 0 {
            continue;
        }
        let mut dx = w.word(si);
        let bx = w.word(si + 2);
        if si == 0x1008 && bx == 0x180 {
            dx = dx & 0xff00 | 4;
            w.set_word(si, dx);
        }
        if bx & 0xff != 0x80 {
            let Some(p) = nearest_location(w, w.word(si), bx) else {
                return nm("no place for a person to go to");
            };
            let n = ((p - place::FIRST) / place::SIZE + 1) as u8;
            w.set_word(si, (w.byte(p + place::KIND) as u16) << 8 | 1);
            w.set_word(si + 2, (n as u16) << 8 | 0x80);
            if n == w.byte(9) {
                w.set_byte(si + 0x0f, w.byte(si + 0x0f) | 4);
            }
            continue;
        }
        let (dl, dh, bh) = (dx as u8, (dx >> 8) as u8, (bx >> 8) as u8);
        if dl != 1 {
            if si >= 0x1028 && bh == 1 {
                let r = roll_2_to_12(w);
                w.set_byte(si, r);
            }
            continue;
        }
        if dh >= 0x21 || bh == w.get::<Progress>().place_index {
            continue;
        }
        let lp = (bh as u16 * place::SIZE).wrapping_add(place::FIRST - place::SIZE);
        if lp + place::SIZE > STORY_END {
            return nm("a walk past the tables");
        }
        if w.byte(lp + place::FLAGS) & super::game::location_flag::HARKONNEN != 0 {
            continue;
        }
        w.set_byte(si, w.byte(si).wrapping_add(1));
        if bh != 1 {
            continue;
        }
        let nx = w.byte(PEOPLE_NEXT_STATE.wrapping_add(w.byte(si + 0x0e) as u16));
        w.set_byte(si, nx);
        w.set_byte(si + 0x0f, w.byte(si + 0x0f) & 0xfb);
        if nx == 6 && w.get::<Progress>().phase < 0x24 {
            w.set_byte(si, 0x0a);
        }
    }
    Some(())
}

/// `loc_040c3`: the party (the people with the player, in the party)
/// follows to (`dx`, `bx`).
fn move_party_here(w: &mut World, dx: u16, bx: u16) {
    let pl = w.get::<PlayerAt>();
    for i in 0..0x10u16 {
        let si = g::CHARACTERS + 0x10 * i;
        if w.word(si + 2) == pl.y && w.word(si) == pl.x && w.byte(si + 0x0f) & 0x40 != 0 {
            w.set_word(si, dx);
            w.set_word(si + 2, bx);
        }
    }
}

/// `loc_02dbf` then `loc_02e52`'s scene (`loc_035ad`): the line's state
/// and the first speaker cleared; at a Harkonnen place the night attack
/// (not modelled); nothing talked here yet; the scene's flags reset, an
/// answer counted; in the comm room (8) with a shipment chosen and ready
/// and its bearer present, the shipment (`loc_02566`); then each person
/// here (`loc_03520`). (The room drawn, the sky, the music: display.)
fn room_arrive(w: &mut World) -> Option<()> {
    w.set_byte(0x47a4, 0);
    w.set_word(FIRST_SPEAKER, 0);
    if w.get::<Progress>().harkonnen_place != 0 {
        w.set_byte(g::SKIP_HARKONNEN_TAKE, w.byte(g::SKIP_HARKONNEN_TAKE) | 1);
        return nm("the night attack at a Harkonnen place");
    }
    w.update::<Progress>(|t| t.talked_here = 0);
    if w.byte(0x11c9) != 0 {
        return nm("a scene on the way (the travel's screen)");
    }
    let was = w.byte(SCENE_FLAGS);
    w.update::<Progress>(|t| t.answer_count = 0);
    w.set_byte(SCENE_COUNT, 0);
    w.set_byte(SCENE_FLAGS, 0);
    if was != 0 {
        return Some(());
    }
    let k = w.get::<Progress>().room;
    let f = w.get::<Economy>().shipment_chosen & w.word(g::SHIPMENT_READY);
    let call = k == 8 && f != 0 && w.get::<Progress>().present & 8 != 0;
    w.update::<Progress>(|t| t.answer_count = t.answer_count.wrapping_add(1));
    if call {
        super::native_player::ship(w, f)?;
    }
    let pl = w.get::<PlayerAt>();
    for i in 0..0x10u16 {
        let si = g::CHARACTERS + 0x10 * i;
        if w.word(si + 2) == pl.y && w.word(si) == pl.x {
            scene_sentence(w, si)?;
        }
    }
    Some(())
}

/// `loc_03520`: a person here, before anyone has spoken in this scene:
/// its greeting (`loc_096f1` with its number); none to say, its waiting
/// event told (`loc_02aaf`, `loc_02b00`); its events here dropped, the
/// scene counted, and (outside a sequence, the talk's reason below 64h
/// and not 3 or 4) its talk started. With neither a greeting nor an
/// event the person is passed over.
fn scene_sentence(w: &mut World, si: u16) -> Option<()> {
    if w.byte(SCENE_COUNT) != 0 {
        return Some(());
    }
    let n = w.byte(si + 0x0e) as u16;
    if !super::native_script::say_numbered(w, n)? {
        let talker = w.byte(0x47c4);
        let Some((ev, at)) = pending_event(w, talker, w.word(g::PLACE)) else {
            return Some(());
        };
        super::native_script::tell_found(w, ev, at)?;
    }
    let talker = w.word(0x47c4);
    super::native_script::drop_events(w, talker as u8, w.word(g::PLACE));
    let st = w.byte(g::TALK_REASON);
    w.update::<Situation>(|s| s.desert_rescues = 0);
    w.set_byte(SCENE_COUNT, w.byte(SCENE_COUNT).wrapping_add(1));
    if st == 3 || st == 4 || w.byte(0x4774) != 0 || st >= 0x64 {
        return Some(());
    }
    super::native_script::start_speaker_pub(w, talker);
    Some(())
}

/// `loc_02aaf`: the first event waiting told by `teller` (teller 0Fh's
/// only at the place `at`): its word and place.
fn pending_event(w: &World, teller: u8, at: u16) -> Option<(u16, u16)> {
    for i in 0..w.byte(g::PENDING_EVENTS) as u16 {
        let e = g::NEXT_EVENT + 4 * i;
        let (ev, p) = (w.word(e), w.word(e + 2));
        if (ev >> 8) as u8 == teller && (teller != 0x0f || p == at) {
            return Some((ev, p));
        }
    }
    None
}

/// For the checks of `loc_02170`, `loc_0503c`, `loc_03520`.
pub fn update_people_check(w: &mut World) -> Option<()> {
    update_people(w)
}
pub fn enter_location_check(w: &mut World, p: u16) -> Option<()> {
    enter_location(w, p)
}
pub fn scene_sentence_check(w: &mut World, si: u16) -> Option<()> {
    scene_sentence(w, si)
}
