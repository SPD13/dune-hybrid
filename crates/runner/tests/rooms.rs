//! The room harness (`dune-run --rooms`) against the real game (needs the
//! game files; set DUNE_DIR, default ../Cryogenic/dune): after skipping the
//! intro and walking to the balcony, every location's rooms are drawn; each
//! room the game drew while playing must be among them, pixel for pixel, and
//! every drawing call must match the driver (`GFX_VERIFY`).
//!
//!     cargo test -p runner --release -- --ignored rooms

use std::{path::PathBuf, process::Command};

fn game_dir() -> PathBuf {
    std::env::var_os("DUNE_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../Cryogenic/dune"))
}

#[test]
#[ignore = "needs the game files"]
fn rooms_match_the_game() {
    let out = std::env::temp_dir().join(format!("dune-rooms-test-{}", std::process::id()));
    let events = "61.5:key:1:1,61.6:key:1:0,76.0:mouse:130:179:1,76.15:mouse:130:179:0,79.0:mouse:130:163:1,79.15:mouse:130:163:0,115.0:mouse:289:177:1,115.15:mouse:289:177:0";
    let run = Command::new(env!("CARGO_BIN_EXE_dune-run"))
        .env("GFX_VERIFY", "1")
        .args(["--dir", game_dir().to_str().unwrap(), "--seconds", "130", "--shot-every", "1000", "--events", events])
        .arg("--out")
        .arg(&out)
        .arg("--rooms")
        .arg(out.join("rooms"))
        .output()
        .expect("dune-run");
    let text = String::from_utf8_lossy(&run.stdout);
    let _ = std::fs::remove_dir_all(&out);
    let line = |needle: &str| text.lines().find(|l| l.contains(needle)).unwrap_or_else(|| panic!("no '{needle}' line:\n{text}")).to_string();

    let distinct = line("distinct rooms");
    let n: usize = distinct.split_whitespace().nth(1).and_then(|n| n.parse().ok()).unwrap_or(0);
    assert!(n >= 27, "{distinct}");
    assert!(!text.contains("rooms: failed"), "{text}");
    let live: Vec<&str> = text.lines().filter(|l| l.starts_with("rooms: live room")).collect();
    assert!(live.len() >= 4, "{text}");
    for l in &live {
        assert!(l.contains(": 0 of "), "{l}");
    }
    let found = line("live rooms found in the harness");
    let (a, b) = found.split_whitespace().nth(1).unwrap().split_once('/').unwrap();
    assert_eq!(a, b, "{found}");
    for l in text.lines().filter(|l| l.trim_start().starts_with("slot ")) {
        assert!(l.contains(" 0 mismatch"), "{l}");
    }
    assert!(text.lines().any(|l| l.trim_start().starts_with("slot 19:") && !l.contains(" 0 ok")), "no line drawn:\n{text}");
}
