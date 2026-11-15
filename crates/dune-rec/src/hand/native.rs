//! The Rust state the recompiled code keeps with a machine
//! ([`Machine::native`]): the subsystems moved out of the 8086 memory
//! (Stage B of `docs/readable-port-plan.md`), each kept in step with the
//! memory, which stays their mirror.

use pc::Machine;

/// The subsystems' state.
#[derive(Default)]
pub struct Native {
    pub clock: super::clock::ClockState,
    pub troops: super::record_table::RecordTable,
    pub locations: super::record_table::RecordTable,
    pub text_box: super::record_table::RecordTable,
    pub dialogue_text: super::record_table::RecordTable,
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
