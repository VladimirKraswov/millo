use millo_controller::DeviceQuery;
use millo_domain::{
    CommandCompletion, CommandResponse, ControllerSnapshot, OperatorConsoleCommandKind,
    OperatorConsoleExchange, Position,
};

use crate::ArbiterError;

const MAX_EXPERT_COMMAND_BYTES: usize = 255;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConsoleCommand {
    Status,
    Query {
        query: DeviceQuery,
        kind: OperatorConsoleCommandKind,
    },
    Raw(String),
}

impl ConsoleCommand {
    pub(crate) fn parse(input: &str, expert_mode: bool) -> Result<Self, ArbiterError> {
        let command = input.trim();
        if command.is_empty() || !command.is_ascii() {
            return Err(ArbiterError::OperatorConsoleCommandRejected);
        }
        if command.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(ArbiterError::OperatorConsoleCommandRejected);
        }

        let safe = match command.to_ascii_uppercase().as_str() {
            "?" => Ok(Self::Status),
            "$I" => Ok(Self::Query {
                query: DeviceQuery::BuildInfo,
                kind: OperatorConsoleCommandKind::BuildInfo,
            }),
            "$$" => Ok(Self::Query {
                query: DeviceQuery::Settings,
                kind: OperatorConsoleCommandKind::Settings,
            }),
            "$G" => Ok(Self::Query {
                query: DeviceQuery::ModalState,
                kind: OperatorConsoleCommandKind::ModalState,
            }),
            "$#" => Ok(Self::Query {
                query: DeviceQuery::Parameters,
                kind: OperatorConsoleCommandKind::Parameters,
            }),
            _ => Err(ArbiterError::OperatorConsoleCommandRejected),
        };
        if safe.is_ok() {
            return safe;
        }
        if !expert_mode || command.len() > MAX_EXPERT_COMMAND_BYTES || matches!(command, "!" | "~")
        {
            return Err(ArbiterError::OperatorConsoleCommandRejected);
        }
        Ok(Self::Raw(command.to_owned()))
    }

    pub(crate) fn normalized(&self) -> &str {
        match self {
            Self::Status => "?",
            Self::Query { query, .. } => query.command(),
            Self::Raw(command) => command,
        }
    }
}

pub(crate) fn status_exchange(snapshot: ControllerSnapshot) -> OperatorConsoleExchange {
    OperatorConsoleExchange {
        command: ConsoleCommand::Status.normalized().to_owned(),
        kind: OperatorConsoleCommandKind::Status,
        completion: CommandCompletion::Ok,
        lines: vec![format_status(&snapshot)],
        code: None,
        snapshot,
    }
}

pub(crate) fn query_exchange(
    kind: OperatorConsoleCommandKind,
    response: CommandResponse,
    snapshot: ControllerSnapshot,
) -> OperatorConsoleExchange {
    OperatorConsoleExchange {
        command: response.command,
        kind,
        completion: response.completion,
        lines: response.lines,
        code: response.code,
        snapshot,
    }
}

pub(crate) fn raw_exchange(
    response: CommandResponse,
    snapshot: ControllerSnapshot,
) -> OperatorConsoleExchange {
    query_exchange(OperatorConsoleCommandKind::Raw, response, snapshot)
}

fn format_status(snapshot: &ControllerSnapshot) -> String {
    let machine = &snapshot.machine;
    let mut fields = vec![machine.reported_mode.clone()];
    if let Some(position) = machine.machine_position {
        fields.push(format!("MPos:{}", format_position(position)));
    }
    if let Some(position) = machine.work_position {
        fields.push(format!("WPos:{}", format_position(position)));
    }
    fields.push(format!(
        "FS:{:.1},{:.0}",
        machine.feed_rate, machine.spindle_speed
    ));
    if let Some(pins) = &machine.pins
        && !pins.raw.is_empty()
    {
        fields.push(format!("Pn:{}", pins.raw));
    }
    format!("<{}>", fields.join("|"))
}

fn format_position(position: Position) -> String {
    match position.a {
        Some(a) => format!(
            "{:.3},{:.3},{:.3},{a:.3}",
            position.x, position.y, position.z
        ),
        None => format!("{:.3},{:.3},{:.3}", position.x, position.y, position.z),
    }
}

#[cfg(test)]
mod tests {
    use millo_domain::{ControllerPins, MachineMode, MachineState, Position};

    use super::*;

    #[test]
    fn accepts_only_the_read_only_operator_allowlist() {
        assert_eq!(
            ConsoleCommand::parse(" ? ", false).unwrap().normalized(),
            "?"
        );
        assert_eq!(
            ConsoleCommand::parse("$i", false).unwrap().normalized(),
            "$I"
        );
        assert_eq!(
            ConsoleCommand::parse("$$", false).unwrap().normalized(),
            "$$"
        );
        assert_eq!(
            ConsoleCommand::parse("$g", false).unwrap().normalized(),
            "$G"
        );
        assert_eq!(
            ConsoleCommand::parse("$#", false).unwrap().normalized(),
            "$#"
        );

        for rejected in [
            "", "G0 X1", "$100=1", "$X", "$H", "!", "~", "\u{18}", "$I\n$$",
        ] {
            assert!(
                matches!(
                    ConsoleCommand::parse(rejected, false),
                    Err(ArbiterError::OperatorConsoleCommandRejected)
                ),
                "{rejected:?} must not cross the safe-console policy"
            );
        }
    }

    #[test]
    fn expert_mode_accepts_one_bounded_line_but_keeps_realtime_controls_typed() {
        assert_eq!(
            ConsoleCommand::parse(" G0 X1.25 ", true)
                .unwrap()
                .normalized(),
            "G0 X1.25"
        );
        assert_eq!(
            ConsoleCommand::parse("$100=1600", true)
                .unwrap()
                .normalized(),
            "$100=1600"
        );
        for rejected in ["!", "~", "$I\n$$", "\u{18}"] {
            assert!(matches!(
                ConsoleCommand::parse(rejected, true),
                Err(ArbiterError::OperatorConsoleCommandRejected)
            ));
        }
    }

    #[test]
    fn status_is_rendered_from_the_actor_snapshot_without_raw_transport_access() {
        let snapshot = ControllerSnapshot {
            machine: MachineState {
                mode: MachineMode::Idle,
                reported_mode: "Idle".to_owned(),
                machine_position: Some(Position {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                    a: Some(45.0),
                }),
                work_position: Some(Position {
                    x: 0.5,
                    y: 1.5,
                    z: 2.5,
                    a: Some(45.0),
                }),
                feed_rate: 120.0,
                spindle_speed: 8000.0,
                pins: Some(ControllerPins {
                    raw: "P".to_owned(),
                    probe: true,
                    ..ControllerPins::default()
                }),
                ..MachineState::default()
            },
            ..ControllerSnapshot::default()
        };

        assert_eq!(
            status_exchange(snapshot).lines,
            [
                "<Idle|MPos:1.000,2.000,3.000,45.000|WPos:0.500,1.500,2.500,45.000|FS:120.0,8000|Pn:P>"
            ]
        );
    }
}
