//! Headless runner for development and regression checks.
//!
//!     dune-run --dir ../Cryogenic/dune --seconds 30 --shot-every 2 --out out
//!
//! Boots DNCDPRG.EXE from `--dir` (DUNE.DAT alongside), runs for the given
//! virtual time and writes PNG screenshots plus a log summary.

use std::{fs, path::PathBuf, process::ExitCode};

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
}

fn parse() -> Result<Args, String> {
    let mut a = Args { dir: ".".into(), seconds: 10.0, shot_every: 1.0, out: "out".into(), cmd: String::new(), mips: 20.0, events: Vec::new(), wav: None };
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

fn write_png(path: &PathBuf, rgb: &[u8]) {
    let file = fs::File::create(path).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), 320, 200);
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
    m.set_speed(args.mips * 1e6);

    let wall = std::time::Instant::now();
    let mut rgb = vec![0u8; 320 * 200 * 3];
    let mut t = 0.0;
    let mut shot = 0;
    let mut audio: Vec<f32> = Vec::new();
    m.hw.audio.enabled = args.wav.is_some();
    let mut events = args.events.iter().peekable();
    let trace_music = std::env::var_os("TRACE_MUSIC").is_some();
    let mut last_music = pc::MusicState::default();
    let exit = loop {
        t += args.shot_every;
        let end = t.min(args.seconds);
        let mut r = RunExit::Deadline;
        while let Some((et, ev)) = events.peek() {
            if *et > end {
                break;
            }
            r = m.run_until((*et * 1e9) as u64);
            if r != RunExit::Deadline {
                break;
            }
            match ev[0].as_str() {
                "key" => m.key(u8::from_str_radix(&ev[1], 16).unwrap(), ev[2] == "1"),
                "mouse" => m.mouse_input(ev[1].parse().unwrap(), ev[2].parse().unwrap(), ev[3].parse().unwrap()),
                _ => eprintln!("unknown event {ev:?}"),
            }
            events.next();
        }
        if r == RunExit::Deadline {
            if trace_music {
                // Step in 10 ms slices to log music state changes.
                let mut tt = m.now_ns();
                while tt < (end * 1e9) as u64 && r == RunExit::Deadline {
                    tt += 10_000_000;
                    r = m.run_until(tt.min((end * 1e9) as u64));
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
                r = m.run_until((end * 1e9) as u64);
            }
        }
        audio.extend(m.take_audio());
        m.screen_rgb(&mut rgb);
        write_png(&args.out.join(format!("shot-{shot:04}.png")), &rgb);
        shot += 1;
        if r != RunExit::Deadline || t >= args.seconds {
            break r;
        }
    };
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
        let dat = fs::read(args.dir.join("DUNE.DAT")).unwrap_or_default();
        let count = u16::from_le_bytes([dat[0], dat[1]]) as usize;
        let mut toc = Vec::new();
        for i in 0..count {
            let e = &dat[2 + i * 25..2 + (i + 1) * 25];
            let name: String = e[..16].iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
            let size = u32::from_le_bytes(e[16..20].try_into().unwrap()) as u64;
            let off = u32::from_le_bytes(e[20..24].try_into().unwrap()) as u64;
            if !name.is_empty() {
                toc.push((off, size, name));
            }
        }
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
    ExitCode::SUCCESS
}
