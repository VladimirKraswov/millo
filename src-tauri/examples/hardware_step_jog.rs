use std::{error::Error, io, time::Duration};

use millo_command::CommandArbiter;
use millo_controller::ControllerConfig;
use millo_domain::{
    HardwareProfile, JogAxis, MachineMode, OperatorConfirmation, ReadinessLevel, StepJogRequest,
};
use millo_serial::{SerialConfig, SerialTransport};

const CONFIRM_MOTION_FLAG: &str = "--confirm-motion";
const CONFIRM_CONFIGURATION_FLAG: &str = "--confirm-disable-limits-and-homing";
const USAGE: &str = "usage: hardware_step_jog <serial-port> <axis:X|Y|Z> \
                     --confirm-disable-limits-and-homing --confirm-motion";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let port = args.next().ok_or_else(|| input_error(USAGE))?;
    let axis = args
        .next()
        .as_deref()
        .and_then(parse_axis)
        .ok_or_else(|| input_error(USAGE))?;
    let flags = args.collect::<Vec<_>>();
    if flags.len() != 2
        || !flags.iter().any(|flag| flag == CONFIRM_CONFIGURATION_FLAG)
        || !flags.iter().any(|flag| flag == CONFIRM_MOTION_FLAG)
    {
        return Err(input_error(format!(
            "both persistent-configuration and motion confirmations are required; {USAGE}"
        ))
        .into());
    }

    let transport = SerialTransport::new(SerialConfig::new(port.clone(), 115_200)?);
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig::default(),
        HardwareProfile::first_machine(),
    );
    let worker = tokio::spawn(worker);

    let result = run_smoke(&arbiter, &port, axis).await;
    let _ = arbiter.disconnect().await;
    worker.abort();
    result
}

async fn run_smoke(
    arbiter: &CommandArbiter,
    port: &str,
    axis: JogAxis,
) -> Result<(), Box<dyn Error>> {
    println!("Connecting to {port} at 115200 baud");
    arbiter.connect().await?;
    let mut snapshot = arbiter.refresh_status().await?;
    if snapshot.reset_notice.is_some() {
        snapshot = arbiter.acknowledge_reset().await?;
    }
    if snapshot.machine.mode != MachineMode::Idle {
        return Err(input_error(format!(
            "controller must be Idle, got {:?}",
            snapshot.machine.mode
        ))
        .into());
    }

    let configuration = arbiter.configure_unhomed_operation().await?;
    println!(
        "Settings: $21 {} -> {}, $22 {} -> {} ({} write(s))",
        setting(&configuration.before, "$21"),
        setting(&configuration.after, "$21"),
        setting(&configuration.before, "$22"),
        setting(&configuration.after, "$22"),
        configuration.writes.len()
    );
    snapshot = arbiter.refresh_status().await?;
    if snapshot.machine.mode != MachineMode::Idle {
        return Err(input_error(format!(
            "controller left Idle after configuration: {:?}",
            snapshot.machine.mode
        ))
        .into());
    }

    let before = snapshot
        .machine
        .machine_position
        .ok_or_else(|| input_error("controller status has no machine position"))?;
    let preparation = arbiter
        .prepare_test_jog(OperatorConfirmation {
            spindle_off: true,
            tool_clear: true,
            power_control_reachable: true,
        })
        .await?;
    let authorization = match preparation.authorization {
        Some(authorization) => authorization,
        None => {
            for check in preparation.inspection.readiness.checks {
                if check.level == ReadinessLevel::Blocker {
                    eprintln!(
                        "BLOCKER {}: {}{}",
                        check.id,
                        check.detail,
                        check
                            .evidence
                            .as_deref()
                            .map(|evidence| format!(" ({evidence})"))
                            .unwrap_or_default()
                    );
                }
            }
            return Err(input_error("hardware readiness did not authorize test jog").into());
        }
    };

    let receipt = arbiter
        .step_jog(StepJogRequest {
            authorization_id: authorization.id,
            axis,
            distance_mm: 0.1,
            feed_mm_per_min: 10.0,
        })
        .await?;
    println!("Accepted: {}", receipt.command);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        snapshot = arbiter.refresh_status().await?;
        if snapshot.machine.mode == MachineMode::Idle {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            if snapshot.machine.mode == MachineMode::Jog {
                let _ = arbiter.cancel_jog().await;
            }
            return Err(input_error("step jog did not return to Idle within 5 seconds").into());
        }
    }

    let after = snapshot
        .machine
        .machine_position
        .ok_or_else(|| input_error("final status has no machine position"))?;
    let deltas = [after.x - before.x, after.y - before.y, after.z - before.z];
    let selected_index = match axis {
        JogAxis::X => 0,
        JogAxis::Y => 1,
        JogAxis::Z => 2,
        JogAxis::A => {
            return Err(input_error("A axis is not supported by this XYZ hardware smoke").into());
        }
    };
    let positions_match = deltas.iter().enumerate().all(|(index, delta)| {
        let expected = if index == selected_index { 0.1 } else { 0.0 };
        let tolerance = if index == selected_index { 0.02 } else { 0.001 };
        (*delta - expected).abs() <= tolerance
    });

    if !positions_match {
        return Err(input_error(format!(
            "unexpected motion delta after {} jog: X {:+.3}, Y {:+.3}, Z {:+.3}",
            axis_name(axis),
            deltas[0],
            deltas[1],
            deltas[2]
        ))
        .into());
    }

    println!(
        "PASS: {} jog completed; state=Idle, delta X {:+.3} mm, Y {:+.3} mm, Z {:+.3} mm",
        axis_name(axis),
        deltas[0],
        deltas[1],
        deltas[2]
    );
    Ok(())
}

fn parse_axis(value: &str) -> Option<JogAxis> {
    match value.to_ascii_uppercase().as_str() {
        "X" => Some(JogAxis::X),
        "Y" => Some(JogAxis::Y),
        "Z" => Some(JogAxis::Z),
        _ => None,
    }
}

fn axis_name(axis: JogAxis) -> &'static str {
    match axis {
        JogAxis::X => "X",
        JogAxis::Y => "Y",
        JogAxis::Z => "Z",
        JogAxis::A => "A",
    }
}

fn input_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn setting<'a>(inspection: &'a millo_domain::DeviceInspection, key: &str) -> &'a str {
    inspection
        .settings
        .get(key)
        .map(String::as_str)
        .unwrap_or("missing")
}
