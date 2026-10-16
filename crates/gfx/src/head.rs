//! Talking heads: what `draw_talking_head_at_si` (1000:9D2D) draws.
//!
//! The game expands a head frame (a string of group bytes) into a list of
//! parts, (sprite + 1, x, y) from the head's origin
//! (`setup_non_lip_sync_data_structure`, 1000:9BEE), and draws every part
//! with one driver call (slot 6, clipped). A whole frame and a lip-sync
//! mouth (a group drawn over the face) both pass through it. A partial
//! redraw (1000:9BB1: the background restored in the changed rectangle)
//! still draws every part of the frame, clipped to that rectangle.
//!
//! A draw is keyed by its parts: each sprite's picture
//! ([`crate::hash::image_hash`]) and its place relative to the parts'
//! top-left, in any order (frames list the same parts in different orders;
//! where their overlaps differ, the plate is not used, since it no longer
//! depicts those pixels). `dune-hd` computes the same key from the head's
//! layout in `DUNE.DAT`, so a pack's plate for a frame or a mouth is found
//! whatever the head's position or the memory the game drew it from.

use crate::{
    Mem,
    hash::{fnv64, image_hash},
    model::{Px, Sink, walk},
    ops::{DrawOp, Regs, SpriteRef},
    sprite,
};

/// The key of a talking-head draw: parts as (picture hash, x, y), in any
/// order; positions in any common origin.
pub fn key(parts: &[(u64, i32, i32)]) -> u64 {
    let (x0, y0) = origin(parts);
    let mut sorted = parts.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    let mut b = Vec::with_capacity(sorted.len() * 16);
    for &(h, x, y) in &sorted {
        b.extend_from_slice(&h.to_le_bytes());
        b.extend_from_slice(&((x - x0) as i16).to_le_bytes());
        b.extend_from_slice(&((y - y0) as i16).to_le_bytes());
    }
    fnv64(&b) ^ 0x4845_4144 // "HEAD"
}

/// The parts' top-left (the smallest x and y): where a head plate starts.
pub fn origin(parts: &[(u64, i32, i32)]) -> (i32, i32) {
    (parts.iter().map(|p| p.1).min().unwrap_or(0), parts.iter().map(|p| p.2).min().unwrap_or(0))
}

/// The picture of a sprite as the game draws it.
pub fn picture_hash(s: &SpriteRef, mem: Mem) -> Option<u64> {
    let rb = sprite::row_bytes(s.format, s.wflags);
    let max = (rb * s.height as usize * 2 + 16).min(0x10000 - s.off as usize);
    let data = mem.bytes(s.seg, s.off, max);
    let (img, _) = sprite::decode(&data, s.wflags, s.height, s.format)?;
    Some(image_hash(&img.px, img.stride, img.height))
}

/// Follows one call of `draw_talking_head_at_si`: its parts, the buffer it
/// draws into and the bytes it wrote.
pub struct HeadTrack {
    pub parts: Vec<(u64, i32, i32)>,
    pub target: Option<u16>,
    pub y_offset: u16,
    pub drawn: Vec<bool>,
    /// A call this tracker could not identify (the draw gets no plate).
    pub unknown: bool,
}

impl Default for HeadTrack {
    fn default() -> Self {
        HeadTrack { parts: Vec::new(), target: None, y_offset: 0, drawn: vec![false; 0x10000], unknown: false }
    }
}

impl HeadTrack {
    pub fn key(&self) -> u64 {
        key(&self.parts)
    }

    /// A drawing call during the head (`mem` as at the call).
    pub fn follow(&mut self, op: &DrawOp, r: &Regs, mem: Mem) {
        if matches!(op, DrawOp::Cursor { .. } | DrawOp::CursorRestore { .. }) {
            return;
        }
        let Some(target) = op.target(r) else { return };
        let (DrawOp::Blit { sprite, x, y } | DrawOp::BlitClipped { sprite, x, y, .. }) = *op else {
            self.unknown = true;
            return;
        };
        if *self.target.get_or_insert(target) != target {
            self.unknown = true;
            return;
        }
        match picture_hash(&sprite, mem) {
            Some(h) => self.parts.push((h, x as i32, y as i32)),
            None => self.unknown = true,
        }
        self.y_offset = r.y_offset;
        walk(op, r, mem, &mut Drawn(&mut self.drawn));
    }
}

struct Drawn<'a>(&'a mut [bool]);

impl Sink for Drawn<'_> {
    #[inline]
    fn put(&mut self, at: u16, _value: u8, _px: Px) {
        self.0[at as usize] = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_follow_parts_not_position() {
        let a = [(1, 10, 20), (2, 14, 30)];
        let moved = [(1, 110, 70), (2, 114, 80)];
        assert_eq!(key(&a), key(&moved));
        assert_ne!(key(&a), key(&[(1, 10, 20), (2, 15, 30)]));
        assert_eq!(key(&a), key(&[(2, 14, 30), (1, 10, 20)]), "the same parts in another order");
    }
}
