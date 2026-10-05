//! Colours for sprites, which only store palette indices.
//!
//! Sheets carry palette chunks between byte 2 and their sprite table:
//! `start`, `count` (0 = 256), then `count` × 6-bit RGB, until `FF FF`.
//! The game's palette accumulates as resources load, so a sheet's chunks are
//! laid over a base palette: the one observed most often while the game drew
//! the sprite (from a `dune-run --gfx-trace` file), else the most common
//! observed palette, else a neutral ramp.

pub type Palette = [[u8; 3]; 256];

/// Neutral base: a grey ramp (8-bit).
pub fn grey() -> Palette {
    std::array::from_fn(|i| [i as u8; 3])
}

/// Apply a sheet's palette chunks over `base` (8-bit output).
pub fn apply_chunks(res: &[u8], base: &Palette) -> Palette {
    let mut pal = *base;
    let table = u16::from_le_bytes([res[0], res[1]]) as usize;
    let mut i = 2;
    while i + 2 <= table.min(res.len()) {
        let (start, count) = (res[i] as usize, res[i + 1] as usize);
        if start == 0xff && count == 0xff {
            break;
        }
        let count = if count == 0 { 256 } else { count };
        i += 2;
        for c in 0..count {
            let (idx, at) = (start + c, i + c * 3);
            if idx > 255 || at + 3 > res.len() {
                break;
            }
            pal[idx] = [0, 1, 2].map(|k| six_to_eight(res[at + k]));
        }
        i += count * 3;
    }
    pal
}

pub fn six_to_eight(v: u8) -> u8 {
    let v = v & 0x3f;
    (v << 2) | (v >> 4)
}

/// sRGB (8-bit) to OKLab.
pub fn oklab(c: [u8; 3]) -> [f32; 3] {
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = (lin(c[0]), lin(c[1]), lin(c[2]));
    let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}
