//! Services: EMS (INT 67h), the XMS driver, DOS file calls.

use super::pure;
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

/// `ems_memory_func_2` (EE02) and `ems_memory_func_1` (EE46): CX bytes
/// copied between DS:SI / ES:DI and the EMS memory at BX (in 1 KB
/// units: the 16 KB page BX / 16 mapped into page 0 by
/// `get_ems_emm_handle`, the offset (BX mod 16) * 400h), page by page.
/// EE02 writes into EMS (ES the page frame, from DS:SI), EE46 reads from
/// it (DS the page frame, to ES:DI). CF set if a mapping failed (clear
/// at the end).
pub fn ems_copy(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{CS, DS, ES, SI};
    if start != 0xee02 && start != 0xee46 {
        return Err(Bail);
    }
    let write = start == 0xee02;
    let at = |o: u16| start + o;
    let (ptr, seg) = if write { (DI, ES) } else { (SI, DS) };
    let mut c = Ctx::new(m);
    c.run(start, 10)?;
    let bx = c.reg(BX);
    c.set_reg(AX, bx);
    let a = c.shl1(W, bx, 2);
    let a = a.rotate_left(8);
    c.set_reg(AX, a);
    let a = c.and(W, a, 0x3c00);
    c.set_reg(AX, a);
    c.set_reg(ptr, a);
    let b = c.shr1(W, bx, 4);
    c.set_reg(BX, b);
    loop {
        c.run(at(0x15), 2)?;
        c.set_reg(AX, 0x4400);
        c.call(0xed40, at(0x1b))?;
        if c.flag(CF) {
            c.run(at(0x1b), 2)?;
            return c.ret();
        }
        let (cx, p) = (c.reg(CX), c.reg(ptr));
        let room = 0x4000u16.wrapping_sub(p);
        let below = room < cx;
        let n = if below { room } else { cx };
        c.run(at(0x1b), 7 + !below as u32 + 3)?;
        let frame = c.word_in(CS, 0xed3c);
        c.set_sreg(seg, frame);
        c.set_reg(DX, cx);
        let r = c.sub(W, 0x4000, p);
        c.set_reg(CX, r);
        c.cmp(W, r, cx);
        if !below {
            c.set_reg(CX, cx);
        }
        let left = c.sub(W, cx, n);
        c.set_reg(DX, left);
        let h = c.shr1(W, n, 1);
        c.set_reg(CX, h);
        c.rep(0xa5);
        c.run(at(0x35), 2)?;
        let cl = c.adc(B, 0, 0);
        c.set_reg(CX, cl);
        c.rep(0xa4);
        c.run(at(0x39), 2 + if left == 0 { 2 } else { 3 })?;
        c.set_reg(CX, left);
        if left == 0 {
            c.set_flag(CF, false);
            return c.ret();
        }
        let b = c.reg(BX);
        let b = c.inc(W, b);
        c.set_reg(BX, b);
        let p = c.reg(ptr);
        c.xor(W, p, p);
        c.set_reg(ptr, 0);
    }
}

/// `xms_memory_func_1` (ECEC): CX bytes (rounded up to words) from
/// ES:DI to the extended memory at BX * 400h + 100000h, by BIOS block
/// move (INT 15h, AH 87h) through the descriptor table at CS:EC6Ch
/// (source and destination limits, 24-bit bases, access 92h).
pub fn xms_move(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{CS, ES, SI};
    entered(start, 0xecec)?;
    let mut c = Ctx::new(m);
    c.run(0xecec, 10 + 12 + 19)?;
    c.set_reg(SI, 0xec6c);
    let cx = c.reg(CX);
    let cx = c.inc(W, cx);
    let words = c.shr1(W, cx, 1);
    c.push(words);
    let bytes = c.shl1(W, words, 1);
    c.set_word_in(CS, 0xec7c, bytes);
    c.set_word_in(CS, 0xec84, bytes);
    let mut ax = c.sreg(ES);
    let mut dx = c.xor(W, 0, 0);
    for _ in 0..4 {
        ax = c.shl1(W, ax, 1);
        dx = c.rcl1(W, dx);
    }
    let di = c.reg(DI);
    let ax = c.add(W, ax, di);
    let dx = c.adc(W, dx, 0);
    c.set_word_in(CS, 0xec86, ax);
    let dx = (dx & 0xff) | 0x9200;
    c.set_word_in(CS, 0xec88, dx);
    c.xor(B, dx & 0xff, dx & 0xff);
    let bx = c.reg(BX);
    let mut dl = (bx >> 8) & 0xff;
    let mut bx = (bx & 0xff) << 8;
    for _ in 0..2 {
        bx = c.shl1(W, bx, 1);
        dl = c.rcl1(B, dl);
    }
    let dl = c.add(B, dl, 0x10);
    c.set_reg(BX, bx);
    let dx = 0x9200 | dl;
    c.set_reg(DX, dx);
    c.set_word_in(CS, 0xec7e, bx);
    c.set_word_in(CS, 0xec80, dx);
    let cs = c.sreg(CS);
    c.push(cs);
    let v = c.pop();
    c.set_sreg(ES, v);
    let v = c.pop();
    c.set_reg(CX, v);
    c.set_reg(AX, ax);
    c.set_reg8(super::r8::AH, 0x87);
    c.interp(0xed37, 0xed39)?;
    c.run(0xed39, 1)?;
    c.ret()
}

/// `map_ems_for_midi_audio` (EDB9): with EMS ([2944h] low bits), three
/// pages allocated (AH 43h; the handle kept at CS:ED3Eh) and mapped as
/// pages 1 to 3 of the frame, which becomes the MIDI buffer ([0DBB8h]
/// the frame's segment, [0DBB6h] 4000h). A failed mapping frees the
/// pages again (AH 45h, run by the original).
pub fn map_ems_for_midi_audio(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::CS;
    entered(start, 0xedb9)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0x2944);
    if f & 0x0f == 0 {
        c.run(0xedb9, 3)?;
        c.test(B, f as u16, 0x0f);
        return c.ret();
    }
    c.run(0xedb9, 5)?;
    c.test(B, f as u16, 0x0f);
    c.set_reg(BX, 3);
    c.set_reg8(super::r8::AH, 0x43);
    c.call(0xed45, 0xedc8)?;
    if c.flag(CF) {
        c.run(0xedc8, 2)?;
        return c.ret();
    }
    c.run(0xedc8, 5)?;
    let dx = c.reg(DX);
    c.set_word_in(CS, 0xed3e, dx);
    c.set_reg(AX, 0x4401);
    let bx = c.reg(BX);
    c.xor(W, bx, bx);
    c.set_reg(BX, 0);
    c.call(0xed45, 0xedd7)?;
    for (at, ax, bx, ret) in [(0xedd7u16, 0x4402u16, 1u16, 0xede2u16), (0xede2, 0x4403, 2, 0xeded)] {
        if c.flag(CF) {
            c.run(at, 1 + 2)?;
            c.set_reg8(super::r8::AH, 0x45);
            return c.jump(0xed45);
        }
        c.run(at, 4)?;
        c.set_reg(AX, ax);
        c.set_reg(BX, bx);
        c.call(0xed45, ret)?;
    }
    if c.flag(CF) {
        c.run(0xeded, 1 + 2)?;
        c.set_reg8(super::r8::AH, 0x45);
        return c.jump(0xed45);
    }
    c.run(0xeded, 5)?;
    let seg = c.word_in(CS, 0xed3c);
    c.set_reg(AX, seg);
    c.set_word(0xdbb8, seg);
    c.set_word(0xdbb6, 0x4000);
    c.ret()
}

/// `init_extended_memory_allocator` (EA7B): by the memory kinds found
/// ([2943h]): himem (`initialize_himem_sys`), EMS (`loc_0ed4c`); with
/// any extended memory a table of [39A9h] + 2 * B9h words allocated
/// (`bump_alloc_get_addr_in_di_and_bump`, its far pointer at [0CE6Ah])
/// and cleared, [0CE70h] B9h.
pub fn init_extended_memory_allocator(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::ES;
    entered(start, 0xea7b)?;
    let mut c = Ctx::new(m);
    let f = c.byte(0x2943);
    c.run(0xea7b, 2 + (f & 0x80 != 0) as u32)?;
    if f & 0x80 != 0 {
        c.call(0xeea0, 0xea85)?;
    }
    let f = c.byte(0x2943);
    c.run(0xea85, 2 + (f & 0x48 != 0) as u32)?;
    if f & 0x48 != 0 {
        c.call(0xed4c, 0xea8f)?;
    }
    let f = c.byte(0x2943);
    if f & 0xe8 == 0 {
        c.run(0xea8f, 3)?;
        return c.ret();
    }
    c.run(0xea8f, 11)?;
    c.set_reg(DI, 0xce6a);
    let n = c.word(0x39a9);
    c.set_reg(AX, 0xb9);
    c.set_byte(0xce70, 0xb9);
    let n = pure::add(W, n, 0xb9);
    let n = pure::add(W, n, 0xb9);
    c.push(n);
    let b = pure::shl1(W, n, 1);
    c.set_reg(CX, b);
    c.call(0xf0f6, 0xeaad)?;
    c.run(0xeaad, 4)?;
    let (di, es) = (c.word(0xce6a), c.word(0xce6c));
    c.set_reg(DI, di);
    c.set_sreg(ES, es);
    let n = c.pop();
    c.set_reg(CX, n);
    c.set_reg(AX, 0);
    c.rep(0xab);
    c.run(0xeab6, 1)?;
    c.ret()
}

/// `install_interrupt_handlers` (E913): [0CE73h] flipped (installed or
/// not), then for each pair at 2913h (vector, code-segment slot; up to
/// a negative word) the vector and the far pointer kept in the code
/// segment swapped, with interrupts off.
pub fn install_interrupt_handlers(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{CS, DS, ES, IF, SI};
    entered(start, 0xe913)?;
    let mut c = Ctx::new(m);
    c.run(0xe913, 5)?;
    let v = c.byte(0xce73);
    let v = c.xor(B, v as u16, 0xff);
    c.set_byte(0xce73, v as u8);
    c.set_reg(SI, 0x2913);
    let f = c.m.cpu.flags();
    c.push(f);
    c.set_flag(IF, false);
    let mut ax = c.lods(W, DS);
    loop {
        c.run(0xe91e, 19)?;
        let slot = c.lods(W, DS);
        c.set_reg(DI, slot);
        c.set_reg(AX, ax);
        let si = c.reg(SI);
        c.push(si);
        let vec = c.shl1(W, ax, 2);
        c.set_reg(SI, vec);
        c.xor(W, 0, 0);
        c.set_sreg(ES, 0);
        let mut last = 0;
        for k in [0u16, 2] {
            let mine = c.word_in(CS, slot.wrapping_add(k));
            let cur = c.word_in(ES, vec.wrapping_add(k));
            c.set_word_in(ES, vec.wrapping_add(k), mine);
            c.set_word_in(CS, slot.wrapping_add(k), cur);
            last = cur;
        }
        c.set_reg(AX, last);
        let si = c.pop();
        c.set_reg(SI, si);
        ax = c.lods(W, DS);
        c.or(W, ax, ax);
        if ax & 0x8000 != 0 {
            break;
        }
    }
    c.run(0xe948, 1)?;
    let f = c.pop();
    c.m.cpu.set_flags(f);
    // Interrupts back on (CLI at E91Ch): one that came meanwhile is taken
    // before the RET.
    c.run(0xe949, 1)?;
    c.ret()
}

/// `alloc_extended_memory` (EBE3): block AX of the extended-memory
/// table (far [0CE6Ah], index [39A9h] + AX): none (0) returns ZF set.
/// Else its use stamp ([si+172h]) set from the counter CS:EA75h, and
/// the chain of 1 KB pages from its first entry copied to ES:DI page by
/// page (`loc_0ec46`, CX bytes, runs of consecutive pages merged; a
/// negative entry ends the chain with its low bits). CX the bytes
/// copied, CF from the last copy inverted; DS, DX kept.
pub fn alloc_extended_memory(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{CS, DS, ES, SI};
    entered(start, 0xebe3)?;
    let mut c = Ctx::new(m);
    let base = c.word(0x39a9);
    let seg = c.word(0xce6c);
    let ax0 = c.reg(AX);
    let s = base.wrapping_add(ax0).wrapping_shl(1);
    let first = c.word_at(seg, s);
    c.run(0xebe3, 9 + if first == 0 { 3 } else { 8 })?;
    let dx = c.reg(DX);
    c.push(dx);
    let ds = c.sreg(DS);
    c.push(ds);
    c.set_reg(SI, base);
    c.set_sreg(DS, seg);
    let s = c.add(W, base, ax0);
    let s = c.shl1(W, s, 1);
    c.set_reg(SI, s);
    c.set_reg(AX, first);
    c.or(W, first, first);
    if first == 0 {
        return pop_ds_dx_ec43(&mut c);
    }
    c.set_reg(BX, first);
    let t = c.word_in(CS, 0xea75);
    let t = c.inc(W, t);
    c.set_reg(AX, t);
    c.set_word(s.wrapping_add(0x172), t);
    c.set_word_in(CS, 0xea75, t);
    c.set_reg(AX, first);
    let di = c.reg(DI);
    c.push(di);
    let es = c.sreg(ES);
    c.push(es);
    let mut ax = first;
    'chain: loop {
        c.run(0xec0a, 3)?;
        c.set_reg(BX, ax);
        let mut dx = ax;
        c.set_reg(DX, dx);
        let mut cx = c.xor(W, 0, 0);
        c.set_reg(CX, 0);
        loop {
            if ax & 0x8000 != 0 {
                c.run(0xec10, 3 + 3)?;
                cx = c.add(W, cx, 0x400);
                c.set_reg(CX, cx);
                ax = c.shl1(W, ax, 1);
                c.set_reg(AX, ax);
                break 'chain;
            }
            let si = (ax << 1).wrapping_sub(2);
            let v = c.word(si);
            let same = v == dx.wrapping_add(1);
            c.run(0xec10, 3 + 6 + if same { 0 } else { 2 + 3 })?;
            cx = c.add(W, cx, 0x400);
            c.set_reg(CX, cx);
            let sh = c.shl1(W, ax, 1);
            c.set_reg(AX, sh);
            c.set_reg(SI, sh);
            let si = c.sub(W, sh, 2);
            c.set_reg(SI, si);
            dx = c.inc(W, dx);
            c.set_reg(DX, dx);
            c.set_reg(AX, v);
            c.cmp(W, v, dx);
            ax = v;
            if same {
                continue;
            }
            ax = c.shl1(W, v, 1);
            c.set_reg(AX, ax);
            if v & 0x8000 != 0 {
                break 'chain;
            }
            ax = c.shr1(W, ax, 1);
            c.set_reg(AX, ax);
            c.push(ax);
            c.call(0xec46, 0xec2e)?;
            let cf = c.flag(CF);
            c.run(0xec2e, 2 + !cf as u32)?;
            ax = c.pop();
            c.set_reg(AX, ax);
            if cf {
                return end_ec3a(&mut c);
            }
            continue 'chain;
        }
    }
    let a = c.m.cpu.shift(7, W, ax as u32, 1) as u16;
    c.set_reg(AX, a);
    let cx = c.reg(CX);
    let cx = c.add(W, cx, a);
    c.set_reg(CX, cx);
    c.call(0xec46, 0xec3a)?;
    end_ec3a(&mut c)
}

/// From EC3Ah: CX the bytes written (DI past the start), ES:DI back;
/// CF inverted; then DS and DX popped.
fn end_ec3a(c: &mut Ctx) -> Result<(), Bail> {
    use pc::cpu::ES;
    c.run(0xec3a, 10)?;
    let di = c.reg(DI);
    c.set_reg(CX, di);
    let v = c.pop();
    c.set_sreg(ES, v);
    let d0 = c.pop();
    c.set_reg(DI, d0);
    let f = c.m.cpu.flags();
    c.push(f);
    let n = c.sub(W, di, d0);
    c.set_reg(CX, n);
    let f = c.pop();
    c.m.cpu.set_flags(f);
    let cf = c.flag(CF);
    c.set_flag(CF, !cf);
    pop_ds_dx_ec43(c)
}

fn pop_ds_dx_ec43(c: &mut Ctx) -> Result<(), Bail> {
    use pc::cpu::DS;
    let v = c.pop();
    c.set_sreg(DS, v);
    let v = c.pop();
    c.set_reg(DX, v);
    c.ret()
}

/// `loc_0ed4c`: EMS set up: the page frame (AH 41h, kept at CS:ED3Ch),
/// at least 3 free pages (AH 42h) for the MIDI buffer
/// (`map_ems_for_midi_audio`); then, unless EMS is only for music
/// ([2943h] bit 3), at least 4 free pages: up to 80h of them allocated
/// (AH 43h, the handle at CS:ED3Ah), [39A9h] their count * 16, and the
/// EMS copy routines installed (CS:EA77h `ems_memory_func_2`, CS:EA79h
/// `ems_memory_func_1`). Any failure clears bits 3 and 6 of [2943h].
pub fn ems_init(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{CS, SI};
    entered(start, 0xed4c)?;
    let mut c = Ctx::new(m);
    c.run(0xed4c, 2)?;
    c.set_reg8(super::r8::AH, 0x41);
    c.call(0xed45, 0xed51)?;
    if c.flag(CF) {
        c.run(0xed51, 1)?;
        return fail_edb3(&mut c);
    }
    c.run(0xed51, 5)?;
    let bx = c.reg(BX);
    c.set_word_in(CS, 0xed3c, bx);
    c.set_reg8(super::r8::AH, 0x42);
    c.xor(W, bx, bx);
    c.set_reg(BX, 0);
    c.call(0xed45, 0xed5f)?;
    let bx = c.reg(BX);
    if c.flag(CF) {
        c.run(0xed5f, 1)?;
        return fail_edb3(&mut c);
    }
    if bx < 3 {
        c.run(0xed5f, 3)?;
        c.cmp(W, bx, 3);
        return fail_edb3(&mut c);
    }
    c.run(0xed5f, 4)?;
    c.cmp(W, bx, 3);
    c.call(0xedb9, 0xed69)?;
    let f = c.byte(0x2943);
    if f & 8 != 0 {
        c.run(0xed69, 2)?;
        c.test(B, f as u16, 8);
        return fail_edb3(&mut c);
    }
    c.run(0xed69, 5)?;
    c.test(B, f as u16, 8);
    c.set_reg8(super::r8::AH, 0x42);
    let bx = c.reg(BX);
    c.xor(W, bx, bx);
    c.set_reg(BX, 0);
    c.call(0xed45, 0xed77)?;
    let bx = c.reg(BX);
    if c.flag(CF) {
        c.run(0xed77, 1)?;
        return fail_edb3(&mut c);
    }
    if bx < 4 {
        c.run(0xed77, 3)?;
        c.cmp(W, bx, 4);
        return fail_edb3(&mut c);
    }
    let big = bx >= 0x80;
    let pages = if big { 0x80 } else { bx };
    let zero = pages << 1 == 0;
    c.run(0xed77, 5 + big as u32 + 3 + if zero { 0 } else { 6 })?;
    c.cmp(W, bx, 4);
    c.cmp(W, bx, 0x80);
    c.set_reg(BX, pages);
    c.set_reg(AX, pages);
    let a = c.shl1(W, pages, 1);
    c.set_reg(AX, a);
    if zero {
        return fail_edb3(&mut c);
    }
    let a = c.shl1(W, a, 3);
    c.set_reg(AX, a);
    c.set_word(0x39a9, a);
    c.set_reg8(super::r8::AH, 0x43);
    c.call(0xed45, 0xed9b)?;
    if c.flag(CF) {
        c.run(0xed9b, 1)?;
        return fail_edb3(&mut c);
    }
    c.run(0xed9b, 7)?;
    let dx = c.reg(DX);
    c.set_word_in(CS, 0xed3a, dx);
    c.set_reg(SI, 0xee46);
    c.set_reg(DI, 0xee02);
    c.set_word_in(CS, 0xea77, 0xee02);
    c.set_word_in(CS, 0xea79, 0xee46);
    c.ret()
}

/// EDB3h: bits 3 and 6 of [2943h] cleared (no EMS).
fn fail_edb3(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xedb3, 2)?;
    let f = c.byte(0x2943);
    let f = c.and(B, f as u16, 0xb7);
    c.set_byte(0x2943, f as u8);
    c.ret()
}

/// `uninitialize_memory_driver` (E8D5): the XMS block freed (AH 0Ah,
/// CS:EE8Ah), the EMS pages and the MIDI pages freed (AH 45h, the
/// handles at CS:ED3Ah and CS:ED3Eh), the timer back to its rate
/// (`loc_0e8a8`, AX 0), the DOS break flag restored (INT 21h AX 3301h,
/// DL [2941h]); with the handlers installed ([0CE73h]) they are put
/// back (`install_interrupt_handlers`).
pub fn uninitialize_memory_driver(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::CS;
    use super::r8;
    entered(start, 0xe8d5)?;
    let mut c = Ctx::new(m);
    let h = c.word_in(CS, 0xee8a);
    c.run(0xe8d5, 2 + 2 * (h != 0) as u32)?;
    c.cmp(W, h, 0);
    if h != 0 {
        c.set_reg8(r8::AH, 0x0a);
        c.call(0xef2b, 0xe8e2)?;
    }
    let h = c.word_in(CS, 0xed3a);
    c.run(0xe8e2, 2 + 2 * (h != 0) as u32)?;
    c.cmp(W, h, 0);
    if h != 0 {
        c.set_reg8(r8::AH, 0x45);
        c.call(0xed40, 0xe8ef)?;
    }
    let d = c.word_in(CS, 0xed3e);
    c.run(0xe8ef, 3 + 2 * (d != 0) as u32)?;
    c.set_reg(DX, d);
    c.or(W, d, d);
    if d != 0 {
        c.set_reg8(r8::AH, 0x45);
        c.call(0xed45, 0xe8fd)?;
    }
    c.run(0xe8fd, 2)?;
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    c.set_reg(AX, 0);
    c.call(0xe8a8, 0xe902)?;
    c.run(0xe902, 2)?;
    let dl = c.byte(0x2941);
    c.set_reg8(r8::DL, dl);
    c.set_reg(AX, 0x3301);
    c.interp(0xe909, 0xe90b)?;
    let f = c.byte(0xce73);
    c.run(0xe90b, 2 + (f == 0) as u32)?;
    c.cmp(B, f as u16, 0);
    if f == 0 {
        return c.ret();
    }
    c.jump(0xe913)
}

/// `bump_alloc_get_addr_in_di_and_bump` (F0F6; F0FFh skips the first
/// part): the far pointer of the free memory ([39B7h], ES:SI) stored
/// at DS:DI, then CX bytes taken from it: [39B9h] (its segment) up by
/// (CX + 15) / 16 paragraphs (AX). Past the limit [0CE68h] the game
/// ends with "out of memory" (`out_of_memory_error`, run by the
/// original).
pub fn bump_alloc(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{ES, SI};
    if start != 0xf0f6 && start != 0xf0ff {
        return Err(Bail);
    }
    let mut c = Ctx::new(m);
    if start == 0xf0f6 {
        c.run(0xf0f6, 3)?;
        let (s, e) = (c.word(0x39b7), c.word(0x39b9));
        c.set_reg(SI, s);
        c.set_sreg(ES, e);
        let di = c.reg(DI);
        c.set_word(di, s);
        c.set_word(di.wrapping_add(2), e);
    }
    let cx = c.reg(CX);
    let paras = ((cx as u32 + 0xf) >> 4) as u16;
    let top = c.word(0x39b9).wrapping_add(paras);
    let lim = c.word(0xce68);
    let over = top > lim;
    c.run(0xf0ff, 12 + if over { 0 } else { 1 })?;
    c.set_reg(AX, cx);
    let a = c.add(W, cx, 0xf);
    let a = c.rcr1(W, a);
    let a = c.shr1(W, a, 3);
    c.set_reg(AX, a);
    let v = c.word(0x39b9);
    let v = c.add(W, v, a);
    c.set_word(0x39b9, v);
    c.push(a);
    c.set_reg(AX, v);
    c.cmp(W, v, lim);
    let a = c.pop();
    c.set_reg(AX, a);
    if over {
        return c.jump(0xf131);
    }
    c.ret()
}

/// `memory_func_qq` (EAB7): resource AX ([0CE6Eh]; CX bytes at ES:DI)
/// kept in extended memory. In the page table (far [0CE6Ah], [39A9h]
/// entries) its slot (+ 2 * AX past the table; use stamp at +172h from
/// CS:EA75h) gets a chain of free pages: free entries (0) found from DI
/// on (wrapping to the start; none free: `loc_0eb74` frees one, or the
/// resource is dropped, `loc_0ebaa` from EB60h), consecutive ones linked
/// (each entry the next page + 1), the last entry the bytes left
/// (negative), each run copied (`loc_0ec59`, BX its first page, CX its
/// bytes) and marked as a run's end (bit 15). Frame [bp]: the table's
/// size in bytes, [bp+2] the bytes left, [bp+4] the run's first page.
/// Wrapped in `loc_0e270`/`loc_0e283`; DS, ES kept.
pub fn memory_func_qq(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{BP, CS, DS, ES, SI, SP, ZF};
    entered(start, 0xeab7)?;
    let mut c = Ctx::new(m);
    c.run(0xeab7, 1)?;
    c.call(0xe270, 0xeaba)?;
    c.run(0xeaba, 22)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.sreg(ES);
    c.push(es);
    let sp = c.reg(SP);
    let bp = c.sub(W, sp, 6);
    c.set_reg(SP, bp);
    c.set_reg(BP, bp);
    let ax = c.reg(AX);
    c.set_word(0xce6e, ax);
    let di = c.reg(DI);
    c.push(di);
    c.push(es);
    let (d, e) = (c.word(0xce6a), c.word(0xce6c));
    c.set_reg(DI, d);
    c.set_sreg(ES, e);
    c.set_reg(SI, ax);
    let n = c.word(0x39a9);
    c.set_reg(AX, n);
    let n2 = c.shl1(W, n, 1);
    c.set_reg(AX, n2);
    c.set_word_in(SS, bp, n2);
    let si = c.shl1(W, ax, 1);
    let si = c.add(W, si, n2);
    c.set_reg(SI, si);
    c.set_reg(DX, si);
    let t = c.word_in(CS, 0xea75);
    let t = c.inc(W, t);
    c.set_reg(AX, t);
    c.set_word_in(ES, si.wrapping_add(0x172), t);
    c.set_word_in(CS, 0xea75, t);
    let cx = c.reg(CX);
    c.set_word_in(SS, bp.wrapping_add(2), cx);
    let v = c.pop();
    c.set_sreg(DS, v);
    let v = c.pop();
    c.set_reg(SI, v);
    loop {
        let di = c.reg(DI);
        let lim = c.word_in(SS, bp);
        let below = di < lim;
        c.run(0xeaed, 2 + (!below) as u32)?;
        c.cmp(W, di, lim);
        if !below {
            c.xor(W, di, di);
            c.set_reg(DI, 0);
        }
        loop {
            c.run(0xeaf4, 5)?;
            let di = c.reg(DI);
            let lim = c.word_in(SS, bp);
            c.set_reg(CX, lim);
            let n = c.sub(W, lim, di);
            let n = c.shr1(W, n, 1);
            c.set_reg(CX, n);
            c.xor(W, 0, 0);
            c.set_reg(AX, 0);
            c.m.cpu.string_op(&mut c.m.hw, 0xaf, None, 0xf2);
            c.run(0xeaff, 1)?;
            if c.flag(ZF) {
                break;
            }
            c.run(0xeb01, 1)?;
            c.call(0xeb74, 0xeb04)?;
            c.run(0xeb04, 2)?;
            if !c.flag(CF) {
                return drop_eb60(&mut c, bp);
            }
            let di = c.reg(DI);
            c.xor(W, di, di);
            c.set_reg(DI, 0);
        }
        c.run(0xeb08, 7)?;
        let di = c.reg(DI);
        let mut di = c.sub(W, di, 2);
        c.set_reg(DI, di);
        c.xor(W, 0, 0);
        let mut cx = 0u16;
        c.set_reg(CX, 0);
        c.set_reg(BX, di);
        let bx = c.shr1(W, di, 1);
        let mut bx = c.inc(W, bx);
        c.set_reg(BX, bx);
        c.set_word_in(SS, bp.wrapping_add(4), bx);
        let mut dx = c.reg(DX);
        loop {
            c.run(0xeb28, 6)?;
            let rem = c.word_in(SS, bp.wrapping_add(2));
            let (a, b) = (dx, di);
            dx = b;
            di = a;
            c.set_reg(DX, dx);
            c.set_reg(DI, di);
            c.set_word_in(ES, di, bx);
            c.set_reg(AX, 0x400);
            cx = c.add(W, cx, 0x400);
            c.set_reg(CX, cx);
            let r = c.sub(W, rem, 0x400);
            c.set_word_in(SS, bp.wrapping_add(2), r);
            if rem <= 0x400 {
                c.run(0xeb37, 4)?;
                c.set_reg(DI, dx);
                c.set_reg(AX, r);
                c.stos(W);
                cx = c.add(W, cx, r);
                c.set_reg(CX, cx);
                break;
            }
            let nd = dx.wrapping_add(2);
            let lim = c.word_in(SS, bp);
            if nd >= lim {
                c.run(0xeb17, 4)?;
                c.set_reg(DI, dx);
                let n = c.add(W, dx, 2);
                c.set_reg(DI, n);
                c.cmp(W, n, lim);
                break;
            }
            let v = c.word_in(ES, nd);
            c.run(0xeb17, if v != 0 { 6 } else { 7 })?;
            c.set_reg(DI, dx);
            let n = c.add(W, dx, 2);
            c.set_reg(DI, n);
            di = n;
            c.cmp(W, n, lim);
            c.cmp(W, v, 0);
            if v != 0 {
                break;
            }
            bx = c.inc(W, bx);
            c.set_reg(BX, bx);
        }
        c.run(0xeb3f, 3)?;
        let b = c.word_in(SS, bp.wrapping_add(4));
        c.set_reg(BX, b);
        c.push(dx);
        c.call(0xec59, 0xeb46)?;
        let rem = c.word_in(SS, bp.wrapping_add(2));
        let again = rem.wrapping_sub(1) <= 0xfc00;
        c.run(0xeb46, 7 + if again { 0 } else { 4 })?;
        let dx = c.pop();
        c.set_reg(DX, dx);
        c.set_reg(DI, dx);
        let v = c.word_in(ES, dx);
        let v = c.or(W, v, 0x8000);
        c.set_word_in(ES, dx, v);
        c.set_reg(AX, rem);
        let a = c.dec(W, rem);
        c.set_reg(AX, a);
        c.cmp(W, a, 0xfc00);
        if !again {
            return end_eb57(&mut c);
        }
    }
}

/// EB60h: no room: the resource dropped (`loc_0ebaa`, SI its slot,
/// [0CE6Eh] FFFFh).
fn drop_eb60(c: &mut Ctx, bp: u16) -> Result<(), Bail> {
    use pc::cpu::SI;
    c.run(0xeb60, 6)?;
    let n = c.word_in(SS, bp);
    c.set_reg(SI, n);
    c.set_reg(AX, 0xffff);
    let old = c.word_in(SS, 0xce6e);
    c.set_word_in(SS, 0xce6e, 0xffff);
    c.set_reg(AX, old);
    let s = c.add(W, n, old);
    let s = c.add(W, s, old);
    c.set_reg(SI, s);
    c.call(0xebaa, 0xeb72)?;
    c.run(0xeb72, 1 + 4)?;
    end_eb57(c)
}

/// EB57h (counted by the caller): the frame dropped, ES and DS back,
/// `loc_0e283`.
fn end_eb57(c: &mut Ctx) -> Result<(), Bail> {
    use pc::cpu::{DS, ES, SP};
    let sp = c.reg(SP);
    let s = c.add(W, sp, 6);
    c.set_reg(SP, s);
    let v = c.pop();
    c.set_sreg(ES, v);
    let v = c.pop();
    c.set_sreg(DS, v);
    c.call(0xe283, 0xeb5f)?;
    c.run(0xeb5f, 1)?;
    c.ret()
}

/// `initialize_system` (E594): the start-up. The variables from 3CBCh to
/// DD1Dh cleared (ES = DS = 1F4Bh), the memory limit [0CE68h] from [2],
/// the program's memory taken (`bump_allocate_bump_cx_bytes`), [0DC32h]
/// the segment past 4C6Fh; the current drive ([0CE76h], [0CE77h]) and
/// the Ctrl-Break flag (kept in [2941h], turned off); DUNE.DAT opened
/// (`open_dune_dat`); the video driver loaded ([2942h] bit 0:
/// `load_driver_ax_with_vtable_at_si`, 38B7h) and started (far [38B9h]:
/// [0DBD8h] the screen), the buffers (`loc_0c08e`, [0CE74h]; the back
/// buffer [0DBD6h], or [0DBD4h] when none), far [38B5h]; the screen kind
/// [0CEEBh] ([2942h] bits 2-4); the joystick (bit 7), the mouse (unless
/// bit 6), the timer (`loc_0e85c`), the extended memory, far [3925h]
/// (BP CE7Ah, AL bit 1); the cursor table 271Ch, the mouse range
/// (0..13Fh, 0..C7h) and position (EDh, ABh); the sound
/// (`initialize_audio`), the HNM memory, the screen; then `loc_0c412`.
pub fn initialize_system(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{BP, DF, DS, ES, SI};
    use super::r8;
    entered(start, 0xe594)?;
    let mut c = Ctx::new(m);
    c.run(0xe594, 8)?;
    c.set_reg(AX, 0x1f4b);
    c.set_sreg(ES, 0x1f4b);
    c.set_reg(CX, 0xdd1d);
    c.set_reg(DI, 0x3cbc);
    let n = c.sub(W, 0xdd1d, 0x3cbc);
    c.set_reg(CX, n);
    c.set_flag(DF, false);
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.rep(0xaa);
    c.run(0xe5a6, 6)?;
    let lim = c.word(2);
    c.set_reg(AX, lim);
    c.push(0x1f4b);
    let v = c.pop();
    c.set_sreg(DS, v);
    c.set_word(0xce68, lim);
    c.set_reg(CX, 0xdd1d);
    c.call(0xf0ff, 0xe5b4)?;
    c.run(0xe5b4, 7)?;
    c.set_reg8(r8::CL, 4);
    let a = c.m.cpu.shift(5, W, 0x4c6f, 4) as u16;
    let ds = c.sreg(DS);
    c.set_reg(CX, ds);
    let a = c.add(W, a, ds);
    c.set_reg(AX, a);
    c.set_word(0xdc32, a);
    c.set_reg8(r8::AH, 0x19);
    c.interp(0xe5c4, 0xe5c6)?;
    c.run(0xe5c6, 3)?;
    let al = c.reg8(r8::AL);
    c.set_byte(0xce76, al);
    c.set_byte(0xce77, al);
    c.set_reg(AX, 0x3301);
    c.interp(0xe5cf, 0xe5d1)?;
    c.run(0xe5d1, 3)?;
    let dl = c.reg8(r8::DL);
    c.set_byte(0x2941, dl);
    c.set_reg(AX, 0x3301);
    let dx = c.reg(DX);
    c.xor(W, dx, dx);
    c.set_reg(DX, 0);
    c.interp(0xe5da, 0xe5dc)?;
    c.run(0xe5dc, 1)?;
    c.call(0xe675, 0xe5df)?;
    c.run(0xe5df, 5)?;
    let f = c.byte(0x2942);
    c.set_reg8(r8::AL, f);
    let ax = c.reg(AX);
    let ax = c.and(W, ax, 1);
    c.set_reg(AX, ax);
    c.set_reg(SI, 0x38b7);
    c.set_reg(CX, 0x2e);
    c.call(0xe57b, 0xe5ee)?;
    c.interp(0xe5ee, 0xe5f2)?;
    c.run(0xe5f2, 2)?;
    let ax = c.reg(AX);
    c.set_word(0xdbd8, ax);
    c.call(0xc08e, 0xe5f8)?;
    c.run(0xe5f8, 3)?;
    let cx = c.reg(CX);
    c.set_word(0xce74, cx);
    c.set_reg(DI, 0xdbdc);
    c.call(0xf0f6, 0xe602)?;
    let bp = c.reg(BP);
    c.run(0xe602, 3 + if bp == 0 { 2 } else { 0 })?;
    c.set_word(0xdbd6, bp);
    c.or(W, bp, bp);
    if bp == 0 {
        c.set_reg(DI, 0xdbd4);
        c.call(0xf0f6, 0xe610)?;
    }
    c.interp(0xe610, 0xe614)?;
    let f = c.byte(0x2942);
    let joy = f & 0x80 != 0;
    c.run(0xe614, 9 + if joy { 2 } else { 0 })?;
    c.set_reg8(r8::AL, f);
    let ax = c.reg(AX);
    c.push(ax);
    let a = c.shr1(B, f as u16, 2);
    let a = c.and(B, a, 7);
    c.set_reg8(r8::AL, a as u8);
    c.set_byte(0xceeb, a as u8);
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.or(B, f as u16, f as u16);
    if joy {
        c.push(ax);
        c.call(0xea32, 0xe62a)?;
        c.run(0xe62a, 1)?;
        let ax = c.pop();
        c.set_reg(AX, ax);
    }
    let mouse = f & 0x40 == 0;
    c.run(0xe62b, 2 + mouse as u32)?;
    c.test(B, f as u16, 0x40);
    if mouse {
        c.call(0xe97a, 0xe632)?;
        c.run(0xe632, 1)?;
    } else {
        c.run(0xe632, 1)?;
    }
    c.call(0xe85c, 0xe635)?;
    c.run(0xe635, 1)?;
    c.call(0xea7b, 0xe638)?;
    c.run(0xe638, 3)?;
    let f = c.byte(0x2942);
    c.set_reg8(r8::AL, f);
    let a = c.and(B, f as u16, 2);
    c.set_reg8(r8::AL, a as u8);
    c.set_reg(BP, 0xce7a);
    c.interp(0xe640, 0xe644)?;
    c.run(0xe644, 7)?;
    c.set_word(0xdc48, 0x271c);
    c.set_byte(0xdc46, 0xff);
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    c.set_reg(AX, 0);
    c.set_reg(BX, 0xc7);
    let cx = c.reg(CX);
    c.xor(W, cx, cx);
    c.set_reg(CX, 0);
    c.set_reg(DX, 0x13f);
    c.call(0xdb14, 0xe65c)?;
    c.run(0xe65c, 3)?;
    c.set_reg(BX, 0xab);
    c.set_reg(DX, 0xed);
    c.call(0xdb03, 0xe665)?;
    for (at, to, ret) in [(0xe665u16, 0xe76au16, 0xe668u16), (0xe668, 0xce6c, 0xe66b), (0xe66b, 0xc07c, 0xe66e), (0xe66e, 0xc0ad, 0xe671)] {
        c.run(at, 1)?;
        c.call(to, ret)?;
    }
    let _ = SI;
    c.run(0xe671, 1)?;
    c.jump(0xc412)
}

/// `initialize_audio` (E76A): the sound driver ([2944h] high nibble + 7,
/// `load_driver_ax_with_vtable_at_si` 398Bh) started (far [3989h], AX
/// [39B5h]; [0DBC8h] its flags), its voice and music buffers
/// (`loc_0a637`, `alloc_voice_buffer`) and `loc_0e851`; if that fails
/// the driver is dropped and the loading tried again without it (or the
/// game ends: F131h). The test sound (`audio_test_frequency`), then the
/// music driver the same way (low nibble + 2 at 396Fh, far [396Dh] with
/// BP 3349h, `loc_0a650`, `alloc_music_buffer`); with EMS for music
/// (`loc_0ae28`) `loc_0e826` and [2943h] bit 4 cleared.
pub fn initialize_audio(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{BP, SI, ZF};
    use super::r8;
    entered(start, 0xe76a)?;
    let mut c = Ctx::new(m);
    loop {
        c.run(0xe76a, 8)?;
        let f = c.byte(0x2944);
        c.set_reg8(r8::AL, f);
        c.set_reg8(r8::CL, 4);
        let a = c.m.cpu.shift(5, B, f as u32, 4) as u16;
        let a = c.add(B, a, 7);
        c.set_reg8(r8::AL, a as u8);
        c.xor(B, 0, 0);
        c.set_reg(AX, a);
        c.set_reg(SI, 0x398b);
        c.set_reg(CX, 8);
        c.call(0xe57b, 0xe77e)?;
        c.run(0xe77e, 1)?;
        let v = c.word(0x39b5);
        c.set_reg(AX, v);
        c.interp(0xe781, 0xe785)?;
        c.run(0xe785, 2)?;
        let bx = c.reg(BX);
        c.set_word(0xdbc8, bx);
        c.call(0xa637, 0xe78c)?;
        c.run(0xe78c, 1)?;
        c.call(0xae54, 0xe78f)?;
        c.run(0xe78f, 1)?;
        c.call(0xe851, 0xe792)?;
        if c.flag(CF) {
            c.run(0xe792, 1)?;
            break;
        }
        let f = c.byte(0x2944);
        if f & 0xf0 == 0 {
            c.run(0xe792, 3 + 1)?;
            c.test(B, f as u16, 0xf0);
            return c.jump(0xf131);
        }
        c.run(0xe792, 3 + 9)?;
        c.test(B, f as u16, 0xf0);
        let f = c.and(B, f as u16, 0xf);
        c.set_byte(0x2944, f as u8);
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
        c.set_word(0x3813, 0);
        c.set_byte(0xdbc8, 0);
        let v = c.word(0x398b);
        let v = c.add(W, v, 0x10);
        c.set_reg(AX, v);
        c.set_word(0x39b9, v);
        c.set_word(0x3cbc, 0x3624);
    }
    c.run(0xe7bc, 3)?;
    let v = c.word(0x3813);
    c.set_reg(AX, v);
    c.set_word(0x381b, v);
    c.call(0xa87e, 0xe7c5)?;
    loop {
        c.run(0xe7c5, 6)?;
        let f = c.byte(0x2944);
        c.set_reg8(r8::AL, f);
        let ax = c.reg(AX);
        let ax = c.and(W, ax, 0xf);
        let ax = c.add(W, ax, 2);
        c.set_reg(AX, ax);
        c.set_reg(SI, 0x396f);
        c.set_reg(CX, 7);
        c.call(0xe57b, 0xe7d7)?;
        c.run(0xe7d7, 3)?;
        c.set_reg(BP, 0x3349);
        c.set_reg(CX, 0xa);
        let v = c.word(0x39b3);
        c.set_reg(AX, v);
        c.interp(0xe7e0, 0xe7e4)?;
        c.run(0xe7e4, 2)?;
        let v = c.word(0xdbc8);
        let bx = c.reg(BX);
        let v = c.or(W, v, bx);
        c.set_word(0xdbc8, v);
        c.call(0xa650, 0xe7eb)?;
        c.run(0xe7eb, 1)?;
        c.call(0xae3f, 0xe7ee)?;
        c.run(0xe7ee, 1)?;
        c.call(0xe851, 0xe7f1)?;
        if c.flag(CF) {
            c.run(0xe7f1, 2)?;
            break;
        }
        let f = c.byte(0x2944);
        if f & 0xf == 0 {
            c.run(0xe7f1, 3 + 1)?;
            c.test(B, f as u16, 0xf);
            return c.jump(0xf131);
        }
        c.run(0xe7f1, 3 + 9)?;
        c.test(B, f as u16, 0xf);
        let f = c.and(B, f as u16, 0xf0);
        c.set_byte(0x2944, f as u8);
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
        c.set_word(0xdbb8, 0);
        c.set_byte(0xdbc9, 0);
        let v = c.word(0x396f);
        let v = c.add(W, v, 0x10);
        c.set_reg(AX, v);
        c.set_word(0x39b9, v);
        c.set_word(0x3cbc, 0x364b);
    }
    c.call(0xae28, 0xe81b)?;
    if c.flag(ZF) {
        c.run(0xe81b, 2)?;
        return c.ret();
    }
    c.run(0xe81b, 2)?;
    c.call(0xe826, 0xe820)?;
    c.run(0xe820, 2)?;
    let f = c.byte(0x2943);
    let f = c.and(B, f as u16, 0xef);
    c.set_byte(0x2943, f as u8);
    c.ret()
}

/// `open_dune_dat` (E675): the patch files DUNE?.DAT (37F2h, the digit
/// at 37F7h counting up to '9'): each found (`loc_0f1fb`) is read to the
/// free memory ([39B7h], `loc_0f260`) and, if it starts with C089h,
/// run (far [39B7h], DX 31FFh). Then DUNE.DAT itself opened (37E9h; on
/// failure returned with CF) into [0DBBAh] and its index read
/// (`loc_0e741`): the resource table at [39B7h] ([0DBBCh]) gets 145h,
/// 14Dh bytes FFh, then for each 19h-byte entry whose name is a known
/// resource (`get_res_index_in_ax_by_name_dssi`, `loc_0f3a7`; a new one
/// opens a 0Ah gap) its place (`loc_0e75b`); the hash table behind it
/// (every 280h bytes, five words) and the memory taken
/// (`bump_allocate_bump_cx_bytes`, CX [0D820h]).
pub fn open_dune_dat(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::{BP, DF, DS, ES, SI, SS, ZF};
    use super::r8;
    entered(start, 0xe675)?;
    let mut c = Ctx::new(m);
    loop {
        c.run(0xe675, 2)?;
        c.set_reg(DX, 0x37f2);
        c.call(0xf1fb, 0xe67b)?;
        if !c.flag(CF) {
            c.run(0xe67b, 3)?;
            let (d, e) = (c.word(0x39b7), c.word(0x39b9));
            c.set_reg(DI, d);
            c.set_sreg(ES, e);
            c.call(0xf260, 0xe684)?;
            let di = c.reg(DI);
            let w = c.word_in(ES, di);
            c.run(0xe684, 2 + if w == 0xc089 { 1 } else { 0 })?;
            c.cmp(W, w, 0xc089);
            if w == 0xc089 {
                c.set_reg(DX, 0x31ff);
                c.interp(0xe68e, 0xe692)?;
            }
        } else {
            c.run(0xe67b, 1)?;
        }
        let d = c.byte(0x37f7).wrapping_add(1);
        c.run(0xe692, 4)?;
        c.set_reg(SI, 0x37f7);
        let v = c.byte(0x37f7);
        let v = c.inc(B, v as u16);
        c.set_byte(0x37f7, v as u8);
        c.cmp(B, d as u16, 0x39);
        if d > 0x39 {
            break;
        }
    }
    c.run(0xe69c, 2)?;
    c.set_reg(DX, 0x37e9);
    c.set_reg(AX, 0x3d00);
    c.interp(0xe6a2, 0xe6a4)?;
    if c.flag(CF) {
        c.run(0xe6a4, 2)?;
        return c.ret();
    }
    c.run(0xe6a4, 3)?;
    let ax = c.reg(AX);
    c.set_word(0xdbba, ax);
    c.call(0xe741, 0xe6ac)?;
    c.run(0xe6ac, 10)?;
    let di = c.reg(DI);
    c.set_reg(SI, di);
    let es = c.sreg(ES);
    c.set_reg(BP, es);
    let (d, e) = (c.word(0x39b7), c.word(0x39b9));
    c.set_reg(DI, d);
    c.set_sreg(ES, e);
    c.set_word(0xdbbc, d);
    c.set_word(0xdbbe, e);
    c.set_reg(AX, 0x145);
    c.stos(W);
    c.set_reg(CX, 0x14d);
    c.set_reg8(r8::AL, 0xff);
    c.rep(0xaa);
    c.run(0xe6c7, 4)?;
    let di = c.reg(DI);
    c.set_word(0xd820, di);
    let ds = c.sreg(DS);
    c.push(ds);
    let bp = c.reg(BP);
    c.set_sreg(DS, bp);
    c.lods(W, DS);
    loop {
        c.run(0xe6cf, 2)?;
        let si = c.reg(SI);
        c.push(si);
        c.call(0xf314, 0xe6d3)?;
        let cf = c.flag(CF);
        c.run(0xe6d3, 2 + if cf { 0 } else { 1 })?;
        let si = c.pop();
        c.set_reg(SI, si);
        if !cf {
            c.call(0xf3a7, 0xe6d9)?;
            let z = c.flag(ZF);
            if z {
                c.run(0xe6d9, 2)?;
            } else {
                c.run(0xe6d9, 13)?;
                for r in [AX, DX, SI, DI] {
                    let v = c.reg(r);
                    c.push(v);
                }
                let end = c.word_in(SS, 0xd820);
                c.set_reg(CX, end);
                c.set_reg(SI, end);
                let di = c.reg(DI);
                let n = c.sub(W, end, di);
                let s = c.sub(W, end, 2);
                c.set_reg(SI, s);
                c.set_reg(DI, s.wrapping_add(0xa));
                let n = c.shr1(W, n, 1);
                c.set_reg(CX, n);
                c.set_flag(DF, true);
                c.m.cpu.string_op(&mut c.m.hw, 0xa5, Some(ES), 0xf3);
                c.run(0xe6f4, 6)?;
                c.set_flag(DF, false);
                for r in [DI, SI, DX, AX] {
                    let v = c.pop();
                    c.set_reg(r, v);
                }
            }
            c.call(0xe75b, 0xe6fc)?;
            c.run(0xe6fc, 1)?;
            let v = c.word_in(SS, 0xd820);
            let v = c.add(W, v, 0xa);
            c.set_word_in(SS, 0xd820, v);
        }
        let si = c.reg(SI);
        let next = c.byte(si.wrapping_add(0x19));
        c.run(0xe702, 3)?;
        let s = c.add(W, si, 0x19);
        c.set_reg(SI, s);
        c.cmp(B, next as u16, 0);
        if next == 0 {
            break;
        }
    }
    c.run(0xe70a, 12)?;
    let v = c.pop();
    c.set_sreg(DS, v);
    c.set_reg(SI, 0x145);
    let end = c.word(0xd820);
    let a = c.sub(W, end, 0x145);
    c.set_reg(AX, a);
    c.xor(W, 0, 0);
    c.set_reg(DX, 0);
    c.set_reg(CX, 0x280);
    c.m.cpu.mul_div(&mut c.m.hw, W, 6, 0x280);
    c.set_reg(DX, 0xa);
    c.mul16(0xa);
    let step = c.reg(AX);
    c.set_reg(DX, step);
    let (d, e) = (c.word_in(SS, 0xdbbc), c.word_in(SS, 0xdbbe));
    c.set_sreg(ES, e);
    let di = c.add(W, d, 2);
    c.set_reg(DI, di);
    let mut si = 0x145u16;
    loop {
        c.run(0xe729, 9)?;
        si = c.add(W, si, step);
        c.set_reg(SI, si);
        c.push(si);
        c.m.cpu.string_op(&mut c.m.hw, 0xa5, Some(ES), 0);
        c.m.cpu.string_op(&mut c.m.hw, 0xa4, Some(ES), 0);
        si = c.pop();
        c.set_reg(SI, si);
        c.set_reg(AX, si);
        c.stos(W);
        let di = c.reg(DI);
        c.cmp(W, di, 0x140);
        if di >= 0x140 {
            break;
        }
    }
    c.run(0xe73a, 2)?;
    let cx = c.word(0xd820);
    c.set_reg(CX, cx);
    c.jump(0xf0ff)
}
