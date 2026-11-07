//! Recompiled routines of DNCDPRG.EXE (seg000), for [`pc::rec`].
//!
//! `src/generated.rs` is produced by `dune-lift` from your own copy of the
//! game and is game-derived: it is git-ignored and must not be shared.
//!
//! ```sh
//! cargo run --release -p lift -- --exe GAME/DNCDPRG.EXE \
//!     --lst ../Cryogenic/doc/DNCDPRG.lst --cov cov-*.txt \
//!     --out crates/dune-rec/src/generated.rs
//! ```
//!
//! [`hand`] holds hand-written, readable ports; each replaces the generated
//! routine with the same entry.

use std::sync::OnceLock;

pub use pc::rec::{Bail, Routine};

#[cfg(has_generated)]
#[allow(clippy::all, unused_mut, unused_variables, unused_imports, unused_parens, unreachable_code, non_snake_case)]
mod generated;

pub mod hand;

#[cfg(has_generated)]
mod gen_tables {
    pub fn routines() -> &'static [(u16, super::Routine)] {
        super::generated::ROUTINES
    }
    pub fn leaders() -> &'static [(u16, super::Routine)] {
        super::generated::LEADERS
    }
    pub fn names() -> &'static [(u16, &'static str)] {
        super::generated::NAMES
    }
}

#[cfg(not(has_generated))]
mod gen_tables {
    pub fn routines() -> &'static [(u16, super::Routine)] {
        &[]
    }
    pub fn leaders() -> &'static [(u16, super::Routine)] {
        &[]
    }
    pub fn names() -> &'static [(u16, &'static str)] {
        &[]
    }
}

/// Every routine by seg000 entry offset: the hand-written ports, and the
/// generated routines for the rest.
pub fn routines() -> &'static [(u16, Routine)] {
    static ALL: OnceLock<Vec<(u16, Routine)>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut all: Vec<(u16, Routine)> =
            gen_tables::routines().iter().filter(|(e, _)| !hand::ROUTINES.iter().any(|h| h.0 == *e)).copied().collect();
        all.extend(hand::ROUTINES.iter().map(|&(e, f, _)| (e, f)));
        all.sort_by_key(|r| r.0);
        all
    })
}

/// Every block leader of the generated code, with a routine to resume in.
pub fn leaders() -> &'static [(u16, Routine)] {
    gen_tables::leaders()
}

/// Listing names of the routines, by entry offset.
pub fn names() -> &'static [(u16, &'static str)] {
    static ALL: OnceLock<Vec<(u16, &'static str)>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut all = gen_tables::names().to_vec();
        for &(e, _, n) in hand::ROUTINES {
            if !all.iter().any(|r| r.0 == e) {
                all.push((e, n));
            }
        }
        all.sort_by_key(|r| r.0);
        all
    })
}

/// Whether the routine at `entry` is a hand-written port.
pub fn is_hand(entry: u16) -> bool {
    hand::ROUTINES.iter().any(|h| h.0 == entry)
}
