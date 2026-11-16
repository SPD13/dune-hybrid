//! The sound's state as Rust state (Stage B of
//! `docs/readable-port-plan.md`): where the voice file is read, the
//! sound settings, the music loaded, playing and asked for, and what the
//! MIDI driver returned (`hand/voice.rs`, `hand/audio.rs`,
//! `hand/sound.rs`).
//!
//! A table of one record ([`Which::SoundState`], 14h bytes at DBC0h) in
//! the generic record tables (`hand/record_table.rs`, the memory kept as
//! the checked mirror); [`SoundState`] is its typed view. The drivers'
//! interrupt code writes it too (counted with the generated code).

use super::globals as g;
use super::record_table::{self, Single, Which};

/// Where the record starts in the data segment.
pub const AT: u16 = Which::SoundState.base();

/// The sound's state.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct SoundState {
    /// [DBC0h] ([`g::VOICE_FILE_POS`]): where the voice file is read
    /// next (32 bits).
    pub voice_pos: u32,
    /// [DBC4h] ([`g::VOICE_FILE_LEFT`]): the voice file's bytes left (32
    /// bits).
    pub voice_left: u32,
    /// [DBC8h] ([`g::SOUND_SETTINGS`]): the settings bits (bit 0: PCM
    /// enabled), its high byte cleared apart when the memory is set up.
    pub settings: u16,
    /// [DBCAh] ([`g::MUSIC_LOADED`]): the music whose data is loaded.
    pub music_loaded: u8,
    /// [DBCBh] ([`g::CURRENT_MUSIC`]): the music playing (0: none).
    pub current_music: u8,
    /// [DBCCh]: the music asked for (`play_requested_music` plays it).
    pub requested_music: u8,
    /// [DBCDh] ([`g::IS_SOUND_PRESENT`]): from the MIDI driver
    /// (Cryogenic: `IsSoundPresent`); negative: no music.
    pub present: u8,
    /// [DBCEh] ([`g::MIDI_FUNC5_BX`]): MIDI driver function 5's BX
    /// (Cryogenic: `midiFunc5returnBx`).
    pub midi_bx: u16,
    /// [DBD0h] ([`g::MIDI_FUNC5_CX`]): what the MIDI driver's tick
    /// returned in CX.
    pub midi_cx: u16,
    /// [DBD2h] ([`g::MUSIC_DBD2`]): the timer when the music was last
    /// picked (0: not yet; a new pick waits C8h ticks).
    pub picked_at: u16,
}

impl Single for SoundState {
    const WHICH: Which = Which::SoundState;

    fn from_bytes(b: &[u8]) -> SoundState {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        let d = |i: usize| w(i) as u32 | (w(i + 2) as u32) << 16;
        SoundState {
            voice_pos: d(0),
            voice_left: d(4),
            settings: w(8),
            music_loaded: b[0x0a],
            current_music: b[0x0b],
            requested_music: b[0x0c],
            present: b[0x0d],
            midi_bx: w(0x0e),
            midi_cx: w(0x10),
            picked_at: w(0x12),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        b[0..4].copy_from_slice(&self.voice_pos.to_le_bytes());
        b[4..8].copy_from_slice(&self.voice_left.to_le_bytes());
        b[8..0x0a].copy_from_slice(&self.settings.to_le_bytes());
        b[0x0a] = self.music_loaded;
        b[0x0b] = self.current_music;
        b[0x0c] = self.requested_music;
        b[0x0d] = self.present;
        b[0x0e..0x10].copy_from_slice(&self.midi_bx.to_le_bytes());
        b[0x10..0x12].copy_from_slice(&self.midi_cx.to_le_bytes());
        b[0x12..0x14].copy_from_slice(&self.picked_at.to_le_bytes());
        b
    }
}

const _: () = assert!(g::VOICE_FILE_POS == AT && g::VOICE_FILE_LEFT == AT + 4 && g::SOUND_SETTINGS == AT + 8);
const _: () = assert!(g::MUSIC_LOADED == AT + 0x0a && g::CURRENT_MUSIC == AT + 0x0b && g::IS_SOUND_PRESENT == AT + 0x0d);
const _: () = assert!(g::MIDI_FUNC5_BX == AT + 0x0e && g::MIDI_FUNC5_CX == AT + 0x10 && g::MUSIC_DBD2 == AT + 0x12);
