//! Arithmetic/logic with x86 flag semantics. Operations are generic over the
//! operand width through [`Width`]; values travel as `u32` masked to the width.

use crate::{AF, CF, Cpu, OF, PF, SF, ZF};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Width {
    B,
    W,
}

impl Width {
    #[inline]
    pub fn mask(self) -> u32 {
        match self {
            Width::B => 0xff,
            Width::W => 0xffff,
        }
    }
    #[inline]
    pub fn sign(self) -> u32 {
        match self {
            Width::B => 0x80,
            Width::W => 0x8000,
        }
    }
    #[inline]
    pub fn bits(self) -> u32 {
        match self {
            Width::B => 8,
            Width::W => 16,
        }
    }
}

#[inline]
fn parity(v: u32) -> bool {
    (v as u8).count_ones() % 2 == 0
}

impl Cpu {
    /// Set SF, ZF, PF from a result.
    #[inline]
    pub(crate) fn set_szp(&mut self, w: Width, r: u32) {
        let r = r & w.mask();
        self.set_flag(SF, r & w.sign() != 0);
        self.set_flag(ZF, r == 0);
        self.set_flag(PF, parity(r));
    }

    pub(crate) fn add(&mut self, w: Width, a: u32, b: u32, carry: bool) -> u32 {
        let c = carry as u32;
        let full = a + b + c;
        let r = full & w.mask();
        self.set_flag(CF, full > w.mask());
        self.set_flag(AF, (a ^ b ^ r) & 0x10 != 0);
        self.set_flag(OF, (a ^ r) & (b ^ r) & w.sign() != 0);
        self.set_szp(w, r);
        r
    }

    pub(crate) fn sub(&mut self, w: Width, a: u32, b: u32, borrow: bool) -> u32 {
        let c = borrow as u32;
        let r = a.wrapping_sub(b).wrapping_sub(c) & w.mask();
        self.set_flag(CF, b + c > a);
        self.set_flag(AF, (a ^ b ^ r) & 0x10 != 0);
        self.set_flag(OF, (a ^ b) & (a ^ r) & w.sign() != 0);
        self.set_szp(w, r);
        r
    }

    pub(crate) fn logic(&mut self, w: Width, r: u32) -> u32 {
        let r = r & w.mask();
        self.set_flag(CF, false);
        self.set_flag(OF, false);
        self.set_flag(AF, false);
        self.set_szp(w, r);
        r
    }

    pub(crate) fn inc(&mut self, w: Width, a: u32) -> u32 {
        let cf = self.flag(CF);
        let r = self.add(w, a, 1, false);
        self.set_flag(CF, cf);
        r
    }

    pub(crate) fn dec(&mut self, w: Width, a: u32) -> u32 {
        let cf = self.flag(CF);
        let r = self.sub(w, a, 1, false);
        self.set_flag(CF, cf);
        r
    }

    /// The eight ALU operations of opcodes 00..3F and group 1 (0x80..0x83).
    /// Returns None for CMP (result not written back).
    pub(crate) fn alu_op(&mut self, op: usize, w: Width, a: u32, b: u32) -> Option<u32> {
        match op {
            0 => Some(self.add(w, a, b, false)),
            1 => Some(self.logic(w, a | b)),
            2 => {
                let c = self.flag(CF);
                Some(self.add(w, a, b, c))
            }
            3 => {
                let c = self.flag(CF);
                Some(self.sub(w, a, b, c))
            }
            4 => Some(self.logic(w, a & b)),
            5 => Some(self.sub(w, a, b, false)),
            6 => Some(self.logic(w, a ^ b)),
            _ => {
                self.sub(w, a, b, false);
                None
            }
        }
    }

    /// Group 2: rotates and shifts. `count` is already masked to 5 bits;
    /// a zero count leaves operand and flags untouched.
    pub(crate) fn shift(&mut self, op: usize, w: Width, a: u32, count: u32) -> u32 {
        if count == 0 {
            return a;
        }
        let bits = w.bits();
        let mask = w.mask();
        let sign = w.sign();
        let mut r = a & mask;
        let mut cf = self.flag(CF);
        match op {
            0 => {
                // ROL
                for _ in 0..count {
                    let out = r & sign != 0;
                    r = ((r << 1) | out as u32) & mask;
                    cf = out;
                }
                self.set_flag(CF, cf);
                self.set_flag(OF, (r & sign != 0) ^ cf);
            }
            1 => {
                // ROR
                for _ in 0..count {
                    let out = r & 1 != 0;
                    r = (r >> 1) | if out { sign } else { 0 };
                    cf = out;
                }
                self.set_flag(CF, cf);
                self.set_flag(OF, (r & sign != 0) ^ (r & (sign >> 1) != 0));
            }
            2 => {
                // RCL
                for _ in 0..count {
                    let out = r & sign != 0;
                    r = ((r << 1) | cf as u32) & mask;
                    cf = out;
                }
                self.set_flag(CF, cf);
                self.set_flag(OF, (r & sign != 0) ^ cf);
            }
            3 => {
                // RCR
                for _ in 0..count {
                    let out = r & 1 != 0;
                    r = (r >> 1) | if cf { sign } else { 0 };
                    cf = out;
                }
                self.set_flag(CF, cf);
                self.set_flag(OF, (r & sign != 0) ^ (r & (sign >> 1) != 0));
            }
            4 | 6 => {
                // SHL / SAL
                for _ in 0..count {
                    cf = r & sign != 0;
                    r = (r << 1) & mask;
                }
                self.set_flag(CF, cf);
                self.set_flag(OF, (r & sign != 0) ^ cf);
                self.set_flag(AF, false);
                self.set_szp(w, r);
            }
            5 => {
                // SHR
                self.set_flag(OF, r & sign != 0);
                for _ in 0..count {
                    cf = r & 1 != 0;
                    r >>= 1;
                }
                self.set_flag(CF, cf);
                self.set_flag(AF, false);
                self.set_szp(w, r);
            }
            _ => {
                // SAR
                let neg = r & sign != 0;
                for _ in 0..count.min(bits) {
                    cf = r & 1 != 0;
                    r = (r >> 1) | if neg { sign } else { 0 };
                }
                if count > bits {
                    cf = neg;
                }
                self.set_flag(CF, cf);
                self.set_flag(OF, false);
                self.set_flag(AF, false);
                self.set_szp(w, r);
            }
        }
        r
    }
}
