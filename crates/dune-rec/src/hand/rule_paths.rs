//! Which paths of the rules (`ecology`, `troop_day`, `troop_work`,
//! `spice_accounts`, the clock's time step) a run took: a count per named
//! path, kept with the machine and reported by the runner (`rule paths:`),
//! so a path no game reached shows as 0 (covered then only by the rules'
//! unit tests).

use pc::Machine;
use std::collections::BTreeMap;

/// Every path counted, so the report lists those never taken too.
pub const PATHS: &[&str] = &[
    "ecology: day grown",
    "ecology: cell grown",
    "ecology: wind-trap watered",
    "ecology: wind-trap full",
    "troop day: raid over",
    "troop day: raid going on",
    "troop day: restless",
    "troop day: idle report",
    "time: stopped",
    "time: new day",
    "time: fatigue to 0",
    "motivation: capped",
    "motivation: irrigation",
    "motivation: attack at home",
    "motivation: late floor",
    "motivation: late penalty",
    "rate: unarmed",
    "rate: up",
    "rate: down",
    "income: negative",
    "income: fell",
    "income: rose",
    "income: raised",
    "attack: not yet",
    "attack: other phase",
    "attack: player at the palace",
    "attack: due",
    "attack: target picked",
    "target: kind",
    "target: undiscovered",
    "target: spared",
    "take: late",
    "take: before",
    "take: too soon",
    "take: waited",
    "take: quiet day",
    "take: drawn",
    "take: not drawn",
    "raid: garrison 1",
    "raid: garrison 0",
];

/// The counts.
#[derive(Default)]
pub struct RulePaths {
    pub hits: BTreeMap<&'static str, u64>,
}

/// Path `name` taken (when `taken`).
pub(super) fn hit(m: &mut Machine, name: &'static str, taken: bool) {
    debug_assert!(PATHS.contains(&name), "{name}");
    if taken {
        *super::native::native(m).rule_paths.hits.entry(name).or_default() += 1;
    }
}

/// Path `name` taken `n` times.
pub(super) fn hit_n(m: &mut Machine, name: &'static str, n: u64) {
    debug_assert!(PATHS.contains(&name), "{name}");
    if n > 0 {
        *super::native::native(m).rule_paths.hits.entry(name).or_default() += n;
    }
}

/// Every path and its count, in [`PATHS`]' order.
pub fn counts(m: &mut Machine) -> Vec<(&'static str, u64)> {
    let h = &super::native::native(m).rule_paths.hits;
    PATHS.iter().map(|&p| (p, h.get(p).copied().unwrap_or(0))).collect()
}
