//! Sprite sheets: lists of icons, palettes from sheets, the ornithopters.

use super::pure;
use super::{B, Ctx, W, entered, globals as g, r8};
use pc::{
    Machine,
    cpu::{AX, BX, CX, DS, DX, SI, SP, SS},
    rec::Bail,
};

mod code {
    pub const OPEN_RESOURCE_BY_INDEX: u16 = 0xc13e;
    pub const GET_SUBRESOURCE_POINTER: u16 = 0xc1f4;
    pub const DRAW_SPRITE_CLOBBERING_BX_DX: u16 = 0xc22f;
    pub const DRAW_ORNI: u16 = 0x3aa9;
}

/// The sky resources: SKY is 28h, SKYDN 29h (`open_sky_or_skydn_palette`),
/// SKYRS 2Eh.
const SKY: u8 = 0x28;
const SKYRS: u8 = 0x2e;

/// `draw_icons_list_at_si` (C21B): draws the sprites listed at SI (sprite,
/// x, y words), up to FFFFh. Exit: SI past the end, AX = FFFFh; flags from
/// the last `cmp`.
pub fn draw_icons_list_at_si(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xc21b)?;
    let mut c = Ctx::new(m);
    loop {
        let si = c.reg(SI);
        let sprite = c.word_in(DS, si);
        if sprite == 0xffff {
            c.run(0xc21b, 4)?; // lodsw; cmp; je; ret
            c.lods(W, DS);
            c.cmp(W, sprite, 0xffff);
            return c.ret();
        }
        c.run(0xc21b, 10)?;
        c.lods(W, DS);
        c.cmp(W, sprite, 0xffff);
        let x = c.lods(W, DS);
        let y = c.lods(W, DS);
        c.set_reg(AX, sprite);
        c.set_reg(DX, x);
        c.set_reg(BX, y);
        let si = c.reg(SI);
        c.push(si);
        c.call(code::DRAW_SPRITE_CLOBBERING_BX_DX, 0xc22c)?;
        c.run(0xc22c, 2)?; // pop si; jmp
        let si = c.pop();
        c.set_reg(SI, si);
    }
}

/// From 3978: opens sheet AL, takes sub-resource BL as the palette.
/// Exit: ES:SI = the sub-resource, DX = SI + 6 (its colours), AX = BL.
fn open_palette(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0x3978, 2)?;
    let sheet = c.reg8(r8::AL);
    c.set_byte(g::PALETTE_RESOURCE, sheet);
    c.call(code::OPEN_RESOURCE_BY_INDEX, 0x397e)?;
    c.run(0x397e, 4)?;
    let sub = c.reg8(r8::BL);
    c.set_reg8(r8::AL, sub);
    let ah = c.reg8(r8::AH);
    c.xor(B, ah as u16, ah as u16);
    c.set_reg8(r8::AH, 0);
    c.set_byte(g::PALETTE_SUB, sub);
    c.call(code::GET_SUBRESOURCE_POINTER, 0x3988)?;
    c.run(0x3988, 2)?;
    let si = c.reg(SI);
    c.set_reg(DX, si.wrapping_add(6));
    c.ret()
}

/// `open_sky_palette_al_sub_bl_dsdx` (3978).
pub fn open_sky_palette_al_sub_bl_dsdx(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3978)?;
    open_palette(&mut Ctx::new(m))
}

/// `open_sky_or_skydn_palette_al_sub_bl` (3971): the sky (or its night
/// variant) palette BL. Flags as `open_palette` leaves them.
pub fn open_sky_or_skydn_palette_al_sub_bl(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3971)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    let variant = c.byte(g::SKY_VARIANT);
    let sheet = pure::add(B, SKY as u16, variant as u16);
    c.set_reg(AX, sheet);
    open_palette(&mut c)
}

/// `open_skyrs_palette` (0820): palette BL of SKYRS.
pub fn open_skyrs_palette(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x0820)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?; // mov ax, 2Eh; jmp 3978h
    c.set_reg(AX, SKYRS as u16);
    open_palette(&mut c)
}

/// The ornithopters: from 3A73, the next position (DX + 46h, BX + 0Ah);
/// `loop` back to 3A6A, which draws one (`sub_3aa9`, keeping CX, BX, DX).
fn ornis(c: &mut Ctx, mut draw_first: bool) -> Result<(), Bail> {
    loop {
        if draw_first {
            c.run(0x3a6a, 4)?;
            for r in [CX, BX, DX] {
                let v = c.reg(r);
                c.push(v);
            }
            c.call(code::DRAW_ORNI, 0x3a70)?;
            let sp = c.reg(SP);
            let cx = c.word_in(SS, sp.wrapping_add(4));
            c.run(0x3a70, 3 + 3 + (cx == 1) as u32)?;
            for r in [DX, BX, CX] {
                let v = c.pop();
                c.set_reg(r, v);
            }
        } else {
            let cx = c.reg(CX);
            c.run(0x3a73, 3 + (cx == 1) as u32)?;
        }
        draw_first = true;
        let (dx, bx) = (c.reg(DX), c.reg(BX));
        let dx = c.add(W, dx, 0x46);
        c.set_reg(DX, dx);
        let bx = c.add(W, bx, 0x0a);
        c.set_reg(BX, bx);
        let cx = c.reg(CX).wrapping_sub(1);
        c.set_reg(CX, cx);
        if cx == 0 {
            return c.ret();
        }
    }
}

/// `draw_ornis_loop` (3A6A): CX ornithopters from (DX, BX).
pub fn draw_ornis_loop(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3a6a)?;
    ornis(&mut Ctx::new(m), true)
}

/// `draw_ornis` (3A73): CX - 1 ornithopters, from the position after
/// (DX, BX).
pub fn draw_ornis(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3a73)?;
    ornis(&mut Ctx::new(m), false)
}
