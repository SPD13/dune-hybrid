//! Sprite sheets: the game's image resources.
//!
//! Layout: `u16` offset `T` of the sprite table (palette chunks, if any, sit
//! between byte 2 and `T`); at `T`, `u16` offsets relative to `T`, the first
//! of which also gives the count (`offset / 2`); each sprite is a 4-byte
//! header (`u16` width and flags, `u8` height, `u8` palette offset) and its
//! pixel data.

use crate::{
    hash::{image_hash, sprite_hash},
    ops::Format,
    sprite::{self, Image},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SheetSprite {
    pub index: usize,
    /// Offset of the pixel data in the resource.
    pub data: usize,
    pub wflags: u16,
    pub height: u8,
    pub pal: u8,
    pub data_len: usize,
    /// `hash::sprite_hash` of the stored data.
    pub hash: u64,
    /// `hash::image_hash` of the decoded picture.
    pub content: u64,
}

impl SheetSprite {
    pub fn format(&self) -> Format {
        Format::from_pal(self.pal)
    }
    pub fn width(&self) -> u16 {
        self.wflags & 0x1ff
    }
    pub fn image(&self, res: &[u8]) -> Option<Image> {
        sprite::decode(&res[self.data..], self.wflags, self.height, self.format()).map(|(i, _)| i)
    }
}

/// Parse `res` as a sprite sheet; None if it is not one.
pub fn parse(res: &[u8]) -> Option<Vec<SheetSprite>> {
    let u16_at = |o: usize| res.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let table = u16_at(0)?;
    let first = u16_at(table)?;
    if first < 2 || first % 2 != 0 || table + first > res.len() {
        return None;
    }
    let count = first / 2;
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let start = table + u16_at(table + index * 2)?;
        let end = if index + 1 < count { table + u16_at(table + (index + 1) * 2)? } else { res.len() };
        if start + 4 > res.len() || end < start + 4 || end > res.len() {
            return None;
        }
        let wflags = u16_at(start)? as u16;
        let height = res[start + 2];
        let pal = res[start + 3];
        if wflags & 0x1ff == 0 || height == 0 {
            // Empty slots exist in some sheets.
            out.push(SheetSprite { index, data: start + 4, wflags, height, pal, data_len: 0, hash: 0, content: 0 });
            continue;
        }
        let (img, used) = sprite::decode(&res[start + 4..end], wflags, height, Format::from_pal(pal))?;
        let data = &res[start + 4..start + 4 + used];
        let content = image_hash(&img.px, img.stride, img.height);
        out.push(SheetSprite { index, data: start + 4, wflags, height, pal, data_len: used, hash: sprite_hash(data, wflags, height), content });
    }
    Some(out)
}
