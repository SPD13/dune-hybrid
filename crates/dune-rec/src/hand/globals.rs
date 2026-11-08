//! Data-segment globals (offsets in DS = 1F4Bh for the CD executable).
//! Names from the routines that use them, and from Cryogenic's
//! `GlobalsOnDs.cs` where it has one (noted "Cryogenic"). A name with only
//! an offset is not understood yet.

// --- Game state -----------------------------------------------------------

/// Game time (Cryogenic: `GameElapsedTime`): the low nibble is the time of
/// day (16 slots a day), bits 4..6 select the sky's palette group.
pub const GAME_TIME: u16 = 0x0002;
/// Not understood yet: `get_map_position` returns the words at 4 and 6 as
/// a position (DX, BX); `add_room_frame_task` checks byte 4 for 4 and byte
/// 5 below 20h; `get_orni_position` byte 5 against 20h.
pub const WORD_0004: u16 = 0x0004;
pub const BYTE_0005: u16 = 0x0005;
pub const WORD_0006: u16 = 0x0006;
/// The intro's current room character (0 or 2).
pub const INTRO_0012: u16 = 0x0012;
/// Sietches discovered.
pub const SIETCHES_FOUND: u16 = 0x0027;
pub const BYTE_0028: u16 = 0x0028;
/// The player's charisma (0..C8h); every 4 points raise the troops'
/// motivation.
pub const CHARISMA: u16 = 0x0029;
/// Game phase (story progress), raised by `set_game_phase`.
pub const GAME_PHASE: u16 = 0x002a;
/// Cleared when the game phase changes.
pub const PHASE_CHANGED_FLAG: u16 = 0x00ff;
/// Story bits.
pub const STORY_BITS_000A: u16 = 0x000a;
/// Talking-head animation step (counted down by `ui_head_animate_down`).
pub const HEAD_ANIMATION_STEP: u16 = 0x00e8;
pub const BYTE_0FE8: u16 = 0x0fe8;
pub const BYTE_1038: u16 = 0x1038;
pub const BYTE_1141: u16 = 0x1141;
pub const WORD_1156: u16 = 0x1156;
pub const BYTE_11D0: u16 = 0x11d0;
pub const COUNTER_122A: u16 = 0x122a;
pub const BYTE_0FD8: u16 = 0x0fd8;
pub const FLAG_0FDB: u16 = 0x0fdb;
pub const WORD_0FEA: u16 = 0x0fea;
pub const WORD_0FF8: u16 = 0x0ff8;
pub const WORD_0FFA: u16 = 0x0ffa;
pub const WORD_1018: u16 = 0x1018;
pub const BYTE_1057: u16 = 0x1057;
pub const WORD_1154: u16 = 0x1154;
pub const BYTE_1178: u16 = 0x1178;
pub const WORD_1201: u16 = 0x1201;
pub const FLAG_0FFB: u16 = 0x0ffb;
pub const BITS_1067: u16 = 0x1067;
/// The troops: 68 records of 1Bh bytes.
pub const TROOPS: u16 = 0x08aa;
pub const TROOPS_END: u16 = 0x0fbb;
/// Pointer to the record whose position `get_map_position` reports when
/// the map y is 80h (a marker).
pub const MAP_POSITION_RECORD: u16 = 0x114e;

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
pub const FLAG_28BE: u16 = 0x28be;
/// Pointer to the next scene of the intro script (Cryogenic:
/// `SceneSequenceOffset`).
pub const SCENE_SEQUENCE_OFFSET: u16 = 0x4854;
pub const FRAME_TASKS_ACTIVE: u16 = 0x46d7;
/// Toggled by `blink` (the cursor or text blink phase).
pub const BLINK_PHASE: u16 = 0x4775;
pub const FLAG_47A5: u16 = 0x47a5;
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
pub const BYTE_DBE6: u16 = 0xdbe6;
/// The resource the palette came from.
pub const PALETTE_RESOURCE: u16 = 0xdbb4;
/// MIDI driver function 5's BX (Cryogenic: `midiFunc5returnBx`).
pub const MIDI_FUNC5_BX: u16 = 0xdbce;
/// HNM playback: 0 or 1 while a video plays (`check_if_hnm_complete`).
pub const HNM_STATE: u16 = 0xdbe7;
/// HNM frame counters.
pub const HNM_FRAME: u16 = 0xdbe8;
pub const HNM_FRAME_2: u16 = 0xdbea;
pub const HNM_FRAME_3: u16 = 0xdbec;
pub const HNM_FRAME_4: u16 = 0xdbee;
/// HNM read pointers and buffers.
pub const HNM_READ_PTR: u16 = 0xdc0c;
pub const HNM_BUF_SEG: u16 = 0xdc0e;
pub const HNM_DC10: u16 = 0xdc10;
pub const HNM_BUF_SEG_2: u16 = 0xdc12;
pub const HNM_DC16: u16 = 0xdc16;
pub const HNM_DC18: u16 = 0xdc18;
pub const HNM_READ_PTR_2: u16 = 0xdc1a;
pub const HNM_DC20: u16 = 0xdc20;
/// Offset of the palette chunk being played.
pub const HNM_PAL_CHUNK: u16 = 0xdc1e;
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
/// Mouse buttons (low byte) and state, position.
pub const MOUSE_BUTTONS: u16 = 0xdc34;
pub const MOUSE_BUTTONS_SEEN: u16 = 0xdc35;
pub const MOUSE_X: u16 = 0xdc36;
pub const MOUSE_Y: u16 = 0xdc38;
pub const MOUSE_RECT: u16 = 0xdc58;

// --- Text ------------------------------------------------------------------

/// Far pointers to the string tables: commands, and the loaded phrases.
pub const COMMAND_STRINGS: u16 = 0x47ac;
pub const PHRASE_STRINGS: u16 = 0x47b0;
/// The loaded phrase file's last offset.
pub const PHRASE_END: u16 = 0x47b4;
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
/// The open video file's handle, 0 for none. (Cryogenic names it
/// `IsAnimateMenuUnneeded`.)
pub const HNM_FILE_HANDLE: u16 = 0x35a6;
/// The video's file offset and the bytes left, 32 bits each (Cryogenic:
/// `hnmFileOffset`, `hnmFileRemain`).
pub const HNM_FILE_OFFSET: u16 = 0xdc04;
pub const HNM_FILE_REMAIN: u16 = 0xdc08;

// --- The game screen ----------------------------------------------------------

pub const BYTE_00FB: u16 = 0x00fb;
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
/// Set when the map view is up (no lower palette rows).
pub const MAP_VIEW_FLAG: u16 = 0x227d;
/// Set by `set_sky_palette`.
pub const SKY_PALETTE_SET: u16 = 0x46df;
/// The talking head's zooms: the tick last seen, the start, the interval
/// (Cryogenic: `TimeBetweenFaceZooms`).
pub const HEAD_ZOOM_START: u16 = 0x476e;
pub const HEAD_ZOOM_SEEN: u16 = 0x4770;
pub const TIME_BETWEEN_FACE_ZOOMS: u16 = 0x4772;
/// A gradient section's start.
pub const GRADIENT_X: u16 = 0x47e8;
pub const GRADIENT_Y: u16 = 0x47ea;
/// The communication device's animation step.
pub const COMM_STEP: u16 = 0xd816;
pub const WORD_3CBE: u16 = 0x3cbe;
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
pub const BYTE_00EA: u16 = 0x00ea;
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
pub const HNM_FRAME_TICKS: u16 = 0xdbff;
pub const HNM_FLAGS: u16 = 0xdbfe;
/// Cryogenic: `hnmFlagMsb`.
pub const HNM_FLAG_MSB: u16 = 0xdbb5;
/// The voice buffer playing.
pub const VOICE_BUFFER_PLAYING: u16 = 0x3824;
pub const WORD_479E: u16 = 0x479e;
/// The end of the table `handle_ctrl_v_once` appends to (in the code
/// segment).
pub const CTRL_V_TABLE_END: u16 = 0x11bd;
