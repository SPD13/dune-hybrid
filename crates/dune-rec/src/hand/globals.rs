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
/// The spice gained today: the stock plus what was paid out, less
/// yesterday's stock (`new_day`, 1C84h; raised at C046h).
pub const SPICE_INCOME: u16 = 0x00a6;
/// The day before's [`SPICE_INCOME`] (1C87h).
pub const SPICE_INCOME_BEFORE: u16 = 0x00ae;
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
/// The spice in stock at the last day's change (`new_day`, 1C7Ah).
pub const SPICE_YESTERDAY: u16 = 0x1170;
/// The spice paid out today: shipped to the Emperor or paid to a
/// smuggler (`loc_02524`, 2528h; cleared at 1C70h).
pub const SPICE_PAID: u16 = 0x1172;
/// The day the shipment is due (2093h; put back by a good one,
/// 25BDh).
pub const SHIPMENT_DUE: u16 = 0x118d;
/// Set with the last reminder of an unpaid demand (24CBh): the demand's
/// day check goes on to 215Fh from then (`spice_demand_day`).
pub const SPICE_DEMAND_LAPSED: u16 = 0x11bb;
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
/// The equipment offered (the phrase code 83h's, in [`PHRASE_ARGS`]): 0E8h + its index i (0 the ornithopter, then
/// the krys knives, laser guns, weirding modules, atomics), its name
/// the command phrase of that number.
pub const SMUGGLER_ITEM: u16 = 0x11f1;
/// How many kinds of equipment the smugglers sell (counted up by two
/// events, 1166h and 11C6h).
pub const ITEMS_OFFERED: u16 = 0x1141;
/// The sietches found and held: one up when one is found (426Fh) or
/// freed (6E40h), one down when the Harkonnens take one (74CEh).
pub const SIETCHES_HELD: u16 = 0x0027;
/// How many have joined you ([28h]).
pub const JOINED: u16 = 0x0028;
/// The player's charisma (0..C8h); every 4 points raise the troops'
/// motivation.
pub const CHARISMA: u16 = 0x0029;
/// Game phase (story progress), raised by `set_game_phase`.
pub const GAME_PHASE: u16 = 0x002a;
/// The days since [`GAME_PHASE`] last changed: 0 then, one up each
/// day (`new_day`, 1C58h).
pub const DAYS_IN_PHASE: u16 = 0x00ff;
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
/// The location the Harkonnens attack next (picked at 1E8Eh, 1F05h; 0:
/// none); a character below 9 talks of it before phase 64h
/// (`speaker_mood`).
pub const ATTACK_TARGET: u16 = 0x11db;
/// The followed troop's (troop 3, 8E0h) note: the location it heads for
/// (0: none; `job_1` sets it out, `troop_arrived_at`), then a map point
/// (6 bytes in all, copied to 4718h at 8087h).
pub const FOLLOWED_TROOP_PLACE: u16 = 0x11d3;
/// Set when a fight was seen (the night attack, 2DE2h; `map_func`):
/// the next Harkonnen take (`harkonnen_take`) is skipped, and it is
/// cleared.
pub const SKIP_HARKONNEN_TAKE: u16 = 0x11bc;
/// The regions whose marks are to be taken off the map (bit 12 - n for
/// the region n; `location_raided` adds, `unmark_pending_regions` takes).
pub const REGIONS_TO_UNMARK: u16 = 0x115a;
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
/// The record of the location talked about (`describe_place`; its
/// fields at [`TOPIC_PLACE_KIND`] on).
pub const TOPIC_PLACE_RECORD: u16 = 0x11ce;
/// The picture of a place the Harkonnens hold, as an icon list
/// (`enter_location`: 2Fh..33h by its kind; drawn at 0AF5h).
pub const HARKONNEN_PLACE_ICONS: u16 = 0x11dd;
/// The time [2] at the last time slot (`new_time_1b23`): its change of
/// hour goes to [46DEh].
pub const LAST_TIME: u16 = 0x1174;
/// The day shown on the results screen ([2] to the day; `result_bars`).
pub const RESULTS_DATE: u16 = 0x115c;

/// The phrases a phrase's codes 81h..8Fh stand for (a word each, by the
/// code's low nibble; `loc_08979`): slot 0 the phrase `draw_script_caption`
/// draws, and from the code 81h:
pub const PHRASE_ARGS: u16 = 0x11eb;
/// 81h, 82h: a place's name, its two words (`note_point`: its byte 0,
/// its byte 1 + 0Ch);
pub const PHRASE_ARG_PLACE: u16 = 0x11ed;
/// 84h: a troop's job (+18h; `describe_troop`), an encounter's place
/// (+48h; `scan_encounters`);
pub const PHRASE_ARG_SUBJECT: u16 = 0x11f3;
/// 85h: how long a troop has been at it (70h..74h, by its age;
/// `record_age_phrase`), an encounter's side (CEh, D0h);
pub const PHRASE_ARG_DETAIL: u16 = 0x11f5;
/// 86h..88h: a troop's three skills (`skill_level_phrase`);
pub const PHRASE_ARG_SKILLS: u16 = 0x11f7;
/// 89h: the direction of the nearest place (`nearest_places`, + DAh);
pub const PHRASE_ARG_DIRECTION: u16 = 0x11fd;
/// 8Ah: a troop's word 12h's low nibble, as a phrase (0..0Fh: the
/// first words of the places' names; what it says of the troop is
/// unclear).
pub const PHRASE_ARG_8A: u16 = 0x11ff;
/// The steps the player has walked outside a location (low 7 bits, up
/// to 7Fh; `ui_click_move_room`, 3F49h), bit 7 while a step is to be
/// shown (`loc_03723`); 0 on arriving at one (401Fh).
pub const WALK_STEPS: u16 = 0x4735;

// --- Tables ---------------------------------------------------------------

/// The game area's rectangle.
pub const GAME_AREA_RECT: u16 = 0x1470;
/// Sky palette per time-of-day slot (16 bytes).
pub const SKY_PALETTE_BY_SLOT: u16 = 0x2280;
/// HNM resource flag names, a word pointer per video.
pub const HNM_FLAG_NAMES: u16 = 0x33a3;
/// [D834h]: the area to redraw, x0, y0, x1, y1 (y0 C7h when empty); the
/// game area's copies are made from it.
pub const DIRTY_RECT: u16 = 0xd834;
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
/// The sub-resource index `open_sky_palette_al_sub_bl_dsdx` was asked for.
pub const PALETTE_SUB: u16 = 0x46d6;
/// The sky resource offset (0 or 1: SKY or SKYDN).
pub const SKY_VARIANT: u16 = 0x22e3;
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
/// The last control code (80h and up) met while a phrase is put together
/// (`loc_0895b`); with [`ENTRY_FLAGS`] bit 10h it picks the variant
/// (`loc_08a43`).
pub const PHRASE_CODE: u16 = 0x477f;
/// The phrase whose voice plays (AX of `load_voc_and_lipsync_data`): set
/// with the sentence (`loc_088af`), 190h in the intro, 10Ah + [0D810h]
/// more for a sentence of the speaker's own, 1000h more for its second
/// part (once `TALKING_HEAD_TIME`).
pub const VOICE_PHRASE: u16 = 0x4780;
/// The top row of the text sheet's last blit (`blit_dialogue_frame`): it
/// blits again only when [0D83Ah] is past it.
pub const TEXT_SHEET_TOP: u16 = 0x4782;
/// The language (Cryogenic: `languageSetting`).
pub const LANGUAGE: u16 = 0xceeb;
/// [28E7h]: how the people talk, 0, 1 or 2: the settings' choice - 8;
/// 2 set at the start for the languages 0 and 3 ([`LANGUAGE`]), 1 in the
/// intro and on the map (kept meanwhile in [`TALK_MODE_SAVED`]); with 0
/// no text box is drawn (8C98h). Was taken for the voices' language.
pub const TALK_MODE: u16 = 0x28e7;
/// Text colour (also the outline colour of `draw_rect_outline`).
pub const TEXT_COLOUR: u16 = 0xdbe4;

// --- More video ------------------------------------------------------------

/// The map's click handler (Cryogenic: `MapClickHandlerAddress`).
pub const MAP_CLICK_HANDLER: u16 = 0x2570;
/// 80h while a transition runs.
pub const TRANSITION_ACTIVE: u16 = 0xdce6;
/// The stack of UI screens: 4-byte entries (a list of UI elements, its
/// click handler) from here up to [`UI_STACK_LAST`]; `hand/screens.rs`.
pub const UI_STACK: u16 = 0x21be;
/// The stack's last entry (a list pushed there replaces it, 21D6h).
pub const UI_STACK_LAST: u16 = 0x21d6;
/// Points at the stack's current entry, the screen on top.
pub const UI_STACK_TOP: u16 = 0x21da;
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
/// [1AFEh]: word 0Ah (the sprite) of the first frieze side element
/// ([`FRIEZE_SIDES`]); set when there is no date and time panel.
pub const FRIEZE_LEFT_SPRITE: u16 = 0x1afe;
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
/// The voice file's part letter (`loc_0a8fa`: added to the name as
/// 41h + it), 0 with each new sentence.
pub const VOICE_LANGUAGE_PART: u16 = 0x47e0;
pub const MUSIC_LOADED: u16 = 0xdbca;

// --- The globe, the frieze ------------------------------------------------------

/// The globe's map (resource 92h) and its rotation (0..18Eh, the row
/// table after it).
pub const GLOBE_MAP: u16 = 0x4c60;
pub const GLOBE_ROTATION: u16 = 0x494c;
/// The mark on the map (x, then y at [`MAP_MARK_Y`]; set from the map
/// position, 5B60h): the centre of the map's projection, and the globe's
/// starting tilt and rotation.
pub const MAP_MARK: u16 = 0x197c;
pub const MAP_MARK_Y: u16 = 0x197e;
/// A second mark (x, y) swapped with [`MAP_MARK`] (B69Ah), with the
/// screen's centre.
pub const OTHER_MARK: u16 = 0x1980;
pub const OTHER_MARK_Y: u16 = 0x1982;
/// The x of the globe's side decorations (0: closed, down to -6Ah: open).
pub const GLOBE_DECORATIONS_X: u16 = 0xdd0f;
pub const GLOBE_DD03: u16 = 0xdd03;
/// The frieze's UI elements (3, the sides from +8).
pub const FRIEZE_ELEMENTS: u16 = 0x1ae6;
/// The closed book's two page counters (UI elements) and their pages
/// (signed bytes, FFh: none; `draw_gauges`, `in_dialogue_rects`).
pub const BOOK_COUNTERS: u16 = 0x1c0c;
/// [1152h], [1153h]: the two people shown in the frieze with their gauges
/// (sprite 41h + the character, in the UI elements 1C0Ch and 1C1Ah; FFh
/// none); the blink every other step of [2222h] hides one (FFh).
pub const SPEAKER_SLOTS: u16 = 0x1152;
pub const SPEAKER_SLOT_2: u16 = 0x1153;
/// The open book's page (a pointer `book_next_page` moves on, two pages at a
/// time).
pub const BOOK_POSITION: u16 = 0x11bf;

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
/// The video of the flight: 1 in the intro (`load_intro_script`), else 2
/// (42FFh, 50DBh); the room view's first frame is from it (37FFh), and
/// in flight the next video follows the terrain from it (`loc_04ec6`).
pub const FLIGHT_VIDEO: u16 = 0x487e;
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
/// The map's statistics (`map_func`, BFE3h): 1 + the percentage of map
/// cells with bits 30h at 10h or 20h.
pub const MAP_PERCENT_10_20: u16 = 0x00a2;
/// The percentage of cells with bits 30h at 30h (an encounter there,
/// 41A1h).
pub const MAP_PERCENT_30: u16 = 0x00a4;
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
/// The text box's pixels: a sheet in the text buffer the text is laid
/// out into (4C60h, 9017h; or its 2A80h bytes moved up to end at A5BFh,
/// A2D4h), blitted to the screen from it (9035h, 90ADh).
pub const TEXT_SHEET: u16 = 0x22fc;
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

/// Why the scripts are asked for a line, the occasion they test: 1
/// entering a room (3FAEh), 5 after it (3FCAh), 2 a join refused
/// (95DEh), the advisor's comment on a troop (0Ah, 0Ch; 7BC2h), 64h + a
/// character leaving the party (969Eh; 64h..73h: a wait after each
/// line); 0 once told (95F2h).
pub const TALK_REASON: u16 = 0x23;


/// The video sprite list the dirty rect is redrawn with.
pub const SPRITE_LIST: u16 = 0xdbe0;

/// The direction of the player's move on the map (`loc_051cb`).
pub const HEADING: u16 = 0x11c7;

/// Set while a story sequence plays: a script of steps the game plays by
/// itself (`loc_01771` counts it up; scripts at CS:128Fh, 12DBh, 12FBh,
/// 1313h, 1321h, 134Fh, 1370h). A click of both buttons only steps it
/// (`loc_01707`), the hovered item blinks ([`BLINK_PHASE`]), and the
/// map's comments say no voice (once `KEYBOARD_CURSOR`).
pub const SEQUENCE_PLAYING: u16 = 0x4774;
/// Toggled every 64h ticks by `blink` (the frame task 176Bh) while a
/// sequence plays: the hovered item's highlight.
pub const BLINK_PHASE: u16 = 0x4775;
/// Where the player was when the sequence began ([4]'s low byte, then
/// [46E0h]): put back at its end.
pub const SEQUENCE_RETURN: u16 = 0x4776;
/// The position markers of the sequence's scene (a list in CS, 0 for
/// none; `sal_read_position_markers`).
pub const SEQUENCE_MARKERS: u16 = 0x4778;
/// The sequence's next step: a byte in CS, an index in the table
/// CS:1475h (FFh: the end).
pub const SEQUENCE_NEXT: u16 = 0x477a;

/// A rectangle (x0, y0, x1, y1) put together for a draw: the dialogue
/// sprites' (`draw_dialogue_sprites`), the number display's.
pub const SCRATCH_RECT: u16 = 0x47d4;
/// One up while a character's sentence is said (`loc_096d8`,
/// `loc_09ef1`): its voice is a character's.
pub const TALKING: u16 = 0x47dc;
/// [`TALKING`] as it was when the text's voice started
/// (`start_text_voice`), for the voice played again (`loc_09eeb`).
pub const VOICE_TALKING: u16 = 0x47dd;
/// The flags word of the dialogue entry being said (bit 10h: its
/// phrases' variants picked at random; the low nibble 1: ...).
pub const ENTRY_FLAGS: u16 = 0x47de;
/// The sentence's number part: 0 none, bit 80h begun, bit 1 to draw;
/// [`NUMBER_ENTRY`] its entry in the text, [`NUMBER_RECORD`] its record
/// (word 0 the kind, 38h and up a number to say, then four words for
/// `font_draw_interpolated_string`).
pub const NUMBER_PART: u16 = 0x47e1;
pub const NUMBER_ENTRY: u16 = 0x47e2;
pub const NUMBER_RECORD: u16 = 0x47e4;
/// The record whose point `note_point` noted (DI): the portrait of a
/// talker from 0Eh is drawn from it (`show_person_on_map`).
pub const NOTED_RECORD: u16 = 0x47e6;

/// The room the player is in at a place.
pub const CURRENT_ROOM: u16 = 0xb;

// The story state (DS:0..FFh): the variables the condition scripts
// (CONDIT, `hand/script.rs`) can test, by their offset.

/// A random word (`rand` at D84Eh, DDA3h), rotated a few bits at a time
/// by its users for random choices (0BE8h, 0C40h, 71CAh).
pub const RANDOM_BITS: u16 = 0x00;
/// The room the player is walking into at a place (3FAAh), set before
/// [`TALK_REASON`] 1 so the scripts can stop the move.
pub const ENTERING_ROOM: u16 = 0x0c;
/// The room left: the old [`CURRENT_ROOM`] (4060h).
pub const PREVIOUS_ROOM: u16 = 0x0d;
/// The game time since the speaker joined the party (its word 8, with
/// its flag 40h) or left it (its word 0Ah) (950Fh).
pub const SPEAKER_TIME_SINCE: u16 = 0x16;
/// The speaker's character byte 0Fh (40h: in the party; 9502h).
pub const SPEAKER_FLAGS: u16 = 0x18;
/// 0 when a speaker starts (9417h), FFh once one of its lines is used
/// (A092h).
pub const SPEAKER_SAID_LINE: u16 = 0x19;
/// The answers given in this talk with a smuggler or the Emperor's
/// envoy: 0 at the start (2241h, 23A0h), one up per answer (2499h);
/// the patience test (247Fh) and the offer picked (24FEh) use it. 1 at
/// a room scene's start (35C4h), else 0 (35B6h).
pub const ANSWER_COUNT: u16 = 0x1a;
/// Join and leave requests since the last line: 0 at 947Ah, one up on
/// each (95EEh, 953Fh).
pub const REQUESTS: u16 = 0x1b;
/// The high byte of the message being read (2931h), 0 once closed
/// (29B8h); 0Ch calls `loc_0215f` (28E1h).
pub const MESSAGE_PARAM: u16 = 0x24;
/// One up the first time the player walks past a sietch's entrance
/// (kind below 20h, byte 0Ah bit 10h; 3F8Eh).
pub const SIETCHES_ENTERED: u16 = 0x25;
/// FFh right after such a first walk in (3F9Ah), cleared on the next
/// room move (3F37h).
pub const FIRST_TIME_INSIDE: u16 = 0x26;
/// The troop talked about (`describe_troop`, 31F6h, for the scripts):
/// its location (word 4).
pub const TOPIC_TROOP_PLACE: u16 = 0x2c;
/// Its number (byte 0).
pub const TOPIC_TROOP_ID: u16 = 0x2e;
/// Its job (byte 3's low four bits).
pub const TOPIC_TROOP_JOB: u16 = 0x2f;
/// Its byte 3: the job and the state bits (40h: moving; 20h..22h the
/// specialised).
pub const TOPIC_TROOP_STATE: u16 = 0x30;
/// Its word 12h's low four bits (also a phrase number at [11FFh]).
pub const TOPIC_TROOP_WORD_12_LOW: u16 = 0x31;
/// Its word 10h (bits 200h/100h pick the status line, 797Bh).
pub const TOPIC_TROOP_WORD_10: u16 = 0x32;
/// Its word 12h.
pub const TOPIC_TROOP_WORD_12: u16 = 0x34;
/// Its motivation as `troop_motivation` gives it (a join is refused
/// below (100 - charisma) / 4, 95D6h).
pub const TOPIC_TROOP_MOTIVATION: u16 = 0x36;
/// The skill its job uses (byte 16h + job / 4).
pub const TOPIC_TROOP_JOB_SKILL: u16 = 0x37;
/// Its three skills (bytes 16h..18h). (3 bytes)
pub const TOPIC_TROOP_SKILLS: u16 = 0x38;
/// The needs it covers (byte 19h).
pub const TOPIC_TROOP_NEEDS: u16 = 0x3b;
/// Its size (byte 1Ah).
pub const TOPIC_TROOP_SIZE: u16 = 0x3c;
/// Its byte 19h before a change of skills (77FCh, 7CFAh); then the
/// bits gained (7D90h).
pub const TOPIC_TROOP_NEEDS_GAINED: u16 = 0x3d;
/// The bits lost in that change (7D96h).
pub const TOPIC_TROOP_NEEDS_LOST: u16 = 0x3e;
/// 40h if need bit 40h was gained at [`LAST_PLACE`] (7DACh), else 0.
pub const TOPIC_TROOP_GAINED_HERE: u16 = 0x3f;
/// The days since its byte 14h (it joined or got its job).
pub const TOPIC_TROOP_DAYS: u16 = 0x40;
/// [`TOPIC_TROOP_AGE`] / 16.
pub const TOPIC_TROOP_AGE_DAYS: u16 = 0x41;
/// The game time since its word 0Ah (32CDh).
pub const TOPIC_TROOP_AGE: u16 = 0x42;
/// Its word 0Ch: the rate, or the destination while moving.
pub const TOPIC_TROOP_WORD_0C: u16 = 0x44;
/// Its word 0Eh (its low byte + E8h: the item phrase, [11F1h]).
pub const TOPIC_TROOP_WORD_0E: u16 = 0x46;
/// Its rate by age (`troop_rate_by_age`; 0 unless byte 3 is 0).
pub const TOPIC_TROOP_RATE: u16 = 0x48;
/// Its word 0Eh / [`TOPIC_TROOP_AGE`], rounded (32C1h).
pub const TOPIC_TROOP_AVERAGE: u16 = 0x4a;
/// FFh while the troop picked is farther than [1176h] (7C11h): the
/// advisor doesn't go to it (971Eh).
pub const TALK_OUT_OF_REACH: u16 = 0x4c;
/// The place talked about (`describe_place`, 331Eh, for the scripts;
/// its record at [11CEh]): its kind (byte 8).
pub const TOPIC_PLACE_KIND: u16 = 0x4d;
/// Its name: byte 1 low, byte 0 high.
pub const TOPIC_PLACE_NAME: u16 = 0x4e;
/// [`CHANCES`] by its byte 1.
pub const TOPIC_PLACE_CHANCE: u16 = 0x50;
/// Its byte 0Ah (80h unmapped, 2 Harkonnen held, 8 taken).
pub const TOPIC_PLACE_STATUS: u16 = 0x51;
/// Its spice (byte 12h).
pub const TOPIC_PLACE_SPICE: u16 = 0x52;
/// Bit 7 - k: need k not met by the troops' skills (from 46FEh,
/// 3363h).
pub const TOPIC_PLACE_UNMET_NEEDS: u16 = 0x53;
/// Its byte 1Bh.
pub const TOPIC_PLACE_BYTE_1B: u16 = 0x54;
/// Its seven needs (bytes 14h..1Ah). (7 bytes)
pub const TOPIC_PLACE_NEEDS: u16 = 0x55;
/// The words 10h of its troops (not the Harkonnens') ORed together.
pub const TOPIC_PLACE_TROOP_BITS_10: u16 = 0x5c;
/// Their words 12h ORed together.
pub const TOPIC_PLACE_TROOP_BITS_12: u16 = 0x5e;
/// Its troops of ours (not the idle counted in
/// [`TOPIC_PLACE_IDLE`]); 60h..92h counted by `count_place_troops`.
pub const TOPIC_PLACE_OUR_TROOPS: u16 = 0x60;
/// Its Harkonnen troops (byte 10h bit 80h).
pub const TOPIC_PLACE_HARKONNEN_TROOPS: u16 = 0x61;
/// Its troops by job, both sides (job 3, 7.. as job & 0Ch). (10h bytes)
pub const TOPIC_PLACE_JOBS: u16 = 0x62;
/// Its troops by word 12h's low four bits (71h is also job 0Fh's
/// count). (10h bytes)
pub const TOPIC_PLACE_TROOP_KINDS: u16 = 0x71;
/// Troops of ours on their way there.
pub const TOPIC_PLACE_OUR_INBOUND: u16 = 0x7e;
/// Harkonnen troops on their way there.
pub const TOPIC_PLACE_HARKONNEN_INBOUND: u16 = 0x7f;
/// Those by job. (10h bytes)
pub const TOPIC_PLACE_INBOUND_JOBS: u16 = 0x80;
/// Its troops of ours without a job (byte 3 just 80h).
pub const TOPIC_PLACE_IDLE: u16 = 0x90;
/// [`TOPIC_PLACE_OUR_TROOPS`] + [`TOPIC_PLACE_OUR_INBOUND`].
pub const TOPIC_PLACE_OUR_TOTAL: u16 = 0x91;
/// [`TOPIC_PLACE_HARKONNEN_TROOPS`] +
/// [`TOPIC_PLACE_HARKONNEN_INBOUND`].
pub const TOPIC_PLACE_HARKONNEN_TOTAL: u16 = 0x92;
/// Its Harkonnen troops' yields added (`troop_yield`, 3428h).
pub const TOPIC_PLACE_HARKONNEN_STRENGTH: u16 = 0x94;
/// Its other troops' yields added (3415h).
pub const TOPIC_PLACE_OUR_STRENGTH: u16 = 0x96;
/// A battle's Harkonnen strength lost (7326h, 7365h; 0 on arriving,
/// 4037h).
pub const BATTLE_HARKONNEN_LOSS: u16 = 0x98;
/// Our strength lost (732Ch, 736Ch).
pub const BATTLE_OUR_LOSS: u16 = 0x9a;
/// The odds in 256 that our side strikes: 80h x larger / smaller (up
/// to FCh), negated when the Harkonnens are stronger (`strike_odds`).
pub const TOPIC_PLACE_STRIKE_ODDS: u16 = 0x9c;
/// Random 0..3 on meeting a smuggler (2393h), one on (mod 4) per
/// haggle (2474h); 10h once it breaks off (246Bh). The scripts pick
/// its lines by it.
pub const SMUGGLER_MOOD: u16 = 0x9e;
/// The answer picked (1..3, 2496h; cleared at 2359h, A24Dh); 3 with
/// spice on talking to character 3 (223Bh).
pub const ANSWER: u16 = 0x9f;
/// The spice of the places of kind 28h and up not taken, an eighth
/// each, plus up to a sixteenth at random (1CE1h..1D0Bh).
pub const FIELD_SPICE_ESTIMATE: u16 = 0xa8;
/// Our troops' sizes added (C04Bh).
pub const ARMY_SIZE: u16 = 0xaa;
/// The Harkonnen troops' sizes added (C063h).
pub const HARKONNEN_ARMY_SIZE: u16 = 0xac;
/// How much [`SPICE_INCOME`] rose against the day before (else 0).
pub const SPICE_INCOME_RISE: u16 = 0xb0;
/// How much it fell (else 0).
pub const SPICE_INCOME_FALL: u16 = 0xb2;
/// The places the Harkonnens took (1F97h).
pub const HARKONNEN_TAKEOVERS: u16 = 0xc4;
/// Random on each arrival (4FC6h): its low four bits turn where the
/// characters present stand (3DBEh).
pub const ROOM_SEAT_SHIFT: u16 = 0xc5;
/// The nearest place to the one talked about (`nearest_places`, 5274h):
/// +0 the distance, +2 the record (FFFFh: none), +4 the direction
/// (0..7). Then four more such slots by kind, with bytes between. (6 bytes)
pub const NEAREST_PLACE: u16 = 0xca;
/// The nearest of kind below 28h, mapped. (6 bytes)
pub const NEAREST_LOW_MAPPED: u16 = 0xd0;
/// Set on each talk with character 1 (its charisma step, A1BFh): 80h -
/// [`REACH`] / 6, 0 once the reach is 64h or more; then one up each day
/// while it is 1 or more, up to FFh (1C62h); FFh from phase 15h (1091h).
pub const CHARACTER_1_DAYS: u16 = 0xd5;
/// The nearest of kind below 28h, unmapped. (6 bytes)
pub const NEAREST_LOW_UNMAPPED: u16 = 0xd6;
/// 0: the message list shows the new ones, FFh: those read (2841h).
pub const MESSAGES_READ_LIST: u16 = 0xdb;
/// The nearest of kind 28h and up, mapped. (6 bytes)
pub const NEAREST_HIGH_MAPPED: u16 = 0xdc;
/// The side of the heading a place sighted is on, 0 left, 1 right
/// (4156h).
pub const SIGHTING_SIDE: u16 = 0xe1;
/// The nearest of kind 28h and up, unmapped. (6 bytes)
pub const NEAREST_HIGH_UNMAPPED: u16 = 0xe2;
/// One up each time the player collapses in the desert and is brought
/// back (37A3h); 0 on leaving a place (3FD2h).
pub const DESERT_RESCUES: u16 = 0xe7;
/// The subject (low six bits) of the message being read (2970h).
pub const MESSAGE_SUBJECT: u16 = 0xe9;
/// The number a sentence of event 1 says (299Dh, 2BFEh).
pub const MESSAGE_NUMBER: u16 = 0xeb;
/// Turned by job 10 (7683h).
pub const JOB10_COUNTER: u16 = 0xec;
/// The met troop's motivation, FFh with its state bit 10h (31EAh);
/// 29h less per person asked (95A4h).
pub const LEADER_MOTIVATION: u16 = 0xed;
/// Bit by speaker: asked during this meeting (9590h; cleared at 31DBh).
pub const LEADER_ASKED: u16 = 0xee;
/// Added to a sentence's offset: 0, or 0Ah for character 0Ch with
/// [10A7h] bit 10h (91A0h).
pub const SENTENCE_BANK: u16 = 0xf0;
/// A place's name (byte 1 low, byte 0 high) for message 2B0Ah (1F5Bh).
pub const EVENT_PLACE_NAME: u16 = 0xf2;
/// One up per step in the desert up to 14h (3F59h), one down per
/// time slot, 0 below 10h (1B2Fh); from 10h the greeting is the one
/// for coming back from the desert (918Ah).
pub const DESERT_FATIGUE: u16 = 0xf4;
/// One up per action 14 with character 1 (A17Ah); 0 as
/// [`DESERT_FATIGUE`] drops (1B3Ah).
pub const DESERT_REMARKS: u16 = 0xf5;
/// 1 when the troops' march to the place is shown (4D2Eh).
pub const MARCH_SHOWN: u16 = 0xf6;
/// Bit by character (its byte 0Eh): characters 4, 5, 7 at the place
/// talked about (0 at [`LAST_PLACE`]; 3385h).
pub const TOPIC_PLACE_CHARACTERS: u16 = 0xf7;
/// One up when `pick_attack_location` marks a target (1E92h), one
/// down when the marked troops set out (1EEFh).
pub const ATTACKS_PENDING: u16 = 0xf8;
/// 8 up per call of `attack_clock_tick` (10h at 9551h); on its wrap the
/// attacks step.
pub const ATTACK_CLOCK: u16 = 0xf9;
/// [4737h] after the troops' tick (6CCFh): while set, the waiting
/// troops are looked at again (6CD9h).
pub const TROOPS_RECHECK: u16 = 0xfa;
/// `location_balance` (with bit 0) on entering a Harkonnen place
/// (614Dh), 80h even; 0 on every entry (503Ch).
pub const BATTLE_BALANCE: u16 = 0xfd;
/// [`GAME_PHASE`] at the last day's change (1C4Bh).
pub const LAST_DAY_PHASE: u16 = 0xfe;

// --- Named from the ports' raw accesses (2026-11-19) ---

/// [0115h]: counted up once as the intro ends and the game starts (02B4h, before the player is put in the palace); nothing else in the listing reads it by address.
pub const GAME_STARTS: u16 = 0x0115;
/// [08E4h]: the location (word 4) of troop 3's record (8E0h, the followed troop); `harkonnen_take` tells event 0Dh instead of 0Ch for it.
pub const FOLLOWED_TROOP_LOCATION: u16 = 0x08e4;
/// [118Fh]: the byte before [`PENDING_EVENTS`] (SI - 1 at 2A3Bh): cleared when the last pending event is dropped (`drop_oldest_event`).
pub const PENDING_EVENTS_FLAG: u16 = 0x118f;
/// [120Dh]: the x of the first sprite of the palace plan's sprite list (120Bh: sprite, x, y words, FFFFh ends): the origin the people panel's slots are placed from (`people_panel`).
pub const PALACE_PLAN_X: u16 = 0x120d;
/// [120Fh]: the y of the first sprite of the palace plan's sprite list (120Bh); with [`PALACE_PLAN_X`] the people panel's origin.
pub const PALACE_PLAN_Y: u16 = 0x120f;
/// [1211h]: the second sprite of the palace plan's list (120Bh; sprite 3 at 10Ah, 41h): FFFFh from `on_phase_54`, which ends the list there.
pub const PALACE_PLAN_SPRITE_2: u16 = 0x1211;
/// [1217h]: the third sprite of the palace plan's list (120Bh; sprite 4 at EEh, 41h): FFFFh from `on_phase_1c`, which ends the list there.
pub const PALACE_PLAN_SPRITE_3: u16 = 0x1217;
/// [121Dh]: the fourth sprite of the palace plan's list (120Bh; sprite 5 at C1h, 41h): FFFFh from `on_phase_0c`, which ends the list there.
pub const PALACE_PLAN_SPRITE_4: u16 = 0x121d;
/// [122Eh]: the fourth button code of palace room 2 (the room list 1225h from [`ROOM_LISTS`], 5-byte entries: label phrase, 4 codes). Bit 80h keeps the button off; `on_phase_08` clears it.
pub const PALACE_ROOM_2_BUTTON_4: u16 = 0x122e;
/// [1245h]: the second button code of palace room 7 (room list 1225h, entry at 1243h). Bit 80h keeps it off; `on_phase_1c` clears it.
pub const PALACE_ROOM_7_BUTTON_2: u16 = 0x1245;
/// [1248h]: the label phrase of palace room 8 (room list 1225h, entry at 1248h; 26h). `build_action_list` sets it to 27h / 26h / 28h by the troops and [47A9h].
pub const PALACE_ROOM_8_LABEL: u16 = 0x1248;
/// [1259h]: the second button code of palace room 11 (room list 1225h, entry at 1257h). Bit 80h keeps it off; `on_phase_54` clears it.
pub const PALACE_ROOM_11_BUTTON_2: u16 = 0x1259;
/// [13C4h]: the room lists, a word table by the high byte of [4] (20h, the palace, is 1225h). Each list holds 5-byte entries, one per room from 1: label phrase, then 4 button codes (on from -5 up; `list_entry`, `panel_buttons`).
pub const ROOM_LISTS: u16 = 0x13c4;
/// [144Dh]: the next state of a person at place 1, by their byte 0Eh (`xlat` at 21C5h, `update_people`).
pub const PEOPLE_NEXT_STATE: u16 = 0x144d;
/// [1472h]: [`GAME_AREA_RECT`]'s y0 (word 2).
pub const GAME_AREA_RECT_Y0: u16 = 0x1472;
/// [1474h]: [`GAME_AREA_RECT`]'s x1 (word 4; 140h).
pub const GAME_AREA_RECT_X1: u16 = 0x1474;
/// [1476h]: [`GAME_AREA_RECT`]'s y1 (word 6; 98h).
pub const GAME_AREA_RECT_Y1: u16 = 0x1476;
/// [149Ah]: the next free slot of the map points' ring (x, y words, CS:E40Ch..E85Ch); `draw_map_points` starts from it (gfx.rs `POINTS_NEXT`).
pub const MAP_POINTS_NEXT: u16 = 0x149a;
/// [1668h]: the location caption's frame, a 0Ah-byte box (x0, y0, x1, y1 words, outline colour, fill colour). Its x0 places the caption lines (`draw_location_caption`, `caption_count_line`, `location_panel_line`).
pub const CAPTION_FRAME: u16 = 0x1668;
/// [166Ah]: [`CAPTION_FRAME`]'s y0.
pub const CAPTION_FRAME_Y0: u16 = 0x166a;
/// [166Ch]: [`CAPTION_FRAME`]'s x1, the right end of the count line (BP for `loc_0617a`).
pub const CAPTION_FRAME_X1: u16 = 0x166c;
/// [166Eh]: [`CAPTION_FRAME`]'s y1, the bottom for the skill bars (`location_panel_line`).
pub const CAPTION_FRAME_Y1: u16 = 0x166e;
/// [1671h]: [`CAPTION_FRAME`]'s fill colour (byte 9; 10h).
pub const CAPTION_FRAME_FILL: u16 = 0x1671;
/// [18E9h]: the place/troop panel's frame, a 0Ah-byte box (x0, y0, x1, y1 words, outline colour F5h, fill colour FBh). `open_place_panel` sets its y; the skills boxes and bars are placed from it.
pub const PLACE_PANEL: u16 = 0x18e9;
/// [18EBh]: [`PLACE_PANEL`]'s y0 (5, or 50h for a low particle).
pub const PLACE_PANEL_Y0: u16 = 0x18eb;
/// [18EDh]: [`PLACE_PANEL`]'s x1.
pub const PLACE_PANEL_X1: u16 = 0x18ed;
/// [18EFh]: [`PLACE_PANEL`]'s y1 (y0 + 43h), the bottom for its skill bars.
pub const PLACE_PANEL_Y1: u16 = 0x18ef;
/// [18F2h]: [`PLACE_PANEL`]'s fill colour (byte 9; FBh). Also the colour of the dialogue frame drawn around the text box (8FE3h).
pub const PLACE_PANEL_FILL: u16 = 0x18f2;
/// [18F3h]: the picture box inside [`PLACE_PANEL`], a 0Ah-byte box (x0, y0, x1, y1, two colours), 3Dh square, placed by `open_place_panel`. [4720h] points to it while a place view is open.
pub const PLACE_PICTURE_BOX: u16 = 0x18f3;
/// [18F5h]: [`PLACE_PICTURE_BOX`]'s y0.
pub const PLACE_PICTURE_BOX_Y0: u16 = 0x18f5;
/// [18F7h]: [`PLACE_PICTURE_BOX`]'s x1.
pub const PLACE_PICTURE_BOX_X1: u16 = 0x18f7;
/// [18F9h]: [`PLACE_PICTURE_BOX`]'s y1.
pub const PLACE_PICTURE_BOX_Y1: u16 = 0x18f9;
/// [192Fh]: the sprite for each of the 7 skills' bars, a byte per skill (`skill_bars`).
pub const SKILL_BAR_SPRITES: u16 = 0x192f;
/// [1936h]: the skills panel's first box (0Ah bytes: rect, outline, fill), placed from [`PLACE_PANEL`] by `place_panel_rects` and drawn by `draw_skills_panel`.
pub const SKILLS_BOX: u16 = 0x1936;
/// [1938h]: [`SKILLS_BOX`]'s y0.
pub const SKILLS_BOX_Y0: u16 = 0x1938;
/// [193Ah]: [`SKILLS_BOX`]'s x1.
pub const SKILLS_BOX_X1: u16 = 0x193a;
/// [193Ch]: [`SKILLS_BOX`]'s y1.
pub const SKILLS_BOX_Y1: u16 = 0x193c;
/// [1940h]: the skills panel's second box (0Ah bytes: rect, then the colours at +8), where the bars from 46FEh and the label 6Fh go. [0DBE2h] points to it while the panel is up.
pub const SKILL_BARS_BOX: u16 = 0x1940;
/// [1942h]: [`SKILL_BARS_BOX`]'s y0.
pub const SKILL_BARS_BOX_Y0: u16 = 0x1942;
/// [1946h]: [`SKILL_BARS_BOX`]'s y1, the bars' bottom.
pub const SKILL_BARS_BOX_Y1: u16 = 0x1946;
/// [1948h]: [`SKILL_BARS_BOX`]'s colour word (+8: outline 19h, fill 1Eh), also the text colour of its bars and label.
pub const SKILL_BARS_BOX_COLOURS: u16 = 0x1948;
/// [1954h]: the number of the troop the panel shows (0: none; troops.rs `SHOWN_TROOP`). Its high byte is [`CAPTIONED_TROOP`].
pub const SHOWN_TROOP: u16 = 0x1954;
/// [1955h]: the number of the troop whose caption/panel was last placed (`troop_caption`; 3 for troop 8E0h at 5B1Eh). Read with [`SHOWN_TROOP`] as a word (86DDh).
pub const CAPTIONED_TROOP: u16 = 0x1955;
/// [1960h]: the sprite layers of the location seen in the flight scene, as (offset, sprite) word pairs: (1960h, 19h), (1964h, 18h), ([`SITE_OFFSET`], [`SITE_SPRITE`]). `place_scene` sets the offsets.
pub const SITE_LAYERS: u16 = 0x1960;
/// [1964h]: the offset of the second layer of [`SITE_LAYERS`] (the site's offset + 1).
pub const SITE_LAYER_2_OFFSET: u16 = 0x1964;
/// [1968h]: the offset of the location seen ahead (direction off ours times 32, `location_ahead`; or from the record, `place_scene`).
pub const SITE_OFFSET: u16 = 0x1968;
/// [196Ah]: the scene sprite of the location seen ahead, by its SAL index from [`SITE_SPRITE_BY_SAL`] (0: none).
pub const SITE_SPRITE: u16 = 0x196a;
/// [196Ch]: the calls of `location_ahead` left before it searches the cells ahead again (6 after a find).
pub const SITE_SEARCH_DELAY: u16 = 0x196c;
/// [196Dh]: the scene sprite of a location by its SAL index (`calc_SAL_index`; bytes 11h, 10h, 12h, ...).
pub const SITE_SPRITE_BY_SAL: u16 = 0x196d;
/// [1AF4h]: the frieze's two side elements (0Eh-byte UI records; the second is the date panel at E4h..12Ch, 98h..C6h). `nudge_panel` steps their +0Ah words ([`FRIEZE_LEFT_SPRITE`], [`FRIEZE_RIGHT_SPRITE`]) toward a multiple of 3.
pub const FRIEZE_SIDES: u16 = 0x1af4;
/// [1B0Ch]: word 0Ah (the sprite) of the second frieze side element (1B02h, the date panel). With 3..5, `draw_panel_button_frame` also outlines the rect 2458h.
pub const FRIEZE_RIGHT_SPRITE: u16 = 0x1b0c;
/// [1B48h]: the panel's five text action rows (0Eh-byte UI elements at 5Ch..E4h, from y 9Fh, 8 high; `draw_1b48_and_cursor`, `button_label`, `highlight_hovered_text_action_item`).
pub const TEXT_ACTIONS: u16 = 0x1b48;
/// [1B4Ch]: the x1 of [`TEXT_ACTIONS`]'s first row (E4h), the right end of the column the cursor is tested against.
pub const TEXT_ACTIONS_X1: u16 = 0x1b4c;
/// [1B98h]: the sprite (+0Ah) of the first of the panel's six buttons (1B8Eh, copied from a set such as 1C76h): 23h in the plain set, 21h/22h at a place.
pub const FIRST_BUTTON_SPRITE: u16 = 0x1b98;
/// [1BDCh]: the flags (+8) of the sixth panel button (1BD4h), the palace plan's: 80h in the palace (place 100h), else 20h (`panel_buttons`).
pub const PLAN_BUTTON_FLAGS: u16 = 0x1bdc;
/// [1BE4h]: [`TEXT_BOX`]'s y0 (word 2); 2 less while the dialogue frame is drawn (`frame_8fd1`).
pub const TEXT_BOX_Y0: u16 = 0x1be4;
/// [1BEEh]: [`TEXT_BOX`]'s click handler (word 0Ch): 7BEDh with the dialogue frame, 9468h on the parchment, F66h by default.
pub const TEXT_BOX_HANDLER: u16 = 0x1bee;
/// [1BF4h]: the talking head element's x1 (word 4 of the element at [`HEAD_ORIGIN_X`]), kept within 140h (`setup_intro_talking_head`).
pub const HEAD_BOX_X1: u16 = 0x1bf4;
/// [1C14h]: the flags (+8) of the first of [`BOOK_COUNTERS`]: 80h when back from the map (1886h), 0 when a place is left.
pub const BOOK_COUNTER_1_FLAGS: u16 = 0x1c14;
/// [1C22h]: the flags (+8) of the second of [`BOOK_COUNTERS`] (1C1Ah): 80h when back from the map, 0 when a place is left.
pub const BOOK_COUNTER_2_FLAGS: u16 = 0x1c22;
/// [1C30h]: the flags (+8) of the UI element after the book counters (1C28h, 0..28h by 4..2Eh, handler B1EEh): 80h on a book page with a chapter video ([`CHAPTER_VIDEO`]), else 0.
pub const BOOK_VIDEO_BUTTON_FLAGS: u16 = 0x1c30;
/// [1CC4h]: the flags (+8) of the palace plan button (1CBCh, sprite 24h, handler 18EEh) in the default button set 1C76h; set together with [`PLAN_BUTTON_FLAGS`].
pub const SET_PLAN_BUTTON_FLAGS: u16 = 0x1cc4;
/// [1F0Fh]: the action list: a 0 byte, then 4-byte entries (phrase byte, flags byte with 40h off, handler word), a 0 word last. Rebuilt by `build_action_list`, emptied at 2C54h.
pub const ACTION_LIST: u16 = 0x1f0f;
/// [1F10h]: the phrase (and flags) word of [`ACTION_LIST`]'s first entry; 0 for an empty list.
pub const ACTION_LIST_FIRST: u16 = 0x1f10;
/// [1F11h]: the flags of [`ACTION_LIST`]'s first entry (40h off), kept in step with [`MAP_ACTION_FLAGS`] when that entry is the map action (`note_21fd`).
pub const ACTION_LIST_FIRST_FLAGS: u16 = 0x1f11;
/// [1F12h]: the handler of [`ACTION_LIST`]'s first entry (4FFBh: the map action).
pub const ACTION_LIST_FIRST_HANDLER: u16 = 0x1f12;
/// [21FDh]: the flags byte of the map-mode action template 21FCh (phrase A9h, handler 4FFBh): 40h (off) while the countdown [4726h] runs, cleared with it (`note_21fd`).
pub const MAP_ACTION_FLAGS: u16 = 0x21fd;
/// [2220h]: the UI list `install_current_ui_list` installs (handler F66h): 1FBAh for a room or scene, 1FAEh for a talk.
pub const CURRENT_UI_LIST: u16 = 0x2220;
/// [2222h]: the book gauges' blink, a counter per side (a byte each), stepped every 64 ticks (`blink_gauges`).
pub const GAUGE_BLINK: u16 = 0x2222;
/// [2240h]: the width (word 4) of the picture text box 223Ch (x, y, width, height); with [4793h] the size of [`TEXT_SHEET`] (`outline_picture`).
pub const PICTURE_BOX_WIDTH: u16 = 0x2240;
/// [2244h]: the box a phrase over the map is drawn in (x, y, width, height words; dialogue.rs `MAP_BOX`), placed from [`PLACE_PANEL`] by `open_place_panel`.
pub const MAP_BOX: u16 = 0x2244;
/// [2246h]: [`MAP_BOX`]'s y.
pub const MAP_BOX_Y: u16 = 0x2246;
/// [2254h]: the communication device's frame, a 0Ah-byte box (rect, colours; `fill_and_outline`); the caller's head is drawn at its x + 1, y + 1.
pub const COMM_FRAME: u16 = 0x2254;
/// [2256h]: [`COMM_FRAME`]'s y0.
pub const COMM_FRAME_Y0: u16 = 0x2256;
/// [225Dh]: the comm device's callers' table (it starts at [`COMM_FRAME`]'s byte 9): a byte per caller, its head (negative: none; `start_comm_call` `xlat`). `comm_draw_caller` also reads word [225Dh + 2 * caller] as a position.
pub const COMM_CALLER_FACES: u16 = 0x225d;
/// [226Dh]: word 8 of the book's text box 2265h (x, y, width, height, then this; 0Ah). `sub_9468` sets it to 0Ah when a dialogue text is set up.
pub const BOOK_BOX_WORD_8: u16 = 0x226d;
/// [227Eh]: a routine run once after a dialogue entry's voice starts (taken, F66h left; 2CCFh for talker 5; dialogue.rs `ENTRY_HOOK`).
pub const DIALOGUE_ENTRY_HOOK: u16 = 0x227e;
/// [22A6h]: the talking head loaded (FFFFh: none). A new one closes the text and opens its resource; it also picks the scene's sound sequence and the test voice (dialogue.rs `HEAD`).
pub const LOADED_HEAD: u16 = 0x22a6;
/// [2406h]: the kind bits (BX) the book's page search uses (`scan_dialogues` of A8h), kept from the first page found (AF8Ch) for the next pages; 0 when a dialogue panel opens.
pub const BOOK_PAGE_KINDS: u16 = 0x2406;
/// [2440h]: the globe's clip rectangle (x0, y0, x1, y1: 60h, 0Fh, EAh, 86h; globe.rs `GLOBE_CLIP_RECT`), also the bounds of `open_box`.
pub const GLOBE_CLIP_RECT: u16 = 0x2440;
/// [2442h]: [`GLOBE_CLIP_RECT`]'s y0.
pub const GLOBE_CLIP_RECT_Y0: u16 = 0x2442;
/// [2444h]: [`GLOBE_CLIP_RECT`]'s x1.
pub const GLOBE_CLIP_RECT_X1: u16 = 0x2444;
/// [2446h]: [`GLOBE_CLIP_RECT`]'s y1.
pub const GLOBE_CLIP_RECT_Y1: u16 = 0x2446;
/// [2460h]: the globe's tilt (signed word, kept within its range; `globe_increment_tilt_by_ax`, `set_globe_tilt_and_rotation`).
pub const GLOBE_TILT: u16 = 0x2460;
/// [2581h]: the y shift count of [`MOUSE_SCALE`] (byte 1).
pub const MOUSE_SCALE_Y: u16 = 0x2581;
/// [2772h]: word 8 of the outline rect 276Ah, the dash pattern handed to the driver's outline (BP, far [3901h]): 5555h (dotted) around the map point, else FFFFh.
pub const OUTLINE_PATTERN: u16 = 0x2772;
/// [2786h]: the routine that picks the next particle to draw: C827h (`skip_zero_words`, list order) by default, C835h (`lowest_record`, lowest first) on the map.
pub const DRAW_ORDER_PICKER: u16 = 0x2786;
/// [2882h]: the high word of the lip-sync clock's step ([0DC2Ch]:[0DC2Eh] moved by [2882h]:[2884h]), measured by `audio_test_frequency`.
pub const LIP_SYNC_STEP_HIGH: u16 = 0x2882;
/// [2884h]: the low word of the lip-sync clock's step (see [`LIP_SYNC_STEP_HIGH`]).
pub const LIP_SYNC_STEP_LOW: u16 = 0x2884;
/// [28CFh]: the settings panel's language row: the choice per 7-pixel column (`xlat`). Below 7 a language, above 7 the mode 8 + n for [28E7h] (`settings_click`).
pub const SETTINGS_ROW_CHOICES: u16 = 0x28cf;
/// [28E8h]: the copy of [28E7h] ([`TALK_MODE`]) kept while the map or a dialogue forces it. Saved on closing the settings (`close_settings_panel`), put back on leaving the map.
pub const TALK_MODE_SAVED: u16 = 0x28e8;
/// [2941h]: DOS's Ctrl-Break flag as it was at start (INT 21h AX 3301h DL); turned off meanwhile and given back on exit.
pub const CTRL_BREAK_SAVED: u16 = 0x2941;
/// [2944h]: the sound drivers chosen: high nibble the sound/voice driver (+7), low nibble the music driver (+2). Each is cleared when its driver fails to start; the low one also gates EMS for MIDI.
pub const AUDIO_DRIVERS: u16 = 0x2944;
/// [375Ch]: the song for each music situation (a byte by AL, `xlat` at AD75h in `pick_music`): 0 none, a plain song, or bit 80h a marked one (low 6 bits) faded to when it changes.
pub const MUSIC_BY_SITUATION: u16 = 0x375c;
/// [37E2h]: a letter of the voice file name (VOICE_FILE_NAME + 8): 49h ('I') set by `play_voice_19` (AB5Ah), flipped by XOR 6 to read the other half of the lip-sync data (A74Ah).
pub const VOICE_FILE_NAME_LETTER: u16 = 0x37e2;
/// [37F7h]: the digit of the patch file name DUNE?.DAT (at 37F2h), counted up to '9' by `open_dune_dat` (E692h); each one found is loaded and run.
pub const PATCH_FILE_DIGIT: u16 = 0x37f7;
/// [37FAh]: the music playlist (9 bytes, copied from 3804h at ACA6h; shuffled in mode 2 of MUSIC_SETTINGS by `loc_0acbf`); the song list restarts from it at its end (AD21h).
pub const MUSIC_PLAYLIST: u16 = 0x37fa;
/// [380Eh]: pointer to the next song of MUSIC_PLAYLIST, read and moved on at AD18h/AD30h.
pub const MUSIC_PLAYLIST_NEXT: u16 = 0x380e;
/// [38AFh]: the slot digit of the save file name (at 38A8h, +7): '1' + the slot (B38Ch, B3BDh); `save_slot_label_si` stops above '2' (32h).
pub const SAVE_FILE_DIGIT: u16 = 0x38af;
/// [38FBh]: the segment of the video decoder at CS:CC92h, patched into it by `hnm_decode_video_frame` (CC96h) for videos from 19h.
pub const HNM_DECODER_SEGMENT: u16 = 0x38fb;
/// [396Fh]: the music driver's segment: the first segment slot of its table of 7 far entries at 396Dh (`load_driver_ax_with_vtable_at_si`, E7CEh; far [3979h] plays a song); + 10h becomes ALLOCATOR_NEXT_SEGMENT when it fails.
pub const MUSIC_DRIVER_SEGMENT: u16 = 0x396f;
/// [398Bh]: the sound (PCM) driver's segment: the first segment slot of its table of 8 far entries at 3989h (E775h); + 10h becomes ALLOCATOR_NEXT_SEGMENT when it fails.
pub const SOUND_DRIVER_SEGMENT: u16 = 0x398b;
/// [3991h]: the sound driver's entry 2 (the table at 3989h, 4 bytes an entry): plays the voice buffer at SI (VOC_BUFFER, 3811h; A56Fh and others).
pub const SOUND_DRIVER_PLAY: u16 = 0x3991;
/// [39A9h]: the 1 KB blocks held in EMS (the EMS pages allocated * 16, ED93h); the extended-memory table is indexed from it ([39A9h] + AX, `alloc_extended_memory`) and sized [39A9h] + 2 * B9h words; `hnm_initialize_memory_handler` compares it with 15Eh.
pub const EMS_BLOCKS: u16 = 0x39a9;
/// [39ABh]: the joystick's thresholds, four words (set by `initialize_joystick` from the centre: x/2, x*3/2, y/2, y*3/2; all 0 uncalibrated): left of this x is left (DD24h),
pub const JOYSTICK_LEFT: u16 = 0x39ab;
/// [39ADh]: right of this x is right (DD2Eh),
pub const JOYSTICK_RIGHT: u16 = 0x39ad;
/// [39AFh]: above this y is up (DD40h),
pub const JOYSTICK_UP: u16 = 0x39af;
/// [39B1h]: below this y is down (DD4Ah).
pub const JOYSTICK_DOWN: u16 = 0x39b1;
/// [39B3h]: what the music driver's set-up (far [396Dh]) gets in AX (E7DDh); probably its port or settings.
pub const MUSIC_DRIVER_ARG: u16 = 0x39b3;
/// [39B5h]: what the sound driver's set-up (far [3989h]) gets in AX (E77Eh); probably its port or settings.
pub const SOUND_DRIVER_ARG: u16 = 0x39b5;
/// [3CBCh]: the message printed after the exit (a 0-ended string, 005Bh), 0 for none: 3624h / 364Bh when a sound / music driver fails, 368Dh out of memory, 36B4h. Also where the start-up clearing (to DD1Dh) begins.
pub const EXIT_MESSAGE: u16 = 0x3cbc;
/// [4540h]: the talking head's sprite list as last drawn (a count, then sprite, x, y bytes; dialogue.rs HEAD_SPRITES_KEPT), compared with the list at 4608h (`rects_missing`); its word 0 when the dialogue frame is to be redrawn.
pub const HEAD_SPRITES_DRAWN: u16 = 0x4540;
/// [46D2h]: the talking head's offset, x (from the table 22B9h by the head and pose [47D0h], 7AC7h), subtracted from its sprites' places (9DAEh),
pub const HEAD_OFFSET_X: u16 = 0x46d2;
/// [46D4h]: and its y (7ACBh, 9DB3h).
pub const HEAD_OFFSET_Y: u16 = 0x46d4;
/// [46D8h]: set while the map and its panel are redrawn (7B38h, troops.rs REDRAWING) and at a caption's redraw (5F04h): then 7B2Bh does not restore what is under a box (`loc_0c0e8`); cleared by `redraw_panel_box` (7B0Fh).
pub const SKIP_BOX_RESTORE: u16 = 0x46d8;
/// [46DAh]: 1 while `run_clock_ticks` runs (0FD9h..1005h); the hour's update does not redraw the room meanwhile (1BBFh, 1BDCh).
pub const CLOCK_TICKING: u16 = 0x46da;
/// [46DEh]: the change of the time [2]'s high nibble at the time step (1B56h): non-zero on a new day, while the day's updates run (`grow_spice`, `troop_day_update`).
pub const DAY_CHANGED: u16 = 0x46de;
/// [46E0h]: the sky palette shown, swapped with SKY_PALETTE_SET to see whether the light changed (2E2Eh, then a fade); restored from SEQUENCE_RETURN's high byte (1742h), one down per video step (48D1h).
pub const SKY_PALETTE_SHOWN: u16 = 0x46e0;
/// [46EDh]: MapView field `redraw` (hand/map_view.rs): the routine that draws the view (49A0h, 4377h, ...).
pub const MAP_VIEW_REDRAW: u16 = 0x46ed;
/// [46FEh]: a location's 7 skill needs (its bytes 14h..1Ah) less what its troops' skills meet (`noted_location_skill_needs`, `skill_needs`); troop-panel scratch.
pub const LOCATION_SKILL_NEEDS: u16 = 0x46fe;
/// [46FFh]: LOCATION_SKILL_NEEDS[1] (the location's byte 15h unmet): while non-zero the room shows its figures (3A39h) and the action 21DCh is enabled; it replaces PLACE_TROOP_COUNT's low byte at 4E83h.
pub const LOCATION_SKILL_NEED_1: u16 = 0x46ff;
/// [4710h]: the place view's inner box, a rect (left, top, right, bottom; used as the mouse rect at [0DBE0h]): left and top from the map point [11C1h]/[11C3h] (5406h), or 5Ch and the panel's y for the place panel (7A15h);
pub const PLACE_BOX: u16 = 0x4710;
/// [4712h]: its top,
pub const PLACE_BOX_TOP: u16 = 0x4712;
/// [4714h]: its right (the view's right + 5),
pub const PLACE_BOX_RIGHT: u16 = 0x4714;
/// [4716h]: its bottom (the view's bottom + 0Ch; the map header changes within 0Ah of it).
pub const PLACE_BOX_BOTTOM: u16 = 0x4716;
/// [4718h]: the locations picked on the map, 3 words (to 471Dh; filled at 8282h, PICKED_COUNT of them).
pub const PICKED_LOCATIONS: u16 = 0x4718;
/// [4720h]: the box last shown in the place view (1E6Eh on opening, 18F3h from the dialogue), redrawn and replaced at 553Eh; 0 for none.
pub const PLACE_VIEW_LAST_BOX: u16 = 0x4720;
/// [4722h]: the place view's toggle (XOR FFh by a click under its box, 5970h; 0 on opening): set, the regions are coloured by the troops' strength instead of their kind (`region_colours`), the legend is string 68h, and no region is looked up under the cursor.
pub const PLACE_VIEW_STRENGTH: u16 = 0x4722;
/// [4724h]: the region under the cursor in the place view (50h..5Fh, FFh none); its names are redrawn when it changes (`map_header_by_cursor`).
pub const MAP_HEADER_REGION: u16 = 0x4724;
/// [4725h]: the colour the map header was last drawn in (CL: FEh normal, F5h near the box's bottom); redrawn only on a change (57B7h).
pub const MAP_HEADER_COLOUR: u16 = 0x4725;
/// [4726h]: raised by 20h for a walk through the open desert (5031h), down by 20h at a map place with bits 30h (`map_place_watch`); when it runs out the communicator caller [46D9h] becomes 2. Cleared by `reset_desert_countdown`.
pub const DESERT_COUNTDOWN: u16 = 0x4726;
/// [472Dh]: where the room's figures stand, x (+ 0Ch, 3A5Fh): the extra hotspot of a dialogue (CX 2Fh, `dialogue_hit`); 0 when the room is drawn (37B8h),
pub const ROOM_FIGURES_X: u16 = 0x472d;
/// [472Fh]: and its y (+ 8, 3A67h).
pub const ROOM_FIGURES_Y: u16 = 0x472f;
/// [4733h]: a word: the count from `loc_0661d` (handler 4E04h) in the low byte, the location's byte 14h (its first skill need) in the high byte [4734h] (4DF9h); 0 for the map place's scene (4E12h); the enemy watch needs [4734h] set.
pub const PLACE_TROOP_COUNT: u16 = 0x4733;
/// [4736h]: the dotted pattern of a move's trail: 44h at `start_move`, rotated left each span (659Dh); its carry picks the bytes whose bits 30h become 10h.
pub const MOVE_TRAIL_PATTERN: u16 = 0x4736;
/// [4737h]: cleared at the troops' tick (6C8Ah), FFh when job 8 starts a troop (769Eh); copied to TROOPS_RECHECK after the tick (6CCCh).
pub const TROOPS_TICK_RECHECK: u16 = 0x4737;
/// [4738h]: how many locations are picked at PICKED_LOCATIONS (up to 3).
pub const PICKED_COUNT: u16 = 0x4738;
/// [4739h]: the bits ORed into the map cells by a location's circle (30h at `draw_location_circle`, 20h at 644Eh; `location_circle_span`).
pub const CIRCLE_COLOUR: u16 = 0x4739;
/// [473Ah]: 1 during the event at 7317h..7391h (its waits with SCENE_TIMER 0Bh / 11h): meanwhile a creature's new random move never gets bit 10h (0C5Bh).
pub const CREATURES_STILL: u16 = 0x473a;
/// [473Ch]: the location of record 1018h for this troops' tick, or 0 (6C83h); `phase_30_check` and job 4 look at it.
pub const TICK_LOCATION: u16 = 0x473c;
/// [473Eh]: 0 in the intro (`load_intro_script`), 1 in the game's screens (36A3h, 42F5h, 50E1h): the map frame gets the icons 14C0h of resource 24h, the message line moves 4 down (45F4h), the view is shown through the game area.
pub const GAME_FRAME: u16 = 0x473e;
/// [4749h]: the map pointer's box (sprite 4Ch), a rect: left (0 when the pointer is outside the view),
pub const MAP_POINTER_BOX: u16 = 0x4749;
/// [474Bh]: top,
pub const MAP_POINTER_BOX_TOP: u16 = 0x474b;
/// [474Dh]: right,
pub const MAP_POINTER_BOX_RIGHT: u16 = 0x474d;
/// [474Fh]: bottom.
pub const MAP_POINTER_BOX_BOTTOM: u16 = 0x474f;
/// [4751h]: the map pointer's blinks counted (0 when placed; the sprite drawn on odd counts, `blink_map_pointer`).
pub const MAP_POINTER_BLINKS: u16 = 0x4751;
/// [4752h]: the troop marker's particle (in the list at PARTICLES), stepped alone between the full steps (`animate_map_sprites`); the first of two pointers kept right when a particle is removed.
pub const TROOP_MARKER_PARTICLE: u16 = 0x4752;
/// [4756h]: the troop character 14 stands for (`place_troop_marker`); told of first in its sentence 0Eh.
pub const CHARACTER_14_TROOP: u16 = 0x4756;
/// [4758h]: the troops character 15 stands for, 8 word slots (to 4767h; character 12's troop at 4768h after them).
pub const CHARACTER_15_TROOPS: u16 = 0x4758;
/// [476Ah]: the slots of CHARACTER_15_TROOPS filled (modulo 8, then one up); cleared with the visitors (`reset_visitor_positions`).
pub const CHARACTER_15_TROOP_COUNT: u16 = 0x476a;
/// [476Bh]: the slot (+ 1) of CHARACTER_15_TROOPS holding troop 8E0h, 0 for none.
pub const CHARACTER_15_SPECIAL_SLOT: u16 = 0x476b;
/// [476Ch]: the slot of CHARACTER_15_TROOPS talked of (the speaker 0Fh + it; `phrase_9381`, `pick_sentence`).
pub const CHARACTER_15_SLOT: u16 = 0x476c;
/// [476Dh]: 0 for `offer_answers`, 1 for dialogue action 4 (A24Ah); picks how the answers are set up (24F7h).
pub const ANSWER_MODE: u16 = 0x476d;
/// [47F6h]: pointer to the scene's position markers (17h bytes on the stack in `draw_SAL`, 3B5Ch), moved on as each is used.
pub const SAL_MARKERS_PTR: u16 = 0x47f6;
/// [485Dh]: the scene timer (byte 7 of the scene's spawner record at 4856h), down each frame step; at 10h and below the scene picture (`loc_00d0d`); set to 0Bh / 11h at 7382h.
pub const SCENE_TIMER: u16 = 0x485d;
/// [485Eh]: the scrolling scene's resource (`setup_scroll_scene`); its record runs to 487Dh.
pub const SCROLL_SCENE_RESOURCE: u16 = 0x485e;
/// [4860h]: the scrolling scene's view rect (`clamp_view_rect`): left,
pub const SCROLL_VIEW: u16 = 0x4860;
/// [4862h]: top,
pub const SCROLL_VIEW_TOP: u16 = 0x4862;
/// [4864h]: right,
pub const SCROLL_VIEW_RIGHT: u16 = 0x4864;
/// [4866h]: bottom.
pub const SCROLL_VIEW_BOTTOM: u16 = 0x4866;
/// [486Ah]: the scrolling scene's script position (in the segment [0DBB2h]; restarted from [4868h] at FFh).
pub const SCROLL_SCRIPT: u16 = 0x486a;
/// [486Eh]: the scrolling scene's place: x,
pub const SCROLL_X: u16 = 0x486e;
/// [4870h]: y,
pub const SCROLL_Y: u16 = 0x4870;
/// [4872h]: its step along x (8.8, through the fraction [4876h]),
pub const SCROLL_STEP_X: u16 = 0x4872;
/// [4874h]: its step along y (through [4878h]),
pub const SCROLL_STEP_Y: u16 = 0x4874;
/// [4876h]: the x fraction accumulated,
pub const SCROLL_FRACTION_X: u16 = 0x4876;
/// [4878h]: the y fraction accumulated.
pub const SCROLL_FRACTION_Y: u16 = 0x4878;
/// [487Ah]: the scrolling scene's size (from its sprite table): width,
pub const SCROLL_WIDTH: u16 = 0x487a;
/// [487Ch]: height.
pub const SCROLL_HEIGHT: u16 = 0x487c;
/// [494Ah]: the globe's latitude rows, 63h of 8 bytes in the data BAh read to 4948h: the circle's length, its offset by the rotation and that offset's fraction (`recalculate_globe_rotation_table`); row 0's length (18Eh) is the full turn.
pub const LATITUDE_ROWS: u16 = 0x494a;
/// [494Eh]: the fraction of GLOBE_ROTATION (row 0's +4 in LATITUDE_ROWS), 0 when the rotation is set (BA7Fh).
pub const GLOBE_ROTATION_FRACTION: u16 = 0x494e;
/// [A5C0h]: the locations shown on the map: 6-byte records (a non-zero word, x, y), ended by a 0 word; the cursor's hotspots, and the places whose troops `refresh_all_troops` draws. Cleared on leaving the hotspot mode.
pub const MAP_MARKERS: u16 = 0xa5c0;
/// [AA6Eh]: where `run_cs_script` reads next in the code-segment script (set from CS:[167h] + its first word, restarted there at an FFh).
pub const CS_SCRIPT_PTR: u16 = 0xaa6e;
/// [AA74h]: the segment of the CONDIT script, the high word of the far pointer [`CONDITIONS`].
pub const CONDITIONS_SEGMENT: u16 = 0xaa74;
/// [AB6Ah]: entry 7Ah of the phrase table at AA76h (words by number): the line `say_next_place_line` starts with when [47BAh] is 0 or FFFFh.
pub const PHRASE_7A: u16 = 0xab6a;
/// [AB84h]: entry 87h of the phrase table at AA76h: the phrase `say_phase_change_phrase` steps the dialogue on (mode 80h, character 10h).
pub const PHASE_CHANGE_PHRASE: u16 = 0xab84;
/// [CE66h]: non-zero while the head panel 1E6Eh is redrawn from the picture CD9Eh (by the driver) instead of copied from the second buffer (`redraw_head_panel`); 1 for the head's step down in `head_down_from_picture`.
pub const HEAD_PANEL_FROM_PICTURE: u16 = 0xce66;
/// [CE6Ah]: far pointer to the extended-memory page table ([39A9h] + 2 * B9h words; a slot per kept resource, its use stamp at +172h), allocated by `init_extended_memory_allocator`.
pub const EXT_MEMORY_TABLE: u16 = 0xce6a;
/// [CE6Ch]: the segment of [`EXT_MEMORY_TABLE`] (the far pointer's high word).
pub const EXT_MEMORY_TABLE_SEGMENT: u16 = 0xce6c;
/// [CE6Eh]: the resource `memory_func_qq` is storing in extended memory (FFFFh when it was dropped for want of room).
pub const EXT_MEMORY_RESOURCE: u16 = 0xce6e;
/// [CE70h]: resources with an index below it are kept in extended memory (B9h when there is some, else 0).
pub const EXT_MEMORY_RESOURCE_LIMIT: u16 = 0xce70;
/// [CE72h]: counted down by the timer interrupt; when it goes negative it restarts at 0Ah and the BIOS's own handler is chained (else just the PIC's EOI).
pub const BIOS_TICK_COUNTDOWN: u16 = 0xce72;
/// [CE73h]: FFh while the game's interrupt handlers are installed (flipped by `install_interrupt_handlers`, tested on shutdown).
pub const INTERRUPTS_INSTALLED: u16 = 0xce73;
/// [CE76h]: the current drive at start-up (INT 21h AH 19h), to go back to.
pub const STARTUP_DRIVE: u16 = 0xce76;
/// [CE77h]: the drive selected now (set at start-up and by `loc_0f29b`, INT 21h AH 0Eh).
pub const CURRENT_DRIVE: u16 = 0xce77;
/// [CE78h]: the resource being opened (`open_resource_by_index_si_into_esdi`), read by `read_and_maybe_hsq`.
pub const LOADING_RESOURCE: u16 = 0xce78;
/// [CE9Dh]: the Enter key's state (the keyboard table at CE81h by scan code, 1Ch): non-zero while down; acts as a mouse button.
pub const KEY_ENTER: u16 = 0xce9d;
/// [CE9Eh]: the Ctrl key's state (CE81h + scan code 1Dh): FFh while held (Ctrl-V; `KEYS_GLIDE` in cursor.rs).
pub const KEY_CTRL: u16 = 0xce9e;
/// [CEBAh]: the space bar's state (CE81h + scan code 39h): non-zero while down; acts as a mouse button.
pub const KEY_SPACE: u16 = 0xceba;
/// [CEE6h]: FFh while a joystick button is down (`joystick_as_keys`, in the key table past the scan codes); acts as a mouse button.
pub const JOYSTICK_BUTTON: u16 = 0xcee6;
/// [CEECh]: the font resource (BBh, C7h for language 6), loaded at start-up: the small font's width table here, the tall font's at CF6Ch; [`FONT_WIDTHS`] points at one of them.
pub const FONT_DATA: u16 = 0xceec;
/// [D810h]: entry 0Eh of the table at D7F4h (17 words: each phrase group's first number, from AA76h at start-up): added to a phrase number for head 0Eh (`next_phrase_a789`, the timed map phrases).
pub const PHRASE_GROUP_14_FIRST: u16 = 0xd810;
/// [D814h]: entry 10h of the table at D7F4h (each phrase group's first number): taken off the phrase number when [47DCh] is set (`load_voc_and_lipsync_data`).
pub const PHRASE_GROUP_16_FIRST: u16 = 0xd814;
/// [D818h]: a scratch word: the row offset (y) of a map span or circle walk (`span_row`, `start_move`, `circle_at`); also BX kept by `gfx_zero_block`, BP by `loc_02023`.
pub const SPAN_ROW: u16 = 0xd818;
/// [D81Ah]: a scratch word: the block size CX kept by `gfx_zero_block` for `loc_01556`.
pub const SCRATCH_COUNT: u16 = 0xd81a;
/// [D81Ch]: a scratch word: the x around which a map span or circle walk runs (`span_row`, `start_move`, `circle_at`); also a sum in `location_balance`.
pub const SPAN_CENTRE_X: u16 = 0xd81c;
/// [D820h]: the end of the resource lookup table being built by `open_dune_dat` (10 bytes a resource); its final size is allocated from it.
pub const RES_LOOKUP_TABLE_END: u16 = 0xd820;
/// [D836h]: the top (y0) of the rect at [`DIRTY_RECT`] (x0, y0, x1, y1: the area to redraw; C7h when empty).
pub const DIRTY_RECT_TOP: u16 = 0xd836;
/// [D838h]: the right edge (x1) of the rect at [`DIRTY_RECT`] (0 when empty).
pub const DIRTY_RECT_RIGHT: u16 = 0xd838;
/// [D83Ah]: the bottom (y1) of the rect at [`DIRTY_RECT`] (0 when empty; kept at most 98h for the dialogue frame, lowered to the voice's line).
pub const DIRTY_RECT_BOTTOM: u16 = 0xd83a;
/// [DBB2h]: the segment of the loaded sprite sheet, the high word of [`SPRITE_SHEET`] (moved down when the allocator frees space).
pub const SPRITE_SHEET_SEGMENT: u16 = 0xdbb2;
/// [DBB8h]: the segment of the music data, the high word of [`MUSIC_DATA`] (the EMS frame when the MIDI buffer is mapped there).
pub const MUSIC_DATA_SEGMENT: u16 = 0xdbb8;
/// [DBBEh]: the segment of the resource lookup table, the high word of [`RES_LOOKUP_TABLE`].
pub const RES_LOOKUP_TABLE_SEGMENT: u16 = 0xdbbe;
/// [DBE2h]: the second UI element list drawn over the dirty rect after [`SPRITE_LIST`] (0: none; the map 4710h, the panel rects 1940h).
pub const SPRITE_LIST_2: u16 = 0xdbe2;
/// [DBE5h]: the text's second colour, the high byte of [`TEXT_COLOUR`] (F3h for the button labels, swapped with FAh for a highlighted one).
pub const TEXT_BACK_COLOUR: u16 = 0xdbe5;
/// [DC28h]: the lip-sync table of the phrase loaded (`load_voc_and_lipsync_data`): the mouth frames' data, its word +2 the voice's line.
pub const LIPSYNC_TABLE: u16 = 0xdc28;
/// [DC2Ah]: the mouth frame shown (FFh after a load: none); `set_lipsync_data_to_al` skips the one already shown.
pub const LIPSYNC_FRAME: u16 = 0xdc2a;
/// [DC2Ch]: the high word of the 32-bit lip-sync time ([DC2Ch]:[DC2Eh]), the timer when the voice started; stepped by the lip-sync task.
pub const LIPSYNC_TIME: u16 = 0xdc2c;
/// [DC2Eh]: the low word (fraction) of the lip-sync time [`LIPSYNC_TIME`] (8000h at the start).
pub const LIPSYNC_TIME_LOW: u16 = 0xdc2e;
/// [DC30h]: the phrase to say next (taken by `next_phrase_a789`; 0: none); set for a person on the map, cleared with the next event.
pub const NEXT_PHRASE: u16 = 0xdc30;
/// [DC48h]: pointer to the keypad's moves (271Ch): 6-byte records (a byte, dx, dy) for the 13 keys whose states are at CEC8h.
pub const KEY_MOVES: u16 = 0xdc48;
/// [DC4Ah]: the timer's low byte at the cursor glide's last step (a step every 6 ticks or more).
pub const GLIDE_TIME: u16 = 0xdc4a;
/// [DC4Bh]: the cursor glide's steps left (0: not gliding; the main loop then reads the mouse).
pub const GLIDE_STEPS: u16 = 0xdc4b;
/// [DC4Ch]: where the cursor glides to (x, then y at DC4Eh).
pub const GLIDE_TARGET: u16 = 0xdc4c;
/// [DC4Eh]: the y of [`GLIDE_TARGET`].
pub const GLIDE_TARGET_Y: u16 = 0xdc4e;
/// [DC51h]: the keyboard glide's speed so far (x, then y at DC53h), cleared when no move key is down.
pub const GLIDE_SPEED: u16 = 0xdc51;
/// [DC53h]: the y of [`GLIDE_SPEED`].
pub const GLIDE_SPEED_Y: u16 = 0xdc53;
/// [DC55h]: the keyboard glide's fractions of a pixel (two bytes, x and y), cleared with the speed.
pub const GLIDE_FRACTIONS: u16 = 0xdc55;
/// [DC57h]: bit 0: Enter, space or the joystick button down at the last read; a new press adds to [`MOUSE_BUTTONS`].
pub const KEY_BUTTON_DOWN: u16 = 0xdc57;
/// [DC5Ch]: the UI element held down that repeats (bit 40h of its byte 9), its handler called again every 32h ticks; 0: none.
pub const REPEAT_ELEMENT: u16 = 0xdc5c;
/// [DC5Eh]: the timer when the element was last pressed (for the repeat of [`REPEAT_ELEMENT`]).
pub const PRESS_TIME: u16 = 0xdc5e;
/// [DC60h]: the UI element drawn pressed (bit 20h of byte 9), redrawn up on release (`loc_0d677`); 0: none.
pub const PRESSED_ELEMENT: u16 = 0xdc60;
/// [DC62h]: the cursor's x at the main loop's last pass (y at DC64h), for its movement.
pub const POINTER_PREV_X: u16 = 0xdc62;
/// [DC64h]: the y of [`POINTER_PREV_X`].
pub const POINTER_PREV_Y: u16 = 0xdc64;
/// [DCE4h]: the current UI list's scroll byte (from its header, signed; bit 80h set past the four slots): its sign picks the "more" mark's arrow (`item_place`).
pub const ITEM_SCROLL: u16 = 0xdce4;
/// [DCE9h]: the rect a button's label is drawn in (x0, y0, x1 E3h, y1 y0 + 7; `button_label`).
pub const LABEL_RECT: u16 = 0xdce9;
/// [DCEBh]: the top of [`LABEL_RECT`].
pub const LABEL_RECT_TOP: u16 = 0xdceb;
/// [DCEDh]: the right edge of [`LABEL_RECT`] (E3h).
pub const LABEL_RECT_RIGHT: u16 = 0xdced;
/// [DCEFh]: the bottom of [`LABEL_RECT`] (its top + 7).
pub const LABEL_RECT_BOTTOM: u16 = 0xdcef;
/// [DCF1h]: the timer / 64 at the gauges' last blink step (`blink_gauges` steps on a new value).
pub const GAUGE_BLINK_TICK: u16 = 0xdcf1;
/// [DCF2h]: the zoomed globe's width on screen (its height at DCF4h); rows narrower are centred in it.
pub const GLOBE_WIDTH: u16 = 0xdcf2;
/// [DCF4h]: the zoomed globe's height (rows) on screen.
pub const GLOBE_HEIGHT: u16 = 0xdcf4;
/// [DCF6h]: the globe's centre on screen (x, y at DCF8h; A0h, 4Ch while rotating).
pub const GLOBE_CENTRE_X: u16 = 0xdcf6;
/// [DCF8h]: the y of [`GLOBE_CENTRE_X`].
pub const GLOBE_CENTRE_Y: u16 = 0xdcf8;
/// [DD00h]: the segment of the globe's map image, the high word of [`MAP_GFX_DATA`].
pub const MAP_GFX_SEGMENT: u16 = 0xdd00;
/// [DD06h]: the rect of a box opening (`open_box`): x0, y0, x1, y1, then its colour byte at DD0Eh; grown step by step within the bounds at 2440h.
pub const OPEN_BOX_RECT: u16 = 0xdd06;
/// [DD08h]: the top of [`OPEN_BOX_RECT`].
pub const OPEN_BOX_TOP: u16 = 0xdd08;
/// [DD0Ah]: the right edge of [`OPEN_BOX_RECT`].
pub const OPEN_BOX_RIGHT: u16 = 0xdd0a;
/// [DD0Ch]: the bottom of [`OPEN_BOX_RECT`].
pub const OPEN_BOX_BOTTOM: u16 = 0xdd0c;
/// [DD0Eh]: the outline colour of [`OPEN_BOX_RECT`] (7).
pub const OPEN_BOX_COLOUR: u16 = 0xdd0e;
/// [DD17h]: the result bars' 6 heights, after their targets at [`RESULT_BARS`]; cleared (three words) when the results are drawn, stepped toward the targets.
pub const RESULT_BAR_HEIGHTS: u16 = 0xdd17;
/// [DD19h]: bars 2 and 3 of [`RESULT_BAR_HEIGHTS`] (cleared as a word).
pub const RESULT_BAR_HEIGHTS_2: u16 = 0xdd19;
/// [DD1Bh]: bars 4 and 5 of [`RESULT_BAR_HEIGHTS`] (cleared as a word).
pub const RESULT_BAR_HEIGHTS_4: u16 = 0xdd1b;
