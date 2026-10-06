//! Reference HD compositor on scripted scenes (needs the game files; set
//! DUNE_DIR, default ../Cryogenic/dune). For each scene: every HD pixel's
//! block depicts the screen's pixel, under 5% of the screen falls back to
//! low resolution, and the HD image matches its golden hash.
//!
//!     cargo test -p runner --release -- --ignored hd_golden
//!     PRINT_GOLDEN=1 cargo test ... -- --nocapture   # to update the hashes

use std::{cell::RefCell, path::PathBuf, rc::Rc};

use gfx::{Mem, Regs, compose::Follower, hash::fnv64};
use pc::{
    Machine,
    fs::MemFs,
    gfx::{DriverCall, GfxEvent},
};

fn game_dir() -> PathBuf {
    std::env::var_os("DUNE_DIR").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../Cryogenic/dune"))
}

fn regs(c: &DriverCall) -> Regs {
    c.regs()
}

enum Input {
    Key(u8, bool),
    Mouse(i32, i32, u16),
}

#[test]
#[ignore = "needs the game files"]
fn hd_golden() {
    let dir = game_dir();
    let exe = std::fs::read(dir.join("DNCDPRG.EXE")).expect("DNCDPRG.EXE");
    let mut fs = MemFs::default();
    fs.files.insert("DUNE.DAT".into(), std::fs::read(dir.join("DUNE.DAT")).expect("DUNE.DAT"));
    let mut m = Machine::new(&exe, "ADP330 SBP2227", Box::new(fs)).unwrap();
    m.hw.audio.enabled = false;
    let follower = Rc::new(RefCell::new(Follower::new(4)));
    let f = follower.clone();
    m.gfx.enabled = true;
    m.gfx.hook = Some(Box::new(move |e, hw| match e {
        GfxEvent::Enter(c) => {
            f.borrow_mut().enter(c.slot, &regs(c), Mem(&hw.mem), &hw.vga.dac);
        }
        GfxEvent::Return { .. } => f.borrow_mut().leave(Mem(&hw.mem)),
    }));

    // Skip the intro (Esc), then talk to Leto.
    let mut script: Vec<(f64, Input)> = Vec::new();
    for t in [8.0, 10.5, 13.0, 15.5] {
        script.push((t, Input::Key(0x01, true)));
        script.push((t + 0.1, Input::Key(0x01, false)));
    }
    for (t, b) in [(18.0, 0), (18.2, 1), (18.4, 0)] {
        script.push((t, Input::Mouse(130, 179, b)));
    }
    let scenes = [(12.0, "palace", 0x226b_28d0_2658_bdb5_u64), (22.0, "dialogue", 0x0f1d_4cef_7f0d_a46d_u64)];

    let mut events = script.into_iter().peekable();
    let k = 4;
    let mut rgb = vec![0u8; 320 * k * 200 * k * 3];
    let mut failures = Vec::new();
    for (at, name, golden) in scenes {
        while let Some((t, _)) = events.peek() {
            if *t > at {
                break;
            }
            let (t, input) = events.next().unwrap();
            m.run_until((t * 1e9) as u64);
            match input {
                Input::Key(sc, down) => m.key(sc, down),
                Input::Mouse(x, y, b) => m.mouse_input(x, y, b),
            }
        }
        m.run_until((at * 1e9) as u64);
        let dac8 = m.hw.vga.dac.map(|c| c.map(|v| (v << 2) | (v >> 4)));
        let st = follower.borrow().comp.present(&m.hw.mem[0xa0000..0xa0000 + 64000], &dac8, &mut rgb, false);
        let hash = fnv64(&rgb);
        let fallback = st.fallback as f64 / 640.0;
        println!("{name}: {:.1}% fallback, {} dominance errors, hash {hash:#018x}", fallback, st.dominance_errors);
        if st.dominance_errors != 0 {
            failures.push(format!("{name}: {} HD blocks do not depict their pixel", st.dominance_errors));
        }
        if fallback >= 5.0 {
            failures.push(format!("{name}: {fallback:.1}% of the screen is not HD"));
        }
        if std::env::var_os("PRINT_GOLDEN").is_none() && hash != golden {
            failures.push(format!("{name}: HD image hash {hash:#018x}, expected {golden:#018x}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
