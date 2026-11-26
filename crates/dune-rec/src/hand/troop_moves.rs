//! The troops' movement and job changes, as rules over the Rust state
//! (Stage B of `docs/readable-port-plan.md`: subsystem logic in Rust).
//!
//! Each tick `troops_tick` (6C6Fh) walks the troops: a troop waits, works
//! at its job (the job handlers) or travels ([`tick_action`]); a
//! travelling one takes [`steps_per_tick`] steps (`troop_move`, 8308h),
//! each [`step`] toward its location (`troop_step`, 8604h); a still step
//! is its arrival ([`arrival`], [`not_found`]; `troop_arrived`, 8357h), where a troop
//! searching for equipment goes on to its next place ([`at_destination`]). Setting out
//! (`troop_set_out`, 84A6h) and a job given (`set_troop_job`, 6AD4h) are
//! decided here too. The ports in `hand/troops.rs` stay the original's
//! 8086 frame around these pure functions.

use super::game::{location_flag, troop_job, troop_state};

/// Bits of a troop's word 12h that make it wait its turn: 10h, 20h
/// (cleared when the tick goes round again) and 400h (held).
pub const WAITING: u16 = 0x430;
/// Of them, held: it skips the tick even when it goes round again.
pub const HELD: u16 = 0x400;
/// A troop this size or bigger is never merged into another.
pub const MERGE_BELOW: u8 = 0x14;

/// What a troop does in a tick, by its state (byte 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TickAction {
    /// Not a troop, or just started: nothing.
    Skip,
    /// On its way: it moves.
    Travel,
    /// At its job (the low four bits): the job's handler.
    Work(u8),
}

pub fn tick_action(state: u8) -> TickAction {
    if state & (troop_state::NOT_A_TROOP | troop_state::STARTED) != 0 {
        TickAction::Skip
    } else if state & troop_state::TRAVELLING != 0 {
        TickAction::Travel
    } else {
        TickAction::Work(state & 0x0f)
    }
}

/// Whether a troop waits its turn (`word_12`), and whether a small one
/// (`size` below [`MERGE_BELOW`]) is first offered to merge.
pub fn waits(word_12: u16) -> bool {
    word_12 & WAITING != 0
}

pub fn may_merge(size: u8) -> bool {
    size < MERGE_BELOW
}

/// Equipment bit 40h (the ornithopter) doubles a troop's steps.
pub const FAST: u8 = 0x40;

/// The steps a travelling troop takes a tick: 8 with [`FAST`], else 4.
pub fn steps_per_tick(equipment: u8) -> u16 {
    if equipment & FAST != 0 { 8 } else { 4 }
}

/// Closer than this (in steps along the larger axis) a troop has arrived.
pub const CLOSE: u16 = 7;

/// How a step was found: along x (the x distance in scale units at least
/// the y one) or y, and whether the larger distance halved to 0 (no step
/// across then). `Close`: arrived. `StraightDown`: along y with no x
/// distance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Along {
    Close { by_x: bool },
    X { half_zero: bool },
    Y { half_zero: bool },
    StraightDown,
}

/// A step toward a place: the x step (`dx`, ± the scale) and the y step
/// (`dy`, ±1), both 0 when close; the step along the smaller axis comes
/// only when the seed's top bit (`carry`) is set, and the seed turns
/// (rotated left) whenever the larger distance halves to more than 0.
/// `ax`, `di`: the distances as the original leaves them in AX and DI (the
/// larger halved), `bp` its BP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub dx: u16,
    pub dy: u16,
    pub ax: u16,
    pub di: u16,
    pub bp: u16,
    pub turns: bool,
    pub along: Along,
}

impl Step {
    pub fn still(&self) -> bool {
        self.dx | self.dy == 0
    }
}

fn neg(v: u16) -> bool {
    v & 0x8000 != 0
}

fn abs(v: u16) -> u16 {
    if neg(v) { v.wrapping_neg() } else { v }
}

/// The step from a distance (`dx`, `dy`: the place's position less the
/// troop's) with the map's x `scale` at the troop's row (not 0) and the
/// seed's top bit `carry`.
pub fn step(dx: u16, dy: u16, scale: u16, carry: bool) -> Step {
    let y_dir: u16 = if neg(dy) { 0xffff } else { 1 };
    let across = abs(dx) / scale;
    let down = abs(dy);
    let x_step = if neg(dx) { scale.wrapping_neg() } else { scale };
    let mut s = Step { dx, dy: y_dir, ax: across, di: down, bp: scale, turns: false, along: Along::StraightDown };
    if across >= down {
        if across < CLOSE {
            (s.dx, s.dy, s.along) = (0, 0, Along::Close { by_x: true });
        } else {
            if down == 0 {
                s.dy = 0;
            }
            s.bp = x_step;
            s.ax = across >> 1;
            s.along = Along::X { half_zero: s.ax == 0 };
            if s.ax != 0 {
                s.dx = x_step;
                s.turns = true;
                if !carry {
                    s.dy = 0;
                }
            }
        }
    } else if down < CLOSE {
        (s.dx, s.dy, s.along) = (0, 0, Along::Close { by_x: false });
    } else if across != 0 {
        s.bp = x_step;
        s.di = down >> 1;
        s.along = Along::Y { half_zero: s.di == 0 };
        if s.di != 0 {
            s.dx = if carry { x_step } else { 0 };
            s.turns = true;
        }
    }
    s
}

/// Whether an arriving troop has found its place: one the Harkonnens
/// hold always, else unless `loc_05d36` skips it (`skipped`, asked only
/// then).
pub fn place_found(harkonnen: bool, skipped: impl FnOnce() -> bool) -> bool {
    harkonnen || !skipped()
}

/// What an arrival at a found place leads to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arrival {
    /// Spying, at a place not found before: the place shown.
    Discovered,
    /// Meeting the place's own troop (its byte 9).
    Meets,
    /// The troops there told.
    Tell,
}

/// The arrival of a troop at `job` (AL as the original keeps it: the
/// state's low four bits) at a found place of `flags` whose first troop
/// is it (`meets`).
pub fn arrival(job: u8, flags: u8, meets: bool) -> Arrival {
    if job == troop_job::ESPIONAGE {
        if flags & location_flag::UNDISCOVERED != 0 { Arrival::Discovered } else { Arrival::Tell }
    } else if meets {
        Arrival::Meets
    } else {
        Arrival::Tell
    }
}

/// An arrival where the place is not found: a spy or an attacking troop
/// (jobs 5, 6: `job`, AL then) drops its figure (AL 0 after it); then one
/// searching for equipment (jobs 3, 7, 0Bh: AL's bits 3) goes on to its
/// next place, others are settled.
pub fn not_found(job: u8) -> (bool, bool) {
    let drops = (5..=6).contains(&job);
    let al = if drops { 0 } else { job };
    (drops, al & 3 == 3)
}

/// Searching for equipment: at its last place already (`place` its next,
/// word 0Ch)?
pub fn at_destination(place: u16, next: u16) -> bool {
    place == next
}

/// The equipment a troop searching for it takes on leaving a place: its kind
/// (word 0Eh's low byte) when the old place still has one spare, its bit
/// (word 0Eh's high byte) into the troop's equipment; word 0Eh keeps the
/// kind only then (`None`: nothing taken, word 0Eh kept).
pub fn take_carried(carried: u16, spare: bool, equipment: u8) -> Option<(u8, u16)> {
    spare.then(|| (equipment | (carried >> 8) as u8, carried & 0xff))
}

/// Setting out: already on its way, the troop only takes the new target
/// (a search for equipment, the state's bits 3, ends: `move_ends`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetOut {
    Retarget { move_ends: bool },
    Leaves,
}

pub fn set_out(state: u8) -> SetOut {
    if state & troop_state::TRAVELLING != 0 {
        SetOut::Retarget { move_ends: state & 3 == 3 }
    } else {
        SetOut::Leaves
    }
}

/// An attacking troop (state exactly 6) leaving a place the Harkonnens hold
/// leaves it to them when it was the last of the player's troops there
/// (`count` the troops there, `ours` the player's, it included: the
/// original tests `ours - 1` signed not above 0).
pub fn abandons(count: u16, ours: u16) -> bool {
    count != 0 && ours.wrapping_sub(1) as i16 <= 0
}

/// Below this kind a place costs an attacking troop motivation to go to.
pub const NEAR_KIND: u8 = 0x28;

/// An attacking troop's going to a place of `kind` and `flags`: it stays where
/// it is when the place is below [`NEAR_KIND`] and held by the
/// Harkonnens; else it goes, losing motivation for one below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackerGoes {
    Stays,
    LosesMotivation,
    Goes,
}

pub fn attacker_goes(kind: u8, flags: u8) -> AttackerGoes {
    if kind >= NEAR_KIND {
        AttackerGoes::Goes
    } else if flags & location_flag::HARKONNEN != 0 {
        AttackerGoes::Stays
    } else {
        AttackerGoes::LosesMotivation
    }
}

/// The motivation an attacking troop loses for a near place.
pub const NEAR_MOTIVATION_LOSS: u8 = 3;

/// The steps a troop sets out with (unless it waits, bit 10h of byte 10h).
pub const FIRST_STEPS: u16 = 7;

/// At this place irrigation (job 8) is bulb growing (0Ah) while the place's
/// byte 1Ah is 0.
pub const SPECIAL_PLACE: u16 = 0x07c8;
pub const SPECIAL_JOB: u8 = troop_job::BULB_GROWING;

/// The job a troop takes when given `job` at `place` (its byte 1Ah:
/// `byte_1a`, read only at the [`SPECIAL_PLACE`]).
pub fn job_given(job: u8, place: u16, byte_1a: impl FnOnce() -> u8) -> u8 {
    if job == troop_job::IRRIGATION && place == SPECIAL_PLACE && byte_1a() == 0 { SPECIAL_JOB } else { job }
}

/// A job other than 2 marks the troop's activity in its word 12h: 2000h
/// shifted left by it (as the original's `shl ax, cl`: the count's low
/// five bits, a word).
pub fn activity_mark(state: u8, activity: u8) -> Option<u16> {
    (state != 2).then(|| (0x2000u32 << (activity & 0x1f)) as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks() {
        assert_eq!(tick_action(troop_state::TRAVELLING | 3), TickAction::Travel);
        assert_eq!(tick_action(troop_state::STARTED), TickAction::Skip);
        assert_eq!(tick_action(5), TickAction::Work(5));
        assert!(waits(0x400) && !waits(0x40));
        assert_eq!(steps_per_tick(0x41), 8);
        assert_eq!(steps_per_tick(0x01), 4);
    }

    #[test]
    fn steps() {
        // Far along x, the seed's bit clear: x only.
        let s = step(0x0100, 0x0010, 4, false);
        assert_eq!((s.dx, s.dy, s.turns), (4, 0, true));
        // With it: both.
        let s = step(0xff00, 0x0010, 4, true);
        assert_eq!((s.dx, s.dy), (0xfffc, 1));
        // Close.
        assert!(step(8, 3, 4, true).still());
        // Along y, no x distance: y only, the seed kept.
        let s = step(0, 0x40, 4, true);
        assert_eq!((s.dx, s.dy, s.turns, s.along), (0, 1, false, Along::StraightDown));
    }

    #[test]
    fn arrivals() {
        assert_eq!(arrival(troop_job::ESPIONAGE, location_flag::UNDISCOVERED, false), Arrival::Discovered);
        assert_eq!(arrival(troop_job::ESPIONAGE, 0, true), Arrival::Tell);
        assert_eq!(arrival(1, 0, true), Arrival::Meets);
        assert_eq!(not_found(6), (true, false));
        assert_eq!(not_found(3), (false, true));
        assert!(place_found(true, || true) && !place_found(false, || true));
        assert_eq!(take_carried(0x0402, true, 0x01), Some((0x05, 2)));
        assert_eq!(take_carried(0x0402, false, 0x01), None);
    }

    #[test]
    fn setting_out_and_jobs() {
        assert_eq!(set_out(troop_state::TRAVELLING | 3), SetOut::Retarget { move_ends: true });
        assert_eq!(set_out(6), SetOut::Leaves);
        assert!(abandons(2, 1) && !abandons(2, 2) && !abandons(0, 0));
        assert_eq!(attacker_goes(0x30, location_flag::HARKONNEN), AttackerGoes::Goes);
        assert_eq!(attacker_goes(0x10, location_flag::HARKONNEN), AttackerGoes::Stays);
        assert_eq!(attacker_goes(0x10, 0), AttackerGoes::LosesMotivation);
        assert_eq!(job_given(troop_job::IRRIGATION, SPECIAL_PLACE, || 0), SPECIAL_JOB);
        assert_eq!(job_given(troop_job::IRRIGATION, SPECIAL_PLACE, || 1), troop_job::IRRIGATION);
        assert_eq!(activity_mark(2, 1), None);
        assert_eq!(activity_mark(4, 1), Some(0x4000));
        assert_eq!(activity_mark(4, 3), Some(0));
    }
}
