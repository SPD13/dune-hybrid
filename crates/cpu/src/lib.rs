//! Real-mode x86 interpreter: the 8086 instruction set plus the 80186/80286
//! real-mode additions (PUSHA, ENTER, immediate shifts, IMUL imm, INS/OUTS,
//! BOUND, SMSW). It reports itself as a 286 to CPU-detection code (FLAGS bits
//! 12..15 read as zero in real mode).
//!
//! The CPU knows nothing about the machine around it: memory and ports go
//! through [`Bus`], and the opcode sequence `FE 38 nn` is a host callback
//! (the BIOS/DOS stubs use it) reported as [`Step::Callback`].

mod alu;
mod exec;

pub use exec::Step;

/// Physical memory and I/O ports, as seen by the CPU.
pub trait Bus {
    fn read8(&mut self, addr: u32) -> u8;
    fn write8(&mut self, addr: u32, value: u8);
    fn in8(&mut self, port: u16) -> u8;
    fn out8(&mut self, port: u16, value: u8);
    fn in16(&mut self, port: u16) -> u16 {
        self.in8(port) as u16 | (self.in8(port.wrapping_add(1)) as u16) << 8
    }
    fn out16(&mut self, port: u16, value: u16) {
        self.out8(port, value as u8);
        self.out8(port.wrapping_add(1), (value >> 8) as u8);
    }
}

// General registers, in ModRM encoding order.
pub const AX: usize = 0;
pub const CX: usize = 1;
pub const DX: usize = 2;
pub const BX: usize = 3;
pub const SP: usize = 4;
pub const BP: usize = 5;
pub const SI: usize = 6;
pub const DI: usize = 7;

// Segment registers, in encoding order.
pub const ES: usize = 0;
pub const CS: usize = 1;
pub const SS: usize = 2;
pub const DS: usize = 3;

// FLAGS bits.
pub const CF: u16 = 1 << 0;
pub const PF: u16 = 1 << 2;
pub const AF: u16 = 1 << 4;
pub const ZF: u16 = 1 << 6;
pub const SF: u16 = 1 << 7;
pub const TF: u16 = 1 << 8;
pub const IF: u16 = 1 << 9;
pub const DF: u16 = 1 << 10;
pub const OF: u16 = 1 << 11;

/// Bits that exist in real-mode FLAGS on a 286 (bit 1 always reads as 1).
const FLAGS_MASK: u16 = 0x0fd5;

/// Linear address of `seg:off`, before the A20 mask.
#[inline]
pub fn linear(seg: u16, off: u16) -> u32 {
    ((seg as u32) << 4) + off as u32
}

/// Address mask with the A20 line disabled (8086-style 1 MB wrap).
pub const A20_OFF: u32 = 0x0f_ffff;
/// Address mask with A20 enabled (real mode reaches 0x10FFEF).
pub const A20_ON: u32 = 0xff_ffff;

#[derive(Clone, Debug)]
pub struct Cpu {
    pub regs: [u16; 8],
    pub sregs: [u16; 4],
    pub ip: u16,
    flags: u16,
    /// Set by HLT; cleared when an interrupt is delivered.
    pub halted: bool,
    /// Instructions executed (REP iterations count individually).
    pub instructions: u64,
    /// One-instruction interrupt shadow after MOV SS / POP SS / STI.
    pub irq_inhibit: bool,
    /// IP of the instruction being executed (for faults and diagnostics).
    pub insn_ip: u16,
    pub insn_cs: u16,
    /// [`A20_OFF`] or [`A20_ON`].
    pub addr_mask: u32,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Cpu {
            regs: [0; 8],
            sregs: [0; 4],
            ip: 0,
            flags: 0x0002,
            halted: false,
            instructions: 0,
            irq_inhibit: false,
            insn_ip: 0,
            insn_cs: 0,
            addr_mask: A20_OFF,
        }
    }

    #[inline]
    pub fn flags(&self) -> u16 {
        self.flags
    }

    #[inline]
    pub fn set_flags(&mut self, value: u16) {
        self.flags = (value & FLAGS_MASK) | 0x0002;
    }

    #[inline]
    pub fn flag(&self, f: u16) -> bool {
        self.flags & f != 0
    }

    #[inline]
    pub fn set_flag(&mut self, f: u16, on: bool) {
        if on {
            self.flags |= f;
        } else {
            self.flags &= !f;
        }
    }

    #[inline]
    pub fn reg8(&self, r: usize) -> u8 {
        if r < 4 {
            self.regs[r] as u8
        } else {
            (self.regs[r - 4] >> 8) as u8
        }
    }

    #[inline]
    pub fn set_reg8(&mut self, r: usize, v: u8) {
        if r < 4 {
            self.regs[r] = (self.regs[r] & 0xff00) | v as u16;
        } else {
            self.regs[r - 4] = (self.regs[r - 4] & 0x00ff) | (v as u16) << 8;
        }
    }

    /// Physical address of `seg:off` under the current A20 mask.
    #[inline]
    pub fn phys(&self, seg: u16, off: u16) -> u32 {
        linear(seg, off) & self.addr_mask
    }

    pub fn read8<B: Bus>(&self, bus: &mut B, seg: usize, off: u16) -> u8 {
        bus.read8(self.phys(self.sregs[seg], off))
    }

    pub fn read16<B: Bus>(&self, bus: &mut B, seg: usize, off: u16) -> u16 {
        let s = self.sregs[seg];
        bus.read8(self.phys(s, off)) as u16 | (bus.read8(self.phys(s, off.wrapping_add(1))) as u16) << 8
    }

    pub fn write8<B: Bus>(&self, bus: &mut B, seg: usize, off: u16, v: u8) {
        bus.write8(self.phys(self.sregs[seg], off), v);
    }

    pub fn write16<B: Bus>(&self, bus: &mut B, seg: usize, off: u16, v: u16) {
        let s = self.sregs[seg];
        bus.write8(self.phys(s, off), v as u8);
        bus.write8(self.phys(s, off.wrapping_add(1)), (v >> 8) as u8);
    }

    pub fn push<B: Bus>(&mut self, bus: &mut B, v: u16) {
        self.regs[SP] = self.regs[SP].wrapping_sub(2);
        self.write16(bus, SS, self.regs[SP], v);
    }

    pub fn pop<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let v = self.read16(bus, SS, self.regs[SP]);
        self.regs[SP] = self.regs[SP].wrapping_add(2);
        v
    }

    /// Deliver interrupt `n` (software or hardware): push FLAGS, CS, IP and
    /// jump through the real-mode vector table.
    pub fn interrupt<B: Bus>(&mut self, bus: &mut B, n: u8) {
        let flags = self.flags;
        self.push(bus, flags);
        self.push(bus, self.sregs[CS]);
        self.push(bus, self.ip);
        self.set_flag(IF, false);
        self.set_flag(TF, false);
        let vec = (n as u32) * 4;
        self.ip = bus.read8(vec) as u16 | (bus.read8(vec + 1) as u16) << 8;
        self.sregs[CS] = bus.read8(vec + 2) as u16 | (bus.read8(vec + 3) as u16) << 8;
        self.halted = false;
    }

    /// Whether a maskable hardware interrupt may be taken before the next
    /// instruction.
    pub fn accepts_irq(&self) -> bool {
        self.flag(IF) && !self.irq_inhibit
    }
}

#[cfg(test)]
mod tests;
