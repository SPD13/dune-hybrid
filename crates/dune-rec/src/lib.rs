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
//!
//! `src/generated_{vga,music,sound}.rs`, when present, are the drivers the
//! game loads, lifted from their segments (`dune-run --dump-drivers`, then
//! `dune-lift --raw`): game-derived and git-ignored too.

use std::sync::OnceLock;

pub use pc::rec::{Bail, Routine};

#[cfg(has_generated)]
#[allow(clippy::all, unused_mut, unused_variables, unused_imports, unused_parens, unreachable_code, non_snake_case)]
mod generated;

pub mod hand;

/// A lifted driver: what [`pc::rec::Rec::with_driver`] takes.
pub struct Driver {
    pub name: &'static str,
    pub routines: &'static [(u16, Routine)],
    pub leaders: &'static [(u16, Routine)],
    pub names: &'static [(u16, &'static str)],
    /// The code bytes it was lifted from (ranges, FNV-1a hash).
    pub code: (&'static [(u16, u16)], u64),
    /// `DS:` offset of the word holding its segment (the segment of its
    /// table's first far pointer, as seg000:e5e5, e7ce, e775 load them).
    pub seg_at: u16,
}

macro_rules! driver {
    ($cfg:ident, $module:ident, $name:literal, $seg_at:literal) => {
        #[cfg($cfg)]
        #[allow(clippy::all, dead_code, unused_mut, unused_variables, unused_imports, unused_parens, unreachable_code, non_snake_case)]
        mod $module;
        #[cfg($cfg)]
        fn $module() -> Option<Driver> {
            Some(Driver {
                name: $name,
                routines: $module::ROUTINES,
                leaders: $module::LEADERS,
                names: $module::NAMES,
                code: $module::CODE,
                seg_at: $seg_at,
            })
        }
        #[cfg(not($cfg))]
        fn $module() -> Option<Driver> {
            None
        }
    };
}

driver!(has_generated_vga, generated_vga, "graphics", 0x38b7);
driver!(has_generated_music, generated_music, "music", 0x396f);
driver!(has_generated_sound, generated_sound, "sound", 0x398b);

/// The lifted drivers present (`src/generated_{vga,music,sound}.rs`), in
/// the order of their code spaces (1, 2, ...).
pub fn drivers() -> Vec<Driver> {
    [generated_vga(), generated_music(), generated_sound()].into_iter().flatten().collect()
}

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

/// The hand-written ports in use: all of them, none with `REC_NO_HAND=1`
/// in the environment, or all but the entries in `REC_HAND_SKIP` (hex,
/// comma-separated): to tell whether a difference comes from one.
fn hand_ports() -> &'static [(u16, Routine, &'static str)] {
    static PORTS: OnceLock<&'static [(u16, Routine, &'static str)]> = OnceLock::new();
    PORTS.get_or_init(|| {
        if std::env::var_os("REC_NO_HAND").is_some_and(|v| v == "1") {
            return &[];
        }
        let Ok(skip) = std::env::var("REC_HAND_SKIP") else { return hand::ROUTINES };
        let skip: Vec<u16> = skip.split(',').filter_map(|s| u16::from_str_radix(s.trim(), 16).ok()).collect();
        Vec::leak(hand::ROUTINES.iter().copied().filter(|r| !skip.contains(&r.0)).collect())
    })
}

/// Every routine by seg000 entry offset: the hand-written ports, and the
/// generated routines for the rest.
pub fn routines() -> &'static [(u16, Routine)] {
    static ALL: OnceLock<Vec<(u16, Routine)>> = OnceLock::new();
    ALL.get_or_init(|| {
        let mut all: Vec<(u16, Routine)> =
            gen_tables::routines().iter().filter(|(e, _)| !hand_ports().iter().any(|h| h.0 == *e)).copied().collect();
        all.extend(hand_ports().iter().map(|&(e, f, _)| (e, f)));
        all.sort_by_key(|r| r.0);
        all
    })
}

/// Every block leader of the generated code, with a routine to resume in
/// (a hand port's for [`hand::RESUMES`], unless `REC_NO_HAND=1`).
pub fn leaders() -> &'static [(u16, Routine)] {
    static ALL: OnceLock<Vec<(u16, Routine)>> = OnceLock::new();
    ALL.get_or_init(|| {
        let hand = std::env::var_os("REC_NO_HAND").is_none_or(|v| v != "1");
        gen_tables::leaders()
            .iter()
            .map(|&(e, f)| match hand::RESUMES.iter().find(|r| r.0 == e) {
                Some(&(_, h, _)) if hand => (e, h),
                _ => (e, f),
            })
            .collect()
    })
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
    hand_ports().iter().any(|h| h.0 == entry)
}
