//! The condition scripts (CONDIT): operands and operators.
//!
//! An operand in the script (ES:SI) is a type byte, then:
//! - below 80h: a game variable, the byte after it its offset in the data
//!   segment; type 1 reads a byte variable, others a word;
//! - 80h: a byte constant;
//! - above 80h: a word constant.

use super::pure;
use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BX, DX, ES, SI},
    rec::Bail,
};

/// `read_condit_operand_into_ax` (A30B): AX = the operand at ES:SI, SI past
/// it. BX kept. Flags: from `cmp al, 1` (word variables), `xor ah, ah`
/// (byte variables and constants) or `cmp al, 80h` (word constants).
pub fn read_condit_operand_into_ax(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa30b)?;
    let mut c = Ctx::new(m);
    c.check(start, 13)?;
    let kind = c.lods(B, ES) as u8;
    if kind < 0x80 {
        let bx = c.reg(BX);
        c.push(bx);
        let si = c.reg(SI);
        let var = c.byte_in(ES, si) as u16;
        c.set_reg(SI, si.wrapping_add(1));
        c.set_reg(BX, var);
        if kind == 1 {
            let v = c.byte(var);
            c.set_reg8(r8::AL, v);
            let zero = pure::xor(B, 0, 0);
            c.set_reg8(r8::AH, zero as u8);
            c.count(13);
        } else {
            let v = c.word(var);
            c.set_reg(AX, v);
            c.count(12);
        }
        let bx = c.pop();
        c.set_reg(BX, bx);
    } else if kind == 0x80 {
        c.lods(B, ES);
        let zero = pure::xor(B, 0, 0);
        c.set_reg8(r8::AH, zero as u8);
        c.count(7);
    } else {
        c.lods(W, ES);
        c.count(6);
    }
    c.ret()
}

/// The operators of `evaluate_operator_bx_on_dx_and_ax`, by the code they
/// jump to (the table at CS:A376 is indexed by the operator × 2).
#[derive(Clone, Copy)]
enum Operator {
    Equal,
    Below,
    Above,
    NotEqual,
    LessOrEqual,
    GreaterOrEqual,
    Add,
    Sub,
    And,
    Or,
    False,
}

impl Operator {
    fn at(target: u16) -> Option<Operator> {
        Some(match target {
            0xa348 => Operator::Equal,
            0xa34f => Operator::Below,
            0xa356 => Operator::Above,
            0xa35d => Operator::NotEqual,
            0xa364 => Operator::LessOrEqual,
            0xa36b => Operator::GreaterOrEqual,
            0xa33c => Operator::Add,
            0xa33f => Operator::Sub,
            0xa342 => Operator::And,
            0xa345 => Operator::Or,
            0xa36f => Operator::False,
            _ => return None,
        })
    }
}

/// The operator table, in the code segment.
const OPERATORS: u16 = 0xa376;

/// `evaluate_operator_bx_on_dx_and_ax` (A334): DX = DX (operator BX) AX.
/// Comparisons give FFFFh (true) or 0; comparisons `<`/`>` are unsigned,
/// `<=`/`>=` signed. Exit: BX = the operator × 2 (`and bx, 1Fh`); flags
/// from the operation, or the `cmp` when true, or `xor dx, dx` when false.
pub fn evaluate_operator_bx_on_dx_and_ax(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xa334)?;
    let mut c = Ctx::new(m);
    let index = c.reg(BX) & 0x1f;
    let target = c.code_word(OPERATORS.wrapping_add(index));
    let Some(op) = Operator::at(target) else {
        return Err(Bail); // not an operator: the interpreter goes there
    };
    let (dx, ax) = (c.reg(DX), c.reg(AX));
    let compared = match op {
        Operator::Equal => Some(dx == ax),
        Operator::Below => Some(dx < ax),
        Operator::Above => Some(dx > ax),
        Operator::NotEqual => Some(dx != ax),
        Operator::LessOrEqual => Some((dx as i16) <= (ax as i16)),
        Operator::GreaterOrEqual => Some((dx as i16) >= (ax as i16)),
        _ => None,
    };
    c.run(start, if compared.is_some() { 6 } else { 4 })?;
    let bx = c.reg(BX);
    c.and(W, bx, 0x1f);
    c.set_reg(BX, index);
    let result = match (op, compared) {
        (_, Some(t)) => {
            c.cmp(W, dx, ax);
            if t { 0xffff } else { c.xor(W, dx, dx) }
        }
        (Operator::Add, _) => c.add(W, dx, ax),
        (Operator::Sub, _) => c.sub(W, dx, ax),
        (Operator::And, _) => c.and(W, dx, ax),
        (Operator::Or, _) => c.or(W, dx, ax),
        _ => c.xor(W, dx, dx),
    };
    c.set_reg(DX, result);
    c.ret()
}
