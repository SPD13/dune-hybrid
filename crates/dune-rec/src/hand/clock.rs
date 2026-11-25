//! The game clock as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! The clock is a [`Clock`] kept with the machine ([`Machine::native`], [`super::native`]);
//! the ports that count, stop or read it go through it, and the clock's
//! logic (the timer's tick, a step of the game time, stopping and
//! starting) is its methods.
//!
//! The clock owns its memory (Stage B's ownership): its words in the data
//! segment live in [`ClockMemory`], registered with the bus
//! (`pc::hw::Owned`) on the first use. Every read and write of them by
//! any code goes to it: the graphics driver polling the ticks through
//! the pointer it was given, the interpreter's tick when an event falls
//! inside the tick's stretch, a save's restore, the original's reads
//! through null pointers. The memory keeps a copy written through for
//! the tools (snapshots, state hashes, the shadow check), which the clock
//! takes back where they replace the memory. So there is nothing to sync
//! in: `syncs` and `ticks_outside` stay 0 (the runner's `clock:` line,
//! the suite's gate).

use super::Ctx;
use super::globals as g;
use pc::{Machine, cpu::Bus};

/// The game clock.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Clock {
    /// The timer's ticks, counted by its interrupt (CE7Ah, with the
    /// high word at CE7Ch): the waits measure time with them.
    pub ticks: u32,
    /// Above 0 (signed): the interrupt counts nothing ([0CEEAh]).
    pub held: u8,
    /// Not 0: the game time stands still; a count of who stopped it
    /// ([2788h]).
    pub stopped: u8,
    /// Ticks left until the game time steps ([46DBh]), counted down by
    /// the interrupt and restarted from `period` ([146Eh]).
    pub countdown: u16,
    pub period: u16,
    /// Steps the interrupt took that the game has not yet run ([46DDh]).
    pub steps_pending: u8,
    /// The game time ([2]): 16 slots a day.
    pub time: u16,
}

/// The clock's part of the Rust state kept with a machine
/// ([`super::native`]).
#[derive(Default)]
pub struct ClockState {
    /// The clock has been read from the memory once.
    started: bool,
    /// Sync-ins after the first (see the module's notes).
    clock_syncs: u64,
    /// Timer ticks the original's code counted itself (the interpreter
    /// ran the handler's tick): the memory was the clock with one tick
    /// more, as [`Clock::tick`] predicts. Taken, not counted as sync-ins.
    ticks_outside: u64,
}

fn native(m: &mut Machine) -> &mut ClockState {
    &mut super::native::native(m).clock
}

/// How often the clock took the memory's values after the first time
/// (unforeseen writes).
pub fn syncs(m: &mut Machine) -> u64 {
    native(m).clock_syncs
}

/// How many timer ticks the original's code counted (see [`ClockState`]).
pub fn ticks_outside(m: &mut Machine) -> u64 {
    native(m).ticks_outside
}

impl Clock {
    /// The clock (its memory owned from the first use on, [`ClockMemory`]).
    pub(super) fn get(c: &mut Ctx) -> Clock {
        own(c.m);
        c.m.hw.owned::<ClockMemory>().expect("the clock's memory").0
    }

    /// The clock changed to `self`: the bytes that changed written on the
    /// bus (to the clock, and the memory's copy).
    pub(super) fn set(self, c: &mut Ctx) {
        let old = Clock::get(c);
        let _q = Quiet::new(c.m);
        for &(off, size) in &WORDS {
            for i in 0..size {
                let (a, b) = (old.byte_at(off + i), self.byte_at(off + i));
                if a != b {
                    let at = c.m.cpu.phys(g::GAME_DS, off + i);
                    c.m.hw.write8(at, b);
                }
            }
        }
    }

    /// The byte at DS:`off`, one of the clock's words.
    fn byte_at(&self, off: u16) -> u8 {
        let (word, i) = WORDS.iter().find(|&&(o, n)| off >= o && off < o + n).map(|&(o, _)| (o, off - o)).expect("a clock word");
        (self.word(word) >> (8 * i)) as u8
    }

    /// The byte at DS:`off` set to `v`.
    fn set_byte_at(&mut self, off: u16, v: u8) {
        let (word, i) = WORDS.iter().find(|&&(o, n)| off >= o && off < o + n).map(|&(o, _)| (o, off - o)).expect("a clock word");
        let mask = 0xffu16 << (8 * i);
        let w = self.word(word) & !mask | (v as u16) << (8 * i);
        match word {
            g::TIMER => self.ticks = self.ticks & 0xffff_0000 | w as u32,
            g::TIMER_HIGH => self.ticks = self.ticks & 0xffff | (w as u32) << 16,
            g::CLOCK_HELD => self.held = w as u8,
            g::CLOCK_STOPPED => self.stopped = w as u8,
            g::CLOCK_COUNTDOWN => self.countdown = w,
            g::CLOCK_PERIOD => self.period = w,
            g::CLOCK_STEPS_PENDING => self.steps_pending = w as u8,
            _ => self.time = w,
        }
    }

    /// The word (or byte) at DS:`word`, one of the clock's.
    fn word(&self, word: u16) -> u16 {
        match word {
            g::TIMER => self.ticks as u16,
            g::TIMER_HIGH => (self.ticks >> 16) as u16,
            g::CLOCK_HELD => self.held as u16,
            g::CLOCK_STOPPED => self.stopped as u16,
            g::CLOCK_COUNTDOWN => self.countdown,
            g::CLOCK_PERIOD => self.period,
            g::CLOCK_STEPS_PENDING => self.steps_pending as u16,
            _ => self.time,
        }
    }

    /// The interrupt holds the clock.
    pub fn is_held(&self) -> bool {
        self.held as i8 > 0
    }

    /// The timer's tick, as its interrupt counts it (unless held): the
    /// ticks up; unless stopped the countdown down, and at its end
    /// restarted from the period with the game time a step on and the
    /// step noted for the game to run. Returns whether the time stepped.
    pub fn tick(&mut self) -> bool {
        self.ticks = self.ticks.wrapping_add(1);
        if self.stopped != 0 {
            return false;
        }
        self.countdown = self.countdown.wrapping_sub(1);
        if self.countdown & 0x8000 == 0 {
            return false;
        }
        self.countdown = self.period;
        self.time = self.time.wrapping_add(1);
        self.steps_pending = self.steps_pending.wrapping_add(1);
        true
    }

    /// Stopped once more (`stop_clock`).
    pub fn stop(&mut self) {
        self.stopped = self.stopped.wrapping_add(1);
    }

    /// One stop less, not below 0 (`release_clock`): a count already 0
    /// or 80h and up (negative) stays.
    pub fn release(&mut self) {
        let n = self.stopped.wrapping_sub(1);
        if n & 0x80 == 0 {
            self.stopped = n;
        }
    }

    /// Running again, whoever stopped it (`start_clock`).
    pub fn start(&mut self) {
        self.stopped = 0;
    }

    /// A step of the game time run at once (`run_clock_ticks`): the
    /// countdown restarted, the time on, the step noted.
    pub fn restart_countdown(&mut self) {
        self.countdown = self.period;
    }
    pub fn advance(&mut self) {
        self.time = self.time.wrapping_add(1);
        self.steps_pending = 1;
    }

    /// The slot of the day (0 to 15).
    pub fn time_of_day(&self) -> u16 {
        self.time & 0x0f
    }

    /// The day counted from 0.
    pub fn day_from_0(&self) -> u16 {
        self.time >> 4
    }

    /// The day as the game shows it (it turns 3 slots early).
    pub fn day(&self) -> u16 {
        self.time.wrapping_add(3) >> 4
    }
}

/// A save was loaded into the memory: the clock takes its values there
/// (a sync-in at the boundary, not counted).
pub(super) fn loaded(c: &mut Ctx) {
    // The restore wrote the words on the bus: the clock has them.
    own(c.m);
}

/// The machine ran outside the recompiled code on purpose (a tool
/// stepping it in the interpreter, a snapshot restored): nothing to do,
/// the clock owns its memory (whatever ran wrote the clock, and a
/// restored memory was absorbed).
pub fn outside(_m: &mut Machine) {}

/// The clock as the memory the bus serves from it: its words in the data
/// segment ([`WORDS`]) live in it.
pub struct ClockMemory(Clock);

fn ds_base() -> u32 {
    (g::GAME_DS as u32) << 4
}

impl pc::hw::Owned for ClockMemory {
    fn ranges(&self) -> Vec<(u32, u32)> {
        WORDS.iter().map(|&(o, n)| (ds_base() + o as u32, n as u32)).collect()
    }
    fn read(&self, addr: u32) -> u8 {
        self.0.byte_at((addr - ds_base()) as u16)
    }
    fn write(&mut self, addr: u32, v: u8) {
        self.0.set_byte_at((addr - ds_base()) as u16, v);
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The clock owns its words from now on (once).
fn own(m: &mut Machine) {
    if m.hw.owned::<ClockMemory>().is_none() {
        m.hw.own(Box::new(ClockMemory(Clock::default())));
        native(m).started = true;
    }
}

/// The clock's words in the memory: DS offset and size.
const WORDS: [(u16, u16); 8] = [
    (g::TIMER, 2),
    (g::TIMER_HIGH, 2),
    (g::CLOCK_HELD, 1),
    (g::CLOCK_STOPPED, 1),
    (g::CLOCK_COUNTDOWN, 2),
    (g::CLOCK_PERIOD, 2),
    (g::CLOCK_STEPS_PENDING, 1),
    (g::GAME_TIME, 2),
];

/// Marks the clock's own accesses to its words (not counted by the
/// watch, [`watch`]) while it lives.
struct Quiet {
    m: *mut Machine,
    was: bool,
}

impl Quiet {
    fn new(m: &mut Machine) -> Quiet {
        let was = m.hw.watch.as_ref().is_some_and(|w| w.quiet);
        if let Some(w) = &mut m.hw.watch {
            w.quiet = true;
        }
        Quiet { m, was }
    }
}

impl Drop for Quiet {
    fn drop(&mut self) {
        // SAFETY: the machine outlives the guard (made and dropped inside
        // one call on it, and only its `watch` is touched).
        let m = unsafe { &mut *self.m };
        if let Some(w) = &mut m.hw.watch {
            w.quiet = self.was;
        }
    }
}

/// `CLOCK_WATCH=1` (the runner): watch the clock's words for accesses
/// by code other than the clock's (Stage B: what still reads or writes
/// them outside it).
pub fn watch(m: &mut Machine) {
    let addrs = WORDS.iter().flat_map(|&(off, n)| (0..n).map(move |i| m_phys(off + i)));
    m.hw.watch = Some(Box::new(pc::hw::MemWatch::new(addrs.collect::<Vec<_>>())));
}

fn m_phys(off: u16) -> u32 {
    ((g::GAME_DS as u32) << 4) + off as u32
}

/// The watch's findings ([`watch`]): one line per (who, word, read or
/// write), most accesses first. `name` names a hand port or a routine by
/// its entry, `owner` gives the routine a seg000 address belongs to.
pub fn watch_report(m: &Machine, name: impl Fn(u16) -> String, owner: impl Fn(u16) -> String) -> Vec<String> {
    let Some(w) = &m.hw.watch else { return Vec::new() };
    let word = |addr: u32| {
        let off = addr.wrapping_sub((g::GAME_DS as u32) << 4) as u16;
        WORDS.iter().find(|&&(o, n)| off >= o && off < o + n).map_or(format!("{off:04x}"), |&(o, _)| format!("[{o:04x}]"))
    };
    let mut by: std::collections::HashMap<(String, String, bool), u64> = Default::default();
    for (&(who, addr, write), &n) in &w.hits {
        let (kind, cs, ip) = (who >> 32, (who >> 16) as u16, who as u16);
        let kind = match kind {
            pc::hw::WHO_HAND => "hand",
            pc::hw::WHO_GENERATED => "generated block",
            _ => "interpreter",
        };
        let who = if kind == "hand" {
            format!("hand {ip:04x} {}", name(ip))
        } else if cs == pc::LOAD_SEG {
            format!("{kind} {cs:04x}:{ip:04x} (in {})", owner(ip))
        } else {
            format!("{kind} {cs:04x}:{ip:04x} (a driver)")
        };
        *by.entry((who, word(addr), write)).or_default() += n;
    }
    let mut v: Vec<_> = by.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.into_iter().map(|((who, word, write), n)| format!("clock watch: {n:9} {} {word} by {who}", if write { "write" } else { "read " })).collect()
}

/// The byte at DS:`off` as the scripts read their variables (the story
/// state, DS:0-FFh): from the clock where it is one of its words (the
/// game time, [2]), else the memory's.
pub(super) fn story_byte(c: &mut Ctx, off: u16) -> u8 {
    match off.wrapping_sub(g::GAME_TIME) {
        0 => time(c) as u8,
        1 => (time(c) >> 8) as u8,
        _ => c.byte(off),
    }
}

/// The game time ([`Clock::time`]).
pub(super) fn time(c: &mut Ctx) -> u16 {
    Clock::get(c).time
}

/// The timer's ticks, low word ([`Clock::ticks`]).
pub(super) fn ticks(c: &mut Ctx) -> u16 {
    Clock::get(c).ticks as u16
}

/// The clock changed by `f`.
pub(super) fn update(c: &mut Ctx, f: impl FnOnce(&mut Clock)) {
    let mut k = Clock::get(c);
    f(&mut k);
    k.set(c);
}
