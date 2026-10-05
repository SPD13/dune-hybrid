//! Every distinct sprite of DUNE.DAT, decoded, with the colours to show it in.

use std::{collections::HashMap, fs::File, io::BufRead, path::Path};

use gfx::{dat, ops::Format, sheet};

use crate::palette::{self, Palette};

pub struct Asset {
    /// `gfx::hash::sprite_hash`: what the game's drawing calls are matched on.
    pub hash: u64,
    pub res: String,
    pub part: usize,
    pub format: Format,
    /// Visible width, and the decoded row length (4-bit rows are padded).
    pub width: usize,
    pub stride: usize,
    pub height: usize,
    /// Palette-local values (`stride` × `height`): nibbles for 4-bit sprites.
    pub local: Vec<u8>,
    /// Colours of the 256 palette indices while this sprite is shown.
    pub palette: Palette,
}

impl Asset {
    pub fn opaque(&self, v: u8) -> bool {
        match self.format {
            Format::Byte => true,
            _ => v != 0,
        }
    }

    /// The palette index a local value is drawn with (the sheet's own
    /// palette offset; the game may override it per draw).
    pub fn global(&self, v: u8) -> u8 {
        match self.format {
            Format::Nibble { pal } => v.wrapping_add(pal),
            _ => v,
        }
    }

    pub fn rgb(&self, v: u8) -> [u8; 3] {
        self.palette[self.global(v) as usize]
    }

    pub fn format_name(&self) -> &'static str {
        match self.format {
            Format::Nibble { .. } => "nibble",
            Format::Byte => "byte",
            Format::ByteKey => "bytekey",
        }
    }
}

/// Palettes observed while the game ran (from `dune-run --gfx-trace`): the
/// one most often in use when each sprite was drawn, and the most common.
#[derive(Default)]
pub struct Observed {
    pub by_sprite: HashMap<u64, Palette>,
    pub common: Option<Palette>,
}

impl Observed {
    pub fn load(path: &Path) -> std::io::Result<Observed> {
        let mut dacs: HashMap<u64, Palette> = HashMap::new();
        let mut counts: HashMap<u64, HashMap<u64, u32>> = HashMap::new();
        let mut totals: HashMap<u64, u32> = HashMap::new();
        let mut current = 0u64;
        for line in std::io::BufReader::new(File::open(path)?).lines() {
            let line = line?;
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else { continue };
            if let Some(hex) = v.get("dac").and_then(|d| d.as_str()) {
                let bytes: Vec<u8> = (0..hex.len() / 2).filter_map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()).collect();
                if bytes.len() == 768 {
                    current = gfx::hash::fnv64(&bytes);
                    dacs.entry(current).or_insert_with(|| std::array::from_fn(|i| [0, 1, 2].map(|k| palette::six_to_eight(bytes[i * 3 + k]))));
                }
            } else if let Some(h) = v.get("hash").and_then(|h| h.as_str()).and_then(|h| u64::from_str_radix(h, 16).ok())
                && current != 0
            {
                *counts.entry(h).or_default().entry(current).or_default() += 1;
                *totals.entry(current).or_default() += 1;
            }
        }
        let best = |m: &HashMap<u64, u32>| m.iter().max_by_key(|(k, n)| (**n, **k)).map(|(k, _)| *k);
        Ok(Observed {
            by_sprite: counts.iter().filter_map(|(h, m)| best(m).map(|d| (*h, dacs[&d]))).collect(),
            common: best(&totals).map(|d| dacs[&d]),
        })
    }
}

/// All distinct sprites (by hash) of the sheets in DUNE.DAT.
pub fn extract(dat_path: &Path, observed: &Observed) -> std::io::Result<Vec<Asset>> {
    let mut f = File::open(dat_path)?;
    let toc = dat::toc(&mut f)?;
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for e in &toc {
        let Ok(res) = dat::load(&mut f, e) else { continue };
        let Some(list) = sheet::parse(&res) else { continue };
        for s in list {
            if s.data_len == 0 || !seen.insert(s.hash) {
                continue;
            }
            let Some(img) = s.image(&res) else { continue };
            let base = observed.by_sprite.get(&s.hash).or(observed.common.as_ref()).copied().unwrap_or_else(palette::grey);
            out.push(Asset {
                hash: s.hash,
                res: e.name.clone(),
                part: s.index,
                format: s.format(),
                width: img.width,
                stride: img.stride,
                height: img.height,
                local: img.px,
                palette: palette::apply_chunks(&res, &base),
            });
        }
    }
    Ok(out)
}

/// A hash of DUNE.DAT's table of contents, to recognise the game version.
pub fn toc_hash(dat_path: &Path) -> std::io::Result<u64> {
    let mut f = File::open(dat_path)?;
    let toc = dat::toc(&mut f)?;
    let text: String = toc.iter().map(|e| format!("{}:{}:{};", e.name, e.size, e.offset)).collect();
    Ok(gfx::hash::fnv64(text.as_bytes()))
}
