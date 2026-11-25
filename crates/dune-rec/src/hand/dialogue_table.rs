//! The dialogue table, as Rust state (Stage B of
//! `docs/readable-port-plan.md`).
//!
//! Resource BDh, loaded at start-up to DS:AA76h (`initialize_resources`,
//! 11F8h bytes; a save keeps them whole): a word per list number (the
//! phrases, the speakers' dialogue lists, by 8 · speaker + n), then the
//! lists' 4-byte entries from AA78h on (a flags byte, the condition's id,
//! the phrase; FFFFh ends a list). The state the game changes is in the
//! flags: bit 7 once the entry was said (`say_dialogue_entry`), so a line
//! is said once.
//!
//! [`DialogueTable`] holds the bytes and owns their memory
//! (`pc::hw::Owned`, a direct array for the bus), registered with the
//! record tables: the load at start-up and a save's restore write into it
//! through the bus, as every reader reads from it.

use super::globals as g;
use pc::Machine;

/// Where the table starts in the data segment.
pub const AT: u16 = 0xaa76;
/// Its size (the save's block).
pub const LEN: usize = 0x11f8;
/// An entry's flags: said.
pub const SAID: u8 = 0x80;

const BASE: u32 = ((g::GAME_DS as u32) << 4) + AT as u32;

/// The dialogue table's bytes, owning their memory.
pub struct DialogueTable {
    bytes: Box<[u8]>,
}

impl DialogueTable {
    /// The byte at DS:`at` (within the table).
    pub fn byte(&self, at: u16) -> u8 {
        self.bytes[(at - AT) as usize]
    }

    /// The word at DS:`at` (within the table).
    pub fn word(&self, at: u16) -> u16 {
        self.byte(at) as u16 | (self.byte(at + 1) as u16) << 8
    }

    /// List `n`'s offset (its word in the table's head).
    pub fn list(&self, n: u16) -> u16 {
        self.word(AT + 2 * n)
    }

    /// The entries of list `n` that were said, and all of them (a list
    /// outside the table counts none).
    pub fn said_in(&self, n: u16) -> (usize, usize) {
        let (mut said, mut all) = (0, 0);
        let mut e = self.list(n);
        while e >= AT && (e as usize) + 4 <= AT as usize + LEN && self.word(e) != 0xffff {
            all += 1;
            said += (self.byte(e) & SAID != 0) as usize;
            e += 4;
        }
        (said, all)
    }

    /// The lists: the head's words up to the first list's start.
    pub fn lists(&self) -> u16 {
        let mut first = AT + LEN as u16;
        let mut n = 0;
        while AT + 2 * n < first {
            let l = self.list(n);
            if l > AT + 2 * n && l < first {
                first = l;
            }
            n += 1;
        }
        n
    }

    /// The entries said and all, over every list (an entry in two lists
    /// counted once).
    pub fn said(&self) -> (usize, usize) {
        let mut seen = std::collections::BTreeMap::new();
        for n in 0..self.lists() {
            let mut e = self.list(n);
            while e >= AT && (e as usize) + 4 <= AT as usize + LEN && self.word(e) != 0xffff {
                seen.insert(e, self.byte(e) & SAID != 0);
                e += 4;
            }
        }
        (seen.values().filter(|&&s| s).count(), seen.len())
    }
}

impl pc::hw::Owned for DialogueTable {
    fn ranges(&self) -> Vec<(u32, u32)> {
        vec![(BASE, LEN as u32)]
    }
    fn read(&self, addr: u32) -> u8 {
        self.bytes[(addr - BASE) as usize]
    }
    fn write(&mut self, addr: u32, v: u8) {
        self.bytes[(addr - BASE) as usize] = v;
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

/// The table registered with the bus, unless it is.
pub(super) fn own(m: &mut Machine) {
    if m.hw.owned::<DialogueTable>().is_none() {
        let _ = m.hw.own(Box::new(DialogueTable { bytes: vec![0; LEN].into_boxed_slice() }));
    }
}

/// The table, if owned yet.
pub fn get(m: &Machine) -> Option<&DialogueTable> {
    m.hw.owned::<DialogueTable>()
}
