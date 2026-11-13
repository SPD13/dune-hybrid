# Decompile handoff: making the interpreter-free build reliable

State on 2026-11-12. Updated after the interpreter-free build passed its rounds and the validation. Read this first, then
[decompile.md](decompile.md) (the pipeline; section 7 is the build without
the interpreter).

## Where things are

- **Worktree:** `dune-hybrid-decompile` (next to the main `dune-hybrid`
  checkout), branch `decompile`.
- **This session's work** (committed and pushed on 2026-11-12, validated:
  see "Validation"):

  | File | What changed |
  |---|---|
  | `tools/lift/` (new) | The lift and soak scripts (below). |
  | `crates/lift/src/main.rs` | Static self-rewrites (`static_rewrites`, `cs_store`), jump tables (`jump_table`), `div`/`idiv` block starts, 1234h placeholder immediates, `raw_patched` for seg000 too. |
  | `crates/pc/src/rec.rs` | `REC_DRIVER_FORMS` dumps, timestamps on "not the one lifted", `check_from`, one-block mode, fallback from hand ports to generated code, `REC_TRACE_SKIP`. |
  | `crates/pc/src/lib.rs` | Run loop order: finish pending shadow checks, then the breakpoint, then `rec_hook` (one-block mode needs the breakpoint before Rust; `--call-each` needs the checks before the breakpoint). |
  | `crates/runner/src/main.rs` | `REC_INTERP`, `REC_DRIVERS`, `REC_SAMPLES`, `REC_FROM`. |
  | `Cargo.toml` | `[profile.release.package.dune-rec] debug = 0` (compile memory). |
| `docs/decompile.md` | Section 7: the soak loop, the patterns now handled, results; next steps. |
| `CLAUDE.md`, `docs/decompile-handoff.md` | Pointer to this handoff, and this file. |

- **Lift inputs (game-derived, never in git):**
  `..\dune-lift\` (next to the worktree), with its own `README.md`:
  - `cov/`: coverage files.
  - `drivers/`:
    - `drv*/`: driver dumps.
    - `forms/`: driver forms found by soak runs; two graphics forms so far.
    - `overwritten/`: dumps of driver memory overwritten by data. Kept
      out of the lift on purpose.
    - `vga-rewritten.txt`: graphics instructions known to be rewritten.
    - `dcov*.txt`: driver coverage.
  - Block-start lists:
    - `leaders.txt`: 993 extra block starts, grown by `merge.sh`.
    - `leaders-hand.txt` and `leaders-listing.txt`: regenerated on every lift.
  - `ref/off/`: reference screenshots for `validate.sh`.
  - `runs/`: soak results.
  - `backup/`: the generated files before the last relift.
- **Builds:** `target-hand` (with the interpreter) and `target-noint`
  (without) are both built from the latest lift and the latest code.

## The loop (scripts in `tools/lift/`, all read `common.sh`)

```sh
cd dune-hybrid-decompile
tools/lift/lift.sh > ../dune-lift/lift.log 2>&1   # lift seg000 + 3 drivers, build both runners (~10 min)
tools/lift/soak.sh check TAG SPEC...               # games on the build without the interpreter
tools/lift/soak.sh collect TAG SPEC...             # games on the build with it: REC_MISSES, REC_INTERP, driver forms
tools/lift/merge.sh TAG                            # add misses and "Unlifted at" stops to leaders.txt
tools/lift/validate.sh TAG                         # shadow + call-each + --rec on screenshots vs ref/off
```

- **SPEC** is `LANG,SEED,SECONDS[,LADDER[,STEP]]`:
  - `ENG,401,3600`: a random-input game;
  - `GER,403,3600,70:70`: with the phase ladder from 70 s, one phase every
    70 s;
  - `FRA,404,3600,,0.2`: clicks every 0.2 s;
  - `ENG,0,900`: the intro with no input.
- **Each round:** `check` a set of games, then look at each `Unlifted at
  CS:IP` stop. If it's a plain missing block start, `merge.sh` it. If it's
  a pattern, fix the lifter. Then relift and check again.
- **One game at a time**, always (see "Memory").

## Results this session

| Round | Games run to the end without the interpreter | Stops fixed after it |
|---|---|---|
| c3 | 20 of 23 | intro jump table, phase-ladder breakpoint, 2 block starts |
| c4–c6 | 4/9, 4/9, 3/9 (stops moving later each time) | listing code pointers, unbounded tables, bail-retry rule |
| c7 | 1 of 9 (the phase-ladder games now reached `3385`) | hand port fallback, `dd` interrupt vectors |
| c8 | 1 of 10 | `div` block starts, second graphics stub family |
| c9 | **9 of 10** (intro 900 s and all phase-ladder games but one) | unreferenced listing labels (game 303 at `1000:283e`) |
| c10 | **11 of 11**: nine hour-long games (six languages, five with the ladder), game 303, the intro | |
| c11 | 1 of 2 | none to fix: game 410 is the voice-file-over-the-sound-driver case below |
| c12 | **7 of 7 two-hour games** (seeds 501-507, six languages, ladders 70:35 to 70:200) | none |
| c13 | **5 of 5 four-hour games** (seeds 601-605, clicks 0.15 s to 3 s, ladders 70:45, 70:60) | none |

Speed without the interpreter: 180–220 MIPS. The interpreter alone runs
about 50.

## What was found and fixed (and why)

1. **Bug: the lift missed self-rewritten instructions.** The graphics
   driver turns `add di,0x140` into `sub di,0x140` by writing one ModRM
   byte (`mov byte ptr cs:[0xC43],0xEF`); no dump ever caught that form.
   The intro drew one scene wrong at 309 s and crashed at 328 s, in
   `--rec on` too.
   - The lifter now reads every `mov cs:[const], …` aimed at an
     instruction. An immediate value gives a known form; a register value
     marks the instruction rewritten, and its value is then read as it
     runs.
   - This runs in the drivers and in seg000 (b270, c13b, efd5).
   - Shadow mode missed it because it checks only the first 20 calls of
     each routine. `REC_SAMPLES` and `REC_FROM` lift that limit.
2. **Jump tables:**
   - `jmp cs:[reg+T]`, bounded by a `cmp` before it, or unbounded: the
     words pointing at code within 4 KB of the table.
   - From the listing: `dw` code pointers, `dd 0000XXXXh` interrupt
     vectors (e94a: the divide-error handler), and labels with no
     cross-reference. All from `listing_leaders.py`, 329 starts.
3. **Phase ladder without the interpreter.** The breakpoint harness used
   to interpret. Now Rust runs one block at a time while a breakpoint is
   set. Its planned stops don't count toward the "bailed twice with no
   progress" rule.
4. **Hand ports that give up.** `loc_03385` declines a divide overflow.
   Without the interpreter, the generated code at that address takes over
   (`Rec::generated`), with a guard against looping. The generated code
   raises the divide error correctly.
5. **After a `div`/`idiv`, a block start.** The game's `INT 0` handler
   returns 2 bytes past the `div`.
6. **Graphics driver placeholders.** Immediates `0x1234` are patched
   through computed addresses: every one is read as it runs. The second
   stub family (`1728`, `17a9`, `17c1`, `17cf`, reached by the rewritten
   `loop` at `1777`) is in `vga-rewritten.txt` and `leaders.txt`.

**Not bugs** (checked against the plain interpreter):
- **Seed 205 with ladder 70:30 freezes from about 140 s; game 410
  (ladder 70:60) stops at `5635:0591` after 1090 s.** Both games' sound
  driver code differs in exactly the same ranges. The game loads
  a voice file over the sound driver and then runs that data as code. The
  phase ladder's forced story stages cause it. Leave such games out.
- **"Driver not the one lifted" right after such an overwrite** is
  correct. Dumps of overwritten memory go in `drivers/overwritten/`, never
  in `forms/`: diff a new form first, only within the lifted code ranges.

## Validation (after all the changes)

`tools/lift/validate.sh h201`:
- The same differences as at `be055c8`, all marked as the harness's.
- 1,222 routines verified equal in game a (1,215 before).
- Hand ports: 1 with a difference (`loc_06447`, as before).
- `--rec on`: 10/10 screenshots identical.

The first run (`h200`) caught a regression from the run-loop reorder,
since fixed: `--call-each`'s checks were cut off at its return
breakpoint, so about 300 routines went unchecked.

## Memory

Claude Code kills background jobs when the machine runs low on memory.
That happened 6 times in the first part of the session, mostly while
compiling `dune-rec`: 15 MB of generated Rust, compiled as one crate,
twice per lift.
- After the user closed Chrome and SteamVR, every compile went through.
  The compiles now run in the foreground (about 5 min each, under the
  10 min limit).
- To stop the kills altogether, start Claude Code with
  `CLAUDE_CODE_DISABLE_BG_SHELL_PRESSURE_REAP=1`.
- After a kill, do not restart on your own: report and ask.
- If it keeps happening: split `generated.rs` into several modules, or
  give `dune-rec` `opt-level = 1` (not measured).

## Next steps, in order

1. **Keep the emulator working.** The interpreter stays the default build
   and `--rec` stays opt-in (user's request, 2026-11-12): see "Choosing
   how the game runs" in `decompile.md`. Commit only when the user asks,
   with the no-reply email; never `generated*.rs` or `dune-lift/`.
2. **More soak rounds:** new seeds, longer games (2 hours or more), other
   ladder steps.
   - `soak.sh check`; each stop gets `merge.sh` or a lifter fix, then
     `lift.sh`.
   - Leave out games where the ladder makes the game overwrite its sound
     driver: check with `REC_DRIVER_FORMS` and `codediff`.
3. **Re-check (done 2026-11-12, see `decompile.md` section 3):**
   - `loc_06447`: its mark came from the data word at 64EDh. A real flags
     difference in its port (the circle walk's calls) is fixed.
   - Real play with `REC_SAMPLES=2000` in English, German, the ladder, the
     intro and two late windows: one real bug, in `draw_speech_bubble`
     (fixed), and nothing else.
   - Still marked, by the harness's random calls only: `vga_slot_22`,
     `sub_f1a0`, `parse_command_line`, the sound slots.
4. **The readable ports** (`docs/decompile.md`, Next steps 1): started.
   - The routines left without a port: `python tools/lift/unported.py`
     (entry, size, the listing's label).
   - About half of them have no label in the listing and start in the
     middle of an instruction or a routine (`181c` is `into`): call
     targets only the harness reaches. Leave them generated.
   - Left on purpose for now: `start`, `game_loop`, `play_intro`,
     `parse_command_line`, the interrupt handlers (`pit_timer_callback`,
     `hnm_render_timer_callback`), the XMS and keyboard set-up.
   - Check each batch with real play in two languages, not only the
     validation.
   - Video scenes (2026-11-12): checked in real play with `REC_ONLY` and
     `REC_BUDGET` raised, and from the test saves (`--save`, `load:1`;
     Day 10, 13 and 23). Two port bugs found that way: `sub_b1ee` counted
     one instruction too many, and `loc_0a25b` skipped the stack write of
     `push ds; pop es`. `loc_01566` and `loc_0491c` stay out (no run
     reaches them).
   - Indirect calls: the listing writes `call word ptr [x]` for far calls
     too. Take the generated code's comment (`call far dword ptr`) as the
     truth: far calls go through `interp`.

## Diagnostics added (environment variables)

| Variable | Effect |
|---|---|
| `REC_MISSES=FILE` / `REC_INTERP=FILE` | Where Rust had no block / where the interpreter ran (build with the interpreter). |
| `REC_DRIVER_FORMS=DIR` | Dump a lifted driver whose code changed in place, once per form. |
| `REC_DRIVERS=graphics,sound` | Use only those lifted drivers (bisecting a divergence). |
| `REC_SAMPLES=N`, `REC_FROM=SECONDS` | Shadow-check the first N calls per routine, only from that time on. |
| `REC_DIFF_LOG=1` | Print each differing call with its virtual time and instruction count. |
| `REC_HAND_SKIP=8f28,...` | Leave those hand ports out (hex entries): which port a difference comes from. |
| `REC_ONLY=0e77,c8fb,...` | Shadow-check only those entries (long calls then have the pending slots to themselves; much faster). |
| `REC_BUDGET=N`, `REC_EVENTS=N` | Longest call checked (instructions, default 50,000,000) and recorded events kept (default 2^21): raise both for videos and scenes. |
| `--save FILE` + event `T:load:1` | Start real play from a save: the game's own load (1000:B3B0) at the next main-loop top. Test saves (Day 1 to 23, never in git): `../dune-hybrid/tests-data/saves/`. |
| `REC_TRACE_SKIP=1` | When a bail repeats with nothing changed, print what was due. |

**Divergence hunting that worked:**
1. Take screenshots every second of the plain interpreter and of
   `--rec on`, and find the first frame that differs.
2. `REC_NO_DRIVER=1` / `REC_DRIVERS=x` to find the part that diverges.
3. Shadow mode with `REC_FROM` a few seconds before the divergence and a
   large `REC_SAMPLES`.
