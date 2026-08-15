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

fn drilling_settings(group_key: &str, tool_id: &str, depth_mm: f64) -> PcbJobSettings {
    PcbJobSettings {
        safe_z_mm: 3.0,
        surface_z_mm: 0.0,
        isolation: PcbIsolationSettings {
            enabled: false,
            tool_id: String::new(),
            depth_mm: 0.08,
            copper_thickness_mm: 0.035,
            clearance_mm: 0.05,
            passes: 1,
            feed_mm_per_min: 300.0,
            plunge_mm_per_min: 60.0,
            spindle_rpm: 18_000,
        },
        drilling: PcbDrillingSettings {
            enabled: true,
            depth_mm,
            mappings: vec![PcbDrillToolMapping {
                group_key: group_key.to_owned(),
                tool_id: tool_id.to_owned(),
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
    let mut tools = factory_presets();
    tools
        .iter_mut()
        .find(|tool| tool.id == "preset-dreanique-sp1f-d1-0-l03")
        .unwrap()
        .diameter_mm = 0.8;
    tools
        .iter_mut()
        .find(|tool| tool.id == "preset-dreanique-sp1f-d2-0-l04")
        .unwrap()
        .diameter_mm = 1.2;
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
                copper_thickness_mm: 0.035,
                clearance_mm: 0.05,
                passes: 2,
                feed_mm_per_min: 300.0,
                plunge_mm_per_min: 60.0,
                spindle_rpm: 18_000,
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
    assert_eq!(job.summary.tool_count, 3);
    assert_eq!(job.summary.tool_change_count, 2);
    assert_eq!(job.source.matches(" M6").count(), 2);
    assert!(job.source.lines().any(|line| line == "T1"));
    assert!(!job.source.lines().any(|line| line == "T1 M6"));
    assert!(
        !job.source
            .lines()
            .any(|line| matches!(line.trim(), "M3" | "M4"))
    );
    assert!(job.source.contains("M30"));
    assert!(job.source.contains("Z-1.3"), "outline tabs keep 0.4 mm");
    assert!(job.source.contains("effective Ø0.1282 mm at Z-0.08 · F300"));
    assert!(!job.program.toolpath.is_empty());
}

#[test]
fn inspects_and_emits_excellon_g85_slots_with_a_milling_tool() {
    let board = PcbInspectRequest {
        files: vec![source(
            "drill-slots.drl",
            PcbLayerRole::Drill,
            include_bytes!("fixtures/drill-slots.drl"),
        )],
        transform: PcbTransform::default(),
    };
    let inspection = inspect_pcb(board.clone()).unwrap();
    assert_eq!(inspection.drill_hits.len(), 1);
    assert_eq!(inspection.drill_slots.len(), 1);
    assert_eq!(inspection.drill_groups[0].hit_count, 1);
    assert_eq!(inspection.drill_groups[0].slot_count, 1);

    let tools = factory_presets();
    let tool = tools
        .iter()
        .find(|tool| tool.id == "preset-dreanique-sp1f-d1-0-l03")
        .unwrap();
    let job = generate_pcb_job(
        PcbJobRequest {
            source_name: "slots.nc".to_owned(),
            board,
            settings: drilling_settings("drill-slots.drl::T1", &tool.id, 1.0),
        },
        &tools,
    )
    .unwrap();
    assert!(job.source.contains("G1 X5 Y1"));
    assert!(!job.program.toolpath.is_empty());
}

#[test]
fn rejects_an_oversized_or_drill_type_tool_for_a_slot() {
    let board = PcbInspectRequest {
        files: vec![source(
            "drill-slots.drl",
            PcbLayerRole::Drill,
            include_bytes!("fixtures/drill-slots.drl"),
        )],
        transform: PcbTransform::default(),
    };
    let mut tools = factory_presets();
    let wide = tools
        .iter()
        .find(|tool| tool.id == "preset-dreanique-sp1f-d2-0-l04")
        .unwrap();
    let oversized = generate_pcb_job(
        PcbJobRequest {
            source_name: "invalid-slot.nc".to_owned(),
            board: board.clone(),
            settings: drilling_settings("drill-slots.drl::T1", &wide.id, 1.0),
        },
        &tools,
    );
    assert!(matches!(oversized, Err(PcbError::DrillToolTooLarge { .. })));

    let mut drill = tools
        .iter()
        .find(|tool| tool.id == "preset-dreanique-sp1f-d1-0-l03")
        .unwrap()
        .clone();
    drill.id = "fixture-drill-1mm".to_owned();
    drill.kind = millo_tooling::ToolKind::Drill;
    tools.push(drill.clone());
    let side_loaded_drill = generate_pcb_job(
        PcbJobRequest {
            source_name: "invalid-slot.nc".to_owned(),
            board,
            settings: drilling_settings("drill-slots.drl::T1", &drill.id, 1.0),
        },
        &tools,
    );
    assert!(matches!(
        side_loaded_drill,
        Err(PcbError::SlotRequiresMillingTool { .. })
    ));
}

#[test]
fn accepts_gerber_x2_drill_hits_and_linear_routes() {
    let inspection = inspect_pcb(PcbInspectRequest {
        files: vec![source(
            "x2-drill.gbr",
            PcbLayerRole::Drill,
            include_bytes!("fixtures/x2-drill.gbr"),
        )],
        transform: PcbTransform::default(),
    })
    .unwrap();

    assert_eq!(inspection.drill_hits.len(), 1);
    assert_eq!(inspection.drill_slots.len(), 1);
    assert_eq!(inspection.drill_groups[0].source_tool_number, 10);
    assert_eq!(inspection.drill_groups[0].diameter_mm, 0.8);
}

#[test]
fn accepts_single_quadrant_copper_arcs_and_curved_drill_routes() {
    let copper = inspect_pcb(PcbInspectRequest {
        files: vec![source(
            "single-quadrant.gbr",
            PcbLayerRole::Copper,
            b"G04 single quadrant fixture*\n%FSLAX24Y24*%\n%MOMM*%\n%ADD10C,0.200*%\nD10*\nG74*\nX010000Y000000D02*\nG03X000000Y010000I010000J000000D01*\nM02*\n",
        )],
        transform: PcbTransform::default(),
    })
    .expect("G74 arc has an unambiguous center");
    assert_eq!(copper.paths.len(), 1);
    assert!(copper.paths[0].points.len() > 10);

    let drill = inspect_pcb(PcbInspectRequest {
        files: vec![source(
            "curved-route.gbr",
            PcbLayerRole::Drill,
            b"G04 #@! TF.FileFunction,Plated,1,2,PTH*\n%FSLAX24Y24*%\n%MOMM*%\n%ADD10C,0.800*%\nD10*\nG74*\nX010000Y000000D02*\nG03X000000Y010000I010000J000000D01*\nM02*\n",
        )],
        transform: PcbTransform::default(),
    })
    .expect("curved Gerber drill route must be flattened safely");
    assert!(drill.drill_slots.len() > 10);
    assert_eq!(drill.drill_groups[0].slot_count, drill.drill_slots.len());
}

#[test]
fn applies_gerber_step_and_repeat_to_drill_data() {
    let inspection = inspect_pcb(PcbInspectRequest {
        files: vec![source(
            "repeated-drill.gbr",
            PcbLayerRole::Drill,
            b"G04 repeated drill fixture*\n%FSLAX24Y24*%\n%MOMM*%\n%ADD10C,0.800*%\nD10*\n%SRX2Y2I10.0J8.0*%\nX010000Y010000D03*\n%SR*%\nM02*\n",
        )],
        transform: PcbTransform::default(),
    })
    .unwrap();

    assert_eq!(inspection.drill_hits.len(), 4);
    assert_eq!(inspection.drill_groups[0].hit_count, 4);
    assert!((inspection.bounds.width_mm - 10.0).abs() < 0.001);
    assert!((inspection.bounds.height_mm - 8.0).abs() < 0.001);
}

#[test]
fn renders_aperture_macros_and_step_and_repeat() {
    let inspection = inspect_pcb(PcbInspectRequest {
        files: vec![source(
            "macro-repeat.gbr",
            PcbLayerRole::Copper,
            include_bytes!("fixtures/macro-repeat.gbr"),
        )],
        transform: PcbTransform::default(),
    })
    .unwrap();

    assert_eq!(inspection.paths.len(), 4);
    assert!((inspection.bounds.width_mm - 7.0).abs() < 0.01);
    assert!((inspection.bounds.height_mm - 5.0).abs() < 0.01);
}

#[test]
fn ignores_non_machining_layers_without_parsing_them_as_copper() {
    let mut request = board();
    request.files.push(source(
        "board-mask.gts",
        PcbLayerRole::Ignore,
        b"this fixture must not reach the Gerber parser",
    ));

    let inspection = inspect_pcb(request).unwrap();
    let ignored = inspection
        .files
        .iter()
        .find(|file| file.source_name == "board-mask.gts")
        .unwrap();
    assert_eq!(ignored.role, PcbLayerRole::Ignore);
    assert_eq!(ignored.primitive_count, 0);
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
                copper_thickness_mm: 0.035,
                clearance_mm: 0.0,
                passes: 1,
                feed_mm_per_min: 300.0,
                plunge_mm_per_min: 60.0,
                spindle_rpm: 18_000,
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
fn rejects_negative_file_polarity_without_inventing_a_board_boundary() {
    let request = PcbInspectRequest {
        files: vec![source(
            "negative.gbr",
            PcbLayerRole::Copper,
            b"G04 negative fixture*\n%FSLAX24Y24*%\n%MOMM*%\n%TF.FilePolarity,Negative*%\n%ADD10C,0.200*%\nD10*\nX010000Y010000D03*\nM02*\n",
        )],
        transform: PcbTransform::default(),
    };

    let result = inspect_pcb(request);
    assert!(
        matches!(
            result,
            Err(PcbError::UnsupportedGerberFeature(_, ref feature))
                if feature == "negative file polarity without a finite image boundary"
        ),
        "unexpected parser result: {result:?}"
    );
}

#[test]
fn accepts_easyeda_legacy_absolute_coordinate_command() {
    let request = PcbInspectRequest {
        files: vec![source(
            "Gerber_BottomLayer.GBL",
            PcbLayerRole::Copper,
            b"G04 EasyEDA fixture*\n%FSLAX24Y24*%\n%MOMM*%\nG90*\nG71D02*\n%ADD10C,0.200*%\nD10*\nX010000Y010000D03*\nM02*\n",
        )],
        transform: PcbTransform::default(),
    };

    let inspection = inspect_pcb(request).expect("legacy absolute mode is equivalent to FSLA");
    assert_eq!(inspection.files[0].source_name, "Gerber_BottomLayer.GBL");
    assert_eq!(inspection.paths.len(), 1);
}

#[test]
fn normalizes_legacy_inch_units_when_modern_moin_is_missing() {
    let request = PcbInspectRequest {
        files: vec![source(
            "legacy-inch.gbr",
            PcbLayerRole::Copper,
            b"G04 legacy inch fixture*\n%FSLAX24Y24*%\nG70*\n%ADD10C,0.010*%\nD10*\nX000000Y000000D03*\nX010000Y000000D03*\nM02*\n",
        )],
        transform: PcbTransform::default(),
    };

    let inspection = inspect_pcb(request).expect("G70 must be normalized to MOIN");
    assert!((inspection.bounds.width_mm - 25.654).abs() < 0.01);
}

#[test]
fn rejects_conflicting_legacy_and_extended_units() {
    let request = PcbInspectRequest {
        files: vec![source(
            "conflicting-units.gbr",
            PcbLayerRole::Copper,
            b"G04 conflicting units fixture*\n%FSLAX24Y24*%\n%MOMM*%\nG70*\n%ADD10C,0.200*%\nD10*\nX010000Y010000D03*\nM02*\n",
        )],
        transform: PcbTransform::default(),
    };

    assert!(matches!(
        inspect_pcb(request),
        Err(PcbError::UnsupportedGerberFeature(_, ref feature))
            if feature == "conflicting legacy and extended units"
    ));
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
