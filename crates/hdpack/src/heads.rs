//! Talking heads: how the game assembles a character's face from parts.
//!
//! A talking-head sheet (resource `2 + character`, e.g. LETO.HSQ) keeps its
//! animation in its last entry (`setup_lip_sync_data_from_sprite_sheet`,
//! seg000:91A0; offsets relative to the sprite table `T`):
//! - 4 bytes, then the head's origin rectangle (4 words; parts are drawn at
//!   origin + their offset);
//! - a word: offset (from itself) of the frame tables; then the group table
//!   `G`: words, offsets from `G`, for group bytes 2, 3, …;
//! - each group: (sprite + 1, x, y) byte triples, ended by 0
//!   (`setup_non_lip_sync_data_structure`, seg000:9BEE);
//! - frame tables: a table of offsets (from itself) to frames, its last entry
//!   being the lip-sync (mouth) table; a frame is a string of group bytes
//!   (a byte 1 adds 256 to the next one, minus 2), ended by 0.

use std::path::Path;

use gfx::ops::Format;

use crate::catalog::{self, Asset, Observed};

#[derive(Clone, Debug)]
pub struct Part {
    /// Sprite index in the sheet.
    pub sprite: usize,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug)]
pub struct Head {
    pub origin: [u16; 4],
    pub groups: Vec<Vec<Part>>,
    /// Frames of the frame tables, as group indices (0-based), deduplicated.
    pub frames: Vec<Vec<usize>>,
    /// The lip-sync table: the frames (group lists) drawn while the character talks, in order.
    pub lipsync: Vec<Vec<usize>>,
}

pub fn parse(res: &[u8]) -> Option<Head> {
    let u16_at = |o: usize| res.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let t = u16_at(0)?;
    let first = u16_at(t)?;
    if first < 4 || first % 2 != 0 {
        return None;
    }
    let last = t + u16_at(t + first - 2)?;
    let r = last + 4;
    let origin = [u16_at(r)? as u16, u16_at(r + 2)? as u16, u16_at(r + 4)? as u16, u16_at(r + 6)? as u16];
    if origin[0] >= 320 || origin[1] >= 200 {
        return None;
    }
    let tables = r + 8 + u16_at(r + 8)?;
    let g = r + 10;
    if tables <= g || tables > res.len() {
        return None;
    }
    let sprites = first / 2;
    // Groups: as many table words as fit before the first group's data.
    let first_group = g + u16_at(g)?;
    let count = first_group.checked_sub(g)? / 2;
    let mut groups = Vec::new();
    for i in 0..count {
        let mut at = g + u16_at(g + 2 * i)?;
        let mut parts = Vec::new();
        loop {
            let s = *res.get(at)? as usize;
            if s == 0 {
                break;
            }
            if s - 1 >= sprites {
                return None;
            }
            parts.push(Part { sprite: s - 1, x: *res.get(at + 1)? as i32, y: *res.get(at + 2)? as i32 });
            at += 3;
        }
        groups.push(parts);
    }
    // Frames: every string the frame tables point at.
    let mut frames: Vec<Vec<usize>> = Vec::new();
    let read_table = |table: usize, frames: &mut Vec<Vec<usize>>| -> Option<()> {
        let n = u16_at(table)? / 2;
        for i in 0..n {
            let mut at = table + u16_at(table + 2 * i)?;
            let mut f = Vec::new();
            loop {
                let mut b = *res.get(at)? as usize;
                at += 1;
                if b == 0 {
                    break;
                }
                if b == 1 {
                    b = 256 + *res.get(at)? as usize;
                    at += 1;
                }
                f.push(b.checked_sub(2)?);
            }
            if !f.is_empty() && f.iter().all(|&g| g < count) && !frames.contains(&f) {
                frames.push(f);
            }
        }
        Some(())
    };
    read_table(tables, &mut frames)?;
    let n = u16_at(tables)? / 2;
    let mouth = tables + u16_at(tables + 2 * (n - 1))?;
    let mut lipsync = Vec::new();
    let _ = read_table(mouth, &mut lipsync);
    for f in &lipsync {
        if !frames.contains(f) {
            frames.push(f.clone());
        }
    }
    Some(Head { origin, groups, frames, lipsync })
}

/// The picture of `parts` drawn in order (palette indices, `None` where
/// nothing is drawn), with its top-left corner, from the sheet's `sprites`.
pub fn compose(parts: &[Part], sprites: &[Option<&Asset>]) -> Option<(i32, i32, usize, usize, Vec<Option<u8>>)> {
    let mut x0 = i32::MAX;
    let mut y0 = i32::MAX;
    let mut x1 = i32::MIN;
    let mut y1 = i32::MIN;
    for p in parts {
        let a = sprites.get(p.sprite).copied().flatten()?;
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x + a.stride as i32);
        y1 = y1.max(p.y + a.height as i32);
    }
    if parts.is_empty() {
        return None;
    }
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let mut px = vec![None; w * h];
    for p in parts {
        let a = sprites[p.sprite].unwrap();
        for y in 0..a.height {
            for x in 0..a.width {
                let i = y * a.stride + x;
                if a.solid(i) {
                    px[(p.y - y0) as usize * w + y * w + (p.x - x0) as usize + x] = Some(a.global(a.local[i]));
                }
            }
        }
    }
    Some((x0, y0, w, h, px))
}

/// A face assembled the way the game shows it, upscaled in one piece; the
/// parts listed are then cut out of it (in their place, at their size), so
/// each part's art continues the face around it.
pub struct Context {
    /// The composed picture (global palette indices, the sheet's palette;
    /// `cover`: drawn pixels).
    pub picture: Asset,
    /// (sprite `hash`, x, y) of each part to cut out of the picture.
    pub parts: Vec<(u64, usize, usize)>,
}

/// For every talking head: its default frame (the first), in which its own
/// parts are cut; and that frame with each other group drawn over it (a
/// mouth, eyes, a gesture), in which that group's parts are cut. A part
/// keeps the first context it appears in.
pub fn contexts(dat: &Path, observed: &Observed) -> std::io::Result<Vec<Context>> {
    let mut f = std::fs::File::open(dat)?;
    let mut out = Vec::new();
    let mut done = std::collections::HashSet::new();
    for e in gfx::dat::toc(&mut f)? {
        let Ok(res) = gfx::dat::load(&mut f, &e) else { continue };
        let Some(head) = parse(&res) else { continue };
        let Some(sheet) = catalog::sheet_assets(&e.name, &res, observed) else { continue };
        let refs: Vec<Option<&Asset>> = sheet.iter().map(|a| a.as_ref()).collect();
        let Some(base) = head.frames.first() else { continue };
        let base: Vec<Part> = base.iter().flat_map(|&g| head.groups[g].iter().cloned()).collect();
        let mut variants: Vec<(Vec<Part>, Vec<Part>)> = vec![(base.clone(), base.clone())];
        for g in &head.groups {
            let mut all = base.clone();
            all.extend(g.iter().cloned());
            variants.push((all, g.clone()));
        }
        for (all, cut) in variants {
            let cut: Vec<&Part> = cut.iter().filter(|p| refs.get(p.sprite).copied().flatten().is_some_and(|a| !done.contains(&a.hash))).collect();
            if cut.is_empty() {
                continue;
            }
            let Some((x0, y0, w, h, px)) = compose(&all, &refs) else { continue };
            // Only around the parts to cut (and a margin of face): the
            // model sees what touches them, at a fraction of the cost.
            const MARGIN: i32 = 16;
            let (mut cx0, mut cy0, mut cx1, mut cy1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
            for p in &cut {
                let a = refs[p.sprite].unwrap();
                cx0 = cx0.min(p.x - x0 - MARGIN);
                cy0 = cy0.min(p.y - y0 - MARGIN);
                cx1 = cx1.max(p.x - x0 + a.stride as i32 + MARGIN);
                cy1 = cy1.max(p.y - y0 + a.height as i32 + MARGIN);
            }
            let (cx0, cy0) = (cx0.max(0) as usize, cy0.max(0) as usize);
            let (cx1, cy1) = ((cx1 as usize).min(w), (cy1 as usize).min(h));
            let px: Vec<Option<u8>> = (cy0..cy1).flat_map(|y| (cx0..cx1).map(move |x| (x, y))).map(|(x, y)| px[y * w + x]).collect();
            let (x0, y0, w, h) = (x0 + cx0 as i32, y0 + cy0 as i32, cx1 - cx0, cy1 - cy0);
            let Some(first) = refs.iter().flatten().next() else { continue };
            let local: Vec<u8> = px.iter().map(|p| p.unwrap_or(0)).collect();
            let key = gfx::hash::fnv64(&px.iter().flat_map(|p| p.map_or([0, 0], |v| [1, v])).collect::<Vec<u8>>()) ^ (w as u64) << 48 ^ (h as u64) << 32;
            let parts: Vec<(u64, usize, usize)> = cut
                .iter()
                .filter_map(|p| {
                    let a = refs[p.sprite]?;
                    done.insert(a.hash);
                    Some((a.hash, (p.x - x0) as usize, (p.y - y0) as usize))
                })
                .collect();
            out.push(Context {
                picture: Asset {
                    hash: key,
                    content: key,
                    res: format!("FACE-{}", e.name.trim_end_matches(".HSQ")),
                    part: out.len(),
                    format: Format::Byte,
                    width: w,
                    stride: w,
                    height: h,
                    cover: Some(px.iter().map(|p| p.is_some()).collect()),
                    room: false,
                    local,
                    palette: first.palette,
                },
                parts,
            });
        }
    }
    Ok(out)
}
