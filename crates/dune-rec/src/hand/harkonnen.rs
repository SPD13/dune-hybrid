//! The Harkonnens' attacks, as rules over the Rust state (Stage B of
//! `docs/readable-port-plan.md`: subsystem logic in Rust).
//!
//! Three routines decide them: `pick_attack_location` (1E43h,
//! `hand/troops.rs`: which of the player's places they attack next),
//! `harkonnen_take` (1DD7h, `hand/story.rs`: whether they take a place
//! today) and `location_raided` (7443h, `hand/map.rs`: a place the player
//! raided). Their decisions are these pure functions; the ports stay the
//! original's 8086 frame (the troops counted and moved by the calls they
//! make, the instruction counts, the registers).

use super::game::location_flag;

/// The story phase in which an attack target is picked.
pub const PICKING_PHASE: u8 = 0x5c;
/// Location 63 (7C8h): no attack is picked while the player is there.
pub const PALACE: u16 = 0x07c8;
/// Location 17 (2C0h): never attacked.
pub const NEVER_ATTACKED: u16 = 0x02c0;
/// Places of this kind (byte 8) and up are not attacked.
pub const UNATTACKED_KIND: u8 = 0x28;

/// Whether an attack target is picked today, or why not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackDue {
    /// Before the day set ([`g::ATTACK_DAY`](super::globals::ATTACK_DAY)).
    NotYet,
    /// Not in the [`PICKING_PHASE`].
    OtherPhase,
    /// The player is at the [`PALACE`].
    PlayerAtPalace,
    Due,
}

/// Whether a target is picked on `day` (the day set: `from`) in `phase`,
/// the player at the location `here`.
pub fn attack_due(day: u16, from: u16, phase: u8, here: u16) -> AttackDue {
    if day < from {
        AttackDue::NotYet
    } else if phase != PICKING_PHASE {
        AttackDue::OtherPhase
    } else if here == PALACE {
        AttackDue::PlayerAtPalace
    } else {
        AttackDue::Due
    }
}

/// Why a place is not a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotATarget {
    /// Its kind is [`UNATTACKED_KIND`] or more.
    Kind,
    /// Not discovered yet.
    Undiscovered,
    /// [`NEVER_ATTACKED`].
    Spared,
}

/// Whether the place at `offset` (its `kind`, its `flags`) can be the
/// target (`None`: it can; the one with the most of the player's troops
/// is picked).
pub fn not_a_target(kind: u8, flags: u8, offset: u16) -> Option<NotATarget> {
    if kind >= UNATTACKED_KIND {
        Some(NotATarget::Kind)
    } else if flags & location_flag::UNDISCOVERED != 0 {
        Some(NotATarget::Undiscovered)
    } else if offset == NEVER_ATTACKED {
        Some(NotATarget::Spared)
    } else {
        None
    }
}

/// From this phase on the Harkonnens may take a place any day.
pub const TAKING_PHASE: u8 = 0x3c;
/// Before it, only this long (time units) after [1154h].
pub const TAKING_WAIT: u16 = 0x70;

/// Whether a takeover may happen now, or why not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TakeWindow {
    /// From the [`TAKING_PHASE`] on.
    Late,
    /// Before it: the time is before [1154h].
    Before,
    /// Before it: less than [`TAKING_WAIT`] since [1154h].
    TooSoon,
    /// Before it, the wait over.
    Waited,
}

/// The takeover's window in `phase` at the time `now`, [1154h] being
/// `since`.
pub fn take_window(phase: u8, now: u16, since: u16) -> TakeWindow {
    if phase >= TAKING_PHASE {
        TakeWindow::Late
    } else if now < since {
        TakeWindow::Before
    } else if now - since < TAKING_WAIT {
        TakeWindow::TooSoon
    } else {
        TakeWindow::Waited
    }
}

impl TakeWindow {
    pub fn open(self) -> bool {
        matches!(self, TakeWindow::Late | TakeWindow::Waited)
    }
}

/// No takeover on the days whose time has bit 10h (every other day).
pub fn quiet_day(now: u16) -> bool {
    now & 0x10 != 0
}

/// The takeover's draw from the random word [0]: taken when its top bit
/// is set; the word rotated left one either way.
pub fn take_draw(random: u16) -> (bool, u16) {
    (random & 0x8000 != 0, random.rotate_left(1))
}

/// A raided place: taken until two days on (its byte 0Bh), its radius
/// first [`RAID_RADIUS`].
pub const RAID_DAYS: u8 = 2;
pub const RAID_RADIUS: u8 = 5;

/// The day a raid on `day` ends.
pub fn raid_ends(day: u8) -> u8 {
    day.wrapping_add(RAID_DAYS)
}

/// The region mark of a place whose byte 0 is `region`, for
/// [`g::REGIONS_TO_UNMARK`](super::globals::REGIONS_TO_UNMARK): 8000h
/// rotated left by it (as `rol` does, the count's low five bits).
pub fn region_mark(region: u8) -> u16 {
    0x8000u16.rotate_left((region & 0x1f) as u32)
}

/// The player's troops meeting a place's own troop raid it when it is
/// of [`UNATTACKED_KIND`] or more; a sietch is only freed of its
/// Harkonnen mark.
pub fn meeting_raids(kind: u8) -> bool {
    kind >= UNATTACKED_KIND
}

/// How many Harkonnen troops a raid leaves garrisoned at most: 1 on a
/// quarter of the random word's values (its low two bits 0), else 0.
pub fn garrison_bound(random: u16) -> u16 {
    (random & 3 == 0) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attacks() {
        assert_eq!(attack_due(10, 12, PICKING_PHASE, 0x100), AttackDue::NotYet);
        assert_eq!(attack_due(12, 12, 0x50, 0x100), AttackDue::OtherPhase);
        assert_eq!(attack_due(12, 12, PICKING_PHASE, PALACE), AttackDue::PlayerAtPalace);
        assert_eq!(attack_due(12, 12, PICKING_PHASE, 0x100), AttackDue::Due);
        assert_eq!(not_a_target(0x28, 0, 0x100), Some(NotATarget::Kind));
        assert_eq!(not_a_target(0x10, 0, NEVER_ATTACKED), Some(NotATarget::Spared));
        assert_eq!(not_a_target(0x10, 0, 0x100), None);
    }

    #[test]
    fn takeovers() {
        assert_eq!(take_window(0x3c, 0, 0x50), TakeWindow::Late);
        assert_eq!(take_window(0x10, 0x40, 0x50), TakeWindow::Before);
        assert_eq!(take_window(0x10, 0x60, 0x50), TakeWindow::TooSoon);
        assert_eq!(take_window(0x10, 0xc0, 0x50), TakeWindow::Waited);
        assert!(quiet_day(0x0170) && !quiet_day(0x0160));
        assert_eq!(take_draw(0x8001), (true, 0x0003));
        assert_eq!(take_draw(0x4000), (false, 0x8000));
    }

    #[test]
    fn raids() {
        assert_eq!(raid_ends(0xff), 1);
        assert_eq!(region_mark(0), 0x8000);
        assert_eq!(region_mark(1), 0x0001);
        assert_eq!(region_mark(4), 0x0008);
        assert_eq!(garrison_bound(0x1234), 1);
        assert_eq!(garrison_bound(0x1235), 0);
    }
}
