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

pub use pc::rec::{Bail, Routine};

#[cfg(has_generated)]
#[allow(clippy::all, unused_mut, unused_variables, unused_imports, unused_parens, unreachable_code, non_snake_case)]
mod generated;

/// Every recompiled routine, by seg000 entry offset (empty when the crate was
/// built without `src/generated.rs`).
pub fn routines() -> &'static [(u16, Routine)] {
    #[cfg(has_generated)]
    {
        generated::ROUTINES
    }
    #[cfg(not(has_generated))]
    {
        &[]
    }
}

/// Listing names of the routines, by entry offset.
pub fn names() -> &'static [(u16, &'static str)] {
    #[cfg(has_generated)]
    {
        generated::NAMES
    }
    #[cfg(not(has_generated))]
    {
        &[]
    }
}
