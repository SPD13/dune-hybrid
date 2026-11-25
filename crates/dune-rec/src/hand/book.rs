//! The book (the dialogue lines noted), as Rust state (Stage B of
//! `docs/readable-port-plan.md`).
//!
//! The lines worth keeping (a dialogue entry whose action has bits 0Ch,
//! `say_dialogue_entry`; ten more from DS:242Ah the first time Ctrl-V is
//! pressed, `handle_ctrl_v_once`) are noted in a list of words in the code
//! segment, from CS:00AAh: the space of the start-up code
//! (`initialize_resources`' calls at 00B0h), which runs once and is then
//! free. Each word: the entry's number (from the dialogue table's entries,
//! AA78h, 4 bytes each), the speaker in bits 11..15; 0 ends the list.
//! [`g::CTRL_V_TABLE_END`] is the next free word, [`g::BOOK_POSITION`] the
//! book's page in it. A save keeps the A2h bytes (81 words) whole.
//!
//! [`Book`] holds the bytes and owns their memory (`pc::hw::Owned`, a
//! direct array for the bus). It is registered where the start-up code is
//! surely past (a line noted, Ctrl-V, a save made or restored), so that
//! code is never read from it.

use super::Ctx;
use pc::Machine;

/// Where the list starts in the code segment.
pub const AT: u16 = 0x00aa;
/// Its size (the save's block).
pub const LEN: usize = 0xa2;

/// The bytes, owning their memory.
pub struct Book {
    base: u32,
    bytes: Box<[u8]>,
}

impl Book {
    /// The lines noted: (the entry's number, the speaker), up to the
    /// ending 0.
    pub fn lines(&self) -> Vec<(u16, u8)> {
        self.bytes
            .chunks(2)
            .map(|w| w[0] as u16 | (w[1] as u16) << 8)
            .take_while(|&w| w != 0)
            .map(|w| (w & 0x7ff, (w >> 11) as u8))
            .collect()
    }

    /// The segment it is in.
    pub fn segment(&self) -> u16 {
        (self.base >> 4) as u16
    }
}

impl pc::hw::Owned for Book {
    fn ranges(&self) -> Vec<(u32, u32)> {
        vec![(self.base, LEN as u32)]
    }
    fn read(&self, addr: u32) -> u8 {
        self.bytes[(addr - self.base) as usize]
    }
    fn write(&mut self, addr: u32, v: u8) {
        self.bytes[(addr - self.base) as usize] = v;
    }
    fn direct(&mut self) -> Option<&mut [u8]> {
        Some(&mut self.bytes)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The book registered with the bus (at the caller's CS:00AAh, the
/// game's code segment), unless it is. Only past the start-up code.
pub(super) fn own(c: &mut Ctx) {
    if c.m.hw.owned::<Book>().is_some() {
        return;
    }
    let base = c.m.cpu.phys(pc::LOAD_SEG, AT);
    let _ = c.m.hw.own(Box::new(Book { base, bytes: vec![0; LEN].into_boxed_slice() }));
}

/// The book, if owned yet.
pub fn get(m: &Machine) -> Option<&Book> {
    m.hw.owned::<Book>()
}
