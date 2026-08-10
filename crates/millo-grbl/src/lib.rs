use millo_domain::{MachineMode, MachineState, Position};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum IncomingLine {
    Status(MachineState),
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
}

pub fn parse_incoming_line(line: &str) -> Result<IncomingLine, StatusParseError> {
    let line = line.trim();

    if line.starts_with('<') {
        return parse_status_line(line).map(IncomingLine::Status);
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
            _ => {}
        }
    }

    Ok(state)
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

#[cfg(test)]
mod tests {
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
}
