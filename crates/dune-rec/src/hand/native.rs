//! The Rust state the recompiled code keeps with a machine
//! ([`Machine::native`]): the subsystems moved out of the 8086 memory
//! (Stage B of `docs/readable-port-plan.md`), each kept in step with the
//! memory, which stays their mirror.

use pc::Machine;

/// The subsystems' state.
#[derive(Default)]
pub struct Native {
    pub clock: super::clock::ClockState,
    pub save_image: super::save_image::SaveImageState,
    pub save_file: super::save_file::SaveFileState,
    pub ecology: super::ecology::EcologyState,
    pub rule_paths: super::rule_paths::RulePaths,
    pub troops: super::record_table::RecordTable,
    pub locations: super::record_table::RecordTable,
    pub text_box: super::record_table::RecordTable,
    pub dialogue_text: super::record_table::RecordTable,
    pub conversation: super::record_table::RecordTable,
    pub map_view: super::record_table::RecordTable,
    pub map_selection: super::record_table::RecordTable,
    pub flight: super::record_table::RecordTable,
    pub typed_message: super::record_table::RecordTable,
    pub screen_buffers: super::record_table::RecordTable,
    pub mouse_state: super::record_table::RecordTable,
    pub sound_state: super::record_table::RecordTable,
    pub video_state: super::record_table::RecordTable,
    pub travel: super::record_table::RecordTable,
    pub font_pen: super::record_table::RecordTable,
    pub voice_buffers: super::record_table::RecordTable,
    pub characters: super::record_table::RecordTable,
    pub player: super::record_table::RecordTable,
    pub smugglers: super::record_table::RecordTable,
    pub progress: super::record_table::RecordTable,
    pub topic_troop: super::record_table::RecordTable,
    pub topic_place: super::record_table::RecordTable,
    pub place_forces: super::record_table::RecordTable,
    pub economy: super::record_table::RecordTable,
    pub situation: super::record_table::RecordTable,
    pub sequence: super::record_table::RecordTable,
    pub phrase: super::record_table::RecordTable,
    pub sentence: super::record_table::RecordTable,
    pub messages: super::record_table::RecordTable,
    pub phrase_args: super::record_table::RecordTable,
}

/// The machine's Rust state (made on first use).
pub fn native(m: &mut Machine) -> &mut Native {
    m.native.get_or_insert_with(|| Box::new(Native::default())).downcast_mut::<Native>().expect("native state")
}

/// The machine ran outside the recompiled code on purpose (a tool
/// stepping it in the interpreter, a snapshot restored), or a save was
/// loaded: every subsystem takes the memory's values (not counted as a
/// sync-in).
pub fn outside(m: &mut Machine) {
    super::clock::outside(m);
    super::record_table::outside(m);
}

/// A snapshot restored: every subsystem takes the memory's state whole.
pub fn restored(m: &mut Machine) {
    super::clock::outside(m);
    super::record_table::loaded(m);
}
