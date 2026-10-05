//! Snapshot round trip on the real game (needs the game files; set DUNE_DIR,
//! default ../Cryogenic/dune): a run interrupted by save/restore must end in
//! exactly the same machine state as an uninterrupted one.
//!
//!     cargo test -p pc --release -- --ignored snapshot

use std::path::PathBuf;

use pc::{Machine, fs::MemFs};

fn game_dir() -> PathBuf {
    std::env::var_os("DUNE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../Cryogenic/dune"))
}

fn machine() -> Machine {
    let dir = game_dir();
    let exe = std::fs::read(dir.join("DNCDPRG.EXE")).expect("DNCDPRG.EXE");
    let mut fs = MemFs::default();
    fs.files.insert("DUNE.DAT".into(), std::fs::read(dir.join("DUNE.DAT")).expect("DUNE.DAT"));
    let mut m = Machine::new(&exe, "ADP330 SBP2227", Box::new(fs)).unwrap();
    m.set_speed(20e6);
    m.hw.audio.enabled = false;
    m
}

#[test]
#[ignore = "needs the game files"]
fn snapshot_round_trip_is_exact() {
    const SECOND: u64 = 1_000_000_000;
    let mut straight = machine();
    straight.run_until(30 * SECOND);

    let mut first = machine();
    first.run_until(15 * SECOND);
    let snap = first.save_state();
    println!("snapshot: {} KB", snap.len() / 1024);
    let mut resumed = machine();
    resumed.load_state(&snap).unwrap();
    resumed.run_until(30 * SECOND);

    assert_eq!(straight.cpu.instructions, resumed.cpu.instructions);
    assert_eq!(straight.regs_string(), resumed.regs_string());
    let diff = straight.hw.mem.iter().zip(&resumed.hw.mem).filter(|(a, b)| a != b).count();
    assert_eq!(diff, 0, "memory differs in {diff} bytes");
    assert_eq!(straight.hw.vga.dac, resumed.hw.vga.dac);
}

#[test]
#[ignore = "needs the game files"]
fn run_is_independent_of_slicing() {
    // The browser advances the machine in ~14 ms slices; a headless replay may
    // use one long call. Both must end in the same state.
    const END: u64 = 20_000_000_000;
    let mut whole = machine();
    whole.run_until(END);
    let mut sliced = machine();
    let mut t = 0;
    while t < END {
        t = (t + 14_000_000).min(END);
        sliced.run_until(t);
    }
    assert_eq!(whole.cpu.instructions, sliced.cpu.instructions);
    assert_eq!(whole.regs_string(), sliced.regs_string());
    assert!(whole.hw.mem == sliced.hw.mem, "memory differs");
}

#[test]
#[ignore = "needs the game files"]
fn graphics_recorder_does_not_change_the_run() {
    // Recording driver calls only watches: the machine must end in exactly
    // the same state with it on (and a hook attached) as with it off.
    const END: u64 = 20_000_000_000;
    let mut plain = machine();
    plain.run_until(END);
    let mut recorded = machine();
    recorded.gfx.enabled = true;
    let calls = std::rc::Rc::new(std::cell::Cell::new(0u64));
    let seen = calls.clone();
    recorded.gfx.hook = Some(Box::new(move |_, _| seen.set(seen.get() + 1)));
    recorded.run_until(END);
    assert!(calls.get() > 100, "recorder saw {} events", calls.get());
    assert_eq!(plain.save_state(), recorded.save_state());
}
