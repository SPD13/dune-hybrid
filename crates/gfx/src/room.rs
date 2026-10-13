//! Rooms: what `draw_SAL` draws, identified by its drawing operations.
//!
//! The game builds each room from its room files (`*.SAL`) as a run of
//! driver calls into fb1: sprites, floor and wall ramps (slot 36) and lines
//! (slot 19). A room's key hashes that run by content (sprite pixels,
//! positions, flips, palette offsets, ramp and line parameters), never by
//! where things sit in memory, so the same room has the same key in the
//! game, in the room harness and in an HD art pack. Characters standing in
//! the room are drawn inside the same routine; callers leave their calls out.

use crate::{
    Mem,
    hash::fnv64,
    model::{Px, Sink, walk},
    ops::{DrawOp, Format, Regs, SpriteRef},
    sprite,
};

/// Rows of the game area (the part of the screen rooms are drawn in).
pub const ROWS: usize = 152;
/// Bytes of the game area (320 × 152).
pub const AREA: usize = 320 * ROWS;

/// Running key of a room's drawing operations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoomKey(u64);

impl RoomKey {
    pub fn new() -> RoomKey {
        RoomKey(0xcbf2_9ce4_8422_2325)
    }

    pub fn value(self) -> u64 {
        self.0
    }

    /// Add one operation (`mem` as at the call: sprites are read from it).
    pub fn add(&mut self, op: &DrawOp, r: &Regs, mem: Mem) {
        let mut b: Vec<u8> = self.0.to_le_bytes().to_vec();
        let mut w = |v: u16| b.extend_from_slice(&v.to_le_bytes());
        w(r.y_offset);
        match *op {
            DrawOp::Blit { sprite, x, y } => {
                w(1);
                sprite_id(&sprite, mem, &mut w);
                w(x);
                w(y);
            }
            DrawOp::BlitClipped { sprite, x, y, clip } => {
                w(2);
                sprite_id(&sprite, mem, &mut w);
                for v in [x, y, clip.x0, clip.y0, clip.x1, clip.y1, r.cx] {
                    w(v);
                }
            }
            DrawOp::BlitScaled { sprite, x, y, out_w, out_h, step } => {
                w(3);
                sprite_id(&sprite, mem, &mut w);
                for v in [x, y, out_w, out_h, step] {
                    w(v);
                }
            }
            DrawOp::Gradient { x, y, len, ramp, step, noise, pattern, backwards } => {
                for v in [4, x, y, len, ramp, step, noise, pattern, backwards as u16] {
                    w(v);
                }
            }
            DrawOp::Line { x0, y0, x1, y1, colour, pattern, clip } => {
                for v in [5, x0, y0, x1, y1, colour as u16, pattern, clip.x0, clip.y0, clip.x1, clip.y1] {
                    w(v);
                }
            }
            DrawOp::Fill { rect, colour } => {
                for v in [6, rect.x0, rect.y0, rect.x1, rect.y1, colour as u16] {
                    w(v);
                }
            }
            DrawOp::Clear => w(7),
            // Anything else in a room (not seen so far): its kind only.
            ref other => {
                w(8);
                b.extend_from_slice(format!("{:?}", std::mem::discriminant(other)).as_bytes());
            }
        }
        self.0 = fnv64(&b);
    }
}

/// A sprite by content: its stored data, size, flags and palette offset.
fn sprite_id(s: &SpriteRef, mem: Mem, w: &mut impl FnMut(u16)) {
    let rb = sprite::row_bytes(s.format, s.wflags);
    let max = (rb * s.height as usize * 2 + 16).min(0x10000 - s.off as usize);
    let data = mem.bytes(s.seg, s.off, max);
    let used = sprite::decode(&data, s.wflags, s.height, s.format).map(|(_, used)| used).unwrap_or(0);
    let h = fnv64(&data[..used]);
    for i in 0..4 {
        w((h >> (16 * i)) as u16);
    }
    w(s.wflags);
    w(s.height as u16);
    w(match s.format {
        Format::Nibble { pal } => pal as u16,
        Format::Byte => 0x1fe,
        Format::ByteKey => 0x1ff,
    });
}

/// Follows the drawing calls of one room (between `draw_SAL`'s entry and
/// return): its key, the buffer it draws into and the bytes it wrote, and
/// the bytes characters standing in it were drawn over.
#[derive(Clone)]
pub struct RoomTrack {
    pub key: RoomKey,
    /// The buffer the room draws into (its first call's target).
    pub target: Option<u16>,
    pub calls: usize,
    /// Bytes of the target the room's own calls wrote.
    pub drawn: Vec<bool>,
    /// Which of the room's calls (1-based) wrote each byte last (0: none):
    /// where two neighbours differ, two pieces of the room meet.
    pub owner: Vec<u16>,
    /// Bytes characters were drawn over.
    pub overdrawn: Vec<bool>,
    characters: u32,
}

impl Default for RoomTrack {
    fn default() -> Self {
        RoomTrack {
            key: RoomKey::new(),
            target: None,
            calls: 0,
            drawn: vec![false; 0x10000],
            owner: vec![0; 0x10000],
            overdrawn: vec![false; 0x10000],
            characters: 0,
        }
    }
}

impl RoomTrack {
    /// A character standing in the room starts (`true`) or ends being drawn.
    pub fn character(&mut self, begin: bool) {
        self.characters = if begin { self.characters + 1 } else { self.characters.saturating_sub(1) };
    }

    /// A drawing call during the room (`mem` as at the call). The mouse
    /// cursor (drawn from an interrupt at any time) is not part of a room,
    /// nor are calls to another buffer than the room's.
    pub fn follow(&mut self, op: &DrawOp, r: &Regs, mem: Mem) {
        if matches!(op, DrawOp::Cursor { .. } | DrawOp::CursorRestore { .. }) {
            return;
        }
        let Some(target) = op.target(r) else { return };
        if self.characters > 0 {
            if self.target == Some(target) {
                cover(op, r, mem, &mut self.overdrawn);
            }
            return;
        }
        if *self.target.get_or_insert(target) != target {
            return;
        }
        self.key.add(op, r, mem);
        self.calls += 1;
        walk(op, r, mem, &mut Owners { drawn: &mut self.drawn, owner: &mut self.owner, call: self.calls.min(0xffff) as u16 });
    }
}

struct Owners<'a> {
    drawn: &'a mut [bool],
    owner: &'a mut [u16],
    call: u16,
}

impl Sink for Owners<'_> {
    #[inline]
    fn put(&mut self, at: u16, _value: u8, _px: Px) {
        self.drawn[at as usize] = true;
        self.owner[at as usize] = self.call;
    }
}

/// The bytes an operation writes (a transparent sprite pixel is not written).
pub struct Coverage<'a>(pub &'a mut [bool]);

impl Sink for Coverage<'_> {
    #[inline]
    fn put(&mut self, at: u16, _value: u8, _px: Px) {
        self.0[at as usize] = true;
    }
}

/// Mark the bytes `op` writes in `cover` (65536 entries, the target segment).
pub fn cover(op: &DrawOp, r: &Regs, mem: Mem, cover: &mut [bool]) -> bool {
    walk(op, r, mem, &mut Coverage(cover))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::Rect;

    #[test]
    fn keys_follow_content_not_addresses() {
        let mut mem = vec![0u8; 0x30000];
        // The same 2×1 raw 8-bit sprite at two addresses.
        mem[0x10000..0x10002].copy_from_slice(&[5, 6]);
        mem[0x20000..0x20002].copy_from_slice(&[5, 6]);
        let s = |seg| SpriteRef { wflags: 2, height: 1, format: Format::Byte, seg, off: 0 };
        let r = Regs::default();
        let key = |seg| {
            let mut k = RoomKey::new();
            k.add(&DrawOp::Blit { sprite: s(seg), x: 3, y: 4 }, &r, Mem(&mem));
            k.value()
        };
        assert_eq!(key(0x1000), key(0x2000));
        let mut moved = RoomKey::new();
        moved.add(&DrawOp::Blit { sprite: s(0x1000), x: 4, y: 4 }, &r, Mem(&mem));
        assert_ne!(moved.value(), key(0x1000));
    }

    #[test]
    fn lines_cover_their_pixels() {
        let mem = vec![0u8; 0x10];
        let mut c = vec![false; 0x10000];
        let clip = Rect { x0: 0, y0: 0, x1: 320, y1: 152 };
        let op = DrawOp::Line { x0: 10, y0: 5, x1: 13, y1: 5, colour: 7, pattern: 0xffff, clip };
        assert!(cover(&op, &Regs::default(), Mem(&mem), &mut c));
        let set: Vec<usize> = (0..0x10000).filter(|&i| c[i]).collect();
        assert_eq!(set, vec![5 * 320 + 10, 5 * 320 + 11, 5 * 320 + 12, 5 * 320 + 13]);
    }
}
