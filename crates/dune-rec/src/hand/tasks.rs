//! Frame tasks: routines the frame loop calls every few frames.
//!
//! The list at [`g::FRAME_TASKS`]: a word count (at most 20), then 6-byte
//! entries: the period in frames, a frame counter, the routine. While the
//! frame loop walks the list, [`g::FRAME_TASK_WALK`] points to its cursor
//! (the current entry, and the entries left), which adding or removing a
//! task keeps right.

use super::{Ctx, W, entered, globals as g};
use pc::{
    Machine,
    cpu::{AX, BP, BX, CX, DI, DS, ES, SI},
    rec::Bail,
};

const MAX_TASKS: u16 = 0x14;
const ENTRY_SIZE: u16 = 6;

/// `add_frame_task` (DA25): adds routine SI, every BP frames, unless the
/// list is full. Exit: ES = DS; flags from the `cmp` (full), else from the
/// cursor check. AX, BX, DI, BP as the original leaves them.
pub fn add_frame_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xda25)?;
    let mut c = Ctx::new(m);
    c.check(start, 25)?;
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.set_reg(DI, g::FRAME_TASKS);
    let count = c.word(g::FRAME_TASKS);
    c.set_reg(AX, count);
    let n = c.inc(W, count);
    c.set_reg(AX, n);
    c.cmp(W, n, MAX_TASKS);
    if n > MAX_TASKS {
        c.count(8);
        return c.ret();
    }
    c.stos(W); // the new count
    // ax = 6 × the old count: the offset of the new entry.
    let old = c.dec(W, n);
    let two = c.add(W, old, old);
    c.set_reg(BX, two);
    let four = c.add(W, two, two);
    let offset = c.add(W, four, two);
    let di = c.reg(DI);
    let di = c.add(W, di, offset);
    c.set_reg(DI, di);
    let (period, routine) = (c.reg(BP), c.reg(SI));
    c.set_reg(AX, period);
    c.stos(W);
    let zero = c.xor(W, period, period);
    c.set_reg(AX, zero);
    c.stos(W);
    c.set_reg(AX, routine);
    c.stos(W);
    let walk = c.word(g::FRAME_TASK_WALK);
    c.set_reg(BP, walk);
    c.or(W, walk, walk);
    if walk == 0 {
        c.count(24);
        return c.ret();
    }
    // The list grew under the walk: one more entry left.
    let left = c.word(walk.wrapping_add(2));
    let left = c.inc(W, left);
    c.set_word(walk.wrapping_add(2), left);
    c.count(25);
    c.ret()
}

/// The search and removal of `remove_frame_task`, from DA5F.
fn remove(c: &mut Ctx) -> Result<(), Bail> {
    c.run(0xda5f, 3)?; // mov di; mov cx, [di]; jcxz
    c.set_reg(DI, g::FRAME_TASKS);
    let count = c.word(g::FRAME_TASKS);
    c.set_reg(CX, count);
    if count == 0 {
        c.run(0xda72, 1)?;
        return c.ret();
    }
    c.run(0xda66, 1)?;
    let mut di = c.add(W, g::FRAME_TASKS, ENTRY_SIZE); // the first entry's routine
    c.set_reg(DI, di);
    let routine = c.reg(SI);
    loop {
        let here = c.word(di);
        let cx = c.reg(CX);
        if here == routine {
            c.run(0xda69, 2)?; // cmp; je
            c.cmp(W, here, routine);
            break;
        }
        c.run(0xda69, 4)?; // cmp; je; add di, 6; loop
        c.cmp(W, here, routine);
        di = c.add(W, di, ENTRY_SIZE);
        c.set_reg(DI, di);
        c.set_reg(CX, cx.wrapping_sub(1));
        if cx == 1 {
            c.run(0xda72, 1)?; // not in the list
            return c.ret();
        }
    }

    // Found: DI to the entry's start, one task less.
    c.check(0xda73, 5 + 4 + 2 + 8)?;
    let entry = c.sub(W, di, 4);
    c.set_reg(DI, entry);
    let count = c.word(g::FRAME_TASKS);
    let count = c.dec(W, count);
    c.set_word(g::FRAME_TASKS, count);
    let walk = c.word(g::FRAME_TASK_WALK);
    c.set_reg(BP, walk);
    c.or(W, walk, walk);
    let mut n = 5; // sub; dec; mov; or; je
    if walk != 0 {
        let current = c.word(walk);
        c.cmp(W, entry, current);
        if entry > current {
            // Removed after the cursor: one entry less left.
            let left = c.word(walk.wrapping_add(2));
            let left = c.dec(W, left);
            c.set_word(walk.wrapping_add(2), left);
            n += 3; // cmp; ja; dec
        } else {
            // At or before it: the cursor moves back with the entries.
            let current = c.sub(W, current, ENTRY_SIZE);
            c.set_word(walk, current);
            n += 4; // cmp; ja; sub; jmp
        }
    }
    // Entries after it: CX - 1 of them, moved down over it.
    let cx = c.reg(CX);
    let after = c.dec(W, cx);
    c.set_reg(CX, after);
    n += 2; // dec cx; je
    if after == 0 {
        c.count(n + 1);
        return c.ret();
    }
    c.count(n + 7);
    let two = c.add(W, after, after);
    let words = c.add(W, two, after);
    c.set_reg(AX, after);
    c.set_reg(CX, words);
    let from = c.add(W, entry, ENTRY_SIZE);
    c.set_reg(SI, from);
    let ds = c.sreg(DS);
    c.push(ds);
    let es = c.pop();
    c.set_sreg(ES, es);
    c.run(0xdaa0, 1)?;
    c.rep(0xa5); // rep movsw
    c.run(0xdaa2, 1)?;
    c.ret()
}

/// `remove_frame_task` (DA5F): removes routine SI from the frame tasks, if
/// there. Registers and flags as the original leaves them.
pub fn remove_frame_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0xda5f)?;
    remove(&mut Ctx::new(m))
}

/// `remove_room_frame_task` (39E6): removes the room animation's task
/// (C0B6h).
pub fn remove_room_frame_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x39e6)?;
    let mut c = Ctx::new(m);
    c.run(start, 2)?;
    c.set_reg(SI, ROOM_ANIMATION_TASK);
    remove(&mut c)
}

/// `add_room_frame_task` (3A7C): replaces the room animation's task: it
/// runs (every 12 frames) only where byte 4 is 4 and byte 5 below 20h.
/// Flags from the last `cmp`, or as `add_frame_task` leaves them.
pub fn add_room_frame_task(m: &mut Machine, start: u16) -> Result<(), Bail> {
    entered(start, 0x3a7c)?;
    let mut c = Ctx::new(m);
    c.run(start, 1)?;
    c.call(0x39e6, 0x3a7f)?;
    let place = c.word(g::WORD_0004);
    let (low, high) = (place & 0xff, place >> 8);
    let animated = low == 4 && high < 0x20;
    c.run(0x3a7f, if low != 4 { 4 } else if !animated { 6 } else { 8 })?;
    c.set_reg(AX, place);
    c.cmp(super::B, low, 4);
    if low == 4 {
        c.cmp(super::B, high, 0x20);
    }
    if animated {
        c.set_reg(SI, ROOM_ANIMATION_TASK);
        c.set_reg(BP, 0x0c);
        c.call(0xda25, 0x3a94)?;
        c.run(0x3a94, 1)?;
    }
    c.ret()
}

/// The room animation's frame task.
const ROOM_ANIMATION_TASK: u16 = 0xc0b6;
