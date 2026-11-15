//! The map's selection as Rust state (Stage B of
//! `docs/readable-port-plan.md`): the troop selected and the record
//! shown, the troop panel's state, the caption under the cursor, the
//! place the cursor is over, and the animated sprites' frame.
//!
//! A table of one record ([`Which::MapSelection`], 0Fh bytes at 46EFh)
//! in the generic record tables (`hand/record_table.rs`, the memory kept
//! as the checked mirror); [`MapSelection`] is its typed view. The bytes
//! after it (46FEh-470Fh) are scratch of the troop panel's computations
//! (a location's needs and spread bits, the nearest location), not part
//! of it.

use super::record_table::{self, Which};
use super::{Ctx, DS, globals as g};

/// Where the record starts in the data segment.
pub const AT: u16 = 0x46ef;

/// The map's selection.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct MapSelection {
    /// [46EFh]: the troop selected on the map, 0 for none.
    pub selected: u16,
    /// [46F1h]: the record (troop or place) shown and set up last.
    pub shown: u16,
    /// [46F3h]: set while the map screen is toggled in over a place's
    /// panel (its title 52h then, else 53h); cleared when it is left.
    pub map_screen: u8,
    /// [46F4h]: the troop panel up: 0 none, 1 an order panel (the skills
    /// spread over a location's needs).
    pub panel: u8,
    /// [46F5h]: with [`Self::panel`], clicks go to the skills panel.
    pub skills_panel: u8,
    /// [46F6h]: the animated map sprites' frame (every fourth step all of
    /// them move).
    pub sprite_frame: u8,
    /// [46F7h]: the caption's kind (`location_map_kind`), 0 for none.
    pub caption_kind: u8,
    /// [46F8h]: the location whose caption is up, 0 for none.
    pub caption: u16,
    /// [46FAh]: the troop whose panel is to be drawn, 0 for none.
    pub panel_dirty: u16,
    /// [46FCh]: the place under the cursor (the message line's subject),
    /// 0 for none.
    pub subject: u16,
}

impl MapSelection {
    fn from_bytes(b: &[u8]) -> MapSelection {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        MapSelection {
            selected: w(0),
            shown: w(2),
            map_screen: b[4],
            panel: b[5],
            skills_panel: b[6],
            sprite_frame: b[7],
            caption_kind: b[8],
            caption: w(9),
            panel_dirty: w(0x0b),
            subject: w(0x0d),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        let mut w = |i: usize, v: u16| {
            b[i] = v as u8;
            b[i + 1] = (v >> 8) as u8;
        };
        w(0, self.selected);
        w(2, self.shown);
        w(9, self.caption);
        w(0x0b, self.panel_dirty);
        w(0x0d, self.subject);
        b[4] = self.map_screen;
        b[5] = self.panel;
        b[6] = self.skills_panel;
        b[7] = self.sprite_frame;
        b[8] = self.caption_kind;
        b
    }

    /// The record's bytes at DS (another data segment than the game's:
    /// not the table's).
    fn raw(c: &mut Ctx) -> MapSelection {
        let mut b = [0; record_table::MAX];
        for (k, v) in b.iter_mut().enumerate().take(Which::MapSelection.size() as usize) {
            *v = c.byte(AT + k as u16);
        }
        MapSelection::from_bytes(&b)
    }
}

/// The map's selection, in step with the memory.
pub(super) fn get(c: &mut Ctx) -> MapSelection {
    if c.sreg(DS) != g::GAME_DS {
        return MapSelection::raw(c);
    }
    MapSelection::from_bytes(&record_table::get(c, Which::MapSelection, 0))
}

/// The map's selection changed to `t`: kept, and its changed bytes
/// written through.
pub(super) fn set(c: &mut Ctx, t: MapSelection) {
    if c.sreg(DS) != g::GAME_DS {
        let (old, new) = (MapSelection::raw(c).to_bytes(), t.to_bytes());
        for k in 0..Which::MapSelection.size() as usize {
            if old[k] != new[k] {
                c.set_byte(AT + k as u16, new[k]);
            }
        }
        return;
    }
    record_table::set(c, Which::MapSelection, 0, t.to_bytes());
}

/// The map's selection changed by `f`.
pub(super) fn update(c: &mut Ctx, f: impl FnOnce(&mut MapSelection)) {
    let mut t = get(c);
    f(&mut t);
    set(c, t);
}
