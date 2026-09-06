use millo_gcode::{
    MAX_PREVIEW_POINTS, MAX_SOURCE_BYTES, MAX_SOURCE_LINES, ProgramParseRequest, parse_program,
};

/// Run explicitly with: cargo test -p millo-gcode --release --test large_program -- --ignored --nocapture
#[test]
#[ignore = "million-line native geometry memory/performance regression"]
fn million_line_xyza_program_retains_native_geometry_and_checkpoints() {
    let source = format!("G21 G91 G93\n{}", "G1 X0.01 A0.1 F60\n".repeat(999_999));
    assert!(source.len() < MAX_SOURCE_BYTES);
    let start = std::time::Instant::now();
    let program = parse_program(ProgramParseRequest {
        source_name: "million-lines.nc".into(),
        source,
    })
    .unwrap();
    eprintln!(
        "million-line XYZA parse: {:?}, native points: {}",
        start.elapsed(),
        program
            .toolpath
            .iter()
            .map(|segment| segment.points.len())
            .sum::<usize>()
    );
    assert_eq!(MAX_SOURCE_LINES, 2_000_000);
    assert_eq!(MAX_PREVIEW_POINTS, 4_000_000);
    assert_eq!(program.lines.len(), 1_000_000);
    assert_eq!(program.execution_checkpoints.len(), 1_000_000);
    assert_eq!(program.toolpath.len(), 999_999);
    assert_eq!(program.summary.estimated_motion_time_seconds, 999_999.0);
    assert!(program.summary.preview_complete && program.summary.time_estimate_complete);
    assert!(program.warnings.is_empty());
    assert!((program.summary.rotary_travel_degrees - 99_999.9).abs() < 0.001);
}
