use millo_domain::CommandResponse;
use millo_grbl::build_device_inspection;

#[test]
fn parses_recorded_device_inspector_responses() {
    let responses: Vec<CommandResponse> =
        serde_json::from_str(include_str!("fixtures/device_inspector.json")).unwrap();

    let inspection = build_device_inspection(responses);

    assert_eq!(
        inspection.firmware_version.as_deref(),
        Some("1.1h.20190825")
    );
    assert_eq!(
        inspection.firmware_build_info.as_deref(),
        Some("XYZ Router")
    );
    let capabilities = inspection.controller_capabilities.as_ref().unwrap();
    assert_eq!(capabilities.option_flags, "V");
    assert_eq!(capabilities.planner_buffer_blocks, Some(15));
    assert_eq!(capabilities.rx_buffer_bytes, Some(128));
    assert_eq!(
        inspection.settings.get("$21").map(String::as_str),
        Some("0")
    );
    assert_eq!(
        inspection.settings.get("$22").map(String::as_str),
        Some("0")
    );
    assert!(inspection.modal_state.contains(&"G21".to_owned()));
    assert_eq!(
        inspection.parameters.get("PRB").map(String::as_str),
        Some("0.000,0.000,0.000:0")
    );
}
