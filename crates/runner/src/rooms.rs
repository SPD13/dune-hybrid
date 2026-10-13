//! `dune-run --rooms DIR`: the game draws every room of its room files.
//!
//! At the end of the run (a snapshot of the game in progress, e.g. after
//! skipping the intro), every location's rooms are drawn the way the intro
//! draws a palace room (seg000:0972): `loc_008f0` (1000:08F0) selects the
//! location and room and loads the room file, `loc_037b2` (1000:37B2)
//! clears the game area and draws the room into fb1 (sky or landscape, then
//! `draw_SAL`). The palette is the driver's (`driver:05BF`, what
//! `update_screen_palette` would send to the DAC).
//! Characters are left out (`[12]` = 0, `[47A4]` bit 7). Each distinct room
//! (by [`gfx::room::RoomKey`]) is written to DIR as:
//! - `<key>.json`: room byte, room file, the locations it was drawn for,
//!   the number of drawing calls and the palette;
//! - `<key>.idx`: fb1's game area (320 × 152 palette indices);
//! - `<key>.cov`: which of those bytes the room's calls wrote (0/1);
//! - `<key>.png`: a preview.
//!
//! Rooms drawn while the game ran normally are kept as well (`live-<key>.*`),
//! so the harness can be checked against the real game: same keys, same
//! pixels where the room drew.
//!
//! Location records are at `DS:00E4 + 1Ch × i` (byte 8: location type); the
//! rooms of a location type are 5-byte records (room byte, then exits up,
//! right, down, left) at `[DS:13C4 + 2 × type]`, room 1 first. Exits FCh-FFh
//! lead outside; bit 7 marks a door that is not always open.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt::Write as _,
    fs,
    path::Path,
    rc::Rc,
};

use gfx::{
    DrawOp, Mem,
    room::{AREA, RoomTrack},
};
use pc::{
    GAME_DS, Hardware, LOAD_SEG, Machine,
    gfx::{GfxEvent, Mark},
};

const LOCATIONS: u16 = 0x00e4;
const LOCATION_SIZE: u16 = 0x1c;
const ROOM_GRAPHS: u16 = 0x13c4;
/// Instruction budget per call (resource loading included).
const BUDGET: u64 = 200_000_000;

fn ds8(m: &Machine, off: u16) -> u8 {
    m.hw.mem[(GAME_DS as usize) * 16 + off as usize]
}

fn ds16(m: &Machine, off: u16) -> u16 {
    ds8(m, off) as u16 | (ds8(m, off.wrapping_add(1)) as u16) << 8
}

fn set_ds8(m: &mut Machine, off: u16, v: u8) {
    m.hw.mem[(GAME_DS as usize) * 16 + off as usize] = v;
}

/// A room as `draw_SAL` left it.
pub struct Seen {
    pub room: u8,
    pub key: u64,
    pub calls: usize,
    /// fb1's game area after the room was drawn.
    pub low: Vec<u8>,
    /// Bytes of the game area the room's calls wrote.
    pub cover: Vec<bool>,
    /// Bytes characters standing in the room were drawn over.
    pub overdrawn: Vec<bool>,
    /// Which call drew each byte (see [`RoomTrack::owner`]).
    pub owner: Vec<u16>,
}

/// Follows the graphics events of a run and collects the rooms drawn (the
/// same tracking as the HD compositor's, [`RoomTrack`]).
#[derive(Default)]
pub struct Tracker {
    room: Option<RoomTrack>,
    pub seen: Vec<Seen>,
    /// Rooms whose room-file entry drew nothing (outdoor views: landscape only).
    pub empty: usize,
}

impl Tracker {
    pub fn on_event(&mut self, e: &GfxEvent, hw: &Hardware) {
        match *e {
            GfxEvent::Begin(Mark::Room { .. }) => self.room = Some(RoomTrack::default()),
            GfxEvent::Begin(Mark::Character) | GfxEvent::End(Mark::Character) => {
                if let Some(t) = self.room.as_mut() {
                    t.character(matches!(e, GfxEvent::Begin(_)));
                }
            }
            GfxEvent::Enter(c) => {
                if let Some(t) = self.room.as_mut() {
                    let r = c.regs();
                    let mem = Mem(&hw.mem);
                    t.follow(&DrawOp::decode(c.slot, &r, mem), &r, mem);
                }
            }
            GfxEvent::End(Mark::Room { room }) => {
                let Some(t) = self.room.take() else { return };
                let Some(target) = t.target else {
                    self.empty += 1;
                    return;
                };
                let base = (target as usize) << 4;
                self.seen.push(Seen {
                    room,
                    key: t.key.value(),
                    calls: t.calls,
                    low: hw.mem[base..base + AREA].to_vec(),
                    cover: t.drawn[..AREA].to_vec(),
                    overdrawn: t.overdrawn[..AREA].to_vec(),
                    owner: t.owner[..AREA].to_vec(),
                });
            }
            _ => {}
        }
    }
}

/// Seam metric on an HD frame (`rgb`, `k`× the screen, `dac` 8-bit) for the
/// room `s`, where the screen still shows it: the mean *excess* colour step
/// across the border between two neighbouring pixels' HD blocks, beyond the
/// step between the two original pixels (a seam is a jump the original does
/// not have), at joins between two of the room's calls and inside one call.
/// Returns (joins, inside, number of join borders).
pub fn seams(s: &Seen, screen: &[u8], rgb: &[u8], dac: &[[u8; 3]; 256], k: usize) -> (f64, f64, usize) {
    let w = 320 * k;
    let px = |x: usize, y: usize| -> [i32; 3] {
        let o = (y * w + x) * 3;
        [rgb[o] as i32, rgb[o + 1] as i32, rgb[o + 2] as i32]
    };
    let step = |a: [i32; 3], b: [i32; 3]| ((0..3).map(|i| (a[i] - b[i]).pow(2)).sum::<i32>() as f64).sqrt();
    let low = |i: usize| dac[s.low[i] as usize].map(|c| c as i32);
    let shows = |i: usize| s.cover[i] && !s.overdrawn[i] && screen[i] == s.low[i];
    let (mut join, mut nj, mut inside, mut ni) = (0f64, 0usize, 0f64, 0usize);
    for y in 0..gfx::room::ROWS {
        for x in 0..320 {
            let i = y * 320 + x;
            if !shows(i) {
                continue;
            }
            for (dx, dy) in [(1usize, 0usize), (0, 1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 320 || ny >= gfx::room::ROWS {
                    continue;
                }
                let j = ny * 320 + nx;
                if !shows(j) {
                    continue;
                }
                // Mean step between the facing texel columns/rows of the two blocks.
                let mut d = 0.0;
                for t in 0..k {
                    let (a, b) = if dx == 1 { (px(x * k + k - 1, y * k + t), px(nx * k, y * k + t)) } else { (px(x * k + t, y * k + k - 1), px(x * k + t, ny * k)) };
                    d += step(a, b);
                }
                let excess = (d / k as f64 - step(low(i), low(j))).max(0.0);
                if s.owner[i] != s.owner[j] {
                    join += excess;
                    nj += 1;
                } else {
                    inside += excess;
                    ni += 1;
                }
            }
        }
    }
    (join / nj.max(1) as f64, inside / ni.max(1) as f64, nj)
}

/// Rooms of location type `t`, by room number (1-based), with their room byte.
fn rooms_of(m: &Machine, t: u8) -> BTreeMap<u8, u8> {
    let table = ds16(m, ROOM_GRAPHS + 2 * t as u16);
    let mut out = BTreeMap::new();
    let mut todo = vec![1u8];
    while let Some(r) = todo.pop() {
        if r == 0 || r > 0x40 || out.contains_key(&r) {
            continue;
        }
        let rec = table.wrapping_add((r as u16 - 1) * 5);
        out.insert(r, ds8(m, rec));
        for k in 1..5 {
            let e = ds8(m, rec + k);
            if e < 0xf0 {
                todo.push(e & 0x7f);
            }
        }
    }
    out
}

/// The game's palette as the driver holds it (6-bit values at `driver:05BF`).
fn driver_palette(m: &Machine) -> [[u8; 3]; 256] {
    let drv = ds16(m, 0x38b5 + 2) as usize;
    let at = drv * 16 + 0x5bf;
    std::array::from_fn(|i| [0, 1, 2].map(|k| m.hw.mem[at + i * 3 + k]))
}

fn write_room(dir: &Path, prefix: &str, s: &Seen, json: &str, dac: &[[u8; 3]; 256]) {
    let name = format!("{prefix}{:016x}", s.key);
    fs::write(dir.join(format!("{name}.json")), json).unwrap();
    fs::write(dir.join(format!("{name}.idx")), &s.low).unwrap();
    fs::write(dir.join(format!("{name}.cov")), s.cover.iter().map(|&c| c as u8).collect::<Vec<u8>>()).unwrap();
    // Preview: uncovered pixels darkened.
    let rgb: Vec<u8> = s
        .low
        .iter()
        .zip(&s.cover)
        .flat_map(|(&v, &c)| {
            let p = dac[v as usize].map(|x| (x << 2) | (x >> 4));
            if c { p } else { p.map(|x| x / 3) }
        })
        .collect();
    crate::write_png_sized(&dir.join(format!("{name}.png")), 320, gfx::room::ROWS as u32, &rgb);
}

fn dac_hex(dac: &[[u8; 3]; 256]) -> String {
    dac.as_flattened().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Rooms seen during the normal run (from `tracker`), then every room of
/// every location drawn from a snapshot of `m`.
pub fn capture(m: &mut Machine, dir: &Path, tracker: Rc<RefCell<Tracker>>) {
    fs::create_dir_all(dir).unwrap();
    let live: Vec<Seen> = std::mem::take(&mut tracker.borrow_mut().seen);
    let dac = driver_palette(m);
    for s in &live {
        let json = format!(r#"{{"key":"{:016x}","room":{},"calls":{},"live":true}}"#, s.key, s.room, s.calls);
        write_room(dir, "live-", s, &json, &dac);
    }
    println!("rooms: {} drawn during the run", live.len());

    let state = m.save_state();
    // Locations: records until the list ends (first byte FFh).
    let mut locations = Vec::new();
    for i in 1..0x80u16 {
        let rec = LOCATIONS + LOCATION_SIZE * i;
        if ds8(m, rec) == 0xff {
            break;
        }
        locations.push((i as u8, ds8(m, rec + 8)));
    }
    struct Room {
        seen: Seen,
        sal: u8,
        dac: [[u8; 3]; 256],
        at: Vec<(u8, u8, u8)>,
    }
    let mut rooms: HashMap<u64, Room> = HashMap::new();
    let mut failures = Vec::new();
    let mut runs = 0;
    for &(loc, t) in &locations {
        for (&r, &byte) in &rooms_of(m, t) {
            m.load_state(&state).unwrap();
            tracker.borrow_mut().seen.clear();
            runs += 1;
            let ds = GAME_DS;
            m.cpu.sregs[pc::cpu::DS] = ds;
            m.cpu.sregs[pc::cpu::ES] = ds;
            m.cpu.set_flag(pc::cpu::DF, false);
            set_ds8(m, 0x12, 0);
            set_ds8(m, 0x13, 0);
            m.cpu.regs[pc::cpu::DX] = (t as u16) << 8 | r as u16;
            m.cpu.regs[pc::cpu::BX] = (loc as u16) << 8 | 0x80;
            let step = |m: &mut Machine, off: u16, what: &str| m.call_near(LOAD_SEG, off, BUDGET).map_err(|e| format!("{what}: {e}"));
            let res = (|| {
                step(m, 0x08f0, "select")?;
                let flags = ds8(m, 0x47a4);
                set_ds8(m, 0x47a4, flags | 0x80);
                step(m, 0xc07c, "fb1")?;
                step(m, 0x37b2, "draw")
            })();
            if let Err(e) = res {
                failures.push(format!("location {loc} type {t:02x} room {r} (byte {byte}): {e}"));
                continue;
            }
            let sal = ds8(m, 0x144c);
            let mut tr = tracker.borrow_mut();
            let Some(seen) = tr.seen.pop() else { continue };
            let entry = rooms.entry(seen.key).or_insert_with(|| Room { seen, sal, dac: driver_palette(m), at: Vec::new() });
            entry.at.push((loc, t, r));
        }
    }
    m.load_state(&state).unwrap();

    const SAL: [&str; 4] = ["SIET.SAL", "PALACE.SAL", "VILG.SAL", "HARK.SAL"];
    let mut keys: Vec<&u64> = rooms.keys().collect();
    keys.sort();
    let mut index = String::from("[\n");
    let mut by_file: BTreeMap<u8, BTreeSet<u8>> = BTreeMap::new();
    for (n, key) in keys.iter().enumerate() {
        let room = &rooms[key];
        let s = &room.seen;
        by_file.entry(room.sal).or_default().insert(s.room);
        let at: Vec<String> = room.at.iter().map(|(l, t, r)| format!("[{l},{t},{r}]")).collect();
        let sheet = if s.room == 0 { 0 } else { (s.room as u16 - 1) >> 4 };
        let json = format!(
            r#"{{"key":"{:016x}","room":{},"sal":"{}","sheet":{},"calls":{},"covered":{},"locations":[{}],"dac":"{}"}}"#,
            s.key,
            s.room,
            SAL.get(room.sal as usize).unwrap_or(&"?"),
            if sheet == 0 { 0 } else { sheet + 0x13 },
            s.calls,
            s.cover.iter().filter(|&&c| c).count(),
            at.join(","),
            dac_hex(&room.dac)
        );
        write_room(dir, "", s, &json, &room.dac);
        let _ = writeln!(index, "  {}{}", json.replace(&format!(r#","dac":"{}""#, dac_hex(&room.dac)), ""), if n + 1 < keys.len() { "," } else { "" });
    }
    index.push_str("]\n");
    fs::write(dir.join("index.json"), index).unwrap();
    println!("rooms: {} distinct rooms from {runs} location rooms ({} locations)", rooms.len(), locations.len());
    for (sal, set) in &by_file {
        println!("rooms: {} room bytes {:?}", SAL.get(*sal as usize).unwrap_or(&"?"), set);
    }
    println!("rooms: {} location rooms drew nothing from their room file (outdoor views)", tracker.borrow().empty);
    for f in &failures {
        println!("rooms: failed {f}");
    }
    // The harness against the real game.
    let mut checked = 0;
    for s in &live {
        match rooms.get(&s.key) {
            Some(h) => {
                let shown = (0..AREA).filter(|&i| s.cover[i] && !s.overdrawn[i]);
                let diff = shown.clone().filter(|&i| h.seen.low[i] != s.low[i] || !h.seen.cover[i]).count();
                let chars = (0..AREA).filter(|&i| s.overdrawn[i]).count();
                println!(
                    "rooms: live room {} ({:016x}): {diff} of {} pixels differ from the harness ({chars} under characters)",
                    s.room,
                    s.key,
                    shown.count()
                );
                checked += 1;
            }
            None => println!("rooms: live room {} ({:016x}) not drawn by the harness", s.room, s.key),
        }
    }
    println!("rooms: {checked}/{} live rooms found in the harness", live.len());
}
