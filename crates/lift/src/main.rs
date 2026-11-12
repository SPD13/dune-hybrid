//! `dune-lift`: static recompiler from DNCDPRG.EXE's main code segment (seg000)
//! to Rust, for `crates/dune-rec` and [`pc::rec`].
//!
//!     dune-lift --exe GAME/DNCDPRG.EXE --lst DNCDPRG.lst [--cov FILE...] --out generated.rs
//!
//! Inputs, all clean-room: the executable's own bytes (decoded here, with its
//! relocations applied as the DOS loader does at segment 1000h), the
//! annotated listing for routine entries and names, and `dune-run --coverage`
//! files for what static analysis cannot know (indirect call and jump
//! targets, routines the listing decodes as data).
//!
//! Each routine becomes `fn f_XXXX(m: &mut Machine) -> Result<(), Bail>`:
//! a `match` over the routine's block addresses inside a loop, so any jump
//! shape is kept. Registers, flags, memory and the stack stay in the
//! emulated machine; flags come from the interpreter's own ALU. Common
//! instructions are written out natively; the rest (string ops,
//! multiply/divide, BCD, PUSHF/POPF, ...) run through the interpreter one
//! instruction at a time; `INT`, far calls and anything leaving seg000 go
//! through [`pc::Machine::rec_interp`] or bail. See `pc::rec` for the contract.
//!
//! A driver the game loads (DNVGA, ...) is lifted from its segment as
//! `dune-run --dump-drivers` writes it:
//!
//!     dune-lift --raw graphics.bin --seg 334b --slots 46 --name vga [--cov FILE...] --out generated_vga.rs
//!
//! Its entries are the jump table at 0100h (`--slots` of them, 3 bytes
//! each) and every near call target found from there. The generated code
//! takes its segment from CS at run time. With `--also IMAGE`, instructions
//! whose bytes differ between the images (the driver rewrites them) run
//! through the interpreter, which reads them from memory.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    process::ExitCode,
};

use iced_x86::{
    ConditionCode, Decoder, DecoderOptions, FlowControl, Formatter, Instruction, IntelFormatter, Mnemonic, OpKind, Register,
};

/// The program's load segment (matches `pc::LOAD_SEG`).
const LOAD_SEG: u16 = 0x1000;
/// seg000 length (code; the data segment follows at 0F4Bh paragraphs).
const SEG000_LEN: usize = 0xf4b0;
/// Code bytes the program rewrites at run time. C13C: the immediate of
/// `mov ax, 25h` at C13B (patched at 0AD3 and 0B24), read as it runs.
const PATCHED: &[u16] = &[0xc13c];
/// The instructions holding them.
const PATCHED_INSNS: &[u16] = &[0xc13b];

struct Args {
    exe: String,
    lst: String,
    cov: Vec<String>,
    out: String,
    only: Option<BTreeSet<u16>>,
    /// A driver's segment image instead of the executable.
    raw: Option<String>,
    /// The segment the driver ran at in the coverage files.
    seg: u16,
    slots: u16,
    name: String,
    /// More images of the same driver, from other times or runs: code bytes
    /// that differ between them are rewritten at run time.
    also: Vec<String>,
    /// More entries (code reached by interrupts: a driver's handlers).
    entries: Vec<u16>,
    /// Files of `SEG:OFF count` lines (`REC_MISSES` from `dune-run --rec
    /// on`): where Rust had no block to go on from; they become block starts.
    leaders: Vec<String>,
    /// Instructions (raw mode) known to be rewritten at run time though no
    /// `--also` image shows it (siblings of rewritten ones).
    rewritten: Vec<u16>,
}

fn parse() -> Result<Args, String> {
    let mut a = Args {
        exe: String::new(),
        lst: String::new(),
        cov: Vec::new(),
        out: String::new(),
        only: None,
        raw: None,
        seg: LOAD_SEG,
        slots: 0,
        name: String::new(),
        also: Vec::new(),
        entries: Vec::new(),
        leaders: Vec::new(),
        rewritten: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().ok_or(format!("missing value for {k}"));
        match k.as_str() {
            "--exe" => a.exe = v()?,
            "--lst" => a.lst = v()?,
            "--cov" => a.cov.push(v()?),
            "--out" => a.out = v()?,
            "--raw" => a.raw = Some(v()?),
            "--seg" => a.seg = u16::from_str_radix(&v()?, 16).map_err(|e| format!("--seg: {e}"))?,
            "--slots" => a.slots = v()?.parse().map_err(|e| format!("--slots: {e}"))?,
            "--name" => a.name = v()?,
            "--also" => a.also.push(v()?),
            "--leaders" => a.leaders.push(v()?),
            "--rewritten" => {
                for e in v()?.split(',') {
                    a.rewritten.push(u16::from_str_radix(e.trim(), 16).map_err(|e| format!("--rewritten: {e}"))?);
                }
            }
            "--entries" => {
                for e in v()?.split(',') {
                    a.entries.push(u16::from_str_radix(e.trim(), 16).map_err(|e| format!("--entries: {e}"))?);
                }
            }
            "--only" => {
                let set = v()?.split(',').map(|s| u16::from_str_radix(s.trim(), 16).map_err(|e| format!("--only: {e}"))).collect::<Result<_, _>>()?;
                a.only = Some(set);
            }
            other if !other.starts_with("--") => a.cov.push(other.to_string()),
            _ => return Err(format!("unknown option {k}")),
        }
    }
    if a.raw.is_some() {
        if a.out.is_empty() || a.slots == 0 || a.name.is_empty() {
            return Err("usage: dune-lift --raw DRIVER.bin --seg HEX --slots N --name NAME [--cov FILE]... --out FILE".into());
        }
        return Ok(a);
    }
    if a.exe.is_empty() || a.lst.is_empty() || a.out.is_empty() {
        return Err("usage: dune-lift --exe DNCDPRG.EXE --lst DNCDPRG.lst [--cov FILE]... [--only HEX,HEX] --out generated.rs".into());
    }
    Ok(a)
}

/// seg000 as it is in memory after loading at [`LOAD_SEG`].
fn load_seg000(exe: &[u8]) -> Result<Vec<u8>, String> {
    let w = |o: usize| u16::from_le_bytes([exe[o], exe[o + 1]]) as usize;
    if exe.len() < 0x1c || &exe[0..2] != b"MZ" {
        return Err("not an MZ executable".into());
    }
    let header = w(8) * 16;
    let (nrel, reltab) = (w(6), w(0x18));
    let mut image = exe[header..].to_vec();
    for i in 0..nrel {
        let (off, seg) = (w(reltab + i * 4), w(reltab + i * 4 + 2));
        let at = seg * 16 + off;
        if at + 1 < image.len() {
            let v = u16::from_le_bytes([image[at], image[at + 1]]).wrapping_add(LOAD_SEG);
            image[at..at + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
    image.truncate(SEG000_LEN);
    Ok(image)
}

/// Routine entries and names from the listing: call targets and named labels.
fn listing_entries(path: &str) -> Result<BTreeMap<u16, String>, String> {
    let text = fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let text = String::from_utf8_lossy(&text);
    let mut labels: BTreeMap<String, u16> = BTreeMap::new();
    let mut called = BTreeSet::new();
    for line in text.lines() {
        if line.starts_with("; Segment: seg001") {
            break;
        }
        let Some(rest) = line.strip_prefix("seg000:") else { continue };
        let Some((addr, body)) = rest.split_once(' ') else { continue };
        let Ok(off) = u16::from_str_radix(addr, 16) else { continue };
        if let Some(l) = body.strip_prefix(' ')
            && let Some(name) = l.strip_suffix(':').or_else(|| l.split_once(':').map(|(n, _)| n))
            && !name.is_empty()
            && !name.contains(' ')
            && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        {
            labels.insert(name.to_string(), off);
            continue;
        }
        let code = body.trim_start();
        if let Some(t) = code.strip_prefix("call") {
            let t = t.split(';').next().unwrap().trim();
            let t = t.trim_start_matches("near ptr ").trim_start_matches("far ptr ");
            if t.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !t.is_empty() {
                called.insert(t.to_string());
            }
        }
    }
    let mut entries = BTreeMap::new();
    for (name, &off) in &labels {
        if called.contains(name) || !name.starts_with("loc_") {
            entries.insert(off, name.clone());
        }
    }
    entries.entry(0).or_insert_with(|| "start".into());
    Ok(entries)
}

/// Coverage files: seg000 targets of indirect calls (new entries) and of
/// indirect jumps (block starts), by the site they came from.
struct Indirect {
    call_targets: BTreeSet<u16>,
    jump_targets: BTreeMap<u16, BTreeSet<u16>>,
    /// Instructions the code rewrites at run time (`--also` images): every
    /// form seen at that address, the lifted image's first, with its bytes.
    variants: BTreeMap<u16, Vec<(Vec<u8>, Instruction)>>,
    /// Block starts asked for (`--leaders`).
    extra_leaders: BTreeSet<u16>,
}

fn read_coverage(paths: &[String], seg: &[u8], cs: u16) -> Result<Indirect, String> {
    let mut ind = Indirect { call_targets: BTreeSet::new(), jump_targets: BTreeMap::new(), variants: BTreeMap::new(), extra_leaders: BTreeSet::new() };
    for p in paths {
        let text = fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.first() != Some(&"j") || f.len() < 3 {
                continue;
            }
            let split = |s: &str| {
                let (a, b) = s.split_once(':').unwrap_or(("0", "0"));
                (u16::from_str_radix(a, 16).unwrap_or(0), u16::from_str_radix(b, 16).unwrap_or(0))
            };
            let ((fc, fi), (tc, ti)) = (split(f[1]), split(f[2]));
            if fc != cs || tc != cs {
                continue;
            }
            let insn = decode_at(seg, fi);
            match insn.flow_control() {
                FlowControl::IndirectCall => {
                    ind.call_targets.insert(ti);
                }
                FlowControl::IndirectBranch => {
                    ind.jump_targets.entry(fi).or_default().insert(ti);
                }
                _ => {}
            }
        }
    }
    Ok(ind)
}

/// `--leaders` files: `SEG:OFF count` lines; the offsets in segment `cs`.
fn read_leaders(paths: &[String], cs: u16) -> Result<BTreeSet<u16>, String> {
    let mut set = BTreeSet::new();
    for p in paths {
        let text = fs::read_to_string(p).map_err(|e| format!("{p}: {e}"))?;
        for line in text.lines() {
            let Some((s, o)) = line.split_whitespace().next().and_then(|a| a.split_once(':')) else { continue };
            if u16::from_str_radix(s, 16) == Ok(cs)
                && let Ok(o) = u16::from_str_radix(o, 16)
            {
                set.insert(o);
            }
        }
    }
    Ok(set)
}

/// The targets of `jmp word ptr cs:[REG+TABLE]` whose index is bounded by a
/// `cmp REG2, N` just before it (and doubled by `and REG2, even` or
/// `shl`/`add` to itself, or not): the table's words, read from the image.
/// Unbounded: the words pointing at code near the table.
fn jump_table(seg: &[u8], before: &BTreeMap<u16, Instruction>, insn: &Instruction) -> Vec<u16> {
    if insn.mnemonic() != Mnemonic::Jmp
        || insn.op0_kind() != OpKind::Memory
        || insn.memory_segment() != Register::CS
        || insn.memory_index() != Register::None
        || !matches!(insn.memory_base(), Register::BX | Register::SI | Register::DI | Register::BP)
    {
        return Vec::new();
    }
    let ip = insn.ip() as u16;
    let base = insn.memory_displacement32() as usize;
    let near: Vec<&Instruction> = before.range(ip.saturating_sub(0x20)..ip).map(|(_, i)| i).collect();
    let Some(n) = near.iter().rev().find_map(|i| {
        (i.mnemonic() == Mnemonic::Cmp && i.op0_kind() == OpKind::Register && matches!(i.op1_kind(), OpKind::Immediate16 | OpKind::Immediate8to16 | OpKind::Immediate8))
            .then(|| i.immediate(1) as u32)
    }) else {
        // No bound: the words that point at code near the table (within
        // 4 KB), up to two that do not in a row.
        let mut out = Vec::new();
        let mut misses = 0;
        for k in 0..64 {
            let a = base + 2 * k;
            if a + 1 >= seg.len() {
                break;
            }
            let t = u16::from_le_bytes([seg[a], seg[a + 1]]) as usize;
            // (Not into the words read so far: code may follow the table.)
            if t >= 0x100 && t.abs_diff(base) < 0x1000 && !(base..a + 2).contains(&t) {
                out.push(t as u16);
                misses = 0;
            } else {
                misses += 1;
                if misses == 2 {
                    break;
                }
            }
        }
        return out;
    };
    let doubled = near.iter().any(|i| {
        (i.mnemonic() == Mnemonic::And && i.op1_kind() != OpKind::Register && i.immediate(1) & 1 == 0)
            || (i.mnemonic() == Mnemonic::Shl && i.op1_kind() == OpKind::Immediate8 && i.immediate(1) == 1)
            || (i.mnemonic() == Mnemonic::Add && i.op1_kind() == OpKind::Register && i.op0_register() == i.op1_register())
    });
    let words = if doubled { n.div_ceil(2) } else { n };
    if words == 0 || words > 0x80 {
        return Vec::new();
    }
    (0..words as usize)
        .filter_map(|k| {
            let a = base + 2 * k;
            (a + 1 < seg.len()).then(|| u16::from_le_bytes([seg[a], seg[a + 1]]))
        })
        .filter(|&t| t >= 0x100 && (t as usize) < seg.len())
        .collect()
}

fn decode_at(seg: &[u8], ip: u16) -> Instruction {
    let mut d = Decoder::with_ip(16, &seg[ip as usize..], ip as u64, DecoderOptions::NONE);
    d.decode()
}

/// A routine: the instructions reachable from its entry without calls.
struct Routine {
    entry: u16,
    insns: BTreeMap<u16, Instruction>,
    leaders: BTreeSet<u16>,
}

fn explore(seg: &[u8], entry: u16, ind: &Indirect) -> Routine {
    let mut r = Routine { entry, insns: BTreeMap::new(), leaders: BTreeSet::from([entry]) };
    let mut todo = vec![entry];
    while let Some(start) = todo.pop() {
        let mut ip = start;
        loop {
            if r.insns.contains_key(&ip) || ip as usize >= seg.len() {
                break;
            }
            let insn = decode_at(seg, ip);
            r.insns.insert(ip, insn);
            if insn.is_invalid() {
                break;
            }
            // A rewritten instruction ends its block; each of its forms goes
            // on at its own successors.
            if let Some(vs) = ind.variants.get(&ip) {
                for (_, v) in vs {
                    if v.is_invalid() {
                        continue;
                    }
                    let vn = v.next_ip() as u16;
                    let mut go = |t: u16, r: &mut Routine| {
                        r.leaders.insert(t);
                        todo.push(t);
                    };
                    match v.flow_control() {
                        FlowControl::UnconditionalBranch | FlowControl::ConditionalBranch
                            if v.op0_kind() == OpKind::NearBranch16 =>
                        {
                            go(v.near_branch16(), &mut r);
                            if v.flow_control() == FlowControl::ConditionalBranch {
                                go(vn, &mut r);
                            }
                        }
                        FlowControl::UnconditionalBranch
                        | FlowControl::IndirectBranch
                        | FlowControl::Return
                        | FlowControl::Exception => {}
                        _ if v.mnemonic() == Mnemonic::Hlt => {}
                        _ => go(vn, &mut r),
                    }
                }
                break;
            }
            let next = insn.next_ip() as u16;
            match insn.flow_control() {
                FlowControl::UnconditionalBranch | FlowControl::ConditionalBranch
                    if insn.op0_kind() == OpKind::NearBranch16 =>
                {
                    let t = insn.near_branch16();
                    r.leaders.insert(t);
                    todo.push(t);
                    if insn.flow_control() == FlowControl::UnconditionalBranch {
                        break;
                    }
                }
                FlowControl::UnconditionalBranch | FlowControl::IndirectBranch | FlowControl::Return => {
                    if let Some(ts) = ind.jump_targets.get(&ip) {
                        for &t in ts {
                            r.leaders.insert(t);
                            todo.push(t);
                        }
                    }
                    for t in jump_table(seg, &r.insns, &insn) {
                        r.leaders.insert(t);
                        todo.push(t);
                    }
                    break;
                }
                FlowControl::Exception => break,
                _ if insn.mnemonic() == Mnemonic::Hlt => break,
                // After a call or INT, execution resumes here: a leader, so
                // Rust can take over again there (and blocks hold no calls).
                FlowControl::Call | FlowControl::IndirectCall | FlowControl::Interrupt => {
                    r.leaders.insert(next);
                }
                // After STI a pending interrupt comes one instruction later:
                // a block start there, so `rec_tick` sees it.
                _ if insn.mnemonic() == Mnemonic::Sti => {
                    r.leaders.insert(next);
                }
                // A divide error's handler may return past the DIV (the
                // program's own skips it): a block start after it.
                _ if matches!(insn.mnemonic(), Mnemonic::Div | Mnemonic::Idiv) => {
                    r.leaders.insert(next);
                }
                // A REP string op runs as one interpreter step, however many
                // iterations: alone in its block, interrupt timing stays exact.
                _ if insn.has_rep_prefix() || insn.has_repne_prefix() => {
                    r.leaders.insert(ip);
                    r.leaders.insert(next);
                }
                _ => {}
            }
            ip = next;
        }
    }
    for &l in &ind.extra_leaders {
        if r.insns.contains_key(&l) {
            r.leaders.insert(l);
        }
    }
    r
}

// ---- code generation ----

fn reg16(r: Register) -> Option<&'static str> {
    Some(match r {
        Register::AX => "AX",
        Register::CX => "CX",
        Register::DX => "DX",
        Register::BX => "BX",
        Register::SP => "SP",
        Register::BP => "BP",
        Register::SI => "SI",
        Register::DI => "DI",
        _ => return None,
    })
}

fn reg8(r: Register) -> Option<u8> {
    Some(match r {
        Register::AL => 0,
        Register::CL => 1,
        Register::DL => 2,
        Register::BL => 3,
        Register::AH => 4,
        Register::CH => 5,
        Register::DH => 6,
        Register::BH => 7,
        _ => return None,
    })
}

fn sreg(r: Register) -> Option<&'static str> {
    Some(match r {
        Register::ES => "ES",
        Register::CS => "CS",
        Register::SS => "SS",
        Register::DS => "DS",
        _ => return None,
    })
}

/// Width of an instruction's data: byte or word.
fn width(insn: &Instruction) -> Option<bool> {
    for i in 0..insn.op_count() {
        match insn.op_kind(i) {
            OpKind::Register => {
                let r = insn.op_register(i);
                if reg8(r).is_some() {
                    return Some(false);
                }
                if reg16(r).is_some() || sreg(r).is_some() {
                    return Some(true);
                }
            }
            OpKind::Memory => match insn.memory_size().size() {
                1 => return Some(false),
                2 => return Some(true),
                _ => {}
            },
            _ => {}
        }
    }
    None
}

/// Effective address offset expression for the memory operand.
fn ea(insn: &Instruction) -> String {
    let mut parts = Vec::new();
    for r in [insn.memory_base(), insn.memory_index()] {
        if let Some(n) = reg16(r) {
            parts.push(format!("m.cpu.regs[{n}]"));
        }
    }
    let disp = insn.memory_displacement32() as u16;
    if parts.is_empty() {
        return format!("0x{disp:04x}u16");
    }
    let mut e = parts.remove(0);
    for p in parts {
        e = format!("{e}.wrapping_add({p})");
    }
    if disp != 0 {
        e = format!("{e}.wrapping_add(0x{disp:04x})");
    }
    e
}

/// Read expression for operand `i` (as u32), or None if unsupported.
fn get(insn: &Instruction, i: u32, word: bool) -> Option<String> {
    Some(match insn.op_kind(i) {
        OpKind::Register => {
            let r = insn.op_register(i);
            if let Some(n) = reg16(r) {
                format!("(m.cpu.regs[{n}] as u32)")
            } else if let Some(n) = reg8(r) {
                format!("(m.cpu.reg8({n}) as u32)")
            } else {
                format!("(m.cpu.sregs[{}] as u32)", sreg(r)?)
            }
        }
        OpKind::Memory => {
            let seg = sreg(insn.memory_segment())?;
            let f = if word { "read16" } else { "read8" };
            format!("(m.cpu.{f}(&mut m.hw, {seg}, {}) as u32)", ea(insn))
        }
        OpKind::Immediate8 | OpKind::Immediate16 | OpKind::Immediate8to16 => {
            let mask = if word { 0xffff } else { 0xff };
            format!("0x{:x}u32", insn.immediate(i) as u32 & mask)
        }
        _ => return None,
    })
}

/// Statement writing `v` (a u32 expression) to operand `i`.
fn put(insn: &Instruction, i: u32, word: bool, v: &str) -> Option<String> {
    Some(match insn.op_kind(i) {
        OpKind::Register => {
            let r = insn.op_register(i);
            if let Some(n) = reg16(r) {
                format!("m.cpu.regs[{n}] = ({v}) as u16;")
            } else if let Some(n) = reg8(r) {
                format!("m.cpu.set_reg8({n}, ({v}) as u8);")
            } else {
                format!("m.cpu.sregs[{}] = ({v}) as u16;", sreg(r)?)
            }
        }
        OpKind::Memory => {
            let seg = sreg(insn.memory_segment())?;
            let (f, t) = if word { ("write16", "u16") } else { ("write8", "u8") };
            format!("{{ let a = {}; let v = ({v}) as {t}; m.cpu.{f}(&mut m.hw, {seg}, a, v); }}", ea(insn))
        }
        _ => return None,
    })
}

fn cc(c: ConditionCode) -> u8 {
    match c {
        ConditionCode::o => 0,
        ConditionCode::no => 1,
        ConditionCode::b => 2,
        ConditionCode::ae => 3,
        ConditionCode::e => 4,
        ConditionCode::ne => 5,
        ConditionCode::be => 6,
        ConditionCode::a => 7,
        ConditionCode::s => 8,
        ConditionCode::ns => 9,
        ConditionCode::p => 10,
        ConditionCode::np => 11,
        ConditionCode::l => 12,
        ConditionCode::ge => 13,
        ConditionCode::le => 14,
        _ => 15,
    }
}

/// How an instruction was translated (for the summary).
#[derive(Default)]
struct Stats {
    /// Code bytes rewritten at run time ([`PATCHED`] for seg000).
    patched: &'static [u16],
    driver: bool,
    native: usize,
    stepped: usize,
    interp: usize,
    bail: usize,
}

/// Code for one instruction. Returns (code, ends_block).
fn lift(insn: &Instruction, st: &mut Stats) -> (String, bool) {
    let ip = insn.ip() as u16;
    let next = insn.next_ip() as u16;
    let count = "m.cpu.instructions += 1;";
    let bail = format!("m.cpu.ip = 0x{ip:04x}; return Err(Bail);");
    // Through the interpreter, one instruction; it must end at `next`.
    let step = format!(
        "m.cpu.ip = 0x{ip:04x}; m.step_insn()?; if m.cpu.ip != 0x{next:04x} || m.cpu.sregs[CS] != seg {{ return Err(Bail); }}"
    );
    let interp = format!("m.rec_interp(0x{ip:04x}, 0x{next:04x})?;");
    if insn.is_invalid() {
        st.bail += 1;
        return (bail, true);
    }
    let patched = st.patched.iter().any(|&p| p >= ip && p < next);
    if patched {
        st.stepped += 1;
        return (step, false);
    }
    let w = width(insn).unwrap_or(true);
    let wn = if w { "Width::W" } else { "Width::B" };
    let native = |code: Option<String>, st: &mut Stats| match code {
        Some(c) => {
            st.native += 1;
            (format!("{c} {count}"), false)
        }
        None => {
            st.stepped += 1;
            (step.clone(), false)
        }
    };
    use Mnemonic::*;
    let m = insn.mnemonic();
    match m {
        Mov => native(get(insn, 1, w).and_then(|v| put(insn, 0, w, &v)), st),
        Add | Or | Adc | Sbb | And | Sub | Xor | Cmp => {
            let k = match m {
                Add => 0,
                Or => 1,
                Adc => 2,
                Sbb => 3,
                And => 4,
                Sub => 5,
                Xor => 6,
                _ => 7,
            };
            let code = (|| {
                let a = get(insn, 0, w)?;
                let b = get(insn, 1, w)?;
                let p = put(insn, 0, w, "r")?;
                Some(format!("{{ let a = {a}; let b = {b}; if let Some(r) = m.cpu.alu_op({k}, {wn}, a, b) {{ {p} }} }}"))
            })();
            native(code, st)
        }
        Test => native(
            (|| Some(format!("{{ let a = {}; let b = {}; m.cpu.logic({wn}, a & b); }}", get(insn, 0, w)?, get(insn, 1, w)?)))(),
            st,
        ),
        Inc | Dec => {
            let f = if m == Inc { "inc" } else { "dec" };
            native((|| Some(format!("{{ let a = {}; let r = m.cpu.{f}({wn}, a); {} }}", get(insn, 0, w)?, put(insn, 0, w, "r")?)))(), st)
        }
        Not => native(
            (|| {
                let mask = if w { "0xffff" } else { "0xff" };
                // `put` to memory names the address `a`: compute first.
                Some(format!("{{ let a = {}; let r = !a & {mask}; {} }}", get(insn, 0, w)?, put(insn, 0, w, "r")?))
            })(),
            st,
        ),
        Neg => native(
            (|| {
                Some(format!(
                    "{{ let a = {}; let r = m.cpu.sub({wn}, 0, a, false); m.cpu.set_flag(CF, a != 0); {} }}",
                    get(insn, 0, w)?,
                    put(insn, 0, w, "r")?
                ))
            })(),
            st,
        ),
        Rol | Ror | Rcl | Rcr | Shl | Sal | Shr | Sar => {
            let k = match m {
                Rol => 0,
                Ror => 1,
                Rcl => 2,
                Rcr => 3,
                Shl => 4,
                Sal => 6,
                Shr => 5,
                _ => 7,
            };
            let code = (|| {
                let a = get(insn, 0, w)?;
                let n = match insn.op1_kind() {
                    OpKind::Register if insn.op1_register() == Register::CL => "(m.cpu.reg8(1) as u32)".to_string(),
                    OpKind::Immediate8 => format!("{}u32", insn.immediate8()),
                    _ => return None,
                };
                Some(format!("{{ let a = {a}; let n = {n} & 0x1f; let r = m.cpu.shift({k}, {wn}, a, n); {} }}", put(insn, 0, w, "r")?))
            })();
            native(code, st)
        }
        Push => native(
            (|| {
                // 286: PUSH SP pushes the value before the decrement.
                Some(format!("{{ let v = {} as u16; m.cpu.push(&mut m.hw, v); }}", get(insn, 0, true)?))
            })(),
            st,
        ),
        Pop => native(
            (|| {
                let p = put(insn, 0, true, "v")?;
                if insn.op0_kind() == OpKind::Memory {
                    // The interpreter decodes the address before popping.
                    let seg = sreg(insn.memory_segment())?;
                    Some(format!("{{ let a = {}; let v = m.cpu.pop(&mut m.hw); m.cpu.write16(&mut m.hw, {seg}, a, v); }}", ea(insn)))
                } else {
                    Some(format!("{{ let v = m.cpu.pop(&mut m.hw) as u32; {p} }}"))
                }
            })(),
            st,
        ),
        Lea => native(put(insn, 0, true, &format!("{} as u32", ea(insn))), st),
        Les | Lds => native(
            (|| {
                let r = reg16(insn.op0_register())?;
                let seg = sreg(insn.memory_segment())?;
                let s = if m == Les { "ES" } else { "DS" };
                Some(format!(
                    "{{ let a = {}; let v = m.cpu.read16(&mut m.hw, {seg}, a); let sv = m.cpu.read16(&mut m.hw, {seg}, a.wrapping_add(2)); m.cpu.regs[{r}] = v; m.cpu.sregs[{s}] = sv; }}",
                    ea(insn)
                ))
            })(),
            st,
        ),
        Xchg => native(
            (|| {
                // As the interpreter: a = r/m, b = reg; r/m = b; reg = a.
                let (rm, reg) = if insn.op0_kind() == OpKind::Memory { (0, 1) } else { (1, 0) };
                Some(format!(
                    "{{ let a = {}; let b = {}; {} {} }}",
                    get(insn, rm, w)?,
                    get(insn, reg, w)?,
                    put(insn, rm, w, "b")?,
                    put(insn, reg, w, "a")?
                ))
            })(),
            st,
        ),
        Cbw => native(Some("m.cpu.regs[AX] = m.cpu.reg8(0) as i8 as u16;".into()), st),
        Cwd => native(Some("m.cpu.regs[DX] = if m.cpu.regs[AX] & 0x8000 != 0 { 0xffff } else { 0 };".into()), st),
        Clc => native(Some("m.cpu.set_flag(CF, false);".into()), st),
        Stc => native(Some("m.cpu.set_flag(CF, true);".into()), st),
        Cmc => native(Some("{ let c = m.cpu.flag(CF); m.cpu.set_flag(CF, !c); }".into()), st),
        Cld => native(Some("m.cpu.set_flag(DF, false);".into()), st),
        Std => native(Some("m.cpu.set_flag(DF, true);".into()), st),
        Nop => native(Some(String::new()), st),
        In | Out => {
            let code = (|| {
                let (port_op, data_op) = if m == In { (1, 0) } else { (0, 1) };
                let port = match insn.op_kind(port_op) {
                    OpKind::Immediate8 => format!("0x{:x}u16", insn.immediate8()),
                    OpKind::Register if insn.op_register(port_op) == Register::DX => "m.cpu.regs[DX]".into(),
                    _ => return None,
                };
                let word = insn.op_register(data_op) == Register::AX;
                Some(match (m, word) {
                    // Devices see the time of this instruction (the PIT counter).
                    (In, false) => format!("{{ m.hw.now_ns = m.now_ns(); let v = m.hw.in8({port}); m.cpu.set_reg8(0, v); }}"),
                    (In, true) => format!("{{ m.hw.now_ns = m.now_ns(); let v = m.hw.in16({port}); m.cpu.regs[AX] = v; }}"),
                    (_, false) => format!("{{ m.hw.now_ns = m.now_ns(); let p = {port}; let v = m.cpu.reg8(0); m.hw.out8(p, v); }}"),
                    (_, true) => format!("{{ m.hw.now_ns = m.now_ns(); let p = {port}; let v = m.cpu.regs[AX]; m.hw.out16(p, v); }}"),
                })
            })();
            native(code, st)
        }
        Jmp if insn.op0_kind() == OpKind::NearBranch16 => {
            st.native += 1;
            (format!("{count} pc = 0x{:04x}; continue;", insn.near_branch16()), true)
        }
        Jmp if matches!(insn.op0_kind(), OpKind::Register | OpKind::Memory) && insn.memory_size().size() != 4 => {
            st.native += 1;
            let t = get(insn, 0, true).unwrap_or_default();
            (format!("{count} pc = {t} as u16; continue;"), true)
        }
        _ if insn.flow_control() == FlowControl::ConditionalBranch && insn.op0_kind() == OpKind::NearBranch16 => {
            st.native += 1;
            let t = insn.near_branch16();
            let c = match m {
                Jcxz => "m.cpu.regs[CX] == 0".to_string(),
                Loop => "{ m.cpu.regs[CX] = m.cpu.regs[CX].wrapping_sub(1); m.cpu.regs[CX] != 0 }".into(),
                Loope => "{ m.cpu.regs[CX] = m.cpu.regs[CX].wrapping_sub(1); m.cpu.regs[CX] != 0 && m.cpu.flag(ZF) }".into(),
                Loopne => "{ m.cpu.regs[CX] = m.cpu.regs[CX].wrapping_sub(1); m.cpu.regs[CX] != 0 && !m.cpu.flag(ZF) }".into(),
                _ => format!("m.cpu.cond({})", cc(insn.condition_code())),
            };
            (format!("{count} if {c} {{ pc = 0x{t:04x}; continue; }}"), false)
        }
        Call if insn.op0_kind() == OpKind::NearBranch16 => {
            st.native += 1;
            let t = insn.near_branch16();
            (format!("{count} m.cpu.push(&mut m.hw, 0x{next:04x}); m.rec_call(0x{t:04x}, 0x{next:04x})?;"), false)
        }
        Call if matches!(insn.op0_kind(), OpKind::Register | OpKind::Memory) && insn.memory_size().size() != 4 => {
            st.native += 1;
            let t = get(insn, 0, true).unwrap_or_default();
            (format!("{count} {{ let t = {t} as u16; m.cpu.push(&mut m.hw, 0x{next:04x}); m.rec_call(t, 0x{next:04x})?; }}"), false)
        }
        Ret => {
            st.native += 1;
            let n = if insn.op_count() > 0 { insn.immediate16() } else { 0 };
            (format!("{count} m.cpu.ip = m.cpu.pop(&mut m.hw); m.cpu.regs[SP] = m.cpu.regs[SP].wrapping_add(0x{n:x}); return Ok(());"), true)
        }
        // String ops and MUL/DIV: the interpreter's own operations, without
        // its fetch and decode. (A REP string op is a block of its own.)
        Movsb | Movsw | Cmpsb | Cmpsw | Stosb | Stosw | Lodsb | Lodsw | Scasb | Scasw => {
            st.native += 1;
            let op = match m {
                Movsb => 0xa4,
                Movsw => 0xa5,
                Cmpsb => 0xa6,
                Cmpsw => 0xa7,
                Stosb => 0xaa,
                Stosw => 0xab,
                Lodsb => 0xac,
                Lodsw => 0xad,
                Scasb => 0xae,
                _ => 0xaf,
            };
            let seg = sreg(insn.segment_prefix()).map_or("None".to_string(), |s| format!("Some({s})"));
            let rep = if insn.has_repne_prefix() {
                0xf2
            } else if insn.has_rep_prefix() || insn.has_repe_prefix() {
                0xf3
            } else {
                0
            };
            (format!("{count} m.cpu.string_op(&mut m.hw, 0x{op:02x}, {seg}, 0x{rep:02x});"), false)
        }
        // IMUL r16, r/m16, imm: as the interpreter (CF = OF = the product
        // does not fit in 16 bits; the other flags unchanged).
        Imul if insn.op_count() == 3 => {
            let code = (|| {
                let a = get(insn, 1, true)?;
                let b = match insn.op2_kind() {
                    OpKind::Immediate16 => insn.immediate16() as i16 as i32,
                    OpKind::Immediate8to16 => insn.immediate8to16() as i32,
                    _ => return None,
                };
                let r = put(insn, 0, true, "(r as u16 as u32)")?;
                Some(format!(
                    "{{ let r = ({a} as u16 as i16 as i32) * ({b}); {r} let over = r != (r as i16) as i32; m.cpu.set_flag(CF, over); m.cpu.set_flag(pc::cpu::OF, over); }}"
                ))
            })();
            native(code, st)
        }
        // AAM base (not 0: that raises #DE, left to the interpreter).
        Aam if insn.immediate8() != 0 => {
            let b = insn.immediate8();
            native(
                Some(format!(
                    "{{ let al = m.cpu.reg8(0); m.cpu.set_reg8(4, al / {b}); m.cpu.set_reg8(0, al % {b}); m.cpu.set_szp(Width::B, (al % {b}) as u32); }}"
                )),
                st,
            )
        }
        // HLT: the CPU waits for an interrupt; the run loop sleeps.
        Hlt => {
            st.native += 1;
            (format!("{count} m.cpu.ip = 0x{next:04x}; m.cpu.halted = true; return Err(Bail);"), true)
        }
        Mul | Imul | Div | Idiv if insn.op_count() == 1 => {
            let sub = match m {
                Mul => 4,
                Imul => 5,
                Div => 6,
                _ => 7,
            };
            match get(insn, 0, w) {
                Some(a) => {
                    st.native += 1;
                    // A divide error leaves CS:IP at the handler: bail there.
                    (
                        format!(
                            "{count} {{ let a = {a}; m.cpu.insn_cs = seg; m.cpu.insn_ip = 0x{ip:04x}; m.cpu.ip = 0x{next:04x}; if m.cpu.mul_div(&mut m.hw, {wn}, {sub}, a) {{ return Err(Bail); }} }}"
                        ),
                        false,
                    )
                }
                None => {
                    st.stepped += 1;
                    (step, false)
                }
            }
        }
        Xlatb => {
            let seg = sreg(insn.segment_prefix()).unwrap_or("DS");
            native(
                Some(format!(
                    "{{ let off = m.cpu.regs[BX].wrapping_add(m.cpu.reg8(0) as u16); let v = m.cpu.read8(&mut m.hw, {seg}, off); m.cpu.set_reg8(0, v); }}"
                )),
                st,
            )
        }
        Lahf => native(Some("{ let f = m.cpu.flags() as u8; m.cpu.set_reg8(4, f); }".into()), st),
        Sahf => native(Some("{ let ah = m.cpu.reg8(4) as u16; let f = (m.cpu.flags() & 0xff00) | (ah & 0xd5); m.cpu.set_flags(f); }".into()), st),
        Pushf => native(Some("{ let f = m.cpu.flags(); m.cpu.push(&mut m.hw, f); }".into()), st),
        Popf => native(Some("{ let f = m.cpu.pop(&mut m.hw); m.cpu.set_flags(f); }".into()), st),
        Cli => native(Some("m.cpu.set_flag(IF, false);".into()), st),
        // Interrupts on after the next instruction: `rec_tick` keeps the
        // hold to the next block's first instruction.
        Sti => native(Some("m.cpu.set_flag(IF, true); m.cpu.irq_inhibit = true;".into()), st),
        // Control that leaves the routine's segment or frame: done here, then
        // the run loop (or the caller) goes on from the new CS:IP.
        Iret => {
            st.native += 1;
            (
                format!("{count} m.cpu.ip = m.cpu.pop(&mut m.hw); m.cpu.sregs[CS] = m.cpu.pop(&mut m.hw); {{ let f = m.cpu.pop(&mut m.hw); m.cpu.set_flags(f); }} return Ok(());"),
                true,
            )
        }
        Retf => {
            st.native += 1;
            let n = if insn.op_count() > 0 { insn.immediate16() } else { 0 };
            (
                format!("{count} m.cpu.ip = m.cpu.pop(&mut m.hw); m.cpu.sregs[CS] = m.cpu.pop(&mut m.hw); m.cpu.regs[SP] = m.cpu.regs[SP].wrapping_add(0x{n:x}); return Ok(());"),
                true,
            )
        }
        // The target is read from the code as it runs: the program patches
        // these (EFDCh and F04Dh chain to the interrupt handlers it replaced).
        Jmp if insn.op0_kind() == OpKind::FarBranch16 => {
            st.native += 1;
            let at = next.wrapping_sub(4);
            (
                format!(
                    "{count} {{ let o = m.cpu.read16(&mut m.hw, CS, 0x{at:04x}); let s = m.cpu.read16(&mut m.hw, CS, 0x{:04x}); m.cpu.ip = o; m.cpu.sregs[CS] = s; }} return Ok(());",
                    at.wrapping_add(2)
                ),
                true,
            )
        }
        Jmp if insn.op0_kind() == OpKind::Memory && sreg(insn.memory_segment()).is_some() => {
            st.native += 1;
            let seg = sreg(insn.memory_segment()).unwrap();
            (
                format!(
                    "{count} {{ let a = {}; let o = m.cpu.read16(&mut m.hw, {seg}, a); let s = m.cpu.read16(&mut m.hw, {seg}, a.wrapping_add(2)); m.cpu.ip = o; m.cpu.sregs[CS] = s; }} return Ok(());",
                    ea(insn)
                ),
                true,
            )
        }
        Jmp => {
            st.bail += 1;
            (bail, true)
        }
        Int | Int3 | Into | Call => {
            st.interp += 1;
            (interp, false)
        }
        _ => {
            st.stepped += 1;
            (step, false)
        }
    }
}

/// Instructions in the block starting at `leader` (up to the next leader or
/// the end of the block).
fn block_len(r: &Routine, leader: u16) -> u32 {
    let mut n = 0;
    let mut ip = leader;
    while let Some(insn) = r.insns.get(&ip) {
        n += 1;
        let ends = matches!(
            insn.flow_control(),
            FlowControl::UnconditionalBranch | FlowControl::IndirectBranch | FlowControl::Return | FlowControl::Exception
        ) || insn.mnemonic() == Mnemonic::Hlt
            || insn.is_invalid();
        ip = insn.next_ip() as u16;
        if ends || r.leaders.contains(&ip) {
            break;
        }
    }
    n.max(1)
}

fn emit_routine(out: &mut String, r: &Routine, name: &str, st: &mut Stats, ind: &Indirect) {
    let mut fmt = IntelFormatter::new();
    fmt.options_mut().set_hex_prefix("0x");
    fmt.options_mut().set_hex_suffix("");
    let _ = writeln!(out, "/// `{name}` ({}:{:04x})", if st.driver { "driver" } else { "seg000" }, r.entry);
    let _ = writeln!(out, "pub fn f_{:04x}(m: &mut Machine, start: u16) -> Result<(), Bail> {{", r.entry);
    let _ = writeln!(out, "    let seg = m.cpu.sregs[CS];");
    let _ = writeln!(out, "    let mut pc: u16 = start;");
    let _ = writeln!(out, "    loop {{");
    let _ = writeln!(out, "        match pc {{");
    for &leader in &r.leaders {
        if !r.insns.contains_key(&leader) {
            continue;
        }
        let _ = writeln!(out, "            0x{leader:04x} => {{");
        let _ = writeln!(out, "                if m.rec_tick({}) {{ m.cpu.ip = 0x{leader:04x}; return Err(Bail); }}", block_len(r, leader));
        let mut ip = leader;
        loop {
            let Some(insn) = r.insns.get(&ip) else {
                // Falls into code this routine did not decode: let the interpreter go on.
                let _ = writeln!(out, "                m.cpu.ip = 0x{ip:04x}; return Err(Bail);");
                break;
            };
            let mut text = String::new();
            fmt.format(insn, &mut text);
            if let Some(vs) = ind.variants.get(&ip) {
                // Rewritten at run time: the form in memory now, natively
                // if it is one seen, else through the interpreter.
                let _ = writeln!(out, "                // {ip:04x}: {text} (rewritten: {} forms)", vs.len());
                let _ = writeln!(
                    out,
                    "                let code = |i: u16| m.hw.mem[(((seg as usize) << 4) + 0x{ip:04x}usize.wrapping_add(i as usize)) & 0xf_ffff];"
                );
                let patched = std::mem::take(&mut st.patched);
                let vs_all = vs;
                let mut vs: Vec<&(Vec<u8>, Instruction)> = vs.iter().collect();
                vs.sort_by_key(|(b, _)| std::cmp::Reverse(b.len()));
                for (bytes, v) in vs {
                    if v.is_invalid() {
                        continue;
                    }
                    let cond: Vec<String> = bytes.iter().enumerate().map(|(i, b)| format!("code({i}) == 0x{b:02x}")).collect();
                    let mut vt = String::new();
                    fmt.format(v, &mut vt);
                    let (code, ends) = lift(v, st);
                    let _ = writeln!(out, "                if {} {{", cond.join(" && "));
                    let _ = writeln!(out, "                    // {vt}");
                    let _ = writeln!(out, "                    {code}");
                    if !ends {
                        let _ = writeln!(out, "                    pc = 0x{:04x}; continue;", v.next_ip() as u16);
                    }
                    let _ = writeln!(out, "                }}");
                }
                // Forms that differ only in their immediate (a constant the
                // code writes into itself): any value, read as it runs.
                if let Some(code) = immediate_form(&vs_all, st) {
                    let k = vs_all[0].1.len() - imm_size(&vs_all[0].1);
                    let cond: Vec<String> = vs_all[0].0[..k].iter().enumerate().map(|(i, b)| format!("code({i}) == 0x{b:02x}")).collect();
                    let _ = writeln!(out, "                if {} {{", cond.join(" && "));
                    let _ = writeln!(out, "                    {code}");
                    let _ = writeln!(out, "                    pc = 0x{:04x}; continue;", vs_all[0].1.next_ip() as u16);
                    let _ = writeln!(out, "                }}");
                }
                // Branches whose forms differ only in their displacement: any
                // target, computed as it runs (Rust goes on there from any
                // block start).
                if let Some((code, k, ends)) = branch_form(&vs_all, st) {
                    let cond: Vec<String> = vs_all[0].0[..k].iter().enumerate().map(|(i, b)| format!("code({i}) == 0x{b:02x}")).collect();
                    let next = vs_all[0].1.next_ip() as u16;
                    let disp = if vs_all[0].1.len() - k == 1 {
                        format!("(code({k}) as i8 as i16 as u16)")
                    } else {
                        format!("(code({k}) as u16 | (code({}) as u16) << 8)", k + 1)
                    };
                    let _ = writeln!(out, "                if {} {{", cond.join(" && "));
                    let _ = writeln!(out, "                    let t: u16 = 0x{next:04x}u16.wrapping_add({disp});");
                    let _ = writeln!(out, "                    {code}");
                    if !ends {
                        let _ = writeln!(out, "                    pc = 0x{next:04x}; continue;");
                    }
                    let _ = writeln!(out, "                }}");
                }
                st.patched = patched;
                st.stepped += 1;
                let _ = writeln!(
                    out,
                    "                m.cpu.ip = 0x{ip:04x}; m.step_insn()?; if m.cpu.sregs[CS] != seg {{ return Err(Bail); }} pc = m.cpu.ip; continue;"
                );
                break;
            }
            let (code, ends) = lift(insn, st);
            let _ = writeln!(out, "                // {ip:04x}: {text}");
            let _ = writeln!(out, "                {code}");
            if ends {
                break;
            }
            ip = insn.next_ip() as u16;
            if r.leaders.contains(&ip) {
                let _ = writeln!(out, "                pc = 0x{ip:04x}; continue;");
                break;
            }
        }
        let _ = writeln!(out, "            }}");
    }
    let _ = writeln!(out, "            _ => {{ m.cpu.ip = pc; return Err(Bail); }}");
    let _ = writeln!(out, "        }}");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "}}\n");
}

/// Size of an instruction's immediate (0: none).
fn imm_size(insn: &Instruction) -> usize {
    (0..insn.op_count())
        .map(|i| match insn.op_kind(i) {
            OpKind::Immediate8 | OpKind::Immediate8to16 => 1,
            OpKind::Immediate16 => 2,
            _ => 0,
        })
        .sum()
}

/// For a rewritten instruction whose forms differ only in their trailing
/// immediate: its code with the immediate read from memory as it runs
/// (`code(i)`, the instruction's bytes). None if the forms differ
/// otherwise, or it does not lift natively.
fn immediate_form(vs: &[(Vec<u8>, Instruction)], st: &mut Stats) -> Option<String> {
    let (b0, i0) = &vs[0];
    let size = imm_size(i0);
    let k = i0.len().checked_sub(size)?;
    if size == 0 || (0..i0.op_count()).filter(|&i| matches!(i0.op_kind(i), OpKind::Immediate8 | OpKind::Immediate8to16 | OpKind::Immediate16)).count() != 1 {
        return None;
    }
    if !vs.iter().all(|(b, v)| v.code() == i0.code() && v.len() == i0.len() && b[..k] == b0[..k]) {
        return None;
    }
    let mut p = *i0;
    let (token, read) = match (0..p.op_count()).map(|i| p.op_kind(i)).find(|k| matches!(k, OpKind::Immediate8 | OpKind::Immediate8to16 | OpKind::Immediate16))? {
        OpKind::Immediate16 => {
            p.set_immediate16(0x5a5a);
            ("0x5a5au32", format!("(code({k}) as u32 | (code({}) as u32) << 8)", k + 1))
        }
        OpKind::Immediate8to16 => {
            p.set_immediate8to16(0x5b);
            ("0x5bu32", format!("(code({k}) as i8 as i16 as u16 as u32)"))
        }
        _ => {
            p.set_immediate8(0x5b);
            ("0x5bu32", format!("(code({k}) as u32)"))
        }
    };
    let (n, s) = (st.native, st.stepped);
    let (code, ends) = lift(&p, st);
    (st.native, st.stepped) = (n, s);
    (!ends && code.matches(token).count() == 1 && !code.contains("step_insn")).then(|| code.replace(token, &read))
}

/// For a rewritten near branch (JMP, Jcc, LOOP) whose forms differ only
/// in the displacement: its code with the target `t` (computed by the
/// caller), the number of bytes before the displacement, and whether the
/// code ends the block.
fn branch_form(vs: &[(Vec<u8>, Instruction)], st: &mut Stats) -> Option<(String, usize, bool)> {
    let (b0, i0) = &vs[0];
    if i0.op0_kind() != OpKind::NearBranch16 || i0.mnemonic() == Mnemonic::Call {
        return None;
    }
    let size = match i0.len() {
        2 => 1,
        3 if b0[0] == 0xe9 => 2,
        _ => return None,
    };
    let k = i0.len() - size;
    if !vs.iter().all(|(b, v)| v.code() == i0.code() && v.len() == i0.len() && b[..k] == b0[..k]) {
        return None;
    }
    let mut p = *i0;
    p.set_near_branch16(0x5a5a);
    let (n, s) = (st.native, st.stepped);
    let (code, ends) = lift(&p, st);
    (st.native, st.stepped) = (n, s);
    (code.matches("0x5a5a").count() == 1 && !code.contains("step_insn")).then(|| (code.replace("0x5a5a", "t"), k, ends))
}

/// A store into the code segment at a fixed address (`mov cs:[A], src`):
/// the address, its size, and the bytes stored if they are an immediate.
fn cs_store(insn: &Instruction) -> Option<(u16, usize, Option<Vec<u8>>)> {
    if insn.mnemonic() != Mnemonic::Mov
        || insn.op_count() != 2
        || insn.op0_kind() != OpKind::Memory
        || insn.memory_segment() != Register::CS
        || insn.memory_base() != Register::None
        || insn.memory_index() != Register::None
    {
        return None;
    }
    let addr = insn.memory_displacement32() as u16;
    let size = insn.memory_size().size();
    let val = match insn.op1_kind() {
        OpKind::Immediate8 => Some(vec![insn.immediate8()]),
        OpKind::Immediate16 => Some(insn.immediate16().to_le_bytes().to_vec()),
        _ => None,
    };
    Some((addr, size, val))
}

/// Instructions the code itself rewrites with `mov cs:[A], ...` (found
/// statically, so forms no dump caught are lifted too): an immediate stored
/// gives the instruction's form with it; a register stored only marks it
/// rewritten (its immediate or displacement is then read as it runs).
/// Returns whether anything new was added.
fn static_rewrites(seg: &[u8], all: &BTreeMap<u16, Instruction>, ind: &mut Indirect, patched: &mut Vec<u16>) -> bool {
    let mut more = false;
    for (&sip, s) in all {
        let Some((addr, size, val)) = cs_store(s) else { continue };
        let Some((&tip, t)) = all.range(..=addr).next_back().filter(|(_, t)| t.next_ip() as u16 > addr) else { continue };
        if tip == sip || addr as usize + size > t.next_ip() as usize {
            continue;
        }
        let n = t.len();
        let orig = (seg[tip as usize..tip as usize + n].to_vec(), *t);
        if !ind.variants.contains_key(&tip) {
            ind.variants.insert(tip, vec![orig]);
            more = true;
        }
        for b in tip..t.next_ip() as u16 {
            if !patched.contains(&b) {
                patched.push(b);
            }
        }
        let Some(val) = val else { continue };
        let k = (addr - tip) as usize;
        let forms: Vec<Vec<u8>> = ind.variants[&tip].iter().map(|(b, _)| b.clone()).collect();
        for b in forms {
            if b.len() < k + size {
                continue;
            }
            let mut buf = seg.to_vec();
            buf[tip as usize..tip as usize + b.len()].copy_from_slice(&b);
            buf[addr as usize..addr as usize + size].copy_from_slice(&val);
            let v = decode_at(&buf, tip);
            let vb = buf[tip as usize..v.next_ip() as usize].to_vec();
            let vs = ind.variants.get_mut(&tip).unwrap();
            if !vs.iter().any(|(x, _)| *x == vb) {
                vs.push((vb, v));
                more = true;
            }
        }
    }
    more
}

/// FNV-1a over the bytes of `ranges` (the same as `pc::rec`'s check).
fn code_hash(seg: &[u8], ranges: &[(u16, u16)]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &(s, e) in ranges {
        for &b in &seg[s as usize..e as usize] {
            h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

fn main() -> ExitCode {
    let a = match parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let run = || -> Result<(), String> {
        let raw_patched: &'static [u16];
        let (seg, mut entries, ind, source) = if let Some(raw) = &a.raw {
            let seg = fs::read(raw).map_err(|e| format!("{raw}: {e}"))?;
            if seg.len() != 0x10000 {
                return Err(format!("{raw}: expected a 64 KB segment image"));
            }
            let mut ind = read_coverage(&a.cov, &seg, a.seg)?;
            ind.extra_leaders = read_leaders(&a.leaders, a.seg)?;
            let mut entries: BTreeMap<u16, String> =
                (0..a.slots).map(|i| (0x100 + 3 * i, format!("{}_slot_{i:02}", a.name))).collect();
            for &t in ind.call_targets.iter().chain(&a.entries) {
                entries.entry(t).or_insert_with(|| format!("{}_{t:04x}", a.name));
            }
            // Every near call target reachable from the entries.
            let mut todo: Vec<u16> = entries.keys().copied().collect();
            while let Some(e) = todo.pop() {
                for insn in explore(&seg, e, &ind).insns.values() {
                    if insn.mnemonic() == Mnemonic::Call && insn.op0_kind() == OpKind::NearBranch16 {
                        let t = insn.near_branch16();
                        if !entries.contains_key(&t) {
                            entries.insert(t, format!("{}_{t:04x}", a.name));
                            todo.push(t);
                        }
                    }
                }
            }
            // Instructions that differ in another image are rewritten at run
            // time (a field may differ in only one byte: the whole
            // instruction). Their forms are followed while exploring, which
            // can reach more code: repeat until nothing new.
            let imgs: Vec<Vec<u8>> = a.also.iter().map(|o| fs::read(o).map_err(|e| format!("{o}: {e}"))).collect::<Result<_, _>>()?;
            let mut ind = ind;
            let mut patched = Vec::new();
            loop {
                let mut more = false;
                for &e in entries.keys().chain(&ind.extra_leaders.clone()) {
                    let r = explore(&seg, e, &ind);
                    for (&ip, insn) in &r.insns {
                        let n = insn.len();
                        for img in &imgs {
                            let i = ip as usize;
                            if img[i..i + n] == seg[i..i + n] {
                                continue;
                            }
                            for b in ip..insn.next_ip() as u16 {
                                if !patched.contains(&b) {
                                    patched.push(b);
                                }
                            }
                            let i0 = ind.variants.len();
                            ind.variants.entry(ip).or_insert_with(|| vec![(seg[i..i + n].to_vec(), *insn)]);
                            more |= ind.variants.len() != i0;
                            // The image's own instruction there, unless its
                            // code before runs into it (another boundary).
                            let pred = r.insns.range(..ip).next_back().filter(|(_, p)| p.next_ip() as u16 == ip);
                            if let Some((&p, _)) = pred
                                && decode_at(img, p).next_ip() as u16 != ip
                            {
                                continue;
                            }
                            let v = decode_at(img, ip);
                            let vb = img[i..i + v.len()].to_vec();
                            let vs = ind.variants.entry(ip).or_insert_with(|| vec![(seg[i..i + n].to_vec(), *insn)]);
                            if !vs.iter().any(|(b, _)| *b == vb) {
                                vs.push((vb, v));
                                more = true;
                            }
                        }
                    }
                }
                let mut all = BTreeMap::new();
                for &e in entries.keys().chain(&ind.extra_leaders.clone()) {
                    all.extend(explore(&seg, e, &ind).insns);
                }
                more |= static_rewrites(&seg, &all, &mut ind, &mut patched);
                // Immediates 1234h are placeholders the driver patches in
                // through computed addresses: read as it runs.
                for (&ip, insn) in &all {
                    let imm = (0..insn.op_count()).any(|i| insn.op_kind(i) == OpKind::Immediate16 && insn.immediate16() == 0x1234);
                    if imm && !ind.variants.contains_key(&ip) {
                        ind.variants.insert(ip, vec![(seg[ip as usize..insn.next_ip() as usize].to_vec(), *insn)]);
                        for b in ip..insn.next_ip() as u16 {
                            if !patched.contains(&b) {
                                patched.push(b);
                            }
                        }
                        more = true;
                    }
                }
                if !more {
                    break;
                }
            }
            for &ip in &a.rewritten {
                let insn = decode_at(&seg, ip);
                ind.variants.entry(ip).or_insert_with(|| vec![(seg[ip as usize..insn.next_ip() as usize].to_vec(), insn)]);
                for b in ip..insn.next_ip() as u16 {
                    if !patched.contains(&b) {
                        patched.push(b);
                    }
                }
            }
            patched.sort_unstable();
            println!("rewritten instructions: {}", ind.variants.iter().map(|(ip, v)| format!("{ip:04x}x{}", v.len())).collect::<Vec<_>>().join(" "));
            println!("rewritten code bytes: {}", patched.iter().map(|b| format!("{b:04x}")).collect::<Vec<_>>().join(" "));
            raw_patched = Box::leak(patched.into_boxed_slice());
            (seg, entries, ind, raw.clone())
        } else {
            let exe = fs::read(&a.exe).map_err(|e| format!("{}: {e}", a.exe))?;
            let seg = load_seg000(&exe)?;
            let mut entries = listing_entries(&a.lst)?;
            let mut ind = read_coverage(&a.cov, &seg, LOAD_SEG)?;
            ind.extra_leaders = read_leaders(&a.leaders, LOAD_SEG)?;
            for &ip in PATCHED_INSNS {
                let insn = decode_at(&seg, ip);
                ind.variants.insert(ip, vec![(seg[ip as usize..insn.next_ip() as usize].to_vec(), insn)]);
            }
            for &t in &ind.call_targets {
                entries.entry(t).or_insert_with(|| format!("sub_{t:04x}"));
            }
            // Code the program rewrites with `mov cs:[A], ...`.
            let mut patched: Vec<u16> = PATCHED.to_vec();
            loop {
                let mut all = BTreeMap::new();
                for &e in entries.keys().chain(&ind.extra_leaders.clone()) {
                    all.extend(explore(&seg, e, &ind).insns);
                }
                if !static_rewrites(&seg, &all, &mut ind, &mut patched) {
                    break;
                }
            }
            patched.sort_unstable();
            println!("rewritten instructions: {}", ind.variants.iter().map(|(ip, v)| format!("{ip:04x}x{}", v.len())).collect::<Vec<_>>().join(" "));
            raw_patched = Box::leak(patched.into_boxed_slice());
            (seg, entries, ind, "DNCDPRG.EXE".to_string())
        };
        // A block start asked for in code no routine reaches: a routine of
        // its own from there.
        if !ind.extra_leaders.is_empty() {
            let mut covered = BTreeSet::new();
            for &e in entries.keys() {
                covered.extend(explore(&seg, e, &ind).insns.into_keys());
            }
            let name = if a.raw.is_some() { a.name.clone() } else { "resume".into() };
            let mut todo = Vec::new();
            for &l in &ind.extra_leaders {
                if !covered.contains(&l) {
                    entries.insert(l, format!("{name}_from_{l:04x}"));
                    covered.extend(explore(&seg, l, &ind).insns.into_keys());
                    todo.push(l);
                }
            }
            // And what they call.
            while let Some(e) = todo.pop() {
                for insn in explore(&seg, e, &ind).insns.values() {
                    if insn.mnemonic() == Mnemonic::Call && insn.op0_kind() == OpKind::NearBranch16 {
                        let t = insn.near_branch16();
                        if !entries.contains_key(&t) {
                            entries.insert(t, format!("{name}_{t:04x}"));
                            todo.push(t);
                        }
                    }
                }
            }
        }
        if let Some(only) = &a.only {
            entries.retain(|k, _| only.contains(k));
        }
        let mut out = String::new();
        let _ = writeln!(out, "// Generated by dune-lift from {source}. Game-derived: do not commit or share.\n");
        out.push_str("use pc::Machine;\nuse pc::cpu::{AX, BP, BX, CF, CS, CX, DF, DI, DS, DX, ES, IF, SI, SP, SS, ZF, Bus, Width};\n");
        out.push_str("use pc::rec::{Bail, Routine};\n\n");
        let mut st = Stats { patched: raw_patched, driver: a.raw.is_some(), ..Stats::default() };
        let mut sizes = 0usize;
        // Block leader -> a routine that has it (the first, by entry).
        let mut leaders: BTreeMap<u16, u16> = BTreeMap::new();
        for (&e, name) in &entries {
            let r = explore(&seg, e, &ind);
            sizes += r.insns.len();
            for &l in &r.leaders {
                if r.insns.contains_key(&l) {
                    leaders.entry(l).or_insert(e);
                }
            }
            if name.contains("_from_") {
                // Only resumed into, and only if no other routine has its blocks.
                out.push_str("#[allow(dead_code)]\n");
            }
            emit_routine(&mut out, &r, name, &mut st, &ind);
        }
        if a.raw.is_some() {
            // The code the routines were lifted from (rewritten bytes left
            // out), so the machine can tell when the driver in memory is
            // not this one.
            let mut bytes = BTreeSet::new();
            for &e in entries.keys() {
                for (&ip, insn) in &explore(&seg, e, &ind).insns {
                    for b in ip..insn.next_ip() as u16 {
                        if !st.patched.contains(&b) {
                            bytes.insert(b);
                        }
                    }
                }
            }
            let mut ranges: Vec<(u16, u16)> = Vec::new();
            for &b in &bytes {
                match ranges.last_mut() {
                    Some(r) if r.1 == b => r.1 = b + 1,
                    _ => ranges.push((b, b + 1)),
                }
            }
            let hash = code_hash(&seg, &ranges);
            out.push_str("/// Code bytes the routines were lifted from: ranges, and their hash.\n");
            out.push_str("pub static CODE: (&[(u16, u16)], u64) = (&[\n");
            for (s, e) in &ranges {
                let _ = writeln!(out, "    (0x{s:04x}, 0x{e:04x}),");
            }
            let _ = writeln!(out, "], 0x{hash:016x});\n");
        }
        out.push_str("pub static ROUTINES: &[(u16, Routine)] = &[\n");
        // Not the `--leaders` routines: nothing calls them (they only
        // resume), so they are not checked as calls.
        for (&e, name) in &entries {
            if !name.contains("_from_") {
                let _ = writeln!(out, "    (0x{e:04x}, f_{e:04x}),");
            }
        }
        out.push_str("];\n\npub static LEADERS: &[(u16, Routine)] = &[\n");
        for (&l, &e) in &leaders {
            let _ = writeln!(out, "    (0x{l:04x}, f_{e:04x}),");
        }
        out.push_str("];\n\npub static NAMES: &[(u16, &str)] = &[\n");
        for (&e, name) in &entries {
            let _ = writeln!(out, "    (0x{e:04x}, \"{name}\"),");
        }
        out.push_str("];\n");
        fs::write(&a.out, out).map_err(|e| format!("{}: {e}", a.out))?;
        let total = st.native + st.stepped + st.interp + st.bail;
        println!(
            "{} routines, {} instructions emitted ({} distinct-by-routine): {:.1}% native, {:.1}% stepped, {:.1}% interpreted, {:.1}% bail",
            entries.len(),
            total,
            sizes,
            100.0 * st.native as f64 / total as f64,
            100.0 * st.stepped as f64 / total as f64,
            100.0 * st.interp as f64 / total as f64,
            100.0 * st.bail as f64 / total as f64
        );
        Ok(())
    };
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
