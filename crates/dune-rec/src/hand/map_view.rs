//! The map view as Rust state (Stage B of `docs/readable-port-plan.md`):
//! the rect the map shows, which view is up (the map, zoomed in, a
//! place's view), the changes seen since it was drawn and the routine
//! that redraws it.
//!
//! A table of one record ([`Which::MapView`], 0Ch bytes at 46E3h) in the
//! generic record tables (`hand/record_table.rs`, the memory kept as the
//! checked mirror); [`MapView`] is its typed view. The generated
//! routines that fill the rect (given 46E3h in DI) write the mirror and
//! are counted apart.

use super::record_table::{self, Which};
use super::{Ctx, DS, globals as g};

/// Where the record starts in the data segment.
pub const AT: u16 = 0x46e3;

/// The map view.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct MapView {
    /// [46E3h]: the rect shown: left, top, right, bottom (also the
    /// mouse rect while the view is up).
    pub rect: [u16; 4],
    /// [46EBh] ([`g::MAP_MODE`]): which view is up: 0 none, 80h bit the
    /// map zoomed in, 40h bit a place's view (dropped when a dialogue
    /// takes over), 1 while a person's mark is drawn.
    pub mode: u8,
    /// [46ECh]: changes seen since the view was drawn (a location whose
    /// spice ran short, a troop's marker moved); cleared on a redraw.
    pub changes: u8,
    /// [46EDh]: the routine that draws the view (49A0h, 5A9Ah, ...;
    /// jumped to without a talking head).
    pub redraw: u16,
}

impl MapView {
    /// The map zoomed in (mode bit 80h).
    pub fn zoomed(&self) -> bool {
        self.mode & 0x80 != 0
    }

    /// A place's view up (mode bit 40h).
    pub fn place_view(&self) -> bool {
        self.mode & 0x40 != 0
    }

    fn from_bytes(b: &[u8]) -> MapView {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        MapView {
            rect: [w(0), w(2), w(4), w(6)],
            mode: b[8],
            changes: b[9],
            redraw: w(0x0a),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        let mut w = |i: usize, v: u16| {
            b[i] = v as u8;
            b[i + 1] = (v >> 8) as u8;
        };
        for (k, v) in self.rect.iter().enumerate() {
            w(2 * k, *v);
        }
        w(0x0a, self.redraw);
        b[8] = self.mode;
        b[9] = self.changes;
        b
    }

    /// The record's bytes at DS (another data segment than the game's:
    /// not the table's).
    fn raw(c: &mut Ctx) -> MapView {
        let mut b = [0; record_table::MAX];
        for (k, v) in b.iter_mut().enumerate().take(Which::MapView.size() as usize) {
            *v = c.byte(AT + k as u16);
        }
        MapView::from_bytes(&b)
    }
}

const _: () = assert!(g::MAP_MODE == AT + 8);

/// The map view, in step with the memory.
pub(super) fn get(c: &mut Ctx) -> MapView {
    if c.sreg(DS) != g::GAME_DS {
        return MapView::raw(c);
    }
    MapView::from_bytes(&record_table::get(c, Which::MapView, 0))
}

/// The map view changed to `t`: kept, and its changed bytes written
/// through.
pub(super) fn set(c: &mut Ctx, t: MapView) {
    if c.sreg(DS) != g::GAME_DS {
        let (old, new) = (MapView::raw(c).to_bytes(), t.to_bytes());
        for k in 0..Which::MapView.size() as usize {
            if old[k] != new[k] {
                c.set_byte(AT + k as u16, new[k]);
            }
        }
        return;
    }
    record_table::set(c, Which::MapView, 0, t.to_bytes());
}

/// The map view changed by `f`.
pub(super) fn update(c: &mut Ctx, f: impl FnOnce(&mut MapView)) {
    let mut t = get(c);
    f(&mut t);
    set(c, t);
}
