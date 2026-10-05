//! Reference HD compositor (CPU): follows the game's drawing operations and
//! keeps, for every buffer the game draws into, a `k`× shadow made of HD
//! texels, plus the low-resolution picture that shadow depicts (its
//! *provenance*, `P`).
//!
//! The model ([`crate::model::walk`]) says where every pixel of an operation
//! goes and where it comes from; the compositor writes the matching HD block:
//! - sprites: the sprite magnified by MMPX (2× per pass), so the art keeps
//!   the game's own colours, and therefore its palette effects;
//! - glyphs: a smoothed mask of the 1-bit glyph, blended over what is below;
//! - fills: solid blocks;
//! - copies: the source buffer's HD blocks (and provenance).
//!
//! When the picture is presented, a screen pixel shows its HD block only if
//! the real screen still holds the value the shadow depicts (`A000 == P`),
//! there and at its eight neighbours;
//! anything drawn some other way (game code writing memory directly,
//! operations not modelled yet) falls back to the low-resolution pixel. The
//! result is exact by construction: the low-resolution screen is the truth.

use std::collections::HashMap;

use crate::{
    Mem,
    hash::fnv64,
    mmpx::mmpx2x,
    model::{Px, Sink, walk},
    ops::{DrawOp, Format, Regs, SpriteRef},
    sprite,
};

/// One HD texel: colour = mix(palette[a], palette[b], t / 255).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Texel {
    pub a: u8,
    pub b: u8,
    pub t: u8,
}

impl Texel {
    #[inline]
    pub const fn solid(v: u8) -> Texel {
        Texel { a: v, b: v, t: 0 }
    }
    /// The palette index this texel mostly shows.
    #[inline]
    pub fn dominant(self) -> u8 {
        if self.t < 128 { self.a } else { self.b }
    }
}

pub type Dac = [[u8; 3]; 256];

/// A buffer's HD shadow: `k`×`k` texels per byte of the 64 KB segment
/// (block-major: the block of offset `o` starts at `o * k * k`).
pub struct Shadow {
    /// The low-resolution picture the HD texels depict.
    pub low: Vec<u8>,
    /// Bytes whose HD block is only the low-resolution pixel magnified.
    pub lowres: Vec<bool>,
    pub hd: Vec<Texel>,
    /// Rows (of 320 bytes) whose shadow changed since the last
    /// [`Compositor::take_screen_dirty`].
    pub dirty: Vec<bool>,
}

impl Shadow {
    fn seeded(k: usize, mem: Mem, seg: u16) -> Shadow {
        let low = mem.bytes(seg, 0, 0x10000);
        let hd = low.iter().flat_map(|&v| std::iter::repeat_n(Texel::solid(v), k * k)).collect();
        Shadow { low, lowres: vec![true; 0x10000], hd, dirty: vec![true; 205] }
    }
}

/// A sprite magnified: `w` × `h` texels, `None` where transparent.
struct HdSprite {
    w: usize,
    texels: Vec<Option<Texel>>,
    /// From an art pack: its blocks may average dithering out, so they are
    /// not forced to show their own pixel's colour.
    from_art: bool,
}

/// Supplies HD art for a sprite (by `hash::image_hash`) at scale `k`:
/// typically an HD art pack.
pub type ArtSource = Box<dyn FnMut(u64, usize) -> Option<crate::pack::PackSprite>>;

/// A glyph's smoothed mask: coverage 0-255 per texel.
struct HdGlyph {
    w: usize,
    h: usize,
    cover: Vec<u8>,
}

#[derive(Default, Clone, Copy, Debug)]
pub struct PresentStats {
    /// Screen pixels shown from HD texels.
    pub hd: u32,
    /// Screen pixels shown from the low-resolution picture.
    pub fallback: u32,
    /// HD pixels whose block mostly shows another colour than the screen's.
    pub dominance_errors: u32,
}

pub struct Compositor {
    pub k: usize,
    buffers: HashMap<u16, Shadow>,
    sprites: HashMap<u64, HdSprite>,
    /// Insertion order of `sprites`, for evicting the oldest.
    sprite_order: std::collections::VecDeque<u64>,
    sprite_texels: usize,
    /// Upper bound on cached HD sprite texels (memory is ~1 byte each).
    pub sprite_budget: usize,
    glyphs: HashMap<u64, HdGlyph>,
    /// Calls the compositor could not follow (unmodelled slots).
    pub unfollowed: u64,
    /// Debug: report every write to this offset (any buffer) on stderr.
    pub watch: Option<u16>,
    /// HD art to use instead of the built-in MMPX, when it has the sprite.
    art: Option<ArtSource>,
    /// Sprites drawn from `art` / magnified with MMPX (cache misses).
    pub art_hits: u64,
    pub art_misses: u64,
}

const SCREEN: u16 = 0xa000;

impl Compositor {
    /// `k` is 2 or 4.
    pub fn new(k: usize) -> Compositor {
        assert!(k == 2 || k == 4);
        Compositor {
            k,
            buffers: HashMap::new(),
            sprites: HashMap::new(),
            sprite_order: Default::default(),
            sprite_texels: 0,
            sprite_budget: 64 << 20,
            glyphs: HashMap::new(),
            unfollowed: 0,
            watch: None,
            art: None,
            art_hits: 0,
            art_misses: 0,
        }
    }

    /// Use HD art from `art` (an HD art pack) where it has a sprite.
    pub fn set_art(&mut self, art: Option<ArtSource>) {
        self.art = art;
        self.sprites.clear();
        self.sprite_order.clear();
        self.sprite_texels = 0;
    }

    /// Forget everything (after a snapshot restore): shadows are re-seeded
    /// from memory, as low-resolution.
    pub fn reset(&mut self) {
        self.buffers.clear();
    }

    pub fn shadow(&self, seg: u16) -> Option<&Shadow> {
        self.buffers.get(&seg)
    }

    fn ensure(&mut self, seg: u16, mem: Mem) {
        let k = self.k;
        self.buffers.entry(seg).or_insert_with(|| Shadow::seeded(k, mem, seg));
    }

    /// After a call the compositor could not follow (an unmodelled slot)
    /// wrote to `target`: bring the shadow back in line with memory. Bytes
    /// that now equal what another buffer depicts at the same offset take
    /// that buffer's HD block (transitions and effects that move pictures
    /// between buffers in place); `prefer` lists buffers to try first (the
    /// call's source segments). Anything else becomes low-resolution.
    pub fn resync(&mut self, target: u16, mem: Mem, prefer: &[u16]) {
        if !self.buffers.contains_key(&target) {
            return;
        }
        let kk = self.k * self.k;
        let mut dst = self.buffers.remove(&target).unwrap();
        let mut order: Vec<u16> = prefer.iter().copied().filter(|s| self.buffers.contains_key(s)).collect();
        for &seg in self.buffers.keys() {
            if !order.contains(&seg) {
                order.push(seg);
            }
        }
        let base = (target as usize) << 4;
        for o in 0..0x10000usize {
            let v = mem.0.get(base + o).copied().unwrap_or(0);
            if dst.low[o] == v {
                continue;
            }
            dst.low[o] = v;
            dst.dirty[o / 320] = true;
            match order.iter().map(|seg| &self.buffers[seg]).find(|src| src.low[o] == v) {
                Some(src) => {
                    dst.hd[o * kk..(o + 1) * kk].copy_from_slice(&src.hd[o * kk..(o + 1) * kk]);
                    dst.lowres[o] = src.lowres[o];
                }
                None => {
                    dst.hd[o * kk..(o + 1) * kk].fill(Texel::solid(v));
                    dst.lowres[o] = true;
                }
            }
        }
        self.buffers.insert(target, dst);
    }

    /// Follow a driver call, at its entry (`mem` before the call).
    pub fn apply(&mut self, op: &DrawOp, r: &Regs, mem: Mem, dac: &Dac) {
        let Some(target) = op.target(r) else { return };
        if matches!(op, DrawOp::Other { .. }) {
            self.unfollowed += 1;
            return;
        }
        self.ensure(SCREEN, mem);
        self.ensure(target, mem);
        let source = match op {
            DrawOp::Blit { sprite, .. } | DrawOp::BlitClipped { sprite, .. } => self.sprite_source(sprite, mem, dac, false),
            DrawOp::BlitScaled { sprite, .. } => self.sprite_source(sprite, mem, dac, true),
            DrawOp::Glyph { w, h, seg, off, .. } => {
                let bits = mem.bytes(*seg, *off, *h as usize);
                Source::Glyph(self.glyph(&bits, *w, *h))
            }
            _ => Source::None,
        };
        let (mirror_x, mirror_y) = match op {
            DrawOp::Blit { sprite, .. } | DrawOp::BlitScaled { sprite, .. } => (sprite.hflip(), sprite.vflip()),
            _ => (false, false),
        };
        let (fg, bg) = match op {
            DrawOp::Glyph { fg, bg, .. } => (*fg, *bg),
            _ => (0, None),
        };
        // Take the target out of the map so the sink can read other buffers.
        let mut dst = self.buffers.remove(&target).unwrap();
        let mut sink = HdSink {
            k: self.k,
            dst: &mut dst,
            others: &self.buffers,
            target,
            source: &source,
            sprites: &self.sprites,
            glyphs: &self.glyphs,
            mirror_x,
            mirror_y,
            fg,
            bg,
        };
        walk(op, r, mem, &mut sink);
        if let Some(w) = self.watch {
            let kk = self.k * self.k;
            eprintln!(
                "watch {w}: {target:04x} {:?} low {:02x} lowres {} hd {:?}",
                op,
                dst.low[w as usize],
                dst.lowres[w as usize],
                &dst.hd[w as usize * kk..(w as usize + 1) * kk]
            );
        }
        self.buffers.insert(target, dst);
    }

    /// Where a sprite's HD texels come from: its magnified art, or the HD
    /// shadow of the buffer it is read from (images the game composed itself,
    /// e.g. subtitle lines).
    /// `scaled`: drawn through the scaling path, which only reads the rows it
    /// needs, so the sprite may be taller than `s.height`.
    fn sprite_source(&mut self, s: &SpriteRef, mem: Mem, dac: &Dac, scaled: bool) -> Source {
        let lin = ((s.seg as usize) << 4) + s.off as usize;
        if !matches!(s.format, Format::Nibble { .. }) && !s.rle() {
            for &seg in self.buffers.keys() {
                let base = (seg as usize) << 4;
                if (base..base + 0x10000).contains(&lin) {
                    return Source::Buffer { seg, start: (lin - base) as u16, stride: s.width() };
                }
            }
        }
        let rb = sprite::row_bytes(s.format, s.wflags);
        let max = (rb * s.height as usize * 2 + 16).min(0x10000 - s.off as usize);
        let data = mem.bytes(s.seg, s.off, max);
        let Some((img, used)) = sprite::decode(&data, s.wflags, s.height, s.format) else {
            return Source::None;
        };
        let pal = match s.format {
            Format::Nibble { pal } => pal,
            _ => 0,
        };
        let key = fnv64(&data[..used]) ^ ((s.wflags & 0x81ff) as u64) << 40 ^ (s.height as u64) << 52 ^ (pal as u64) << 32 ^ (self.k as u64) << 60;
        if !self.sprites.contains_key(&key) {
            let mut art = self.art_texels(s, &img);
            // A scaled draw does not say how tall its (raw) sprite is: the
            // game decoded the whole sprite there, so try a few more rows.
            if art.is_none() && scaled && self.art.is_some() {
                for h in s.height as usize + 1..=(s.height as usize + 16).min(255) {
                    let raw = mem.bytes(s.seg, s.off, rb * h);
                    if let Some((taller, _)) = sprite::decode(&raw, s.wflags & 0x7fff, h as u8, s.format) {
                        art = self.art_texels(s, &taller);
                        if art.is_some() {
                            break;
                        }
                    }
                }
            }
            if art.is_some() {
                self.art_hits += 1;
            } else if self.art.is_some() {
                self.art_misses += 1;
            }
            let from_art = art.is_some();
            let texels = art.unwrap_or_else(|| self.mmpx_texels(s, &img, dac));
            self.sprite_texels += texels.len();
            self.sprites.insert(key, HdSprite { w: img.stride * self.k, texels, from_art });
            self.sprite_order.push_back(key);
            while self.sprite_texels > self.sprite_budget && self.sprite_order.len() > 1 {
                let old = self.sprite_order.pop_front().unwrap();
                if let Some(s) = self.sprites.remove(&old) {
                    self.sprite_texels -= s.texels.len();
                }
            }
        }
        Source::Sprite(key)
    }

    /// The sprite's texels from the art source, if it has them at this scale.
    fn art_texels(&mut self, s: &SpriteRef, img: &sprite::Image) -> Option<Vec<Option<Texel>>> {
        let k = self.k;
        let (stride, height) = (img.stride, img.height);
        let art = self.art.as_mut()?;
        let p = art(crate::hash::image_hash(&img.px, stride, height), k).filter(|p| p.w == stride * k && p.h == height * k)?;
        let global = |v: u8| match s.format {
            Format::Nibble { pal } => v.wrapping_add(pal),
            _ => v,
        };
        Some(p.px.iter().map(|t| (t[3] >= 128).then(|| Texel { a: global(t[0]), b: global(t[1]), t: t[2] })).collect())
    }

    /// The sprite magnified by MMPX (palette values kept).
    fn mmpx_texels(&self, s: &SpriteRef, img: &sprite::Image, dac: &Dac) -> Vec<Option<Texel>> {
        {
            // Global palette values; 0x100 is "transparent".
            let vals: Vec<u16> = img
                .px
                .iter()
                .map(|&v| match s.format {
                    Format::Nibble { pal } if v != 0 => v.wrapping_add(pal) as u16,
                    Format::Byte => v as u16,
                    Format::ByteKey if v != 0 => v as u16,
                    _ => 0x100,
                })
                .collect();
            let luma = |v: u16| -> f32 {
                if v > 255 {
                    return -1.0;
                }
                let [r, g, b] = dac[v as usize];
                0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32
            };
            let (mut w, mut h, mut px) = (img.stride, img.height, vals);
            for _ in 0..self.k / 2 {
                px = mmpx2x(&px, w, h, &luma);
                w *= 2;
                h *= 2;
            }
            let _ = (w, h);
            px.into_iter().map(|v| (v < 256).then(|| Texel::solid(v as u8))).collect()
        }
    }

    fn glyph(&mut self, bits: &[u8], w: u8, h: u8) -> u64 {
        let key = fnv64(bits) ^ (w as u64) << 48 ^ (h as u64) << 56 ^ (self.k as u64) << 40;
        let k = self.k;
        self.glyphs.entry(key).or_insert_with(|| smooth_glyph(bits, w as usize, h as usize, k));
        key
    }

    /// Render the screen at `k`× into `rgb` (320k × 200k × 3).
    pub fn present(&self, screen: &[u8], dac: &Dac, rgb: &mut [u8], overlay: bool) -> PresentStats {
        let k = self.k;
        let (ow, oh) = (320 * k, 200 * k);
        assert!(rgb.len() >= ow * oh * 3);
        let mut stats = PresentStats::default();
        let shadow = self.buffers.get(&SCREEN);
        let colour = |t: Texel| -> [u8; 3] {
            let (a, b) = (dac[t.a as usize], dac[t.b as usize]);
            let w = t.t as u32;
            [0, 1, 2].map(|i| ((a[i] as u32 * (255 - w) + b[i] as u32 * w + 127) / 255) as u8)
        };
        for p in 0..64000usize {
            let (x, y) = (p % 320, p / 320);
            let v = screen[p];
            let hd = shadow.filter(|s| shows_hd(s, screen, p));
            match hd {
                Some(s) => {
                    stats.hd += 1;
                    let near = neighbourhood(screen, p);
                    let block: Vec<Texel> = s.hd[p * k * k..(p + 1) * k * k].iter().map(|&t| fresh(t, v, &near)).collect();
                    let block = &block[..];
                    let agree = block.iter().filter(|t| t.dominant() == v).count();
                    let wrong = agree * 2 < k * k;
                    if self.watch == Some(p as u16) {
                        eprintln!("present {p}: screen {v:02x} low {:02x} block {block:?} wrong {wrong}", s.low[p]);
                    }
                    if wrong {
                        if self.watch.is_some() && stats.dominance_errors < 5 {
                            eprintln!("dominance error at ({x},{y}): screen {v:02x} block {block:?}");
                        }
                        stats.dominance_errors += 1;
                    }
                    for (i, &t) in block.iter().enumerate() {
                        let mut c = colour(t);
                        if overlay {
                            if wrong {
                                c = [0, 0, 255];
                            } else {
                                c[1] = c[1].saturating_add(40);
                            }
                        }
                        let o = ((y * k + i / k) * ow + x * k + i % k) * 3;
                        rgb[o..o + 3].copy_from_slice(&c);
                    }
                }
                None => {
                    stats.fallback += 1;
                    let mut c = dac[v as usize];
                    if overlay {
                        c[0] = c[0].saturating_add(60);
                    }
                    for i in 0..k * k {
                        let o = ((y * k + i / k) * ow + x * k + i % k) * 3;
                        rgb[o..o + 3].copy_from_slice(&c);
                    }
                }
            }
        }
        stats
    }
}

impl Compositor {
    /// Screen rows whose shadow changed since the last call (all of them
    /// before the screen is first drawn).
    pub fn take_screen_dirty(&mut self) -> Vec<bool> {
        match self.buffers.get_mut(&SCREEN) {
            Some(s) => {
                let rows = s.dirty[..200].to_vec();
                s.dirty.fill(false);
                rows
            }
            None => vec![true; 200],
        }
    }

    /// The screen at `k`× as HD texels for a GPU to resolve with the live
    /// palette: RGBA8 per texel, R = a, G = b, B = t (weight of b), A = 255
    /// for HD texels and 0 where the pixel falls back to low resolution.
    /// Only the screen rows marked in `rows` are written; `row_fallback`
    /// receives those rows' counts of low-resolution pixels.
    pub fn present_texels(&self, screen: &[u8], out: &mut [u8], rows: &[bool], row_fallback: &mut [u16]) -> PresentStats {
        let k = self.k;
        let ow = 320 * k;
        assert!(out.len() >= ow * 200 * k * 4);
        let mut stats = PresentStats::default();
        let shadow = self.buffers.get(&SCREEN);
        for (y, _) in rows.iter().enumerate().filter(|(_, r)| **r) {
            row_fallback[y] = 0;
        }
        for p in (0..64000usize).filter(|p| rows[p / 320]) {
            let (x, y) = (p % 320, p / 320);
            match shadow.filter(|s| shows_hd(s, screen, p)) {
                Some(s) => {
                    stats.hd += 1;
                    let near = neighbourhood(screen, p);
                    for (i, &t) in s.hd[p * k * k..(p + 1) * k * k].iter().enumerate() {
                        let t = fresh(t, screen[p], &near);
                        let o = ((y * k + i / k) * ow + x * k + i % k) * 4;
                        out[o..o + 4].copy_from_slice(&[t.a, t.b, t.t, 255]);
                    }
                }
                None => {
                    stats.fallback += 1;
                    row_fallback[y] += 1;
                    for i in 0..k * k {
                        let o = ((y * k + i / k) * ow + x * k + i % k) * 4;
                        out[o..o + 4].copy_from_slice(&[screen[p], screen[p], 0, 0]);
                    }
                }
            }
        }
        stats
    }
}

/// The values of pixel `p` and its eight neighbours on the screen.
#[inline]
fn neighbourhood(screen: &[u8], p: usize) -> [u8; 9] {
    let (x, y) = ((p % 320) as i32, (p / 320) as i32);
    let mut out = [screen[p]; 9];
    let mut i = 0;
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (nx, ny) = (x + dx, y + dy);
            if (0..320).contains(&nx) && (0..200).contains(&ny) {
                out[i] = screen[(ny * 320 + nx) as usize];
            }
            i += 1;
        }
    }
    out
}

/// An HD texel may only show the pixel's own colour or a colour around it:
/// a sprite's rounded corners take their neighbours' colours, but those
/// neighbours may since have been covered by another sprite (a portrait's
/// hair drawn next to its face). Such stale colours become the pixel's own.
#[inline]
fn fresh(t: Texel, own: u8, near: &[u8; 9]) -> Texel {
    let a = if near.contains(&t.a) { t.a } else { own };
    let b = if near.contains(&t.b) { t.b } else { own };
    Texel { a, b, t: t.t }
}

/// HD only where the shadow depicts this pixel and its eight neighbours
/// (an HD block's edges come from its neighbours' art).
#[inline]
fn shows_hd(s: &Shadow, screen: &[u8], p: usize) -> bool {
    if s.lowres[p] {
        return false;
    }
    let (x, y) = ((p % 320) as i32, (p / 320) as i32);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let (nx, ny) = (x + dx, y + dy);
            if (0..320).contains(&nx) && (0..200).contains(&ny) {
                let q = (ny * 320 + nx) as usize;
                if s.low[q] != screen[q] {
                    return false;
                }
            }
        }
    }
    true
}

/// Drives a [`Compositor`] from driver call events: calls are followed at
/// entry, and after a call that could not be followed the target buffer is
/// resynchronised with memory when it returns.
pub struct Follower {
    pub comp: Compositor,
    calls: Vec<(DrawOp, Regs)>,
}

impl Follower {
    pub fn new(k: usize) -> Follower {
        Follower { comp: Compositor::new(k), calls: Vec::new() }
    }

    /// The game entered driver slot `slot` (`mem` and `dac` before the call).
    pub fn enter(&mut self, slot: u8, r: &Regs, mem: Mem, dac: &Dac) -> DrawOp {
        let op = DrawOp::decode(slot, r, mem);
        self.comp.apply(&op, r, mem, dac);
        self.calls.push((op.clone(), *r));
        op
    }

    /// The innermost call returned (`mem` after it).
    pub fn leave(&mut self, mem: Mem) {
        if let Some((DrawOp::Other { .. }, r)) = self.calls.pop() {
            self.comp.resync(r.es, mem, &[r.ds, r.si]);
        }
    }

    /// Forget calls in progress and all shadows (after a snapshot restore).
    pub fn reset(&mut self) {
        self.calls.clear();
        self.comp.reset();
    }
}

enum Source {
    None,
    Sprite(u64),
    Glyph(u64),
    /// 8-bit pixels read from another (tracked) buffer, `stride` per row.
    Buffer {
        seg: u16,
        start: u16,
        stride: u16,
    },
}

struct HdSink<'a> {
    k: usize,
    dst: &'a mut Shadow,
    others: &'a HashMap<u16, Shadow>,
    target: u16,
    source: &'a Source,
    sprites: &'a HashMap<u64, HdSprite>,
    glyphs: &'a HashMap<u64, HdGlyph>,
    mirror_x: bool,
    mirror_y: bool,
    fg: u8,
    bg: Option<u8>,
}

impl HdSink<'_> {
    fn block(&mut self, at: u16) -> &mut [Texel] {
        let kk = self.k * self.k;
        self.dst.dirty[at as usize / 320] = true;
        &mut self.dst.hd[at as usize * kk..(at as usize + 1) * kk]
    }

    /// Texel (`tx`, `ty`) of a sprite block, honouring flips.
    fn sprite_texel(&self, key: u64, col: u16, row: u16, tx: usize, ty: usize) -> Option<Texel> {
        let s = &self.sprites[&key];
        let k = self.k;
        let tx = if self.mirror_x { k - 1 - tx } else { tx };
        let ty = if self.mirror_y { k - 1 - ty } else { ty };
        let (x, y) = (col as usize * k + tx, row as usize * k + ty);
        if x >= s.w {
            return None;
        }
        s.texels.get(y * s.w + x).copied().flatten()
    }

    /// An enlarged pixel: part (`ix`, `iy`) of `nx` × `ny` copies of the
    /// pixel at `seg:from` takes the matching part of that pixel's HD block.
    fn zoom_block(&mut self, at: u16, seg: u16, from: u16, value: u8, (ix, iy, nx, ny): (u8, u8, u8, u8)) {
        let k = self.k;
        let from_u = from as usize;
        let src = if seg == self.target { Some(&*self.dst) } else { self.others.get(&seg) };
        let (block, lowres) = match src {
            Some(s) if s.low[from_u] == value => {
                let sb = &s.hd[from_u * k * k..(from_u + 1) * k * k];
                let block: Vec<Texel> = (0..k * k)
                    .map(|i| {
                        let (tx, ty) = (i % k, i / k);
                        let sx = (ix as usize * k + tx) / nx as usize;
                        let sy = (iy as usize * k + ty) / ny as usize;
                        sb[sy.min(k - 1) * k + sx.min(k - 1)]
                    })
                    .collect();
                (block, s.lowres[from_u])
            }
            _ => (vec![Texel::solid(value); k * k], true),
        };
        let own = block.iter().filter(|t| t.dominant() == value).count();
        let block = if own * 2 < k * k { vec![Texel::solid(value); k * k] } else { block };
        self.block(at).copy_from_slice(&block);
        self.dst.lowres[at as usize] = lowres;
        self.dst.low[at as usize] = value;
    }

    /// Copy the HD block (and provenance) of `seg:from` to `at`, if that
    /// block depicts `value`; otherwise the pixel becomes low-resolution.
    fn copy_block(&mut self, at: u16, seg: u16, from: u16, value: u8) {
        let kk = self.k * self.k;
        let (at_u, from_u) = (at as usize, from as usize);
        let src = if seg == self.target { None } else { self.others.get(&seg) };
        match src {
            Some(s) if s.low[from_u] == value => {
                let lowres = s.lowres[from_u];
                let block: Vec<Texel> = s.hd[from_u * kk..(from_u + 1) * kk].to_vec();
                self.block(at).copy_from_slice(&block);
                self.dst.lowres[at_u] = lowres;
            }
            None if seg == self.target && self.dst.low[from_u] == value => {
                let lowres = self.dst.lowres[from_u];
                if from != at {
                    self.dst.hd.copy_within(from_u * kk..(from_u + 1) * kk, at_u * kk);
                }
                self.dst.dirty[at_u / 320] = true;
                self.dst.lowres[at_u] = lowres;
            }
            _ => {
                self.block(at).fill(Texel::solid(value));
                self.dst.lowres[at_u] = true;
            }
        }
        self.dst.low[at_u] = value;
    }
}

impl Sink for HdSink<'_> {
    fn put(&mut self, at: u16, value: u8, px: Px) {
        let k = self.k;
        // Copies take their provenance from the source (read it before
        // touching the destination: they can be the same buffer).
        if let Px::Copy { seg, at: from } = px {
            return self.copy_block(at, seg, from, value);
        }
        if let Px::Zoom { seg, at: from, ix, iy, nx, ny } = px {
            return self.zoom_block(at, seg, from, value, (ix, iy, nx, ny));
        }
        if let (Px::Sprite { col, row }, Source::Buffer { seg, start, stride }) = (px, self.source) {
            let from = start.wrapping_add(row.wrapping_mul(*stride)).wrapping_add(col);
            return self.copy_block(at, *seg, from, value);
        }
        self.dst.low[at as usize] = value;
        self.dst.lowres[at as usize] = false;
        match (px, self.source) {
            (Px::Sprite { col, row }, Source::Sprite(key)) => {
                let key = *key;
                let mut texels: Vec<Option<Texel>> = (0..k * k).map(|i| self.sprite_texel(key, col, row, i % k, i / k)).collect();
                // Keep the block mostly this pixel's colour (HD art must
                // depict the low-resolution picture).
                let own = texels.iter().filter(|t| t.map(|t| t.dominant()) == Some(value)).count();
                if own * 2 < k * k && !self.sprites[&key].from_art {
                    texels.iter_mut().for_each(|t| *t = Some(Texel::solid(value)));
                }
                let block = self.block(at);
                for (b, t) in block.iter_mut().zip(texels) {
                    if let Some(t) = t {
                        *b = t;
                    }
                }
            }
            (Px::Glyph { col, row, on }, Source::Glyph(key)) => {
                let g = &self.glyphs[key];
                let (gw, gh) = (g.w, g.h);
                let cover: Vec<u8> =
                    (0..k * k).map(|i| g.cover[(row as usize * k + i / k).min(gh - 1) * gw + (col as usize * k + i % k).min(gw - 1)]).collect();
                let cover = balance(cover, on);
                let (fg, bg) = (self.fg, self.bg);
                let block = self.block(at);
                for (b, c) in block.iter_mut().zip(cover) {
                    let under = bg.unwrap_or(b.dominant());
                    *b = Texel { a: fg, b: under, t: 255 - c };
                }
            }
            _ => self.block(at).fill(Texel::solid(value)),
        }
    }

    fn skip(&mut self, at: u16, px: Px) {
        let k = self.k;
        match (px, self.source) {
            // HD sprite art reaching into a transparent neighbour: draw it
            // where it stays a minority of the block.
            (Px::Sprite { col, row }, Source::Sprite(key)) => {
                let key = *key;
                let texels: Vec<Option<Texel>> = (0..k * k).map(|i| self.sprite_texel(key, col, row, i % k, i / k)).collect();
                if texels.iter().filter(|t| t.is_some()).count() * 2 < k * k {
                    let block = self.block(at);
                    for (b, t) in block.iter_mut().zip(texels) {
                        if let Some(t) = t {
                            *b = t;
                        }
                    }
                }
            }
            (Px::Glyph { col, row, .. }, Source::Glyph(key)) => {
                let g = &self.glyphs[key];
                let (gw, gh) = (g.w, g.h);
                let cover: Vec<u8> =
                    (0..k * k).map(|i| g.cover[(row as usize * k + i / k).min(gh - 1) * gw + (col as usize * k + i % k).min(gw - 1)]).collect();
                let cover = balance(cover, false);
                let fg = self.fg;
                let block = self.block(at);
                for (b, c) in block.iter_mut().zip(cover) {
                    if c > 0 {
                        *b = Texel { a: fg, b: b.dominant(), t: 255 - c };
                    }
                }
            }
            _ => {}
        }
    }
}

/// Keep a glyph block mostly showing what the low-resolution pixel shows
/// (`on`: the foreground): if the smoothed mask disagrees for most texels,
/// clamp it to the pixel's side.
fn balance(mut cover: Vec<u8>, on: bool) -> Vec<u8> {
    let agree = cover.iter().filter(|&&c| (c >= 128) == on).count();
    if agree * 2 < cover.len() {
        for c in cover.iter_mut() {
            *c = if on { (*c).max(128) } else { (*c).min(127) };
        }
    }
    cover
}

/// A 1-bit glyph magnified `k`×: the bitmap is interpolated between pixel
/// centres and thresholded at one half, 4×4 samples per texel. Strokes keep
/// their width; corners and diagonals come out rounded and anti-aliased.
fn smooth_glyph(bits: &[u8], w: usize, h: usize, k: usize) -> HdGlyph {
    let on = |x: isize, y: isize| -> f32 {
        if x < 0 || y < 0 || x >= w as isize || y >= h as isize {
            return 0.0;
        }
        ((bits[y as usize] << x) & 0x80 != 0) as u8 as f32
    };
    let sample = |u: f32, v: f32| -> bool {
        // Pixel centres sit at i + 0.5.
        let (fx, fy) = (u - 0.5, v - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as isize, y0 as isize);
        let top = on(x0, y0) * (1.0 - tx) + on(x0 + 1, y0) * tx;
        let bottom = on(x0, y0 + 1) * (1.0 - tx) + on(x0 + 1, y0 + 1) * tx;
        top * (1.0 - ty) + bottom * ty >= 0.5
    };
    let (gw, gh) = (w * k, h * k);
    let mut cover = vec![0u8; gw * gh];
    const S: usize = 4;
    for ty in 0..gh {
        for tx in 0..gw {
            let mut n = 0;
            for sy in 0..S {
                for sx in 0..S {
                    let u = (tx as f32 + (sx as f32 + 0.5) / S as f32) / k as f32;
                    let v = (ty as f32 + (sy as f32 + 0.5) / S as f32) / k as f32;
                    n += sample(u, v) as usize;
                }
            }
            cover[ty * gw + tx] = (n * 255 / (S * S)) as u8;
        }
    }
    HdGlyph { w: gw, h: gh, cover }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::Regs;

    fn dac() -> Dac {
        let mut d = [[0u8; 3]; 256];
        for (i, c) in d.iter_mut().enumerate() {
            *c = [i as u8 / 4; 3];
        }
        d
    }

    /// Memory with a sprite at 1000:0000 and a glyph at 1000:0100.
    fn mem() -> Vec<u8> {
        let mut m = vec![0u8; 0x110000];
        // 8×8 raw 4-bit staircase: pixel (x, y) = 5 when x <= y.
        for y in 0..8 {
            for xb in 0..4 {
                let lo = if xb * 2 <= y { 5 } else { 0 };
                let hi = if xb * 2 + 1 <= y { 5 } else { 0 };
                m[0x10000 + y * 4 + xb] = lo | hi << 4;
            }
        }
        // Glyph: a 1-pixel diagonal.
        for y in 0..5 {
            m[0x10100 + y] = 0x80 >> y;
        }
        m
    }

    #[test]
    fn sprites_are_magnified_by_mmpx_and_depict_the_low_res_pixels() {
        let m = mem();
        let mut c = Compositor::new(4);
        let r = Regs { es: 0x2000, ..Default::default() };
        let sprite = SpriteRef { wflags: 8, height: 8, format: Format::Nibble { pal: 0x10 }, seg: 0x1000, off: 0 };
        c.apply(&DrawOp::Blit { sprite, x: 10, y: 10 }, &r, Mem(&m), &dac());
        let s = c.shadow(0x2000).unwrap();
        // A pixel on the diagonal: its block is mostly 0x15 but MMPX cuts the corner.
        let at = 15 * 320 + 15;
        assert_eq!(s.low[at], 0x15);
        let block = &s.hd[at * 16..at * 16 + 16];
        let own = block.iter().filter(|t| t.dominant() == 0x15).count();
        assert!(own >= 8 && own < 16, "diagonal block not rounded: {block:?}");
    }

    #[test]
    fn glyphs_are_smoothed() {
        let m = mem();
        let mut c = Compositor::new(4);
        let r = Regs { es: 0x2000, ..Default::default() };
        c.apply(&DrawOp::Glyph { x: 0, y: 0, w: 5, h: 5, fg: 0xfa, bg: Some(0xf1), seg: 0x1000, off: 0x100 }, &r, Mem(&m), &dac());
        let s = c.shadow(0x2000).unwrap();
        let at = 320 + 1;
        let block = &s.hd[at * 16..at * 16 + 16];
        assert!(block.iter().any(|t| t.t > 0 && t.t < 255) || block.iter().any(|t| t.dominant() != 0xfa), "glyph block not smoothed: {block:?}");
    }
}
