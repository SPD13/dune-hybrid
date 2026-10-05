//! Graphics recording for the runner: a JSON-lines trace of driver calls and
//! a conformance check of `gfx::model` against the real driver.
//!
//! The check copies the target segment when a modelled call starts, applies
//! the model to the copy, and compares it with memory when the call returns.
//! A call interrupted by another driver call (the mouse handler drawing the
//! cursor) is not compared.

use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    io::{BufWriter, Write},
    path::Path,
    rc::Rc,
};

use gfx::{DrawOp, Mem, Regs, hash::sprite_hash, model, ops::Format, sprite};
use pc::gfx::{DriverCall, GfxEvent, SLOTS};

/// The low-resolution model, remembering which bytes it wrote.
struct Touch<'a> {
    dst: &'a mut [u8],
    touched: &'a mut [bool],
}

impl model::Sink for Touch<'_> {
    fn put(&mut self, at: u16, value: u8, _px: model::Px) {
        self.dst[at as usize] = value;
        self.touched[at as usize] = true;
    }
}

pub fn regs(c: &DriverCall) -> Regs {
    c.regs()
}

#[derive(Default, Clone, Copy)]
pub struct SlotStats {
    pub ok: u64,
    pub mismatch: u64,
    pub unmodelled: u64,
    pub interrupted: u64,
}

struct Pending {
    call: DriverCall,
    op: DrawOp,
    target: u16,
    touched: Option<Vec<bool>>,
    model: Vec<u8>,
    interrupted: bool,
}

pub struct Recorder {
    verify: bool,
    /// The reference HD compositor, following every call.
    pub hd: Option<gfx::compose::Follower>,
    trace: Option<BufWriter<fs::File>>,
    /// Sprite hash → (resource name, index in the sheet).
    catalog: HashMap<u64, (String, usize)>,
    last_dac: u64,

    pending: Option<Pending>,
    pub stats: [SlotStats; SLOTS],
    pub reports: Vec<String>,
}

impl Recorder {
    /// `dat`: DUNE.DAT, to name traced sprites after their sheet.
    pub fn new(verify: bool, trace: Option<&Path>, dat: &Path, hd: Option<usize>) -> Rc<RefCell<Recorder>> {
        let mut catalog = HashMap::new();
        if let (Some(_), Ok(mut f)) = (trace, fs::File::open(dat)) {
            {
                for e in gfx::dat::toc(&mut f).unwrap_or_default() {
                    let Ok(res) = gfx::dat::load(&mut f, &e) else {
                        continue;
                    };
                    for s in gfx::sheet::parse(&res).unwrap_or_default() {
                        catalog.entry(s.hash).or_insert((e.name.clone(), s.index));
                    }
                }
            }
        }
        Rc::new(RefCell::new(Recorder {
            verify,
            hd: hd.map(|k| {
                let mut f = gfx::compose::Follower::new(k);
                f.comp.watch = std::env::var("HD_WATCH").ok().and_then(|v| v.parse().ok());
                f
            }),
            trace: trace.map(|p| BufWriter::new(fs::File::create(p).unwrap())),
            catalog,
            last_dac: 0,

            pending: None,
            stats: [SlotStats::default(); SLOTS],
            reports: Vec::new(),
        }))
    }

    pub fn on_event(&mut self, e: &GfxEvent, hw: &pc::Hardware) {
        let mem = &hw.mem[..];
        match e {
            GfxEvent::Enter(c) => {
                let r = regs(c);
                let op = DrawOp::decode(c.slot, &r, Mem(mem));
                if let Some(hd) = self.hd.as_mut() {
                    hd.enter(c.slot, &r, Mem(mem), &hw.vga.dac);
                }
                if let Some(t) = self.trace.as_mut() {
                    // Palettes in use, for choosing HD art colours offline.
                    let dac = gfx::hash::fnv64(hw.vga.dac.as_flattened());
                    if dac != self.last_dac {
                        self.last_dac = dac;
                        let hex: String = hw.vga.dac.as_flattened().iter().map(|b| format!("{b:02x}")).collect();
                        let _ = writeln!(t, r#"{{"t":{},"dac":"{hex}"}}"#, c.at_ns);
                    }
                    let _ = writeln!(t, "{}", trace_line(c, &op, mem, &self.catalog));
                }
                if c.depth > 0 {
                    if let Some(p) = self.pending.as_mut() {
                        p.interrupted = true;
                    }
                    return;
                }
                if !self.verify {
                    return;
                }
                let Some(target) = op.target(&r) else {
                    // Palette, mode, retrace: state, not drawing.
                    return;
                };
                let base = (target as usize) << 4;
                let mut seg = mem[base..base + 0x10000].to_vec();
                // The game's data segment also changes under interrupts:
                // there, only the bytes the model writes are compared.
                let mut touched = (target == pc::GAME_DS).then(|| vec![false; 0x10000]);
                let modelled = match touched.as_mut() {
                    Some(t) => model::walk(&op, &r, Mem(mem), &mut Touch { dst: &mut seg, touched: t }),
                    None => model::apply(&op, &r, Mem(mem), &mut seg),
                };
                if modelled {
                    self.pending = Some(Pending { call: *c, op, target, model: seg, touched, interrupted: false });
                } else {
                    self.stats[c.slot as usize].unmodelled += 1;
                }
            }
            GfxEvent::Return { depth, .. } => {
                if let Some(hd) = self.hd.as_mut() {
                    hd.leave(Mem(mem));
                }
                if *depth != 0 {
                    return;
                }
                let Some(p) = self.pending.take() else { return };
                let s = &mut self.stats[p.call.slot as usize];
                if p.interrupted {
                    s.interrupted += 1;
                    return;
                }
                let base = (p.target as usize) << 4;
                let real = &mem[base..base + 0x10000];
                let diffs: Vec<usize> = (0..0x10000).filter(|&i| real[i] != p.model[i] && p.touched.as_ref().is_none_or(|t| t[i])).collect();
                if diffs.is_empty() {
                    s.ok += 1;
                } else {
                    s.mismatch += 1;
                    if self.reports.len() < 40 {
                        let show: Vec<String> =
                            diffs.iter().take(8).map(|&i| format!("({},{}) real {:02x} model {:02x}", i % 320, i / 320, real[i], p.model[i])).collect();
                        self.reports.push(format!(
                            "slot {} at {:.3}s: {} bytes differ; {:?}\n    regs {:x?}\n    {}",
                            p.call.slot,
                            p.call.at_ns as f64 / 1e9,
                            diffs.len(),
                            p.op,
                            regs(&p.call),
                            show.join(", ")
                        ));
                    }
                }
            }
        }
    }

    pub fn summary(&self) -> String {
        let mut out = String::new();
        for (slot, s) in self.stats.iter().enumerate() {
            if s.ok + s.mismatch + s.unmodelled + s.interrupted > 0 {
                out += &format!("  slot {slot:2}: {} ok, {} mismatch, {} unmodelled, {} interrupted\n", s.ok, s.mismatch, s.unmodelled, s.interrupted);
            }
        }
        out
    }
}

/// One trace record: the call, its decoded operation and, for sprites, the
/// sprite's identity hash.
fn trace_line(c: &DriverCall, op: &DrawOp, mem: &[u8], catalog: &HashMap<u64, (String, usize)>) -> String {
    // Which buffer is the target: the VGA screen, or the game's [DBD6] and
    // [DBDE] frame buffers or [DC32] scratch buffer. ([DBD8] normally holds
    // A000 but the game repoints it while composing off screen.)
    let var = |off: u16| Mem(mem).u16(pc::GAME_DS, off);
    let dst = match c.es {
        0xa000 => "screen",
        es if es == var(0xdbd6) => "fb1",
        es if es == var(0xdbde) => "fb2",
        es if es == var(0xdc32) => "scratch",
        _ => "other",
    };
    let base = format!(
        r#""t":{},"slot":{},"d":{},"dst":"{dst}","es":{},"ax":{},"bx":{},"cx":{},"dx":{},"si":{},"di":{},"bp":{},"ds":{},"yo":{}"#,
        c.at_ns, c.slot, c.depth, c.es, c.ax, c.bx, c.cx, c.dx, c.si, c.di, c.bp, c.ds, c.y_offset
    );
    let extra = match op {
        DrawOp::Blit { sprite: s, x, y } | DrawOp::BlitClipped { sprite: s, x, y, .. } | DrawOp::BlitScaled { sprite: s, x, y, .. } => {
            let pal = match s.format {
                Format::Nibble { pal } => pal as i32,
                Format::Byte => -2,
                Format::ByteKey => -1,
            };
            // Enough for the worst-case RLE of this size.
            let max = (sprite::row_bytes(s.format, s.wflags) * s.height as usize * 2 + 16).min(0x10000 - s.off as usize);
            let data = Mem(mem).bytes(s.seg, s.off, max);
            let hash = sprite::decode(&data, s.wflags, s.height, s.format).map(|(_, used)| sprite_hash(&data[..used], s.wflags, s.height));
            // The game's scratch buffer (DS:4C60): images it decoded itself.
            let lin = ((s.seg as usize) << 4) + s.off as usize;
            let scratch = (pc::GAME_DS as usize) * 16 + 0x4c60;
            let src = if (scratch..scratch + 64000).contains(&lin) { "scratch" } else { "res" };
            let found = hash.and_then(|h| catalog.get(&h));
            format!(
                r#","op":"sprite","src":"{src}","x":{},"y":{},"w":{},"h":{},"flags":{},"pal":{},"hash":"{}","res":"{}","part":{}"#,
                *x as i16,
                *y as i16,
                s.width(),
                s.height,
                s.wflags >> 13,
                pal,
                hash.map(|h| format!("{h:016x}")).unwrap_or_default(),
                found.map(|f| f.0.as_str()).unwrap_or(""),
                found.map(|f| f.1 as i64).unwrap_or(-1)
            )
        }
        DrawOp::Glyph { x, y, w, h, fg, bg, .. } => {
            format!(r#","op":"glyph","x":{x},"y":{y},"w":{w},"h":{h},"fg":{fg},"bg":{}"#, bg.map(|b| b as i32).unwrap_or(-1))
        }
        DrawOp::Other { .. } => r#","op":"other""#.into(),
        DrawOp::NoDraw | DrawOp::SetYOffset { .. } => r#","op":"state""#.into(),
        other => format!(r#","op":"{}""#, format!("{other:?}").split([' ', '{']).next().unwrap_or("")),
    };
    format!("{{{base}{extra}}}")
}
