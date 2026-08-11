use millo_gcode::{
    ProgramParseError, ProgramParseRequest, ProgramWarningCode, ProgramWarningSeverity,
    ToolpathKind, parse_program,
};

fn parse_fixture(name: &str, source: &str) -> millo_gcode::GcodeProgram {
    parse_program(ProgramParseRequest {
        source_name: name.to_owned(),
        source: source.to_owned(),
    })
    .unwrap()
}

#[test]
fn parses_metric_compact_words_comments_and_xy_arcs() {
    let program = parse_fixture(
        "metric-profile.nc",
        include_str!("fixtures/metric-profile.nc"),
    );

    assert_eq!(program.summary.line_count, 10);
    assert_eq!(program.summary.motion_count, 6);
    assert_eq!(program.toolpath[0].kind, ToolpathKind::Rapid);
    assert_eq!(program.toolpath[4].kind, ToolpathKind::ArcClockwise);
    assert_eq!(program.toolpath[5].kind, ToolpathKind::ArcCounterclockwise);
    let bounds = program.summary.bounds.unwrap();
    assert!((bounds.min.x - 0.0).abs() < 0.001);
    assert!((bounds.max.x - 20.0).abs() < 0.001);
    assert!((bounds.max.y - 10.0).abs() < 0.001);
    assert!(program.summary.preview_complete);
    assert!(program.summary.dry_run_eligible);
    assert!(program.toolpath[4].points.len() > 10);
}

#[test]
fn accepts_common_program_headers_and_modal_cancels() {
    let program = parse_fixture(
        "common-header.ngc",
        include_str!("fixtures/common-header.ngc"),
    );

    assert!(program.warnings.is_empty());
    assert!(program.summary.preview_complete);
    assert!(program.summary.dry_run_eligible);
    assert_eq!(program.summary.motion_count, 2);
}

#[test]
fn accepts_percent_program_delimiters_as_non_executable_lines() {
    let program = parse_fixture("delimited.nc", "%\nG21 G90 G94\nG1 X1 F10\nM30\n%");

    assert!(program.warnings.is_empty());
    assert!(!program.lines[0].executable);
    assert!(!program.lines[4].executable);
    assert_eq!(program.summary.executable_line_count, 3);
}

#[test]
fn converts_imperial_incremental_motion_to_millimeters() {
    let program = parse_fixture(
        "imperial-incremental.tap",
        include_str!("fixtures/imperial-incremental.tap"),
    );

    let bounds = program.summary.bounds.unwrap();
    assert!((bounds.max.x - 25.4).abs() < 0.001);
    assert!((bounds.max.y - 12.7).abs() < 0.001);
    assert!(program.features.uses_imperial_units);
    assert!(program.features.uses_incremental_distance);
    assert_eq!(program.summary.motion_count, 2);
}

#[test]
fn surfaces_safety_commands_without_exposing_them_as_preview_motion() {
    let program = parse_fixture(
        "operator-review.nc",
        include_str!("fixtures/operator-review.nc"),
    );

    assert!(program.features.has_spindle_activation);
    assert!(program.features.has_spindle_speed);
    assert!(program.features.has_tool_change);
    assert!(program.features.has_probe_cycle);
    assert!(program.features.has_machine_coordinate_move);
    assert!(!program.summary.dry_run_eligible);
    assert!(!program.summary.preview_complete);
    for expected in [
        ProgramWarningCode::SpindleActivation,
        ProgramWarningCode::SpindleSpeed,
        ProgramWarningCode::ToolChange,
        ProgramWarningCode::UnsafeMachineCommand,
    ] {
        assert!(
            program
                .warnings
                .iter()
                .any(|warning| warning.code == expected)
        );
    }
    assert_eq!(program.summary.motion_count, 1);
}

#[test]
fn malformed_input_is_loaded_with_line_addressable_warnings() {
    let program = parse_fixture("malformed.nc", include_str!("fixtures/malformed.nc"));

    assert!(!program.summary.dry_run_eligible);
    assert!(program.warnings.iter().any(|warning| {
        warning.source_line == 2 && warning.code == ProgramWarningCode::UnclosedComment
    }));
    assert!(program.warnings.iter().any(|warning| {
        warning.source_line == 3 && warning.code == ProgramWarningCode::InvalidToken
    }));
    assert!(program.warnings.iter().any(|warning| {
        warning.source_line == 4 && warning.code == ProgramWarningCode::ArcDefinition
    }));
}

#[test]
fn rejects_missing_identity_empty_source_and_oversized_input() {
    assert_eq!(
        parse_program(ProgramParseRequest {
            source_name: " ".to_owned(),
            source: "G0 X1".to_owned(),
        }),
        Err(ProgramParseError::MissingSourceName)
    );
    assert!(matches!(
        parse_program(ProgramParseRequest {
            source_name: "x".repeat(millo_gcode::MAX_SOURCE_NAME_BYTES + 1),
            source: "G0 X1".to_owned(),
        }),
        Err(ProgramParseError::SourceNameTooLong { .. })
    ));
    assert_eq!(
        parse_program(ProgramParseRequest {
            source_name: "empty.nc".to_owned(),
            source: "\n  ".to_owned(),
        }),
        Err(ProgramParseError::EmptySource)
    );
    assert!(matches!(
        parse_program(ProgramParseRequest {
            source_name: "huge.nc".to_owned(),
            source: " ".repeat(millo_gcode::MAX_SOURCE_BYTES + 1),
        }),
        Err(ProgramParseError::SourceTooLarge { .. })
    ));
}

#[test]
fn unknown_m_codes_fail_the_future_dry_run_gate() {
    let program = parse_fixture("unknown-m.nc", "G21\nM10\nG0 X1");

    assert!(!program.summary.dry_run_eligible);
    assert!(program.warnings.iter().any(|warning| {
        warning.code == ProgramWarningCode::UnsupportedMCode
            && warning.severity == ProgramWarningSeverity::Safety
    }));
}
