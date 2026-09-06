use millo_gcode::{
    ProgramParseOptions, ProgramParseRequest, ProgramWarningCode, ToolpathKind, parse_program,
    parse_program_with_options,
};

fn parse(source: &str) -> millo_gcode::GcodeProgram {
    parse_program(ProgramParseRequest {
        source_name: "rotary.nc".into(),
        source: source.into(),
    })
    .unwrap()
}

#[test]
fn a_is_unwrapped_degrees_under_both_units_and_distance_modes() {
    let program =
        parse("G20 G90 G93\nG1 X1 A450 F2\nG91 G1 X1 A-90 F4\nG21 G90 G1 X60 A-720 F5\nM2");
    assert!(program.summary.preview_complete);
    assert!(program.summary.time_estimate_complete);
    assert_eq!(program.summary.estimated_motion_time_seconds, 57.0);
    let segments = &program.toolpath;
    assert_eq!(segments[0].points[1].x, 25.4);
    assert_eq!(segments[1].points[1].x, 50.8);
    assert_eq!(segments[0].rotary.unwrap().end_degrees, 450.0);
    assert_eq!(segments[1].rotary.unwrap().end_degrees, 360.0);
    assert_eq!(segments[2].rotary.unwrap().end_degrees, -720.0);
    assert_eq!(program.summary.rotary_travel_degrees, 1620.0);
    let bounds = program.summary.rotary_bounds.unwrap();
    assert_eq!(
        (bounds.min_degrees, bounds.max_degrees, bounds.size_degrees),
        (-720.0, 450.0, 1170.0)
    );
    let checkpoint = program.execution_checkpoints.last().unwrap();
    assert_eq!(checkpoint.a, Some(-720.0));
    assert!(checkpoint.a_is_absolute);
    assert!(program.features.uses_rotary_a && program.features.uses_inverse_time_feed);
}

#[test]
fn pure_a_is_motion_and_g94_does_not_claim_cartesian_timing() {
    let program = parse("G20 G90 G94\nG1 A90 F60\nG0 A180\nG1 X1 F2\nM2");
    assert_eq!(program.toolpath.len(), 3);
    let segment = &program.toolpath[0];
    assert_eq!(segment.points.len(), 2);
    assert_eq!(segment.points[0], segment.points[1]);
    assert_eq!(segment.distance_mm, 0.0);
    assert_eq!(segment.estimated_duration_seconds, None);
    assert_eq!(segment.feed_rate_mm_per_min, None);
    assert_eq!(program.toolpath[1].kind, ToolpathKind::Rapid);
    assert_eq!(program.toolpath[2].rotary.unwrap().start_degrees, 180.0);
    assert_eq!(program.toolpath[2].rotary.unwrap().end_degrees, 180.0);
    assert_eq!(program.toolpath[2].estimated_duration_seconds, Some(30.0));
    assert!(!program.summary.time_estimate_complete);
    assert!(program.summary.dry_run_eligible);
    assert!(
        program
            .warnings
            .iter()
            .any(|warning| warning.code == ProgramWarningCode::RotaryTimingUnavailable)
    );
}

#[test]
fn simultaneous_helical_arc_a_is_synchronous_in_all_cartesian_planes() {
    for (plane, arc) in [
        ("G17", "X10 Y0 Z2 I5 J0"),
        ("G18", "Z10 X0 Y2 K5 I0"),
        ("G19", "Y10 Z0 X2 J5 K0"),
    ] {
        let program = parse(&format!("G21 G90 G93 {plane}\nG3 {arc} A720 F8"));
        assert!(
            program.warnings.is_empty(),
            "{plane}: {:?}",
            program.warnings
        );
        assert!(program.features.uses_rotary_arc);
        assert_eq!(program.toolpath.len(), 1);
        let segment = &program.toolpath[0];
        assert_eq!(segment.estimated_duration_seconds, Some(7.5));
        assert_eq!(segment.rotary.unwrap().end_degrees, 720.0);
        assert!(segment.distance_mm > 15.0);
        assert!(segment.distance_mm < 17.0);
    }
}

#[test]
fn g93_requires_f_on_every_rotary_motion_block() {
    for source in [
        "G93 G1 A1 F2\nG1 A2",
        "G93 G1 A1 F2\nG1 A1",
        "G93 G1 A1 F0",
        "G93 G2 X10 I5 A90",
    ] {
        let program = parse(source);
        assert!(!program.summary.dry_run_eligible, "{source}");
        assert!(
            program
                .warnings
                .iter()
                .any(|warning| warning.code == ProgramWarningCode::FeedRate)
        );
    }
    let rapid = parse("G93 G0 A1");
    assert!(rapid.summary.dry_run_eligible);
}

#[test]
fn rotary_context_errors_and_block_delete_are_not_ignored() {
    for source in [
        "G1 A1 A2 F100",
        "G4 P1 A90",
        "G80 A90",
        "G92 A0",
        "G1 B90 F100",
    ] {
        assert!(!parse(source).summary.dry_run_eligible, "{source}");
    }
    let program = parse_program_with_options(
        ProgramParseRequest {
            source_name: "optional.nc".into(),
            source: "/G1 A90 F100\nG1 X1 F100".into(),
        },
        ProgramParseOptions { block_delete: true },
    )
    .unwrap();
    assert!(!program.features.uses_rotary_a);
    assert!(program.summary.rotary_bounds.is_none());
    assert!(program.toolpath[0].rotary.is_none());
}

#[test]
fn additive_json_contract_and_old_xyz_payloads_round_trip() {
    let program = parse("G93 G1 A90 F2\nM2");
    let json = serde_json::to_value(&program).unwrap();
    assert_eq!(json["toolpath"][0]["rotary"]["startDegrees"], 0.0);
    assert_eq!(json["toolpath"][0]["rotary"]["endDegrees"], 90.0);
    assert_eq!(json["features"]["usesRotaryA"], true);
    assert_eq!(json["summary"]["rotaryBounds"]["sizeDegrees"], 90.0);
    assert_eq!(json["summary"]["rotaryTravelDegrees"], 90.0);
    assert!(json.get("executionCheckpoints").is_none());
    let mut old = serde_json::to_value(parse("G1 X1 F100")).unwrap();
    for name in ["usesRotaryA", "usesRotaryArc", "usesInverseTimeFeed"] {
        old["features"].as_object_mut().unwrap().remove(name);
    }
    old["summary"]
        .as_object_mut()
        .unwrap()
        .remove("rotaryTravelDegrees");
    let decoded: millo_gcode::GcodeProgram = serde_json::from_value(old).unwrap();
    assert!(!decoded.features.uses_rotary_a);
    assert!(decoded.toolpath[0].rotary.is_none());
}

#[test]
fn checkpoint_distinguishes_initial_relative_angle_from_absolute_datum() {
    let program = parse("G91 G93\nG1 A30 F2\nG1 A10 F2\nG90 G1 A0 F2\nM2");
    assert_eq!(program.execution_checkpoints[2].a, Some(30.0));
    assert!(!program.execution_checkpoints[2].a_is_absolute);
    assert_eq!(program.execution_checkpoints[4].a, Some(0.0));
    assert!(program.execution_checkpoints[4].a_is_absolute);
}
