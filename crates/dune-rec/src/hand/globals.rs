//! Data-segment globals (offsets in DS = 1F4Bh for the CD executable).
//! Names from the routines that use them, and from Cryogenic's
//! `GlobalsOnDs.cs` where it has one (noted "Cryogenic"). A name with only
//! an offset is not understood yet.

// --- Game state -----------------------------------------------------------

/// Game time (Cryogenic: `GameElapsedTime`): the low nibble is the time of
/// day (16 slots a day), bits 4..6 select the sky's palette group.
pub const GAME_TIME: u16 = 0x0002;
/// Where the player is (`hand/player.rs`): the map x, or at a location
/// its room (low byte) and the location's kind (high byte; 20h and up
/// the sietches and palaces, `get_orni_position` tests it).
pub const PLAYER_X: u16 = 0x0004;
/// The map y; 80h in its low byte: at the location whose index is its
/// high byte.
pub const PLAYER_Y: u16 = 0x0006;
/// The kind of the location the player is in (x's high byte, kept on
/// entering, 4043h; 21h a palace), FFh outside one.
pub const PLACE_KIND: u16 = 0x0008;
/// The index of the location the player is in (y's high byte), FFh
/// outside one.
pub const PLACE_INDEX: u16 = 0x0009;
/// Bit sets over the characters (bit = a character's number, its byte
/// 0Eh): those ever spoken to (93EFh).
pub const MET: u16 = 0x000e;
/// Those in the party, following the player (joined at 9616h: their
/// flag 40h and join time set; left at 9568h).
pub const PARTY: u16 = 0x0010;
/// Those at the player's place, shown in the room (rebuilt at 30A1h;
/// the intro sets its own).
pub const PRESENT: u16 = 0x0012;
/// Those spoken to in this room (cleared on arriving, 2E07h).
pub const TALKED_HERE: u16 = 0x0014;
/// The spice in stock (harvests add to it, 702Ah; shipments to the
/// Emperor take from it, 2524h).
pub const SPICE: u16 = 0x00a0;
/// The spice gained this phase: the stock plus what was shipped, less
/// the stock when the phase began (1C84h, C046h).
pub const SPICE_GAINED: u16 = 0x00a6;
/// The previous phase's [`SPICE_GAINED`] (1C87h).
pub const SPICE_GAINED_BEFORE: u16 = 0x00ae;
/// Four words: the amounts the player may ship to the Emperor, worked
/// out from the demand (22B1h); the answer picks one (2509h).
pub const SHIPMENT_OFFERS: u16 = 0x00b4;
/// The spice the Emperor demands (set at 2114h, from [`DEMANDS`], a
/// random factor and [`SHIPMENT_RATIO`]).
pub const SPICE_DEMANDED: u16 = 0x00bc;
/// How the last shipment compared to the demand: shipped × 80h ÷
/// demanded (80h: as much, 1..FFh; 258Dh); its sign picks the
/// Emperor's line (2125h).
pub const SHIPMENT_RATIO: u16 = 0x00be;
/// The shipment's bits: 80h a demand is on (211Dh sets 90h), 1 an
/// amount chosen (24B9h), 2/4 which offers were made (22EBh); 8 tested
/// at 25A8h, 20h set at 2951h.
pub const SHIPMENT_FLAGS: u16 = 0x00bf;
/// The amount chosen for the next shipment (250Dh), shipped when the
/// player is in room 8 with character 3 present (35CFh).
pub const SHIPMENT_CHOSEN: u16 = 0x00c0;
/// How many demands the Emperor has made (20D5h).
pub const DEMANDS: u16 = 0x00c3;
/// The days left before the shipment is due (20A0h).
pub const SHIPMENT_DAYS_LEFT: u16 = 0x00cf;
/// FFFFh once an amount is chosen (2510h), 0 once shipped (25D7h).
pub const SHIPMENT_READY: u16 = 0x1158;
/// The spice in stock when the phase began (1C7Ah).
pub const PHASE_START_SPICE: u16 = 0x1170;
/// The spice paid out this phase: shipped to the Emperor or paid to a
/// smuggler (`loc_02524`, 2528h; cleared at 1C70h).
pub const SPICE_PAID: u16 = 0x1172;
/// The day the shipment is due (2093h; put back by a good one,
/// 25BDh).
pub const SHIPMENT_DUE: u16 = 0x118d;
/// The events waiting to be told: a count (up to 10, the oldest dropped
/// for a new one, `loc_029f0`), then 4-byte entries from
/// [`NEXT_EVENT`]: a word with the kind (to [`EVENT_KIND`]; 1: told) and
/// in its high byte the character who tells it (to [`TALKER`]; told
/// when it is [`PRESENT`], 2AD8h), and a word, the location record
/// where (0 for none; for character 0Fh it must be [`PLACE`]). The first is told in the room view after a while
/// (`loc_02b2a`).
pub const PENDING_EVENTS: u16 = 0x1190;
/// The first event waiting ([`PENDING_EVENTS`]): its kind and teller,
pub const NEXT_EVENT: u16 = 0x1191;
/// and its location record.
pub const NEXT_EVENT_PLACE: u16 = 0x1193;
/// The messages kept (ten words from 1179h, the oldest first, the
/// newest last: an event's phrase, `loc_026da` adding one unless it is
/// there, dropping the oldest when full).
pub const MESSAGES: u16 = 0x1179;
/// How many of [`MESSAGES`] there are.
pub const MESSAGE_COUNT: u16 = 0x00c8;
/// How many of them are unread (up as one comes, 2713h; down as one
/// is read, 2941h).
pub const UNREAD_MESSAGES: u16 = 0x00c9;
/// The smugglers: six records of 11h bytes. Byte 0 the location's
/// byte 0 where it is; 1 its patience ([`SMUGGLER_PATIENCE`]); 2 bits
/// (8: met, 20h/40h how the last talk ended); 3 the day of the last
/// visit; 4..8 how many of each of the five items it has (refilled
/// each day, `loc_01c46`); 9..0Dh their prices (halved, 80h: sold
/// there); word 0Eh the spice owed it; 10h the day the debt began. The
/// current one is at [10B4h], character 13's word 0Ch.
pub const SMUGGLERS: u16 = 0x10d8;
/// The current smuggler's byte 2, for the dialogue's conditions
/// (`loc_0235f`).
pub const SMUGGLER_FLAGS: u16 = 0x001c;
/// The current smuggler's byte 1: how many answers it stands haggling
/// before it breaks off, the price then 0 (2483h).
pub const SMUGGLER_PATIENCE: u16 = 0x001d;
/// The days since the player last visited the current smuggler
/// (its byte 3; 1 the first time, 2339h).
pub const SMUGGLER_DAYS_AWAY: u16 = 0x001e;
/// The days since the player went into debt with the current smuggler
/// (its byte 10h; 0 without a debt, 237Eh).
pub const DEBT_DAYS: u16 = 0x001f;
/// The spice owed to the current smuggler (its word 0Eh; a purchase adds
/// to both, 23F0h).
pub const SMUGGLER_DEBT: u16 = 0x0020;
/// How many smugglers the player owes spice (23FCh; one less when paid,
/// 2520h).
pub const DEBTS: u16 = 0x0022;
/// The price asked for [`SMUGGLER_ITEM`] (twice the smuggler's byte
/// 9 + i, 23D1h), an eighth off each time it is haggled (23D5h).
pub const SMUGGLER_PRICE: u16 = 0x009d;
/// The equipment offered: 0E8h + its index i (0 the ornithopter, then
/// the krys knives, laser guns, weirding modules, atomics), its name
/// the command phrase of that number.
pub const SMUGGLER_ITEM: u16 = 0x11f1;
/// How many kinds of equipment the smugglers sell (counted up by two
/// events, 1166h and 11C6h).
pub const ITEMS_OFFERED: u16 = 0x1141;
/// Sietches discovered.
pub const SIETCHES_FOUND: u16 = 0x0027;
/// How many have joined you ([28h]).
pub const JOINED: u16 = 0x0028;
/// The player's charisma (0..C8h); every 4 points raise the troops'
/// motivation.
pub const CHARISMA: u16 = 0x0029;
/// Game phase (story progress), raised by `set_game_phase`.
pub const GAME_PHASE: u16 = 0x002a;
/// Cleared when the game phase changes.
pub const PHASE_CHANGED_FLAG: u16 = 0x00ff;
/// Story bits.
pub const EVENT_FLAGS: u16 = 0x000a;
/// The endgame's stage: 0 before it (no Emperor's shipments then,
/// 20AEh), 1 when it starts (749Dh), one more as it goes: the location
/// record 11Ch taken (73A9h, 2D2Ch), `loc_0a1ed`, and at 4 a talk with
/// character 2 once the total of `loc_01258` reaches 1000 (1243h). From
/// 7 the hour's update stops (1B5Eh).
pub const ENDGAME_STAGE: u16 = 0x00c2;
/// Talking-head animation step (counted down by `ui_head_animate_down`).
pub const HEAD_ANIMATION_STEP: u16 = 0x00e8;
pub const BYTE_0FE8: u16 = 0x0fe8;
/// The chance of an event, by the location's byte 0 (1..; `troop_event_roll`;
/// [`ITEMS_OFFERED`] is at its index 0).
pub const CHANCES: u16 = 0x1141;
/// The day from which `pick_attack_location` picks ([1156h]).
pub const ATTACK_DAY: u16 = 0x1156;
pub const BYTE_11D0: u16 = 0x11d0;
pub const COUNTER_122A: u16 = 0x122a;
pub const BYTE_0FD8: u16 = 0x0fd8;
pub const FLAG_0FDB: u16 = 0x0fdb;
pub const WORD_1154: u16 = 0x1154;
/// The count of [`g::JOINED`] from which `troop_joins` calls 121Fh.
pub const JOINED_ENOUGH: u16 = 0x1178;
pub const WORD_1201: u16 = 0x1201;
pub const FLAG_0FFB: u16 = 0x0ffb;
/// The troops: 68 records of 1Bh bytes.
pub const TROOPS: u16 = 0x08aa;
pub const TROOPS_END: u16 = 0x0fbb;
/// The record of the location the player is in (set on arriving, 4024h;
/// 0 outside one, 4763h); `get_map_position` reports its position when
/// the map y is 80h.
pub const PLACE: u16 = 0x114e;
/// The record of the last location the player was in (set with
/// [`PLACE`], kept after leaving).
pub const LAST_PLACE: u16 = 0x1150;

// --- Tables ---------------------------------------------------------------

/// The game area's rectangle.
pub const GAME_AREA_RECT: u16 = 0x1470;
/// Sky palette per time-of-day slot (16 bytes).
pub const SKY_PALETTE_BY_SLOT: u16 = 0x2280;
/// HNM resource flag names, a word pointer per video.
pub const HNM_FLAG_NAMES: u16 = 0x33a3;
/// Rectangles: the game area's copies.
pub const RECT_D834: u16 = 0xd834;
pub const RECT_D83C: u16 = 0xd83c;
/// Sine/cosine table: 4-byte entries (value, scale), indexed by angle.
pub const TRIG_TABLE: u16 = 0x4948;
/// UI element tables, one of 0Eh bytes per cursor image.
pub const UI_ELEMENTS_BY_CURSOR: u16 = 0x1b9c;

// --- Video, fonts, HNM ------------------------------------------------------

/// Font glyph drawing routine (Cryogenic: `FontRelated`).
pub const FONT_DRAW: u16 = 0x2518;
/// Font width table (Cryogenic: `FontRelated`).
pub const FONT_WIDTHS: u16 = 0x47a0;
/// Settings UI: the menu's origin, subtracted from / added to a position.
pub const SETTINGS_UI_ORIGIN_X: u16 = 0x2886;
pub const SETTINGS_UI_ORIGIN_Y: u16 = 0x2888;
/// What the cursor is on: 1 a slider, 2 a switch, 0 nothing.
pub const SETTINGS_HOVER: u16 = 0x28be;
/// Pointer to the next scene of the intro script (Cryogenic:
/// `SceneSequenceOffset`).
pub const SCENE_SEQUENCE_OFFSET: u16 = 0x4854;
pub const FRAME_TASKS_ACTIVE: u16 = 0x46d7;
/// Toggled by `blink` (the cursor or text blink phase).
pub const BLINK_PHASE: u16 = 0x4775;
/// The sub-resource index `open_sky_palette_al_sub_bl_dsdx` was asked for.
pub const PALETTE_SUB: u16 = 0x46d6;
/// The sky resource offset (0 or 1: SKY or SKYDN).
pub const SKY_VARIANT: u16 = 0x22e3;
/// How long the talking head plays (`intro_talking_head_play`).
pub const TALKING_HEAD_TIME: u16 = 0x4780;
/// Pointer to the filename buffer (`strcpy_to_filename_buf`).
pub const FILENAME_BUF_PTR: u16 = 0x38a6;
/// What `strcpy_to_filename_buf` returns in DX.
pub const FILENAME_BUF: u16 = 0x3826;
/// Font drawing position (Cryogenic: `CharacterXCoord`, `CharacterYCoord`)
/// and the line's start.
pub const FONT_X: u16 = 0xd82c;
pub const FONT_Y: u16 = 0xd82e;
pub const FONT_LINE_X: u16 = 0xd830;
pub const FONT_LINE_Y: u16 = 0xd832;
/// The loaded sprite sheet resource, far pointer (Cryogenic:
/// `spriteSheetResourcePointer`).
pub const SPRITE_SHEET: u16 = 0xdbb0;
/// Framebuffer segments (Cryogenic: `framebufferFront`, `screenBuffer`,
/// `framebufferActive`, `framebuffer2Segment`).
pub const FRAMEBUFFER_FRONT: u16 = 0xdbd6;
pub const SCREEN_BUFFER: u16 = 0xdbd8;
pub const FRAMEBUFFER_ACTIVE: u16 = 0xdbda;
pub const FRAMEBUFFER_2: u16 = 0xdbde;
pub const BACKBUFFER: u16 = 0xdc32;
/// The resource the palette came from.
pub const PALETTE_RESOURCE: u16 = 0xdbb4;
/// MIDI driver function 5's BX (Cryogenic: `midiFunc5returnBx`).
pub const MIDI_FUNC5_BX: u16 = 0xdbce;
/// HNM playback: 0 or 1 while a video plays (`check_if_hnm_complete`).
pub const HNM_STATE: u16 = 0xdbe7;
/// HNM frame counters.
pub const HNM_FRAME: u16 = 0xdbe8;
pub const HNM_FRAME_4: u16 = 0xdbee;
/// HNM read pointers and buffers.
pub const HNM_READ_PTR: u16 = 0xdc0c;
pub const HNM_DC10: u16 = 0xdc10;
pub const HNM_READ_PTR_2: u16 = 0xdc1a;
/// Map drawing: far pointer to its data, and the argument byte.
pub const MAP_GFX_DATA: u16 = 0xdcfe;
pub const MAP_GFX_ARG: u16 = 0xdd02;
pub const HNM_CE74: u16 = 0xce74;
/// The frame task list (see `hand::tasks`).
pub const FRAME_TASKS: u16 = 0xdc6a;
/// The frame loop's cursor while it walks the list (0 when not walking).
pub const FRAME_TASK_WALK: u16 = 0xdc66;

// --- Sound -------------------------------------------------------------------

/// PCM driver states (3 = done).
pub const PCM_STATE: u16 = 0x3817;
pub const PCM_STATE_2: u16 = 0x381f;
/// Segment of the lip-sync data: the voice buffer's.
pub const LIPSYNC_SEG: u16 = 0x3813;
/// Voice file handle (Cryogenic: `resFileHandle`).
pub const VOICE_FILE_HANDLE: u16 = 0x3821;
/// Settings bits; bit 0: PCM enabled.
pub const SOUND_SETTINGS: u16 = 0xdbc8;
/// Lip-sync read pointer into [`LIPSYNC_SEG`]; 0 = none.
pub const LIPSYNC_PTR: u16 = 0xdc26;
pub const VOC_PCM_PLAYING: u16 = 0xdc2b;
/// The music playing (0: none).
pub const CURRENT_MUSIC: u16 = 0xdbcb;
/// The music data, far pointer; bit 0 of [`MUSIC_SETTINGS`]: stereo.
pub const MUSIC_DATA: u16 = 0xdbb6;
pub const MUSIC_SETTINGS: u16 = 0x3810;
pub const MUSIC_DBD2: u16 = 0xdbd2;
/// What the MIDI driver's tick returned in CX.
pub const MIDI_FUNC5_CX: u16 = 0xdbd0;
/// The voice buffer, far pointer: the loaded VOC's data, its size, and
/// the VOC loaded.
pub const VOC_BUFFER: u16 = 0x3811;
pub const VOC_SIZE: u16 = 0x3815;
pub const VOC_LOADED: u16 = 0x376a;
/// Command-line options (Cryogenic: `cmdArgsMemory`); bit 10h: no MIDI
/// tick.
pub const CMD_ARGS_MEMORY: u16 = 0x2943;
/// The timer the game waits on, moved by its interrupt (Cryogenic:
/// `VideoPlayRelatedIndex`).
pub const TIMER: u16 = 0xce7a;
/// Its high word (the timer's wraps).
pub const TIMER_HIGH: u16 = 0xce7c;
/// The game clock: ticks of the timer left until the game time ([2])
/// steps (counted down by the timer's interrupt), the count it restarts
/// from, and the steps taken by the interrupt not yet run by the game
/// (`run_clock_ticks`).
pub const CLOCK_COUNTDOWN: u16 = 0x46db;
pub const CLOCK_PERIOD: u16 = 0x146e;
pub const CLOCK_STEPS_PENDING: u16 = 0x46dd;
/// Above 0 (signed) the timer's interrupt counts nothing (the clock held).
pub const CLOCK_HELD: u16 = 0xceea;
/// While not 0 the game clock does not run (the timer's interrupt skips
/// it): a count of who stopped it.
pub const CLOCK_STOPPED: u16 = 0x2788;
/// The allocator: next free pointer, the last segment (Cryogenic:
/// `allocatorNextFreeOffset`, `allocatorNextFreeSegment`,
/// `allocatorLastFreeSegment`).
pub const ALLOCATOR_NEXT_OFFSET: u16 = 0x39b7;
pub const ALLOCATOR_NEXT_SEGMENT: u16 = 0x39b9;
pub const ALLOCATOR_LAST_SEGMENT: u16 = 0xce68;
/// Set from the MIDI driver (Cryogenic: `IsSoundPresent`).
pub const IS_SOUND_PRESENT: u16 = 0xdbcd;
/// Set to load resources as stored (Cryogenic: `disableHsq`).
pub const DISABLE_HSQ: u16 = 0xce71;
/// The resource lookup table, far pointer (Cryogenic: `dnmajFunc2`).
pub const RES_LOOKUP_TABLE: u16 = 0xdbbc;
/// DUNE.DAT's file handle (read through SS).
pub const DUNE_DAT_HANDLE: u16 = 0xdbba;

// --- Input -------------------------------------------------------------------

/// Mouse cursor image (Cryogenic: `mouseCursorImageAddress`).
pub const MOUSE_CURSOR_IMAGE: u16 = 0x2582;
/// Last key scancode (Cryogenic: `keyHit`).
pub const KEY_HIT: u16 = 0xcee8;
/// Set when that key was Esc.
pub const ESC_HIT: u16 = 0xcee9;
/// Random number seeds.
pub const RAND_MASKED_SEED: u16 = 0xd824;
pub const RAND_SEED: u16 = 0xd826;
/// The third generator's seed (`rand_below`).
pub const RAND_BELOW_SEED: u16 = 0xd828;
/// Mouse buttons (low byte) and state, position.
pub const MOUSE_BUTTONS: u16 = 0xdc34;
pub const MOUSE_BUTTONS_SEEN: u16 = 0xdc35;
pub const MOUSE_X: u16 = 0xdc36;
pub const MOUSE_Y: u16 = 0xdc38;
pub const MOUSE_RECT: u16 = 0xdc58;

// --- Text ------------------------------------------------------------------

/// Which phrase file: PHRASE1x while [`PHRASE_PART`] is below
/// [`PHRASE_SPLIT`], else PHRASE2x; the one loaded.
pub const PHRASE_PART: u16 = 0x477c;
pub const PHRASE_SPLIT: u16 = 0xaad6;
pub const PHRASE_LOADED: u16 = 0x477e;
/// The language (Cryogenic: `languageSetting`).
pub const LANGUAGE: u16 = 0xceeb;
/// The voices' language, two bytes.
pub const VOICE_LANGUAGE: u16 = 0x28e7;
/// Text colour (also the outline colour of `draw_rect_outline`).
pub const TEXT_COLOUR: u16 = 0xdbe4;

// --- More video ------------------------------------------------------------

/// The map's click handler (Cryogenic: `MapClickHandlerAddress`).
pub const MAP_CLICK_HANDLER: u16 = 0x2570;
/// 80h while a transition runs.
pub const TRANSITION_ACTIVE: u16 = 0xdce6;
/// The slot of the current UI list that shows the "more" mark (A0h),
/// when the list has more items than its four slots (FFh for none;
/// D3D5h, D3F9h); `item_place` puts it at A0h.
pub const MORE_ITEM: u16 = 0xdce5;
/// The item of the current UI list under the cursor (FFh for none;
/// `highlight_hovered_text_action_item`, D5DFh).
pub const HOVERED_ITEM: u16 = 0xdce7;
/// How many items the current UI list shows (D3DEh, D3FDh).
pub const ITEM_COUNT: u16 = 0xdce8;
/// The [`TIMER`] at the last click (D897h) or scene change (18B4h,
/// 2E58h); the room view's waits count from it (`loc_02b2a`).
pub const LAST_ACTION_TIME: u16 = 0xdc5a;
/// The open video file's handle, 0 for none. (Cryogenic names it
/// `IsAnimateMenuUnneeded`.)
pub const HNM_FILE_HANDLE: u16 = 0x35a6;
/// The video's file offset and the bytes left, 32 bits each (Cryogenic:
/// `hnmFileOffset`, `hnmFileRemain`).
pub const HNM_FILE_OFFSET: u16 = 0xdc04;

// --- The game screen ----------------------------------------------------------

/// FFh while the map interface is up (`ui_transition_to_map_interface`,
/// 5A3Dh), 0 in the room view; switched at 186Eh and 5A10h.
pub const MAP_INTERFACE: u16 = 0x00fb;
/// The book: 1 while it is open (AEE7h), with bits 2 and 4 for its
/// pages' state (AF43h, B011h, cleared at B039h) and 80h while a
/// phrase is said over it (B17Ah); 0 when closed (B193h).
pub const BOOK: u16 = 0x00c6;
/// Set: the talking head does not animate up.
pub const HEAD_ANIMATION_BLOCKED: u16 = 0x11c9;
pub const BYTE_11C8: u16 = 0x11c8;
/// The scene layout (SAL) loaded, and its buffer; bit 0 of [`SAL_FLAGS`]
/// keeps kinds 0 and 1 from loading one (Cryogenic: `loadedSalIndex`).
pub const LOADED_SAL_INDEX: u16 = 0x144c;
pub const SAL_FLAGS: u16 = 0x4732;
pub const SAL_BUFFER: u16 = 0xbc6e;
/// The clip rectangle of the sun and moon.
pub const SKY_CLIP_RECT: u16 = 0x1efe;
/// Counted up while a full-screen sequence plays without the game's
/// screen: the intro (from 168Eh; cleared when the game starts, 029Dh),
/// the end video (47BDh..47C5h) and the scenes `loc_00d45` and
/// `loc_01566` play. Meanwhile no head is drawn, the music is not
/// changed, and text goes in the subtitle colour.
pub const CINEMATIC: u16 = 0x227d;
/// Set by `set_sky_palette`.
pub const SKY_PALETTE_SET: u16 = 0x46df;
/// The character waiting to call on the communicator (2, 3, 4, 6, 7 as
/// events set it), 80h while its call is on (`loc_00d8e` takes it at
/// 0DD5h), 0 for none.
pub const COMM_CALLER: u16 = 0x46d9;
/// What the hour's update (`loc_01b5e`) has to redraw when it ends, in
/// a location: 1..7Fh the room (1BB8h; set when a troop arrives there,
/// 1C12h, or the player moves, 2009h, 83F7h), 80h the whole screen
/// (1BD2h; something reaches the player, 6DE1h). Cleared at 1BB2h.
pub const REDRAW: u16 = 0x473b;
/// The talking head's zooms: the tick last seen, the start, the interval
/// (Cryogenic: `TimeBetweenFaceZooms`).
pub const HEAD_ZOOM_START: u16 = 0x476e;
pub const HEAD_ZOOM_SEEN: u16 = 0x4770;
pub const TIME_BETWEEN_FACE_ZOOMS: u16 = 0x4772;
/// A gradient section's start.
pub const GRADIENT_X: u16 = 0x47e8;
pub const GRADIENT_Y: u16 = 0x47ea;
/// Where `draw_gradient_section` puts the next edge point of a scene
/// polygon (`SAL_polygon`): from 4C60h for the right side, 4C62h for the
/// left, every other word (one x per row).
pub const POLYGON_EDGES: u16 = 0x22d9;
/// A scene polygon's colour step along a line (the signed byte * 16,
/// 3C07h), and
pub const POLYGON_SHADE_X: u16 = 0x22db;
/// its step from one line to the next (3C14h).
pub const POLYGON_SHADE_Y: u16 = 0x22dd;
/// What the polygon's line drawer gets in SI (3CA2h): 2, and in the high
/// byte the style's bits 3Eh.
pub const POLYGON_LINE_STEP: u16 = 0x22df;
/// A scene polygon's style ([si-1]; bit 0: lines drawn right to left,
/// bits 3Eh: dithered),
pub const POLYGON_STYLE: u16 = 0x47ec;
/// its colour ([si-2]),
pub const POLYGON_COLOUR: u16 = 0x47ed;
/// its first point (x, y),
pub const POLYGON_START_X: u16 = 0x47ee;
pub const POLYGON_START_Y: u16 = 0x47f0;
/// and the point where its right side ends (x, y; the left side is
/// closed back to it, 3C71h).
pub const POLYGON_END_X: u16 = 0x47f2;
pub const POLYGON_END_Y: u16 = 0x47f4;
/// The communication device's animation step.
pub const COMM_STEP: u16 = 0xd816;
/// The particles: a count, then 11h-byte records.
pub const PARTICLES: u16 = 0x3cbe;
pub const WORD_3CBE: u16 = PARTICLES;
/// Set: no date and time panel.
pub const WORD_1AFE: u16 = 0x1afe;
/// The sun's and moon's positions per time-of-day slot, 8 bytes each.
pub const SUN_MOON_POSITIONS: u16 = 0x1e7e;
/// The communication device's panel icons (a list for
/// `draw_icons_list_at_si`).
pub const COMM_PANEL_ICONS: u16 = 0x14ce;
/// The ornithopter's rotor speed.
pub const ORNI_SPEED: u16 = 0x4731;
/// Where `format_interpolated_string` writes.
pub const FORMATTED_STRING: u16 = 0xa6b0;
/// Talking heads without lip sync: the frame entries built, their count,
/// the frame table; the head's origin.
pub const HEAD_FRAMES: u16 = 0x460a;
pub const HEAD_FRAME_COUNT: u16 = 0x4608;
pub const HEAD_FRAME_TABLE: u16 = 0x47cc;
pub const HEAD_ORIGIN_X: u16 = 0x1bf0;
pub const HEAD_ORIGIN_Y: u16 = 0x1bf2;
/// The settings sliders (a value byte, a drawn flag, the x...): voice and
/// the two music ones.
pub const SLIDER_VOICE: u16 = 0x288e;
pub const SLIDER_MUSIC: u16 = 0x2896;
pub const SLIDER_MUSIC_2: u16 = 0x289e;
pub const BYTE_28AE: u16 = 0x28ae;
/// The CONDIT script, far pointer (Cryogenic: `resConditOffset`).
pub const CONDITIONS: u16 = 0xaa72;
/// The resource open as the sprite sheet (Cryogenic: `resourceId`).
pub const RESOURCE_ID: u16 = 0x2784;
/// The kind of the event being told (from [`NEXT_EVENT`]; 1: its phrase from
/// [0EBh]), FFh for none (2B1Ah, 2C7Ah).
pub const EVENT_KIND: u16 = 0x00ea;
/// The voice file: its name (DOS form at 37DAh), the position and the
/// bytes left (32 bits each), the parts read; the music loaded.
pub const VOICE_FILE_NAME: u16 = 0x37da;
pub const VOICE_FILE_POS: u16 = 0xdbc0;
pub const VOICE_FILE_LEFT: u16 = 0xdbc4;
pub const VOICE_PART: u16 = 0x3823;
pub const VOICE_LANGUAGE_PART: u16 = 0x47e0;
pub const MUSIC_LOADED: u16 = 0xdbca;

// --- The globe, the frieze ------------------------------------------------------

/// The globe's map (resource 92h), its rotation (0..18Eh, the row table
/// after it), its starting tilt and rotation.
pub const GLOBE_MAP: u16 = 0x4c60;
pub const GLOBE_ROTATION: u16 = 0x494c;
pub const GLOBE_TILT_START: u16 = 0x197c;
/// The x of the globe's side decorations (0: closed, down to -6Ah: open).
pub const GLOBE_DECORATIONS_X: u16 = 0xdd0f;
pub const GLOBE_DD03: u16 = 0xdd03;
/// The frieze's UI elements (3, the sides from +8).
pub const FRIEZE_ELEMENTS: u16 = 0x1ae6;
/// The closed book's two page counters (UI elements) and their pages.
pub const BOOK_COUNTERS: u16 = 0x1c0c;
pub const BOOK_PAGE: u16 = 0x1152;

// --- The mouse, the frame ------------------------------------------------------

/// Command-line options (Cryogenic: `cmdArgs`): bit 40h no mouse driver,
/// 80h joystick.
pub const CMD_ARGS: u16 = 0x2942;
/// The mouse's shift counts (x, y) between the driver and the game
/// (Cryogenic: `mousePosScalerX`, `mousePosScalerY`).
pub const MOUSE_SCALE: u16 = 0x2580;
pub const MOUSE_RANGE: u16 = 0xdc3a;
/// Where the cursor was drawn.
pub const CURSOR_X: u16 = 0xdc42;
pub const CURSOR_Y: u16 = 0xdc44;
/// 0 while the cursor is drawn, negative while hidden.
pub const CURSOR_SHOWN: u16 = 0xdc46;
/// Negative while it is hidden for a rectangle being drawn.
pub const CURSOR_HIDDEN_FOR_RECT: u16 = 0xdc47;
/// Set: only the plain arrow.
pub const CURSOR_PLAIN: u16 = 0x4723;
/// The frame tasks' last tick.
pub const FRAME_TASKS_LAST_TICK: u16 = 0xdc68;
/// HNM timing: FFFFh for the video's own; the tick of the last frame,
/// ticks per frame; the video's flags (bit 80h).
pub const HNM_SYNC: u16 = 0xdc1c;
pub const HNM_FRAME_TICK: u16 = 0xdc22;
pub const HNM_FLAGS: u16 = 0xdbfe;
/// Cryogenic: `hnmFlagMsb`.
pub const HNM_FLAG_MSB: u16 = 0xdbb5;
/// The voice buffer playing.
pub const VOICE_BUFFER_PLAYING: u16 = 0x3824;
/// The dialogue box up ([479Eh]): 2 or more when drawn.
pub const BOX_SHOWN: u16 = 0x479e;
/// The end of the table `handle_ctrl_v_once` appends to (in the code
/// segment).
pub const CTRL_V_TABLE_END: u16 = 0x11bd;
/// The video opened (twice), its body's offset in the file and its size
/// (32 bits each).
pub const HNM_VIDEO: u16 = 0xdc00;
pub const HNM_BODY_OFFSET: u16 = 0xdbf6;

// --- Scenes ----------------------------------------------------------------------

/// The pause: P pressed (Cryogenic: `keyPstatus`), and whether it may
/// pause now.
pub const KEY_P_STATUS: u16 = 0xce9a;
pub const PAUSE_ALLOWED: u16 = 0xce80;
/// The chapter's video.
pub const CHAPTER_VIDEO: u16 = 0x243e;
/// The result bars: 6 targets, then 6 heights.
pub const RESULT_BARS: u16 = 0xdd11;
/// The map's statistics (`map_func`).
pub const MAP_SHARE_A: u16 = 0x00a2;
pub const MAP_SHARE_B: u16 = 0x00a4;
/// The next IRUL subtitle (its frame, then its text).
pub const SUBTITLE_NEXT: u16 = 0x3622;
/// The bitmap fonts' rows.
pub const TALL_FONT_ROWS: u16 = 0x2514;
pub const SMALL_FONT_ROWS: u16 = 0x2516;

/// The game's data segment.
pub const GAME_DS: u16 = 0x1f4b;
/// The map's mode (0 when the map is not up): 80h and up zoomed in (a
/// map view; bit 7 also: the particles are hotspots), 40h the place
/// view; below 80h the hotspot mode (bit 0: the 8 points are hotspots,
/// when on; not 0: the markers' records too).
pub const MAP_MODE: u16 = 0x46eb;
/// How far from the map position a caption (a troop's, a place's) shows;
/// raised by the charisma step.
pub const REACH: u16 = 0x1176;

/// The character talking; its record is FD8h + 10h Â· it.
pub const TALKER: u16 = 0x47c4;

/// The text box: a UI element (0Eh bytes: the rectangle x0, y0, x1, y1,
/// the flags at +8, the sprite at +0Ah) set from the speech bubble's
/// rectangle (`draw_speech_bubble`, 8F2Ch); the first the cursor tests
/// in a dialogue (D57Bh).
pub const TEXT_BOX: u16 = 0x1be2;
/// The text box's flags (word 8): 80h while a text is up, the box then
/// taking clicks (D57Eh); cleared when it is closed (`loc_098f5`).
pub const TEXT_BOX_FLAGS: u16 = 0x1bea;
/// The talking head's UI element's flags (word 8 of the element at
/// [`HEAD_ORIGIN_X`]): 80h while it takes clicks (D53Ah), cleared with
/// the text (`loc_098f5`, 98CEh).
pub const HEAD_BOX_FLAGS: u16 = 0x1bf8;
/// The flags (word 8) of the UI element at 1BFEh, the picture's (by its
/// uses): 80h while a video plays there (4F1Ah), on the parchment
/// (8FAFh) and in some scenes (0EFAh, 2980h); a click on it, when
/// [`HEAD_BOX_FLAGS`] has 80h, is item 3 (D5A0h). Cleared with the text
/// (`loc_098f5`).
pub const PICTURE_FLAGS: u16 = 0x1c06;

/// Set while the player is at a place the Harkonnens hold or occupy:
/// counted up on entering one (`loc_0503c`, 5058h: its location's byte
/// 0Ah bit 2, or a kind `location_kind_check` does not pass with troops
/// there), set when the Harkonnens take the place the player is at
/// (`sub_1dd7`, 200Dh), cleared on leaving (4718h). Its picture is shown
/// instead of the room (`loc_06144`, 2Fh..33h by kind): no sky, and
/// dialogue lines redraw their text.
pub const HARKONNEN_PLACE: u16 = 0x2b;

/// The game's stage (64h to 73h: a wait after each line).
pub const STAGE: u16 = 0x23;


/// The video sprite list the dirty rect is redrawn with.
pub const SPRITE_LIST: u16 = 0xdbe0;

/// The direction of the player's move on the map (`loc_051cb`).
pub const HEADING: u16 = 0x11c7;

/// Set while the cursor is moved with the keyboard (the item it is on
/// at [4775h]).
pub const KEYBOARD_CURSOR: u16 = 0x4774;

/// The room the player is in at a place.
pub const CURRENT_ROOM: u16 = 0xb;
