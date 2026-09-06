use millo_domain::{CommandCompletion, CommandResponse};
use millo_grbl::{
    RotaryAxisEvidence, build_device_inspection, parse_status_line as parse_status,
    rotary_axis_evidence,
};

fn identity(lines: &[&str]) -> millo_domain::DeviceInspection {
    build_device_inspection(vec![CommandResponse {
        command: "$I".to_owned(),
        completion: CommandCompletion::Ok,
        lines: lines.iter().map(|line| (*line).to_owned()).collect(),
        code: None,
    }])
}

#[test]
fn explicit_xyza_and_fourth_status_are_independent_evidence_sources() {
    let inspection = identity(&["[AXS:4:XYZA]"]);
    assert_eq!(
        rotary_axis_evidence(Some(&inspection), None),
        Some(RotaryAxisEvidence::ReportedAxes)
    );
    for frame in [
        "<Idle|MPos:1,2,3,0>",
        "<Run|WPos:1,2,3,-720|WCO:0,0,0,90>",
        "<Hold:0|MPos:1,2,3,180|WPos:1,2,3,90>",
    ] {
        let status = parse_status(frame).unwrap();
        assert_eq!(
            rotary_axis_evidence(None, Some(&status)),
            Some(RotaryAxisEvidence::StatusPosition)
        );
        assert_eq!(
            rotary_axis_evidence(Some(&inspection), Some(&status)),
            Some(RotaryAxisEvidence::ReportedAxes)
        );
    }
}

#[test]
fn branding_options_settings_and_offsets_alone_do_not_enable_a() {
    for lines in [
        vec!["[VER:1.1h:stock Grbl]", "[OPT:V,15,128]"],
        vec!["[FIRMWARE:grblHAL]", "[OPT:VNMSLW,35,1024,4,0]"],
        vec!["[VER:3.9:FluidNC]", "[MSG:Machine:four axis]"],
    ] {
        let mut inspection = identity(&lines);
        inspection
            .settings
            .insert("$103".to_owned(), "10".to_owned());
        assert_eq!(rotary_axis_evidence(Some(&inspection), None), None);
    }
    for frame in [
        "<Idle|MPos:0,0,0>",
        "<Idle|WCO:0,0,0,0>",
        "<Idle|FS:0,0|A:S>",
    ] {
        assert_eq!(
            rotary_axis_evidence(None, Some(&parse_status(frame).unwrap())),
            None
        );
    }
}

#[test]
fn contradictory_or_malformed_topology_blocks_status_fallback() {
    let status = parse_status("<Idle|MPos:0,0,0,0>").unwrap();
    for declaration in [
        "[AXS:3:XYZ]",
        "[AXS:4:XYZU]",
        "[AXS:5:XYZAB]",
        "[AXS:6:XYZABC]",
        "[AXS:4:XYZ]",
        "[AXS:3:XYZA]",
        "[AXS:4:XYAA]",
        "[AXS:4]",
        "[AXS:4:XYZA",
        "[AXS:4:XYZA:extra]",
    ] {
        let inspection = identity(&["[AXS:4:XYZA]", declaration]);
        assert_eq!(
            rotary_axis_evidence(Some(&inspection), Some(&status)),
            None,
            "{declaration}"
        );
    }
    let inspection = identity(&["[AXS:4:XYZA]"]);
    for frame in [
        "<Idle|MPos:0,0,0>",
        "<Idle|MPos:0,0,0,0|WPos:0,0,0>",
        "<Idle|MPos:0,0,0,0|WCO:0,0,0>",
    ] {
        assert_eq!(
            rotary_axis_evidence(Some(&inspection), Some(&parse_status(frame).unwrap())),
            None
        );
    }
}

#[test]
fn failed_identity_is_not_successful_axis_evidence() {
    let mut inspection = identity(&["[AXS:4:XYZA]"]);
    inspection.responses[0].completion = CommandCompletion::Error;
    assert_eq!(rotary_axis_evidence(Some(&inspection), None), None);
    inspection.responses[0].completion = CommandCompletion::Ok;
    inspection.responses[0].command = "$#".to_owned();
    assert_eq!(rotary_axis_evidence(Some(&inspection), None), None);
    inspection.responses[0].command = "$I+".to_owned();
    assert_eq!(
        rotary_axis_evidence(Some(&inspection), None),
        Some(RotaryAxisEvidence::ReportedAxes)
    );
}

#[test]
fn grblhal_startup_banner_marks_a_reset_boundary() {
    assert!(matches!(
        millo_grbl::parse_incoming_line("GrblHAL 1.1f ['$' or '$HELP' for help]").unwrap(),
        millo_grbl::IncomingLine::ResetBanner { version: Some(version), .. } if version == "1.1f"
    ));
}

#[test]
fn typed_rotary_zero_is_a_only_and_return_requires_clearance() {
    use millo_domain::{ReturnToWorkZeroRequest, WorkAxis, WorkCoordinateSystem};
    assert_eq!(
        millo_grbl::encode_set_work_zero(WorkAxis::A, WorkCoordinateSystem::G55),
        "G10 L20 P2 A0"
    );
    assert_eq!(
        millo_grbl::encode_set_work_value(WorkAxis::A, WorkCoordinateSystem::G54, -90.0),
        "G10 L20 P1 A-90.000"
    );
    assert_eq!(
        millo_grbl::encode_return_to_work_zero(ReturnToWorkZeroRequest {
            axis: WorkAxis::A,
            feed_mm_per_min: 360.0
        }),
        Err(millo_grbl::JogValidationError::RotaryClearanceRequired)
    );
    assert_eq!(serde_json::to_string(&WorkAxis::A).unwrap(), "\"a\"");
}

#[test]
fn non_finite_or_unsupported_position_vectors_are_rejected() {
    for vector in [
        "0,0,0,NaN",
        "0,0,0,inf",
        "NaN,0,0,90",
        "0,0,0,0,0",
        "0,0",
        "0,0,0,",
    ] {
        for field in ["MPos", "WPos", "WCO"] {
            assert!(
                parse_status(&format!("<Idle|{field}:{vector}>")).is_err(),
                "{field}:{vector}"
            );
        }
    }
}
