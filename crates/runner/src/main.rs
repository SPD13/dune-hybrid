//! Headless runner for development and regression checks.
//!
//!     dune-run --dir ../Cryogenic/dune --seconds 30 --shot-every 2 --out out
//!
//! Boots DNCDPRG.EXE from `--dir` (DUNE.DAT alongside), runs for the given
//! virtual time and writes PNG screenshots plus a log summary.

use std::{fs, path::PathBuf, process::ExitCode};

use pc::{Machine, RunExit, fs::DirFs};

struct Args {
    dir: PathBuf,
    seconds: f64,
    shot_every: f64,
    out: PathBuf,
    cmd: String,
    mips: f64,
}

fn parse() -> Result<Args, String> {
    let mut a = Args { dir: ".".into(), seconds: 10.0, shot_every: 1.0, out: "out".into(), cmd: String::new(), mips: 20.0 };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().ok_or(format!("missing value for {k}"));
        match k.as_str() {
            "--dir" => a.dir = v()?.into(),
            "--seconds" => a.seconds = v()?.parse().map_err(|e| format!("{e}"))?,
            "--shot-every" => a.shot_every = v()?.parse().map_err(|e| format!("{e}"))?,
            "--out" => a.out = v()?.into(),
            "--cmd" => a.cmd = v()?,
            "--mips" => a.mips = v()?.parse().map_err(|e| format!("{e}"))?,
            _ => return Err(format!("unknown option {k}")),
        }
    }
    Ok(a)
}

fn write_png(path: &PathBuf, rgb: &[u8]) {
    let file = fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), 320, 200);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgb).unwrap();
}

fn main() -> ExitCode {
    let args = match parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let exe = match fs::read(args.dir.join("DNCDPRG.EXE")) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("DNCDPRG.EXE: {e}");
            return ExitCode::FAILURE;
        }
    };
    fs::create_dir_all(&args.out).unwrap();
    let mut m = Machine::new(&exe, &args.cmd, Box::new(DirFs { root: args.dir.clone() })).unwrap();
    m.set_speed(args.mips * 1e6);

    let wall = std::time::Instant::now();
    let mut rgb = vec![0u8; 320 * 200 * 3];
    let mut t = 0.0;
    let mut shot = 0;
    let exit = loop {
        t += args.shot_every;
        let r = m.run_until((t.min(args.seconds) * 1e9) as u64);
        m.screen_rgb(&mut rgb);
        write_png(&args.out.join(format!("shot-{shot:04}.png")), &rgb);
        shot += 1;
        if r != RunExit::Deadline || t >= args.seconds {
            break r;
        }
    };
    let elapsed = wall.elapsed().as_secs_f64();
    println!("exit: {exit:?} after {:.2}s virtual, {:.2}s wall, {:.1} MIPS real", m.now_ns() as f64 / 1e9, elapsed, m.cpu.instructions as f64 / elapsed / 1e6);
    println!("cpu: {}", m.regs_string());
    println!("vga mode {:02x}; PIT {:.1} Hz", m.hw.vga.mode, m.hw.pit.irq0_hz());
    let ints: Vec<String> = m.int_counts.iter().enumerate().filter(|(_, c)| **c > 0).map(|(i, c)| format!("{i:02x}:{c}")).collect();
    println!("interrupts: {}", ints.join(" "));
    if !m.hw.unknown_ports.is_empty() {
        println!("unknown ports: {:x?}", m.hw.unknown_ports);
    }
    if !m.console.is_empty() {
        println!("console: {:?}", m.console);
    }
    for l in &m.log {
        println!("log: {l}");
    }
    ExitCode::SUCCESS
}
