//! Exhaustive conformance of `gfx::model` with the game's own DNVGA driver:
//! every sprite of every sheet in DUNE.DAT is drawn by the real driver (slot
//! 5 at several positions and flips, slot 6 with clip rectangles, slot 35
//! scaled) and by the model, and the target buffers must be identical.
//! Needs the game files (DUNE_DIR, default ../Cryogenic/dune).
//!
//!     cargo test -p runner --release -- --ignored driver

use std::{collections::BTreeMap, fs::File, path::PathBuf};

use gfx::{DrawOp, Mem, Regs, model, ops::Format, sheet};
use pc::{
    GAME_DS, Machine,
    cpu::{AX, BP, BX, CX, DF, DI, DS, DX, ES, SI, SP, SS},
    fs::MemFs,
};

fn game_dir() -> PathBuf {
    std::env::var_os("DUNE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../Cryogenic/dune"))
}

/// Where test sprites are copied (top of conventional memory, unused by
/// the driver and the frame buffers).
const SRC_SEG: u16 = 0x9000;

struct Rig {
    m: Machine,
    drv: u16,
    fb: u16,
    sp: u16,
    /// The driver's code as loaded (it patches itself while drawing).
    code: Vec<u8>,
    pattern: Vec<u8>,
}

impl Rig {
    fn new() -> Rig {
        let dir = game_dir();
        let exe = std::fs::read(dir.join("DNCDPRG.EXE")).expect("DNCDPRG.EXE");
        let mut fs = MemFs::default();
        fs.files.insert("DUNE.DAT".into(), std::fs::read(dir.join("DUNE.DAT")).expect("DUNE.DAT"));
        let mut m = Machine::new(&exe, "ADP330 SBP2227", Box::new(fs)).unwrap();
        m.hw.audio.enabled = false;
        m.run_until(3_000_000_000);
        let rd = |m: &Machine, off: usize| u16::from_le_bytes([m.hw.mem[GAME_DS as usize * 16 + off], m.hw.mem[GAME_DS as usize * 16 + off + 1]]);
        let drv = rd(&m, 0x38b7);
        let fb = rd(&m, 0xdbd6);
        assert!(drv != 0 && fb != 0, "driver {drv:04x} frame buffer {fb:04x}");
        let sp = m.cpu.regs[SP];
        let code = m.hw.mem[(drv as usize) << 4..((drv as usize) << 4) + 0x10000].to_vec();
        let pattern = (0..0x10000).map(|i| (i as u8).wrapping_mul(7) | 1).collect();
        Rig { m, drv, fb, sp, code, pattern }
    }

    /// Run `slot` with `regs` on a patterned buffer. Ok(None) when the model
    /// does not cover the call, Err when the real driver does not return.
    fn compare(&mut self, slot: u8, r: Regs) -> Result<Option<(Vec<u8>, Vec<u8>)>, String> {
        let base = (self.fb as usize) << 4;
        self.m.hw.mem[base..base + 0x10000].copy_from_slice(&self.pattern);
        let drv_base = (self.drv as usize) << 4;
        self.m.hw.mem[drv_base..drv_base + 0x10000].copy_from_slice(&self.code);
        self.m.hw.mem[drv_base + 0x1a3..drv_base + 0x1a5].copy_from_slice(&r.y_offset.to_le_bytes());
        let op = DrawOp::decode(slot, &r, Mem(&self.m.hw.mem));
        let mut expect = self.m.hw.mem[base..base + 0x10000].to_vec();
        if !model::apply(&op, &r, Mem(&self.m.hw.mem), &mut expect) {
            return Ok(None);
        }
        let c = &mut self.m.cpu;
        c.regs[AX] = r.ax;
        c.regs[BX] = r.bx;
        c.regs[CX] = r.cx;
        c.regs[DX] = r.dx;
        c.regs[SI] = r.si;
        c.regs[DI] = r.di;
        c.regs[BP] = r.bp;
        c.regs[SP] = self.sp;
        c.sregs[DS] = r.ds;
        c.sregs[ES] = r.es;
        c.set_flag(DF, false);
        self.m.call_far(self.drv, 0x100 + 3 * slot as u16, 5_000_000)?;
        Ok(Some((self.m.hw.mem[base..base + 0x10000].to_vec(), expect)))
    }
}

#[test]
#[ignore = "needs the game files"]
fn driver_matches_model_for_every_sprite() {
    let mut rig = Rig::new();
    let mut f = File::open(game_dir().join("DUNE.DAT")).unwrap();
    let toc = gfx::dat::toc(&mut f).unwrap();
    let ss = rig.m.cpu.sregs[SS];
    let clip_bp = rig.sp.wrapping_sub(0x200);
    let (mut calls, mut skipped, mut mismatched) = (0u64, 0u64, 0u64);
    let mut hangs: Vec<String> = Vec::new();
    // (slot, format, rle, flips) -> (calls, mismatches, hangs)
    let mut by_kind: BTreeMap<String, (u64, u64, u64)> = BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    for e in &toc {
        let Ok(res) = gfx::dat::load(&mut f, e) else { continue };
        let Some(list) = sheet::parse(&res) else { continue };
        for s in list.iter().filter(|s| s.data_len > 0) {
            // Copy header + data to SRC_SEG:0000; the pixels start at 4.
            let bytes = &res[s.data - 4..s.data + s.data_len];
            let at = (SRC_SEG as usize) << 4;
            rig.m.hw.mem[at..at + bytes.len()].copy_from_slice(bytes);
            let (w, h) = (s.width(), s.height as u16);
            let base = Regs { ds: SRC_SEG, si: 4, es: rig.fb, ss, cx: h | (s.pal as u16) << 8, ..Default::default() };
            let mut cases: Vec<(u8, Regs, &str)> = Vec::new();
            for (x, y, yo) in [(0u16, 0u16, 0u16), (3, 7, 0), (130, 47, 0x1e00), (319u16.saturating_sub(w / 2), 150, 0)] {
                for flips in [0u16, 0x2000, 0x4000, 0x6000] {
                    cases.push((5, Regs { dx: x, bx: y, di: s.wflags | flips, y_offset: yo, ..base }, "blit"));
                }
            }
            if matches!(s.format(), Format::Nibble { .. }) {
                let (x, y) = (21u16, 13u16);
                for (x0, y0, x1, y1) in [(0, 0, 320, 200), (x + 3, y + 2, x + w.saturating_sub(5), y + h.saturating_sub(1)), (x + w / 2, 0, 320, 200), (0, y + h / 2, x + w / 3, 200)] {
                    let rect = [x0, y0, x1, y1];
                    for (i, v) in rect.iter().enumerate() {
                        let a = ((ss as usize) << 4) + clip_bp.wrapping_add(2 * i as u16) as usize;
                        rig.m.hw.mem[a..a + 2].copy_from_slice(&v.to_le_bytes());
                    }
                    cases.push((6, Regs { dx: x, bx: y, di: s.wflags, cx: h, bp: clip_bp, ..base }, "clipped"));
                }
                if s.wflags & 0x8000 == 0 {
                    for (step, flips) in [(0x100u16, 0x6000u16), (0x180, 0), (0xc0, 0x2000)] {
                        // A zero size makes the driver loop 65536 times; the game never asks for it.
                        let out_w = ((w as u32 * 256 / step as u32) as u16).clamp(1, 300);
                        let out_h = ((h as u32 * 256 / step as u32) as u16).clamp(1, 190);
                        cases.push((35, Regs { dx: 5, bx: 5, ax: out_w, cx: out_h | (s.pal as u16) << 8, bp: step, di: s.wflags | flips, ..base }, "scaled"));
                    }
                }
            }
            for (slot, r, what) in cases {
                calls += 1;
                let fmt = match s.format() {
                    Format::Nibble { .. } => "4bit",
                    Format::Byte => "8bit",
                    Format::ByteKey => "8bit-key",
                };
                let kind = format!("slot {slot:2} {fmt:8} {} flips {}", if s.wflags & 0x8000 != 0 { "rle" } else { "raw" }, (r.di >> 13) & 3);
                let k = by_kind.entry(kind).or_default();
                k.0 += 1;
                let (real, expect) = match rig.compare(slot, r) {
                    Ok(Some(pair)) => pair,
                    Ok(None) => {
                        skipped += 1;
                        continue;
                    }
                    Err(err) => {
                        by_kind.get_mut(&format!("slot {slot:2} {fmt:8} {} flips {}", if s.wflags & 0x8000 != 0 { "rle" } else { "raw" }, (r.di >> 13) & 3)).unwrap().2 += 1;
                        hangs.push(format!("{} #{} {what} slot {slot} pal {:02x} wflags {:04x}: {err}", e.name, s.index, s.pal, r.di));
                        continue;
                    }
                };
                if real == expect {
                    continue;
                }
                mismatched += 1;
                by_kind.get_mut(&format!("slot {slot:2} {fmt:8} {} flips {}", if s.wflags & 0x8000 != 0 { "rle" } else { "raw" }, (r.di >> 13) & 3)).unwrap().1 += 1;
                let diffs: Vec<usize> = (0..0x10000).filter(|&i| real[i] != expect[i]).collect();
                if failures.len() < 30 {
                    let i = diffs[0];
                    failures.push(format!(
                        "{} #{} {what} slot {slot} {}x{} pal {:02x} wflags {:04x}: {} bytes differ, first ({},{}) real {:02x} model {:02x}; regs {r:x?}",
                        e.name, s.index, w, h, s.pal, r.di, diffs.len(), i % 320, i / 320, real[i], expect[i]
                    ));
                }
            }
        }
    }
    for (k, (n, bad, hang)) in &by_kind {
        println!("{k}: {n} calls, {bad} mismatched, {hang} driver hangs");
    }
    println!("{calls} driver calls: {mismatched} mismatched, {skipped} not modelled, {} where the driver does not return", hangs.len());
    for h in hangs.iter().take(10) {
        println!("driver hang: {h}");
    }
    for f in &failures {
        println!("{f}");
    }
    assert_eq!(mismatched, 0, "mismatching calls (first 30 shown)");
}
