//! BIOS: interrupt vector table, ROM stubs, BIOS data area, and the
//! INT 08h/09h/10h/15h/16h/1Ah/33h services.

use cpu::{AX, BX, CX, DX, ES};

use crate::{DOS_DATA_SEG, Machine, STUB_BASE, STUB_SEG};

const BDA: u16 = 0x0040;

/// Stub offset of interrupt `n`.
pub(crate) fn stub_off(n: u8) -> u16 {
    STUB_BASE + n as u16 * 8
}

fn write_stub(m: &mut Machine, n: u8, code: &[u8]) {
    let off = stub_off(n);
    for (i, &b) in code.iter().enumerate() {
        let a = cpu::linear(STUB_SEG, off + i as u16) as usize;
        m.hw.mem[a] = b;
    }
}

pub(crate) fn install(m: &mut Machine) {
    for n in 0..=255u8 {
        // Default: just IRET.
        let code: &[u8] = match n {
            // Timer: tick, then chain INT 1Ch like the real BIOS.
            0x08 => &[0xfe, 0x38, 0x08, 0xcd, 0x1c, 0xcf],
            0x09 | 0x0a..=0x0f | 0x70..=0x77 | 0x10..=0x12 | 0x15 | 0x16 | 0x1a | 0x21 | 0x2f | 0x33 | 0x67 | 0x00 | 0x06 => {
                &[0xfe, 0x38, n, 0xcf]
            }
            _ => &[0xcf],
        };
        write_stub(m, n, code);
        let vec = n as usize * 4;
        let off = stub_off(n);
        m.hw.mem[vec..vec + 2].copy_from_slice(&off.to_le_bytes());
        m.hw.mem[vec + 2..vec + 4].copy_from_slice(&STUB_SEG.to_le_bytes());
    }
    // BIOS model byte (AT) and date.
    m.hw.mem[0xffffe] = 0xfc;
    // BIOS data area.
    m.mem_write16(BDA, 0x10, 0x0021); // equipment
    m.mem_write16(BDA, 0x13, 640); // memory KB
    m.mem_write(BDA, 0x49, 0x03); // video mode
    m.mem_write16(BDA, 0x4a, 80); // columns
    m.mem_write16(BDA, 0x63, 0x3d4); // CRTC base
    m.mem_write(BDA, 0x84, 24); // rows - 1
    m.mem_write16(BDA, 0x85, 16); // char height
    // Keyboard buffer pointers (empty).
    m.mem_write16(BDA, 0x1a, 0x1e);
    m.mem_write16(BDA, 0x1c, 0x1e);
    m.mem_write16(BDA, 0x80, 0x1e);
    m.mem_write16(BDA, 0x82, 0x3e);
    // InDOS flag lives in DOS data; zero.
    m.mem_write(DOS_DATA_SEG, 0, 0);
    // Default VGA DAC: EGA colours then a grey ramp.
    const EGA: [[u8; 3]; 16] = [
        [0, 0, 0], [0, 0, 42], [0, 42, 0], [0, 42, 42], [42, 0, 0], [42, 0, 42], [42, 21, 0], [42, 42, 42],
        [21, 21, 21], [21, 21, 63], [21, 63, 21], [21, 63, 63], [63, 21, 21], [63, 21, 63], [63, 63, 21], [63, 63, 63],
    ];
    for i in 0..256 {
        m.hw.vga.dac[i] = if i < 16 { EGA[i] } else { [((i - 16) / 4) as u8; 3] };
    }
}

pub(crate) fn eoi(m: &mut Machine, n: u8) {
    if n >= 0x70 {
        m.hw.out_pic_eoi(true);
    }
    m.hw.out_pic_eoi(false);
}

/// INT 08h: BIOS tick counter at 0040:006C, EOI, then (in the stub) INT 1Ch.
pub(crate) fn timer_tick(m: &mut Machine) {
    let lo = m.mem_read16(BDA, 0x6c);
    let hi = m.mem_read16(BDA, 0x6e);
    let mut t = (hi as u32) << 16 | lo as u32;
    t += 1;
    if t >= 0x1800b0 {
        t = 0;
        m.mem_write(BDA, 0x70, 1);
    }
    m.mem_write16(BDA, 0x6c, t as u16);
    m.mem_write16(BDA, 0x6e, (t >> 16) as u16);
    m.hw.out_pic_eoi(false);
}

/// Set-1 make code to ASCII (unshifted), for the BIOS buffer.
fn scancode_ascii(sc: u8) -> u8 {
    const ROW: &[u8] = b"\x001234567890-=\x08\tqwertyuiop[]\r\x00asdfghjkl;'`\x00\\zxcvbnm,./\x00*\x00 ";
    match sc {
        0x01 => 0x1b,
        1..=0x39 => ROW.get(sc as usize - 1).copied().unwrap_or(0),
        _ => 0,
    }
}

/// INT 09h: read the scancode and queue make codes for INT 16h.
pub(crate) fn keyboard_irq(m: &mut Machine) {
    let sc = cpu::Bus::in8(&mut m.hw, 0x60);
    if sc & 0x80 == 0 && m.bios_keys.len() < 16 {
        m.bios_keys.push_back((sc as u16) << 8 | scancode_ascii(sc) as u16);
    }
    m.hw.out_pic_eoi(false);
}

pub(crate) fn int10(m: &mut Machine) {
    match m.ah() {
        0x00 => {
            let mode = m.al() & 0x7f;
            let clear = m.al() & 0x80 == 0;
            m.hw.vga.mode = mode;
            m.mem_write(BDA, 0x49, mode);
            if mode == 0x13 {
                m.mem_write16(BDA, 0x4a, 40);
                if clear {
                    m.hw.mem[0xa0000..0xb0000].fill(0);
                }
            } else {
                m.mem_write16(BDA, 0x4a, 80);
                if clear {
                    for i in (0xb8000..0xb8000 + 4000).step_by(2) {
                        m.hw.mem[i] = b' ';
                        m.hw.mem[i + 1] = 7;
                    }
                }
            }
        }
        0x0f => {
            let cols = m.mem_read(BDA, 0x4a);
            m.cpu.regs[AX] = (cols as u16) << 8 | m.hw.vga.mode as u16;
            m.cpu.set_reg8(7, 0); // BH = page
        }
        0x0e => {
            let c = m.al();
            m.console.push(c as char);
        }
        0x10 => match m.al() {
            0x10 => {
                let i = (m.cpu.regs[BX] & 0xff) as usize;
                m.hw.vga.dac[i] = [m.cpu.reg8(6) & 0x3f, m.cpu.reg8(5) & 0x3f, m.cpu.reg8(1) & 0x3f];
            }
            0x12 => {
                let (start, count) = (m.cpu.regs[BX] as usize, m.cpu.regs[CX] as usize);
                let (seg, mut off) = (m.cpu.sregs[ES], m.cpu.regs[DX]);
                for i in 0..count {
                    let mut c = [0u8; 3];
                    for v in &mut c {
                        *v = m.mem_read(seg, off) & 0x3f;
                        off = off.wrapping_add(1);
                    }
                    m.hw.vga.dac[(start + i) & 0xff] = c;
                }
            }
            0x15 => {
                let c = m.hw.vga.dac[(m.cpu.regs[BX] & 0xff) as usize];
                m.cpu.set_reg8(6, c[0]);
                m.cpu.set_reg8(5, c[1]);
                m.cpu.set_reg8(1, c[2]);
            }
            0x17 => {
                let (start, count) = (m.cpu.regs[BX] as usize, m.cpu.regs[CX] as usize);
                let (seg, mut off) = (m.cpu.sregs[ES], m.cpu.regs[DX]);
                for i in 0..count {
                    for k in 0..3 {
                        let v = m.hw.vga.dac[(start + i) & 0xff][k];
                        m.mem_write(seg, off, v);
                        off = off.wrapping_add(1);
                    }
                }
            }
            _ => {}
        },
        0x1a => {
            if m.al() == 0 {
                m.set_al(0x1a);
                m.cpu.regs[BX] = 0x0008;
            }
        }
        0x12 => {
            if m.cpu.reg8(3) == 0x10 {
                m.cpu.regs[BX] = 0x0003;
                m.cpu.regs[CX] = 0;
            }
        }
        0x01 | 0x02 | 0x03 | 0x05 | 0x06 | 0x07 | 0x0b | 0x11 => {}
        ah => m.log(format!("INT 10h AH={ah:02x} unhandled")),
    }
}

pub(crate) fn int15(m: &mut Machine) {
    match m.ah() {
        0x4f => m.set_cf(true), // keyboard intercept: pass the key through
        0x88 => {
            m.cpu.regs[AX] = 0;
            m.set_cf(false);
        }
        0x90 | 0x91 => {
            m.set_ah(0);
            m.set_cf(false);
        }
        ah => {
            m.log(format!("INT 15h AH={ah:02x} unsupported"));
            m.set_ah(0x86);
            m.set_cf(true);
        }
    }
}

pub(crate) fn int16(m: &mut Machine) {
    match m.ah() {
        0x00 | 0x10 => match m.bios_keys.pop_front() {
            Some(k) => m.cpu.regs[AX] = k,
            None => {
                // Block: re-execute the INT 16h when the stub returns.
                let sp = m.cpu.regs[cpu::SP];
                let ip = m.cpu.read16(&mut m.hw, cpu::SS, sp);
                m.cpu.write16(&mut m.hw, cpu::SS, sp, ip.wrapping_sub(2));
            }
        },
        0x01 | 0x11 => match m.bios_keys.front() {
            Some(&k) => {
                m.cpu.regs[AX] = k;
                m.cpu.set_flag(cpu::ZF, false);
            }
            None => m.cpu.set_flag(cpu::ZF, true),
        },
        0x02 | 0x12 => m.set_al(0),
        _ => {}
    }
}

pub(crate) fn int1a(m: &mut Machine) {
    let secs = m.now_ns() / 1_000_000_000 + 12 * 3600;
    let bcd = |v: u64| (((v / 10) << 4) | (v % 10)) as u8;
    match m.ah() {
        0x00 => {
            m.cpu.regs[CX] = m.mem_read16(BDA, 0x6e);
            m.cpu.regs[DX] = m.mem_read16(BDA, 0x6c);
            let midnight = m.mem_read(BDA, 0x70);
            m.mem_write(BDA, 0x70, 0);
            m.set_al(midnight);
        }
        0x02 => {
            m.cpu.set_reg8(5, bcd(secs / 3600 % 24));
            m.cpu.set_reg8(1, bcd(secs / 60 % 60));
            m.cpu.set_reg8(6, bcd(secs % 60));
            m.cpu.set_reg8(2, 0);
            m.set_cf(false);
        }
        0x04 => {
            m.cpu.regs[CX] = 0x1993;
            m.cpu.regs[DX] = 0x0101;
            m.set_cf(false);
        }
        _ => m.set_cf(true),
    }
}

/// INT 33h: a Microsoft-compatible mouse driver fed by the host.
pub(crate) fn int33(m: &mut Machine) {
    let ax = m.cpu.regs[AX];
    match ax {
        0x00 | 0x21 => {
            m.cpu.regs[AX] = 0xffff;
            m.cpu.regs[BX] = 2;
            m.mouse.visible = -1;
            m.mouse.min_x = 0;
            m.mouse.max_x = 639;
            m.mouse.min_y = 0;
            m.mouse.max_y = 199;
        }
        0x01 => m.mouse.visible += 1,
        0x02 => m.mouse.visible -= 1,
        0x03 => {
            m.cpu.regs[BX] = m.mouse.buttons;
            m.cpu.regs[CX] = m.mouse.x as u16;
            m.cpu.regs[DX] = m.mouse.y as u16;
        }
        0x04 => {
            let mm = &mut m.mouse;
            mm.x = (m.cpu.regs[CX] as i16 as i32).clamp(mm.min_x, mm.max_x.max(mm.min_x));
            mm.y = (m.cpu.regs[DX] as i16 as i32).clamp(mm.min_y, mm.max_y.max(mm.min_y));
        }
        0x07 => {
            let (a, b) = (m.cpu.regs[CX] as i16 as i32, m.cpu.regs[DX] as i16 as i32);
            m.mouse.min_x = a.min(b);
            m.mouse.max_x = a.max(b);
            m.mouse.x = m.mouse.x.clamp(m.mouse.min_x, m.mouse.max_x);
        }
        0x08 => {
            let (a, b) = (m.cpu.regs[CX] as i16 as i32, m.cpu.regs[DX] as i16 as i32);
            m.mouse.min_y = a.min(b);
            m.mouse.max_y = a.max(b);
            m.mouse.y = m.mouse.y.clamp(m.mouse.min_y, m.mouse.max_y);
        }
        0x0f | 0x13 | 0x1a | 0x1d => {}
        0x24 => {
            m.cpu.regs[BX] = 0x0626;
            m.cpu.regs[CX] = 0x0400;
        }
        _ => m.log(format!("INT 33h AX={ax:04x} unhandled")),
    }
}
