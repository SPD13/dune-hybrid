//! The simulator without the emulator (Stage C of
//! `docs/readable-port-plan.md`): time advanced by the native rules alone
//! on a [`World`], a save written from it.
//!
//! Each clock step is the clock's own (`Clock::advance`: the time one slot
//! on, a step waiting), then the time step's rules
//! ([`super::native_step::time_step`]), then `frames` passes of the main
//! loop, each drawing the random bits [0] from `rand` (D84Bh). The
//! original draws a varying number of times between steps (its frames
//! follow the display), so a simulated run is exact step by step (the
//! check `NATIVE_SIM` proves each step given the same random state), not
//! the original's long run byte for byte.
//!
//! The starting world comes from a machine (a save loaded in the
//! emulator): the save's blocks plus the game's tables the save lacks
//! (the map's terrain, the latitude and scale tables). Loading those from
//! the game's files is the next step.

use super::save_file::SaveImage;
use super::world::World;

/// What a simulated run did.
pub struct Run {
    pub world: World,
    /// Clock steps done.
    pub steps: u32,
    /// Why it stopped early (a case the rules don't model), if it did.
    pub stopped: Option<&'static str>,
}

/// `steps` clock steps from `w`, `frames` main-loop passes after each.
pub fn run(mut w: World, steps: u32, frames: u32) -> Run {
    for done in 0..steps {
        let t = w.time().wrapping_add(1);
        w.set_word(2, t);
        w.save.day = t;
        w.set_byte(0x46dd, 1);
        if super::native_step::time_step(&mut w).is_none() {
            return Run { world: w, steps: done, stopped: Some(super::world::last_why()) };
        }
        for _ in 0..frames {
            let r = w.rand();
            w.set_word(super::globals::RANDOM_BITS, r);
        }
    }
    Run { world: w, steps, stopped: None }
}

/// The world as a save's image (the map's states packed from its cells).
pub fn save_image(w: &World) -> SaveImage {
    SaveImage { day: w.time(), map: w.packed_map(), book: w.save.book.clone(), dialogue: w.save.dialogue.clone(), story: w.save.story.clone() }
}
