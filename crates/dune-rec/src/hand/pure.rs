//! The flag helpers' results without the flags.
//!
//! A readable port uses these where no one reads the flags an operation
//! leaves (its callers, `flags_used.py`, and nothing after it in the
//! port). Each gives exactly what the CPU computes: operands of byte width
//! are not masked first where the CPU doesn't mask them (`add`, `sub`),
//! the result always is, and a shift by 0 returns its operand as it is.

#![allow(dead_code)] // the set is complete; not every op is used yet

use pc::cpu::Width;

fn mask(w: Width) -> u32 {
    w.mask()
}

pub fn add(w: Width, a: u16, b: u16) -> u16 {
    ((a as u32 + b as u32) & mask(w)) as u16
}

pub fn sub(w: Width, a: u16, b: u16) -> u16 {
    ((a as u32).wrapping_sub(b as u32) & mask(w)) as u16
}

pub fn and(w: Width, a: u16, b: u16) -> u16 {
    ((a & b) as u32 & mask(w)) as u16
}

pub fn or(w: Width, a: u16, b: u16) -> u16 {
    ((a | b) as u32 & mask(w)) as u16
}

pub fn xor(w: Width, a: u16, b: u16) -> u16 {
    ((a ^ b) as u32 & mask(w)) as u16
}

pub fn inc(w: Width, a: u16) -> u16 {
    add(w, a, 1)
}

pub fn dec(w: Width, a: u16) -> u16 {
    sub(w, a, 1)
}

pub fn neg(w: Width, a: u16) -> u16 {
    sub(w, 0, a)
}

/// `shl x, cl` (the count mod 32).
pub fn shl(w: Width, a: u16, count: u8) -> u16 {
    shl1(w, a, (count & 0x1f) as u32)
}

/// `n` times `shl x, 1`.
pub fn shl1(w: Width, a: u16, n: u32) -> u16 {
    if n == 0 {
        return a;
    }
    (((a as u64 & mask(w) as u64) << n.min(32)) & mask(w) as u64) as u16
}

/// `n` times `shr x, 1`.
pub fn shr1(w: Width, a: u16, n: u32) -> u16 {
    if n == 0 {
        return a;
    }
    ((a as u32 & mask(w)) >> n.min(31)) as u16
}

pub fn rol1(w: Width, a: u16) -> u16 {
    let r = a as u32 & mask(w);
    (((r << 1) | (r >> (w.bits() - 1))) & mask(w)) as u16
}

pub fn ror1(w: Width, a: u16) -> u16 {
    let r = a as u32 & mask(w);
    (((r >> 1) | ((r & 1) << (w.bits() - 1))) & mask(w)) as u16
}

/// What `lahf` loads after `cmp a, b` (SF ZF 0 AF 0 PF 1 CF): code that
/// keeps a compare's flags as data.
pub fn lahf_cmp(w: Width, a: u16, b: u16) -> u8 {
    let r = sub(w, a, b);
    let top = if w == Width::W { 0x8000 } else { 0x80 };
    let cf = (a as u32 & mask(w)) < (b as u32 & mask(w));
    let af = (a ^ b ^ r) & 0x10 != 0;
    let pf = (r as u8).count_ones() % 2 == 0;
    (r & top != 0) as u8 * 0x80 | (r == 0) as u8 * 0x40 | af as u8 * 0x10 | pf as u8 * 4 | 2 | cf as u8
}
