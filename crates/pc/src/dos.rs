//! DOS: the MZ loader, PSP/environment, and the INT 21h/2Fh services the game
//! uses (files, vectors, drive queries, exit).

use std::io::{Read, Seek, SeekFrom, Write};

use cpu::{AX, BX, CS, CX, DI, DS, DX, ES, SI, SP, SS};

use crate::{DOS_DATA_SEG, ENV_SEG, LOAD_SEG, MEM_TOP_SEG, Machine, PSP_SEG, fs};

pub(crate) fn load_exe(m: &mut Machine, exe: &[u8], cmdline: &str) -> Result<(), String> {
    let w = |o: usize| u16::from_le_bytes([exe[o], exe[o + 1]]);
    if exe.len() < 0x1c || &exe[0..2] != b"MZ" {
        return Err("not an MZ executable".into());
    }
    let (last, pages, nrel, hdr) = (w(2) as usize, w(4) as usize, w(6) as usize, w(8) as usize * 16);
    let mut size = pages * 512;
    if last != 0 {
        size -= 512 - last;
    }
    let image = exe.get(hdr..size.min(exe.len())).ok_or("truncated image")?;
    let base = LOAD_SEG as usize * 16;
    m.hw.mem[base..base + image.len()].copy_from_slice(image);
    let reloc = w(0x18) as usize;
    for i in 0..nrel {
        let (off, seg) = (w(reloc + i * 4), w(reloc + i * 4 + 2));
        let a = cpu::linear(LOAD_SEG.wrapping_add(seg), off) as usize;
        let v = u16::from_le_bytes([m.hw.mem[a], m.hw.mem[a + 1]]).wrapping_add(LOAD_SEG);
        m.hw.mem[a..a + 2].copy_from_slice(&v.to_le_bytes());
    }

    // Environment: variables, then the program path.
    let mut env = b"PATH=C:\\\0COMSPEC=C:\\COMMAND.COM\0\0".to_vec();
    env.extend_from_slice(&1u16.to_le_bytes());
    env.extend_from_slice(b"C:\\DUNE\\DNCDPRG.EXE\0");
    let eb = ENV_SEG as usize * 16;
    m.hw.mem[eb..eb + env.len()].copy_from_slice(&env);

    // PSP.
    let p = PSP_SEG;
    m.mem_write16(p, 0x00, 0x20cd);
    m.mem_write16(p, 0x02, MEM_TOP_SEG);
    m.mem_write16(p, 0x2c, ENV_SEG);
    m.mem_write16(p, 0x16, PSP_SEG); // parent
    for i in 0..20u16 {
        m.mem_write(p, 0x18 + i, if i < 5 { i as u8 } else { 0xff });
    }
    m.mem_write16(p, 0x32, 20);
    m.mem_write16(p, 0x34, 0x18);
    m.mem_write16(p, 0x36, p);
    m.mem_write(p, 0x50, 0xcd);
    m.mem_write(p, 0x51, 0x21);
    m.mem_write(p, 0x52, 0xcb);
    let tail = if cmdline.is_empty() { String::new() } else { format!(" {cmdline}") };
    let tail = &tail.as_bytes()[..tail.len().min(126)];
    m.mem_write(p, 0x80, tail.len() as u8);
    for (i, &c) in tail.iter().enumerate() {
        m.mem_write(p, 0x81 + i as u16, c);
    }
    m.mem_write(p, 0x81 + tail.len() as u16, 0x0d);

    let c = &mut m.cpu;
    c.sregs[CS] = LOAD_SEG.wrapping_add(w(0x16));
    c.ip = w(0x14);
    c.sregs[SS] = LOAD_SEG.wrapping_add(w(0x0e));
    c.regs[SP] = w(0x10);
    c.sregs[DS] = PSP_SEG;
    c.sregs[ES] = PSP_SEG;
    c.set_flags(0x0202);
    Ok(())
}

fn error(m: &mut Machine, code: u16) {
    m.cpu.regs[AX] = code;
    m.set_cf(true);
}

fn alloc_handle(m: &mut Machine, name: &str, f: Box<dyn fs::DosFile>) -> Option<u16> {
    let slot = m.files.iter().skip(5).position(|f| f.is_none()).map(|i| i + 5);
    let h = match slot {
        Some(h) => h,
        None if m.files.len() < 40 => {
            m.files.push(None);
            m.files.len() - 1
        }
        None => return None,
    };
    m.files[h] = Some(crate::OpenFile { name: name.to_string(), f });
    Some(h as u16)
}

pub(crate) fn int21(m: &mut Machine) {
    let ah = m.ah();
    m.set_cf(false);
    match ah {
        0x02 => {
            let c = m.cpu.reg8(2);
            m.console.push(c as char);
        }
        0x06 => {
            // Direct console I/O: input requests report "no character".
            if m.cpu.reg8(2) == 0xff {
                m.cpu.set_flag(cpu::ZF, true);
                m.set_al(0);
            } else {
                let c = m.cpu.reg8(2);
                m.console.push(c as char);
            }
        }
        0x09 => {
            let (seg, mut off) = (m.cpu.sregs[DS], m.cpu.regs[DX]);
            for _ in 0..1024 {
                let c = m.mem_read(seg, off);
                if c == b'$' {
                    break;
                }
                m.console.push(c as char);
                off = off.wrapping_add(1);
            }
        }
        0x0b => m.set_al(0),
        0x0c => {
            // Flush the type-ahead, then run the sub-function in AL.
            m.bios_keys.clear();
            let al = m.al();
            if matches!(al, 0x06 | 0x07 | 0x08 | 0x0a) {
                m.set_ah(al);
                int21(m);
            }
        }
        0x0e => m.set_al(3),
        0x19 => m.set_al(2),
        0x1a => m.dta = (m.cpu.sregs[DS], m.cpu.regs[DX]),
        0x25 => {
            let v = m.al() as u16 * 4;
            let (off, seg) = (m.cpu.regs[DX], m.cpu.sregs[DS]);
            m.mem_write16(0, v, off);
            m.mem_write16(0, v + 2, seg);
        }
        0x2a => {
            m.cpu.regs[CX] = 1993;
            m.cpu.regs[DX] = 0x0101;
            m.set_al(5);
        }
        0x2c => {
            let s = m.now_ns() / 10_000_000;
            let secs = s / 100 + 12 * 3600;
            m.cpu.regs[CX] = ((secs / 3600 % 24) as u16) << 8 | (secs / 60 % 60) as u16;
            m.cpu.regs[DX] = ((secs % 60) as u16) << 8 | (s % 100) as u16;
        }
        0x2f => {
            m.cpu.sregs[ES] = m.dta.0;
            m.cpu.regs[BX] = m.dta.1;
        }
        0x30 => {
            m.cpu.regs[AX] = 0x0005;
            m.cpu.regs[BX] = 0;
            m.cpu.regs[CX] = 0;
        }
        0x33 => match m.al() {
            0x00 => m.cpu.set_reg8(2, 0),
            0x01 => {}
            0x05 => m.cpu.set_reg8(2, 3),
            _ => {}
        },
        0x34 => {
            m.cpu.sregs[ES] = DOS_DATA_SEG;
            m.cpu.regs[BX] = 0;
        }
        0x35 => {
            let v = m.al() as u16 * 4;
            m.cpu.regs[BX] = m.mem_read16(0, v);
            m.cpu.sregs[ES] = m.mem_read16(0, v + 2);
        }
        0x3b => {}
        0x3c | 0x3d => {
            let name = fs::normalize(&m.read_asciiz(m.cpu.sregs[DS], m.cpu.regs[DX]));
            let f = if ah == 0x3c { m.fs.create(&name) } else { m.fs.open(&name) };
            match f {
                Some(f) => match alloc_handle(m, &name, f) {
                    Some(h) => {
                        m.log(format!("open {name} -> {h}"));
                        m.cpu.regs[AX] = h;
                    }
                    None => error(m, 4),
                },
                None => {
                    m.log(format!("open {name}: not found"));
                    error(m, if ah == 0x3c { 3 } else { 2 });
                }
            }
        }
        0x3e => {
            let h = m.cpu.regs[BX] as usize;
            match m.files.get_mut(h).and_then(|f| f.take()) {
                Some(mut of) => of.f.close(),
                None if h < 5 => {}
                None => error(m, 6),
            }
        }
        0x3f => {
            let (h, count) = (m.cpu.regs[BX] as usize, m.cpu.regs[CX] as usize);
            let (seg, off) = (m.cpu.sregs[DS], m.cpu.regs[DX]);
            let Some(Some(crate::OpenFile { f, .. })) = m.files.get_mut(h) else {
                if h < 5 {
                    m.cpu.regs[AX] = 0;
                } else {
                    error(m, 6);
                }
                return;
            };
            let mut buf = vec![0u8; count];
            let mut n = 0;
            while n < count {
                match f.read(&mut buf[n..]) {
                    Ok(0) => break,
                    Ok(k) => n += k,
                    Err(_) => break,
                }
            }
            for (i, &b) in buf[..n].iter().enumerate() {
                m.mem_write(seg, off.wrapping_add(i as u16), b);
            }
            m.cpu.regs[AX] = n as u16;
        }
        0x40 => {
            let (h, count) = (m.cpu.regs[BX] as usize, m.cpu.regs[CX] as usize);
            let (seg, off) = (m.cpu.sregs[DS], m.cpu.regs[DX]);
            let data: Vec<u8> = (0..count).map(|i| m.mem_read(seg, off.wrapping_add(i as u16))).collect();
            if h == 1 || h == 2 {
                m.console.push_str(&String::from_utf8_lossy(&data));
                m.cpu.regs[AX] = count as u16;
                return;
            }
            match m.files.get_mut(h) {
                Some(Some(crate::OpenFile { f, .. })) => {
                    let n = f.write(&data).unwrap_or(0);
                    m.cpu.regs[AX] = n as u16;
                }
                _ => error(m, 6),
            }
        }
        0x41 => {}
        0x42 => {
            let h = m.cpu.regs[BX] as usize;
            let off = ((m.cpu.regs[CX] as u32) << 16 | m.cpu.regs[DX] as u32) as i32 as i64;
            let whence = match m.al() {
                0 => SeekFrom::Start(off as u64),
                1 => SeekFrom::Current(off),
                _ => SeekFrom::End(off),
            };
            match m.files.get_mut(h) {
                Some(Some(crate::OpenFile { f, .. })) => match f.seek(whence) {
                    Ok(p) => {
                        m.cpu.regs[AX] = p as u16;
                        m.cpu.regs[DX] = (p >> 16) as u16;
                    }
                    Err(_) => error(m, 25),
                },
                _ => error(m, 6),
            }
        }
        0x44 => match m.al() {
            0x00 => {
                let h = m.cpu.regs[BX];
                m.cpu.regs[DX] = if h < 5 { 0x80d3 } else { 0x0002 };
            }
            _ => m.cpu.regs[AX] = 0,
        },
        0x47 => {
            let (seg, off) = (m.cpu.sregs[DS], m.cpu.regs[SI]);
            m.mem_write(seg, off, 0);
        }
        0x48 => {
            m.cpu.regs[BX] = 0;
            error(m, 8);
        }
        0x49 | 0x4a => {}
        0x4c => {
            let code = m.al();
            m.close_all();
            m.exited = Some(code);
        }
        0x4e | 0x4f => error(m, 18),
        0x62 | 0x51 => m.cpu.regs[BX] = PSP_SEG,
        _ => {
            m.log(format!("INT 21h AH={ah:02x} unhandled ({})", m.regs_string()));
            error(m, 1);
        }
    }
    let _ = DI;
}

pub(crate) fn int2f(m: &mut Machine) {
    match m.cpu.regs[AX] {
        0x4300 => m.set_al(0),   // no XMS driver
        0x1500 => m.cpu.regs[BX] = 0, // no MSCDEX
        0x1687 => {}            // no DPMI (AX unchanged, non-zero)
        ax => m.log(format!("INT 2Fh AX={ax:04x} ignored")),
    }
}
