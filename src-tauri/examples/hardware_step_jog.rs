use std::{error::Error, io, time::Duration};

use millo_command::CommandArbiter;
use millo_controller::ControllerConfig;
use millo_domain::{
    HardwareProfile, JogAxis, MachineMode, OperatorConfirmation, ReadinessLevel, StepJogRequest,
};
use millo_serial::{SerialConfig, SerialTransport};

const CONFIRM_FLAG: &str = "--confirm-motion";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let port = args
        .next()
        .ok_or_else(|| input_error("usage: hardware_step_jog <serial-port> --confirm-motion"))?;
    if args.next().as_deref() != Some(CONFIRM_FLAG) || args.next().is_some() {
        return Err(
            input_error("motion confirmation missing; pass exactly --confirm-motion").into(),
        );
    }

    let transport = SerialTransport::new(SerialConfig::new(port.clone(), 115_200)?);
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig::default(),
        HardwareProfile::first_machine(),
    );
    let worker = tokio::spawn(worker);

    let result = run_smoke(&arbiter, &port).await;
    let _ = arbiter.disconnect().await;
    worker.abort();
    result
}

async fn run_smoke(arbiter: &CommandArbiter, port: &str) -> Result<(), Box<dyn Error>> {
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
            axis: JogAxis::X,
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
    let delta = after.x - before.x;
    if (delta - 0.1).abs() > 0.02
        || (after.y - before.y).abs() > 0.001
        || (after.z - before.z).abs() > 0.001
    {
        return Err(input_error(format!(
            "unexpected motion delta: X {delta:.3}, Y {:.3}, Z {:.3}",
            after.y - before.y,
            after.z - before.z
        ))
        .into());
    }

    println!(
        "PASS: Idle, X {:+.3} mm, Y {:+.3} mm, Z {:+.3} mm",
        delta,
        after.y - before.y,
        after.z - before.z
    );
    Ok(())
}

fn input_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
