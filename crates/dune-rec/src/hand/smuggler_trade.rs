//! The smugglers' trade, as rules over the Rust state (Stage B of
//! `docs/readable-port-plan.md`: subsystem logic in Rust).
//!
//! A smuggler ([`super::smugglers::Smuggler`]) is restocked each day
//! (`new_day`, `hand/tails.rs`), visited (`visit_smuggler`), offers its
//! equipment one kind after another (`open_deal`), hears the player's
//! answers (`answer_*`: buy, refuse, haggle), and is paid what it is owed
//! (`end_of_talk`, which also takes Duncan Idaho's shipment offers). The
//! decisions are these pure functions; the ports in `hand/story.rs` and
//! `hand/tails.rs` stay the original's 8086 frame around them.

/// The command phrase naming equipment kind 0 (the others follow).
pub const FIRST_ITEM_PHRASE: u16 = 0xe8;
/// A kind's price byte: this bit when the smuggler sells it (restocked).
pub const SOLD: u8 = 0x80;

/// Whether a kind is restocked today: none left, and sold there.
pub fn restocks(stock: u8, price: u8) -> bool {
    stock == 0 && price & SOLD != 0
}

/// A restock's draw from the random word `bx` (rotated left two each
/// time): the new stock (0 to 3), and the word after.
pub fn restock_draw(bx: u16) -> (u8, u16) {
    let bx = bx.rotate_left(2);
    ((bx & 3) as u8, bx)
}

/// The days since a visit on `visited_on`, `today`: 1 at the first visit
/// (not `met` yet).
pub fn days_away(today: u8, visited_on: u8, met: bool) -> u8 {
    if met { today.wrapping_sub(visited_on) } else { 1 }
}

/// The days a debt has run: since `since`, 0 without one.
pub fn debt_days(today: u8, since: u8, debt: u16) -> u8 {
    if debt == 0 { 0 } else { today.wrapping_sub(since) }
}

/// The kind first offered at a visit: the random word's low three bits
/// modulo the kinds offered (as the original subtracts: `None` without
/// any, where it would loop forever), with the subtractions made.
pub fn first_offer(random: u16, kinds: u8) -> Option<(u16, u32)> {
    if kinds == 0 {
        return None;
    }
    let mut v = random & 7;
    let mut rounds = 0;
    while v >= kinds as u16 {
        v -= kinds as u16;
        rounds += 1;
    }
    Some((FIRST_ITEM_PHRASE + v, rounds))
}

/// The price asked for a kind: twice its price byte's low seven bits.
pub fn asked_price(price: u8) -> u8 {
    (price & 0x7f).wrapping_shl(1)
}

/// The next offer after the kind of `item` (its phrase): the next kind
/// in stock, wrapping round the `kinds` offered at most twice (as the
/// original's `loop` counts). Words, as the original's `inc ax` makes
/// them: the kind is the phrase less [`FIRST_ITEM_PHRASE`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NextOffer {
    /// The kind offered (AX at 23BCh), `None` when the wraps ran out.
    pub kind: Option<u16>,
    /// AX and CX as the search leaves them.
    pub ax: u16,
    pub cx: u16,
    /// Kinds stepped to (`inc ax`), wraps (`sub al`), kinds looked at in
    /// the stock.
    pub steps: u32,
    pub wraps: u32,
    pub looked: u32,
}

pub fn next_offer(item: u16, kinds: u8, mut in_stock: impl FnMut(u16) -> bool) -> NextOffer {
    let mut o = NextOffer { kind: None, ax: item.wrapping_sub(FIRST_ITEM_PHRASE), cx: 2, steps: 0, wraps: 0, looked: 0 };
    loop {
        o.ax = o.ax.wrapping_add(1);
        o.steps += 1;
        loop {
            if (o.ax as u8) < kinds {
                break;
            }
            o.ax = o.ax & 0xff00 | (o.ax as u8).wrapping_sub(kinds) as u16;
            o.wraps += 1;
            o.cx -= 1;
            if o.cx == 0 {
                return o;
            }
        }
        o.looked += 1;
        if in_stock(o.ax) {
            o.kind = Some(o.ax);
            return o;
        }
    }
}

/// The price after a haggle: an eighth less (rounded down).
pub fn haggled(price: u8) -> u8 {
    price - (price >> 3)
}

/// The answers (the byte [`g::ANSWER`](super::globals::ANSWER)).
pub mod answer {
    pub const BUY: u8 = 1;
    pub const REFUSE: u8 = 2;
    pub const HAGGLE: u8 = 3;
}

/// The smuggler's mood (the byte
/// [`g::SMUGGLER_MOOD`](super::globals::SMUGGLER_MOOD)): 0 to 3, picked
/// as the deal opens, one on (wrapping) at each haggle; this bit when it
/// takes offence.
pub const OFFENDED: u8 = 0x10;

/// What a refusal does: the smuggler takes offence on a draw of 0 (the
/// random word's low three bits: 1 in 8), else drops the offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Offended,
    Dropped,
}

pub fn refusal(draw: u16) -> Refusal {
    if draw & 7 == 0 { Refusal::Offended } else { Refusal::Dropped }
}

/// What a haggle does, from the random word ANDed with 3 (`draw`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Haggle {
    /// A draw of 0 (1 in 4): offence taken.
    Offended,
    /// Its patience ran out (the answers so far, plus the draw's bit 0,
    /// reach it): the offer dropped.
    OutOfPatience,
    /// The price an eighth lower ([`haggled`]).
    Lowered,
}

pub fn haggle(draw: u16, answers: u8, patience: u8) -> Haggle {
    if draw & 3 == 0 {
        Haggle::Offended
    } else if (draw as u8 & 1).wrapping_add(answers) >= patience {
        Haggle::OutOfPatience
    } else {
        Haggle::Lowered
    }
}

/// The mood after a haggle not taken badly: one on, kept to 0..3 (the
/// offence bit dropped).
pub fn mood_after_haggle(mood: u8) -> u8 {
    mood.wrapping_add(1) & 3
}

/// A purchase on credit at `price`: the debt after it and whether it is a
/// new debt (it was 0: one more of the player's debts).
pub fn bought_on_credit(debt: u16, price: u8) -> (u16, bool) {
    let after = debt.wrapping_add(price as u16);
    (after, after == price as u16)
}

/// How a talk ended (bits 60h of a smuggler's flags): 40h on a refusal,
/// 20h on a haggle, kept clear by a purchase.
pub fn talk_ended(flags: u8, last: u8) -> u8 {
    let f = flags & !super::smugglers::smuggler_flag::TALK_ENDED;
    match last {
        answer::REFUSE => f | 0x40,
        _ => f | 0x20,
    }
}

/// Duncan Idaho's shipment offer taken: the one of the four by the
/// answers given (one less, kept to 0..3).
pub fn offer_taken(answers: u8) -> u16 {
    (answers as u16).wrapping_sub(1) & 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restock_and_visit() {
        assert!(restocks(0, 0x85) && !restocks(1, 0x85) && !restocks(0, 0x05));
        assert_eq!(restock_draw(0x4001), (1, 0x0005));
        assert_eq!(days_away(10, 7, true), 3);
        assert_eq!(days_away(10, 7, false), 1);
        assert_eq!(debt_days(3, 250, 5), 9);
        assert_eq!(debt_days(3, 250, 0), 0);
        assert_eq!(first_offer(0x0007, 3), Some((0xe9, 2)));
        assert_eq!(first_offer(7, 0), None);
    }

    #[test]
    fn offers() {
        assert_eq!(asked_price(0x8c), 0x18);
        // From kind 1 of 3: kind 2 out of stock, 0 in it.
        let o = next_offer(0xe9, 3, |k| k == 0);
        assert_eq!((o.kind, o.steps, o.wraps, o.looked), (Some(0), 2, 1, 2));
        // None in stock: two wraps and done.
        let o = next_offer(0xe8, 3, |_| false);
        assert_eq!((o.kind, o.cx), (None, 0));
        assert_eq!(haggled(0x40), 0x38);
    }

    #[test]
    fn answers() {
        assert_eq!(refusal(8), Refusal::Offended);
        assert_eq!(refusal(9), Refusal::Dropped);
        assert_eq!(haggle(0, 0, 5), Haggle::Offended);
        assert_eq!(haggle(3, 4, 5), Haggle::OutOfPatience);
        assert_eq!(haggle(2, 4, 5), Haggle::Lowered);
        assert_eq!(mood_after_haggle(0x13), 0);
        assert_eq!(bought_on_credit(0, 0x20), (0x20, true));
        assert_eq!(bought_on_credit(0x10, 0x20), (0x30, false));
        assert_eq!(talk_ended(0x6b, answer::REFUSE), 0x4b);
        assert_eq!(talk_ended(0x4b, answer::HAGGLE), 0x2b);
        assert_eq!(offer_taken(0), 3);
        assert_eq!(offer_taken(2), 1);
    }
}
