//! The story's loose variables, as Rust state (Stage B of
//! `docs/readable-port-plan.md`).
//!
//! The story's part of the data segment (DS:0-1260h, the block a save
//! keeps) is mostly records: the tables of `hand/record_table.rs`, the
//! clock's words, the events. Between them lie single variables and a few
//! small tables reached through a pointer or an index: the player's place,
//! the spice's accounts of the day, the shipment, the two people in the
//! frieze, the attack planned, the random word, the palace plan's sprite
//! list and the palace rooms' button lists, ... [`VARS`] lists them, each
//! by name, offset and size; [`Variables`] holds their bytes and owns
//! their memory (`pc::hw::Owned`): every read and write of them on the
//! bus goes to it, the ports' `c.word(g::PLACE)` as the generated code's
//! and the interpreter's. Registered with the record tables (at the first
//! use of any).

use super::globals as g;
use pc::Machine;

/// A loose variable (or a small table): its name, offset in the data
/// segment and size.
#[derive(Clone, Copy, Debug)]
pub struct Var {
    pub name: &'static str,
    pub at: u16,
    pub len: u16,
}

const fn var(name: &'static str, at: u16, len: u16) -> Var {
    Var { name, at, len }
}

/// [08A8h]: the word after the last location record (70 of them from
/// 100h).
pub const AFTER_LOCATIONS: u16 = 0x08a8;
/// [0FBBh]: the byte after the troops.
pub const AFTER_TROOPS: u16 = 0x0fbb;
/// [0FBCh]: the location record of a reset troop (`reset_troop`): a
/// location's size (1Ch), just past the troops.
pub const NOWHERE: u16 = 0x0fbc;
/// [113Eh].
pub const BYTE_113E: u16 = 0x113e;
/// [113Fh]: a record pointer walked and kept at 2282h..229Fh.
pub const WORD_113F: u16 = 0x113f;
/// [115Eh]: the statistics' numbers as text (`stats_numbers`, DI).
pub const STATS_NUMBERS: u16 = 0x115e;
/// [116Ah]: the statistics' letters (`stats_numbers`, BP; read at
/// BEC6h).
pub const STATS_LETTERS: u16 = 0x116a;
/// [11B9h].
pub const WORD_11B9: u16 = 0x11b9;
/// [11D9h]: the word after the followed troop's note.
pub const WORD_11D9: u16 = 0x11d9;
/// [120Bh]: the palace plan's sprite list (sprite, x, y words; FFFFh
/// ends it).
pub const PALACE_PLAN: u16 = 0x120b;
/// [1225h]: the palace's room list (5-byte entries: label phrase, 4
/// button codes; `g::ROOM_LISTS`).
pub const PALACE_ROOMS: u16 = 0x1225;

/// The variables, by offset. Each one's meaning is its constant's doc
/// (here or in [`super::globals`]).
pub const VARS: [Var; 38] = [
    var("random bits", g::RANDOM_BITS, 2),
    var("after the locations", AFTER_LOCATIONS, 2),
    var("after the troops", AFTER_TROOPS, 1),
    var("nowhere", NOWHERE, 0x1c),
    var("byte 113E", BYTE_113E, 1),
    var("word 113F", WORD_113F, 2),
    var("chances", g::CHANCES, 0x0d),
    var("place", g::PLACE, 2),
    var("last place", g::LAST_PLACE, 2),
    var("speaker slot 1", g::SPEAKER_SLOTS, 1),
    var("speaker slot 2", g::SPEAKER_SLOT_2, 1),
    var("word 1154", g::WORD_1154, 2),
    var("attack day", g::ATTACK_DAY, 2),
    var("shipment ready", g::SHIPMENT_READY, 2),
    var("regions to unmark", g::REGIONS_TO_UNMARK, 2),
    var("results date", g::RESULTS_DATE, 2),
    var("stats numbers", STATS_NUMBERS, 0x0c),
    var("stats letters", STATS_LETTERS, 6),
    var("spice yesterday", g::SPICE_YESTERDAY, 2),
    var("spice paid", g::SPICE_PAID, 2),
    var("last time", g::LAST_TIME, 2),
    var("reach", g::REACH, 2),
    var("joined enough", g::JOINED_ENOUGH, 1),
    var("shipment due", g::SHIPMENT_DUE, 2),
    var("word 11B9", WORD_11B9, 2),
    var("spice demand lapsed", g::SPICE_DEMAND_LAPSED, 1),
    var("skip Harkonnen take", g::SKIP_HARKONNEN_TAKE, 1),
    var("ctrl-V table end", g::CTRL_V_TABLE_END, 2),
    var("book position", g::BOOK_POSITION, 2),
    var("topic place record", g::TOPIC_PLACE_RECORD, 2),
    var("byte 11D0", g::BYTE_11D0, 3),
    var("followed troop place", g::FOLLOWED_TROOP_PLACE, 6),
    var("word 11D9", WORD_11D9, 2),
    var("attack target", g::ATTACK_TARGET, 2),
    var("Harkonnen place icons", g::HARKONNEN_PLACE_ICONS, 0x0e),
    var("palace plan", PALACE_PLAN, 0x1a),
    var("palace rooms", PALACE_ROOMS, 0x3c),
    // (DS:1261h on is not the save's.)
    var("end", 0x1261, 0),
];

/// The bytes the variables hold.
pub const BYTES: usize = {
    let mut n = 0;
    let mut i = 0;
    while i < VARS.len() {
        n += VARS[i].len as usize;
        i += 1;
    }
    n
};

// In order, none overlapping the next.
const _: () = {
    let mut i = 1;
    while i < VARS.len() {
        assert!(VARS[i - 1].at + VARS[i - 1].len <= VARS[i].at);
        i += 1;
    }
};
const _: () = assert!(BYTES == 217);

/// The variables' bytes, owning their memory.
pub struct Variables {
    bytes: [u8; BYTES],
    /// By the offset in the data segment (to 1261h): the byte's index in
    /// `bytes`, `NONE` for none.
    slot: Box<[u16]>,
}

const NONE: u16 = u16::MAX;
const BASE: u32 = (g::GAME_DS as u32) << 4;

impl Variables {
    fn new() -> Variables {
        let mut slot = vec![NONE; 0x1261].into_boxed_slice();
        let mut k = 0;
        for v in VARS {
            for i in 0..v.len {
                slot[(v.at + i) as usize] = k;
                k += 1;
            }
        }
        Variables { bytes: [0; BYTES], slot }
    }

    /// The variable's byte at DS:`at`.
    pub fn byte(&self, at: u16) -> u8 {
        self.bytes[self.slot[at as usize] as usize]
    }

    /// The variable's word at DS:`at`.
    pub fn word(&self, at: u16) -> u16 {
        self.byte(at) as u16 | (self.byte(at + 1) as u16) << 8
    }

    /// The variable holding DS:`at`, if one does.
    pub fn named(at: u16) -> Option<&'static Var> {
        VARS.iter().find(|v| (v.at..v.at + v.len).contains(&at))
    }
}

impl pc::hw::Owned for Variables {
    fn ranges(&self) -> Vec<(u32, u32)> {
        VARS.iter().filter(|v| v.len > 0).map(|v| (BASE + v.at as u32, v.len as u32)).collect()
    }
    fn read(&self, addr: u32) -> u8 {
        self.byte((addr - BASE) as u16)
    }
    fn write(&mut self, addr: u32, v: u8) {
        let k = self.slot[(addr - BASE) as usize];
        self.bytes[k as usize] = v;
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The variables registered with the bus, unless they are.
pub(super) fn own(m: &mut Machine) {
    if m.hw.owned::<Variables>().is_none() {
        let _ = m.hw.own(Box::new(Variables::new()));
    }
}

/// The variables, if owned yet.
pub fn get(m: &Machine) -> Option<&Variables> {
    m.hw.owned::<Variables>()
}
