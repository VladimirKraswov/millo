use millo_sketch::*;

fn fixture() -> SketchJobRequest {
    let project: serde_json::Value = serde_json::from_str(include_str!(
        "../../../fixtures/sketch/constrained-holes.millo-sketch.json"
    ))
    .unwrap();
    serde_json::from_value(project["document"].clone()).unwrap()
}
fn center(reference: &str, offset: f64) -> SketchAxisConstraint {
    SketchAxisConstraint {
        reference_id: Some(reference.into()),
        reference_anchor: SketchAnchor::Named(SketchAnchorName::Center),
        own_anchor: SketchAnchor::Named(SketchAnchorName::Center),
        offset_mm: offset,
    }
}

#[test]
fn shared_project_resolves_edge_clearance_and_hole_spacing_instead_of_cached_centres() {
    let doc = resolve_sketch(fixture()).unwrap();
    assert_eq!((doc.shapes[0].x_mm, doc.shapes[0].y_mm), (12.0, 20.0));
    assert_eq!((doc.shapes[1].x_mm, doc.shapes[1].y_mm), (42.0, 20.0));
    let mut resized = doc;
    resized.shapes[0].geometry = SketchGeometry::Circle { diameter: 10.0 };
    let resolved = resolve_sketch(resized).unwrap();
    assert_eq!(resolved.shapes[0].x_mm, 15.0);
    assert_eq!(resolved.shapes[1].x_mm, 45.0);
}

#[test]
fn rounded_rotated_edges_and_polygon_vertices_have_exact_anchors() {
    let mut doc = fixture();
    doc.shapes[0].geometry = SketchGeometry::Rectangle {
        width: 20.0,
        height: 10.0,
        radius: 2.0,
    };
    doc.shapes[0].rotation_degrees = 45.0;
    let resolved = resolve_sketch(doc.clone()).unwrap();
    assert!((resolved.shapes[0].x_mm - (12.0 + 11.0 / 2.0f64.sqrt())).abs() < 1e-8);
    doc.shapes[0].geometry = SketchGeometry::Polygon {
        points: vec![
            SketchPoint { x: -5.0, y: -5.0 },
            SketchPoint { x: 5.0, y: -5.0 },
            SketchPoint { x: 0.0, y: 5.0 },
        ],
    };
    doc.shapes[0].rotation_degrees = 90.0;
    doc.shapes[1]
        .constraints
        .x
        .as_mut()
        .unwrap()
        .reference_anchor = SketchAnchor::Vertex(1);
    let resolved = resolve_sketch(doc).unwrap();
    assert!((resolved.shapes[1].x_mm - 50.0).abs() < 1e-8);
}

#[test]
fn cycles_fail_but_cross_axis_dependencies_are_independent() {
    let mut doc = fixture();
    doc.shapes[0].constraints.x = Some(center("b", 0.0));
    assert!(
        resolve_sketch(doc.clone())
            .unwrap_err()
            .0
            .contains("Циклическая")
    );
    doc.shapes[1].constraints.x = None;
    doc.shapes[1].x_mm = 60.0;
    let resolved = resolve_sketch(doc).unwrap();
    assert_eq!(resolved.shapes[0].x_mm, 60.0);
    assert_eq!(resolved.shapes[1].y_mm, 20.0);
}

#[test]
fn invalid_dimensions_never_reach_the_cam_planner() {
    for constraint in [
        center("missing", 0.0),
        center("a", f64::NAN),
        SketchAxisConstraint {
            reference_anchor: SketchAnchor::Vertex(30),
            ..center("a", 1.0)
        },
    ] {
        let mut doc = fixture();
        doc.shapes[1].constraints.x = Some(constraint);
        assert!(resolve_sketch(doc).is_err());
    }
    let mut doc = fixture();
    doc.shapes[0]
        .constraints
        .x
        .as_mut()
        .unwrap()
        .reference_anchor = SketchAnchor::Vertex(0);
    assert!(resolve_sketch(doc).is_err());
}

#[test]
fn generated_paths_use_solved_positions_and_old_projects_remain_readable() {
    let job = generate_sketch_job(fixture(), &millo_tooling::factory_presets()).unwrap();
    let paths = &job.summary.paths;
    for (path, expected) in paths.iter().zip([12.0, 42.0]) {
        let min = path
            .points
            .iter()
            .map(|p| p.x)
            .fold(f64::INFINITY, f64::min);
        let max = path
            .points
            .iter()
            .map(|p| p.x)
            .fold(f64::NEG_INFINITY, f64::max);
        assert!(((min + max) / 2.0 - expected).abs() < 0.01);
    }
    let mut old = serde_json::to_value(fixture()).unwrap();
    for shape in old["shapes"].as_array_mut().unwrap() {
        shape.as_object_mut().unwrap().remove("constraints");
        shape.as_object_mut().unwrap().remove("locked");
    }
    let old: SketchJobRequest = serde_json::from_value(old).unwrap();
    assert_eq!(resolve_sketch(old.clone()).unwrap(), old);
}
