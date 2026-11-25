//! The game's record tables as Rust state (Stage B of
//! `docs/readable-port-plan.md`): the troops (`hand/troop_table.rs`),
//! the locations, the text box (`hand/text_box.rs`) and the dialogue's
//! text (`hand/dialogue_text.rs`), the conversation
//! (`hand/conversation.rs`), the map view (`hand/map_view.rs`), the
//! map's selection (`hand/map_selection.rs`), the flight
//! (`hand/flight.rs`) and the message typed out
//! (`hand/typed_message.rs`), the screen buffers
//! (`hand/screen_buffers.rs`), the mouse (`hand/mouse_state.rs`), the
//! sound (`hand/sound_state.rs`), the video playing
//! (`hand/video_state.rs`), the player's travel (`hand/travel.rs`) and
//! the font's pen (`hand/font_pen.rs`), these thirteen tables of one,
//! the two voice buffers (`hand/voice_buffers.rs`) and the characters
//! (`hand/characters.rs`),
//! kept with the machine ([`super::native`]).
//!
//! Each table owns its memory (Stage B's ownership): its records live in
//! a [`TableMemory`] registered with the bus (`pc::hw::Owned`; all the
//! tables at the first use of any). Every read and write of their bytes
//! by any code goes to it: the views (`hand/game.rs`'s `Troop`,
//! `Location`, the typed tables of one), a port's plain accesses, the
//! generated code, the interpreter, the drivers. The memory keeps a copy
//! written through for the tools (snapshots, state hashes, the shadow
//! check), which the tables take back where the tools replace the
//! memory. So nothing writes them behind their back: the sync-in counts
//! (`syncs`, `after_generated`, `at_stops`) stay 0, and the tools' stops
//! and a save loaded need nothing ([`outside`], [`loaded`]).
//!
//! `TROOPS_TRACE=1` prints each write a port makes into a table without
//! its view.

use super::{Ctx, globals as g};
use pc::cpu::DS;
use pc::{Machine, cpu::Bus};

/// The longest record.
pub const MAX: usize = 0x40;

/// The tables.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Which {
    /// 43h troops of 1Bh bytes at 8AAh.
    Troops,
    /// 46h locations of 1Ch bytes at 100h (an FFh after the last).
    Locations,
    /// The text box's layout, 1Ch bytes at 4784h.
    TextBox,
    /// The dialogue's text being played, 0Eh bytes at 47C6h.
    DialogueText,
    /// The conversation and scene state, 24h bytes at 47A2h.
    Conversation,
    /// The map view, 0Ch bytes at 46E3h.
    MapView,
    /// The map's selection, 0Fh bytes at 46EFh.
    MapSelection,
    /// The flight, 6 bytes at 4727h.
    Flight,
    /// The message typed out on the message line, 0Ah bytes at 473Fh.
    TypedMessage,
    /// The screen buffers' segments, 0Ch bytes at DBD4h.
    ScreenBuffers,
    /// The mouse and its cursor, 14h bytes at DC34h.
    MouseState,
    /// The sound's state, 14h bytes at DBC0h.
    SoundState,
    /// The HNM video playing, 40h bytes at DBE6h.
    Video,
    /// The player's travel on the map, 0Dh bytes at 11C1h.
    Travel,
    /// The font's pen, 8 bytes at D82Ch.
    FontPen,
    /// The two voice buffers, 8 bytes each at 3811h.
    VoiceBuffers,
    /// 10h characters of 10h bytes at FD8h.
    Characters,
    /// Where the player is, 4 bytes at DS:4.
    Player,
    /// Six smugglers of 11h bytes at 10D8h.
    Smugglers,
    /// The story's progress, the talk and the smuggler dealt with, 24h bytes at DS:8.
    Progress,
    /// The troop talked about, for the scripts, 21h bytes at DS:2Ch.
    TopicTroop,
    /// The place talked about, for the scripts, 13h bytes at DS:4Dh.
    TopicPlace,
    /// The troops at the place talked about and its battle, 3Dh bytes at DS:60h.
    PlaceForces,
    /// The spice, the trade and the Emperor's demands, 2Dh bytes at DS:9Dh.
    Economy,
    /// The places nearest, the desert walk, the messages read and the attacks, 36h bytes at DS:CAh.
    Situation,
    /// The story sequence playing, 8 bytes at DS:4774h.
    Sequence,
    /// The phrase file, the phrase put together and its voice, 8 bytes at DS:477Ch.
    Phrase,
    /// The sentence being said, 0Ch bytes at DS:47DCh.
    Sentence,
    /// The messages kept (`hand/messages.rs`).
    Messages,
    /// The phrases a phrase's codes 80h..8Fh stand for (`hand/phrase_args.rs`).
    PhraseArgs,
}

impl Which {
    pub const ALL: [Which; 30] = [
        Which::Troops,
        Which::Locations,
        Which::TextBox,
        Which::DialogueText,
        Which::Conversation,
        Which::MapView,
        Which::MapSelection,
        Which::Flight,
        Which::TypedMessage,
        Which::ScreenBuffers,
        Which::MouseState,
        Which::SoundState,
        Which::Video,
        Which::Travel,
        Which::FontPen,
        Which::VoiceBuffers,
        Which::Characters,
        Which::Player,
        Which::Smugglers,
        Which::Progress,
        Which::TopicTroop,
        Which::TopicPlace,
        Which::PlaceForces,
        Which::Economy,
        Which::Situation,
        Which::Sequence,
        Which::Phrase,
        Which::Sentence,
        Which::Messages,
        Which::PhraseArgs,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Which::Troops => "troops",
            Which::Locations => "locations",
            Which::TextBox => "text box",
            Which::DialogueText => "dialogue text",
            Which::Conversation => "conversation",
            Which::MapView => "map view",
            Which::MapSelection => "map selection",
            Which::Flight => "flight",
            Which::TypedMessage => "typed message",
            Which::ScreenBuffers => "screen buffers",
            Which::MouseState => "mouse",
            Which::SoundState => "sound",
            Which::Video => "video",
            Which::Travel => "travel",
            Which::FontPen => "font pen",
            Which::VoiceBuffers => "voice buffers",
            Which::Characters => "characters",
            Which::Player => "player",
            Which::Smugglers => "smugglers",
            Which::Progress => "progress",
            Which::TopicTroop => "topic troop",
            Which::TopicPlace => "topic place",
            Which::PlaceForces => "place forces",
            Which::Economy => "economy",
            Which::Situation => "situation",
            Which::Sequence => "sequence",
            Which::Phrase => "phrase",
            Which::Sentence => "sentence",
            Which::Messages => "messages",
            Which::PhraseArgs => "phrase args",
        }
    }
    /// Where the table starts in the game's data segment.
    pub const fn base(self) -> u16 {
        match self {
            Which::Troops => g::TROOPS,
            Which::Locations => 0x100,
            Which::TextBox => 0x4784,
            Which::DialogueText => 0x47c6,
            Which::Conversation => 0x47a2,
            Which::MapView => 0x46e3,
            Which::MapSelection => 0x46ef,
            Which::Flight => 0x4727,
            Which::TypedMessage => 0x473f,
            Which::ScreenBuffers => 0xdbd4,
            Which::MouseState => 0xdc34,
            Which::SoundState => 0xdbc0,
            Which::Video => 0xdbe6,
            Which::Travel => 0x11c1,
            Which::FontPen => 0xd82c,
            Which::VoiceBuffers => 0x3811,
            Which::Characters => 0x0fd8,
            Which::Player => 0x0004,
            Which::Smugglers => g::SMUGGLERS,
            Which::Progress => 0x8,
            Which::TopicTroop => 0x2c,
            Which::TopicPlace => 0x4d,
            Which::PlaceForces => 0x60,
            Which::Economy => 0x9d,
            Which::Situation => 0xca,
            Which::Sequence => 0x4774,
            Which::Phrase => 0x477c,
            Which::Sentence => 0x47dc,
            Which::Messages => 0x1179,
            Which::PhraseArgs => 0x11eb,
        }
    }
    pub const fn size(self) -> u16 {
        match self {
            Which::Troops => 0x1b,
            Which::Locations => 0x1c,
            Which::TextBox => 0x1c,
            Which::DialogueText => 0x0e,
            Which::Conversation => 0x24,
            Which::MapView => 0x0c,
            Which::MapSelection => 0x0f,
            Which::Flight => 6,
            Which::TypedMessage => 0x0a,
            Which::ScreenBuffers => 0x0c,
            Which::MouseState => 0x14,
            Which::SoundState => 0x14,
            Which::Video => 0x40,
            Which::Travel => 0x0d,
            Which::FontPen => 8,
            Which::VoiceBuffers => 8,
            Which::Characters => 0x10,
            Which::Player => 4,
            Which::Smugglers => 0x11,
            Which::Progress => 0x24,
            Which::TopicTroop => 0x21,
            Which::TopicPlace => 0x13,
            Which::PlaceForces => 0x3d,
            Which::Economy => 0x2d,
            Which::Situation => 0x36,
            Which::Sequence => 0x8,
            Which::Phrase => 0x8,
            Which::Sentence => 0xc,
            Which::Messages => 0x14,
            Which::PhraseArgs => 0x20,
        }
    }
    pub const fn count(self) -> usize {
        match self {
            Which::Troops => ((g::TROOPS_END - g::TROOPS) / 0x1b) as usize,
            Which::Locations => 0x46,
            Which::TextBox => 1,
            Which::DialogueText => 1,
            Which::Conversation => 1,
            Which::MapView => 1,
            Which::MapSelection => 1,
            Which::Flight => 1,
            Which::TypedMessage => 1,
            Which::ScreenBuffers => 1,
            Which::MouseState => 1,
            Which::SoundState => 1,
            Which::Video => 1,
            Which::Travel => 1,
            Which::FontPen => 1,
            Which::VoiceBuffers => 2,
            Which::Characters => 0x10,
            Which::Player => 1,
            Which::Smugglers => 6,
            Which::Progress => 1,
            Which::TopicTroop => 1,
            Which::TopicPlace => 1,
            Which::PlaceForces => 1,
            Which::Economy => 1,
            Which::Situation => 1,
            Which::Sequence => 1,
            Which::Phrase => 1,
            Which::Sentence => 1,
            Which::Messages => 1,
            Which::PhraseArgs => 1,
        }
    }
    fn end(self) -> u16 {
        self.base() + self.size() * self.count() as u16
    }
}

/// A record's bytes (the first [`Which::size`] used).
pub type Bytes = [u8; MAX];

/// One table's part of the Rust state kept with a machine.
#[derive(Default)]
pub struct RecordTable {
    /// Its owner on the bus ([`TableMemory`], registered on the first
    /// use): the index `Hardware::own` gave.
    owner: Option<usize>,
    /// Unforeseen writes (see the module's notes).
    syncs: u64,
    /// Writes of the generated code.
    after_generated: u64,
    /// Records the memory had changed at one of the tools' stops
    /// ([`outside`]).
    at_stops: u64,
}

fn table(m: &mut Machine, w: Which) -> &mut RecordTable {
    let n = super::native::native(m);
    let t = match w {
        Which::Troops => &mut n.troops,
        Which::Locations => &mut n.locations,
        Which::TextBox => &mut n.text_box,
        Which::DialogueText => &mut n.dialogue_text,
        Which::Conversation => &mut n.conversation,
        Which::MapView => &mut n.map_view,
        Which::MapSelection => &mut n.map_selection,
        Which::Flight => &mut n.flight,
        Which::TypedMessage => &mut n.typed_message,
        Which::ScreenBuffers => &mut n.screen_buffers,
        Which::MouseState => &mut n.mouse_state,
        Which::SoundState => &mut n.sound_state,
        Which::Video => &mut n.video_state,
        Which::Travel => &mut n.travel,
        Which::FontPen => &mut n.font_pen,
        Which::VoiceBuffers => &mut n.voice_buffers,
        Which::Characters => &mut n.characters,
        Which::Player => &mut n.player,
        Which::Smugglers => &mut n.smugglers,
        Which::Progress => &mut n.progress,
        Which::TopicTroop => &mut n.topic_troop,
        Which::TopicPlace => &mut n.topic_place,
        Which::PlaceForces => &mut n.place_forces,
        Which::Economy => &mut n.economy,
        Which::Situation => &mut n.situation,
        Which::Sequence => &mut n.sequence,
        Which::Phrase => &mut n.phrase,
        Which::Sentence => &mut n.sentence,
        Which::Messages => &mut n.messages,
        Which::PhraseArgs => &mut n.phrase_args,
    };
    t
}

/// How often a record of `w` took the memory's bytes, unforeseen.
pub fn syncs(m: &mut Machine, w: Which) -> u64 {
    table(m, w).syncs
}

/// How often a record of `w` took the memory's bytes the generated code
/// wrote.
pub fn after_generated(m: &mut Machine, w: Which) -> u64 {
    table(m, w).after_generated
}

/// The record of `w` at `offset` in the data segment, if it starts one.
pub fn index(w: Which, offset: u16) -> Option<usize> {
    let k = offset.checked_sub(w.base())?;
    (offset < w.end() && k % w.size() == 0).then_some((k / w.size()) as usize)
}

fn record_offset(w: Which, i: usize) -> u16 {
    w.base() + i as u16 * w.size()
}

/// A table's records as the memory the bus serves from them: they live
/// here, every read and write of their bytes by any code goes here, and
/// the memory keeps a copy for the tools (`pc::hw::Owned`).
pub struct TableMemory {
    which: Which,
    bytes: Vec<u8>,
}

impl TableMemory {
    fn base(&self) -> u32 {
        ((g::GAME_DS as u32) << 4) + self.which.base() as u32
    }
}

impl pc::hw::Owned for TableMemory {
    fn ranges(&self) -> Vec<(u32, u32)> {
        vec![(self.base(), self.bytes.len() as u32)]
    }
    fn read(&self, addr: u32) -> u8 {
        self.bytes[(addr - self.base()) as usize]
    }
    fn write(&mut self, addr: u32, v: u8) {
        let i = (addr - self.base()) as usize;
        self.bytes[i] = v;
    }
    fn direct(&mut self) -> Option<&mut [u8]> {
        // One range, never resized while owned.
        Some(&mut self.bytes)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// The owner of `w`'s memory. At the first use of any table, every table
/// is registered (taking the memory's bytes), and the loose variables
/// between them (`hand/variables.rs`), and the dialogue table
/// (`hand/dialogue_table.rs`).
fn owner(m: &mut Machine, w: Which) -> usize {
    if let Some(i) = table(m, w).owner {
        return i;
    }
    for v in Which::ALL {
        if table(m, v).owner.is_none() {
            let len = v.size() as usize * v.count();
            let i = m.hw.own(Box::new(TableMemory { which: v, bytes: vec![0; len] }));
            table(m, v).owner = Some(i);
        }
    }
    super::variables::own(m);
    super::dialogue_table::own(m);
    table(m, w).owner.expect("registered")
}

/// Record `i` of `w`.
pub(super) fn get(c: &mut Ctx, w: Which, i: usize) -> Bytes {
    let k = owner(c.m, w);
    let t = c.m.hw.owned_at::<TableMemory>(k).expect("a table's memory");
    let n = w.size() as usize;
    let mut b = [0u8; MAX];
    b[..n].copy_from_slice(&t.bytes[i * n..i * n + n]);
    b
}

/// Record `i` of `w` changed to `r`: its bytes that changed written on
/// the bus (to the table, and the memory's copy).
pub(super) fn set(c: &mut Ctx, w: Which, i: usize, r: Bytes) {
    let old = get(c, w, i);
    for k in 0..w.size() as usize {
        if r[k] != old[k] {
            let a = c.m.cpu.phys(g::GAME_DS, record_offset(w, i) + k as u16);
            c.m.hw.write8(a, r[k]);
        }
    }
}

/// A table with a typed view of its records (the tables of one: the text
/// box, the dialogue text, the conversation, the map view, the map's
/// selection, the flight, the message typed out, the screen buffers, the
/// mouse, the sound, the video, the travel, the font's pen; the voice
/// buffers and the characters): the view to and from a record's bytes. [`single`] and
/// [`update_single`] keep a table of one; [`nth`],
/// [`set_nth`] and [`update_nth`] record `i` of a longer one.
pub(super) trait Single: Sized {
    /// Its table.
    const WHICH: Which;
    fn from_bytes(b: &[u8]) -> Self;
    fn to_bytes(&self) -> Bytes;
}

/// The bytes of record `i` of `w` at DS (another data segment than the
/// game's: not the table's).
fn at_ds(c: &mut Ctx, w: Which, i: usize) -> Bytes {
    let mut b = [0; MAX];
    for (k, v) in b.iter_mut().enumerate().take(w.size() as usize) {
        *v = c.byte(record_offset(w, i) + k as u16);
    }
    b
}

/// Record `i` of `T`'s table, in step with the memory (with DS not the
/// game's, the bytes there).
pub(super) fn nth<T: Single>(c: &mut Ctx, i: usize) -> T {
    if c.sreg(DS) != g::GAME_DS {
        return T::from_bytes(&at_ds(c, T::WHICH, i));
    }
    T::from_bytes(&get(c, T::WHICH, i))
}

/// Record `i` of `T`'s table changed to `t`: kept, and its changed bytes
/// written through.
pub(super) fn set_nth<T: Single>(c: &mut Ctx, i: usize, t: T) {
    let w = T::WHICH;
    if c.sreg(DS) != g::GAME_DS {
        let (old, new) = (at_ds(c, w, i), t.to_bytes());
        for k in 0..w.size() as usize {
            if old[k] != new[k] {
                c.set_byte(record_offset(w, i) + k as u16, new[k]);
            }
        }
        return;
    }
    set(c, w, i, t.to_bytes());
}

/// Record `i` of `T`'s table changed by `f`.
pub(super) fn update_nth<T: Single>(c: &mut Ctx, i: usize, f: impl FnOnce(&mut T)) {
    let mut t = nth(c, i);
    f(&mut t);
    set_nth(c, i, t);
}

/// The one record of `T`'s table.
pub(super) fn single<T: Single>(c: &mut Ctx) -> T {
    nth(c, 0)
}

/// The one record of `T`'s table changed by `f`.
pub(super) fn update_single<T: Single>(c: &mut Ctx, f: impl FnOnce(&mut T)) {
    update_nth(c, 0, f)
}

/// Byte `field` of record `i` of `w`.
pub(super) fn byte(c: &mut Ctx, w: Which, i: usize, field: u16) -> u8 {
    get(c, w, i)[field as usize]
}

/// Byte `field` of record `i` of `w` set.
pub(super) fn set_byte(c: &mut Ctx, w: Which, i: usize, field: u16, v: u8) {
    let mut b = get(c, w, i);
    b[field as usize] = v;
    set(c, w, i, b);
}

/// A port wrote `len` bytes at the linear address `at` other than through
/// a view: the table and record it hit, if any (for the trace; the write
/// went to the table's memory on the bus already).
pub(super) fn follow(m: &mut Machine, at: u32, len: u32) -> Option<(Which, usize, u16)> {
    let mut hit = None;
    for w in Which::ALL {
        let t0 = m.cpu.phys(g::GAME_DS, w.base());
        let t1 = m.cpu.phys(g::GAME_DS, w.end());
        if at + len > t0 && at < t1 {
            let k = (at.max(t0) - t0) as usize;
            hit = Some((w, k / w.size() as usize, (k % w.size() as usize) as u16));
        }
    }
    hit
}

/// The machine ran outside the recompiled code on purpose (a tool
/// stepping it to a stop): nothing to do, the tables own their memory.
pub fn outside(_m: &mut Machine) {}

/// A save loaded or a snapshot restored: nothing to do (the restore
/// wrote the tables on the bus; a restored memory was absorbed).
pub fn loaded(_m: &mut Machine) {}

/// How often a record of `w` had changed at the tools' stops.
pub fn at_stops(m: &mut Machine, w: Which) -> u64 {
    table(m, w).at_stops
}
