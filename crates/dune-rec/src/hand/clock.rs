//! The game clock as Rust state (Stage B of `docs/readable-port-plan.md`).
//!
//! The clock is a [`Clock`] kept with the machine ([`Machine::native`]);
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
use pc::Machine;

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

/// The Rust state kept with a machine.
#[derive(Default)]
pub struct Native {
    clock: Clock,
    /// The clock has been read from the memory once.
    started: bool,
    /// Sync-ins after the first (see the module's notes).
    clock_syncs: u64,
}

/// The machine's Rust state (made on first use).
fn native(m: &mut Machine) -> &mut Native {
    m.native.get_or_insert_with(|| Box::new(Native::default())).downcast_mut::<Native>().expect("native state")
}

/// How often the clock took the memory's values after the first time.
pub fn syncs(m: &mut Machine) -> u64 {
    native(m).clock_syncs
}

impl Clock {
    /// The clock as the memory has it.
    fn mirror(c: &mut Ctx) -> Clock {
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
            if n.started {
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
    let n = native(c.m);
    n.started = true;
    n.clock = mem;
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
