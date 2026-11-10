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
    c.test(B, f as u16, 0x80);
    if f & 0x80 != 0 {
        c.call(0xeea0, 0xea85)?;
    }
    let f = c.byte(0x2943);
    c.run(0xea85, 2 + (f & 0x48 != 0) as u32)?;
    c.test(B, f as u16, 0x48);
    if f & 0x48 != 0 {
        c.call(0xed4c, 0xea8f)?;
    }
    let f = c.byte(0x2943);
    if f & 0xe8 == 0 {
        c.run(0xea8f, 3)?;
        c.test(B, f as u16, 0xe8);
        return c.ret();
    }
    c.run(0xea8f, 11)?;
    c.test(B, f as u16, 0xe8);
    c.set_reg(DI, 0xce6a);
    let n = c.word(0x39a9);
    c.set_reg(AX, 0xb9);
    c.set_byte(0xce70, 0xb9);
    let n = c.add(W, n, 0xb9);
    let n = c.add(W, n, 0xb9);
    c.push(n);
    let b = c.shl1(W, n, 1);
    c.set_reg(CX, b);
    c.call(0xf0f6, 0xeaad)?;
    c.run(0xeaad, 4)?;
    let (di, es) = (c.word(0xce6a), c.word(0xce6c));
    c.set_reg(DI, di);
    c.set_sreg(ES, es);
    let n = c.pop();
    c.set_reg(CX, n);
    c.xor(W, 0, 0);
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
    c.run(0xe948, 2)?;
    let f = c.pop();
    c.m.cpu.set_flags(f);
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
