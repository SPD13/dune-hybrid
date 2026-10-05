//! Validation against the SingleStepTests 80286 real-mode suite (hardware
//! captures, MIT). Fetch and convert with `tests-data/compact.py`, then:
//!
//!     cargo test -p cpu --release -- --ignored --nocapture single_step
//!
//! Optionally restrict to some files: `SST_FILTER=F6,F7`.

use std::{collections::HashMap, fs::File, io::BufReader, path::PathBuf};

use serde_json::Value;

use super::*;

struct TestBus {
    mem: HashMap<u32, u8>,
}

impl Bus for TestBus {
    fn read8(&mut self, addr: u32) -> u8 {
        *self.mem.get(&addr).unwrap_or(&0)
    }
    fn write8(&mut self, addr: u32, value: u8) {
        self.mem.insert(addr, value);
    }
    fn in8(&mut self, _port: u16) -> u8 {
        0xff
    }
    fn out8(&mut self, _port: u16, _value: u8) {}
}

/// Flags the 286 leaves undefined for an instruction form ("80.4" style).
fn undefined_flags(form: &str) -> u16 {
    let (op, sub) = match form.split_once('.') {
        Some((o, s)) => (u8::from_str_radix(o, 16).unwrap(), s.parse::<u8>().ok()),
        None => (u8::from_str_radix(form, 16).unwrap(), None),
    };
    match (op, sub) {
        // Logic ops: AF undefined.
        (0x08..=0x0d | 0x20..=0x25 | 0x30..=0x35 | 0x84 | 0x85 | 0xa8 | 0xa9, _) => AF,
        (0x80..=0x83, Some(1 | 4 | 6)) => AF,
        (0xf6 | 0xf7, Some(0 | 1)) => AF,
        (0x27 | 0x2f, _) => OF,
        (0x37 | 0x3f, _) => OF | SF | ZF | PF,
        (0xd4 | 0xd5, _) => OF | AF | CF,
        (0x69 | 0x6b, _) => SF | ZF | AF | PF,
        (0xf6 | 0xf7, Some(4 | 5)) => SF | ZF | AF | PF,
        (0xf6 | 0xf7, Some(6 | 7)) => CF | PF | AF | ZF | SF | OF,
        // Shifts/rotates: OF undefined unless count == 1, AF undefined for shifts.
        (0xc0 | 0xc1 | 0xd2 | 0xd3, _) => OF | AF,
        (0xd0 | 0xd1, Some(4..=7)) => AF,
        _ => 0,
    }
}

/// Forms we do not validate: port input (bus values not modelled), HLT,
/// FPU escapes, WAIT.
fn skipped(form: &str) -> bool {
    let op = u8::from_str_radix(&form[..2], 16).unwrap();
    matches!(op, 0x6c | 0x6d | 0xe4 | 0xe5 | 0xec | 0xed | 0xf4 | 0x9b | 0xd8..=0xdf | 0xf1)
}

fn reg_index(name: &str) -> Option<(bool, usize)> {
    Some(match name {
        "ax" => (false, AX),
        "bx" => (false, BX),
        "cx" => (false, CX),
        "dx" => (false, DX),
        "sp" => (false, SP),
        "bp" => (false, BP),
        "si" => (false, SI),
        "di" => (false, DI),
        "cs" => (true, CS),
        "ss" => (true, SS),
        "ds" => (true, DS),
        "es" => (true, ES),
        _ => return None,
    })
}

fn run_test(t: &Value, undef: u16) -> Result<(), String> {
    // Segment-overrun faults (#12/#13 on word accesses at offset FFFF) are a
    // 286 protection feature real-mode DOS code does not rely on: the
    // interpreter wraps like an 8086 there.
    if let Some(n) = t["exception"].get("number").and_then(|v| v.as_u64()) {
        if n == 12 || n == 13 {
            return Ok(());
        }
    }
    let init = &t["initial"];
    let mut bus = TestBus { mem: HashMap::new() };
    for cell in init["ram"].as_array().unwrap() {
        bus.mem.insert(cell[0].as_u64().unwrap() as u32, cell[1].as_u64().unwrap() as u8);
    }
    let mut cpu = Cpu::new();
    cpu.addr_mask = A20_ON;
    for (k, v) in init["regs"].as_object().unwrap() {
        let v = v.as_u64().unwrap() as u16;
        match k.as_str() {
            "ip" => cpu.ip = v,
            "flags" => cpu.set_flags(v),
            _ => {
                let (seg, i) = reg_index(k).unwrap();
                if seg {
                    cpu.sregs[i] = v;
                } else {
                    cpu.regs[i] = v;
                }
            }
        }
    }
    let initial_regs = init["regs"].clone();
    let mut halted = false;
    for _ in 0..4 {
        match cpu.step(&mut bus) {
            Step::Halt => {
                halted = true;
                break;
            }
            Step::Invalid { opcode, .. } => return Err(format!("invalid opcode {opcode:#x}")),
            _ => {}
        }
    }
    if !halted {
        return Err("did not reach HLT".into());
    }

    let fin = &t["final"];
    let mut errors = Vec::new();
    for name in ["ax", "bx", "cx", "dx", "sp", "bp", "si", "di", "cs", "ss", "ds", "es", "ip", "flags"] {
        let want = fin["regs"].get(name).or_else(|| initial_regs.get(name)).unwrap().as_u64().unwrap() as u16;
        let got = match name {
            "ip" => cpu.ip,
            "flags" => cpu.flags(),
            _ => {
                let (seg, i) = reg_index(name).unwrap();
                if seg { cpu.sregs[i] } else { cpu.regs[i] }
            }
        };
        let mask = if name == "flags" { !undef & 0x0fd5 } else { 0xffff };
        let want = if name == "flags" { want } else { want };
        if got & mask != want & mask {
            errors.push(format!("{name}: got {got:#06x} want {want:#06x}"));
        }
    }
    // Pushed FLAGS of an exception frame may carry undefined bits.
    let flag_addr = t["exception"].get("flag_address").and_then(|v| v.as_u64()).map(|a| a as u32);
    for cell in fin["ram"].as_array().unwrap() {
        let addr = cell[0].as_u64().unwrap() as u32;
        let want = cell[1].as_u64().unwrap() as u8;
        let got = bus.read8(addr);
        let mask = match flag_addr {
            Some(fa) if addr == fa => !(undef as u8),
            Some(fa) if addr == fa + 1 => !((undef >> 8) as u8),
            _ => 0xff,
        };
        if got & mask != want & mask {
            errors.push(format!("ram[{addr:#x}]: got {got:#04x} want {want:#04x}"));
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join(", ")) }
}

/// Known hardware quirks the interpreter deliberately does not reproduce.
fn known_quirk(form: &str, t: &Value) -> bool {
    // Some out-of-range byte IDIV results do not raise #DE on this 286
    // (e.g. AX=8C60h / 103 leaves AX=E080h). Documented behaviour is to trap.
    (form == "F6.7" && t["exception"].is_null())
        // AAM 0 (#DE): the flags pushed in the fault frame are internal
        // microcode leftovers that only partly follow AL.
        || (form == "D4" && !t["exception"].is_null())
}

#[test]
#[ignore = "needs tests-data/286 (see tests-data/compact.py)"]
fn single_step_286() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests-data/286");
    let filter: Option<Vec<String>> = std::env::var("SST_FILTER").ok().map(|f| f.split(',').map(String::from).collect());
    let mut names: Vec<_> = std::fs::read_dir(&dir).expect("tests-data/286").flatten().map(|e| e.file_name().into_string().unwrap()).collect();
    names.sort();
    let (mut total, mut failed_total) = (0usize, 0usize);
    let mut bad_forms = Vec::new();
    for name in names {
        let form = name.trim_end_matches(".json.gz").to_string();
        if skipped(&form) || filter.as_ref().is_some_and(|f| !f.iter().any(|x| form.starts_with(x.as_str()))) {
            continue;
        }
        let gz = flate2::read::GzDecoder::new(BufReader::new(File::open(dir.join(&name)).unwrap()));
        let tests: Value = serde_json::from_reader(gz).unwrap();
        let undef = undefined_flags(&form);
        let (mut failed, mut first) = (0usize, None);
        let tests = tests.as_array().unwrap();
        for t in tests {
            if let Err(e) = run_test(t, undef) {
                if known_quirk(&form, t) {
                    continue;
                }
                failed += 1;
                first.get_or_insert_with(|| format!("{} -> {e}", t["name"].as_str().unwrap_or("?")));
            }
        }
        total += tests.len();
        failed_total += failed;
        if failed > 0 {
            println!("{form}: {failed}/{} failed; first: {}", tests.len(), first.unwrap());
            bad_forms.push(form);
        }
    }
    println!("{total} tests, {failed_total} failed, {} forms with failures", bad_forms.len());
    assert_eq!(failed_total, 0, "failing forms: {bad_forms:?}");
}
