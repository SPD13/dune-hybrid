//! Waits on the timer: the PIT interrupt moves [0CE7Ah] on; these loops
//! read it until it has moved far enough. Each read is its own stretch,
//! so the port bails (or, in a shadow replay, settles the interrupt) at
//! the tick, as the original meets it.

use super::{B, Ctx, W, r8, entered};
use pc::{
    Machine,
    cpu::{AX, BX, CF, CX, DI, IF, SI, SS, ZF},
    rec::Bail,
};

/// The timer word (ticks of the PIT interrupt).
const TIMER: u16 = 0xce7a;

/// `pushf` then `sti`.
fn pushf_sti(c: &mut Ctx) {
    let f = c.m.cpu.flags();
    c.push(f);
    c.set_flag(IF, true);
}

/// `popf`.
fn popf(c: &mut Ctx) {
    let f = c.pop();
    c.m.cpu.set_flags(f);
}

/// `wait_a_bit` (E387): AX ticks of the timer, interrupts on meanwhile
/// (the flags kept); AX and CX kept.
pub fn wait_a_bit(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe387)?;
    let mut c = Ctx::new(m);
    let (ax, cx) = (c.reg(AX), c.reg(CX));
    if ax == 0 {
        c.run(0xe387, 4 + 3)?;
        c.push(ax);
        c.push(cx);
        c.set_reg(CX, 0);
        let cx = c.pop();
        c.set_reg(CX, cx);
        c.pop();
        return c.ret();
    }
    c.run(0xe387, 4 + 2)?;
    c.push(ax);
    c.push(cx);
    c.set_reg(CX, ax);
    pushf_sti(&mut c);
    let mut n = ax;
    loop {
        c.run(0xe38f, 1)?;
        let t0 = c.word_in(SS, TIMER);
        c.set_reg(AX, t0);
        loop {
            c.run(0xe393, 2)?;
            let t = c.word_in(SS, TIMER);
            c.cmp(W, t0, t);
            if t != t0 {
                break;
            }
        }
        c.run(0xe39a, 1)?;
        n = n.wrapping_sub(1);
        c.set_reg(CX, n);
        if n == 0 {
            break;
        }
    }
    c.run(0xe39c, 4)?;
    popf(&mut c);
    let cx = c.pop();
    c.set_reg(CX, cx);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// `wait_interruptable` (DDB0): AX ticks, or until a key or click
/// (`loc_0dd63` gives CF; then CF set). Registers kept (`save_regs` /
/// `restore_regs`), [0CEE8h] 0, the head drawn first unless [227Dh]
/// (`loc_0d64e`), the scancode cleared at the end (`kb_clear_scancode`).
/// Without an input, CF clear and NZ (`or al, 1`).
pub fn wait_interruptable(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xddb0)?;
    wait_at_ddb0(&mut Ctx::new(m))
}

fn wait_at_ddb0(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xddb0, 1)?;
    c.call(0xe270, 0xddb3)?;
    let k = c.byte(0x227d);
    c.run(0xddb3, 4 + (k == 0) as u32)?;
    let ax = c.reg(AX);
    c.push(ax);
    c.set_byte(0xcee8, 0);
    c.cmp(B, k as u16, 0);
    if k == 0 {
        c.call(0xd64e, 0xddc3)?;
    }
    c.run(0xddc3, 4)?;
    c.set_reg(SI, 0xffff);
    c.set_reg(DI, 0xffff);
    let cx = c.pop();
    c.set_reg(CX, cx);
    c.set_flag(IF, true);
    loop {
        c.run(0xddca, 3)?;
        let t0 = c.word(TIMER);
        c.push(t0);
        let cx = c.reg(CX);
        c.push(cx);
        c.call(0xdd63, 0xddd2)?;
        let input = c.flag(CF);
        c.run(0xddd2, 3 + (!input) as u32)?;
        let cx = c.pop();
        c.set_reg(CX, cx);
        let t0 = c.pop();
        c.set_reg(AX, t0);
        if input {
            break;
        }
        c.set_reg(BX, t0);
        let d = loop {
            c.run(0xddd8, 3)?;
            let t = c.word_in(SS, TIMER);
            let d = c.sub(W, t0, t);
            c.set_reg(AX, d);
            if d != 0 {
                break d;
            }
        };
        c.run(0xdde1, 2)?;
        let cx = c.add(W, cx, d);
        c.set_reg(CX, cx);
        if c.flag(CF) {
            continue;
        }
        c.run(0xdde5, 1)?;
        let al = c.or(B, d & 0xff, 1);
        c.set_reg8(r8::AL, al as u8);
        break;
    }
    c.run(0xdde7, 2)?;
    let f = c.m.cpu.flags();
    c.push(f);
    c.call(0xde4e, 0xddeb)?;
    c.run(0xddeb, 2)?;
    popf(c);
    c.call(0xe283, 0xddef)?;
    c.run(0xddef, 1)?;
    c.ret()
}

/// `wait_for_pcm_voice_interruptable` (DDF0): with the voice off
/// ([0DBCDh] negative), NZ at once; with no voice playing (`loc_0aba3`
/// gives ZF), [`wait_interruptable`]; else until the voice ends (NZ) or a
/// key or click (`loc_0dd63`: CF).
pub fn wait_for_pcm_voice(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xddf0)?;
    let mut c = Ctx::new(m);
    let v = c.byte(0xdbcd);
    c.run(0xddf0, 2)?;
    c.cmp(B, v as u16, 0);
    if v & 0x80 != 0 {
        return nz_ret(&mut c, 0xde07);
    }
    c.run(0xddf7, 1)?;
    c.call(0xaba3, 0xddfa)?;
    if c.flag(ZF) {
        c.run(0xddfa, 1)?;
        return wait_at_ddb0(&mut c);
    }
    c.run(0xddfa, 2)?;
    c.call(0xaba3, 0xddff)?;
    loop {
        if c.flag(ZF) {
            c.run(0xddff, 1)?;
            return nz_ret(&mut c, 0xde07);
        }
        c.run(0xddff, 2)?;
        c.call(0xdd63, 0xde04)?;
        if c.flag(CF) {
            c.run(0xde04, 2)?;
            return c.ret();
        }
        c.run(0xde04, 2)?;
        c.call(0xaba3, 0xddff)?;
    }
}

/// DE07h-DE0Bh: `push ax; or al, 1; pop ax; ret` (NZ, CF clear).
fn nz_ret(c: &mut Ctx, at: u16) -> Result<(), Bail> {
    c.run(at, 4)?;
    let ax = c.reg(AX);
    c.push(ax);
    c.or(B, ax & 0xff, 1);
    c.pop();
    c.ret()
}

/// `audio_test_frequency` (A87E), with interrupts on (the flags kept):
/// unless `loc_0ae2f` gives ZF, the voice stopped, voice 0Bh started and
/// the sound driver's function at [3991h] called with SI 3811h; then the
/// ticks until `loc_0a9e7` gives NZ, times 800h, as [2884h] (low) and
/// [2882h] (high).
pub fn audio_test_frequency(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa87e)?;
    let mut c = Ctx::new(m);
    c.run(0xa87e, 3)?;
    pushf_sti(&mut c);
    c.call(0xae2f, 0xa883)?;
    if c.flag(ZF) {
        c.run(0xa883, 3)?;
        popf(&mut c);
        return c.ret();
    }
    c.run(0xa883, 2)?;
    c.call(0xac14, 0xa888)?;
    c.run(0xa888, 2)?;
    c.set_reg8(r8::AL, 0x0b);
    c.call(0xabe9, 0xa88d)?;
    c.run(0xa88d, 1)?;
    c.set_reg(SI, 0x3811);
    c.interp(0xa890, 0xa894)?;
    c.run(0xa894, 2)?;
    let t0 = c.word(TIMER);
    c.push(t0);
    c.call(0xa9e7, 0xa89b)?;
    while c.flag(ZF) {
        c.run(0xa89b, 2)?;
        c.call(0xa9e7, 0xa89b)?;
    }
    c.run(0xa89b, 10)?;
    let t = c.word(TIMER);
    let bx = c.pop();
    c.set_reg(BX, bx);
    let d = c.sub(W, t, bx);
    c.set_reg(AX, d);
    c.set_reg(CX, 0x800);
    c.mul16(0x800);
    let (lo, hi) = (c.reg(AX), c.reg(pc::cpu::DX));
    c.set_word(0x2882, hi);
    c.set_word(0x2884, lo);
    popf(&mut c);
    c.ret()
}

/// `loc_0e85c` (start-up): the PIT's speed measured. With interrupts off,
/// the handlers installed (`loc_0e913`), INT 8 pointed at E8B8h (which
/// sets CS:[0E8D4h]) until it has run once, then at `pit_timer_callback`;
/// the count CS:[0E8D2h] it measured, when both its bytes are set, gives
/// CS:[0EFD9h] (rounded count / 1745h, less 1, from 0 to 0Ah); then the
/// PIT set to 1745h (on into `set_pit_timer_frequency`).
pub fn measure_pit_e85c(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{CS, DX, ES};
    entered(start, 0xe85c)?;
    let mut c = Ctx::new(m);
    c.run(0xe85c, 2)?;
    c.set_flag(IF, false);
    c.call(0xe913, 0xe860)?;
    c.run(0xe860, 6)?;
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.set_sreg(ES, 0);
    c.set_reg(DI, 0x20);
    c.set_word_in(ES, 0x20, 0xe8b8);
    pushf_sti(&mut c);
    loop {
        c.run(0xe86e, 2)?;
        let v = c.byte_in(CS, 0xe8d4);
        c.cmp(B, v as u16, 0);
        if v != 0 {
            break;
        }
    }
    let ax = c.word_in(CS, 0xe8d2);
    let (al, ah) = (ax & 0xff, ax >> 8);
    let measured = ah != 0 && al != 0;
    let (q, r) = (ax / 0x1745, ax % 0x1745);
    let up = measured && (r << 1) >= 0x1745;
    let q1 = q.wrapping_add(up as u16).wrapping_sub(1);
    let neg = q1 & 0x8000 != 0;
    let q2 = if neg { q1.wrapping_add(1) } else { q1 };
    let n = 5
        + if ah == 0 {
            0
        } else if al == 0 {
            2
        } else {
            2 + 6 + up as u32 + 2 + neg as u32 + 2 + (q2 >= 0x0a) as u32 + 1
        }
        + 1;
    c.run(0xe876, n)?;
    popf(&mut c);
    c.set_word_in(ES, 0x20, 0xef6a);
    c.set_reg(AX, ax);
    c.or(B, ah, ah);
    if ah != 0 {
        c.or(B, al, al);
    }
    if measured {
        c.xor(W, 0, 0);
        c.set_reg(DX, r);
        c.set_reg(CX, 0x1745);
        // `div cx` (its flags are overwritten by the `shl` below).
        let d2 = c.shl1(W, r, 1);
        c.set_reg(DX, d2);
        c.cmp(W, d2, 0x1745);
        let mut a = q;
        if up {
            a = c.inc(W, a);
        }
        a = c.dec(W, a);
        if neg {
            a = c.inc(W, a);
        }
        c.cmp(W, a, 0x0a);
        if a >= 0x0a {
            a = (a & 0xff00) | 0x0a;
        }
        c.set_reg(AX, a);
        c.m.cpu.write8(&mut c.m.hw, CS, 0xefd9, a as u8);
    }
    c.set_reg(AX, 0x1745);
    c.jump(0xe8a8)
}
