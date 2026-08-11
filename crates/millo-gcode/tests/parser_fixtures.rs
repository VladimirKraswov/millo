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
fn parses_every_grbl_arc_plane_helices_full_circle_and_dwell() {
    let program = parse_fixture(
        "all-arc-planes.nc",
        include_str!("fixtures/all-arc-planes.nc"),
    );

    assert!(program.warnings.is_empty());
    assert!(program.summary.preview_complete);
    assert!(program.summary.dry_run_eligible);
    assert!(program.summary.time_estimate_complete);
    assert_eq!(program.summary.motion_count, 5);
    assert_eq!(program.toolpath[1].kind, ToolpathKind::ArcCounterclockwise);
    assert_eq!(program.toolpath[2].kind, ToolpathKind::ArcCounterclockwise);
    assert_eq!(program.toolpath[3].kind, ToolpathKind::ArcClockwise);
    assert_eq!(program.toolpath[4].kind, ToolpathKind::ArcClockwise);

    let xy_end = program.toolpath[1].points.last().unwrap();
    assert_point(*xy_end, [20.0, 10.0, 2.0]);
    let xz_end = program.toolpath[2].points.last().unwrap();
    assert_point(*xz_end, [30.0, 12.0, 12.0]);
    let yz_end = program.toolpath[3].points.last().unwrap();
    assert_point(*yz_end, [25.0, 22.0, 22.0]);
    let full_circle = &program.toolpath[4].points;
    assert!(full_circle.len() > 50);
    assert_point(full_circle[0], [25.0, 22.0, 22.0]);
    assert_point(*full_circle.last().unwrap(), [25.0, 22.0, 22.0]);

    let bounds = program.summary.bounds.unwrap();
    assert_point(bounds.min, [0.0, 0.0, 0.0]);
    assert_point(bounds.max, [30.0, 27.0, 22.0]);
    assert!((program.summary.dwell_time_seconds - 0.25).abs() < 1e-9);
    assert!(program.summary.estimated_motion_time_seconds > 44.0);
    assert!(program.summary.estimated_motion_time_seconds < 46.0);
    assert!(
        (program.summary.estimated_total_time_seconds
            - program.summary.estimated_motion_time_seconds
            - 0.25)
            .abs()
            < 1e-9
    );
    assert!(
        program
            .toolpath
            .iter()
            .all(|segment| segment.estimated_duration_seconds.is_some())
    );
}

#[test]
fn supports_inverse_time_and_marks_rapid_or_missing_feed_as_incomplete() {
    let inverse = parse_fixture(
        "inverse-time.nc",
        "G21 G90 G93\nG1 X10 F2\nG1 X20 F4\nG4 P0.5",
    );
    assert!(inverse.warnings.is_empty());
    assert!(inverse.summary.time_estimate_complete);
    assert!((inverse.summary.estimated_motion_time_seconds - 45.0).abs() < 0.001);
    assert!((inverse.summary.estimated_total_time_seconds - 45.5).abs() < 0.001);

    let rapid = parse_fixture("rapid-time.nc", "G21 G90 G94\nG0 X10\nG1 X20 F60");
    assert!(!rapid.summary.time_estimate_complete);
    assert!((rapid.summary.estimated_motion_time_seconds - 10.0).abs() < 0.001);

    let missing_feed = parse_fixture("missing-feed.nc", "G21 G90 G94\nG1 X10");
    assert!(!missing_feed.summary.time_estimate_complete);
    assert!(!missing_feed.summary.dry_run_eligible);
    assert!(missing_feed.warnings.iter().any(|warning| {
        warning.code == ProgramWarningCode::FeedRate
            && warning.severity == ProgramWarningSeverity::Error
    }));
}

#[test]
fn blocks_grbl_incompatible_arc_modes_and_modal_group_conflicts() {
    let absolute_centers = parse_fixture(
        "absolute-centers.nc",
        "G21 G90 G94 G90.1\nG1 X10 F100\nG17 G3 X20 Y10 I10 J10",
    );
    assert!(!absolute_centers.summary.dry_run_eligible);
    assert_eq!(absolute_centers.summary.motion_count, 2);
    assert!(absolute_centers.warnings.iter().any(|warning| {
        warning.code == ProgramWarningCode::UnsupportedGCode && warning.message.contains("G90.1")
    }));

    let conflict = parse_fixture("modal-conflict.nc", "G21 G90 G94\nG0 G1 X1 F10");
    assert!(!conflict.summary.preview_complete);
    assert!(!conflict.summary.dry_run_eligible);
    assert!(conflict.warnings.iter().any(|warning| {
        warning.code == ProgramWarningCode::ModalGroupConflict
            && warning.severity == ProgramWarningSeverity::Error
    }));

    let center_only_arc = parse_fixture(
        "center-only-arc.nc",
        "G21 G90 G94 G17\nG1 X10 F100\nG2 I-5 J0",
    );
    assert!(!center_only_arc.summary.dry_run_eligible);
    assert!(center_only_arc.warnings.iter().any(|warning| {
        warning.code == ProgramWarningCode::ArcDefinition
            && warning.message.contains("explicit X, Y, or Z")
    }));

    for (name, source) in [
        ("linear-offset.nc", "G21 G90 G94\nG1 X1 I0 F10"),
        ("arc-turns.nc", "G21 G90 G94 G17\nG2 X1 I0.5 P2 F10"),
        ("m-conflict.nc", "G21 G90 G94\nM3 M4 S1000\nG1 X1 F10"),
    ] {
        let invalid = parse_fixture(name, source);
        assert!(!invalid.summary.dry_run_eligible, "{name}");
        assert!(
            invalid.warnings.iter().any(|warning| matches!(
                warning.code,
                ProgramWarningCode::UnsupportedWord | ProgramWarningCode::ModalGroupConflict
            )),
            "{name}"
        );
    }
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
    assert!(!program.lines[1].executable);
    assert_eq!(program.lines[1].normalized, "O1001");
}

#[test]
fn fails_closed_instead_of_rewriting_stream_control_syntax() {
    for (name, source, expected) in [
        (
            "optional-block.nc",
            "G21 G90 G94\n/G1 X1 F10",
            ProgramWarningCode::OptionalBlockUnsupported,
        ),
        (
            "checksummed.nc",
            "G21 G90 G94\nN2 G1 X1 F10*42",
            ProgramWarningCode::ChecksumUnsupported,
        ),
    ] {
        let program = parse_fixture(name, source);
        assert!(!program.summary.dry_run_eligible, "{name}");
        assert!(program.warnings.iter().any(|warning| {
            warning.code == expected && warning.severity == ProgramWarningSeverity::Error
        }));
    }
}

#[test]
fn rejects_program_numbers_mixed_with_executable_words() {
    let program = parse_fixture("mixed-program-number.nc", "O1001 G21\nG1 X1 F10");

    assert!(!program.summary.dry_run_eligible);
    assert!(program.warnings.iter().any(|warning| {
        warning.source_line == 1
            && warning.code == ProgramWarningCode::UnsupportedWord
            && warning.message.contains("metadata-only")
    }));
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
fn parses_the_hardware_air_square_with_exact_bounds() {
    let program = parse_fixture(
        "air-square-20mm.nc",
        include_str!("../../../fixtures/programs/air-square-20mm.nc"),
    );

    assert!(program.warnings.is_empty());
    assert!(program.summary.preview_complete);
    assert!(program.summary.dry_run_eligible);
    assert_eq!(program.summary.motion_count, 4);
    assert!(!program.features.has_spindle_activation);
    assert!(!program.features.has_spindle_speed);
    let bounds = program.summary.bounds.unwrap();
    assert_eq!(bounds.min.x, 0.0);
    assert_eq!(bounds.min.y, 0.0);
    assert_eq!(bounds.max.x, 20.0);
    assert_eq!(bounds.max.y, 20.0);
    assert_eq!(bounds.size.z, 0.0);
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

fn assert_point(point: millo_gcode::ProgramPoint, expected: [f64; 3]) {
    assert!((point.x - expected[0]).abs() < 0.001, "x={}", point.x);
    assert!((point.y - expected[1]).abs() < 0.001, "y={}", point.y);
    assert!((point.z - expected[2]).abs() < 0.001, "z={}", point.z);
}
