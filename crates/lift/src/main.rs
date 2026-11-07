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
/// Code bytes the program rewrites at run time: instructions containing
/// them always run through the interpreter, which fetches from memory.
/// C13C: the immediate of `mov ax, 25h` (patched at 0AD3 and 0B24).
const PATCHED: &[u16] = &[0xc13c];

struct Args {
    exe: String,
    lst: String,
    cov: Vec<String>,
    out: String,
    only: Option<BTreeSet<u16>>,
}

fn parse() -> Result<Args, String> {
    let mut a = Args { exe: String::new(), lst: String::new(), cov: Vec::new(), out: String::new(), only: None };
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().ok_or(format!("missing value for {k}"));
        match k.as_str() {
            "--exe" => a.exe = v()?,
            "--lst" => a.lst = v()?,
            "--cov" => a.cov.push(v()?),
            "--out" => a.out = v()?,
            "--only" => {
                let set = v()?.split(',').map(|s| u16::from_str_radix(s.trim(), 16).map_err(|e| format!("--only: {e}"))).collect::<Result<_, _>>()?;
                a.only = Some(set);
            }
            other if !other.starts_with("--") => a.cov.push(other.to_string()),
            _ => return Err(format!("unknown option {k}")),
        }
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
}

fn read_coverage(paths: &[String], seg: &[u8]) -> Result<Indirect, String> {
    let mut ind = Indirect { call_targets: BTreeSet::new(), jump_targets: BTreeMap::new() };
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
            if fc != LOAD_SEG || tc != LOAD_SEG {
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
            if r.insns.contains_key(&ip) || ip as usize >= SEG000_LEN {
                break;
            }
            let insn = decode_at(seg, ip);
            r.insns.insert(ip, insn);
            if insn.is_invalid() {
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
                    break;
                }
                FlowControl::Exception => break,
                _ if insn.mnemonic() == Mnemonic::Hlt => break,
                // After a call or INT, execution resumes here: a leader, so
                // Rust can take over again there (and blocks hold no calls).
                FlowControl::Call | FlowControl::IndirectCall | FlowControl::Interrupt => {
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
        "m.cpu.ip = 0x{ip:04x}; m.step_insn()?; if m.cpu.ip != 0x{next:04x} || m.cpu.sregs[CS] != LOAD_SEG {{ return Err(Bail); }}"
    );
    let interp = format!("m.rec_interp(0x{ip:04x}, 0x{next:04x})?;");
    if insn.is_invalid() {
        st.bail += 1;
        return (bail, true);
    }
    let patched = PATCHED.iter().any(|&p| p >= ip && p < next);
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
                Some(format!("{{ let a = {}; {} }}", get(insn, 0, w)?, put(insn, 0, w, &format!("!a & {mask}"))?))
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
                    (In, false) => format!("{{ let v = m.hw.in8({port}); m.cpu.set_reg8(0, v); }}"),
                    (In, true) => format!("{{ let v = m.hw.in16({port}); m.cpu.regs[AX] = v; }}"),
                    (_, false) => format!("{{ let p = {port}; let v = m.cpu.reg8(0); m.hw.out8(p, v); }}"),
                    (_, true) => format!("{{ let p = {port}; let v = m.cpu.regs[AX]; m.hw.out16(p, v); }}"),
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
        // Control that leaves the routine's segment or frame: the interpreter takes over.
        Retf | Iret | Hlt => {
            st.bail += 1;
            (bail, true)
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

fn emit_routine(out: &mut String, r: &Routine, name: &str, st: &mut Stats) {
    let mut fmt = IntelFormatter::new();
    fmt.options_mut().set_hex_prefix("0x");
    fmt.options_mut().set_hex_suffix("");
    let _ = writeln!(out, "/// `{name}` (seg000:{:04x})", r.entry);
    let _ = writeln!(out, "pub fn f_{:04x}(m: &mut Machine, start: u16) -> Result<(), Bail> {{", r.entry);
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

fn main() -> ExitCode {
    let a = match parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let run = || -> Result<(), String> {
        let exe = fs::read(&a.exe).map_err(|e| format!("{}: {e}", a.exe))?;
        let seg = load_seg000(&exe)?;
        let mut entries = listing_entries(&a.lst)?;
        let ind = read_coverage(&a.cov, &seg)?;
        for &t in &ind.call_targets {
            entries.entry(t).or_insert_with(|| format!("sub_{t:04x}"));
        }
        if let Some(only) = &a.only {
            entries.retain(|k, _| only.contains(k));
        }
        let mut out = String::new();
        out.push_str("// Generated by dune-lift from DNCDPRG.EXE. Game-derived: do not commit or share.\n\n");
        out.push_str("use pc::Machine;\nuse pc::cpu::{AX, BP, BX, CF, CS, CX, DF, DI, DS, DX, ES, SI, SP, SS, ZF, Bus, Width};\n");
        out.push_str("use pc::rec::{Bail, Routine};\n\nconst LOAD_SEG: u16 = 0x1000;\n\n");
        let mut st = Stats::default();
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
            emit_routine(&mut out, &r, name, &mut st);
        }
        out.push_str("pub static ROUTINES: &[(u16, Routine)] = &[\n");
        for &e in entries.keys() {
            let _ = writeln!(out, "    (0x{e:04x}, f_{e:04x}),");
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
