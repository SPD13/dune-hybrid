//! Sound hardware: an OPL3 (AdLib) at 388h, a Sound Blaster Pro DSP at 220h
//! (IRQ 7, 8-bit DMA channel 1) and the 8237 DMA controller that feeds it.
//!
//! Audio is produced in virtual time: before any register write takes effect,
//! [`Audio::catch_up`] renders everything up to "now", so music and voices
//! keep their timing relative to the program no matter how fast the host is.

use std::collections::VecDeque;

use oplon::Opl2;

/// Host output rate.
pub const OUTPUT_RATE: u32 = 48_000;
const NS_PER_FRAME: f64 = 1e9 / OUTPUT_RATE as f64;

pub const SB_BASE: u16 = 0x220;
pub const SB_IRQ: u8 = 7;
pub const SB_DMA: usize = 1;

/// OPL3 register interface around the `oplon` synthesis core.
pub struct Opl {
    chip: Opl2,
    addr: [u16; 2],
    timer: [u8; 2],
    timer_start_ns: [Option<u64>; 2],
    timer_mask: u8,
    status: u8,
}

impl Opl {
    fn new() -> Self {
        Opl { chip: Opl2::new(OUTPUT_RATE), addr: [0; 2], timer: [0; 2], timer_start_ns: [None; 2], timer_mask: 0, status: 0 }
    }

    fn status(&mut self, now: u64) -> u8 {
        // Timer 1 ticks every 80 µs, timer 2 every 320 µs.
        for (i, period) in [(0usize, 80_000u64), (1, 320_000)] {
            if let Some(start) = self.timer_start_ns[i] {
                let len = (256 - self.timer[i] as u64) * period;
                if now >= start + len {
                    let flag = if i == 0 { 0x40 } else { 0x20 };
                    if self.timer_mask & flag == 0 {
                        self.status |= flag | 0x80;
                    }
                }
            }
        }
        // Low bits 0 identify an OPL3 (an OPL2 reads 06h).
        self.status
    }

    fn write_data(&mut self, bank: usize, v: u8, now: u64) {
        let reg = self.addr[bank];
        if bank == 0 {
            match reg {
                0x02 => self.timer[0] = v,
                0x03 => self.timer[1] = v,
                0x04 => {
                    if v & 0x80 != 0 {
                        self.status = 0;
                        return;
                    }
                    self.timer_mask = v & 0x60;
                    self.timer_start_ns[0] = (v & 1 != 0).then_some(now);
                    self.timer_start_ns[1] = (v & 2 != 0).then_some(now);
                }
                _ => self.chip.write_reg(reg as u8, v),
            }
        } else {
            self.chip.write_reg_high(reg as u8, v);
        }
    }
}

/// Intel 8237 DMA controller (channels 0-3, 8-bit).
#[derive(Default)]
pub struct Dma {
    base_addr: [u16; 4],
    base_count: [u16; 4],
    cur_addr: [u16; 4],
    cur_count: [u16; 4],
    page: [u8; 4],
    mode: [u8; 4],
    masked: [bool; 4],
    flip_flop: bool,
    reached_tc: [bool; 4],
}

impl Dma {
    fn new() -> Self {
        Dma { masked: [true; 4], ..Default::default() }
    }

    pub fn write(&mut self, port: u16, v: u8) {
        match port {
            0x00..=0x07 => {
                let ch = (port / 2) as usize;
                let is_count = port & 1 == 1;
                let (base, cur) = if is_count {
                    (&mut self.base_count[ch], &mut self.cur_count[ch])
                } else {
                    (&mut self.base_addr[ch], &mut self.cur_addr[ch])
                };
                *base = if self.flip_flop { (*base & 0x00ff) | (v as u16) << 8 } else { (*base & 0xff00) | v as u16 };
                *cur = *base;
                self.flip_flop = !self.flip_flop;
                self.reached_tc[ch] = false;
            }
            0x0a => self.masked[(v & 3) as usize] = v & 4 != 0,
            0x0b => self.mode[(v & 3) as usize] = v,
            0x0c => self.flip_flop = false,
            0x0d => {
                self.flip_flop = false;
                self.masked = [true; 4];
            }
            0x0f => {
                for c in 0..4 {
                    self.masked[c] = v & (1 << c) != 0;
                }
            }
            0x87 => self.page[0] = v,
            0x83 => self.page[1] = v,
            0x81 => self.page[2] = v,
            0x82 => self.page[3] = v,
            _ => {}
        }
    }

    pub fn read(&mut self, port: u16) -> u8 {
        match port {
            0x00..=0x07 => {
                let ch = (port / 2) as usize;
                let v = if port & 1 == 1 { self.cur_count[ch] } else { self.cur_addr[ch] };
                let b = if self.flip_flop { (v >> 8) as u8 } else { v as u8 };
                self.flip_flop = !self.flip_flop;
                b
            }
            0x08 => {
                let mut s = 0;
                for c in 0..4 {
                    if self.reached_tc[c] {
                        s |= 1 << c;
                    }
                }
                self.reached_tc = [false; 4];
                s
            }
            _ => 0xff,
        }
    }

    /// Read the next byte of `ch` from memory; None when masked or exhausted.
    fn next_byte(&mut self, ch: usize, mem: &[u8]) -> Option<u8> {
        if self.masked[ch] || self.reached_tc[ch] {
            return None;
        }
        let addr = (self.page[ch] as usize) << 16 | self.cur_addr[ch] as usize;
        let v = mem.get(addr).copied().unwrap_or(0);
        let down = self.mode[ch] & 0x20 != 0;
        self.cur_addr[ch] = if down { self.cur_addr[ch].wrapping_sub(1) } else { self.cur_addr[ch].wrapping_add(1) };
        if self.cur_count[ch] == 0 {
            // Terminal count: auto-init reloads, otherwise the channel stops.
            self.reached_tc[ch] = true;
            if self.mode[ch] & 0x10 != 0 {
                self.cur_addr[ch] = self.base_addr[ch];
                self.cur_count[ch] = self.base_count[ch];
                self.reached_tc[ch] = false;
            }
        } else {
            self.cur_count[ch] -= 1;
        }
        Some(v)
    }
}

/// Sound Blaster Pro DSP (8-bit DMA playback) and mixer.
pub struct SoundBlaster {
    reset_latch: bool,
    out: VecDeque<u8>,
    cmd: Option<u8>,
    args: Vec<u8>,
    time_constant: u8,
    block_len: u16,
    speaker: bool,
    /// Active DMA transfer: bytes left in the block, auto-init?
    dma_left: u32,
    auto_init: bool,
    active: bool,
    paused: bool,
    /// Fractional source-sample position for resampling.
    phase: f64,
    last: [f32; 2],
    irq_pending: bool,
    mixer_index: u8,
    mixer: [u8; 256],
}

impl SoundBlaster {
    fn new() -> Self {
        let mut mixer = [0u8; 256];
        mixer[0x22] = 0xff;
        mixer[0x04] = 0xff;
        SoundBlaster {
            reset_latch: false,
            out: VecDeque::new(),
            cmd: None,
            args: Vec::new(),
            time_constant: 0xa5,
            block_len: 0x7ff,
            speaker: false,
            dma_left: 0,
            auto_init: false,
            active: false,
            paused: false,
            phase: 0.0,
            last: [0.0; 2],
            irq_pending: false,
            mixer_index: 0,
            mixer,
        }
    }

    fn stereo(&self) -> bool {
        self.mixer[0x0e] & 0x02 != 0
    }

    /// Source byte rate (both channels together in stereo).
    fn byte_rate(&self) -> f64 {
        1_000_000.0 / (256.0 - self.time_constant as f64)
    }

    fn command(&mut self, v: u8) {
        if let Some(c) = self.cmd {
            self.args.push(v);
            let need = match c {
                0x40 | 0x10 | 0xe0 => 1,
                0x14 | 0x16 | 0x17 | 0x24 | 0x48 | 0x80 | 0x91 => 2,
                _ => 0,
            };
            if self.args.len() < need {
                return;
            }
            let a = std::mem::take(&mut self.args);
            self.cmd = None;
            match c {
                0x40 => self.time_constant = a[0],
                0x14 | 0x91 => {
                    self.dma_left = u16::from_le_bytes([a[0], a[1]]) as u32 + 1;
                    self.auto_init = false;
                    self.active = true;
                    self.paused = false;
                }
                0x48 => self.block_len = u16::from_le_bytes([a[0], a[1]]),
                0xe0 => self.out.push_back(!a[0]),
                0x80 => {
                    // Silence block: just the IRQ.
                    self.irq_pending = true;
                }
                _ => {}
            }
            return;
        }
        match v {
            0x1c | 0x90 => {
                self.dma_left = self.block_len as u32 + 1;
                self.auto_init = true;
                self.active = true;
                self.paused = false;
            }
            0xd0 => self.paused = true,
            0xd4 => self.paused = false,
            0xda => self.auto_init = false,
            0xd1 => self.speaker = true,
            0xd3 => self.speaker = false,
            0xd8 => self.out.push_back(if self.speaker { 0xff } else { 0 }),
            0xe1 => {
                self.out.push_back(3);
                self.out.push_back(2);
            }
            0xf2 => self.irq_pending = true,
            0x40 | 0x10 | 0xe0 | 0x14 | 0x16 | 0x17 | 0x24 | 0x48 | 0x80 | 0x91 => self.cmd = Some(v),
            _ => {}
        }
    }
}

/// All sound devices plus the output buffer the host drains.
pub struct Audio {
    pub opl: Opl,
    pub sb: SoundBlaster,
    pub dma: Dma,
    /// Interleaved stereo f32 at [`OUTPUT_RATE`].
    pub out: Vec<f32>,
    frames_rendered: u64,
    /// Silence output when the host does not want audio (saves time).
    pub enabled: bool,
}

impl Audio {
    pub fn new() -> Self {
        Audio { opl: Opl::new(), sb: SoundBlaster::new(), dma: Dma::new(), out: Vec::new(), frames_rendered: 0, enabled: true }
    }

    /// Render output frames up to virtual time `now`. Returns true when the
    /// SB finished a block (the caller raises its IRQ).
    pub fn catch_up(&mut self, now: u64, mem: &[u8]) -> bool {
        let target = (now as f64 / NS_PER_FRAME) as u64;
        if target <= self.frames_rendered {
            return false;
        }
        let n = (target - self.frames_rendered).min(OUTPUT_RATE as u64);
        self.frames_rendered = target;
        let mut irq = false;
        let sb_step = self.sb.byte_rate() / OUTPUT_RATE as f64 / if self.sb.stereo() { 2.0 } else { 1.0 };
        for _ in 0..n {
            let (l, r) = self.opl.chip.render_frame();
            let (mut fl, mut fr) = (l as f32 / 32768.0, r as f32 / 32768.0);
            // Sound Blaster: step through DMA bytes at the DSP rate.
            if self.sb.active && !self.sb.paused {
                self.sb.phase += sb_step;
                while self.sb.phase >= 1.0 {
                    self.sb.phase -= 1.0;
                    let chans = if self.sb.stereo() { 2 } else { 1 };
                    for c in 0..chans {
                        match self.dma.next_byte(SB_DMA, mem) {
                            Some(b) => {
                                let s = (b as f32 - 128.0) / 128.0;
                                self.sb.last[c] = s;
                                if chans == 1 {
                                    self.sb.last[1] = s;
                                }
                            }
                            None => {
                                self.sb.last = [0.0; 2];
                            }
                        }
                        self.sb.dma_left = self.sb.dma_left.saturating_sub(1);
                    }
                    if self.sb.dma_left == 0 {
                        irq = true;
                        if self.sb.auto_init {
                            self.sb.dma_left = self.sb.block_len as u32 + 1;
                        } else {
                            self.sb.active = false;
                            self.sb.last = [0.0; 2];
                            break;
                        }
                    }
                }
            }
            if self.sb.speaker {
                fl += self.sb.last[0] * 0.5;
                fr += self.sb.last[1] * 0.5;
            }
            if self.enabled {
                self.out.push(fl);
                self.out.push(fr);
            }
        }
        if self.sb.irq_pending {
            self.sb.irq_pending = false;
            irq = true;
        }
        irq
    }

    /// When the Sound Blaster next needs servicing (end of its DMA block or
    /// a pending IRQ), so the machine can wake up exactly then.
    pub fn next_event_ns(&self, now: u64) -> Option<u64> {
        if self.sb.irq_pending {
            return Some(now);
        }
        if self.sb.active && !self.sb.paused {
            let secs = self.sb.dma_left as f64 / self.sb.byte_rate();
            return Some(now + (secs * 1e9) as u64 + NS_PER_FRAME as u64);
        }
        None
    }

    /// Port reads handled by the sound devices.
    pub fn read(&mut self, port: u16, now: u64) -> Option<u8> {
        Some(match port {
            0x388 | 0x38a => self.opl.status(now),
            0x389 | 0x38b => 0xff,
            0x00..=0x0f => self.dma.read(port),
            p if p == SB_BASE + 0xa => self.sb.out.pop_front().unwrap_or(0xff),
            p if p == SB_BASE + 0xc => 0x7f, // write buffer ready
            p if p == SB_BASE + 0xe => {
                // Data available; reading acknowledges the 8-bit IRQ.
                if self.sb.out.is_empty() { 0x7f } else { 0xff }
            }
            p if p == SB_BASE + 0x5 => self.sb.mixer[self.sb.mixer_index as usize],
            p if p == SB_BASE + 0x6 => 0xff,
            _ => return None,
        })
    }

    /// Port writes handled by the sound devices (after `catch_up`).
    pub fn write(&mut self, port: u16, v: u8, now: u64) -> bool {
        match port {
            0x388 => self.opl.addr[0] = v as u16,
            0x38a => self.opl.addr[1] = v as u16,
            0x389 => self.opl.write_data(0, v, now),
            0x38b => self.opl.write_data(1, v, now),
            0x00..=0x0f | 0x81..=0x83 | 0x87 => self.dma.write(port, v),
            p if p == SB_BASE + 0x6 => {
                if v & 1 != 0 {
                    self.sb.reset_latch = true;
                } else if self.sb.reset_latch {
                    self.sb.reset_latch = false;
                    let mixer = self.sb.mixer;
                    self.sb = SoundBlaster::new();
                    self.sb.mixer = mixer;
                    self.sb.out.push_back(0xaa);
                }
            }
            p if p == SB_BASE + 0xc => self.sb.command(v),
            p if p == SB_BASE + 0x4 => self.sb.mixer_index = v,
            p if p == SB_BASE + 0x5 => self.sb.mixer[self.sb.mixer_index as usize] = v,
            _ => return false,
        }
        true
    }
}

impl Default for Audio {
    fn default() -> Self {
        Self::new()
    }
}
