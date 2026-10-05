//! dune-hd: generates an HD art pack for Dune Hybrid from your own DUNE.DAT.
//!
//!     dune-hd build --dat path/to/DUNE.DAT --out dune-hd-pack.zip [options]
//!     dune-hd extract --dat path/to/DUNE.DAT [--work work]     # sprite previews
//!
//! Options for `build`:
//!   --work DIR             cache and intermediate files (default: work)
//!   --backend NAME         mmpx (default), nearest, or command
//!   --command TEMPLATE     external upscaler run once over a folder of PNGs;
//!                          {in}/{out} are folders, {scale} is 4
//!   --model-name / --model-license / --model-source   recorded in the pack
//!   --trace FILE           palettes observed with `dune-run --gfx-trace`
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
//!
//! The pack is made from your copy of the game and is for your own use:
//! do not redistribute it.

mod art;
mod catalog;
mod palette;
mod zip;

use std::{
    collections::HashMap,
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
    only: Vec<String>,
    limit: Option<usize>,
    max_drift: f32,
    min_size: usize,
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
        only: Vec::new(),
        limit: None,
        max_drift: 0.05,
        min_size: 6,
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
            "--only" => a.only = v()?.split(',').map(|s| s.trim().to_uppercase()).collect(),
            "--limit" => a.limit = Some(v()?.parse().map_err(|e| format!("{e}"))?),
            "--max-drift" => a.max_drift = v()?.parse().map_err(|e| format!("{e}"))?,
            "--min-size" => a.min_size = v()?.parse().map_err(|e| format!("{e}"))?,
            "--previews" => a.previews = true,
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
        "command" => Backend::Command { template: template.ok_or("--backend command needs --command TEMPLATE")? },
        other => return Err(format!("unknown backend {other}")),
    };
    Ok(a)
}

fn main() {
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

fn run(args: &Args) -> std::io::Result<()> {
    let observed = match &args.trace {
        Some(p) => catalog::Observed::load(p)?,
        None => Default::default(),
    };
    let mut assets = catalog::extract(&args.dat, &observed)?;
    if !args.only.is_empty() {
        assets.retain(|a| args.only.contains(&a.res));
    }
    if let Some(n) = args.limit {
        assets.truncate(n);
    }
    eprintln!("{} sprites ({} with observed palettes)", assets.len(), assets.iter().filter(|a| observed.by_sprite.contains_key(&a.content)).count());
    match args.cmd.as_str() {
        "extract" => extract(args, &assets),
        "build" => build(args, &assets),
        other => Err(std::io::Error::other(format!("unknown command {other}"))),
    }
}

fn colour_image(a: &Asset) -> Vec<u8> {
    a.local.iter().flat_map(|&v| if a.opaque(v) { let c = a.rgb(v); [c[0], c[1], c[2], 255] } else { [0; 4] }).collect()
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
            let cache = args.work.join("cache").join(args.backend.id());
            std::fs::create_dir_all(&cache)?;
            let mut todo = Vec::new();
            for a in assets {
                let p = cache.join(format!("{:016x}.png", a.hash));
                match art::read_rgb(&p) {
                    Ok(img) if img.w == a.stride * 4 && img.h == a.height * 4 => {
                        out.insert(a.hash, img);
                    }
                    _ => todo.push(a),
                }
            }
            eprintln!("upscaling {} sprites ({} cached)", todo.len(), out.len());
            if !todo.is_empty() {
                let prepared: Vec<(u64, (Rgb, usize, usize))> = todo.iter().map(|a| (a.hash, art::model_input(a))).collect();
                let inputs: Vec<(u64, Rgb)> = prepared.iter().map(|(h, (img, _, _))| (*h, Rgb { w: img.w, h: img.h, px: img.px.clone() })).collect();
                let results = art::run_command(template, &inputs, 4, &args.work.join("upscale"))?;
                let mut missing = 0;
                for (a, (_, (img, px, py))) in todo.iter().zip(&prepared) {
                    let Some(up) = results.get(&a.hash) else {
                        missing += 1;
                        continue;
                    };
                    if up.w != img.w * 4 || up.h != img.h * 4 {
                        return Err(std::io::Error::other(format!(
                            "the upscaler returned {}×{} for a {}×{} input: dune-hd needs exactly 4×",
                            up.w, up.h, img.w, img.h
                        )));
                    }
                    let Some(cropped) = art::crop(up, *px, *py, 4, a.stride, a.height) else { continue };
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
    Ok(out)
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

fn build(args: &Args, assets: &[Asset]) -> std::io::Result<()> {
    let ups = upscale(args, assets)?;
    eprintln!("projecting onto the game's palette…");
    let results: Vec<(usize, Option<(Texels, Texels)>)> = assets
        .par_iter()
        .enumerate()
        .map(|(i, a)| {
            let pair = match args.backend {
                Backend::Mmpx => Some((art::mmpx_texels(a, 4), art::mmpx_texels(a, 2))),
                _ => ups.get(&a.hash).map(|up| (art::project(a, up, 4), art::project(a, &art::halve(up), 2))),
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
                    if a.opaque(v) {
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
    let mut rows = String::new();
    let (mut kept, mut dropped, mut bytes) = (0, 0, 0usize);
    for (i, pair) in &results {
        let a = &assets[*i];
        let id = format!("{:016x}", a.content);
        let big_enough = a.width >= args.min_size && a.height >= args.min_size;
        let (q4, q2, ok) = match pair {
            Some((t4, t2)) => (t4.quality, t4.drift, big_enough && t4.drift <= args.max_drift && t2.drift <= args.max_drift),
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
            if ok { "in the pack" } else { "left to the built-in upscaler" }
        );
        if !ok {
            dropped += 1;
            continue;
        }
        let (t4, t2) = pair.as_ref().unwrap();
        for (dir, t) in [("a4", t4), ("a2", t2)] {
            let png = encode_png(t);
            bytes += png.len();
            zip.add(&format!("{dir}/{id}.png"), &png)?;
        }
        kept += 1;
        manifest_assets.push(json!({"id": id, "res": a.res, "part": a.part, "w": a.stride, "h": a.height, "width": a.width, "format": a.format_name(), "block_match": q4, "drift": q2}));
    }
    let manifest = json!({
        "format": "dune-hybrid-hd-pack",
        "version": 2,
        "generator": format!("dune-hd {}", env!("CARGO_PKG_VERSION")),
        "game": {"toc": format!("{:016x}", catalog::toc_hash(&args.dat)?)},
        "scales": [4, 2],
        "texels": "RGBA: R = a, G = b (palette-local values: nibbles for 4-bit sprites), B = weight of b (0-255), A = coverage",
        "hash": "gfx::hash::image_hash of the sprite's decoded picture (palette-local values) and size",
        "model": {"name": args.model.0, "license": args.model.1, "source": args.model.2},
        "assets": manifest_assets,
        "notice": NOTICE,
    });
    zip.add("manifest.json", serde_json::to_string_pretty(&manifest)?.as_bytes())?;
    zip.add("NOTICE.txt", format!("{NOTICE}\n\nUpscaler: {} ({}), {}\nGenerator: dune-hd (Apache-2.0), part of Dune Hybrid.\n", args.model.0, args.model.1, args.model.2).as_bytes())?;
    let report = format!(
        "<!doctype html><meta charset=utf-8><title>dune-hd report</title>\
         <style>body{{font:14px system-ui;margin:2em}}td,th{{padding:2px 8px;text-align:left}}tr.drop{{color:#a33}}</style>\
         <h1>dune-hd report</h1><p>Upscaler: {} ({}). {kept} sprites in the pack, {dropped} left to the built-in upscaler \
         (smaller than {} pixels, or drift above {}: the mean OKLab distance between the art's local colours and the original's).</p>\
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
        "{}: {kept} sprites ({:.1} MB of art), {dropped} left to the built-in upscaler; report in {}",
        args.out.display(),
        bytes as f64 / 1e6,
        Path::new(&args.work).join("report.html").display()
    );
    Ok(())
}
