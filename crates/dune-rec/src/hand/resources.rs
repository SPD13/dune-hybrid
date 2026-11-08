//! Resources in memory: opening one as the current sprite sheet, and the
//! condition evaluator over the CONDIT script.

use super::{B, Ctx, W, entered, globals as g};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CX, DI, DS, DX, ES, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const OPEN_RESOURCE_BY_INDEX: u16 = 0xc13e;
    pub const LOAD_RESOURCE_TO: u16 = 0xf0b9;
    pub const APPLY_SPRITE_SHEET_PALETTE: u16 = 0xc1aa;
    pub const READ_CONDIT_OPERAND: u16 = 0xa30b;
    pub const EVALUATE_OPERATOR: u16 = 0xa334;
    /// The immediate of `mov ax, 25h` at C13B: written by the game (the
    /// on-map resource changes), so read where it runs.
    pub const ONMAP_RESOURCE_IMMEDIATE: u16 = 0xc13c;
}

/// Per resource, a far pointer to it in memory (segment 0: not loaded).
const RESOURCE_POINTERS: u16 = 0xd844;
/// Per resource, when it was last opened (the timer's high byte and the
/// next): at `RESOURCE_LAST_USE + 2 × index`.
const RESOURCE_LAST_USE: u16 = 0xda8c;

/// From C13E: makes resource AX the current sprite sheet (unless it is
/// already, or AX is negative): loads it if it is not in memory (applying
/// its palette when it has one, and telling the graphics driver), notes
/// when the previous one was last used. Exit: the sheet's pointer at
/// [`g::SPRITE_SHEET`]; BX, SI, DI kept.
pub(super) fn open(c: &mut Ctx) -> Result<(), Bail> {
    let ax = c.reg(AX);
    if ax & 0x8000 != 0 {
        c.run(0xc13e, 3)?;
        c.or(W, ax, ax);
        return c.ret();
    }
    let current = c.word(g::RESOURCE_ID);
    let same = current == ax;
    c.run(0xc13e, if same { 9 } else { 11 })?;
    c.or(W, ax, ax);
    let bx = c.reg(BX);
    c.push(bx);
    c.set_word(g::RESOURCE_ID, ax);
    c.set_reg(BX, current);
    c.cmp(W, ax, current);
    if same {
        let bx = c.pop();
        c.set_reg(BX, bx);
        return c.ret();
    }
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.push(si);
    c.push(di);
    let slot = c.shl1(W, current, 1);
    c.set_reg(BX, slot);
    if slot & 0x8000 == 0 {
        c.run(0xc153, 2)?;
        let used = c.word(g::TIMER + 1);
        c.set_reg(SI, used);
        c.set_word(slot.wrapping_add(RESOURCE_LAST_USE), used);
    }
    let at = (ax << 2).wrapping_add(RESOURCE_POINTERS);
    let (di, es) = (c.word(at), c.word(at.wrapping_add(2)));
    c.run(0xc15b, 8)?;
    c.set_reg(SI, ax);
    let s = c.shl1(W, ax, 2);
    let s = c.add(W, s, RESOURCE_POINTERS);
    c.set_reg(SI, s);
    c.set_reg(DI, di);
    c.set_sreg(ES, es);
    c.set_reg(BX, es);
    c.or(W, es, es);
    if es != 0 {
        // In memory: its palette again, if it has one.
        let small = di <= 2;
        c.run(0xc16d, if small { 2 } else { 3 })?;
        c.cmp(W, di, 2);
        if !small {
            c.call(code::APPLY_SPRITE_SHEET_PALETTE, 0xc175)?;
            c.run(0xc175, 1)?;
        }
    } else {
        c.run(0xc177, 6)?;
        for r in [CX, DX, BP] {
            let v = c.reg(r);
            c.push(v);
        }
        c.push(s);
        c.set_reg(SI, ax);
        c.call(code::LOAD_RESOURCE_TO, 0xc180)?;
        let di = c.reg(DI);
        let first = c.word_in(ES, di);
        let small = first <= 2;
        c.run(0xc180, if small { 2 } else { 3 })?;
        c.cmp(W, first, 2);
        if !small {
            c.call(code::APPLY_SPRITE_SHEET_PALETTE, 0xc189)?;
        }
        c.run(0xc189, 6)?;
        let slot = c.pop();
        c.set_reg(SI, slot);
        let di = c.reg(DI);
        let data = c.word_in(ES, di);
        c.set_reg(DI, data);
        let cx = c.reg(CX);
        let cx = c.sub(W, cx, data);
        c.set_reg(CX, cx);
        c.set_word(slot, data);
        let es = c.sreg(ES);
        c.set_word(slot.wrapping_add(2), es);
        let id = c.word(g::RESOURCE_ID);
        c.set_reg(AX, id);
        c.interp(0xc197, 0xc19b)?; // the driver: a new sheet
        c.run(0xc19b, 3)?;
        for r in [BP, DX, CX] {
            let v = c.pop();
            c.set_reg(r, v);
        }
    }
    c.run(0xc19e, 6)?;
    let (di, es) = (c.reg(DI), c.sreg(ES));
    c.set_word(g::SPRITE_SHEET, di);
    c.set_word(g::SPRITE_SHEET + 2, es);
    for r in [DI, SI, BX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// `open_resource_by_index` (C13E).
pub fn open_resource_by_index(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc13e)?;
    open(&mut Ctx::new(m))
}

/// `open_onmap_resource` (C13B): the on-map resource (25h, or what the
/// game patched into this instruction).
pub fn open_onmap_resource(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc13b)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let resource = c.code_word(code::ONMAP_RESOURCE_IMMEDIATE);
    c.set_reg(AX, resource);
    open(&mut c)
}

/// `evaluate_condition_ax` (A396): evaluates condition AX of the CONDIT
/// script: operands combined left to right by operators below 80h
/// (`evaluate_operator_bx_on_dx_and_ax`); an operator from 80h up closes
/// a term (kept on the stack with it) and starts the next; FFh ends; then
/// the terms are combined left to right by their operators. Exit: DX =
/// the result, ZF set if false.
pub fn evaluate_condition_ax(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa396)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let sp = c.reg(SP);
    let sp = c.sub(W, sp, 0x32);
    c.set_reg(SP, sp);
    c.set_reg(BP, sp);
    let ax = c.reg(AX);
    let index = c.shl1(W, ax, 1);
    c.set_reg(AX, index);
    let (table, seg) = (c.word(g::CONDITIONS), c.word(g::CONDITIONS + 2));
    c.set_sreg(ES, seg);
    let at = c.add(W, table, index);
    let script = c.word_in(ES, at.wrapping_sub(2));
    c.set_reg(SI, script);
    c.call(code::READ_CONDIT_OPERAND, 0xa3aa)?;
    'term: loop {
        c.run(0xa3aa, 1)?;
        let v = c.reg(AX);
        c.set_reg(DX, v);
        loop {
            let si = c.reg(SI);
            let op = c.byte_in(ES, si);
            if op == 0xff {
                c.run(0xa3ac, 3)?;
                c.lods(B, ES);
                c.cmp(B, 0xff, 0xff);
                break 'term;
            }
            if op & 0x80 != 0 {
                // Close the term: [BP] = its value, [BP+2] = the operator.
                c.run(0xa3ac, 10)?;
                c.lods(B, ES);
                c.cmp(B, op as u16, 0xff);
                c.test(B, op as u16, 0x80);
                let (bp, dx, ax) = (c.reg(BP), c.reg(DX), c.reg(AX));
                c.set_word_in(SS, bp, dx);
                c.set_word_in(SS, bp.wrapping_add(2), ax);
                let bp = c.add(W, bp, 4);
                c.set_reg(BP, bp);
                c.call(code::READ_CONDIT_OPERAND, 0xa3aa)?;
                continue 'term;
            }
            c.run(0xa3ac, 7)?;
            c.lods(B, ES);
            c.cmp(B, op as u16, 0xff);
            c.test(B, op as u16, 0x80);
            c.set_reg8(super::r8::BL, op);
            c.call(code::READ_CONDIT_OPERAND, 0xa3bb)?;
            c.run(0xa3bb, 1)?;
            c.call(code::EVALUATE_OPERATOR, 0xa3be)?;
            c.run(0xa3be, 1)?; // jmp A3ACh
        }
    }

    // A3CB: combine the terms.
    let (sp, bp) = (c.reg(SP), c.reg(BP));
    if sp == bp {
        c.run(0xa3cb, 6)?;
        c.set_reg(SI, sp);
        c.cmp(W, sp, bp);
        return finish(&mut c);
    }
    c.run(0xa3cb, 10)?;
    c.set_reg(SI, sp);
    c.cmp(W, sp, bp);
    let dx = c.reg(DX);
    c.set_word_in(SS, bp, dx);
    let first = c.lods(W, DS);
    c.set_reg(DX, first);
    loop {
        let op = c.lods(W, DS);
        c.set_reg(BX, op);
        c.lods(W, DS);
        c.call(code::EVALUATE_OPERATOR, 0xa3de)?;
        let (si, bp) = (c.reg(SI), c.reg(BP));
        c.run(0xa3de, if si < bp { 6 } else { 5 })?;
        c.cmp(W, si, bp);
        if si >= bp {
            return finish(&mut c);
        }
    }
}

/// `add sp, 32h; or dx, dx; ret` (A3E2), counted by the caller.
fn finish(c: &mut Ctx) -> Result<(), Bail> {
    let sp = c.reg(SP);
    let sp = c.add(W, sp, 0x32);
    c.set_reg(SP, sp);
    let dx = c.reg(DX);
    c.or(W, dx, dx);
    c.ret()
}

/// `loc_0ce53`: with bit 2 of [3403h], resources 63h..68h opened (loaded)
/// ahead.
pub fn preload_resources_63_to_68(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xce53)?;
    let mut c = Ctx::new(m);
    let flags = c.byte(PRELOAD_FLAGS);
    if flags & 4 == 0 {
        c.run(start, 3)?;
        c.test(B, flags as u16, 4);
        return c.ret();
    }
    c.run(start, 6)?;
    c.test(B, flags as u16, 4);
    let mut n = 2u16;
    loop {
        c.push(n);
        let r = c.add(W, n, 0x61);
        c.set_reg(AX, r);
        c.call(code::OPEN_RESOURCE_BY_INDEX, 0xce64)?;
        let next = n + 1;
        c.run(0xce64, 4 + if next < 8 { 3 } else { 1 })?;
        c.pop();
        let next = c.inc(W, n);
        c.cmp(W, next, 8);
        c.set_reg(AX, next);
        if next >= 8 {
            return c.ret();
        }
        n = next;
    }
}

/// Bit 2: preload resources 63h..68h.
const PRELOAD_FLAGS: u16 = 0x3403;
