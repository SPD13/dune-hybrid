//! The game's data by name: views over its records in the data segment.
//!
//! The readable ports (stage A of `docs/readable-port-plan.md`) work with
//! these instead of offsets. Every accessor reads or writes exactly the
//! bytes the original does, so a port written with them is still exact
//! and the shadow check still compares it call by call.
//!
//! Names come from what the ports found each field doing; a field whose
//! meaning isn't known yet keeps its offset in its name (`word_10`).

#![allow(dead_code)] // views grow ahead of the ports that use them

use super::record_table::{self, Which};
use super::{Ctx, globals as g, troop_table};
use pc::cpu::DS;

/// A troop: a 1Bh-byte record in the table at [`g::TROOPS`], numbered from
/// 1. (Byte 3 bit 80h marks a record that isn't a troop of yours.)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Troop(pub u16);

/// Bits of a troop's byte 3, above its job (the low four bits).
pub mod troop_state {
    /// Set by `mark_troop` (7085h).
    pub const MARKED: u8 = 0x10;
    /// Set when the troop is started (`start_troop`, 668Fh).
    pub const STARTED: u8 = 0x20;
    /// On its way: between places, at its own map position (words 6, 8),
    /// going to word 4. Set when it sets out (`troop_set_out`), cleared
    /// when it arrives (`settle_arrived_troop`).
    pub const TRAVELLING: u8 = 0x40;
    /// Not a troop of yours.
    pub const NOT_A_TROOP: u8 = 0x80;
}

/// Bits of a location's byte 0Ah ([`Location::flags`]).
pub mod location_flag {
    /// Set by job 8 the first time it works the location (its spice
    /// cleared, its move started); `grow_location` no longer grows it.
    pub const STARTED: u8 = 0x01;
    /// Held by the Harkonnens: `harkonnen_take` sets it, `abandon_location`
    /// clears it (the player there: [`super::g::HARKONNEN_PLACE`]).
    /// Entering shows their picture (`enter_location`), job 1 does not
    /// work there, the map shows it (`location_to_show`), and its troops
    /// get other sprites (`sprite_list_of`) and sides (`troop_screen_place`).
    pub const HARKONNEN: u8 = 0x02;
    /// Set by `maybe_mark_troop`; job 4 counts down while it is set, then
    /// clears it (`clear_location_4`).
    pub const BIT_4: u8 = 0x04;
    /// Raided (`location_raided`, which raises the charisma and the
    /// motivation): its orders menu and `pick_map_location` treat it as
    /// taken until the day in byte 0Bh has passed; then `troop_day_update`
    /// clears it, makes it a sietch again (byte 8 to its low three bits)
    /// and counts it held ([`super::g::SIETCHES_HELD`]).
    pub const RAIDED: u8 = 0x08;
    /// Visited: set when the player steps into it, for a kind of 20h or
    /// more (`step_3ff5`). Others' troops there get the sprite list 181Fh
    /// (`sprite_list_of`); the map shows it by its type string
    /// (`location_map_kind`).
    pub const VISITED: u8 = 0x10;
    /// Its byte 1Bh counts (set by job 9 when it overflows): it grows
    /// (`grow_spice`) and shows in the map caption (`caption_count_line`).
    pub const COUNTING: u8 = 0x20;
    /// Prospected: set when job 1 finishes there; the map colours its
    /// region by its spice (`region_colours`).
    pub const PROSPECTED: u8 = 0x40;
    /// Not found yet: off the map and skipped by the searches over the
    /// locations (`nearest_location`, `pick_attack_location`, ...);
    /// cleared when a troop arrives harvesting (`troop_arrived_at`) or by
    /// a talk with person 0Ch (`reward_talk_by_talker`).
    pub const UNDISCOVERED: u8 = 0x80;
}

impl Troop {
    pub const SIZE: u16 = 0x1b;

    /// Troop `n` (from 1), as `troop_ptr` (6906h) finds it.
    pub fn number(n: u8) -> Troop {
        Troop(g::TROOPS.wrapping_add((n.wrapping_sub(1) as u16) * Self::SIZE))
    }

    /// Every record of the table, in order.
    pub fn all() -> impl Iterator<Item = Troop> {
        (g::TROOPS..g::TROOPS_END).step_by(Self::SIZE as usize).map(Troop)
    }

    /// Its offset in the data segment (what the original keeps in SI).
    pub fn offset(self) -> u16 {
        self.0
    }

    fn at(self, field: u16) -> u16 {
        self.0.wrapping_add(field)
    }

    /// The record in the Rust table (`hand/troop_table.rs`) for a field of
    /// `len` bytes; none for an offset that starts no record, a field
    /// past it, or another data segment (then the memory, as before).
    fn native(self, c: &mut Ctx, field: u16, len: u16) -> Option<usize> {
        if c.sreg(DS) != g::GAME_DS || field + len > Self::SIZE {
            return None;
        }
        troop_table::index(self.0)
    }
    fn byte(self, c: &mut Ctx, field: u16) -> u8 {
        match self.native(c, field, 1) {
            Some(i) => troop_table::byte(c, i, field),
            None => c.byte(self.at(field)),
        }
    }
    fn set_byte(self, c: &mut Ctx, field: u16, v: u8) {
        match self.native(c, field, 1) {
            Some(i) => troop_table::set_byte(c, i, field, v),
            None => c.set_byte(self.at(field), v),
        }
    }
    fn word(self, c: &mut Ctx, field: u16) -> u16 {
        self.byte(c, field) as u16 | (self.byte(c, field + 1) as u16) << 8
    }
    fn set_word(self, c: &mut Ctx, field: u16, v: u16) {
        self.set_byte(c, field, v as u8);
        self.set_byte(c, field + 1, (v >> 8) as u8);
    }

    /// The whole record (`hand/troop_table.rs`), when this is one of the
    /// table's (with the game's data segment); else none.
    pub fn record(self, c: &mut Ctx) -> Option<troop_table::TroopRecord> {
        let i = self.native(c, 0, Self::SIZE)?;
        Some(troop_table::get(c, i))
    }
    /// The record stored back (only its changed bytes are written).
    pub fn store(self, c: &mut Ctx, r: troop_table::TroopRecord) {
        let i = self.native(c, 0, Self::SIZE).expect("a troop record");
        troop_table::set(c, i, r);
    }

    /// Byte 3: the job (low four bits) and [`troop_state`] bits.
    pub fn state(self, c: &mut Ctx) -> u8 {
        self.byte(c, 3)
    }
    pub fn set_state(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 3, v)
    }
    /// The job: byte 3's low four bits.
    pub fn job(self, c: &mut Ctx) -> u8 {
        self.state(c) & 0x0f
    }

    /// Word 4: its location (a [`Location`] record's offset).
    pub fn location(self, c: &mut Ctx) -> u16 {
        self.word(c, 4)
    }
    pub fn set_location(self, c: &mut Ctx, v: u16) {
        self.set_word(c, 4, v)
    }

    /// Word 10h: its low two bits the step of its last skill gain (the
    /// skill's index + 1); bit 80h, 200h: not known yet.
    pub fn word_10(self, c: &mut Ctx) -> u16 {
        self.word(c, 0x10)
    }
    pub fn set_word_10(self, c: &mut Ctx, v: u16) {
        self.set_word(c, 0x10, v)
    }

    /// Word 0Ah: the game time it last changed (`stamp_record`).
    pub fn changed_at(self, c: &mut Ctx) -> u16 {
        self.word(c, 0x0a)
    }
    pub fn set_changed_at(self, c: &mut Ctx, v: u16) {
        self.set_word(c, 0x0a, v)
    }

    /// Word 12h: bits, not all known (bits 15..13 select the skills its
    /// activity trains, see `troop_skills_decay`; 400h, 40h, 30h).
    pub fn word_12(self, c: &mut Ctx) -> u16 {
        self.word(c, 0x12)
    }

    /// Byte 14h: the day it joined or was last given a job.
    pub fn day(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x14)
    }
    pub fn set_day(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x14, v)
    }

    /// Byte 15h: its motivation (0 to 100).
    pub fn motivation(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x15)
    }
    pub fn set_motivation(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x15, v)
    }

    /// Byte 16h + `k`: its skill `k` (0 to 5Fh).
    pub fn skill(self, c: &mut Ctx, k: u16) -> u8 {
        self.byte(c, 0x16 + k)
    }
    pub fn set_skill(self, c: &mut Ctx, k: u16, v: u8) {
        self.set_byte(c, 0x16 + k, v)
    }
}

impl Troop {
    /// Byte 10h (word 10h's low byte): bit 80h a Harkonnen troop
    /// ([`Troop::is_harkonnen`]); bit 10h marks it for
    /// `refresh_flagged_troop`.
    pub fn byte_10(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x10)
    }
    pub fn set_byte_10(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x10, v)
    }

    pub fn set_word_12(self, c: &mut Ctx, v: u16) {
        self.set_word(c, 0x12, v)
    }
    /// Byte 12h (word 12h's low byte).
    pub fn byte_12(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x12)
    }
    pub fn set_byte_12(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x12, v)
    }

    /// Bit 80h of byte 10h: a Harkonnen troop. Counted apart in the
    /// tallies (`add_troop_strength`, `add_troop_to_balance`), slotted from
    /// 9 at its location (`link_troop`), taken by `harkonnen_take`; its
    /// size grows by itself.
    pub fn is_harkonnen(self, c: &mut Ctx) -> bool {
        self.byte_10(c) & 0x80 != 0
    }

    /// Byte 0: its number (as [`Troop::number`] takes it).
    pub fn id(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0)
    }
    /// Byte 1: the number of the next troop in its location's chain (0
    /// for the last; [`Location::first_troop`] starts it).
    pub fn next(self, c: &mut Ctx) -> u8 {
        self.byte(c, 1)
    }
    pub fn set_next(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 1, v)
    }
    /// Byte 2: its slot at its location (`link_troop` gives the first
    /// free one, from 1, or from 9 for a troop set apart; 0 while
    /// travelling). Its sprite's spot there follows from it.
    pub fn slot(self, c: &mut Ctx) -> u8 {
        self.byte(c, 2)
    }
    pub fn set_slot(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 2, v)
    }
    /// Words 6, 8: its map position (its location's, when it last left
    /// one; moved along while travelling).
    pub fn x(self, c: &mut Ctx) -> u16 {
        self.word(c, 6)
    }
    pub fn y(self, c: &mut Ctx) -> u16 {
        self.word(c, 8)
    }
    pub fn set_position(self, c: &mut Ctx, x: u16, y: u16) {
        self.set_word(c, 6, x);
        self.set_word(c, 8, y);
    }

    /// Word 0Ch: its production rate (`troop_rate`, 708Ah).
    pub fn rate(self, c: &mut Ctx) -> u16 {
        self.word(c, 0x0c)
    }
    pub fn set_rate(self, c: &mut Ctx, v: u16) {
        self.set_word(c, 0x0c, v)
    }
    /// Word 0Eh: cleared with the rate when stamped; summed by
    /// `add_troop_to_balance`.
    pub fn word_0e(self, c: &mut Ctx) -> u16 {
        self.word(c, 0x0e)
    }
    pub fn set_word_0e(self, c: &mut Ctx, v: u16) {
        self.set_word(c, 0x0e, v)
    }
    /// Word 0Ch while moving (job bits 3): the next place it goes (a
    /// [`Location`] record's offset).
    pub fn destination(self, c: &mut Ctx) -> u16 {
        self.word(c, 0x0c)
    }
    /// Word 0Eh while moving: the skill it carries there (the need's
    /// index, and its bit in the high byte).
    pub fn carried(self, c: &mut Ctx) -> u16 {
        self.word(c, 0x0e)
    }
    pub fn set_carried(self, c: &mut Ctx, v: u16) {
        self.set_word(c, 0x0e, v)
    }

    /// Byte 1Ah: its size (how many; the rate grows with it).
    pub fn size(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x1a)
    }
    pub fn set_size(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x1a, v)
    }

    /// Byte 19h: bits, one per need of its location it covers.
    pub fn byte_19(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x19)
    }
    pub fn set_byte_19(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x19, v)
    }
}

/// Charisma (byte [29h]).
pub fn charisma(c: &mut Ctx) -> u8 {
    c.progress().charisma
}
pub fn set_charisma(c: &mut Ctx, v: u8) {
    c.set_byte(g::CHARISMA, v)
}

/// Any record stamped with the game time (`stamp_record`, 6B25h): word 0Ah
/// the time, words 0Ch and 0Eh cleared.
pub fn stamp(c: &mut Ctx, record: u16, time: u16) {
    // A troop's through its view (the same words of another record in
    // the memory).
    let t = Troop(record);
    t.set_changed_at(c, time);
    t.set_rate(c, 0);
    t.set_word_0e(c, 0);
}

/// The game time (word [2]).
pub fn game_time(c: &mut Ctx) -> u16 {
    c.game_time()
}

/// The location the player is at ([114Eh]: a [`Location`] record).
pub fn here(c: &mut Ctx) -> u16 {
    c.word(g::PLACE)
}

/// A particle: an 11h-byte record of the list after the count at
/// [`g::PARTICLES`] (a sprite on the screen, `particles_spawn_particle`).
/// Read and written in the memory.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Particle(pub u16);

/// Bits of a particle's byte 0Ch ([`Particle::flags`]).
pub mod particle_flag {
    /// Not picked by the cursor: `particle_at`, `troop_particle` and the
    /// hotspots (`hotspots_particles`, which skip 80h too) pass it by;
    /// the troop marker's particle has it (`mark_troop_location`).
    pub const UNPICKABLE: u8 = 0x40;
}

impl Particle {
    pub const SIZE: u16 = 0x11;

    pub fn offset(self) -> u16 {
        self.0
    }
    /// Words 0, 2: its rectangle's top left (x0, y0; x0 with flags in
    /// its top bits, `particles_update_dirty_rect`).
    pub fn x0(self, c: &mut Ctx) -> u16 {
        c.word(self.0)
    }
    pub fn y0(self, c: &mut Ctx) -> u16 {
        c.word(self.0.wrapping_add(2))
    }
    /// Words 4, 6: its rectangle's bottom right (x1, y1).
    pub fn x1(self, c: &mut Ctx) -> u16 {
        c.word(self.0.wrapping_add(4))
    }
    pub fn y1(self, c: &mut Ctx) -> u16 {
        c.word(self.0.wrapping_add(6))
    }
    /// Word 8: its sprite (of the resource it was spawned from).
    pub fn sprite(self, c: &mut Ctx) -> u16 {
        c.word(self.0.wrapping_add(8))
    }
    pub fn set_sprite(self, c: &mut Ctx, v: u16) {
        c.set_word(self.0.wrapping_add(8), v)
    }
    /// Word 0Ah: what it shows (SI when spawned): for a troop's, the
    /// [`Troop`] record.
    pub fn owner(self, c: &mut Ctx) -> u16 {
        c.word(self.0.wrapping_add(0x0a))
    }
    /// Byte 0Ch: its [`particle_flag`] bits (0 when spawned).
    pub fn flags(self, c: &mut Ctx) -> u8 {
        c.byte(self.0.wrapping_add(0x0c))
    }
    pub fn set_flags(self, c: &mut Ctx, v: u8) {
        c.set_byte(self.0.wrapping_add(0x0c), v)
    }
    /// Word 0Dh: a scene particle's step (packed signed x, y,
    /// `scene_particles_step`); the spawn variant's record
    /// (`particles_spawn_variant`); 1916h for the troop marker.
    pub fn word_0d(self, c: &mut Ctx) -> u16 {
        c.word(self.0.wrapping_add(0x0d))
    }
    pub fn set_word_0d(self, c: &mut Ctx, v: u16) {
        c.set_word(self.0.wrapping_add(0x0d), v)
    }
    /// Word 0Fh: the spawn record it came from (`particles_spawn_variant`);
    /// 1916h for the troop marker.
    pub fn set_word_0f(self, c: &mut Ctx, v: u16) {
        c.set_word(self.0.wrapping_add(0x0f), v)
    }
}

/// A UI element: a 0Eh-byte record of a UI list (a count, then the
/// records; the current one at [`g::CURRENT_UI_LIST`]): a rectangle the
/// cursor can click, drawn by `draw_ui_element`. Read and written in the
/// memory.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UiElement(pub u16);

/// Bits of a UI element's byte 8 ([`UiElement::flags`]).
pub mod ui_flag {
    /// Without its sprite (`draw_ui_element` draws only the fill).
    pub const NO_SPRITE: u8 = 0x20;
    /// Its rectangle filled first (`draw_ui_element`).
    pub const FILLED: u8 = 0x40;
    /// Live: it takes clicks (`element_under_cursor`) and the keyboard
    /// moves to it (`keyboard_move`).
    pub const LIVE: u8 = 0x80;
}

/// Bits of a UI element's byte 9 ([`UiElement::press`]).
pub mod ui_press {
    /// Drawn pressed while held (`draw_element_pressed`, its sprite one
    /// up; [`super::g::PRESSED_ELEMENT`]).
    pub const SHOWS_PRESSED: u8 = 0x20;
    /// Its handler called again while held ([`super::g::REPEAT_ELEMENT`]).
    pub const REPEATS: u8 = 0x40;
}

impl UiElement {
    pub const SIZE: u16 = 0x0e;

    /// Words 0, 2, 4, 6: its rectangle (x0, y0, x1, y1).
    pub fn x0(self, c: &mut Ctx) -> u16 {
        c.word(self.0)
    }
    pub fn y0(self, c: &mut Ctx) -> u16 {
        c.word(self.0.wrapping_add(2))
    }
    pub fn x1(self, c: &mut Ctx) -> u16 {
        c.word(self.0.wrapping_add(4))
    }
    pub fn y1(self, c: &mut Ctx) -> u16 {
        c.word(self.0.wrapping_add(6))
    }
    /// Byte 8: its [`ui_flag`] bits.
    pub fn flags(self, c: &mut Ctx) -> u8 {
        c.byte(self.0.wrapping_add(8))
    }
    pub fn set_flags(self, c: &mut Ctx, v: u8) {
        c.set_byte(self.0.wrapping_add(8), v)
    }
    /// Byte 9: its [`ui_press`] bits.
    pub fn press(self, c: &mut Ctx) -> u8 {
        c.byte(self.0.wrapping_add(9))
    }
    /// Word 0Ah: its sprite (FFFFh none).
    pub fn sprite(self, c: &mut Ctx) -> u16 {
        c.word(self.0.wrapping_add(0x0a))
    }
    pub fn set_sprite(self, c: &mut Ctx, v: u16) {
        c.set_word(self.0.wrapping_add(0x0a), v)
    }
}

/// A location (sietch, palace, ...): a 1Ch-byte record in the table at
/// 100h, which ends with a byte FFh.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Location(pub u16);

impl Location {
    pub const SIZE: u16 = 0x1c;
    pub const FIRST: u16 = 0x100;

    /// Every record of the table, in order (up to the FFh one).
    pub fn all(c: &mut Ctx) -> Vec<Location> {
        let mut out = Vec::new();
        let mut l = Self::FIRST;
        loop {
            out.push(Location(l));
            l = l.wrapping_add(Self::SIZE);
            if c.byte(l) == 0xff {
                return out;
            }
        }
    }

    pub fn offset(self) -> u16 {
        self.0
    }

    /// The record in the Rust table (`hand/record_table.rs`) for a field of
    /// `len` bytes; none for an offset that starts no record, a field past
    /// it, or another data segment (then the memory, as before).
    fn native(self, c: &mut Ctx, field: u16, len: u16) -> Option<usize> {
        if c.sreg(DS) != g::GAME_DS || field + len > Self::SIZE {
            return None;
        }
        record_table::index(Which::Locations, self.0)
    }
    fn byte(self, c: &mut Ctx, field: u16) -> u8 {
        match self.native(c, field, 1) {
            Some(i) => record_table::byte(c, Which::Locations, i, field),
            None => c.byte(self.0.wrapping_add(field)),
        }
    }
    fn set_byte(self, c: &mut Ctx, field: u16, v: u8) {
        match self.native(c, field, 1) {
            Some(i) => record_table::set_byte(c, Which::Locations, i, field, v),
            None => c.set_byte(self.0.wrapping_add(field), v),
        }
    }
    fn word(self, c: &mut Ctx, field: u16) -> u16 {
        self.byte(c, field) as u16 | (self.byte(c, field + 1) as u16) << 8
    }
    #[allow(dead_code)]
    fn set_word(self, c: &mut Ctx, field: u16, v: u16) {
        self.set_byte(c, field, v as u8);
        self.set_byte(c, field + 1, (v >> 8) as u8);
    }

    /// Location `n` (from 1), as the original finds it (`mul` by 1Ch,
    /// + 0E4h).
    pub fn number(n: u8) -> Location {
        Location((n as u16 * Self::SIZE).wrapping_add(Self::FIRST - Self::SIZE))
    }

    /// Byte 12h: its spice. The top four bits scale its troops' rate
    /// (`troop_rate`); the harvest (`job_harvest`) lowers it; summed / 8
    /// by `estimate_field_spice`.
    pub fn spice(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x12)
    }
    pub fn set_spice(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x12, v)
    }
    /// Byte 11h: what a point of its spice takes of the harvest.
    pub fn harvest_per_spice(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x11)
    }
    /// Byte 13h: the harvest carried over (below byte 11h).
    pub fn harvest_left(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x13)
    }
    pub fn set_harvest_left(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x13, v)
    }

    /// Byte 19h: eight bits, not known yet (spread one per byte by
    /// `spread_location_bits`, 7EFBh), also summed as a number over the
    /// locations (`add_location_sum`, 765Eh).
    pub fn byte_19(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x19)
    }
    pub fn set_byte_19(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x19, v)
    }

    /// Byte 0: indexes the table at 1141h in `troop_event_roll`.
    pub fn byte_00(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0)
    }

    /// Byte 8: its kind. Below 28h a sietch (`abandon_location`, which
    /// makes it 28h + its low three bits; `location_kind_check`,
    /// `raid_rally_own_troop` compare with 28h); `grow_location` grows
    /// those below 20h; kept to its low three bits when the location is
    /// freed (`troop_day_update`).
    pub fn kind(self, c: &mut Ctx) -> u8 {
        self.byte(c, 8)
    }
    pub fn set_kind(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 8, v)
    }

    /// Byte 9: the number of the first troop of its chain (`link_troop`).
    pub fn first_troop(self, c: &mut Ctx) -> u8 {
        self.byte(c, 9)
    }
    pub fn set_first_troop(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 9, v)
    }

    /// Word 0: FFFFh in the word just past the last record (the table's
    /// end, as the walks over it test).
    pub fn is_end(self, c: &mut Ctx) -> bool {
        self.word(c, 0) == 0xffff
    }

    /// Words 2, 4: its map position.
    pub fn x(self, c: &mut Ctx) -> u16 {
        self.word(c, 2)
    }
    pub fn y(self, c: &mut Ctx) -> u16 {
        self.word(c, 4)
    }

    /// Byte 10h: its region (the map colours the region of a location
    /// shown, `loc_057fa`).
    pub fn region(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x10)
    }

    /// Byte 0Ah: its [`location_flag`] bits.
    pub fn flags(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x0a)
    }
    pub fn set_flags(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x0a, v)
    }

    /// Bytes 14h + `k` (`k` 0 to 6): its skill needs (`skill_needs`).
    pub fn need(self, c: &mut Ctx, k: u16) -> u8 {
        self.byte(c, 0x14 + k)
    }
    pub fn set_need(self, c: &mut Ctx, k: u16, v: u8) {
        self.set_byte(c, 0x14 + k, v)
    }

    /// Byte 1Ah: tested by `set_troop_job` at [`Location`] 7C8h.
    pub fn byte_1a(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x1a)
    }

    /// Byte 0Bh: the radius of its circle on the map (`location_circle`,
    /// `start_move`): 2 once a troop of yours has joined there
    /// (`troop_joins`), grown up to 0Ch (`grow_location`, job 8). Read
    /// also as [`Location::found_from`] and [`Location::raid_ends`].
    pub fn radius(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x0b)
    }
    pub fn set_radius(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x0b, v)
    }

    /// Byte 0Bh while [`location_flag::UNDISCOVERED`]: the phase from which
    /// it can be found (`scan_encounters`, `location_ahead`,
    /// `nearest_places`).
    pub fn found_from(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x0b)
    }

    /// Byte 0Bh while [`location_flag::RAIDED`]: the day the raid ends
    /// (its day + 2, `location_raided`; `troop_day_update`).
    pub fn raid_ends(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x0b)
    }
    pub fn set_raid_ends(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x0b, v)
    }

    /// Byte 1Bh: the count the map caption shows with
    /// [`location_flag::COUNTING`] (`caption_count_line`). Job 9 raises it
    /// by its troops' strength until it overflows (then it is 5 and
    /// counting); then it grows (`grow_spice`, up to FAh) and job 8 takes
    /// 0Ch a time from it.
    pub fn count(self, c: &mut Ctx) -> u8 {
        self.byte(c, 0x1b)
    }
    pub fn set_count(self, c: &mut Ctx, v: u8) {
        self.set_byte(c, 0x1b, v)
    }
}
