//! A troop's work, as rules over the Rust state (Stage B of
//! `docs/readable-port-plan.md`: subsystem logic in Rust): how motivated
//! it works and what it produces.
//!
//! Pure functions of a troop's record and its place; `troop_motivation`
//! (6EFDh) and `troop_rate` (708Ah) in `hand/troops.rs` are their 8086
//! frame (the instruction count of the original's path charged, its
//! registers set).

use super::game::troop_job;

/// The most motivation counts.
pub const MAX_MOTIVATION: u8 = 100;
/// While the troops are encouraged ([0FAh]): this much more.
pub const ENCOURAGED: u8 = 20;
/// Attacking at the player's own place: this much more.
pub const AT_HOME: u8 = 30;
/// In the story's phases 64h to 67h: this much less, down to
/// [`LATE_FLOOR`].
pub const LATE_PENALTY: u8 = 40;
pub const LATE_FLOOR: u8 = 10;

/// A troop's effective motivation, and how it was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Motivation {
    /// The motivation it works with (0 to 100, at least 10 late).
    pub value: u8,
    /// Its job as the original leaves it in AH: bit 0 cleared, but for
    /// the attacks.
    pub job: u8,
    /// Held at [`MAX_MOTIVATION`] (a sum above it).
    pub capped: bool,
    /// Held at [`LATE_FLOOR`].
    pub floored: bool,
}

/// The effective motivation of a troop with `motivation` at `job`:
/// [`ENCOURAGED`] more while `encouraged`; attacking at the player's
/// place (`at_home`) [`AT_HOME`] more, at most 100; irrigating or
/// assembling wind-traps (jobs 8, 9) always 100; others at most 100. In
/// the phases 64h to 67h [`LATE_PENALTY`] less, at least [`LATE_FLOOR`]
/// (signed: below 0 is the floor too). (Bytes, wrapping, as the original
/// adds them.)
pub fn effective_motivation(motivation: u8, job: u8, encouraged: bool, at_home: bool, phase: u8) -> Motivation {
    let mut v = motivation;
    if encouraged {
        v = v.wrapping_add(ENCOURAGED);
    }
    let mut capped = false;
    let mut ah = job;
    if job == troop_job::ATTACKS {
        if at_home {
            v = v.wrapping_add(AT_HOME);
            if v >= MAX_MOTIVATION {
                (v, capped) = (MAX_MOTIVATION, true);
            }
        }
    } else {
        ah = job & 0xfe;
        if ah == troop_job::IRRIGATION {
            v = MAX_MOTIVATION;
        } else if v >= MAX_MOTIVATION {
            (v, capped) = (MAX_MOTIVATION, true);
        }
    }
    let mut floored = false;
    if (0x64..0x68).contains(&phase) {
        v = v.wrapping_sub(LATE_PENALTY);
        if (v as i8) < LATE_FLOOR as i8 {
            (v, floored) = (LATE_FLOOR, true);
        }
    }
    Motivation { value: v, job: ah, capped, floored }
}

/// A troop's production rate (its word 0Ch): its effective `motivation`
/// plus the top four bits of its first skill, times its `size`, a quarter
/// of it without equipment kind 0 (`armed`: bit 80h of byte 19h); then
/// that product's high byte + 1 times the top four bits of the place's
/// spice (its byte 12h) + 1... as the original's `inc ax` / `mul ah`
/// computes it: the two bytes of (high byte, spice's top) + 1 multiplied,
/// the result's bytes swapped and rotated left one, 9 bits. Also whether
/// it was quartered (the frame's count).
pub fn production_rate(motivation: u8, skill: u8, size: u8, armed: bool, place_spice: u8) -> (u16, bool) {
    let base = motivation.wrapping_add(skill & 0xf0);
    let mut product = base as u16 * size as u16;
    if !armed {
        product >>= 2;
    }
    let pair = (product & 0xff00 | (place_spice & 0xf0) as u16).wrapping_add(1);
    let rate = ((pair & 0xff) * (pair >> 8)).swap_bytes().rotate_left(1) & 0x01ff;
    (rate, !armed)
}

/// Which way a troop's rate went, as the bits of its word 10h say
/// (bits 0Ch): 8 up, 4 down, none unchanged.
pub fn rate_trend(old: u16, new: u16) -> Option<u8> {
    match old.cmp(&new) {
        std::cmp::Ordering::Less => Some(8),
        std::cmp::Ordering::Greater => Some(4),
        std::cmp::Ordering::Equal => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motivation() {
        let m = effective_motivation(90, troop_job::SPICE_MINING, true, false, 0x30);
        assert_eq!((m.value, m.capped), (100, true));
        let m = effective_motivation(50, troop_job::WIND_TRAP_ASSEMBLY, false, false, 0x30);
        assert_eq!((m.value, m.job), (100, troop_job::IRRIGATION));
        let m = effective_motivation(50, troop_job::ATTACKS, false, true, 0x30);
        assert_eq!((m.value, m.job), (80, troop_job::ATTACKS));
        let m = effective_motivation(30, troop_job::SPICE_MINING, false, false, 0x65);
        assert_eq!((m.value, m.floored), (10, true));
        let m = effective_motivation(80, troop_job::SPICE_MINING, false, false, 0x65);
        assert_eq!((m.value, m.floored), (40, false));
    }

    #[test]
    fn rate() {
        // 9 bits at most, and a quarter without the equipment.
        let (full, q) = production_rate(100, 0xf0, 0xff, true, 0xf0);
        assert!(!q && full <= 0x1ff);
        let (quarter, q) = production_rate(100, 0xf0, 0xff, false, 0xf0);
        assert!(q && quarter < full);
        assert_eq!(rate_trend(5, 9), Some(8));
        assert_eq!(rate_trend(9, 5), Some(4));
        assert_eq!(rate_trend(5, 5), None);
    }
}
