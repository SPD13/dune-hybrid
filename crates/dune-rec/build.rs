//! `src/generated.rs` is written by `dune-lift` from the user's own
//! DNCDPRG.EXE and is never committed. Without it the crate is empty.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_generated)");
    println!("cargo::rerun-if-changed=src/generated.rs");
    if std::path::Path::new("src/generated.rs").exists() {
        println!("cargo::rustc-cfg=has_generated");
    }
    // The lifted drivers (`dune-lift --raw`).
    for d in ["vga", "music", "sound"] {
        println!("cargo::rustc-check-cfg=cfg(has_generated_{d})");
        println!("cargo::rerun-if-changed=src/generated_{d}.rs");
        if std::path::Path::new(&format!("src/generated_{d}.rs")).exists() {
            println!("cargo::rustc-cfg=has_generated_{d}");
        }
    }
}
