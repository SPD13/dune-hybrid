//! List DUNE.DAT's sprite sheets: `cargo run -p gfx --example sheets -- DUNE.DAT [hashes.txt]`.
//! With a file of sprite hashes (one per line), reports which are not found.

use std::{collections::HashMap, fs::File};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("DUNE.DAT path");
    let mut f = File::open(&path).unwrap();
    let toc = gfx::dat::toc(&mut f).unwrap();
    let mut catalog = HashMap::new();
    let (mut sheets, mut sprites) = (0, 0);
    for e in &toc {
        let Ok(res) = gfx::dat::load(&mut f, e) else { continue };
        let Some(list) = gfx::sheet::parse(&res) else { continue };
        sheets += 1;
        sprites += list.len();
        let dims: Vec<String> = list.iter().take(6).map(|s| format!("{}x{}{}", s.width(), s.height, if s.wflags & 0x8000 != 0 { "r" } else { "" })).collect();
        println!("{:4} {:16} {:3} sprites  {}", e.index, e.name, list.len(), dims.join(" "));
        for s in list {
            catalog.insert(s.hash, (e.name.clone(), s.index));
        }
    }
    println!("{sheets} sheets, {sprites} sprites");
    if let Some(hashes) = args.next() {
        let text = std::fs::read_to_string(hashes).unwrap();
        let (mut found, mut missing) = (0, Vec::new());
        for h in text.lines().filter(|l| !l.is_empty()) {
            match catalog.get(&u64::from_str_radix(h, 16).unwrap()) {
                Some(_) => found += 1,
                None => missing.push(h.to_string()),
            }
        }
        println!("traced sprites: {found} found, {} missing {:?}", missing.len(), &missing[..missing.len().min(10)]);
    }
}
