use millo_gcode::{ProgramParseRequest, ProgramWarningCode, parse_program};

fn parse(source: &str) -> millo_gcode::GcodeProgram {
    parse_program(ProgramParseRequest {
        source_name: "geometry.nc".into(),
        source: source.into(),
    })
    .unwrap()
}

#[test]
fn arc_bounds_include_extrema_between_regular_samples_in_every_plane() {
    for (plane, arc) in [
        ("G17", "G3 X10 Y0 I5 J0 F100"),
        ("G18", "G3 Z10 X0 K5 I0 F100"),
        ("G19", "G3 Y10 Z0 J5 K0 F100"),
    ] {
        let program = parse(&format!("G21 G90 G94 {plane}\n{arc}"));
        assert!(program.warnings.is_empty(), "{plane}");
        let bounds = program.summary.bounds.unwrap();
        let low = match plane {
            "G17" => bounds.min.y,
            "G18" => bounds.min.x,
            _ => bounds.min.z,
        };
        assert!((low + 5.0).abs() < 1e-12, "{plane}: {low}");
    }
}

#[test]
fn arc_budget_is_checked_before_sampling_and_reported_once() {
    let source = format!(
        "G21 G90 G94 G17\nG0 X1\n{}M2",
        "G2 X1 I1000000000 F100\n".repeat(2_000)
    );
    let program = parse(&source);
    assert_eq!(program.lines.len(), 2_003);
    assert_eq!(program.toolpath.len(), 1);
    assert!(!program.summary.preview_complete);
    assert!(!program.summary.dry_run_eligible);
    assert!(!program.summary.time_estimate_complete);
    assert_eq!(
        program
            .warnings
            .iter()
            .filter(|warning| warning.code == ProgramWarningCode::PreviewLimit)
            .count(),
        1
    );
    assert_eq!(program.warnings[0].source_line, 3);
}

#[test]
fn arc_radius_tolerance_matches_grbl_for_small_and_large_circles() {
    let small = parse("G21 G90 G94 G17\nG3 X2.004 Y0 I1 J0 F100");
    assert!(small.summary.dry_run_eligible);
    for source in [
        "G3 X2.006 Y0 I1 J0 F100",
        "G3 X200.15 Y0 I100 J0 F100",
        "G3 X2000.6 Y0 I1000 J0 F100",
    ] {
        let invalid = parse(&format!("G21 G90 G94 G17\n{source}"));
        assert!(!invalid.summary.dry_run_eligible, "{source}");
        assert!(
            invalid
                .warnings
                .iter()
                .any(|warning| warning.code == ProgramWarningCode::ArcDefinition)
        );
    }
}

#[test]
fn remaining_arc_budget_is_global_and_omitted_motion_is_not_reported_complete() {
    let radius = millo_gcode::MAX_PREVIEW_POINTS as f64 * 0.6 * 0.5 / std::f64::consts::TAU;
    let program = parse(&format!(
        "G21 G90 G94 G17\nG1 X1 F100\nG2 X1 I{radius}\nG2 X1 I{radius}\nG1 X2"
    ));
    assert_eq!(program.toolpath.len(), 2);
    assert!(
        program
            .toolpath
            .iter()
            .map(|segment| segment.points.len())
            .sum::<usize>()
            <= millo_gcode::MAX_PREVIEW_POINTS
    );
    assert_eq!(
        program.warnings.last().unwrap().code,
        ProgramWarningCode::PreviewLimit
    );
    assert!(!program.summary.time_estimate_complete);
}

#[test]
fn rejects_grbl_float_overflow_and_underflow_without_nonfinite_preview_data() {
    for word in [
        format!("X{}", "9".repeat(300)),
        format!("F0.{}1", "0".repeat(310)),
        format!("P{}", "9".repeat(100)),
    ] {
        let program = parse(&format!("G21 G90 G94\nG1 X1 F100\n{word}"));
        assert!(!program.summary.dry_run_eligible);
        assert!(
            program
                .warnings
                .iter()
                .any(|warning| warning.code == ProgramWarningCode::InvalidToken)
        );
        let json = serde_json::to_value(&program).unwrap();
        assert!(json["summary"]["estimatedTotalTimeSeconds"].is_number());
        assert!(program.summary.cutting_distance_mm.is_finite());
    }
}

#[test]
fn duplicate_value_words_fail_before_firmware_check() {
    for block in ["G1 X1 X2 F100", "G4 P1 P2", "T1 T2 M6", "G1 X1 F100 F200"] {
        let program = parse(&format!("G21 G90 G94\n{block}"));
        assert!(!program.summary.dry_run_eligible, "{block}");
        assert!(
            program
                .warnings
                .iter()
                .any(|warning| warning.code == ProgramWarningCode::DuplicateWord),
            "{block}"
        );
    }
}

#[test]
fn malformed_source_has_bounded_line_and_diagnostic_allocation() {
    use millo_gcode::{MAX_PROGRAM_DIAGNOSTICS, MAX_SOURCE_LINE_BYTES, ProgramParseError};
    let request = |source: String| ProgramParseRequest {
        source_name: "malformed.nc".into(),
        source,
    };
    for source in [
        "!".repeat(MAX_SOURCE_LINE_BYTES + 1),
        format!("({})", "x".repeat(MAX_SOURCE_LINE_BYTES)),
    ] {
        assert_eq!(
            parse_program(request(source)).unwrap_err(),
            ProgramParseError::SourceLineTooLong {
                source_line: 1,
                max_bytes: MAX_SOURCE_LINE_BYTES
            }
        );
    }
    let mut source = ")\n".repeat(MAX_PROGRAM_DIAGNOSTICS);
    assert!(parse_program(request(source.clone())).is_ok());
    source.push_str("G1 Xbad\n");
    assert_eq!(
        parse_program(request(source)).unwrap_err(),
        ProgramParseError::TooManyDiagnostics {
            max_warnings: MAX_PROGRAM_DIAGNOSTICS
        }
    );
}
