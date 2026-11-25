//! The events waiting to be told, as Rust state (Stage B of
//! `docs/readable-port-plan.md`).
//!
//! An event (a troop's report, a place found, an attack seen) waits in
//! this list until the character who tells it is present
//! (`tell_pending_event`, 2AD8h): a count, up to 10 (`add_pending_event`
//! drops the oldest for a new one, `drop_oldest_event`), then the
//! entries, oldest first, each the event's kind (1: told), its teller and
//! the location record where (0 for none; for a Fremen chief, character
//! 0Fh, it must be the player's place). The byte before the count is
//! cleared when the last one is dropped.
//!
//! The first subsystem that owns its memory (the pilot of Stage B's
//! ownership, `docs/readable-port-plan.md`): its 2Ah bytes at DS:118Fh
//! live only in [`Events`], the struct registered with the bus
//! ([`EventsMemory`], `pc::hw::Owned`). Every read and write of them, by
//! any code (the ports, the generated code, the interpreter, the string
//! instructions that move the entries), goes to it; the memory keeps a
//! copy written through for the tools (snapshots, state hashes, the
//! shadow check's compare), and where they replace the memory the struct
//! takes its bytes back from it.

use super::Ctx;
use super::globals as g;
use pc::cpu::Bus;

/// Where the events start in the data segment.
pub const AT: u16 = 0x118f;
/// Their size.
pub const LEN: usize = 0x2a;

/// The most events waiting.
pub const MAX_EVENTS: usize = 10;

/// An event waiting.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Event {
    /// Its kind (to [`g::EVENT_KIND`]); 1 once told.
    pub kind: u8,
    /// The character who tells it (to [`g::TALKER`]).
    pub teller: u8,
    /// The location record where (0 for none).
    pub place: u16,
}

impl Event {
    /// The kind and the teller as the word the code loads (AX: the kind
    /// in AL, the teller in AH).
    pub fn word(self) -> u16 {
        self.kind as u16 | (self.teller as u16) << 8
    }

    /// The event of that word and place.
    pub fn from_word(word: u16, place: u16) -> Event {
        Event { kind: word as u8, teller: (word >> 8) as u8, place }
    }
}

/// The events waiting.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Events {
    /// [`g::PENDING_EVENTS_FLAG`] (118Fh).
    pub flag: u8,
    /// [`g::PENDING_EVENTS`] (1190h): how many wait.
    pub count: u8,
    /// From [`g::NEXT_EVENT`] (1191h), the oldest first.
    pub list: [Event; MAX_EVENTS],
}

/// Event `i` waiting (from 0, the oldest). Past the tenth (a count the
/// game never makes) the words where the code would read it.
pub(super) fn entry(c: &mut Ctx, i: u16) -> Event {
    if (i as usize) < MAX_EVENTS {
        return c.events().list[i as usize];
    }
    let k = g::NEXT_EVENT + 4 * i;
    Event::from_word(c.word(k), c.word(k + 2))
}

/// A field's offset in the record.
const fn at(global: u16) -> usize {
    (global - AT) as usize
}

fn word(b: &[u8], i: usize) -> u16 {
    b[i] as u16 | (b[i + 1] as u16) << 8
}

fn set_word(b: &mut [u8], i: usize, v: u16) {
    b[i..i + 2].copy_from_slice(&v.to_le_bytes());
}

impl Events {
    /// The events as their bytes in the memory.
    pub fn from_bytes(b: &[u8]) -> Events {
        let mut list = [Event::default(); MAX_EVENTS];
        for (i, e) in list.iter_mut().enumerate() {
            let k = at(g::NEXT_EVENT) + 4 * i;
            *e = Event::from_word(word(b, k), word(b, k + 2));
        }
        Events { flag: b[at(g::PENDING_EVENTS_FLAG)], count: b[at(g::PENDING_EVENTS)], list }
    }

    pub fn to_bytes(&self) -> [u8; LEN] {
        let mut b = [0; LEN];
        b[at(g::PENDING_EVENTS_FLAG)] = self.flag;
        b[at(g::PENDING_EVENTS)] = self.count;
        for (i, e) in self.list.iter().enumerate() {
            let k = at(g::NEXT_EVENT) + 4 * i;
            set_word(&mut b, k, e.word());
            set_word(&mut b, k + 2, e.place);
        }
        b
    }
}

const _: () = assert!(g::PENDING_EVENTS_FLAG == 0x118f);
const _: () = assert!(g::PENDING_EVENTS == 0x1190);
const _: () = assert!(g::NEXT_EVENT == 0x1191);
const _: () = assert!(g::NEXT_EVENT_PLACE == 0x1193);
const _: () = assert!(AT == g::PENDING_EVENTS_FLAG && AT as usize + LEN == g::NEXT_EVENT as usize + 4 * MAX_EVENTS);

/// The events as the memory the bus serves from them.
pub struct EventsMemory(Events);

const BASE: u32 = ((g::GAME_DS as u32) << 4) + AT as u32;

impl pc::hw::Owned for EventsMemory {
    fn ranges(&self) -> Vec<(u32, u32)> {
        vec![(BASE, LEN as u32)]
    }
    fn read(&self, addr: u32) -> u8 {
        self.0.to_bytes()[(addr - BASE) as usize]
    }
    fn write(&mut self, addr: u32, v: u8) {
        let mut b = self.0.to_bytes();
        b[(addr - BASE) as usize] = v;
        self.0 = Events::from_bytes(&b);
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The events (their memory owned from the first use on).
pub(super) fn get(c: &mut Ctx) -> Events {
    if c.m.hw.owned::<EventsMemory>().is_none() {
        c.m.hw.own(Box::new(EventsMemory(Events::default())));
    }
    c.m.hw.owned::<EventsMemory>().expect("the events' memory").0
}

/// The events changed by `f`: the bytes that changed written on the bus
/// (to the struct, and the memory's copy).
pub(super) fn update(c: &mut Ctx, f: impl FnOnce(&mut Events)) {
    let old = get(c);
    let mut new = old;
    f(&mut new);
    let (a, b) = (old.to_bytes(), new.to_bytes());
    for i in 0..LEN {
        if a[i] != b[i] {
            let at = c.m.cpu.phys(g::GAME_DS, AT + i as u16);
            c.m.hw.write8(at, b[i]);
        }
    }
}
