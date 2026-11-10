//! Headless runner for development and regression checks.
//!
//!     dune-run --dir ../Cryogenic/dune --seconds 30 --shot-every 2 --out out
//!
//! Boots DNCDPRG.EXE from `--dir` (DUNE.DAT alongside), runs for the given
//! virtual time and writes PNG screenshots plus a log summary.
//!
//! Graphics: `--gfx-trace FILE` writes every DNVGA driver call as JSON lines
//! (decoded operation, target buffer, sprite hash and sheet), `GFX_VERIFY=1`
//! checks `gfx::model` against the real driver call by call, `GFX_STATS=1`
//! counts calls per slot. `--snapshot FILE` starts from a saved state.
//!
//! Decompilation: `--coverage FILE` writes which seg000 instructions ran and
//! where indirect calls and jumps went (see `pc::coverage`; read it with
//! `tools/coverage.py`). It covers the `--rooms` harness too.
//! `--monkey SEED:START:STEP` adds random input from START seconds on, one
//! action every STEP seconds (clicks anywhere, mostly; a few keys), to reach
//! code without playing. The same seed gives the same run. If the random
//! input quits the game, the run rewinds to the last screenshot's state and
//! continues with new random input (at most 50 times).
//! `--phase-ladder START:STEP` raises the game phase (`[DS:2A]`) one step every
//! STEP seconds from START on, the way the story does: at the top of the
//! main loop (1000:D820) it calls `set_game_phase` (1000:121F) with the next
//! phase, which runs that phase's handler. Late-game code without playing.
//!
//! `--rec shadow` checks the recompiled routines (`crates/dune-rec`) against
//! the original as the game runs (see `pc::rec`) and prints a report;
//! `--rec-report FILE` also writes it as TSV. `--rec on` runs them instead
//! of the original. `--call-each N` calls every recompiled routine N times
//! from the main-loop top at the end of the run (see `callall`).

mod callall;
mod gfxcheck;
mod rooms;

use std::{cell::RefCell, fs, path::PathBuf, process::ExitCode, rc::Rc};

use pc::{
    Machine, RunExit,
    fs::{DirFs, OverlayFs},
};

struct Args {
    dir: PathBuf,
    seconds: f64,
    shot_every: f64,
    out: PathBuf,
    cmd: String,
    mips: f64,
    /// Scripted input: "t:key:SC:1|0" or "t:mouse:X:Y:BUTTONS", comma separated.
    events: Vec<(f64, Vec<String>)>,
    wav: Option<PathBuf>,
    /// Write every graphics driver call (JSON lines).
    gfx_trace: Option<PathBuf>,
    /// Start from a snapshot (as saved by the web app or `Machine::save_state`).
    snapshot: Option<PathBuf>,
    /// Follow the drawing with the reference HD compositor at this scale.
    hd: Option<usize>,
    /// Also write HD screenshots.
    hd_shots: bool,
    /// HD art pack (from `dune-hd`) for the HD compositor.
    hd_pack: Option<PathBuf>,
    /// After the run: make the game draw every room into this folder.
    rooms: Option<PathBuf>,
    /// Write seg000 code coverage here.
    coverage: Option<PathBuf>,
    /// Random input: seed, start time, seconds between actions.
    monkey: Option<(u64, f64, f64)>,
    /// Recompiled routines: shadow-check them, or run them.
    rec: Option<pc::rec::RecMode>,
    rec_report: Option<PathBuf>,
    call_each: u32,
    /// `--call-each`: only these entries (default all), and the virtual
    /// time one call may take.
    call_only: Option<Vec<u16>>,
    call_ms: u64,
}

/// Top of the main loop (`loc_0d820`), where nothing is half done.
const MAIN_LOOP: u16 = 0xd820;
/// `set_game_phase` (AL = phase): raises `[2Ah]`, runs the phase's handler.
const SET_GAME_PHASE: u16 = 0x121f;
/// Phases for the ladder. The handler table (word pointers at CS:11E7 +
/// phase / 2) has entries only for multiples of 4; other values the listing
/// compares against are passed on the way.
const PHASES: &[u8] = &[
    0x04, 0x08, 0x0c, 0x10, 0x14, 0x18, 0x1c, 0x20, 0x24, 0x28, 0x2c, 0x30, 0x34, 0x38, 0x3c, 0x40, 0x44, 0x48, 0x4c, 0x50, 0x54,
    0x58, 0x5c, 0x60, 0x64, 0x68, 0x6c,
];

fn parse() -> Result<Args, String> {
    let mut a = Args {
        dir: ".".into(),
        seconds: 10.0,
        shot_every: 1.0,
        out: "out".into(),
        cmd: String::new(),
        mips: 20.0,
        events: Vec::new(),
        wav: None,
        gfx_trace: None,
        snapshot: None,
        hd: None,
        hd_shots: false,
        hd_pack: None,
        rooms: None,
        coverage: None,
        monkey: None,
        rec: None,
        rec_report: None,
        call_each: 0,
        call_only: None,
        call_ms: 300,
    };
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
            "--wav" => a.wav = Some(v()?.into()),
            "--gfx-trace" => a.gfx_trace = Some(v()?.into()),
            "--snapshot" => a.snapshot = Some(v()?.into()),
            "--hd" => a.hd = Some(v()?.parse().map_err(|e| format!("{e}"))?),
            "--hd-shots" => a.hd_shots = true,
            "--hd-pack" => a.hd_pack = Some(v()?.into()),
            "--rooms" => a.rooms = Some(v()?.into()),
            "--coverage" => a.coverage = Some(v()?.into()),
            "--rec" => {
                a.rec = Some(match v()?.as_str() {
                    "shadow" => pc::rec::RecMode::Shadow,
                    "on" => pc::rec::RecMode::On,
                    other => return Err(format!("--rec shadow|on, got {other}")),
                })
            }
            "--rec-report" => a.rec_report = Some(v()?.into()),
            "--call-each" => a.call_each = v()?.parse().map_err(|e| format!("--call-each: {e}"))?,
            "--call-only" => {
                let list = v()?;
                let es: Result<Vec<u16>, _> = list.split(',').map(|e| u16::from_str_radix(e.trim(), 16)).collect();
                a.call_only = Some(es.map_err(|e| format!("--call-only HEX,HEX..: {e}"))?);
            }
            "--call-ms" => a.call_ms = v()?.parse().map_err(|e| format!("--call-ms: {e}"))?,
            "--phase-ladder" => {
                let spec = v()?;
                let (start, step) = spec.split_once(':').ok_or(format!("--phase-ladder START:STEP, got {spec}"))?;
                let start: f64 = start.parse().map_err(|e| format!("{e}"))?;
                let step: f64 = step.parse().map_err(|e| format!("{e}"))?;
                for (i, p) in PHASES.iter().enumerate() {
                    a.events.push((start + step * i as f64, vec!["phase".into(), format!("{p:02x}")]));
                }
                a.events.sort_by(|x, y| x.0.total_cmp(&y.0));
            }
            "--monkey" => {
                let spec = v()?;
                let p: Vec<&str> = spec.split(':').collect();
                let num = |i: usize| p.get(i).ok_or(format!("--monkey SEED:START:STEP, got {spec}"));
                let seed: u64 = num(0)?.parse().map_err(|e| format!("{e}"))?;
                let start: f64 = num(1)?.parse().map_err(|e| format!("{e}"))?;
                let step: f64 = num(2)?.parse().map_err(|e| format!("{e}"))?;
                a.monkey = Some((seed, start, step));
            }
            "--events" => {
                for ev in v()?.split(',') {
                    let parts: Vec<String> = ev.split(':').map(String::from).collect();
                    let t = parts[0].parse().map_err(|e| format!("{e}"))?;
                    a.events.push((t, parts[1..].to_vec()));
                }
                a.events.sort_by(|x, y| x.0.total_cmp(&y.0));
            }
            _ => return Err(format!("unknown option {k}")),
        }
    }
    Ok(a)
}

/// Summary of `--rec`: shadow results per routine, or call counts.
fn rec_report(rec: &pc::rec::Rec, tsv: Option<&std::path::Path>) {
    use std::fmt::Write as _;
    if rec.mode == pc::rec::RecMode::On {
        println!("rec: {} calls into Rust, {} bailed", rec.calls, rec.bails);
        let names: std::collections::HashMap<u16, &str> = dune_rec::names().iter().copied().collect();
        let mut bails: Vec<(u64, usize)> = rec.bail_at.iter().enumerate().filter(|(_, n)| **n > 0).map(|(i, n)| (*n, i)).collect();
        bails.sort_unstable_by(|a, b| b.cmp(a));
        for (n, ip) in bails.iter().take(15) {
            // The routine the bail happened in: the nearest entry at or below.
            let r = names.keys().filter(|&&e| e as usize <= *ip).max().map(|e| names[e]).unwrap_or("?");
            println!("rec bail: {n:9} at {ip:04x} (in {r})");
        }
        let mut ops: Vec<(u64, usize)> = rec.stepped_op.iter().enumerate().filter(|(_, n)| **n > 0).map(|(i, n)| (*n, i)).collect();
        ops.sort_unstable_by(|a, b| b.cmp(a));
        let ops: Vec<String> = ops.iter().take(16).map(|(n, op)| format!("{op:02x}:{n}")).collect();
        println!("rec stepped opcodes: {}", ops.join(" "));
        return;
    }
    let names: std::collections::HashMap<u16, &str> = dune_rec::names().iter().copied().collect();
    let (mut ok, mut diff, mut bailed, mut skipped) = (0, 0, 0, 0);
    let mut rows = String::from("entry	name	ok	diff	bailed	skipped	first_diff	reached
");
    for (e, s) in &rec.stats {
        ok += (s.ok > 0 && s.diff == 0) as u32;
        diff += (s.diff > 0) as u32;
        bailed += (s.bailed > 0 && s.diff == 0 && s.ok == 0) as u32;
        skipped += (s.skipped > 0 && s.ok + s.diff + s.bailed == 0) as u32;
        let hand = if dune_rec::is_hand(*e) { " [hand]" } else { "" };
        let name = format!("{}{hand}", names.get(e).copied().unwrap_or("?"));
        let fd = s.first_diff.clone().unwrap_or_default();
        let name = name.as_str();
        let _ = writeln!(rows, "{e:04x}	{name}	{}	{}	{}	{}	{fd}	{}", s.ok, s.diff, s.bailed, s.skipped, dune_rec::hand::reached(*e) + rec.returned[*e as usize]);
        if s.diff > 0 {
            println!("rec diff: {e:04x} {name}: {} of {} calls differ; first: {fd}", s.diff, s.ok + s.diff + s.bailed);
        }
    }
    println!(
        "rec shadow: {} routines called; {ok} verified equal, {diff} with differences, {bailed} equal with interpreted parts only, {skipped} not checked (too long)",
        rec.stats.len()
    );
    let total = (rec.replay_native + rec.replay_interp).max(1);
    println!(
        "rec replay: {} instructions, {:.1}% run by Rust",
        rec.replay_native + rec.replay_interp,
        100.0 * rec.replay_native as f64 / total as f64
    );
    println!(
        "rec through: {} stretches run through an interrupt, {} of them ending in a bail; {} calls replayed again bailing at them",
        rec.deferred, rec.deferred_bailed, rec.deferred_retried
    );
    if let Some(p) = tsv {
        fs::write(p, rows).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    }
}

/// Waits (up to 10 virtual seconds) for the top of the main loop, then makes
/// the game call `set_game_phase(phase)` from there, as if the loop had.
/// Returns the run's exit if the game stopped instead.
fn enter_phase(m: &mut Machine, phase: u8) -> Option<RunExit> {
    m.break_at = Some((pc::LOAD_SEG, MAIN_LOOP));
    let r = m.run_until(m.now_ns() + 10_000_000_000);
    m.break_at = None;
    match r {
        RunExit::Break => {
            let ds = pc::GAME_DS as usize * 16;
            println!("phase {:02x} -> {phase:02x} at {:.1}s", m.hw.mem[ds + 0x2a], m.now_ns() as f64 / 1e9);
            // CALL from the loop top: return address D820, AL = phase.
            let (ss, sp) = (m.cpu.sregs[pc::cpu::SS], m.cpu.regs[pc::cpu::SP].wrapping_sub(2));
            m.cpu.regs[pc::cpu::SP] = sp;
            let a = (ss as usize * 16 + sp as usize) & 0xf_ffff;
            m.hw.mem[a..a + 2].copy_from_slice(&MAIN_LOOP.to_le_bytes());
            m.cpu.regs[pc::cpu::AX] = (m.cpu.regs[pc::cpu::AX] & 0xff00) | phase as u16;
            m.cpu.ip = SET_GAME_PHASE;
            None
        }
        RunExit::Deadline => {
            println!("phase {phase:02x}: main loop not reached in 10 s, skipped");
            None
        }
        other => Some(other),
    }
}

/// Random input for coverage runs: mostly left clicks (half of them on the
/// interface strip at the bottom), some right clicks and keys.
fn monkey_events(seed: u64, start: f64, step: f64, end: f64) -> Vec<(f64, Vec<String>)> {
    let mut x = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
    let mut rnd = move |n: u64| {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x % n
    };
    let ev = |s: String| s.split(':').map(String::from).collect::<Vec<_>>();
    let mut out = Vec::new();
    let mut t = start;
    while t < end {
        let r = rnd(100);
        if r < 80 {
            let px = rnd(320);
            let py = if rnd(2) == 0 { 152 + rnd(48) } else { rnd(200) };
            let b = if r < 68 { 1 } else { 2 };
            out.push((t, ev(format!("mouse:{px}:{py}:0"))));
            out.push((t + 0.05, ev(format!("mouse:{px}:{py}:{b}"))));
            out.push((t + 0.15, ev(format!("mouse:{px}:{py}:0"))));
        } else {
            // Esc, Enter, Space, the arrow keys.
            let keys = ["01", "1c", "39", "48", "50", "4b", "4d"];
            let k = keys[rnd(keys.len() as u64) as usize];
            out.push((t, ev(format!("key:{k}:1"))));
            out.push((t + 0.1, ev(format!("key:{k}:0"))));
        }
        t += step;
    }
    out
}

fn write_png(path: &PathBuf, rgb: &[u8]) {
    write_png_sized(path, 320, 200, rgb);
}

fn write_png_sized(path: &PathBuf, w: u32, h: u32, rgb: &[u8]) {
    let file = fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(rgb).unwrap();
}

fn write_wav(path: &PathBuf, samples: &[f32]) {
    let mut d = Vec::with_capacity(44 + samples.len() * 2);
    let data_len = (samples.len() * 2) as u32;
    d.extend_from_slice(b"RIFF");
    d.extend_from_slice(&(36 + data_len).to_le_bytes());
    d.extend_from_slice(b"WAVEfmt ");
    d.extend_from_slice(&16u32.to_le_bytes());
    d.extend_from_slice(&1u16.to_le_bytes());
    d.extend_from_slice(&2u16.to_le_bytes());
    d.extend_from_slice(&pc::sound::OUTPUT_RATE.to_le_bytes());
    d.extend_from_slice(&(pc::sound::OUTPUT_RATE * 4).to_le_bytes());
    d.extend_from_slice(&4u16.to_le_bytes());
    d.extend_from_slice(&16u16.to_le_bytes());
    d.extend_from_slice(b"data");
    d.extend_from_slice(&data_len.to_le_bytes());
    for &s in samples {
        d.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    fs::write(path, d).unwrap();
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
    // Saves go to <out>/saves; the game directory is only read.
    let saves = args.out.join("saves");
    fs::create_dir_all(&saves).unwrap();
    let files = OverlayFs { game: DirFs { root: args.dir.clone() }, saves: DirFs { root: saves } };
    let mut m = Machine::new(&exe, &args.cmd, Box::new(files)).unwrap();
    m.trace_mouse = std::env::var_os("TRACE_MOUSE").is_some();
    m.idle_skip = std::env::var_os("NO_IDLE_SKIP").is_none();
    if let Ok(mask) = std::env::var("REPLACED_SONGS") {
        m.replaced_songs = u16::from_str_radix(mask.trim_start_matches("0x"), 16).unwrap_or(0);
    }
    if std::env::var_os("MUSIC_ONLY").is_some() {
        m.hw.audio.voice_gain = 0.0;
    }
    if std::env::var_os("TRACE_READS").is_some() {
        m.trace_reads = Some(Vec::new());
    }
    if std::env::var_os("TRACE_PORTS").is_some() {
        m.hw.trace_ports = Some(Vec::new());
    }
    if let Some(path) = &args.snapshot {
        let data = fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        if let Err(e) = m.load_state(&data) {
            eprintln!("snapshot: {e}");
            return ExitCode::FAILURE;
        }
    }
    if let Some(mode) = args.rec {
        let routines = dune_rec::routines();
        if routines.is_empty() {
            eprintln!("--rec: dune-rec was built without src/generated.rs (run dune-lift first)");
            return ExitCode::FAILURE;
        }
        println!("rec: {} routines, {mode:?}", routines.len());
        m.rec = Some(Box::new(pc::rec::Rec::new(mode, routines, dune_rec::leaders())));
    }
    if args.coverage.is_some() {
        m.coverage = Some(Box::default());
    }
    m.set_speed(args.mips * 1e6);
    let gfx_stats = std::env::var_os("GFX_STATS").is_some();
    let gfx_verify = std::env::var_os("GFX_VERIFY").is_some();
    let recorder = (gfx_verify || args.gfx_trace.is_some() || args.hd.is_some())
        .then(|| gfxcheck::Recorder::new(gfx_verify, args.gfx_trace.as_deref(), &args.dir.join("DUNE.DAT"), args.hd));
    let hd_overlay = std::env::var_os("HD_OVERLAY").is_some();
    let mut hd_totals = gfx::compose::PresentStats::default();
    if gfx_stats || recorder.is_some() {
        m.gfx.enabled = true;
    }
    if let (Some(rec), Some(path)) = (&recorder, &args.hd_pack)
        && let Some(hd) = rec.borrow_mut().hd.as_mut()
    {
        let mut file = fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let index = gfx::pack::PackIndex::read(&mut file).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        println!("hd pack: {} entries", index.len());
        let index = Rc::new(index);
        let file = Rc::new(RefCell::new(file));
        let (i2, f2) = (index.clone(), file.clone());
        let (i3, f3) = (index.clone(), file.clone());
        hd.comp.set_art(Some(Box::new(move |hash, k| index.sprite(&mut *file.borrow_mut(), hash, k))));
        hd.comp.set_plates(Some(Box::new(move |key, k| i2.plate(&mut *f2.borrow_mut(), key, k))));
        hd.comp.set_heads(Some(Box::new(move |key, k| i3.head(&mut *f3.borrow_mut(), key, k))));
    }
    let tracker = args.rooms.as_ref().map(|_| Rc::new(RefCell::new(rooms::Tracker::default())));
    if recorder.is_some() || tracker.is_some() {
        m.gfx.enabled = true;
        let rec = recorder.clone();
        let tr = tracker.clone();
        m.gfx.hook = Some(Box::new(move |e, hw| {
            if let Some(rec) = &rec {
                rec.borrow_mut().on_event(e, hw);
            }
            if let Some(tr) = &tr {
                tr.borrow_mut().on_event(e, hw);
            }
        }));
    }

    // CODE_DIFF=1: report seg000 code bytes the program changes (self-
    // modifying code), against the image as loaded.
    let code_image = std::env::var_os("CODE_DIFF").map(|_| m.hw.mem[0x10000..0x1f4b0].to_vec());
    // Times (--seconds, --events) count from the start, or from the snapshot.
    let t0 = m.now_ns() as f64 / 1e9;
    let wall = std::time::Instant::now();
    let mut rgb = vec![0u8; 320 * 200 * 3];
    let mut t = 0.0;
    let mut shot = 0;
    let mut audio: Vec<f32> = Vec::new();
    m.hw.audio.enabled = args.wav.is_some();
    // Scripted events plus random input; replaced when a monkey run rewinds.
    let schedule = |from: f64, seed_bump: u64| {
        let mut ev: Vec<(f64, Vec<String>)> = args.events.iter().filter(|e| e.0 >= from).cloned().collect();
        if let Some((seed, start, step)) = args.monkey {
            ev.extend(monkey_events(seed.wrapping_add(seed_bump), start.max(from), step, args.seconds));
            ev.sort_by(|x, y| x.0.total_cmp(&y.0));
        }
        ev
    };
    let mut events = schedule(0.0, 0);
    let mut ei = 0;
    let mut checkpoint: Option<(Vec<u8>, f64)> = None;
    let mut rewinds = 0u64;
    let trace_music = std::env::var_os("TRACE_MUSIC").is_some();
    let mut last_music = pc::MusicState::default();
    let exit = loop {
        if args.monkey.is_some() {
            checkpoint = Some((m.save_state(), t));
        }
        t += args.shot_every;
        let end = t.min(args.seconds);
        let mut r = RunExit::Deadline;
        while let Some((et, ev)) = events.get(ei) {
            if *et > end {
                break;
            }
            r = m.run_until(((t0 + *et) * 1e9) as u64);
            if r != RunExit::Deadline {
                break;
            }
            match ev[0].as_str() {
                "key" => m.key(u8::from_str_radix(&ev[1], 16).unwrap(), ev[2] == "1"),
                "mouse" => m.mouse_input(ev[1].parse().unwrap(), ev[2].parse().unwrap(), ev[3].parse().unwrap()),
                "phase" => {
                    let phase = u8::from_str_radix(&ev[1], 16).unwrap();
                    if let Some(r2) = enter_phase(&mut m, phase) {
                        r = r2;
                        break;
                    }
                }
                _ => eprintln!("unknown event {ev:?}"),
            }
            ei += 1;
        }
        if r == RunExit::Deadline {
            if trace_music {
                // Step in 10 ms slices to log music state changes.
                let mut tt = m.now_ns();
                while tt < ((t0 + end) * 1e9) as u64 && r == RunExit::Deadline {
                    tt += 10_000_000;
                    r = m.run_until(tt.min(((t0 + end) * 1e9) as u64));
                    while let Some((at, e)) = m.music.events.pop_front() {
                        let name = |s: u8| s.checked_sub(1).and_then(|i| pc::SONG_NAMES.get(i as usize)).copied().unwrap_or("?");
                        match e {
                            pc::music::MusicEvent::Play { song } => println!("event {:8.2}s PLAY {song} {}", at as f64 / 1e9, name(song)),
                            other => println!("event {:8.2}s {other:?}", at as f64 / 1e9),
                        }
                    }
                    let ms = m.music_state();
                    if ms.voice != last_music.voice {
                        println!("voice {:8.2}s {}", m.now_ns() as f64 / 1e9, if ms.voice { "on" } else { "off" });
                    }
                    if (ms.song, ms.status) != (last_music.song, last_music.status) {
                        let name = ms.song.checked_sub(1).and_then(|i| pc::SONG_NAMES.get(i as usize)).unwrap_or(&"-");
                        println!("music {:8.2}s song {:2} {name:9} status {:02x}", m.now_ns() as f64 / 1e9, ms.song, ms.status);
                    }
                    last_music = ms;
                }
            } else {
                r = m.run_until(((t0 + end) * 1e9) as u64);
            }
        }
        audio.extend(m.take_audio());
        m.screen_rgb(&mut rgb);
        write_png(&args.out.join(format!("shot-{shot:04}.png")), &rgb);
        if let Some(rec) = &recorder
            && let Some(hd) = rec.borrow().hd.as_ref().map(|f| &f.comp)
        {
            {
                let k = hd.k;
                let mut big = vec![0u8; 320 * k * 200 * k * 3];
                // The DAC holds 6-bit values; scale to 8 bits for the PNG.
                let dac8 = m.hw.vga.dac.map(|c| c.map(|v| (v << 2) | (v >> 4)));
                let st = hd.present(&m.hw.mem[0xa0000..0xa0000 + 64000], &dac8, &mut big, hd_overlay);
                hd_totals.hd += st.hd;
                hd_totals.fallback += st.fallback;
                hd_totals.dominance_errors += st.dominance_errors;
                println!(
                    "hd {shot:04}: {:.1}% HD, {:.1}% fallback, {} dominance errors",
                    st.hd as f64 / 640.0,
                    st.fallback as f64 / 640.0,
                    st.dominance_errors
                );
                if args.hd_shots {
                    write_png_sized(&args.out.join(format!("hd-{shot:04}.png")), (320 * k) as u32, (200 * k) as u32, &big);
                }
                if std::env::var_os("HD_SEAMS").is_some()
                    && let Some(tr) = &tracker
                    && let Some(room) = tr.borrow().seen.last()
                {
                    let (join, inside, n) = rooms::seams(room, &m.hw.mem[0xa0000..0xa0000 + 64000], &big, &dac8, k);
                    if n > 0 {
                        println!("seams {shot:04}: room {} joins {join:.2} ({n} borders), inside {inside:.2}", room.room);
                    }
                }
            }
        }
        shot += 1;
        if let (RunExit::Exited(_), Some((state, at))) = (&r, &checkpoint)
            && rewinds < 50
        {
            rewinds += 1;
            println!("monkey: the game exited at {:.1}s; rewinding to {at:.1}s", m.now_ns() as f64 / 1e9 - t0);
            m.exited = None;
            m.load_state(state).expect("rewind");
            t = *at;
            events = schedule(*at, rewinds * 0x1_0000);
            ei = 0;
            continue;
        }
        if r != RunExit::Deadline || t >= args.seconds {
            break r;
        }
    };
    if args.call_each > 0 {
        let entries: Vec<u16> = match &args.call_only {
            Some(es) => es.clone(),
            None => dune_rec::routines().iter().map(|r| r.0).collect(),
        };
        callall::run(&mut m, MAIN_LOOP, &entries, args.call_each, args.call_ms * 1_000_000);
    }
    if let (Some(dir), Some(tr)) = (&args.rooms, &tracker) {
        rooms::capture(&mut m, dir, tr.clone());
    }
    if let (Some(path), Some(c)) = (&args.coverage, &m.coverage) {
        fs::write(path, c.to_text()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        println!("coverage: {} seg000 instructions, {} indirect transfers -> {}", c.count(), c.indirect.len(), path.display());
    }
    if let Some(rec) = &m.rec {
        rec_report(rec, args.rec_report.as_deref());
    }
    if let Some(img) = &code_image {
        let changed: Vec<String> =
            img.iter().zip(&m.hw.mem[0x10000..0x1f4b0]).enumerate().filter(|(_, (a, b))| a != b).map(|(i, (a, b))| format!("{i:04x}:{a:02x}->{b:02x}")).collect();
        println!("code changed: {}", changed.join(" "));
    }
    let elapsed = wall.elapsed().as_secs_f64();
    if let Some(w) = &args.wav {
        write_wav(w, &audio);
        let peak = audio.iter().fold(0f32, |a, &b| a.max(b.abs()));
        println!("audio: {:.1}s, peak {peak:.3}", audio.len() as f64 / 2.0 / pc::sound::OUTPUT_RATE as f64);
    }
    println!("exit: {exit:?} after {:.2}s virtual, {:.2}s wall, {:.1} MIPS real", m.now_ns() as f64 / 1e9, elapsed, m.cpu.instructions as f64 / elapsed / 1e6);
    println!("cpu: {}", m.regs_string());
    println!("battery saver skipped {:.1}% of virtual time", m.idle_skipped_ns as f64 / m.now_ns() as f64 * 100.0);
    println!("vga mode {:02x}; PIT {:.1} Hz", m.hw.vga.mode, m.hw.pit.irq0_hz());
    let ints: Vec<String> = m.int_counts.iter().enumerate().filter(|(_, c)| **c > 0).map(|(i, c)| format!("{i:02x}:{c}")).collect();
    println!("interrupts: {}", ints.join(" "));
    if !m.hw.unknown_ports.is_empty() {
        println!("unknown ports: {:x?}", m.hw.unknown_ports);
    }
    if !m.console.is_empty() {
        println!("console: {:?}", m.console);
    }
    if let Some(t) = &m.hw.trace_ports {
        for (ns, port, v, w) in t.iter().take(400) {
            println!("port {:9.3}ms {} {port:03x} {v:02x}", *ns as f64 / 1e6, if *w { "OUT" } else { "IN " });
        }
    }
    if let Some(reads) = &m.trace_reads {
        // Name DUNE.DAT reads after the archive's table of contents.
        let toc: Vec<(u64, u64, String)> = fs::File::open(args.dir.join("DUNE.DAT"))
            .and_then(|mut f| gfx::dat::toc(&mut f))
            .unwrap_or_default()
            .into_iter()
            .map(|e| (e.offset as u64, e.size as u64, e.name))
            .collect();
        let mut last = String::new();
        for (ns, file, pos, len) in reads {
            let res = toc.iter().find(|(o, s, _)| *pos >= *o && *pos < o + s).map(|t| t.2.clone()).unwrap_or_else(|| file.clone());
            if res != last && !res.ends_with(".VOC") && !res.ends_with(".HNM") {
                println!("read {:8.3}s {res} ({len} bytes)", *ns as f64 / 1e9);
            }
            last = res;
        }
    }
    for l in &m.log {
        println!("log: {l}");
    }
    if let Some(rec) = &recorder
        && let Some(hd) = rec.borrow().hd.as_ref()
        && args.hd_pack.is_some()
    {
        println!("hd pack: {} sprites from the pack, {} without art", hd.comp.art_hits, hd.comp.art_misses);
        println!("hd pack: {} rooms from plates, {} without", hd.comp.plate_hits, hd.comp.plate_misses);
        println!("hd pack: {} head draws from plates, {} without", hd.comp.head_hits, hd.comp.head_misses);
    }
    if hd_totals.hd + hd_totals.fallback > 0 {
        let n = (hd_totals.hd + hd_totals.fallback) as f64 / 100.0;
        println!(
            "hd total: {:.1}% HD, {:.1}% fallback, {} dominance errors",
            hd_totals.hd as f64 / n,
            hd_totals.fallback as f64 / n,
            hd_totals.dominance_errors
        );
    }
    if gfx_stats {
        let calls: Vec<String> = m.gfx.counts.iter().enumerate().filter(|(_, c)| **c > 0).map(|(i, c)| format!("{i}:{c}")).collect();
        println!("gfx slot calls: {}", calls.join(" "));
        let var = |off: usize| u16::from_le_bytes([m.hw.mem[pc::GAME_DS as usize * 16 + off], m.hw.mem[pc::GAME_DS as usize * 16 + off + 1]]);
        println!(
            "gfx buffers: [DBD8]={:04x} [DBD6]={:04x} [DBDE]={:04x} [DBDA] (target)={:04x} [DC32]={:04x}",
            var(0xdbd8),
            var(0xdbd6),
            var(0xdbde),
            var(0xdbda),
            var(0xdc32)
        );
    }
    m.gfx.hook = None;
    if let Some(rec) = &recorder {
        let rec = rec.borrow();
        if gfx_verify {
            print!("gfx verify:\n{}", rec.summary());
            for r in &rec.reports {
                println!("  {r}");
            }
        }
    }
    if let Some(path) = std::env::var_os("DUMP_DRIVER") {
        // The graphics driver's code segment (far pointers at DS:38B5 + 4n).
        let seg = u16::from_le_bytes([m.hw.mem[pc::GAME_DS as usize * 16 + 0x38b7], m.hw.mem[pc::GAME_DS as usize * 16 + 0x38b8]]) as usize;
        fs::write(&path, &m.hw.mem[seg * 16..seg * 16 + 0x10000]).unwrap();
        println!("driver segment {seg:04x} dumped to {path:?}");
    }
    ExitCode::SUCCESS
}
