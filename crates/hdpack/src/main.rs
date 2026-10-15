//! dune-hd: generates an HD art pack for Dune Hybrid from your own DUNE.DAT.
//!
//!     dune-hd build --dat path/to/DUNE.DAT --out dune-hd-pack.zip [options]
//!     dune-hd extract --dat path/to/DUNE.DAT [--work work]     # sprite previews
//!     dune-hd heads --dat path/to/DUNE.DAT [--work work]       # talking heads: frames,
//!         parts (vNN = group NN over frame 0) and NAME.json layouts in work/heads
//!     dune-hd inventory --dat path/to/DUNE.DAT --work WORK [--recipes recipes]
//!         [--rooms DIR] [--trace FILE]   # every asset as a review unit for the
//!         studio (tools/studio): WORK/review/items.json, originals in true colours
//!     dune-hd svg IN.svg OUT.png --width N   # renders a vector emblem (recipes) to
//!         a transparent PNG N pixels wide, for the generation tools
//!
//! Options for `build`:
//!   --work DIR             cache and intermediate files (default: work)
//!   --backend NAME         mmpx (default), nearest, or command
//!   --command TEMPLATE     external upscaler run once over a folder of PNGs;
//!                          {in}/{out} are folders, {scale} is 4
//!   --model-name / --model-license / --model-source   recorded in the pack
//!   --trace FILE           palettes observed with `dune-run --gfx-trace`
//!   --rooms DIR            rooms captured by `dune-run --rooms DIR`: each is
//!                          upscaled in one piece and stored as a room plate
//!                          (seamless joins between its sprites); with
//!                          --only, name ROOMS to keep them
//!   --no-face-context      upscale talking-head parts (eyes, mouths) on
//!                          their own, not over the face they belong to
//!   --room-prescale N      --prescale for rooms (default 1: at 2 the models
//!                          keep the pixel steps of a room's outlines, which
//!                          sprites smooth with their own silhouette)
//!   --only RES[,RES]       only these resources (e.g. LETO.HSQ)
//!   --limit N              only the first N sprites
//!   --max-drift D          drop art whose local colours drift further than D
//!                          from the original's (mean OKLab distance after a
//!                          3×3 average; default 0.05). The game then uses its
//!                          built-in upscaling for that sprite.
//!   --min-size N           leave sprites narrower or shorter than N pixels to
//!                          the built-in upscaling (default 6; models invent
//!                          too much on tiny images)
//!   --previews             write original / upscaled / result images per
//!                          sprite to WORK/preview
//!   --prescale N           enlarge sprites N× (pixel-exact) before the
//!                          upscaler and shrink its output back: models then
//!                          see each original pixel as a feature instead of
//!                          noise, which keeps small faces readable
//!                          (default 2 with --backend command)
//!   --art DIR              finished HD art made elsewhere (e.g. generative):
//!                          DIR/<id>.png, 4× the sprite or room (<id> = the
//!                          content id of `extract`'s catalog, a room key, or
//!                          RES-PPP such as ICONES-009). Projected onto the
//!                          palette like an upscaler's output, without the
//!                          guard or the drift limit. DIR/<id>.json may give
//!                          {"dac": 768 hex bytes of 6-bit VGA palette}: the
//!                          palette the art was made in, for sprites the game
//!                          colours with its own palette, and
//!                          {"true_colour": true}: also store the art in true
//!                          colour (pack version 4), shown with its own colours
//!                          and following the palette's fades (sprites only)
//!   --guard D              where the art drifts locally from the original by
//!                          more than D, or flattens a clear feature (an eye),
//!                          use the built-in upscaling for those pixels
//!                          (default 0.04 with --backend command; 0 = off)
//!
//! The pack is made from your copy of the game and is for your own use:
//! do not redistribute it.

mod art;
mod catalog;
mod heads;
mod inventory;
mod palette;
mod rooms;
mod zip;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use rayon::prelude::*;
use serde_json::json;

use art::{Backend, Rgb, Texels};
use catalog::Asset;

const NOTICE: &str = "This HD art pack was generated on your computer from your own copy of Dune (Cryo Interactive, 1992). \
It is a derivative of the game's copyrighted artwork, for your personal use with Dune Hybrid. Do not redistribute it.";

struct Args {
    cmd: String,
    dat: PathBuf,
    work: PathBuf,
    out: PathBuf,
    backend: Backend,
    model: (String, String, String),
    trace: Option<PathBuf>,
    rooms: Option<PathBuf>,
    /// Finished HD art made elsewhere (see the header).
    art: Option<PathBuf>,
    /// The recipe bundle (`inventory` reads its assets.json).
    recipes: Option<PathBuf>,
    only: Vec<String>,
    limit: Option<usize>,
    max_drift: f32,
    min_size: usize,
    prescale: usize,
    room_prescale: usize,
    face_context: bool,
    guard: Option<f32>,
    /// Write side-by-side previews (original, upscaled, projected) per sprite.
    previews: bool,
}

fn parse() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().ok_or("usage: dune-hd build|extract --dat DUNE.DAT [options] (see the source header)")?;
    let mut a = Args {
        cmd,
        dat: PathBuf::new(),
        work: "work".into(),
        out: "dune-hd-pack.zip".into(),
        backend: Backend::Mmpx,
        model: ("mmpx".into(), "MIT".into(), "https://casual-effects.com/research/McGuire2021PixelArt/".into()),
        trace: None,
        rooms: None,
        art: None,
        recipes: None,
        only: Vec::new(),
        limit: None,
        max_drift: 0.05,
        min_size: 6,
        prescale: 0,
        room_prescale: 1,
        face_context: true,
        guard: None,
        previews: false,
    };
    let mut backend = "mmpx".to_string();
    let mut template = None;
    while let Some(k) = it.next() {
        let mut v = || it.next().ok_or(format!("missing value for {k}"));
        match k.as_str() {
            "--dat" => a.dat = v()?.into(),
            "--work" => a.work = v()?.into(),
            "--out" => a.out = v()?.into(),
            "--backend" => backend = v()?,
            "--command" => template = Some(v()?),
            "--model-name" => a.model.0 = v()?,
            "--model-license" => a.model.1 = v()?,
            "--model-source" => a.model.2 = v()?,
            "--trace" => a.trace = Some(v()?.into()),
            "--rooms" => a.rooms = Some(v()?.into()),
            "--art" => a.art = Some(v()?.into()),
            "--recipes" => a.recipes = Some(v()?.into()),
            "--only" => a.only = v()?.split(',').map(|s| s.trim().to_uppercase()).collect(),
            "--limit" => a.limit = Some(v()?.parse().map_err(|e| format!("{e}"))?),
            "--max-drift" => a.max_drift = v()?.parse().map_err(|e| format!("{e}"))?,
            "--min-size" => a.min_size = v()?.parse().map_err(|e| format!("{e}"))?,
            "--previews" => a.previews = true,
            "--prescale" => a.prescale = v()?.parse().map_err(|e| format!("{e}"))?,
            "--no-face-context" => a.face_context = false,
            "--room-prescale" => a.room_prescale = v()?.parse().map_err(|e| format!("{e}"))?,
            "--guard" => a.guard = Some(v()?.parse::<f32>().map_err(|e| format!("{e}"))?).filter(|g| *g > 0.0).or(Some(0.0)),
            _ => return Err(format!("unknown option {k}")),
        }
    }
    if a.dat.as_os_str().is_empty() {
        return Err("--dat DUNE.DAT is required".into());
    }
    a.backend = match backend.as_str() {
        "mmpx" => Backend::Mmpx,
        "nearest" => {
            a.model = ("nearest".into(), "-".into(), "-".into());
            Backend::Nearest
        }
        "command" => {
            // Defaults that keep small details (faces) for AI upscalers.
            if a.prescale == 0 {
                a.prescale = 2;
            }
            a.guard = a.guard.or(Some(0.04));
            Backend::Command { template: template.ok_or("--backend command needs --command TEMPLATE")? }
        }
        other => return Err(format!("unknown backend {other}")),
    };
    Ok(a)
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("svg") {
        if let Err(e) = svg_command() {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return;
    }
    let args = match parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = run(&args) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

/// `dune-hd svg IN.svg OUT.png --width N`: a vector emblem as a transparent PNG.
fn svg_command() -> std::io::Result<()> {
    let a: Vec<String> = std::env::args().skip(2).collect();
    let usage = || std::io::Error::other("usage: dune-hd svg IN.svg OUT.png --width N");
    let (src, out) = (a.first().ok_or_else(usage)?, a.get(1).ok_or_else(usage)?);
    let width: u32 = match a.iter().position(|x| x == "--width") {
        Some(i) => a.get(i + 1).and_then(|w| w.parse().ok()).ok_or_else(usage)?,
        None => return Err(usage()),
    };
    let tree = resvg::usvg::Tree::from_data(&std::fs::read(src)?, &resvg::usvg::Options::default())
        .map_err(|e| std::io::Error::other(format!("{src}: {e}")))?;
    let size = tree.size();
    let height = (size.height() * width as f32 / size.width()).round().max(1.0) as u32;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).ok_or_else(usage)?;
    let scale = width as f32 / size.width();
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    // tiny-skia keeps premultiplied alpha; PNG wants straight alpha.
    let px: Vec<[u8; 4]> = pixmap.pixels().iter().map(|p| { let c = p.demultiply(); [c.red(), c.green(), c.blue(), c.alpha()] }).collect();
    std::fs::write(out, encode_png(&Texels { w: width as usize, h: height as usize, px, quality: 1.0, drift: 0.0 }))
}

fn run(args: &Args) -> std::io::Result<()> {
    let observed = match &args.trace {
        Some(p) => catalog::Observed::load(p)?,
        None => Default::default(),
    };
    let mut assets = catalog::extract(&args.dat, &observed)?;
    if let Some(dir) = &args.rooms {
        let rooms = rooms::load(dir)?;
        eprintln!("{} rooms from {}", rooms.len(), dir.display());
        assets.extend(rooms);
    }
    if !args.only.is_empty() {
        assets.retain(|a| args.only.contains(&a.res) || (a.room && args.only.iter().any(|o| o == "ROOMS")));
    }
    if let Some(n) = args.limit {
        assets.truncate(n);
    }
    eprintln!("{} sprites ({} with observed palettes)", assets.len(), assets.iter().filter(|a| observed.by_sprite.contains_key(&a.content)).count());
    let (external, true_colour) = match &args.art {
        Some(dir) => load_art(dir, &mut assets)?,
        None => Default::default(),
    };
    match args.cmd.as_str() {
        "extract" => extract(args, &assets),
        "build" => build(args, &assets, external, &true_colour),
        "heads" => dump_heads(args, &observed),
        "inventory" => inventory::run(&args.dat, &args.work, args.recipes.as_deref(), args.rooms.as_deref(), &observed),
        other => Err(std::io::Error::other(format!("unknown command {other}"))),
    }
}

/// Debug: every talking head's groups and frames as pictures (WORK/heads).
fn dump_heads(args: &Args, observed: &catalog::Observed) -> std::io::Result<()> {
    let dir = args.work.join("heads");
    std::fs::create_dir_all(&dir)?;
    let mut f = std::fs::File::open(&args.dat)?;
    for e in gfx::dat::toc(&mut f)? {
        let Ok(res) = gfx::dat::load(&mut f, &e) else { continue };
        let Some(head) = heads::parse(&res) else { continue };
        let Some(sheet) = catalog::sheet_assets(&e.name, &res, observed) else { continue };
        let refs: Vec<Option<&Asset>> = sheet.iter().map(|a| a.as_ref()).collect();
        let name = e.name.trim_end_matches(".HSQ");
        eprintln!("{name}: origin {:?}, {} groups, {} frames: {:?}", head.origin, head.groups.len(), head.frames.len(), head.frames);
        let groups: Vec<Vec<[i32; 3]>> = head.groups.iter().map(|g| g.iter().map(|p| [p.sprite as i32, p.x, p.y]).collect()).collect();
        let layout = json!({"origin": head.origin, "groups": groups, "frames": head.frames, "lipsync": head.lipsync});
        std::fs::write(dir.join(format!("{name}.json")), serde_json::to_string(&layout)?)?;
        let pal = sheet.iter().flatten().next().map(|a| a.palette).unwrap_or_else(palette::grey);
        let save = |label: String, parts: &[heads::Part]| -> std::io::Result<()> {
            if let Some((_, _, w, h, px)) = heads::compose(parts, &refs) {
                let rgba: Vec<u8> = px.iter().flat_map(|p| p.map(|v| { let c = pal[v as usize]; [c[0], c[1], c[2], 255] }).unwrap_or([0; 4])).collect();
                art::write_png(&dir.join(format!("{name}-{label}.png")), w, h, true, &rgba)?;
            }
            Ok(())
        };
        for (i, g) in head.groups.iter().enumerate() {
            save(format!("g{i:02}"), g)?;
        }
        for (i, fr) in head.frames.iter().enumerate() {
            let parts: Vec<heads::Part> = fr.iter().flat_map(|&g| head.groups[g].iter().cloned()).collect();
            save(format!("f{i:02}"), &parts)?;
        }
        // Each group drawn over the default frame, at the frame's size.
        if let Some(base) = head.frames.first() {
            let base: Vec<heads::Part> = base.iter().flat_map(|&g| head.groups[g].iter().cloned()).collect();
            for (i, g) in head.groups.iter().enumerate() {
                let mut parts = base.clone();
                parts.extend(g.iter().cloned());
                save(format!("v{i:02}"), &parts)?;
            }
        }
    }
    eprintln!("heads in {}", dir.display());
    Ok(())
}

/// Finished HD art for some of `assets` (see `--art` in the header), by
/// asset hash; a sidecar palette replaces the asset's colours. Also the
/// assets whose sidecar asks for true colour.
fn load_art(dir: &Path, assets: &mut [Asset]) -> std::io::Result<(HashMap<u64, Rgb>, HashSet<u64>)> {
    let mut out = HashMap::new();
    let mut true_colour = HashSet::new();
    for a in assets.iter_mut() {
        let names = [format!("{:016x}", a.content), format!("{}-{:03}", a.res.trim_end_matches(".HSQ"), a.part)];
        let Some(name) = names.iter().find(|n| dir.join(format!("{n}.png")).exists()) else { continue };
        let mut img = art::read_rgb(&dir.join(format!("{name}.png")))?;
        let (w, h) = (a.stride * 4, a.height * 4);
        if img.h != h || (img.w != w && img.w != a.width * 4) {
            return Err(std::io::Error::other(format!("{name}.png is {}×{}, expected {w}×{h} (4× the sprite)", img.w, img.h)));
        }
        if img.w != w {
            // 4-bit rows are padded: repeat the last column.
            img.px = (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| img.px[y * img.w + x.min(img.w - 1)]).collect();
            img.w = w;
        }
        let side = dir.join(format!("{name}.json"));
        if side.exists() {
            let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&side)?)?;
            if let Some(dac) = json["dac"].as_str() {
                let bytes: Vec<u8> = (0..dac.len() / 2).filter_map(|i| u8::from_str_radix(&dac[i * 2..i * 2 + 2], 16).ok()).collect();
                if bytes.len() != 768 {
                    return Err(std::io::Error::other(format!("{name}.json: \"dac\" must be 768 hex bytes")));
                }
                a.palette = std::array::from_fn(|i| [0, 1, 2].map(|k| palette::six_to_eight(bytes[i * 3 + k])));
            }
            if json["true_colour"].as_bool() == Some(true) {
                if a.room {
                    eprintln!("{name}: true colour is for sprites only so far; the room plate stays in palette form");
                } else {
                    true_colour.insert(a.hash);
                }
            }
        }
        out.insert(a.hash, img);
    }
    eprintln!("{} pictures of finished art from {} ({} in true colour)", out.len(), dir.display(), true_colour.len());
    Ok((out, true_colour))
}

fn colour_image(a: &Asset) -> Vec<u8> {
    a.local.iter().enumerate().flat_map(|(i, &v)| if a.solid(i) { let c = a.rgb(v); [c[0], c[1], c[2], 255] } else { [0; 4] }).collect()
}

fn extract(args: &Args, assets: &[Asset]) -> std::io::Result<()> {
    let dir = args.work.join("extract");
    std::fs::create_dir_all(&dir)?;
    let mut list = Vec::new();
    for a in assets {
        let name = format!("{}-{:03}-{:016x}.png", a.res.trim_end_matches(".HSQ"), a.part, a.hash);
        art::write_png(&dir.join(&name), a.stride, a.height, true, &colour_image(a))?;
        list.push(json!({"id": format!("{:016x}", a.content), "res": a.res, "part": a.part, "w": a.stride, "h": a.height, "width": a.width, "format": a.format_name(), "file": name}));
    }
    std::fs::write(dir.join("catalog.json"), serde_json::to_string_pretty(&list)?)?;
    eprintln!("wrote {} previews to {}", assets.len(), dir.display());
    Ok(())
}

/// 4× upscales of each sprite (cropped to the sprite), cached per backend.
fn upscale(args: &Args, assets: &[Asset]) -> std::io::Result<HashMap<u64, Rgb>> {
    let mut out = HashMap::new();
    match &args.backend {
        Backend::Mmpx => {}
        Backend::Nearest => {
            for a in assets {
                let src = Rgb { w: a.stride, h: a.height, px: a.local.iter().map(|&v| a.rgb(v)).collect() };
                out.insert(a.hash, art::nearest(&src, 4));
            }
        }
        Backend::Command { template } => {
            let (rooms, sprites): (Vec<&Asset>, Vec<&Asset>) = assets.iter().partition(|a| a.room);
            for (group, n) in [(sprites, args.prescale), (rooms, args.room_prescale)] {
                if !group.is_empty() {
                    upscale_command(args, template, &group, n.max(1), &mut out)?;
                }
            }
            if args.face_context {
                // Talking-head parts: cut out of their face, upscaled whole.
                let wanted: std::collections::HashSet<u64> = assets.iter().map(|a| a.hash).collect();
                let observed = match &args.trace {
                    Some(p) => catalog::Observed::load(p)?,
                    None => Default::default(),
                };
                let contexts: Vec<heads::Context> =
                    heads::contexts(&args.dat, &observed)?.into_iter().filter(|c| c.parts.iter().any(|(h, ..)| wanted.contains(h))).collect();
                let pictures: Vec<&Asset> = contexts.iter().map(|c| &c.picture).collect();
                eprintln!("{} faces to upscale whole", pictures.len());
                let mut faces = HashMap::new();
                upscale_command(args, template, &pictures, args.prescale.max(1), &mut faces)?;
                let mut cut = 0;
                for c in &contexts {
                    let Some(up) = faces.get(&c.picture.hash) else { continue };
                    for &(hash, x, y) in &c.parts {
                        let Some(a) = assets.iter().find(|a| a.hash == hash) else { continue };
                        let (w, h) = (a.stride * 4, a.height * 4);
                        let px = (0..h).flat_map(|ty| (0..w).map(move |tx| (tx, ty))).map(|(tx, ty)| up.px[(y * 4 + ty) * up.w + x * 4 + tx]).collect();
                        out.insert(hash, Rgb { w, h, px });
                        cut += 1;
                    }
                }
                eprintln!("{cut} talking-head parts taken from their face");
            }
        }
    }
    Ok(out)
}

/// Upscale `assets` with the external upscaler, enlarged `n`× first;
/// results are cached per command and `n`.
fn upscale_command(args: &Args, template: &str, assets: &[&Asset], n: usize, out: &mut HashMap<u64, Rgb>) -> std::io::Result<()> {
    {
        {
            let cache = args.work.join("cache").join(format!("{}-p{n}", args.backend.id()));
            std::fs::create_dir_all(&cache)?;
            let mut todo = Vec::new();
            for &a in assets {
                let p = cache.join(format!("{:016x}.png", a.hash));
                match art::read_rgb(&p) {
                    Ok(img) if img.w == a.stride * 4 && img.h == a.height * 4 => {
                        out.insert(a.hash, img);
                    }
                    _ => todo.push(a),
                }
            }
            eprintln!("upscaling {} pictures ({} cached)", todo.len(), assets.len() - todo.len());
            if !todo.is_empty() {
                eprintln!("prescale {n}, guard {}", args.guard.filter(|g| *g > 0.0).map(|g| g.to_string()).unwrap_or("off".into()));
                let prepared: Vec<(u64, (Rgb, usize, usize))> = todo.iter().map(|a| (a.hash, art::model_input(a))).collect();
                let inputs: Vec<(u64, Rgb)> = prepared.iter().map(|(h, (img, _, _))| (*h, art::nearest(img, n))).collect();
                let results = art::run_command(template, &inputs, 4, &args.work.join("upscale"))?;
                let mut missing = 0;
                for (a, (_, (img, px, py))) in todo.iter().zip(&prepared) {
                    let Some(up) = results.get(&a.hash) else {
                        missing += 1;
                        continue;
                    };
                    if up.w != img.w * 4 * n || up.h != img.h * 4 * n {
                        return Err(std::io::Error::other(format!(
                            "the upscaler returned {}×{} for a {}×{} input: dune-hd needs exactly 4×",
                            up.w, up.h, img.w, img.h
                        )));
                    }
                    let up = if n > 1 { art::shrink(up, n) } else { Rgb { w: up.w, h: up.h, px: up.px.clone() } };
                    let Some(cropped) = art::crop(&up, *px, *py, 4, a.stride, a.height) else { continue };
                    let data: Vec<u8> = cropped.px.iter().flatten().copied().collect();
                    art::write_png(&cache.join(format!("{:016x}.png", a.hash)), cropped.w, cropped.h, false, &data)?;
                    out.insert(a.hash, cropped);
                }
                if missing > 0 {
                    eprintln!("warning: the upscaler produced no output for {missing} sprites");
                }
            }
        }
    }
    Ok(())
}

fn encode_png(t: &Texels) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut buf, t.w as u32, t.h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::High);
        let data: Vec<u8> = t.px.iter().flatten().copied().collect();
        enc.write_header().unwrap().write_image_data(&data).unwrap();
    }
    buf
}

/// True-colour art (pack version 4): RGB = the art, A = the palette form's
/// coverage, so both have the same silhouette.
fn true_colour_png(up: &Rgb, t: &Texels) -> Vec<u8> {
    let px = up.px.iter().zip(&t.px).map(|(c, t)| [c[0], c[1], c[2], t[3]]).collect();
    encode_png(&Texels { w: t.w, h: t.h, px, quality: 0.0, drift: 0.0 })
}

fn build(args: &Args, assets: &[Asset], external: HashMap<u64, Rgb>, true_colour: &HashSet<u64>) -> std::io::Result<()> {
    let todo: Vec<&Asset> = assets.iter().filter(|a| !external.contains_key(&a.hash)).collect();
    let mut ups = upscale(args, &todo.iter().map(|a| (*a).clone()).collect::<Vec<_>>())?;
    ups.extend(external.iter().map(|(h, img)| (*h, Rgb { w: img.w, h: img.h, px: img.px.clone() })));
    eprintln!("projecting onto the game's palette…");
    let results: Vec<(usize, Option<(Texels, Texels)>)> = assets
        .par_iter()
        .enumerate()
        .map(|(i, a)| {
            let pair = match args.backend {
                // Finished art: projected as made, no guard.
                _ if external.contains_key(&a.hash) => {
                    let up = &ups[&a.hash];
                    Some((art::project(a, up, 4), art::project(a, &art::halve(up), 2)))
                }
                Backend::Mmpx => Some((art::mmpx_texels(a, 4), art::mmpx_texels(a, 2))),
                _ => ups.get(&a.hash).map(|up| {
                    let (mut t4, mut t2) = (art::project(a, up, 4), art::project(a, &art::halve(up), 2));
                    if let Some(limit) = args.guard.filter(|g| *g > 0.0) {
                        art::guard(a, 4, &mut t4, limit);
                        art::guard(a, 2, &mut t2, limit);
                    }
                    (t4, t2)
                }),
            };
            (i, pair)
        })
        .collect();

    if args.previews {
        let dir = args.work.join("preview");
        std::fs::create_dir_all(&dir)?;
        for (i, pair) in &results {
            let (a, Some((t4, _))) = (&assets[*i], pair) else { continue };
            let (w, h) = (a.stride * 4, a.height * 4);
            let mut img = vec![40u8; w * 3 * h * 3];
            let mut put = |panel: usize, x: usize, y: usize, c: [u8; 3]| {
                let o = (y * w * 3 + panel * w + x) * 3;
                img[o..o + 3].copy_from_slice(&c);
            };
            for y in 0..h {
                for x in 0..w {
                    let v = a.local[(y / 4) * a.stride + x / 4];
                    if a.solid((y / 4) * a.stride + x / 4) {
                        put(0, x, y, a.rgb(v));
                    }
                    if let Some(up) = ups.get(&a.hash) {
                        put(1, x, y, up.px[y * w + x]);
                    }
                    let t = t4.px[y * w + x];
                    if t[3] != 0 {
                        let (p, q) = (a.rgb(t[0]), a.rgb(t[1]));
                        let f = t[2] as f32 / 255.0;
                        put(2, x, y, [0, 1, 2].map(|i| (p[i] as f32 * (1.0 - f) + q[i] as f32 * f).round() as u8));
                    }
                }
            }
            art::write_png(&dir.join(format!("{}-{:03}.png", a.res.trim_end_matches(".HSQ"), a.part)), w * 3, h, false, &img)?;
        }
        eprintln!("previews in {}", dir.display());
    }
    let file = std::fs::File::create(&args.out)?;
    let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let mut manifest_assets = Vec::new();
    let mut manifest_rooms = Vec::new();
    let mut rooms_kept = 0;
    let mut rows = String::new();
    let (mut kept, mut dropped, mut bytes) = (0, 0, 0usize);
    for (i, pair) in &results {
        let a = &assets[*i];
        let id = format!("{:016x}", a.content);
        let room = a.room;
        let big_enough = a.width >= args.min_size && a.height >= args.min_size;
        let text_art = !room && gfx::pack::text_art(&a.local, |v| a.opaque(v));
        let finished = external.contains_key(&a.hash);
        let (q4, q2, ok) = match pair {
            Some((t4, _)) if finished => (t4.quality, t4.drift, true),
            Some((t4, t2)) => (t4.quality, t4.drift, big_enough && !text_art &&t4.drift <= args.max_drift && t2.drift <= args.max_drift),
            None => (0.0, 0.0, false),
        };
        rows += &format!(
            "<tr class='{}'><td>{}</td><td>{}</td><td>{}×{}</td><td>{:.3}</td><td>{:.3}</td><td>{}</td></tr>\n",
            if ok { "ok" } else { "drop" },
            a.res,
            a.part,
            a.width,
            a.height,
            q4,
            q2,
            if finished { "in the pack (finished art)" } else if ok { "in the pack" } else { "left to the built-in upscaler" }
        );
        if !ok {
            dropped += 1;
            continue;
        }
        let (t4, t2) = pair.as_ref().unwrap();
        let dirs = if room { ["r4", "r2"] } else { ["a4", "a2"] };
        for (dir, t) in dirs.into_iter().zip([t4, t2]) {
            let png = encode_png(t);
            bytes += png.len();
            zip.add(&format!("{dir}/{id}.png"), &png)?;
        }
        if room {
            // What the plate depicts: R = palette index, A = drawn by the room.
            let low = Texels {
                w: a.stride,
                h: a.height,
                px: a.local.iter().enumerate().map(|(i, &v)| [v, 0, 0, if a.solid(i) { 255 } else { 0 }]).collect(),
                quality: 0.0,
                drift: 0.0,
            };
            let png = encode_png(&low);
            bytes += png.len();
            zip.add(&format!("rl/{id}.png"), &png)?;
            rooms_kept += 1;
            manifest_rooms.push(json!({"id": id, "room": a.part, "file": a.res.trim_start_matches("ROOM-"), "block_match": q4, "drift": q2, "finished_art": finished}));
        } else {
            let colour = true_colour.contains(&a.hash);
            if colour {
                let up = &ups[&a.hash];
                for (dir, (img, t)) in ["c4", "c2"].into_iter().zip([(up, t4), (&art::halve(up), t2)]) {
                    let png = true_colour_png(img, t);
                    bytes += png.len();
                    zip.add(&format!("{dir}/{id}.png"), &png)?;
                }
                let refs: Vec<u8> = (0..=255u8).flat_map(|v| a.rgb(v)).collect();
                bytes += refs.len();
                zip.add(&format!("cr/{id}.rgb"), &refs)?;
            }
            kept += 1;
            manifest_assets.push(json!({"id": id, "res": a.res, "part": a.part, "w": a.stride, "h": a.height, "width": a.width, "format": a.format_name(), "block_match": q4, "drift": q2, "finished_art": finished, "true_colour": colour}));
        }
    }
    let manifest = json!({
        "format": "dune-hybrid-hd-pack",
        "version": 4,
        "generator": format!("dune-hd {}", env!("CARGO_PKG_VERSION")),
        "game": {"toc": format!("{:016x}", catalog::toc_hash(&args.dat)?)},
        "scales": [4, 2],
        "texels": "RGBA: R = a, G = b (palette-local values: nibbles for 4-bit sprites), B = weight of b (0-255), A = coverage",
        "hash": "gfx::hash::image_hash of the sprite's decoded picture (palette-local values) and size",
        "model": {"name": args.model.0, "license": args.model.1, "source": args.model.2},
        "assets": manifest_assets,
        "rooms": manifest_rooms,
        "true_colour": "c4/<hash>.png, c2/<hash>.png: RGB = the art's own colour, A = coverage (as its texels'); cr/<hash>.rgb: 768 bytes, the 8-bit colour of each palette-local value in the palette the art was made in. Shown as colour × the value's current palette colour ÷ that reference colour, so it follows fades",
        "room_plates": "r4/<key>.png, r2/<key>.png: texels as above with global palette indices, for the 320 × 152 game area; rl/<key>.png: R = the low-resolution palette index the plate depicts, A = 255 where the room drew. Key: gfx::room::RoomKey of the room's drawing calls",
        "notice": NOTICE,
    });
    zip.add("manifest.json", serde_json::to_string_pretty(&manifest)?.as_bytes())?;
    zip.add("NOTICE.txt", format!("{NOTICE}\n\nUpscaler: {} ({}), {}\nGenerator: dune-hd (Apache-2.0), part of Dune Hybrid.\n", args.model.0, args.model.1, args.model.2).as_bytes())?;
    let report = format!(
        "<!doctype html><meta charset=utf-8><title>dune-hd report</title>\
         <style>body{{font:14px system-ui;margin:2em}}td,th{{padding:2px 8px;text-align:left}}tr.drop{{color:#a33}}</style>\
         <h1>dune-hd report</h1><p>Upscaler: {} ({}). {kept} sprites in the pack, {dropped} left to the built-in upscaler \
         (smaller than {} pixels, text art such as the intro's subtitle lines (opaque, two colours), \
         or drift above {}: the mean OKLab distance between the art's local colours and the original's).</p>\
         <p>Block match: the share of the original pixels whose 4×4 HD block mostly shows their own colour (low where the art smooths dithering).</p>\
         <p>{NOTICE}</p><table><tr><th>Resource</th><th>Part</th><th>Size</th><th>Block match 4×</th><th>Drift 4×</th><th></th></tr>{rows}</table>",
        args.model.0, args.model.1, args.min_size, args.max_drift
    );
    zip.add("report.html", report.as_bytes())?;
    std::fs::write(args.work.join("report.html"), &report).or_else(|_| {
        std::fs::create_dir_all(&args.work)?;
        std::fs::write(args.work.join("report.html"), &report)
    })?;
    zip.finish()?;
    eprintln!(
        "{}: {kept} sprites and {rooms_kept} rooms ({:.1} MB of art), {dropped} left to the built-in upscaler; report in {}",
        args.out.display(),
        bytes as f64 / 1e6,
        Path::new(&args.work).join("report.html").display()
    );
    Ok(())
}
