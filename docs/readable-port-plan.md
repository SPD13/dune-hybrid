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
   from. An INT counts as reading all flags (its service sees them).
   When the analysis changes, `tools/lift/recheck_dropped.py BASE` puts
   the ports the tools converted (those with `pure::`) back to their
   exact form where the conversion's rules would now refuse them (masks
   only shrink safely); hand rewrites (anything but the tools' own
   output, and the ports listed in `tools/lift/hand_ports.txt`) it only
   lists, for a look.
   Whatever it cannot follow counts as read: code never run (not in the
   generated code), an indirect target no run took. INT and far calls
   (BIOS, DOS, the drivers) pass the flags through.

   Besides the generated code's routines, every direct call target and
   every hand port's entry counts as a routine (some start inside a
   generated routine's code: `setup_lip_sync_data_from_sprite_sheet` at
   91A0h), and the targets of indirect calls get what they read on entry
   from their own code. The call-each harness calls through registers
   holding anything (`call bp` in C097h with a random BP): of its
   indirect targets only those with a port count. An entry that starts in
   the middle of an instruction of the ported routines' code (only the
   harness reaches one) is no routine: a RET after it belongs to the real
   one. And the instruction after another is the one the generated code
   lists next in its block, not the next address decoded (code decoded
   from such an entry overlaps it). What is read after a call follows the
   code on to a RET, and that RET returns to the callers of the routine
   the call was made in (or of the routines that jump into its code),
   whichever routine's code the RET is in. A jump counts only where a
   game could make it: not from a routine without a port that nothing
   calls, at an instruction no game ran (code decoded from the middle of
   an instruction, which only the harness reaches). The current result:

   | Routines | Flags their callers read |
   |---|---|
   | 821 | none: the port can drop every flag-exact helper on its result |
   | 111 | some (mostly CF or ZF alone) |
   | 257 | all, or unknown |

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

Dialogue (`hand/dialogue.rs`) is done the same way: the phrase and
sentence steps, the text's frame task, the text's layout and line
breaking, the speakers and their gauges, the phrase over the map, the
talking head and speech bubble, the book page and text box, the entries
said, the clicks, the head's lip sync set-up and the rects the head
changed (the `lahf` bytes of its compares as data, `pure::lahf_cmp`).
What flag helpers are left are the exits' flags and those an indirect
call or the driver sees.

Map, story, the globe, UI, voice and panels: their ports whose callers
read no flags but which the tool below can't convert (a driver call, a
flag used as data such as `rcl` from a seed, a `pushf` / `popf` pair)
are being converted by hand: everything but the flags a driver call or
an indirect call sees. `pushf` / `popf` pairs still push the exact flags
(the word stays below SP, in the dead stack, but is written).

The engine modules (dos, files, hnm, input, draw, frame, text) were
then done the same way, by hand: each flag helper kept is commented
with its reader (an exit the callers read, an INT or driver call, a
callee that reads flags on entry, a stop where the original reads them).
576 flag-helper calls are left in the hand ports, nearly all of them
such readers'. The start-up before the game's timer handler
(`measure_pit_e85c`, `initialize_mouse`, `initialize_joystick`,
`read_game_port`, `open_dune_dat`) keeps its flags at every stop (see
"Where a port stops"), and `parse_command_line` still differs in the
harness, so those are left as they are.

**Dead flags dropped everywhere else, mechanically.**
`tools/lift/drop_flags.py` takes each port whose callers read no flags
and removes its flag-only statements (a port that reads a flag only right
after a call, the callee's, counts as not reading any), turning the rest of the flag
helpers into their flag-free forms (`hand/pure.rs`: `pure::add(W, a, b)`
for `c.add(W, a, b)`, same results). It leaves a port alone when
anything could still see those flags: it reads a flag itself, calls or
jumps to a routine that reads flags on entry or one the analysis doesn't
know (for a jump into the middle of one, what is read from there on:
`flags_used.py --at`), calls through a register (not a loop over a
literal list of targets), has an INT or far call run that doesn't get
the flags of a call just before it, or hands its
context to a helper that could (followed through the helpers), or can
stop where the original goes on reading flags (a `run` that is not a
call's return: `check_stops.py`). Reading DF (the string direction) is
no reason: no operation sets it. A port
whose callers do read flags is converted too when it never returns
itself: it ends only in tails to routines that read no flags and pass
none through to their RET, so its callers see theirs. A port with
several entries (`match start`) counts the masks of all of them.
`tools/lift/drop_unused_reads.py` then removes the reads only the
dropped compares used. This converted about 400 ports across the other
modules (story, map, the tails, UI, panels, screens and more). The
ports whose callers read flags (about 390) need their exit flags kept,
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
  them (33 routines read any; CF for the troop callbacks, given by the
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
  `tools/lift/check_stops.py` lists the `run` addresses that are not a
  call's return and from which the original reads flags: there the
  port's flags must stay exact (`transfer_sd_block` stops before an `adc`
  that reads the CF of its `shr`; `text_box_for` before a `je` after
  `dec cx`). One more reader at the start-up: until the game installs
  its own timer handler (`install_interrupt_handlers`), a tick goes to
  the BIOS's INT 08h service, which the shadow check compares with the
  flags it is given; so every stop of the code run before that keeps
  its flags exact (`initialize_system` up to E632h, `open_dune_dat`,
  the start of `get_res_index_by_name`). After that the game's own handler
  still chains to the BIOS's every tenth tick or so, with its flags: a
  `dec` before the chain keeps CF, which then comes from the handler's
  `cmp` of the clock's hold (or the frame tasks' return).
- **A shift by CL keeps the flags when CL is 0.** The flags a later
  reader sees may then be those of an earlier instruction: keep that
  one too (`move_cursor_by`: INT 33h after two `shl` by counts that can
  be 0, back to the `test`).
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
  itself adds them to its `run` (`skill_needs`: 8 + 7). `c.rep` counts
  them itself, but the `run` before it doesn't cover them, so an
  interrupt due inside the iterations would be taken after the port's
  stretch, a few instructions late. `c.rep_counted` is the `rep` whose
  iterations the `run` before it counted (`c.run(at, n + CX)`): every
  `rep` site uses it (the count worked out before the `run`: a constant,
  the register, a word on the stack, the copy's size).
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

- **The suite's hash and dropped flags.** The suite's state hash leaves
  out the arithmetic flags (CF, PF, AF, ZF, SF, OF): a port that drops
  flags nobody reads leaves them different from the original's, and a
  hash taken right after one (at the 10-second mark) showed a difference
  in the CPU part only, gone in the next state (`flight_step`, once in
  two of the seven games). A flag that is read and wrong still shows in
  the memory soon after.
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
  timer): done, with a mirror (below);
- troops and their jobs (the troop table, the callbacks), and the
  locations: done, with a mirror (below);
- the dialogue state machine: its text box's layout, the text being
  played and the conversation (47A2h-47C5h) done (below); the rest of
  the 4770h block (talking heads, voice, 4770h-4783h and 47D4h-47EFh)
  first needs names for its bytes;
- the map's projection and drawing (the HD renderer already models it):
  its view (46E3h-46EEh) and its selection (46EFh-46FDh) done (below).

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
  once the state lives in Rust structs. The runner's event `T:save:N`
  makes the game write `DUNE37SN.SAV` at the next main-loop top
  (`create_save_cl`, B389h, as the mirror menu does), and `suite.sh`
  has each of its seven games save 5 s before the end and compares the
  save's bytes with the reference's (all equal with `--rec on`).

### The clock (first subsystem, in Rust with a mirror)

`hand/clock.rs`: the game clock is a `Clock` kept with the machine
(`Machine::native`, a slot for the recompiled code's own state): the
timer's ticks (CE7Ah, CE7Ch), the hold (CEEAh), the stop count (2788h),
the countdown and its period (46DBh, 146Eh), the steps pending (46DDh)
and the game time ([2]). Its logic is its methods: the timer
interrupt's `tick`, `stop` / `release` / `start`, the step that
`run_clock_ticks` runs (`restart_countdown`, `advance`), the day and the
time of day. Every port that counts, stops or reads the clock goes
through it (`c.clock()`, `c.update_clock(..)`, `c.game_time()`,
`c.timer()`); no port touches those words directly.

The memory is a mirror: each change is written through (the suite
compares the memory, and the game's save routine copies those words),
and each access first compares the struct with it. A difference means
code outside the clock wrote them, and the struct takes the memory's
values: a sync-in. This is where the clock and the rest are kept in
step, and why it can't simply own the words yet: any port can give up
partway to the interpreter (at a run deadline, an interrupt due), and
then the original's code goes on writing the memory itself; so do a
loaded save and the shadow check putting the original's memory back.
The runner prints the count (`clock: N sync-ins`, also in each
`suite.sh check` line); `CLOCK_TRACE=1` prints each with the fields that
differed.

The planned boundaries are told to the clock and not counted: a save
loaded (`loc_0b3b0`, now a port, hands the clock the loaded state after
`restore_from_save_memory`), and the runner's own stepping in the
interpreter (to a breakpoint for its events, a rewind:
`clock::outside`). The clock's writers resume in Rust where a call of
theirs gave up partway (`RESUMES`: `run_clock_ticks` after each call,
the timer's tick at EF7Ah, the load after its restore), so the
interpreter or the generated code no longer finishes them. Left: a
device event (the sound) inside the few instructions of the timer's
tick, where the stretch must stop at that instruction and the
interpreter does the tick (about once in 300 s): the memory is then the
clock with one tick more, as `Clock::tick` predicts, and the clock takes
it as such (counted apart, not a sync-in). Any other write fails the
suite: `suite.sh check` wants `clock: 0 sync-ins` in every game.

To drop the mirror later, nothing else may write those words: the
save's image is built from the memory (`create_save_in_memory`), so it
would need the clock's values, and the tick would need to finish in
Rust across such a stop.

### The troops and the locations (in Rust with a mirror)

`hand/record_table.rs`: the game's record tables, the troops (43h of
1Bh bytes at 8AAh) and the locations (46h of 1Ch bytes at 100h), are
kept with the machine (`hand/native.rs` holds the subsystems' state);
the `Troop` and `Location` views (`hand/game.rs`) read and write through
them, with the clock's mirror and check. A troop is also a typed
`TroopRecord` (`hand/troop_table.rs`): `Troop::record` / `store` give a
port the whole record (the harvest's held troop clears its rate and
word 0Eh that way). Writes a port
makes without the view (through `Ctx`) are followed, a save loaded and
the tools' stepping are told; the generated code (the original's code
in Rust, finishing a port that gave up at a stop) is counted apart when
it ran since the record was last verified (`Rec::generated_runs`: a
routine taken from the generated table, or a return into the generated
code from a call it made; a hand port's own calls are marked and don't
count). The runner prints both (`troops: N sync-ins (M written by the
generated code)`, and the same for `locations:`), and the suite wants 0
sync-ins for both in every game, as it does for the clock. `TROOPS_TRACE=1` prints each sync-in and
each write a port makes into the table without the view.

What the sync-ins showed: the troops' job handlers (the table at
CS:6C26h, called from `troops_tick`) had no ports and ran as generated
code. Ported: jobs 0 (`job_harvest`, the spice), 1, 4, 5, 8, 9 and 10;
the sync-ins are now 0 to 2 per suite game (they were up to 53). Job 6
(739Eh) never ran in any run, so it has no generated code to compare
with: left until a run reaches it.

The jobs' exits returned flags their caller doesn't read, but
`flags_used.py` listed them as unknown (all read): no coverage run had
the call through the job table, as the old runs all start a new game,
where the troops have no jobs yet. Coverage from the saves
(`$LIFT_DATA/cov/cov-save*.txt`) and the table itself (`flags_used.py`
takes the job table's targets as known for that call site) make them
known: mask 0, and their exits (tails into `add_skill`,
`set_troop_activity`, `kind_6acb`, whose flags are not exact) are no
longer compared.

### The text box (in Rust with a mirror)

`hand/text_box.rs`: the layout of the box the dialogue's text goes in
(1Ch bytes at 4784h: the margins, the words laid out, the room left, the
inner width, the pen, the inner corner, the style, the line height, the
gaps still a pixel wider, the box shown) is a typed `TextBox`, kept as a
table of one record in `hand/record_table.rs` (so with the same mirror,
checks and counts; `text box:` in the runner's and the suite's output).
The ports read and change it through `Ctx::text_box` /
`update_text_box`; the suite wants 0 sync-ins. The generated code sets
it a few times per game (ports that gave up: counted apart).

The dialogue's text being played (`hand/dialogue_text.rs`, 0Eh bytes at
47C6h: the next phrase, the phrase shown, the head's sentences and frame
table, the phrases left, the sentence kind, the flags, the last sentence
block) is a second table of one, the same way (`Ctx::dialogue_text` /
`update_dialogue_text`; `dialogue text:` in the output). Reads through
SS (the head's tables, while DS is the head's sheet) read the mirror.

The conversation and scene state (`hand/conversation.rs`, 24h bytes at
47A2h: the speaker, the line state, the scene's flags and count, the
device's caller, the first speaker, the command and phrase strings' far
pointers, where a phrase goes on, the pending list, the append point,
the group, the mode, the two heads, the talker) is a third (the
records may now be up to 24h bytes; `conversation:` in the output). The
word the original pushes over the mode and the two heads is
`Conversation::mode_word`.

The map view (`hand/map_view.rs`, 0Ch bytes at 46E3h: the rect shown,
which view is up ([46EBh]: 80h the map zoomed in, 40h a place's view),
the changes seen since it was drawn, the routine that redraws it) is a
fourth (`Ctx::map_view` / `update_map_view`; `map view:` in the
output). The generated routines that fill its rect (given 46E3h in DI)
are counted apart.

The map's selection (`hand/map_selection.rs`, 0Fh bytes at 46EFh: the
troop selected, the record shown, the map screen toggled in, the troop
panel and its skills panel, the animated sprites' frame, the caption's
kind and location, the panel to draw, the place under the cursor) is a
fifth (`Ctx::map_selection` / `update_map_selection`; `map selection:`
in the output). The bytes after it (46FEh-470Fh) are scratch of the
troop panel's computations, left in memory.

Two more from the map's block: the flight (`hand/flight.rs`, 6 bytes at
4727h: flying, the map marker's state, the tick the ring of points last
moved, the steps flown) and the message typed out on the message line
(`hand/typed_message.rs`, 0Ah bytes at 473Fh: the far pointer to its
next character, the pen, the colour; the frame task `sub_46b5` types
it). `flight:` and `typed message:` in the output.

The tables of one share their code: a typed view implements
`record_table::Single` (its `Which`, `from_bytes`, `to_bytes`) and
`record_table::single` / `set_single` / `update_single` keep it (with DS
not the game's, they use the bytes there). Adding one is a `Which`
variant, a field on `Native`, the view, two `Ctx` methods and the name
in `suite.sh`'s loop: `tools/lift/add_table.py SPEC.json` does all but
the view (see its notes), and `tools/lift/drop_consts.py` drops the
globals left unused.

Outside the map's block: the screen buffers (`hand/screen_buffers.rs`,
0Ch bytes at DBD4h: the front buffer's far pointer, the screen's
segment, the segment drawn to, the second buffer's far pointer; the
back buffer [DC32h] apart). The most used word of all the ports
(`active`, [DBDAh]); the generated code switches it often (counted
apart). `screen buffers:` in the output.

The mouse and its cursor (`hand/mouse_state.rs`, 14h bytes at DC34h:
the buttons and the buttons last returned, the position, the range,
where the cursor was drawn, the hide count, hidden for a rect) the same
way; the driver's callback writes it (counted with the generated
code). `mouse:` in the output.

The sound's state (`hand/sound_state.rs`, 14h bytes at DBC0h: the voice
file's position and bytes left, the settings, the music loaded, playing
and asked for, the MIDI driver's returns, when the music was picked).
The music driver's tick in the timer interrupt (generated code, AE28h,
EFC7h) writes it some 5000 times a game: all counted apart, none
unforeseen. Porting that tick would move those writes into Rust.
`sound:` in the output.

The video playing (`hand/video_state.rs`, 40h bytes at DBE6h, the
longest record: the frame counters, the video asked for and open, the
file's position and bytes left, where the frames start, the ring buffer
the file is read ahead into (where data goes, the next chunk, the frame
waiting, where it wrapped, the bytes buffered), the frame's voice and
palette chunks, the timing). The 32-bit file offsets are `u32` fields.
`video:` in the output.

The player's travel on the map (`hand/travel.rs`, 0Dh bytes at 11C1h,
named from the listing: a position kept, the location flown to, the
heading (a byte angle), no target to home on, the screen kind, the
travel held (a call, a dialogue), the heading kept, the step's carried
fraction). `travel:` in the output.

The font's pen (`hand/font_pen.rs`, 8 bytes at D82Ch: where the next
character goes, where the line started). `font pen:` in the output.

The two voice buffers (`hand/voice_buffers.rs`, 8 bytes each at 3811h:
the data's far pointer, its size, the PCM driver's state, the voice
file's part) are the first table of more than one with a typed view:
`record_table::nth` / `update_nth` keep record `i` (and `single` is
`nth(0)`); `Ctx::voice_buffer(i)` / `update_voice_buffer(i, f)`. The
ports that reach a buffer through SI still write its bytes (followed).
`voice buffers:` in the output. The PCM driver's interrupt sets the
states; its lifted code (a driver's space, not seg000) did not count as
the original's code, so its writes showed as unforeseen. `Rec::
generated_runs` now counts the drivers' lifted code and each step of
the interpreter as well (nothing but the hand ports is left out).

The characters (`hand/characters.rs`, 10h records of 10h bytes at
FD8h, the talker's being FD8h + 10h × [47C4h]): where each is (its
place word and position), where a click on it goes (+4, `jmp [si+4]`),
two game times the speaker's mood counts from, +0Eh, and flag bits
(+0Fh). `Ctx::character(i)` / `update_character(i, f)`; the fixed
addresses into the table (some named `BYTE_0FD8`, `WORD_1018`, ..., now
gone) go through it. Most ports still reach a record through SI
(followed). Fields at +6, +0Ch and +0Eh are not understood yet.
`characters:` in the output.

Further reading of the listing named more of it: a character's byte
0Eh is its number, its bit in four sets over the characters at DS:0Eh
(`MET`, ever spoken to), 10h (`PARTY`, following the player: flag 40h,
`joined_at` +8, `left_at` +0Ah), 12h (`PRESENT`, at the player's
place) and 14h (`TALKED_HERE`, spoken to in this room). Its +0/+2
(`x`, `y`) are where it is in the same form as the player's [4]/[6]: a
map position, or with 80h in y's low byte the location y's high byte
indexes (x's low byte then the room). The player's own pair is a table
of one (`hand/player.rs`, `Ctx::player` / `update_player`; `player:`
in the output).

Left, needing names before any grouping: the story state (DS:0-FFh: [0]
the random seed, [4]/[6] the player's place and position, [8]/[9] the
location the player is in (`PLACE_KIND`, `PLACE_INDEX`), [0Ah] event bits, [23h] the talker's dialogue stage, [2Ah] the
game phase; [2Bh], once `IN_DIALOGUE`, is `HARKONNEN_PLACE`: the
player is at a place the Harkonnens hold or occupy, from the listing:
location bit 2 is what `sub_1dd7` sets when they take one) and
1140h-11BFh. Other names found in the listing: [227Dh] `CINEMATIC`
(was `MAP_VIEW_FLAG`: counted up while the intro, the end video and two
scenes play full screen), [FBh] `MAP_INTERFACE`, [C6h] `BOOK`, [EAh]
`EVENT_KIND`, [46D9h] `COMM_CALLER` (the character waiting to call on
the communicator, 80h while the call is on). Open: the six records of
11h bytes at 10D8h (keyed by a location's byte 0, five slots filled at
random each day; the current one at [10B4h]). DCE4h-DCFFh, 4710h-476Fh and 4770h-4783h / 47D4h-47EFh are
mixes.

The spice and the Emperor's demands are named too: [A0h] `SPICE` in
stock, [A6h]/[AEh] the spice gained this phase and the one before,
[1170h] the stock when the phase began, [1172h] `SPICE_SHIPPED` this
phase; [BCh] `SPICE_DEMANDED`, [B4h] the four amounts offered,
[C0h] the one chosen, [BEh] `SHIPMENT_RATIO` (shipped × 80h ÷
demanded), [BFh] the shipment's bits, [C3h] `DEMANDS` made, [118Dh]
`SHIPMENT_DUE` (a day), [CFh] the days left, [1158h] `SHIPMENT_READY`.
The port of `loc_02566`, once `endgame_2566`, is the shipment
(`ship_spice_2566`): it runs when the player is in room 8 with
character 3 present and an amount chosen. The music driver's tick runs as generated code when the timer
interrupt comes inside generated code; that is where most of the
sound's counted writes come from.

## Tools in place

| Tool | What it checks |
|---|---|
| `--rec shadow` (`REC_ONLY`, `REC_BUDGET`, `REC_SAMPLES`) | Each port's call against the original, from the same state. |
| `tools/lift/validate.sh` | All ports in two 10-minute games and the harness, plus `--rec on` screenshots. |
| `tools/lift/hashcmp.sh` | One game, plain emulation against `--rec on`, the whole memory every 10 s (`STATE_HASH=1`). |
| `tools/lift/suite.sh record\|check` | Seven games from the start and from the saves, against a stored reference (states, screens and a save written near the end). |
| `tools/lift/flags_used.py` | Which routines' returned flags any caller reads. |
| `tools/lift/add_table.py`, `drop_consts.py` | Not checks: add a table of one and route the ports' accesses to it; drop the globals left unused. |
| `--save FILE` + event `T:load:1` | Real play from any point of the game (the test saves, Day 1 to 23). |
| event `T:save:N` | The game's state as it serialises it (`DUNE37SN.SAV` in the output folder). |

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
