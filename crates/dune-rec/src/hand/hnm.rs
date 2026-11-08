//! HNM video files: reading and closing.

use super::{Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BX, CF, CX, DS, DX},
    rec::Bail,
};

/// `hnm_close_resource` (CA01): closes the video file, unless it is
/// DUNE.DAT's handle (videos inside it), and resets the frame counters.
/// Exit: CX = 0; flags from `xor cx, cx`.
pub fn hnm_close_resource(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xca01)?;
    let mut c = Ctx::new(m);
    let handle = c.word(g::HNM_FILE_HANDLE);
    if handle == 0 {
        c.run(start, 6)?; // xor; xchg; or; je; xor cx; ret
    } else {
        c.run(start, 5)?;
    }
    let bx = c.reg(BX);
    c.xor(W, bx, bx);
    c.set_word(g::HNM_FILE_HANDLE, 0);
    c.set_reg(BX, handle);
    c.or(W, handle, handle);
    if handle != 0 {
        c.call(0xce01, 0xca0e)?; // hnm_reset_frame_counter
        let dat = c.word(g::DUNE_DAT_HANDLE);
        if handle == dat {
            c.run(0xca0e, 4)?; // cmp; je; xor cx; ret
            c.cmp(W, handle, dat);
        } else {
            c.run(0xca0e, 3)?;
            c.cmp(W, handle, dat);
            c.set_reg8(r8::AH, 0x3e);
            c.interp(0xca16, 0xca18)?; // DOS close
            c.run(0xca18, 2)?;
        }
    }
    let cx = c.reg(CX);
    let zero = c.xor(W, cx, cx);
    c.set_reg(CX, zero);
    c.ret()
}

/// `hnm_read_bytes` (CDBF): reads CX bytes of the video at its file
/// position into the read buffer (DOS seek and read, again until a read
/// gives them all), then moves the position, the bytes left and both read
/// pointers on. CF set if no video file is open.
pub fn hnm_read_bytes(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xcdbf)?;
    let mut c = Ctx::new(m);
    let handle = c.word(g::HNM_FILE_HANDLE);
    c.run(start, if handle < 1 { 4 } else { 3 })?;
    c.set_reg(BX, handle);
    c.cmp(W, handle, 1);
    if handle < 1 {
        return c.ret();
    }
    let read = loop {
        c.run(0xcdc8, 4)?;
        let cx = c.reg(CX);
        c.push(cx);
        let (low, high) = (c.word(g::HNM_FILE_OFFSET), c.word(g::HNM_FILE_OFFSET + 2));
        c.set_reg(CX, high);
        c.set_reg(DX, low);
        c.set_reg(AX, 0x4200);
        c.interp(0xcdd4, 0xcdd6)?; // DOS seek
        c.run(0xcdd6, 4)?;
        let cx = c.pop();
        c.set_reg(CX, cx);
        let ds = c.sreg(DS);
        c.push(ds);
        let (buf, seg) = (c.word(g::HNM_READ_PTR), c.word(g::HNM_READ_PTR + 2));
        c.set_reg(DX, buf);
        c.set_sreg(DS, seg);
        c.set_reg8(r8::AH, 0x3f);
        c.interp(0xcdde, 0xcde0)?; // DOS read
        let (got, wanted) = (c.reg(AX), c.reg(CX));
        if got >= wanted {
            break got;
        }
        c.run(0xcde0, 3)?; // pop ds; cmp; jb
        let ds = c.pop();
        c.set_sreg(DS, ds);
        c.cmp(W, got, wanted);
    };
    c.run(0xcde0, 11)?;
    let ds = c.pop();
    c.set_sreg(DS, ds);
    let wanted = c.reg(CX);
    c.cmp(W, read, wanted);
    // The bytes left (32 bits) go down, the file offset up.
    let left = c.word(g::HNM_FILE_REMAIN);
    let left = c.sub(W, left, read);
    c.set_word(g::HNM_FILE_REMAIN, left);
    let left_high = c.word(g::HNM_FILE_REMAIN + 2);
    let left_high = c.sbb(W, left_high, 0);
    c.set_word(g::HNM_FILE_REMAIN + 2, left_high);
    let offset = c.word(g::HNM_FILE_OFFSET);
    let offset = c.add(W, offset, read);
    c.set_word(g::HNM_FILE_OFFSET, offset);
    let offset_high = c.word(g::HNM_FILE_OFFSET + 2);
    let offset_high = c.adc(W, offset_high, 0);
    c.set_word(g::HNM_FILE_OFFSET + 2, offset_high);
    for p in [g::HNM_READ_PTR, g::HNM_READ_PTR_2] {
        let v = c.word(p);
        let v = c.add(W, v, read);
        c.set_word(p, v);
    }
    c.set_flag(CF, false);
    c.ret()
}
