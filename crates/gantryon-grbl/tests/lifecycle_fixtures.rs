use gantryon_grbl::{IncomingLine, parse_incoming_line};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct LifecycleFixture {
    name: String,
    input: String,
    kind: String,
    version: Option<String>,
    code: Option<u16>,
    mode: Option<String>,
}

#[test]
fn classifies_recorded_lifecycle_lines() {
    let fixtures: Vec<LifecycleFixture> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/grbl/lifecycle-lines.json"
    )))
    .unwrap();

    for fixture in fixtures {
        let line = parse_incoming_line(&fixture.input)
            .unwrap_or_else(|error| panic!("fixture '{}': {error}", fixture.name));

        match (fixture.kind.as_str(), line) {
            ("reset", IncomingLine::ResetBanner { version, .. }) => {
                assert_eq!(version, fixture.version, "{}", fixture.name);
            }
            ("alarm", IncomingLine::Alarm { code, .. })
            | ("error", IncomingLine::Error { code, .. }) => {
                assert_eq!(code, fixture.code, "{}", fixture.name);
            }
            ("status", IncomingLine::Status(state)) => {
                assert_eq!(Some(state.reported_mode), fixture.mode, "{}", fixture.name);
            }
            ("ok", IncomingLine::Ok) => {}
            (expected, actual) => panic!(
                "fixture '{}' expected {expected}, got {actual:?}",
                fixture.name
            ),
        }
    }
}
