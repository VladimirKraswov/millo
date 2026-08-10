use gantryon_grbl::parse_status_line;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusFixture {
    name: String,
    input: String,
    mode: String,
    substate: Option<u8>,
    machine_position: [f64; 3],
    feed_rate: f64,
    spindle_speed: f64,
}

#[test]
fn parses_recorded_status_frames() {
    let fixtures: Vec<StatusFixture> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/grbl/status-lines.json"
    )))
    .unwrap();

    for fixture in fixtures {
        let state = parse_status_line(&fixture.input)
            .unwrap_or_else(|error| panic!("fixture '{}': {error}", fixture.name));
        let position = state.machine_position.unwrap();

        assert_eq!(state.reported_mode, fixture.mode, "{}", fixture.name);
        assert_eq!(state.substate, fixture.substate, "{}", fixture.name);
        assert_eq!(
            [position.x, position.y, position.z],
            fixture.machine_position,
            "{}",
            fixture.name
        );
        assert_eq!(state.feed_rate, fixture.feed_rate, "{}", fixture.name);
        assert_eq!(
            state.spindle_speed, fixture.spindle_speed,
            "{}",
            fixture.name
        );
    }
}
