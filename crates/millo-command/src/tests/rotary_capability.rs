use super::*;
use millo_domain::RotaryAxisProfile;
use millo_gcode::{ProgramParseRequest, parse_program};

fn program(source: &str) -> GcodeProgram {
    parse_program(ProgramParseRequest {
        source_name: "rotary-capability.nc".to_owned(),
        source: source.to_owned(),
    })
    .unwrap()
}

fn profile() -> HardwareProfile {
    let mut profile = HardwareProfile::first_machine();
    profile.axes.push("A".to_owned());
    profile.rotary_axis = Some(RotaryAxisProfile {
        travel_degrees: 720.0,
        max_jog_degrees: 30.0,
        max_feed_degrees_per_min: 720.0,
    });
    profile
}

fn fixture(identity: &str) -> (DeviceInspection, ControllerSnapshot) {
    let inspection = build_device_inspection(vec![CommandResponse {
        command: "$I".to_owned(),
        completion: CommandCompletion::Ok,
        lines: vec!["[AXS:4:XYZA]".to_owned(), identity.to_owned()],
        code: None,
    }]);
    let mut snapshot = ControllerSnapshot::default();
    let position = Position {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        a: Some(0.0),
    };
    snapshot.machine.machine_position = Some(position);
    snapshot.machine.work_position = Some(position);
    snapshot.machine.work_coordinate_offset = Some(position);
    (inspection, snapshot)
}

#[test]
fn program_and_zero_share_strict_firmware_identity() {
    let program = program("G21 G90 G93\nG1 X10 A90 F6");
    for (identity, accepted) in [
        ("[FIRMWARE:grblHAL]", true),
        ("[VER:3.9 FluidNC build:machine]", true),
        ("[FIRMWARE:MilloVirtual]", true),
        ("[VER:1.1h:User named grblHAL FluidNC]", false),
        ("[VER:3.9 FakeFluidNC build:machine]", false),
        ("[FIRMWARE:grblHAL-compatible]", false),
        ("[FIRMWARE:MilloVirtual4AX]", false),
        ("[VER:1.1h:stock GRBL]", false),
    ] {
        let (mut inspection, snapshot) = fixture(identity);
        inspection
            .settings
            .insert("$376".to_owned(), "1".to_owned());
        assert_eq!(
            validate_rotary_capability(&profile(), &inspection, &snapshot).is_ok(),
            accepted,
            "{identity}"
        );
        assert_eq!(
            validate_rotary_program(&program, &profile(), &inspection, &snapshot).is_ok(),
            accepted,
            "{identity}"
        );
    }
}

#[test]
fn only_successful_identity_queries_count_and_conflicts_fail_closed() {
    let program = program("G90 G93\nG1 A90 F6");
    let (mut inspection, snapshot) = fixture("[FIRMWARE:MilloVirtual]");
    inspection.responses[0].command = "$I+".to_owned();
    assert!(validate_rotary_program(&program, &profile(), &inspection, &snapshot).is_ok());
    inspection.responses[0].completion = CommandCompletion::Error;
    assert!(validate_rotary_program(&program, &profile(), &inspection, &snapshot).is_err());
    inspection.responses[0].completion = CommandCompletion::Ok;
    inspection.responses[0].command = "$G".to_owned();
    assert!(validate_rotary_program(&program, &profile(), &inspection, &snapshot).is_err());
    inspection.responses[0].command = "$I".to_owned();
    inspection.responses[0]
        .lines
        .push("[FIRMWARE:grblHAL]".to_owned());
    inspection
        .settings
        .insert("$376".to_owned(), "1".to_owned());
    assert!(validate_rotary_program(&program, &profile(), &inspection, &snapshot).is_err());
}

#[test]
fn grblhal_angular_setting_uses_external_a_bit_not_internal_axis_mask() {
    let program = program("G20 G90 G93\nG1 X1 A90 F6");
    let (mut inspection, snapshot) = fixture("[FIRMWARE:grblHAL]");
    assert!(validate_rotary_program(&program, &profile(), &inspection, &snapshot).is_err());
    for (mask, accepted) in [
        ("1", true),
        ("3", true),
        ("7", true),
        ("0", false),
        ("2", false),
        ("8", false),
        ("NaN", false),
        ("inf", false),
        ("1.5", false),
        ("-1", false),
    ] {
        inspection
            .settings
            .insert("$376".to_owned(), mask.to_owned());
        assert_eq!(
            validate_rotary_program(&program, &profile(), &inspection, &snapshot).is_ok(),
            accepted,
            "$376={mask}"
        );
    }
}

#[test]
fn every_angular_profile_limit_must_be_enabled_finite_and_positive() {
    let program = program("G90 G94\nG1 A90 F60");
    let (inspection, snapshot) = fixture("[FIRMWARE:MilloVirtual]");
    let mut disabled = profile();
    disabled.axes.retain(|axis| axis != "A");
    assert!(validate_rotary_program(&program, &disabled, &inspection, &snapshot).is_err());
    disabled = profile();
    disabled.rotary_axis = None;
    assert!(validate_rotary_program(&program, &disabled, &inspection, &snapshot).is_err());
    for field in 0..3 {
        for value in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let mut invalid = profile();
            let rotary = invalid.rotary_axis.as_mut().unwrap();
            match field {
                0 => rotary.travel_degrees = value,
                1 => rotary.max_jog_degrees = value,
                _ => rotary.max_feed_degrees_per_min = value,
            }
            assert!(validate_rotary_program(&program, &invalid, &inspection, &snapshot).is_err());
        }
    }
}

#[test]
fn all_current_four_vectors_required_and_virtual_needs_explicit_xyza() {
    let program = program("G90 G93\nG1 A90 F6");
    let (mut inspection, snapshot) = fixture("[FIRMWARE:MilloVirtual]");
    for field in 0..3 {
        for a in [None, Some(f64::NAN), Some(f64::INFINITY)] {
            let mut invalid = snapshot.clone();
            let position = match field {
                0 => &mut invalid.machine.machine_position,
                1 => &mut invalid.machine.work_position,
                _ => &mut invalid.machine.work_coordinate_offset,
            };
            position.as_mut().unwrap().a = a;
            let mut missing = invalid.clone();
            match field {
                0 => missing.machine.machine_position = None,
                1 => missing.machine.work_position = None,
                _ => missing.machine.work_coordinate_offset = None,
            }
            assert!(validate_rotary_program(&program, &profile(), &inspection, &invalid).is_err());
            assert!(validate_rotary_program(&program, &profile(), &inspection, &missing).is_err());
        }
    }
    inspection.responses[0].lines.remove(0);
    assert!(validate_rotary_program(&program, &profile(), &inspection, &snapshot).is_err());
    inspection.responses[0].lines[0] = "[VER:3.9 FluidNC build:machine]".to_owned();
    assert!(validate_rotary_program(&program, &profile(), &inspection, &snapshot).is_ok());
    inspection.responses[0].lines.push("[AXS:3:XYZ]".to_owned());
    assert!(validate_rotary_program(&program, &profile(), &inspection, &snapshot).is_err());
}

#[test]
fn rotary_arcs_require_real_firmware_support_but_lines_accept_virtual() {
    let arc = program("G21 G90 G93\nG2 X10 Y0 I5 J0 A90 F6");
    assert!(arc.features.uses_rotary_arc);
    for identity in [
        "[FIRMWARE:grblHAL]",
        "[VER:3.9 FluidNC build:machine]",
        "[FIRMWARE:MilloVirtual]",
    ] {
        let (mut inspection, snapshot) = fixture(identity);
        inspection
            .settings
            .insert("$376".to_owned(), "1".to_owned());
        assert_eq!(
            validate_rotary_program(&arc, &profile(), &inspection, &snapshot).is_ok(),
            identity != "[FIRMWARE:MilloVirtual]"
        );
        assert!(
            validate_rotary_program(
                &program("G90 G93\nG1 A90 F6"),
                &profile(),
                &inspection,
                &snapshot
            )
            .is_ok()
        );
    }
}

#[test]
fn xyz_program_does_not_require_rotary_capability() {
    assert!(
        validate_rotary_program(
            &program("G21 G90 G94\nG1 X10 F60"),
            &HardwareProfile::first_machine(),
            &DeviceInspection::default(),
            &ControllerSnapshot::default(),
        )
        .is_ok()
    );
}

#[test]
fn expensive_program_reference_fence_rejects_a_and_epoch_changes() {
    let (_, mut before) = fixture("[FIRMWARE:MilloVirtual]");
    before.connection = ConnectionState::Connected;
    before.machine.mode = MachineMode::Idle;
    assert!(ensure_unchanged_program_reference(&before, &before).is_ok());
    for field in 0..3 {
        let mut after = before.clone();
        match field {
            0 => after.machine.machine_position.as_mut().unwrap().a = Some(90.0),
            1 => after.machine.work_position.as_mut().unwrap().a = Some(90.0),
            _ => after.machine.work_coordinate_offset.as_mut().unwrap().a = Some(90.0),
        }
        assert!(matches!(
            ensure_unchanged_program_reference(&before, &after),
            Err(ArbiterError::FirstCut(
                FirstCutAuthorizationError::ControllerPositionChanged
            ))
        ));
    }
    for reset in [true, false] {
        let mut after = before.clone();
        if reset {
            after.reset_count += 1;
        } else {
            after.reconnect_count += 1;
        }
        assert!(matches!(
            ensure_unchanged_program_reference(&before, &after),
            Err(ArbiterError::FirstCut(
                FirstCutAuthorizationError::ControllerSessionChanged
            ))
        ));
    }
}
