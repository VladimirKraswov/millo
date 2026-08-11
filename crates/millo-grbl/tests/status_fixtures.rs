use millo_grbl::parse_status_line;
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
    planner_available: Option<u16>,
    rx_available: Option<u16>,
    feed_override: Option<u16>,
    rapid_override: Option<u16>,
    spindle_override: Option<u16>,
    pins: Option<String>,
    accessories: Option<String>,
    line_number: Option<u64>,
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
        assert_eq!(
            state.buffer_state.map(|buffer| buffer.planner_available),
            fixture.planner_available,
            "{}",
            fixture.name
        );
        assert_eq!(
            state.buffer_state.map(|buffer| buffer.rx_available),
            fixture.rx_available,
            "{}",
            fixture.name
        );
        assert_eq!(
            state.overrides.map(|overrides| overrides.feed_percent),
            fixture.feed_override,
            "{}",
            fixture.name
        );
        assert_eq!(
            state.overrides.map(|overrides| overrides.rapid_percent),
            fixture.rapid_override,
            "{}",
            fixture.name
        );
        assert_eq!(
            state.overrides.map(|overrides| overrides.spindle_percent),
            fixture.spindle_override,
            "{}",
            fixture.name
        );
        assert_eq!(
            state.pins.as_ref().map(|pins| pins.raw.as_str()),
            fixture.pins.as_deref(),
            "{}",
            fixture.name
        );
        assert_eq!(
            state
                .accessories
                .as_ref()
                .map(|accessories| accessories.raw.as_str()),
            fixture.accessories.as_deref(),
            "{}",
            fixture.name
        );
        assert_eq!(state.line_number, fixture.line_number, "{}", fixture.name);
    }
}
