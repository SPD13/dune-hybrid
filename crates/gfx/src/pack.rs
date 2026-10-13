//! HD art packs (made by `dune-hd`): a stored ZIP with `a4/<hash>.png` and
//! `a2/<hash>.png` per sprite, RGBA texels R = a, G = b (palette-local
//! values), B = weight of b, A = coverage.

use std::{
    collections::HashMap,
    io::{self, Read, Seek, SeekFrom},
};

/// Text art: a sprite of two colours or fewer with no transparent pixel
/// (`opaque` says which palette-local values are drawn). The intro's
/// subtitle lines are such sprites, text pre-rendered on its background. A
/// pack keeps the exact silhouette of sprites with transparency, but here
/// the letters' shapes are only the border between the two colours: an
/// upscaler blurs it and the projection fills the holes of letters. Text
/// art keeps the built-in MMPX, which follows the strokes exactly.
pub fn text_art(px: &[u8], opaque: impl Fn(u8) -> bool) -> bool {
    let mut seen: [Option<u8>; 2] = [None; 2];
    for &v in px {
        if !opaque(v) {
            return false;
        }
        match seen {
            [None, _] => seen[0] = Some(v),
            [Some(a), None] if a != v => seen[1] = Some(v),
            [Some(a), Some(b)] if a != v && b != v => return false,
            _ => {}
        }
    }
    true
}

/// A sprite's HD art at one scale, in palette-local values.
pub struct PackSprite {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 4]>,
}

/// Where each stored entry's data is in the ZIP.
pub struct PackIndex {
    entries: HashMap<String, (u64, u32)>,
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

impl PackIndex {
    /// Read the central directory (stored entries only).
    pub fn read<R: Read + Seek>(r: &mut R) -> io::Result<PackIndex> {
        let len = r.seek(SeekFrom::End(0))?;
        let tail_len = len.min(65557);
        let mut tail = vec![0; tail_len as usize];
        r.seek(SeekFrom::Start(len - tail_len))?;
        r.read_exact(&mut tail)?;
        let eocd = (0..tail.len().saturating_sub(21)).rev().find(|&i| u32_at(&tail, i) == 0x0605_4b50).ok_or_else(|| io::Error::other("not a ZIP file"))?;
        let (count, cd_len, cd_off) = (u16_at(&tail, eocd + 10) as usize, u32_at(&tail, eocd + 12) as usize, u32_at(&tail, eocd + 16) as u64);
        let mut cd = vec![0; cd_len];
        r.seek(SeekFrom::Start(cd_off))?;
        r.read_exact(&mut cd)?;
        let mut entries = HashMap::new();
        let mut o = 0;
        for _ in 0..count {
            if o + 46 > cd.len() || u32_at(&cd, o) != 0x0201_4b50 {
                break;
            }
            let method = u16_at(&cd, o + 10);
            let size = u32_at(&cd, o + 24);
            let (name_len, extra, comment) = (u16_at(&cd, o + 28) as usize, u16_at(&cd, o + 30) as usize, u16_at(&cd, o + 32) as usize);
            let local = u32_at(&cd, o + 42) as u64;
            let name = String::from_utf8_lossy(&cd[o + 46..o + 46 + name_len]).into_owned();
            if method == 0 {
                // Data follows the local header (whose name/extra lengths may differ).
                let mut lh = [0u8; 30];
                r.seek(SeekFrom::Start(local))?;
                r.read_exact(&mut lh)?;
                let data = local + 30 + u16_at(&lh, 26) as u64 + u16_at(&lh, 28) as u64;
                entries.insert(name, (data, size));
            }
            o += 46 + name_len + extra + comment;
        }
        Ok(PackIndex { entries })
    }

    pub fn read_entry<R: Read + Seek>(&self, r: &mut R, name: &str) -> Option<Vec<u8>> {
        let &(off, len) = self.entries.get(name)?;
        let mut data = vec![0; len as usize];
        r.seek(SeekFrom::Start(off)).ok()?;
        r.read_exact(&mut data).ok()?;
        Some(data)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The art for sprite `hash` at scale `k`, if the pack has it.
    pub fn sprite<R: Read + Seek>(&self, r: &mut R, hash: u64, k: usize) -> Option<PackSprite> {
        decode(&self.read_entry(r, &format!("a{k}/{hash:016x}.png"))?)
    }
}

/// Decode a pack PNG (8-bit RGBA).
pub fn decode(png_data: &[u8]) -> Option<PackSprite> {
    let mut reader = png::Decoder::new(io::Cursor::new(png_data)).read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    let (w, h) = (info.width as usize, info.height as usize);
    Some(PackSprite { w, h, px: buf[..w * h * 4].as_chunks::<4>().0.to_vec() })
}

#[cfg(test)]
mod tests {
    use super::text_art;

    #[test]
    fn text_art_is_opaque_with_at_most_two_colours() {
        let all = |_| true;
        assert!(text_art(&[7, 7, 7], all));
        assert!(text_art(&[0, 0x2c, 0, 0x2c, 0x2c], all));
        assert!(!text_art(&[0, 0x2c, 0, 3], all));
        // Transparent pixels: the pack keeps the silhouette, not text art.
        assert!(!text_art(&[0, 0x2c, 0x2c], |v| v != 0));
    }
}
