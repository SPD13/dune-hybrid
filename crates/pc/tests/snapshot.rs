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
