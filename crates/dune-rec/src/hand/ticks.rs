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

/// The entries of those ports.
const SHARED: &[u16] = &[
    0x1ae7, // sub_1ae7
    0x1b0d, // story's clock wait
    0x1b23, // new_time_1b23
    0x2b2a, // clock_step
    0x9285, // dialogue_hit
    0x92c9, // in_dialogue_rects
    0xa7c2, // lip_sync_frame_task
    0xabcc, // is_voc_pcm_playing
    0xace6, // music_step
    0xae28, // music_enabled
    0xca60, // hnm_do_frame
    0xca8f, // the video frame's
    0xcad4, // hnm_wait_for_frame
    0xd50f, // highlight_hovered_text_action_item
    0xd7b7, // blink_gauges
    0xd962, // cursor_glide
    0xd9d2, // process_frame_tasks
    0xdc20, // redraw_mouse
    0xddb0, // wait_interruptable
    0xddf0, // wait_for_pcm_voice
    0xdf1e, // input's
    0xe387, // wait_a_bit
    0xe3cc, // rand
    0xe85c, // measure_pit_e85c
];

/// Whether the port entered at `entry` shares memory with the handlers.
pub(super) fn shared(entry: u16) -> bool {
    SHARED.contains(&entry)
}
