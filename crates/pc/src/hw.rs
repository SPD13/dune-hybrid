//! The hardware the game touches directly: 8259 PICs, 8254 PIT, the keyboard
//! controller, VGA DAC/status ports and the memory map. Time is virtual
//! (nanoseconds derived from the instruction count), so runs are deterministic.

use std::collections::VecDeque;

use cpu::Bus;

use crate::sound::{self, Audio};

/// 1 MB + HMA.
pub const MEM_SIZE: usize = 0x11_0000;
const PIT_HZ: f64 = 1_193_182.0;
const PIT_TICK_NS: f64 = 1e9 / PIT_HZ;
/// VGA mode 13h refresh (70 Hz) and the vertical-retrace share of a frame.
const FRAME_NS: u64 = 14_285_714;
const VRETRACE_NS: u64 = 1_000_000;

#[derive(Default, Clone)]
pub struct Pic {
    pub irr: u8,
    pub imr: u8,
    pub isr: u8,
    pub base: u8,
    init_step: u8,
    icw4: bool,
    read_isr: bool,
}

impl Pic {
    fn new(base: u8) -> Self {
        Pic { base, ..Default::default() }
    }

    fn write(&mut self, port_lo: bool, v: u8) {
        if !port_lo {
            match self.init_step {
                1 => {
                    self.base = v & 0xf8;
                    self.init_step = 2;
                }
                2 => self.init_step = if self.icw4 { 3 } else { 0 },
                3 => self.init_step = 0,
                _ => self.imr = v,
            }
            return;
        }
        if v & 0x10 != 0 {
            // ICW1
            self.init_step = 1;
            self.icw4 = v & 1 != 0;
            self.imr = 0;
            self.isr = 0;
            self.irr = 0;
        } else if v & 0x08 != 0 {
            // OCW3
            if v & 2 != 0 {
                self.read_isr = v & 1 != 0;
            }
        } else if v & 0x20 != 0 {
            // OCW2: EOI (non-specific clears the highest-priority in-service).
            if v & 0x40 != 0 {
                self.isr &= !(1 << (v & 7));
            } else if self.isr != 0 {
                self.isr &= self.isr - 1;
            }
        }
    }

    fn read(&self, port_lo: bool) -> u8 {
        if port_lo {
            if self.read_isr { self.isr } else { self.irr }
        } else {
            self.imr
        }
    }

    /// Highest-priority requested, unmasked IRQ not blocked by an in-service one.
    fn pending(&self) -> Option<u8> {
        let req = self.irr & !self.imr;
        if req == 0 {
            return None;
        }
        let irq = req.trailing_zeros() as u8;
        let blocking = if self.isr == 0 { 8 } else { self.isr.trailing_zeros() as u8 };
        (irq < blocking).then_some(irq)
    }
}

#[derive(Clone, Default)]
struct PitChannel {
    reload: u32,
    mode: u8,
    access: u8,
    write_hi: bool,
    read_hi: bool,
    latch: Option<u16>,
    pending_lo: u8,
    start_ns: u64,
}

#[derive(Clone)]
pub struct Pit {
    ch: [PitChannel; 3],
    next_irq0_ns: u64,
    speaker_ctl: u8,
    refresh: bool,
}

impl Pit {
    fn new() -> Self {
        let mut ch: [PitChannel; 3] = Default::default();
        for c in &mut ch {
            c.reload = 0x10000;
            c.access = 3;
            c.mode = 3;
        }
        Pit { ch, next_irq0_ns: Self::period_ns(0x10000), speaker_ctl: 0, refresh: false }
    }

    fn period_ns(reload: u32) -> u64 {
        (reload as f64 * PIT_TICK_NS) as u64
    }

    fn count(&self, c: usize, now: u64) -> u16 {
        let ch = &self.ch[c];
        let ticks = ((now.saturating_sub(ch.start_ns)) as f64 / PIT_TICK_NS) as u64;
        let mut v = ch.reload as u64 - ticks % ch.reload as u64;
        if ch.mode == 3 {
            // Square wave counts down by two, twice per period.
            v = (v * 2) % ch.reload as u64;
        }
        v as u16
    }

    fn write_ctl(&mut self, v: u8, now: u64) {
        let c = (v >> 6) as usize;
        if c == 3 {
            return; // read-back (8254) not needed
        }
        let access = (v >> 4) & 3;
        if access == 0 {
            if self.ch[c].latch.is_none() {
                self.ch[c].latch = Some(self.count(c, now));
                self.ch[c].read_hi = false;
            }
            return;
        }
        let ch = &mut self.ch[c];
        ch.access = access;
        ch.mode = (v >> 1) & 7;
        ch.write_hi = false;
        ch.read_hi = false;
        ch.latch = None;
    }

    fn write_data(&mut self, c: usize, v: u8, now: u64) {
        let ch = &mut self.ch[c];
        let done = match ch.access {
            1 => {
                ch.reload = v as u32;
                true
            }
            2 => {
                ch.reload = (v as u32) << 8;
                true
            }
            _ => {
                if ch.write_hi {
                    ch.reload = ch.pending_lo as u32 | (v as u32) << 8;
                    ch.write_hi = false;
                    true
                } else {
                    ch.pending_lo = v;
                    ch.write_hi = true;
                    false
                }
            }
        };
        if done {
            if ch.reload == 0 {
                ch.reload = 0x10000;
            }
            ch.start_ns = now;
            if c == 0 {
                self.next_irq0_ns = now + Self::period_ns(self.ch[0].reload);
            }
        }
    }

    fn read_data(&mut self, c: usize, now: u64) -> u8 {
        let v = match self.ch[c].latch {
            Some(l) => l,
            None => self.count(c, now),
        };
        let ch = &mut self.ch[c];
        let byte = match ch.access {
            1 => v as u8,
            2 => (v >> 8) as u8,
            _ => {
                let b = if ch.read_hi { (v >> 8) as u8 } else { v as u8 };
                ch.read_hi = !ch.read_hi;
                if !ch.read_hi {
                    ch.latch = None;
                }
                return b;
            }
        };
        ch.latch = None;
        byte
    }

    /// PIT channel 0 frequency in Hz (the game reprograms it to 200 Hz).
    pub fn irq0_hz(&self) -> f64 {
        PIT_HZ / self.ch[0].reload as f64
    }
}

#[derive(Clone)]
pub struct Keyboard {
    queue: VecDeque<u8>,
    data: u8,
    full: bool,
}

#[derive(Clone)]
pub struct Vga {
    /// 6-bit DAC entries, RGB.
    pub dac: [[u8; 3]; 256],
    write_index: u8,
    read_index: u8,
    component: u8,
    read_component: u8,
    pub mode: u8,
    pub pel_mask: u8,
}

impl Vga {
    fn new() -> Self {
        Vga {
            dac: [[0; 3]; 256],
            write_index: 0,
            read_index: 0,
            component: 0,
            read_component: 0,
            mode: 3,
            pel_mask: 0xff,
        }
    }

    /// Mode 13h frame as RGB888 (320×200×3), from memory at A0000.
    pub fn render_rgb(&self, mem: &[u8], out: &mut [u8]) {
        let fb = &mem[0xa0000..0xa0000 + 64000];
        for (i, &p) in fb.iter().enumerate() {
            let c = self.dac[(p & self.pel_mask) as usize];
            let o = i * 3;
            out[o] = c[0] << 2 | c[0] >> 4;
            out[o + 1] = c[1] << 2 | c[1] >> 4;
            out[o + 2] = c[2] << 2 | c[2] >> 4;
        }
    }
}

pub struct Hardware {
    pub mem: Vec<u8>,
    pub pic: [Pic; 2],
    pub pit: Pit,
    pub kbd: Keyboard,
    pub vga: Vga,
    /// Current virtual time, refreshed by the machine before each instruction.
    pub now_ns: u64,
    /// Unhandled port accesses, for diagnostics (port, write?).
    pub unknown_ports: Vec<(u16, bool)>,
    /// Joystick port bits (no joystick: buttons up, axes never time out).
    game_port: u8,
    pub audio: Audio,
    /// Diagnostics: log sound/DMA/PIC port traffic (port, value, write?).
    pub trace_ports: Option<Vec<(u64, u16, u8, bool)>>,
    /// CS:IP of the current instruction (maintained only while tracing).
    pub trace_pc: (u16, u16),
    /// Set when a device changed its schedule; the machine re-polls `update`.
    pub reschedule: bool,
}

impl Hardware {
    pub fn new() -> Self {
        Hardware {
            mem: vec![0; MEM_SIZE],
            pic: [Pic::new(0x08), Pic::new(0x70)],
            pit: Pit::new(),
            kbd: Keyboard { queue: VecDeque::new(), data: 0, full: false },
            vga: Vga::new(),
            now_ns: 0,
            unknown_ports: Vec::new(),
            game_port: 0xf0,
            audio: Audio::new(),
            trace_ports: None,
            reschedule: false,
            trace_pc: (0, 0),
        }
    }

    pub fn raise_irq(&mut self, irq: u8) {
        if irq < 8 {
            self.pic[0].irr |= 1 << irq;
        } else {
            self.pic[1].irr |= 1 << (irq - 8);
            self.pic[0].irr |= 1 << 2;
        }
    }

    /// Vector of the interrupt to deliver now, if any (acknowledges it).
    pub fn acknowledge_irq(&mut self) -> Option<u8> {
        let irq = self.pic[0].pending()?;
        self.pic[0].irr &= !(1 << irq);
        self.pic[0].isr |= 1 << irq;
        if irq == 2 {
            if let Some(i2) = self.pic[1].pending() {
                self.pic[1].irr &= !(1 << i2);
                self.pic[1].isr |= 1 << i2;
                return Some(self.pic[1].base + i2);
            }
        }
        Some(self.pic[0].base + irq)
    }

    /// Non-specific EOI to the master (or slave) PIC.
    pub fn out_pic_eoi(&mut self, slave: bool) {
        self.pic[slave as usize].write(true, 0x20);
    }

    pub fn irq_pending(&self) -> bool {
        self.pic[0].pending().is_some()
    }

    /// Render sound up to `now`; raises the Sound Blaster IRQ when a block ends.
    pub fn audio_catch_up(&mut self, now: u64) {
        if self.audio.catch_up(now, &self.mem) {
            self.raise_irq(sound::SB_IRQ);
        }
    }

    /// Advance timers to `now_ns`; returns the next time something is due.
    pub fn update(&mut self, now_ns: u64) -> u64 {
        self.now_ns = now_ns;
        self.audio_catch_up(now_ns);
        let period = Pit::period_ns(self.pit.ch[0].reload).max(1);
        if now_ns >= self.pit.next_irq0_ns {
            self.raise_irq(0);
            // Catch up without flooding: one IRQ per update, skip missed ones.
            let behind = (now_ns - self.pit.next_irq0_ns) / period;
            self.pit.next_irq0_ns += (behind + 1) * period;
        }
        // Keyboard: present the next scancode once the previous was consumed.
        if !self.kbd.full && self.pic[0].irr & 2 == 0 && self.pic[0].isr & 2 == 0 {
            if let Some(sc) = self.kbd.queue.pop_front() {
                self.kbd.data = sc;
                self.kbd.full = true;
                self.raise_irq(1);
            }
        }
        self.reschedule = false;
        match self.audio.next_event_ns(now_ns) {
            Some(t) => t.min(self.pit.next_irq0_ns),
            None => self.pit.next_irq0_ns,
        }
    }

    /// Keyboard input waiting to be delivered.
    pub fn key_pending(&self) -> bool {
        self.kbd.full || !self.kbd.queue.is_empty()
    }

    /// Queue a raw set-1 scancode (make, or make|0x80 for break).
    pub fn key_scancode(&mut self, sc: u8) {
        if self.kbd.queue.len() < 64 {
            self.kbd.queue.push_back(sc);
        }
    }

    fn vga_status(&self) -> u8 {
        let t = self.now_ns % FRAME_NS;
        let vr = t < VRETRACE_NS;
        // Bit 0: display disabled (also during horizontal retrace; approximate
        // with vertical retrace or the odd 32 µs slice of each line).
        let hr = (self.now_ns / 32_000) % 2 == 1;
        (vr as u8) << 3 | (vr || hr) as u8
    }
}

impl Default for Hardware {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus for Hardware {
    #[inline]
    fn read8(&mut self, addr: u32) -> u8 {
        self.mem[addr as usize]
    }

    #[inline]
    fn write8(&mut self, addr: u32, value: u8) {
        // ROM area F0000-FFFFF is read-only (BIOS stubs live there).
        if !(0xf0000..0x100000).contains(&addr) {
            self.mem[addr as usize] = value;
        }
    }

    fn in8(&mut self, port: u16) -> u8 {
        let now = self.now_ns;
        if let Some(v) = self.audio.read(port, now) {
            if let Some(t) = &mut self.trace_ports {
                if !matches!(port, 0x388 | 0x38a) && t.len() < 20000 {
                    t.push((now, port, v, false));
                }
            }
            return v;
        }
        match port {
            0x20 | 0x21 => self.pic[0].read(port == 0x20),
            0xa0 | 0xa1 => self.pic[1].read(port == 0xa0),
            0x40..=0x42 => self.pit.read_data((port - 0x40) as usize, now),
            0x60 => {
                self.kbd.full = false;
                self.kbd.data
            }
            0x61 => {
                self.pit.refresh = !self.pit.refresh;
                self.pit.speaker_ctl & 0x0f | (self.pit.refresh as u8) << 4
            }
            0x64 => self.kbd.full as u8 | 0x14,
            0x201 => self.game_port,
            0x3c7 => 0,
            0x3c9 => {
                let v = self.vga.dac[self.vga.read_index as usize][self.vga.read_component as usize];
                self.vga.read_component += 1;
                if self.vga.read_component == 3 {
                    self.vga.read_component = 0;
                    self.vga.read_index = self.vga.read_index.wrapping_add(1);
                }
                v
            }
            0x3c6 => self.vga.pel_mask,
            0x3da | 0x3ba => self.vga_status(),
            _ => {
                if self.unknown_ports.len() < 256 && !self.unknown_ports.contains(&(port, false)) {
                    self.unknown_ports.push((port, false));
                }
                0xff
            }
        }
    }

    fn out8(&mut self, port: u16, v: u8) {
        let now = self.now_ns;
        if let Some(t) = &mut self.trace_ports {
            if matches!(port, 0x00..=0x0f | 0x20 | 0x21 | 0x81..=0x87 | 0x220..=0x22f | 0x388..=0x38b) && t.len() < 20000 {
                t.push((now, port, v, true));
            }
        }
        if matches!(port, 0x00..=0x0f | 0x81..=0x83 | 0x87 | 0x388..=0x38b | 0x220..=0x22f) {
            // Sound state changes take effect at this instant.
            self.audio_catch_up(now);
            if self.audio.write(port, v, now) {
                self.reschedule = true;
                return;
            }
        }
        match port {
            0x20 | 0x21 => self.pic[0].write(port == 0x20, v),
            0xa0 | 0xa1 => self.pic[1].write(port == 0xa0, v),
            0x40..=0x42 => self.pit.write_data((port - 0x40) as usize, v, now),
            0x43 => self.pit.write_ctl(v, now),
            0x61 => self.pit.speaker_ctl = v,
            0x201 => {} // start joystick one-shots: nothing connected
            0x3c6 => self.vga.pel_mask = v,
            0x3c7 => {
                self.vga.read_index = v;
                self.vga.read_component = 0;
            }
            0x3c8 => {
                self.vga.write_index = v;
                self.vga.component = 0;
            }
            0x3c9 => {
                let i = self.vga.write_index as usize;
                self.vga.dac[i][self.vga.component as usize] = v & 0x3f;
                self.vga.component += 1;
                if self.vga.component == 3 {
                    self.vga.component = 0;
                    self.vga.write_index = self.vga.write_index.wrapping_add(1);
                }
            }
            // CRTC / sequencer / GC / attribute: accepted, not modelled.
            0x3c0 | 0x3c4 | 0x3c5 | 0x3ce | 0x3cf | 0x3d4 | 0x3d5 | 0x3b4 | 0x3b5 | 0x3c2 => {}
            _ => {
                if self.unknown_ports.len() < 256 && !self.unknown_ports.contains(&(port, true)) {
                    self.unknown_ports.push((port, true));
                    if self.trace_ports.is_some() {
                        eprintln!("unknown OUT {port:03x} {v:02x} from {:04x}:{:04x}", self.trace_pc.0, self.trace_pc.1);
                    }
                }
            }
        }
    }
}

// ---- snapshots ----

use crate::state::{Reader, Result, Writer};

impl Pic {
    fn save(&self, w: &mut Writer) {
        for v in [self.irr, self.imr, self.isr, self.base, self.init_step] {
            w.u8(v);
        }
        w.bool(self.icw4);
        w.bool(self.read_isr);
    }
    fn load(&mut self, r: &mut Reader) -> Result<()> {
        self.irr = r.u8()?;
        self.imr = r.u8()?;
        self.isr = r.u8()?;
        self.base = r.u8()?;
        self.init_step = r.u8()?;
        self.icw4 = r.bool()?;
        self.read_isr = r.bool()?;
        Ok(())
    }
}

impl Pit {
    fn save(&self, w: &mut Writer) {
        for c in &self.ch {
            w.u32(c.reload);
            w.u8(c.mode);
            w.u8(c.access);
            w.bool(c.write_hi);
            w.bool(c.read_hi);
            w.bool(c.latch.is_some());
            w.u16(c.latch.unwrap_or(0));
            w.u8(c.pending_lo);
            w.u64(c.start_ns);
        }
        w.u64(self.next_irq0_ns);
        w.u8(self.speaker_ctl);
        w.bool(self.refresh);
    }
    fn load(&mut self, r: &mut Reader) -> Result<()> {
        for c in &mut self.ch {
            c.reload = r.u32()?;
            c.mode = r.u8()?;
            c.access = r.u8()?;
            c.write_hi = r.bool()?;
            c.read_hi = r.bool()?;
            let latched = r.bool()?;
            let l = r.u16()?;
            c.latch = latched.then_some(l);
            c.pending_lo = r.u8()?;
            c.start_ns = r.u64()?;
        }
        self.next_irq0_ns = r.u64()?;
        self.speaker_ctl = r.u8()?;
        self.refresh = r.bool()?;
        Ok(())
    }
}

impl Hardware {
    pub(crate) fn save(&self, w: &mut Writer) {
        w.bytes(&self.mem);
        self.pic[0].save(w);
        self.pic[1].save(w);
        self.pit.save(w);
        w.deque(&self.kbd.queue);
        w.u8(self.kbd.data);
        w.bool(self.kbd.full);
        for c in &self.vga.dac {
            w.u8(c[0]);
            w.u8(c[1]);
            w.u8(c[2]);
        }
        for v in [self.vga.write_index, self.vga.read_index, self.vga.component, self.vga.read_component, self.vga.mode, self.vga.pel_mask, self.game_port] {
            w.u8(v);
        }
        self.audio.save(w);
    }

    pub(crate) fn load(&mut self, r: &mut Reader) -> Result<()> {
        let mem = r.bytes()?;
        if mem.len() != self.mem.len() {
            return Err("snapshot memory size mismatch".into());
        }
        self.mem.copy_from_slice(mem);
        self.pic[0].load(r)?;
        self.pic[1].load(r)?;
        self.pit.load(r)?;
        self.kbd.queue = r.deque()?;
        self.kbd.data = r.u8()?;
        self.kbd.full = r.bool()?;
        for c in &mut self.vga.dac {
            *c = [r.u8()?, r.u8()?, r.u8()?];
        }
        self.vga.write_index = r.u8()?;
        self.vga.read_index = r.u8()?;
        self.vga.component = r.u8()?;
        self.vga.read_component = r.u8()?;
        self.vga.mode = r.u8()?;
        self.vga.pel_mask = r.u8()?;
        self.game_port = r.u8()?;
        self.audio.load(r)?;
        self.reschedule = true;
        Ok(())
    }
}
