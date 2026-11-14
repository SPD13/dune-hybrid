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
   routine, the flags any caller reads after it returns. It follows the
   code after every call site (direct ones, and the indirect calls and
   jumps the coverage runs saw), into callees (what they read on entry,
   and which flags reach their RET unwritten), and through tail jumps.
   Whatever it cannot follow counts as read: code never run (not in the
   generated code), an indirect target no run took. INT and far calls
   (BIOS, DOS, the drivers) pass the flags through. The current result:

   | Routines | Flags their callers read |
   |---|---|
   | 758 | none: the port can drop every flag-exact helper on its result |
   | 63 | some (mostly CF or ZF alone) |
   | 368 | all, or unknown |

   `flags_used.py --rust` writes the table
   (`crates/dune-rec/src/hand/flags_read.rs`); the shadow check takes
   the original's value for every flag no caller reads, so a port that
   drops them still compares equal. One limit: a flag the port leaves
   different can still reach memory through a later `PUSHF` or an
   interrupt's frame on the stack; the whole-run checks would show it.
3. **Timing as a cost.** Replace the interleaved `c.run` calls with one
   cost per straight path, computed from the listing by a helper (the
   `n` arguments are already sums of path lengths). The bail points stay:
   a port must still stop where an interrupt is due. That is the one
   constraint the readable code keeps from the 8086.
4. Each port, as it is rewritten, still has to pass the shadow check in
   real play (English and German, the saves) and `validate.sh`.

### Progress

Started with the troops (`hand/troops.rs`) and the views in
`hand/game.rs` (`Troop`, `Location`, the troop state bits, the game
time, charisma, the location the player is at, a location's needs and
troop chain). Rewritten so far (about 60 routines), each checked equal
call by call in the saves and the harness:

- motivation, skills and charisma: `troop_activity`, `stamp_record`,
  `add_motivation`, `add_motivation_to_all`, `mark_troop` and its two
  entries, `add_skill`, `troop_motivation`, `start_troop`,
  `lose_charisma`, `troop_joins`, `troop_skills_decay`;
- the troop callbacks of the loops: `tally_yield`, `count_by_job`,
  `count_4e04`, `clear_bit_7289`, `troop_68da`, `troop_6e0f`,
  `mark_troop_7506`, `sum_troop_6155`, `troop_8403`, `tally_72a2`,
  `census_34d0`, the raid callbacks (`raid_troop_762a`, `_75ea`,
  `_75af`, `lose_troop_764d`) and `repeat_764d`;
- jobs: `set_job_6ad4` (and `kind_6acb` in `tails.rs`),
  `set_troop_activity`, `redo_job_6ecb`, `for_troop_location`;
- yields and rates: `troop_yield`, `troop_rate_by_age`,
  `scaled_by_troop`;
- the day: `troop_day_update`, `troop_event_roll`, `maybe_mark_troop`,
  `grow_location`;
- a location's needs and bits: `add_location_sum`, `lower_by_bits`,
  `count_location_bits`, `skill_needs` (the three share `BitWalk`),
  `spread_location_bits`, `pack_location_bits`, `clear_location_4`;
- the chain and the map: `reset_troop`, `unlink_troop`, `link_troop`,
  `remove_particle_4752`, `mark_troop_location`, `troop_facing_sprite`,
  `map_distance` (7C63h, 7C8Fh), `call_29f0_f`,
  `for_location_troops_72a2`.

The shape of a rewritten port: read what it needs through the views,
work out the result and the instruction count of its path in plain Rust
(no flag helpers where no caller reads the flags), `c.run(start, cost)`
once (it bails before anything is written if an interrupt falls inside),
then write the results. Where the original calls another routine, the
port still stops there (`c.call`), as that is where an interrupt or the
callee's own bail can come. Registers the original leaves are still set
(callers read them); `c.save` / `c.restore` keep its pushes and pops,
which leave their words in the stack's memory.

Things that bit:

- **Flags on entry.** A port that drops flags must still set those the
  routine it calls or jumps to reads on entry: `flags_used.py --in` lists
  them (55 routines read any; CF for the troop callbacks, given by the
  loops).
- **Writes after the `run`.** Nothing may change before `c.run` (or
  `c.call`): a bail resumes the original at that address with the state
  as it is.
- **`rep` counts per iteration.** The emulator counts each iteration of a
  `rep` string instruction as an instruction. A port that does the copy
  itself adds them to its `run` (`skill_needs`: 8 + 7). Ports that call
  `c.rep` get them counted by it, but their `run` doesn't cover them, so
  an interrupt due inside the iterations is taken after the port's
  stretch, a few instructions late. About 60 older call sites do this; the
  suite shows no drift from it yet, but they should move their
  iterations into the `run`'s check (a follow-up).
- The shadow replay runs on to the original's instruction count: a port
  that counts too few shows as a difference in whatever the caller does
  next (here AX set by the caller), not as a count.

To check a batch: `REC_ONLY=<entries> REC_SAMPLES=200` with
`--rec shadow` in the saves (Day 10, 13, 23: the troops run there, not in
the first ten minutes of a new game) and with `--call-each 4`, then
`suite.sh check`.

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
  in step, the memory must stay identical.
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

## The suite made clean (done)

`suite.sh check` first found two of the seven games different from plain
emulation (the random generator's state, from 100 s and 230 s). Four
causes, all in how Rust code met interrupts, none in what a port
computes:

1. **A hand port's `c.interp` (an INT or far call) ran its instruction
   without checking for an event due right then.** The generated code
   counts the INT in its block's check; the hand ports' `run` before it
   doesn't. `Ctx::interp` now checks first. (Day 1: a timer tick due at
   `int 33h` in `get_mouse_pos_etc` came after the mouse call instead of
   before it.)
2. **A port write that reschedules the devices, mid-block.** The plain
   run loop updates the devices before the next instruction; Rust code
   only at its next block start, so the update (and the time it computes
   the next event from) came a few instructions late. The lifter now
   starts a block after every `OUT` (and `POPF`, which can turn
   interrupts on), as it did after `STI`. (Loading: the sound driver's
   IRQ test loop ran one pass more.)
3. **`STI` in hand ports without its one-instruction hold** where the
   STI is the last instruction counted: `Ctx::sti` keeps the hold.
4. **`POPF` turning interrupts back on in hand ports** (`read_game_port`,
   E948h): the count now stops right after it, so a pending interrupt is
   taken there.

The suite also wasn't hermetic: the game reads and writes its saves in
the output folder, and saves left there by an earlier run changed the
next. Each case now starts from an empty folder.

Tools to find such a point: `TRACE_IRQ=INSN` (every interrupt taken,
every device update and idle skip, every sound port write, from that
instruction count on; diff a plain run with a `--rec on` one),
`STATE_FROM=SECONDS` with a small `--shot-every` and `NO_PNG=1`,
`STATE_DUMP=1`.

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
