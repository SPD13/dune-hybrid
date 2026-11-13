# From exact ports to readable Rust

The decompile has two kinds of Rust code for the game's routines:

- **The generated code** (`crates/dune-rec/src/generated*.rs`): every
  instruction of the original, made by the lifter from your own
  DNCDPRG.EXE. Not in git.
- **The hand ports** (`crates/dune-rec/src/hand/`): one function per
  routine, written from the listing. Nearly every routine of the game's
  code segment has one; what is left either isn't a real routine or never
  runs in this setup (see "What is left" below).

The hand ports are exact: each one counts the original's instructions
(`c.run(at, n)`), keeps the CPU flags as the original leaves them, and
works on the 8086's registers and memory. That is what lets the shadow
check compare every call with the original, but it is not yet code a
person would write. This plan is the next step: Rust that reads like a
program about Dune, still checked against the original at every step.

## What "readable" means here

1. **Named game state instead of offsets.** `troop.activity()` instead of
   `c.byte(si + 3)`, `GameTime::now()` instead of `c.word(2)`.
2. **No flag bookkeeping where nobody reads the flags.** A port keeps
   flag-exact results only where a caller tests them.
3. **No instruction counts in the logic.** Timing stays exact, but as one
   cost per path, not interleaved with the code.
4. **Real control flow.** Returned values instead of registers, enums
   instead of magic numbers, loops instead of labels.

Steps 1 to 3 keep the port exact, so the shadow check still compares every
call. Step 4 eventually leaves the 8086 model behind, and only whole-run
checks can follow it.

## Stage A: readable but exact (the shadow check still applies)

Work module by module (troops first: small records, many callbacks, all
reached in real play). For each:

1. **Typed views of the data.** A `game` module with accessors over the
   game's memory: `Troop` (27-byte records), `Location`, `Npc`, the
   dialogue state, the clock. Each accessor reads and writes the same
   bytes the ports use today. The layouts come from the listing and the
   ports already written. The OpenRakis notes give save-file offsets: use
   them only as hints, and check each against the listing (the clean-room
   rule).
2. **Drop dead flags.** `tools/lift/flags_used.py` lists, for each
   routine, the flags any caller reads after calling it. The current
   result:

   | Routines | Flags their callers read |
   |---|---|
   | 777 | none: the port can drop every flag-exact helper on its result |
   | 149 | some: 66 only CF, 11 only ZF |
   | 263 | unknown: only called indirectly, through tables (assumed all) |

   Refining the indirect ones (their call sites are in the coverage
   files' indirect targets) is the first task of this stage.
3. **Timing as a cost.** Replace the interleaved `c.run` calls with one
   cost per straight path, computed from the listing by a helper (the
   `n` arguments are already sums of path lengths). The bail points stay:
   a port must still stop where an interrupt is due. That is the one
   constraint the readable code keeps from the 8086.
4. Each port, as it is rewritten, still has to pass the shadow check in
   real play (English and German, the saves) and `validate.sh`.

## Stage B: native subsystems (whole-run checks)

Once a subsystem's routines are readable and exact, its state can move
out of the 8086 memory into Rust structs, kept in step with the memory at
the subsystem's boundary. Candidates, smallest boundary first:

- the game clock and the time of day (a few words: [2], [46DBh], the
  timer);
- troops and their jobs (the troop table, the callbacks);
- the dialogue state machine;
- the map's projection and drawing (the HD renderer already models it).

The shadow check no longer applies inside such a subsystem. Two checks
replace it:

- **`tools/lift/suite.sh check`**: the save suite. Seven games (from the
  start in English and German, and from the Day 1, 10, 13 and 23 saves)
  are compared with a recorded reference: the whole memory every 10 s,
  and every screenshot. Until a subsystem stops keeping the 8086 memory
  in step, the memory must stay identical (once the first task
  below makes the reference runs agree).
- **Save files as state**: load a save, play scripted input, write a
  save; the written save (the game's own serialisation of its state) must
  match the reference byte for byte. This is the check that still works
  once the state lives in Rust structs. It needs a scripted "save now"
  event in the runner; that is the first task of this stage.

## Tools in place

| Tool | What it checks |
|---|---|
| `--rec shadow` (`REC_ONLY`, `REC_BUDGET`, `REC_SAMPLES`) | Each port's call against the original, from the same state. |
| `tools/lift/validate.sh` | All ports in two 10-minute games and the harness, plus `--rec on` screenshots. |
| `tools/lift/hashcmp.sh` | One game, plain emulation against `--rec on`, the whole memory every 10 s (`STATE_HASH=1`). |
| `tools/lift/suite.sh record\|check` | Seven games from the start and from the saves, against a stored reference. |
| `tools/lift/flags_used.py` | Which routines' returned flags any caller reads. |
| `--save FILE` + event `T:load:1` | Real play from any point of the game (the test saves, Day 1 to 23). |

## First task: make the suite clean

`suite.sh check` (2026-11-12): five of the seven games are identical to
plain emulation at every 10 s state; Day 1 differs from 100 s, Day 10
from 230 s, in four bytes: the random generator's state (DS:D826h) and
its last number (DS:0000). The screens are identical. Both come from the
`--rec on` run loop and the lifted drivers, not from the hand ports (the
same with `REC_NO_HAND=1`):

1. **Loading, 0.787 s (15,740,135 instructions):** the sound driver's IRQ
   test (driver 03D1h: it starts the card, then counts down in a loop at
   0409h until the card's interrupt sets [28Eh]) runs one loop pass more
   (3 instructions) with the lifted sound driver; interpreted
   (`REC_DRIVERS=graphics,music`), the counts match. From then on the
   instruction count is off by those few instructions.
2. **Day 1, 94.749 s (486,829,174 instructions), with the sound driver
   interpreted:** at 1000:DC2Fh (`redraw_mouse`) the plain run takes the
   sound card's interrupt within 10 instructions; `--rec on` runs on in
   Rust and takes it later. Rust code does not stop for that interrupt
   when it falls due, so the first thing to look at is how `rec_tick`
   learns of it (`hw.update`'s next event, `reschedule`).

Each changes how many passes the main loop's waits make, so `rand` is
called a different number of times. Tools to find such a point:
`STATE_FROM=SECONDS` with a small `--shot-every` and `NO_PNG=1` (state
hashes, CS:IP and the instruction count in a window), `STATE_DUMP=1`
(the memory at each shot, to compare byte by byte).

## What is left of the exact ports

- `loc_01566` and `loc_0491c`: ported, but no run reaches them, so they
  stay out of the table.
- `exit_with_error`, `out_of_memory_error`: they end the program; no check
  can follow them. Leave them generated.
- The XMS routines (`xms_memory_func_1`, `initialize_himem_sys`,
  `xms_move_memory`), `reset_keyboard`, `clear_and_retry`: no run with
  the emulator's setup reaches them (not in any coverage file). Port them
  only together with a run that does.
- About 50 entries without a label in the listing: they start in the
  middle of an instruction or a routine, and only the harness reaches
  them. Leave them generated.
