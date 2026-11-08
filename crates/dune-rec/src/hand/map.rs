//! The map screen: its view rect ([`VIEW`]), the cursor's place on it,
//! the mouse rect, marks and rects copied around.

use super::{B, Ctx, W, entered, r8};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CF, CS, CX, DI, DS, DX, ES, SI, SP, SS},
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
    let old = c.byte(0x4725);
    if old == 0xfe {
        c.run(start, 6)?;
        c.set_reg(CX, 0xf5fe);
        c.set_reg8(r8::AL, old);
        c.cmp(B, old as u16, 0xfe);
        return c.ret();
    }
    c.run(start, 8)?;
    c.set_reg(CX, 0xf5fe);
    c.set_byte(0x4725, 0xfe);
    c.set_reg8(r8::AL, old);
    c.cmp(B, old as u16, 0xfe);
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
