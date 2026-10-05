//! Instruction decode and execution.

use crate::alu::Width::{self, B, W};
use crate::*;

/// Outcome of one [`Cpu::step`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Step {
    /// An instruction executed normally.
    Ok,
    /// `FE 38 nn` — the host must service callback `nn` (IP is past it).
    Callback(u8),
    /// The CPU is halted waiting for an interrupt.
    Halt,
    /// An opcode this interpreter does not implement; IP is left on it.
    Invalid { cs: u16, ip: u16, opcode: u16 },
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Reg(usize),
    Mem(usize, u16),
}

#[derive(Clone, Copy)]
struct Prefix {
    seg: Option<usize>,
    rep: u8,
}

impl Cpu {
    #[inline]
    fn fetch8<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let v = bus.read8(self.phys(self.sregs[CS], self.ip));
        self.ip = self.ip.wrapping_add(1);
        v
    }

    #[inline]
    fn fetch16<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.fetch8(bus) as u16;
        let hi = self.fetch8(bus) as u16;
        lo | hi << 8
    }

    /// Decode a ModRM byte: returns (reg field, r/m operand).
    fn modrm<B: Bus>(&mut self, bus: &mut B, p: Prefix) -> (usize, Op) {
        let m = self.fetch8(bus);
        let md = m >> 6;
        let reg = ((m >> 3) & 7) as usize;
        let rm = (m & 7) as usize;
        if md == 3 {
            return (reg, Op::Reg(rm));
        }
        let r = &self.regs;
        let (base, mut seg) = match rm {
            0 => (r[BX].wrapping_add(r[SI]), DS),
            1 => (r[BX].wrapping_add(r[DI]), DS),
            2 => (r[BP].wrapping_add(r[SI]), SS),
            3 => (r[BP].wrapping_add(r[DI]), SS),
            4 => (r[SI], DS),
            5 => (r[DI], DS),
            6 => (r[BP], SS),
            _ => (r[BX], DS),
        };
        let off = match md {
            0 if rm == 6 => {
                seg = DS;
                self.fetch16(bus)
            }
            0 => base,
            1 => base.wrapping_add(self.fetch8(bus) as i8 as u16),
            _ => base.wrapping_add(self.fetch16(bus)),
        };
        (reg, Op::Mem(p.seg.unwrap_or(seg), off))
    }

    fn get<B: Bus>(&mut self, bus: &mut B, w: Width, op: Op) -> u32 {
        match (op, w) {
            (Op::Reg(r), Width::B) => self.reg8(r) as u32,
            (Op::Reg(r), Width::W) => self.regs[r] as u32,
            (Op::Mem(s, o), Width::B) => self.read8(bus, s, o) as u32,
            (Op::Mem(s, o), Width::W) => self.read16(bus, s, o) as u32,
        }
    }

    fn put<B: Bus>(&mut self, bus: &mut B, w: Width, op: Op, v: u32) {
        match (op, w) {
            (Op::Reg(r), Width::B) => self.set_reg8(r, v as u8),
            (Op::Reg(r), Width::W) => self.regs[r] = v as u16,
            (Op::Mem(s, o), Width::B) => self.write8(bus, s, o, v as u8),
            (Op::Mem(s, o), Width::W) => self.write16(bus, s, o, v as u16),
        }
    }

    fn get_reg(&self, w: Width, r: usize) -> u32 {
        match w {
            Width::B => self.reg8(r) as u32,
            Width::W => self.regs[r] as u32,
        }
    }

    fn put_reg(&mut self, w: Width, r: usize, v: u32) {
        match w {
            Width::B => self.set_reg8(r, v as u8),
            Width::W => self.regs[r] = v as u16,
        }
    }

    fn cond(&self, c: u8) -> bool {
        let f = |b| self.flag(b);
        let r = match c >> 1 {
            0 => f(OF),
            1 => f(CF),
            2 => f(ZF),
            3 => f(CF) || f(ZF),
            4 => f(SF),
            5 => f(PF),
            6 => f(SF) != f(OF),
            _ => f(ZF) || (f(SF) != f(OF)),
        };
        r ^ (c & 1 != 0)
    }

    /// Raise a fault: IP back on the faulting instruction (286 semantics).
    fn fault<B: Bus>(&mut self, bus: &mut B, n: u8) {
        self.ip = self.insn_ip;
        self.sregs[CS] = self.insn_cs;
        self.interrupt(bus, n);
    }

    /// An encoding the 286 rejects (e.g. LEA with a register operand): #UD.
    fn undefined<B: Bus>(&mut self, bus: &mut B) -> Step {
        self.fault(bus, 6);
        Step::Ok
    }

    fn invalid(&mut self, opcode: u16) -> Step {
        let (cs, ip) = (self.insn_cs, self.insn_ip);
        self.ip = ip;
        Step::Invalid { cs, ip, opcode }
    }

    /// Execute one instruction (including all iterations of a REP string op).
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> Step {
        if self.halted {
            return Step::Halt;
        }
        self.irq_inhibit = false;
        self.insn_cs = self.sregs[CS];
        self.insn_ip = self.ip;
        self.instructions += 1;
        let mut p = Prefix { seg: None, rep: 0 };
        loop {
            let op = self.fetch8(bus);
            match op {
                0x26 => p.seg = Some(ES),
                0x2e => p.seg = Some(CS),
                0x36 => p.seg = Some(SS),
                0x3e => p.seg = Some(DS),
                0xf0 => {}
                0xf2 | 0xf3 => p.rep = op,
                _ => return self.exec(bus, op, p),
            }
        }
    }

    fn exec<B: Bus>(&mut self, bus: &mut B, op: u8, p: Prefix) -> Step {
        let ds = p.seg.unwrap_or(DS);
        match op {
            // ALU r/m,reg / reg,r/m / acc,imm
            0x00..=0x3f if op & 7 < 6 => {
                let alu = (op >> 3) as usize;
                let w = if op & 1 == 0 { B } else { W };
                match op & 7 {
                    0 | 1 => {
                        let (reg, rm) = self.modrm(bus, p);
                        let a = self.get(bus, w, rm);
                        let b = self.get_reg(w, reg);
                        if let Some(r) = self.alu_op(alu, w, a, b) {
                            self.put(bus, w, rm, r);
                        }
                    }
                    2 | 3 => {
                        let (reg, rm) = self.modrm(bus, p);
                        let a = self.get_reg(w, reg);
                        let b = self.get(bus, w, rm);
                        if let Some(r) = self.alu_op(alu, w, a, b) {
                            self.put_reg(w, reg, r);
                        }
                    }
                    _ => {
                        let b = if w == B { self.fetch8(bus) as u32 } else { self.fetch16(bus) as u32 };
                        let a = self.get_reg(w, AX);
                        if let Some(r) = self.alu_op(alu, w, a, b) {
                            self.put_reg(w, AX, r);
                        }
                    }
                }
            }
            0x06 | 0x0e | 0x16 | 0x1e => {
                let v = self.sregs[(op >> 3) as usize];
                self.push(bus, v);
            }
            0x07 | 0x17 | 0x1f => {
                let v = self.pop(bus);
                self.sregs[(op >> 3) as usize] = v;
                if op == 0x17 {
                    self.irq_inhibit = true;
                }
            }
            0x0f => return self.exec_0f(bus, p),
            0x27 | 0x2f => {
                // DAA / DAS
                let al = self.reg8(AX);
                let (old_al, old_cf) = (al, self.flag(CF));
                let mut al = al;
                let sub = op == 0x2f;
                self.set_flag(CF, false);
                if al & 0x0f > 9 || self.flag(AF) {
                    let (v, c) = if sub { al.overflowing_sub(6) } else { al.overflowing_add(6) };
                    al = v;
                    self.set_flag(CF, old_cf || c);
                    self.set_flag(AF, true);
                } else {
                    self.set_flag(AF, false);
                }
                if old_al > 0x99 || old_cf {
                    al = if sub { al.wrapping_sub(0x60) } else { al.wrapping_add(0x60) };
                    self.set_flag(CF, true);
                } else if !sub {
                    self.set_flag(CF, false);
                }
                self.set_reg8(AX, al);
                self.set_szp(B, al as u32);
            }
            0x37 | 0x3f => {
                // AAA / AAS
                let mut ax = self.regs[AX];
                if ax & 0x0f > 9 || self.flag(AF) {
                    if op == 0x37 {
                        ax = ax.wrapping_add(0x106);
                    } else {
                        ax = ax.wrapping_sub(6);
                        ax = (ax & 0x00ff) | ((ax >> 8).wrapping_sub(1) << 8);
                    }
                    self.set_flag(AF, true);
                    self.set_flag(CF, true);
                } else {
                    self.set_flag(AF, false);
                    self.set_flag(CF, false);
                }
                self.regs[AX] = ax & 0xff0f;
            }
            0x40..=0x47 => {
                let r = (op & 7) as usize;
                self.regs[r] = self.inc(W, self.regs[r] as u32) as u16;
            }
            0x48..=0x4f => {
                let r = (op & 7) as usize;
                self.regs[r] = self.dec(W, self.regs[r] as u32) as u16;
            }
            0x50..=0x57 => {
                // 286: PUSH SP pushes the value before the decrement.
                let v = self.regs[(op & 7) as usize];
                self.push(bus, v);
            }
            0x58..=0x5f => {
                let v = self.pop(bus);
                self.regs[(op & 7) as usize] = v;
            }
            0x60 => {
                let sp = self.regs[SP];
                for r in [AX, CX, DX, BX] {
                    self.push(bus, self.regs[r]);
                }
                self.push(bus, sp);
                for r in [BP, SI, DI] {
                    self.push(bus, self.regs[r]);
                }
            }
            0x61 => {
                for r in [DI, SI, BP] {
                    self.regs[r] = self.pop(bus);
                }
                self.pop(bus);
                for r in [BX, DX, CX, AX] {
                    self.regs[r] = self.pop(bus);
                }
            }
            0x62 => {
                // BOUND
                let (reg, rm) = self.modrm(bus, p);
                let Op::Mem(s, o) = rm else { return self.undefined(bus) };
                {
                    let idx = self.regs[reg] as i16;
                    let lo = self.read16(bus, s, o) as i16;
                    let hi = self.read16(bus, s, o.wrapping_add(2)) as i16;
                    if idx < lo || idx > hi {
                        self.fault(bus, 5);
                    }
                }
            }
            0x68 => {
                let v = self.fetch16(bus);
                self.push(bus, v);
            }
            0x6a => {
                let v = self.fetch8(bus) as i8 as u16;
                self.push(bus, v);
            }
            0x69 | 0x6b => {
                let (reg, rm) = self.modrm(bus, p);
                let a = self.get(bus, W, rm) as u16 as i16 as i32;
                let b = if op == 0x69 { self.fetch16(bus) as i16 as i32 } else { self.fetch8(bus) as i8 as i32 };
                let r = a * b;
                self.regs[reg] = r as u16;
                let over = r != (r as i16) as i32;
                self.set_flag(CF, over);
                self.set_flag(OF, over);
            }
            0x6c..=0x6f => self.string_io(bus, op, p),
            0x70..=0x7f => {
                let d = self.fetch8(bus) as i8 as u16;
                if self.cond(op & 0x0f) {
                    self.ip = self.ip.wrapping_add(d);
                }
            }
            0x80..=0x83 => {
                let w = if op & 1 == 0 { B } else { W };
                let (alu, rm) = self.modrm(bus, p);
                let a = self.get(bus, w, rm);
                let b = match op {
                    0x81 => self.fetch16(bus) as u32,
                    0x83 => self.fetch8(bus) as i8 as u16 as u32,
                    _ => self.fetch8(bus) as u32,
                };
                if let Some(r) = self.alu_op(alu, w, a, b) {
                    self.put(bus, w, rm, r);
                }
            }
            0x84 | 0x85 => {
                let w = if op & 1 == 0 { B } else { W };
                let (reg, rm) = self.modrm(bus, p);
                let a = self.get(bus, w, rm);
                let b = self.get_reg(w, reg);
                self.logic(w, a & b);
            }
            0x86 | 0x87 => {
                let w = if op & 1 == 0 { B } else { W };
                let (reg, rm) = self.modrm(bus, p);
                let a = self.get(bus, w, rm);
                let b = self.get_reg(w, reg);
                self.put(bus, w, rm, b);
                self.put_reg(w, reg, a);
            }
            0x88..=0x8b => {
                let w = if op & 1 == 0 { B } else { W };
                let (reg, rm) = self.modrm(bus, p);
                if op & 2 == 0 {
                    let v = self.get_reg(w, reg);
                    self.put(bus, w, rm, v);
                } else {
                    let v = self.get(bus, w, rm);
                    self.put_reg(w, reg, v);
                }
            }
            0x8c => {
                let (reg, rm) = self.modrm(bus, p);
                if reg > 3 {
                    return self.undefined(bus);
                }
                let v = self.sregs[reg] as u32;
                self.put(bus, W, rm, v);
            }
            0x8d => {
                let (reg, rm) = self.modrm(bus, p);
                let Op::Mem(_, o) = rm else { return self.undefined(bus) };
                self.regs[reg] = o;
            }
            0x8e => {
                let (reg, rm) = self.modrm(bus, p);
                if reg == CS || reg > 3 {
                    return self.undefined(bus);
                }
                let v = self.get(bus, W, rm) as u16;
                self.sregs[reg] = v;
                if reg == SS {
                    self.irq_inhibit = true;
                }
            }
            0x8f => {
                let (sub, rm) = self.modrm(bus, p);
                if sub != 0 {
                    return self.undefined(bus);
                }
                let v = self.pop(bus);
                self.put(bus, W, rm, v as u32);
            }
            0x90..=0x97 => {
                let r = (op & 7) as usize;
                self.regs.swap(AX, r);
            }
            0x98 => self.regs[AX] = self.reg8(AX) as i8 as u16,
            0x99 => self.regs[DX] = if self.regs[AX] & 0x8000 != 0 { 0xffff } else { 0 },
            0x9a => {
                let ip = self.fetch16(bus);
                let cs = self.fetch16(bus);
                self.push(bus, self.sregs[CS]);
                self.push(bus, self.ip);
                self.ip = ip;
                self.sregs[CS] = cs;
            }
            0x9b => {} // WAIT: no FPU
            0x9c => {
                let f = self.flags();
                self.push(bus, f);
            }
            0x9d => {
                let f = self.pop(bus);
                self.set_flags(f);
            }
            0x9e => {
                let ah = self.reg8(4) as u16;
                let f = (self.flags() & 0xff00) | (ah & 0xd5);
                self.set_flags(f);
            }
            0x9f => {
                let f = self.flags() as u8;
                self.set_reg8(4, f);
            }
            0xa0..=0xa3 => {
                let w = if op & 1 == 0 { B } else { W };
                let off = self.fetch16(bus);
                let m = Op::Mem(ds, off);
                if op & 2 == 0 {
                    let v = self.get(bus, w, m);
                    self.put_reg(w, AX, v);
                } else {
                    let v = self.get_reg(w, AX);
                    self.put(bus, w, m, v);
                }
            }
            0xa4..=0xa7 | 0xaa..=0xaf => self.string(bus, op, p),
            0xa8 | 0xa9 => {
                let w = if op & 1 == 0 { B } else { W };
                let b = if w == B { self.fetch8(bus) as u32 } else { self.fetch16(bus) as u32 };
                let a = self.get_reg(w, AX);
                self.logic(w, a & b);
            }
            0xb0..=0xb7 => {
                let v = self.fetch8(bus);
                self.set_reg8((op & 7) as usize, v);
            }
            0xb8..=0xbf => {
                let v = self.fetch16(bus);
                self.regs[(op & 7) as usize] = v;
            }
            0xc0 | 0xc1 | 0xd0..=0xd3 => {
                let w = if op & 1 == 0 { B } else { W };
                let (sop, rm) = self.modrm(bus, p);
                let a = self.get(bus, w, rm);
                let count = match op {
                    0xc0 | 0xc1 => self.fetch8(bus) as u32,
                    0xd0 | 0xd1 => 1,
                    _ => self.reg8(CX) as u32,
                } & 0x1f;
                let r = self.shift(sop, w, a, count);
                self.put(bus, w, rm, r);
            }
            0xc2 | 0xc3 => {
                let n = if op == 0xc2 { self.fetch16(bus) } else { 0 };
                self.ip = self.pop(bus);
                self.regs[SP] = self.regs[SP].wrapping_add(n);
            }
            0xc4 | 0xc5 => {
                let (reg, rm) = self.modrm(bus, p);
                let Op::Mem(s, o) = rm else { return self.undefined(bus) };
                let v = self.read16(bus, s, o);
                let sv = self.read16(bus, s, o.wrapping_add(2));
                self.regs[reg] = v;
                self.sregs[if op == 0xc4 { ES } else { DS }] = sv;
            }
            0xc6 | 0xc7 => {
                let w = if op & 1 == 0 { B } else { W };
                let (sub, rm) = self.modrm(bus, p);
                if sub != 0 {
                    return self.undefined(bus);
                }
                let v = if w == B { self.fetch8(bus) as u32 } else { self.fetch16(bus) as u32 };
                self.put(bus, w, rm, v);
            }
            0xc8 => {
                // ENTER
                let size = self.fetch16(bus);
                let level = self.fetch8(bus) & 0x1f;
                self.push(bus, self.regs[BP]);
                let frame = self.regs[SP];
                if level > 0 {
                    let mut bp = self.regs[BP];
                    for _ in 1..level {
                        bp = bp.wrapping_sub(2);
                        let v = self.read16(bus, SS, bp);
                        self.push(bus, v);
                    }
                    self.push(bus, frame);
                }
                self.regs[BP] = frame;
                self.regs[SP] = self.regs[SP].wrapping_sub(size);
            }
            0xc9 => {
                self.regs[SP] = self.regs[BP];
                self.regs[BP] = self.pop(bus);
            }
            0xca | 0xcb => {
                let n = if op == 0xca { self.fetch16(bus) } else { 0 };
                self.ip = self.pop(bus);
                self.sregs[CS] = self.pop(bus);
                self.regs[SP] = self.regs[SP].wrapping_add(n);
            }
            0xcc => self.interrupt(bus, 3),
            0xcd => {
                let n = self.fetch8(bus);
                self.interrupt(bus, n);
            }
            0xce => {
                if self.flag(OF) {
                    self.interrupt(bus, 4);
                }
            }
            0xcf => {
                self.ip = self.pop(bus);
                self.sregs[CS] = self.pop(bus);
                let f = self.pop(bus);
                self.set_flags(f);
            }
            0xd4 => {
                let base = self.fetch8(bus);
                if base == 0 {
                    // The 286 sets SF/ZF/PF from AL before raising #DE.
                    let al = self.reg8(AX) as u32;
                    self.set_szp(B, al);
                    self.fault(bus, 0);
                } else {
                    let al = self.reg8(AX);
                    self.set_reg8(4, al / base);
                    self.set_reg8(AX, al % base);
                    self.set_szp(B, (al % base) as u32);
                }
            }
            0xd5 => {
                let base = self.fetch8(bus);
                let al = self.reg8(AX).wrapping_add(self.reg8(4).wrapping_mul(base));
                self.regs[AX] = al as u16;
                self.set_szp(B, al as u32);
            }
            0xd6 => {
                let v = if self.flag(CF) { 0xff } else { 0 };
                self.set_reg8(AX, v);
            }
            0xd7 => {
                let off = self.regs[BX].wrapping_add(self.reg8(AX) as u16);
                let v = self.read8(bus, ds, off);
                self.set_reg8(AX, v);
            }
            0xd8..=0xdf => {
                // FPU escape with no coprocessor: decode the operand and skip.
                self.modrm(bus, p);
            }
            0xe0..=0xe3 => {
                let d = self.fetch8(bus) as i8 as u16;
                let jump = if op == 0xe3 {
                    self.regs[CX] == 0
                } else {
                    self.regs[CX] = self.regs[CX].wrapping_sub(1);
                    let nz = self.regs[CX] != 0;
                    match op {
                        0xe0 => nz && !self.flag(ZF),
                        0xe1 => nz && self.flag(ZF),
                        _ => nz,
                    }
                };
                if jump {
                    self.ip = self.ip.wrapping_add(d);
                }
            }
            0xe4 => {
                let port = self.fetch8(bus) as u16;
                let v = bus.in8(port);
                self.set_reg8(AX, v);
            }
            0xe5 => {
                let port = self.fetch8(bus) as u16;
                self.regs[AX] = bus.in16(port);
            }
            0xe6 => {
                let port = self.fetch8(bus) as u16;
                bus.out8(port, self.reg8(AX));
            }
            0xe7 => {
                let port = self.fetch8(bus) as u16;
                bus.out16(port, self.regs[AX]);
            }
            0xe8 => {
                let d = self.fetch16(bus);
                self.push(bus, self.ip);
                self.ip = self.ip.wrapping_add(d);
            }
            0xe9 => {
                let d = self.fetch16(bus);
                self.ip = self.ip.wrapping_add(d);
            }
            0xea => {
                let ip = self.fetch16(bus);
                let cs = self.fetch16(bus);
                self.ip = ip;
                self.sregs[CS] = cs;
            }
            0xeb => {
                let d = self.fetch8(bus) as i8 as u16;
                self.ip = self.ip.wrapping_add(d);
            }
            0xec => {
                let v = bus.in8(self.regs[DX]);
                self.set_reg8(AX, v);
            }
            0xed => self.regs[AX] = bus.in16(self.regs[DX]),
            0xee => bus.out8(self.regs[DX], self.reg8(AX)),
            0xef => bus.out16(self.regs[DX], self.regs[AX]),
            0xf4 => {
                self.halted = true;
                return Step::Halt;
            }
            0xf5 => {
                let c = self.flag(CF);
                self.set_flag(CF, !c);
            }
            0xf6 | 0xf7 => return self.group3(bus, op, p),
            0xf8 => self.set_flag(CF, false),
            0xf9 => self.set_flag(CF, true),
            0xfa => self.set_flag(IF, false),
            0xfb => {
                self.set_flag(IF, true);
                self.irq_inhibit = true;
            }
            0xfc => self.set_flag(DF, false),
            0xfd => self.set_flag(DF, true),
            0xfe => {
                let (sub, rm) = self.modrm(bus, p);
                match sub {
                    0 => {
                        let a = self.get(bus, B, rm);
                        let r = self.inc(B, a);
                        self.put(bus, B, rm, r);
                    }
                    1 => {
                        let a = self.get(bus, B, rm);
                        let r = self.dec(B, a);
                        self.put(bus, B, rm, r);
                    }
                    7 if matches!(rm, Op::Mem(DS, _) | Op::Mem(_, _)) => {
                        // FE 38 nn: host callback.
                        let n = self.fetch8(bus);
                        return Step::Callback(n);
                    }
                    _ => return self.undefined(bus),
                }
            }
            0xff => {
                let (sub, rm) = self.modrm(bus, p);
                match sub {
                    0 => {
                        let a = self.get(bus, W, rm);
                        let r = self.inc(W, a);
                        self.put(bus, W, rm, r);
                    }
                    1 => {
                        let a = self.get(bus, W, rm);
                        let r = self.dec(W, a);
                        self.put(bus, W, rm, r);
                    }
                    2 => {
                        let t = self.get(bus, W, rm) as u16;
                        self.push(bus, self.ip);
                        self.ip = t;
                    }
                    3 | 5 => {
                        let Op::Mem(s, o) = rm else { return self.undefined(bus) };
                        let ip = self.read16(bus, s, o);
                        let cs = self.read16(bus, s, o.wrapping_add(2));
                        if sub == 3 {
                            self.push(bus, self.sregs[CS]);
                            self.push(bus, self.ip);
                        }
                        self.ip = ip;
                        self.sregs[CS] = cs;
                    }
                    4 => self.ip = self.get(bus, W, rm) as u16,
                    6 => {
                        let v = self.get(bus, W, rm) as u16;
                        self.push(bus, v);
                    }
                    _ => return self.undefined(bus),
                }
            }
            _ => return self.invalid(op as u16),
        }
        Step::Ok
    }

    fn group3<B: Bus>(&mut self, bus: &mut B, op: u8, p: Prefix) -> Step {
        let w = if op & 1 == 0 { B } else { W };
        let (sub, rm) = self.modrm(bus, p);
        let a = self.get(bus, w, rm);
        match sub {
            0 | 1 => {
                let b = if w == B { self.fetch8(bus) as u32 } else { self.fetch16(bus) as u32 };
                self.logic(w, a & b);
            }
            2 => self.put(bus, w, rm, !a & w.mask()),
            3 => {
                let r = self.sub(w, 0, a, false);
                self.set_flag(CF, a != 0);
                self.put(bus, w, rm, r);
            }
            4 => {
                // MUL
                if w == B {
                    let r = self.reg8(AX) as u16 * a as u16;
                    self.regs[AX] = r;
                    let hi = r >> 8 != 0;
                    self.set_flag(CF, hi);
                    self.set_flag(OF, hi);
                    self.set_szp(B, r as u32 & 0xff);
                } else {
                    let r = self.regs[AX] as u32 * a;
                    self.regs[AX] = r as u16;
                    self.regs[DX] = (r >> 16) as u16;
                    let hi = r >> 16 != 0;
                    self.set_flag(CF, hi);
                    self.set_flag(OF, hi);
                    self.set_szp(W, r & 0xffff);
                }
            }
            5 => {
                // IMUL
                if w == B {
                    let r = self.reg8(AX) as i8 as i16 * a as u8 as i8 as i16;
                    self.regs[AX] = r as u16;
                    let over = r != r as i8 as i16;
                    self.set_flag(CF, over);
                    self.set_flag(OF, over);
                    self.set_szp(B, r as u32 & 0xff);
                } else {
                    let r = self.regs[AX] as i16 as i32 * a as u16 as i16 as i32;
                    self.regs[AX] = r as u16;
                    self.regs[DX] = (r >> 16) as u16;
                    let over = r != r as i16 as i32;
                    self.set_flag(CF, over);
                    self.set_flag(OF, over);
                    self.set_szp(W, r as u32 & 0xffff);
                }
            }
            6 => {
                // DIV
                if a == 0 {
                    self.fault(bus, 0);
                    return Step::Ok;
                }
                if w == B {
                    let n = self.regs[AX] as u32;
                    let q = n / a;
                    if q > 0xff {
                        self.fault(bus, 0);
                        return Step::Ok;
                    }
                    self.set_reg8(AX, q as u8);
                    self.set_reg8(4, (n % a) as u8);
                } else {
                    let n = (self.regs[DX] as u32) << 16 | self.regs[AX] as u32;
                    let q = n / a;
                    if q > 0xffff {
                        self.fault(bus, 0);
                        return Step::Ok;
                    }
                    self.regs[AX] = q as u16;
                    self.regs[DX] = (n % a) as u16;
                }
            }
            _ => {
                // IDIV
                if a == 0 {
                    self.fault(bus, 0);
                    return Step::Ok;
                }
                if w == B {
                    let n = self.regs[AX] as i16 as i32;
                    let d = a as u8 as i8 as i32;
                    let q = n / d;
                    if !(-128..=127).contains(&q) {
                        self.fault(bus, 0);
                        return Step::Ok;
                    }
                    self.set_reg8(AX, q as u8);
                    self.set_reg8(4, (n % d) as u8);
                } else {
                    let n = ((self.regs[DX] as u32) << 16 | self.regs[AX] as u32) as i32 as i64;
                    let d = a as u16 as i16 as i64;
                    let q = n / d;
                    if !(-32768..=32767).contains(&q) {
                        self.fault(bus, 0);
                        return Step::Ok;
                    }
                    self.regs[AX] = q as u16;
                    self.regs[DX] = (n % d) as u16;
                }
            }
        }
        Step::Ok
    }

    fn exec_0f<B: Bus>(&mut self, bus: &mut B, p: Prefix) -> Step {
        let op2 = self.fetch8(bus);
        match op2 {
            0x01 => {
                let (sub, rm) = self.modrm(bus, p);
                match sub {
                    // SMSW: real mode, no coprocessor bits.
                    4 => self.put(bus, W, rm, 0xfff0),
                    _ => return self.invalid(0x0f00 | op2 as u16),
                }
            }
            _ => return self.invalid(0x0f00 | op2 as u16),
        }
        Step::Ok
    }

    fn string_step(&mut self, w: Width, reg: usize) {
        let d = if w == B { 1u16 } else { 2 };
        self.regs[reg] = if self.flag(DF) { self.regs[reg].wrapping_sub(d) } else { self.regs[reg].wrapping_add(d) };
    }

    fn string<B: Bus>(&mut self, bus: &mut B, op: u8, p: Prefix) {
        let w = if op & 1 == 0 { B } else { W };
        let src = p.seg.unwrap_or(DS);
        let rep = p.rep != 0;
        if rep && self.regs[CX] == 0 {
            return;
        }
        loop {
            match op & 0xfe {
                0xa4 => {
                    let v = self.get(bus, w, Op::Mem(src, self.regs[SI]));
                    self.put(bus, w, Op::Mem(ES, self.regs[DI]), v);
                    self.string_step(w, SI);
                    self.string_step(w, DI);
                }
                0xa6 => {
                    let a = self.get(bus, w, Op::Mem(src, self.regs[SI]));
                    let b = self.get(bus, w, Op::Mem(ES, self.regs[DI]));
                    self.sub(w, a, b, false);
                    self.string_step(w, SI);
                    self.string_step(w, DI);
                }
                0xaa => {
                    let v = self.get_reg(w, AX);
                    self.put(bus, w, Op::Mem(ES, self.regs[DI]), v);
                    self.string_step(w, DI);
                }
                0xac => {
                    let v = self.get(bus, w, Op::Mem(src, self.regs[SI]));
                    self.put_reg(w, AX, v);
                    self.string_step(w, SI);
                }
                _ => {
                    let a = self.get_reg(w, AX);
                    let b = self.get(bus, w, Op::Mem(ES, self.regs[DI]));
                    self.sub(w, a, b, false);
                    self.string_step(w, DI);
                }
            }
            if !rep {
                return;
            }
            self.instructions += 1;
            self.regs[CX] = self.regs[CX].wrapping_sub(1);
            if self.regs[CX] == 0 {
                return;
            }
            // CMPS/SCAS stop on the REPE/REPNE condition.
            if matches!(op & 0xfe, 0xa6 | 0xae) && self.flag(ZF) != (p.rep == 0xf3) {
                return;
            }
        }
    }

    fn string_io<B: Bus>(&mut self, bus: &mut B, op: u8, p: Prefix) {
        let w = if op & 1 == 0 { B } else { W };
        let src = p.seg.unwrap_or(DS);
        let rep = p.rep != 0;
        if rep && self.regs[CX] == 0 {
            return;
        }
        loop {
            let port = self.regs[DX];
            if op < 0x6e {
                let v = if w == B { bus.in8(port) as u32 } else { bus.in16(port) as u32 };
                self.put(bus, w, Op::Mem(ES, self.regs[DI]), v);
                self.string_step(w, DI);
            } else {
                let v = self.get(bus, w, Op::Mem(src, self.regs[SI]));
                if w == B {
                    bus.out8(port, v as u8);
                } else {
                    bus.out16(port, v as u16);
                }
                self.string_step(w, SI);
            }
            if !rep {
                return;
            }
            self.regs[CX] = self.regs[CX].wrapping_sub(1);
            if self.regs[CX] == 0 {
                return;
            }
        }
    }
}
