//! The save image from the Rust state (Stage B of
//! `docs/readable-port-plan.md`).
//!
//! `create_save_in_memory` builds the image the save file is made from;
//! its last block is the story's part of the data segment, DS:0-1260h:
//! the game time and the clock's words there, the locations, the troops,
//! the characters, the smugglers and the story state's tables. That block
//! now comes from the subsystems kept in Rust ([`super::clock`], the
//! cached records of [`super::record_table`]), laid over the memory for
//! the bytes no subsystem holds yet. The memory is their mirror, so the
//! two agree; where they don't, a byte is counted ([`differing`]): the
//! save then has the Rust state's.

use super::{Ctx, globals as g};
use pc::Machine;
use pc::cpu::Bus;

/// The counts kept with a machine ([`super::native`]).
#[derive(Default)]
pub struct SaveImageState {
    /// Images built.
    saves: u64,
    /// In the last one: bytes taken from the Rust state.
    owned: u64,
    /// In all: bytes where the Rust state and the memory differed.
    differing: u64,
}

fn state(m: &mut Machine) -> &mut SaveImageState {
    &mut super::native::native(m).save_image
}

/// Images built, bytes from the Rust state in the last one, and bytes
/// that differed from the memory in all.
pub fn counts(m: &mut Machine) -> (u64, u64, u64) {
    let s = state(m);
    (s.saves, s.owned, s.differing)
}

/// DS:0 on, `len` bytes, as the save image has them (see the module's
/// notes).
pub(super) fn story_part(c: &mut Ctx, len: u16) -> Vec<u8> {
    let mem: Vec<u8> = (0..len)
        .map(|k| {
            let a = c.m.cpu.phys(g::GAME_DS, k);
            c.m.hw.read8(a)
        })
        .collect();
    let mut img = mem.clone();
    let owned = super::record_table::lay_over(c.m, 0, &mut img) + super::clock::lay_over(c.m, 0, &mut img);
    let differing = img.iter().zip(&mem).filter(|(a, b)| a != b).count();
    if differing > 0 && std::env::var_os("SAVE_TRACE").is_some() {
        let at: Vec<String> = (0..len as usize).filter(|&k| img[k] != mem[k]).take(20).map(|k| format!("{k:04x}:{:02x}>{:02x}", mem[k], img[k])).collect();
        eprintln!("save image: {differing} bytes differ from the memory: {}", at.join(" "));
    }
    let s = state(c.m);
    s.saves += 1;
    s.owned = owned as u64;
    s.differing += differing as u64;
    img
}
