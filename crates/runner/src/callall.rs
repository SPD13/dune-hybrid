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

use pc::{LOAD_SEG, Machine, RunExit, cpu};

#[derive(Default)]
struct Outcome {
    returned: u32,
    timed_out: u32,
    exited: u32,
    other: u32,
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
