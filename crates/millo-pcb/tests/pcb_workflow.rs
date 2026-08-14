use base64::Engine;
use millo_pcb::{
    PcbDrillToolMapping, PcbDrillingSettings, PcbError, PcbInspectRequest, PcbIsolationSettings,
    PcbJobRequest, PcbJobSettings, PcbLayerRole, PcbMarkingSettings, PcbOutlineSettings,
    PcbSourceFile, PcbTransform, generate_pcb_job, inspect_pcb,
};
use millo_tooling::factory_presets;

fn source(name: &str, role: PcbLayerRole, bytes: &[u8]) -> PcbSourceFile {
    PcbSourceFile {
        source_name: name.to_owned(),
        source_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
        role,
    }
}

fn board() -> PcbInspectRequest {
    PcbInspectRequest {
        files: vec![
            source(
                "easyeda-top.gtl",
                PcbLayerRole::Copper,
                include_bytes!("fixtures/easyeda-top.gtl"),
            ),
            source(
                "board-outline.gko",
                PcbLayerRole::Outline,
                include_bytes!("fixtures/board-outline.gko"),
            ),
            source(
                "drill.drl",
                PcbLayerRole::Drill,
                include_bytes!("fixtures/drill.drl"),
            ),
        ],
        transform: PcbTransform::default(),
    }
}

#[test]
fn inspects_copper_outline_and_excellon_groups() {
    let inspection = inspect_pcb(board()).unwrap();

    assert_eq!(inspection.files.len(), 3);
    assert_eq!(inspection.drill_hits.len(), 3);
    assert_eq!(inspection.drill_groups.len(), 2);
    assert_eq!(inspection.drill_groups[0].diameter_mm, 0.8);
    assert_eq!(inspection.drill_groups[0].hit_count, 2);
    assert!(inspection.bounds.width_mm >= 20.0);
    assert!(inspection.bounds.height_mm >= 14.0);
    assert!(inspection.warnings.is_empty());
}

#[test]
fn transform_rotates_mirrors_and_keeps_design_in_requested_offset() {
    let mut request = board();
    request.transform = PcbTransform {
        offset_x_mm: 12.0,
        offset_y_mm: 7.0,
        rotation_quarter_turns: 1,
        mirror_x: true,
    };
    let inspection = inspect_pcb(request).unwrap();

    assert!((inspection.bounds.min_x_mm - 12.0).abs() < 0.001);
    assert!((inspection.bounds.min_y_mm - 7.0).abs() < 0.001);
    assert!(inspection.bounds.width_mm >= 14.0);
    assert!(inspection.bounds.height_mm >= 20.0);
}

#[test]
fn emits_one_valid_program_with_manual_tool_change_barriers() {
    let tools = factory_presets();
    let isolation_tool = tools
        .iter()
        .find(|tool| tool.id == "preset-xc-nlj3-2001")
        .unwrap();
    let drill_tool = tools
        .iter()
        .find(|tool| tool.id == "preset-dreanique-sp1f-d1-0-l03")
        .unwrap();
    let outline_tool = tools
        .iter()
        .find(|tool| tool.id == "preset-dreanique-sp1f-d2-0-l04")
        .unwrap();
    let request = PcbJobRequest {
        source_name: "fixture-board.nc".to_owned(),
        board: board(),
        settings: PcbJobSettings {
            safe_z_mm: 3.0,
            surface_z_mm: 0.0,
            isolation: PcbIsolationSettings {
                enabled: true,
                tool_id: isolation_tool.id.clone(),
                depth_mm: 0.08,
                clearance_mm: 0.05,
                passes: 2,
            },
            drilling: PcbDrillingSettings {
                enabled: true,
                depth_mm: 1.8,
                mappings: vec![
                    PcbDrillToolMapping {
                        group_key: "drill.drl::T1".to_owned(),
                        tool_id: drill_tool.id.clone(),
                    },
                    PcbDrillToolMapping {
                        group_key: "drill.drl::T2".to_owned(),
                        tool_id: outline_tool.id.clone(),
                    },
                ],
            },
            outline: PcbOutlineSettings {
                enabled: true,
                tool_id: outline_tool.id.clone(),
                depth_mm: 1.7,
                depth_per_pass_mm: 0.4,
                tab_count: 4,
                tab_width_mm: 2.0,
                tab_height_mm: 0.4,
            },
            marking: PcbMarkingSettings {
                enabled: false,
                tool_id: isolation_tool.id.clone(),
                depth_mm: 0.04,
            },
        },
    };

    let job = generate_pcb_job(request, &tools).unwrap();
    assert_eq!(job.summary.operations.len(), 4);
    assert_eq!(job.summary.tool_change_count, 3);
    assert_eq!(job.source.matches(" M6").count(), 3);
    assert!(
        !job.source
            .lines()
            .any(|line| matches!(line.trim(), "M3" | "M4"))
    );
    assert!(job.source.contains("M30"));
    assert!(job.source.contains("Z-1.3"), "outline tabs keep 0.4 mm");
    assert!(!job.program.toolpath.is_empty());
}

#[test]
fn rejects_an_unmapped_drill_group_before_gcode_is_emitted() {
    let tools = factory_presets();
    let request = PcbJobRequest {
        source_name: "invalid.nc".to_owned(),
        board: board(),
        settings: PcbJobSettings {
            safe_z_mm: 3.0,
            surface_z_mm: 0.0,
            isolation: PcbIsolationSettings {
                enabled: false,
                tool_id: String::new(),
                depth_mm: 0.08,
                clearance_mm: 0.0,
                passes: 1,
            },
            drilling: PcbDrillingSettings {
                enabled: true,
                depth_mm: 1.8,
                mappings: vec![PcbDrillToolMapping {
                    group_key: "missing::T9".to_owned(),
                    tool_id: tools[0].id.clone(),
                }],
            },
            outline: PcbOutlineSettings {
                enabled: false,
                tool_id: String::new(),
                depth_mm: 1.7,
                depth_per_pass_mm: 0.4,
                tab_count: 0,
                tab_width_mm: 2.0,
                tab_height_mm: 0.4,
            },
            marking: PcbMarkingSettings {
                enabled: false,
                tool_id: String::new(),
                depth_mm: 0.04,
            },
        },
    };

    assert!(
        generate_pcb_job(request, &tools)
            .unwrap_err()
            .to_string()
            .contains("tool mapping")
    );
}

#[test]
fn rejects_incremental_gerber_instead_of_guessing_the_geometry() {
    let request = PcbInspectRequest {
        files: vec![source(
            "incremental.gbr",
            PcbLayerRole::Copper,
            b"G04 incremental fixture*\n%FSLIX24Y24*%\n%MOMM*%\n%ADD10C,0.200*%\nD10*\nX010000Y010000D03*\nM02*\n",
        )],
        transform: PcbTransform::default(),
    };

    let result = inspect_pcb(request);
    assert!(
        matches!(
            result,
            Err(PcbError::UnsupportedGerberFeature(_, ref feature))
                if feature == "incremental coordinates"
        ),
        "unexpected parser result: {result:?}"
    );
}

#[test]
fn rejects_an_empty_excellon_layer_instead_of_silently_omitting_it() {
    let request = PcbInspectRequest {
        files: vec![source(
            "empty.drl",
            PcbLayerRole::Drill,
            b"M48\nMETRIC,LZ,000.000\nT1C0.800\n%\nM30\n",
        )],
        transform: PcbTransform::default(),
    };

    assert!(matches!(
        inspect_pcb(request),
        Err(PcbError::EmptyLayer(name)) if name == "empty.drl"
    ));
}
