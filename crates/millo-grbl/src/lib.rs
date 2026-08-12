use millo_domain::{
    CommandResponse, ControllerAccessories, ControllerBufferState, ControllerCapabilities,
    ControllerOverrides, ControllerPins, DeviceInspection, JogAxis, MachineMode, MachineState,
    Position, StepJogRequest, WorkAxis, WorkCoordinateSystem,
};
use thiserror::Error;

pub const MIN_STEP_JOG_DISTANCE_MM: f64 = 0.01;
pub const MAX_STEP_JOG_DISTANCE_MM: f64 = 100_000.0;
pub const MIN_STEP_JOG_FEED_MM_PER_MIN: f64 = 10.0;
pub const MAX_STEP_JOG_FEED_MM_PER_MIN: f64 = 100_000.0;

#[derive(Debug, Clone, PartialEq)]
pub enum IncomingLine {
    Status(Box<MachineState>),
    ResetBanner {
        raw: String,
        version: Option<String>,
    },
    Alarm {
        code: Option<u16>,
        raw: String,
    },
    Ok,
    Error {
        code: Option<u16>,
        raw: String,
    },
    Message(String),
}

#[derive(Debug, Error, PartialEq)]
pub enum StatusParseError {
    #[error("status frame must be enclosed in '<' and '>'")]
    InvalidFrame,
    #[error("status frame does not contain a machine state")]
    MissingState,
    #[error("field '{field}' contains an invalid number: {value}")]
    InvalidNumber { field: String, value: String },
    #[error("field '{field}' must contain three or four coordinates")]
    InvalidPosition { field: String },
    #[error("field '{field}' must contain exactly {expected} values")]
    InvalidValueCount { field: String, expected: usize },
}

#[derive(Debug, Error, PartialEq)]
pub enum JogValidationError {
    #[error("step jog distance must be finite and non-zero")]
    InvalidDistance,
    #[error("step jog distance must be between {min_mm:.2} and {max_mm:.2} mm")]
    DistanceOutOfRange { min_mm: f64, max_mm: f64 },
    #[error("step jog feed must be finite")]
    InvalidFeed,
    #[error("step jog feed must be between {min_mm_per_min:.0} and {max_mm_per_min:.0} mm/min")]
    FeedOutOfRange {
        min_mm_per_min: f64,
        max_mm_per_min: f64,
    },
}

pub fn encode_step_jog(request: StepJogRequest) -> Result<String, JogValidationError> {
    if !request.distance_mm.is_finite() || request.distance_mm == 0.0 {
        return Err(JogValidationError::InvalidDistance);
    }
    if !(MIN_STEP_JOG_DISTANCE_MM..=MAX_STEP_JOG_DISTANCE_MM).contains(&request.distance_mm.abs()) {
        return Err(JogValidationError::DistanceOutOfRange {
            min_mm: MIN_STEP_JOG_DISTANCE_MM,
            max_mm: MAX_STEP_JOG_DISTANCE_MM,
        });
    }
    if !request.feed_mm_per_min.is_finite() {
        return Err(JogValidationError::InvalidFeed);
    }
    if !(MIN_STEP_JOG_FEED_MM_PER_MIN..=MAX_STEP_JOG_FEED_MM_PER_MIN)
        .contains(&request.feed_mm_per_min)
    {
        return Err(JogValidationError::FeedOutOfRange {
            min_mm_per_min: MIN_STEP_JOG_FEED_MM_PER_MIN,
            max_mm_per_min: MAX_STEP_JOG_FEED_MM_PER_MIN,
        });
    }

    let axis = match request.axis {
        JogAxis::X => 'X',
        JogAxis::Y => 'Y',
        JogAxis::Z => 'Z',
    };
    Ok(format!(
        "$J=G91 G21 {axis}{:.3} F{:.3}",
        request.distance_mm, request.feed_mm_per_min
    ))
}

pub fn active_work_coordinate_system(modal_state: &[String]) -> Option<WorkCoordinateSystem> {
    modal_state.iter().find_map(|word| match word.as_str() {
        "G54" => Some(WorkCoordinateSystem::G54),
        "G55" => Some(WorkCoordinateSystem::G55),
        "G56" => Some(WorkCoordinateSystem::G56),
        "G57" => Some(WorkCoordinateSystem::G57),
        "G58" => Some(WorkCoordinateSystem::G58),
        "G59" => Some(WorkCoordinateSystem::G59),
        _ => None,
    })
}

pub fn encode_set_work_zero(axis: WorkAxis, coordinate_system: WorkCoordinateSystem) -> String {
    let axis = match axis {
        WorkAxis::X => 'X',
        WorkAxis::Y => 'Y',
        WorkAxis::Z => 'Z',
    };
    let parameter = match coordinate_system {
        WorkCoordinateSystem::G54 => 1,
        WorkCoordinateSystem::G55 => 2,
        WorkCoordinateSystem::G56 => 3,
        WorkCoordinateSystem::G57 => 4,
        WorkCoordinateSystem::G58 => 5,
        WorkCoordinateSystem::G59 => 6,
    };
    format!("G10 L20 P{parameter} {axis}0")
}

pub const fn work_coordinate_parameter(coordinate_system: WorkCoordinateSystem) -> &'static str {
    match coordinate_system {
        WorkCoordinateSystem::G54 => "G54",
        WorkCoordinateSystem::G55 => "G55",
        WorkCoordinateSystem::G56 => "G56",
        WorkCoordinateSystem::G57 => "G57",
        WorkCoordinateSystem::G58 => "G58",
        WorkCoordinateSystem::G59 => "G59",
    }
}

pub fn parse_incoming_line(line: &str) -> Result<IncomingLine, StatusParseError> {
    let line = line.trim();

    if line.starts_with('<') {
        return parse_status_line(line).map(|state| IncomingLine::Status(Box::new(state)));
    }

    if let Some(remainder) = line.strip_prefix("Grbl ") {
        let version = remainder.split_whitespace().next().map(str::to_owned);
        return Ok(IncomingLine::ResetBanner {
            raw: line.to_owned(),
            version,
        });
    }

    if let Some(value) = line.strip_prefix("ALARM:") {
        return Ok(IncomingLine::Alarm {
            code: value.trim().parse().ok(),
            raw: line.to_owned(),
        });
    }

    if line == "ok" {
        return Ok(IncomingLine::Ok);
    }

    if let Some(value) = line.strip_prefix("error:") {
        return Ok(IncomingLine::Error {
            code: value.trim().parse().ok(),
            raw: line.to_owned(),
        });
    }

    Ok(IncomingLine::Message(line.to_owned()))
}

pub fn parse_status_line(line: &str) -> Result<MachineState, StatusParseError> {
    let line = line.trim();
    let payload = line
        .strip_prefix('<')
        .and_then(|value| value.strip_suffix('>'))
        .ok_or(StatusParseError::InvalidFrame)?;

    let mut fields = payload.split('|');
    let state_field = fields
        .next()
        .filter(|value| !value.is_empty())
        .ok_or(StatusParseError::MissingState)?;
    let (reported_mode, substate) = parse_mode(state_field)?;

    let mut state = MachineState {
        mode: machine_mode(reported_mode),
        reported_mode: reported_mode.to_owned(),
        substate,
        ..MachineState::default()
    };

    for field in fields {
        let Some((name, value)) = field.split_once(':') else {
            continue;
        };

        match name {
            "MPos" => state.machine_position = Some(parse_position(name, value)?),
            "WPos" => state.work_position = Some(parse_position(name, value)?),
            "WCO" => state.work_coordinate_offset = Some(parse_position(name, value)?),
            "FS" => {
                let values = parse_numbers(name, value)?;
                if let Some(feed_rate) = values.first() {
                    state.feed_rate = *feed_rate;
                }
                if let Some(spindle_speed) = values.get(1) {
                    state.spindle_speed = *spindle_speed;
                }
            }
            "F" => {
                state.feed_rate = parse_numbers(name, value)?
                    .first()
                    .copied()
                    .unwrap_or_default();
            }
            "Bf" => {
                let values = parse_unsigned_numbers::<u16>(name, value)?;
                require_value_count(name, &values, 2)?;
                state.buffer_state = Some(ControllerBufferState {
                    planner_available: values[0],
                    rx_available: values[1],
                });
            }
            "Ov" => {
                let values = parse_unsigned_numbers::<u16>(name, value)?;
                require_value_count(name, &values, 3)?;
                state.overrides = Some(ControllerOverrides {
                    feed_percent: values[0],
                    rapid_percent: values[1],
                    spindle_percent: values[2],
                });
            }
            "Pn" => state.pins = Some(parse_pins(value)),
            "A" => state.accessories = Some(parse_accessories(value)),
            "Ln" => {
                state.line_number =
                    Some(
                        value
                            .parse::<u64>()
                            .map_err(|_| StatusParseError::InvalidNumber {
                                field: name.to_owned(),
                                value: value.to_owned(),
                            })?,
                    );
            }
            _ => {}
        }
    }

    Ok(state)
}

pub fn build_device_inspection(responses: Vec<CommandResponse>) -> DeviceInspection {
    let mut inspection = DeviceInspection::default();

    for response in &responses {
        for line in &response.lines {
            match response.command.as_str() {
                "$I" => parse_identity_line(line, &mut inspection),
                "$$" => parse_setting_line(line, &mut inspection),
                "$G" => parse_modal_line(line, &mut inspection),
                "$#" => parse_parameter_line(line, &mut inspection),
                _ => {}
            }
        }
    }

    inspection.responses = responses;
    inspection
}

fn parse_identity_line(line: &str, inspection: &mut DeviceInspection) {
    if let Some(value) = bracket_value(line, "VER") {
        let (version, build_info) = value.split_once(':').unwrap_or((value, ""));
        inspection.firmware_version = non_empty(version);
        inspection.firmware_build_info = non_empty(build_info);
    } else if let Some(value) = bracket_value(line, "OPT") {
        inspection.firmware_options = non_empty(value);
        inspection.controller_capabilities = parse_capabilities(value);
    }
}

fn parse_capabilities(value: &str) -> Option<ControllerCapabilities> {
    let mut parts = value.split(',');
    let option_flags = parts.next()?.to_owned();
    Some(ControllerCapabilities {
        option_flags,
        planner_buffer_blocks: parts.next().and_then(|value| value.parse().ok()),
        rx_buffer_bytes: parts.next().and_then(|value| value.parse().ok()),
    })
}

fn parse_setting_line(line: &str, inspection: &mut DeviceInspection) {
    let Some((key, value)) = line.strip_prefix('$').and_then(|line| line.split_once('=')) else {
        return;
    };
    if !key.is_empty() && !value.is_empty() && key.bytes().all(|byte| byte.is_ascii_digit()) {
        inspection
            .settings
            .insert(format!("${key}"), value.to_owned());
    }
}

fn parse_modal_line(line: &str, inspection: &mut DeviceInspection) {
    let Some(value) = bracket_value(line, "GC") else {
        return;
    };
    inspection.modal_state = value.split_whitespace().map(str::to_owned).collect();
}

fn parse_parameter_line(line: &str, inspection: &mut DeviceInspection) {
    let Some(payload) = line
        .strip_prefix('[')
        .and_then(|line| line.strip_suffix(']'))
    else {
        return;
    };
    let Some((key, value)) = payload.split_once(':') else {
        return;
    };
    if !key.is_empty() && key != "GC" && key != "VER" && key != "OPT" {
        inspection
            .parameters
            .insert(key.to_owned(), value.to_owned());
    }
}

fn bracket_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.strip_prefix(&format!("[{key}:"))
        .and_then(|line| line.strip_suffix(']'))
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn parse_mode(value: &str) -> Result<(&str, Option<u8>), StatusParseError> {
    let Some((name, substate)) = value.split_once(':') else {
        return Ok((value, None));
    };

    let substate = substate
        .parse::<u8>()
        .map_err(|_| StatusParseError::InvalidNumber {
            field: "state".to_owned(),
            value: substate.to_owned(),
        })?;
    Ok((name, Some(substate)))
}

fn machine_mode(value: &str) -> MachineMode {
    match value {
        "Idle" => MachineMode::Idle,
        "Run" => MachineMode::Run,
        "Hold" => MachineMode::Hold,
        "Jog" => MachineMode::Jog,
        "Alarm" => MachineMode::Alarm,
        "Door" => MachineMode::Door,
        "Check" => MachineMode::Check,
        "Home" => MachineMode::Home,
        "Sleep" => MachineMode::Sleep,
        _ => MachineMode::Unknown,
    }
}

fn parse_position(field: &str, value: &str) -> Result<Position, StatusParseError> {
    let values = parse_numbers(field, value)?;
    if !(3..=4).contains(&values.len()) {
        return Err(StatusParseError::InvalidPosition {
            field: field.to_owned(),
        });
    }

    Ok(Position {
        x: values[0],
        y: values[1],
        z: values[2],
        a: values.get(3).copied(),
    })
}

fn parse_numbers(field: &str, value: &str) -> Result<Vec<f64>, StatusParseError> {
    value
        .split(',')
        .map(|part| {
            part.parse::<f64>()
                .map_err(|_| StatusParseError::InvalidNumber {
                    field: field.to_owned(),
                    value: part.to_owned(),
                })
        })
        .collect()
}

fn parse_unsigned_numbers<T>(field: &str, value: &str) -> Result<Vec<T>, StatusParseError>
where
    T: std::str::FromStr,
{
    value
        .split(',')
        .map(|part| {
            part.parse::<T>()
                .map_err(|_| StatusParseError::InvalidNumber {
                    field: field.to_owned(),
                    value: part.to_owned(),
                })
        })
        .collect()
}

fn require_value_count<T>(
    field: &str,
    values: &[T],
    expected: usize,
) -> Result<(), StatusParseError> {
    if values.len() == expected {
        Ok(())
    } else {
        Err(StatusParseError::InvalidValueCount {
            field: field.to_owned(),
            expected,
        })
    }
}

fn parse_pins(value: &str) -> ControllerPins {
    ControllerPins {
        raw: value.to_owned(),
        x_limit: value.contains('X'),
        y_limit: value.contains('Y'),
        z_limit: value.contains('Z'),
        a_limit: value.contains('A'),
        b_limit: value.contains('B'),
        c_limit: value.contains('C'),
        probe: value.contains('P'),
        door: value.contains('D'),
        hold: value.contains('H'),
        soft_reset: value.contains('R'),
        cycle_start: value.contains('S'),
    }
}

fn parse_accessories(value: &str) -> ControllerAccessories {
    ControllerAccessories {
        raw: value.to_owned(),
        spindle_clockwise: value.contains('S'),
        spindle_counterclockwise: value.contains('C'),
        flood_coolant: value.contains('F'),
        mist_coolant: value.contains('M'),
    }
}

#[cfg(test)]
mod tests {
    use millo_domain::{CommandCompletion, CommandResponse, JogAxis, StepJogRequest};

    use super::*;

    #[test]
    fn preserves_unknown_machine_modes() {
        let state = parse_status_line("<Calibrate|MPos:0,0,0>").unwrap();

        assert_eq!(state.mode, MachineMode::Unknown);
        assert_eq!(state.reported_mode, "Calibrate");
    }

    #[test]
    fn parses_hold_substate_and_a_axis() {
        let state = parse_status_line("<Hold:1|MPos:1,2,3,4|FS:120,9000>").unwrap();

        assert_eq!(state.mode, MachineMode::Hold);
        assert_eq!(state.substate, Some(1));
        assert_eq!(state.machine_position.unwrap().a, Some(4.0));
        assert_eq!(state.feed_rate, 120.0);
        assert_eq!(state.spindle_speed, 9000.0);
    }

    #[test]
    fn parses_buffers_overrides_pins_accessories_and_line_number() {
        let state =
            parse_status_line("<Run|MPos:1,2,3|Bf:15,127|Ov:95,50,110|Pn:XYZPDHRS|A:SCFM|Ln:73>")
                .unwrap();

        assert_eq!(state.buffer_state.unwrap().planner_available, 15);
        assert_eq!(state.buffer_state.unwrap().rx_available, 127);
        assert_eq!(state.overrides.unwrap().feed_percent, 95);
        assert_eq!(state.overrides.unwrap().rapid_percent, 50);
        assert_eq!(state.overrides.unwrap().spindle_percent, 110);
        let pins = state.pins.unwrap();
        assert!(pins.x_limit && pins.y_limit && pins.z_limit && pins.probe);
        assert!(pins.door && pins.hold && pins.soft_reset && pins.cycle_start);
        let accessories = state.accessories.unwrap();
        assert!(accessories.spindle_clockwise && accessories.spindle_counterclockwise);
        assert!(accessories.flood_coolant && accessories.mist_coolant);
        assert_eq!(state.line_number, Some(73));
    }

    #[test]
    fn rejects_malformed_typed_telemetry() {
        assert_eq!(
            parse_status_line("<Idle|MPos:0,0,0|Bf:15>").unwrap_err(),
            StatusParseError::InvalidValueCount {
                field: "Bf".to_owned(),
                expected: 2,
            }
        );
        assert!(matches!(
            parse_status_line("<Idle|MPos:0,0,0|Ln:-1>"),
            Err(StatusParseError::InvalidNumber { field, .. }) if field == "Ln"
        ));
    }

    #[test]
    fn classifies_reset_banner() {
        let line = parse_incoming_line("Grbl 1.1h ['$' for help]").unwrap();

        assert_eq!(
            line,
            IncomingLine::ResetBanner {
                raw: "Grbl 1.1h ['$' for help]".to_owned(),
                version: Some("1.1h".to_owned()),
            }
        );
    }

    #[test]
    fn classifies_alarm_with_code() {
        let line = parse_incoming_line("ALARM:3").unwrap();

        assert_eq!(
            line,
            IncomingLine::Alarm {
                code: Some(3),
                raw: "ALARM:3".to_owned(),
            }
        );
    }

    #[test]
    fn builds_device_inspection_from_grbl_queries() {
        let responses = vec![
            response("$I", &["[VER:1.1h.20240101:Millo Mock]", "[OPT:V,15,128]"]),
            response("$$", &["$0=10", "$30=12000", "not-a-setting"]),
            response("$G", &["[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0 S0]"]),
            response("$#", &["[G54:1.000,2.000,3.000]", "[TLO:0.000]"]),
        ];

        let inspection = build_device_inspection(responses);

        assert_eq!(
            inspection.firmware_version.as_deref(),
            Some("1.1h.20240101")
        );
        assert_eq!(
            inspection.firmware_build_info.as_deref(),
            Some("Millo Mock")
        );
        assert_eq!(inspection.firmware_options.as_deref(), Some("V,15,128"));
        assert_eq!(
            inspection
                .controller_capabilities
                .as_ref()
                .and_then(|capabilities| capabilities.rx_buffer_bytes),
            Some(128)
        );
        assert_eq!(
            inspection.settings.get("$30").map(String::as_str),
            Some("12000")
        );
        assert_eq!(inspection.modal_state[1], "G54");
        assert_eq!(
            inspection.parameters.get("G54").map(String::as_str),
            Some("1.000,2.000,3.000")
        );
        assert_eq!(inspection.responses.len(), 4);
    }

    #[test]
    fn encodes_a_single_axis_incremental_metric_jog() {
        let command = encode_step_jog(StepJogRequest {
            authorization_id: 42,
            axis: JogAxis::Y,
            distance_mm: -0.1,
            feed_mm_per_min: 50.0,
        })
        .unwrap();

        assert_eq!(command, "$J=G91 G21 Y-0.100 F50.000");
        assert!(!command.contains('X'));
        assert!(!command.contains('Z'));
    }

    #[test]
    fn accepts_only_the_hard_step_jog_envelope() {
        let request = |distance_mm, feed_mm_per_min| StepJogRequest {
            authorization_id: 1,
            axis: JogAxis::X,
            distance_mm,
            feed_mm_per_min,
        };

        assert!(encode_step_jog(request(0.01, 10.0)).is_ok());
        assert!(encode_step_jog(request(-100_000.0, 100_000.0)).is_ok());
        assert_eq!(
            encode_step_jog(request(0.0, 50.0)),
            Err(JogValidationError::InvalidDistance)
        );
        assert!(matches!(
            encode_step_jog(request(100_000.001, 50.0)),
            Err(JogValidationError::DistanceOutOfRange { .. })
        ));
        assert_eq!(
            encode_step_jog(request(f64::NAN, 50.0)),
            Err(JogValidationError::InvalidDistance)
        );
        assert!(matches!(
            encode_step_jog(request(0.1, 9.999)),
            Err(JogValidationError::FeedOutOfRange { .. })
        ));
        assert!(matches!(
            encode_step_jog(request(0.1, 100_000.001)),
            Err(JogValidationError::FeedOutOfRange { .. })
        ));
        assert_eq!(
            encode_step_jog(request(0.1, f64::INFINITY)),
            Err(JogValidationError::InvalidFeed)
        );
    }

    #[test]
    fn encodes_work_zero_only_for_typed_axis_and_coordinate_system() {
        assert_eq!(
            encode_set_work_zero(WorkAxis::X, WorkCoordinateSystem::G54),
            "G10 L20 P1 X0"
        );
        assert_eq!(
            encode_set_work_zero(WorkAxis::Y, WorkCoordinateSystem::G56),
            "G10 L20 P3 Y0"
        );
        assert_eq!(
            encode_set_work_zero(WorkAxis::Z, WorkCoordinateSystem::G59),
            "G10 L20 P6 Z0"
        );
    }

    #[test]
    fn finds_only_supported_active_work_coordinate_systems() {
        assert_eq!(
            active_work_coordinate_system(&["G0".to_owned(), "G55".to_owned()]),
            Some(WorkCoordinateSystem::G55)
        );
        assert_eq!(
            active_work_coordinate_system(&["G0".to_owned(), "G53".to_owned()]),
            None
        );
    }

    fn response(command: &str, lines: &[&str]) -> CommandResponse {
        CommandResponse {
            command: command.to_owned(),
            completion: CommandCompletion::Ok,
            lines: lines.iter().map(|line| (*line).to_owned()).collect(),
            code: None,
        }
    }
}
