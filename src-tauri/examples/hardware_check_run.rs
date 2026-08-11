use std::{error::Error, fs, io, path::PathBuf, time::Duration};

use millo_command::{CommandArbiter, ExecutionTarget};
use millo_controller::ControllerConfig;
use millo_domain::{HardwareProfile, MachineMode};
use millo_dry_run::ProgramExecutionOptions;
use millo_gcode::{ProgramParseOptions, ProgramParseRequest, parse_program_with_options};
use millo_run::{ProgramRunIntent, RunPreflightLevel};
use millo_sender::SenderState;
use millo_serial::{SerialConfig, SerialTransport};

const USAGE: &str =
    "usage: hardware_check_run <serial-port> <program.nc> [--optional-stop] [--block-delete]";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let port = args.next().ok_or_else(|| input_error(USAGE))?;
    let path = PathBuf::from(args.next().ok_or_else(|| input_error(USAGE))?);
    let mut execution_options = ProgramExecutionOptions::default();
    for argument in args {
        match argument.as_str() {
            "--optional-stop" => execution_options.optional_stop = true,
            "--block-delete" => execution_options.block_delete = true,
            _ => return Err(input_error(USAGE).into()),
        }
    }

    let source = fs::read_to_string(&path)?;
    let source_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| input_error("program path has no UTF-8 file name"))?
        .to_owned();
    let program = parse_program_with_options(
        ProgramParseRequest {
            source_name,
            source,
        },
        ProgramParseOptions {
            block_delete: execution_options.block_delete,
        },
    )?;
    println!(
        "Parsed {} source lines, {} motions, {:.3} mm of cutting geometry",
        program.summary.line_count,
        program.summary.motion_count,
        program.summary.cutting_distance_mm
    );

    let transport = SerialTransport::new(SerialConfig::new(port.clone(), 115_200)?);
    let (arbiter, worker) = CommandArbiter::new_with_execution_target(
        Box::new(transport),
        ControllerConfig::default(),
        HardwareProfile::first_machine(),
        ExecutionTarget::Serial,
    );
    let worker = tokio::spawn(worker);
    let result = run(&arbiter, &port, program, execution_options).await;
    let _ = arbiter.disconnect().await;
    worker.abort();
    result
}

async fn run(
    arbiter: &CommandArbiter,
    port: &str,
    program: millo_gcode::GcodeProgram,
    execution_options: ProgramExecutionOptions,
) -> Result<(), Box<dyn Error>> {
    println!("Connecting to {port} at 115200 baud");
    arbiter.connect().await?;
    let mut snapshot = arbiter.refresh_status().await?;
    if snapshot.reset_notice.is_some() {
        snapshot = arbiter.acknowledge_reset().await?;
    }
    if snapshot.machine.mode == MachineMode::Alarm {
        snapshot = arbiter.unlock_alarm(true).await?;
    }
    if snapshot.machine.mode != MachineMode::Idle {
        return Err(input_error(format!(
            "controller must be Idle, got {:?}",
            snapshot.machine.mode
        ))
        .into());
    }

    let preflight_program = program.clone();
    let mut updates = arbiter.subscribe_sender();
    let started = arbiter
        .start_check_run_with_options(program, execution_options)
        .await?;
    println!(
        "CHECK START: {} line(s), controller RX capacity {} byte(s), optional stop {}, block delete {}",
        started.total_lines,
        started.rx_buffer_capacity,
        execution_options.optional_stop,
        execution_options.block_delete,
    );

    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let sender = updates.borrow_and_update().clone();
        println!(
            "{:?}: {}/{}{}",
            sender.state,
            sender.acknowledged_lines,
            sender.total_lines,
            sender
                .current_source_line
                .map(|line| format!(" at L{line}"))
                .unwrap_or_default()
        );
        match sender.state {
            SenderState::Completed => break,
            SenderState::Failed | SenderState::Cancelled => {
                return Err(input_error(format!(
                    "Check run ended as {:?}: {}",
                    sender.state,
                    sender.last_error.as_deref().unwrap_or("no error detail")
                ))
                .into());
            }
            _ => {}
        }

        tokio::select! {
            changed = updates.changed() => {
                if changed.is_err() {
                    return Err(input_error("sender event stream closed").into());
                }
            }
            _ = tokio::signal::ctrl_c() => {
                let _ = arbiter.cancel_dry_run().await;
                return Err(input_error("Check run interrupted").into());
            }
            _ = tokio::time::sleep_until(deadline) => {
                let _ = arbiter.cancel_dry_run().await;
                return Err(input_error("Check run exceeded 60 seconds").into());
            }
        }
    }

    let final_snapshot = arbiter.refresh_status().await?;
    if final_snapshot.machine.mode != MachineMode::Idle {
        return Err(input_error(format!(
            "Check run cleanup expected Idle, got {:?}",
            final_snapshot.machine.mode
        ))
        .into());
    }
    let preflight = arbiter
        .preflight_real_run_with_options(
            preflight_program,
            ProgramRunIntent::Cutting,
            execution_options,
        )
        .await?;
    let certificate = preflight
        .checks
        .iter()
        .find(|check| check.id == "grbl-check-certificate")
        .ok_or_else(|| input_error("Cutting preflight did not report a Check certificate"))?;
    if certificate.level != RunPreflightLevel::Pass {
        return Err(input_error(format!(
            "Check certificate was not accepted: {}",
            certificate.detail
        ))
        .into());
    }
    println!(
        "PASS: every line was accepted, GRBL returned to Idle, and Cutting preflight accepted the certificate"
    );
    Ok(())
}

fn input_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
