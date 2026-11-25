//! The save image from the Rust state (Stage B of
//! `docs/readable-port-plan.md`).
//!
//! `create_save_in_memory` builds the image the save file is made from;
//! its last block is the story's part of the data segment, DS:0-1260h:
//! the game time and the clock's words there, the locations, the troops,
//! the characters, the smugglers and the story state's tables. That block
//! comes from the subsystems that own their memory ([`super::clock`], the
//! tables of [`super::record_table`], the events): read on the bus, which
//! serves their bytes from them, and the memory for the bytes no
//! subsystem owns yet. The bytes where the memory's own copy differs are
//! counted (none, but with `OWN_POISON`, where it is CCh).

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
    /// The map's block in the last image: its bytes (0 with the cells not
    /// owned), and in all the bytes where it differed from the cells'
    /// packing ([`super::map_cells::MapCells::packed`]).
    map_bytes: u64,
    map_differing: u64,
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

/// The map's block, and where it differed from the Rust state's packing.
pub fn map_counts(m: &mut Machine) -> (u64, u64) {
    let s = state(m);
    (s.map_bytes, s.map_differing)
}

/// The map's block as written at `seg:at` (the cells read on the bus,
/// so from the Rust state when they own their memory) checked against
/// the cells' own packing.
pub(super) fn check_map_part(c: &mut Ctx, seg: u16, at: u16) {
    let Some(packed) = super::map_cells::get(c.m).map(|mc| mc.packed()) else { return };
    let differing = (0..packed.len())
        .filter(|&k| c.m.hw.peek(c.m.cpu.phys(seg, at.wrapping_add(k as u16))) != packed[k])
        .count();
    let s = state(c.m);
    s.map_bytes = packed.len() as u64;
    s.map_differing += differing as u64;
}

/// DS:0 on, `len` bytes, as the save image has them (see the module's
/// notes).
pub(super) fn story_part(c: &mut Ctx, len: u16) -> Vec<u8> {
    // On the bus: the owners' bytes where the subsystems own the memory.
    let img: Vec<u8> = (0..len)
        .map(|k| {
            let a = c.m.cpu.phys(g::GAME_DS, k);
            c.m.hw.read8(a)
        })
        .collect();
    let owned = (0..len).filter(|&k| c.m.hw.is_owned(c.m.cpu.phys(g::GAME_DS, k))).count();
    // The memory's own copy (written through; CCh with OWN_POISON).
    let differing = (0..len).filter(|&k| img[k as usize] != c.m.hw.mem[c.m.cpu.phys(g::GAME_DS, k) as usize]).count();
    if differing > 0 && std::env::var_os("SAVE_TRACE").is_some() {
        let at: Vec<String> = (0..len as usize)
            .filter(|&k| img[k] != c.m.hw.mem[c.m.cpu.phys(g::GAME_DS, k as u16) as usize])
            .take(20)
            .map(|k| format!("{k:04x}:{:02x}", img[k]))
            .collect();
        eprintln!("save image: {differing} bytes differ from the memory's copy: {}", at.join(" "));
    }
    let s = state(c.m);
    s.saves += 1;
    s.owned = owned as u64;
    s.differing += differing as u64;
    img
}
