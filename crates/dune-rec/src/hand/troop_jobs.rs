//! The troops' jobs, as rules over the Rust state (Stage B of
//! `docs/readable-port-plan.md`: subsystem logic in Rust).
//!
//! The job handlers (the table at CS:6C26h, called from `troops_tick`)
//! in `hand/troops.rs`: spice mining (`job_harvest`, 6FE5h), prospecting
//! (`job_spice_prospecting`, 70CCh), military training
//! (`job_military_training`, 71EFh), espionage (`job_espionage`, 72B0h),
//! irrigation (`job_irrigation`, 7693h), wind-trap assembly
//! (`job_wind_trap_assembly`, 7711h) and bulb growing
//! (`job_bulb_growing`, 767Dh). Their decisions and figures are these pure
//! functions; the handlers stay the original's 8086 frame (the calls,
//! the instruction counts, the registers).

// --- Spice mining --------------------------------------------------------

/// A troop waiting its turn (bit 200h of word 10h) works only in its own
/// slot of the time of day: the time's low nibble equal to its number's
/// (`None`: not waiting).
pub fn harvest_slot(word_10: u16, time: u16, id: u8) -> Option<bool> {
    (word_10 & 0x200 != 0).then_some(time & 0x0f == (id & 0x0f) as u16)
}

/// The harvest at `rate` added to the troop's tally (word 0Eh, `old`):
/// the new tally, and whether it crossed a multiple of 80h (a skill
/// point).
pub fn tally(old: u16, rate: u16) -> (u16, bool) {
    let total = old.wrapping_add(rate);
    (total, (total ^ old) & 0xff80 != 0)
}

/// The harvest counted in tenths: with the tenths left over
/// ([46E1h]), the whole spice gained and the tenths left.
pub fn in_tenths(rate: u16, tenths: u16) -> (u16, u16) {
    let n = rate.wrapping_add(tenths);
    (n / 10, n % 10)
}

/// What the harvest takes from its place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Taken {
    /// The place's spice (its byte 12h) lowered by this, not below 0.
    pub spice_units: u8,
    /// The leftover kept (byte 13h).
    pub left: u8,
    /// The place's spice after.
    pub spice: u8,
    /// More taken than the spice's low nibble (the place view counts it).
    pub more_than_low: bool,
}

/// The harvest's take from its place: the rate plus the leftover (byte
/// 13h) divided by the spice's worth (byte 11h). `None` where the
/// original's `div` faults (a worth of 0, or a quotient above FFh).
pub fn taken(rate: u16, left: u8, per: u8, spice: u8) -> Option<Taken> {
    let n = rate.wrapping_add(left as u16);
    if per == 0 || n / per as u16 > 0xff {
        return None;
    }
    let q = (n / per as u16) as u8;
    Some(Taken { spice_units: q, left: (n % per as u16) as u8, spice: spice.saturating_sub(q), more_than_low: q > spice & 0x0f })
}

// --- Espionage -----------------------------------------------------------

/// A spy's times from its skill 1: `half` = (50h - skill) / 2 and a
/// quarter (signed bytes, as the original's `sar`).
pub fn spy_times(skill: u8) -> (u16, u16) {
    let half = ((0x50u8.wrapping_sub(skill) as i8 as i16) >> 1) as u16;
    (half, ((half as i16) >> 1) as u16)
}

/// Past a time (signed, as the original compares): `t < elapsed`.
pub fn past(t: u16, elapsed: u16) -> bool {
    (t as i16) < elapsed as i16
}

/// The spy's report: the Harkonnens' strength there per its rate
/// (`None`: a rate of 0, where the `div` faults).
pub fn spy_report(strength: u16, rate: u16) -> Option<u16> {
    (rate != 0).then(|| strength / rate)
}

/// Whether a spy at a place not its own troop's starts (is found out):
/// past half its skill 1, on a random draw (its low six bits) not below
/// that skill. `None`: not past yet (no draw made).
pub fn spy_found_out(skill: u8, elapsed: u16, random: impl FnOnce() -> u16) -> Option<bool> {
    let half = (skill >> 1) as u16;
    if half >= elapsed {
        return None;
    }
    Some((random() as u8 & 0x3f) as u16 >= skill as u16)
}

// --- Bulb growing, wind-traps --------------------------------------------

/// Bulb growing's counter ([0ECh]) one on; when it wraps to 0 the place's
/// byte 1Ah becomes [`BULBS_GROWN`] and the troop goes on to irrigation.
pub fn bulb_count(k: u8) -> (u8, bool) {
    (k.wrapping_add(1), k == 0xff)
}

pub const BULBS_GROWN: u8 = 0x10;

/// A wind-trap's assembly: the place's byte 1Bh up by the troop's
/// strength; on a carry the trap is built (the place gets bit 20h of
/// byte 0Ah, bit 8 of byte 8, and byte 1Bh [`WIND_TRAP_START`]).
pub fn assembled(progress: u8, strength: u8) -> (u8, bool) {
    progress.overflowing_add(strength)
}

pub const WIND_TRAP_START: u8 = 5;

// --- Irrigation ----------------------------------------------------------

/// Irrigation's step: skill 2 / 4, at least 1.
pub fn irrigation_step(skill: u8) -> u8 {
    (skill >> 2).max(1)
}

/// The water needed for a step of growth (the place's byte 1Bh), and the
/// greatest radius (byte 0Bh) it grows to.
pub const GROWTH_WATER: u8 = 0x0c;
pub const MAX_RADIUS: u8 = 0x0c;
/// The place's byte 0Eh (signed) goes down 2 a growth, not below this.
pub const LOWEST_0E: i16 = -0x52;

/// What a step of irrigation that carried (the place's byte 0Fh past FFh)
/// does with the place's water `water`, radius and byte 0Eh.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Irrigated {
    /// Not enough water: it is 0, the troop marked.
    DriedUp,
    /// The water less [`GROWTH_WATER`], the radius one up (below
    /// [`MAX_RADIUS`]), byte 0Eh lowered (`None`: kept, it would pass
    /// [`LOWEST_0E`]); AX the lowered value either way.
    Grows { water: u8, radius: u8, byte_0e: Option<u8>, ax: u16 },
}

pub fn irrigated(water: u8, radius: u8, byte_0e: u8) -> Irrigated {
    if water < GROWTH_WATER {
        return Irrigated::DriedUp;
    }
    let ax = (byte_0e as i8 as i16 as u16).wrapping_sub(2);
    let keep = (ax as i16) < LOWEST_0E;
    Irrigated::Grows {
        water: water - GROWTH_WATER,
        radius: if radius < MAX_RADIUS { radius + 1 } else { radius },
        byte_0e: (!keep).then_some(ax as u8),
        ax,
    }
}

// --- Prospecting ---------------------------------------------------------

/// A prospector's rate (the time it takes), when it has none: the place's
/// spice worth (byte 11h) times 16 over its motivation plus skill 0 (a
/// byte). `None` where the `div` faults.
pub fn prospect_rate(per: u8, motivation: u8, skill: u8) -> Option<u16> {
    let v = (per as u16) << 4;
    let d = motivation.wrapping_add(skill);
    (d != 0 && v / d as u16 <= 0xff).then(|| v / d as u16)
}

/// The share of the prospecting done (word 0Eh, in hundredths) while the
/// time since it started is below the rate (`None`: done, found).
pub fn prospect_share(rate: u16, elapsed: u16) -> Option<u16> {
    (rate > elapsed).then(|| ((elapsed as u32 * 100) / rate as u32) as u16)
}

// --- Military training ---------------------------------------------------

/// The trainers' strength a troop drills against and its skill 1: what it
/// gains, doubled (0 when they are not above it).
pub fn drill_gain(strength: u8, skill: u8) -> u16 {
    strength.saturating_sub(skill) as u16 * 2
}

/// The motivation counted in a drill: at least this.
pub const DRILL_MOTIVATION: u8 = 0x1e;

/// The drill's length by the troop's equipment (byte 19h): C8h with
/// bits Ch, FAh with 10h, 12Ch with 20h, else 190h; and the original's
/// chain of tests to it (instructions).
pub fn drill_base(equipment: u8) -> (u16, u32) {
    if equipment & 0x0c != 0 {
        (0xc8, 0)
    } else if equipment & 0x10 != 0 {
        (0xfa, 3)
    } else if equipment & 0x20 != 0 {
        (0x12c, 6)
    } else {
        (0x190, 7)
    }
}

/// The new countdown (word 0Ch) after a drill: the base over the gain
/// plus the motivation (at least [`DRILL_MOTIVATION`]).
pub fn drill_countdown(equipment: u8, gain: u16, motivation: u8) -> u16 {
    drill_base(equipment).0 / (gain + motivation.max(DRILL_MOTIVATION) as u16)
}

/// The exercise at a place with bit 4 (an alert): word 0Eh counts down
/// from FF40h - skill 1, or on from its value when its high byte is FFh;
/// the new word, and whether the exercise is over (its low byte less 1
/// has bit 7: then word 0Eh 0 and the place's bit 4 cleared).
pub fn exercise(word_0e: u16, skill: u8) -> (u16, bool) {
    let ax = if word_0e >> 8 == 0xff { word_0e } else { 0xff00 | 0x40u8.wrapping_sub(skill) as u16 };
    let low = (ax as u8).wrapping_sub(1);
    (ax & 0xff00 | low as u16, low & 0x80 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mining() {
        assert_eq!(harvest_slot(0x200, 0x1234, 0x14), Some(true));
        assert_eq!(harvest_slot(0x200, 0x1235, 0x14), Some(false));
        assert_eq!(harvest_slot(0, 0, 0), None);
        assert_eq!(tally(0x70, 0x20), (0x90, true));
        assert_eq!(tally(0x10, 0x20), (0x30, false));
        assert_eq!(in_tenths(25, 7), (3, 2));
        let t = taken(30, 2, 8, 0x13).unwrap();
        assert_eq!((t.spice_units, t.left, t.spice, t.more_than_low), (4, 0, 0x0f, true));
        assert_eq!(taken(30, 0, 0, 1), None);
        assert_eq!(taken(100, 0, 1, 0).map(|t| t.spice), Some(0));
    }

    #[test]
    fn spying() {
        assert_eq!(spy_times(0x10), (0x20, 0x10));
        // A skill above 50h: negative times.
        assert_eq!(spy_times(0x70), (0xfff0, 0xfff8));
        assert!(past(0xfff0, 0) && !past(5, 5));
        assert_eq!(spy_report(100, 7), Some(14));
        assert_eq!(spy_found_out(0x10, 8, || 0), None);
        assert_eq!(spy_found_out(0x10, 9, || 0x3f), Some(true));
        assert_eq!(spy_found_out(0x10, 9, || 0x40), Some(false));
    }

    #[test]
    fn ecology_jobs() {
        assert_eq!(bulb_count(0xff), (0, true));
        assert_eq!(assembled(0xf0, 0x20), (0x10, true));
        assert_eq!(irrigation_step(3), 1);
        assert_eq!(irrigation_step(0x20), 8);
        assert_eq!(irrigated(0x0b, 3, 0), Irrigated::DriedUp);
        assert_eq!(irrigated(0x20, 0x0c, 0x10), Irrigated::Grows { water: 0x14, radius: 0x0c, byte_0e: Some(0x0e), ax: 0x0e });
        assert!(matches!(irrigated(0x20, 3, 0xae), Irrigated::Grows { byte_0e: None, .. }));
    }

    #[test]
    fn prospecting_and_training() {
        assert_eq!(prospect_rate(8, 0x30, 0x10), Some(2));
        assert_eq!(prospect_rate(8, 0, 0), None);
        assert_eq!(prospect_share(40, 10), Some(25));
        assert_eq!(prospect_share(40, 40), None);
        assert_eq!(drill_gain(0x30, 0x10), 0x40);
        assert_eq!(drill_gain(0x10, 0x30), 0);
        assert_eq!(drill_countdown(0x04, 0, 0x10), 0xc8 / 0x1e);
        assert_eq!(exercise(0x0000, 0x10), (0xff2f, false));
        assert_eq!(exercise(0xff01, 0x10), (0xff00, false));
        assert_eq!(exercise(0xff00, 0x10), (0xffff, true));
    }
}
