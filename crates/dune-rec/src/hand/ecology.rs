//! The planet's day-by-day change, as rules over the Rust state (Stage B
//! of `docs/readable-port-plan.md`: subsystem logic in Rust).
//!
//! Pure functions of the typed state (the map's cells, a location's
//! record), returning what changes and enough of how it was found for
//! the ports that call them (`grow_water`, `spread_spice`,
//! `count_10_bytes` in `hand/map.rs`) to stay the original's 8086 frame:
//! the outcome applied through the owners, the instruction count the
//! original takes on that path charged, the registers it leaves set.
//! Native runs can drop the frame and keep the rules.
//!
//! A cell's kind ([`super::map_cells::kind`]) is 0, 10h, 20h or 30h. Each
//! new day ([`g::DAY_CHANGED`](super::globals::DAY_CHANGED)): every
//! location with a wind-trap gains water by the cells of kind 10h around
//! it ([`wind_trap_water`]), then cells of kind 10h, picked by the game's
//! shift-register seed, become kind 20h ([`grow`]).

use super::map_cells::{KIND, kind};

/// The kind the wind-traps count and [`grow`] turns into [`GROWN`].
pub const SPREADING: u8 = 0x10;
/// What it becomes.
pub const GROWN: u8 = 0x20;
/// The most water a location holds (its byte 1Bh).
pub const MAX_WATER: u8 = 0xfa;
/// The cells [`grow`] reaches: below this.
pub const GROW_END: u16 = 0xc5f9;
/// [`grow`]'s stride from a picked cell to the next.
pub const GROW_STRIDE: u16 = 0x7ff;
/// [`grow`]'s seed steps a day.
pub const GROW_STEPS: u32 = 0x46;
/// The seed's taps (shifted out a 1: XORed in).
pub const SEED_TAPS: u16 = 0x402;

/// The cells of kind [`SPREADING`] among the six around `at` (from `at -
/// 1`, read as three words stepping by `step`, 2 or -2 with the
/// direction flag), by `cell` (an offset into the map's segment).
pub fn spreading_around(cell: impl Fn(u16) -> u8, at: u16, step: u16) -> u16 {
    let mut n = 0;
    let mut p = at.wrapping_sub(1);
    for _ in 0..3 {
        n += (kind(cell(p)) == SPREADING) as u16 + (kind(cell(p.wrapping_add(1))) == SPREADING) as u16;
        p = p.wrapping_add(step);
    }
    n
}

/// A wind-trap's water after a day, from `water` and the cells of kind
/// [`SPREADING`] around it: half of them + 1 more, up to [`MAX_WATER`]
/// (`None` when already there: unchanged).
pub fn wind_trap_water(water: u8, around: u16) -> Option<u8> {
    if water >= MAX_WATER {
        return None;
    }
    let v = water.wrapping_add(((around >> 1) + 1) as u8);
    Some(if v >= MAX_WATER { MAX_WATER } else { v })
}

/// What a day's growth did ([`grow`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Growth {
    /// The seed after.
    pub seed: u16,
    /// The cells that became [`GROWN`], in order, with their new byte (a
    /// cell reached twice is listed once, the second time sees it grown).
    pub grown: Vec<(u16, u8)>,
    /// Cells looked at, and seed steps that XORed the taps in.
    pub looked: u32,
    pub taps: u32,
    /// The last cell looked at (its byte, and its kind) and the offset
    /// after it: what the original leaves in AX and DI.
    pub last: u8,
    pub last_kind: u8,
    pub after: u16,
}

/// A day's growth from `seed`: [`GROW_STEPS`] steps of the seed (a
/// shift right, the taps XORed in when a 1 falls out); from each value,
/// the cells at it and every [`GROW_STRIDE`] after, below [`GROW_END`]:
/// those of kind [`SPREADING`] become [`GROWN`].
pub fn grow(cell: impl Fn(u16) -> u8, seed: u16) -> Growth {
    let mut g = Growth { seed, ..Growth::default() };
    let mut now = std::collections::HashMap::new();
    for _ in 0..GROW_STEPS {
        let out = g.seed & 1 != 0;
        g.seed >>= 1;
        if out {
            g.seed ^= SEED_TAPS;
            g.taps += 1;
        }
        let mut at = g.seed;
        loop {
            let mut b = now.get(&at).copied().unwrap_or_else(|| cell(at));
            let k = kind(b);
            g.looked += 1;
            if k == SPREADING {
                b = b & !KIND | GROWN;
                now.insert(at, b);
                g.grown.push((at, b));
            }
            (g.last, g.last_kind) = (b, k);
            at = at.wrapping_add(GROW_STRIDE);
            if at >= GROW_END {
                break;
            }
        }
        g.after = at;
    }
    g
}

/// The rules' counts, kept with the machine ([`super::native`]): the
/// days grown by the ports and the cells grown, the wind-traps watered.
#[derive(Default)]
pub struct EcologyState {
    pub days: u64,
    pub grown: u64,
    pub watered: u64,
}

/// The counts: days, cells grown, wind-traps watered.
pub fn counts(m: &mut pc::Machine) -> (u64, u64, u64) {
    let s = &super::native::native(m).ecology;
    (s.days, s.grown, s.watered)
}

pub(super) fn count(m: &mut pc::Machine, days: u64, grown: u64, watered: u64) {
    let s = &mut super::native::native(m).ecology;
    s.days += days;
    s.grown += grown;
    s.watered += watered;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water() {
        assert_eq!(wind_trap_water(0xfa, 6), None);
        assert_eq!(wind_trap_water(0x10, 6), Some(0x14));
        assert_eq!(wind_trap_water(0xf8, 6), Some(0xfa));
        assert_eq!(wind_trap_water(0, 0), Some(1));
    }

    #[test]
    fn around() {
        let cells = [0x10u8, 0x15, 0x20, 0x13, 0x30, 0x1f, 0x10, 0x00];
        // From 1: cells 0..6.
        assert_eq!(spreading_around(|a| cells[a as usize], 1, 2), 4);
    }

    #[test]
    fn growth() {
        // All of kind 10h: every cell looked at grows, once.
        let g = grow(|_| 0x15, 0x1234);
        assert!(g.grown.iter().all(|&(_, b)| b == 0x25));
        let mut at: Vec<u16> = g.grown.iter().map(|x| x.0).collect();
        let n = at.len();
        at.sort();
        at.dedup();
        assert_eq!(at.len(), n);
        assert!(g.looked as usize >= n);
    }
}
