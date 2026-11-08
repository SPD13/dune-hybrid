//! Services: EMS (INT 67h), the XMS driver, DOS file calls.

use super::{B, Ctx, W, entered, globals as g};
use pc::{
    Machine,
    cpu::{AX, BX, CF, CX, DI, DX, SS},
    rec::Bail,
};

/// Code-segment words.
mod cs {
    /// The EMS handle the game allocated.
    pub const EMS_HANDLE: u16 = 0xed3a;
    /// The XMS block handle.
    pub const XMS_HANDLE: u16 = 0xee8a;
}

/// After `int 67h`: `cmp ah, 80h; cmc; ret`: CF set if the call succeeded
/// (AH below 80h, the error codes).
fn ems_result(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xed47, 3)?;
    let ah = c.reg8(super::r8::AH);
    c.cmp(B, ah as u16, 0x80);
    let cf = c.flag(CF);
    c.set_flag(CF, !cf);
    c.ret()
}

/// `get_ems_emm_handle` (ED40): EMS function AH on the game's handle.
pub fn get_ems_emm_handle(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xed40)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let handle = c.code_word(cs::EMS_HANDLE);
    c.set_reg(DX, handle);
    c.interp(0xed45, 0xed47)?;
    ems_result(&mut c)
}

/// `call_ems_func` (ED45): EMS function AH.
pub fn call_ems_func(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xed45)?;
    let mut c = Ctx::new(m);
    c.interp(0xed45, 0xed47)?;
    ems_result(&mut c)
}

/// The XMS driver call (EF22); ZF set if it returned AX = 1 (success).
fn xms_call(c: &mut Ctx) -> Result<(), Bail> {
    c.interp(0xef22, 0xef27)?;
    c.run(0xef27, 2)?;
    let ax = c.reg(AX);
    c.cmp(W, ax, 1);
    c.ret()
}

/// `call_xms_driver_func` (EF22): XMS function AH.
pub fn call_xms_driver_func(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xef22)?;
    xms_call(&mut Ctx::new(m))
}

/// `call_xms_func_on_block` (EF2B): XMS function AH on the game's block.
pub fn call_xms_func_on_block(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xef2b)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let handle = c.code_word(cs::XMS_HANDLE);
    c.set_reg(DX, handle);
    xms_call(&mut c)
}

/// `seek_dune_dat_offset_dxax` (F2D6): seeks DUNE.DAT to DX:AX from its
/// start (DOS 42h). Exit: as DOS leaves it (DX:AX = the position, CF on
/// error); BX = the file handle, CX kept.
pub fn seek_dune_dat_offset_dxax(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf2d6)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let (ax, cx, dx) = (c.reg(AX), c.reg(CX), c.reg(DX));
    c.push(cx);
    let handle = c.word_in(SS, g::DUNE_DAT_HANDLE);
    c.set_reg(BX, handle);
    c.set_reg(CX, dx);
    c.set_reg(DX, ax);
    c.set_reg(AX, 0x4200);
    c.interp(0xf2e3, 0xf2e5)?;
    c.run(0xf2e5, 2)?;
    let cx = c.pop();
    c.set_reg(CX, cx);
    c.ret()
}

/// `parse_cmd_is_end_of_arg` (E56B): the next character of the command
/// line (DS:SI, ending at BP), upper-cased; CR (0Dh) past the end. Flags
/// from `cmp al, 20h`: ZF for a space, CF for the end or a control
/// character.
pub fn parse_cmd_is_end_of_arg(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use super::r8::AL;
    use pc::cpu::{BP, DS, SI};
    entered(start, 0xe56b)?;
    let mut c = Ctx::new(m);
    c.check(start, 9)?;
    c.set_reg8(AL, 0x0d);
    let (si, end) = (c.reg(SI), c.reg(BP));
    c.cmp(W, si, end);
    let mut n = 3 + 2; // mov; cmp; jae; cmp al, 20h; ret
    if si < end {
        let mut ch = c.lods(B, DS) as u8;
        c.cmp(B, ch as u16, 0x61);
        n += 3; // lodsb; cmp; jb
        if ch >= 0x61 {
            ch = c.and(B, ch as u16, 0xdf) as u8;
            c.set_reg8(AL, ch);
            n += 1;
        }
    }
    let ch = c.reg8(AL);
    c.cmp(B, ch as u16, 0x20);
    c.count(n);
    c.ret()
}

/// `res_locate_in_lookup_table` (F3A7): finds resource offset DL:AX (24
/// bits) in the two-level lookup table: the first entry (5 bytes: a
/// pointer, then the 24-bit offset it starts at) not below it, then in
/// its page the first 10-byte entry not below it. Exit: ES:DI = that
/// entry; flags from the last comparison.
pub fn res_locate_in_lookup_table(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf3a7)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let table = c.word_in(SS, g::RES_LOOKUP_TABLE);
    let seg = c.word_in(SS, g::RES_LOOKUP_TABLE + 2);
    c.set_sreg(pc::cpu::ES, seg);
    let di = c.sub(W, table, 5);
    c.set_reg(DI, di);
    let page = find(&mut c, 0xf3af, 5, 4, 2)?;
    c.run(0xf3be, 2)?;
    let first = c.word_in(pc::cpu::ES, page);
    let di = c.sub(W, first, 0x0a);
    c.set_reg(DI, di);
    find(&mut c, 0xf3c4, 0x0a, 2, 0)?;
    c.run(0xf3d2, 1)?;
    c.ret()
}

/// One level of [`res_locate_in_lookup_table`]: from DI, step by `size`
/// while DL:AX is above the entry's offset (high byte at `+high`, low
/// word at `+low`). Returns DI, the entry found.
fn find(c: &mut Ctx, at: u16, size: u16, high: u16, low: u16) -> Result<u16, Bail> {
    use super::r8::DL;
    use pc::cpu::ES;
    let (dl, ax) = (c.reg8(DL), c.reg(AX));
    loop {
        let di = c.reg(DI).wrapping_add(size);
        let entry_high = c.byte_in(ES, di.wrapping_add(high));
        let entry_low = c.word_in(ES, di.wrapping_add(low));
        let same_high = dl == entry_high;
        c.run(at, if same_high { 5 } else { 4 })?; // add; cmp; jne; [cmp]; ja
        c.add(W, di.wrapping_sub(size), size);
        c.set_reg(DI, di);
        c.cmp(B, dl as u16, entry_high as u16);
        let above = if same_high {
            c.cmp(W, ax, entry_low);
            ax > entry_low
        } else {
            dl > entry_high
        };
        if !above {
            return Ok(di);
        }
    }
}
