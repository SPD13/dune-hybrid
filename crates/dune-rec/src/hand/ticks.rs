//! Ports that share memory with the interrupt handlers.
//!
//! With `REC_INPLACE`, an interrupt due inside a hand port's stretch is
//! taken in place (`Machine::rec_run`): the port computed the stretch from
//! the state at its start and writes its results after it, so this is
//! exact only when nothing the handler reads or writes is read or written
//! by the stretch. The ports below do share such memory (found with
//! `REC_INPLACE_DIAG` in the save suite): the clock's countdown [46DBh],
//! the ticks [0CE7Ah], the video frame's bytes [0DBB5h] and [0DBD0h],
//! CS:[0E8D4h] while the PIT is measured. Mostly loops that wait for the
//! timer, and the frame tasks the handler itself runs. Their stretches
//! still bail at an interrupt, as without `REC_INPLACE`.
//!
//! A port leaves this list once each instruction of it that reads such
//! memory is a stretch of its own, the read done right after that `run`
//! (the events due at the instruction are taken first, as in the
//! original, and no later one comes before the read), and each that
//! writes it the same, the write right after its `run`
//! (`pcm_test_audio_done`).

/// The entries of those ports. The others that shared such memory read
/// and write it as below; found with `REC_INPLACE_ALL=1
/// REC_INPLACE_DIAG=1` (this list ignored, the conflicts listed by
/// stretch and address: `inplace where ENTRY: KIND@STRETCH:ADDRESS`).
const SHARED: &[u16] = &[
    0xe85c, // measure_pit_speed (start-up: CS:[0E8D4h] while the PIT is measured)
];

/// Whether the port entered at `entry` shares memory with the handlers.
pub(super) fn shared(entry: u16) -> bool {
    SHARED.contains(&entry)
}
