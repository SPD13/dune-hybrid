//! The game clock as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! The clock is a [`Clock`] kept with the machine ([`Machine::native`], [`super::native`]);
//! the ports that count, stop or read it go through it, and the clock's
//! logic (the timer's tick, a step of the game time, stopping and
//! starting) is its methods.
//!
//! The 8086 memory stays a mirror: every change is written through to
//! the original's words (the suite compares the memory, the game's save
//! routine copies them), and every access first compares the struct
//! with them. A difference means code outside the clock wrote them: the
//! interpreter (a port that gave up partway, the timer's interrupt
//! included, runs on as the original), a save loaded, the shadow check
//! putting the original's memory back. The struct then takes the
//! memory's values: a sync-in, counted ([`syncs`]; `CLOCK_TRACE=1`
//! prints each with the fields that differed). That is the boundary
//! where the clock and the rest are kept in step.

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
    clock: Clock,
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
    /// The clock as the memory has it.
    fn mirror(c: &mut Ctx) -> Clock {
        let _q = Quiet::new(c.m);
        let w = |c: &mut Ctx, off| c.word_at(g::GAME_DS, off);
        let b = |c: &mut Ctx, off| c.byte_at_seg(g::GAME_DS, off);
        Clock {
            ticks: (w(c, g::TIMER_HIGH) as u32) << 16 | w(c, g::TIMER) as u32,
            held: b(c, g::CLOCK_HELD),
            stopped: b(c, g::CLOCK_STOPPED),
            countdown: w(c, g::CLOCK_COUNTDOWN),
            period: w(c, g::CLOCK_PERIOD),
            steps_pending: b(c, g::CLOCK_STEPS_PENDING),
            time: w(c, g::GAME_TIME),
        }
    }

    /// The clock's words in the memory set to `self` (those that changed).
    fn write_mirror(&self, old: &Clock, c: &mut Ctx) {
        let _q = Quiet::new(c.m);
        let w = |c: &mut Ctx, off, v: u16| c.set_word_at(g::GAME_DS, off, v);
        let b = |c: &mut Ctx, off, v: u8| c.set_byte_at_seg(g::GAME_DS, off, v);
        if self.ticks as u16 != old.ticks as u16 {
            w(c, g::TIMER, self.ticks as u16);
        }
        if self.ticks >> 16 != old.ticks >> 16 {
            w(c, g::TIMER_HIGH, (self.ticks >> 16) as u16);
        }
        if self.held != old.held {
            b(c, g::CLOCK_HELD, self.held);
        }
        if self.stopped != old.stopped {
            b(c, g::CLOCK_STOPPED, self.stopped);
        }
        if self.countdown != old.countdown {
            w(c, g::CLOCK_COUNTDOWN, self.countdown);
        }
        if self.period != old.period {
            w(c, g::CLOCK_PERIOD, self.period);
        }
        if self.steps_pending != old.steps_pending {
            b(c, g::CLOCK_STEPS_PENDING, self.steps_pending);
        }
        if self.time != old.time {
            w(c, g::GAME_TIME, self.time);
        }
    }

    /// The clock, kept in step with the memory (see the module's notes).
    pub(super) fn get(c: &mut Ctx) -> Clock {
        let mem = Clock::mirror(c);
        let n = native(c.m);
        if !n.started || n.clock != mem {
            // The interpreter ran the timer's tick (an event fell inside
            // its stretch): the memory is what the clock's own tick makes.
            let mut ticked = n.clock;
            if !ticked.is_held() {
                ticked.tick();
            }
            if n.started && ticked == mem {
                n.ticks_outside += 1;
            } else if n.started {
                n.clock_syncs += 1;
                let was = n.clock;
                if std::env::var_os("CLOCK_TRACE").is_some() {
                    eprintln!("clock sync-in at insn {} ({:04x}): {was:?} -> {mem:?}", c.m.cpu.instructions, c.m.cpu.ip);
                }
            }
            let n = native(c.m);
            n.started = true;
            n.clock = mem;
        }
        mem
    }

    /// The clock changed to `self`: kept, and written through.
    pub(super) fn set(self, c: &mut Ctx) {
        let old = Clock::get(c);
        native(c.m).clock = self;
        self.write_mirror(&old, c);
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
    let mem = Clock::mirror(c);
    if std::env::var_os("CLOCK_TRACE").is_some() {
        eprintln!("clock loaded at insn {}: {mem:?}", c.m.cpu.instructions);
    }
    let n = native(c.m);
    n.started = true;
    n.clock = mem;
}

/// The machine ran outside the recompiled code on purpose (a tool
/// stepping it in the interpreter, a snapshot restored): the clock takes
/// the memory's values (a sync-in at the boundary, not counted).
pub fn outside(m: &mut Machine) {
    let quiet = m.hw.watch.as_ref().is_some_and(|w| w.quiet);
    if let Some(w) = &mut m.hw.watch {
        w.quiet = true;
    }
    let a = |m: &Machine, off: u16| m.cpu.phys(g::GAME_DS, off);
    let b = |m: &mut Machine, off: u16| {
        let p = a(m, off);
        m.hw.read8(p)
    };
    let w = |m: &mut Machine, off: u16| b(m, off) as u16 | (b(m, off.wrapping_add(1)) as u16) << 8;
    let mem = Clock {
        ticks: (w(m, g::TIMER_HIGH) as u32) << 16 | w(m, g::TIMER) as u32,
        held: b(m, g::CLOCK_HELD),
        stopped: b(m, g::CLOCK_STOPPED),
        countdown: w(m, g::CLOCK_COUNTDOWN),
        period: w(m, g::CLOCK_PERIOD),
        steps_pending: b(m, g::CLOCK_STEPS_PENDING),
        time: w(m, g::GAME_TIME),
    };
    if let Some(w) = &mut m.hw.watch {
        w.quiet = quiet;
    }
    let n = native(m);
    n.started = true;
    n.clock = mem;
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
