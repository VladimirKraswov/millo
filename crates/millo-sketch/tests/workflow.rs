use millo_sketch::*;
use millo_tooling::{CuttingTool, ToolKind, factory_presets};

fn tools() -> Vec<CuttingTool> {
    let mut tool = factory_presets()
        .into_iter()
        .find(|t| t.kind == ToolKind::FlatEndMill)
        .unwrap();
    tool.id = "mill-2".into();
    tool.diameter_mm = 2.0;
    tool.cutting_length_mm = 12.0;
    let mut second = tool.clone();
    second.id = "mill-1".into();
    second.diameter_mm = 1.0;
    let mut drill = tool.clone();
    drill.id = "drill".into();
    drill.kind = ToolKind::Drill;
    vec![tool, second, drill]
}

fn shape(id: &str, kind: SketchOperationKind) -> SketchShape {
    SketchShape {
        id: id.into(),
        name: id.into(),
        x_mm: 30.0,
        y_mm: 30.0,
        rotation_degrees: 0.0,
        constraints: SketchConstraints::default(),
        locked: false,
        geometry: SketchGeometry::Rectangle {
            width: 20.0,
            height: 20.0,
            radius: 0.0,
        },
        operation: SketchOperation {
            kind,
            tool_id: "mill-2".into(),
            through: false,
            depth_mm: 1.0,
            stepdown_mm: 0.4,
            stepover_percent: 40.0,
            feed_mm_per_min: 300.0,
            plunge_mm_per_min: 80.0,
            spindle_rpm: 10_000,
            tabs: SketchTabs {
                count: 0,
                width_mm: 3.0,
                height_mm: 0.5,
            },
        },
    }
}

fn request(shapes: Vec<SketchShape>) -> SketchJobRequest {
    SketchJobRequest {
        source_name: "panel.nc".into(),
        shapes,
        stock: SketchStock {
            width_mm: 100.0,
            height_mm: 80.0,
            thickness_mm: 3.0,
            safe_z_mm: 5.0,
            breakthrough_mm: 0.2,
            spindle_mode: SketchSpindleMode::Manual,
        },
    }
}

#[test]
fn diameter_compensation_is_on_the_requested_side_and_depth_is_bounded() {
    for (kind, expected_x) in [
        (SketchOperationKind::Inside, 21.0),
        (SketchOperationKind::Outside, 19.0),
    ] {
        let job = generate_sketch_job(request(vec![shape("contour", kind)]), &tools()).unwrap();
        let min = job.summary.paths[0]
            .points
            .iter()
            .map(|p| p.x)
            .fold(f64::INFINITY, f64::min);
        assert!((min - expected_x).abs() < 0.002);
        assert_eq!(job.summary.operations[0].pass_count, 3);
        assert_eq!(job.program.summary.bounds.unwrap().min.z, -1.0);
        assert!(!job.program.features.has_spindle_activation);
        assert!(!job.source.contains("M6"));
        assert!(!job.source.lines().any(|l| {
            l.split_whitespace()
                .any(|word| matches!(word, "G54" | "G55" | "G56" | "G57" | "G58" | "G59"))
        }));
        assert!(job.source.contains("T1\n"));
    }
}

#[test]
fn pocket_clears_centre_and_retracts_before_every_xy_rapid() {
    let job = generate_sketch_job(
        request(vec![shape("pocket", SketchOperationKind::Pocket)]),
        &tools(),
    )
    .unwrap();
    assert!(job.summary.paths.len() > 10);
    let innermost = &job.summary.paths[0].points;
    assert!(
        innermost
            .iter()
            .all(|p| (p.x - 30.0).abs() < 1.0 && (p.y - 30.0).abs() < 1.0)
    );
    let lines = job.source.lines().collect::<Vec<_>>();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("G0 X") {
            assert_eq!(lines[i - 1], "G0 Z5.0000");
        }
    }
}

#[test]
fn inner_features_precede_outline_and_tool_changes_are_real_barriers() {
    let mut outside = shape("outside", SketchOperationKind::Outside);
    outside.operation.through = true;
    outside.operation.tabs.count = 4;
    let mut pocket = shape("pocket", SketchOperationKind::Pocket);
    pocket.geometry = SketchGeometry::Circle { diameter: 8.0 };
    pocket.operation.tool_id = "mill-1".into();
    let job = generate_sketch_job(request(vec![outside, pocket]), &tools()).unwrap();
    assert_eq!(job.summary.operations[0].shape_id, "pocket");
    assert_eq!(job.summary.operations[1].shape_id, "outside");
    assert_eq!(job.summary.tool_change_count, 1);
    assert!(job.summary.tab_paths.len() >= 4);
    assert!(job.source.contains("G0 Z5.0000\nM5\nM9\nT2 M6"));
    assert_eq!(job.source.matches("M6").count(), 1);
    assert!(job.source.contains("G1 Z-2.5000 F80.000"));
    assert_eq!(job.program.summary.bounds.unwrap().min.z, -3.2);
    let sender_plan = millo_dry_run::build_program_run_plan(
        &job.program,
        millo_dry_run::ProgramRunPolicy::Cutting,
    )
    .unwrap();
    assert_eq!(
        sender_plan
            .lines()
            .iter()
            .filter(|l| l.kind() == millo_dry_run::DryRunLineKind::ToolChange)
            .count(),
        1
    );
}

#[test]
fn controlled_spindle_is_explicit_manual_has_no_start_words() {
    let mut req = request(vec![shape("pocket", SketchOperationKind::Pocket)]);
    req.stock.spindle_mode = SketchSpindleMode::Controller;
    let job = generate_sketch_job(req, &tools()).unwrap();
    assert!(job.source.contains("S10000 M3"));
    assert!(job.source.ends_with("M5\nM9\nM30\n"));
}

#[test]
fn peck_drilling_uses_matching_drill_and_clears_chips() {
    let mut drill = shape("hole", SketchOperationKind::Drill);
    drill.geometry = SketchGeometry::Circle { diameter: 2.0 };
    drill.operation.tool_id = "drill".into();
    let job = generate_sketch_job(request(vec![drill.clone()]), &tools()).unwrap();
    assert_eq!(job.source.matches("G0 X30.0000 Y30.0000").count(), 3);
    assert!(!job.source.contains("G1 X"));
    drill.geometry = SketchGeometry::Circle { diameter: 3.0 };
    assert!(
        generate_sketch_job(request(vec![drill]), &tools())
            .unwrap_err()
            .0
            .contains("Диаметр сверла")
    );
}

#[test]
fn rotated_rounded_rectangle_and_concave_polygon_generate() {
    let mut s = shape("round", SketchOperationKind::Pocket);
    s.geometry = SketchGeometry::Rectangle {
        width: 22.0,
        height: 12.0,
        radius: 3.0,
    };
    s.rotation_degrees = 30.0;
    assert!(generate_sketch_job(request(vec![s.clone()]), &tools()).is_ok());
    s.geometry = SketchGeometry::Polygon {
        points: vec![
            SketchPoint { x: -10.0, y: -10.0 },
            SketchPoint { x: 10.0, y: -10.0 },
            SketchPoint { x: 10.0, y: 0.0 },
            SketchPoint { x: 0.0, y: 0.0 },
            SketchPoint { x: 0.0, y: 10.0 },
            SketchPoint { x: -10.0, y: 10.0 },
        ],
    };
    assert!(generate_sketch_job(request(vec![s]), &tools()).is_ok());
}

#[test]
fn rejects_overlaps_bad_geometry_oversize_tool_and_nonfinite_settings() {
    let s = shape("a", SketchOperationKind::Pocket);
    let mut b = s.clone();
    b.id = "b".into();
    b.x_mm += 5.0;
    assert!(
        generate_sketch_job(request(vec![s.clone(), b]), &tools())
            .unwrap_err()
            .0
            .contains("пересекаются")
    );
    let mut small = s.clone();
    small.geometry = SketchGeometry::Circle { diameter: 1.0 };
    assert!(generate_sketch_job(request(vec![small]), &tools()).is_err());
    let mut bad = s.clone();
    bad.geometry = SketchGeometry::Polygon {
        points: vec![
            SketchPoint { x: -5.0, y: -5.0 },
            SketchPoint { x: 5.0, y: 5.0 },
            SketchPoint { x: -5.0, y: 5.0 },
            SketchPoint { x: 5.0, y: -5.0 },
        ],
    };
    assert!(generate_sketch_job(request(vec![bad]), &tools()).is_err());
    let mut req = request(vec![s]);
    req.stock.thickness_mm = f64::NAN;
    assert!(generate_sketch_job(req, &tools()).is_err());
}

#[test]
fn rejects_out_of_stock_excess_depth_unknown_tools_and_unbounded_work() {
    for mutate in [
        |s: &mut SketchShape| s.x_mm = 0.0,
        |s: &mut SketchShape| s.operation.depth_mm = 30.0,
        |s: &mut SketchShape| s.operation.tool_id = "missing".into(),
        |s: &mut SketchShape| {
            s.operation.through = true;
            s.operation.stepdown_mm = 0.01;
        },
    ] {
        let mut s = shape("invalid", SketchOperationKind::Pocket);
        mutate(&mut s);
        assert!(generate_sketch_job(request(vec![s]), &tools()).is_err());
    }
}

#[test]
fn comments_cannot_inject_commands_and_contract_roundtrips_camel_case() {
    let mut s = shape("safe", SketchOperationKind::Pocket);
    s.name = "Hi\nG0 X999\rM3".into();
    let req = request(vec![s]);
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("stepdownMm"));
    let decoded = serde_json::from_str(&json).unwrap();
    let job = generate_sketch_job(decoded, &tools()).unwrap();
    assert!(!job.source.lines().any(|l| l == "G0 X999" || l == "M3"));
    assert_eq!(job.program.summary.bounds.unwrap().max.x, 39.0);
}

#[test]
fn rejects_nonfinite_cutting_length_before_depth_clamping() {
    for length in [f64::NAN, f64::INFINITY] {
        let mut tools = tools();
        tools[0].cutting_length_mm = length;
        assert!(
            generate_sketch_job(
                request(vec![shape("pocket", SketchOperationKind::Pocket)]),
                &tools,
            )
            .is_err()
        );
    }
}
