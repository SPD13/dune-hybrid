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
    "restock: refilled",
    "visit: first",
    "visit: again",
    "deal: offered",
    "deal: nothing left",
    "deal: bought",
    "deal: new debt",
    "refusal: offended",
    "refusal: dropped",
    "haggle: offended",
    "haggle: out of patience",
    "haggle: lowered",
    "talk end: offer taken",
    "talk end: debt paid",
    "talk end: refused",
    "talk end: haggled",
    "tick: waits",
    "tick: merged",
    "tick: skipped",
    "tick: travels",
    "tick: works",
    "move: fast",
    "step: arrived",
    "step: along x",
    "step: along y",
    "step: straight down",
    "step: seed turns",
    "arrival: discovered",
    "arrival: meets",
    "arrival: tells",
    "arrival: not found",
    "arrival: figure dropped",
    "arrival: on to the next place",
    "search: last place",
    "search: equipment taken",
    "search: none spare",
    "set out: retargeted",
    "set out: leaves",
    "set out: place abandoned",
    "set out: attacker stays",
    "set out: attacker loses motivation",
    "job: given",
    "job: bulb growing instead",
    "job: activity marked",
    "meeting: away",
    "meeting: raid",
    "meeting: sietch freed",
    "harvest: off its slot",
    "harvest: its slot",
    "harvest: held",
    "harvest: skill point",
    "harvest: place emptied",
    "spy: due",
    "spy: report",
    "spy: found out",
    "spy: unseen",
    "bulbs: grown",
    "wind-trap: built",
    "irrigation: place started",
    "irrigation: dried up",
    "irrigation: grows",
    "prospect: rate set",
    "prospect: under way",
    "prospect: found",
    "prospect: sets out",
    "training: exercise",
    "training: exercise over",
    "training: countdown",
    "training: new drill",
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
