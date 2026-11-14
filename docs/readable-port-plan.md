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
   A jump into the middle of another routine's code (one whose code
   reaches there from its entry) makes the RETs in that code return to
   the jumper's callers too: that counts for what is read after the calls
   made in the shared code and for its tail jumps (code run on behalf of
   the routines that jump into it), not for the routine's own mask (its
   port runs only from its entry). `--why ADDR` shows where a mask comes
   from. When the analysis changes, the ports the tools converted whose
   mask is no longer 0 go back to their exact form (masks only shrink
   safely).
   Whatever it cannot follow counts as read: code never run (not in the
   generated code), an indirect target no run took. INT and far calls
   (BIOS, DOS, the drivers) pass the flags through. The current result:

   | Routines | Flags their callers read |
   |---|---|
   | 712 | none: the port can drop every flag-exact helper on its result |
   | 61 | some (mostly CF or ZF alone) |
   | 416 | all, or unknown |

   `flags_used.py --rust` writes the table
   (`crates/dune-rec/src/hand/flags_read.rs`); the shadow check takes
   the original's value for every flag no caller reads, so a port that
   drops them still compares equal. A flag the port leaves different
   can still reach memory through an interrupt's frame on the stack where
   the port stops: that lands in the dead stack, which the checks leave
   out (see "Where a port stops" below).
3. **Timing as a cost.** Replace the interleaved `c.run` calls with one
   cost per straight path, computed from the listing by a helper (the
   `n` arguments are already sums of path lengths). The bail points stay:
   a port must still stop where an interrupt is due. That is the one
   constraint the readable code keeps from the 8086.
4. Each port, as it is rewritten, still has to pass the shadow check in
   real play (English and German, the saves) and `validate.sh`.

### Progress

The troops (`hand/troops.rs`, with the views in `hand/game.rs`: `Troop`,
`Location`, the troop state bits, the game time, charisma, the location
the player is at, a location's needs and chain) are done: every routine
in the readable shape, each checked equal call by call in the saves and
the harness, but three whose result is the flags themselves, which stay
as they are: `troop_ptr` (the flags of byte 3 against 80h),
`troop_particle` (ZF through `loopne` / `jcxz`) and the
`set_troop_bit_10` family (CF made by `stc` / `cmc` over a test, kept
by `pushf` / `popf` and returned).

Dialogue (`hand/dialogue.rs`) is about half done the same way: the
phrase and sentence steps, the text's frame task, line breaking, the
speakers and their gauges, the phrase over the map, the talking head and
speech bubble, the dialogue state routines. Left as they are: the rect
comparisons (`rects_missing` and on: flags as data, the `and` of two
string compares), `line_done` and its tails (a pushed return address,
calls through BP), `layout_text`, `format_interpolated_string`,
`say_dialogue_entry`, the book page and text box.

**Dead flags dropped everywhere else, mechanically.**
`tools/lift/drop_flags.py` takes each port whose callers read no flags
and removes its flag-only statements, turning the rest of the flag
helpers into their flag-free forms (`hand/pure.rs`: `pure::add(W, a, b)`
for `c.add(W, a, b)`, same results). It leaves a port alone when
anything could still see those flags: it reads a flag itself, calls or
jumps to a routine that reads flags on entry or one the analysis doesn't
know, calls through a register, has an INT or far call run, or hands its
context to a helper that could (followed through the helpers).
`tools/lift/drop_unused_reads.py` then removes the reads only the
dropped compares used. This converted about 300 ports across the other
modules (story 58, map 46, the tails, UI, panels, screens and more). The
ports whose callers read flags (about 430) need their exit flags kept,
which a line-by-line tool can't place safely: they are for the hand
rewrite, module by module.

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
- **Where a port stops.** A port stops (bails) at the start of any
  `run` when an interrupt is due there, and the interpreter goes on from
  that address with the port's state. Two things then see its flags. The
  interrupt pushes them below SP: that word is in the dead stack, which
  the shadow compare and the state hash leave out (`pc::rec::dead_stack`:
  SS:[39C0h, SP) while SP is in the game's stack, 39C0h..3CBCh). And the
  original's code from there might read them: `flags_used.py --at ADDR..`
  lists what each such address reads; in the troops every such point that
  reads any comes right after a call, where the flags are the callee's.
- **Flags at the exits.** A routine whose callers read flags (mask not 0)
  returns them exact: the readable port does the original's last
  flag-setting instruction of that path with the flag helper (one line,
  commented). The masks grew when `flags_used.py` learned that a jump
  into the middle of another routine (`troop_set_out` into `troop_move`
  at 8313h) makes that routine's RETs return to the jumper's callers.
- **Flags passed through.** Some routines return the flags they were
  entered with (`remove_frame_task` with no tasks, through `jcxz`; the
  front-buffer and font switches): a port that jumps to one, or returns
  right after calling it, must enter it with the flags its own callers
  read. `flags_used.py --in` lists, per routine, the flags it reads on
  entry and those that can reach its RET unwritten.
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
- **Stretches through an interrupt.** When the replay runs a stretch
  through an interrupt, it checks the stretch by interpreting it without
  the interrupt and comparing with the port's state. Its result flags no
  longer count there (`rec_settle`): the port goes on from the original's
  state, and flags something reads would differ in the many calls without
  an interrupt. (`sum_locations_a8` showed this: the original's `cmp`
  before its next `call` left flags the readable port doesn't set.)
- **Memory the original reads twice.** Read it again where the original
  does: `link_troop` marks slots in a table on the stack, and in a
  harness state a mark landed on the troop's own word 10h, which the
  original tests afterwards.
- The shadow replay runs on to the original's instruction count: a port
  that counts too few shows as a difference in whatever the caller does
  next (here AX set by the caller), not as a count.

Open: in a harness run checking every routine (200 calls each),
`globe_fill_blue_background` (B84Ah) left SP 2 lower than the original
in 1 call of 52 (around its `c.interp` of the driver's fill). Not looked
into yet. `hnm_render_timer_callback` differed in 8 of 204, all with the
code segment written (a harness artifact).

- **INT and far calls.** A port that has the interpreter run an INT or a
  far call (`c.interp`) keeps its flags exact before it: an INT pushes
  them and its service sees them (the check compares a service's
  registers and flags), and a far call's code isn't followed by the
  analysis.
- **The harness's stack.** The harness calls routines with a random SP,
  sometimes outside the game's stack; there the flags an interrupt pushes
  where a port stops are not in the dead stack the check leaves out, and
  show as a memory difference of one word (`swap_byte_arrays`, 1 call in
  54). Not a difference in play, where SP is always in the stack.

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
