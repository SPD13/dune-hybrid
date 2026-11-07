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
- One `fn f_XXXX(m: &mut Machine) -> Result<(), Bail>` per routine: a
  `match` on block addresses in a loop, so every jump shape is kept.
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
- For each routine's first 20 calls, the entry state (registers and all
  memory) is kept until the original returns. The Rust version is then
  replayed from that state, and registers, flags and memory are compared.
- A call during which an interrupt, DOS/BIOS service or port I/O happened
  is not replayed (the replay would see a different world). It counts as
  "not checkable".
- The report lists, per routine: equal, different (with the first
  difference), bailed, not checkable, and different instruction counts.

Some routines push a fake return address (DBECh) before their final jump,
so their `ret` does not land at the caller. The replay then follows on with
the interpreter to the frame the original reached.

Measured (three 30-minute random-input runs and a phase-ladder run): 655 to
803 routines called per run, 456 to 558 verified equal, **0 with
differences**, about 200 to 240 not checkable.

The checker was itself tested: a deliberately broken `add` in
`adjust_sub_resource_pointers` was reported as "reg AX bbd0 vs bb2c", and
nothing else changed.

### 4. Running: `--rec on`

The recompiled routines run instead of the original. The run loop calls a
routine at its entry; a routine that bails leaves the machine at an
instruction boundary and the interpreter goes on. Interrupts are not
delivered while a routine runs. Instead, at every block boundary a routine
checks `Machine::rec_tick` and returns to the run loop (a bail) when a
device event or an interrupt is due. Otherwise a loop polling a timer
variable would wait forever. The rest of that call then runs on the
interpreter.

Measured on a 5-minute random-input game: 51 M calls into Rust, 2.5 M of
them bailed. All 10 screenshots are byte-identical to the same run without
`--rec`, and the run is about 1.5× faster (22 s vs 34 s wall).

## Next steps

1. **Checks that can be replayed:** record port reads and DOS results
   during the original call and feed them to the replay. Most "not
   checkable" routines would then be checked too.
2. **Coverage of the rest:** milestone saves; harnesses for the globe,
   battles and the endings (`chankiss` etc.); more ladder × monkey seeds on
   the Windows machine.
3. **Resume after a bail:** let the run loop re-enter Rust at any block
   leader (not just routine entries), so a routine interrupted by an IRQ
   goes back to native code instead of finishing on the interpreter.
4. **Readable Rust:** turn verified routines into idiomatic code (named
   globals, typed structs, no `m.cpu` plumbing). Shadow mode is the oracle,
   so this can run as a loop: rewrite, check, keep only what stays equal.
5. **Drivers** (DNVGA is modelled in `gfx::model`; sound and music next) and
   **DOS** behind a host API, for a build without the interpreter.
