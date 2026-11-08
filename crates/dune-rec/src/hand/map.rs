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
