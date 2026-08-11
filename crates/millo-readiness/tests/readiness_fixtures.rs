use millo_domain::{
    CommandResponse, ConnectionState, ControllerSnapshot, HardwareProfile, MachineMode,
    ReadinessLevel,
};
use millo_grbl::build_device_inspection;
use millo_readiness::assess;
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    responses: Vec<CommandResponse>,
}

fn ready_snapshot() -> ControllerSnapshot {
    ControllerSnapshot {
        connection: ConnectionState::Connected,
        machine: millo_domain::MachineState {
            mode: MachineMode::Idle,
            reported_mode: "Idle".to_owned(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn representative_inspection() -> millo_domain::DeviceInspection {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/unhomed_xyz_router.json")).unwrap();
    build_device_inspection(fixture.responses)
}

#[test]
fn known_unhomed_router_is_ready_only_for_a_guarded_test_jog() {
    let report = assess(
        &HardwareProfile::first_machine(),
        &representative_inspection(),
        &ready_snapshot(),
    );

    assert!(report.test_jog_ready);
    assert!(!report.probe_ready);
    assert_eq!(report.blocker_count, 0);
    assert!(report.caution_count >= 4);
    assert!(
        report
            .checks
            .iter()
            .any(|check| { check.id == "modal-units" && check.level == ReadinessLevel::Caution })
    );
}

#[test]
fn enabled_homing_or_limits_conflict_with_the_recorded_hardware() {
    let mut inspection = representative_inspection();
    inspection.settings.insert("$21".to_owned(), "1".to_owned());

    let report = assess(
        &HardwareProfile::first_machine(),
        &inspection,
        &ready_snapshot(),
    );

    assert!(!report.test_jog_ready);
    assert!(report.checks.iter().any(|check| {
        check.id == "unhomed-operation" && check.level == ReadinessLevel::Blocker
    }));
}

#[test]
fn missing_axis_tuning_blocks_motion() {
    let mut inspection = representative_inspection();
    inspection.settings.remove("$122");

    let report = assess(
        &HardwareProfile::first_machine(),
        &inspection,
        &ready_snapshot(),
    );

    assert!(!report.test_jog_ready);
    assert!(report.checks.iter().any(|check| {
        check.id == "axis-acceleration" && check.level == ReadinessLevel::Blocker
    }));
}

#[test]
fn alarm_state_blocks_motion_even_when_static_settings_are_valid() {
    let mut snapshot = ready_snapshot();
    snapshot.machine.mode = MachineMode::Alarm;
    snapshot.machine.reported_mode = "Alarm".to_owned();

    let report = assess(
        &HardwareProfile::first_machine(),
        &representative_inspection(),
        &snapshot,
    );

    assert!(!report.test_jog_ready);
    assert!(
        report.checks.iter().any(|check| {
            check.id == "controller-state" && check.level == ReadinessLevel::Blocker
        })
    );
}

#[test]
fn laser_mode_blocks_the_milling_profile() {
    let mut inspection = representative_inspection();
    inspection.settings.insert("$32".to_owned(), "1".to_owned());

    let report = assess(
        &HardwareProfile::first_machine(),
        &inspection,
        &ready_snapshot(),
    );

    assert!(!report.test_jog_ready);
    assert!(
        report
            .checks
            .iter()
            .any(|check| { check.id == "milling-mode" && check.level == ReadinessLevel::Blocker })
    );
}
