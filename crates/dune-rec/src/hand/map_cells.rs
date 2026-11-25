//! The map's cells, as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! The planet's map is C5FCh cells, a byte each, at the start of its own
//! segment ([`g::MAP_GFX_SEGMENT`], [DD00h]:0; loaded from its resource by
//! `initialize_resources`, its drawing pointer [`g::MAP_GFX_DATA`] at the
//! equator, 62FCh in). A cell's bits 4..5 are its state the game changes
//! ([`kind`]: the wind-traps count the cells of kind 10h around them, a
//! cell of kind 30h starts the desert's countdown, `map_func` counts them
//! for the statistics at A2h/A4h) and all of it the save keeps, packed four
//! to a byte (`create_save_in_memory`, `restore_from_save_memory`); the
//! other bits are the drawing's.
//!
//! The cells own their memory ([`MapCells`], `pc::hw::Owned`), like the
//! record tables: every read and write of them on the bus (the globe's and
//! the map's drawing, the drivers', the spread, a save's restore) goes to
//! the struct. They are registered where the segment is surely the map's
//! (the game's start, a save made or restored, the statistics): the
//! zoomed globe repoints [DD00h] at the sprite sheet while it draws.

use super::Ctx;
use super::globals as g;
use pc::Machine;

/// The cells.
pub const CELLS: usize = 0xc5fc;

/// The bits of a cell the game changes (and the save keeps).
pub const KIND: u8 = 0x30;

/// A cell's state: 0, 10h, 20h or 30h.
pub const fn kind(cell: u8) -> u8 {
    cell & KIND
}

/// The cells, owning their memory.
pub struct MapCells {
    /// Linear address of the first.
    base: u32,
    cells: Box<[u8]>,
}

impl MapCells {
    /// Cell `i` (from 0, the map's start).
    pub fn cell(&self, i: usize) -> u8 {
        self.cells[i]
    }

    /// The segment they are at.
    pub fn segment(&self) -> u16 {
        (self.base >> 4) as u16
    }

    /// The cells' states packed as the save keeps them: four cells a
    /// byte, the first in the top two bits.
    pub fn packed(&self) -> Vec<u8> {
        self.cells
            .chunks(4)
            .map(|q| q.iter().fold(0u8, |b, &c| b << 2 | (kind(c) >> 4)))
            .collect()
    }
}

impl pc::hw::Owned for MapCells {
    fn ranges(&self) -> Vec<(u32, u32)> {
        vec![(self.base, CELLS as u32)]
    }
    fn read(&self, addr: u32) -> u8 {
        self.cells[(addr - self.base) as usize]
    }
    fn write(&mut self, addr: u32, v: u8) {
        self.cells[(addr - self.base) as usize] = v;
    }
    fn direct(&mut self) -> Option<&mut [u8]> {
        Some(&mut self.cells)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The cells registered with the bus at [DD00h]:0, unless they are.
/// Only where [DD00h] is the map's (not inside the zoomed globe).
/// (Read without the bus's side effects: whatever DS the caller has.)
pub(super) fn own(c: &mut Ctx) {
    own_m(c.m);
}

pub(super) fn own_m(m: &mut Machine) {
    if m.hw.owned::<MapCells>().is_some() {
        return;
    }
    let at = ((g::GAME_DS as u32) << 4) + g::MAP_GFX_SEGMENT as u32;
    let seg = m.hw.peek(at) as u16 | (m.hw.peek(at + 1) as u16) << 8;
    if seg == 0 {
        return;
    }
    let base = (seg as u32) << 4;
    let _ = m.hw.own(Box::new(MapCells { base, cells: vec![0; CELLS].into_boxed_slice() }));
}

/// The cells, if owned yet.
pub fn get(m: &Machine) -> Option<&MapCells> {
    m.hw.owned::<MapCells>()
}
