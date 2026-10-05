//! From a sprite to HD texels: upscaling (built-in or an external model) and
//! projection back onto the game's palette.
//!
//! HD texels must stay palette art: the game fades and recolours its
//! palette at run time, so every texel is one of the sprite's own palette
//! values, or a mix of two of them (`a`, `b`, weight `t` of `b`). Each
//! upscaled pixel is matched, in OKLab, to the colours present in its source
//! pixel's 3×3 neighbourhood; the silhouette comes from MMPX on the sprite's
//! mask, so it follows the low-resolution one exactly.

use std::{collections::HashMap, path::Path, process::Command};

use gfx::mmpx::mmpx2x;

use crate::{catalog::Asset, palette::oklab};

pub struct Rgb {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 3]>,
}

/// HD texels, RGBA8: R = a, G = b (palette-local values), B = weight of b,
/// A = coverage (0 or 255).
pub struct Texels {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 4]>,
    /// Share of opaque source pixels whose HD block mostly shows them.
    pub quality: f32,
    /// How far the HD art's local colours are from the original's: mean
    /// OKLab distance after a 3×3 average of both (dithering averages out;
    /// changed colours or misplaced features do not).
    pub drift: f32,
}

#[derive(Clone)]
pub enum Backend {
    /// Pixel replication (for testing the pipeline).
    Nearest,
    /// MMPX on palette values (what the game's HD mode does without a pack).
    Mmpx,
    /// An external upscaler run once over a folder of PNGs:
    /// `{in}` and `{out}` are folders, `{scale}` the factor.
    Command { template: String },
}

impl Backend {
    pub fn id(&self) -> String {
        match self {
            Backend::Nearest => "nearest".into(),
            Backend::Mmpx => "mmpx".into(),
            Backend::Command { template } => format!("cmd-{:016x}", gfx::hash::fnv64(template.as_bytes())),
        }
    }
}

/// Extra pixels around a sprite given to the model (edge context), and the
/// smallest side models are fed.
const PAD: usize = 4;
const MIN_SIDE: usize = 16;

/// The sprite in colour, transparent pixels filled from their neighbours
/// (so models see no hard halo), padded by repeating the edges.
pub fn model_input(a: &Asset) -> (Rgb, usize, usize) {
    let (w, h) = (a.stride, a.height);
    let mut px: Vec<Option<[u8; 3]>> = a.local.iter().map(|&v| a.opaque(v).then(|| a.rgb(v))).collect();
    // Grow colours into transparent pixels.
    for _ in 0..w.max(h) {
        let mut changed = false;
        let snapshot = px.clone();
        for y in 0..h {
            for x in 0..w {
                if snapshot[y * w + x].is_some() {
                    continue;
                }
                let mut sum = [0u32; 3];
                let mut n = 0;
                for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx >= 0
                        && ny >= 0
                        && (nx as usize) < w
                        && (ny as usize) < h
                        && let Some(c) = snapshot[ny as usize * w + nx as usize]
                    {
                        (0..3).for_each(|k| sum[k] += c[k] as u32);
                        n += 1;
                    }
                }
                if n > 0 {
                    px[y * w + x] = Some(sum.map(|s| (s / n) as u8));
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let px: Vec<[u8; 3]> = px.into_iter().map(|c| c.unwrap_or([0; 3])).collect();
    let pad_x = PAD.max(MIN_SIDE.saturating_sub(w).div_ceil(2));
    let pad_y = PAD.max(MIN_SIDE.saturating_sub(h).div_ceil(2));
    let (pw, ph) = (w + 2 * pad_x, h + 2 * pad_y);
    let mut out = Vec::with_capacity(pw * ph);
    for y in 0..ph {
        for x in 0..pw {
            let sx = x.saturating_sub(pad_x).min(w - 1);
            let sy = y.saturating_sub(pad_y).min(h - 1);
            out.push(px[sy * w + sx]);
        }
    }
    (Rgb { w: pw, h: ph, px: out }, pad_x, pad_y)
}

/// Cut the sprite back out of an upscaled padded image.
pub fn crop(up: &Rgb, pad_x: usize, pad_y: usize, k: usize, w: usize, h: usize) -> Option<Rgb> {
    let (cw, ch) = (w * k, h * k);
    if up.w < (w + 2 * pad_x) * k || up.h < (h + 2 * pad_y) * k {
        return None;
    }
    let px = (0..ch).flat_map(|y| (0..cw).map(move |x| (x + pad_x * k, y + pad_y * k))).map(|(x, y)| up.px[y * up.w + x]).collect();
    Some(Rgb { w: cw, h: ch, px })
}

pub fn nearest(src: &Rgb, k: usize) -> Rgb {
    let px = (0..src.h * k).flat_map(|y| (0..src.w * k).map(move |x| (x / k, y / k))).map(|(x, y)| src.px[y * src.w + x]).collect();
    Rgb { w: src.w * k, h: src.h * k, px }
}

/// 2×2 box filter (4× level to 2× level).
pub fn halve(src: &Rgb) -> Rgb {
    let (w, h) = (src.w / 2, src.h / 2);
    let px = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| {
            let mut s = [0u32; 3];
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let c = src.px[(y * 2 + dy) * src.w + x * 2 + dx];
                (0..3).for_each(|i| s[i] += c[i] as u32);
            }
            s.map(|v| ((v + 2) / 4) as u8)
        })
        .collect();
    Rgb { w, h, px }
}

/// The sprite's silhouette at `k`× (MMPX on the opaque mask).
pub fn mask(a: &Asset, k: usize) -> Vec<bool> {
    let mut m: Vec<u16> = a.local.iter().map(|&v| a.opaque(v) as u16).collect();
    let (mut w, mut h) = (a.stride, a.height);
    let luma = |v: u16| v as f32;
    let mut s = 1;
    while s < k {
        m = mmpx2x(&m, w, h, &luma);
        w *= 2;
        h *= 2;
        s *= 2;
    }
    m.into_iter().map(|v| v != 0).collect()
}

/// MMPX on the palette values themselves (no colours needed).
pub fn mmpx_texels(a: &Asset, k: usize) -> Texels {
    let mut px: Vec<u16> = a.local.iter().map(|&v| if a.opaque(v) { v as u16 } else { 0x100 }).collect();
    let (mut w, mut h) = (a.stride, a.height);
    let luma = |v: u16| if v > 255 { -1.0 } else { let c = a.rgb(v as u8); 0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32 };
    let mut s = 1;
    while s < k {
        px = mmpx2x(&px, w, h, &luma);
        w *= 2;
        h *= 2;
        s *= 2;
    }
    let px: Vec<[u8; 4]> = px.into_iter().map(|v| if v > 255 { [0; 4] } else { [v as u8, v as u8, 0, 255] }).collect();
    let quality = quality(a, k, &px);
    let drift = drift(a, k, &px);
    Texels { w, h, px, quality, drift }
}

/// Match an upscaled image to the sprite's palette values.
pub fn project(a: &Asset, up: &Rgb, k: usize) -> Texels {
    let (w, h) = (a.stride * k, a.height * k);
    assert_eq!((up.w, up.h), (w, h));
    let silhouette = mask(a, k);
    let lab: HashMap<u8, [f32; 3]> = a.local.iter().filter(|&&v| a.opaque(v)).map(|&v| (v, oklab(a.rgb(v)))).collect();
    let mix_lab = |p: u8, q: u8, t: f32| -> [f32; 3] {
        let (cp, cq) = (a.rgb(p), a.rgb(q));
        oklab([0, 1, 2].map(|i| (cp[i] as f32 * (1.0 - t) + cq[i] as f32 * t).round() as u8))
    };
    let dist = |x: [f32; 3], y: [f32; 3]| (0..3).map(|i| (x[i] - y[i]) * (x[i] - y[i])).sum::<f32>();
    let mut px = vec![[0u8; 4]; w * h];
    for sy in 0..a.height {
        for sx in 0..a.stride {
            // Candidate values: the opaque pixels around this one.
            let mut cand: Vec<u8> = Vec::new();
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let (nx, ny) = (sx as i32 + dx, sy as i32 + dy);
                    if nx < 0 || ny < 0 || nx as usize >= a.stride || ny as usize >= a.height {
                        continue;
                    }
                    let v = a.local[ny as usize * a.stride + nx as usize];
                    if a.opaque(v) && !cand.contains(&v) {
                        cand.push(v);
                    }
                }
            }
            if cand.is_empty() {
                continue;
            }
            const STEPS: usize = 8;
            let mut mixes: Vec<(u8, u8, u8, [f32; 3])> = Vec::new();
            for i in 0..cand.len() {
                for j in i + 1..cand.len() {
                    for s in 1..STEPS {
                        let t = s as f32 / STEPS as f32;
                        mixes.push((cand[i], cand[j], (t * 255.0).round() as u8, mix_lab(cand[i], cand[j], t)));
                    }
                }
            }
            for ty in 0..k {
                for tx in 0..k {
                    let (x, y) = (sx * k + tx, sy * k + ty);
                    if !silhouette[y * w + x] {
                        continue;
                    }
                    let target = oklab(up.px[y * w + x]);
                    // The source pixel's own value wins ties (palettes
                    // repeat colours under different indices).
                    let own = a.local[sy * a.stride + sx];
                    let (mut best, mut best_d) = if a.opaque(own) { ([own, own, 0], dist(lab[&own], target)) } else { ([cand[0], cand[0], 0], f32::MAX) };
                    for &v in &cand {
                        let d = dist(lab[&v], target);
                        if d < best_d - 1e-7 {
                            best_d = d;
                            best = [v, v, 0];
                        }
                    }
                    // A blend only where it is clearly closer (edges, gradients).
                    for &(p, q, t, m) in &mixes {
                        let d = dist(m, target);
                        if d < best_d * 0.6 {
                            best_d = d;
                            best = [p, q, t];
                        }
                    }
                    px[y * w + x] = [best[0], best[1], best[2], 255];
                }
            }
        }
    }
    let quality = quality(a, k, &px);
    let drift = drift(a, k, &px);
    Texels { w, h, px, quality, drift }
}

/// See [`Texels::drift`].
fn drift(a: &Asset, k: usize, px: &[[u8; 4]]) -> f32 {
    let (sw, sh) = (a.stride, a.height);
    let w = sw * k;
    // Per source pixel: its colour, and the mean colour of its HD block.
    let mut src = vec![None; sw * sh];
    let mut hd = vec![None; sw * sh];
    for sy in 0..sh {
        for sx in 0..sw {
            let v = a.local[sy * sw + sx];
            if a.opaque(v) {
                src[sy * sw + sx] = Some(a.rgb(v).map(|c| c as f32));
            }
            let mut sum = [0f32; 3];
            let mut n = 0;
            for i in 0..k * k {
                let t = px[(sy * k + i / k) * w + sx * k + i % k];
                if t[3] != 0 {
                    let (p, q, f) = (a.rgb(t[0]), a.rgb(t[1]), t[2] as f32 / 255.0);
                    (0..3).for_each(|c| sum[c] += p[c] as f32 * (1.0 - f) + q[c] as f32 * f);
                    n += 1;
                }
            }
            if n * 2 >= k * k {
                hd[sy * sw + sx] = Some(sum.map(|c| c / n as f32));
            }
        }
    }
    let blur = |img: &[Option<[f32; 3]>], x: usize, y: usize| -> Option<[u8; 3]> {
        let mut sum = [0f32; 3];
        let mut n = 0;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx >= 0
                    && ny >= 0
                    && (nx as usize) < sw
                    && (ny as usize) < sh
                    && let Some(c) = img[ny as usize * sw + nx as usize]
                {
                    (0..3).for_each(|i| sum[i] += c[i]);
                    n += 1;
                }
            }
        }
        (n > 0).then(|| sum.map(|c| (c / n as f32).round() as u8))
    };
    let (mut total, mut n) = (0f32, 0usize);
    for y in 0..sh {
        for x in 0..sw {
            if src[y * sw + x].is_none() {
                continue;
            }
            if let (Some(p), Some(q)) = (blur(&src, x, y), blur(&hd, x, y)) {
                let (p, q) = (oklab(p), oklab(q));
                total += (0..3).map(|i| (p[i] - q[i]) * (p[i] - q[i])).sum::<f32>().sqrt();
                n += 1;
            }
        }
    }
    if n == 0 { 0.0 } else { total / n as f32 }
}

/// Share of opaque source pixels whose k×k block mostly shows their value.
fn quality(a: &Asset, k: usize, px: &[[u8; 4]]) -> f32 {
    let w = a.stride * k;
    let (mut ok, mut n) = (0usize, 0usize);
    for sy in 0..a.height {
        for sx in 0..a.stride {
            let v = a.local[sy * a.stride + sx];
            if !a.opaque(v) {
                continue;
            }
            n += 1;
            let agree = (0..k * k)
                .filter(|i| {
                    let t = px[(sy * k + i / k) * w + sx * k + i % k];
                    t[3] != 0 && (if t[2] < 128 { t[0] } else { t[1] }) == v
                })
                .count();
            ok += (agree * 2 >= k * k) as usize;
        }
    }
    if n == 0 { 1.0 } else { ok as f32 / n as f32 }
}

pub fn write_png(path: &Path, w: usize, h: usize, rgba: bool, data: &[u8]) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(if rgba { png::ColorType::Rgba } else { png::ColorType::Rgb });
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::High);
    enc.write_header().map_err(std::io::Error::other)?.write_image_data(data).map_err(std::io::Error::other)
}

pub fn read_rgb(path: &Path) -> std::io::Result<Rgb> {
    let dec = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path)?));
    let mut reader = dec.read_info().map_err(std::io::Error::other)?;
    let mut buf = vec![0; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buf).map_err(std::io::Error::other)?;
    let ch = match info.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Indexed => return Err(std::io::Error::other("indexed PNG output is not supported")),
    };
    if info.bit_depth != png::BitDepth::Eight {
        return Err(std::io::Error::other("only 8-bit PNG output is supported"));
    }
    let (w, h) = (info.width as usize, info.height as usize);
    let px = (0..w * h)
        .map(|i| {
            let p = &buf[i * ch..i * ch + ch];
            if ch < 3 { [p[0]; 3] } else { [p[0], p[1], p[2]] }
        })
        .collect();
    Ok(Rgb { w, h, px })
}

/// Run an external upscaler over `inputs` (written to `dir/in`, read back
/// from `dir/out`). Returns the images that came back.
pub fn run_command(template: &str, inputs: &[(u64, Rgb)], k: usize, dir: &Path) -> std::io::Result<HashMap<u64, Rgb>> {
    let (indir, outdir) = (dir.join("in"), dir.join("out"));
    for d in [&indir, &outdir] {
        let _ = std::fs::remove_dir_all(d);
        std::fs::create_dir_all(d)?;
    }
    for (hash, img) in inputs {
        let data: Vec<u8> = img.px.iter().flatten().copied().collect();
        write_png(&indir.join(format!("{hash:016x}.png")), img.w, img.h, false, &data)?;
    }
    let cmd = template
        .replace("{in}", &indir.display().to_string())
        .replace("{out}", &outdir.display().to_string())
        .replace("{scale}", &k.to_string());
    eprintln!("running: {cmd}");
    let status = Command::new("sh").arg("-c").arg(&cmd).status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!("upscaler failed: {status}")));
    }
    let mut out = HashMap::new();
    for (hash, _) in inputs {
        let p = outdir.join(format!("{hash:016x}.png"));
        if let Ok(img) = read_rgb(&p) {
            out.insert(*hash, img);
        }
    }
    Ok(out)
}
