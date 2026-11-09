//! The map screen: its view rect ([`VIEW`]), the cursor's place on it,
//! the mouse rect, marks and rects copied around.

use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CS, CX, DI, DS, DX, ES, SI, SP, SS, ZF},
    rec::Bail,
};

mod code {
    pub const SAVE_REGS: u16 = 0xe270;
    pub const RESTORE_REGS: u16 = 0xe283;
    pub const SET_CLICK_HANDLER: u16 = 0xd95e;
    pub const GET_MAP_POSITION: u16 = 0x407e;
    pub const PARTICLES_UPDATE_DIRTY_RECT: u16 = 0xc6ad;
}

/// The map's view rect: left, top, right, bottom.
const VIEW: u16 = 0x46e3;
/// A point on the map (x, y words).
const POINT_4710: u16 = 0x4710;
/// The mark on the map (x at 197Ch, y at 197Eh).
const MARK: u16 = 0x197c;
/// The video sprite list the dirty rect is redrawn with.
const SPRITE_LIST: u16 = 0xdbe0;
const SPRITE_LIST_MAP: u16 = 0x194a;

/// `loc_0557b`: DX, BX = the point at [`POINT_4710`].
pub fn get_point_4710(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x557b)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let (x, y) = (c.word(POINT_4710), c.word(POINT_4710 + 2));
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    c.ret()
}

/// `loc_058e4`: `loc_057e5` with a 100h-byte buffer on the stack (BP),
/// then the driver's function at 395Dh with BH = [4722h].
pub fn with_stack_buffer_57e5(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x58e4)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let sp = c.reg(SP);
    let sp = c.sub(W, sp, 0x100);
    c.set_reg(SP, sp);
    c.set_reg(BP, sp);
    c.call(0x57e5, 0x58ed)?;
    c.run(0x58ed, 1)?;
    let bh = c.byte(0x4722);
    c.set_reg8(r8::BH, bh);
    c.interp(0x58f1, 0x58f5)?;
    c.run(0x58f5, 2)?;
    let sp = c.reg(SP);
    let sp = c.add(W, sp, 0x100);
    c.set_reg(SP, sp);
    c.ret()
}

/// `loc_05982`: the driver's function at 3961h on the screen buffer, at
/// the point at 11C1h, SI = 0AAh, CX = 6Ch (registers kept).
pub fn gfx_call_3961(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5982)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SAVE_REGS, 0x5985)?;
    c.run(0x5985, 5)?;
    let (x, y) = (c.word(0x11c1), c.word(0x11c3));
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    c.set_reg(SI, 0xaa);
    c.set_reg(CX, 0x6c);
    let screen = c.word(super::globals::SCREEN_BUFFER);
    c.set_sreg(ES, screen);
    c.interp(0x5997, 0x599b)?;
    c.run(0x599b, 1)?;
    c.call(code::RESTORE_REGS, 0x599e)?;
    c.run(0x599e, 1)?;
    c.ret()
}

/// `loc_05ad9`: the mouse rect = the view.
pub fn mouse_rect_view(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5ad9)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(SI, VIEW);
    super::input::set_some_mouse_rect(c.m, 0xdaaa)
}

/// `loc_05ad3`: the click handler 1A9Eh, and the mouse rect = the view.
pub fn map_click_handler_and_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5ad3)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(AX, 0x1a9e);
    c.call(code::SET_CLICK_HANDLER, 0x5ad9)?;
    c.run(0x5ad9, 2)?;
    c.set_reg(SI, VIEW);
    super::input::set_some_mouse_rect(c.m, 0xdaaa)
}

/// `loc_05b10`: `loc_08850`, then with bit 40h of [46EBh] the mouse rect
/// = the rect at [`POINT_4710`].
pub fn after_8850(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5b10)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x8850, 0x5b13)?;
    let kinds = c.byte(0x46eb);
    let on = kinds & 0x40 != 0;
    c.run(0x5b13, if on { 5 } else { 3 })?;
    c.test(B, kinds as u16, 0x40);
    if !on {
        return c.ret();
    }
    c.set_reg(SI, POINT_4710);
    super::input::set_some_mouse_rect(c.m, 0xdaaa)
}

/// From 5B60 (`at`, `prefix` instructions before, done by `ops`): the mark
/// at (DX, BX).
fn set_mark(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 3)?;
    ops(c);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.set_word(MARK + 2, bx);
    c.set_word(MARK, dx);
    c.ret()
}

/// `loc_05b60`: the mark at (DX, BX).
pub fn set_mark_dx_bx(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5b60)?;
    set_mark(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_05b55`: the mark at the point of the record at DI (+2, +4).
pub fn set_mark_at_record(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5b55)?;
    set_mark(&mut Ctx::new(m), start, 3, |c| {
        let di = c.reg(DI);
        let (x, y) = (c.word(di.wrapping_add(2)), c.word(di.wrapping_add(4)));
        c.set_reg(DX, x);
        c.set_reg(BX, y);
    })
}

/// `loc_05b5d`: the mark at the map position (`get_map_position`).
pub fn set_mark_at_map_position(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5b5d)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::GET_MAP_POSITION, 0x5b60)?;
    set_mark(&mut c, 0x5b60, 0, |_| {})
}

/// From 5B96: the view rect copied to DI (4 words). The stretch from `at`
/// has `prefix` instructions before, done by `ops`.
fn copy_view(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 8)?;
    ops(c);
    c.set_reg(SI, VIEW);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    for _ in 0..4 {
        c.movs(W);
    }
    c.ret()
}

/// `loc_05b96`: the view rect to DI.
pub fn copy_view_to_di(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5b96)?;
    copy_view(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_05b93`: the view rect to the clip rect (D834h).
pub fn copy_view_to_clip_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5b93)?;
    copy_view(&mut Ctx::new(m), start, 1, |c| c.set_reg(DI, 0xd834))
}

/// `loc_05b8d`: the view rect to D83Ch and to the clip rect.
pub fn copy_view_to_both_rects(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5b8d)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(DI, 0xd83c);
    c.call(0x5b96, 0x5b93)?;
    copy_view(&mut c, 0x5b93, 1, |c| c.set_reg(DI, 0xd834))
}

/// `loc_05beb`: with the map's sprite list in use, the list dropped and
/// the dirty rect redrawn (registers kept).
pub fn drop_map_sprite_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5beb)?;
    let mut c = Ctx::new(m);
    let list = c.word(SPRITE_LIST);
    let ours = list == SPRITE_LIST_MAP;
    c.run(start, 3)?;
    c.cmp(W, list, SPRITE_LIST_MAP);
    if !ours {
        return c.ret();
    }
    c.call(code::SAVE_REGS, 0x5bf6)?;
    c.run(0x5bf6, 3)?;
    let si = c.reg(SI);
    c.xor(W, si, si);
    let old = c.word(SPRITE_LIST);
    c.set_word(SPRITE_LIST, 0);
    c.set_reg(SI, old);
    c.call(code::PARTICLES_UPDATE_DIRTY_RECT, 0x5bff)?;
    c.run(0x5bff, 1)?;
    c.call(code::RESTORE_REGS, 0x5c02)?;
    c.run(0x5c02, 1)?;
    c.ret()
}

/// `loc_05d1d`: CF set when (DX, BX) is in the view (right, bottom out).
pub fn in_view(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5d1d)?;
    let mut c = Ctx::new(m);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let v = [c.word(VIEW), c.word(VIEW + 2), c.word(VIEW + 4), c.word(VIEW + 6)];
    let n = if dx < v[0] {
        4
    } else if dx >= v[2] {
        6
    } else if bx < v[1] {
        9
    } else {
        10
    };
    c.run(start, n)?;
    c.cmp(W, dx, v[0]);
    let cf = c.flag(CF);
    c.set_flag(CF, !cf);
    if dx >= v[0] {
        c.cmp(W, dx, v[2]);
        if dx < v[2] {
            c.cmp(W, bx, v[1]);
            let cf = c.flag(CF);
            c.set_flag(CF, !cf);
            if bx >= v[1] {
                c.cmp(W, bx, v[3]);
            }
        }
    }
    c.ret()
}

/// `loc_062d6`: `loc_0b647`, then CF set when (DX, BX) is outside the
/// view (right, bottom out).
pub fn outside_view_after_b647(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x62d6)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xb647, 0x62d9)?;
    outside_view_tail(&mut c)
}

/// From 62D9: CF set when (DX, BX) is outside the view (right, bottom
/// out); returns.
fn outside_view_tail(c: &mut Ctx) -> Result<(), Bail> {
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let v = [c.word(VIEW), c.word(VIEW + 2), c.word(VIEW + 4), c.word(VIEW + 6)];
    let n = if dx < v[0] {
        3
    } else if dx >= v[2] {
        6
    } else if bx < v[1] {
        8
    } else {
        10
    };
    c.run(0x62d9, n)?;
    c.cmp(W, dx, v[0]);
    if dx >= v[0] {
        c.cmp(W, dx, v[2]);
        let cf = c.flag(CF);
        c.set_flag(CF, !cf);
        if dx < v[2] {
            c.cmp(W, bx, v[1]);
            if bx >= v[1] {
                c.cmp(W, bx, v[3]);
                let cf = c.flag(CF);
                c.set_flag(CF, !cf);
            }
        }
    }
    c.ret()
}

/// `loc_05d36`: CF set when the location at DI is of kind 28h or more
/// with bit 8 of byte 0Ah, or of a kind below 28h.
pub fn location_kind_check(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5d36)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let kind = c.byte(di.wrapping_add(8));
    let f = c.byte(di.wrapping_add(0x0a));
    let n = if kind < 0x28 { 3 } else if f & 8 == 0 { 5 } else { 6 };
    c.run(start, n)?;
    c.cmp(B, kind as u16, 0x28);
    if kind >= 0x28 {
        c.test(B, f as u16, 8);
        if f & 8 != 0 {
            c.set_flag(CF, true);
        }
    }
    c.ret()
}

/// `loc_05ed0`: BP = the 6-byte entry at A5C0h.. (SS) whose word is DI
/// (ZF set), or the 0 one at the end (ZF clear).
pub fn find_marker(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5ed0)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let mut bp = 0xa5bau16;
    let mut n = 1;
    loop {
        bp = bp.wrapping_add(6);
        let w = c.word_in(SS, bp);
        n += 4;
        if w == di {
            n += 1;
            break;
        }
        n += 2;
        if w == 0 {
            n += 2;
            break;
        }
    }
    c.run(start, n)?;
    let mut bp = 0xa5bau16;
    loop {
        bp = c.add(W, bp, 6);
        let w = c.word_in(SS, bp);
        c.set_reg(AX, w);
        c.cmp(W, w, di);
        if w == di {
            break;
        }
        c.or(W, w, w);
        if w == 0 {
            c.or(W, bp, bp);
            break;
        }
    }
    c.set_reg(BP, bp);
    c.ret()
}

/// `loc_06144`: [0FDh] = `loc_060f8`'s AL with bit 0 (registers kept).
pub fn note_60f8(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6144)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SAVE_REGS, 0x6147)?;
    c.run(0x6147, 2)?;
    let es = c.sreg(ES);
    c.push(es);
    c.call(0x60f8, 0x614b)?;
    c.run(0x614b, 4)?;
    let al = c.reg8(r8::AL);
    let al = c.or(B, al as u16, 1) as u8;
    c.set_reg8(r8::AL, al);
    c.set_byte(0xfd, al);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.call(code::RESTORE_REGS, 0x6154)?;
    c.run(0x6154, 1)?;
    c.ret()
}

/// `loc_0633b`: `loc_0634d` from 12h above the mark down, a row at a time,
/// while it clears CF.
pub fn scan_up_from_mark(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x633b)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let (x, y) = (c.word(MARK), c.word(MARK + 2));
    c.set_reg(DX, x);
    let y = c.sub(W, y, 0x12);
    c.set_reg(BX, y);
    c.call(0x634d, 0x6349)?;
    loop {
        let cf = c.flag(CF);
        c.run(0x6349, 3)?;
        let bx = c.reg(BX);
        let bx = c.inc(W, bx);
        c.set_reg(BX, bx);
        if cf {
            return c.ret();
        }
        c.call(0x634d, 0x6349)?;
    }
}

/// `loc_06515`: `loc_05d50`, then the move of the record at DI set up
/// ([4736h] = 44h, CX = its byte 0Bh, its words 0Ch, 0Eh (as a signed
/// byte) to D81Ch, D818h; the step routine patched to 653Ah) and
/// `loc_064b2`.
pub fn start_move(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6515)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x5d50, 0x6518)?;
    c.run(0x6518, 10)?;
    c.set_byte(0x4736, 0x44);
    let cx = c.reg(CX);
    c.xor(W, cx, cx);
    let di = c.reg(DI);
    let cl = c.byte(di.wrapping_add(0x0b));
    c.set_reg(CX, cl as u16);
    c.set_word_in(CS, 0x64ed, 0x653a);
    let a = c.word(di.wrapping_add(0x0c));
    c.set_word(0xd81c, a);
    let b = c.word(di.wrapping_add(0x0e));
    let b = b as u8 as i8 as u16;
    c.set_reg(AX, b);
    c.set_word(0xd818, b);
    c.call(0x64b2, 0x6539)?;
    c.run(0x6539, 1)?;
    c.ret()
}

/// `loc_080c8`: [4738h] and the words at 4718h..471Ch cleared.
pub fn clear_4718(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x80c8)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    c.set_byte(0x4738, 0);
    let ax = c.reg(AX);
    c.xor(W, ax, ax);
    c.set_reg(AX, 0);
    for at in [0x4718, 0x471a, 0x471c] {
        c.set_word(at, 0);
    }
    c.ret()
}

/// `loc_081d7`: an entry at DI: DX at +4, then `loc_0b647`'s point (DX, BX)
/// at +0, +2; DI on to the next, marked 8000h.
pub fn add_point_entry(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x81d7)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let (di, dx) = (c.reg(DI), c.reg(DX));
    c.set_word(di.wrapping_add(4), dx);
    c.push(di);
    c.call(0xb647, 0x81de)?;
    c.run(0x81de, 6)?;
    let di = c.pop();
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    c.set_word(di, dx);
    c.set_word(di.wrapping_add(2), bx);
    let di = c.add(W, di, 6);
    c.set_reg(DI, di);
    c.set_word(di, 0x8000);
    c.ret()
}

/// `loc_082a0`: ZF set unless the current UI elements are one of the two
/// map lists (212Eh, 2136h) with bit 40h of [46EBh] clear.
pub fn map_list_without_40(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x82a0)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xd41b, 0x82a3)?;
    let bp = c.reg(BP);
    let first = bp == 0x212e;
    let ours = first || bp == 0x2136;
    c.run(0x82a3, if first { 2 } else { 3 } + 1 + if ours { 3 } else { 0 } + 1)?;
    c.cmp(W, bp, 0x212e);
    if !first {
        c.cmp(W, bp, 0x2136);
    }
    if ours {
        let k = c.byte(0x46eb);
        let al = !k;
        c.set_reg8(r8::AL, al);
        c.test(B, al as u16, 0x40);
    }
    c.ret()
}

/// `loc_0848f`: `loc_08347` while the troop at SI is at the location at
/// [11D3h] and without bit 40h; DI = that location, ZF from `or di, di`.
pub fn while_at_11d3(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x848f)?;
    let mut c = Ctx::new(m);
    let (mut at, mut jmp) = (start, 0);
    loop {
        let si = c.reg(SI);
        let di = c.word(0x11d3);
        let here = c.word(si.wrapping_add(4)) == di;
        let f = c.byte(si.wrapping_add(3));
        let go = here && f & 0x40 == 0;
        c.run(at, jmp + if !here { 5 } else if !go { 7 } else { 6 })?;
        c.set_reg(DI, di);
        let w = c.word(si.wrapping_add(4));
        c.cmp(W, w, di);
        if here {
            c.test(B, f as u16, 0x40);
        }
        if !go {
            c.or(W, di, di);
            return c.ret();
        }
        c.call(0x8347, 0x84a1)?;
        (at, jmp) = (0x84a1, 1);
    }
}

/// `loc_08770`: with [1954h] set, `loc_0878c` with [46F3h] cleared, then
/// the text pointer cleared (registers kept).
pub fn redraw_8770(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8770)?;
    let mut c = Ctx::new(m);
    let v = c.byte(0x1954);
    c.run(start, 3)?;
    c.cmp(B, v as u16, 0);
    if v == 0 {
        return c.ret();
    }
    c.call(code::SAVE_REGS, 0x877a)?;
    c.run(0x877a, 2)?;
    c.set_byte(0x46f3, 0);
    c.call(0x878c, 0x8782)?;
    c.run(0x8782, 2)?;
    c.set_word(0x1bea, 0);
    c.call(code::RESTORE_REGS, 0x878b)?;
    c.run(0x878b, 1)?;
    c.ret()
}

/// `loc_08850`: `loc_07b36`, then the handler at [46EDh].
pub fn redraw_and_handle(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8850)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x7b36, 0x8853)?;
    c.run(0x8853, 1)?;
    let h = c.word(0x46ed);
    c.call(h, 0x8857)?;
    c.run(0x8857, 1)?;
    c.ret()
}

/// `loc_08ac3`: [47E0h] = AL, bit 10h of [47DEh] cleared.
pub fn note_47e0(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8ac3)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let al = c.reg8(r8::AL);
    c.set_byte(0x47e0, al);
    let v = c.byte(0x47de);
    let v = c.and(B, v as u16, 0xef);
    c.set_byte(0x47de, v as u8);
    c.ret()
}

/// `loc_08fd1`: the record at 1BE2h drawn by the driver (function at
/// 38DDh) in colour [18F2h], after setting it (+8 = 80h, +0Ch = 7BEDh,
/// +2 two up); its word 2 two down after (SI kept).
pub fn gfx_draw_1be2(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8fd1)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let si = c.reg(SI);
    c.push(si);
    let r = 0x1be2u16;
    c.set_reg(SI, r);
    c.set_word(r + 8, 0x80);
    c.set_word(r + 0x0c, 0x7bed);
    let y = c.word(r + 2);
    let y = c.sub(W, y, 2);
    c.set_word(r + 2, y);
    let al = c.byte(0x18f2);
    c.set_reg8(r8::AL, al);
    let fb = c.word(super::globals::FRAMEBUFFER_ACTIVE);
    c.set_sreg(ES, fb);
    c.interp(0x8fea, 0x8fee)?;
    c.run(0x8fee, 3)?;
    let y = c.word(r + 2);
    let y = c.add(W, y, 2);
    c.set_word(r + 2, y);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_09025`: the driver's function at 38C9h on the active framebuffer:
/// 140h wide from (0, 92h - [4793h]), [4793h] high (CH = FFh), the image
/// at [22FCh]; the top noted at [4782h].
pub fn gfx_call_38c9(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9025)?;
    let mut c = Ctx::new(m);
    c.run(start, 9)?;
    let h = c.word(0x4793);
    c.set_reg(CX, h);
    let top = c.sub(W, 0x92, h);
    c.set_reg(BX, top);
    let dx = c.reg(DX);
    c.xor(W, dx, dx);
    c.set_reg(DX, 0);
    c.set_reg8(r8::CH, 0xff);
    c.set_reg(DI, 0x140);
    let si = c.word(0x22fc);
    c.set_reg(SI, si);
    let fb = c.word(super::globals::FRAMEBUFFER_ACTIVE);
    c.set_sreg(ES, fb);
    c.set_word(0x4782, top);
    c.interp(0x9041, 0x9045)?;
    c.run(0x9045, 1)?;
    c.ret()
}

/// From 49A0 (the stretch from `at`, `prefix` instructions before, done by
/// `ops`): the zoomed map drawn into the back buffer (the clip rect the
/// view, `map_draw_zoomed_globe` in hotspot mode, `loc_05b69`, the icons,
/// `loc_05dce`), the place's mark (sprite 2Eh, `loc_062c9`) when there is
/// one in view; then the front buffer active again.
fn draw_zoomed_map(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 1)?;
    ops(c);
    c.call(0xc085, 0x49a3)?;
    c.run(0x49a3, 1)?;
    c.call(0x5b93, 0x49a6)?;
    c.run(0x49a6, 2)?;
    c.set_byte(0x46eb, 1);
    c.call(0xb6c3, 0x49ae)?;
    c.run(0x49ae, 1)?;
    c.call(0x5b69, 0x49b1)?;
    c.run(0x49b1, 1)?;
    c.call(0xc137, 0x49b4)?;
    c.run(0x49b4, 1)?;
    c.call(0x5dce, 0x49b7)?;
    let si = c.word(0x11c5);
    c.run(0x49b7, 3 + (si != 0) as u32 + if si == 0 { 2 } else { 0 })?;
    c.set_reg(SI, si);
    c.or(W, si, si);
    if si != 0 {
        c.call(0x62c9, 0x49c2)?;
        let out = c.flag(CF);
        c.run(0x49c2, if out { 3 } else { 5 })?;
        if !out {
            let bx = c.reg(BX);
            let bx = c.dec(W, bx);
            c.set_reg(BX, bx);
            let dx = c.reg(DX);
            let dx = c.dec(W, dx);
            c.set_reg(DX, dx);
            c.set_reg(AX, 0x2e);
            c.call(0xc22f, 0x49cc)?;
            c.run(0x49cc, 2)?;
        }
    }
    c.set_byte(0x46eb, 0);
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `loc_049a0` (the map's redraw handler): see [`draw_zoomed_map`].
pub fn draw_zoomed_map_49a0(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x49a0)?;
    draw_zoomed_map(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_04988`: the zoomed map opened: [46FCh] cleared, the mark at the
/// map position, the view rect from 148Ah, the redraw handler 49A0h, and
/// [`draw_zoomed_map`].
pub fn open_zoomed_map(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4988)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_word(0x46fc, 0);
    c.call(0x5b5d, 0x4991)?;
    c.run(0x4991, 3)?;
    c.set_reg(SI, 0x148a);
    c.set_reg(DI, VIEW);
    c.call(0x5b99, 0x499a)?;
    draw_zoomed_map(&mut c, 0x499a, 1, |c| c.set_word(0x46ed, 0x49a0))
}

/// `loc_05198`: the step (DX, BX) for the direction AL (0..FFh, 40h a
/// quarter): 20h along the axis of the quarter it is in, signed, the rest
/// across (from AL).
pub fn direction_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5198)?;
    let mut c = Ctx::new(m);
    let ax = c.reg(AX);
    let bl = (ax as u8).wrapping_add(0x20);
    let bh = bl & 0x7f;
    let neg = bl & 0x80 != 0;
    let along = bh >= 0x40;
    c.run(start, 6 + if along { 4 } else { 3 } + if neg { 3 } else { 0 } + 3)?;
    let b = c.add(B, ax & 0xff, 0x20);
    let h = c.and(B, b, 0x7f);
    c.set_reg(BX, h << 8 | b);
    c.cmp(B, h, 0x40);
    let mut al = ax as u8;
    if along {
        let mut dx = 0x20u16;
        al = c.sub(B, al as u16, 0x40) as u8;
        c.or(B, b, b);
        if neg {
            dx = c.neg(W, dx);
            al = c.sub(B, al as u16, 0x80) as u8;
            al = c.neg(B, al as u16) as u8;
        }
        c.set_reg(DX, dx);
        let v = al as i8 as u16;
        c.set_reg(AX, v);
        c.set_reg(BX, v);
    } else {
        c.or(B, b, b);
        let mut bx = 0xffe0u16;
        if neg {
            al = c.sub(B, al as u16, 0x80) as u8;
            al = c.neg(B, al as u16) as u8;
            bx = c.neg(W, bx);
        }
        c.set_reg(BX, bx);
        let v = al as i8 as u16;
        c.set_reg(AX, v);
        c.set_reg(DX, v);
    }
    c.ret()
}

/// `loc_05323`: for the place at [BP] (SS): `loc_05133` with its words 2,
/// 4; [BP+2] = the octant of the result ((AL + 10h) >> 5). BX, DX kept.
pub fn place_octant(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5323)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    let bp = c.reg(BP);
    let di = c.word_in(SS, bp);
    let (cx, d) = (c.word(di.wrapping_add(4)), c.word(di.wrapping_add(2)));
    c.set_reg(CX, cx);
    c.set_reg(DI, d);
    c.push(bp);
    c.call(0x5133, 0x5332)?;
    c.run(0x5332, 10)?;
    let bp = c.pop();
    c.set_reg(BP, bp);
    let al = c.reg8(r8::AL);
    let mut v = c.add(B, al as u16, 0x10);
    for _ in 0..3 {
        v = c.rol1(B, v);
    }
    let ax = c.reg(AX) & 0xff00 | v;
    let ax = c.and(W, ax, 7);
    c.set_reg(AX, ax);
    c.m.cpu.write8(&mut c.m.hw, SS, bp.wrapping_add(2), ax as u8);
    let dx = c.pop();
    c.set_reg(DX, dx);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.ret()
}

/// `loc_05344`: DI = the location nearest to (DX, BX) (not of bit 80h of
/// byte 0Ah; the distance as the code measures it in BP). SI kept.
pub fn nearest_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5344)?;
    let mut c = Ctx::new(m);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    // The path, purely.
    let mut n = 3;
    let mut si = 0x100u16;
    let mut bp = 0xffffu16;
    loop {
        n += 2;
        if c.word(si) == 0xffff {
            n += 2;
            break;
        }
        n += 2;
        if c.byte(si.wrapping_add(0x0a)) & 0x80 == 0 {
            let dxd = c.word(si.wrapping_add(2)).wrapping_sub(dx);
            n += 3 + (dxd & 0x8000 != 0) as u32;
            let dyd = c.word(si.wrapping_add(4)).wrapping_sub(bx);
            n += 3 + (dyd & 0x8000 != 0) as u32;
            let cxa = if dxd & 0x8000 != 0 { dxd.wrapping_neg() } else { dxd };
            let ax = if dyd & 0x8000 != 0 { dyd.wrapping_neg() } else { dyd };
            let mut cx = cxa >> 8;
            n += 4;
            if (cx as u8) < ax as u8 {
                cx = ax;
                n += 1;
            }
            n += 2;
            if cx < bp {
                bp = cx;
                n += 2;
            }
        }
        n += 2;
        si = si.wrapping_add(0x1c);
    }
    c.run(start, n)?;
    let s0 = c.reg(SI);
    c.push(s0);
    c.set_reg(BP, 0xffff);
    let mut si = 0x100u16;
    loop {
        c.set_reg(SI, si);
        let w = c.word(si);
        c.cmp(W, w, 0xffff);
        if w == 0xffff {
            break;
        }
        let f = c.byte(si.wrapping_add(0x0a));
        c.test(B, f as u16, 0x80);
        if f & 0x80 == 0 {
            let x = c.word(si.wrapping_add(2));
            let mut cx = c.sub(W, x, dx);
            if cx & 0x8000 != 0 {
                cx = c.neg(W, cx);
            }
            let y = c.word(si.wrapping_add(4));
            let mut ax = c.sub(W, y, bx);
            if ax & 0x8000 != 0 {
                ax = c.neg(W, ax);
            }
            c.set_reg(AX, ax);
            cx >>= 8;
            c.xor(B, 0, 0);
            c.cmp(B, cx, ax & 0xff);
            if (cx as u8) < ax as u8 {
                cx = ax;
            }
            c.set_reg(CX, cx);
            let bp = c.reg(BP);
            c.cmp(W, cx, bp);
            if cx < bp {
                c.set_reg(BP, cx);
                c.set_reg(DI, si);
            }
        }
        si = c.add(W, si, 0x1c);
    }
    let _ = bp;
    let s0 = c.pop();
    c.set_reg(SI, s0);
    c.ret()
}

/// `loc_051cb`: the heading byte [11C7h] for a move by (DX, BX): far
/// (|BX| over 4Dh) against its half, turned to the side (bit 7 kept, 40h);
/// for a move to a place (neither [11CBh] nor [11C8h]), `loc_05124`'s AL
/// unless CF.
pub fn heading_51cb(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x51cb)?;
    let mut c = Ctx::new(m);
    let (a, b) = (c.byte(0x11cb), c.byte(0x11c8));
    let bx = c.reg(BX);
    if a == 0 && b == 0 {
        c.run(start, 4 + 4)?;
        c.cmp(B, 0, 0);
        c.cmp(B, 0, 0);
        let di = c.word(0x11c5);
        c.set_reg(DI, di);
        let dx = c.reg(DX);
        c.push(bx);
        c.push(dx);
        c.call(0x5124, 0x51fe)?;
        let cf = c.flag(CF);
        c.run(0x51fe, if cf { 4 } else { 5 })?;
        let dx = c.pop();
        c.set_reg(DX, dx);
        let bx = c.pop();
        c.set_reg(BX, bx);
        if !cf {
            let al = c.reg8(r8::AL);
            c.set_byte(0x11c7, al);
        }
        return c.ret();
    }
    let pre = if a != 0 { 2 } else { 4 };
    let near = (bx as i16) >= -0x4d && (bx as i16) <= 0x4d;
    if near {
        c.run(start, pre + 4 + 1)?;
    } else {
        let h = c.byte(0x11c7);
        let same = (h.wrapping_sub(0x40) ^ (bx >> 8) as u8) & 0x80 != 0;
        c.run(start, pre + if (bx as i16) < -0x4d { 2 } else { 4 } + if same { 6 } else { 10 })?;
    }
    c.cmp(B, a as u16, 0);
    if a == 0 {
        c.cmp(B, b as u16, 0);
    }
    c.cmp(W, bx, 0xffb3);
    if (bx as i16) >= -0x4d {
        c.cmp(W, bx, 0x4d);
    }
    if near {
        return c.ret();
    }
    let al = c.byte(0x11c7);
    let ah = c.sub(B, al as u16, 0x40);
    let ah = c.xor(B, ah, bx >> 8);
    c.set_reg(AX, ah << 8 | al as u16);
    if ah & 0x80 != 0 {
        return c.ret();
    }
    let v = c.and(B, al as u16, 0x80);
    let v = c.or(B, v, 0x40);
    c.set_reg(AX, ah << 8 | v);
    c.set_byte(0x11c7, v as u8);
    c.ret()
}

/// `loc_0578b`: for AL (not negative), the driver's function at 3961h at
/// [`POINT_4710`] + (5Eh + 4 AL, 62h), 5 x 7. AX kept.
pub fn gfx_call_3961_at_4710(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x578b)?;
    let mut c = Ctx::new(m);
    let al = c.reg8(r8::AL);
    if al & 0x80 != 0 {
        c.run(start, 3)?;
        c.or(B, al as u16, al as u16);
        return c.ret();
    }
    c.run(start, 4)?;
    c.or(B, al as u16, al as u16);
    let ax = c.reg(AX);
    c.push(ax);
    c.call(0x557b, 0x5793)?;
    c.run(0x5793, 8)?;
    let dx = c.reg(DX);
    let dx = c.add(W, dx, 0x5e);
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 0x62);
    c.set_reg(BX, bx);
    let al = c.reg8(r8::AL) as u16;
    c.xor(B, 0, 0);
    let a4 = c.shl1(W, al, 2);
    c.set_reg(AX, a4);
    let dx = c.add(W, dx, a4);
    c.set_reg(DX, dx);
    c.set_reg(SI, 5);
    c.set_reg(CX, 7);
    c.interp(0x57a7, 0x57ab)?;
    c.run(0x57ab, 2)?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.ret()
}

/// `loc_058fa`: leaving bit 40h of [46EBh]: the sprite list (DBE0h, or
/// DBE2h unless the first is 4710h's) dropped and its rect redrawn, the
/// mouse rect the view. Registers kept.
pub fn drop_point_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x58fa)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x46eb);
    c.run(start, 3)?;
    c.test(B, k as u16, 0x40);
    if k & 0x40 == 0 {
        return c.ret();
    }
    c.call(code::SAVE_REGS, 0x5904)?;
    let first = c.word(SPRITE_LIST) == POINT_4710;
    c.run(0x5904, 4 + (!first) as u32 + 3)?;
    let k = c.byte(0x46eb);
    let k = c.and(B, k as u16, 0xbf);
    c.set_byte(0x46eb, k as u8);
    let w = c.word(SPRITE_LIST);
    c.cmp(W, w, POINT_4710);
    let di = if first { SPRITE_LIST } else { SPRITE_LIST + 2 };
    c.set_reg(DI, di);
    let si = c.reg(SI);
    c.xor(W, si, si);
    let old = c.word(di);
    c.set_word(di, 0);
    c.set_reg(SI, old);
    c.call(code::PARTICLES_UPDATE_DIRTY_RECT, 0x591c)?;
    c.run(0x591c, 1)?;
    c.call(0x5ad9, 0x591f)?;
    c.run(0x591f, 1)?;
    c.call(code::RESTORE_REGS, 0x5922)?;
    c.run(0x5922, 1)?;
    c.ret()
}

/// From 5B6E (the stretch from `at`, `prefix` instructions before, done by
/// `ops`): a 4-wide frame around the rect at SI, colours AL, AL - 2, ...
fn frame_rect(c: &mut Ctx, at: u16, prefix: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, prefix + 5 + 5)?;
    ops(c);
    let si = c.reg(SI);
    let r = [c.word(si), c.word(si.wrapping_add(2)), c.word(si.wrapping_add(4)), c.word(si.wrapping_add(6))];
    c.set_reg(DX, r[0]);
    c.set_reg(BX, r[1]);
    c.set_reg(DI, r[2]);
    c.set_reg(CX, r[3]);
    c.set_reg(BP, 4);
    let mut first = true;
    loop {
        if !first {
            c.run(0x5b7c, 5)?;
        }
        first = false;
        let ax = c.reg(AX);
        c.push(ax);
        let bp = c.reg(BP);
        c.push(bp);
        let dx = c.reg(DX);
        let dx = c.dec(W, dx);
        c.set_reg(DX, dx);
        let bx = c.reg(BX);
        let bx = c.dec(W, bx);
        c.set_reg(BX, bx);
        c.call(0xc560, 0x5b83)?;
        let sp = c.reg(SP);
        let left = c.word_in(SS, sp).wrapping_sub(1);
        c.run(0x5b83, 7 + if left == 0 { 1 } else { 0 })?;
        let bp = c.pop();
        let ax = c.pop();
        c.set_reg(AX, ax);
        let di = c.reg(DI);
        let di = c.inc(W, di);
        c.set_reg(DI, di);
        let cx = c.reg(CX);
        let cx = c.inc(W, cx);
        c.set_reg(CX, cx);
        let al = c.sub(B, ax & 0xff, 2);
        c.set_reg(AX, ax & 0xff00 | al);
        let bp = c.dec(W, bp);
        c.set_reg(BP, bp);
        if bp == 0 {
            return c.ret();
        }
    }
}

/// `loc_05b6e`: see [`frame_rect`].
pub fn frame_rect_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5b6e)?;
    frame_rect(&mut Ctx::new(m), start, 0, |_| {})
}

/// `loc_05b69`: the view's frame, from colour FCh.
pub fn frame_view(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5b69)?;
    frame_rect(&mut Ctx::new(m), start, 2, |c| {
        c.set_reg(SI, VIEW);
        c.set_reg8(r8::AL, 0xfc);
    })
}

/// `loc_05bb0`: the map's caption: on the screen, the sprite list 194Ah,
/// `loc_07b1b`, phrase E2h with the number [28h] drawn (tall font, colour
/// F0h) at the list's place + (0Ah, 8); then the front buffer active.
pub fn draw_map_caption(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5bb0)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xc08e, 0x5bb3)?;
    c.run(0x5bb3, 3)?;
    c.set_reg(SI, SPRITE_LIST_MAP);
    c.set_word(SPRITE_LIST, SPRITE_LIST_MAP);
    c.call(0x7b1b, 0x5bbd)?;
    c.run(0x5bbd, 1)?;
    c.call(0xd068, 0x5bc0)?;
    c.run(0x5bc0, 2)?;
    c.set_reg(SI, 0xe2);
    c.call(0xcf70, 0x5bc6)?;
    c.run(0x5bc6, 1)?;
    c.call(0xd03c, 0x5bc9)?;
    c.run(0x5bc9, 3)?;
    let n = c.byte(0x28);
    c.set_reg8(r8::AL, n);
    c.xor(B, 0, 0);
    c.set_reg(AX, n as u16);
    c.call(0xe2e3, 0x5bd1)?;
    c.run(0x5bd1, 7)?;
    let (x, y) = (c.word(SPRITE_LIST_MAP), c.word(SPRITE_LIST_MAP + 2));
    let x = c.add(W, x, 0x0a);
    c.set_reg(DX, x);
    let y = c.add(W, y, 8);
    c.set_reg(BX, y);
    c.set_reg(CX, 0xf0);
    c.set_reg(AX, 0xe2);
    c.call(0xd194, 0x5be8)?;
    c.run(0x5be8, 1)?;
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `loc_05d50`: one more at [46ECh] for the location at DI shown in the
/// hotspot mode (not of bit 80h of byte 0Ah, `loc_062c9` in view). SI, DI
/// kept.
pub fn count_shown_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5d50)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let f = c.byte(di.wrapping_add(0x0a));
    let k = c.byte(0x46eb);
    let si = c.reg(SI);
    let n = if f & 0x80 != 0 { 7 } else if k == 0 { 9 } else { 8 };
    c.run(start, n)?;
    c.push(si);
    c.push(di);
    c.test(B, f as u16, 0x80);
    if f & 0x80 == 0 {
        c.cmp(B, k as u16, 0);
        if k != 0 {
            c.set_reg(SI, di);
            c.call(0x62c9, 0x5d64)?;
            let out = c.flag(CF);
            c.run(0x5d64, if out { 4 } else { 5 })?;
            if !out {
                let v = c.byte(0x46ec);
                let v = c.inc(B, v as u16);
                c.set_byte(0x46ec, v as u8);
            }
        }
    }
    let di = c.pop();
    c.set_reg(DI, di);
    let si = c.pop();
    c.set_reg(SI, si);
    c.ret()
}

/// `loc_05e42`: AX = the map icon of the location at SI: 3Ah (7Ah zoomed)
/// plus how many of 20h, 21h, 28h, 30h its kind reaches. CL = the kind.
pub fn location_icon(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5e42)?;
    let mut c = Ctx::new(m);
    let zoom = c.byte(0x46eb) & 0x80 != 0;
    let si = c.reg(SI);
    let kind = c.byte(si.wrapping_add(8));
    let reached = [0x20u8, 0x21, 0x28, 0x30].iter().take_while(|&&t| kind >= t).count() as u32;
    c.run(start, 3 + zoom as u32 + 1 + 3 * reached + if reached < 4 { 3 } else { 1 })?;
    let k = c.byte(0x46eb);
    c.test(B, k as u16, 0x80);
    let mut ax = if zoom { 0x7a } else { 0x3a };
    c.set_reg8(r8::CL, kind);
    for (i, t) in [0x20u16, 0x21, 0x28, 0x30].into_iter().enumerate() {
        c.cmp(B, kind as u16, t);
        if i as u32 >= reached {
            break;
        }
        ax = c.inc(W, ax);
    }
    c.set_reg(AX, ax);
    c.ret()
}

/// `loc_05f91`: the map's marks dropped ([46F8h], [46F7h]), the front
/// buffer active, the sprite list gone with its rect (1668h) redrawn,
/// then driver function 30 unless redrawing.
pub fn drop_map_marks(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5f91)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    c.set_word(0x46f8, 0);
    c.set_byte(0x46f7, 0);
    c.set_reg(SI, 0x1668);
    c.call(0xc07c, 0x5fa2)?;
    c.run(0x5fa2, 2)?;
    c.set_word(SPRITE_LIST, 0);
    c.call(code::PARTICLES_UPDATE_DIRTY_RECT, 0x5fab)?;
    c.run(0x5fab, 2)?;
    c.set_reg8(r8::AL, 8);
    super::troops::gfx_call_30_unless_redrawing(c.m, 0x7b2b)
}

/// `loc_062c9`: in the hotspot mode, CF set when the screen place of the
/// record at SI (its map point, `loc_0b647`) is outside the view; CF set
/// out of it ([46EBh] = 0).
pub fn record_outside_view(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x62c9)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x46eb);
    if k < 1 {
        c.run(start, 3)?;
        c.cmp(B, k as u16, 1);
        return c.ret();
    }
    c.run(start, 5)?;
    c.cmp(B, k as u16, 1);
    let si = c.reg(SI);
    let (x, y) = (c.word(si.wrapping_add(2)), c.word(si.wrapping_add(4)));
    c.set_reg(DX, x);
    c.set_reg(BX, y);
    c.call(0xb647, 0x62d9)?;
    outside_view_tail(&mut c)
}

/// `loc_0627e`: CF set when the location at DI is a place to show (bit 2
/// of byte 0Ah, or a kind past `location_kind_check` with troops counted
/// by `loc_05098`). Registers kept.
pub fn location_to_show(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x627e)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(code::SAVE_REGS, 0x6281)?;
    let di = c.reg(DI);
    let f = c.byte(di.wrapping_add(0x0a));
    let show = if f & 2 != 0 {
        c.run(0x6281, 3)?;
        c.test(B, f as u16, 2);
        true
    } else {
        c.run(0x6281, 3)?;
        c.test(B, f as u16, 2);
        c.call(0x5d36, 0x628a)?;
        if c.flag(CF) {
            c.run(0x628a, 2)?;
            false
        } else {
            c.run(0x628a, 2)?;
            c.call(0x5098, 0x628f)?;
            let dx = c.reg(DX);
            c.run(0x628f, 3)?;
            c.or(W, dx, dx);
            dx != 0
        }
    };
    c.call(code::RESTORE_REGS, if show { 0x6296 } else { 0x629b })?;
    c.run(if show { 0x6296 } else { 0x629b }, 2)?;
    c.set_flag(CF, show);
    c.ret()
}

/// `loc_06252`: AX = how the location at DI shows on the map: 1 a place to
/// show (`loc_0627e`), 2 one of a kind past `location_kind_check`; for
/// the others with bit 10h of byte 0Ah, by its type string (0 none or
/// type 3, 1 type 2, else 2).
pub fn location_map_kind(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6252)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x627e, 0x6255)?;
    if c.flag(CF) {
        c.run(0x6255, 3)?;
        c.set_reg(AX, 1);
        return c.ret();
    }
    c.run(0x6255, 2)?;
    c.call(0x5d36, 0x625a)?;
    let cf = c.flag(CF);
    let di = c.reg(DI);
    let f = c.byte(di.wrapping_add(0x0a));
    if !cf {
        c.run(0x625a, 3)?;
        c.set_reg(AX, 2);
        return c.ret();
    }
    if f & 0x10 == 0 {
        c.run(0x625a, 5)?;
        c.set_reg(AX, 2);
        c.test(B, f as u16, 0x10);
        return c.ret();
    }
    c.run(0x625a, 5)?;
    c.set_reg(AX, 2);
    c.test(B, f as u16, 0x10);
    c.call(0x6231, 0x6269)?;
    let ax = c.reg(AX);
    let al = ax as u8;
    let n = if ax == 0 { 3 } else if al == 3 { 6 } else if al != 2 { 8 } else { 9 };
    c.run(0x6269, n)?;
    c.or(W, ax, ax);
    if ax == 0 {
        return c.ret();
    }
    c.cmp(B, al as u16, 3);
    if al == 3 {
        c.xor(W, ax, ax);
        c.set_reg(AX, 0);
        return c.ret();
    }
    c.cmp(B, al as u16, 2);
    c.set_reg(AX, ax & 0xff00 | 2);
    if al == 2 {
        c.set_reg(AX, 1);
    }
    c.ret()
}

/// `loc_0634d`: for (DX, BX) in the view (`loc_062d6`), `loc_0636a` and
/// `loc_0639a` from it (the map row both ways); CF clear then, set when
/// outside. BX, DX kept.
pub fn scan_row_both_ways(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x634d)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.call(0x62d6, 0x6352)?;
    let out = c.flag(CF);
    c.run(0x6352, if out { 4 } else { 8 })?;
    let si = c.pop();
    c.set_reg(SI, si);
    let cx = c.pop();
    c.set_reg(CX, cx);
    if out {
        return c.ret();
    }
    for r in [BX, CX, DX, SI] {
        let v = c.reg(r);
        c.push(v);
    }
    c.call(0x636a, 0x635d)?;
    c.run(0x635d, 7)?;
    for r in [SI, DX, CX, BX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    let (cx, si) = (c.reg(CX), c.reg(SI));
    c.push(cx);
    c.push(si);
    c.call(0x639a, 0x6366)?;
    c.run(0x6366, 4)?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    let bx = c.pop();
    c.set_reg(BX, bx);
    c.set_flag(CF, false);
    c.ret()
}

/// `loc_0642e`: DX = how many of the 6 bytes from ES:SI - 1 have bits
/// 30h = 10h.
pub fn count_10_bytes(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x642e)?;
    let mut c = Ctx::new(m);
    let si = c.reg(SI).wrapping_sub(1);
    let step = c.string_step(W);
    let mut n = 3;
    let mut p = si;
    for _ in 0..3 {
        let w = c.word_in(ES, p);
        n += 4 + ((w >> 8) & 0x30 == 0x10) as u32 + 2 + (w & 0x30 == 0x10) as u32 + 1;
        p = p.wrapping_add(step);
    }
    c.run(start, n + 1)?;
    c.set_reg(CX, 3);
    c.set_reg(SI, si);
    c.xor(W, 0, 0);
    let mut dx = 0u16;
    for i in 0..3u16 {
        let w = c.lods(W, ES);
        let w = c.and(W, w, 0x3030);
        c.set_reg(AX, w);
        c.cmp(B, w >> 8, 0x10);
        if w >> 8 == 0x10 {
            dx = c.inc(W, dx);
        }
        c.cmp(B, w & 0xff, 0x10);
        if w & 0xff == 0x10 {
            dx = c.inc(W, dx);
        }
        c.set_reg(CX, 2 - i);
    }
    c.set_reg(DX, dx);
    c.ret()
}

/// `loc_080ac`: BP = 2136h; bit 40h of its byte 3 (through SS) set, CX 1,
/// when [4738h] is 1 or 2, else clear and CX 0. AL = [4738h] - 1, BX =
/// 824Dh.
pub fn map_mode_flag(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x80ac)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x4738);
    let on = k.wrapping_sub(1) < 2;
    c.run(start, 7 + if on { 0 } else { 2 } + 2)?;
    c.set_reg(BP, 0x2136);
    let v = c.byte_in(SS, 0x2139);
    let v = c.and(B, v as u16, 0xbf);
    c.m.cpu.write8(&mut c.m.hw, SS, 0x2139, v as u8);
    let cx = c.reg(CX);
    c.xor(W, cx, cx);
    c.set_reg(CX, 0);
    c.set_reg8(r8::AL, k);
    let al = c.dec(B, k as u16);
    c.set_reg8(r8::AL, al as u8);
    c.cmp(B, al, 2);
    if !on {
        let v = c.or(B, v, 0x40);
        c.m.cpu.write8(&mut c.m.hw, SS, 0x2139, v as u8);
        let cx = c.inc(W, 0);
        c.set_reg(CX, cx);
    }
    c.set_reg(BX, 0x824d);
    c.ret()
}

/// `loc_082b7`: in the map's hotspot mode (bit 40h of [46EBh]): marks
/// dropped (`loc_058fa`), then, registers kept, the screen redrawn to the
/// back buffer: panel 18E9h, the troops (`loc_068eb`), `loc_07be0`.
pub fn redraw_map_marks(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x82b7)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x46eb);
    if k & 0x40 == 0 {
        c.run(start, 3)?;
        c.test(B, k as u16, 0x40);
        return c.ret();
    }
    c.run(start, 3)?;
    c.test(B, k as u16, 0x40);
    c.call(0x58fa, 0x82c1)?;
    c.run(0x82c1, 1)?;
    c.call(code::SAVE_REGS, 0x82c4)?;
    c.run(0x82c4, 1)?;
    c.call(0xc08e, 0x82c7)?;
    c.run(0x82c7, 2)?;
    c.set_reg(SI, 0x18e9);
    c.call(0xc551, 0x82cd)?;
    c.run(0x82cd, 1)?;
    c.call(0x68eb, 0x82d0)?;
    c.run(0x82d0, 1)?;
    c.call(0x7be0, 0x82d3)?;
    c.run(0x82d3, 1)?;
    c.call(0xc07c, 0x82d6)?;
    c.run(0x82d6, 1)?;
    c.call(code::RESTORE_REGS, 0x82d9)?;
    c.run(0x82d9, 1)?;
    c.ret()
}

/// `loc_07bbe`: with [46F1h] = SI, [23h] = AL, [47BAh] = 0, to the back
/// buffer: `loc_096f1` with AX 0Fh and, unless CF, `loc_09efd`; then SI
/// back, in the hotspot mode `loc_09901`, and the front buffer active.
pub fn show_message_0f(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7bbe)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let (si, al) = (c.reg(SI), c.reg8(r8::AL));
    c.set_word(0x46f1, si);
    c.set_byte(0x23, al);
    c.set_word(0x47ba, 0);
    c.call(0xc08e, 0x7bce)?;
    c.run(0x7bce, 2)?;
    c.set_reg(AX, 0x0f);
    c.call(0x96f1, 0x7bd4)?;
    if c.flag(CF) {
        c.run(0x7bd4, 1)?;
    } else {
        c.run(0x7bd4, 2)?;
        c.call(0x9efd, 0x7bd9)?;
    }
    let k = c.byte(0x46eb);
    let si = c.word(0x46f1);
    if k & 0x80 != 0 {
        c.run(0x7bd9, 5)?;
        c.set_reg(SI, si);
        c.cmp(B, k as u16, 0);
        c.call(0x9901, 0x7c60)?;
        c.run(0x7c60, 1)?;
    } else {
        c.run(0x7bd9, 5)?;
        c.set_reg(SI, si);
        c.cmp(B, k as u16, 0);
    }
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `loc_057b2`: unless [4725h] is already FEh (it becomes that), to the
/// back buffer: the map frame (`loc_0557b`) and, at (BX + 62h, DX + 6),
/// sprite 65h (68h with [4722h]) by `loc_0dbb2`, `loc_0d194`; front
/// buffer active. BX, DX kept.
pub fn draw_map_header(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x57b2)?;
    let mut c = Ctx::new(m);
    map_header_from(&mut c, start, 1, 0xf5fe, None)
}

/// From 57B5 with CX (its low byte the header's state), after `pre`
/// instructions of the stretch that starts at `at` (`ax`: AX they left).
fn map_header_from(c: &mut Ctx, at: u16, pre: u32, cx: u16, ax: Option<u16>) -> Result<(), Bail> {
    let cl = cx as u8;
    let old = c.byte(0x4725);
    if old == cl {
        c.run(at, pre + 5)?;
        if let Some(a) = ax {
            c.set_reg(AX, a);
        }
        c.set_reg(CX, cx);
        c.set_reg8(r8::AL, old);
        c.cmp(B, old as u16, cl as u16);
        return c.ret();
    }
    c.run(at, pre + 7)?;
    if let Some(a) = ax {
        c.set_reg(AX, a);
    }
    c.set_reg(CX, cx);
    c.set_byte(0x4725, cl);
    c.set_reg8(r8::AL, old);
    c.cmp(B, old as u16, cl as u16);
    let (bx, dx) = (c.reg(BX), c.reg(DX));
    c.push(bx);
    c.push(dx);
    c.call(0xc08e, 0x57c4)?;
    c.run(0x57c4, 1)?;
    c.call(0x557b, 0x57c7)?;
    let k = c.byte(0x4722);
    c.run(0x57c7, 6 + (k != 0) as u32)?;
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 0x62);
    c.set_reg(BX, bx);
    let dx = c.reg(DX);
    let dx = c.add(W, dx, 6);
    c.set_reg(DX, dx);
    c.set_reg(AX, if k != 0 { 0x68 } else { 0x65 });
    c.cmp(B, k as u16, 0);
    c.call(0xdbb2, 0x57dd)?;
    c.run(0x57dd, 1)?;
    c.call(0xd194, 0x57e0)?;
    c.run(0x57e0, 3)?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    let bx = c.pop();
    c.set_reg(BX, bx);
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `loc_05a1a`: the map screen opened: [28E7h] = 1, `loc_018ba`,
/// `loc_05b5d`, the transition C108h (BP 5A56h, AL 34h, DX FFFFh), unless
/// [46F3h] `loc_05bb0`; then the head animates up.
pub fn open_map_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5a1a)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(0x28e7, 1);
    c.call(0x18ba, 0x5a22)?;
    c.run(0x5a22, 1)?;
    c.call(0x5b5d, 0x5a25)?;
    c.run(0x5a25, 4)?;
    c.set_reg(BP, 0x5a56);
    c.set_reg8(r8::AL, 0x34);
    c.set_reg(DX, 0xffff);
    c.call(0xc108, 0x5a30)?;
    let k = c.byte(0x46f3);
    if k != 0 {
        c.run(0x5a30, 3)?;
        c.cmp(B, k as u16, 0);
    } else {
        c.run(0x5a30, 3)?;
        c.cmp(B, k as u16, 0);
        c.call(0x5bb0, 0x5a3a)?;
        c.run(0x5a3a, 1)?;
    }
    super::ui::ui_head_animate_up(c.m, 0x17e6)
}

/// `loc_056ed`: with BL set, at (([4712h] + 62h, AX + 82h) by
/// `loc_0c2fd`) DX + 6: the text `loc_0d04e`, ":" and the digit BL by the
/// character writer [2518h]; DX then 12h on. CX kept.
pub fn draw_map_label(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x56ed)?;
    let mut c = Ctx::new(m);
    let bx = c.reg(BX);
    if bx & 0xff == 0 {
        c.run(start, 3)?;
        c.or(B, 0, 0);
        return c.ret();
    }
    c.run(start, 9)?;
    c.or(B, bx & 0xff, bx & 0xff);
    let (cx, dx) = (c.reg(CX), c.reg(DX));
    c.push(cx);
    c.push(dx);
    c.push(bx);
    let b = c.word(0x4712);
    let b = c.add(W, b, 0x62);
    c.set_reg(BX, b);
    let ax = c.reg(AX);
    let ax = c.add(W, ax, 0x82);
    c.set_reg(AX, ax);
    c.call(0xc2fd, 0x5701)?;
    c.run(0x5701, 2)?;
    let dx = c.reg(DX);
    let dx = c.add(W, dx, 6);
    c.set_reg(DX, dx);
    c.call(0xd04e, 0x5707)?;
    c.run(0x5707, 2)?;
    c.set_reg8(r8::AL, 0x3a);
    let t = c.word(0x2518);
    c.call(t, 0x570d)?;
    c.run(0x570d, 3)?;
    let ax = c.pop();
    let al = c.add(B, ax & 0xff, 0x30);
    c.set_reg(AX, ax & 0xff00 | al);
    let t = c.word(0x2518);
    c.call(t, 0x5714)?;
    c.run(0x5714, 4)?;
    let dx = c.pop();
    let cx = c.pop();
    c.set_reg(CX, cx);
    let dx = c.add(W, dx, 0x12);
    c.set_reg(DX, dx);
    c.ret()
}

/// `loc_063c7`: sprite 78h (79h for AH 10h) by `draw_sprite_centred` at
/// (BX + DI & 3, DX + (BP + DI) / 4 & 3). Registers kept.
pub fn draw_map_dot(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x63c7)?;
    let mut c = Ctx::new(m);
    let ah = c.reg8(r8::AH);
    let eq = ah == 0x10;
    c.run(start, 17 + eq as u32)?;
    let (bx, dx, si, di, bp) = (c.reg(BX), c.reg(DX), c.reg(SI), c.reg(DI), c.reg(BP));
    let es = c.sreg(ES);
    for v in [bx, dx, si, di, bp, es] {
        c.push(v);
    }
    c.cmp(B, ah as u16, 0x10);
    let mut ax = 0x78;
    if eq {
        ax = c.inc(W, ax);
    }
    c.set_reg(AX, ax);
    let p = c.add(W, bp, di);
    let d = c.and(W, di, 3);
    let p = c.shr1(W, p, 2);
    let p = c.and(W, p, 3);
    c.set_reg(BP, p);
    c.set_reg(DI, d);
    let b = c.add(W, bx, d);
    c.set_reg(BX, b);
    let y = c.add(W, dx, p);
    c.set_reg(DX, y);
    c.call(0xc343, 0x63e9)?;
    c.run(0x63e9, 7)?;
    let v = c.pop();
    c.set_sreg(ES, v);
    for r in [BP, DI, SI, DX, BX] {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// `loc_07dd9`: the two rects at 1936h and 1940h (through SS; [0DBE2h] =
/// 1940h) placed from the panel rect 18E9h.
pub fn place_panel_rects(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7dd9)?;
    let mut c = Ctx::new(m);
    let (x0, y0, x1, y1) = (c.word(0x18e9), c.word(0x18eb), c.word(0x18ed), c.word(0x18ef));
    let mut b = y1.wrapping_add(1);
    let wrap = b >= 0x70;
    c.run(start, 20 + wrap as u32 + 4)?;
    c.set_reg(SI, 0x18e9);
    c.set_reg(DI, 0x1936);
    c.set_reg(BP, 0x1940);
    c.set_word(0xdbe2, 0x1940);
    let a = x0.wrapping_add(0x30);
    c.set_word_in(SS, 0x1940, a);
    c.set_word(0x1936, a.wrapping_add(0x4d));
    c.set_word(0x193a, x1);
    c.set_word_in(SS, 0x1944, x1);
    c.set_word(0x1938, y0.wrapping_add(0x2b));
    c.set_word(0x193c, y1);
    if wrap {
        b = b.wrapping_sub(0x6d);
    }
    c.set_word_in(SS, 0x1942, b);
    let ax = c.add(W, b, 0x28);
    c.set_reg(AX, ax);
    c.set_word_in(SS, 0x1946, ax);
    c.ret()
}

/// `loc_08256`: the troops redrawn (`loc_068eb`); for the map list SI
/// 8E0h, the location DI (a sietch, below 20h, or with bit 8 of byte 0Ah)
/// added to the 3 picks at 4718h (the oldest dropped by `loc_080c8` when
/// full), with `loc_0542f`, `loc_080ac`, `loc_0d33a`; after the third a
/// pause of 32h, CF clear. CF set for one not taken (or more to pick).
pub fn pick_map_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8256)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x68eb, 0x8259)?;
    let (si, di) = (c.reg(SI), c.reg(DI));
    if si != 0x8e0 {
        c.run(0x8259, 4)?;
        c.cmp(W, si, 0x8e0);
        c.set_flag(CF, false);
        return c.ret();
    }
    let al = c.byte(di.wrapping_add(8));
    let f = c.byte(di.wrapping_add(0x0a));
    if al >= 0x20 && f & 8 == 0 {
        c.run(0x8259, 9)?;
        c.cmp(W, si, 0x8e0);
        c.set_reg8(r8::AL, al);
        c.cmp(B, al as u16, 0x20);
        c.test(B, f as u16, 8);
        c.set_flag(CF, true);
        return c.ret();
    }
    let k = c.byte(0x4738);
    let n = 5 + if al >= 0x20 { 2 } else { 0 } + 2;
    c.run(0x8259, n + (k >= 3) as u32)?;
    c.cmp(W, si, 0x8e0);
    c.set_reg8(r8::AL, al);
    c.cmp(B, al as u16, 0x20);
    if al >= 0x20 {
        c.test(B, f as u16, 8);
    }
    c.cmp(B, k as u16, 3);
    if k >= 3 {
        c.call(0x80c8, 0x8276)?;
    }
    c.run(0x8276, 6)?;
    let bx = c.reg(BX);
    c.xor(W, bx, bx);
    let k = c.byte(0x4738);
    let k2 = c.inc(B, k as u16);
    c.set_byte(0x4738, k2 as u8);
    let bx = c.shl1(W, k as u16, 1);
    c.set_reg(BX, bx);
    let di = c.reg(DI);
    c.set_word(bx.wrapping_add(0x4718), di);
    c.call(0x542f, 0x8289)?;
    c.run(0x8289, 1)?;
    c.call(0x80ac, 0x828c)?;
    c.run(0x828c, 1)?;
    c.call(0xd33a, 0x828f)?;
    let k = c.byte(0x4738);
    if k < 3 {
        c.run(0x828f, 4)?;
        c.cmp(B, k as u16, 3);
        c.set_flag(CF, true);
        return c.ret();
    }
    c.run(0x828f, 4)?;
    c.cmp(B, k as u16, 3);
    c.set_reg(AX, 0x32);
    c.call(0xe3a0, 0x829e)?;
    c.run(0x829e, 2)?;
    c.set_flag(CF, false);
    c.ret()
}

/// `loc_09bb1`: `loc_09bee`; with no [4540h] the rect D834h from 1BF0h
/// (`loc_05b99`, its first word then 80h), else `loc_09c2d` (done when
/// [0D834h] is 13Fh); the rect's bottom at most 98h, then `loc_0c446`,
/// `loc_09d16`. SI kept.
pub fn dialogue_frame_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x9bb1)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x9bee, 0x9bb4)?;
    let si = c.reg(SI);
    let w = c.word(0x4540);
    if w == 0 {
        c.run(0x9bb4, 6)?;
        c.push(si);
        c.cmp(W, w, 0);
        c.set_reg(SI, 0x1bf0);
        c.set_reg(DI, 0xd834);
        c.call(0x5b99, 0x9bc5)?;
        c.run(0x9bc5, 2)?;
        let s = c.reg(SI);
        c.set_word(s, 0x80);
    } else {
        c.run(0x9bb4, 4)?;
        c.push(si);
        c.cmp(W, w, 0);
        c.call(0x9c2d, 0x9bcf)?;
        let x = c.word(0xd834);
        if x == 0x13f {
            c.run(0x9bcf, 4)?;
            c.cmp(W, x, 0x13f);
            let s = c.pop();
            c.set_reg(SI, s);
            return c.ret();
        }
        c.run(0x9bcf, 2)?;
        c.cmp(W, x, 0x13f);
    }
    let b = c.word(0xd83a);
    c.run(0x9bd7, 4 + (b >= 0x98) as u32)?;
    c.set_reg(SI, 0xd834);
    c.cmp(W, b, 0x98);
    if b >= 0x98 {
        c.set_word(0xd83a, 0x98);
    }
    c.call(0xc446, 0x9be9)?;
    c.run(0x9be9, 1)?;
    c.call(0x9d16, 0x9bec)?;
    c.run(0x9bec, 2)?;
    let s = c.pop();
    c.set_reg(SI, s);
    c.ret()
}

/// `loc_064ef`: the handler CS:[64EDh] called for (DI, SI, the span DX -
/// DI + 1), then for (DI, the span, SI) in BX. Registers kept.
pub fn span_handler_twice(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x64ef)?;
    let mut c = Ctx::new(m);
    c.run(start, 15)?;
    let regs = [AX, BX, CX, DX, SI, DI, BP];
    for r in regs {
        let v = c.reg(r);
        c.push(v);
    }
    let (dx, di, si) = (c.reg(DX), c.reg(DI), c.reg(SI));
    let cx = c.sub(W, dx, di);
    let cx = c.inc(W, cx);
    c.set_reg(CX, cx);
    c.set_reg(DX, di);
    c.push(si);
    c.push(cx);
    c.push(di);
    let t = c.code_word(0x64ed);
    c.call(t, 0x6505)?;
    c.run(0x6505, 4)?;
    let dx = c.pop();
    c.set_reg(DX, dx);
    let cx = c.pop();
    c.set_reg(CX, cx);
    let bx = c.pop();
    c.set_reg(BX, bx);
    let t = c.code_word(0x64ed);
    c.call(t, 0x650d)?;
    c.run(0x650d, 8)?;
    for r in regs.into_iter().rev() {
        let v = c.pop();
        c.set_reg(r, v);
    }
    c.ret()
}

/// `loc_055c0`: [`unmark_map_cells`] for the region byte 10h of each
/// location record from 100h (1Ch each, to an FFh) whose byte 0 is BL;
/// for BL 7 also for region 42h.
pub fn unmark_regions_of(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x55c0)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    let bl = c.reg8(r8::BL);
    let mut si = 0x100u16;
    c.set_reg(SI, si);
    loop {
        let b = c.byte(si);
        if b == bl {
            c.run(0x55c3, 4)?;
            c.cmp(B, b as u16, bl as u16);
            let al = c.byte(si.wrapping_add(0x10));
            c.set_reg8(r8::AL, al);
            c.call(0x55dd, 0x55cd)?;
            si = c.reg(SI);
        } else {
            c.run(0x55c3, 2)?;
            c.cmp(B, b as u16, bl as u16);
        }
        let s = si.wrapping_add(0x1c);
        let nb = c.byte(s);
        c.run(0x55cd, 3)?;
        let s = c.add(W, si, 0x1c);
        c.set_reg(SI, s);
        c.cmp(B, nb as u16, 0xff);
        si = s;
        if nb == 0xff {
            break;
        }
    }
    let bl = c.reg8(r8::BL);
    if bl == 7 {
        c.run(0x55d5, 3)?;
        c.set_reg8(r8::AL, 0x42);
        c.cmp(B, bl as u16, 7);
        return unmark_cells(&mut c);
    }
    c.run(0x55d5, 4)?;
    c.set_reg8(r8::AL, 0x42);
    c.cmp(B, bl as u16, 7);
    c.ret()
}

/// `loc_055dd`: every map cell of the region AL (bytes of the table
/// ES:DI = [0DBB0h], C5F9h of them) whose byte at DS = [0DD00h] has bits
/// 30h set loses bit 10h. DS kept.
pub fn unmark_map_cells(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x55dd)?;
    let mut c = Ctx::new(m);
    unmark_cells(&mut c)
}

fn unmark_cells(c: &mut Ctx) -> Result<(), Bail> {
    let al = c.reg8(r8::AL);
    if al == 0 {
        c.run(0x55dd, 5)?;
        let ds = c.sreg(DS);
        c.push(ds);
        c.or(B, 0, 0);
        let ds = c.pop();
        c.set_sreg(DS, ds);
        return c.ret();
    }
    c.run(0x55dd, 7)?;
    let ds = c.sreg(DS);
    c.push(ds);
    c.or(B, al as u16, al as u16);
    let (di, es, nds) = (c.word(0xdbb0), c.word(0xdbb2), c.word(0xdd00));
    c.set_reg(DI, di);
    c.set_sreg(ES, es);
    c.set_sreg(DS, nds);
    c.set_reg(CX, 0xc5f9);
    loop {
        c.m.cpu.string_op(&mut c.m.hw, 0xae, None, 0xf2); // repne scasb
        if !c.flag(pc::cpu::ZF) {
            c.run(0x55ef, 3)?;
            break;
        }
        let p = c.reg(DI).wrapping_sub(1);
        let b = c.byte(p);
        let hit = b & 0x30 == 0x30;
        let cx = c.reg(CX);
        let more = cx != 0;
        c.run(0x55ef, 5 + hit as u32 + 2 + if more { 1 } else { 2 })?;
        let ah = c.and(B, b as u16, 0x30);
        c.set_reg8(r8::AH, ah as u8);
        c.cmp(B, ah, 0x30);
        if hit {
            let v = c.and(B, b as u16, 0xef);
            c.set_byte(p, v as u8);
        }
        let cx1 = c.inc(W, cx);
        c.set_reg(CX, cx1.wrapping_sub(1));
        if !more {
            break;
        }
    }
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}

/// `loc_04b5f`: the view rect from (DX, BX) and its size ([487Ah],
/// [487Ch]), each corner clamped (signed) to the bounds at 1470h:
/// [4860h], [4862h] the start, [4864h], [4866h] the end.
pub fn clamp_view_rect(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4b5f)?;
    let mut c = Ctx::new(m);
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let b = [c.word(0x1470), c.word(0x1472), c.word(0x1474), c.word(0x1476)];
    let (w, h) = (c.word(0x487a), c.word(0x487c));
    let clamp = |v: u16, lo: u16, hi: u16, n: &mut u32| -> u16 {
        let mut v = v;
        *n += 2;
        if (v as i16) < lo as i16 {
            v = lo;
            *n += 1;
        }
        *n += 2;
        if (v as i16) > hi as i16 {
            v = hi;
            *n += 1;
        }
        v
    };
    let mut n = 3;
    let x0 = clamp(dx, b[0], b[2], &mut n);
    n += 1;
    let x1 = clamp(dx.wrapping_add(w), b[0], b[2], &mut n);
    n += 1 + 2;
    let y0 = clamp(bx, b[1], b[3], &mut n);
    n += 1;
    let y1 = clamp(bx.wrapping_add(h), b[1], b[3], &mut n);
    n += 1 + 1;
    c.run(start, n)?;
    c.set_reg(SI, 0x1470);
    let replay = |c: &mut Ctx, v: u16, lo: u16, hi: u16| -> u16 {
        c.cmp(W, v, lo);
        let v = if (v as i16) < lo as i16 { lo } else { v };
        c.cmp(W, v, hi);
        if (v as i16) > hi as i16 { hi } else { v }
    };
    let ax = c.add(W, dx, w);
    let v = replay(&mut c, dx, b[0], b[2]);
    c.set_reg(DX, v);
    c.set_word(0x4860, x0);
    let v = replay(&mut c, ax, b[0], b[2]);
    c.set_word(0x4864, x1);
    let ax = c.add(W, bx, h);
    let v2 = replay(&mut c, bx, b[1], b[3]);
    c.set_reg(BX, v2);
    c.set_word(0x4862, y0);
    let v3 = replay(&mut c, ax, b[1], b[3]);
    c.set_reg(AX, v3);
    c.set_word(0x4866, y1);
    let _ = v;
    c.ret()
}

/// `loc_0600e`: the location DI's caption on the back buffer ([46F8h] =
/// DI after `loc_05ee4`): its type and name (`draw_string_location_type`,
/// `draw_location_name`, tall font) by the frame at 1668h, and for a kind
/// other than 2 (`location_map_kind`) `loc_060ac`, with `loc_0605c` first
/// for kind 0 when [0Ah] has bit 20h; front buffer active.
pub fn draw_location_caption(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x600e)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xc08e, 0x6011)?;
    c.run(0x6011, 2)?;
    let di = c.reg(DI);
    c.push(di);
    c.call(0x5ee4, 0x6015)?;
    c.run(0x6015, 3)?;
    let di = c.pop();
    c.set_reg(DI, di);
    c.set_word(0x46f8, di);
    c.call(0xd068, 0x601d)?;
    c.run(0x601d, 7)?;
    let ch = c.byte(0x1671);
    c.set_reg(CX, (ch as u16) << 8 | 0x9a);
    let dx = c.word(0x1668);
    let bx = c.word(0x166a);
    let dx = c.add(W, dx, 0x0c);
    c.set_reg(DX, dx);
    let bx = c.add(W, bx, 4);
    c.set_reg(BX, bx);
    c.call(0x629d, 0x6034)?;
    c.run(0x6034, 4)?;
    c.set_reg8(r8::CL, 0x96);
    let dx = c.reg(DX);
    let dx = c.sub(W, dx, 8);
    c.set_reg(DX, dx);
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 9);
    c.set_reg(BX, bx);
    c.call(0x62a6, 0x603f)?;
    c.run(0x603f, 1)?;
    c.call(0x6252, 0x6042)?;
    let al = c.reg8(r8::AL);
    if al == 2 {
        c.run(0x6042, 3)?;
        c.cmp(B, 2, 2);
        return super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c);
    }
    let f = c.byte(0x0a);
    let pre = f & 0x20 != 0 && al == 0;
    let n = 2 + 2 + if f & 0x20 != 0 { 2 } else { 0 } + if pre { 2 } else { 0 } + if pre { 0 } else { 1 };
    c.run(0x6042, n)?;
    c.cmp(B, al as u16, 2);
    c.test(B, f as u16, 0x20);
    if f & 0x20 != 0 {
        c.or(B, al as u16, al as u16);
    }
    if pre {
        let ax = c.reg(AX);
        c.push(ax);
        c.call(0x605c, 0x6055)?;
        c.run(0x6055, 2)?;
        let ax = c.pop();
        c.set_reg(AX, ax);
    }
    c.call(0x60ac, 0x6059)?;
    c.run(0x6059, 1)?;
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `loc_07d0c`: the skills panel on the back buffer: the rects 1936h and
/// 1940h (`loc_07b1b`), `loc_07e1e`, the needs 4C60h saved to 4C7Ch, the
/// bars (`loc_07e3d` from 46FEh), the label 6Fh (`loc_0d194`),
/// `fade_panel` (registers kept); front buffer active.
pub fn draw_skills_panel(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7d0c)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xc08e, 0x7d0f)?;
    c.run(0x7d0f, 2)?;
    c.set_reg(SI, 0x1936);
    c.call(0x7b1b, 0x7d15)?;
    c.run(0x7d15, 2)?;
    c.set_reg(SI, 0x1940);
    c.call(0x7b1b, 0x7d1b)?;
    c.run(0x7d1b, 1)?;
    c.call(0x7e1e, 0x7d1e)?;
    c.run(0x7d1e, 6)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(SI, 0x4c60);
    c.set_reg(DI, 0x4c7c);
    c.set_reg(CX, 0x0e);
    c.rep(0xa5);
    c.run(0x7d2b, 9)?;
    let bx = c.word(0x1942);
    let bx = c.add(W, bx, 4);
    c.set_reg(BX, bx);
    let dx = c.word(0x1940);
    let dx = c.add(W, dx, 0x50);
    c.set_reg(DX, dx);
    c.set_reg(SI, 0x46fe);
    let bp = c.word(0x1946);
    c.set_reg(BP, bp);
    let cx = c.word(0x1948);
    c.set_reg(CX, cx);
    c.set_word(0xdbe4, cx);
    c.call(0x7e3d, 0x7d4b)?;
    c.run(0x7d4b, 5)?;
    let cx = c.word(0x1948);
    c.set_reg(CX, cx);
    let dx = c.word(0x1940);
    let dx = c.add(W, dx, 8);
    c.set_reg(DX, dx);
    c.set_reg(AX, 0x6f);
    c.call(0xd194, 0x7d5c)?;
    c.run(0x7d5c, 1)?;
    c.call(0xe270, 0x7d5f)?;
    c.run(0x7d5f, 1)?;
    c.call(0xd280, 0x7d62)?;
    c.run(0x7d62, 1)?;
    c.call(0xe283, 0x7d65)?;
    c.run(0x7d65, 1)?;
    super::video::set_fb1_as_active_framebuffer(c.m, 0xc07c)
}

/// `loc_04c45`: the sprite list AX - 2 of the table at [486Ch]: each
/// entry (sprite, x, y; to a 0) drawn by the driver's blit (far
/// [38CDh]) at (x + [486Eh], y + [4870h]) from the sprite table
/// [0DBB0h] to the screen [0DBDAh], clipped by 4860h.
pub fn draw_sprite_list(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4c45)?;
    let mut c = Ctx::new(m);
    c.run(start, 5)?;
    let ax = c.reg(AX);
    let ax = c.sub(W, ax, 2);
    let ax = c.shl1(W, ax, 1);
    c.set_reg(AX, ax);
    c.set_reg(BP, ax);
    let base = c.word_in(SS, 0x486c);
    let off = c.word_in(DS, ax.wrapping_add(base));
    let si = c.add(W, base, off);
    c.set_reg(SI, si);
    loop {
        let si = c.reg(SI);
        let al = c.byte(si);
        if al == 0 {
            c.run(0x4c54, 4)?;
            c.lods(B, DS);
            c.or(B, 0, 0);
            return c.ret();
        }
        c.run(0x4c54, 23)?;
        c.lods(B, DS);
        c.or(B, al as u16, al as u16);
        c.xor(B, 0, 0);
        c.set_reg(AX, al as u16);
        let x = c.lods(B, DS);
        let y = c.lods(B, DS);
        let ox = c.word_in(SS, 0x486e);
        let dx = c.add(W, x, ox);
        c.set_reg(DX, dx);
        let oy = c.word_in(SS, 0x4870);
        let bx = c.add(W, y, oy);
        c.set_reg(BX, bx);
        let si = c.reg(SI);
        c.push(si);
        let bp = c.dec(W, al as u16);
        let es = c.word_in(SS, 0xdbda);
        c.set_sreg(ES, es);
        let tab = c.word_in(SS, 0xdbb0);
        let bp = c.shl1(W, bp, 1);
        let off = c.word_in(DS, bp.wrapping_add(tab));
        let s = c.add(W, tab, off);
        c.set_reg(SI, s);
        let w = c.lods(W, DS);
        c.set_reg(DI, w);
        let h = c.lods(W, DS);
        c.xor(B, 0, 0);
        let h = h & 0xff;
        c.set_reg(AX, h);
        c.set_reg(CX, h);
        c.set_reg(BP, 0x4860);
        c.interp(0x4c89, 0x4c8e)?;
        c.run(0x4c8e, 2)?;
        let s = c.pop();
        c.set_reg(SI, s);
    }
}

/// `loc_05584`: the pending region marks [115Ah] taken (cleared): for
/// each of its 12 bits from the bottom (region 13 - CX), unless one of
/// that region's locations from 100h passes `location_kind_check`,
/// `unmark_regions_of` (BL the region). Registers and DS, ES kept.
pub fn unmark_pending_regions(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5584)?;
    let mut c = Ctx::new(m);
    let pend = c.word(0x115a);
    if pend == 0 {
        c.run(start, 5)?;
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
        c.or(W, 0, 0);
        return c.ret();
    }
    c.run(start, 5)?;
    c.xor(W, 0, 0);
    c.set_word(0x115a, 0);
    c.set_reg(AX, pend);
    c.or(W, pend, pend);
    c.call(0xe270, 0x5591)?;
    c.run(0x5591, 3)?;
    let (ds, es) = (c.sreg(DS), c.sreg(ES));
    c.push(ds);
    c.push(es);
    c.set_reg8(r8::CL, 0x0c);
    loop {
        let ax = c.reg(AX);
        let bit = ax & 1 != 0;
        c.run(0x5595, 2)?;
        let ax = c.shr1(W, ax, 1);
        c.set_reg(AX, ax);
        if bit {
            c.run(0x5599, 5)?;
            c.push(ax);
            let cx = c.reg(CX);
            c.push(cx);
            let bl = c.sub(B, 0x0d, cx & 0xff);
            c.set_reg8(r8::BL, bl as u8);
            let mut di = 0x100u16;
            c.set_reg(DI, di);
            let mut found = false;
            loop {
                let b = c.byte(di);
                if b == bl as u8 {
                    c.run(0x55a2, 3)?;
                    c.cmp(B, b as u16, bl);
                    c.call(0x5d36, 0x55a9)?;
                    let cf = c.flag(CF);
                    c.run(0x55a9, 1)?;
                    if !cf {
                        found = true;
                        break;
                    }
                    di = c.reg(DI);
                } else {
                    c.run(0x55a2, 2)?;
                    c.cmp(B, b as u16, bl);
                }
                let nd = di.wrapping_add(0x1c);
                let nb = c.byte(nd);
                c.run(0x55ab, 3)?;
                let d = c.add(W, di, 0x1c);
                c.set_reg(DI, d);
                c.cmp(B, nb as u16, 0xff);
                di = d;
                if nb == 0xff {
                    break;
                }
            }
            if !found {
                c.run(0x55b3, 1)?;
                c.call(0x55c0, 0x55b6)?;
            }
            c.run(0x55b6, 2)?;
            let cx = c.pop();
            c.set_reg(CX, cx);
            let ax = c.pop();
            c.set_reg(AX, ax);
        }
        let cx = c.reg(CX).wrapping_sub(1);
        c.run(0x55b8, if cx != 0 { 1 } else { 4 })?;
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    let es = c.pop();
    c.set_sreg(ES, es);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.call(0xe283, 0x55bf)?;
    c.run(0x55bf, 1)?;
    c.ret()
}

/// `loc_04a5a`: `loc_0c137`, then on the back buffer (`loc_0c085`; the
/// front pointer [0DBDAh] kept) the ring of map points at CS:[149Ah]
/// (E40Ch..E85Ch, 4 bytes each): each with a mark (AH 0) inside the view
/// (`loc_062d6`) and the map frame (CCh..139h, 4..39h, after -1) gets
/// sprite 2Fh (`loc_0c22f`).
pub fn draw_map_points(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4a5a)?;
    let mut c = Ctx::new(m);
    draw_map_points_at(&mut c)
}

fn draw_map_points_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x4a5a, 1)?;
    c.call(0xc137, 0x4a5d)?;
    c.run(0x4a5d, 2)?;
    let fb = c.word(0xdbda);
    c.push(fb);
    c.call(0xc085, 0x4a64)?;
    c.run(0x4a64, 1)?;
    let head = c.word(0x149a);
    c.set_reg(SI, head);
    loop {
        let si = c.reg(SI);
        let step = c.string_step(W);
        let w1 = c.code_word(si.wrapping_add(step));
        let mark = (w1 >> 8) as u8 == 0 || (w1 >> 8) as u8 >= 0x81;
        let _ = mark;
        let neg = ((w1 >> 8) as u8).wrapping_sub(1) & 0x80 != 0;
        c.run(0x4a68, 6 + if neg { 2 } else { 0 })?;
        let dx = c.lods(W, CS);
        c.set_reg(DX, dx);
        let bx = c.lods(W, CS);
        c.set_reg(BX, bx);
        let ah = c.dec(B, bx >> 8);
        c.set_reg(AX, ah << 8 | bx & 0xff);
        if neg {
            let si = c.reg(SI);
            c.push(si);
            c.call(0x62d6, 0x4a78)?;
            if c.flag(CF) {
                c.run(0x4a78, 2)?;
            } else {
                let (bx, dx) = (c.reg(BX), c.reg(DX));
                let (x, y) = (dx.wrapping_sub(1), bx.wrapping_sub(1));
                let (x, y) = (x as i16, y as i16);
                let n = if x < 0xcc {
                    4
                } else if y < 4 {
                    6
                } else if x >= 0x13a {
                    8
                } else if y >= 0x3a {
                    10
                } else {
                    0
                };
                if n != 0 {
                    c.run(0x4a78, 1 + n + 1)?;
                } else {
                    c.run(0x4a78, 1 + 10 + 2)?;
                }
                let y = c.dec(W, bx);
                c.set_reg(BX, y);
                let x = c.dec(W, dx);
                c.set_reg(DX, x);
                c.cmp(W, x, 0xcc);
                if n == 0 || n > 4 {
                    c.cmp(W, y, 4);
                    if n == 0 || n > 6 {
                        c.cmp(W, x, 0x13a);
                        if n == 0 || n > 8 {
                            c.cmp(W, y, 0x3a);
                        }
                    }
                }
                if n == 0 {
                    c.set_reg(AX, 0x2f);
                    c.call(0xc22f, 0x4a98)?;
                    c.run(0x4a98, 1)?;
                }
            }
            let s = c.pop();
            c.set_reg(SI, s);
        }
        let si = c.reg(SI);
        let wrap = si >= 0xe85c;
        let si2 = if wrap { 0xe40c } else { si };
        let more = si2 != head;
        c.run(0x4a99, 2 + wrap as u32 + 2 + if more { 0 } else { 2 })?;
        c.cmp(W, si, 0xe85c);
        c.set_reg(SI, si2);
        c.cmp(W, si2, head);
        if !more {
            break;
        }
    }
    let fb = c.pop();
    c.set_word(0xdbda, fb);
    c.ret()
}

/// `loc_049d4`: `loc_04988`, then [`draw_map_points`].
pub fn map_points_after_4988(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x49d4)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x4988, 0x49d7)?;
    c.run(0x49d7, 2)?;
    draw_map_points_at(&mut c)
}

/// `sub_599f`: with [4723h] set (taken, cleared): the point 4710h
/// (`get_point_4710`); at (DX, BX) = [11C1h], [11C3h] still,
/// [`gfx_call_3961`], else [0DBE0h] 0 and [`after_8850`].
pub fn map_point_moved(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x599f)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x4723);
    if k == 0 {
        c.run(start, 5)?;
        c.xor(B, 0, 0);
        c.set_reg8(r8::AL, 0);
        c.or(B, 0, 0);
        return c.ret();
    }
    c.run(start, 5)?;
    c.xor(B, 0, 0);
    c.set_byte(0x4723, 0);
    c.set_reg8(r8::AL, k);
    c.or(B, k as u16, k as u16);
    c.call(0x557b, 0x59ac)?;
    let (dx, bx) = (c.reg(DX), c.reg(BX));
    let (x, y) = (c.word(0x11c1), c.word(0x11c3));
    if dx == x && bx == y {
        c.run(0x59ac, 4)?;
        c.cmp(W, dx, x);
        c.cmp(W, bx, y);
        return gfx_call_3961(c.m, 0x5982);
    }
    c.run(0x59ac, if dx != x { 4 } else { 6 })?;
    c.cmp(W, dx, x);
    if dx == x {
        c.cmp(W, bx, y);
    }
    c.set_word(0xdbe0, 0);
    after_8850(c.m, 0x5b10)
}

/// `loc_060f8`: AL = the balance of the troops of the location DI
/// (`each_troop` with `loc_06155` summing into BX, DX, CX, [0D81Ch]):
/// each side's ratio ·256, their difference over the larger, /2 + 80h.
/// Bails before a division that would fault.
pub fn location_balance(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x60f8)?;
    let mut c = Ctx::new(m);
    c.run(start, 6)?;
    c.xor(W, 0, 0);
    c.set_reg(BX, 0);
    c.set_reg(CX, 0);
    c.set_reg(DX, 0);
    c.set_word(0xd81c, 0);
    c.set_reg(BP, 0x6155);
    c.call(0x6603, 0x6108)?;
    let (bx, cx, dx) = (c.reg(BX), c.reg(CX), c.reg(DX));
    let e = c.word(0xd81c);
    let s1 = bx.wrapping_add(dx);
    let q1 = if s1 != 0 { ((dx as u32) << 8) / s1 as u32 } else { 0 };
    let s2 = cx.wrapping_add(e);
    let q2 = if s2 != 0 { ((e as u32) << 8) / s2 as u32 } else { 0 };
    if q1 > 0xffff {
        c.m.cpu.ip = 0x6108;
        return Err(Bail);
    }
    let q1 = q1 as u16;
    if q2 > 0xffff {
        c.m.cpu.ip = 0x6108;
        return Err(Bail);
    }
    let q2 = q2 as u16;
    let si = if q1 >= q2 { q1 } else { q2 };
    let diff = q1.wrapping_sub(q2);
    if si != 0 {
        let num = (diff as i16 as i32) * 256 + if diff & 0x8000 != 0 { 255 } else { 0 };
        let q = num / si as i16 as i32;
        if si as i16 == 0 || !(-32768..=32767).contains(&q) {
            c.m.cpu.ip = 0x6108;
            return Err(Bail);
        }
    }
    let n = 3 + 3 * (s1 != 0) as u32 + 5 + 3 * (s2 != 0) as u32 + 4 + (q1 < q2) as u32 + 4 + 4 * (si != 0) as u32 + 3;
    c.run(0x6108, n)?;
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    let b = c.add(W, bx, dx);
    c.set_reg(BX, b);
    if s1 != 0 {
        c.set_reg(AX, (dx & 0xff) << 8);
        c.set_reg(DX, dx >> 8);
        c.m.cpu.mul_div(&mut c.m.hw, W, 6, s1 as u32);
    }
    let ax = c.reg(AX);
    c.set_reg(BX, ax);
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.set_reg(DX, e);
    let s = c.add(W, cx, e);
    c.set_reg(CX, s);
    if s2 != 0 {
        c.set_reg(AX, (e & 0xff) << 8);
        c.set_reg(DX, e >> 8);
        c.m.cpu.mul_div(&mut c.m.hw, W, 6, s2 as u32);
    }
    let ax = c.reg(AX);
    c.set_reg(CX, ax);
    c.set_reg(SI, q1);
    c.cmp(W, q1, q2);
    c.set_reg(SI, si);
    c.set_reg(AX, q1);
    let d = c.sub(W, q1, q2);
    c.set_reg(AX, d);
    c.or(W, si, si);
    if si != 0 {
        let dxs = if d & 0x8000 != 0 { 0xffff } else { 0 };
        let a = d.swap_bytes();
        let (a, dl) = ((a & 0xff00) | (dxs & 0xff), a as u8);
        c.set_reg(AX, a);
        c.set_reg(DX, (dxs & 0xff00) | dl as u16);
        c.m.cpu.mul_div(&mut c.m.hw, W, 7, si as u32);
    }
    let ax = c.reg(AX);
    let ax = c.m.cpu.shift(7, W, ax as u32, 1) as u16;
    let al = c.add(B, ax & 0xff, 0x80);
    c.set_reg(AX, ax & 0xff00 | al);
    c.ret()
}

/// `loc_0605c`: the location DI's line of the caption: the label 6Ch
/// (colour 90h) at ([1668h] + 4, BX + 0Ah), then for a location with bit
/// 20h of byte 0Ah its count (byte 1Bh, `draw_number_al` with the phrase
/// 75h) and `loc_0617a` (BP [166Ch]), else the phrase 6Dh. BX moves
/// down 7.
pub fn caption_count_line(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x605c)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xd075, 0x605f)?;
    c.run(0x605f, 6)?;
    c.set_reg8(r8::CL, 0x90);
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 0x0a);
    c.set_reg(BX, bx);
    let x = c.word(0x1668);
    let dx = c.add(W, x, 4);
    c.set_reg(DX, dx);
    c.set_reg(AX, 0x6c);
    c.call(0xd194, 0x6071)?;
    let di = c.reg(DI);
    let al = c.byte(di.wrapping_add(0x1b));
    let f = c.byte(di.wrapping_add(0x0a));
    c.run(0x6071, 5)?;
    c.set_reg8(r8::AL, al);
    c.set_reg(SI, 0x75);
    c.set_reg(BP, 0x6d);
    c.test(B, f as u16, 0x20);
    if f & 0x20 == 0 {
        c.run(0x609d, 5)?;
        c.set_reg(AX, 0x6d);
        let bx = c.reg(BX);
        let bx = c.add(W, bx, 7);
        c.set_reg(BX, bx);
        let x = c.word(0x1668);
        let dx = c.add(W, x, 0x0a);
        c.set_reg(DX, dx);
        return super::text::font_draw_string_with_color_at_pos(c.m, 0xd194);
    }
    c.run(0x6080, 4)?;
    let (cx, ax) = (c.reg(CX), c.reg(AX));
    c.push(cx);
    c.push(ax);
    c.push(0x75);
    c.call(0xe295, 0x6086)?;
    c.run(0x6086, 8)?;
    let a = c.pop();
    c.set_reg(AX, a);
    let x = c.pop();
    c.set_reg(CX, x);
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 7);
    c.set_reg(BX, bx);
    let x = c.word(0x1668);
    let dx = c.add(W, x, 4);
    c.set_reg(DX, dx);
    let bp = c.word(0x166c);
    c.set_reg(BP, bp);
    c.push(bx);
    c.call(0x617a, 0x609a)?;
    c.run(0x609a, 3)?;
    let b = c.pop();
    c.set_reg(BX, b);
    let x = c.pop();
    c.set_reg(CX, x);
    c.ret()
}

/// `loc_04bdf`: the scrolling scene: resource [485Eh] opened, the 8.8
/// positions [4878h]/[4876h] stepped by [4874h]/[4872h] into [4870h]/
/// [486Eh], the view clamped there (`clamp_view_rect`), then the
/// script at [486Ah] (in the segment [0DBB2h]; restarted from [4868h] at
/// an FFh) up to its 0: each entry ([`draw_sprite_list`], 1 takes the
/// next byte as AL, AH 1); [486Ah] after the 0.
pub fn draw_scroll_scene(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4bdf)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let r = c.word(0x485e);
    c.set_reg(AX, r);
    c.call(0xc13e, 0x4be5)?;
    c.run(0x4be5, 17)?;
    for (pos, step, out) in [(0x4878u16, 0x4874u16, 0x4870u16), (0x4876, 0x4872, 0x486e)] {
        let a = c.word(pos);
        c.xor(B, 0, 0);
        let s = c.word(step);
        let a = c.add(W, a & 0xff, s);
        c.set_word(pos, a);
        let v = (a >> 8) as u8 as i8 as i16 as u16;
        c.set_reg(AX, v);
        let o = c.word(out);
        let o = c.add(W, o, v);
        c.set_word(out, o);
    }
    let dx = c.word(0x486e);
    let bx = c.word(0x4870);
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.call(0x4b5f, 0x4c16)?;
    let si = c.word(0x486a);
    let seg = c.word_in(SS, 0xdbb2);
    let b = c.byte_at_seg(seg, si);
    c.run(0x4c16, if b == 0xff { 5 } else { 4 })?;
    c.set_reg(SI, si);
    c.set_sreg(DS, seg);
    c.cmp(B, b as u16, 0xff);
    if b == 0xff {
        let s = c.word_in(SS, 0x4868);
        c.set_reg(SI, s);
    }
    loop {
        let p = c.reg(SI);
        let al = c.byte(p);
        if al == 0 {
            c.run(0x4c29, 8)?;
            c.lods(B, DS);
            let ah = c.reg8(r8::AH);
            c.xor(B, ah as u16, ah as u16);
            c.set_reg8(r8::AH, 0);
            c.or(B, 0, 0);
            let ss = c.sreg(SS);
            c.push(ss);
            let v = c.pop();
            c.set_sreg(DS, v);
            let si = c.reg(SI);
            c.set_word(0x486a, si);
            return c.ret();
        }
        c.run(0x4c29, 8 + if al == 1 { 2 } else { 0 })?;
        c.lods(B, DS);
        let ah = c.reg8(r8::AH);
        c.xor(B, ah as u16, ah as u16);
        c.set_reg8(r8::AH, 0);
        c.or(B, al as u16, al as u16);
        c.cmp(B, al as u16, 1);
        if al == 1 {
            c.set_reg8(r8::AH, 1);
            c.lods(B, DS);
        }
        let si = c.reg(SI);
        c.push(si);
        c.call(0x4c45, 0x4c3b)?;
        c.run(0x4c3b, 2)?;
        let s = c.pop();
        c.set_reg(SI, s);
    }
}

/// `loc_05adf`: the map screen left: `loc_07b36`, its state cleared
/// ([46EBh], [46F3h], [3CBEh], [0A5C0h], [0DBE0h], [0DBE2h], [1954h]),
/// [2786h] = C827h, the frame task 6B34h removed.
pub fn leave_map_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5adf)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x7b36, 0x5ae2)?;
    c.run(0x5ae2, 11)?;
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.set_byte(0x46eb, 0);
    c.set_byte(0x46f3, 0);
    for p in [0x3cbeu16, 0xa5c0, 0xdbe0, 0xdbe2, 0x1954] {
        c.set_word(p, 0);
    }
    c.set_word(0x2786, 0xc827);
    c.set_reg(SI, 0x6b34);
    super::tasks::remove_frame_task(c.m, 0xda5f)
}

/// `loc_04aeb`: resource 39h opened, the front buffer active, the CS
/// script run (`run_cs_script`), `cursor_over_panel`, then [`sub_c4dd`].
pub fn scene_39(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4aeb)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(AX, 0x39);
    c.call(0xc13e, 0x4af1)?;
    c.run(0x4af1, 1)?;
    c.call(0xc07c, 0x4af4)?;
    c.run(0x4af4, 1)?;
    c.call(0x4d6c, 0x4af7)?;
    c.run(0x4af7, 1)?;
    c.call(0x4b2b, 0x4afa)?;
    c.run(0x4afa, 1)?;
    super::draw::sub_c4dd(c.m, 0xc4dd)
}

/// `loc_0644e`: the circle of radius byte 0Bh of the location DI around
/// its map point, colour 20h ([`circle_6447`]'s drawing).
pub fn location_circle(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x644e)?;
    let mut c = Ctx::new(m);
    c.run(start, 3)?;
    c.set_byte(0x4739, 0x20);
    let cx = c.reg(CX);
    c.xor(W, cx, cx);
    let di = c.reg(DI);
    let r = c.byte(di.wrapping_add(0x0b));
    c.set_reg(CX, r as u16);
    circle_at(&mut c, 0x6458)
}

/// `loc_06447`: the circle of radius CX around the map point of the
/// location DI in colour 30h ([4739h]): its spans (`loc_064ef` with the
/// handler 646Fh) by the midpoint walk at `loc_064b2`, from (word 2,
/// word 4) in [0D81Ch], [0D818h].
pub fn circle_6447(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6447)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_byte(0x4739, 0x30);
    circle_at(&mut c, 0x6458)
}

fn circle_at(c: &mut Ctx, at: u16) -> Result<(), Bail> {
    let di = c.reg(DI);
    let (x, y) = (c.word(di.wrapping_add(2)), c.word(di.wrapping_add(4)));
    let mut r = [c.reg(AX), 0, c.reg(CX), 0, 0, 0, 0];
    let mut label = 0x64b2u16;
    let mut pre = 6;
    let mut first = true;
    loop {
        let Ok((n, end, out, old_cx, carry)) = circle_sim(label, r) else {
            c.m.cpu.ip = if first { at } else { label };
            return Err(Bail);
        };
        c.run(if first { at } else { label }, pre + n)?;
        if first {
            c.set_word_in(CS, 0x64ed, 0x646f);
            c.set_word(0xd81c, x);
            c.set_word(0xd818, y);
            first = false;
        }
        pre = 0;
        let [ax, bx, cx, dx, si, di, bp] = out;
        for (reg, v) in [(AX, ax), (BX, bx), (CX, cx), (DX, dx), (SI, si), (DI, di), (BP, bp)] {
            c.set_reg(reg, v);
        }
        match end {
            Some(back) => {
                c.call(0x64ef, back)?;
                r = [c.reg(AX), c.reg(BX), c.reg(CX), c.reg(DX), c.reg(SI), c.reg(DI), c.reg(BP)];
                label = back;
            }
            None => {
                // `dec cx` keeps CF: the last `add ax, cx` set it.
                c.set_flag(CF, carry);
                c.dec(W, old_cx);
                return c.ret();
            }
        }
    }
}

/// One stretch of the midpoint walk at `loc_064b2` from `label`: its
/// instruction count, the return address of the `loc_064ef` call that
/// ends it (or none for the `ret`), the registers (AX, BX, CX, DX, SI,
/// DI, BP), for the `ret` CX before its last `dec`, and the carry of
/// the stretch's last `add ax, cx`.
#[allow(clippy::type_complexity)]
fn circle_sim(label: u16, r: [u16; 7]) -> Result<(u32, Option<u16>, [u16; 7], u16, bool), Bail> {
    let [mut ax, mut bx, mut cx, mut dx, mut si, mut di, mut bp] = r;
    let mut n = 0u32;
    let mut carry = false;
    let mut l = label;
    loop {
        if n > 1_000_000 {
            return Err(Bail);
        }
        match l {
            0x64b2 => {
                bx = 0u16.wrapping_sub(cx);
                dx = 0;
                di = 0;
                si = cx.wrapping_sub(1);
                ax = cx;
                bp = 0;
                n += 9;
                l = 0x64c6;
            }
            0x64c3 => {
                bp = bp.wrapping_add(1);
                dx = dx.wrapping_add(1);
                di = di.wrapping_sub(1);
                n += 3;
                l = 0x64c6;
            }
            0x64c6 => {
                ax = ax.wrapping_sub(bp);
                n += 2;
                if ax & 0x8000 == 0 {
                    l = 0x64c3;
                } else {
                    n += 1;
                    return Ok((n, Some(0x64cd), [ax, bx, cx, dx, si, di, bp], 0, carry));
                }
            }
            0x64cd => {
                bx = bx.wrapping_add(1);
                si = si.wrapping_sub(1);
                ax = ax.wrapping_add(cx);
                n += 4;
                if ax & 0x8000 != 0 {
                    l = 0x64d5;
                } else {
                    n += 1;
                    cx = cx.wrapping_sub(1);
                    l = if cx != 0 { 0x64c3 } else { 0x64d5 };
                }
            }
            0x64d5 => {
                bp = bp.wrapping_add(1);
                cx = cx.wrapping_sub(1);
                n += 3;
                l = 0x64de;
            }
            0x64d9 => {
                dx = dx.wrapping_add(1);
                di = di.wrapping_sub(1);
                ax = ax.wrapping_sub(bp);
                bp = bp.wrapping_add(1);
                n += 4;
                l = 0x64de;
            }
            0x64de => {
                n += 1;
                return Ok((n, Some(0x64e1), [ax, bx, cx, dx, si, di, bp], 0, carry));
            }
            0x64e1 => {
                bx = bx.wrapping_add(1);
                si = si.wrapping_sub(1);
                let (v, cf) = ax.overflowing_add(cx);
                ax = v;
                carry = cf;
                n += 4;
                if carry {
                    l = 0x64e9;
                } else {
                    n += 1;
                    cx = cx.wrapping_sub(1);
                    l = if cx != 0 { 0x64de } else { 0x64e9 };
                }
            }
            0x64e9 => {
                let old = cx;
                cx = cx.wrapping_sub(1);
                n += 2;
                if cx & 0x8000 == 0 {
                    l = 0x64d9;
                } else {
                    n += 1;
                    return Ok((n, None, [ax, bx, cx, dx, si, di, bp], old, carry));
                }
            }
            _ => return Err(Bail),
        }
    }
}

/// `loc_05d44`: in a map mode of [11C9h], [4728h] gets bit 1; then
/// [`count_shown_location`].
pub fn note_shown_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5d44)?;
    let mut c = Ctx::new(m);
    note_shown_at(&mut c)
}

fn note_shown_at(c: &mut Ctx) -> Result<(), Bail> {
    let k = c.byte(0x11c9);
    c.run(0x5d44, if k & 3 != 0 { 3 } else { 2 })?;
    c.test(B, k as u16, 3);
    if k & 3 != 0 {
        let v = c.byte(0x4728);
        let v = c.or(B, v as u16, 1);
        c.set_byte(0x4728, v as u8);
    }
    count_shown_location(c.m, 0x5d50)
}

/// `loc_074b6`: the location DI loses bit 2 of byte 0Ah; at the current
/// location [46D9h] = 6. Else a sietch (byte 8 below 28h) becomes
/// abandoned (byte 8 to 28h + its low 3 bits, [27h] down, the records at
/// FD8h at its place set to 3), `loc_07506` for its troops, the circle
/// of radius 5 around it, bits 9 of byte 0Ah cleared, and
/// [`note_shown_location`].
pub fn abandon_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x74b6)?;
    let mut c = Ctx::new(m);
    let di = c.reg(DI);
    let here = c.word(0x114e);
    if di == here {
        c.run(start, 5)?;
        let v = c.byte(di.wrapping_add(0x0a));
        let v = c.and(B, v as u16, 0xfd);
        c.set_byte(di.wrapping_add(0x0a), v as u8);
        c.cmp(W, di, here);
        c.set_byte(0x46d9, 6);
        return c.ret();
    }
    let al = c.byte(di.wrapping_add(8));
    if al >= 0x28 {
        c.run(start, 6 + 2)?;
        let v = c.byte(di.wrapping_add(0x0a));
        let v = c.and(B, v as u16, 0xfd);
        c.set_byte(di.wrapping_add(0x0a), v as u8);
        c.cmp(W, di, here);
        c.set_reg8(r8::AL, al);
        c.cmp(B, al as u16, 0x28);
    } else {
        c.run(start, 12)?;
        let v = c.byte(di.wrapping_add(0x0a));
        let v = c.and(B, v as u16, 0xfd);
        c.set_byte(di.wrapping_add(0x0a), v as u8);
        c.cmp(W, di, here);
        c.cmp(B, al as u16, 0x28);
        let a = c.and(B, al as u16, 7);
        let a = c.add(B, a, 0x28);
        c.set_reg8(r8::AL, a as u8);
        c.set_byte(di.wrapping_add(8), a as u8);
        let v = c.byte(0x27);
        let v = c.dec(B, v as u16);
        c.set_byte(0x27, v as u8);
        let si = c.reg(SI);
        c.push(si);
        c.call(0x40ae, 0x74d6)?;
        let (bx, dx) = (c.reg(BX), c.reg(DX));
        let mut n = 3;
        for i in 0..9u16 {
            let w = c.word(0xfd8 + 0x10 * i + 2);
            n += 2 + (w == bx) as u32 + 2;
        }
        c.run(0x74d6, n + 1 + 2)?;
        let dx = dx & 0xff00 | 3;
        c.set_reg(DX, dx);
        let mut si = 0xfd8u16;
        for i in 0..9u16 {
            let w = c.word(si.wrapping_add(2));
            c.cmp(W, bx, w);
            if w == bx {
                c.set_word(si, dx);
            }
            si = c.add(W, si, 0x10);
            c.set_reg(CX, 8 - i);
        }
        let s = c.pop();
        c.set_reg(SI, s);
        c.set_reg(BP, 0x7506);
        c.call(0x6603, 0x74f1)?;
        c.run(0x74f1, 3)?;
        let di = c.reg(DI);
        c.push(di);
        c.set_reg(CX, 5);
        c.call(0x6447, 0x74f8)?;
        return abandon_tail(&mut c);
    }
    c.set_reg(BP, 0x7506);
    c.call(0x6603, 0x74f1)?;
    c.run(0x74f1, 3)?;
    let di = c.reg(DI);
    c.push(di);
    c.set_reg(CX, 5);
    c.call(0x6447, 0x74f8)?;
    abandon_tail(&mut c)
}

fn abandon_tail(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x74f8, 3)?;
    let di = c.pop();
    c.set_reg(DI, di);
    let v = c.byte(di.wrapping_add(0x0a));
    let v = c.and(B, v as u16, 0xf6);
    c.set_byte(di.wrapping_add(0x0a), v as u8);
    note_shown_at(c)
}

/// `loc_0617a`: CL (low byte of CX; none for 0) copies of the sprite AX
/// in a row from (DX, BX) to BP: spaced its width + 2, or squeezed to
/// fit (at least 2 apart, then half as many); BX then below it (its
/// height + 1, `get_subresource_ax_pointer_to_dssi`).
pub fn sprite_row(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x617a)?;
    let mut c = Ctx::new(m);
    let cx = c.reg(CX) & 0xff;
    if cx == 0 {
        c.run(start, 3)?;
        c.and(W, 0, 0xff);
        c.set_reg(CX, 0);
        return c.ret();
    }
    c.run(start, 7)?;
    c.and(W, cx, 0xff);
    c.set_reg(CX, cx);
    let (bx, di, ax, dx) = (c.reg(BX), c.reg(DI), c.reg(AX), c.reg(DX));
    for v in [bx, di, ax, dx] {
        c.push(v);
    }
    c.call(0xc1f4, 0x6187)?;
    let (bp0, dx1, si) = (c.reg(BP), c.reg(DX), c.reg(SI));
    let span = bp0.wrapping_sub(dx1);
    let w = (c.word_in(ES, si) & 0xfff).wrapping_add(2);
    let q = span / cx;
    let (bp, cnt, n) = if q >= w {
        (w, cx, 10)
    } else {
        let d = span.wrapping_sub(w);
        let q2 = d / cx;
        if q2 >= 2 {
            (q2, cx, 17)
        } else {
            (2, d >> 1, 20)
        }
    };
    if cnt == 0 {
        c.m.cpu.ip = 0x6187;
        return Err(Bail);
    }
    c.run(0x6187, n + 2 + 6)?;
    let di1 = c.sub(W, bp0, dx1);
    c.set_reg(DI, di1);
    let b = c.word_in(ES, si);
    let b = c.and(W, b, 0xfff);
    let b = c.add(W, b, 2);
    c.set_reg(BP, b);
    c.set_reg(AX, di1);
    c.xor(W, 0, 0);
    c.set_reg(DX, 0);
    c.m.cpu.mul_div(&mut c.m.hw, W, 6, cx as u32);
    let a = c.reg(AX);
    c.cmp(W, a, b);
    if n > 10 {
        let d = c.sub(W, di1, b);
        c.set_reg(DI, d);
        c.set_reg(AX, d);
        c.xor(W, 0, 0);
        c.set_reg(DX, 0);
        c.m.cpu.mul_div(&mut c.m.hw, W, 6, cx as u32);
        let a = c.reg(AX);
        c.set_reg(BP, a);
        c.cmp(W, a, 2);
        if n > 17 {
            c.set_reg(BP, 2);
            c.set_reg(CX, d);
            let h = c.shr1(W, d, 1);
            c.set_reg(CX, h);
        }
    }
    let d = c.pop();
    c.set_reg(DX, d);
    let a = c.pop();
    c.set_reg(AX, a);
    let _ = (bp, ax, dx);
    let mut first = true;
    loop {
        if !first {
            c.run(0x61b7, 6)?;
        }
        first = false;
        for r in [AX, BX, CX, DX, BP] {
            let v = c.reg(r);
            c.push(v);
        }
        c.call(0xc22f, 0x61bf)?;
        let cx = c.word_in(SS, c.reg(SP).wrapping_add(4)).wrapping_sub(1);
        c.run(0x61bf, 7 + if cx == 0 { 3 } else { 0 })?;
        for r in [BP, DX, CX, BX, AX] {
            let v = c.pop();
            c.set_reg(r, v);
        }
        let (dx, bp) = (c.reg(DX), c.reg(BP));
        let dx = c.add(W, dx, bp);
        c.set_reg(DX, dx);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    let d = c.pop();
    c.set_reg(DI, d);
    let b = c.pop();
    c.set_reg(BX, b);
    let ax = c.reg(AX);
    let _ = ax;
    c.call(0xc1f4, 0x61cd)?;
    c.run(0x61cd, 3)?;
    let si = c.reg(SI);
    let h = c.byte_in(ES, si.wrapping_add(2));
    let bx = c.reg(BX);
    let bl = c.add(B, bx & 0xff, h as u16);
    let bx = c.inc(W, bx & 0xff00 | bl);
    c.set_reg(BX, bx);
    c.ret()
}

/// `loc_05746`: the map header by the cursor (DX, BX): outside the rect
/// 4710h its normal state ([`draw_map_header`]); near its bottom (from
/// [4716h] - 0Ah) the state F5h; else the normal state and, out of
/// [4722h], the region under the cursor (the driver's far [3965h] on
/// the screen [0DBD8h]; 50h..5Fh, else FFh) into [4724h], its two names
/// redrawn (`loc_0578b`) when it changed.
pub fn map_header_by_cursor(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5746)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(DI, 0x4710);
    c.call(0xd6fe, 0x574c)?;
    if !c.flag(CF) {
        c.run(0x574c, 1)?;
        return draw_map_header(c.m, 0x57b2);
    }
    let bx = c.reg(BX);
    let lim = c.word(0x4716).wrapping_sub(0x0a);
    if bx >= lim {
        return map_header_from(&mut c, 0x574c, 7, 0xfef5, Some(lim));
    }
    c.run(0x574c, 6)?;
    let a = c.word(0x4716);
    let a = c.sub(W, a, 0x0a);
    c.set_reg(AX, a);
    c.cmp(W, bx, a);
    c.call(0x57b2, 0x575b)?;
    let k = c.byte(0x4722);
    if k != 0 {
        c.run(0x575b, 3)?;
        c.cmp(B, k as u16, 0);
        return c.ret();
    }
    c.run(0x575b, 3)?;
    c.cmp(B, 0, 0);
    let es = c.word(0xdbd8);
    c.set_sreg(ES, es);
    c.interp(0x5766, 0x576a)?;
    let al = c.reg8(r8::AL);
    let v = al.wrapping_sub(0x50);
    let r = if v < 0x10 { v } else { 0xff };
    let old = c.byte(0x4724);
    c.run(0x576a, 3 + (v >= 0x10) as u32 + 4 + if old == r { 1 } else { 1 })?;
    let v2 = c.sub(B, al as u16, 0x50);
    c.cmp(B, v2, 0x10);
    c.set_reg(AX, (r as u16) << 8 | old as u16);
    c.set_byte(0x4724, r);
    c.cmp(B, old as u16, r as u16);
    if old == r {
        return c.ret();
    }
    c.call(0xdbb2, 0x577f)?;
    c.run(0x577f, 1)?;
    c.call(0x578b, 0x5782)?;
    c.run(0x5782, 2)?;
    let ax = c.reg(AX);
    c.set_reg(AX, ax & 0xff00 | ax >> 8);
    c.call(0x578b, 0x5787)?;
    c.run(0x5787, 1)?;
    c.call(0xdbec, 0x578a)?;
    c.run(0x578a, 1)?;
    c.ret()
}

/// `loc_04e12`: the scene for the map place: [4733h] 0, the cell
/// (`get_map_position`, `loc_0b532`) picks the mode (`pick_mode_dc02`);
/// [196Ah] = 0, or for a cell with bit 40h near a record
/// (`loc_0409a`, not at [6] 80h) within 4 across: the offsets
/// [1968h], [1964h], [1960h], the sprite from 196Dh by the SAL index
/// (19 and up shifted by the place's kind, at most 17h) in [196Ah], and
/// `loc_04ded` for it (SI, DI swapped meanwhile).
pub fn place_scene(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4e12)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_word(0x4733, 0);
    c.call(0x407e, 0x4e1b)?;
    c.run(0x4e1b, 1)?;
    c.call(0xb532, 0x4e1e)?;
    c.run(0x4e1e, 2)?;
    let ax = c.reg(AX);
    c.push(ax);
    c.call(0x4ec6, 0x4e22)?;
    let al = c.word_in(SS, c.reg(SP)) as u8;
    let near = al & 0x40 != 0;
    c.run(0x4e22, 4 + near as u32)?;
    c.set_word(0x196a, 0);
    let a = c.pop();
    c.set_reg(AX, a);
    c.test(B, al as u16, 0x40);
    if near {
        c.call(0x409a, 0x4e30)?;
        let zf = c.flag(ZF);
        let six = c.byte(6);
        let (si, dx) = (c.reg(SI), c.reg(DX));
        let d = c.word(si.wrapping_add(2)).wrapping_sub(dx).wrapping_add(4);
        let go = zf && six != 0x80 && d < 8;
        if !go {
            let n = if !zf { 1 } else if six == 0x80 { 3 } else { 8 };
            c.run(0x4e30, n)?;
            if zf {
                c.cmp(B, six as u16, 0x80);
                if six != 0x80 {
                    let w = c.word(si.wrapping_add(2));
                    let a = c.sub(W, w, dx);
                    let a = c.add(W, a, 4);
                    c.set_reg(AX, a);
                    c.cmp(W, a, 8);
                }
            }
        } else {
            c.run(0x4e30, 15)?;
            c.cmp(B, six as u16, 0x80);
            let w = c.word(si.wrapping_add(2));
            let a = c.sub(W, w, dx);
            let a = c.add(W, a, 4);
            c.cmp(W, a, 8);
            c.set_word(0x1968, a);
            let a = c.inc(W, a);
            c.set_word(0x1964, a);
            let a = c.sub(W, a, 2);
            c.set_word(0x1960, a);
            c.xor(W, 0, 0);
            c.set_reg(AX, 0);
            c.call(0x5e4f, 0x4e58)?;
            let ax = c.reg(AX);
            let cl = c.reg8(r8::CL);
            let s = c.byte(0x196du16.wrapping_add(ax & 0xff));
            let big = s >= 0x13;
            let k = (cl.wrapping_sub(0x28)) & 0xfb;
            let v = s.wrapping_add(k);
            let cap = big && v >= 0x17;
            c.run(0x4e58, 4 + if big { 5 + cap as u32 } else { 0 } + 3)?;
            c.set_reg(BX, 0x196d);
            c.cmp(B, s as u16, 0x13);
            let mut al = s;
            if big {
                let x = c.sub(B, cl as u16, 0x28);
                let x = c.and(B, x, 0xfb);
                c.set_reg8(r8::CL, x as u8);
                let v = c.add(B, s as u16, x);
                c.cmp(B, v, 0x17);
                al = if cap { 0x17 } else { v as u8 };
            }
            let ax = ax & 0xff00 | al as u16;
            c.set_reg(AX, ax);
            c.set_word(0x196a, ax);
            let (si, di) = (c.reg(SI), c.reg(DI));
            c.set_reg(SI, di);
            c.set_reg(DI, si);
            c.call(0x4ded, 0x4e76)?;
            c.run(0x4e76, 1)?;
            let (si, di) = (c.reg(SI), c.reg(DI));
            c.set_reg(SI, di);
            c.set_reg(DI, si);
        }
    }
    let v = c.word(0x196a);
    if v == 0 {
        c.run(0x4e78, 4)?;
        c.set_reg(AX, 0);
        c.or(W, 0, 0);
        return c.ret();
    }
    let k = c.byte(0x46ff);
    c.run(0x4e78, 7 + if k == 0 { 1 } else { 2 })?;
    c.set_reg(AX, v);
    c.or(W, v, v);
    let w = c.word(0x4733);
    c.set_reg(CX, w & 0xff00 | k as u16);
    c.xor(B, 0, 0);
    c.set_reg(CX, k as u16);
    c.ret()
}

/// `loc_05e6d`: DI = the nearest (|dx| + |dy| from (DX, BX)) of the
/// map's shown locations (the list at A5C0h, 6 bytes each: location,
/// x, y with the map mode in the high byte) whose byte 8 is below AL and
/// whose mode is [46EBh]; AX its distance (FFFFh, DI 0 for none). BX,
/// DX, SI kept; the four locals stay below the stack.
pub fn nearest_shown_location(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5e6d)?;
    let mut c = Ctx::new(m);
    let (al, dx0, bx0, si0) = (c.reg8(r8::AL), c.reg(DX), c.reg(BX), c.reg(SI));
    let mode = c.byte(0x46eb);
    let mut best = 0xffffu16;
    let mut best_di = 0u16;
    let mut n = 8;
    let mut p = 0xa5bau16;
    loop {
        p = p.wrapping_add(6);
        let di = c.word(p);
        n += 4;
        if di == 0 {
            break;
        }
        n += 2;
        if c.byte(di.wrapping_add(8)) >= al {
            continue;
        }
        let bx = c.word(p.wrapping_add(4));
        n += 3;
        if (bx >> 8) as u8 != mode {
            continue;
        }
        let d1 = c.word(p.wrapping_add(2)).wrapping_sub(dx0);
        n += 4 + (d1 & 0x8000 != 0) as u32;
        let d1 = if d1 & 0x8000 != 0 { d1.wrapping_neg() } else { d1 };
        let d2 = (bx & 0xff).wrapping_sub(bx0);
        n += 2 + (d2 & 0x8000 != 0) as u32;
        let d2 = if d2 & 0x8000 != 0 { d2.wrapping_neg() } else { d2 };
        let d = d1.wrapping_add(d2);
        n += 3;
        if d < best {
            best = d;
            best_di = di;
            n += 3;
        }
    }
    n += 7;
    c.run(start, n)?;
    c.push(si0);
    let bp = c.reg(SP);
    c.set_reg(BP, bp);
    let low = bp.wrapping_sub(8);
    for (off, v) in [(8u16, best), (6, dx0), (4, bx0), (2, best_di)] {
        c.set_word_in(SS, bp.wrapping_sub(off), v);
    }
    c.set_reg(SI, p);
    c.set_reg(DI, best_di);
    c.set_reg(AX, best);
    c.set_reg(DX, dx0);
    c.set_reg(BX, bx0);
    let sp = c.add(W, low, 8);
    c.set_reg(SP, sp);
    let s = c.pop();
    c.set_reg(SI, s);
    c.ret()
}

/// `loc_05605`: the map header's area (the rect 4710h less its frame:
/// +6, +62h, -6, -2, built on the stack) filled with colour F5h by the
/// driver (far [38DDh]) on the screen [0DBDAh]; [4724h] = FFh,
/// `loc_0d075`; then [`get_point_4710`].
pub fn clear_map_header(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5605)?;
    let mut c = Ctx::new(m);
    c.run(start, 20)?;
    let sp = c.reg(SP);
    let sp = c.sub(W, sp, 8);
    c.set_reg(SP, sp);
    c.set_reg(SI, 0x4710);
    c.set_reg(DI, sp);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    for (add, v) in [(true, 6u16), (true, 0x62), (false, 6), (false, 2)] {
        let a = c.lods(W, DS);
        let r = if add { c.add(W, a, v) } else { c.sub(W, a, v) };
        c.set_reg(AX, r);
        c.stos(W);
    }
    let di = c.reg(DI);
    c.set_reg(SI, di.wrapping_sub(8));
    c.set_reg8(r8::AL, 0xf5);
    let es = c.word(0xdbda);
    c.set_sreg(ES, es);
    c.interp(0x562c, 0x5630)?;
    c.run(0x5630, 2)?;
    c.set_byte(0x4724, 0xff);
    c.call(0xd075, 0x5638)?;
    c.run(0x5638, 2)?;
    let sp = c.reg(SP);
    let sp = c.add(W, sp, 8);
    c.set_reg(SP, sp);
    get_point_4710(c.m, 0x557b)
}

/// `loc_07bb9`: `loc_031f6` (AX kept), then [`show_message_0f`].
pub fn show_message_after_31f6(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7bb9)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let ax = c.reg(AX);
    c.push(ax);
    c.call(0x31f6, 0x7bbd)?;
    c.run(0x7bbd, 1)?;
    let a = c.pop();
    c.set_reg(AX, a);
    show_message_0f(c.m, 0x7bbe)
}

/// `loc_0636a` / `loc_0639a`: from the map point (SI, CX after
/// `loc_0b58b` of (DX, BX)) the map row walked right (to [46E7h]) or left
/// (to [46E3h]), 4 screen pixels a cell, wrapping at BP; each cell of ES:DI
/// with bits 30h = 10h gets its dot (`draw_map_dot`).
pub fn map_row_right(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x636a)?;
    map_row(m, start, true)
}

pub fn map_row_left(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x639a)?;
    map_row(m, start, false)
}

fn map_row(m: &mut Machine, start: u16, right: bool) -> Result<(), Bail> {
    let mut c = Ctx::new(m);
    let b = start.wrapping_sub(0x636a);
    c.run(start, 3)?;
    let (si, dx) = (c.reg(SI), c.reg(DX));
    c.set_reg(SI, dx);
    c.set_reg(DX, si);
    let (cx, bx) = (c.reg(CX), c.reg(BX));
    c.set_reg(CX, bx);
    c.set_reg(BX, cx);
    c.call(0xb58b, 0x6371 + b)?;
    c.run(0x6371 + b, 2)?;
    let (si, dx) = (c.reg(SI), c.reg(DX));
    c.set_reg(SI, dx);
    c.set_reg(DX, si);
    let cx = c.reg(CX);
    c.set_reg(BX, cx);
    loop {
        let di = c.reg(DI);
        let w = c.word_in(ES, di);
        let hit = w & 0x30 == 0x10;
        c.run(0x6375 + b, 4 + hit as u32)?;
        let a = c.and(W, w, 0x3030);
        c.set_reg(AX, a);
        c.cmp(B, a & 0xff, 0x10);
        if hit {
            c.call(0x63c7, 0x6398 + b - if right { 0 } else { 0x30 - 0x2d })?;
            c.run(if right { 0x6398 } else { 0x63c5 }, 1)?;
        }
        let dx = c.reg(DX);
        let (lim, out) = if right {
            let l = c.word(0x46e7);
            (l, dx.wrapping_add(4) >= l)
        } else {
            let l = c.word(0x46e3);
            (l, dx.wrapping_sub(4) < l)
        };
        let si = c.reg(SI);
        let bp = c.reg(BP);
        let (wrap, n) = if out {
            (false, 4)
        } else if right {
            let w = si.wrapping_add(1) >= bp;
            (w, 3 + 4 + if w { 3 } else { 0 })
        } else {
            let w = si.wrapping_sub(1) & 0x8000 != 0;
            (w, 3 + 3 + if w { 3 } else { 0 })
        };
        c.run(if right { 0x637f } else { 0x63af }, n)?;
        let d = if right { c.add(W, dx, 4) } else { c.sub(W, dx, 4) };
        c.set_reg(DX, d);
        c.cmp(W, d, lim);
        if out {
            return c.ret();
        }
        let di = c.reg(DI);
        if right {
            let di = c.inc(W, di);
            let s = c.inc(W, si);
            c.cmp(W, s, bp);
            let (s, di) = if wrap { (c.sub(W, s, bp), c.sub(W, di, bp)) } else { (s, di) };
            c.set_reg(SI, s);
            c.set_reg(DI, di);
        } else {
            let di = c.dec(W, di);
            let s = c.dec(W, si);
            let (s, di) = if wrap { (c.add(W, s, bp), c.add(W, di, bp)) } else { (s, di) };
            c.set_reg(SI, s);
            c.set_reg(DI, di);
        }
    }
}

/// `loc_04c92`: a scrolling scene set up: resource AL ([485Eh]) opened,
/// its place (DX, BX) and steps (BP, CX) into 486Eh..4874h, the
/// positions cleared; from its sprite table ([0DBB0h]): the scene's size
/// ([487Ah], [487Ch]), its sprite lists ([486Ch]) and the script AH
/// ([4868h], [486Ah]). DS kept.
pub fn setup_scroll_scene(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4c92)?;
    let mut c = Ctx::new(m);
    scroll_scene_at(&mut c, 0x4c92, 0)
}

/// `loc_04d57`: [`setup_scroll_scene`] for resource 31h at (5 + BH·4, 29h),
/// still.
pub fn scroll_scene_31(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4d57)?;
    let mut c = Ctx::new(m);
    scroll_scene_at(&mut c, 0x4d57, 9)
}

fn scroll_scene_at(c: &mut Ctx, at: u16, pre: u32) -> Result<(), Bail> {
    c.run(at, pre + 11)?;
    if pre != 0 {
        c.set_reg8(r8::AL, 0x31);
        c.set_reg(DX, 5);
        let bx = c.reg(BX);
        let bh = c.shl1(B, bx >> 8, 2);
        let dl = c.add(B, 5, bh);
        c.set_reg(DX, dl);
        c.set_reg(BX, 0x29);
        c.xor(W, 0, 0);
        c.set_reg(BP, 0);
        c.set_reg(CX, 0);
    }
    let (ds, ax) = (c.sreg(DS), c.reg(AX));
    c.push(ds);
    c.push(ax);
    c.xor(B, 0, 0);
    let a = ax & 0xff;
    c.set_reg(AX, a);
    c.set_word(0x485e, a);
    let (dx, bx, bp, cx) = (c.reg(DX), c.reg(BX), c.reg(BP), c.reg(CX));
    c.set_word(0x486e, dx);
    c.set_word(0x4870, bx);
    c.set_word(0x4872, bp);
    c.set_word(0x4874, cx);
    c.set_word(0x4876, 0);
    c.set_word(0x4878, 0);
    c.call(0xc13e, 0x4cb8)?;
    c.run(0x4cb8, 31)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    let (tab, seg) = (c.word(0xdbb0), c.word(0xdbb2));
    c.set_sreg(DS, seg);
    let bx = c.word(tab);
    let o = c.word(bx.wrapping_add(tab).wrapping_sub(2));
    let si = c.add(W, tab, o);
    c.set_reg(DI, 0x4860);
    let si = c.add(W, si, 4);
    c.set_reg(SI, si);
    let x0 = c.lods(W, DS);
    let y0 = c.lods(W, DS);
    let x1 = c.lods(W, DS);
    let w = c.sub(W, x1, x0);
    c.set_word_in(SS, 0x487a, w);
    let y1 = c.lods(W, DS);
    let h = c.sub(W, y1, y0);
    c.set_word_in(SS, 0x487c, h);
    c.set_reg(DX, x0);
    c.set_reg(BX, y0);
    let si = c.reg(SI);
    let a = c.add(W, si, 2);
    c.set_word_in(SS, 0x486c, a);
    let o = c.word(si);
    let si = c.add(W, si, o);
    let ax = c.pop();
    c.xor(B, 0, 0);
    let ah = ax >> 8;
    let bp = c.shl1(W, ah, 1);
    c.set_reg(AX, bp);
    c.set_reg(BP, bp);
    let o = c.word(bp.wrapping_add(si));
    let si = c.add(W, si, o);
    c.set_reg(SI, si);
    c.set_word_in(SS, 0x4868, si);
    c.set_word_in(SS, 0x486a, si);
    let ds = c.pop();
    c.set_sreg(DS, ds);
    c.ret()
}

/// `loc_061d3`: [`sprite_row`] down a column: CL copies of the sprite AX
/// from (DX, BX) to BP, spaced its height + 2 or squeezed; DX then right
/// of it (its width + 1).
pub fn sprite_column(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x61d3)?;
    let mut c = Ctx::new(m);
    let cx = c.reg(CX) & 0xff;
    if cx == 0 {
        c.run(start, 3)?;
        c.and(W, 0, 0xff);
        c.set_reg(CX, 0);
        return c.ret();
    }
    c.run(start, 7)?;
    c.and(W, cx, 0xff);
    c.set_reg(CX, cx);
    let (bx, di, ax, dx) = (c.reg(BX), c.reg(DI), c.reg(AX), c.reg(DX));
    for v in [bx, di, ax, dx] {
        c.push(v);
    }
    c.call(0xc1f4, 0x61e0)?;
    let (bp0, bx1, si) = (c.reg(BP), c.reg(BX), c.reg(SI));
    let span = bp0.wrapping_sub(bx1);
    let h = (c.word_in(ES, si.wrapping_add(2)) & 0xff).wrapping_add(2);
    let q = span / cx;
    let (cnt, n) = if q >= h {
        (cx, 10)
    } else {
        let d = span.wrapping_sub(h);
        if d / cx >= 2 { (cx, 17) } else { (d >> 1, 20) }
    };
    if cnt == 0 {
        c.m.cpu.ip = 0x61e0;
        return Err(Bail);
    }
    c.run(0x61e0, n + 2 + 6)?;
    let di1 = c.sub(W, bp0, bx1);
    c.set_reg(DI, di1);
    let b = c.word_in(ES, si.wrapping_add(2));
    let b = c.and(W, b, 0xff);
    let b = c.add(W, b, 2);
    c.set_reg(BP, b);
    c.set_reg(AX, di1);
    c.xor(W, 0, 0);
    c.set_reg(DX, 0);
    c.m.cpu.mul_div(&mut c.m.hw, W, 6, cx as u32);
    let a = c.reg(AX);
    c.cmp(W, a, b);
    if n > 10 {
        let d = c.sub(W, di1, b);
        c.set_reg(DI, d);
        c.set_reg(AX, d);
        c.xor(W, 0, 0);
        c.set_reg(DX, 0);
        c.m.cpu.mul_div(&mut c.m.hw, W, 6, cx as u32);
        let a = c.reg(AX);
        c.set_reg(BP, a);
        c.cmp(W, a, 2);
        if n > 17 {
            c.set_reg(BP, 2);
            let k = c.shr1(W, d, 1);
            c.set_reg(CX, k);
        }
    }
    let d = c.pop();
    c.set_reg(DX, d);
    let a = c.pop();
    c.set_reg(AX, a);
    let _ = (ax, dx);
    let mut first = true;
    loop {
        if !first {
            c.run(0x6211, 6)?;
        }
        first = false;
        for r in [AX, BX, CX, DX, BP] {
            let v = c.reg(r);
            c.push(v);
        }
        c.call(0xc22f, 0x6219)?;
        let cx = c.word_in(SS, c.reg(SP).wrapping_add(4)).wrapping_sub(1);
        c.run(0x6219, 7 + if cx == 0 { 3 } else { 0 })?;
        for r in [BP, DX, CX, BX, AX] {
            let v = c.pop();
            c.set_reg(r, v);
        }
        let (bx, bp) = (c.reg(BX), c.reg(BP));
        let bx = c.add(W, bx, bp);
        c.set_reg(BX, bx);
        c.set_reg(CX, cx);
        if cx == 0 {
            break;
        }
    }
    let d = c.pop();
    c.set_reg(DI, d);
    let b = c.pop();
    c.set_reg(BX, b);
    c.call(0xc1f4, 0x6227)?;
    c.run(0x6227, 5)?;
    let si = c.reg(SI);
    let w = c.word_in(ES, si);
    let hi = c.and(B, w >> 8, 0x0f);
    let ax = hi << 8 | w & 0xff;
    c.set_reg(AX, ax);
    let dx = c.reg(DX);
    let dx = c.add(W, dx, ax);
    let dx = c.inc(W, dx);
    c.set_reg(DX, dx);
    c.ret()
}

/// `loc_08250`: [`map_click_handler_and_rect`], then the on-map
/// resource opened (`open_onmap_resource`).
pub fn map_click_and_resource(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8250)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x5ad3, 0x8253)?;
    c.run(0x8253, 1)?;
    super::resources::open_onmap_resource(c.m, 0xc13b)
}

/// `loc_07e1e`: [`skill_bars`] for the needs at 4705h in the panel
/// 18E9h (from (x + 80h, y + 2Dh) to its bottom), tall font, colour F0h.
pub fn skill_bars_4705(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7e1e)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xd068, 0x7e21)?;
    c.run(0x7e21, 7)?;
    c.set_reg(SI, 0x4705);
    let (x, y) = (c.word(0x18e9), c.word(0x18eb));
    let dx = c.add(W, x, 0x80);
    c.set_reg(DX, dx);
    let bx = c.add(W, y, 0x2d);
    c.set_reg(BX, bx);
    let bp = c.word(0x18ef);
    c.set_reg(BP, bp);
    c.set_word(0xdbe4, 0xf0);
    skill_bars_at(&mut c)
}

/// `loc_07e3d`: the 7 skill needs at SI drawn as bars of the sprites at
/// 192Fh (`sprite_column` from (DX, BX) to BP), their columns' x
/// ranges at 4C60h; when none is needed, the phrase 69h instead. DI kept.
pub fn skill_bars(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7e3d)?;
    let mut c = Ctx::new(m);
    skill_bars_at(&mut c)
}

fn skill_bars_at(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x7e3d, 1)?;
    c.call(0xc13b, 0x7e40)?;
    c.run(0x7e40, 7)?;
    let di0 = c.reg(DI);
    c.push(di0);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(DI, 0x4c60);
    c.set_reg(CX, 0x0e);
    c.xor(W, 0, 0);
    c.set_reg(AX, 0);
    c.rep(0xab);
    c.run(0x7e4d, 4)?;
    let si = c.reg(SI);
    c.set_reg(DI, si);
    c.set_reg(CX, 7);
    c.xor(B, 0, 0);
    c.m.cpu.string_op(&mut c.m.hw, 0xae, None, 0xf3); // repe scasb
    if c.flag(pc::cpu::ZF) {
        c.run(0x7e56, 4)?;
        let bx = c.reg(BX);
        let bx = c.add(W, bx, 5);
        c.set_reg(BX, bx);
        let dx = c.reg(DX);
        let dx = c.add(W, dx, 0x0c);
        c.set_reg(DX, dx);
        c.call(0xd04e, 0x7e61)?;
        c.run(0x7e61, 2)?;
        c.set_reg(AX, 0x69);
        c.call(0xd19b, 0x7e67)?;
        c.run(0x7e67, 2)?;
        let d = c.pop();
        c.set_reg(DI, d);
        return c.ret();
    }
    c.run(0x7e56, 2)?;
    c.xor(W, 0, 0);
    c.set_reg(DI, 0);
    for i in 0..7u16 {
        let si = c.reg(SI);
        let n = c.byte(si);
        if n != 0 {
            c.run(0x7e6b, 3 + 10)?;
            c.set_reg8(r8::AL, n);
            c.or(B, n as u16, n as u16);
            c.set_reg8(r8::CL, n);
            let bp = c.reg(BP);
            c.push(si);
            c.push(i);
            c.push(bp);
            let sp = c.byte(0x192f + i);
            c.xor(B, 0, 0);
            c.set_reg(AX, sp as u16);
            let d = c.shl1(W, i, 2);
            c.set_reg(DI, d);
            let dx = c.reg(DX);
            c.set_word(d.wrapping_add(0x4c60), dx);
            c.call(0x61d3, 0x7e87)?;
            c.run(0x7e87, 4)?;
            let (dx, d) = (c.reg(DX), c.reg(DI));
            c.set_word(d.wrapping_add(0x4c62), dx);
            for r in [BP, DI, SI] {
                let v = c.pop();
                c.set_reg(r, v);
            }
        } else {
            c.run(0x7e6b, 3)?;
            c.set_reg8(r8::AL, 0);
            c.or(B, 0, 0);
        }
        c.run(0x7e8e, 4 + if i == 6 { 2 } else { 0 })?;
        let si = c.reg(SI);
        let s = c.inc(W, si);
        c.set_reg(SI, s);
        let d = c.inc(W, i);
        c.set_reg(DI, d);
        c.cmp(W, d, 7);
    }
    let d = c.pop();
    c.set_reg(DI, d);
    c.ret()
}

/// `loc_05dce`: the map's shown locations listed at A5C0h (after those
/// of the other mode when [46EBh] has bit 40h; `loc_0633b` first when it
/// has bit 80h): each visible one (`loc_062c9`, no bit 80h of byte 0Ah)
/// gets an entry (location, screen x, y with the mode) and its icon
/// (`location_icon`, 5 on for one farther than [1176h] by
/// `record_map_distance` with its kind below 20h) drawn
/// (`draw_sprite_centred`). A 0 ends the list.
pub fn list_shown_locations(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5dce)?;
    let mut c = Ctx::new(m);
    let mode = c.byte(0x46eb);
    let neg = mode & 0x80 != 0;
    c.run(start, 3 + if neg { 2 } else { 0 })?;
    c.set_reg8(r8::AL, mode);
    c.or(B, mode as u16, mode as u16);
    if neg {
        let ax = c.reg(AX);
        c.push(ax);
        c.call(0x633b, 0x5dd9)?;
        c.run(0x5dd9, 1)?;
        let a = c.pop();
        c.set_reg(AX, a);
    }
    let al = c.reg8(r8::AL);
    let bit = al & 0x40;
    let mut di = 0xa5c0u16;
    if bit == 0 {
        c.run(0x5dda, 3 + 1)?;
        c.set_reg(DI, di);
        let a = c.and(B, al as u16, 0x40);
        c.set_reg8(r8::AL, a as u8);
    } else {
        let mut n = 3 + 1;
        di = di.wrapping_sub(6);
        loop {
            di = di.wrapping_add(6);
            n += 3;
            if c.word(di) == 0 {
                break;
            }
            n += 2;
            if c.byte(di.wrapping_add(5)) & bit != 0 {
                break;
            }
        }
        c.run(0x5dda, n + 1)?;
        let a = c.and(B, al as u16, 0x40);
        c.set_reg8(r8::AL, a as u8);
        let mut d = c.sub(W, 0xa5c0, 6);
        loop {
            d = c.add(W, d, 6);
            let w = c.word(d);
            c.cmp(W, w, 0);
            if w == 0 {
                break;
            }
            let b = c.byte(d.wrapping_add(5));
            c.test(B, b as u16, bit as u16);
            if b & bit != 0 {
                break;
            }
        }
        c.set_reg(DI, d);
    }
    c.set_reg(SI, 0x100);
    let mut si = 0x100u16;
    loop {
        let w = c.word(si);
        if w == 0xffff {
            c.run(0x5df4, 4)?;
            c.cmp(W, w, 0xffff);
            let di = c.reg(DI);
            c.set_word(di, 0);
            return c.ret();
        }
        let f = c.byte(si.wrapping_add(0x0a));
        'one: {
            if f & 0x80 != 0 {
                c.run(0x5df4, 4)?;
                c.cmp(W, w, 0xffff);
                c.test(B, f as u16, 0x80);
                break 'one;
            }
            c.run(0x5df4, 5)?;
            c.cmp(W, w, 0xffff);
            c.test(B, f as u16, 0x80);
            c.call(0x62c9, 0x5e02)?;
            if c.flag(CF) {
                c.run(0x5e02, 1)?;
                break 'one;
            }
            c.run(0x5e02, 7)?;
            let (di, s, dx, bx) = (c.reg(DI), c.reg(SI), c.reg(DX), c.reg(BX));
            c.set_word(di, s);
            c.set_word(di.wrapping_add(2), dx);
            let m2 = c.byte(0x46eb);
            let bx = (m2 as u16) << 8 | bx & 0xff;
            c.set_word(di.wrapping_add(4), bx);
            c.xor(B, 0, 0);
            c.set_reg(BX, bx & 0xff);
            c.call(0x5e42, 0x5e15)?;
            let cl = c.reg8(r8::CL);
            if cl < 0x20 {
                c.run(0x5e15, 7)?;
                c.cmp(B, cl as u16, 0x20);
                for r in [AX, BX, DX, SI] {
                    let v = c.reg(r);
                    c.push(v);
                }
                c.call(0x7c8f, 0x5e21)?;
                let d = c.reg(AX);
                let lim = c.word(0x1176);
                let far = d > lim;
                c.run(0x5e21, 6 + far as u32 + 3)?;
                c.cmp(W, d, lim);
                for r in [SI, DX, BX, AX] {
                    let v = c.pop();
                    c.set_reg(r, v);
                }
                if far {
                    let a = c.reg(AX);
                    let a = c.add(W, a, 5);
                    c.set_reg(AX, a);
                }
            } else {
                c.run(0x5e15, 2 + 3)?;
                c.cmp(B, cl as u16, 0x20);
            }
            let (s, d) = (c.reg(SI), c.reg(DI));
            c.push(s);
            c.push(d);
            c.call(0xc343, 0x5e33)?;
            c.run(0x5e33, 3)?;
            let d = c.pop();
            c.set_reg(DI, d);
            let s = c.pop();
            c.set_reg(SI, s);
            let d = c.add(W, d, 6);
            c.set_reg(DI, d);
        }
        si = c.reg(SI);
        c.run(0x5e38, 2)?;
        si = c.add(W, si, 0x1c);
        c.set_reg(SI, si);
    }
}

/// `loc_040f9`: in a map mode with [10h] set: the 51h map cells around
/// the position (`get_map_position`, `loc_0b56c` 9×9) with bit 40h whose
/// record (`loc_0409a`) is a location with bit 80h of byte 0Ah, due
/// (phase from its byte 0Bh) and ahead (`loc_05124`'s direction within
/// 60h of [11C7h]): the encounter told (its side in [0E1h], phrase CEh
/// or D0h, its place `loc_06231` + 48h, [23h] 3, `loc_0425b`,
/// `loc_04944`, `loc_0dbb2`, `loc_02efb`, `loc_02ffb`, `loc_0d397`).
pub fn scan_encounters(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x40f9)?;
    let mut c = Ctx::new(m);
    let mode = c.byte(0x11c9);
    if mode & 3 == 0 {
        c.run(start, 3)?;
        c.test(B, mode as u16, 3);
        return c.ret();
    }
    let w = c.word(0x10);
    if w == 0 {
        c.run(start, 5)?;
        c.test(B, mode as u16, 3);
        c.cmp(W, 0, 0);
        return c.ret();
    }
    c.run(start, 5)?;
    c.test(B, mode as u16, 3);
    c.cmp(W, w, 0);
    c.call(0x407e, 0x410b)?;
    c.run(0x410b, 3)?;
    c.set_reg(CX, 9);
    c.set_reg(AX, 9);
    c.call(0xb56c, 0x4114)?;
    c.run(0x4114, 1)?;
    c.set_reg(CX, 0x51);
    let mut cx = 0x51u16;
    loop {
        let si = c.reg(SI);
        let al = c.byte(si);
        'one: {
            if al & 0x40 == 0 {
                c.run(0x4117, 3)?;
                c.lods(B, DS);
                c.test(B, al as u16, 0x40);
                break 'one;
            }
            c.run(0x4117, 7)?;
            c.lods(B, DS);
            c.test(B, al as u16, 0x40);
            let si = c.reg(SI);
            let di = c.word(si);
            c.set_reg(DI, di);
            c.push(cx);
            c.push(si);
            c.call(0x409a, 0x4123)?;
            let tail = |c: &mut Ctx| -> Result<(), Bail> {
                c.run(0x417a, 2)?;
                let s = c.pop();
                c.set_reg(SI, s);
                let x = c.pop();
                c.set_reg(CX, x);
                Ok(())
            };
            if !c.flag(ZF) {
                c.run(0x4123, 1)?;
                tail(&mut c)?;
                break 'one;
            }
            let si = c.reg(SI);
            let f = c.byte(si.wrapping_add(0x0a));
            let ph = c.byte(0x2a);
            let due = c.byte(si.wrapping_add(0x0b));
            if f & 0x80 == 0 {
                c.run(0x4123, 3)?;
                c.test(B, f as u16, 0x80);
                tail(&mut c)?;
                break 'one;
            }
            if ph < due {
                c.run(0x4123, 6)?;
                c.test(B, f as u16, 0x80);
                c.set_reg8(r8::AL, ph);
                c.cmp(B, ph as u16, due as u16);
                tail(&mut c)?;
                break 'one;
            }
            c.run(0x4123, 8)?;
            c.test(B, f as u16, 0x80);
            c.set_reg8(r8::AL, ph);
            c.cmp(B, ph as u16, due as u16);
            c.set_reg(DI, si);
            c.call(0x5124, 0x4138)?;
            if c.flag(CF) {
                c.run(0x4138, 1)?;
                tail(&mut c)?;
                break 'one;
            }
            let al = c.reg8(r8::AL);
            let dir = c.byte(0x11c7);
            let v = al.wrapping_sub(dir).wrapping_add(0x60);
            if v >= 0xc0 {
                c.run(0x4138, 5)?;
                let a = c.sub(B, al as u16, dir as u16);
                let a = c.add(B, a, 0x60);
                c.set_reg8(r8::AL, a as u8);
                c.cmp(B, a, 0xc0);
                tail(&mut c)?;
                break 'one;
            }
            let right = v >= 0x60;
            c.run(0x4138, 5 + 4 + 2 * right as u32 + 3)?;
            let a = c.sub(B, al as u16, dir as u16);
            let a = c.add(B, a, 0x60);
            c.cmp(B, a, 0xc0);
            c.xor(B, 0, 0);
            c.cmp(B, a, 0x60);
            let mut ah = 0u16;
            let mut cxv = 0xceu16;
            if right {
                ah = c.inc(B, 0);
                cxv = c.add(W, 0xce, 2);
            }
            c.set_reg(AX, ah << 8 | a);
            c.set_reg(CX, cxv);
            c.set_word(0x11f5, cxv);
            c.set_byte(0xe1, ah as u8);
            c.call(0x6231, 0x415d)?;
            c.run(0x415d, 4)?;
            let ax = c.reg(AX);
            let ax = c.add(W, ax, 0x48);
            c.set_reg(AX, ax);
            c.set_word(0x11f3, ax);
            c.set_byte(0x23, 3);
            c.call(0x425b, 0x416b)?;
            for (at, to) in [(0x416bu16, 0x4944u16), (0x416e, 0xdbb2), (0x4171, 0x2efb), (0x4174, 0x2ffb), (0x4177, 0xd397)] {
                c.run(at, 1)?;
                c.call(to, at + 3)?;
            }
            tail(&mut c)?;
        }
        let si = c.reg(SI);
        cx = c.reg(CX).wrapping_sub(1);
        c.run(0x417c, 2 + (cx == 0) as u32)?;
        let s = c.add(W, si, 2);
        c.set_reg(SI, s);
        c.set_reg(CX, cx);
        if cx == 0 {
            return c.ret();
        }
    }
}

/// `loc_05f79`: with a caption up ([46F8h], taken): under the cursor's
/// hotspot (`loc_0d41b`, BP) another element: `loc_0d316`, `loc_0d2ea`
/// and [`fade_panel`]; on none (its first byte FFh) the caption cleared
/// ([46F7h] 0, the frame 1668h redrawn on the front buffer, `loc_07b2b`
/// with AL 8).
pub fn drop_caption(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5f79)?;
    let mut c = Ctx::new(m);
    let old = c.word(0x46f8);
    if old == 0 {
        c.run(start, 5)?;
        c.xor(W, 0, 0);
        c.set_reg(AX, 0);
        c.or(W, 0, 0);
        return c.ret();
    }
    c.run(start, 5)?;
    c.xor(W, 0, 0);
    c.set_word(0x46f8, 0);
    c.set_reg(AX, old);
    c.or(W, old, old);
    c.call(0xd41b, 0x5f86)?;
    let bp = c.reg(BP);
    let b = c.byte_in(SS, bp);
    let a = c.reg(AX);
    if b != 0xff {
        c.run(0x5f86, 4 + 1)?;
        c.set_reg(AX, a & 0xff00 | b as u16);
        let v = c.inc(B, b as u16);
        c.set_reg8(r8::AL, v as u8);
        c.call(0xd316, 0xd2e5)?;
        c.run(0xd2e5, 1)?;
        c.call(0xd2ea, 0xd2e8)?;
        c.run(0xd2e8, 1)?;
        return super::screens::fade_panel(c.m, 0xd280);
    }
    c.run(0x5f86, 3 + 4)?;
    c.set_reg8(r8::AL, 0);
    c.inc(B, 0xff);
    c.set_word(0x46f8, 0);
    c.set_byte(0x46f7, 0);
    c.set_reg(SI, 0x1668);
    c.call(0xc07c, 0x5fa2)?;
    c.run(0x5fa2, 2)?;
    c.set_word(0xdbe0, 0);
    c.call(0xc6ad, 0x5fab)?;
    c.run(0x5fab, 2)?;
    c.set_reg8(r8::AL, 8);
    super::troops::gfx_call_30_unless_redrawing(c.m, 0x7b2b)
}

/// `loc_041e1`: while [196Ch] counts down, CF clear. Else the 8 map cells
/// ahead (`loc_0b56c`, a row or a column by the direction [11C7h]) are
/// searched for a location (bit 40h, `loc_0409a`; with bit 80h of byte
/// 0Ah only once due) lying ahead (`loc_05124` within 20h of [11C7h]):
/// its offset (direction·32) in [1968h], its scene sprite (196Dh by
/// `calc_SAL_index`) in [196Ah], [196Ch] 6, CF set.
pub fn location_ahead(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x41e1)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x196c);
    if k != 0 {
        c.run(start, 5)?;
        c.cmp(B, k as u16, 0);
        let v = c.dec(B, k as u16);
        c.set_byte(0x196c, v as u8);
        c.set_flag(CF, false);
        return c.ret();
    }
    let dir = c.byte(0x11c7);
    let a = dir.wrapping_add(0x20);
    let col = a & 0x40 == 0;
    c.run(start, 8 + (!col) as u32 + 1)?;
    c.cmp(B, 0, 0);
    c.set_reg8(r8::AL, dir);
    let a = c.add(B, dir as u16, 0x20);
    c.set_reg8(r8::AL, a as u8);
    c.test(B, a, 0x40);
    if col {
        c.set_reg(CX, 1);
        c.set_reg(AX, 8);
    } else {
        c.set_reg(CX, 8);
        c.set_reg(AX, 1);
    }
    c.call(0xb56c, 0x41fb)?;
    c.run(0x41fb, 1)?;
    c.set_reg(CX, 8);
    let mut cx = 8u16;
    loop {
        let si = c.reg(SI);
        let al = c.byte(si);
        let next = |c: &mut Ctx, cx: u16| -> Result<bool, Bail> {
            let more = cx != 1;
            c.run(0x4203, 2 + if more { 0 } else { 2 })?;
            let s = c.reg(SI);
            let s = c.add(W, s, 2);
            c.set_reg(SI, s);
            c.set_reg(CX, cx - 1);
            if !more {
                c.set_flag(CF, false);
            }
            Ok(more)
        };
        if al & 0x40 == 0 {
            c.run(0x41fe, 3)?;
            c.lods(B, DS);
            c.test(B, al as u16, 0x40);
            if !next(&mut c, cx)? {
                return c.ret();
            }
            cx -= 1;
            continue;
        }
        c.run(0x41fe, 7)?;
        c.lods(B, DS);
        c.test(B, al as u16, 0x40);
        let si = c.reg(SI);
        let di = c.word(si);
        c.set_reg(DI, di);
        c.push(cx);
        c.push(si);
        c.call(0x409a, 0x4211)?;
        let skip = |c: &mut Ctx, at: u16, n: u32| -> Result<(), Bail> {
            c.run(at, n + 3)?;
            let s = c.pop();
            c.set_reg(SI, s);
            let x = c.pop();
            c.set_reg(CX, x);
            Ok(())
        };
        let found = 'f: {
            if !c.flag(ZF) {
                skip(&mut c, 0x4211, 1)?;
                break 'f false;
            }
            let si = c.reg(SI);
            let f = c.byte(si.wrapping_add(0x0a));
            if f & 0x80 != 0 {
                let ph = c.byte(0x2a);
                let due = c.byte(si.wrapping_add(0x0b));
                if ph < due {
                    skip(&mut c, 0x4211, 6)?;
                    break 'f false;
                }
                c.run(0x4211, 8)?;
                c.test(B, f as u16, 0x80);
                c.set_reg8(r8::AL, ph);
                c.cmp(B, ph as u16, due as u16);
            } else {
                c.run(0x4211, 5)?;
                c.test(B, f as u16, 0x80);
            }
            c.set_reg(DI, si);
            c.call(0x5124, 0x4226)?;
            if c.flag(CF) {
                skip(&mut c, 0x4226, 1)?;
                break 'f false;
            }
            let al = c.reg8(r8::AL);
            let v = al.wrapping_sub(dir).wrapping_add(0x20);
            if v >= 0x40 {
                skip(&mut c, 0x4226, 5)?;
                break 'f false;
            }
            c.run(0x4226, 15)?;
            let a = c.sub(B, al as u16, dir as u16);
            let a = c.add(B, a, 0x20);
            c.cmp(B, a, 0x40);
            let a = c.sub(B, a, 0x20);
            let a = a as u8 as i8 as i16 as u16;
            let a = c.shl1(W, a, 5);
            c.set_word(0x1968, a);
            c.xor(W, 0, 0);
            c.set_reg(AX, 0);
            c.call(0x5e4f, 0x4247)?;
            c.run(0x4247, 8)?;
            c.set_reg(BX, 0x196d);
            let ax = c.reg(AX);
            let s = c.byte(0x196du16.wrapping_add(ax & 0xff));
            let ax = ax & 0xff00 | s as u16;
            c.set_reg(AX, ax);
            c.set_word(0x196a, ax);
            let s = c.pop();
            c.set_reg(SI, s);
            let x = c.pop();
            c.set_reg(CX, x);
            c.set_byte(0x196c, 6);
            c.set_flag(CF, true);
            true
        };
        if found {
            return c.ret();
        }
        if !next(&mut c, cx)? {
            return c.ret();
        }
        cx -= 1;
    }
}

/// `loc_062f2`: the troops redrawn (`loc_068eb`), then the marker 36h at
/// the troop SI on the map: at its own place ([`marker_at_dx_bx`]'s view
/// check `loc_062d6`) when it is at its location (bit 40h), else at its
/// location's ([`marker_at_record`]).
pub fn troop_marker(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x62f2)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x68eb, 0x62f5)?;
    let si = c.reg(SI);
    let b3 = c.byte(si.wrapping_add(3));
    if b3 & 0x40 != 0 {
        c.run(0x62f5, 5)?;
        c.test(B, b3 as u16, 0x40);
        let (x, y) = (c.word(si.wrapping_add(6)), c.word(si.wrapping_add(8)));
        c.set_reg(DX, x);
        c.set_reg(BX, y);
        c.call(0x62d6, 0x630f)?;
        return map_marker(&mut c, 0x630f, 2, |c| c.set_reg(AX, 0x36));
    }
    c.run(0x62f5, 4)?;
    c.test(B, b3 as u16, 0x40);
    let l = c.word(si.wrapping_add(4));
    c.set_reg(SI, l);
    c.call(0x62c9, 0x6301)?;
    map_marker(&mut c, 0x6301, 2, |c| c.set_reg(AX, 0x36))
}

/// `loc_062fe`: the marker 36h at the record SI's place on the map, when
/// it is in view (`loc_062c9`).
pub fn marker_at_record(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x62fe)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x62c9, 0x6301)?;
    map_marker(&mut c, 0x6301, 2, |c| c.set_reg(AX, 0x36))
}

/// `loc_06314`: the marker 4Ch at the map position (`get_map_position`)
/// when in view (`loc_062d6`), 0Dh higher.
pub fn marker_at_dx_bx(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x6314)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x407e, 0x6317)?;
    c.run(0x6317, 1)?;
    c.call(0x62d6, 0x631a)?;
    map_marker(&mut c, 0x631a, 4, |c| {
        c.set_reg(AX, 0x4c);
        let f = c.m.cpu.flags();
        c.push(f);
        let dx = c.reg(DX);
        let d = c.sub(W, dx, 0x0d);
        c.set_reg(DX, d);
        let f = c.pop();
        c.m.cpu.set_flags(f);
    })
}

/// From `loc_06322` (after `pre` instructions from `at`, whose effect
/// `ops` has; CF from the view check): unless CF, the sprite AX drawn at
/// (DX, BX - its height) clipped (`draw_sprite_clipped_clobbering_bx_dx`)
/// with its resource opened meanwhile.
fn map_marker(c: &mut Ctx, at: u16, pre: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    let out = c.flag(CF);
    c.run(at, pre + if out { 2 } else { 4 })?;
    ops(c);
    if out {
        return c.ret();
    }
    let r = c.word(0x2784);
    c.push(r);
    let ax = c.reg(AX);
    c.push(ax);
    c.call(0xc137, 0x632c)?;
    c.run(0x632c, 2)?;
    let a = c.pop();
    c.set_reg(AX, a);
    c.call(0xc1f4, 0x6330)?;
    c.run(0x6330, 2)?;
    let si = c.reg(SI);
    let h = c.byte_in(ES, si.wrapping_add(2));
    let bx = c.reg(BX);
    let bl = c.sub(B, bx & 0xff, h as u16);
    c.set_reg(BX, bx & 0xff00 | bl);
    c.call(0xc30d, 0x6337)?;
    c.run(0x6337, 2)?;
    let a = c.pop();
    c.set_reg(AX, a);
    super::resources::open_resource_by_index(c.m, 0xc13e)
}

/// `loc_057e5`: the map's region colours into the 100h bytes at BP
/// (70h for none; resource 3Ah opened; registers kept): for each
/// location from 100h (no bit 80h of byte 0Ah), its region (byte 10h)
/// gets 75h (78h with bit 2 of [2942h]), or 50h + its byte 12h / 16
/// for a sietch with bit 40h; with [4722h] instead the troops' strength
/// there (`loc_06639` with `loc_05728`, then `loc_0586e`, BH).
pub fn region_colours(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x57e5)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xe270, 0x57e8)?;
    c.run(0x57e8, 3)?;
    let es = c.sreg(ES);
    c.push(es);
    c.set_reg(AX, 0x3a);
    c.call(0xc13e, 0x57ef)?;
    c.run(0x57ef, 6)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let e = c.pop();
    c.set_sreg(ES, e);
    let bp = c.reg(BP);
    c.set_reg(DI, bp);
    c.set_reg8(r8::AL, 0x70);
    c.set_reg(CX, 0x100);
    c.rep(0xaa);
    let k = c.byte(0x4722);
    if k == 0 {
        let alt = c.byte(0x2942) & 2 != 0;
        let mut n = 2 + 3;
        let mut p = 0x100u16;
        loop {
            let f = c.byte(p.wrapping_add(0x0a));
            n += 3;
            if f & 0x80 == 0 {
                n += 6 + alt as u32 + 2 + if f & 0x40 != 0 { 6 } else { 0 } + 1;
            }
            p = p.wrapping_add(0x1c);
            n += 3;
            if c.byte(p) == 0xff {
                break;
            }
        }
        c.run(0x57fa, n + 2)?;
        c.cmp(B, 0, 0);
        c.set_reg(SI, 0x100);
        c.set_reg(DI, bp);
        c.xor(W, 0, 0);
        let mut bx = 0u16;
        let mut p = 0x100u16;
        loop {
            let f = c.byte(p.wrapping_add(0x0a));
            c.set_reg8(r8::AL, f);
            c.test(B, f as u16, 0x80);
            if f & 0x80 == 0 {
                bx = c.byte(p.wrapping_add(0x10)) as u16;
                c.test(B, f as u16, 0x40);
                let fl = c.m.cpu.flags();
                c.push(fl);
                let w = c.byte(0x2942);
                c.test(B, w as u16, 2);
                let mut al = if alt { 0x78u16 } else { 0x75 };
                let fl = c.pop();
                c.m.cpu.set_flags(fl);
                if f & 0x40 != 0 {
                    let b = c.byte(p.wrapping_add(0x12)) as u16;
                    let b = c.shr1(B, b, 4);
                    al = c.add(B, b, 0x50);
                }
                c.set_reg8(r8::AL, al as u8);
                c.set_byte(bx.wrapping_add(bp), al as u8);
            }
            let np = c.add(W, p, 0x1c);
            let b = c.byte(np);
            c.cmp(B, b as u16, 0xff);
            p = np;
            if b == 0xff {
                break;
            }
        }
        c.set_reg(SI, p);
        c.set_reg(BX, bx);
        let e = c.pop();
        c.set_sreg(ES, e);
        c.call(code::RESTORE_REGS, 0x583e)?;
        c.run(0x583e, 1)?;
        return c.ret();
    }
    c.run(0x57fa, 3)?;
    c.cmp(B, k as u16, 0);
    c.set_reg(DI, 0x100);
    let mut di = 0x100u16;
    loop {
        let f = c.byte(di.wrapping_add(0x0a));
        if f & 0x80 != 0 {
            c.run(0x5842, 3)?;
            c.set_reg8(r8::AL, f);
            c.test(B, f as u16, 0x80);
        } else {
            c.run(0x5842, 8)?;
            c.set_reg8(r8::AL, f);
            c.test(B, f as u16, 0x80);
            let bp = c.reg(BP);
            c.push(bp);
            c.xor(W, 0, 0);
            c.set_reg(BX, 0);
            c.set_reg(CX, 0);
            c.set_reg(BP, 0x5728);
            c.call(0x6639, 0x5854)?;
            c.run(0x5854, 2)?;
            let b = c.pop();
            c.set_reg(BP, b);
            c.call(0x586e, 0x5858)?;
            c.run(0x5858, 4)?;
            let d = c.reg(DI);
            let r = c.byte(d.wrapping_add(0x10));
            c.xor(B, 0, 0);
            c.set_reg(AX, r as u16);
            c.set_reg(SI, r as u16);
            let bp = c.reg(BP);
            let bh = c.reg8(r8::BH);
            c.m.cpu.write8(&mut c.m.hw, SS, bp.wrapping_add(r as u16), bh);
            di = d;
        }
        let nd = di.wrapping_add(0x1c);
        let nb = c.byte(nd);
        c.run(0x5861, 3 + if nb == 0xff { 2 } else { 0 })?;
        let d = c.add(W, di, 0x1c);
        c.set_reg(DI, d);
        c.cmp(B, nb as u16, 0xff);
        di = d;
        if nb == 0xff {
            break;
        }
    }
    let e = c.pop();
    c.set_sreg(ES, e);
    c.call(code::RESTORE_REGS, 0x586d)?;
    c.run(0x586d, 1)?;
    c.ret()
}

/// `loc_0186b`: the map screen toggled ([0FBh] negated): to the map
/// ([`open_map_screen`]), or back: `loc_0d2bd`, `leave_map_screen`,
/// [28E7h] = [28E8h], `loc_0b930`, the buttons 1C14h/1C22h back to 80h
/// and drawn (`loc_0c097`, BP D75Ah), the transition 34h (`loc_0c108`),
/// front buffer, `loc_0ae04`, [0DC5Ah] the time, the head up
/// ([`ui_head_animate_up`]). With [46D9h] set the code jumps to 2DB1h
/// instead; the port leaves that to the original.
pub fn toggle_map_screen(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x186b)?;
    let mut c = Ctx::new(m);
    toggle_map_at(&mut c, start, 0)
}

fn toggle_map_at(c: &mut Ctx, at: u16, pre: u32) -> Result<(), Bail> {
    c.run(at, pre + 1)?;
    c.call(0xdaa3, 0x186e)?;
    let v = c.byte(0xfb);
    let nv = v.wrapping_neg();
    if nv & 0x80 != 0 {
        c.run(0x186e, 3)?;
        let r = c.neg(B, v as u16);
        c.set_byte(0xfb, r as u8);
        return open_map_screen(c.m, 0x5a1a);
    }
    c.run(0x186e, 3)?;
    let r = c.neg(B, v as u16);
    c.set_byte(0xfb, r as u8);
    c.call(0xd2bd, 0x187a)?;
    c.run(0x187a, 1)?;
    c.call(0x5adf, 0x187d)?;
    c.run(0x187d, 3)?;
    let a = c.byte(0x28e8);
    c.set_reg8(r8::AL, a);
    c.set_byte(0x28e7, a);
    c.call(0xb930, 0x1886)?;
    c.run(0x1886, 4)?;
    c.set_word(0x1c14, 0x80);
    c.set_word(0x1c22, 0x80);
    c.set_reg(BP, 0xd75a);
    c.call(0xc097, 0x1898)?;
    if c.byte(0x46d9) != 0 {
        c.m.cpu.ip = 0x1898;
        return Err(Bail);
    }
    c.run(0x1898, 6)?;
    c.set_reg8(r8::AL, 0x34);
    c.set_reg(BP, 0x2db1);
    c.cmp(B, 0, 0);
    c.xor(W, 0, 0);
    c.set_reg(DX, 0);
    c.call(0xc108, 0x18ab)?;
    c.run(0x18ab, 1)?;
    c.call(0xc07c, 0x18ae)?;
    c.run(0x18ae, 1)?;
    c.call(0xae04, 0x18b1)?;
    c.run(0x18b1, 3)?;
    let t = c.word(0xce7a);
    c.set_reg(AX, t);
    c.set_word(0xdc5a, t);
    super::ui::ui_head_animate_up(c.m, 0x17e6)
}

/// `loc_08763`: with [46F3h] set, [`redraw_8770`] and the map screen
/// toggled ([`toggle_map_screen`]); else just [`redraw_8770`].
pub fn redraw_and_toggle(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8763)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x46f3);
    if k == 0 {
        c.run(start, 2)?;
        c.cmp(B, 0, 0);
        return redraw_8770(c.m, 0x8770);
    }
    c.run(start, 3)?;
    c.cmp(B, k as u16, 0);
    c.call(0x8770, 0x876d)?;
    toggle_map_at(&mut c, 0x876d, 1)
}

/// `loc_05c03`: the map's hover: after 3E8h ticks over the element 194Ah
/// the map redrawn (`loc_05beb`, cursor hidden meanwhile); over the map
/// (4710h, as [0DBE0h] or [0DBE2h]) the nearest shown location within 9
/// of the cursor (`nearest_shown_location`, `point_in_rect` first)
/// becomes [46FCh], its caption redrawn (`loc_05692`) when it changed,
/// and with none the header by the cursor (`map_header_by_cursor`).
pub fn map_hover(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5c03)?;
    let mut c = Ctx::new(m);
    let e0 = c.word(0xdbe0);
    if e0 == 0x194a {
        let (t1, t0) = (c.word(0xce7a), c.word(0xdc5a));
        let t = t1.wrapping_sub(t0);
        if t >= 0x3e8 {
            c.run(start, 8)?;
            c.set_reg(DI, e0);
            c.cmp(W, e0, 0x194a);
            let (t1, t0) = (c.word(0xce7a), c.word(0xdc5a));
            let a = c.sub(W, t1, t0);
            c.set_reg(AX, a);
            c.cmp(W, a, 0x3e8);
            c.call(0xdbb2, 0x5c1c)?;
            c.run(0x5c1c, 1)?;
            c.call(0x5beb, 0x5c1f)?;
            c.run(0x5c1f, 1)?;
            return super::mouse::draw_mouse(c.m, 0xdbec);
        }
    }
    let e2 = c.word(0xdbe2);
    let pre = 3 + if e0 == 0x194a { 4 } else { 0 };
    let over = e0 == 0x4710 || e2 == 0x4710;
    let n = pre + 2 + if e0 == 0x4710 { 1 } else { 3 + if over { 1 } else { 1 } };
    c.run(start, n)?;
    c.set_reg(DI, e0);
    c.cmp(W, e0, 0x194a);
    if e0 == 0x194a {
        let (t1, t0) = (c.word(0xce7a), c.word(0xdc5a));
            let a = c.sub(W, t1, t0);
        c.set_reg(AX, a);
        c.cmp(W, a, 0x3e8);
    }
    c.cmp(W, e0, 0x4710);
    if e0 != 0x4710 {
        c.set_reg(DI, e2);
        c.cmp(W, e2, 0x4710);
        if e2 != 0x4710 {
            return c.ret();
        }
    }
    c.call(0xd6fe, 0x5c35)?;
    if c.flag(CF) {
        c.run(0x5c35, 4)?;
        c.set_byte(0x46eb, 0x40);
        c.set_reg8(r8::AL, 0xff);
        c.call(0x5e6d, 0x5c41)?;
        let ax = c.reg(AX);
        c.run(0x5c41, 3 + if ax < 9 { 0 } else { 1 })?;
        c.set_byte(0x46eb, 0xc0);
        c.cmp(W, ax, 9);
        if ax >= 9 {
            c.xor(W, 0, 0);
            c.set_reg(DI, 0);
        }
    } else {
        c.run(0x5c35, 2)?;
        c.xor(W, 0, 0);
        c.set_reg(DI, 0);
    }
    let di = c.reg(DI);
    let old = c.word(0x46fc);
    c.run(0x5c4d, 4 + if old != di { 5 } else { 0 })?;
    c.set_reg(AX, old);
    c.set_word(0x46fc, di);
    c.cmp(W, old, di);
    if old != di {
        let (bx, dx) = (c.reg(BX), c.reg(DX));
        c.push(bx);
        c.push(dx);
        c.push(di);
        let fb = c.word(0xdbda);
        c.push(fb);
        c.call(0xdbb2, 0x5c61)?;
        c.run(0x5c61, 1)?;
        c.call(0xc08e, 0x5c64)?;
        c.run(0x5c64, 1)?;
        c.call(0x5692, 0x5c67)?;
        c.run(0x5c67, 4)?;
        let v = c.pop();
        c.set_word(0xdbda, v);
        for r in [DI, DX, BX] {
            let v = c.pop();
            c.set_reg(r, v);
        }
    }
    let di = c.reg(DI);
    if di != 0 {
        c.run(0x5c6e, 3)?;
        c.or(W, di, di);
        return c.ret();
    }
    c.run(0x5c6e, 3)?;
    c.or(W, 0, 0);
    c.call(0x5746, 0x5c75)?;
    c.run(0x5c75, 1)?;
    c.ret()
}

/// `loc_07443`: the location DI raided: its byte 0Bh 5 and the circle
/// (`location_circle`), then the day + 2; charisma down 4, `loc_06f56`
/// with 1; bit 8 of byte 0Ah; its region mark into [115Ah]; its troops
/// through `loc_075af` and `loc_075ea`, then `loc_0762a` repeated until
/// CX is at most DX (1 on a quarter of the seed's ticks); unless
/// `loc_01cda` leaves DL above 1, [0C2h] 1, bit 2 of [0FF7h] and
/// [1007h] cleared and `loc_0765e`; then [`count_shown_location`].
pub fn location_raided(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x7443)?;
    let mut c = Ctx::new(m);
    c.run(start, 4)?;
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.push(si);
    c.push(di);
    c.set_byte(di.wrapping_add(0x0b), 5);
    c.call(0x644e, 0x744c)?;
    c.run(0x744c, 3)?;
    let di = c.pop();
    c.set_reg(DI, di);
    let si = c.pop();
    c.set_reg(SI, si);
    c.call(0x1ac5, 0x7451)?;
    c.run(0x7451, 4)?;
    let al = c.reg8(r8::AL);
    let a = c.add(B, al as u16, 2);
    let di = c.reg(DI);
    c.set_byte(di.wrapping_add(0x0b), a as u8);
    c.set_reg8(r8::AL, 4);
    c.call(0x6f78, 0x745b)?;
    c.run(0x745b, 2)?;
    c.set_reg8(r8::AL, 1);
    c.call(0x6f56, 0x7460)?;
    c.run(0x7460, 9)?;
    let di = c.reg(DI);
    let f = c.byte(di.wrapping_add(0x0a));
    let f = c.or(B, f as u16, 8);
    c.set_byte(di.wrapping_add(0x0a), f as u8);
    let cx = c.reg(CX);
    c.push(cx);
    let cl = c.byte(di);
    c.set_reg8(r8::CL, cl);
    let r = c.m.cpu.shift(0, W, 0x8000, (cl & 0x1f) as u32) as u16;
    c.set_reg(AX, r);
    c.set_word(0x115a, r);
    let x = c.pop();
    c.set_reg(CX, x);
    c.set_reg(BP, 0x75af);
    c.call(0x6603, 0x7476)?;
    c.run(0x7476, 2)?;
    c.set_reg(BP, 0x75ea);
    c.call(0x6603, 0x747c)?;
    let seed = c.word(0);
    let z = seed & 3 == 0;
    c.run(0x747c, 3 + z as u32)?;
    c.xor(W, 0, 0);
    c.test(W, seed, 3);
    let mut dx = 0u16;
    if z {
        dx = c.inc(W, 0);
    }
    c.set_reg(DX, dx);
    loop {
        c.run(0x7487, 3)?;
        c.xor(W, 0, 0);
        c.set_reg(CX, 0);
        c.set_reg(BP, 0x762a);
        c.call(0x6603, 0x748f)?;
        let (cx, dx) = (c.reg(CX), c.reg(DX));
        c.run(0x748f, 2 + if cx > dx { 0 } else { 3 })?;
        c.cmp(W, cx, dx);
        if cx <= dx {
            break;
        }
    }
    let (si, di) = (c.reg(SI), c.reg(DI));
    c.push(si);
    c.push(di);
    c.call(0x1cda, 0x7498)?;
    let dl = c.reg8(r8::DL);
    if dl > 1 {
        c.run(0x7498, 2 + 3)?;
        c.cmp(B, dl as u16, 1);
    } else {
        c.run(0x7498, 7)?;
        c.cmp(B, dl as u16, 1);
        c.set_byte(0xc2, 1);
        for p in [0xff7u16, 0x1007] {
            let v = c.byte(p);
            let v = c.and(B, v as u16, 0xfd);
            c.set_byte(p, v as u8);
        }
        let d = c.pop();
        c.set_reg(DI, d);
        c.call(0x765e, 0x74b0)?;
        c.run(0x74b0, 4)?;
        let d = c.reg(DI);
        c.push(d);
    }
    let d = c.pop();
    c.set_reg(DI, d);
    let s = c.pop();
    c.set_reg(SI, s);
    count_shown_location(c.m, 0x5d50)
}

/// `map_func_qq`: with [46DEh] set, each location from 100h with bit 20h
/// of byte 0Ah gains spice (byte 1Bh, up to FAh) by half the cells of
/// kind 10h around it + 1 (`count_10_bytes` at its map cell, word 6);
/// then [`spread_spice`].
pub fn grow_spice(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x63f0)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x46de);
    if k == 0 {
        c.run(start, 3)?;
        c.cmp(B, 0, 0);
        return c.ret();
    }
    c.run(start, 4)?;
    c.cmp(B, k as u16, 0);
    let es = c.word(0xdd00);
    c.set_sreg(ES, es);
    c.set_reg(DI, 0x100);
    let mut di = 0x100u16;
    loop {
        let al = c.byte(di.wrapping_add(0x0a));
        let bl = c.byte(di.wrapping_add(0x1b));
        'one: {
            if al & 0x20 == 0 {
                c.run(0x63fe, 3)?;
                c.set_reg8(r8::AL, al);
                c.test(B, al as u16, 0x20);
                break 'one;
            }
            if bl >= 0xfa {
                c.run(0x63fe, 7)?;
                c.set_reg8(r8::AL, al);
                c.test(B, al as u16, 0x20);
                c.set_reg(BX, 0xfa00 | bl as u16);
                c.cmp(B, bl as u16, 0xfa);
                break 'one;
            }
            c.run(0x63fe, 9)?;
            c.set_reg8(r8::AL, al);
            c.test(B, al as u16, 0x20);
            c.set_reg(BX, 0xfa00 | bl as u16);
            c.cmp(B, bl as u16, 0xfa);
            let si = c.word(di.wrapping_add(6));
            c.set_reg(SI, si);
            c.call(0x642e, 0x6414)?;
            let dx = c.reg(DX);
            let d = (dx >> 1).wrapping_add(1);
            let v = bl.wrapping_add(d as u8);
            let cap = v >= 0xfa;
            c.run(0x6414, 5 + cap as u32 + 1)?;
            let d = c.shr1(W, dx, 1);
            let d = c.inc(W, d);
            c.set_reg(DX, d);
            let v = c.add(B, bl as u16, d & 0xff);
            c.cmp(B, v, 0xfa);
            let v = if cap { 0xfa } else { v as u8 };
            c.set_reg(BX, 0xfa00 | v as u16);
            let di = c.reg(DI);
            c.set_byte(di.wrapping_add(0x1b), v);
        }
        di = c.reg(DI);
        let nd = di.wrapping_add(0x1c);
        let w = c.word(nd);
        c.run(0x6422, 3 + if w == 0xffff { 1 } else { 0 })?;
        let d = c.add(W, di, 0x1c);
        c.set_reg(DI, d);
        c.cmp(W, w, 0xffff);
        di = d;
        if w == 0xffff {
            break;
        }
    }
    spread_at(&mut c, 0x65b6, 0)
}

/// `loc_065b6`: 46h steps of the shift-register seed CS:[65B4h] (taps
/// 402h); for each, the map cells at it and every 7FFh after (below
/// C5F9h, in [0DD00h]) of kind 10h become kind 20h, counted in BP; with
/// any in the map's hotspot mode [46ECh] is counted. Bails at entry with
/// DF set.
pub fn spread_spice(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x65b6)?;
    let mut c = Ctx::new(m);
    spread_at(&mut c, start, 0)
}

fn spread_at(c: &mut Ctx, at: u16, pre: u32) -> Result<(), Bail> {
    if c.flag(pc::cpu::DF) {
        c.m.cpu.ip = at;
        return Err(Bail);
    }
    let seg = c.word(0xdd00);
    let mut si = c.code_word(0x65b4);
    let mut bp = 0u16;
    let mut n = pre + 4;
    let mut writes: Vec<(u16, u8)> = Vec::new();
    let mut ax = c.reg(AX);
    let mut di = 0u16;
    for _ in 0..0x46 {
        let cf = si & 1 != 0;
        si >>= 1;
        n += 2;
        if cf {
            si ^= 0x402;
            n += 1;
        }
        di = si;
        n += 1;
        loop {
            let mut al = match writes.iter().rev().find(|w| w.0 == di) {
                Some(w) => w.1,
                None => c.byte_at_seg(seg, di),
            };
            let ah = al & 0x30;
            n += 5;
            if ah == 0x10 {
                al = al & 0xcf | 0x20;
                writes.push((di, al));
                bp = bp.wrapping_add(1);
                n += 5;
            }
            ax = (ah as u16) << 8 | al as u16;
            di = di.wrapping_add(0x7ff);
            n += 3;
            if di >= 0xc5f9 {
                break;
            }
        }
        n += 1;
    }
    let mode = c.byte(0x46eb);
    n += 3 + if bp == 0 { 1 } else { 2 + if mode & 0x80 != 0 { 2 } else { 1 } };
    if n > 300_000 {
        c.m.cpu.ip = at;
        return Err(Bail);
    }
    c.run(at, n)?;
    c.set_sreg(ES, seg);
    for (p, v) in writes {
        c.m.cpu.write8(&mut c.m.hw, ES, p, v);
    }
    c.set_reg(AX, ax);
    c.set_reg(DI, di);
    c.set_reg(SI, si);
    c.set_reg(CX, 0);
    c.set_reg(BP, bp);
    c.set_word_in(pc::cpu::CS, 0x65b4, si);
    c.or(W, bp, bp);
    if bp != 0 {
        c.cmp(B, mode as u16, 0);
        if mode & 0x80 != 0 {
            let v = c.byte(0x46ec);
            let v = c.inc(B, v as u16);
            c.set_byte(0x46ec, v as u8);
        }
    }
    c.ret()
}

/// `sub_59c1`: with the map point up ([4723h]): below 98h it moves by
/// (DI, CX), kept 5 inside 0..8Ch, 0..18h ([11C1h], [11C3h]; drawn
/// before and after by [`gfx_call_3961`]); from 98h down
/// [`map_point_moved`].
pub fn move_map_point(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x59c1)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x4723);
    if k == 0 {
        c.run(start, 3)?;
        c.cmp(B, 0, 0);
        return c.ret();
    }
    let bx = c.reg(BX);
    if bx >= 0x98 {
        c.run(start, 4)?;
        c.cmp(B, k as u16, 0);
        c.cmp(W, bx, 0x98);
        return map_point_moved(c.m, 0x599f);
    }
    c.run(start, 5)?;
    c.cmp(B, k as u16, 0);
    c.cmp(W, bx, 0x98);
    c.call(0x5982, 0x59d1)?;
    let (di, cx) = (c.reg(DI), c.reg(CX));
    let (x, y) = (c.word(0x11c1), c.word(0x11c3));
    let clamp = |v: u16, lim: u16, d: u16| -> (u16, u32) {
        let a = v.wrapping_add(d).wrapping_sub(5);
        if a < lim {
            (d, 0)
        } else if (a as i16) < lim as i16 {
            (d.wrapping_sub(a), 2)
        } else {
            (d.wrapping_sub(a.wrapping_sub(lim)), 3)
        }
    };
    let (d1, e1) = clamp(x, 0x8c, di);
    let (d2, e2) = clamp(y, 0x18, cx);
    c.run(0x59d1, 6 + e1 + 6 + e2 + 2)?;
    c.set_reg(SI, 0x11c1);
    let a = c.add(W, x, di);
    let a = c.sub(W, a, 5);
    c.cmp(W, a, 0x8c);
    let mut a1 = a;
    if e1 == 3 {
        a1 = c.sub(W, a, 0x8c);
    }
    if e1 != 0 {
        c.sub(W, di, a1);
    }
    c.set_reg(DI, d1);
    let nx = c.add(W, x, d1);
    c.set_word(0x11c1, nx);
    let a = c.add(W, y, cx);
    let a = c.sub(W, a, 5);
    c.cmp(W, a, 0x18);
    let mut a2 = a;
    if e2 == 3 {
        a2 = c.sub(W, a, 0x18);
    }
    if e2 != 0 {
        c.sub(W, cx, a2);
    }
    c.set_reg(CX, d2);
    c.set_reg(AX, a2);
    let ny = c.add(W, y, d2);
    c.set_word(0x11c3, ny);
    gfx_call_3961(c.m, 0x5982)
}

/// `loc_05206`: a step from (DX, BX) along the heading `heading_51cb`
/// sets: the `direction_step` (DX, BX) scaled by the row factor
/// [4880h + 2|BX|] / 20h; the across part plus the fraction [11CCh] carried
/// (above 1FFh it divides the along part instead), its whole part added
/// to BX, the along part to DX; past the pole (BX + 60h from C0h) the
/// heading [11C7h] and DX turn by 80h / 8000h. Bails before a division
/// that would fault.
pub fn heading_step(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5206)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x51cb, 0x5209)?;
    let (bx0, dx0) = (c.reg(BX), c.reg(DX));
    let neg = bx0 & 0x4000 != 0;
    c.run(0x5209, 7 + neg as u32)?;
    let h = c.byte(0x11c7);
    c.set_reg8(r8::AL, h);
    c.push(dx0);
    c.push(bx0);
    let mut b = c.shl1(W, bx0, 1);
    if neg {
        b = c.neg(W, b);
    }
    c.set_reg(BX, b);
    let bp = c.word(b.wrapping_add(0x4880));
    c.set_reg(BP, bp);
    c.call(0x5198, 0x521b)?;
    let (bp, dx, bx) = (c.reg(BP), c.reg(DX), c.reg(BX));
    let fits = |q: i32| (-0x7fff..=0x7fff).contains(&q);
    let q1 = (bp as i16 as i32) * (dx as i16 as i32) / 0x20;
    let q2 = (bx as i16 as i32) * (bp as i16 as i32) / 0x20;
    if !fits(q1) || !fits(q2) {
        c.m.cpu.ip = 0x521b;
        return Err(Bail);
    }
    let (q1, q2) = (q1 as u16, q2 as u16);
    let neg2 = q2 & 0x8000 != 0;
    let frac = c.word(0x11cc);
    let s = (if neg2 { q2.wrapping_neg() } else { q2 }).wrapping_add(frac);
    let big = s >> 8 > 1;
    if big {
        let q3 = ((q1 as i16 as i32) << 8) / (s as i16 as i32);
        if !fits(q3) {
            c.m.cpu.ip = 0x521b;
            return Err(Bail);
        }
    }
    let ax = if big { 0x100 } else { s };
    let mut v = (ax >> 8) as u8 as i8 as i16 as u16;
    if neg2 {
        v = v.wrapping_neg();
    }
    let t = bx0.wrapping_add(v).wrapping_add(0x60);
    let pole = t >= 0xc0;
    c.run(0x521b, 14 + neg2 as u32 + if big { 9 } else { 0 } + 5 + neg2 as u32 + 8 + if pole { 2 } else { 0 } + 1)?;
    c.set_reg(CX, 0x20);
    c.set_reg(AX, bp);
    c.m.cpu.mul_div(&mut c.m.hw, W, 5, dx as u32); // imul dx
    c.m.cpu.mul_div(&mut c.m.hw, W, 7, 0x20); // idiv cx
    let a = c.reg(AX);
    c.set_reg(AX, bx);
    c.set_reg(BX, a);
    c.m.cpu.mul_div(&mut c.m.hw, W, 5, bp as u32); // imul bp
    c.m.cpu.mul_div(&mut c.m.hw, W, 7, 0x20); // idiv cx
    c.set_reg(DX, q1);
    c.set_reg(BX, q2);
    c.or(W, q2, q2);
    let mut a = q2;
    if neg2 {
        a = c.neg(W, q2);
    }
    let a = c.add(W, a, frac);
    c.set_reg(AX, a);
    c.cmp(B, a >> 8, 1);
    if big {
        c.set_reg(CX, s);
        let hi = if q1 & 0x8000 != 0 { 0xff00 } else { 0 };
        c.set_reg(DX, hi | q1 >> 8);
        c.set_reg(AX, q1 << 8);
        c.xor(B, 0, 0);
        c.m.cpu.mul_div(&mut c.m.hw, W, 7, s as u32); // idiv cx
        let q3 = c.reg(AX);
        c.set_reg(DX, q3);
        c.set_reg(AX, 0x100);
    }
    c.set_byte(0x11cc, ax as u8);
    c.set_reg(AX, (ax >> 8) as u8 as i8 as i16 as u16);
    c.or(W, q2, q2);
    if neg2 {
        let a = c.reg(AX);
        c.neg(W, a);
        c.set_reg(AX, v);
    }
    let pb = c.pop();
    let nb = c.add(W, pb, v);
    c.set_reg(BX, nb);
    let pa = c.pop();
    let d = c.reg(DX);
    let mut nd = c.add(W, d, pa);
    c.set_reg(AX, pa);
    let t = c.add(W, nb, 0x60);
    c.set_reg(AX, t);
    c.cmp(W, t, 0xc0);
    if pole {
        let h = c.byte(0x11c7);
        let h = c.add(B, h as u16, 0x80);
        c.set_byte(0x11c7, h as u8);
        nd = c.add(W, nd, 0x8000);
    }
    c.set_reg(DX, nd);
    c.ret()
}

/// `sub_5a56`: into the map view. Already in it ([46EBh] bit 7): its
/// redraw (`map_view_redraw`). Else the screen set up (`loc_0d2bd`,
/// `loc_04aca`, `loc_0b930`, palette 6B34h, 1482h to 46E3h, `loc_05b69`,
/// the portrait), [46EBh] 80h, the music, the redraw installed as the
/// frame handler [46EDh] and run, `loc_0d792` and the buttons 1CCAh.
pub fn enter_map_view(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5a56)?;
    let mut c = Ctx::new(m);
    let v = c.byte(0x46eb);
    if v & 0x80 != 0 {
        return map_view_redraw(&mut c, start, 2, |c| c.cmp(B, v as u16, 0));
    }
    c.run(start, 3)?;
    c.cmp(B, v as u16, 0);
    c.call(0xd2bd, 0x5a60)?;
    c.run(0x5a60, 1)?;
    c.call(0x4aca, 0x5a63)?;
    c.run(0x5a63, 1)?;
    c.call(0xb930, 0x5a66)?;
    c.run(0x5a66, 3)?;
    c.set_reg(SI, 0x6b34);
    c.set_reg(BP, 0x0f);
    c.call(0xda25, 0x5a6f)?;
    c.run(0x5a6f, 3)?;
    c.set_reg(SI, 0x1482);
    c.set_reg(DI, 0x46e3);
    c.call(0x5b99, 0x5a78)?;
    c.run(0x5a78, 1)?;
    c.call(0x5b69, 0x5a7b)?;
    c.run(0x5a7b, 1)?;
    c.call(0x1797, 0x5a7e)?;
    c.run(0x5a7e, 2)?;
    c.set_byte(0x46eb, 0x80);
    c.call(0xad5e, 0x5a86)?;
    c.run(0x5a86, 4)?;
    c.set_word(0x2786, 0xc835);
    c.set_reg(AX, 0x5a9a);
    c.set_word(0x46ed, 0x5a9a);
    c.call(0x5a9a, 0x5a94)?;
    c.run(0x5a94, 1)?;
    c.call(0xd792, 0x5a97)?;
    super::screens::set_buttons(&mut c, 0x5a97, 3, |c| c.set_reg(SI, 0x1cca))
}

/// From `loc_05a9a` (after `pre` instructions from `at` with `ops`): the
/// map view redrawn with [46EBh] 80h meanwhile (`loc_05b8d`, `loc_0b6c3`,
/// the shown locations, the troop marker, [3CBEh] 0, `loc_06715`, the
/// place 46E3h, `loc_0878c`; `loc_05406` for its bit 40h), the click
/// handler 1A9Eh, [0DC58h] = 46E3h.
fn map_view_redraw(c: &mut Ctx, at: u16, pre: u32, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    c.run(at, pre + 1)?;
    ops(c);
    c.call(0xc07c, 0x5a9d)?;
    c.run(0x5a9d, 1)?;
    c.call(0x5b8d, 0x5aa0)?;
    c.run(0x5aa0, 4)?;
    let old = c.byte(0x46eb);
    c.set_byte(0x46eb, 0x80);
    let ax = c.reg(AX) & 0xff00 | old as u16;
    c.set_reg(AX, ax);
    c.push(ax);
    c.call(0xb6c3, 0x5aaa)?;
    for (to, ret) in [(0xc13bu16, 0x5aadu16), (0x5dce, 0x5ab0), (0x6314, 0x5ab3), (0xc412, 0x5ab6)] {
        c.run(ret - 3, 1)?;
        c.call(to, ret)?;
    }
    c.run(0x5ab6, 2)?;
    c.set_word(0x3cbe, 0);
    c.call(0x6715, 0x5abf)?;
    c.run(0x5abf, 2)?;
    c.set_reg(SI, 0x46e3);
    c.call(0xc6ad, 0x5ac5)?;
    c.run(0x5ac5, 1)?;
    c.call(0x878c, 0x5ac8)?;
    let ax = c.word_in(pc::cpu::SS, c.reg(pc::cpu::SP));
    let bit = ax & 0x40 != 0;
    c.run(0x5ac8, if bit { 5 } else { 6 })?;
    let ax = c.pop();
    c.set_reg(AX, ax);
    c.set_byte(0x46eb, ax as u8);
    let a = c.and(B, ax & 0xff, 0x40);
    c.set_reg(AX, ax & 0xff00 | a);
    if bit {
        c.call(0x5406, 0x5ad3)?;
        c.run(0x5ad3, 2)?;
    }
    c.set_reg(AX, 0x1a9e);
    c.call(0xd95e, 0x5ad9)?;
    c.run(0x5ad9, 4)?;
    c.set_reg(SI, 0x46e3);
    c.set_word(0xdc58, 0x46e3);
    c.ret()
}

/// `loc_08acc`: with bit 10h of [47DEh], the count AL for the phrase BP
/// noted by [`note_47e0`]: CFh up to 8; 55h and 61h less 1, up to 4;
/// 44h in mode 1 less 1, up to 8. BX kept.
pub fn note_phrase_count(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8acc)?;
    let mut c = Ctx::new(m);
    let saved = c.m.cpu.flags();
    let (n, _) = phrase_count_walk(&mut c, false);
    c.m.cpu.set_flags(saved);
    c.run(start, n)?;
    let (_, call) = phrase_count_walk(&mut c, true);
    if let Some(call) = call {
        if call {
            c.call(0x8ac3, 0x8ac1)?;
            c.run(0x8ac1, 2)?;
        }
        let b = c.pop();
        c.set_reg(BX, b);
    }
    c.ret()
}

/// The code of `loc_08acc` up to its call or its end (with `write`, its
/// registers and push too).
fn phrase_count_walk(c: &mut Ctx, write: bool) -> (u32, Option<bool>) {
    let t = c.byte_in(SS, 0x47de);
    c.test(B, t as u16, 0x10);
    if t & 0x10 == 0 {
        return (3, None);
    }
    let bx = c.reg(BX);
    if write {
        c.push(bx);
    }
    let bp = c.reg(BP);
    let mut ax = c.reg(AX);
    let mut bh = (bx >> 8) as u8;
    let mut n = 4u32;
    let mut lim = None;
    for (k, l) in [(0xcfu16, 8u8), (0x55, 4), (0x61, 4), (0x44, 8)] {
        n += 2;
        c.cmp(W, bp, k);
        if bp == k {
            lim = Some((k, l));
            break;
        }
    }
    let call = match lim {
        None => false,
        Some((k, l)) => {
            let mut go = true;
            if k == 0x44 {
                n += 4;
                let v = c.and(B, t as u16, 0x0f);
                bh = v as u8;
                c.cmp(B, v, 1);
                go = v == 1;
            }
            if go {
                if k != 0xcf {
                    n += 1;
                    ax = c.dec(W, ax);
                }
                n += 2;
                c.cmp(B, ax & 0xff, l as u16);
                if (ax as u8) > l {
                    n += 2;
                    ax = ax & 0xff00 | l as u16;
                }
            }
            go
        }
    };
    if write {
        c.set_reg(AX, ax);
        c.set_reg(BX, (bh as u16) << 8 | bx & 0xff);
    }
    (n + if call { 1 } else { 2 }, Some(call))
}

/// `ui_main_view_map_interface`: the map view's redraw (the frame
/// handler [`enter_map_view`] installs).
pub fn ui_main_view_map_interface(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5a9a)?;
    let mut c = Ctx::new(m);
    map_view_redraw(&mut c, start, 0, |_| {})
}

/// `loc_08a3b`: with bit 10h of [47DEh], the phrase AX checked against
/// the room [477Fh]: by its range for the room (8Bh: from 108h; 84h:
/// 48h, 49h..4Bh, 18h..23h above it; 83h / 8Ch: E8h..EEh; 86h..88h with
/// [47DEh] mode 1: D1h..D7h; 85h, mode 1: 74h as 1, else 0; 89h:
/// DAh..E1h) rebased and noted by [`note_47e0`]. BX kept.
pub fn check_room_phrase(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x8a3b)?;
    let mut c = Ctx::new(m);
    let saved = c.m.cpu.flags();
    let (n, _) = room_phrase_walk(&mut c, false);
    c.m.cpu.set_flags(saved);
    c.run(start, n)?;
    let (_, call) = room_phrase_walk(&mut c, true);
    match call {
        None => c.ret(),
        Some(false) => {
            let b = c.pop();
            c.set_reg(BX, b);
            c.ret()
        }
        Some(true) => {
            c.call(0x8ac3, 0x8ac1)?;
            c.run(0x8ac1, 2)?;
            let b = c.pop();
            c.set_reg(BX, b);
            c.ret()
        }
    }
}

/// The code of `loc_08a3b` up to its call (with `write`, its registers
/// and push too): its count, and whether it pushed and calls.
fn room_phrase_walk(c: &mut Ctx, write: bool) -> (u32, Option<bool>) {
    let t = c.byte(0x47de);
    c.test(B, t as u16, 0x10);
    if t & 0x10 == 0 {
        return (3, None);
    }
    let bx0 = c.reg(BX);
    if write {
        c.push(bx0);
    }
    let bh = c.byte(0x477f);
    let mut bl = bx0 as u8;
    let mut ax = c.reg(AX);
    // test, jne, push, mov bh; cmp bh, 8Bh, jne.
    let mut n = 6u32;
    let mode = |c: &mut Ctx, bl: &mut u8| {
        let m = c.byte(0x47de);
        let v = c.and(B, m as u16, 0x0f);
        *bl = v as u8;
        c.cmp(B, v, 1);
        v == 1
    };
    // `cmp al, k` / `cmp ax, k` then `jae` over the call.
    let below = |c: &mut Ctx, ax: u16, wide: bool, k: u16| {
        if wide {
            c.cmp(W, ax, k);
            ax < k
        } else {
            c.cmp(B, ax & 0xff, k);
            ((ax & 0xff) as u16) < k
        }
    };
    c.cmp(B, bh as u16, 0x8b);
    let call = if bh == 0x8b {
        n += 2;
        ax = c.sub(W, ax, 0x108);
        true
    } else {
        n += 2;
        c.cmp(B, bh as u16, 0x84);
        if bh == 0x84 {
            n += 2;
            ax = c.sub(W, ax, 0x48);
            if ax == 0 {
                true
            } else {
                n += 3;
                ax = c.dec(W, ax);
                c.cmp(B, ax & 0xff, 3);
                if (ax as u8) < 3 {
                    true
                } else {
                    n += 4;
                    ax = c.sub(W, ax, 0xffcf);
                    below(c, ax, true, 0xc)
                }
            }
        } else {
            n += 2;
            c.cmp(B, bh as u16, 0x83);
            let hit = if bh == 0x83 {
                n += 1;
                true
            } else {
                n += 2;
                c.cmp(B, bh as u16, 0x8c);
                bh == 0x8c
            };
            if hit {
                n += 4;
                ax = c.sub(W, ax, 0xe8);
                below(c, ax, false, 7)
            } else {
                n += 4;
                bl = c.sub(B, bh as u16, 0x86) as u8;
                c.cmp(B, bl as u16, 3);
                if bl < 3 {
                    n += 4;
                    if !mode(c, &mut bl) {
                        false
                    } else {
                        n += 4;
                        ax = c.sub(W, ax, 0xd1);
                        below(c, ax, false, 7)
                    }
                } else {
                    n += 2;
                    c.cmp(B, bh as u16, 0x85);
                    if bh == 0x85 {
                        n += 4;
                        if !mode(c, &mut bl) {
                            false
                        } else {
                            n += 3;
                            c.cmp(W, ax, 0x74);
                            let eq = ax == 0x74;
                            ax &= 0xff00;
                            if eq {
                                n += 2;
                                ax = c.inc(W, ax);
                            }
                            true
                        }
                    } else {
                        n += 2;
                        c.cmp(B, bh as u16, 0x89);
                        if bh != 0x89 {
                            false
                        } else {
                            n += 3;
                            ax = c.sub(W, ax, 0xda);
                            below(c, ax, false, 8)
                        }
                    }
                }
            }
        }
    };
    if write {
        c.set_reg(AX, ax);
        c.set_reg(BX, (bh as u16) << 8 | bl as u16);
    }
    (n + if call { 1 } else { 2 }, Some(call))
}

/// `loc_05f25`: the caption box for a text CX wide around (DX, BL):
/// at SI (left = DX + 0Fh, less 82h from D2h; top = BL - CX / 2 kept in
/// 4..94h - CX; right + 6Ah; bottom + CX), [0DBE0h] = SI; its frame
/// corner (DX, BL) - 0Ah at 0D816h; then drawn in colour 6
/// ([`redraw_panel_box`]).
pub fn caption_box(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5f25)?;
    let mut c = Ctx::new(m);
    let (bx0, dx0, cx, si) = (c.reg(BX) & 0xff, c.reg(DX), c.reg(CX), c.reg(SI));
    let half = cx >> 1;
    let b1 = bx0.wrapping_sub(half);
    let lo = (b1 as i16) < 4;
    let b2 = if lo { 4 } else { b1 };
    let lim = 0x94u16.wrapping_sub(cx);
    let hi = !((b2 as i16) < (lim as i16));
    let b3 = if hi { lim } else { b2 };
    let d1 = dx0.wrapping_add(0x0f);
    let wrap = !((d1 as i16) < 0xd2);
    c.run(start, 8 + lo as u32 + 4 + hi as u32 + 3 + wrap as u32 + 17)?;
    c.set_reg(BX, bx0);
    c.xor(B, 0, 0);
    c.push(bx0);
    c.push(dx0);
    let a = c.shr1(W, cx, 1);
    c.set_reg(AX, a);
    let b = c.sub(W, bx0, a);
    c.cmp(W, b, 4);
    let a = c.sub(W, 0x94, cx);
    c.set_reg(AX, a);
    c.cmp(W, b2, a);
    let mut d = c.add(W, dx0, 0x0f);
    c.cmp(W, d, 0xd2);
    if wrap {
        d = c.sub(W, d, 0x82);
    }
    let _ = b;
    c.set_word(si, d);
    c.set_word(si.wrapping_add(2), b3);
    let d2 = c.add(W, d, 0x6a);
    c.set_word(si.wrapping_add(4), d2);
    let b4 = c.add(W, b3, cx);
    c.set_word(si.wrapping_add(6), b4);
    c.set_word(0xdbe0, si);
    let dx = c.pop();
    let bx = c.pop();
    c.set_reg(AX, 0x0a);
    let dx = c.sub(W, dx, 0x0a);
    let bx = c.sub(W, bx, 0x0a);
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.set_reg(DI, 0xd816);
    c.set_word(0xd816, dx);
    c.set_word(0xd818, bx);
    c.set_reg8(r8::AL, 6);
    super::gfx::redraw_panel_box(c.m, 0x7b0f)
}

/// `loc_05d6d`: [46ECh] 0; outside the map views ([46EBh] 0) nothing;
/// in one (positive) `loc_0dbb2` and its frame handler [46EDh] (bails
/// unless that is [`ui_main_view_map_interface`]); in the map view proper
/// (bit 7) its redraw with [46EBh] 80h meanwhile (`loc_05b8d`,
/// `loc_0b6c3`, the shown locations, the troop marker, [3CBEh] 0,
/// `loc_06715`, the place 46E3h, the troop [46EFh]'s location marked,
/// `loc_01c18`; `loc_0542f` for bit 40h), the map resource opened last.
pub fn refresh_map_view(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5d6d)?;
    let mut c = Ctx::new(m);
    let v = c.byte(0x46eb);
    if v == 0 {
        c.run(start, 5)?;
        c.set_byte(0x46ec, 0);
        c.cmp(B, 0, 0);
        return c.ret();
    }
    if v & 0x80 == 0 {
        if c.word(0x46ed) != 0x5a9a {
            return Err(Bail);
        }
        c.run(start, 5)?;
        c.set_byte(0x46ec, 0);
        c.cmp(B, v as u16, 0);
        c.call(0xdbb2, 0x5d7e)?;
        if c.word(0x46ed) != 0x5a9a {
            c.m.cpu.ip = 0x5d7e;
            return Err(Bail);
        }
        c.run(0x5d7e, 1)?;
        return ui_main_view_map_interface(c.m, 0x5a9a);
    }
    c.run(start, 4)?;
    c.set_byte(0x46ec, 0);
    c.cmp(B, v as u16, 0);
    c.call(0xc07c, 0x5d85)?;
    c.run(0x5d85, 1)?;
    c.call(0xdbb2, 0x5d88)?;
    c.run(0x5d88, 1)?;
    c.call(0x5b8d, 0x5d8b)?;
    c.run(0x5d8b, 5)?;
    let old = c.byte(0x46eb);
    c.set_byte(0x46eb, 0x80);
    let ax = c.reg(AX) & 0xff00 | old as u16;
    c.set_reg(AX, ax);
    c.push(ax);
    let t = c.word(0x46ef);
    c.push(t);
    c.call(0xb6c3, 0x5d99)?;
    for (to, ret) in [(0xc13bu16, 0x5d9cu16), (0x5dce, 0x5d9f), (0x6314, 0x5da2), (0xc412, 0x5da5)] {
        c.run(ret - 3, 1)?;
        c.call(to, ret)?;
    }
    c.run(0x5da5, 2)?;
    c.set_word(0x3cbe, 0);
    c.call(0x6715, 0x5dae)?;
    c.run(0x5dae, 2)?;
    c.set_reg(SI, 0x46e3);
    c.call(0xc6ad, 0x5db4)?;
    let si = c.word_in(SS, c.reg(SP));
    c.run(0x5db4, 3 + if si != 0 { 1 } else { 1 })?;
    let si = c.pop();
    c.set_reg(SI, si);
    c.or(W, si, si);
    if si != 0 {
        c.call(0x697c, 0x5dbc)?;
        c.run(0x5dbc, 1)?;
    }
    c.call(0x1c18, 0x5dbf)?;
    let ax = c.word_in(SS, c.reg(SP));
    let bit = ax & 0x40 != 0;
    c.run(0x5dbf, if bit { 5 } else { 5 })?;
    let ax = c.pop();
    c.set_byte(0x46eb, ax as u8);
    let a = c.and(B, ax & 0xff, 0x40);
    c.set_reg(AX, ax & 0xff00 | a);
    if bit {
        c.call(0x542f, 0x5dca)?;
        c.run(0x5dca, 1)?;
    }
    super::resources::open_onmap_resource(c.m, 0xc13b)
}

/// `loc_0813e`: the route lines of the troop SI on the map: its points
/// (`add_point_entry` into a list on the stack, after `shown_troop`):
/// its own place (+6, +8), then its target location's (record +4), or
/// for the troop 8E0h every location of the list 4718h; each pair joined
/// by a line in colour 0Ch (far [3901h], clipped to 46E3h); across the
/// map's seam (x apart by 50h or more, against the move's sign) drawn
/// from the side's end, 190h past it. Bails unless SS = DS.
pub fn route_lines(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x813e)?;
    let mut c = Ctx::new(m);
    if c.sreg(SS) != c.sreg(DS) {
        return Err(Bail);
    }
    c.run(start, 3)?;
    let sp = c.reg(SP);
    let sp = c.sub(W, sp, 0x30);
    c.set_reg(SP, sp);
    c.set_reg(DI, sp);
    c.call(0x68eb, 0x8146)?;
    c.run(0x8146, 3)?;
    let si = c.reg(SI);
    let (dx, bx) = (c.word(si.wrapping_add(6)), c.word(si.wrapping_add(8)));
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.call(0x81d7, 0x814f)?;
    let si = c.reg(SI);
    if si == 0x8e0 {
        c.run(0x814f, 3)?;
        c.cmp(W, si, 0x8e0);
        c.set_reg(SI, 0x4718);
        let mut at = 0x8158;
        loop {
            let si = c.reg(SI);
            let p = c.word(si);
            if p == 0 {
                c.run(at, 3)?;
                c.lods(W, DS);
                c.or(W, 0, 0);
                break;
            }
            c.run(at, 7)?;
            c.lods(W, DS);
            c.or(W, p, p);
            let (dx, bx) = (c.word(p.wrapping_add(2)), c.word(p.wrapping_add(4)));
            c.set_reg(DX, dx);
            c.set_reg(BX, bx);
            c.call(0x81d7, 0x8168)?;
            at = 0x8168;
            c.run(at, 1)?;
            at = 0x8158;
        }
    } else {
        c.run(0x814f, 6)?;
        c.cmp(W, si, 0x8e0);
        let p = c.word(si.wrapping_add(4));
        let (dx, bx) = (c.word(p.wrapping_add(2)), c.word(p.wrapping_add(4)));
        c.set_reg(DX, dx);
        c.set_reg(BX, bx);
        c.call(0x81d7, 0x8176)?;
    }
    c.run(0x8176, 1)?;
    let sp = c.reg(SP);
    c.set_reg(SI, sp);
    loop {
        let si = c.reg(SI);
        let next = c.word(si.wrapping_add(6));
        if next == 0x8000 {
            c.run(0x8178, 10)?;
            for r in [DX, BX, AX] {
                let v = c.lods(W, DS);
                if r != AX {
                    c.set_reg(r, v);
                }
            }
            c.set_reg(DI, next);
            c.cmp(W, next, 0x8000);
            let sp = c.reg(SP);
            let sp = c.add(W, sp, 0x30);
            c.set_reg(SP, sp);
            return c.ret();
        }
        // The pair: from (x, y) (first seen as x0) to (di, cx) (w0).
        let (x, y, x0) = (c.word(si), c.word(si.wrapping_add(2)), c.word(si.wrapping_add(4)));
        let s1 = si.wrapping_add(6);
        let (cy, w0) = (c.word(s1.wrapping_add(2)), c.word(s1.wrapping_add(4)));
        let dax = x0.wrapping_sub(w0);
        let dsi = x.wrapping_sub(next);
        let same = (dax ^ dsi) & 0x8000 == 0;
        let mut n = 8 + 7;
        let (left, right) = (c.word(0x46e3), c.word(0x46e7));
        let mut seam = None;
        let mut chk = None;
        if !same {
            let neg = dsi & 0x8000 != 0;
            let a = if neg { dsi.wrapping_neg() } else { dsi };
            n += 5 + neg as u32;
            chk = Some((a, neg));
            if a >= 0x50 {
                n += 2;
                let inside = (x as i16) >= (left as i16) && {
                    n += 2;
                    (x as i16) < (right as i16)
                };
                if inside {
                    n += 3;
                }
                let s = if inside { dsi.wrapping_neg() } else { dsi };
                n += 4 + (s & 0x8000 == 0) as u32;
                seam = Some((inside, s));
            }
        }
        n += 4;
        c.run(0x8178, n)?;
        c.lods(W, DS);
        c.set_reg(DX, x);
        c.lods(W, DS);
        c.set_reg(BX, y);
        c.lods(W, DS);
        c.set_reg(DI, next);
        c.cmp(W, next, 0x8000);
        c.push(s1);
        c.set_reg(CX, cy);
        let ax = c.sub(W, x0, w0);
        let si2 = c.sub(W, x, next);
        let ax = c.xor(W, ax, si2);
        c.set_reg(AX, ax);
        c.set_reg(SI, si2);
        if let Some((a, neg)) = chk {
            c.set_reg(AX, si2);
            c.or(W, si2, si2);
            if neg {
                c.neg(W, si2);
            }
            c.set_reg(AX, a);
            c.cmp(W, a, 0x50);
        }
        if let Some((inside, s)) = seam {
            c.cmp(W, x, left);
            if (x as i16) >= (left as i16) {
                c.cmp(W, x, right);
            }
            let (mut dx, mut di, mut cx, mut bx) = (x, next, cy, y);
            if inside {
                std::mem::swap(&mut dx, &mut di);
                std::mem::swap(&mut cx, &mut bx);
                c.neg(W, si2);
            }
            c.set_reg(SI, s);
            let mut ax = 0x190u16;
            c.or(W, s, s);
            if s & 0x8000 == 0 {
                ax = c.neg(W, ax);
            }
            let dx = c.add(W, dx, ax);
            c.set_reg(AX, ax);
            c.set_reg(DX, dx);
            c.set_reg(DI, di);
            c.set_reg(CX, cx);
            c.set_reg(BX, bx);
        }
        let es = c.word(0xdbda);
        c.set_sreg(ES, es);
        c.set_reg8(r8::AL, 0x0c);
        c.set_reg(BP, 0x5555);
        c.set_reg(SI, 0x46e3);
        c.interp(0x81cc, 0x81d0)?;
        c.run(0x81d0, 2)?;
        let s = c.pop();
        c.set_reg(SI, s);
    }
}

/// `loc_04586`: the message line's subject: in view (`in_view`) and near
/// a shown location (`nearest_shown_location` 9 or more) the direction
/// of the map pointer [4749h] (`direction_256`, by eighths: FFF0h + k,
/// FFFFh off the eighth's middle or with no pointer), else 0; the line
/// redrawn ([`redraw_message_line`]) when that changes [46FCh].
pub fn message_subject(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x4586)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x5d1d, 0x4589)?;
    if !c.flag(CF) {
        return subject_tail(&mut c, 0x4589, 2, 0, |c| c.set_reg(DI, 0));
    }
    c.run(0x4589, 4)?;
    c.set_reg(DI, 0);
    c.set_reg8(r8::AL, 0xff);
    c.call(0x5e6d, 0x4593)?;
    let ax = c.reg(AX);
    if ax < 9 {
        return subject_tail(&mut c, 0x4593, 2, 0, |c| c.cmp(W, ax, 9));
    }
    let p = c.word(0x4749);
    if p == 0 {
        return subject_tail(&mut c, 0x4593, 6, 0xffff, |c| {
            c.cmp(W, ax, 9);
            c.set_reg(DI, 0xffff);
            c.set_reg(DX, 0);
            c.or(W, 0, 0);
        });
    }
    c.run(0x4593, 12)?;
    c.cmp(W, ax, 9);
    c.set_reg(DI, 0xffff);
    let d = c.add(W, p, 0x0b);
    let x = c.word(0xdc36);
    let d = c.sub(W, d, x);
    let d = c.neg(W, d);
    c.set_reg(DX, d);
    let (y, t) = (c.word(0xdc38), c.word(0x474f));
    let b = c.sub(W, y, t);
    c.set_reg(BX, b);
    c.call(0x514e, 0x45b7)?;
    let al = c.reg8(r8::AL).wrapping_add(3);
    if al & 0x1f >= 6 {
        return subject_tail(&mut c, 0x45b7, 6, 0xffff, |c| {
            let a = c.add(B, (al.wrapping_sub(3)) as u16, 3);
            let h = c.and(B, a, 0x1f);
            c.set_reg(AX, h << 8 | a);
            c.cmp(B, h, 6);
            c.set_reg(DI, 0xffff);
        });
    }
    let k = (al.rotate_left(3) & 7) as u16 | 0xfff0;
    subject_tail(&mut c, 0x45b7, 12, k, |c| {
        let a = c.add(B, (al.wrapping_sub(3)) as u16, 3);
        let h = c.and(B, a, 0x1f);
        c.set_reg(AX, h << 8 | a);
        c.cmp(B, h, 6);
        c.set_reg(DI, 0xffff);
        let mut v = a as u8;
        for _ in 0..3 {
            v = c.m.cpu.shift(0, B, v as u32, 1) as u8;
        }
        let v = c.and(B, v as u16, 7);
        let v = c.or(W, h << 8 | v, 0xfff0);
        c.set_reg(AX, v);
        c.set_reg(DI, v);
    })
}

/// From 45D3h (after `pre` instructions from `at` with `ops`, DI then
/// `di`): [46FCh] = DI; on a change the line redrawn.
fn subject_tail(c: &mut Ctx, at: u16, pre: u32, di: u16, ops: impl FnOnce(&mut Ctx)) -> Result<(), Bail> {
    let old = c.word(0x46fc);
    c.run(at, pre + 4 + if old == di { 1 } else { 0 })?;
    ops(c);
    c.set_reg(AX, di);
    c.set_word(0x46fc, di);
    c.set_reg(AX, old);
    c.cmp(W, old, di);
    if old == di {
        return c.ret();
    }
    super::story::redraw_message_line(c.m, 0x45de)
}

/// `loc_05ee4`: the caption of the map marker for the location DI
/// ([`find_marker`]): its kind (`location_map_kind`) noted in [46F7h]
/// (the same location and kind: the box just refilled,
/// [`fill_and_outline`]; else the old marks dropped first), and for a
/// marker shown (bit 7 of its y high byte) the box sized for the kind's
/// text width (11D0h table) at it ([`caption_box`]).
pub fn marker_caption(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5ee4)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x5ed0, 0x5ee7)?;
    if !c.flag(ZF) {
        c.run(0x5ee7, 2)?;
        return c.ret();
    }
    c.run(0x5ee7, 3)?;
    c.set_reg(SI, 0x1668);
    c.call(0x6252, 0x5eef)?;
    let ax = c.reg(AX);
    let (di, cur) = (c.reg(DI), c.word(0x46f8));
    let al = (ax as u8).wrapping_add(1);
    let kind = c.byte(0x46f7);
    let ops = |c: &mut Ctx| {
        c.set_reg(BX, ax);
        let a = c.inc(W, ax);
        c.set_reg(AX, a);
        c.cmp(W, di, cur);
    };
    if di == cur {
        if al == kind {
            c.run(0x5eef, 7)?;
            ops(&mut c);
            c.cmp(B, al as u16, kind as u16);
            return super::gfx::fill_and_outline(c.m, 0x7b1b);
        }
        c.run(0x5eef, 7)?;
        ops(&mut c);
        c.cmp(B, al as u16, kind as u16);
        c.call(0xe270, 0x5f04)?;
        c.run(0x5f04, 2)?;
        c.set_byte(0x46d8, 1);
        c.call(0x5f91, 0x5f0c)?;
        c.run(0x5f0c, 1)?;
        c.call(0xc08e, 0x5f0f)?;
        c.run(0x5f0f, 1)?;
        c.call(0xe283, 0x5f12)?;
    } else {
        c.run(0x5eef, 4)?;
        ops(&mut c);
    }
    let al = c.reg8(r8::AL);
    let bx = c.reg(BX);
    let cl = c.byte(bx.wrapping_add(0x11d0));
    let bp = c.reg(BP);
    let (dx, by) = (c.word_in(SS, bp.wrapping_add(2)), c.word_in(SS, bp.wrapping_add(4)));
    let shown = by & 0x8000 != 0;
    c.run(0x5f12, 7 + if shown { 0 } else { 1 })?;
    c.set_byte(0x46f7, al);
    c.set_reg(CX, cl as u16);
    c.xor(B, 0, 0);
    c.set_reg(DX, dx);
    c.set_reg(BX, by);
    c.or(B, by >> 8, by >> 8);
    if !shown {
        return c.ret();
    }
    caption_box(c.m, 0x5f25)
}

/// `loc_060ac`: the location's panel line at (DX = [1668h] + 4, BX + 0Ch)
/// in colour 9Ah: a location to show (`location_to_show`) its string 6Eh
/// and the skill bars of its record (DI + 14h, [166Eh]); else the string
/// 4Ch and the balance gauge sprite (8Eh + (balance + 0Fh) / 32).
pub fn location_panel_line(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x60ac)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0xc13b, 0x60af)?;
    c.run(0x60af, 1)?;
    c.call(0xd068, 0x60b2)?;
    c.run(0x60b2, 5)?;
    c.set_reg8(r8::CL, 0x9a);
    let d = c.word(0x1668);
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 0x0c);
    c.set_reg(BX, bx);
    let d = c.add(W, d, 4);
    c.set_reg(DX, d);
    c.call(0x627e, 0x60c1)?;
    if !c.flag(CF) {
        c.run(0x60c1, 3)?;
        c.set_reg(AX, 0x6e);
        c.call(0xd194, 0x60c9)?;
        c.run(0x60c9, 4)?;
        let bx = c.reg(BX);
        let bx = c.add(W, bx, 0x0a);
        c.set_reg(BX, bx);
        let di = c.reg(DI);
        c.set_reg(SI, di.wrapping_add(0x14));
        let bp = c.word(0x166e);
        c.set_reg(BP, bp);
        return skill_bars(c.m, 0x7e3d);
    }
    c.run(0x60c1, 3)?;
    c.set_reg(AX, 0x4c);
    c.call(0xd194, 0x60dc)?;
    c.run(0x60dc, 5)?;
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 6);
    c.set_reg(BX, bx);
    let dx = c.reg(DX);
    let dx = c.add(W, dx, 0x2f);
    c.set_reg(DX, dx);
    c.push(bx);
    c.push(dx);
    c.call(0x60f8, 0x60e7)?;
    c.run(0x60e7, 8)?;
    let al = c.reg8(r8::AL) as u16;
    c.xor(B, 0, 0);
    let a = c.add(W, al, 0x0f);
    c.set_reg8(r8::CL, 5);
    let a = c.m.cpu.shift(5, W, a as u32, 5) as u16;
    let a = c.add(W, a, 0x8e);
    c.set_reg(AX, a);
    let d = c.pop();
    c.set_reg(DX, d);
    let b = c.pop();
    c.set_reg(BX, b);
    super::draw::draw_sprite_clobbering_bx_dx(c.m, 0xc22f)
}

/// `loc_0541f`: the zoomed map view opened on the point [197Ch]: the
/// point kept in [1980h], the message subject and [479Eh] cleared; the
/// view's frame (46E3h) and its inner box (4710h) placed around the map
/// position ([`get_point_4710`]), the frame sprite 8Dh, the zoomed globe
/// drawn into the back buffer from resource 3Ah ([46EBh] 40h), the map
/// layers, a rect FBh around the point's screen place (40h by 28h,
/// clamped to the box; none when flat); the place [0DBE0h] / [0DBE2h]
/// noted, the previous box [4720h] redrawn, as the second place the
/// troop marker and its route lines, else the marker at DX, BX,
/// the screen updated, [46EBh] C0h, the clip rect, `loc_05b8d`; then
/// the mouse rect 4710h.
pub fn open_place_view(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x541f)?;
    let mut c = Ctx::new(m);
    c.run(start, 7)?;
    let (p0, p1) = (c.word(0x197c), c.word(0x197e));
    c.set_word(0x1982, p1);
    c.set_word(0x1980, p0);
    c.set_word(0x46fc, 0);
    c.set_word(0x479e, 0);
    c.call(0x557b, 0x543e)?;
    c.run(0x543e, 15)?;
    c.set_reg(DI, 0x46e3);
    let dx = c.reg(DX);
    let dx = c.add(W, dx, 5);
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 7);
    c.set_word(0x46e3, dx);
    c.set_word(0x46e5, bx);
    let dx = c.add(W, dx, 0xa0);
    c.set_word(0x46e7, dx);
    let bx = c.add(W, bx, 0x59);
    c.set_word(0x46e9, bx);
    c.set_reg(SI, 0x4710);
    let dx = c.add(W, dx, 5);
    c.set_word(0x4714, dx);
    let bx = c.add(W, bx, 0x0c);
    c.set_word(0x4716, bx);
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.call(0xc13b, 0x546b)?;
    c.run(0x546b, 1)?;
    c.call(0x557b, 0x546e)?;
    c.run(0x546e, 2)?;
    c.set_reg(AX, 0x8d);
    c.call(0xc22f, 0x5474)?;
    c.run(0x5474, 1)?;
    c.call(0xc07c, 0x5477)?;
    c.run(0x5477, 1)?;
    c.call(0x5b8d, 0x547a)?;
    c.run(0x547a, 3)?;
    let v = c.word(0xdd00);
    c.push(v);
    c.set_reg(AX, 0x3a);
    c.call(0xc13e, 0x5484)?;
    c.run(0x5484, 1)?;
    c.call(0x5584, 0x5487)?;
    c.run(0x5487, 4)?;
    let v = c.word(0xdbb2);
    c.set_word(0xdd00, v);
    c.set_byte(0x46eb, 0x40);
    c.call(0xb69a, 0x5497)?;
    c.run(0x5497, 1)?;
    c.call(0xb6c3, 0x549a)?;
    c.run(0x549a, 2)?;
    let v = c.pop();
    c.set_word(0xdd00, v);
    c.call(0x58e4, 0x54a1)?;
    for (at, to) in [(0x54a1u16, 0xc137u16), (0x54a4, 0x5dce), (0x54a7, 0x5605), (0x54aa, 0x563e)] {
        c.run(at, 1)?;
        c.call(to, at + 3)?;
    }
    c.run(0x54ad, 4)?;
    c.set_word(0x2772, 0x5555);
    let (dx, bx) = (c.word(0x1980), c.word(0x1982));
    c.set_reg(DX, dx);
    c.set_reg(BX, bx);
    c.call(0xb647, 0x54be)?;
    // The rect around the point, clamped to the box (counted first, its
    // registers and flags undone).
    let regs = c.m.cpu.regs;
    let flags = c.m.cpu.flags();
    let (n, _) = point_rect_walk(&mut c);
    c.m.cpu.regs = regs;
    c.m.cpu.set_flags(flags);
    c.run(0x54be, n)?;
    let (_, draw) = point_rect_walk(&mut c);
    if draw {
        c.call(0xc560, 0x551d)?;
    }
    let si = 0x4710u16;
    let a = c.word(0xdbe0);
    let second = a != 0 && a != si;
    let prev = c.word(0x4720);
    c.run(0x551d, 5 + if a == 0 { 0 } else if a == si { 2 } else { 3 } + 7 + if prev != 0 { 3 } else { 0 })?;
    c.set_word(0x2772, 0xffff);
    c.set_reg(SI, si);
    c.set_reg(DI, 0xdbe0);
    c.cmp(W, a, 0);
    if a != 0 {
        c.cmp(W, a, si);
    }
    let di = if second { 0xdbe2 } else { 0xdbe0 };
    c.set_reg(DI, di);
    c.set_word(di, si);
    c.cmp(W, di, 0xdbe2);
    let f = c.m.cpu.flags();
    c.push(f);
    c.xor(W, 0, 0);
    c.set_word(0x4720, 0);
    c.set_reg(DI, prev);
    c.or(W, prev, prev);
    if prev != 0 {
        c.set_reg(SI, prev);
        c.set_reg(DI, si);
        c.set_reg8(r8::AL, 6);
        c.call(0xc0e8, 0x554d)?;
    }
    let f = c.word_in(SS, c.reg(SP));
    let zf = f & 0x40 != 0;
    c.run(0x554d, 3)?;
    let f = c.pop();
    c.m.cpu.set_flags(f);
    if zf {
        c.call(0x62f2, 0x5553)?;
        c.run(0x5553, 1)?;
        c.call(0x813e, 0x5556)?;
        c.run(0x5556, 1)?;
    } else {
        c.call(0x6314, 0x555b)?;
    }
    c.run(0x555b, 2)?;
    c.set_reg(SI, 0x4710);
    c.call(0xc4f0, 0x5561)?;
    c.run(0x5561, 1)?;
    c.call(0xb69a, 0x5564)?;
    c.run(0x5564, 4)?;
    c.set_byte(0x46eb, 0xc0);
    c.set_reg(SI, 0x1482);
    c.set_reg(DI, 0x46e3);
    c.call(0x5b99, 0x5572)?;
    c.run(0x5572, 1)?;
    c.call(0x5b8d, 0x5575)?;
    mouse_rect_4710(&mut c)
}

/// From 54BEh: the point's rect (BX - 14h..+13h, DX - 28h..+27h)
/// clamped to the box [4710h] + 5..+A4h, [4712h] + 7..+5Fh; its count,
/// and whether it is drawn (`draw_rect_outline`, AL FBh).
fn point_rect_walk(c: &mut Ctx) -> (u32, bool) {
    let (dx0, bx0) = (c.reg(DX), c.reg(BX));
    let mut n = 8u32;
    let mut cx = bx0;
    let mut di = dx0;
    let mut bx = c.sub(W, bx0, 0x14);
    cx = c.add(W, cx, 0x13);
    let mut dx = c.sub(W, dx0, 0x28);
    di = c.add(W, di, 0x27);
    let x = c.word(0x4710);
    let mut ax = c.add(W, x, 5);
    // `cmp r, ax`, then `jge` (signed) or `jbe` over `mov r, ax`.
    let clamp = |c: &mut Ctx, r: &mut u16, ax: u16, low: bool, n: &mut u32| {
        c.cmp(W, *r, ax);
        *n += 2;
        let keep = if low { (*r as i16) >= (ax as i16) } else { *r <= ax };
        if !keep {
            *r = ax;
            *n += 1;
        }
    };
    clamp(c, &mut dx, ax, true, &mut n);
    clamp(c, &mut di, ax, true, &mut n);
    ax = c.add(W, ax, 0x9f);
    n += 1;
    clamp(c, &mut dx, ax, false, &mut n);
    clamp(c, &mut di, ax, false, &mut n);
    let y = c.word(0x4712);
    ax = c.add(W, y, 7);
    n += 2;
    clamp(c, &mut bx, ax, true, &mut n);
    clamp(c, &mut cx, ax, true, &mut n);
    ax = c.add(W, ax, 0x58);
    n += 1;
    clamp(c, &mut bx, ax, false, &mut n);
    clamp(c, &mut cx, ax, false, &mut n);
    c.set_reg(AX, ax);
    c.set_reg(BX, bx);
    c.set_reg(CX, cx);
    c.set_reg(DX, dx);
    c.set_reg(DI, di);
    c.cmp(W, bx, cx);
    n += 2;
    if bx == cx {
        return (n, false);
    }
    c.cmp(W, dx, di);
    n += 2;
    if dx == di {
        return (n, false);
    }
    c.set_reg8(r8::AL, 0xfb);
    (n + 2, true)
}

/// `loc_05575` (after the jump there): the mouse rect 4710h.
fn mouse_rect_4710(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x5575, 2)?;
    c.set_reg(SI, 0x4710);
    super::input::set_some_mouse_rect(c.m, 0xdaaa)
}

/// `loc_05575`: the mouse rect set to the zoomed map's box 4710h.
pub fn mouse_to_zoom_box(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5575)?;
    let mut c = Ctx::new(m);
    mouse_rect_4710(&mut c)
}

/// `sub_5b05`: the map list (`map_list_without_40`): empty (ZF) opens
/// the place view ([`open_place_view`]); else the mark at the map
/// position, `redraw_and_handle`, and in the zoomed view (bit 40h of
/// [46EBh]) the mouse rect back on its box.
pub fn map_list_or_zoom(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x5b05)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x82a0, 0x5b08)?;
    if c.flag(ZF) {
        c.run(0x5b08, 2)?;
        return open_place_view(c.m, 0x541f);
    }
    c.run(0x5b08, 2)?;
    c.call(0x5b5d, 0x5b10)?;
    c.run(0x5b10, 1)?;
    c.call(0x8850, 0x5b13)?;
    let v = c.byte(0x46eb);
    c.run(0x5b13, 3)?;
    c.test(B, v as u16, 0x40);
    if v & 0x40 != 0 {
        return mouse_rect_4710(&mut c);
    }
    c.ret()
}

/// `loc_0563e`: the zoomed map's legend under (DX + 6, BX + 62h) in the
/// colour F5FEh ([4725h] FEh): with no place picked ([4722h] 0) the
/// string 65h, a "-" and a "+" 41h apart from x + 53h, and the zoom
/// buttons (sprites 80h, 81h) at the box's corner; else the string 68h.
pub fn zoom_legend(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x563e)?;
    let mut c = Ctx::new(m);
    let k = c.byte(0x4722);
    c.run(start, 6 + if k != 0 { 2 } else { 3 })?;
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 0x62);
    c.set_reg(BX, bx);
    let dx = c.reg(DX);
    let dx = c.add(W, dx, 6);
    c.set_reg(DX, dx);
    c.set_reg(CX, 0xf5fe);
    c.set_byte(0x4725, 0xfe);
    c.cmp(B, k as u16, 0);
    if k != 0 {
        c.set_reg(AX, 0x68);
        return super::text::font_draw_string_with_color_at_pos(c.m, 0xd194);
    }
    c.push(dx);
    c.set_reg(AX, 0x65);
    c.call(0xd194, 0x5659)?;
    c.run(0x5659, 5)?;
    let a = c.pop();
    let a = c.add(W, a, 0x53);
    c.set_reg(AX, a);
    c.set_word(0xd82c, a);
    c.set_reg8(r8::AL, 0x2d);
    let to = c.word(0x2518);
    c.call(to, 0x5666)?;
    c.run(0x5666, 3)?;
    let v = c.word(0xd82c);
    let v = c.add(W, v, 0x41);
    c.set_word(0xd82c, v);
    c.set_reg8(r8::AL, 0x2b);
    let to = c.word(0x2518);
    c.call(to, 0x5671)?;
    c.run(0x5671, 1)?;
    c.call(0xc13b, 0x5674)?;
    c.run(0x5674, 1)?;
    c.call(0x557b, 0x5677)?;
    c.run(0x5677, 4)?;
    let dx = c.reg(DX);
    let dx = c.add(W, dx, 0x5f);
    c.set_reg(DX, dx);
    let bx = c.reg(BX);
    let bx = c.add(W, bx, 0x63);
    c.set_reg(BX, bx);
    c.set_reg(AX, 0x80);
    c.call(0xc2fd, 0x5683)?;
    c.run(0x5683, 3)?;
    let dx = c.reg(DX);
    let dx = c.add(W, dx, 0x3c);
    c.set_reg(DX, dx);
    c.set_reg(AX, 0x81);
    super::draw::draw_sprite_clobbering_bx_dx(c.m, 0xc22f)
}

/// `loc_0878c`: the map panel's buttons (UI list 20F2h, on the stack
/// segment): [47BAh] 0; its action button 40A7h, or A7h (enabled unless
/// [46FFh]) when the troop [8] is a plain one (below 20h, or [0Bh] below
/// 3, after `skill_needs` for the place [114Eh]); bit 40h of +0Bh and
/// +12h 0 before phase 5, else cleared and 67h; the second button 62h
/// (enabled without [3CBEh]) from [1176h] 2, else 4093h, disabled for a
/// place with a troop +9 whose `loc_06906` sets CF; then installed
/// (`loc_0d338`, BX F66h) and the map resource opened.
pub fn map_panel_buttons(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x878c)?;
    let mut c = Ctx::new(m);
    let t = c.byte(8);
    let b = c.byte(0x0b);
    let plain = t != 0xff && (t < 0x20 || b < 3);
    let mut ax = 0x40a7u16;
    if plain {
        c.run(start, if t < 0x20 { 6 } else { 8 } + 3)?;
        c.set_word(0x47ba, 0);
        c.set_reg(AX, ax);
        c.cmp(B, t as u16, 0xff);
        c.cmp(B, t as u16, 0x20);
        if t >= 0x20 {
            c.cmp(B, b as u16, 3);
        }
        let di = c.reg(DI);
        c.push(di);
        let p = c.word(0x114e);
        c.set_reg(DI, p);
        c.call(0x7f27, 0x87b2)?;
        let k = c.byte(0x46ff);
        c.run(0x87b2, 4 + if k == 0 { 1 } else { 0 })?;
        let d = c.pop();
        c.set_reg(DI, d);
        ax = 0xa7;
        c.cmp(B, k as u16, 0);
        if k == 0 {
            let h = c.or(B, 0, 0x40);
            ax = h << 8 | 0xa7;
        }
        c.set_reg(AX, ax);
    } else {
        c.run(start, if t == 0xff { 4 } else { 8 })?;
        c.set_word(0x47ba, 0);
        c.set_reg(AX, ax);
        c.cmp(B, t as u16, 0xff);
        if t != 0xff {
            c.cmp(B, t as u16, 0x20);
            c.cmp(B, b as u16, 3);
        }
    }
    let ph = c.byte(0x2a);
    let n2 = c.word(0x1176);
    let late = ph >= 5;
    let mut n = 6 + if late { 2 } else { 0 } + 2;
    let mut chk = None;
    if n2 >= 2 {
        let e = c.word(0x3cbe);
        n += 4 + if e == 0 { 1 } else { 0 };
    } else {
        let di = c.word(0x114e);
        n += 4;
        if di != 0 {
            let tr = c.byte(di.wrapping_add(9));
            n += 3;
            if tr != 0 {
                n += 1;
                chk = Some(tr);
            }
        }
    }
    c.run(0x87c0, n)?;
    c.set_reg(BP, 0x20f2);
    c.set_word_in(SS, 0x2100, ax);
    let f = c.byte_in(SS, 0x20fd);
    let f = c.or(B, f as u16, 0x40);
    c.m.cpu.write8(&mut c.m.hw, SS, 0x20fd, f as u8);
    c.set_word_in(SS, 0x2104, 0);
    c.cmp(B, ph as u16, 5);
    if late {
        let f = c.and(B, f, 0xbf);
        c.m.cpu.write8(&mut c.m.hw, SS, 0x20fd, f as u8);
        c.set_word_in(SS, 0x2104, 0x67);
    }
    c.cmp(W, n2, 2);
    if n2 >= 2 {
        let e = c.word(0x3cbe);
        let mut a = 0x62u16;
        c.cmp(W, e, 0);
        if e == 0 {
            let h = c.or(B, 0, 0x40);
            a |= h << 8;
        }
        c.set_reg(AX, a);
        c.set_word_in(SS, 0x20f8, a);
    } else {
        c.set_word_in(SS, 0x20f8, 0x4093);
        let di = c.word(0x114e);
        c.set_reg(DI, di);
        c.or(W, di, di);
        if di != 0 {
            let tr = c.byte(di.wrapping_add(9));
            c.set_reg8(r8::AL, tr);
            c.or(B, tr as u16, tr as u16);
        }
        if chk.is_some() {
            c.call(0x6906, 0x87fd)?;
            let cf = c.flag(CF);
            c.run(0x87fd, if cf { 3 } else { 1 })?;
            if cf {
                let v = c.word_in(SS, 0x20f8);
                let v = c.and(W, v, 0xbfff);
                c.set_word_in(SS, 0x20f8, v);
            }
        }
    }
    c.run(0x8816, 2)?;
    c.set_reg(BX, 0x0f66);
    c.call(0xd338, 0x881c)?;
    c.run(0x881c, 1)?;
    super::resources::open_onmap_resource(c.m, 0xc13b)
}
