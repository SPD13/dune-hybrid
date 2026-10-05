# dune-hybrid

Cryo's **Dune** (1992, CD version 3.7) running in the browser. The original `DNCDPRG.EXE` runs on our own small PC emulator, compiled to WebAssembly. Routines can then be replaced one at a time by native Rust ports until no original code runs.

**Development policy:** this engine is written from the disassembly (`Cryogenic/doc/DNCDPRG.lst`), the running original (Cryogenic/Spice86) and Cryogenic's Apache-2.0 sources. Do **not** consult madmoose/dune-re's game-logic code while working here.

## Layout

| Crate | What |
|---|---|
| `crates/cpu` | Real-mode 80286 interpreter, validated against 1.44 M hardware test vectors |
| `crates/pc` | The PC: 8259 PIC, 8254 PIT, keyboard controller, VGA mode 13h, BIOS/DOS/mouse services (Rust callbacks behind ROM stubs) |
| `crates/runner` | `dune-run`: headless native runner (screenshots, scripted input, diagnostics) |
| `crates/web` | wasm-bindgen host used by the browser worker |
| `web/` | Vite + TypeScript page: picks and stores the game files, runs the worker, draws and forwards input |

The program loads at segment `0x1000`, as in Spice86/Cryogenic's `-p 4096`, so addresses match the reference disassembly and Cryogenic's overrides.

## Run natively (headless)

```sh
cargo build --release
./target/release/dune-run --dir path/to/dune --seconds 30 --shot-every 2 --out out \
    --events "20:key:01:1,20.2:key:01:0,24:mouse:160:182:0"
```

- `--dir` is the folder holding `DNCDPRG.EXE` and `DUNE.DAT`.
- Screenshots are written to `out/shot-NNNN.png`.
- Saves go to `out/saves`; the game folder is never written to.
- Events take the form `time:key:SCANCODE(hex):1|0` or `time:mouse:X:Y:BUTTONS`.

## Run in the browser

```sh
cd web && npm install && npm run wasm && npm run dev   # http://localhost:5174
```

Choose `DNCDPRG.EXE` and `DUNE.DAT` once; they are kept in the browser's private storage (OPFS). In dev, `?devfiles` imports them from `../../Cryogenic/dune`.

## CPU tests

```sh
cd tests-data
git clone --depth 1 --filter=blob:none --sparse https://github.com/SingleStepTests/80286.git ss286
(cd ss286 && git sparse-checkout set v1_real_mode)
curl -O https://raw.githubusercontent.com/SingleStepTests/80286/main/tools/moo2json.py
python3 ../tools/sst-compact.py
cargo test -p cpu --release -- --ignored single_step
```

## Status

- [x] M1: the original boots in the browser and plays the intro; keyboard and mouse work.
- [ ] M2: sound (OPL3 music, Sound Blaster voices), save games in IndexedDB.
- [ ] M3: an override table, so routines are ported to Rust and checked against the original in-process.
