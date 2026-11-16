//! The HNM video playing as Rust state (Stage B of
//! `docs/readable-port-plan.md`): the video and its file, the frame
//! counters, the ring buffer its data is read ahead into, and the frame's
//! timing (`hand/hnm.rs`, `hand/hnmload.rs`, `hand/video.rs`).
//!
//! A table of one record ([`Which::Video`], 40h bytes at DBE6h) in the
//! generic record tables (`hand/record_table.rs`, the memory kept as the
//! checked mirror); [`VideoState`] is its typed view. The ring buffer
//! itself (up to [CE74h], in its own segment) stays in memory.

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::Video.base();

/// The video playing.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct VideoState {
    /// [DBE6h]: ticks to wait (the intro's pauses, 6).
    pub delay: u8,
    /// [DBE7h] ([`g::HNM_STATE`]): 0 or 1 while a video plays
    /// (`check_if_hnm_complete`).
    pub playing: u8,
    /// [DBE8h] ([`g::HNM_FRAME`]): the frame shown.
    pub frame: u16,
    /// [DBEAh]: the frames read ahead.
    pub fetched: u16,
    /// [DBECh]: the last frame, FFFFh once passed
    /// (reset: FFFFh).
    pub last_frame: u16,
    /// [DBEEh] ([`g::HNM_FRAME_4`]): the frame to stop reading ahead at
    /// (reset: FFFFh).
    pub stop_frame: u16,
    /// [DBF0h]..[DBF5h]: not used by the ports.
    pub words_dbf0: [u16; 3],
    /// [DBF6h] ([`g::HNM_BODY_OFFSET`]): where the frames start in the
    /// file.
    pub body_offset: u32,
    /// [DBFAh]: the frames' size.
    pub body_size: u32,
    /// [DBFEh] ([`g::HNM_FLAGS`]): the video's flags (bit 80h: read whole
    /// parts; 40h: frames to the back buffer; 10h, 20h: full-screen
    /// pictures).
    pub flags: u8,
    /// [DBFFh]: ticks per frame with the video's
    /// own timing.
    pub frame_ticks: u8,
    /// [DC00h] ([`g::HNM_VIDEO`]): the video asked for.
    pub video: u16,
    /// [DC02h]: the video open.
    pub video_open: u16,
    /// [DC04h] ([`g::HNM_FILE_OFFSET`]): where the file is read next
    /// (Cryogenic: `hnmFileOffset`).
    pub file_offset: u32,
    /// [DC08h]: the file's bytes left (Cryogenic:
    /// `hnmFileRemain`).
    pub file_left: u32,
    /// [DC0Ch] ([`g::HNM_READ_PTR`]): far pointer where the file's data
    /// goes in the ring buffer (back to 0 past its end).
    pub fill: [u16; 2],
    /// [DC10h] ([`g::HNM_DC10`]): far pointer to the next chunk in the
    /// ring buffer.
    pub next_chunk: [u16; 2],
    /// [DC14h]: far pointer to the frame waiting to be drawn (segment 0:
    /// none).
    pub pending_frame: [u16; 2],
    /// [DC18h]: where the fill pointer was when it went
    /// back to 0 (the ring's used end).
    pub wrapped_at: u16,
    /// [DC1Ah] ([`g::HNM_READ_PTR_2`]): the bytes read and not yet
    /// decoded.
    pub buffered: u16,
    /// [DC1Ch] ([`g::HNM_SYNC`]): the frame's voice block ("sd"), FFFFh
    /// for none (then the video's own timing).
    pub voice_chunk: u16,
    /// [DC1Eh]: the frame's palette chunk ("pl"),
    /// FFFFh for none.
    pub palette_chunk: u16,
    /// [DC20h]: the next part's size (read ahead before
    /// the next frame).
    pub part_size: u16,
    /// [DC22h] ([`g::HNM_FRAME_TICK`]): the timer at the last frame.
    pub frame_tick: u16,
    /// [DC24h]: the picture's tag.
    pub picture_tag: u16,
}

impl VideoState {
    /// The flags and the ticks per frame set as the word the original
    /// stores (the video table's flags word).
    pub fn set_flags_word(&mut self, v: u16) {
        self.flags = v as u8;
        self.frame_ticks = (v >> 8) as u8;
    }
}

impl Single for VideoState {
    const WHICH: Which = Which::Video;

    fn from_bytes(b: &[u8]) -> VideoState {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        let d = |i: usize| w(i) as u32 | (w(i + 2) as u32) << 16;
        VideoState {
            delay: b[0],
            playing: b[1],
            frame: w(2),
            fetched: w(4),
            last_frame: w(6),
            stop_frame: w(8),
            words_dbf0: [w(0x0a), w(0x0c), w(0x0e)],
            body_offset: d(0x10),
            body_size: d(0x14),
            flags: b[0x18],
            frame_ticks: b[0x19],
            video: w(0x1a),
            video_open: w(0x1c),
            file_offset: d(0x1e),
            file_left: d(0x22),
            fill: [w(0x26), w(0x28)],
            next_chunk: [w(0x2a), w(0x2c)],
            pending_frame: [w(0x2e), w(0x30)],
            wrapped_at: w(0x32),
            buffered: w(0x34),
            voice_chunk: w(0x36),
            palette_chunk: w(0x38),
            part_size: w(0x3a),
            frame_tick: w(0x3c),
            picture_tag: w(0x3e),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        b[0] = self.delay;
        b[1] = self.playing;
        let words = [
            (2, self.frame),
            (4, self.fetched),
            (6, self.last_frame),
            (8, self.stop_frame),
            (0x0a, self.words_dbf0[0]),
            (0x0c, self.words_dbf0[1]),
            (0x0e, self.words_dbf0[2]),
            (0x1a, self.video),
            (0x1c, self.video_open),
            (0x26, self.fill[0]),
            (0x28, self.fill[1]),
            (0x2a, self.next_chunk[0]),
            (0x2c, self.next_chunk[1]),
            (0x2e, self.pending_frame[0]),
            (0x30, self.pending_frame[1]),
            (0x32, self.wrapped_at),
            (0x34, self.buffered),
            (0x36, self.voice_chunk),
            (0x38, self.palette_chunk),
            (0x3a, self.part_size),
            (0x3c, self.frame_tick),
            (0x3e, self.picture_tag),
        ];
        for (i, v) in words {
            b[i..i + 2].copy_from_slice(&v.to_le_bytes());
        }
        for (i, v) in [(0x10, self.body_offset), (0x14, self.body_size), (0x1e, self.file_offset), (0x22, self.file_left)] {
            b[i..i + 4].copy_from_slice(&v.to_le_bytes());
        }
        b[0x18] = self.flags;
        b[0x19] = self.frame_ticks;
        b
    }
}

const _: () = assert!(g::HNM_STATE == AT + 1 && g::HNM_FRAME == AT + 2 && g::HNM_FRAME_4 == AT + 8);
const _: () = assert!(g::HNM_BODY_OFFSET == AT + 0x10 && g::HNM_FLAGS == AT + 0x18 && g::HNM_VIDEO == AT + 0x1a);
const _: () = assert!(g::HNM_FILE_OFFSET == AT + 0x1e && g::HNM_READ_PTR == AT + 0x26 && g::HNM_DC10 == AT + 0x2a);
const _: () = assert!(g::HNM_READ_PTR_2 == AT + 0x34 && g::HNM_SYNC == AT + 0x36 && g::HNM_FRAME_TICK == AT + 0x3c);
