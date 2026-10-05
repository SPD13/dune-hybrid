//! Graphics driver call recorder: notices when the game calls its DNVGA
//! driver and when each call returns, so a host can follow (and later
//! re-render in high resolution) what is drawn where.
//!
//! The game reaches the driver through far pointers in its data segment,
//! filled when the driver is loaded (seg000:e57b): slot `n` is at
//! `DS:38B5h + 4n` and points at `driver:0100h + 3n`, a `jmp` to the
//! implementation. The recorder watches that span of entry points; a call
//! ends when execution reaches the far return address with the stack back
//! where it was before the call.
//!
//! Recording is off by default and never changes what the machine does.

use crate::{GAME_DS, Hardware};

/// Entries in the driver's jump table.
pub const SLOTS: usize = 46;
/// `DS:` offset of slot 0's far pointer (offset, then segment).
const TABLE: usize = 0x38b5;
/// Driver offset of the Y offset added to every screen address (slot 33).
pub const DRV_Y_OFFSET: u16 = 0x01a3;

/// Registers at the moment the game enters a driver slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DriverCall {
    pub slot: u8,
    /// Virtual time of the call.
    pub at_ns: u64,
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub si: u16,
    pub di: u16,
    pub bp: u16,
    pub ds: u16,
    pub es: u16,
    pub ss: u16,
    pub flags: u16,
    /// The driver's segment.
    pub drv: u16,
    /// `drv:01A3`, the Y offset (in bytes) the driver adds to screen addresses.
    pub y_offset: u16,
    /// Nesting depth (an interrupt handler may draw the cursor mid-call).
    pub depth: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GfxEvent {
    Enter(DriverCall),
    /// The call `slot` at nesting `depth` returned.
    Return { slot: u8, depth: u8, at_ns: u64 },
}

pub type GfxHook = Box<dyn FnMut(&GfxEvent, &Hardware)>;

struct Pending {
    slot: u8,
    ret: u32,
    sp: u16,
}

pub struct GfxRecorder {
    /// Record calls (watch the entry points every instruction).
    pub enabled: bool,
    /// Linear address of slot 0 (0 until the driver is loaded).
    pub(crate) base: u32,
    pub(crate) drv: u16,
    /// Calls per slot since recording started.
    pub counts: [u64; SLOTS],
    /// Called on every entry and return, with the machine's memory.
    pub hook: Option<GfxHook>,
    stack: Vec<Pending>,
}

impl Default for GfxRecorder {
    fn default() -> Self {
        GfxRecorder { enabled: false, base: 0, drv: 0, counts: [0; SLOTS], hook: None, stack: Vec::new() }
    }
}

impl GfxRecorder {
    #[inline]
    pub(crate) fn active(&self) -> bool {
        self.enabled && self.base != 0
    }

    /// Re-read the driver's segment (cheap; called on device events).
    pub(crate) fn refresh(&mut self, mem: &[u8]) {
        let a = GAME_DS as usize * 16 + TABLE + 2;
        let seg = u16::from_le_bytes([mem[a], mem[a + 1]]);
        self.drv = seg;
        self.base = if seg == 0 { 0 } else { ((seg as u32) << 4) + 0x100 };
    }

    /// Forget calls in progress (after a snapshot restore).
    pub(crate) fn reset(&mut self) {
        self.stack.clear();
    }

    /// Which slot starts at `pc`, if any.
    #[inline]
    pub(crate) fn slot_at(&self, pc: u32) -> Option<u8> {
        let rel = pc.wrapping_sub(self.base);
        (rel < (SLOTS * 3) as u32 && rel.is_multiple_of(3)).then_some((rel / 3) as u8)
    }

    /// Does `pc`/`sp` complete the innermost call in progress?
    #[inline]
    pub(crate) fn returns_at(&self, pc: u32, sp: u16) -> bool {
        self.stack.last().is_some_and(|p| p.ret == pc && p.sp == sp)
    }

    pub(crate) fn enter(&mut self, mut call: DriverCall, ret: u32, sp_after: u16, hw: &Hardware) {
        call.depth = self.stack.len() as u8;
        self.counts[call.slot as usize] += 1;
        self.stack.push(Pending { slot: call.slot, ret, sp: sp_after });
        if let Some(h) = self.hook.as_mut() {
            h(&GfxEvent::Enter(call), hw);
        }
    }

    pub(crate) fn leave(&mut self, at_ns: u64, hw: &Hardware) {
        let Some(p) = self.stack.pop() else { return };
        if let Some(h) = self.hook.as_mut() {
            h(&GfxEvent::Return { slot: p.slot, depth: self.stack.len() as u8, at_ns }, hw);
        }
    }
}
