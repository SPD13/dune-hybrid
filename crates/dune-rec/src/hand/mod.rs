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

use pc::{
    Machine,
    cpu::{AX, Bus, CS, DI, DS, ES, SI, SP, SS, Width},
    rec::{Bail, Routine},
};

mod audio;
mod dos;
mod draw;
mod files;
mod frame;
mod gfx;
mod globals;
mod globe;
mod hnm;
mod hsq;
mod input;
mod intro;
mod mouse;
mod panels;
mod phases;
mod math;
mod records;
mod resources;
mod screen;
mod script;
mod sound;
mod sprites;
mod story;
mod tasks;
mod text;
mod time;
mod ui;
mod video;
mod voice;

/// The ports, by entry offset, with the listing's names.
pub static ROUTINES: &[(u16, Routine, &str)] = &[
    // Stubs: a bare `ret`.
    (0x0f66, story::nothing, "sub_0f66"),
    (0x10b7, story::nothing, "sub_10b7"),
    (0x1dd3, story::nothing, "sub_1dd3"),
    (0xd917, story::nothing, "sub_d917"),
    // Story, intro, flags.
    (0x0945, story::intro_script_set_offset, "intro_script_set_offset"),
    (0x10b2, story::set_flag_0ffb, "sub_10b2"),
    (0x1127, story::set_flag_0fdb, "sub_1127"),
    (0x112d, story::set_bit_1067, "sub_112d"),
    (0x121f, story::set_game_phase, "set_game_phase"),
    (0xa1d6, story::clear_flag_47a5, "sub_a1d6"),
    (0xa5aa, story::clear_flag_28be, "sub_a5aa"),
    (0x1011, story::sub_1011, "sub_1011"),
    (0x117b, story::sub_117b, "sub_117b"),
    (0x11b3, story::sub_11b3, "sub_11b3"),
    // The intro's steps.
    (0x0579, intro::clear_global_y_offset, "clear_global_y_offset"),
    (0x06aa, intro::intro_play_hnm_86_frames, "intro_play_hnm_86_frames"),
    (0x06bd, intro::intro_play_hnm_skippable, "intro_play_hnm_skippable"),
    (0x06fc, intro::sub_06fc, "sub_06fc"),
    (0x0704, intro::intro_play_hnm_with_frame_task, "intro_play_hnm_with_frame_task"),
    (0x070c, intro::sub_070c, "sub_070c"),
    (0x078d, intro::sub_078d, "sub_078d"),
    (0x0798, intro::intro_talking_head_play, "intro_talking_head_play"),
    (0x085d, intro::intro_play_desert_sky_animation, "intro_play_desert_sky_animation"),
    (0x0868, intro::intro_midnight, "intro_midnight"),
    (0x087b, intro::sub_087b, "sub_087b"),
    (0x0625, intro::play_virgin_hnm, "play_VIRGIN_HNM"),
    (0x0661, intro::play_cryo_hnm, "play_CRYO_OR_CRYO2_HNM"),
    (0x0684, intro::play_present_hnm, "play_PRESENT_HNM"),
    (0x0737, intro::sub_0737, "sub_0737"),
    (0x073c, intro::sub_073c, "sub_073c"),
    (0x0740, intro::sub_0740, "sub_0740"),
    (0x0747, intro::sub_0747, "sub_0747"),
    (0x0752, intro::sub_0752, "sub_0752"),
    (0x0756, intro::sub_0756, "sub_0756"),
    (0x075a, intro::sub_075a, "sub_075a"),
    (0x075e, intro::sub_075e, "sub_075e"),
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
    (0x1053, phases::sub_1053, "sub_1053"),
    (0x10b8, phases::sub_10b8, "sub_10b8"),
    (0x10be, phases::sub_10be, "sub_10be"),
    (0x1133, phases::sub_1133, "sub_1133"),
    (0x1139, phases::sub_1139, "sub_1139"),
    (0x1196, phases::sub_1196, "sub_1196"),
    (0x425b, phases::sub_425b, "sub_425b"),
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
    (0xdb44, math::shl_cx_and_dx, "shl_cx_and_dx"),
    (0xe16f, math::nearness_e16f, "sub_e16f"),
    (0xe18c, math::nearness_e18c, "sub_e18c"),
    (0xe192, math::nearness_e192, "sub_e192"),
    (0xe19b, math::nearness_e19b, "sub_e19b"),
    (0xe270, math::save_regs, "save_regs"),
    (0xe283, math::restore_regs, "restore_regs"),
    (0xe3b7, math::rand_masked, "rand_masked"),
    (0xe3cc, math::rand, "rand"),
    // Records (locations, map objects).
    (0x01e0, records::sub_01e0, "sub_01e0"),
    (0x3a95, records::get_orni_position, "get_orni_position"),
    (0x407e, records::get_map_position, "get_map_position"),
    (0x40c9, records::sub_40c9, "sub_40c9"),
    (0x5082, records::sub_5082, "sub_5082"),
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
    // (0xc102, screen::copy_pal_and_transition, "copy_pal_and_transition"),
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
    (0x19fc, ui::sub_19fc, "sub_19fc"),
    (0x1a9b, ui::draw_moon_and_sun, "draw_moon_and_sun"),
    (0x1ae7, ui::sub_1ae7, "sub_1ae7"),
    (0x1dda, ui::sub_1dda, "sub_1dda"),
    (0x1dfe, ui::sub_1dfe, "sub_1dfe"),
    (0x274e, ui::comm_draw_panels, "comm_draw_panels"),
    (0x275f, ui::comm_draw_glow, "comm_draw_glow"),
    // Unverified (never called to completion in the checking runs):
    // (0x2795, ui::comm_fade_in_glow, "comm_fade_in_glow"),
    // Unverified (never called to completion in the checking runs):
    // (0x281c, ui::comm_animate_incoming_call, "comm_animate_incoming_call"),
    (0x2d74, ui::open_sal_resource, "open_SAL_resource"),
    (0x388d, ui::set_sky_palette, "set_sky_palette"),
    (0x38b4, ui::draw_sky, "draw_sky"),
    (0x39b9, ui::sub_39b9, "loc_039b9"),
    (0x3e13, ui::draw_gradient_section, "draw_gradient_section"),
    (0x4ad7, ui::sub_4ad7, "sub_4ad7"),
    (0x5a3d, ui::ui_transition_to_map_interface, "ui_transition_to_map_interface"),
    (0x629d, ui::draw_string_location_type, "draw_string_location_type"),
    (0x62a6, ui::draw_location_name, "draw_location_name"),
    (0x6f93, ui::decrease_troop_motivation, "decrease_troop_motivation"),
    (0xae04, ui::sub_ae04, "loc_0ae04"),
    (0xd95b, ui::sub_d95b, "loc_0d95b"),
    // Resources, conditions.
    (0xa396, resources::evaluate_condition_ax, "evaluate_condition_ax"),
    (0xc13b, resources::open_onmap_resource, "open_onmap_resource"),
    (0xc13e, resources::open_resource_by_index, "open_resource_by_index"),
    // More of the game screen.
    (0x0b21, panels::sub_0b21, "loc_00b21"),
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
    (0xaded, panels::sub_aded, "loc_0aded"),
    (0xc0d5, panels::sub_c0d5, "loc_0c0d5"),
    (0xc85b, panels::sub_c85b, "loc_0c85b"),
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
    (0xb93b, globe::sub_b93b, "loc_0b93b"),
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
    (0xc827, frame::sub_c827, "sub_c827"),
    (0xb23f, frame::sub_b23f, "sub_b23f"),
    (0xb270, frame::handle_ctrl_v_once, "handle_ctrl_v_once"),
    // Drawing sprites, UI elements, screen updates, strings.
    (0xbfa7, draw::string_update_ordinal_suffix, "string_update_ordinal_suffix"),
    (0xc137, draw::load_icones_sprites, "load_icones_sprites"),
    (0xc22f, draw::draw_sprite_clobbering_bx_dx, "draw_sprite_clobbering_bx_dx"),
    (0xc4dd, draw::sub_c4dd, "loc_0c4dd"),
    (0xc4ed, draw::update_screen_at_sprite_rect_updating_head, "update_screen_at_sprite_rect_updating_head"),
    (0xc4f0, draw::draw_head_if_needed_and_update_screen_rect_at_si, "draw_head_if_needed_and_update_screen_rect_at_si"),
    (0xc4fb, draw::draw_head_if_needed_and_update_screen_rect, "draw_head_if_needed_and_update_screen_rect"),
    (0xd200, draw::draw_ui_element, "draw_ui_element"),
    (0xe75b, draw::res_store_in_lookup_table, "res_store_in_lookup_table"),
    // Input.
    (0xdaa3, input::clear_some_mouse_rect, "clear_some_mouse_rect"),
    (0xdaaa, input::set_some_mouse_rect, "set_some_mouse_rect"),
    (0xdb4c, input::mouse_stuff, "mouse_stuff"),
    (0xdd5a, input::get_and_reset_key_scancode, "get_and_reset_key_scancode"),
    (0xde4e, input::kb_clear_scancode, "kb_clear_scancode"),
    (0xde54, input::kb_check_for_esc_key_hit, "kb_check_for_esc_key_hit"),
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
/// operations, calls, and the instruction count.
struct Ctx<'a> {
    m: &'a mut Machine,
}

#[allow(dead_code)]
impl Ctx<'_> {
    fn new(m: &mut Machine) -> Ctx<'_> {
        Ctx { m }
    }

    // --- Virtual time -------------------------------------------------

    /// Start a straight-line stretch of `insns` original instructions from
    /// offset `at`. Bails there, before doing anything, if an interrupt (or
    /// the end of a shadow check) falls inside it, so the interpreter takes
    /// over at the exact instruction; else counts them.
    fn run(&mut self, at: u16, insns: u32) -> Result<(), Bail> {
        self.check(at, insns)?;
        self.count(insns);
        Ok(())
    }

    /// [`Ctx::run`]'s check alone, for stretches that do port I/O: they
    /// count their instructions as they go ([`Ctx::count`]), so the device
    /// sees the time of each `in`/`out`.
    fn check(&mut self, at: u16, insns: u32) -> Result<(), Bail> {
        if self.m.rec_tick(insns) {
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
    }
    fn set_word(&mut self, off: u16, v: u16) {
        self.m.cpu.write16(&mut self.m.hw, DS, off, v);
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
    }
    /// Word at `segment:off` for a segment value no register holds.
    fn word_at(&mut self, segment: u16, off: u16) -> u16 {
        let lo = self.m.cpu.phys(segment, off);
        let hi = self.m.cpu.phys(segment, off.wrapping_add(1));
        self.m.hw.read8(lo) as u16 | (self.m.hw.read8(hi) as u16) << 8
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
        self.m.cpu.string_op(&mut self.m.hw, op, None, 0xf3);
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
    /// `mul r16`: DX:AX = AX × v.
    fn mul16(&mut self, v: u16) {
        self.m.cpu.mul_div(&mut self.m.hw, W, 4, v as u32);
    }

    // --- Ports ----------------------------------------------------------

    /// `out port, v`, at the time of its instruction.
    fn out(&mut self, port: u16, v: u8) {
        self.m.hw.now_ns = self.m.now_ns();
        self.m.hw.out8(port, v);
    }

    // --- Control --------------------------------------------------------

    /// `CALL target` from the instruction ending at `ret` (the original's
    /// return address goes on the stack: the callee may look at it).
    fn call(&mut self, target: u16, ret: u16) -> Result<(), Bail> {
        self.push(ret);
        self.m.rec_call(target, ret)
    }

    /// The instruction at `ip`, which ends at `next`, through the
    /// interpreter: `INT` (BIOS, DOS, EMS) and far calls (the drivers'
    /// tables). It counts itself.
    fn interp(&mut self, ip: u16, next: u16) -> Result<(), Bail> {
        self.m.rec_interp(ip, next)
    }

    /// `RET`.
    fn ret(&mut self) -> Result<(), Bail> {
        self.m.cpu.ip = self.pop();
        Ok(())
    }
}

/// Bail unless entered at `entry` (a port only starts at its entry; the
/// generated code resumes inside it).
fn entered(start: u16, entry: u16) -> Result<(), Bail> {
    if start == entry { Ok(()) } else { Err(Bail) }
}
