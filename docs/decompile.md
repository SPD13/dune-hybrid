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

crates/dune-rec/src/hand/: readable ports, replacing generated routines
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
- `--call-each N` (`runner/src/callall.rs`): at the end of the run, from the
  top of the main loop, every routine is called directly N times: first with
  the registers as they are there, then with random ones, each from the same
  saved state, until it returns or for 300 ms of virtual time. The calls mean
  nothing to the game, but under `--rec shadow` each is a full check, and
  `--coverage` counts what they run. Random registers send indirect jumps to
  garbage addresses: keep these coverage files out of the lifter's input.
- `tools/coverage.py` merges any number of files: instruction and routine
  percentages, the largest routines never entered, executed code the listing
  shows as data.

Measured on the CD 3.7 executable. Intro, rooms, phase ladder and 12
random-input runs (about an hour of compute): 57% of the listing's 24,133
seg000 instructions and 74% of its 1,006 routines. With `--call-each 6` from
three game states (a few minutes each): **80% of the instructions, every
routine**.

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
- 98.8% of instructions are written out natively. String ops and
  MUL/IMUL/DIV/IDIV call the interpreter's own operations
  (`Cpu::string_op`, `Cpu::mul_div`) without its fetch and decode. IRET,
  RETF and far jumps are native too. The rest (BCD, ENTER/LEAVE,
  PUSHA/POPA, STI) go through the interpreter one instruction at a time.
  `INT` and far calls use `Machine::rec_interp`.
- Self-modifying code (found by `CODE_DIFF=1 dune-run ...`, which lists the
  seg000 bytes that changed during a run):
  - the immediate of `mov ax, 25h` at C13B (patched at 0AD3 and 0B24):
    instructions covering it always run through the interpreter;
  - the targets of the far jumps at EFDC and F04D, set at start-up to the
    interrupt handlers the game replaced (it chains to them): every direct
    far jump reads its target from the code when it runs.
  - The other changed bytes are data kept in the code segment (saved
    vectors at E956–E979, counters at 00AA, CC94, E8D4).

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
**0 with differences**. With `--call-each 4` added: all 1,113 routines
called, about 1,080 checked equal, 0 with differences. About 60% of the
replayed instructions ran in Rust. Not checked: the few routines whose calls
are too long (`start`, `play_intro`, `game_loop`).

A call that wrote into the code segment is marked in the report ("the call
wrote into the code segment"): if it changed code, Rust, which keeps the
code as loaded, would run differently.

The checker was itself tested: a deliberately broken `add` in
`adjust_sub_resource_pointers` was reported as "reg AX bbd0 vs bb2c", and
nothing else changed.

### 4. Running: `--rec on`

The recompiled routines run instead of the original. The run loop calls a
routine at any block start it reaches; a routine that bails leaves the
machine at an instruction boundary, the interpreter runs that instruction,
and Rust takes over again at the next block start.

The result is the same, to the instruction, as without `--rec`:
- Interrupts are not delivered while Rust runs. At each block start a
  routine checks `Machine::rec_tick`: if an interrupt is pending, or a device
  event or the run's deadline falls inside the block (virtual time is a
  function of the instruction count, so this is known in advance), it bails
  and the interpreter runs the block.
- `INT` and far calls (BIOS/DOS services, the drivers) run on the
  interpreter inside the routine, with the same per-instruction checks,
  the music-driver tracking and the battery saver's idle detection.
- Port I/O sees the time of its instruction (the PIT counter).
- While a harness breakpoint is set (`--phase-ladder`), Rust stays off: it
  would run through the breakpoint.

Measured: three 5- and 10-minute random-input games (one with the phase
ladder): every screenshot byte-identical to the same run without `--rec`
(50 of 50), at 135 to 165 MIPS against 63 for the interpreter (2.2 to 2.6×).

### 5. Readable ports: `crates/dune-rec/src/hand/`

Hand-written Rust for single routines, from the listing, replacing the
generated routine of the same entry, and checked by shadow mode the same way
(`[hand]` in the report; `REC_NO_HAND=1` turns them all off, to tell whether
a difference comes from one). Modules by area: `story`, `phases`, `intro`,
`time`, `records`, `script` (condition operands and operators), `tasks`
(frame tasks), `video`, `gfx`, `screen`, `sprites`, `text`, `hnm`, `hsq`
(the decompressor), `sound`, `audio`, `voice`, `input`, `dos`, `files`,
`resources` (opening sprite sheets, the condition evaluator), `ui`,
`panels`, `globe`, `mouse`, `draw`, `hnmload`, `scenes`, `saves`, `frame`
(the frame-task runner, input checks, HNM frame steps), `cursor` (the
keyboard cursor's hotspot search), `map` (the map screen), `troops`,
`dialogue`, `screens` (the stack of UI screens, the panel's buttons);
data-segment
globals are named in `globals.rs` (from the routines that use them and
Cryogenic's `GlobalsOnDs.cs`; unknown ones keep their offset).

A port must end in the original's exact state: the registers and flags it
returns are spelled out in its doc comment (callers may rely on any of
them), stack slots a push/pop left below SP are written too, and it keeps
the instruction count (virtual time). `Ctx` holds the bookkeeping:
- `run(at, n)` counts a straight stretch of original instructions and bails
  at `at` before changing anything if an interrupt (or a check's end) falls
  inside; `check(at, max)` + `count(n)` for data-dependent paths (checking
  the longest is always safe);
- the usual shape: work out the path from pure reads, `run`, then do the
  work with flag-exact operations (`add`, `cmp`, `shr1`, `rcl1`, `lods`,
  `movs`, `rep`, ...), so a bail leaves the machine exactly at a stretch's
  first instruction;
- `call` (near, with the original's return address on the stack),
  `interp` (`INT` and far calls into the drivers), and tail jumps call the
  target port directly.

Ported: 819 routines (the small ones first: flags and setters, game time,
location kinds, frame tasks, condition operators, the intro's steps,
framebuffer and rectangle copies, fonts and strings, HNM reading, HSQ
decompression, voice and music start/stop, DUNE.DAT access, the memory
driver calls; then the condition evaluator, resource opening, the sky,
the date panel, the communication device, settings sliders, voice-file
streaming, music loading; the globe and the frieze, the mouse cursor,
the frame-task runner, the input check, HNM frame stepping; sprite
drawing, plain and scaled, and UI elements; opening videos and the
intro's video loaders, the IRUL videos with subtitles, the result bars,
the font glyphs, map statistics; saved games' RLE and the restore of
the map, particles, numbers in strings; the keyboard cursor's hotspot
search, HNM ring-buffer bookkeeping, line and rect outlines, the
settings screen's hit tests and volumes, the clock-stop counter, the
map screen's view rect, mark and projection (map to screen and back),
map rows, troop and location records, dialogue state and the dialogue
table scan, save-slot labels, the stats' change marks; the stack of UI
screens, the panel's buttons and gauges, the element under the cursor,
the joystick read as keys, particle variants and moves, clipped rect
copies, the HNM memory setup; troops joining, merging and unlinking,
their skills, rates and facing, the skills panel, pending events and
their telling, sentences and number words, sprite lists, region marks
on the map, the cursor move), all equal to the original in every
checked call (two 10-minute games with `--call-each 4`, 0 differences), and `--rec on`
screenshots unchanged. A port no run has checked to completion stays out of the table
(`copy_pal_and_transition`, `comm_fade_in_glow`,
`comm_animate_incoming_call`, `sub_b1ee`: longer than the harness's
300 ms). A port may also cover only a routine's common path and bail at
its entry for the rest (`pause_if_p_key_pressed`: the pause screen
itself stays generated).

The check catches a port's mistakes where they matter: an instruction
count one short in `apply_sprite_sheet_palette` showed as differences in
the 40 routines that open resources, and was found from them.

`handle_ctrl_v_once` (B270) turns itself into a bare `ret` after it ran
(a code write the generated code does not see): its port reads its own
first byte where it runs.

Open: under `--call-each` on seed 21, two generated routines differ
(`hnm_render_timer_callback` and `parse_command_line`, which it interrupts,
both with the harness's artificial state); they differ the same way with
`REC_NO_HAND=1`, so it is not a port. Not seen in normal play.
Since a port added a routine the generated table lacked (`loc_05d44`, now
1,116 routines), the harness's random registers fall differently, and
`format_interpolated_string` and `hnm_decode_typed_chunk_video_to_bp` each
get one call whose garbage state makes the original rewrite its own code
("the call wrote into the code segment"); Rust keeps the code as it was.
Without that batch's ports (back to 1,115 routines) they pass; the
circle ports alone (which do write code) are not the cause.

## Next steps

1. **More readable ports**, the main work from here: name the data segment
   (globals and the record layouts: locations, troops, the dialogue
   scripts) and port routine by routine, leaves first. Shadow mode plus
   `--call-each` is the oracle, so this can run as a loop: rewrite, check,
   keep only what stays equal.
2. **Coverage of the rest** (the 20% of instructions never run): milestone
   saves, and harness states deeper in the game (the endings: `chankiss`
   etc.).
3. **Drivers** (DNVGA is modelled in `gfx::model`; sound and music next) and
   **DOS** behind a host API, for a build without the interpreter.
