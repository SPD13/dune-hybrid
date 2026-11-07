# Decompiling DNCDPRG.EXE to Rust

Goal: replace the original program, routine by routine, with Rust that is
checked against the original as the game runs, until a native engine
remains (the roadmap's "override mechanism"). No full playthrough is needed:
the work is driven by measured coverage and automated checks.

**Clean-room rule.** Inputs are only the executable's own bytes, the
annotated listing (`../Cryogenic/doc/DNCDPRG.lst`), the running original
and Cryogenic's sources; never dune-re's game logic.

**Never commit generated code.** `crates/dune-rec/src/generated.rs` is
produced from your own `DNCDPRG.EXE`. It is game-derived and git-ignored.

## Pipeline

```
DNCDPRG.EXE ─┐                                       ┌─ dune-run --rec shadow   (check)
DNCDPRG.lst ─┼─ dune-lift ─ crates/dune-rec ─────────┤
coverage ────┘   (crates/lift)  generated.rs         └─ dune-run --rec on       (run)
   ▲
   └── dune-run --coverage  (intro, --rooms, --phase-ladder, --monkey)
```

### 1. Coverage: what the game runs, without playing

```sh
dune-run --dir GAME --cmd "ADP220 SBP2227 ENG" --seconds 2400 \
    --events "61.5:key:1:1,61.6:key:1:0" --monkey 12:66:0.6 --coverage cov-m12.txt
dune-run ... --phase-ladder 70:14 --coverage cov-ladder.txt
python tools/coverage.py --lst ../Cryogenic/doc/DNCDPRG.lst cov-*.txt
```

- `--coverage FILE` (`pc::coverage`): every seg000 instruction start
  executed, plus the targets of indirect calls and jumps, which the listing
  cannot resolve.
- `--monkey SEED:START:STEP`: random clicks and keys. If they quit the game,
  the run rewinds to the last screenshot's state and goes on with new input.
- `--phase-ladder START:STEP`: raises the game phase (`[DS:2A]`) step by step
  through `set_game_phase` (1000:121F), injected at the top of the main loop
  (1000:D820). Its handler table has entries only for multiples of 4.
- `--rooms DIR`: the room harness also counts.
- `tools/coverage.py` merges any number of files: instruction and routine
  percentages, the largest routines never entered, executed code the listing
  shows as data.

Measured on the CD 3.7 executable (intro, rooms, phase ladder and 12
random-input runs, about an hour of compute): 57% of the listing's 24,133
seg000 instructions and 74% of its 1,006 routines.

### 2. Lifting: listing + bytes → Rust

```sh
cargo run --release -p lift -- --exe GAME/DNCDPRG.EXE \
    --lst ../Cryogenic/doc/DNCDPRG.lst --out crates/dune-rec/src/generated.rs cov-*.txt
```

- Decodes seg000 from the EXE's bytes (relocations applied for load
  segment 1000h) with `iced-x86`, never from the listing's text. The listing
  decodes some tables as code (the phase handlers near 11B3), and coverage
  proved code runs there.
- Routine entries: the listing's call targets and named labels, plus the
  recorded targets of indirect calls. Each routine includes everything
  reachable by jumps (not calls), and recorded indirect-jump targets.
- One `fn f_XXXX(m: &mut Machine, start: u16) -> Result<(), Bail>` per
  routine: a `match` on block addresses in a loop, so every jump shape is
  kept, and it can start at any of its blocks (`LEADERS` maps every block
  start to a routine that has it). Blocks end after each call and `INT`; a
  `REP` string op is a block of its own.
  State stays in the emulated machine; flags come from the interpreter's
  own ALU (`cpu::alu`, now public), so they match by construction.
- About 96% of instructions are written out natively. The rest go through
  the interpreter one instruction at a time (string ops, MUL/DIV, BCD,
  PUSHF/POPF, ENTER/LEAVE, PUSHA/POPA). `INT` and far calls use
  `Machine::rec_interp`; RETF, IRET and far jumps bail.
- Self-modifying code: the only seg000 site is the immediate of
  `mov ax, 25h` at C13B (patched at 0AD3 and 0B24). Instructions covering
  a patched byte always run through the interpreter.

### 3. Checking: shadow mode

```sh
dune-run ... --rec shadow --rec-report shadow.tsv
```

`pc::rec`, `RecMode::Shadow`:
- The original runs normally.
- For each routine's first 20 calls (real `CALL`s only: an entry reached by
  a jump has no return address), the entry state (registers and all memory)
  is kept.
- While a check is pending, everything the program receives from outside
  the CPU is recorded (`hw::IoLog`): port reads and writes, hardware
  interrupts with the instruction count they came at, and the result of
  every BIOS/DOS service (registers, and the memory it wrote).
- The check ends when the stack rises above the call's entry frame: at its
  `ret`, or wherever the routine dropped the frame (some pop their own return
  address, or push a fake one like DBECh and end in a jump chain).
- The Rust version is then replayed from the entry state, fed from the log,
  for exactly as many instructions as the original took. Rust resumes at
  every block start; the interpreter runs only where Rust bailed (to take a
  recorded interrupt at the exact instruction, or at the end point).
  Registers, flags and all memory are compared. Port I/O, services or
  interrupts different from the log are differences too.
- The report lists, per routine: equal, equal with interpreted parts,
  different (with the first difference), and not checked (calls over 50 M
  instructions).

Measured (30-minute random-input runs, seeds 12 and 13, and a phase-ladder
run): 679 to 803 routines called per run, 638 to 758 of them checked equal,
**0 with differences**. About 60% of the replayed instructions ran in Rust.
Not checked: the few routines whose calls are too long (`start`,
`play_intro`, `game_loop`, `sub_afb5`), and entry points that were only ever
reached by jumps.

The checker was itself tested: a deliberately broken `add` in
`adjust_sub_resource_pointers` was reported as "reg AX bbd0 vs bb2c", and
nothing else changed.

### 4. Running: `--rec on`

The recompiled routines run instead of the original. The run loop calls a
routine at any block start it reaches; a routine that bails leaves the
machine at an instruction boundary, the interpreter runs that instruction,
and Rust takes over again at the next block start. Interrupts are not
delivered while a routine runs. Instead, at every block boundary a routine
checks `Machine::rec_tick` and returns to the run loop (a bail) when a
device event or an interrupt is due. Otherwise a loop polling a timer
variable would wait forever.

Measured on a 5-minute random-input game: 5.3 M entries into Rust, 2.6 M of
them ending in a bail (mostly for an interrupt). All 10 screenshots are
byte-identical to the same run without `--rec`, and the run is about 1.6×
faster (21 s vs 34 s wall).

## Next steps

1. **Coverage of the rest:** milestone saves; harnesses for the globe,
   battles and the endings (`chankiss` etc.); more ladder × monkey seeds on
   the Windows machine. Shadow-check those runs too.
2. **Fewer bails:** run more instruction kinds natively (string ops,
   MUL/DIV, PUSHF/POPF) and far calls into seg000, so less runs on the
   interpreter in `--rec on` and in replays.
3. **Readable Rust:** turn verified routines into idiomatic code (named
   globals, typed structs, no `m.cpu` plumbing). Shadow mode is the oracle,
   so this can run as a loop: rewrite, check, keep only what stays equal.
4. **Drivers** (DNVGA is modelled in `gfx::model`; sound and music next) and
   **DOS** behind a host API, for a build without the interpreter.
