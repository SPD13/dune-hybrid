//! Code coverage of the main program (seg000), for the decompilation work.
//!
//! When [`crate::Machine::coverage`] is set, every instruction the CPU starts
//! in the program's code segment ([`crate::LOAD_SEG`]) is marked, and every
//! indirect `CALL`/`JMP` (opcode FFh /2../5) records where it went. The
//! listing (`DNCDPRG.lst`) cannot resolve those targets statically; these
//! records can. `tools/coverage.py` joins a coverage file with the listing.

use std::collections::BTreeMap;

use cpu::Cpu;

use crate::{LOAD_SEG, hw::Hardware};

/// An indirect transfer: (from CS, from IP, to CS, to IP).
pub type Edge = (u16, u16, u16, u16);

pub struct Coverage {
    /// One byte per seg000 offset: 1 where an instruction started.
    pub hit: Vec<u8>,
    /// Indirect calls and jumps that start in seg000, with their counts.
    pub indirect: BTreeMap<Edge, u64>,
    /// Instructions executed outside seg000 (drivers, BIOS stubs), by segment.
    pub other: BTreeMap<u16, u64>,
    /// Instruction starts in the drivers' segments (below F000h), by segment.
    pub other_hit: BTreeMap<u16, Vec<u8>>,
    pending: Option<(u16, u16)>,
}

impl Default for Coverage {
    fn default() -> Self {
        Coverage { hit: vec![0; 0x10000], indirect: BTreeMap::new(), other: BTreeMap::new(), other_hit: BTreeMap::new(), pending: None }
    }
}

impl Coverage {
    /// Before the CPU steps: mark the instruction at CS:IP.
    #[inline]
    pub fn before(&mut self, cpu: &Cpu, hw: &Hardware) {
        let (cs, ip) = (cpu.sregs[cpu::CS], cpu.ip);
        if cs != LOAD_SEG {
            *self.other.entry(cs).or_default() += 1;
            if cs >= 0xf000 {
                return;
            }
            self.other_hit.entry(cs).or_insert_with(|| vec![0; 0x10000])[ip as usize] = 1;
        } else {
            self.hit[ip as usize] = 1;
        }
        // Skip prefixes, then look for FF /2 (call near), /3 (call far),
        // /4 (jmp near), /5 (jmp far) through a register or memory.
        let base = (cs as usize) << 4;
        let mut off = ip;
        for _ in 0..4 {
            match hw.mem[(base + off as usize) & 0xf_ffff] {
                0x26 | 0x2e | 0x36 | 0x3e | 0xf0 | 0xf2 | 0xf3 => off = off.wrapping_add(1),
                _ => break,
            }
        }
        if hw.mem[(base + off as usize) & 0xf_ffff] == 0xff {
            let reg = (hw.mem[(base + off.wrapping_add(1) as usize) & 0xf_ffff] >> 3) & 7;
            if (2..=5).contains(&reg) {
                self.pending = Some((cs, ip));
            }
        }
    }

    /// After the CPU steps: record where a pending indirect transfer went.
    #[inline]
    pub fn after(&mut self, cpu: &Cpu) {
        if let Some((cs, ip)) = self.pending.take() {
            *self.indirect.entry((cs, ip, cpu.sregs[cpu::CS], cpu.ip)).or_default() += 1;
        }
    }

    /// Instruction starts hit so far.
    pub fn count(&self) -> usize {
        self.hit.iter().filter(|&&h| h != 0).count()
    }

    /// Text format, merged by `tools/coverage.py`:
    /// `i <off>` per instruction start, `j <cs>:<ip> <cs>:<ip> <n>` per
    /// indirect transfer, `o <seg> <n>` per other segment, `h <seg>:<off>`
    /// per instruction start in a driver's segment. All hex.
    pub fn to_text(&self) -> String {
        use std::fmt::Write as _;
        let mut s = String::from("# dune-run coverage v1 (seg000 = 1000)\n");
        for (off, &h) in self.hit.iter().enumerate() {
            if h != 0 {
                let _ = writeln!(s, "i {off:04x}");
            }
        }
        for ((fc, fi, tc, ti), n) in &self.indirect {
            let _ = writeln!(s, "j {fc:04x}:{fi:04x} {tc:04x}:{ti:04x} {n}");
        }
        for (seg, n) in &self.other {
            let _ = writeln!(s, "o {seg:04x} {n}");
        }
        for (seg, hit) in &self.other_hit {
            for (off, &h) in hit.iter().enumerate() {
                if h != 0 {
                    let _ = writeln!(s, "h {seg:04x}:{off:04x}");
                }
            }
        }
        s
    }
}
