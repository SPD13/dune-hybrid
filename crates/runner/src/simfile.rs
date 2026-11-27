//! The simulator's world from the game's files (no emulator): a save file
//! for the state it keeps, and what the game sets up at its start
//! (`initialize_resources`, 00D1h) for the rest: the latitude table from
//! TABLAT.BIN (its words byte-swapped, at DS:4948h), the steps' scales
//! computed from it (DS:4880h), the map's cells from MAP.HSQ (the save's
//! states in their bits 4..5; the rows' base [0DCFEh] 62FCh past the
//! map's start) and the growth seed from the code (CS:65B4h). Resource
//! numbers go to names through the table at DS:31FFh of DNCDPRG.EXE.

use std::fs::File;
use std::path::Path;

use dune_rec::hand::save_file::SaveImage;
use dune_rec::hand::world::{EXTRAS, World};

/// The EXE's load image (past its 200h-byte header) and where DS:0 is in
/// it (the data segment F4Bh paragraphs in).
struct Exe {
    image: Vec<u8>,
}

impl Exe {
    const DS: usize = 0xf4b0;
    fn ds_word(&self, off: u16) -> u16 {
        let k = Self::DS + off as usize;
        self.image[k] as u16 | (self.image[k + 1] as u16) << 8
    }
    fn cs_word(&self, off: u16) -> u16 {
        let k = off as usize;
        self.image[k] as u16 | (self.image[k + 1] as u16) << 8
    }
    /// Resource `n`'s file name (the table at DS:31FFh: a size word, then
    /// the name).
    fn resource_name(&self, n: u16) -> String {
        let p = self.ds_word(0x31ff + 2 * n) as usize + 2;
        self.image[Self::DS + p..].iter().take_while(|&&b| b != 0).map(|&b| b as char).collect()
    }
}

fn resource(dir: &Path, exe: &Exe, n: u16) -> Result<Vec<u8>, String> {
    let name = exe.resource_name(n);
    let mut f = File::open(dir.join("DUNE.DAT")).map_err(|e| format!("DUNE.DAT: {e}"))?;
    let toc = gfx::dat::toc(&mut f).map_err(|e| format!("DUNE.DAT: {e}"))?;
    let e = toc.iter().find(|e| e.name.eq_ignore_ascii_case(&name)).ok_or(format!("{name} not in DUNE.DAT"))?;
    gfx::dat::load(&mut f, e).map_err(|e| format!("{name}: {e}"))
}

/// The world of the save file `save` in the game directory `dir`.
pub fn world(dir: &Path, save: &[u8]) -> Result<World, String> {
    let exe = std::fs::read(dir.join("DNCDPRG.EXE")).map_err(|e| format!("DNCDPRG.EXE: {e}"))?;
    let hdr = (exe[8] as usize | (exe[9] as usize) << 8) * 16;
    let exe = Exe { image: exe[hdr..].to_vec() };
    let img = SaveImage::from_file(save).ok_or("not a save file")?;
    let map = resource(dir, &exe, 0xbf)?;
    let cells_n = dune_rec::hand::map_cells::CELLS;
    // MAP.HSQ is C5F9h bytes (the growth's end); the owner's C5FCh cells
    // run 3 past it.
    let mut cells: Vec<u8> = (0..cells_n).map(|i| map.get(i).copied().unwrap_or(0)).collect();
    for (i, c) in cells.iter_mut().enumerate() {
        let s = img.map.get(i / 4).map_or(0, |b| b >> (2 * (3 - i % 4)) & 3);
        *c = *c & !0x30 | s << 4;
    }
    let mut w = World {
        save: img,
        rand_seed: 0,
        rand_masked_seed: 0,
        rand_below_seed: 0,
        extras: vec![0; EXTRAS.iter().map(|e| e.1 as usize).sum()],
        growth_seed: exe.cs_word(0x65b4),
        cells,
    };
    let lat = resource(dir, &exe, 0xba)?;
    for k in 0..0x18cu16 {
        let (a, b) = (*lat.get(2 * k as usize).unwrap_or(&0), *lat.get(2 * k as usize + 1).unwrap_or(&0));
        let at = 0x4948 + 2 * k;
        if at < 0x4948 + 0x318 {
            w.set_byte(at, b);
            w.set_byte(at + 1, a);
        }
    }
    for k in 0..0x63u16 {
        let v = w.word(0x494a + 8 * k);
        let d = (v as u32) << 1;
        if d < 2 {
            return Err("TABLAT.BIN: a zero row".into());
        }
        let (q, r) = (0x10000 / d, 0x10000 % d);
        w.set_word(0x4880 + 2 * k, q as u16 + (v as u32) .lt(&r) as u16);
    }
    w.set_word(0xdcfe, 0x62fc);
    // The start-up code (0192h) marks each place's map cell (its word 6)
    // with bit 40h; a save keeps the words, not the marks.
    let mut p = 0x100u16;
    while p < 0x08a8 && w.word(p) != 0xffff {
        let o = w.word(p + 6);
        let v = w.cell(o);
        w.set_cell(o, v | 0x40);
        p += 0x1c;
    }
    Ok(w)
}
