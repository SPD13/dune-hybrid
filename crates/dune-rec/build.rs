//! `src/generated.rs` is written by `dune-lift` from the user's own
//! DNCDPRG.EXE and is never committed. Without it the crate is empty.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(has_generated)");
    println!("cargo::rustc-check-cfg=cfg(has_generated_vga)");
    println!("cargo::rerun-if-changed=src/generated.rs");
    println!("cargo::rerun-if-changed=src/generated_vga.rs");
    if std::path::Path::new("src/generated.rs").exists() {
        println!("cargo::rustc-cfg=has_generated");
    }
    if std::path::Path::new("src/generated_vga.rs").exists() {
        println!("cargo::rustc-cfg=has_generated_vga");
    }
}
