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

## Choosing how the game runs

The emulator stays the default; the Rust code is opt-in, so what worked
before keeps working.

| Run | How | Needs `generated*.rs` |
|---|---|---|
| **Emulated** (interpreter only, as before) | `cargo build --release`, then `dune-run …` with no `--rec` | no |
| **Rust, checked** against the interpreter | `dune-run … --rec shadow` | yes |
| **Rust, exact timing**, interpreter where Rust has no code | `dune-run … --rec on` | yes |
| **Rust, native timing** | `dune-run … --rec native` | yes |
| **Without the interpreter** at all | `cargo build --release -p runner --no-default-features`, then `--rec native` | yes |

- The web app always builds with the interpreter and does not use the
  generated code.
- Without the generated files, `dune-rec` still compiles (a fresh clone);
  `--rec` then has only the hand ports.

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
  `--call-only E1,E2,..` (hex entries) limits it to those routines and
  `--call-ms MS` sets the time a call may take: routines that wait for the
  timer (the communication device, palette transitions) need 2 to 3 s.
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
  RETF, far jumps and STI are native too (the instruction after STI starts
  a block, so a pending interrupt is seen where the CPU takes it). The rest
  (BCD, ENTER/LEAVE, PUSHA/POPA) go through the interpreter one
  instruction at a time. `INT` and far calls use `Machine::rec_interp`.
- `--leaders FILE` (the `REC_MISSES` file of a `--rec on` run, see below):
  more block starts, where Rust found none to go on from; one outside every
  routine becomes a routine of its own (`resume_from_XXXX`).
- Self-modifying code (found by `CODE_DIFF=1 dune-run ...`, which lists the
  seg000 bytes that changed during a run):
  - the immediate of `mov ax, 25h` at C13B (patched at 0AD3 and 0B24):
    read from the code as it runs (as the drivers' rewritten immediates,
    below);
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

**Through interrupts** (readable ports only). A port's stretch with a
recorded interrupt due inside it does not bail in a replay: the port runs
it without the interrupt, and at its next boundary (the next stretch, a
call, a jump, an `INT`, its return) `Machine::rec_settle` interprets the
stretch from its start twice:
- without the interrupts, it must give exactly the port's state (else the
  call differs: "stretch from ... differs");
- with them, at their recorded instruction, it gives the original's state,
  and the port goes on from that.

If the interrupts changed registers, flags or where the stretch ended, the
port bails there instead. A port may keep a value it read across stretches
(right under `--rec on`, where no interrupt comes while Rust runs). A call
that differs only with stretches run through interrupts is therefore
replayed once more, bailing at them as before, and counted from that.
Stretches with port I/O (`check` and `count`) still bail. A port with long
loops of its own, which always met an interrupt and never got to its end
in Rust, is checked to its end this way (24,000 such stretches per
10-minute run).

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
- `INT` and far calls (BIOS/DOS services, the drivers) run inside the
  routine, instruction by instruction, with the same per-instruction checks,
  the music-driver tracking and the battery saver's idle detection. The
  `INT` itself, the calls (`CALL rel16`, `CALL ptr16:16`, `CALL [disp16]`
  near and far) and the BIOS stubs an `INT` reaches (`FE 38 nn`, a
  service; `INT 1Ch` from the timer stub; `IRET`) are run by
  `Machine::host_step`, not the interpreter: the services behind them are
  Rust already (`pc::bios`/`dos`). Past the call, lifted code (a driver's,
  seg000's) runs natively from any block start; anything else on the
  interpreter.
- After a bail, Rust takes over again at the same place: a port may stop
  where the generated code goes on. A second bail there with nothing
  changed (no instruction ran, no interrupt or device event was taken, the
  battery saver did not sleep, the run's deadline is the same) has the
  interpreter run one instruction.
- `REC_MISSES=FILE` writes where Rust found no block start to go on from
  (a port's stop in the middle of a block, a bail before an event inside a
  block): the next lift's `--leaders`.
- Port I/O sees the time of its instruction (the PIT counter).
- While a harness breakpoint is set (`--phase-ladder`), Rust stays off: it
  would run through the breakpoint.

Measured: three 5- and 10-minute random-input games (one with the phase
ladder): every screenshot byte-identical to the same run without `--rec`
(50 of 50), at 135 to 165 MIPS against 63 for the interpreter (2.2 to 2.6×).

**`--rec native`** is `--rec on` without the exact timing: device events,
interrupts and the run's deadline are taken at the first block start at or
after they fall due (`Rec::late_events`), so the interpreter no longer runs
blocks just to land an interrupt on its instruction. Interrupts are
asynchronous on the real machine too; this is the timing a build without
the interpreter has. Measured: a 5-minute random-input game ran 4
instructions more of 2.08 billion than without `--rec`, every screenshot
identical, and the same to the instruction however the run is sliced
(with or without `--shot-every`); its audio differs from 69 s on in 0.8%
of the samples, at about 1% of the signal (a sound event a few samples
later). It found a bug the exact mode hid: STI's
one-instruction interrupt hold was never cleared by Rust code (the
interpreter clears it at every step), so the game's first wait for the
timer (E86Eh, after it installs its handler) waited forever; `rec_tick`
now clears it when a block runs.

The report shows what is still interpreted, by segment and by address
(`rec interpreted most at:`).

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
`dialogue`, `screens` (the stack of UI screens, the panel's buttons),
`tails` (short entries that set up registers and go on into another
routine);
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
- a `rep` counts as one instruction but every repetition advances the
  count: the stretch ends at the `rep`, and a new `run` covers what
  follows (else an interrupt due right after it is missed);
- `call` (near, with the original's return address on the stack),
  `interp` (`INT` and far calls into the drivers), and tail jumps call the
  target port directly.

Ported: 1,089 routines (the small ones first: flags and setters, game time,
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
on the map, the cursor move; the EMS and XMS copies, the memory
drivers' setup and release, the bump allocator, the interrupt
handlers, mouse and joystick setup, save-file timestamps and
compression, the voice block decoder, tiled sprites, the HNM header, opening resources and files, the frame's input read, HNM read-ahead, resource names, HNM frame decoding, the
start-up resources, the intro's second part, the MIDI position
check, the mouse driver's event callback, freeing and
keeping resources in memory, RLE images, mouth frames, the speech bubble, redrawing
a rectangle with its particles, the scenes' sprites and polygons, the
panel's buttons, loading a phrase's voice and lip-sync,
an HNM frame's chunks; the intro's scene
entries and other short entries ending in a jump), all equal to the original in every
checked call (two 10-minute games with `--call-each 4`, 0 differences), and `--rec on`
screenshots unchanged. In a replay, a callee that bails (an interrupt due inside it, a timer
wait) is finished there, with the recorded interrupts and Rust resumed
at block leaders, and the caller's Rust code goes on after it (only in
shadow replays: `--rec on` leaves interrupts and devices to the run
loop). A call is still counted as bailed when any part of it was
interpreted. The report's `reached` column counts the
calls in which a port ran all of its own code (it returned without a
bail, or got to its `ret` or final jump). A jump to code that is no
routine's entry resumes the generated code there (its block leader)
rather than bailing. A port no run has checked to completion stays out of the table
(`sub_b1ee`, and a dozen routines no run reaches: the Emperor's demand,
some scenes and video steps). Routines that wait for the timer
(`copy_pal_and_transition`, the communication device) are checked with
`--call-only` and `--call-ms 3000`; routines of the late story (the
night attack's steps, the map panel) from the state the phase ladder
leaves (`--phase-ladder 70:10 --seconds 380`, then `--call-each` on them). A port may also cover only a routine's common path and bail at
its entry for the rest (`pause_if_p_key_pressed`: the pause screen
itself stays generated).

The check catches a port's mistakes where they matter: an instruction
count one short in `apply_sprite_sheet_palette` showed as differences in
the 40 routines that open resources, and was found from them.

`handle_ctrl_v_once` (B270) turns itself into a bare `ret` after it ran
(a code write the generated code does not see): its port reads its own
first byte where it runs. `load_voc_and_lipsync_data` (A6CC) flips the
immediate of its test sound each time: its port reads that word too.

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

### 6. The drivers, lifted: `generated_{vga,music,sound}.rs`

The game loads its drivers from DUNE.DAT into memory it allocates; the
listing does not have them. Under `--rec on`, after the seg000 routines,
23% of all instructions were still interpreted, and 97% of those were the
graphics driver (DNVGA; `rec interpreted: ... by segment` in the report,
`rec drivers:` names the segments). It is lifted from its own bytes:

```sh
dune-run ... --dump-drivers DIR --coverage cov.txt   # DIR/graphics.bin, sound.bin, music.bin
dune-lift --raw DIR/graphics.bin --seg 334b --slots 46 --name vga     --also OTHER/graphics.bin ... --cov cov.txt --out crates/dune-rec/src/generated_vga.rs
```

- Entries: the driver's jump table at 0100h (46 slots of 3 bytes, the far
  pointers the game keeps at `DS:38B5h`), and every near call target
  found from them. The generated code takes its segment from CS.
- The driver rewrites some of its own code (15 instructions: immediates
  it computes, the targets of two jumps, a `nop nop` it turns into a jump,
  an `inc`/`dec`, a `loop`'s target): `--also` gives images of the driver
  from other times and runs. Each instruction whose bytes differ between
  them gets every form seen (`rewritten: N forms`), and the generated code
  picks by the bytes in memory; forms that differ only in the immediate
  also get a form for any value, read from the code as it runs. All forms'
  targets are explored (the jumps at 1EA5 and 1F28 lead to the driver's
  hottest loop, 1D83, which a lift of one image never reached). A form
  starting where its own image has no instruction boundary is left out.
  Bytes no form matches go through the interpreter. The output also
  holds a hash of all the other code bytes (with an instruction counted
  as rewritten as a whole: a word field may differ in one byte between
  images). The machine checks the driver in memory against it as soon as
  the game's pointer to it changes, then after 1 ms doubling to every
  100 ms while it differs (it is still being loaded), and every 100 ms
  once it matched; the driver runs natively only in a segment that
  passed (before it is loaded and set up, or if the game's pointer to it
  is overwritten, it is interpreted). A shadow check whose call changed
  the driver's code is marked like one that changed seg000's.
- Coverage files now also record the drivers' instruction starts (`h
  SEG:OFF`) and indirect jumps, for the lifter.
- The music and sound drivers the same way: `--slots 7 --name music`
  (table at `DS:396Dh`, segment word at 396Fh) and `--slots 8 --name
  sound` (398Bh), with `--entries 03ad,0591` for the sound driver's
  interrupt handlers (code the coverage shows run but no call reaches).
- `pc::rec` has a code space per lifted driver (1 graphics, 2 music, 3
  sound), at the segment read from the game's driver table. Shadow mode
  checks the drivers' routines like seg000's (their report keys are
  10000h times the space plus the offset); calls from seg000 are far
  calls. A difference in a call made while a driver's code was not the
  lifted one (overwritten by a random-register harness call, within the
  100 ms between checks) is marked "a driver's code was not the lifted
  one at the call". The music tracker still sees every call to the music
  driver. Under `--rec
  on` the driver runs natively from any of its block leaders, also inside
  a port's or the generated code's far call (`rec_interp`). Not while the
  graphics recorder watches the driver (`GFX_VERIFY`, the HD compositor).
  `REC_NO_DRIVER=1` leaves it to the interpreter.

Measured (the validation runs: two 10-minute random-input games with
`--call-each 4`, the long-call and phase-ladder runs): graphics, 71 of 79
routines called, 61 checked equal, 1 with differences (slot 22, in calls
that also wrote into seg000's code, as `hnm_render_timer_callback`);
music, 32 of 33 called, 29 equal; sound, 36 of 51 called, 35 equal, 1
different only with its code overwritten by the harness. Under `--rec on`
the interpreted share fell from 23% to 0.8% of instructions (what is
left: the BIOS stubs, seg000's bails, the graphics driver's rewritten
instructions), and a 5-minute run went from about 100 to 190-240 MIPS,
with every screenshot and the audio output byte-identical. With the BIOS
stubs and `INT`s on the host and the tighter re-entry after a bail, 0.5%
(244 MIPS), screenshots still identical. With the rewritten instructions'
forms, the calls on the host and the extra block starts: 10,609 of 2.08
billion instructions interpreted in a 5-minute game (0.0005%; most of them
running to an interrupt's exact instruction), 196 MIPS against 51 for the
interpreter alone, screenshots identical. The relift (all coverage files, the
extra block starts) has 1,202 routines; the validation runs show only
differences marked as the harness's (a call that wrote into the code
segment, or a driver whose code the harness's random calls had overwritten:
more of these in the sound driver now that drivers are re-checked as soon
as the game's pointer to them changes).

Two lifter bugs it found: `not` on memory negated the address (the
memory `put` names the address `a`, hiding the value), and a generated
block's interrupt check only looked at the next recorded event, missing
an interrupt behind a port read (the driver's retrace polling loops).

### 7. Without the interpreter

```sh
cargo build --release -p runner --no-default-features   # pc without its `interp` feature
dune-run ... --rec native
```

`pc`'s `interp` feature (on by default, and for the web build) is the
x86 interpreter: every use goes through `Machine::cpu_step`. Built without
it, `Cpu::step` is never instantiated (it is generic over the bus), so the
interpreter is not in the binary; only `--rec native` runs, and code with
no Rust version stops the machine: `exit: Unlifted at CS:IP`. `dune-rec`
takes `pc` without default features, so it cannot turn it back on.

What made it possible, beyond the sections above:
- `IMUL r16, r/m16, imm`, `AAM` and `HLT` lifted (`HLT` stops the routine
  halted; the run loop sleeps until the interrupt).
- A driver is checked as soon as the game calls into it (at most once a
  millisecond while it is not found to be the lifted one), not only on
  the next periodic check: a just-loaded driver runs natively from its
  first call.
- Interrupt handlers no call reaches (the temporary timer handler at
  E8B8 that the start-up code installs to measure the machine), and
  driver code the dumps never showed running: found by `REC_MISSES` from
  the interpreter build (`--rec native` and `--rec on`, random-input games
  and the phase ladder: about 940 places), and lifted as block starts or
  routines of their own with `--leaders` (and what those call).
- Every hand port's stretch start is a block start (a port may stop at
  any of them, in the middle of a generated block): `--leaders` from
  `grep -rhoE "c\.(run|check)\(0x[0-9a-f]{4}" crates/dune-rec/src/hand/`
  (2,933 places, as `1000:XXXX` lines).
- Rewritten branches whose forms differ only in their displacement get a
  form computing the target as it runs (the graphics driver's `loop` at
  163B, whose target it rewrites among a family of entries at 166E-16AA);
  `--rewritten HEX,...` marks instructions known to be rewritten that no
  dump shows rewritten (1684, 1687: siblings of rewritten ones). Without
  it, the first rewrite made the whole driver "not the lifted one".
- A driver runs natively at the segment its code was verified at, even
  while the game's pointer to it moves.
- In native timing, STI's one-instruction hold also ends at the block
  boundary: an interrupt pending then is taken there, one instruction
  early (else only the interpreter could run the instruction before it).

Measured: a 5-minute random-input game without the interpreter ran to its
deadline at 197 MIPS (51 for the interpreter alone), every screenshot
identical to the interpreter's, the same final CPU state. Ten 20-minute
random-input games (seeds 12, 13, 21, 31, 44, 57, 66, 78, 90, 101), one at
a time: all ran to their deadline at 140 to 160 MIPS. The last stop
found (seed 12, in the sound driver) was a device due again at the very
time the run loop had just updated it: in native timing an event due
exactly at a block's start now waits for the block's end, as time must
pass for it.

The shadow validation (two 10-minute games with `--call-each 4`, the
long-call and phase-ladder runs) shows only differences marked as the
harness's, and `--rec on` stays screenshot-identical.

#### Making it reliable: the soak loop (`tools/lift/`)

The lift inputs are game-derived, so they live outside git in
`../dune-lift/`; its README lists them. The scripts read that folder,
or `$LIFT_DATA`:

```sh
tools/lift/lift.sh                    # lift seg000 + the drivers, build both runners
tools/lift/soak.sh check TAG SPEC...  # games without the interpreter, one at a time
tools/lift/merge.sh TAG               # their "Unlifted at" stops (and REC_MISSES) -> leaders.txt
tools/lift/validate.sh TAG            # shadow + call-each + --rec on screenshots
```

A SPEC is `LANG,SEED,SECONDS[,LADDER[,STEP]]`. Each stop names an
address: either a block start to add, or a pattern the lifter should know.
The patterns found so far, now handled in general:
- **Code rewritten through `mov cs:[A], ...`** (`static_rewrites`). An
  immediate stored gives the instruction's form with it; a register stored
  marks the instruction rewritten, its value read as it runs.
  - This was a real bug. The graphics driver turns `add di,140h` into
    `sub di,140h` by storing one ModRM byte, and no dump had caught that
    form: the intro drew a scene wrong at 309 s and crashed at 328 s,
    under `--rec on` too.
  - Shadow mode missed it: it checks the first 20 calls of a routine
    (`REC_SAMPLES`, `REC_FROM` change that).
- **Jump tables:**
  - `jmp cs:[reg+T]`, bounded by a `cmp` just before it, or unbounded:
    the words pointing at code within 4 KB of the table.
  - From the listing (`listing_leaders.py`): `dw` code pointers, `dd`
    far pointers (the interrupt vectors the program installs), labels
    with no cross-reference.
- **After a `div`/`idiv`, a block start:** the program's divide-error
  handler returns past the instruction.
- **The graphics driver's `1234h` placeholder immediates:** they are
  patched through computed addresses, so they are read as the code runs.
  The rewritten `loop` families' entries are in
  `drivers/vga-rewritten.txt`.
- **Hand ports that give up** on a case they don't handle (`loc_03385`: a
  divide overflow). Without the interpreter, the generated code at that
  place runs instead.
- **Breakpoints (the phase ladder, `--call-each`):** without the
  interpreter, Rust runs one block at a time while one is set.

Measured (rounds c9-c10): 20 of 21 games ran to their deadline without
the interpreter:
- the 900 s intro;
- nine hour-long random-input games in six languages, five of them with
  the phase ladder;
- game 303, which has the phase ladder too.

They ran at 170 to 234 MIPS. Afterwards, 1 of 2 new games stopped: the
phase ladder had forced a stage in which a voice file is loaded over the
sound driver, which the game then calls. The same happens with the
original (seed 205 freezes there under the plain interpreter). Not
something the lift can fix; such games are left out.

The validation after these changes is unchanged from before them:
- the same differences, all marked as the harness's;
- 1,222 routines verified equal in the first game;
- `--rec on` 10/10 screenshot-identical.

## Next steps

1. **More readable ports**, the main work from here: name the data segment
   (globals and the record layouts: locations, troops, the dialogue
   scripts) and port routine by routine, leaves first. Shadow mode plus
   `--call-each` is the oracle, so this can run as a loop: rewrite, check,
   keep only what stays equal.
2. **Coverage of the rest** (the 20% of instructions never run): milestone
   saves, and harness states deeper in the game (the endings: `chankiss`
   etc.).
3. **The soak loop** (section 7): more seeds, longer games, other ladder
   steps, until rounds come back clean. `tools/lift/soak.sh check`,
   `merge.sh`, `lift.sh`; one game at a time.
4. **The differences marked as the harness's**, re-checked with every call
   (`REC_SAMPLES`, `REC_FROM`): one such mark hid the graphics bug above.
