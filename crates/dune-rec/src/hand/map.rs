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
