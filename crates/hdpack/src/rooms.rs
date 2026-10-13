//! Rooms captured by `dune-run --rooms DIR`, as assets: each room is
//! upscaled in one piece (so the art continues across the joins between its
//! sprites, floors and lines) and becomes a room plate in the pack.

use std::path::Path;

use gfx::{ops::Format, room};

use crate::{catalog::Asset, palette};

/// The rooms in `dir` (`<key>.json`, `.idx`, `.cov`; rooms seen during the
/// harness's own run, `live-*`, are left out).
pub fn load(dir: &Path) -> std::io::Result<Vec<Asset>> {
    let mut out = Vec::new();
    let mut names: Vec<_> = std::fs::read_dir(dir)?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for name in names {
        let Some(stem) = name.strip_suffix(".json") else { continue };
        if stem.starts_with("live-") || stem == "index" {
            continue;
        }
        let json: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join(&name))?)?;
        let key = u64::from_str_radix(stem, 16).map_err(|e| std::io::Error::other(format!("{name}: {e}")))?;
        let local = std::fs::read(dir.join(format!("{stem}.idx")))?;
        let cover: Vec<bool> = std::fs::read(dir.join(format!("{stem}.cov")))?.into_iter().map(|c| c != 0).collect();
        if local.len() != room::AREA || cover.len() != room::AREA {
            return Err(std::io::Error::other(format!("{stem}: expected {} pixels", room::AREA)));
        }
        let dac = json["dac"].as_str().unwrap_or_default();
        let bytes: Vec<u8> = (0..dac.len() / 2).filter_map(|i| u8::from_str_radix(&dac[i * 2..i * 2 + 2], 16).ok()).collect();
        if bytes.len() != 768 {
            return Err(std::io::Error::other(format!("{stem}: no palette")));
        }
        let sal = json["sal"].as_str().unwrap_or("?").trim_end_matches(".SAL").to_string();
        out.push(Asset {
            hash: key,
            content: key,
            res: format!("ROOM-{sal}"),
            part: json["room"].as_u64().unwrap_or(0) as usize,
            format: Format::Byte,
            width: 320,
            stride: 320,
            height: room::ROWS,
            local,
            palette: std::array::from_fn(|i| [0, 1, 2].map(|k| palette::six_to_eight(bytes[i * 3 + k]))),
            cover: Some(cover),
            room: true,
        });
    }
    Ok(out)
}
