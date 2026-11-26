//! `--call-each N`: a harness that reaches code no input gets to.
//!
//! From the top of the main loop, every recompiled routine is called
//! directly, `N` times: first with the registers as they are there, then with
//! random general registers. Each call starts from the same saved state and
//! runs until it returns (to the main-loop top, pushed as its return address)
//! or for at most `call_ns` of virtual time (`--call-ms`, default 300).
//! A call still running then is not checked (its shadow check is dropped).
//!
//! The calls are meaningless to the game, but not to the checks: under
//! `--rec shadow` each one is a shadow check (the original and the Rust
//! version start from the same state, so they must end the same way), and
//! `--coverage` counts everything they execute.
//!
//! `--call-with SPECS`: the same, once per spec, for routines whose inputs
//! random registers never hit (a video number, a chapter's video): each
//! spec `ENTRY:REG=hex:[addr]=hex...` (`;` between specs) sets general
//! registers and data-segment words before its call.

use pc::{LOAD_SEG, Machine, RunExit, cpu};

#[derive(Default)]
struct Outcome {
    returned: u32,
    timed_out: u32,
    exited: u32,
    other: u32,
}

/// A call with chosen inputs (`--call-with`).
pub struct Call {
    entry: u16,
    regs: Vec<(usize, u16)>,
    /// (segment, offset, value): `[addr]` in the game's DS, `[seg:off]`
    /// anywhere (the map's cells, the code segment's words).
    words: Vec<(u16, u16, u16)>,
}

impl Call {
    /// `ENTRY:REG=hex:[addr]=hex:[seg:off]=hex...`, all hex; REG a
    /// general register, `[addr]` a word in the game's DS.
    pub fn parse(spec: &str) -> Result<Call, String> {
        let bad = || format!("--call-with ENTRY:REG=hex:[addr]=hex:[seg:off]=hex..., got {spec}");
        let hex = |s: &str| u16::from_str_radix(s, 16).map_err(|_| bad());
        // Split on the colons outside the brackets.
        let mut fields = Vec::new();
        let (mut depth, mut from) = (0, 0);
        for (i, ch) in spec.char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => depth -= 1,
                ':' if depth == 0 => {
                    fields.push(&spec[from..i]);
                    from = i + 1;
                }
                _ => {}
            }
        }
        fields.push(&spec[from..]);
        let mut parts = fields.into_iter();
        let entry = hex(parts.next().ok_or_else(bad)?)?;
        let (mut regs, mut words) = (Vec::new(), Vec::new());
        for set in parts {
            let (k, v) = set.split_once('=').ok_or_else(bad)?;
            if let Some(addr) = k.strip_prefix('[').and_then(|k| k.strip_suffix(']')) {
                let (seg, off) = match addr.split_once(':') {
                    Some((s, o)) => (hex(s)?, hex(o)?),
                    None => (pc::GAME_DS, hex(addr)?),
                };
                words.push((seg, off, hex(v)?));
            } else {
                let names = ["AX", "CX", "DX", "BX", "SP", "BP", "SI", "DI"];
                let r = names.iter().position(|n| n.eq_ignore_ascii_case(k)).ok_or_else(bad)?;
                regs.push((r, hex(v)?));
            }
        }
        Ok(Call { entry, regs, words })
    }
}

/// `--call-with`: each call once, from the main-loop top, as `run` does.
pub fn run_with(m: &mut Machine, main_loop: u16, calls: &[Call], call_ns: u64) {
    m.break_at = Some((LOAD_SEG, main_loop));
    let r = m.run_until(m.now_ns() + 10_000_000_000);
    m.break_at = None;
    if r != RunExit::Break {
        println!("call-with: the main loop was not reached ({r:?})");
        return;
    }
    let base = m.save_state();
    for c in calls {
        m.load_state(&base).expect("call-with: restore");
        if let Some(f) = m.native_restored {
            f(m);
        }
        m.exited = None;
        m.rec_drop_checks();
        for &(r, v) in &c.regs {
            m.cpu.regs[r] = v;
        }
        for &(seg, off, v) in &c.words {
            // On the bus: an owned word goes to its owner.
            let at = (seg as u32 * 16 + off as u32) & 0xf_ffff;
            m.hw.poke(at, v as u8);
            m.hw.poke(at + 1, (v >> 8) as u8);
        }
        let (ss, sp) = (m.cpu.sregs[cpu::SS], m.cpu.regs[cpu::SP].wrapping_sub(2));
        m.cpu.regs[cpu::SP] = sp;
        let a = (ss as usize * 16 + sp as usize) & 0xf_ffff;
        m.hw.mem[a..a + 2].copy_from_slice(&main_loop.to_le_bytes());
        m.cpu.ip = c.entry;
        m.rec_check_here();
        let t0 = m.now_ns();
        m.break_at = Some((LOAD_SEG, main_loop));
        let r = m.run_until(t0 + call_ns);
        m.break_at = None;
        let how = match r {
            RunExit::Break => "returned".to_string(),
            RunExit::Deadline => format!("still running after {} ms", call_ns / 1_000_000),
            other => format!("{other:?}"),
        };
        println!("call-with {:04x}: {how} ({:.1} s)", c.entry, (m.now_ns() - t0) as f64 / 1e9);
    }
    m.rec_drop_checks();
    m.load_state(&base).expect("call-with: restore");
    if let Some(f) = m.native_restored {
        f(m);
    }
    m.exited = None;
}

/// Run the harness from the machine's current state; it is left as found.
pub fn run(m: &mut Machine, main_loop: u16, entries: &[u16], samples: u32, call_ns: u64) {
    m.break_at = Some((LOAD_SEG, main_loop));
    let r = m.run_until(m.now_ns() + 10_000_000_000);
    m.break_at = None;
    if r != RunExit::Break {
        println!("call-each: the main loop was not reached ({r:?})");
        return;
    }
    let base = m.save_state();
    let mut out = Outcome::default();
    let mut rng = 0x9e37_79b9_7f4a_7c15u64;
    let started = std::time::Instant::now();
    for (i, &entry) in entries.iter().enumerate() {
        for k in 0..samples {
            m.load_state(&base).expect("call-each: restore");
            if let Some(f) = m.native_restored {
                f(m);
            }
            m.exited = None;
            m.rec_drop_checks();
            if k > 0 {
                for r in [cpu::AX, cpu::BX, cpu::CX, cpu::DX, cpu::SI, cpu::DI, cpu::BP] {
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    m.cpu.regs[r] = rng as u16;
                }
            }
            // CALL from the main-loop top: return address there.
            let (ss, sp) = (m.cpu.sregs[cpu::SS], m.cpu.regs[cpu::SP].wrapping_sub(2));
            m.cpu.regs[cpu::SP] = sp;
            let a = (ss as usize * 16 + sp as usize) & 0xf_ffff;
            m.hw.mem[a..a + 2].copy_from_slice(&main_loop.to_le_bytes());
            m.cpu.ip = entry;
            m.rec_check_here();
            m.break_at = Some((LOAD_SEG, main_loop));
            let r = m.run_until(m.now_ns() + call_ns);
            m.break_at = None;
            match r {
                RunExit::Break => out.returned += 1,
                RunExit::Deadline => out.timed_out += 1,
                RunExit::Exited(_) => out.exited += 1,
                _ => out.other += 1,
            }
        }
        if (i + 1) % 100 == 0 {
            println!("call-each: {}/{} routines, {:.0}s", i + 1, entries.len(), started.elapsed().as_secs_f64());
        }
    }
    m.rec_drop_checks();
    m.load_state(&base).expect("call-each: restore");
    if let Some(f) = m.native_restored {
        f(m);
    }
    m.exited = None;
    println!(
        "call-each: {} calls: {} returned, {} still running after {} ms, {} quit the game, {} stopped otherwise",
        entries.len() as u32 * samples,
        out.returned,
        out.timed_out,
        call_ns / 1_000_000,
        out.exited,
        out.other
    );
}
