//! Files: DUNE.DAT and loose files, the memory allocator's helpers, the
//! keyboard buffer, waiting on the timer.

use super::{Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CX, DI, DS, DX, ES, SI, SS},
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
