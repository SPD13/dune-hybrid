//! A minimal DOS PC: just enough machine and BIOS/DOS for DNCDPRG.EXE.
//!
//! The original program runs on the [`cpu`] interpreter. BIOS and DOS are
//! implemented in Rust behind `FE 38 nn` callback stubs in the ROM area, so
//! interrupt vectors the game hooks and chains keep working. Time is virtual:
//! derived from the instruction count at a configurable speed, so a run is
//! reproducible.

mod bios;
mod dos;
pub mod fs;
pub mod hw;
pub mod sound;

use std::collections::VecDeque;

pub use cpu::{self, Cpu, Step};
pub use fs::FileSystem;
pub use hw::Hardware;

use cpu::{AX, BX, CS, CX, DS, DX, ES, SP, SS};

/// Segment the program image is loaded at (matches Spice86/Cryogenic `-p 4096`,
/// so addresses line up with the reference disassembly and Cryogenic).
pub const LOAD_SEG: u16 = 0x1000;
pub const PSP_SEG: u16 = LOAD_SEG - 0x10;
const ENV_SEG: u16 = 0x0f00;
/// Top of conventional memory (640 KB).
pub const MEM_TOP_SEG: u16 = 0xa000;
/// BIOS stub area: one 8-byte stub per interrupt vector.
pub const STUB_SEG: u16 = 0xf000;
const STUB_BASE: u16 = 0x1000;
/// DOS's private data (InDOS flag …).
const DOS_DATA_SEG: u16 = 0x0070;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunExit {
    /// Reached the requested virtual time.
    Deadline,
    /// INT 21h/4Ch.
    Exited(u8),
    /// HLT with interrupts disabled: nothing can wake the CPU.
    Stuck,
    /// An opcode the interpreter does not implement.
    Invalid { cs: u16, ip: u16, opcode: u16 },
}

#[derive(Default, Clone, Copy)]
pub struct Mouse {
    pub x: i32,
    pub y: i32,
    pub buttons: u16,
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
    pub visible: i32,
    /// When buttons last went down (virtual ns) and a deferred release.
    pressed_at: u64,
    deferred: Option<u16>,
}

/// The game samples buttons with INT 33h/3 instead of queuing clicks, so a
/// press is held at least this long (virtual time) before a release applies.
const MIN_BUTTON_HOLD_NS: u64 = 60_000_000;

pub struct Machine {
    pub cpu: Cpu,
    pub hw: Hardware,
    pub fs: Box<dyn FileSystem>,
    /// Virtual CPU speed.
    pub ns_per_insn: f64,
    /// Virtual time skipped while halted.
    idle_ns: u64,
    pub exited: Option<u8>,
    pub mouse: Mouse,
    /// BIOS keyboard buffer (scan << 8 | ascii) for INT 16h.
    pub bios_keys: VecDeque<u16>,
    /// Text written to the console (INT 21h 02/09/40, INT 10h 0Eh).
    pub console: String,
    /// Diagnostics: unhandled services etc.
    pub log: VecDeque<String>,
    files: Vec<Option<Box<dyn fs::DosFile>>>,
    dta: (u16, u16),
    /// Interrupt counters for diagnostics.
    pub int_counts: [u64; 256],
    /// Log INT 33h calls (diagnostics).
    pub trace_mouse: bool,
}

impl Machine {
    /// Build a machine with `exe` loaded and ready to run. `cmdline` is the
    /// command tail (e.g. "ADP330 SBP2227").
    pub fn new(exe: &[u8], cmdline: &str, fs: Box<dyn FileSystem>) -> Result<Machine, String> {
        let mut m = Machine {
            cpu: Cpu::new(),
            hw: Hardware::new(),
            fs,
            ns_per_insn: 1e9 / 20e6,
            idle_ns: 0,
            exited: None,
            mouse: Mouse { max_x: 639, max_y: 199, visible: -1, ..Default::default() },
            bios_keys: VecDeque::new(),
            console: String::new(),
            log: VecDeque::new(),
            files: Vec::new(),
            dta: (PSP_SEG, 0x80),
            int_counts: [0; 256],
            trace_mouse: false,
        };
        for _ in 0..5 {
            m.files.push(None);
        }
        bios::install(&mut m);
        dos::load_exe(&mut m, exe, cmdline)?;
        Ok(m)
    }

    /// Set the virtual CPU speed in instructions per second.
    pub fn set_speed(&mut self, ips: f64) {
        // Keep virtual time continuous across the change.
        let now = self.now_ns();
        self.ns_per_insn = 1e9 / ips;
        let base = (self.cpu.instructions as f64 * self.ns_per_insn) as u64;
        self.idle_ns = now.saturating_sub(base);
    }

    #[inline]
    pub fn now_ns(&self) -> u64 {
        (self.cpu.instructions as f64 * self.ns_per_insn) as u64 + self.idle_ns
    }

    pub fn log(&mut self, msg: String) {
        if self.log.len() >= 2000 {
            self.log.pop_front();
        }
        self.log.push_back(msg);
    }

    /// Run until virtual time `deadline_ns` (or an exit condition).
    pub fn run_until(&mut self, deadline_ns: u64) -> RunExit {
        let mut next_event = self.hw.update(self.now_ns());
        loop {
            if let Some(code) = self.exited {
                return RunExit::Exited(code);
            }
            let now = self.now_ns();
            if now >= deadline_ns {
                return RunExit::Deadline;
            }
            self.hw.now_ns = now;
            if now >= next_event || self.hw.reschedule {
                next_event = self.hw.update(now);
                if let Some(b) = self.mouse.deferred {
                    if now >= self.mouse.pressed_at + MIN_BUTTON_HOLD_NS {
                        self.mouse.buttons = b;
                        self.mouse.deferred = None;
                    }
                }
            }
            if self.cpu.accepts_irq() && self.hw.irq_pending() {
                if let Some(v) = self.hw.acknowledge_irq() {
                    self.int_counts[v as usize] += 1;
                    self.cpu.interrupt(&mut self.hw, v);
                }
            }
            if self.hw.trace_ports.is_some() {
                self.hw.trace_pc = (self.cpu.sregs[CS], self.cpu.ip);
            }
            match self.cpu.step(&mut self.hw) {
                Step::Ok => {}
                Step::Callback(n) => self.callback(n),
                Step::Halt => {
                    if !self.cpu.flag(cpu::IF) {
                        return RunExit::Stuck;
                    }
                    // Sleep until the next timer event (or the deadline).
                    let wake = next_event.min(deadline_ns).max(now + 1);
                    self.idle_ns += wake - now;
                    next_event = self.hw.update(self.now_ns());
                    if self.hw.irq_pending() {
                        self.cpu.halted = false;
                    } else if self.now_ns() >= deadline_ns {
                        return RunExit::Deadline;
                    }
                    // Halted with nothing pending: loop to wait for the next event.
                    self.cpu.halted = !self.hw.irq_pending();
                }
                Step::Invalid { cs, ip, opcode } => return RunExit::Invalid { cs, ip, opcode },
            }
        }
    }

    fn callback(&mut self, n: u8) {
        self.int_counts[n as usize] += 1;
        match n {
            0x08 => bios::timer_tick(self),
            0x09 => bios::keyboard_irq(self),
            0x0a..=0x0f | 0x70..=0x77 => bios::eoi(self, n),
            0x10 => bios::int10(self),
            0x11 => self.cpu.regs[AX] = 0x0021,
            0x12 => self.cpu.regs[AX] = 640,
            0x15 => bios::int15(self),
            0x16 => bios::int16(self),
            0x1a => bios::int1a(self),
            0x21 => dos::int21(self),
            0x2f => dos::int2f(self),
            0x33 => bios::int33(self),
            0x67 => {
                self.set_ah(0x84);
            }
            0x00 => self.log(format!("divide error at {:04x}:{:04x}", self.cpu.insn_cs, self.cpu.insn_ip)),
            0x06 => self.log(format!("invalid opcode #UD near {:04x}:{:04x}", self.cpu.insn_cs, self.cpu.insn_ip)),
            _ => {}
        }
        if matches!(n, 0x10..=0x1a | 0x21 | 0x2f | 0x33 | 0x67) {
            self.write_back_flags();
        }
    }

    /// Software-interrupt services return status in FLAGS; the stub's IRET
    /// would restore the caller's FLAGS, so patch the stacked copy.
    fn write_back_flags(&mut self) {
        const ARITH: u16 = 0x08d5;
        let sp = self.cpu.regs[SP].wrapping_add(4);
        let stacked = self.cpu.read16(&mut self.hw, SS, sp);
        let v = (stacked & !ARITH) | (self.cpu.flags() & ARITH);
        self.cpu.write16(&mut self.hw, SS, sp, v);
    }

    // ---- register helpers for the service code ----
    pub(crate) fn ah(&self) -> u8 {
        (self.cpu.regs[AX] >> 8) as u8
    }
    pub(crate) fn al(&self) -> u8 {
        self.cpu.regs[AX] as u8
    }
    pub(crate) fn set_ah(&mut self, v: u8) {
        self.cpu.set_reg8(4, v);
    }
    pub(crate) fn set_al(&mut self, v: u8) {
        self.cpu.set_reg8(0, v);
    }
    pub(crate) fn set_cf(&mut self, on: bool) {
        self.cpu.set_flag(cpu::CF, on);
    }
    pub(crate) fn mem_read(&self, seg: u16, off: u16) -> u8 {
        self.hw.mem[(cpu::linear(seg, off) & cpu::A20_OFF) as usize]
    }
    pub(crate) fn mem_write(&mut self, seg: u16, off: u16, v: u8) {
        let a = (cpu::linear(seg, off) & cpu::A20_OFF) as usize;
        self.hw.mem[a] = v;
    }
    pub(crate) fn mem_read16(&self, seg: u16, off: u16) -> u16 {
        self.mem_read(seg, off) as u16 | (self.mem_read(seg, off.wrapping_add(1)) as u16) << 8
    }
    pub(crate) fn mem_write16(&mut self, seg: u16, off: u16, v: u16) {
        self.mem_write(seg, off, v as u8);
        self.mem_write(seg, off.wrapping_add(1), (v >> 8) as u8);
    }
    /// ASCIIZ string at seg:off.
    pub(crate) fn read_asciiz(&self, seg: u16, off: u16) -> String {
        let mut s = String::new();
        for i in 0..128u16 {
            let c = self.mem_read(seg, off.wrapping_add(i));
            if c == 0 {
                break;
            }
            s.push(c as char);
        }
        s
    }

    pub fn regs_string(&self) -> String {
        let c = &self.cpu;
        format!(
            "AX={:04x} BX={:04x} CX={:04x} DX={:04x} SI={:04x} DI={:04x} BP={:04x} SP={:04x} DS={:04x} ES={:04x} SS={:04x} CS:IP={:04x}:{:04x} F={:04x}",
            c.regs[AX], c.regs[BX], c.regs[CX], c.regs[DX], c.regs[cpu::SI], c.regs[cpu::DI], c.regs[cpu::BP], c.regs[SP],
            c.sregs[DS], c.sregs[ES], c.sregs[SS], c.sregs[CS], c.ip, c.flags()
        )
    }

    // ---- host input ----

    /// Host keyboard: set-1 make code, `pressed` false sends the break code.
    pub fn key(&mut self, scancode: u8, pressed: bool) {
        self.hw.key_scancode(if pressed { scancode } else { scancode | 0x80 });
    }

    /// Host mouse in screen pixels (320×200) and button mask (1 L, 2 R, 4 M).
    pub fn mouse_input(&mut self, px: i32, py: i32, buttons: u16) {
        // initialize_mouse probes the driver's granularity (seg000:e996): this
        // driver keeps single units, so the game picks a scaler of 0 and works
        // in screen pixels directly.
        let now = self.now_ns();
        let m = &mut self.mouse;
        m.x = px.clamp(m.min_x, m.max_x.max(m.min_x));
        m.y = py.clamp(m.min_y, m.max_y.max(m.min_y));
        if buttons & !m.buttons != 0 {
            m.pressed_at = now;
        }
        if m.buttons & !buttons != 0 && now < m.pressed_at + MIN_BUTTON_HOLD_NS {
            m.deferred = Some(buttons);
            m.buttons |= buttons;
        } else {
            m.deferred = None;
            m.buttons = buttons;
        }
    }

    /// Audio produced so far (interleaved stereo f32 at 48 kHz), drained.
    pub fn take_audio(&mut self) -> Vec<f32> {
        let now = self.now_ns();
        self.hw.audio_catch_up(now);
        std::mem::take(&mut self.hw.audio.out)
    }

    /// The current mode 13h frame as RGB888.
    pub fn screen_rgb(&self, out: &mut [u8]) {
        self.hw.vga.render_rgb(&self.hw.mem, out);
    }

    /// Close every open file (flushes in-memory saves).
    pub fn close_all(&mut self) {
        for f in self.files.iter_mut().flatten() {
            f.close();
        }
    }
}
