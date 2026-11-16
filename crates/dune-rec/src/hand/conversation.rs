//! The conversation and scene state as Rust state (Stage B of
//! `docs/readable-port-plan.md`): who talks, the speaker and the phrase
//! files the dialogue's lines come from, where a phrase goes on, the
//! scene's flags and count.
//!
//! A table of one record ([`Which::Conversation`], 24h bytes at 47A2h)
//! in the generic record tables (`hand/record_table.rs`, the memory kept
//! as the checked mirror); [`Conversation`] is its typed view. Some
//! fields are only partly understood; their names say what the ports
//! found them doing.

use super::record_table::{self, Single, Which};
use super::globals as g;

/// Where the record starts in the data segment.
pub const AT: u16 = Which::Conversation.base();

/// The conversation and scene state.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Conversation {
    /// [47A2h]: the speaker's record.
    pub speaker: u16,
    /// [47A4h]: the dialogue's state bits: 80h ends it with this line, 1
    /// keeps the view.
    pub line_state: u8,
    /// [47A5h]: FFh, 80h (giving orders) or 0; set by small routines
    /// (A1C4h..A1DCh), not known further.
    pub flag_47a5: u8,
    /// [47A6h]: the scene's flags, taken once when a scene starts
    /// (`scene_start`); FFh when none.
    pub scene_flags: u8,
    /// [47A7h]: counts the scene's records (`scene_sentence` once per
    /// scene).
    pub scene_count: u8,
    /// [47A8h]: set when the entry said ends the list; the script loop
    /// looks at it and clears it.
    pub last_entry: u8,
    /// [47A9h]: the device's caller (a call on the communicator).
    pub caller: u8,
    /// [47AAh]: the first speaker record of the scene (`add_speaker`).
    pub first_speaker: u16,
    /// [47ACh]: far pointer (offset, segment) to the command strings.
    pub commands: [u16; 2],
    /// [47B0h]: far pointer to the loaded phrase strings.
    pub phrases: [u16; 2],
    /// [47B4h]: the loaded phrase file's last offset.
    pub phrase_end: u16,
    /// [47B6h]: far pointer to where the phrase being said goes on (0:
    /// none runs).
    pub phrase_resume: [u16; 2],
    /// [47BAh]: the dialogue list the dialogue goes on from (0: from its
    /// group).
    pub pending: u16,
    /// [47BCh]: where `append_phrase` writes next.
    pub append_at: u16,
    /// [47BEh]: the dialogue's group (an entry of the phrases' groups: 4
    /// to a speaker's mood, 8 to a speaker).
    pub group: u16,
    /// [47C0h]: not used by the ports.
    pub word_47c0: u16,
    /// [47C2h]: the dialogue's mode bits (80h, 20h; ANDed with the
    /// step's).
    pub mode: u8,
    /// [47C3h]: set while two talking heads take turns
    /// (`two_heads_text_step`).
    pub two_heads: u8,
    /// [47C4h]: the character talking ([`g::TALKER`]; its record is FD8h
    /// + 10h × it).
    pub talker: u16,
}

impl Conversation {
    /// The mode and [`Self::two_heads`] as the word the original pushes
    /// and loads.
    pub fn mode_word(&self) -> u16 {
        self.mode as u16 | (self.two_heads as u16) << 8
    }
    pub fn set_mode_word(&mut self, v: u16) {
        self.mode = v as u8;
        self.two_heads = (v >> 8) as u8;
    }
}

impl Single for Conversation {
    const WHICH: Which = Which::Conversation;

    fn from_bytes(b: &[u8]) -> Conversation {
        let w = |i: usize| b[i] as u16 | (b[i + 1] as u16) << 8;
        Conversation {
            speaker: w(0),
            line_state: b[2],
            flag_47a5: b[3],
            scene_flags: b[4],
            scene_count: b[5],
            last_entry: b[6],
            caller: b[7],
            first_speaker: w(8),
            commands: [w(0x0a), w(0x0c)],
            phrases: [w(0x0e), w(0x10)],
            phrase_end: w(0x12),
            phrase_resume: [w(0x14), w(0x16)],
            pending: w(0x18),
            append_at: w(0x1a),
            group: w(0x1c),
            word_47c0: w(0x1e),
            mode: b[0x20],
            two_heads: b[0x21],
            talker: w(0x22),
        }
    }

    fn to_bytes(&self) -> record_table::Bytes {
        let mut b = [0; record_table::MAX];
        let mut w = |i: usize, v: u16| {
            b[i] = v as u8;
            b[i + 1] = (v >> 8) as u8;
        };
        w(0, self.speaker);
        w(8, self.first_speaker);
        w(0x0a, self.commands[0]);
        w(0x0c, self.commands[1]);
        w(0x0e, self.phrases[0]);
        w(0x10, self.phrases[1]);
        w(0x12, self.phrase_end);
        w(0x14, self.phrase_resume[0]);
        w(0x16, self.phrase_resume[1]);
        w(0x18, self.pending);
        w(0x1a, self.append_at);
        w(0x1c, self.group);
        w(0x1e, self.word_47c0);
        w(0x22, self.talker);
        b[2] = self.line_state;
        b[3] = self.flag_47a5;
        b[4] = self.scene_flags;
        b[5] = self.scene_count;
        b[6] = self.last_entry;
        b[7] = self.caller;
        b[0x20] = self.mode;
        b[0x21] = self.two_heads;
        b
    }
}

const _: () = assert!(g::TALKER == AT + 0x22);
