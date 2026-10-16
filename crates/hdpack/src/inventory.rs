//! `dune-hd inventory`: all of DUNE.DAT's art as review units for the studio
//! (`tools/studio`): talking-head frames, rooms, scenes and sprites, each
//! drawn in its true colours, sorted into classes by the recipe bundle's
//! `assets.json`.
//!
//! Writes `WORK/review/items.json` (what the studio lists) and the originals
//! to `WORK/inventory/src/`. Talking heads are listed by the frames the game
//! lists (`head/NAME/fNN`) and by every group drawn over frame 0
//! (`head/NAME/vGG`: the mouths and eyes swapped in while talking). Sprites
//! not coloured by their own sheet keep their palette in
//! `WORK/inventory/palettes/<art_name>.dac` (768 bytes, 8-bit RGB), for the
//! pack export. A sprite's colours come, in order, from: the
//! palette observed while the game drew it (`--trace`); its sheet's own
//! palette chunks, when they cover every value it uses; the palette its
//! sheet's rule names (`game`: a captured room's palette; `SKYDN:N` /
//! `SKY:N`: a time-of-day palette). Anything else is marked as not known.

use std::{
    collections::{BTreeMap, HashSet},
    path::Path,
};

use serde_json::{Value, json};

use crate::{
    art,
    catalog::{self, Asset, Observed},
    heads,
    palette::{self, Palette},
    rooms,
};

struct Rule {
    sheets: Vec<String>,
    class: String,
    palette: Option<String>,
    note: Option<String>,
}

struct Recipes {
    game_room: Option<String>,
    classes: BTreeMap<String, String>,
    rules: Vec<Rule>,
    /// (min width, min height, class)
    sizes: Vec<(usize, usize, String)>,
}

impl Recipes {
    fn load(dir: Option<&Path>) -> std::io::Result<Recipes> {
        let v: Value = match dir.map(|d| d.join("assets.json")).filter(|p| p.exists()) {
            Some(p) => serde_json::from_slice(&std::fs::read(p)?)?,
            None => json!({}),
        };
        let strs = |v: &Value| v.as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default();
        Ok(Recipes {
            game_room: v["game_palette_room"].as_str().map(String::from),
            classes: v["classes"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or(k).to_string())).collect()).unwrap_or_default(),
            rules: v["rules"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|r| Rule {
                            sheets: strs(&r["sheets"]),
                            class: r["class"].as_str().unwrap_or("other").to_string(),
                            palette: r["palette"].as_str().map(String::from),
                            note: r["note"].as_str().map(String::from),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            sizes: v["size_rules"]
                .as_array()
                .map(|a| a.iter().map(|r| (r["min_width"].as_u64().unwrap_or(0) as usize, r["min_height"].as_u64().unwrap_or(0) as usize, r["class"].as_str().unwrap_or("other").to_string())).collect())
                .unwrap_or_default(),
        })
    }

    fn class_name(&self, key: &str) -> String {
        self.classes.get(key).cloned().unwrap_or_else(|| key.to_string())
    }

    fn rule(&self, sheet: &str) -> Option<&Rule> {
        let matches = |p: &str| match p.strip_suffix('*') {
            Some(prefix) => sheet.starts_with(prefix),
            None => sheet == p,
        };
        self.rules.iter().find(|r| r.sheets.iter().any(|p| matches(p)))
    }
}

/// Time-of-day palettes of SKY.HSQ / SKYDN.HSQ: entries 8-40 are blocks of
/// (u16 length, first index, count, 6-bit RGB); as (first, colours).
fn sky_palettes(res: &[u8]) -> Vec<(usize, Vec<[u8; 3]>)> {
    let u16_at = |o: usize| res.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let mut offs: Vec<usize> = (0..41).map_while(|i| u16_at(2 + 2 * i).map(|o| o + 2)).collect();
    offs.push(res.len());
    let mut out = Vec::new();
    for i in 8..offs.len().saturating_sub(1) {
        let Some(e) = res.get(offs[i]..offs[i + 1]).filter(|e| e.len() >= 6) else { break };
        let (start, count) = (e[4] as usize, e[5] as usize);
        let colours = e[6..].chunks_exact(3).take(count).map(|c| [0, 1, 2].map(|k| palette::six_to_eight(c[k]))).collect();
        out.push((start, colours));
    }
    out
}

/// Which palette indices a sheet's own chunks set.
fn chunk_cover(res: &[u8]) -> [bool; 256] {
    // No 6-bit colour expands to (1, 2, 3): what stays so was not set.
    let mark = [[1u8, 2, 3]; 256];
    let p = palette::apply_chunks(res, &mark);
    std::array::from_fn(|i| p[i] != [1, 2, 3])
}

fn used(a: &Asset) -> HashSet<u8> {
    (0..a.local.len()).filter(|&i| a.solid(i)).map(|i| a.global(a.local[i])).collect()
}

struct Colours<'a> {
    observed: &'a Observed,
    game: Option<Palette>,
    sky: BTreeMap<String, Vec<(usize, Vec<[u8; 3]>)>>,
}

impl Colours<'_> {
    /// The palette to show `a` (from sheet `res`) in: (palette, source, known for sure).
    fn resolve(&self, a: &Asset, res: &[u8], rule: Option<&Rule>) -> (Palette, String, bool) {
        if self.observed.by_sprite.contains_key(&a.content) {
            return (a.palette, "trace".into(), true);
        }
        let cover = chunk_cover(res);
        let used = used(a);
        if !used.is_empty() && used.iter().all(|&v| cover[v as usize]) {
            return (palette::apply_chunks(res, &palette::grey()), "sheet".into(), true);
        }
        match rule.and_then(|r| r.palette.as_deref()) {
            Some("game") if self.game.is_some() => return (palette::apply_chunks(res, self.game.as_ref().unwrap()), "game".into(), true),
            Some(p) if p.starts_with("SKY") => {
                // One or more time-of-day blocks, laid in order ("SKYDN:0+SKY:0").
                let mut base = [[0u8; 3]; 256];
                let mut set = [false; 256];
                for spec in p.split('+') {
                    let (name, n) = spec.split_once(':').unwrap_or((spec, "0"));
                    let Some((start, cols)) = self.sky.get(name).and_then(|v| v.get(n.parse::<usize>().unwrap_or(0))) else { continue };
                    for (i, c) in cols.iter().enumerate().filter(|(i, _)| start + i < 256) {
                        base[start + i] = *c;
                        set[start + i] = true;
                    }
                }
                let ok = used.iter().all(|&v| cover[v as usize] || set[v as usize]);
                return (palette::apply_chunks(res, &base), p.to_string(), ok);
            }
            _ => {}
        }
        match self.observed.common {
            Some(common) => (palette::apply_chunks(res, &common), "trace (most common palette: a guess)".into(), false),
            None => (palette::apply_chunks(res, &palette::grey()), "unknown".into(), false),
        }
    }
}

fn rgba(w: usize, h: usize, px: impl Fn(usize, usize) -> Option<[u8; 3]>) -> Vec<u8> {
    (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).flat_map(|(x, y)| px(x, y).map(|c| [c[0], c[1], c[2], 255]).unwrap_or([0; 4])).collect()
}

pub fn run(dat: &Path, work: &Path, recipes_dir: Option<&Path>, rooms_dir: Option<&Path>, observed: &Observed) -> std::io::Result<()> {
    let recipes = Recipes::load(recipes_dir)?;
    let src = work.join("inventory").join("src");
    std::fs::create_dir_all(&src)?;
    std::fs::create_dir_all(work.join("review"))?;
    // The palette of each sprite not coloured by its own sheet (768 bytes, 8-bit RGB).
    let palettes = work.join("inventory").join("palettes");
    std::fs::create_dir_all(&palettes)?;
    let rel = |p: &Path| p.strip_prefix(work).unwrap_or(p).to_string_lossy().replace('\\', "/");
    let mut items: Vec<Value> = Vec::new();

    // Rooms first: one of them gives the game's palette.
    let room_list = match rooms_dir {
        Some(d) => rooms::load(d)?,
        None => Vec::new(),
    };
    let game = recipes
        .game_room
        .as_ref()
        .and_then(|k| room_list.iter().find(|r| format!("{:016x}", r.content) == *k))
        .or(room_list.first())
        .map(|r| r.palette);
    for r in &room_list {
        let key = format!("{:016x}", r.content);
        let path = src.join(format!("room-{key}.png"));
        art::write_png(&path, r.stride, r.height, true, &rgba(r.stride, r.height, |x, y| r.solid(y * r.stride + x).then(|| r.rgb(r.local[y * r.stride + x]))))?;
        let sal = r.res.trim_start_matches("ROOM-").to_string();
        items.push(json!({
            "id": format!("room/{key}"), "title": format!("{sal} room {}", r.part), "class": recipes.class_name("rooms"), "class_key": "rooms", "group": sal,
            "kind": "room", "size": [r.stride, r.height], "palette": {"source": "room capture", "ok": true},
            "art_name": key, "original": {"path": rel(&path), "label": "original (pixel art)"}, "versions": [], "default": null,
        }));
    }

    let mut f = std::fs::File::open(dat)?;
    let toc = gfx::dat::toc(&mut f)?;
    let mut sky = BTreeMap::new();
    for e in &toc {
        if e.name == "SKY.HSQ" || e.name == "SKYDN.HSQ" {
            if let Ok(res) = gfx::dat::load(&mut f, e) {
                sky.insert(e.name.trim_end_matches(".HSQ").to_string(), sky_palettes(&res));
            }
        }
    }
    let colours = Colours { observed, game, sky };
    let mut seen = HashSet::new();
    let mut unknown = 0;
    for e in &toc {
        let Ok(res) = gfx::dat::load(&mut f, e) else { continue };
        let Some(sheet) = catalog::sheet_assets(&e.name, &res, observed) else { continue };
        let name = e.name.trim_end_matches(".HSQ").to_string();
        if let Some(head) = heads::parse(&res) {
            // A talking head: reviewed by the frames the game lists.
            let refs: Vec<Option<&Asset>> = sheet.iter().map(|a| a.as_ref()).collect();
            let cover = chunk_cover(&res);
            for (i, fr) in head.frames.iter().enumerate() {
                let parts: Vec<heads::Part> = fr.iter().flat_map(|&g| head.groups[g].iter().cloned()).collect();
                let Some((_, _, w, h, px)) = heads::compose(&parts, &refs) else { continue };
                let pal = sheet.iter().flatten().next().map(|a| a.palette).unwrap_or_else(palette::grey);
                let ok = px.iter().flatten().all(|&v| cover[v as usize]);
                let path = src.join(format!("head-{name}-f{i:02}.png"));
                art::write_png(&path, w, h, true, &rgba(w, h, |x, y| px[y * w + x].map(|v| pal[v as usize])))?;
                let single = fr.len() == 1;
                items.push(json!({
                    "id": format!("head/{name}/f{i:02}"), "title": format!("{name} f{i:02}"), "class": recipes.class_name("heads"), "class_key": "heads", "group": name, "groups": fr,
                    "kind": "head-frame", "size": [w, h], "palette": {"source": "sheet", "ok": ok}, "art_name": format!("{name}-f{i:02}"),
                    "note": format!("groups {:?}{}", fr, if single { " (a lip-sync or single-group frame)" } else { "" }),
                    "original": {"path": rel(&path), "label": "original (pixel art)"}, "versions": [], "default": null,
                }));
            }
            // While a character talks, the game draws its face with groups
            // swapped in (mouths, eyes), as each sentence's lip-sync data
            // says: each group outside frame 0, drawn over it (generated as
            // a rerender of frame 0, see generate.py). Groups that build the
            // faces listed as frames (the Fremen sheets' hair, beards) are
            // reviewed in those frames instead.
            let faces: HashSet<usize> = head.frames.iter().filter(|fr| fr.len() >= 2).flatten().copied().collect();
            if let Some(base) = head.frames.first() {
                let base_parts: Vec<heads::Part> = base.iter().flat_map(|&g| head.groups[g].iter().cloned()).collect();
                let rest = heads::compose(&base_parts, &refs);
                for (g, group) in head.groups.iter().enumerate().filter(|(g, _)| !base.contains(g) && !faces.contains(g)) {
                    let mut parts = base_parts.clone();
                    parts.extend(group.iter().cloned());
                    let Some((x0, y0, w, h, px)) = heads::compose(&parts, &refs) else { continue };
                    if rest.as_ref().is_some_and(|r| (r.0, r.1, r.2, r.3) == (x0, y0, w, h) && r.4 == px) {
                        continue; // draws nothing new over frame 0
                    }
                    let pal = sheet.iter().flatten().next().map(|a| a.palette).unwrap_or_else(palette::grey);
                    let ok = px.iter().flatten().all(|&v| cover[v as usize]);
                    let path = src.join(format!("head-{name}-v{g:02}.png"));
                    art::write_png(&path, w, h, true, &rgba(w, h, |x, y| px[y * w + x].map(|v| pal[v as usize])))?;
                    let mut groups = base.clone();
                    groups.push(g);
                    items.push(json!({
                        "id": format!("head/{name}/v{g:02}"), "title": format!("{name} v{g:02}"), "class": recipes.class_name("heads"), "class_key": "heads", "group": name, "groups": groups,
                        "kind": "head-group", "size": [w, h], "palette": {"source": "sheet", "ok": ok}, "art_name": format!("{name}-v{g:02}"),
                        "note": format!("group {g} over frame 0 (a mouth or eyes the game swaps in while {name} talks)"),
                        "original": {"path": rel(&path), "label": "original (pixel art)"}, "versions": [], "default": null,
                    }));
                }
            }
            for a in sheet.iter().flatten() {
                seen.insert(a.content);
            }
            continue;
        }
        let rule = recipes.rule(&name);
        for a in sheet.iter().flatten() {
            if !seen.insert(a.content) {
                continue;
            }
            let text = gfx::pack::text_art(&a.local, |v| a.opaque(v));
            let class = if text {
                "text".to_string()
            } else if let Some(r) = rule {
                r.class.clone()
            } else if let Some((_, _, c)) = recipes.sizes.iter().find(|(w, h, _)| a.width >= *w && a.height >= *h) {
                c.clone()
            } else {
                "other".to_string()
            };
            let (pal, source, ok) = colours.resolve(a, &res, rule);
            if !ok {
                unknown += 1;
            }
            let art_name = format!("{name}-{:03}", a.part);
            if source != "sheet" {
                // Colours `dune-hd build` cannot find in the sheet: kept for
                // the pack export (the --art palette sidecar).
                std::fs::write(palettes.join(format!("{art_name}.dac")), pal.iter().flatten().copied().collect::<Vec<u8>>())?;
            }
            let path = src.join(format!("{art_name}.png"));
            art::write_png(&path, a.width, a.height, true, &rgba(a.width, a.height, |x, y| {
                let i = y * a.stride + x;
                a.solid(i).then(|| pal[a.global(a.local[i]) as usize])
            }))?;
            let mut item = json!({
                "id": format!("sprite/{art_name}"), "title": art_name, "class": recipes.class_name(&class), "class_key": class, "group": name,
                "kind": "sprite", "size": [a.width, a.height], "palette": {"source": source, "ok": ok},
                "art_name": art_name, "content": format!("{:016x}", a.content), "format": a.format_name(),
                "original": {"path": rel(&path), "label": "original (pixel art)"}, "versions": [], "default": null,
            });
            if let Some(n) = rule.and_then(|r| r.note.as_ref()) {
                item["note"] = json!(n);
            }
            items.push(item);
        }
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for i in &items {
        *counts.entry(i["class"].as_str().unwrap_or("?").to_string()).or_default() += 1;
    }
    std::fs::write(work.join("review").join("items.json"), serde_json::to_string_pretty(&json!({"generator": "dune-hd inventory", "items": items}))?)?;
    eprintln!("{} items in {}:", items.len(), work.join("review/items.json").display());
    for (c, n) in &counts {
        eprintln!("  {n:5}  {c}");
    }
    eprintln!("{unknown} sprites whose colours are not known for sure (marked in the studio)");
    Ok(())
}
