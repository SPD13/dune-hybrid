//! The simulator's acting player (Stage C of `docs/readable-port-plan.md`):
//! the game's player actions as native primitives, each the game's own
//! routine without its screens, and a script of timed actions
//! (`DAY ACTION ARGS`, one a line, `#` comments):
//!
//! - `ship AMOUNT` or `ship demand`: spice shipped to the Emperor
//!   (`loc_02566`, the comm's answer);
//! - `job TROOP JOB`: troop number TROOP given job JOB (`loc_06ad4`, the
//!   orders menu);
//! - `go TROOP PLACE`: troop TROOP sent to the place record PLACE (hex,
//!   `loc_084a6`);
//! - `recruit TROOP`: troop TROOP joins you (`loc_066ce`, its leader
//!   talked to at its sietch: the script vouches for the visit);
//! - `buy`: from the smuggler met (character 0Dh's), the next item it
//!   has offered (`loc_02388`) and bought on credit (`loc_023e6`), into
//!   the player's place's equipment (the script vouches for the visit).
//!
//! The decisions are the script's; the rules are the game's.

use super::globals as g;
use super::world::{nm, World};
use super::{economy::Economy, situation::Situation};

/// One action of the script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Ship(Option<u16>),
    Job(u8, u8),
    Go(u8, u16),
    Recruit(u8),
    Buy,
}

/// A script's line: on day `day` (counted from 0, the time / 16), `action`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Timed {
    pub day: u16,
    pub action: Action,
}

/// The script's text read (numbers decimal, places hex).
pub fn parse(text: &str) -> Result<Vec<Timed>, String> {
    let mut out = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        let err = |what: &str| format!("line {}: {what}: {line}", n + 1);
        let num = |s: &str| s.parse::<u16>().map_err(|_| err("a number"));
        let day = num(f[0])?;
        let action = match (f.get(1).copied(), f.len()) {
            (Some("ship"), 3) => Action::Ship(if f[2] == "demand" { None } else { Some(num(f[2])?) }),
            (Some("job"), 4) => Action::Job(num(f[2])? as u8, num(f[3])? as u8),
            (Some("go"), 4) => Action::Go(num(f[2])? as u8, u16::from_str_radix(f[3], 16).map_err(|_| err("a place (hex)"))?),
            (Some("recruit"), 3) => Action::Recruit(num(f[2])? as u8),
            (Some("buy"), 2) => Action::Buy,
            _ => return Err(err("an action (ship, job, go, recruit, buy)")),
        };
        out.push(Timed { day, action });
    }
    Ok(out)
}

/// The action done.
pub fn act(w: &mut World, a: &Action) -> Option<()> {
    match *a {
        Action::Ship(amount) => {
            let amount = amount.unwrap_or_else(|| w.get::<Economy>().demanded);
            ship(w, amount)
        }
        Action::Job(n, job) => {
            let t = super::native_troops::troop_number(n);
            super::native_jobs::set_job(w, t, job);
            Some(())
        }
        Action::Go(n, place) => super::native_moves::set_out(w, super::native_troops::troop_number(n), place),
        Action::Recruit(n) => troop_joins(w, super::native_troops::troop_number(n)),
        Action::Buy => {
            if open_deal(w)? {
                buy_on_credit(w);
            }
            Some(())
        }
    }
}

/// `loc_02566`: `amount` of spice shipped to the Emperor: paid
/// (`loc_02524`: the spice less it, the spice paid more); its share of
/// the demand ([0BCh]) the ratio ([0BEh]: x 100h, at most 1FFh, halved,
/// at least 1); the rating 7 from C0h, 6 above 80h, 5 at 80h, else 4 (the
/// ratio 0 then with flag 8); the flags (40h for 6 and up, 8 for 4; 80h);
/// the demand's day on by the rating and a random 0..3 (by the demands'
/// count), the days left, no call waiting ([1158h]). (Record 1008h's
/// release when it holds bits 42h, and the scene: not modelled, display.)
pub fn ship(w: &mut World, amount: u16) -> Option<()> {
    let f = w.byte(0x1008 + 0x0f);
    if f & 0x40 != 0 && f & 2 != 0 {
        return nm("record 1008h released by a shipment");
    }
    w.update::<Economy>(|e| e.spice = e.spice.wrapping_sub(amount));
    w.set_word(g::SPICE_PAID, w.word(g::SPICE_PAID).wrapping_add(amount));
    let den = w.get::<Economy>().demanded as u32;
    let num = (amount as u32) << 8;
    if den == 0 || num / den > 0xffff {
        return nm("a division fault");
    }
    let q = (num / den) as u16;
    let mut al = (q.min(0x1ff) >> 1) as u8;
    if al == 0 {
        al = 1;
    }
    let flags = w.get::<Economy>().shipment_flags;
    let (rating, ah) = if al >= 0xc0 {
        (7u16, 0x40u8)
    } else if al > 0x80 {
        (6, 0x40)
    } else if al == 0x80 {
        (5, 0)
    } else {
        (4, 8)
    };
    w.update::<Economy>(|e| {
        e.shipment_ratio = al;
        if rating < 5 && flags & 8 != 0 {
            e.shipment_ratio = 0;
        }
        e.shipment_flags = ah | 0x80;
    });
    w.set_word(g::SHIPMENT_DUE, w.word(g::SHIPMENT_DUE).wrapping_add(rating));
    let mask = ((w.get::<Economy>().demands >> 1) & 3) as u16;
    let r = w.rand_masked(mask);
    w.set_word(g::SHIPMENT_DUE, w.word(g::SHIPMENT_DUE).wrapping_add(r));
    let d = (w.time() >> 4).wrapping_sub(w.word(g::SHIPMENT_DUE)).wrapping_neg();
    w.update::<Situation>(|s| s.shipment_days_left = d as u8);
    w.set_word(g::SHIPMENT_READY, 0);
    Some(())
}

/// The built-in player (`--sim-policy`), on the same actions, before each
/// step: a demand waiting (flag 10h) is paid, in full when the spice
/// allows, else with what there is; each troop of yours awaiting orders
/// (job 2, not travelling, not started) is set mining (job 0) where it
/// is.
pub fn policy(w: &mut World) -> Option<()> {
    let e = w.get::<Economy>();
    if e.shipment_flags & 0x10 != 0 && e.spice > 0 {
        ship(w, e.demanded.min(e.spice))?;
    }
    let mut t = super::world::troop::FIRST;
    while t < g::TROOPS_END {
        let s = w.byte(t + super::world::troop::STATE);
        if s & 0x0f == 2 && s & 0xe0 == 0 && s & 0x40 == 0 {
            super::native_jobs::set_job(w, t, 0);
        }
        t += super::world::troop::SIZE;
    }
    Some(())
}

/// `loc_066ce`: the troop `t`, not yours (state bit 80h) nor the
/// Harkonnens', joins you: one more joined ([28h]; reaching [1178h], phase
/// 4Ch), a point of charisma, awaiting orders (job 2, its started bit
/// kept), stamped, its day today; its place, if never grown (byte 0Bh 0),
/// radius 2 and its circle.
pub fn troop_joins(w: &mut World, t: u16) -> Option<()> {
    use super::world::{place, troop};
    let s = w.byte(t + troop::STATE);
    if s & 0x80 == 0 || w.byte(t + troop::BYTE_10) & 0x80 != 0 {
        return Some(());
    }
    let joined = w.byte(0x28).wrapping_add(1);
    w.set_byte(0x28, joined);
    if joined >= w.byte(0x1178) {
        super::native_script::set_game_phase(w, 0x4c)?;
    }
    super::native_raids::raise_charisma(w, 1);
    let s = w.byte(t + troop::STATE);
    w.set_byte(t + troop::STATE, s & 0x20 | 2);
    super::native_troops::stamp(w, t);
    w.set_byte(t + troop::DAY, (w.time() >> 4) as u8);
    let p = w.word(t + troop::LOCATION);
    if w.byte(p + place::RADIUS) == 0 {
        w.set_byte(p + place::RADIUS, 2);
        super::native_troops::circle(w, p, 2, 0x20)?;
    }
    Some(())
}

/// The smuggler met (character 0Dh's word 0Ch).
fn current_smuggler(w: &World) -> u16 {
    w.word(g::CHARACTERS + 0xd0 + 0x0c)
}

/// `loc_02388`: the deal opened: phase 3Ch at least, the smuggler's mood
/// (`rand_masked` 0..3), its visit today, no answers yet; the next kind
/// in stock after the one offered last
/// ([`super::smuggler_trade::next_offer`]) offered at twice its price's
/// low seven bits. Whether one was offered.
pub fn open_deal(w: &mut World) -> Option<bool> {
    use super::{phrase_args::PhraseArgs, progress::Progress, smuggler_trade};
    super::native_script::set_game_phase(w, 0x3c)?;
    let mood = w.rand_masked(3);
    w.update::<Economy>(|e| e.smuggler_mood = mood as u8);
    let day = (w.time() >> 4) as u8;
    let di = current_smuggler(w);
    let item = w.get::<PhraseArgs>().slots[super::phrase_args::SMUGGLER_ITEM];
    let kinds = w.byte(g::ITEMS_OFFERED);
    let o = smuggler_trade::next_offer(item, kinds, |k| w.byte(di.wrapping_add(4).wrapping_add(k)) != 0);
    w.set_byte(di.wrapping_add(3), day);
    w.update::<Progress>(|t| t.answer_count = 0);
    let Some(k) = o.kind else {
        return Some(false);
    };
    let phrase = k.wrapping_add(smuggler_trade::FIRST_ITEM_PHRASE);
    w.update::<PhraseArgs>(|t| t.slots[super::phrase_args::SMUGGLER_ITEM] = phrase);
    let asked = (w.byte(di.wrapping_add(9).wrapping_add(k)) & 0x7f) << 1;
    w.update::<Economy>(|e| e.smuggler_price = asked);
    Some(true)
}

/// `loc_023e6`: the item offered bought on credit: the price added to the
/// debts (the player's and the smuggler's; a first debt counted, its day
/// today), the talk not ended, the stock one less, the player's place's
/// equipment of that kind one more.
pub fn buy_on_credit(w: &mut World) {
    use super::{phrase_args::PhraseArgs, progress::Progress, smuggler_trade};
    let di = current_smuggler(w);
    let price = w.get::<Economy>().smuggler_price as u16;
    let debt = w.word(di.wrapping_add(0x0e));
    let (_, new) = smuggler_trade::bought_on_credit(debt, price as u8);
    w.set_byte(di.wrapping_add(2), w.byte(di.wrapping_add(2)) & !super::smugglers::smuggler_flag::TALK_ENDED);
    w.update::<Economy>(|e| e.smuggler_price = 0);
    w.update::<Progress>(|t| {
        t.smuggler_debt = t.smuggler_debt.wrapping_add(price);
        if new {
            t.debts = t.debts.wrapping_add(1);
        }
    });
    w.set_word(di.wrapping_add(0x0e), debt.wrapping_add(price));
    w.set_byte(di.wrapping_add(0x10), (w.time() >> 4) as u8);
    let i = w.get::<PhraseArgs>().slots[super::phrase_args::SMUGGLER_ITEM].wrapping_sub(0xe8);
    let s = di.wrapping_add(4).wrapping_add(i);
    w.set_byte(s, w.byte(s).wrapping_sub(1));
    let at = i.wrapping_add(w.word(g::PLACE)).wrapping_add(0x14);
    w.set_byte(at, w.byte(at).wrapping_add(1));
}
