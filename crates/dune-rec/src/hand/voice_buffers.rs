//! The two voice buffers as Rust state (Stage B of
//! `docs/readable-port-plan.md`): the VOC data read from the voice file
//! in parts, played by the PCM driver in turn (`hand/voice.rs`,
//! `hand/audio.rs`, `hand/sound.rs`).
//!
//! A table of two records ([`Which::VoiceBuffers`], 8 bytes each at
//! 3811h) in the generic record tables (`hand/record_table.rs`, the
//! memory kept as the checked mirror); [`VoiceBuffer`] is the view of
//! one. Ports that reach a buffer through SI (+4, +6, +7) still write
//! its bytes (followed); the PCM driver's interrupt writes the states
//! (counted with the generated code).

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the table starts in the data segment.
pub const AT: u16 = Which::VoiceBuffers.base();

/// The buffer at `offset` (3811h or 3819h), if it is one.
pub fn index(offset: u16) -> Option<usize> {
    record_table::index(Which::VoiceBuffers, offset)
}

/// A voice buffer.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct VoiceBuffer {
    /// +0: far pointer (offset, segment) to its data; the first's segment
    /// is also the lip-sync data's ([`g::LIPSYNC_SEG`]).
    pub data: [u16; 2],
    /// +4: the bytes in it.
    pub size: u16,
    /// +6: the PCM driver's state for it (1 filled, 3 played; 0 free).
    pub state: u8,
    /// +7: the voice file's part in it (bit 80h on the last part).
    pub part: u8,
}

impl VoiceBuffer {
    /// The state and the part as the word the original stores.
    pub fn state_word(&self) -> u16 {
        self.state as u16 | (self.part as u16) << 8
    }
    pub fn set_state_word(&mut self, v: u16) {
        self.state = v as u8;
        self.part = (v >> 8) as u8;
    }
}

impl Single for VoiceBuffer {
    const WHICH: Which = Which::VoiceBuffers;

    fn from_bytes(b: &[u8]) -> VoiceBuffer {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        VoiceBuffer { data: [w(0), w(2)], size: w(4), state: b[6], part: b[7] }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        for (i, v) in [(0, self.data[0]), (2, self.data[1]), (4, self.size)] {
            b[i..i + 2].copy_from_slice(&v.to_le_bytes());
        }
        b[6] = self.state;
        b[7] = self.part;
        b
    }
}

const _: () = assert!(g::VOC_BUFFER == AT && g::LIPSYNC_SEG == AT + 2 && g::VOC_SIZE == AT + 4);
const _: () = assert!(g::PCM_STATE == AT + 6 && g::PCM_STATE_2 == AT + 8 + 6);
