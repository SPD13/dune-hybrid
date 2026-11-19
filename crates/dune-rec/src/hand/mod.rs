//! Hand-written ports: readable Rust for routines of DNCDPRG.EXE, written
//! from the listing (`DNCDPRG.lst`) and checked against the original by
//! shadow mode exactly like the generated code (`dune-run --rec shadow`).
//! Where one exists it replaces the generated routine of the same entry.
//!
//! A port must leave the machine as the original does: the registers and
//! flags it returns (callers may rely on any of them, so the exit state is
//! spelled out), the memory it writes (stack slots below SP included), and
//! its instruction count, which is the game's virtual time ([`Ctx::run`]).
//! The meaning goes in names and comments; the bookkeeping stays in [`Ctx`].
//!
//! The usual shape: work out the path from the inputs first (pure reads),
//! then `run` the stretch's instruction count (which may bail before anything
//! changed), then do the work, with flags set by the same operations as the
//! original's. A stretch that bails must leave the machine untouched: the
//! interpreter takes over at its first instruction.
//!
//! Names of data-segment globals ([`globals`]) come from the routines that
//! use them and from Cryogenic's sources (`GlobalsOnDs.cs`); unknown ones
//! keep their offset.

use std::sync::atomic::{AtomicU32, Ordering};

use pc::{
    Machine,
    cpu::{AX, Bus, CS, DI, DS, ES, SI, SP, SS, Width},
    rec::{Bail, Routine},
};

mod audio;
pub mod clock;
pub mod native;
pub mod troop_table;
pub mod record_table;
pub mod text_box;
pub mod dialogue_text;
pub mod conversation;
pub mod map_view;
pub mod map_selection;
pub mod flight;
pub mod typed_message;
pub mod screen_buffers;
pub mod mouse_state;
pub mod sound_state;
pub mod video_state;
pub mod travel;
pub mod font_pen;
pub mod voice_buffers;
pub mod characters;
pub mod player;
pub mod smugglers;
pub mod progress;
pub mod topic_troop;
pub mod topic_place;
pub mod place_forces;
pub mod economy;
pub mod situation;
pub mod sequence;
pub mod phrase;
pub mod sentence;
pub mod flags_read;
mod cursor;
mod dialogue;
mod dos;
mod game;
mod draw;
mod files;
mod frame;
mod gfx;
mod globals;
mod globe;
mod hnm;
mod hnmload;
mod hsq;
mod input;
mod irq;
mod intro;
mod map;
mod mouse;
mod panels;
mod phases;
mod pure;
mod main_loop;
mod math;
mod records;
mod resources;
mod saves;
mod scenes;
mod screen;
mod screens;
mod script;
mod sound;
mod sprites;
mod story;
mod tasks;
mod text;
mod time;
mod troops;
mod tails;
mod ui;
mod video;
mod videos;
mod ticks;
mod voice;
mod waits;

/// The ports, by entry offset, with the listing's names.
/// Ports for block leaders that are no routine's entry (the interpreter
/// and `--rec on` resume there): they replace the generated code's resume
/// there. Not called, so never shadow-checked: whole-run state hashes
/// check them (`STATE_HASH=1`).
pub static RESUMES: &[(u16, Routine, &str)] = &[
    (0xd820, main_loop::main_loop_top, "game_loop's top"),
    // The clock's writers, after their calls (see `hand/clock.rs`).
    (0x0fe1, time::run_clock_ticks_at, "run_clock_ticks after start_clock"),
    (0x0ff6, time::run_clock_ticks_at, "run_clock_ticks after the pending step"),
    (0x1002, time::run_clock_ticks_at, "run_clock_ticks after the step"),
    (0xef7a, irq::pit_timer_tick, "pit_timer_callback's tick"),
    (0xb3f0, saves::load_save_cl_after_restore, "loc_0b3b0 after the restore"),
];

pub static ROUTINES: &[(u16, Routine, &str)] = &[
    // Stubs: a bare `ret`.
    (0x0f66, story::nothing, "sub_0f66"),
    (0x10b7, story::nothing, "sub_10b7"),
    (0x1dd3, story::nothing, "sub_1dd3"),
    (0xd917, story::nothing, "sub_d917"),
    // Story, intro, flags.
    (0x0945, story::intro_script_set_offset, "intro_script_set_offset"),
    (0x10b2, story::on_phase_20, "sub_10b2"),
    (0x1127, story::on_phase_38, "sub_1127"),
    (0x112d, story::on_phase_40, "sub_112d"),
    (0x121f, story::set_game_phase, "set_game_phase"),
    (0xa1d6, story::forbid_scripted_action, "sub_a1d6"),
    (0xa5aa, story::clear_settings_hover, "sub_a5aa"),
    (0x1011, story::on_phase_04, "on_phase_04"),
    (0x117b, story::on_phase_50, "on_phase_50"),
    (0x11b3, story::on_phase_5c, "on_phase_5c"),
    // The intro's steps.
    (0x0579, intro::clear_global_y_offset, "clear_global_y_offset"),
    (0x06aa, intro::intro_play_hnm_86_frames, "intro_play_hnm_86_frames"),
    (0x06bd, intro::intro_play_hnm_skippable, "intro_play_hnm_skippable"),
    (0x06fc, intro::intro_play_hnm_with_sky_fade, "intro_play_hnm_with_sky_fade"),
    (0x0704, intro::intro_play_hnm_with_frame_task, "intro_play_hnm_with_frame_task"),
    (0x070c, intro::intro_hnm_frame_task, "intro_hnm_frame_task"),
    (0x078d, intro::intro_talking_head_with_sound, "intro_talking_head_with_sound"),
    (0x0798, intro::intro_talking_head_play, "intro_talking_head_play"),
    (0x085d, intro::intro_play_desert_sky_animation, "intro_play_desert_sky_animation"),
    (0x0868, intro::intro_midnight, "intro_midnight"),
    (0x087b, intro::intro_desert_sky_slow, "intro_desert_sky_slow"),
    (0x0625, intro::play_virgin_hnm, "play_VIRGIN_HNM"),
    (0x0661, intro::play_cryo_hnm, "play_CRYO_OR_CRYO2_HNM"),
    (0x0684, intro::play_present_hnm, "play_PRESENT_HNM"),
    (0x0737, intro::intro_still_56, "intro_still_56"),
    (0x073c, intro::intro_still_57, "intro_still_57"),
    (0x0740, intro::intro_still_58_on_clear, "intro_still_58_on_clear"),
    (0x0747, intro::intro_still_59, "intro_still_59"),
    (0x0752, intro::intro_still_5b, "intro_still_5b"),
    (0x0756, intro::intro_still_5c, "intro_still_5c"),
    (0x075a, intro::intro_still_5d, "intro_still_5d"),
    (0x075e, intro::intro_still_5e, "intro_still_5e"),
    (0x077c, intro::intro_inside_sietch, "intro_inside_sietch"),
    (0x07e0, intro::intro_water_ripples_in_cave, "intro_water_ripples_in_cave"),
    (0x07fd, intro::intro_load_desert_sky_animation, "intro_load_desert_sky_animation"),
    (0x08b6, intro::intro_27_baron_guards_play, "intro_27_baron_guards_play"),
    (0x0972, intro::intro_palace_equipment_room, "intro_palace_equipment_room"),
    (0x098a, intro::intro_lady_jessica_1, "intro_lady_jessica_1"),
    (0x09a5, intro::intro_outside_of_palace, "intro_outside_of_palace"),
    // Sprite sheets, palettes.
    (0x0820, sprites::open_skyrs_palette, "open_skyrs_palette"),
    (0x3971, sprites::open_sky_or_skydn_palette_al_sub_bl, "open_sky_or_skydn_palette_al_sub_bl"),
    (0x3978, sprites::open_sky_palette_al_sub_bl_dsdx, "open_sky_palette_al_sub_bl_dsdx"),
    (0x3a6a, sprites::draw_ornis_loop, "draw_ornis_loop"),
    (0x3a73, sprites::draw_ornis, "draw_ornis"),
    (0xc21b, sprites::draw_icons_list_at_si, "draw_icons_list_at_si"),
    (0x6f78, story::increase_charisma_by_al, "increase_charisma_by_al"),
    // Phase handlers, location discovery.
    (0x1053, phases::on_phase_14, "on_phase_14"),
    (0x10b8, phases::on_phase_28, "on_phase_28"),
    (0x10be, phases::on_phase_2c, "on_phase_2c"),
    (0x1133, phases::on_phase_44, "on_phase_44"),
    (0x1139, phases::on_phase_48, "on_phase_48"),
    (0x1196, phases::on_phase_58, "on_phase_58"),
    (0x425b, phases::discover_location, "discover_location"),
    // Condition scripts.
    (0xa30b, script::read_condit_operand_into_ax, "read_condit_operand_into_ax"),
    (0xa334, script::evaluate_operator_bx_on_dx_and_ax, "evaluate_operator_bx_on_dx_and_ax"),
    // Frame tasks.
    (0x39e6, tasks::remove_room_frame_task, "remove_room_frame_task"),
    (0xda25, tasks::add_frame_task, "add_frame_task"),
    (0xda5f, tasks::remove_frame_task, "remove_frame_task"),
    (0x3a7c, tasks::add_room_frame_task, "add_room_frame_task"),
    // Game time.
    (0x1ac5, time::get_ingame_day_q, "get_ingame_day_q"),
    (0x1ad1, time::get_ingame_day, "get_ingame_day_in_ax"),
    (0x1ae0, time::get_ingame_time_of_day, "get_ingame_time_of_day"),
    (0x395c, time::sky_palette_for_game_time, "get_sky_palette_id_from_game_time_in_bl"),
    // Maths, registers.
    (0xb5a0, math::trig_lookup, "tablat_lookup_from_bx_to_ax_bp"),
    (0xb58b, globe::map_func, "map_func"),
    (0xdb44, math::shl_cx_and_dx, "shl_cx_and_dx"),
    (0xe16f, math::nearness_right, "sub_e16f"),
    (0xe18c, math::nearness_left, "sub_e18c"),
    (0xe192, math::nearness_up, "sub_e192"),
    (0xe19b, math::nearness_down, "sub_e19b"),
    (0xe1a2, math::nearness_up_right, "loc_0e1a2"),
    (0xe1b9, math::nearness_up_left, "loc_0e1b9"),
    (0xe1c3, math::nearness_down_left, "loc_0e1c3"),
    (0xe1cb, math::nearness_down_right, "loc_0e1cb"),
    (0xe270, math::save_regs, "save_regs"),
    (0xe283, math::restore_regs, "restore_regs"),
    (0xe3b7, math::rand_masked, "rand_masked"),
    (0xe3cc, math::rand, "rand"),
    // Records (locations, map objects).
    (0x01e0, records::place_troop_at_location, "place_troop_at_location"),
    (0x3a95, records::get_orni_position, "get_orni_position"),
    (0x407e, records::get_map_position, "get_map_position"),
    (0x40c9, records::move_following_character, "move_following_character"),
    (0x5082, records::count_troop_sides, "count_troop_sides"),
    (0x5e4f, records::calc_sal_index, "calc_SAL_index"),
    (0x6231, records::get_location_type_string_offset, "get_location_type_string_offset"),
    // Video, fonts, HNM playback, UI.
    (0x176b, video::blink, "blink"),
    (0xa453, video::settings_ui_sub_global_offset, "settings_ui_sub_global_offset"),
    (0xa45c, video::settings_ui_add_global_offset, "settings_ui_add_global_offset"),
    (0xc07c, video::set_fb1_as_active_framebuffer, "set_fb1_as_active_framebuffer"),
    (0xc085, video::set_backbuffer_as_frame_buffer, "set_backbuffer_as_frame_buffer"),
    (0xc08e, video::set_screen_as_active_framebuffer, "set_screen_as_active_framebuffer"),
    (0xc1f4, video::get_subresource_pointer, "get_subresource_ax_pointer_to_dssi"),
    (0xc921, video::get_hnm_resource_flag_name_ptr, "get_hnm_resource_flag_name_ptr_by_index_ax_to_bx"),
    (0xcc85, video::check_if_hnm_complete, "check_if_hnm_complete"),
    (0xcdf7, video::hnm_advance_read_ptr, "hnm_advance_read_ptr"),
    (0xce01, video::hnm_reset_frame_counter, "hnm_reset_frame_counter"),
    (0xce07, video::hnm_reset_counters, "hnm_reset_counters"),
    (0xce1a, video::hnm_reset_buffers, "hnm_reset_buffers"),
    (0xd04e, video::font_set_draw_position, "font_set_draw_position"),
    (0xd05f, video::font_get_draw_position, "font_get_draw_position"),
    (0xd068, video::font_select_tall_font, "font_select_tall_font"),
    (0xd075, video::font_select_small_font, "font_select_small_font"),
    (0xd694, video::ui_elements_for_cursor, "set_di_to_ui_elements_ptr_based_on_cursor_image"),
    (0xda53, video::clear_frame_tasks, "clear_frame_tasks"),
    (0xf2fc, video::strcpy_to_filename_buf, "strcpy_to_filename_buf"),
    (0x5b99, video::copy_rect_at_si_to_di, "copy_rect_at_si_to_di"),
    (0x5ba0, video::copy_game_area_rect_to_unknown_rect, "copy_game_area_rect_to_unknown_rect"),
    (0x5ba8, video::copy_game_area_rect_to_clip_rect, "copy_game_area_rect_to_clip_rect"),
    (0x99b2, video::swap_byte_arrays, "swap_byte_arrays"),
    // Graphics driver calls, sprites, HNM frames, UI drawing.
    (0x181e, gfx::ui_head_animate_down, "ui_head_animate_down"),
    (0x1803, gfx::head_down_from_picture, "ui_head_animate_down (1803)"),
    // Callbacks called through a register (no listing name).
    (0x8751, troops::hide_shown_troop, "loc_08751"),
    (0x7bed, troops::text_box_click, "loc_07bed"),
    (0x824d, map::marks_then_click, "loc_0824d"),
    (0x81ec, map::map_click_pick, "loc_081ec"),
    (0x6e82, troops::mark_idle_troop_elsewhere, "loc_06e82"),
    (0x6ea8, troops::mark_idle_troop, "loc_06ea8"),
    (0xa219, story::next_phase_unless_done, "loc_0a219"),
    (0xa235, story::next_phase_group_unless_done, "loc_0a235"),
    (0x316e, story::place_troop_marker, "loc_0316e"),
    (0xb977, gfx::map_func_gfx, "map_func_gfx"),
    (0xc097, gfx::gfx_call_bp_with_front_buffer_as_screen, "gfx_call_bp_with_front_buffer_as_screen"),
    (0xc0ad, gfx::gfx_clear_active_framebuffer, "gfx_clear_active_framebuffer"),
    (0xc0f4, gfx::update_screen_palette, "update_screen_palette"),
    (0xc2fd, gfx::draw_sprite, "draw_sprite"),
    (0xc305, gfx::draw_sprite_clipped, "draw_sprite_clipped"),
    (0xc412, gfx::copy_active_framebuffer_to_framebuffer_2, "copy_active_framebuffer_to_framebuffer_2"),
    (0xc432, gfx::clear_game_area, "clear_game_area"),
    (0xc49a, gfx::gfx_copy_framebuffer_to_screen, "gfx_copy_framebuffer_to_screen"),
    (0xc4cd, gfx::gfx_copy_whole_framebuf_to_screen, "gfx_copy_whole_framebuf_to_screen"),
    (0xc9e8, gfx::hnm_do_frame_skippable, "hnm_do_frame_skippable"),
    (0xc9f4, gfx::hnm_do_frame_and_check_if_frame_advanced, "hnm_do_frame_and_check_if_frame_advanced"),
    (0xcd8f, gfx::hnm_read_header_size, "hnm_read_header_size"),
    (0xce3b, gfx::hnm_handle_pal_chunk, "hnm_handle_pal_chunk"),
    (0xd19b, gfx::font_draw_phrase_or_command_string, "font_draw_phrase_or_command_string"),
    (0xd1f2, gfx::draw_ui_elements_list_at_ds_si, "draw_ui_elements_list_at_ds_si"),
    // Screen updates, clipped sprites, transitions.
    // Not in the table yet: never called in the checking runs.
    (0xc102, screen::copy_pal_and_transition, "copy_pal_and_transition"),
    (0xc108, screen::transition_al, "transition"),
    (0xc202, screen::center_sprite_coordinates, "center_sprite_coordinates"),
    (0xc30d, screen::draw_sprite_clipped_clobbering, "draw_sprite_clipped_clobbering_bx_dx"),
    (0xc32f, screen::draw_sprite_list, "draw_sprite_list"),
    (0xc43e, screen::copy_game_area_to_screen_fb2_to_fb1, "copy_game_area_to_screen_fb2_to_fb1"),
    (0xc443, screen::copy_clip_rect_to_screen_from_fb2, "copy_clip_rect_to_screen_from_fb2"),
    (0xc446, screen::copy_rect_fb2_to_fb1_at_si, "copy_rect_fb2_to_fb1"),
    (0xc474, screen::copy_game_rect_from_fb2_to_fb1, "copy_game_rect_from_fb2_to_fb1"),
    (0xc477, screen::gfx_copy_rect_fb1_to_fb2, "gfx_copy_rect_fb1_to_fb2"),
    (0xc4aa, screen::gfx_copy_rect_to_screen, "gfx_copy_rect_to_screen"),
    (0xc51e, screen::particles_update_screen_rect, "particles_update_screen_rect"),
    (0xd1ef, screen::draw_all_ui_elements, "draw_all_ui_elements"),
    // HSQ decompression.
    (0xf3d3, hsq::check_hsq_checksum, "check_hsq_checksum"),
    (0xf403, hsq::hsq_decomp_skip_header, "hsq_decomp_skip_header_dssi_to_esdi"),
    (0xf40d, hsq::hsq_decomp_no_header, "hsq_decomp_no_header"),
    // HNM files.
    (0xca01, hnm::hnm_close_resource, "hnm_close_resource"),
    (0xcdbf, hnm::hnm_read_bytes, "hnm_read_bytes"),
    // Text.
    (0xcf70, text::get_phrase_or_command_string_si, "get_phrase_or_command_string_si"),
    (0xcfa0, text::check_amr_or_eng_language, "check_amr_or_eng_language"),
    (0xd00f, text::load_phrase_hsq, "load_PHRASExx_HSQ"),
    (0xd16a, text::font_draw_glyph, "font_draw_glyph_func"),
    (0xd194, text::font_draw_string_with_color_at_pos, "font_draw_phrase_or_command_string_with_color_at_pos"),
    (0xd1a6, text::font_draw_phrase_list, "font_draw_phrase_list"),
    (0xd1bb, text::font_draw_string, "font_draw_string"),
    // Sound, timer.
    (0xac30, sound::call_pcm_audio_vtable_func_5, "call_pcm_audio_vtable_func_5"),
    (0xaeb7, sound::midi_func_2_0, "midi_func_2_0"),
    (0xa814, sound::advance_lipsync, "advance_lipsync"),
    (0xa9e7, sound::pcm_test_audio_done, "pcm_test_audio_done"),
    (0xaba3, sound::check_pcm_voice_file_open, "check_pcm_voice_file_open"),
    (0xabc6, sound::set_voc_pcm_is_not_playing, "set_voc_pcm_is_not_playing"),
    (0xabcc, sound::is_voc_pcm_playing, "is_voc_pcm_playing"),
    (0xae2f, sound::check_pcm_enabled, "check_pcm_enabled"),
    (0xe8a8, sound::set_pit_timer_frequency, "set_pit_timer_frequency"),
    // Services: EMS, XMS, DOS.
    (0xed40, dos::get_ems_emm_handle, "get_ems_emm_handle"),
    (0xed45, dos::call_ems_func, "call_ems_func"),
    (0xef22, dos::call_xms_driver_func, "call_xms_driver_func"),
    (0xef2b, dos::call_xms_func_on_block, "call_xms_func_on_block"),
    (0xf2d6, dos::seek_dune_dat_offset_dxax, "seek_dune_dat_offset_dxax"),
    (0xe56b, dos::parse_cmd_is_end_of_arg, "parse_cmd_is_end_of_arg"),
    (0xf3a7, dos::res_locate_in_lookup_table, "res_locate_in_lookup_table"),
    // Voice and music.
    (0xa9a1, audio::close_pcm_voice_file_handle, "close_pcm_voice_file_handle"),
    (0xab15, audio::audio_start_voc, "audio_start_voc"),
    (0xabdb, audio::play_pcm_al, "play_pcm_al"),
    (0xabe9, audio::open_voc_resource, "open_voc_resource"),
    (0xac14, audio::pcm_stop_voc, "pcm_stop_voc_q"),
    (0xad50, audio::play_music_wormsuit, "play_music_WORMSUIT_HSQ"),
    (0xad57, audio::play_music_morning, "play_music_MORNING_HSQ"),
    (0xad95, audio::play_music, "play_music"),
    (0xefba, audio::pit_timer_callback_midi, "pit_timer_callback_midi"),
    // Files, memory, keyboard buffer, waiting.
    (0xde68, files::kb_drain_and_clear, "kb_drain_and_clear"),
    (0xe3a0, files::wait_processing_frame_tasks, "wait_processing_frame_tasks"),
    (0xe57b, files::load_driver_ax_with_vtable_at_si, "load_driver_ax_with_vtable_at_si"),
    (0xe741, files::read_dune_dat_toc, "read_dune_dat_toc"),
    (0xec46, files::call_memory_func_2, "call_memory_func_2"),
    (0xec59, files::call_memory_func_1, "call_memory_func_1"),
    (0xf08e, files::clear_keyboard_array, "clear_keyboard_array"),
    (0xf11c, files::alloc_check_cx_pages_available, "alloc_check_cx_pages_available"),
    (0xf1fb, files::open_res_or_file, "open_res_or_file_with_name_in_dx_size_ax"),
    (0xf260, files::read_ffff_to_esdi_and_close, "read_ffff_to_esdi_and_close"),
    (0xf2a7, files::seek_dune_dat_to_res_dsdx, "seek_dune_dat_to_res_dsdx"),
    (0xf2ea, files::read_dune_dat_cx_to_esdi, "read_dune_dat_cx_to_esdi"),
    // The game screen's UI.
    (0x0098, ui::adjust_sub_resource_pointers, "adjust_sub_resource_pointers"),
    (0x093f, ui::intro_script_load_word, "intro_script_load_word"),
    (0x17e6, ui::ui_head_animate_up, "ui_head_animate_up"),
    (0x19fc, ui::close_people_panel, "close_people_panel"),
    (0x1a9b, ui::draw_moon_and_sun, "draw_moon_and_sun"),
    (0x1ae7, ui::talking_head_zoom_tick, "talking_head_zoom_tick"),
    (0x1dda, ui::note_shipment_event_if_due, "note_shipment_event_if_due"),
    (0x1dfe, ui::grow_harkonnen_troops, "grow_harkonnen_troops"),
    (0x274e, ui::comm_draw_panels, "comm_draw_panels"),
    (0x275f, ui::comm_draw_glow, "comm_draw_glow"),
    // Unverified (never called to completion in the checking runs):
    (0x2795, ui::comm_fade_in_glow, "comm_fade_in_glow"),
    // Unverified (never called to completion in the checking runs):
    (0x281c, ui::comm_animate_incoming_call, "comm_animate_incoming_call"),
    (0x2d74, ui::open_sal_resource, "open_SAL_resource"),
    (0x388d, ui::set_sky_palette, "set_sky_palette"),
    (0x38b4, ui::draw_sky, "draw_sky"),
    (0x39b9, ui::send_sky_palette_rows, "loc_039b9"),
    (0x3e13, ui::draw_gradient_section, "draw_gradient_section"),
    (0x4ad7, ui::travel_no_target_if_al_odd, "travel_no_target_if_al_odd"),
    (0x5a3d, ui::ui_transition_to_map_interface, "ui_transition_to_map_interface"),
    (0x629d, ui::draw_string_location_type, "draw_string_location_type"),
    (0x62a6, ui::draw_location_name, "draw_location_name"),
    (0x6f93, ui::decrease_troop_motivation, "decrease_troop_motivation"),
    (0xae04, ui::music_step_if_enabled, "loc_0ae04"),
    (0xd95b, ui::set_map_click_handler, "loc_0d95b"),
    // Resources, conditions.
    (0xa396, resources::evaluate_condition_ax, "evaluate_condition_ax"),
    (0xc13b, resources::open_onmap_resource, "open_onmap_resource"),
    (0xc13e, resources::open_resource_by_index, "open_resource_by_index"),
    // More of the game screen.
    (0x0b21, panels::stop_scene_voice_and_particles, "loc_00b21"),
    (0x1a34, panels::ui_draw_date_and_time_indicator, "ui_draw_date_and_time_indicator"),
    (0x2773, panels::comm_fade_out_glow, "comm_fade_out_glow"),
    (0x27b6, panels::comm_draw_glow_index, "comm_draw_glow_index"),
    (0x27c9, panels::comm_draw_flicker, "comm_draw_flicker"),
    (0x3aa9, panels::draw_orni, "draw_orni"),
    (0x8865, panels::font_draw_interpolated_string, "font_draw_interpolated_string_w_color_at_pos"),
    (0x9bee, panels::setup_non_lip_sync_data_structure, "setup_non_lip_sync_data_structure"),
    (0x9d2d, panels::draw_talking_head_at_si, "draw_talking_head_at_si"),
    (0xa42c, panels::settings_ui_draw_language_buttons, "settings_ui_draw_language_buttons"),
    (0xa4c6, panels::settings_ui_draw_volume_sliders, "settings_ui_draw_volume_sliders"),
    (0xa502, panels::settings_ui_draw_slider, "settings_ui_draw_slider"),
    (0xa7a5, panels::lip_sync_stop, "lip_sync_stop"),
    (0xa83f, panels::voc_get_lipsync_data, "voc_get_lipsync_data"),
    (0xaded, panels::set_pcm_volume, "loc_0aded"),
    (0xc0d5, panels::transition_front_buffer_to_screen, "loc_0c0d5"),
    (0xc85b, panels::schedule_head_zoom, "loc_0c85b"),
    // Voice files, music loading.
    (0xa8bc, voice::create_voc_file_name_from_bx, "create_voc_file_name_from_bx"),
    (0xa90b, voice::open_pcm_voice_file, "open_pcm_voice_file"),
    (0xa93f, voice::read_audio_file, "read_audio_file"),
    (0xa9b9, voice::refill_voice_buffer, "loc_0a9b9"),
    (0xae28, voice::music_enabled, "loc_0ae28"),
    (0xae62, voice::load_music, "load_music"),
    (0xaec6, voice::sound_off, "loc_0aec6"),
    // The globe, the frieze.
    (0xb84a, globe::globe_fill_blue_background, "globe_fill_blue_background_to_front_buffer"),
    (0xb85a, globe::draw_globe_with_atmosphere, "draw_globe_with_atmosphere"),
    (0xb87e, globe::draw_globe_side_decorations, "draw_globe_side_decorations"),
    (0xb8a7, globe::setup_globe_draw, "setup_globe_draw"),
    (0xb8f3, globe::globe_slide_decorations_open, "globe_slide_decorations_open"),
    (0xb915, globe::globe_slide_decorations_close, "globe_slide_decorations_close"),
    (0xb930, globe::remove_globe_frame_tasks, "remove_globe_frame_tasks"),
    (0xb93b, globe::remove_second_globe_task, "loc_0b93b"),
    (0xb9e0, globe::globe_rotation_increment_ax, "globe_rotation_increment_ax"),
    (0xb9f6, globe::recalculate_globe_rotation_table, "recalculate_globe_rotation_table"),
    (0xd75a, globe::frieze_sides_closed_book, "ui_set_and_draw_frieze_sides_closed_book"),
    (0xd792, globe::frieze_sides_map, "ui_set_and_draw_frieze_sides_map"),
    (0xd795, globe::ui_set_and_draw_frieze_sides, "ui_set_and_draw_frieze_sides"),
    (0xd7ad, globe::frieze_sides_open_book, "ui_set_and_draw_frieze_sides_open_book"),
    (0xd7b2, globe::frieze_sides_globe, "ui_set_and_draw_frieze_sides_globe"),
    // The mouse.
    (0xdae3, mouse::set_mouse_pos, "set_mouse_pos"),
    (0xdb14, mouse::define_mouse_range, "define_mouse_range"),
    (0xdb67, mouse::draw_mouse_cursor_if_needed, "draw_mouse_cursor_if_needed"),
    (0xdb74, mouse::restore_mouse_if_rect_intersects, "restore_mouse_if_rect_intersects"),
    (0xdbb2, mouse::call_restore_cursor, "call_restore_cursor"),
    (0xdbec, mouse::draw_mouse, "draw_mouse"),
    (0xdc20, mouse::redraw_mouse, "redraw_mouse"),
    (0xdc6a, mouse::get_mouse_cursor_image_addr, "get_mouse_cursor_image_addr"),
    // The frame.
    (0xd9d2, frame::process_frame_tasks, "process_frame_tasks"),
    (0xdd63, frame::stc_on_user_input, "stc_on_user_input"),
    (0xe353, frame::wait_processing_frame_tasks_interruptable, "wait_processing_frame_tasks_interruptable"),
    (0xcad4, frame::hnm_wait_for_frame, "hnm_wait_for_frame"),
    (0xca60, frame::hnm_do_frame, "hnm_do_frame"),
    (0xc1aa, frame::apply_sprite_sheet_palette, "apply_sprite_sheet_palette"),
    (0xc1ba, frame::apply_palette, "apply_palette"),
    (0xd03c, frame::find_last_numeric_digit, "find_last_numeric_digit_in_str_at_es_si"),
    (0xc827, frame::skip_zero_words, "skip_zero_words"),
    (0xc835, frame::lowest_record, "loc_0c835"),
    (0xb23f, frame::redraw_frieze_screen, "redraw_frieze_screen"),
    (0xb270, frame::handle_ctrl_v_once, "handle_ctrl_v_once"),
    // Drawing sprites, UI elements, screen updates, strings.
    (0xbfa7, draw::string_update_ordinal_suffix, "string_update_ordinal_suffix"),
    (0xc137, draw::load_icones_sprites, "load_icones_sprites"),
    (0xc22f, draw::draw_sprite_clobbering_bx_dx, "draw_sprite_clobbering_bx_dx"),
    (0xc4dd, draw::update_game_area_on_screen, "loc_0c4dd"),
    (0xc4ed, draw::update_screen_at_sprite_rect_updating_head, "update_screen_at_sprite_rect_updating_head"),
    (0xc4f0, draw::draw_head_if_needed_and_update_screen_rect_at_si, "draw_head_if_needed_and_update_screen_rect_at_si"),
    (0xc4fb, draw::draw_head_if_needed_and_update_screen_rect, "draw_head_if_needed_and_update_screen_rect"),
    (0xd200, draw::draw_ui_element, "draw_ui_element"),
    (0xe75b, draw::res_store_in_lookup_table, "res_store_in_lookup_table"),
    // Opening videos.
    (0x061c, hnmload::load_virgin_hnm, "load_VIRGIN_HNM"),
    (0x064d, hnmload::load_cryo_hnm, "load_CRYO_HNM"),
    (0x0658, hnmload::load_cryo2_hnm, "load_CRYO2_HNM"),
    (0x0678, hnmload::load_present_hnm, "load_PRESENT_HNM"),
    (0x069e, hnmload::load_intro_hnm, "load_INTRO_HNM"),
    (0x09ef, hnmload::play_credits_hnm, "play_CREDITS_HNM"),
    (0x0911, hnmload::remove_all_frame_tasks, "remove_all_frame_tasks"),
    (0xc92b, hnmload::hnm_open_and_load_palette, "hnm_open_and_load_palette"),
    (0xca1b, hnmload::hnm_load_first_frame, "hnm_load_first_frame"),
    (0xcda0, hnmload::hnm_open_at_body_start, "hnm_open_at_body_start"),
    // Scenes and screens.
    (0xb1af, scenes::ui_draw_book_turning_page, "ui_draw_book_turning_page"),
    (0xb1ee, scenes::play_chapter_video, "play_chapter_video"),
    // Waits on the timer.
    (0xe387, waits::wait_a_bit, "wait_a_bit"),
    (0xddb0, waits::wait_interruptable, "wait_interruptable"),
    (0xddf0, waits::wait_for_pcm_voice, "wait_for_pcm_voice_interruptable"),
    (0xa87e, waits::audio_test_frequency, "audio_test_frequency"),
    // The program's frame and the timer interrupt.
    (0x0000, main_loop::start, "start"),
    (0x0580, main_loop::play_intro, "play_intro"),
    (0xe4ad, main_loop::parse_command_line, "parse_command_line"),
    (0xd815, main_loop::game_loop, "game_loop"),
    (0xef6a, irq::pit_timer_callback, "pit_timer_callback"),
    (0xcec9, irq::hnm_render_timer_callback, "hnm_render_timer_callback"),
    (0xe85c, waits::measure_pit_speed, "loc_0e85c"),
    (0x72a2, troops::tally_job4_troop, "loc_072a2"),
    (0x2566, videos::ship_spice_to_emperor, "loc_02566"),
    // Unverified (no run reaches them; the harness's calls quit the game):
    // (0x1566, videos::cinematic_fade_loop, "loc_01566"),
    // (0x491c, videos::play_night_video, "loc_0491c"),
    (0xb236, scenes::load_chapter_video, "load_chapter_video"),
    (0xbdfa, scenes::ui_stats_draw_ingame_day_and_charisma, "ui_stats_draw_ingame_day_and_charisma"),
    (0xbe57, scenes::animate_result_bars, "animate_result_bars"),
    (0xbfe3, scenes::map_func, "map_func"),
    (0xc0b6, scenes::room_frame_task, "room_frame_task"),
    (0xcefc, scenes::load_irul_hsq, "load_IRULn_HSQ"),
    (0xcf1b, scenes::play_irul_hsq, "play_IRULx_HSQ"),
    (0xd096, scenes::tall_font_glyph, "sub_d096"),
    (0xd12f, scenes::small_font_glyph, "font_draw_glyph_func_small"),
    (0xde7b, scenes::pause_if_p_key_pressed, "pause_if_p_key_pressed"),
    // Saves, particles, numbers, reading resources.
    (0xb473, saves::restore_from_save_memory, "restore_from_save_memory"),
    (0xb4bb, saves::decompress_sav, "decompress_sav"),
    (0xc58a, saves::particles_remove_particle, "particles_remove_particle"),
    (0xc60b, saves::particles_spawn_particle, "particles_spawn_particle"),
    (0xe2db, saves::find_string_and_replace_digits, "find_string_and_replace_digits"),
    (0xe2e3, saves::string_replace_number_ending_at_es_si, "string_replace_number_ending_at_es_si"),
    (0xf244, saves::read_resource_to_esdi, "read_resource_to_esdi"),
    // Input.
    (0xdaa3, input::clear_some_mouse_rect, "clear_some_mouse_rect"),
    (0xdaaa, input::set_some_mouse_rect, "set_some_mouse_rect"),
    (0xdb4c, input::mouse_stuff, "mouse_stuff"),
    (0xdd5a, input::get_and_reset_key_scancode, "get_and_reset_key_scancode"),
    (0xde4e, input::kb_clear_scancode, "kb_clear_scancode"),
    (0xde54, input::kb_check_for_esc_key_hit, "kb_check_for_esc_key_hit"),
    // The keyboard cursor: the nearest hotspot, accelerated steps.
    (0xd41b, cursor::current_ui_elements, "loc_0d41b"),
    (0xd6fe, cursor::point_in_rect, "loc_0d6fe"),
    (0xe159, cursor::rect_hotspot, "loc_0e159"),
    (0xe068, cursor::hotspots_points, "loc_0e068"),
    (0xe0a2, cursor::hotspots_particles, "loc_0e0a2"),
    (0xe0db, cursor::hotspots_markers, "loc_0e0db"),
    (0xe11c, cursor::hotspots_eight_points, "loc_0e11c"),
    (0xe243, cursor::accelerated_step, "loc_0e243"),
    // Numbers, random, empty routines.
    (0xe31c, saves::string_replace_5_digits_ending_at_es_si, "loc_0e31c"),
    (0xe295, saves::draw_number_al, "loc_0e295"),
    (0xe290, saves::draw_number_at, "loc_0e290"),
    (0xe3df, math::rand_below, "loc_0e3df"),
    (0x4e8d, story::nothing, "loc_04e8d"),
    (0xe26f, story::nothing, "loc_0e26f"),
    // Graphics, video bookkeeping, preloading.
    (0xc0e8, gfx::gfx_call_30, "loc_0c0e8"),
    (0xc343, gfx::draw_sprite_centred, "loc_0c343"),
    (0xc53e, gfx::draw_line, "loc_0c53e"),
    (0xc560, gfx::draw_rect_outline, "draw_rect_outline"),
    (0xc551, gfx::draw_rect_outline_at_si, "loc_0c551"),
    (0xc2f2, gfx::open_resource_and_draw_sprite_0, "loc_0c2f2"),
    (0xc46f, screen::copy_rect_backbuffer_to_fb1, "loc_0c46f"),
    (0xc613, saves::particles_add_particle, "loc_0c613"),
    (0xca59, hnm::hnm_reset_frame_tick, "loc_0ca59"),
    (0xca8f, hnm::hnm_prefetch_flagged, "loc_0ca8f"),
    (0xca9a, hnm::hnm_clear_flag_msb, "loc_0ca9a"),
    (0xcaa0, hnm::hnm_decode_frame_if_read, "loc_0caa0"),
    (0xcc0c, hnm::hnm_advance_chunk, "loc_0cc0c"),
    (0xcc2b, hnm::hnm_ring_has_room, "loc_0cc2b"),
    (0xcc4e, hnm::hnm_consume_chunk, "loc_0cc4e"),
    (0xce4b, hnm::hnm_note_video_and_frame, "loc_0ce4b"),
    (0xceb0, hnmload::hnm_open_noting_body, "loc_0ceb0"),
    (0xce53, resources::preload_resources_63_to_68, "loc_0ce53"),
    // The settings screen.
    (0xa44c, panels::settings_ui_draw_second_buttons, "loc_0a44c"),
    (0xa465, panels::settings_ui_button_position, "loc_0a465"),
    (0xa685, panels::settings_on_slider, "loc_0a685"),
    (0xa6b2, panels::settings_on_switch, "loc_0a6b2"),
    (0xa672, panels::settings_on_a_slider, "loc_0a672"),
    (0xa69f, panels::settings_on_a_switch, "loc_0a69f"),
    (0xa594, panels::settings_hover, "loc_0a594"),
    (0xa637, panels::settings_apply_music_volume, "loc_0a637"),
    (0xa650, panels::settings_apply_sound_volume, "loc_0a650"),
    (0xade0, panels::settings_apply_voice_volume, "loc_0ade0"),
    // The clock, the globe, the map, flags.
    (0xb2b9, time::stop_clock, "loc_0b2b9"),
    (0xb2b3, time::release_clock, "loc_0b2b3"),
    (0xb2be, time::start_clock, "loc_0b2be"),
    (0xa8b1, voice::hex_digit, "loc_0a8b1"),
    (0xbc4e, globe::on_globe, "loc_0bc4e"),
    (0xb532, globe::map_byte_at, "loc_0b532"),
    (0xb5c5, globe::map_cell_scaled, "loc_0b5c5"),
    (0xa1c4, story::allow_scripted_action, "loc_0a1c4"),
    (0xa1e2, story::is_scripted_action_allowed, "loc_0a1e2"),
    (0xa1e8, story::mark_last_entry, "loc_0a1e8"),
    // Game state: flags, phases, records, locations.
    (0x100b, story::on_phase_01, "loc_0100b"),
    (0x101b, story::set_0fe8_to_9, "loc_0101b"),
    (0x1bb2, story::clear_redraw, "loc_01bb2"),
    (0x215f, story::set_46d9_to_7, "loc_0215f"),
    (0x4ab8, story::set_flying, "loc_04ab8"),
    (0x4ac4, story::release_travel, "loc_04ac4"),
    (0x4aca, story::hold_travel, "loc_04aca"),
    (0x50be, story::stop_keeping_heading, "loc_050be"),
    (0x1392, story::select_scene_ui_list, "loc_01392"),
    (0x1399, story::select_talk_ui_list, "loc_01399"),
    (0x9901, story::hide_text_box, "loc_09901"),
    (0x11cb, story::go_to_phase_60, "loc_011cb"),
    (0x1243, story::sum_three_records, "loc_01243"),
    (0x1258, story::for_three_records, "loc_01258"),
    (0x127c, story::is_4_in_phases_15_to_1f, "loc_0127c"),
    (0x1d9f, story::check_character_7_attack, "loc_01d9f"),
    (0x1ebe, story::phase_64_for_record, "loc_01ebe"),
    (0x23d5, story::haggle_smuggler_price, "loc_023d5"),
    (0x2524, story::pay_spice, "loc_02524"),
    (0x3310, story::skill_level_phrase, "loc_03310"),
    (0x33be, story::measure_place_forces, "loc_033be"),
    (0x366f, story::pick_one_of_two_speakers, "loc_0366f"),
    (0x36d3, story::handle_flag_23, "loc_036d3"),
    (0x390a, story::finish_sky_fade, "loc_0390a"),
    (0x395f, story::sky_palette_for_time, "loc_0395f"),
    (0x3ae9, story::clear_speaker_places, "loc_03ae9"),
    (0x41cc, story::note_21fd_al, "loc_041cc"),
    (0x41c5, story::reset_desert_countdown, "loc_041c5"),
    (0x469b, story::stop_typing_message, "loc_0469b"),
    (0x5119, story::turn_heading_by_al, "loc_05119"),
    (0x4965, story::fly_to_location, "loc_04965"),
    (0x4b3b, story::advance_flight_step, "loc_04b3b"),
    (0x4ded, story::note_place_troops, "loc_04ded"),
    (0x409a, records::find_location_by_word_6, "loc_0409a"),
    (0x40ae, records::location_coords, "loc_040ae"),
    (0x40e6, records::update_flagged_record, "loc_040e6"),
    (0x3efe, records::list_entry, "loc_03efe"),
    (0x2e98, records::note_point, "loc_02e98"),
    (0x33ad, records::mark_record_bit, "loc_033ad"),
    (0x097e, intro::intro_show_place, "loc_0097e"),
    (0x0978, intro::intro_show_place_empty, "loc_00978"),
    (0x0a16, intro::video_step_keep_framebuffer, "loc_00a16"),
    (0x4937, intro::sound_at_frame_11, "loc_04937"),
    (0x105b, intro::for_each_in_code_list, "loc_0105b"),
    (0x1834, gfx::gfx_call_25, "loc_01834"),
    (0x4b16, gfx::gfx_call_18, "loc_04b16"),
    (0x49ea, gfx::clear_points_ring, "loc_049ea"),
    (0x4a00, gfx::add_point_to_ring, "loc_04a00"),
    (0x0cf2, math::unpack_unsigned, "loc_00cf2"),
    // The map screen.
    (0x557b, map::place_box_origin, "loc_0557b"),
    (0x58e4, map::recolour_map_regions, "loc_058e4"),
    (0x5982, map::toggle_drag_frame, "loc_05982"),
    (0x5ad9, map::mouse_rect_view, "loc_05ad9"),
    (0x5ad3, map::map_click_handler_and_rect, "loc_05ad3"),
    (0x5b10, map::redraw_and_reclip_mouse, "loc_05b10"),
    (0x5b60, map::set_mark_dx_bx, "loc_05b60"),
    (0x5b55, map::set_mark_at_record, "loc_05b55"),
    (0x5b5d, map::set_mark_at_map_position, "loc_05b5d"),
    (0x5b96, map::copy_view_to_di, "loc_05b96"),
    (0x5b93, map::copy_view_to_clip_rect, "loc_05b93"),
    (0x5b8d, map::copy_view_to_both_rects, "loc_05b8d"),
    (0x5beb, map::drop_map_sprite_list, "loc_05beb"),
    (0x5d1d, map::in_view, "loc_05d1d"),
    (0x62d6, map::map_point_outside_view, "loc_062d6"),
    (0x5d36, map::location_kind_check, "loc_05d36"),
    (0x5ed0, map::find_marker, "loc_05ed0"),
    (0x6144, map::note_battle_balance, "loc_06144"),
    (0x633b, map::scan_up_from_mark, "loc_0633b"),
    (0x6515, map::start_move, "loc_06515"),
    // Troops, location bits.
    (0x6906, troops::troop_ptr, "loc_06906"),
    (0x693b, troops::troop_activity, "loc_0693b"),
    (0x66b1, troops::reset_troop, "loc_066b1"),
    (0x6757, troops::spawn_troop_particle_if_visible, "loc_06757"),
    (0x69a3, troops::remove_troop_marker_particle, "loc_069a3"),
    (0x6b25, troops::stamp_record, "loc_06b25"),
    (0x6e02, troops::lose_all_harkonnen_troops, "loc_06e02"),
    (0x6ebf, troops::for_troop_location, "loc_06ebf"),
    (0x6f48, troops::add_motivation, "loc_06f48"),
    (0x7085, troops::mark_troop_10, "loc_07085"),
    (0x719c, troops::mark_troop_200_10, "loc_0719c"),
    (0x71b2, troops::call_29f0_f, "loc_071b2"),
    (0x765e, troops::add_location_sum, "loc_0765e"),
    (0x7efb, troops::spread_location_bits, "loc_07efb"),
    (0x7f11, troops::pack_location_bits, "loc_07f11"),
    (0x7f5f, troops::count_location_bits, "loc_07f5f"),
    (0x7b2b, troops::gfx_call_30_unless_redrawing, "loc_07b2b"),
    (0x7b36, troops::redraw_map_and_panel, "loc_07b36"),
    // More of the map screen, text, dialogue.
    (0x80c8, map::clear_picked_locations, "loc_080c8"),
    (0x81d7, map::add_point_entry, "loc_081d7"),
    (0x82a0, map::map_list_without_40, "loc_082a0"),
    (0x848f, map::drop_reached_waypoints, "loc_0848f"),
    (0x8770, map::refresh_map_panel_buttons, "loc_08770"),
    (0x8850, map::redraw_and_handle, "loc_08850"),
    (0x8ac3, map::set_sentence_voice_part, "loc_08ac3"),
    (0x8fd1, map::draw_text_box_frame, "loc_08fd1"),
    (0x9025, map::draw_text_sheet, "loc_09025"),
    (0x88e1, text::text_8b11_unless_command, "loc_088e1"),
    (0x8a23, text::decimal_digits, "loc_08a23"),
    (0x96b5, dialogue::say_phase_change_phrase, "loc_096b5"),
    (0x96d8, dialogue::talk_as, "loc_096d8"),
    (0x970b, dialogue::say_phrase, "loc_0970b"),
    (0x98f5, dialogue::clear_text_state, "loc_098f5"),
    (0x9945, dialogue::add_two_heads_text_task, "loc_09945"),
    (0x9985, dialogue::finish_two_heads_text, "loc_09985"),
    (0x9ef1, dialogue::start_text_voice_talking, "loc_09ef1"),
    (0x9f1c, dialogue::start_random_sentence, "loc_09f1c"),
    (0x9f82, dialogue::tall_font_colour_f0, "loc_09f82"),
    (0x9556, dialogue::release_record, "loc_09556"),
    (0x956d, dialogue::stamp_record_time, "loc_0956d"),
    (0xb150, dialogue::scan_dialogues_from, "loc_0b150"),
    (0xb147, dialogue::scan_dialogues_a8, "loc_0b147"),
    (0xaf00, dialogue::mark_dialogue_buttons, "loc_0af00"),
    (0xb126, dialogue::append_phrase, "loc_0b126"),
    (0xa9f4, voice::swap_voice_buffer, "loc_0a9f4"),
    (0xabd5, voice::while_voice_playing, "loc_0abd5"),
    (0xad43, voice::play_requested_music, "loc_0ad43"),
    (0xadbe, voice::fade_out_music, "loc_0adbe"),
    (0xacbf, voice::shuffle_music_playlist, "loc_0acbf"),
    (0xb17a, story::on_phase_change, "loc_0b17a"),
    (0xb254, story::find_chapter_video, "loc_0b254"),
    (0xac3a, panels::settings_music_buttons, "loc_0ac3a"),
    // The map's projection, map rows, save labels.
    (0xb69a, globe::swap_projection, "loc_0b69a"),
    (0xb647, globe::map_to_screen, "loc_0b647"),
    (0xb5f9, globe::screen_to_map, "loc_0b5f9"),
    (0xb683, globe::globe_row_span, "loc_0b683"),
    (0xb5cf, globe::step_globe_position, "loc_0b5cf"),
    (0xb53b, globe::read_map_row, "loc_0b53b"),
    (0xb56c, globe::read_map_rows, "loc_0b56c"),
    (0xb2cd, saves::save_slot_label_ax, "loc_0b2cd"),
    (0xb2c4, saves::save_slot_label_si, "loc_0b2c4"),
    (0xbf73, scenes::stat_3_digits, "loc_0bf73"),
    (0xbf61, scenes::stat_5_digits, "loc_0bf61"),
    (0xbd00, scenes::transition_bordered_rect, "loc_0bd00"),
    (0xaba9, voice::wait_voice_done, "loc_0aba9"),
    (0xb02c, voice::voice_2_and_transition, "loc_0b02c"),
    (0xb8ea, globe::start_globe_drag_task, "loc_0b8ea"),
    // The stack of UI screens, the cursor's sprite, fonts.
    (0xd2fd, screens::pop_screen, "loc_0d2fd"),
    (0xd2ea, screens::leave_screen, "loc_0d2ea"),
    (0xd2bd, screens::leave_screens, "loc_0d2bd"),
    (0xd316, screens::ask_redraw, "loc_0d316"),
    (0xd454, screens::item_place, "loc_0d454"),
    (0xd617, screens::cursor_sprite_90, "loc_0d617"),
    (0xd61d, screens::cursor_sprite_9f, "loc_0d61d"),
    (0xd64e, screens::highlight_at_origin, "loc_0d64e"),
    (0xd65a, screens::draw_element_pressed, "loc_0d65a"),
    (0xd741, screens::draw_panel_button_frame, "loc_0d741"),
    (0xd95e, screens::set_click_handler, "loc_0d95e"),
    (0xd0e3, screens::special_char, "loc_0d0e3"),
    (0xd082, video::font_select_tall_font_with_sprites, "loc_0d082"),
    (0xd08f, video::font_use_plain_glyphs, "loc_0d08f"),
    (0xdb03, screens::move_cursor, "loc_0db03"),
    (0xdf07, screens::draw_1b48_and_cursor, "loc_0df07"),
    (0xd72b, screens::set_buttons_si, "loc_0d72b"),
    (0xd717, screens::set_buttons_for_screen, "loc_0d717"),
    (0xd763, screens::draw_gauges, "loc_0d763"),
    (0xd6b7, screens::element_under_cursor, "loc_0d6b7"),
    (0xdd10, input::joystick_as_keys, "loc_0dd10"),
    (0xc5cf, saves::particles_spawn_variant, "loc_0c5cf"),
    (0xc661, saves::particles_move_particle, "loc_0c661"),
    (0xc7d4, screen::copy_rect_clipped, "loc_0c7d4"),
    (0xce6c, hnmload::hnm_initialize_memory_handler, "hnm_initialize_memory_handler"),
    (0x0cea, math::unpack_signed, "loc_00cea"),
    (0x0fd9, time::run_clock_ticks, "loc_00fd9"),
    (0x19df, panels::draw_marks, "loc_019df"),
    (0x1a0f, panels::redraw_date_panel, "loc_01a0f"),
    (0x15a8, panels::draw_script_caption, "loc_015a8"),
    (0x18ba, panels::leave_place, "loc_018ba"),
    (0x0d0d, panels::scene_picture, "loc_00d0d"),
    (0x157e, gfx::gfx_zero_block, "loc_0157e"),
    (0x1e24, troops::location_has_troop_400, "loc_01e24"),
    (0x1e01, troops::character_at_flagged_troop_location, "loc_01e01"),
    (0x1eda, troops::attack_clock_tick, "loc_01eda"),
    (0x221d, troops::troop_to_place, "loc_0221d"),
    (0x1d66, troops::refresh_records_fd8, "loc_01d66"),
    (0x1cda, troops::estimate_field_spice, "loc_01cda"),
    (0x21fa, troops::random_2_to_12, "loc_021fa"),
    (0x1bec, story::revisit_harkonnen_place, "loc_01bec"),
    (0x1c18, story::refresh_map_overlays, "loc_01c18"),
    (0x235f, story::select_smuggler, "loc_0235f"),
    (0x2318, story::visit_smuggler, "loc_02318"),
    (0x23e6, story::buy_from_smuggler_on_credit, "loc_023e6"),
    (0x24d2, story::grade_be, "loc_024d2"),
    (0x272f, story::drop_oldest_message, "loc_0272f"),
    (0x2a34, story::drop_oldest_event, "loc_02a34"),
    (0x28e1, story::close_message, "loc_028e1"),
    (0x2b00, story::speak_event_line, "loc_02b00"),
    (0x2a7f, story::drop_told_events, "loc_02a7f"),
    (0x2aaf, story::find_pending_event, "loc_02aaf"),
    (0x3127, story::reset_visitor_positions, "loc_03127"),
    (0x3093, story::setup_3093_entry, "loc_03093"),
    (0x3090, story::reset_dialogue_then_set_up_place, "loc_03090"),
    (0x301a, screens::set_mode_buttons, "loc_0301a"),
    (0x398c, ui::send_sky_palette_rows_default_driver, "loc_0398c"),
    (0x38a2, ui::sky_palette_with_tasks, "loc_038a2"),
    (0x3385, story::characters_at_place, "loc_03385"),
    (0x33d9, story::strike_odds, "loc_033d9"),
    (0x348a, story::troop_strength, "loc_0348a"),
    (0x34a5, story::count_place_troops, "loc_034a5"),
    (0x3df4, story::put_in_ring, "loc_03df4"),
    (0x37ad, gfx::draw_resource_22, "loc_037ad"),
    (0x36ee, story::for_records_here_bp, "loc_036ee"),
    (0x40c3, story::move_party_here, "loc_040c3"),
    (0x4944, story::set_up_flight, "loc_04944"),
    (0x5124, story::head_for_location, "loc_05124"),
    (0x456c, story::place_phrase, "loc_0456c"),
    (0x4658, story::start_message_57, "loc_04658"),
    (0x368b, story::show_speaker_screen, "loc_0368b"),
    (0x3901, screens::start_sky_fade_task, "loc_03901"),
    (0x4b2b, screens::cursor_over_panel, "loc_04b2b"),
    (0x6603, troops::for_each_troop, "loc_06603"),
    (0x5098, troops::count_location_troop_sides, "loc_05098"),
    (0x661d, troops::for_each_troop_below_80, "loc_0661d"),
    (0x668f, troops::start_troop, "loc_0668f"),
    (0x503c, story::enter_location, "loc_0503c"),
    (0x4415, story::leave_hotspot_mode, "sub_4415"),
    (0x46b5, story::message_task, "sub_46b5"),
    (0x4a1a, gfx::draw_last_ring_point, "loc_04a1a"),
    (0x4285, gfx::load_intro_script, "loc_04285"),
    (0x49a0, map::zoomed_map_redraw_handler, "loc_049a0"),
    (0x4988, map::open_zoomed_map, "loc_04988"),
    (0x5198, map::direction_step, "loc_05198"),
    (0x5323, map::place_octant, "loc_05323"),
    (0x5344, map::nearest_location, "loc_05344"),
    (0x51cb, map::heading_for_move, "loc_051cb"),
    (0x578b, map::toggle_box_cell_highlight, "loc_0578b"),
    (0x58fa, map::drop_point_list, "loc_058fa"),
    (0x5b6e, map::frame_rect_si, "loc_05b6e"),
    (0x5b69, map::frame_view, "loc_05b69"),
    (0x5bb0, map::draw_map_caption, "loc_05bb0"),
    (0x5d50, map::count_shown_location, "loc_05d50"),
    (0x5e42, map::location_icon, "loc_05e42"),
    (0x5f91, map::drop_map_marks, "loc_05f91"),
    (0x62c9, map::record_outside_view, "loc_062c9"),
    (0x627e, map::location_to_show, "loc_0627e"),
    (0x6252, map::location_map_kind, "loc_06252"),
    (0x634d, map::scan_row_both_ways, "loc_0634d"),
    (0x642e, map::count_10_bytes, "loc_0642e"),
    (0x68eb, troops::shown_troop, "loc_068eb"),
    (0x6917, troops::troop_particle, "loc_06917"),
    (0x6946, troops::particle_at, "loc_06946"),
    (0x6cfc, troops::grow_location, "loc_06cfc"),
    (0x6edd, troops::add_skill, "loc_06edd"),
    (0x6f56, troops::add_motivation_to_all, "loc_06f56"),
    (0x68d2, troops::refresh_flagged_troops, "loc_068d2"),
    (0x6efd, troops::troop_motivation, "loc_06efd"),
    (0x6fb0, troops::lose_charisma, "loc_06fb0"),
    (0x6b8a, troops::troop_bit_10_by_location, "loc_06b8a"),
    (0x6b96, troops::troop_bit_10_by_state, "loc_06b96"),
    (0x697c, troops::mark_troop_location, "loc_0697c"),
    (0x758d, troops::scaled_by_troop, "loc_0758d"),
    (0x7ee2, troops::find_range, "loc_07ee2"),
    (0x7f75, troops::lower_by_bits, "loc_07f75"),
    (0x846c, troops::troop_location_dirty, "loc_0846c"),
    (0x80ac, map::map_mode_flag, "loc_080ac"),
    (0x82b7, map::redraw_map_marks, "loc_082b7"),
    (0x7bbe, map::show_message_0f, "loc_07bbe"),
    (0x8347, story::shift_followed_troop_note, "loc_08347"),
    (0x9702, dialogue::say_sentence, "loc_09702"),
    (0x994f, dialogue::sentence_offset, "loc_0994f"),
    (0x996c, dialogue::skip_32_strings, "loc_0996c"),
    (0x9a60, dialogue::skip_strings, "loc_09a60"),
    (0x908c, dialogue::blit_dialogue_frame, "loc_0908c"),
    (0x727d, troops::clear_location_4, "loc_0727d"),
    (0x7298, troops::tally_location_job4_troops, "loc_07298"),
    (0x83fd, troops::alert_location_troops, "loc_083fd"),
    (0x77d7, troops::troop_take_skill, "loc_077d7"),
    (0x7b58, troops::drop_selected_troop, "loc_07b58"),
    (0x83bc, troops::settle_arrived_troop, "loc_083bc"),
    (0x71bc, troops::maybe_mark_troop, "loc_071bc"),
    (0x998e, story::swap_game_state, "loc_0998e"),
    (0x57b2, map::draw_map_header, "loc_057b2"),
    (0x5a1a, map::open_map_screen, "loc_05a1a"),
    (0x56ed, map::draw_map_label, "loc_056ed"),
    (0xeb74, files::evict_oldest, "loc_0eb74"),
    (0x63c7, map::draw_map_dot, "loc_063c7"),
    (0x7dd9, map::place_panel_rects, "loc_07dd9"),
    (0x8256, map::pick_map_location, "loc_08256"),
    (0x9bb1, map::dialogue_frame_rect, "loc_09bb1"),
    (0x66ce, troops::troop_joins, "loc_066ce"),
    (0x6d7b, troops::troop_skills_decay, "loc_06d7b"),
    (0x8df0, dialogue::check_dialogue_entries, "loc_08df0"),
    (0x93df, dialogue::start_speaker, "loc_093df"),
    (0x96f1, dialogue::say_numbered, "loc_096f1"),
    (0x7f27, troops::noted_location_skill_needs, "loc_07f27"),
    (0x7f2a, troops::skill_needs, "loc_07f2a"),
    (0x85cc, troops::reset_location_troop, "loc_085cc"),
    (0x7c63, troops::troop_map_distance, "loc_07c63"),
    (0xd280, screens::fade_panel, "loc_0d280"),
    (0x4d6c, story::run_cs_script, "loc_04d6c"),
    (0x26ac, story::draw_scene_picture, "loc_026ac"),
    (0xe851, story::check_allocator_room, "loc_0e851"),
    (0x0d45, story::sky_flash, "loc_00d45"),
    (0xebaa, files::free_chain, "loc_0ebaa"),
    (0xf204, files::open_file_size, "loc_0f204"),
    (0x9d6a, dialogue::say_phrase_list, "loc_09d6a"),
    (0x9046, dialogue::outline_picture, "loc_09046"),
    (0x8461, troops::drop_troop_particle, "loc_08461"),
    (0x6715, troops::refresh_all_troops, "loc_06715"),
    (0x9bac, dialogue::redraw_dialogue_frame, "loc_09bac"),
    (0x9f40, dialogue::speaker_text_setup, "loc_09f40"),
    (0x9908, dialogue::start_dialogue_text, "loc_09908"),
    (0x4ec6, story::next_flight_video, "loc_04ec6"),
    (0x6c46, story::phase_30_check, "loc_06c46"),
    (0x6d19, troops::merge_troop, "loc_06d19"),
    (0x64ef, map::span_handler_twice, "loc_064ef"),
    (0x7c8f, troops::record_map_distance, "loc_07c8f"),
    (0x79de, troops::redraw_dirty_panel, "loc_079de"),
    (0x94f3, dialogue::speaker_mood, "loc_094f3"),
    (0x92c9, cursor::in_dialogue_rects, "loc_092c9"),
    (0xa49c, ui::draw_counter, "loc_0a49c"),
    (0xa47d, ui::draw_counters, "loc_0a47d"),
    (0x9f31, dialogue::speaker_sentence, "loc_09f31"),
    (0x9b09, dialogue::number_words, "loc_09b09"),
    (0x40d5, story::arrive_and_update_party, "loc_040d5"),
    (0x4182, story::map_place_watch, "loc_04182"),
    (0x6bd7, troops::troop_bit_10_by_skill, "loc_06bd7"),
    (0x8018, troops::lower_needs_by_movers, "loc_08018"),
    (0x708a, troops::troop_rate, "loc_0708a"),
    (0x6fe5, troops::job_harvest, "loc_06fe5"),
    (0x72b0, troops::job_5, "loc_072b0"),
    (0x767d, troops::job_10, "loc_0767d"),
    (0x7711, troops::job_9, "loc_07711"),
    (0x7693, troops::job_8, "loc_07693"),
    (0x70cc, troops::job_1, "loc_070cc"),
    (0x71ef, troops::job_4, "loc_071ef"),
    (0x6827, troops::troop_facing_sprite, "loc_06827"),
    (0xd23d, screens::nudge_panel, "loc_0d23d"),
    (0x55c0, map::unmark_regions_of, "loc_055c0"),
    (0x55dd, map::unmark_map_cells, "loc_055dd"),
    (0x0fb2, ui::sky_refresh, "loc_00fb2"),
    (0x26da, story::add_message, "loc_026da"),
    (0x1045, story::event_10b, "sub_1045"),
    (0x1103, story::on_phase_30, "sub_1103"),
    (0x3950, story::stop_sky_fade, "loc_03950"),
    (0x29f0, story::add_pending_event, "loc_029f0"),
    (0x2ad8, story::tell_pending_event, "loc_02ad8"),
    (0x38f1, ui::start_sky_task, "loc_038f1"),
    (0x4b5f, map::clamp_view_rect, "loc_04b5f"),
    (0x600e, map::draw_location_caption, "loc_0600e"),
    (0x7d0c, map::draw_skills_panel, "loc_07d0c"),
    (0x2a51, story::drop_events, "loc_02a51"),
    (0x32c7, story::record_age_phrase, "loc_032c7"),
    (0x4c45, map::draw_sprite_list, "loc_04c45"),
    (0x5584, map::unmark_pending_regions, "loc_05584"),
    (0xe826, files::scan_dat_toc, "loc_0e826"),
    (0x1771, story::start_sequence, "loc_01771"),
    (0x4da0, story::draw_cs_sprite_list, "loc_04da0"),
    (0x9d94, story::draw_dialogue_sprites, "loc_09d94"),
    (0x9ab4, dialogue::sentence_number_part, "loc_09ab4"),
    (0x6639, troops::for_troops_at_location, "loc_06639"),
    (0x858c, troops::unlink_troop, "loc_0858c"),
    (0x9b49, dialogue::speak_number_part, "loc_09b49"),
    (0x0a3e, story::stop_keep_framebuffer_task, "loc_00a3e"),
    (0x4d00, story::stop_alarm_task, "loc_04d00"),
    (0x08f0, story::set_current_place, "loc_008f0"),
    (0x9a7b, dialogue::random_sentence, "loc_09a7b"),
    (0xdaaf, input::move_cursor_by, "loc_0daaf"),
    (0x8c8a, dialogue::close_dialogue_box, "loc_08c8a"),
    (0xa0f1, dialogue::map_mode_2_record, "loc_0a0f1"),
    (0x4a5a, map::draw_map_points, "loc_04a5a"),
    (0x49d4, map::open_zoomed_map_with_points, "loc_049d4"),
    (0xbdbb, screens::stats_screen_marks, "loc_0bdbb"),
    (0x599f, map::map_point_moved, "sub_599f"),
    (0x8e9e, dialogue::store_spacing, "loc_08e9e"),
    (0x110f, story::on_phase_34, "sub_110f"),
    (0x29ee, story::note_pair_0, "loc_029ee"),
    (0x1166, story::event_105, "sub_1166"),
    (0x22b1, story::bands_b4, "loc_022b1"),
    (0x514e, math::direction_256, "loc_0514e"),
    (0x60f8, map::location_balance, "loc_060f8"),
    (0x9b8b, dialogue::drop_dialogue_text, "loc_09b8b"),
    (0x9655, dialogue::drop_speaker_gauge, "loc_09655"),
    (0x605c, map::caption_count_line, "loc_0605c"),
    (0x7847, troops::troop_orders_menu, "loc_07847"),
    (0x102f, story::on_phase_0c, "sub_102f"),
    (0x7b1b, gfx::fill_and_outline, "loc_07b1b"),
    (0x686e, troops::troop_screen_place, "loc_0686e"),
    (0x4bdf, map::draw_scroll_scene, "loc_04bdf"),
    (0x38e1, ui::sky_if_changed, "loc_038e1"),
    (0xa541, ui::close_settings_panel, "sub_a541"),
    (0xab92, voice::voice_stream_task, "loc_0ab92"),
    (0xc868, voice::scene_sound_sequence, "loc_0c868"),
    (0x7b0f, gfx::redraw_panel_box, "loc_07b0f"),
    (0x5adf, map::leave_map_screen, "loc_05adf"),
    (0x1e43, troops::pick_attack_location, "loc_01e43"),
    (0x6e20, troops::troop_day_update, "loc_06e20"),
    (0xb98b, globe::globe_frame, "loc_0b98b"),
    (0xba9e, globe::globe_turn_to, "loc_0ba9e"),
    (0x1071, story::start_phase_15, "loc_01071"),
    (0x4aeb, map::scene_39, "loc_04aeb"),
    (0x644e, map::location_circle, "loc_0644e"),
    (0x6447, map::draw_location_circle, "loc_06447"),
    (0x646f, map::location_circle_span, "loc_0646f"),
    (0x653a, map::move_trail_span, "loc_0653a"),
    (0x5d44, map::note_shown_location, "loc_05d44"),
    (0x74b6, map::abandon_location, "loc_074b6"),
    (0x617a, map::sprite_row, "loc_0617a"),
    (0x329d, troops::troop_rate_by_age, "loc_0329d"),
    (0x5746, map::map_header_by_cursor, "loc_05746"),
    (0x714c, troops::troop_event_roll, "loc_0714c"),
    (0x98e6, dialogue::reset_dialogue, "loc_098e6"),
    (0x4e12, map::place_scene, "loc_04e12"),
    (0x331e, story::describe_place, "loc_0331e"),
    (0x342d, troops::troop_yield, "loc_0342d"),
    (0x3406, troops::add_troop_strength, "loc_03406"),
    (0x5728, troops::count_by_job, "loc_05728"),
    (0x4e04, troops::count_job0_troops_bit_80, "loc_04e04"),
    (0x7289, troops::clear_troop_bit_40, "loc_07289"),
    (0x762a, troops::raid_garrison_harkonnen_troop, "loc_0762a"),
    (0x764d, troops::lose_harkonnen_troop, "loc_0764d"),
    (0x75ea, troops::raid_renew_harkonnen_troop, "loc_075ea"),
    (0x75af, troops::raid_rally_own_troop, "loc_075af"),
    (0x7506, troops::flag_harkonnen_or_start_troop, "loc_07506"),
    (0x34d0, troops::count_troop_at_place, "loc_034d0"),
    (0x391d, tasks::sky_fade_step, "loc_0391d"),
    (0x6ad4, troops::set_troop_job, "loc_06ad4"),
    (0x6ecb, troops::redo_flagged_troop_job, "loc_06ecb"),
    (0x30b9, dialogue::add_speaker, "sub_30b9"),
    (0x3120, dialogue::add_speaker_40, "sub_3120"),
    (0x2017, story::ui_comm, "ui_comm"),
    (0x17be, ui::redraw_head_panel, "loc_017be"),
    (0xbf26, scenes::stats_numbers, "loc_0bf26"),
    (0xaa70, voice::transfer_sd_block, "transfer_sd_block_qq"),
    (0x5e6d, map::nearest_shown_location, "loc_05e6d"),
    (0xba15, globe::globe_increment_tilt, "globe_increment_tilt_by_ax"),
    (0x9cc6, dialogue::grow_dialogue_rect, "loc_09cc6"),
    (0xd7b7, screens::blink_gauges, "loc_0d7b7"),
    (0x3d83, story::sal_read_position_markers, "sal_read_position_markers"),
    (0x8ed3, text::word_width, "loc_08ed3"),
    (0x5605, map::clear_map_header, "loc_05605"),
    (0x7bb9, map::comment_on_troop, "loc_07bb9"),
    (0x636a, map::map_row_right, "loc_0636a"),
    (0x639a, map::map_row_left, "loc_0639a"),
    (0x6ac5, troops::set_troop_activity, "loc_06ac5"),
    (0x4c92, map::setup_scroll_scene, "loc_04c92"),
    (0x4d57, map::scroll_scene_31, "loc_04d57"),
    (0x3916, ui::sky_fade_task, "loc_03916"),
    (0xb9b9, globe::globe_tilt_down, "loc_0b9b9"),
    (0xb9c0, globe::globe_tilt_up, "loc_0b9c0"),
    (0xb9cc, globe::globe_turn_left, "loc_0b9cc"),
    (0xb9d3, globe::globe_turn_right, "loc_0b9d3"),
    (0x7f90, troops::nearest_needing_location, "loc_07f90"),
    (0x851f, troops::link_troop, "loc_0851f"),
    (0xbc99, screens::open_box, "loc_0bc99"),
    (0x61d3, map::sprite_column, "loc_061d3"),
    (0x8250, map::map_click_and_resource, "loc_08250"),
    (0x7e1e, map::draw_place_skill_bars, "loc_07e1e"),
    (0x7e3d, map::skill_bars, "loc_07e3d"),
    (0x5dce, map::list_shown_locations, "loc_05dce"),
    (0x43e3, ui::show_view, "loc_043e3"),
    (0x40f9, map::scan_encounters, "loc_040f9"),
    (0xba75, globe::set_globe_tilt_and_rotation, "set_globe_tilt_and_rotation"),
    (0x9673, dialogue::add_speaker_gauge, "loc_09673"),
    (0x1843, ui::head_blink, "loc_01843"),
    (0x1442, ui::chankiss, "chankiss"),
    (0x3af9, ui::bring_portrait, "loc_03af9"),
    (0x6c6f, troops::troops_tick, "loc_06c6f"),
    (0x8e16, dialogue::break_lines, "loc_08e16"),
    (0x5f79, map::drop_caption, "loc_05f79"),
    (0x9123, dialogue::pick_sentence, "loc_09123"),
    (0x920f, dialogue::open_talking_head_resource, "open_talking_head_resource"),
    (0x41e1, map::location_ahead, "loc_041e1"),
    (0x7be0, troops::refresh_selected_troop, "loc_07be0"),
    (0x11e6, story::event_place_message, "sub_11e6"),
    (0x1797, ui::ui_draw_head_and_shoulders, "ui_draw_head_and_shoulders"),
    (0x0086, ui::draw_head_panel, "loc_00086"),
    (0x439f, ui::scene_24, "loc_0439f"),
    (0x62f2, map::troop_marker, "loc_062f2"),
    (0x62fe, map::marker_at_record, "loc_062fe"),
    (0x6314, map::marker_at_dx_bx, "loc_06314"),
    (0x0a23, story::video_step, "loc_00a23"),
    (0x57e5, map::region_colours, "loc_057e5"),
    (0x1948, ui::people_panel, "loc_01948"),
    (0x98b2, dialogue::close_dialogue_text, "loc_098b2"),
    (0x9285, cursor::dialogue_hit, "loc_09285"),
    (0x978e, dialogue::redraw_dialogue_scene, "loc_0978e"),
    (0x8604, troops::troop_step, "loc_08604"),
    (0x0c3b, story::creature_step, "loc_00c3b"),
    (0x31f6, story::describe_troop, "loc_031f6"),
    (0x44ab, screens::blink_map_pointer, "sub_44ab"),
    (0x2ffb, screens::panel_buttons, "loc_02ffb"),
    (0x1027, story::on_phase_08, "sub_1027"),
    (0x10a4, story::on_phase_1c, "sub_10a4"),
    (0x1188, story::on_phase_54, "loc_01188"),
    (0x0083, ui::draw_head_panel_after_language_check, "loc_00083"),
    (0x2090, story::next_demand, "loc_02090"),
    (0x99da, dialogue::dialogue_text_step, "loc_099da"),
    (0x80df, dialogue::show_phrase_on_map, "loc_080df"),
    (0x88af, dialogue::show_phrase, "loc_088af"),
    (0xb9ae, globe::globe_drag, "sub_b9ae"),
    (0x9efd, dialogue::start_text_voice, "loc_09efd"),
    (0xab45, voice::play_voice_lowering_music, "loc_0ab45"),
    (0xab4f, voice::play_voice_19, "loc_0ab4f"),
    (0x186b, map::toggle_map_screen, "loc_0186b"),
    (0x8763, map::redraw_and_toggle, "loc_08763"),
    (0xd962, input::cursor_glide, "loc_0d962"),
    (0x2170, story::update_people, "loc_02170"),
    (0x5c03, map::map_hover, "loc_05c03"),
    (0x7443, map::location_raided, "loc_07443"),
    (0x5133, math::direction_between, "loc_05133"),
    (0x63f0, map::grow_spice, "map_func_qq"),
    (0x65b6, map::spread_spice, "loc_065b6"),
    (0xaa96, voice::music_for_situation, "loc_0aa96"),
    (0x4821, ui::scene_2a, "sub_4821"),
    (0xace6, voice::music_step, "loc_0ace6"),
    (0xad5e, voice::pick_music, "loc_0ad5e"),
    (0x59c1, map::move_map_point, "sub_59c1"),
    (0x5206, map::heading_step, "loc_05206"),
    (0xcfb9, resources::startup_tables, "loc_0cfb9"),
    (0x5a56, map::enter_map_view, "sub_5a56"),
    (0xaf26, ui::open_dialogue_panel, "sub_af26"),
    (0xaf43, ui::panel_with_head, "sub_af43"),
    (0x8acc, map::note_phrase_count, "loc_08acc"),
    (0x5a9a, map::ui_main_view_map_interface, "ui_main_view_map_interface"),
    (0xbc64, globe::globe_click, "sub_bc64"),
    (0xb8c6, globe::enter_globe, "sub_b8c6"),
    (0x8a3b, map::check_room_phrase, "loc_08a3b"),
    (0x9c2d, dialogue::diff_rects, "loc_09c2d"),
    (0x9c54, dialogue::rects_missing, "loc_09c54"),
    (0x99be, dialogue::two_heads_text_step, "loc_099be"),
    (0x5f25, map::caption_box, "loc_05f25"),
    (0x45de, story::redraw_message_line, "loc_045de"),
    (0x5d6d, map::refresh_map_view, "loc_05d6d"),
    (0x813e, map::route_lines, "loc_0813e"),
    (0x6770, troops::troop_sprite_list, "loc_06770"),
    (0x4586, map::message_subject, "loc_04586"),
    (0x445d, screens::place_map_pointer, "loc_0445d"),
    (0x4377, screens::redraw_map_frame, "sub_4377"),
    (0x5ee4, map::marker_caption, "loc_05ee4"),
    (0x60ac, map::location_panel_line, "loc_060ac"),
    (0x3520, story::scene_sentence, "sub_3520"),
    (0x97cf, dialogue::line_done, "loc_097cf"),
    (0x9f9e, dialogue::say_dialogue_entry, "loc_09f9e"),
    (0xa172, dialogue::note_remark_by_talker, "loc_0a172"),
    (0xa125, dialogue::reward_talk_by_talker, "loc_0a125"),
    (0xa244, dialogue::offer_answers, "loc_0a244"),
    (0xa248, dialogue::offer_answers_flagged, "loc_0a248"),
    (0xa25b, dialogue::find_scene_voice_entry, "loc_0a25b"),
    (0x8403, troops::alert_troop, "loc_08403"),
    (0x02c1, tails::play_intro_scene, "loc_002c1"),
    (0xa28e, dialogue::show_person_on_map, "loc_0a28e"),
    (0x9761, dialogue::start_speaker_dialogue, "loc_09761"),
    (0xd2e2, screens::leave_and_fade, "loc_0d2e2"),
    (0x9468, dialogue::next_dialogue_line, "sub_9468"),
    (0x945b, dialogue::dialogue_line, "sub_945b"),
    (0xd32f, screens::unwind_and_fade, "loc_0d32f"),
    (0xa3f9, panels::settings_ui_draw, "settings_ui_draw"),
    (0xa576, panels::settings_click, "sub_a576"),
    (0x450e, story::map_pick_place, "sub_450e"),
    (0x541f, map::open_place_view, "loc_0541f"),
    (0x5575, map::mouse_to_zoom_box, "loc_05575"),
    (0x5b05, map::map_list_or_zoom, "sub_5b05"),
    (0x9f8b, dialogue::say_entry_mask_20, "loc_09f8b"),
    (0x563e, map::zoom_legend, "loc_0563e"),
    (0x3d2f, ui::draw_character, "loc_03d2f"),
    (0xb039, dialogue::book_page, "loc_0b039"),
    (0x878c, map::map_panel_buttons, "loc_0878c"),
    (0x1dd7, story::harkonnen_take, "sub_1dd7"),
    (0xd0ff, text::font_draw_glyph_func, "sub_d0ff"),
    (0x1dd4, story::spice_demand_day, "sub_1dd4"),
    (0x8ccd, dialogue::text_box_for, "loc_08ccd"),
    (0x6b34, map::animate_map_sprites, "sub_6b34"),
    (0x5ce4, troops::map_troop_click, "sub_5ce4"),
    (0x78bc, troops::troop_caption, "loc_078bc"),
    (0x78e9, troops::troop_panel, "loc_078e9"),
    (0xd50f, cursor::highlight_hovered_text_action_item, "highlight_hovered_text_action_item"),
    (0x0b45, story::scene_particles_step, "sub_0b45"),
    (0x9197, dialogue::setup_lip_sync_data_from_current, "setup_lip_sync_data_from_current"),
    (0x91a0, dialogue::setup_lip_sync_data_from_sprite_sheet, "setup_lip_sync_data_from_sprite_sheet"),
    (0x0169, globe::map2_resource_func, "map2_resource_func"),
    (0x8831, map::ui_click_map_buttons, "ui_click_map_buttons"),
    (0x881f, map::ui_click_map_buttons, "ui_click_map_left"),
    (0x8824, map::ui_click_map_buttons, "ui_click_map_right"),
    (0x8829, map::ui_click_map_buttons, "ui_click_map_up"),
    (0x882e, map::ui_click_map_buttons, "ui_click_map_down"),
    (0xcf4b, hnm::irulx_draw_or_clear_subtitle, "IRULx_draw_or_clear_subtitle"),
    (0xd338, cursor::install_ui_list_unhighlighted, "loc_0d338"),
    (0xd33a, cursor::install_ui_list, "loc_0d33a"),
    (0xd36d, cursor::replace_ui_list, "loc_0d36d"),
    (0xd397, cursor::redraw_ui_slots, "loc_0d397"),
    (0x2ebf, screens::install_current_ui_list, "loc_02ebf"),
    (0x2eb2, screens::place_panel_back, "loc_02eb2"),
    (0x5944, map::place_view_click, "loc_05944"),
    (0x35ad, story::scene_start, "loc_035ad"),
    (0x5c76, map::map_view_click, "sub_5c76"),
    (0x90bd, dialogue::speaker_list_item, "loc_090bd"),
    (0xb941, globe::globe_list, "loc_0b941"),
    (0xbc1f, globe::globe_or_map_list, "sub_bc1f"),
    (0x2e52, story::scene_refresh, "loc_02e52"),
    (0x2db1, story::room_arrive, "loc_02db1"),
    (0x2dbf, story::room_arrive, "loc_02dbf"),
    (0x2dd3, story::room_arrive, "loc_02dd3"),
    (0x37b2, story::draw_room, "loc_037b2"),
    (0x37b5, story::draw_room, "loc_037b5"),
    (0x37eb, story::show_place_view, "loc_037eb"),
    (0x1b0d, story::time_step, "loc_01b0d"),
    (0x1b23, story::time_step, "loc_01b23"),
    (0x3551, story::scene_record, "loc_03551"),
    (0x2b2a, story::clock_step, "loc_02b2a"),
    (0x4f09, map::flight_step, "sub_4f09"),
    (0x4f0c, map::flight_step, "loc_04f0c"),
    (0x9215, dialogue::dialogue_click, "loc_09215"),
    (0x8308, troops::troop_move, "loc_08308"),
    (0x8357, troops::troop_arrived, "loc_08357"),
    (0x84a6, troops::troop_set_out, "loc_084a6"),
    (0x8b11, dialogue::layout_text, "loc_08b11"),
    (0x8944, dialogue::format_interpolated_string, "format_interpolated_string"),
    (0x2efb, screens::build_action_list, "loc_02efb"),
    (0x88d2, dialogue::format_and_show, "loc_088d2"),
    (0x64b2, map::circle_walk, "loc_064b2"),
    (0x88f1, dialogue::expand_phrase, "loc_088f1"),
    (0x5274, map::nearest_places, "loc_05274"),
    (0xbd25, globe::globe_point, "loc_0bd25"),
    (0xb827, globe::draw_globe_and_ui, "draw_globe_and_ui_to_front_buffer"),
    (0x3e80, story::edge_points, "draw_gradient_section_2"),
    (0xae3f, voice::alloc_music_buffer, "loc_0ae3f"),
    (0xae54, voice::alloc_voice_buffer, "loc_0ae54"),
    (0xbaf2, globe::globe_cell_at, "loc_0baf2"),
    (0xb6c3, globe::draw_zoomed_globe, "map_draw_zoomed_globe"),
    (0xb7d2, globe::globe_row, "loc_0b7d2"),
    (0xa5df, panels::settings_drag, "sub_a5df"),
    (0x9d16, dialogue::draw_talking_head, "copy_non_pcm_lip_sync_data_and_draw_talking_head"),
    (0xa7c2, voice::lip_sync_frame_task, "lip_sync_frame_task"),
    (0xd48a, ui::button_label, "loc_0d48a"),
    (0xee02, dos::ems_copy, "ems_memory_func_2"),
    (0xee46, dos::ems_copy, "ems_memory_func_1"),
    (0xecec, dos::xms_move, "xms_memory_func_1"),
    (0xedb9, dos::map_ems_for_midi_audio, "map_ems_for_midi_audio"),
    (0xea7b, dos::init_extended_memory_allocator, "init_extended_memory_allocator"),
    (0xe913, dos::install_interrupt_handlers, "install_interrupt_handlers"),
    (0xdce0, input::read_game_port, "read_game_port"),
    (0xea32, input::initialize_joystick, "initialize_joystick"),
    (0xb30f, saves::load_save_game_timestamp, "load_save_game_timestamp"),
    (0xebe3, dos::alloc_extended_memory, "alloc_extended_memory"),
    (0xed4c, dos::ems_init, "loc_0ed4c"),
    (0xe97a, input::initialize_mouse, "initialize_mouse"),
    (0xb4ea, saves::compress_sav, "compress_sav"),
    (0xaa0f, voice::decode_sd_block, "decode_sd_block"),
    (0xe8d5, dos::uninitialize_memory_driver, "uninitialize_memory_driver"),
    (0xf0f6, dos::bump_alloc, "bump_alloc_get_addr_in_di_and_bump"),
    (0xf0ff, dos::bump_alloc, "bump_allocate_bump_cx_bytes"),
    (0xc370, draw::blit_repeated_x, "blit_repeated_x"),
    (0xf229, files::open_res_or_file_or_die, "open_res_or_file_or_die"),
    (0xc93c, hnmload::hnm_read_header, "hnm_read_header"),
    (0xf0b9, files::open_resource_by_index, "open_resource_by_index_si_into_esdi"),
    (0xf0d6, files::read_and_maybe_hsq, "read_and_maybe_hsq"),
    (0xf0a0, files::open_resource_force_hsq, "open_resource_force_hsq"),
    (0xf255, files::open_nonres_file, "open_nonres_file"),
    (0xdf1e, input::get_mouse_pos_etc, "get_mouse_pos_etc"),
    (0xcb1a, hnmload::hnm_prefetch, "hnm_prefetch"),
    (0xf314, files::get_res_index_by_name, "get_res_index_in_ax_by_name_dssi"),
    (0xcc96, hnmload::hnm_decode_video_frame, "hnm_decode_video_frame"),
    (0x00d1, files::initialize_resources, "initialize_resources"),
    (0x021c, intro::play_intro2, "play_intro2"),
    (0xde0c, sound::check_midi, "check_midi"),
    (0xe9f4, input::mouse_event, "mouse_func_uncalled"),
    (0xf13f, files::allocator_free_space, "allocator_attempt_to_free_space"),
    (0xeab7, dos::memory_func_qq, "memory_func_qq"),
    (0xc2a1, saves::unrle_image_to_tmp_buffer, "unrle_image_to_tmp_buffer"),
    (0xa82e, voice::set_lipsync_data_to_al, "set_lipsync_data_to_al"),
    (0x8f28, dialogue::draw_speech_bubble, "draw_speech_bubble"),
    (0xc6ad, saves::particles_update_dirty_rect, "particles_update_dirty_rect"),
    (0x3b59, story::draw_sal, "draw_SAL"),
    (0x3be9, story::sal_polygon, "SAL_polygon"),
    (0xd443, screens::press_panel_button, "sub_d443"),
    (0xd42f, screens::press_panel_button, "sub_d42f"),
    (0xd434, screens::press_panel_button, "sub_d434"),
    (0xd439, screens::press_panel_button, "sub_d439"),
    (0xd43e, screens::press_panel_button, "sub_d43e"),
    (0xa6cc, voice::load_voc_and_lipsync_data, "load_voc_and_lipsync_data"),
    (0xccf4, hnmload::hnm_decode_typed_chunk, "hnm_decode_typed_chunk_video_to_bp"),
    (0x02de, tails::intro_part_2, "intro_part_2"),
    (0x02e0, tails::draw_stars, "draw_stars"),
    (0x06ce, tails::intro_open_desert_video, "intro_load_desert_flyover_video"),
    (0x06d3, tails::intro_open_desert_video, "intro_desert_flight_to_sietch"),
    (0x0788, tails::intro_chani_in_sietch, "intro_chani_in_sietch"),
    (0x3f24, tails::ui_click_room_left, "ui_click_room_left"),
    (0xd239, tails::step_time_panel_to_phase_2, "loc_0d239"),
    (0xb2aa, tails::stop_clock_and_push_screen, "loc_0b2aa"),
    (0x1860, tails::blink_and_toggle_map_screen, "loc_01860"),
    (0x6acb, tails::give_troop_job_unless_same, "loc_06acb"),
    (0x6c15, tails::troop_job_check, "loc_06c15"),
    (0xb96b, tails::globe_open_and_redraw, "loc_0b96b"),
    (0xc02e, tails::update_totals, "loc_0c02e"),
    (0x0acd, tails::intro_night_attack, "intro_28_night_attack_start"),
    (0x6dbb, tails::set_people_state_at_place, "loc_06dbb"),
    (0xafc7, tails::book_next_page, "sub_afc7"),
    (0x941d, tails::dialogue_area_click, "sub_941d"),
    (0x0f08, tails::draw_jessica_on_palace_plan, "loc_00f08"),
    (0x1707, tails::run_script_next_step, "loc_01707"),
    (0x18ee, tails::ui_draw_palace_plan, "ui_draw_palace_plan"),
    (0x7c02, tails::select_place_and_tell, "loc_07c02"),
    (0xbed7, tails::result_bars, "loc_0bed7"),
    (0x380c, tails::set_room_light_palette, "loc_0380c"),
    (0x1c46, tails::new_day, "loc_01c46"),
    (0xb389, tails::create_save_cl, "create_save_cl"),
    (0xb3b0, saves::load_save_cl, "loc_0b3b0"),
    (0xa03f, tails::apply_dialogue_line, "loc_0a03f"),
    (0x79ee, tails::open_place_panel, "loc_079ee"),
    (0x5692, tails::draw_map_place_panel, "loc_05692"),
    (0x4795, tails::run_night_sequence, "loc_04795"),
    (0x0981, tails::set_place_and_draw_room, "loc_00981"),
    (0x4913, tails::load_night_video_first_frame, "loc_04913"),
    // Video scenes (checked with REC_BUDGET raised).
    (0x0e66, videos::play_comm_device_video, "loc_00e66"),
    (0x0e6c, videos::play_video_lowering_head, "loc_00e6c"),
    (0x0e77, videos::play_wormsuit_video, "loc_00e77"),
    (0x37f4, videos::draw_flight_room, "loc_037f4"),
    (0x488a, videos::play_sal_scene, "loc_0488a"),
    (0x4abe, videos::redraw_flight_room, "loc_04abe"),
    (0xc8c1, videos::draw_video_sprite_and_wait, "loc_0c8c1"),
    (0xc8fb, videos::play_video_with_callback, "loc_0c8fb"),
    (0x2c9a, tails::set_up_talk_screen, "loc_02c9a"),
    (0x2540, tails::empty_action, "loc_02540"),
    (0xa1f7, tails::start_sequence_for_phase, "loc_0a1f7"),
    (0x6155, troops::add_troop_to_balance, "loc_06155"),
    (0xa1d0, tails::set_47a5_ff, "loc_0a1d0"),
    (0xa1dc, tails::set_47a5_80, "loc_0a1dc"),
    (0xa1ed, tails::count_0a1ed, "loc_0a1ed"),
    (0x80c7, tails::empty_callback, "loc_080c7"),
    (0x68da, troops::refresh_flagged_troop, "loc_068da"),
    (0x6e0f, troops::move_troop_bit_20_to_1000, "loc_06e0f"),
    (0xe675, dos::open_dune_dat, "open_dune_dat"),
    (0xb427, tails::create_save_in_memory, "create_save_in_memory"),
    (0x49d9, tails::redraw_map_view, "loc_049d9"),
    (0x6a89, tails::change_shown_troop_job, "loc_06a89"),
    (0x6a35, tails::set_shown_troop_job_keeping_bits, "loc_06a35"),
    (0x6a33, tails::set_shown_troop_job_3, "loc_06a33"),
    (0x0d8e, tails::start_comm_call, "loc_00d8e"),
    (0x0f95, tails::advance_time_to, "loc_00f95"),
    (0x780a, tails::open_selection_orders_screen, "loc_0780a"),
    (0x2c92, tails::transition_to_talk_screen, "loc_02c92"),
    (0x28a1, tails::comm_open_caller, "loc_028a1"),
    (0x2806, tails::comm_ring_incoming_call, "loc_02806"),
    (0x542f, map::reopen_place_view, "loc_0542f"),
    (0xe594, dos::initialize_system, "initialize_system"),
    (0xe76a, dos::initialize_audio, "initialize_audio"),
    (0x0771, tails::intro_lady_jessica_2, "intro_lady_jessica_2"),
    (0x0995, tails::intro_duke_leto, "intro_duke_leto"),
    (0x07ee, tails::intro_paul_on_red_background, "intro_paul_on_red_background"),
    (0xb024, tails::book_turn_page_back, "loc_0b024"),
    (0x07c6, tails::intro_somebody_in_sietch, "intro_somebody_in_sietch"),
    (0x0802, tails::intro_show_desert_sky, "loc_00802"),
    (0x09ad, tails::intro_26_baron, "intro_26_baron"),
    (0x7ba3, tails::select_and_open_panel, "loc_07ba3"),
    (0x13c8, tails::start_script_at_si, "loc_013c8"),
    (0xd323, tails::push_screen, "loc_0d323"),
    (0x189a, tails::redraw_room_by_transition, "loc_0189a"),
    (0x07a3, tails::intro_kynes, "intro_kynes"),
    (0x09f5, tails::play_credits_with_music, "loc_009f5"),
    (0x094a, tails::intro_draw_sky_and_sheet_2d, "loc_0094a"),
    (0x47fb, tails::step_orni_rotor, "loc_047fb"),
    (0xaed6, tails::open_book, "sub_aed6"),
    (0x09c7, tails::setup_intro_talking_head, "loc_009c7"),
    (0x0886, tails::intro_25_sting_start, "intro_25_sting_start"),
    (0x28b5, tails::comm_draw_caller, "loc_028b5"),
    (0x0ed0, tails::open_palace_plan_overview, "sub_0ed0"),
    (0x82da, tails::move_troop_to, "loc_082da"),
    (0xbe1d, tails::results_draw_text_and_icons, "results_draw_text_and_icones"),
    (0x0826, tails::intro_desert_sky_next_picture, "sub_0826"),
    (0x9719, tails::say_next_place_line, "loc_09719"),
    (0xb18b, tails::b18b, "sub_b18b"),
    (0x586e, tails::dispatch_by_troops_present, "loc_0586e"),
    (0xcfe4, tails::load_language_phrases, "loc_0cfe4"),
    (0x5406, tails::center_map_and_open_place_view, "loc_05406"),
    (0xbc81, tails::leave_globe_to_map, "loc_0bc81"),
    (0x06d8, tails::intro_start_video_12, "sub_06d8"),
    (0x4057, story::enter_room, "loc_04057"),
    (0x3f15, story::room_move, "ui_click_room_up"),
    (0x3f1a, story::room_move, "ui_click_room_right"),
    (0x3f1f, story::room_move, "ui_click_room_down"),
    (0x3f27, story::room_move, "ui_click_move_room"),
    (0xbc0c, globe::draw_globe_cursor_at_dx_bx, "draw_globe_cursor_at_dx_bx"),
    (0x98af, dialogue::close_dialogue, "loc_098af"),
];

/// 8-bit register numbers (the CPU's encoding).
#[allow(dead_code)]
mod r8 {
    pub const AL: usize = 0;
    pub const CL: usize = 1;
    pub const DL: usize = 2;
    pub const BL: usize = 3;
    pub const AH: usize = 4;
    pub const CH: usize = 5;
    pub const DH: usize = 6;
    pub const BH: usize = 7;
}

const B: Width = Width::B;
const W: Width = Width::W;

/// Access to the machine for ports: registers, memory, flag-exact

/// Per entry: how often a port ran all of its own code (reached its
/// `ret` or its final jump). A shadow check counts a call as bailed when
/// anything in it bailed, also a callee or the routine a port ends in;
/// this tells whether the port's own code was run by Rust to the end.
static REACHED: [AtomicU32; 0x10000] = [const { AtomicU32::new(0) }; 0x10000];

/// How often the port at `entry` reached its own end (see [`REACHED`]).
pub fn reached(entry: u16) -> u32 {
    REACHED[entry as usize].load(Ordering::Relaxed)
}

/// operations, calls, and the instruction count.
#[allow(clippy::struct_field_names)]
struct Ctx<'a> {
    m: &'a mut Machine,
    /// Where the port was entered (for [`reached`]).
    entry: u16,
    /// `REC_INPLACE_DIAG` (see [`Ctx::boundary`]): on for this port, the
    /// addresses it read since its last boundary, and what the interrupts
    /// taken in place in its last stretch wrote and read.
    diag: bool,
    pre_reads: Vec<u32>,
    pending: Option<(Vec<u32>, Vec<u32>, bool, u16)>,
    /// The length of its last `run` (diagnostics: what it reads right
    /// after a one-instruction stretch belongs to that stretch).
    last_len: u32,
    pre_after_one: bool,
}

#[allow(dead_code)]
impl Ctx<'_> {
    fn new(m: &mut Machine) -> Ctx<'_> {
        let entry = m.cpu.ip;
        let diag = m.rec.as_ref().is_some_and(|r| r.inplace_diag) && !m.in_irq_in_place();
        if diag {
            m.hw.read_log = Some(Vec::new());
            m.hw.write_log = Some(Vec::new());
        }
        Ctx { m, entry, diag, pre_reads: Vec::new(), pending: None, last_len: 0, pre_after_one: false }
    }

    // --- Virtual time -------------------------------------------------

    /// Start a straight-line stretch of `insns` original instructions from
    /// offset `at`. Bails there, before doing anything, if an interrupt (or
    /// the end of a shadow check) falls inside it, so the interpreter takes
    /// over at the exact instruction; else counts them.
    fn run(&mut self, at: u16, insns: u32) -> Result<(), Bail> {
        self.boundary();
        self.m.cpu.ip = at;
        if self.m.rec_run(insns, self.in_place())? {
            self.m.cpu.ip = at;
            return Err(Bail);
        }
        self.last_len = insns;
        if self.diag {
            let rec = self.m.rec.as_mut().unwrap();
            if !rec.handler_wrote.is_empty() || !rec.handler_read.is_empty() {
                let wrote = std::mem::take(&mut rec.handler_wrote);
                let read = std::mem::take(&mut rec.handler_read);
                let inside = std::mem::replace(&mut rec.handler_inside, false);
                let e = rec.inplace_conflicts.entry(self.entry).or_default();
                e.0[0] += 1;
                if let Some(a) = first_common(&self.pre_reads, &wrote) {
                    e.0[1] += 1;
                    e.1[1] = a;
                }
                let w = rec.inplace_where.entry(self.entry).or_default();
                let kind = if self.pre_after_one { 4 } else { 1 };
                for a in all_common(&self.pre_reads, &wrote) {
                    *w.entry((kind, at, a)).or_default() += 1;
                }
                self.pending = Some((wrote, read, inside, at));
            }
        }
        Ok(())
    }

    /// Whether interrupts due in its stretches may be taken in place: not
    /// in a port sharing memory with the handlers (`ticks.rs`), unless
    /// `REC_INPLACE_ALL` (diagnostics).
    fn in_place(&self) -> bool {
        !ticks::shared(self.entry) || self.m.rec.as_ref().is_some_and(|r| r.inplace_all)
    }

    /// `REC_INPLACE_DIAG`: a boundary of the port's stretches (a `run`, a
    /// call, its end). What it read and wrote since the last one is checked
    /// against what the interrupts taken in place in its last stretch
    /// wrote and read; what it read is kept for the next stretch.
    fn boundary(&mut self) {
        self.m.rec_stack_fixup();
        if !self.diag {
            return;
        }
        let mut reads = self.m.hw.read_log.replace(Vec::new()).unwrap_or_default();
        let mut writes = self.m.hw.write_log.replace(Vec::new()).unwrap_or_default();
        reads.sort_unstable();
        reads.dedup();
        writes.sort_unstable();
        writes.dedup();
        if let Some((wrote, read, inside, at)) = self.pending.take()
            && inside
        {
            // (An interrupt at the stretch's first instruction comes before
            // all of it: reads and writes after it are right.)
            let rec = self.m.rec.as_mut().unwrap();
            let e = rec.inplace_conflicts.entry(self.entry).or_default();
            if let Some(a) = first_common(&reads, &wrote) {
                e.0[2] += 1;
                e.1[2] = a;
            }
            let mut touched = read.clone();
            touched.extend_from_slice(&wrote);
            touched.sort_unstable();
            touched.dedup();
            if let Some(a) = first_common(&writes, &touched) {
                e.0[3] += 1;
                e.1[3] = a;
            }
            let w = rec.inplace_where.entry(self.entry).or_default();
            for a in all_common(&reads, &wrote) {
                *w.entry((2, at, a)).or_default() += 1;
            }
            for a in all_common(&writes, &touched) {
                *w.entry((3, at, a)).or_default() += 1;
            }
        }
        self.pre_reads = reads;
        self.pre_after_one = self.last_len == 1;
    }

    /// `REC_INPLACE_DIAG`: after a callee, what it read and wrote is not
    /// the port's (a hand port checks its own; other code does not take
    /// interrupts in place).
    fn after_callee(&mut self) {
        if self.diag {
            self.m.hw.read_log = Some(Vec::new());
            self.m.hw.write_log = Some(Vec::new());
            self.last_len = 0;
        }
    }

    /// [`Ctx::run`]'s check alone, for stretches that do port I/O: they
    /// count their instructions as they go ([`Ctx::count`]), so the device
    /// sees the time of each `in`/`out`.
    fn check(&mut self, at: u16, insns: u32) -> Result<(), Bail> {
        self.stretch(at, insns, false)
    }

    /// The check of a stretch; `through`: one without port I/O, which a
    /// shadow replay may run through an interrupt due inside it
    /// ([`Machine::rec_stretch`]).
    fn stretch(&mut self, at: u16, insns: u32, through: bool) -> Result<(), Bail> {
        self.m.cpu.ip = at;
        if self.m.rec_stretch(insns, through)? {
            self.m.cpu.ip = at;
            return Err(Bail);
        }
        Ok(())
    }

    fn count(&mut self, insns: u32) {
        self.m.cpu.instructions += insns as u64;
    }

    // --- Registers ----------------------------------------------------

    fn reg(&self, r: usize) -> u16 {
        self.m.cpu.regs[r]
    }
    fn set_reg(&mut self, r: usize, v: u16) {
        self.m.cpu.regs[r] = v;
    }
    fn reg8(&self, r: usize) -> u8 {
        self.m.cpu.reg8(r)
    }
    fn set_reg8(&mut self, r: usize, v: u8) {
        self.m.cpu.set_reg8(r, v);
    }
    fn sreg(&self, s: usize) -> u16 {
        self.m.cpu.sregs[s]
    }
    fn set_sreg(&mut self, s: usize, v: u16) {
        self.m.cpu.sregs[s] = v;
    }
    fn flag(&self, f: u16) -> bool {
        self.m.cpu.flag(f)
    }
    fn set_flag(&mut self, f: u16, on: bool) {
        self.m.cpu.set_flag(f, on);
    }

    // --- Memory -------------------------------------------------------

    /// Data-segment byte.
    fn byte(&mut self, off: u16) -> u8 {
        self.m.cpu.read8(&mut self.m.hw, DS, off)
    }
    fn word(&mut self, off: u16) -> u16 {
        self.m.cpu.read16(&mut self.m.hw, DS, off)
    }
    fn set_byte(&mut self, off: u16, v: u8) {
        self.m.cpu.write8(&mut self.m.hw, DS, off, v);
        self.wrote(DS, off, 1);
    }
    fn set_word(&mut self, off: u16, v: u16) {
        self.m.cpu.write16(&mut self.m.hw, DS, off, v);
        self.wrote(DS, off, 2);
    }
    /// A port wrote `len` bytes at `seg:off`: a record of a table kept in
    /// Rust (`hand/record_table.rs`) there follows (the port is Rust code,
    /// not a write from outside). `TROOPS_TRACE=1` lists such writes,
    /// which don't go through a view yet.
    fn wrote(&mut self, seg: usize, off: u16, len: u16) {
        let at = self.m.cpu.phys(self.m.cpu.sregs[seg], off);
        if let Some((w, i, field)) = record_table::follow(self.m, at, len as u32) {
            static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            if *ON.get_or_init(|| std::env::var_os("TROOPS_TRACE").is_some()) {
                eprintln!("{} raw write by {:04x} (at {:04x}): record {} field {field:02x} ({len})", w.name(), self.entry, self.m.cpu.ip, i + 1);
            }
        }
    }
    /// Byte or word through segment register `seg`.
    fn byte_in(&mut self, seg: usize, off: u16) -> u8 {
        self.m.cpu.read8(&mut self.m.hw, seg, off)
    }
    fn word_in(&mut self, seg: usize, off: u16) -> u16 {
        self.m.cpu.read16(&mut self.m.hw, seg, off)
    }
    fn set_word_in(&mut self, seg: usize, off: u16, v: u16) {
        self.m.cpu.write16(&mut self.m.hw, seg, off, v);
        self.wrote(seg, off, 2);
    }
    /// Word at `segment:off` for a segment value no register holds.
    fn word_at(&mut self, segment: u16, off: u16) -> u16 {
        let lo = self.m.cpu.phys(segment, off);
        let hi = self.m.cpu.phys(segment, off.wrapping_add(1));
        self.m.hw.read8(lo) as u16 | (self.m.hw.read8(hi) as u16) << 8
    }
    fn byte_at_seg(&mut self, segment: u16, off: u16) -> u8 {
        let a = self.m.cpu.phys(segment, off);
        self.m.hw.read8(a)
    }
    fn set_word_at(&mut self, segment: u16, off: u16, v: u16) {
        let lo = self.m.cpu.phys(segment, off);
        let hi = self.m.cpu.phys(segment, off.wrapping_add(1));
        self.m.hw.write8(lo, v as u8);
        self.m.hw.write8(hi, (v >> 8) as u8);
    }
    fn set_byte_at_seg(&mut self, segment: u16, off: u16, v: u8) {
        let a = self.m.cpu.phys(segment, off);
        self.m.hw.write8(a, v);
    }
    // --- The game clock (Rust state, see [`clock`]) --------------------

    /// The game clock, in step with the memory.
    fn clock(&mut self) -> clock::Clock {
        clock::Clock::get(self)
    }
    /// The clock changed by `f` (written through to the memory).
    fn update_clock(&mut self, f: impl FnOnce(&mut clock::Clock)) {
        clock::update(self, f)
    }
    /// The text box's layout (Rust state, see [`text_box`]), in step
    /// with the memory.
    fn text_box(&mut self) -> text_box::TextBox {
        record_table::single(self)
    }
    /// The text box changed by `f` (written through to the memory).
    fn update_text_box(&mut self, f: impl FnOnce(&mut text_box::TextBox)) {
        record_table::update_single(self, f)
    }
    /// The dialogue's text being played (Rust state, see
    /// [`dialogue_text`]), in step with the memory.
    fn dialogue_text(&mut self) -> dialogue_text::DialogueText {
        record_table::single(self)
    }
    /// The dialogue's text changed by `f` (written through).
    fn update_dialogue_text(&mut self, f: impl FnOnce(&mut dialogue_text::DialogueText)) {
        record_table::update_single(self, f)
    }
    /// The conversation and scene state (Rust state, see
    /// [`conversation`]), in step with the memory.
    fn conversation(&mut self) -> conversation::Conversation {
        record_table::single(self)
    }
    /// The conversation changed by `f` (written through).
    fn update_conversation(&mut self, f: impl FnOnce(&mut conversation::Conversation)) {
        record_table::update_single(self, f)
    }
    /// The map view (Rust state, see [`map_view`]), in step with the
    /// memory.
    fn map_view(&mut self) -> map_view::MapView {
        record_table::single(self)
    }
    /// The map view changed by `f` (written through).
    fn update_map_view(&mut self, f: impl FnOnce(&mut map_view::MapView)) {
        record_table::update_single(self, f)
    }
    /// The map's selection (Rust state, see [`map_selection`]), in step
    /// with the memory.
    fn map_selection(&mut self) -> map_selection::MapSelection {
        record_table::single(self)
    }
    /// The map's selection changed by `f` (written through).
    fn update_map_selection(&mut self, f: impl FnOnce(&mut map_selection::MapSelection)) {
        record_table::update_single(self, f)
    }
    /// The flight (Rust state, see [`flight`]), in step with the memory.
    fn flight(&mut self) -> flight::Flight {
        record_table::single(self)
    }
    /// The flight changed by `f` (written through).
    fn update_flight(&mut self, f: impl FnOnce(&mut flight::Flight)) {
        record_table::update_single(self, f)
    }
    /// The message typed out on the message line (Rust state, see
    /// [`typed_message`]), in step with the memory.
    fn typed_message(&mut self) -> typed_message::TypedMessage {
        record_table::single(self)
    }
    /// The message typed out changed by `f` (written through).
    fn update_typed_message(&mut self, f: impl FnOnce(&mut typed_message::TypedMessage)) {
        record_table::update_single(self, f)
    }
    /// The screen buffers (Rust state, see [`screen_buffers`]), in step
    /// with the memory.
    fn screen_buffers(&mut self) -> screen_buffers::ScreenBuffers {
        record_table::single(self)
    }
    /// The screen buffers changed by `f` (written through).
    fn update_screen_buffers(&mut self, f: impl FnOnce(&mut screen_buffers::ScreenBuffers)) {
        record_table::update_single(self, f)
    }
    /// The mouse and its cursor (Rust state, see [`mouse_state`]), in step with the memory.
    fn mouse_state(&mut self) -> mouse_state::MouseState {
        record_table::single(self)
    }
    /// The mouse and its cursor changed by `f` (written through).
    fn update_mouse_state(&mut self, f: impl FnOnce(&mut mouse_state::MouseState)) {
        record_table::update_single(self, f)
    }
    /// The sound's state (Rust state, see [`sound_state`]), in step with the memory.
    fn sound_state(&mut self) -> sound_state::SoundState {
        record_table::single(self)
    }
    /// The sound's state changed by `f` (written through).
    fn update_sound_state(&mut self, f: impl FnOnce(&mut sound_state::SoundState)) {
        record_table::update_single(self, f)
    }
    /// The video playing (Rust state, see [`video_state`]), in step with the memory.
    fn video_state(&mut self) -> video_state::VideoState {
        record_table::single(self)
    }
    /// The video playing changed by `f` (written through).
    fn update_video_state(&mut self, f: impl FnOnce(&mut video_state::VideoState)) {
        record_table::update_single(self, f)
    }
    /// The player's travel (Rust state, see [`travel`]), in step with the memory.
    fn travel(&mut self) -> travel::Travel {
        record_table::single(self)
    }
    /// The player's travel changed by `f` (written through).
    fn update_travel(&mut self, f: impl FnOnce(&mut travel::Travel)) {
        record_table::update_single(self, f)
    }
    /// The font's pen (Rust state, see [`font_pen`]), in step with the memory.
    fn font_pen(&mut self) -> font_pen::FontPen {
        record_table::single(self)
    }
    /// The font's pen changed by `f` (written through).
    fn update_font_pen(&mut self, f: impl FnOnce(&mut font_pen::FontPen)) {
        record_table::update_single(self, f)
    }
    /// Voice buffer `i` (0, 1; Rust state, see [`voice_buffers`]), in
    /// step with the memory.
    fn voice_buffer(&mut self, i: usize) -> voice_buffers::VoiceBuffer {
        record_table::nth(self, i)
    }
    /// Voice buffer `i` changed by `f` (written through).
    fn update_voice_buffer(&mut self, i: usize, f: impl FnOnce(&mut voice_buffers::VoiceBuffer)) {
        record_table::update_nth(self, i, f)
    }
    /// Character `i` (Rust state, see [`characters`]), in step with the
    /// memory.
    fn character(&mut self, i: usize) -> characters::Character {
        record_table::nth(self, i)
    }
    /// Character `i` changed by `f` (written through).
    fn update_character(&mut self, i: usize, f: impl FnOnce(&mut characters::Character)) {
        record_table::update_nth(self, i, f)
    }
    /// Where the player is (Rust state, see [`player`]), in step with the memory.
    fn player(&mut self) -> player::PlayerAt {
        record_table::single(self)
    }
    /// Where the player is changed by `f` (written through).
    fn update_player(&mut self, f: impl FnOnce(&mut player::PlayerAt)) {
        record_table::update_single(self, f)
    }
    /// Smuggler `i` (Rust state, see [`smugglers`]), in step with the
    /// memory.
    fn smuggler(&mut self, i: usize) -> smugglers::Smuggler {
        record_table::nth(self, i)
    }
    /// Smuggler `i` changed by `f` (written through).
    fn update_smuggler(&mut self, i: usize, f: impl FnOnce(&mut smugglers::Smuggler)) {
        record_table::update_nth(self, i, f)
    }
    /// The story's progress (Rust state, see [`progress`]), in step with the memory.
    fn progress(&mut self) -> progress::Progress {
        record_table::single(self)
    }
    /// The story's progress changed by `f` (written through).
    fn update_progress(&mut self, f: impl FnOnce(&mut progress::Progress)) {
        record_table::update_single(self, f)
    }
    /// The troop talked about (Rust state, see [`topic_troop`]), in step with the memory.
    fn topic_troop(&mut self) -> topic_troop::TopicTroop {
        record_table::single(self)
    }
    /// The troop talked about changed by `f` (written through).
    fn update_topic_troop(&mut self, f: impl FnOnce(&mut topic_troop::TopicTroop)) {
        record_table::update_single(self, f)
    }
    /// The place talked about (Rust state, see [`topic_place`]), in step with the memory.
    fn topic_place(&mut self) -> topic_place::TopicPlace {
        record_table::single(self)
    }
    /// The place talked about changed by `f` (written through).
    fn update_topic_place(&mut self, f: impl FnOnce(&mut topic_place::TopicPlace)) {
        record_table::update_single(self, f)
    }
    /// The forces at the place talked about (Rust state, see [`place_forces`]), in step with the memory.
    fn place_forces(&mut self) -> place_forces::PlaceForces {
        record_table::single(self)
    }
    /// The forces at the place talked about changed by `f` (written through).
    fn update_place_forces(&mut self, f: impl FnOnce(&mut place_forces::PlaceForces)) {
        record_table::update_single(self, f)
    }
    /// The spice and the trade (Rust state, see [`economy`]), in step with the memory.
    fn economy(&mut self) -> economy::Economy {
        record_table::single(self)
    }
    /// The spice and the trade changed by `f` (written through).
    fn update_economy(&mut self, f: impl FnOnce(&mut economy::Economy)) {
        record_table::update_single(self, f)
    }
    /// The situation (Rust state, see [`situation`]), in step with the memory.
    fn situation(&mut self) -> situation::Situation {
        record_table::single(self)
    }
    /// The situation changed by `f` (written through).
    fn update_situation(&mut self, f: impl FnOnce(&mut situation::Situation)) {
        record_table::update_single(self, f)
    }
    /// The story sequence (Rust state, see [`sequence`]), in step with the memory.
    fn sequence(&mut self) -> sequence::Sequence {
        record_table::single(self)
    }
    /// The story sequence changed by `f` (written through).
    fn update_sequence(&mut self, f: impl FnOnce(&mut sequence::Sequence)) {
        record_table::update_single(self, f)
    }
    /// The phrase and its voice (Rust state, see [`phrase`]), in step with the memory.
    fn phrase(&mut self) -> phrase::Phrase {
        record_table::single(self)
    }
    /// The phrase and its voice changed by `f` (written through).
    fn update_phrase(&mut self, f: impl FnOnce(&mut phrase::Phrase)) {
        record_table::update_single(self, f)
    }
    /// The sentence being said (Rust state, see [`sentence`]), in step with the memory.
    fn sentence(&mut self) -> sentence::Sentence {
        record_table::single(self)
    }
    /// The sentence being said changed by `f` (written through).
    fn update_sentence(&mut self, f: impl FnOnce(&mut sentence::Sentence)) {
        record_table::update_single(self, f)
    }
    /// The game time ([2]).
    fn game_time(&mut self) -> u16 {
        clock::time(self)
    }
    /// The timer's ticks ([0CE7Ah]).
    fn timer(&mut self) -> u16 {
        clock::ticks(self)
    }

    fn code_word(&mut self, off: u16) -> u16 {
        self.m.cpu.read16(&mut self.m.hw, CS, off)
    }

    // --- Stack --------------------------------------------------------

    fn push(&mut self, v: u16) {
        self.m.cpu.push(&mut self.m.hw, v);
    }
    fn pop(&mut self) -> u16 {
        self.m.cpu.pop(&mut self.m.hw)
    }
    /// `push` of each register in `regs`, in order (as the original saves
    /// them; [`Ctx::restore`] gets them back).
    fn save(&mut self, regs: &[usize]) {
        for &r in regs {
            let v = self.reg(r);
            self.push(v);
        }
    }
    /// `pop` into each register in `regs`, last first.
    fn restore(&mut self, regs: &[usize]) {
        for &r in regs.iter().rev() {
            let v = self.pop();
            self.set_reg(r, v);
        }
    }
    /// A word that a `push` then `pop` left in memory `words` below SP:
    /// the original wrote it, so the port does too.
    fn below_sp(&mut self, words: u16, v: u16) {
        let sp = self.reg(SP).wrapping_sub(2 * words);
        self.set_word_in(SS, sp, v);
    }

    // --- String instructions (one element; DF gives the direction) -------

    /// How far SI/DI move per element.
    fn string_step(&self, w: Width) -> u16 {
        let n = if w == B { 1u16 } else { 2 };
        if self.flag(pc::cpu::DF) { n.wrapping_neg() } else { n }
    }
    /// `lodsb` / `lodsw` from `seg`:SI into AL / AX.
    fn lods(&mut self, w: Width, seg: usize) -> u16 {
        let si = self.reg(SI);
        let v = if w == B { self.byte_in(seg, si) as u16 } else { self.word_in(seg, si) };
        if w == B {
            self.set_reg8(r8::AL, v as u8);
        } else {
            self.set_reg(AX, v);
        }
        let step = self.string_step(w);
        self.set_reg(SI, si.wrapping_add(step));
        v
    }
    /// `stosb` / `stosw`: AL / AX to ES:DI.
    fn stos(&mut self, w: Width) {
        let di = self.reg(DI);
        if w == B {
            let al = self.reg8(r8::AL);
            self.m.cpu.write8(&mut self.m.hw, ES, di, al);
            self.wrote(ES, di, 1);
        } else {
            let ax = self.reg(AX);
            self.set_word_in(ES, di, ax);
        }
        let step = self.string_step(w);
        self.set_reg(DI, di.wrapping_add(step));
    }
    /// `movsb` / `movsw` from DS:SI to ES:DI.
    fn movs(&mut self, w: Width) {
        let (si, di) = (self.reg(SI), self.reg(DI));
        if w == B {
            let v = self.byte_in(DS, si);
            self.m.cpu.write8(&mut self.m.hw, ES, di, v);
            self.wrote(ES, di, 1);
        } else {
            let v = self.word_in(DS, si);
            self.set_word_in(ES, di, v);
        }
        let step = self.string_step(w);
        self.set_reg(SI, si.wrapping_add(step));
        self.set_reg(DI, di.wrapping_add(step));
    }
    /// `rep movsw` and the like: the CPU's own, which counts each element
    /// as an instruction (the `rep` itself is in the stretch before).
    fn rep(&mut self, op: u8) {
        let di = self.reg(DI);
        self.m.cpu.string_op(&mut self.m.hw, op, None, 0xf3);
        // `movs` and `stos` wrote ES:DI on: the tables there follow.
        if matches!(op, 0xa4 | 0xa5 | 0xaa | 0xab) {
            let (end, size) = (self.reg(DI), if op & 1 == 0 { 1 } else { 2 });
            let (from, len) = if self.flag(pc::cpu::DF) {
                (end.wrapping_add(size), di.wrapping_sub(end))
            } else {
                (di, end.wrapping_sub(di))
            };
            if len != 0 {
                self.wrote(ES, from, len);
            }
        }
    }
    /// A `rep` string instruction whose iterations the `run` before it
    /// already counted (`c.run(at, n + iterations)`): its interrupt check
    /// covers them, as the original's check does between iterations.
    fn rep_counted(&mut self, op: u8) {
        let counted = self.m.cpu.instructions;
        self.rep(op);
        self.m.cpu.instructions = counted;
    }

    // --- Flag-exact operations (the original's instruction) -----------

    fn add(&mut self, w: Width, a: u16, b: u16) -> u16 {
        self.m.cpu.add(w, a as u32, b as u32, false) as u16
    }
    fn sub(&mut self, w: Width, a: u16, b: u16) -> u16 {
        self.m.cpu.sub(w, a as u32, b as u32, false) as u16
    }
    /// `adc`: with the carry flag.
    fn adc(&mut self, w: Width, a: u16, b: u16) -> u16 {
        let cf = self.flag(pc::cpu::CF);
        self.m.cpu.add(w, a as u32, b as u32, cf) as u16
    }
    /// `sbb`: with the carry flag as borrow.
    fn sbb(&mut self, w: Width, a: u16, b: u16) -> u16 {
        let cf = self.flag(pc::cpu::CF);
        self.m.cpu.sub(w, a as u32, b as u32, cf) as u16
    }
    fn cmp(&mut self, w: Width, a: u16, b: u16) {
        self.m.cpu.sub(w, a as u32, b as u32, false);
    }
    fn and(&mut self, w: Width, a: u16, b: u16) -> u16 {
        self.m.cpu.logic(w, (a & b) as u32) as u16
    }
    fn or(&mut self, w: Width, a: u16, b: u16) -> u16 {
        self.m.cpu.logic(w, (a | b) as u32) as u16
    }
    fn xor(&mut self, w: Width, a: u16, b: u16) -> u16 {
        self.m.cpu.logic(w, (a ^ b) as u32) as u16
    }
    fn test(&mut self, w: Width, a: u16, b: u16) {
        self.m.cpu.logic(w, (a & b) as u32);
    }
    fn inc(&mut self, w: Width, a: u16) -> u16 {
        self.m.cpu.inc(w, a as u32) as u16
    }
    fn dec(&mut self, w: Width, a: u16) -> u16 {
        self.m.cpu.dec(w, a as u32) as u16
    }
    fn neg(&mut self, w: Width, a: u16) -> u16 {
        self.m.cpu.sub(w, 0, a as u32, false) as u16
    }
    /// `shl x, cl` (count masked to 5 bits, as the CPU does).
    fn shl(&mut self, w: Width, a: u16, count: u8) -> u16 {
        self.m.cpu.shift(4, w, a as u32, (count & 0x1f) as u32) as u16
    }
    /// `n` times `shl x, 1` (flags differ from one `shl x, n`).
    fn shl1(&mut self, w: Width, a: u16, n: u32) -> u16 {
        (0..n).fold(a, |x, _| self.m.cpu.shift(4, w, x as u32, 1) as u16)
    }
    /// `n` times `shr x, 1`.
    fn shr1(&mut self, w: Width, a: u16, n: u32) -> u16 {
        (0..n).fold(a, |x, _| self.m.cpu.shift(5, w, x as u32, 1) as u16)
    }
    /// `rcr x, 1` and `rcl x, 1`: through the carry flag.
    fn rcr1(&mut self, w: Width, a: u16) -> u16 {
        self.m.cpu.shift(3, w, a as u32, 1) as u16
    }
    fn rcl1(&mut self, w: Width, a: u16) -> u16 {
        self.m.cpu.shift(2, w, a as u32, 1) as u16
    }
    /// `aam base`: AH = AL / base, AL = AL % base; SF, ZF, PF from AL
    /// (as the CPU; `base` is not 0).
    fn aam(&mut self, base: u8) {
        let al = self.reg8(r8::AL);
        self.set_reg8(r8::AH, al / base);
        self.set_reg8(r8::AL, al % base);
        self.m.cpu.set_szp(B, (al % base) as u32);
    }
    /// `rol x, 1` / `ror x, 1`.
    fn rol1(&mut self, w: Width, a: u16) -> u16 {
        self.m.cpu.shift(0, w, a as u32, 1) as u16
    }
    fn ror1(&mut self, w: Width, a: u16) -> u16 {
        self.m.cpu.shift(1, w, a as u32, 1) as u16
    }
    /// `mul r8`: AX = AL × v.
    fn mul8(&mut self, v: u8) {
        self.m.cpu.mul_div(&mut self.m.hw, B, 4, v as u32);
    }
    /// `mul r16`: DX:AX = AX × v.
    fn mul16(&mut self, v: u16) {
        self.m.cpu.mul_div(&mut self.m.hw, W, 4, v as u32);
    }

    /// `STI`: interrupts on after the next instruction (the hold the run
    /// loop and the generated code keep; `rec_tick` ends it).
    fn sti(&mut self) {
        self.set_flag(pc::cpu::IF, true);
        self.m.cpu.irq_inhibit = true;
    }

    // --- Ports ----------------------------------------------------------

    /// `out port, v`, at the time of its instruction.
    fn out(&mut self, port: u16, v: u8) {
        self.m.hw.now_ns = self.m.now_ns();
        self.m.hw.out8(port, v);
    }

    /// `in al, port`, at the time of its instruction.
    fn inp(&mut self, port: u16) -> u8 {
        self.m.hw.now_ns = self.m.now_ns();
        self.m.hw.in8(port)
    }

    // --- Control --------------------------------------------------------

    /// `CALL target` from the instruction ending at `ret` (the original's
    /// return address goes on the stack: the callee may look at it).
    fn call(&mut self, target: u16, ret: u16) -> Result<(), Bail> {
        self.boundary();
        self.push(ret);
        if let Some(r) = self.m.rec.as_mut() {
            r.from_port = true;
        }
        let r = self.m.rec_call(target, ret);
        self.after_callee();
        r
    }

    /// The instruction at `ip`, which ends at `next`, through the
    /// interpreter: `INT` (BIOS, DOS, EMS) and far calls (the drivers'
    /// tables). It counts itself, so the caller's `run` does not cover it:
    /// an event due at it is checked here, before it (the generated code's
    /// block check counts it; the plain emulator takes the event first).
    fn interp(&mut self, ip: u16, next: u16) -> Result<(), Bail> {
        self.boundary();
        self.m.cpu.ip = ip;
        if self.m.rec_at(self.in_place())? {
            self.m.cpu.ip = ip;
            return Err(Bail);
        }
        let r = self.m.rec_interp(ip, next);
        self.after_callee();
        r
    }

    /// A `JMP` to `target` (counted by the caller's run): the routine
    /// there when the table has one, else the generated code from there
    /// (any block leader resumes the routine it belongs to, up to its
    /// `ret`), else the interpreter from there.
    fn jump(&mut self, target: u16) -> Result<(), Bail> {
        self.boundary();
        REACHED[self.entry as usize].fetch_add(1, Ordering::Relaxed);
        self.m.cpu.ip = target;
        self.m.rec_settle()?;
        match self.m.rec.as_ref().and_then(|r| r.table[target as usize].or(r.resume[target as usize])) {
            Some(f) => f(self.m, target),
            None => Err(Bail),
        }
    }

    /// `IRET`: IP, CS and the flags from the stack.
    fn iret(&mut self) -> Result<(), Bail> {
        self.boundary();
        REACHED[self.entry as usize].fetch_add(1, Ordering::Relaxed);
        let ip = self.pop();
        let cs = self.pop();
        let f = self.pop();
        self.m.cpu.ip = ip;
        self.m.cpu.sregs[pc::cpu::CS] = cs;
        self.m.cpu.set_flags(f);
        Ok(())
    }

    /// `RET`.
    fn ret(&mut self) -> Result<(), Bail> {
        self.boundary();
        REACHED[self.entry as usize].fetch_add(1, Ordering::Relaxed);
        self.m.cpu.ip = self.pop();
        Ok(())
    }
}

/// Bail unless entered at `entry` (a port only starts at its entry; the
/// generated code resumes inside it).
fn entered(start: u16, entry: u16) -> Result<(), Bail> {
    if start == entry { Ok(()) } else { Err(Bail) }
}

/// The first address in both sorted lists.
fn all_common(a: &[u32], b: &[u32]) -> Vec<u32> {
    let (mut i, mut j, mut v) = (0, 0, Vec::new());
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                v.push(a[i]);
                i += 1;
                j += 1;
            }
        }
    }
    v
}

fn first_common(a: &[u32], b: &[u32]) -> Option<u32> {
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => return Some(a[i]),
        }
    }
    None
}
