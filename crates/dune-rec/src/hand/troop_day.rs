//! A troop's new day, as rules over the Rust state (Stage B of
//! `docs/readable-port-plan.md`: subsystem logic in Rust).
//!
//! Each new day ([`g::DAY_CHANGED`](super::globals::DAY_CHANGED)),
//! `troop_day_update` (6E20h, `hand/troops.rs`) runs for every troop of
//! the player at its location. The decisions it takes are these pure
//! functions of the records' fields; the port stays the original's 8086
//! frame around them (the calls it makes: `set_people_state_at_place`,
//! `grow_location`, the motivation's loss, the idle troops' marks, the
//! event), with the instruction counts and registers the original leaves.

use super::game::location_flag;

/// A raided location stays taken until this many days past the day in
/// its byte 0Bh: `today - ends` (a byte, wrapping) below FEh frees it.
pub const RAID_HOLD: u8 = 0xfe;

/// Whether a raided location's raid is over on `today` (its byte 0Bh:
/// `ends`; the location raided two days before it).
pub fn raid_over(today: u8, ends: u8) -> bool {
    today.wrapping_sub(ends) < RAID_HOLD
}

/// A raided location freed: its flags without [`location_flag::RAIDED`],
/// its kind kept to the low three bits (a sietch again), its radius
/// (byte 0Bh) [`FREED_RADIUS`]; and one more sietch held.
pub fn freed(flags: u8, kind: u8) -> (u8, u8) {
    (flags & !location_flag::RAIDED, kind & 7)
}

/// The byte 0Bh of a freed location.
pub const FREED_RADIUS: u8 = 5;

/// After this many days at the same job a troop loses motivation.
pub const RESTLESS_AFTER: u8 = 8;

/// Whether a troop that joined or got its job on day `since` loses
/// motivation on `today` (more than [`RESTLESS_AFTER`] days: a byte,
/// wrapping).
pub fn restless(today: u8, since: u8) -> bool {
    today.wrapping_sub(since) > RESTLESS_AFTER
}

/// The idle troops elsewhere that make the location's first troop report
/// (the message pair 302h): exactly this many.
pub const IDLE_REPORT: u8 = 3;

/// Whether the count of idle troops elsewhere makes a report.
pub fn idle_report(idle: u8) -> bool {
    idle == IDLE_REPORT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raids() {
        // Raided on day 10: ends on 12; taken on 10 and 11.
        assert!(!raid_over(10, 12));
        assert!(!raid_over(11, 12));
        assert!(raid_over(12, 12));
        assert!(raid_over(40, 12));
        assert_eq!(freed(0x2b, 0x4d), (0x23, 5));
    }

    #[test]
    fn restlessness() {
        assert!(!restless(18, 10));
        assert!(restless(19, 10));
        // Across the byte's wrap.
        assert!(restless(3, 250));
    }
}
