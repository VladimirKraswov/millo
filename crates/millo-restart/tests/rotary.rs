use millo_gcode::{ProgramParseRequest, ProgramWorkCoordinateSystem, parse_program};
use millo_restart::{
    RotaryRestartError, RotaryRestartState, SafeStartError, SafeStartIntent, SafeStartRequest,
    build_safe_start, build_safe_start_with_rotary,
};

fn state() -> RotaryRestartState {
    RotaryRestartState {
        work_a_degrees: 100.0,
        work_offset_a_degrees: 12.0,
        reference_work_offset_a_degrees: 12.0,
        initial_work_a_degrees: 20.0,
        work_coordinate_system: ProgramWorkCoordinateSystem::G54,
        clearance_confirmed: true,
    }
}

#[test]
fn validated_rotary_restart_indexes_at_clearance_and_restores_units_feed_and_distance() {
    let source = "G20 G90 G93 G54\nG0 X0 Y0 Z1 A90\nG91 G1 X1 A30 F2\nG1 X1 A30 F3";
    let program = parse_program(ProgramParseRequest {
        source_name: "rotary.nc".into(),
        source: source.into(),
    })
    .unwrap();
    let request = SafeStartRequest {
        selected_source_line: 4,
        safe_z_mm: 30.0,
        intent: SafeStartIntent::AirRun,
    };
    assert_eq!(
        build_safe_start(&program, source, request).unwrap_err(),
        SafeStartError::Rotary(RotaryRestartError::StateRequired)
    );
    let package = build_safe_start_with_rotary(&program, source, request, Some(state())).unwrap();
    assert_eq!(package.restart_a_degrees, Some(120.0));
    assert!(
        package
            .request
            .source
            .contains("G0 Z30.0000\nG0 A120\nG0 X25.4000 Y0.0000\nG0 Z25.4000")
    );
    assert!(
        package
            .request
            .source
            .contains("G20 G91 G91.1 G93 G17 G1 F2.0000\nG1 X1 A30 F3")
    );
    assert!(!package.request.source.contains("M3"));
    let reparsed = parse_program(package.request).unwrap();
    assert_eq!(
        reparsed
            .toolpath
            .last()
            .unwrap()
            .rotary
            .unwrap()
            .end_degrees,
        150.0
    );
    assert_eq!(
        reparsed.toolpath.last().unwrap().estimated_duration_seconds,
        Some(20.0)
    );
}

#[test]
fn relative_initial_a_is_resolved_against_verified_start_angle_and_wco_is_bound() {
    let source = "G21 G90 G93\nG0 X0 Y0 Z5\nG91 G1 A30 F2\nG1 A10 F2";
    let program = parse_program(ProgramParseRequest {
        source_name: "relative.nc".into(),
        source: source.into(),
    })
    .unwrap();
    let request = SafeStartRequest {
        selected_source_line: 4,
        safe_z_mm: 8.0,
        intent: SafeStartIntent::Cutting,
    };
    let package = build_safe_start_with_rotary(&program, source, request, Some(state())).unwrap();
    assert_eq!(package.restart_a_degrees, Some(50.0));
    let mut invalid = state();
    invalid.work_offset_a_degrees = 13.0;
    assert_eq!(
        build_safe_start_with_rotary(&program, source, request, Some(invalid)).unwrap_err(),
        SafeStartError::Rotary(RotaryRestartError::WorkOffsetChanged)
    );
    invalid = state();
    invalid.clearance_confirmed = false;
    assert_eq!(
        build_safe_start_with_rotary(&program, source, request, Some(invalid)).unwrap_err(),
        SafeStartError::Rotary(RotaryRestartError::InvalidState)
    );
}

#[test]
fn rotary_restart_rejects_implicit_or_only_incremental_cartesian_anchors() {
    for source in [
        "G21 G90 G93\nG1 A90 F2\nG1 A180 F2",
        "G21 G91 G93\nG0 X10 Y10 Z5\nG1 A90 F2",
        "G21 G90 G93\n/G0 X0 Y0 Z5\nG1 A90 F2",
    ] {
        let program = millo_gcode::parse_program_with_options(
            ProgramParseRequest {
                source_name: "unknown-xyz.nc".into(),
                source: source.into(),
            },
            millo_gcode::ProgramParseOptions {
                block_delete: source.contains('/'),
            },
        )
        .unwrap();
        let error = build_safe_start_with_rotary(
            &program,
            source,
            SafeStartRequest {
                selected_source_line: 3,
                safe_z_mm: 8.0,
                intent: SafeStartIntent::Cutting,
            },
            Some(state()),
        )
        .unwrap_err();
        assert_eq!(
            error,
            SafeStartError::Rotary(RotaryRestartError::CartesianAnchorUnknown)
        );
    }
}
