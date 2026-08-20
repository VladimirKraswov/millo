use millo_controller::DeviceQuery;
use millo_domain::{
    CommandCompletion, CommandResponse, ControllerSnapshot, OperatorConsoleCommandKind,
    OperatorConsoleExchange, Position,
};

use crate::ArbiterError;

const MAX_COMMAND_BYTES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SafeConsoleCommand {
    Status,
    Query {
        query: DeviceQuery,
        kind: OperatorConsoleCommandKind,
    },
}

impl SafeConsoleCommand {
    pub(crate) fn parse(input: &str) -> Result<Self, ArbiterError> {
        let command = input.trim();
        if command.is_empty() || command.len() > MAX_COMMAND_BYTES || !command.is_ascii() {
            return Err(ArbiterError::OperatorConsoleCommandRejected);
        }
        if command.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(ArbiterError::OperatorConsoleCommandRejected);
        }

        match command.to_ascii_uppercase().as_str() {
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
        }
    }

    pub(crate) const fn normalized(self) -> &'static str {
        match self {
            Self::Status => "?",
            Self::Query { query, .. } => query.command(),
        }
    }
}

pub(crate) fn status_exchange(snapshot: ControllerSnapshot) -> OperatorConsoleExchange {
    OperatorConsoleExchange {
        command: SafeConsoleCommand::Status.normalized().to_owned(),
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
        assert_eq!(SafeConsoleCommand::parse(" ? ").unwrap().normalized(), "?");
        assert_eq!(SafeConsoleCommand::parse("$i").unwrap().normalized(), "$I");
        assert_eq!(SafeConsoleCommand::parse("$$").unwrap().normalized(), "$$");
        assert_eq!(SafeConsoleCommand::parse("$g").unwrap().normalized(), "$G");
        assert_eq!(SafeConsoleCommand::parse("$#").unwrap().normalized(), "$#");

        for rejected in [
            "", "G0 X1", "$100=1", "$X", "$H", "!", "~", "\u{18}", "$I\n$$",
        ] {
            assert!(
                matches!(
                    SafeConsoleCommand::parse(rejected),
                    Err(ArbiterError::OperatorConsoleCommandRejected)
                ),
                "{rejected:?} must not cross the safe-console policy"
            );
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
