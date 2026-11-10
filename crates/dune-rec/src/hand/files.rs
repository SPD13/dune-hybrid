//! Files: DUNE.DAT and loose files, the memory allocator's helpers, the
//! keyboard buffer, waiting on the timer.

use super::{Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CX, DI, DS, DX, ES, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const SEEK_DUNE_DAT_OFFSET_DXAX: u16 = 0xf2d6;
    pub const READ_DUNE_DAT_CX_TO_ESDI: u16 = 0xf2ea;
    pub const GET_RES_INDEX_BY_NAME: u16 = 0xf314;
    pub const RES_LOCATE_IN_LOOKUP_TABLE: u16 = 0xf3a7;
    pub const SEEK_DUNE_DAT_TO_RES: u16 = 0xf2a7;
    pub const STRCPY_TO_FILENAME_BUF: u16 = 0xf2fc;
    pub const LOAD_RESOURCE_TO: u16 = 0xf0b9;
    pub const ALLOC_MORE: u16 = 0xf13f;
    pub const GET_AND_RESET_KEY_SCANCODE: u16 = 0xdd5a;
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
    pub const PROCESS_FRAME_TASKS: u16 = 0xd9d2;
}

/// Code-segment words: the memory driver's functions (XMS or EMS, set up
/// at start).
mod cs {
    pub const MEMORY_FUNC_1: u16 = 0xea77;
    pub const MEMORY_FUNC_2: u16 = 0xea79;
}

/// The key state array (67h bytes).
const KEY_STATES: u16 = 0xce81;
const KEY_STATES_LEN: u16 = 0x67;

/// From F08E: clears the last key and the key states. Exit: AX = 0;
/// flags from `xor ax, ax`.
fn clear_keys(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xf08e, 7)?;
    let es = c.sreg(ES);
    c.push(es);
    let ds = c.sreg(DS);
    c.push(ds);
    let ds = c.pop();
    c.set_sreg(ES, ds);
    let ax = c.reg(AX);
    let zero = c.xor(W, ax, ax);
    c.set_reg(AX, zero);
    c.set_byte(g::KEY_HIT, 0);
    c.set_reg(DI, KEY_STATES);
    c.set_reg(CX, KEY_STATES_LEN);
    c.run(0xf09c, 1)?;
    c.rep(0xaa); // rep stosb
    c.run(0xf09e, 2)?;
    let es = c.pop();
    c.set_sreg(ES, es);
    c.ret()
}

/// `clear_keyboard_array` (F08E).
pub fn clear_keyboard_array(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf08e)?;
    clear_keys(&mut Ctx::new(m))
}

/// `kb_drain_and_clear` (DE68): takes keys until none is left, then
/// clears the key states.
pub fn kb_drain_and_clear(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xde68)?;
    let mut c = Ctx::new(m);
    loop {
        c.run(0xde68, 1)?;
        c.call(code::GET_AND_RESET_KEY_SCANCODE, 0xde6b)?;
        if c.flag(pc::cpu::ZF) {
            break;
        }
        c.run(0xde6b, 1)?;
    }
    c.run(0xde6b, 2)?; // jne; jmp F08Eh
    clear_keys(&mut c)
}

/// `wait_processing_frame_tasks` (E3A0): AX ticks of the timer, running
/// the frame tasks after each. Exit: CX = 0.
pub fn wait_processing_frame_tasks(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe3a0)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let ticks = c.reg(AX);
    c.set_reg(CX, ticks);
    loop {
        c.run(0xe3a2, 1)?;
        let seen = c.word(g::TIMER);
        c.set_reg(AX, seen);
        loop {
            // The timer moves on in its interrupt: the stretch bails then.
            let now = c.word(g::TIMER);
            if now != seen {
                break;
            }
            c.run(0xe3a5, 2)?;
            c.cmp(W, seen, now);
        }
        c.run(0xe3a5, 3)?;
        let now = c.word(g::TIMER);
        c.cmp(W, seen, now);
        c.call(code::SAVE_REGS, 0xe3ae)?;
        c.run(0xe3ae, 1)?;
        c.call(code::PROCESS_FRAME_TASKS, 0xe3b1)?;
        c.run(0xe3b1, 1)?;
        c.call(code::RESTORE_REGS, 0xe3b4)?;
        let left = c.reg(CX).wrapping_sub(1);
        c.run(0xe3b4, if left == 0 { 2 } else { 1 })?;
        c.set_reg(CX, left);
        if left == 0 {
            return c.ret();
        }
    }
}

/// `load_driver_ax_with_vtable_at_si` (E57B): loads driver AX (resource
/// AX + C8h) and fills the segments of its CX far pointers at SI (4 bytes
/// apart) with its code segment, 10h below where it was loaded.
pub fn load_driver_ax_with_vtable_at_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe57b)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let (cx, si) = (c.reg(CX), c.reg(SI));
    c.push(cx);
    c.push(si);
    let ax = c.reg(AX);
    let resource = c.add(W, ax, 0xc8);
    c.set_reg(AX, resource);
    c.set_reg(SI, resource);
    c.call(code::LOAD_RESOURCE_TO, 0xe585)?;
    c.run(0xe585, 4)?;
    let si = c.pop();
    c.set_reg(SI, si);
    let cx = c.pop();
    c.set_reg(CX, cx);
    let es = c.sreg(ES);
    let segment = c.sub(W, es, 0x10);
    c.set_reg(AX, segment);
    loop {
        let (si, cx) = (c.reg(SI), c.reg(CX));
        let left = cx.wrapping_sub(1);
        c.run(0xe58c, if left == 0 { 4 } else { 3 })?;
        c.set_word(si, segment);
        let si = c.add(W, si, 4);
        c.set_reg(SI, si);
        c.set_reg(CX, left);
        if left == 0 {
            return c.ret();
        }
    }
}

/// `read_dune_dat_toc` (E741): reads DUNE.DAT's table of contents (from
/// its start, up to FFFFh bytes) to 800h paragraphs above the allocator's
/// segment; again until it reads.
pub fn read_dune_dat_toc(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe741)?;
    let mut c = Ctx::new(m);
    loop {
        c.run(0xe741, 3)?;
        let (ax, dx) = (c.reg(AX), c.reg(DX));
        let zero = c.xor(W, ax, ax);
        c.set_reg(AX, zero);
        let zero = c.xor(W, dx, dx);
        c.set_reg(DX, zero);
        c.call(code::SEEK_DUNE_DAT_OFFSET_DXAX, 0xe748)?;
        c.run(0xe748, 6)?;
        let seg = c.word(g::ALLOCATOR_NEXT_SEGMENT);
        let seg = c.add(W, seg, 0x800);
        c.set_reg(AX, seg);
        c.set_sreg(ES, seg);
        let di = c.reg(DI);
        let zero = c.xor(W, di, di);
        c.set_reg(DI, zero);
        c.set_reg(CX, 0xffff);
        c.call(code::READ_DUNE_DAT_CX_TO_ESDI, 0xe758)?;
        let failed = c.flag(pc::cpu::CF);
        c.run(0xe758, if failed { 1 } else { 2 })?;
        if !failed {
            return c.ret();
        }
    }
}

/// `read_dune_dat_cx_to_esdi` (F2EA): reads CX bytes of DUNE.DAT to ES:DI
/// (DOS 3Fh). ZF/CF from comparing the bytes read (AX) with CX.
pub fn read_dune_dat_cx_to_esdi(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf2ea)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.sreg(ES);
    c.push(es);
    let es = c.pop();
    c.set_sreg(DS, es);
    let handle = c.word_in(SS, g::DUNE_DAT_HANDLE);
    c.set_reg(BX, handle);
    let di = c.reg(DI);
    c.set_reg(DX, di);
    c.set_reg8(r8::AH, 0x3f);
    c.interp(0xf2f6, 0xf2f8)?;
    c.run(0xf2f8, 3)?;
    let (ax, cx) = (c.reg(AX), c.reg(CX));
    c.cmp(W, ax, cx);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}

/// `seek_dune_dat_to_res_dsdx` (F2A7): seeks DUNE.DAT to the resource
/// named at DS:DX; CF set if DUNE.DAT is not open or the name is not in
/// it. Exit: the resource's size in BP:CX (high byte, low word), and DOS's
/// seek result. DI, ES kept.
pub fn seek_dune_dat_to_res_dsdx(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf2a7)?;
    let mut c = Ctx::new(m);
    let handle = c.word(g::DUNE_DAT_HANDLE);
    let open = handle >= 1;
    c.run(start, if open { 6 } else { 7 })?;
    let (di, es) = (c.reg(DI), c.sreg(ES));
    c.push(di);
    c.push(es);
    c.cmp(W, handle, 1);
    if open {
        let dx = c.reg(DX);
        c.set_reg(SI, dx);
        c.call(code::GET_RES_INDEX_BY_NAME, 0xf2b5)?;
        if c.flag(pc::cpu::CF) {
            c.run(0xf2b5, 4)?;
        } else {
            c.run(0xf2b5, 2)?;
            c.call(code::RES_LOCATE_IN_LOOKUP_TABLE, 0xf2ba)?;
            if c.flag(pc::cpu::CF) {
                c.run(0xf2ba, 4)?;
            } else {
                c.run(0xf2ba, 8)?;
                let cx = c.reg(CX);
                c.xor(W, cx, cx);
                let entry = c.reg(DI);
                let high = c.byte_in(ES, entry.wrapping_add(5));
                c.set_reg(CX, high as u16);
                c.set_reg(BP, high as u16);
                let size = c.word_in(ES, entry.wrapping_add(3));
                c.set_reg(CX, size);
                let low = c.word_in(ES, entry.wrapping_add(6));
                c.set_reg(AX, low);
                let high = c.word_in(ES, entry.wrapping_add(8));
                c.set_reg(DX, high);
                c.call(code::SEEK_DUNE_DAT_OFFSET_DXAX, 0xf2d3)?;
                c.run(0xf2d3, 3)?;
            }
        }
    }
    let es = c.pop();
    c.set_sreg(ES, es);
    let di = c.pop();
    c.set_reg(DI, di);
    c.ret()
}

/// `open_res_or_file_with_name_in_dx_size_ax` (F1FB): the resource named
/// at DX, from DUNE.DAT if it is there, else as a file of its own (DOS
/// open, then seeks to its end and back for the size). Exit: the size in
/// CX:BP, BX the file handle; CF set if not found.
pub fn open_res_or_file(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf1fb)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let dx = c.reg(DX);
    c.push(dx);
    c.call(code::SEEK_DUNE_DAT_TO_RES, 0xf1ff)?;
    let in_dat = !c.flag(pc::cpu::CF);
    if in_dat {
        c.run(0xf1ff, 3)?; // pop si; jae; ret
        let si = c.pop();
        c.set_reg(SI, si);
        return c.ret();
    }
    c.run(0xf1ff, 5)?;
    let name = c.pop();
    c.set_reg(SI, name);
    c.set_reg(DX, name);
    c.push(name);
    c.call(code::STRCPY_TO_FILENAME_BUF, 0xf208)?;
    c.run(0xf208, 1)?;
    c.set_reg(AX, 0x3d00);
    c.interp(0xf20b, 0xf20d)?; // DOS open
    let failed = c.flag(pc::cpu::CF);
    c.run(0xf20d, if failed { 3 } else { 6 })?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    if failed {
        return c.ret();
    }
    let handle = c.reg(AX);
    c.set_reg(BX, handle);
    let cx = c.reg(CX);
    let zero = c.xor(W, cx, cx);
    c.set_reg(CX, zero);
    c.set_reg(DX, zero);
    c.set_reg(AX, 0x4202);
    c.interp(0xf219, 0xf21b)?; // DOS seek to the end: DX:AX = the size
    c.run(0xf21b, 5)?;
    let (ax, dx) = (c.reg(AX), c.reg(DX));
    c.push(ax);
    c.push(dx);
    let cx = c.reg(CX);
    let zero = c.xor(W, cx, cx);
    c.set_reg(CX, zero);
    c.set_reg(DX, zero);
    c.set_reg(AX, 0x4200);
    c.interp(0xf224, 0xf226)?; // and back to the start
    c.run(0xf226, 3)?;
    let high = c.pop();
    c.set_reg(BP, high);
    let low = c.pop();
    c.set_reg(CX, low);
    c.ret()
}

/// `read_ffff_to_esdi_and_close` (F260): reads the open file BX to ES:DI
/// (up to FFFFh bytes) and closes it. Exit: CX = the bytes read, flags
/// from the read; DX, DI, ES kept.
pub fn read_ffff_to_esdi_and_close(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf260)?;
    let mut c = Ctx::new(m);
    c.run(start, 9)?;
    let (dx, di, es, ds) = (c.reg(DX), c.reg(DI), c.sreg(ES), c.sreg(DS));
    for v in [dx, di, es, ds, es] {
        c.push(v);
    }
    let es = c.pop();
    c.set_sreg(DS, es);
    c.set_reg(CX, 0xffff);
    c.set_reg(DX, di);
    c.set_reg8(r8::AH, 0x3f);
    c.interp(0xf26d, 0xf26f)?; // DOS read
    c.run(0xf26f, 4)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let read = c.reg(AX);
    c.set_reg(CX, read);
    let flags = c.m.cpu.flags();
    c.push(flags);
    c.set_reg8(r8::AH, 0x3e);
    c.interp(0xf275, 0xf277)?; // DOS close
    c.run(0xf277, 5)?;
    let flags = c.pop();
    c.m.cpu.set_flags(flags);
    let es = c.pop();
    c.set_sreg(ES, es);
    let di = c.pop();
    c.set_reg(DI, di);
    let dx = c.pop();
    c.set_reg(DX, dx);
    c.ret()
}

/// `alloc_check_cx_pages_available` (F11C): makes sure CX paragraphs are
/// free above the allocator's next segment, freeing (`sub_f13f`) until
/// they are. Exit: ES:DI = the allocator's next free pointer, AX = its
/// segment + CX; flags from the `cmp`.
pub fn alloc_check_cx_pages_available(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf11c)?;
    let mut c = Ctx::new(m);
    loop {
        c.run(0xf11c, 6)?;
        let (di, seg) = (c.word(g::ALLOCATOR_NEXT_OFFSET), c.word(g::ALLOCATOR_NEXT_SEGMENT));
        c.set_reg(DI, di);
        c.set_sreg(ES, seg);
        let cx = c.reg(CX);
        let end = c.add(W, seg, cx);
        c.set_reg(AX, end);
        let last = c.word(g::ALLOCATOR_LAST_SEGMENT);
        c.cmp(W, end, last);
        if end < last {
            return c.ret();
        }
        c.call(code::ALLOC_MORE, 0xf12e)?;
        c.run(0xf12e, 1)?;
    }
}

/// A memory driver call through a near pointer in the code segment, with
/// the block number BX counted from 1 (EC46, EC59).
fn memory_call(c: &mut Ctx, table: u16, ret: u16) -> Result<(), Bail> {
    let bx = c.reg(BX);
    let bx = c.dec(W, bx);
    c.set_reg(BX, bx);
    let target = c.code_word(table);
    c.call(target, ret)
}

/// `call_memory_func_2` (EC46): memory function 2 (copy to ES:DI?); then
/// DI += CX, flags as the function left them. CX, DS, ES kept.
pub fn call_memory_func_2(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xec46)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let (cx, di, ds, es) = (c.reg(CX), c.reg(DI), c.sreg(DS), c.sreg(ES));
    for v in [cx, di, ds, es] {
        c.push(v);
    }
    memory_call(&mut c, cs::MEMORY_FUNC_2, 0xec50)?;
    c.run(0xec50, 8)?;
    let es = c.pop();
    c.set_sreg(ES, es);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let di = c.pop();
    c.set_reg(DI, di);
    let cx = c.pop();
    c.set_reg(CX, cx);
    let flags = c.m.cpu.flags();
    c.push(flags);
    let di = c.add(W, di, cx);
    c.set_reg(DI, di);
    let flags = c.pop();
    c.m.cpu.set_flags(flags);
    c.ret()
}

/// `call_memory_func_1` (EC59): memory function 1; then SI += CX (AX =
/// CX), flags as the function left them. DS, ES kept.
pub fn call_memory_func_1(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xec59)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    let (si, ds, es, cx) = (c.reg(SI), c.sreg(DS), c.sreg(ES), c.reg(CX));
    for v in [si, ds, es, cx] {
        c.push(v);
    }
    memory_call(&mut c, cs::MEMORY_FUNC_1, 0xec63)?;
    c.run(0xec63, 8)?;
    let count = c.pop();
    c.set_reg(AX, count);
    let es = c.pop();
    c.set_sreg(ES, es);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let si = c.pop();
    c.set_reg(SI, si);
    let flags = c.m.cpu.flags();
    c.push(flags);
    let si = c.add(W, si, count);
    c.set_reg(SI, si);
    let flags = c.pop();
    c.m.cpu.set_flags(flags);
    c.ret()
}

/// `loc_0eb74`: of the B9h words at ES:[BP] (SI) with a value, the one
/// whose age (CS:[0EA75h] - its word 172h further) is the largest goes
/// to `loc_0ebaa` (SI = DI = it). BX the age, 0 for none. DX, SI kept.
pub fn evict_oldest(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xeb74)?;
    let mut c = Ctx::new(m);
    let bp = c.reg(BP);
    let si0 = c.word_in(SS, bp);
    let now = c.code_word(0xea75);
    let mut n = 6;
    let (mut ax, mut bx, mut di) = (c.reg(AX), 0u16, c.reg(DI));
    let mut p = si0;
    for _ in 0..0xb9 {
        ax = c.word_in(ES, p);
        n += 3;
        if ax != 0 {
            let t = c.word_in(ES, p.wrapping_add(0x172));
            ax = now.wrapping_sub(t);
            n += 4;
            if ax >= bx {
                di = p;
                bx = ax;
                n += 2;
            }
        }
        p = p.wrapping_add(2);
        n += 2;
    }
    n += 2;
    let (dx, si) = (c.reg(DX), c.reg(SI));
    if bx != 0 {
        c.run(start, n + 2)?;
    } else {
        c.run(start, n + 3)?;
    }
    c.push(dx);
    c.push(si);
    c.set_reg(SI, p);
    c.set_reg(CX, 0);
    c.set_reg(DX, now);
    c.set_reg(AX, ax);
    c.set_reg(BX, bx);
    c.set_reg(DI, di);
    c.or(W, bx, bx);
    if bx != 0 {
        c.set_reg(SI, di);
        c.call(0xebaa, 0xeba7)?;
        c.run(0xeba7, 3)?;
    }
    let s = c.pop();
    c.set_reg(SI, s);
    let d = c.pop();
    c.set_reg(DX, d);
    c.ret()
}

/// `loc_0ebaa`: the chain of words at ES:SI freed: each taken (zeroed),
/// the next at 2·it - 2, until one with bit 15 set. Bails at entry for a
/// chain that runs into itself (the original would loop forever).
pub fn free_chain(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xebaa)?;
    let mut c = Ctx::new(m);
    // The walk zeroes what it takes: replayed on a copy of those bytes.
    let mut zeroed = std::collections::HashSet::new();
    let mut p = c.reg(SI);
    let mut n = 0;
    loop {
        if n > 7 * 0x10000 {
            return Err(Bail);
        }
        let lo = if zeroed.contains(&p) { 0 } else { c.byte_in(ES, p) };
        let q = p.wrapping_add(1);
        let hi = if zeroed.contains(&q) { 0 } else { c.byte_in(ES, q) };
        zeroed.insert(p);
        zeroed.insert(q);
        let v = lo as u16 | (hi as u16) << 8;
        n += 7;
        if v & 0x8000 != 0 {
            break;
        }
        p = v.wrapping_shl(1).wrapping_sub(2);
    }
    c.run(start, n + 1)?;
    loop {
        let si = c.reg(SI);
        let bx = c.reg(BX);
        c.xor(W, bx, bx);
        let v = c.word_in(ES, si);
        c.set_word_in(ES, si, 0);
        let b = c.m.cpu.shift(4, W, v as u32, 1) as u16;
        c.set_reg(BX, b);
        let cf = c.flag(pc::cpu::CF);
        let s = c.dec(W, b);
        let s = c.dec(W, s);
        c.set_reg(SI, s);
        if cf {
            return c.ret();
        }
    }
}

/// `loc_0f204`: the file DS:DX opened for reading (`loc_0f2fc` first,
/// DX kept); CF set when that fails, else BX the handle and CX:BP its
/// size (seek to the end and back).
pub fn open_file_size(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf204)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let dx = c.reg(DX);
    c.push(dx);
    c.call(0xf2fc, 0xf208)?;
    c.run(0xf208, 1)?;
    c.set_reg(AX, 0x3d00);
    c.interp(0xf20b, 0xf20d)?;
    let failed = c.flag(pc::cpu::CF);
    c.run(0xf20d, if failed { 3 } else { 6 })?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    if failed {
        return c.ret();
    }
    let ax = c.reg(AX);
    c.set_reg(BX, ax);
    let cx = c.reg(CX);
    c.xor(W, cx, cx);
    c.set_reg(CX, 0);
    c.set_reg(DX, 0);
    c.set_reg(AX, 0x4202);
    c.interp(0xf219, 0xf21b)?;
    c.run(0xf21b, 5)?;
    let (ax, dx) = (c.reg(AX), c.reg(DX));
    c.push(ax);
    c.push(dx);
    let cx = c.reg(CX);
    c.xor(W, cx, cx);
    c.set_reg(CX, 0);
    c.set_reg(DX, 0);
    c.set_reg(AX, 0x4200);
    c.interp(0xf224, 0xf226)?;
    c.run(0xf226, 3)?;
    let bp = c.pop();
    c.set_reg(BP, bp);
    let cx = c.pop();
    c.set_reg(CX, cx);
    c.ret()
}

/// `loc_0e826`: with [0DBBAh] set, the DUNE.DAT table (`read_dune_dat_toc`)
/// walked from ES:DI + 2 (FAh entries of 19h bytes, DS = ES): each that
/// `loc_0f314` keeps (CF clear) and `loc_0f3a7` matches (ZF) goes to
/// `loc_0e75b`. DS kept.
pub fn scan_dat_toc(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xe826)?;
    let mut c = Ctx::new(m);
    let k = c.word(0xdbba);
    if k == 0 {
        c.run(start, 3)?;
        c.cmp(W, 0, 0);
        return c.ret();
    }
    c.run(start, 3)?;
    c.cmp(W, k, 0);
    c.call(0xe741, 0xe830)?;
    c.run(0xe830, 6)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let di = c.reg(DI);
    c.set_reg(SI, di);
    let es = c.sreg(ES);
    c.push(es);
    let v = c.pop();
    c.set_sreg(DS, v);
    c.lods(W, DS);
    c.set_reg(CX, 0xfa);
    loop {
        c.run(0xe839, 3)?;
        let (cx, si) = (c.reg(CX), c.reg(SI));
        c.push(cx);
        c.push(si);
        c.call(0xf314, 0xe83e)?;
        let cf = c.flag(pc::cpu::CF);
        if cf {
            c.run(0xe83e, 2)?;
            let s = c.pop();
            c.set_reg(SI, s);
        } else {
            c.run(0xe83e, 3)?;
            let s = c.pop();
            c.set_reg(SI, s);
            c.call(0xf3a7, 0xe844)?;
            if c.flag(pc::cpu::ZF) {
                c.run(0xe844, 2)?;
                c.call(0xe75b, 0xe849)?;
            } else {
                c.run(0xe844, 1)?;
            }
        }
        let si = c.reg(SI);
        let cx = c.word_in(SS, c.reg(SP)).wrapping_sub(1);
        c.run(0xe849, if cx != 0 { 3 } else { 5 })?;
        let si = c.add(W, si, 0x19);
        c.set_reg(SI, si);
        c.pop();
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}

/// `open_res_or_file_or_die` (F229): the file named at DS:DX opened
/// (`loc_0f1fb`: DUNE.DAT entry or loose file). If that fails the name
/// goes into the error text and the game ends (`exit_with_error`, run
/// by the original from F22Fh).
pub fn open_res_or_file_or_die(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::CF;
    entered(start, 0xf229)?;
    let mut c = Ctx::new(m);
    c.run(0xf229, 1)?;
    c.call(0xf1fb, 0xf22c)?;
    if !c.flag(CF) {
        c.run(0xf22c, 2)?;
        return c.ret();
    }
    c.run(0xf22c, 1)?;
    c.m.cpu.ip = 0xf22f;
    Err(Bail)
}

/// `open_resource_by_index_si_into_esdi` (F0B9): resource SI (kept in
/// [0CE78h]) by its name in the table at 31FFh. A size word before the
/// name means the memory is taken here: CX that many bytes checked
/// (`alloc_check_cx_pages_available`), read (`read_and_maybe_hsq`), then
/// allocated (`bump_allocate_bump_cx_bytes`); else it is read only.
pub fn open_resource_by_index(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xf0b9)?;
    let mut c = Ctx::new(m);
    c.run(0xf0b9, 7)?;
    let si = c.reg(SI);
    c.set_word(0xce78, si);
    let s = c.shl1(W, si, 1);
    let p = c.word(s.wrapping_add(0x31ff));
    c.set_reg(SI, p);
    let ax = c.lods(W, DS);
    let dx = p.wrapping_add(2);
    c.set_reg(DX, dx);
    c.or(W, ax, ax);
    if ax == 0 {
        return c.jump(0xf0d6);
    }
    c.run(0xf0ca, 3)?;
    c.set_reg(CX, ax);
    c.push(dx);
    c.call(0xf11c, 0xf0d0)?;
    c.run(0xf0d0, 2)?;
    let v = c.pop();
    c.set_reg(DX, v);
    c.call(0xf0d6, 0xf0d4)?;
    c.run(0xf0d4, 1)?;
    c.jump(0xf0ff)
}

/// `read_and_maybe_hsq` (F0D6): resource [0CE78h] into ES:DI: from
/// extended memory when it is kept there (index below [0CE70h],
/// `alloc_extended_memory`), else read from DUNE.DAT
/// (`read_resource_to_esdi`) and, for such an index, kept in extended
/// memory (`memory_func_qq`); then unpacked if it is HSQ
/// (`check_hsq_checksum`).
pub fn read_and_maybe_hsq(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::CF;
    use super::B;
    entered(start, 0xf0d6)?;
    let mut c = Ctx::new(m);
    let ax = c.word(0xce78);
    let lim = c.byte(0xce70);
    let below = (ax as u8) < lim;
    c.run(0xf0d6, 4)?;
    c.set_reg(AX, ax);
    c.cmp(B, ax & 0xff, lim as u16);
    if below {
        c.call(0xebe3, 0xf0e2)?;
        c.run(0xf0e2, 2)?;
        if c.flag(CF) {
            return c.jump(0xf3d3);
        }
    }
    c.call(0xf244, 0xf0e7)?;
    let ax = c.word(0xce78);
    let lim = c.byte(0xce70);
    let below = (ax as u8) < lim;
    c.run(0xf0e7, 4)?;
    c.set_reg(AX, ax);
    c.cmp(B, ax & 0xff, lim as u16);
    if below {
        c.call(0xeab7, 0xf0f3)?;
        c.run(0xf0f3, 1)?;
    }
    c.jump(0xf3d3)
}

/// `open_resource_force_hsq` (F0A0): resource SI read into DS:4C60h as
/// it is ([0CE71h] up: no unpacking), then unpacked from there to the
/// caller's ES:DI (`hsq_decomp_skip_header_dssi_to_esdi`).
pub fn open_resource_force_hsq(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use super::B;
    entered(start, 0xf0a0)?;
    let mut c = Ctx::new(m);
    c.run(0xf0a0, 7)?;
    let di = c.reg(DI);
    c.push(di);
    let es = c.sreg(ES);
    c.push(es);
    let k = c.byte(0xce71);
    let k = c.inc(B, k as u16);
    c.set_byte(0xce71, k as u8);
    let ds = c.sreg(DS);
    c.push(ds);
    let v = c.pop();
    c.set_sreg(ES, v);
    c.set_reg(DI, 0x4c60);
    c.call(0xf0b9, 0xf0ae)?;
    c.run(0xf0ae, 5)?;
    let k = c.byte(0xce71);
    let k = c.dec(B, k as u16);
    c.set_byte(0xce71, k as u8);
    let di = c.reg(DI);
    c.set_reg(SI, di);
    let v = c.pop();
    c.set_sreg(ES, v);
    let v = c.pop();
    c.set_reg(DI, v);
    c.jump(0xf403)
}

/// `open_nonres_file` (F255): the loose file named at DS:DX
/// (`loc_0f204`; the game ends if it is missing, run by the original)
/// read whole into ES:DI and closed; CX the bytes read, the flags those
/// of the read.
pub fn open_nonres_file(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use pc::cpu::CF;
    entered(start, 0xf255)?;
    let mut c = Ctx::new(m);
    c.run(0xf255, 4)?;
    let (dx, di, es) = (c.reg(DX), c.reg(DI), c.sreg(ES));
    c.push(dx);
    c.push(di);
    c.push(es);
    c.call(0xf204, 0xf25b)?;
    if c.flag(CF) {
        c.run(0xf25b, 1)?;
        c.m.cpu.ip = 0xf22f;
        return Err(Bail);
    }
    c.run(0xf25b, 13)?;
    let es = c.pop();
    c.set_sreg(ES, es);
    let di = c.pop();
    c.set_reg(DI, di);
    let dx = c.pop();
    c.set_reg(DX, dx);
    c.push(dx);
    c.push(di);
    c.push(es);
    let ds = c.sreg(DS);
    c.push(ds);
    c.push(es);
    let v = c.pop();
    c.set_sreg(DS, v);
    c.set_reg(CX, 0xffff);
    c.set_reg(DX, di);
    c.set_reg8(r8::AH, 0x3f);
    c.interp(0xf26d, 0xf26f)?;
    c.run(0xf26f, 4)?;
    let v = c.pop();
    c.set_sreg(DS, v);
    let ax = c.reg(AX);
    c.set_reg(CX, ax);
    let f = c.m.cpu.flags();
    c.push(f);
    c.set_reg8(r8::AH, 0x3e);
    c.interp(0xf275, 0xf277)?;
    c.run(0xf277, 5)?;
    let f = c.pop();
    c.m.cpu.set_flags(f);
    let v = c.pop();
    c.set_sreg(ES, v);
    let v = c.pop();
    c.set_reg(DI, v);
    let v = c.pop();
    c.set_reg(DX, v);
    c.ret()
}

/// `get_res_index_in_ax_by_name_dssi` (F314): the resource index (AX,
/// CF clear) of the name at DS:SI (ES = SS). A name "..\P" + digits is
/// a phrase file: the index from its letters (DL the first less 40h,
/// shifted left with "is it not 'O'" at +4; BX three hex digits, BH's
/// top from the letter at +5). Else the name (up to 16 bytes, CX its
/// length) is compared with the last one opened ([0CE78h]), then with
/// all F7h names of the table at 31FFh; CF set if none matches.
pub fn get_res_index_by_name(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use super::B;
    use pc::cpu::{CF, ZF};
    entered(start, 0xf314)?;
    let mut c = Ctx::new(m);
    let si0 = c.reg(SI);
    let tag = c.word(si0.wrapping_add(2));
    c.run(0xf314, 4)?;
    let ss = c.sreg(SS);
    c.push(ss);
    let v = c.pop();
    c.set_sreg(ES, v);
    c.cmp(W, tag, 0x505c);
    if tag == 0x505c {
        return res_index_by_number(&mut c);
    }
    c.run(0xf31d, 3)?;
    c.push(si0);
    c.set_reg(CX, 0x10);
    c.set_reg(DX, 0x10);
    let mut cx = 0x10u16;
    loop {
        c.run(0xf323, 3)?;
        let a = c.lods(B, DS);
        c.or(B, a, a);
        cx -= 1;
        c.set_reg(CX, cx);
        if cx == 0 || a == 0 {
            break;
        }
    }
    let z = c.flag(ZF);
    c.run(0xf328, 1 + z as u32 + 11 + 1)?;
    if z {
        cx = c.inc(W, cx);
        c.set_reg(CX, cx);
    }
    let n = c.sub(W, cx, 0x10);
    let n = c.neg(W, n);
    c.set_reg(CX, n);
    let si = c.pop();
    c.set_reg(SI, si);
    c.xor(W, 0, 0);
    c.set_reg(DX, 0);
    let ax = c.word(0xce78);
    c.set_reg(AX, ax);
    let d = c.shl1(W, ax, 1);
    c.set_reg(DI, d);
    let d = c.word(d.wrapping_add(0x31ff));
    let d = c.add(W, d, 2);
    c.set_reg(DI, d);
    c.push(n);
    c.push(si);
    c.m.cpu.string_op(&mut c.m.hw, 0xa6, None, 0xf3);
    let eq = c.flag(ZF);
    c.run(0xf345, 3 + if eq { 2 } else { 0 })?;
    let si = c.pop();
    c.set_reg(SI, si);
    let cx = c.pop();
    c.set_reg(CX, cx);
    if eq {
        c.set_flag(CF, false);
        return c.ret();
    }
    c.run(0xf349, 2)?;
    let mut bx = 0x31ffu16;
    c.set_reg(BX, bx);
    let mut bp = 0xf7u16;
    c.set_reg(BP, bp);
    loop {
        c.run(0xf34f, 9)?;
        let d = c.word_in(ES, bx);
        c.set_reg(DI, d);
        c.set_reg(AX, bx);
        let a = c.sub(W, bx, 0x31ff);
        let a = c.shr1(W, a, 1);
        c.set_reg(AX, a);
        bx = c.add(W, bx, 2);
        c.set_reg(BX, bx);
        let d = c.add(W, d, 2);
        c.set_reg(DI, d);
        let (cx, si) = (c.reg(CX), c.reg(SI));
        c.push(cx);
        c.push(si);
        c.m.cpu.string_op(&mut c.m.hw, 0xa6, None, 0xf3);
        let eq = c.flag(ZF);
        c.run(0xf363, 5)?;
        let si = c.pop();
        c.set_reg(SI, si);
        let cx = c.pop();
        c.set_reg(CX, cx);
        if eq {
            c.set_flag(CF, false);
            return c.ret();
        }
        bp = c.dec(W, bp);
        c.set_reg(BP, bp);
        if bp == 0 {
            c.run(0xf36a, 2)?;
            c.set_flag(CF, true);
            return c.ret();
        }
    }
}

/// F36Ch: the phrase file's index from its name (SI at its start).
fn res_index_by_number(c: &mut Ctx) -> Result<(), Bail> {
    use super::B;
    use pc::cpu::CF;
    let si = c.reg(SI).wrapping_add(4);
    let mut n = 6u32;
    for k in 1..4u16 {
        let a = c.byte(si.wrapping_add(k));
        n += 3 + (a >= 0x41) as u32 + 7;
    }
    let last = c.byte(si.wrapping_add(5));
    let below = last < 0x41;
    n += 7 + if below { 0 } else { 5 } + 3;
    c.run(0xf36c, n)?;
    let s0 = c.reg(SI);
    let s = c.add(W, s0, 4);
    c.set_reg(SI, s);
    let a = c.lods(B, DS);
    let a = c.sub(B, a, 0x40);
    c.set_reg8(r8::AL, a as u8);
    c.set_reg8(r8::DL, a as u8);
    let mut dl = a;
    c.xor(W, 0, 0);
    let mut bx = 0u16;
    c.set_reg(BX, 0);
    c.set_reg(CX, 3);
    for k in 0..3u16 {
        let a = c.lods(B, DS);
        c.cmp(B, a, 0x41);
        let a = if a >= 0x41 { c.sub(B, a, 7) } else { a };
        let a = c.and(B, a, 0xf);
        c.set_reg8(r8::AL, a as u8);
        bx = c.shl1(W, bx, 4);
        let bl = c.or(B, bx & 0xff, a);
        bx = (bx & 0xff00) | bl;
        c.set_reg(BX, bx);
        c.set_reg(CX, 2 - k);
    }
    let a = c.lods(B, DS);
    c.cmp(B, a, 0x4f);
    let cf = c.flag(CF);
    c.set_flag(CF, !cf);
    dl = c.rcl1(B, dl);
    c.set_reg8(r8::DL, dl as u8);
    let a = c.lods(B, DS);
    let a = c.sub(B, a, 0x41);
    c.set_reg8(r8::AL, a as u8);
    if !below {
        let a = c.shl1(B, a, 4);
        c.set_reg8(r8::AL, a as u8);
        let bh = c.or(B, bx >> 8, a);
        bx = (bh << 8) | (bx & 0xff);
        c.set_reg(BX, bx);
    }
    c.set_reg(AX, bx);
    c.set_flag(CF, false);
    c.ret()
}

/// `initialize_resources` (00D1): the start-up resources. BAh (the
/// locations' data) read to 4948h and its words byte-swapped (18Ch of
/// them); for each of the 63h records from 494Ah (8 bytes each) the
/// rounded 10000h / (2 * its first word) kept at 4880h; BFh read and
/// the globe's map at it + 62FCh ([0DCFEh]); BDh unpacked to AA76h; BCh
/// read ([0AA72h] its far pointer); two buffers taken (1D4Ch bytes at
/// [47ACh], ADD4h at [47B0h]); `loc_0cfb9`, then no sprite sheet
/// (`loc_0c137`). Bails if a divisor would fault.
pub fn initialize_resources(m: &mut Machine, start: u16) -> Result<(), Bail> {
    use super::B;
    use pc::cpu::CF;
    entered(start, 0x00d1)?;
    let mut c = Ctx::new(m);
    c.run(0x00d1, 5)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let v = c.pop();
    c.set_sreg(ES, v);
    c.set_reg(DI, 0x4948);
    c.set_reg(SI, 0xba);
    c.call(0xf0b9, 0x00dc)?;
    c.run(0x00dc, 2 + 0x18c * 4)?;
    c.set_reg(CX, 0x18c);
    let di = c.reg(DI);
    c.set_reg(SI, di);
    for k in 0..0x18cu16 {
        let a = c.lods(W, DS);
        c.set_reg(AX, a.swap_bytes());
        c.stos(W);
        c.set_reg(CX, 0x18b - k);
    }
    let mut ok = true;
    for k in 0..0x63u16 {
        let v = c.word(0x494a + 8 * k).wrapping_shl(1);
        if v < 2 {
            ok = false;
        }
    }
    if !ok {
        c.m.cpu.ip = 0x00e7;
        return Err(Bail);
    }
    c.run(0x00e7, 3 + 0x63 * 10 + 2)?;
    c.set_reg(DI, 0x4880);
    c.set_reg(CX, 0x63);
    c.set_reg(SI, 0x494a);
    for k in 0..0x63u16 {
        let si = 0x494a + 8 * k;
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
        c.set_reg(DX, 1);
        let w = c.word(si);
        let b = c.shl1(W, w, 1);
        c.set_reg(BX, b);
        c.m.cpu.mul_div(&mut c.m.hw, W, 6, b as u32);
        let dx = c.reg(DX);
        c.cmp(W, w, dx);
        let cf = c.flag(CF) as u16;
        let ax = c.reg(AX);
        let ax = c.adc(W, ax, 0);
        let _ = cf;
        c.set_reg(AX, ax);
        c.stos(W);
        let s = c.add(W, si, 8);
        c.set_reg(SI, s);
        c.set_reg(CX, 0x62 - k);
    }
    c.set_reg(SI, 0xbf);
    c.call(0xf0b9, 0x010c)?;
    c.run(0x010c, 9)?;
    let di = c.reg(DI);
    c.set_reg(AX, di);
    let a = c.add(W, di, 0x62fc);
    c.set_reg(AX, a);
    c.set_word(0xdcfe, a);
    let es = c.sreg(ES);
    c.set_word(0xdd00, es);
    let ds = c.sreg(DS);
    c.push(ds);
    let v = c.pop();
    c.set_sreg(ES, v);
    c.set_reg(DI, 0xaa76);
    c.set_reg(SI, 0xbd);
    c.call(0xf0a0, 0x0123)?;
    c.run(0x0123, 1)?;
    c.call(0x0098, 0x0126)?;
    c.run(0x0126, 2)?;
    c.set_reg(SI, 0xbc);
    c.call(0xf0b9, 0x012c)?;
    c.run(0x012c, 3)?;
    let (di, es) = (c.reg(DI), c.sreg(ES));
    c.set_word(0xaa72, di);
    c.set_word(0xaa74, es);
    c.call(0x0098, 0x0137)?;
    for (at, slot, n, ret) in [(0x0137u16, 0x47acu16, 0x1d4cu16, 0x0148u16), (0x0148, 0x47b0, 0xadd4, 0x0159)] {
        c.run(at, 5)?;
        let (a, e) = (c.word(0x39b7), c.word(0x39b9));
        c.set_reg(AX, a);
        c.set_sreg(ES, e);
        c.set_word(slot, a);
        c.set_word(slot + 2, e);
        c.set_reg(CX, n);
        c.call(0xf0ff, ret)?;
    }
    c.run(0x0159, 1)?;
    c.call(0xcfb9, 0x015c)?;
    c.run(0x015c, 1)?;
    let _ = B;
    c.jump(0xc137)
}
