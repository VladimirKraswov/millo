use std::{error::Error, fs, io, path::PathBuf, time::Duration};

use millo_command::{CommandArbiter, ExecutionTarget};
use millo_controller::ControllerConfig;
use millo_domain::{
    ControllerSnapshot, DeviceInspection, HardwareProfile, MachineMode, MachineTravel, Position,
    ProbeWorkflowMode, SpindleControl,
};
use millo_dry_run::ProgramExecutionOptions;
use millo_gcode::{GcodeProgram, ProgramParseRequest, parse_program};
use millo_run::{FirstCutConfirmation, ProgramRunIntent, RunPreflightLevel};
use millo_sender::{SenderSnapshot, SenderState};
use millo_serial::{SerialConfig, SerialTransport};

const REQUIRED_FLAGS: [&str; 8] = [
    "--execute-cut",
    "--confirm-stock-secured",
    "--confirm-tool-secured",
    "--confirm-xyz-zero",
    "--confirm-safe-z",
    "--confirm-spindle-running",
    "--confirm-path-clear",
    "--confirm-power-control",
];
const USAGE: &str = "usage: hardware_cut_run <serial-port> <program.nc> --execute-cut --confirm-stock-secured --confirm-tool-secured --confirm-xyz-zero --confirm-safe-z --confirm-spindle-running --confirm-path-clear --confirm-power-control";

struct Args {
    port: String,
    program_path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_args(std::env::args().skip(1))?;
    let source = fs::read_to_string(&args.program_path)?;
    let source_name = args
        .program_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| input_error("program path has no UTF-8 file name"))?
        .to_owned();
    let program = parse_program(ProgramParseRequest {
        source_name,
        source,
    })?;
    validate_program_envelope(&program, &hardware_profile())?;

    let transport = SerialTransport::new(SerialConfig::new(&args.port, 115_200)?);
    let (arbiter, worker) = CommandArbiter::new_with_execution_target(
        Box::new(transport),
        ControllerConfig::default(),
        hardware_profile(),
        ExecutionTarget::Serial,
    );
    let worker = tokio::spawn(worker);
    let result = execute(&arbiter, &args.port, program).await;
    let _ = arbiter.disconnect().await;
    worker.abort();
    result
}

async fn execute(
    arbiter: &CommandArbiter,
    port: &str,
    program: GcodeProgram,
) -> Result<(), Box<dyn Error>> {
    let bounds = program
        .summary
        .bounds
        .ok_or_else(|| input_error("program has no motion bounds"))?;
    println!(
        "PROGRAM: {} source lines, {} motions, bounds X {:.3}..{:.3}, Y {:.3}..{:.3}, Z {:.3}..{:.3} mm, ETA {:.1} s",
        program.summary.line_count,
        program.summary.motion_count,
        bounds.min.x,
        bounds.max.x,
        bounds.min.y,
        bounds.max.y,
        bounds.min.z,
        bounds.max.z,
        program.summary.estimated_total_time_seconds,
    );
    println!("CONNECT: {port} at 115200 baud");
    arbiter.connect().await?;
    let mut controller = arbiter.refresh_status().await?;
    if controller.reset_notice.is_some() {
        controller = arbiter.acknowledge_reset().await?;
    }
    if controller.machine.mode == MachineMode::Alarm {
        controller = arbiter.unlock_alarm(true).await?;
    }
    ensure_idle(&controller)?;

    let mut sender_updates = arbiter.subscribe_sender();
    let check = arbiter.start_check_run(program.clone()).await?;
    println!(
        "CHECK START: {} sender lines, RX window {} bytes",
        check.total_lines, check.rx_buffer_capacity
    );
    monitor_sender(
        arbiter,
        &mut sender_updates,
        "CHECK",
        Duration::from_secs(120),
        false,
    )
    .await?;

    let report = arbiter
        .preflight_real_run(program.clone(), ProgramRunIntent::Cutting)
        .await?;
    print_preflight(&report);
    if !report.ready {
        return Err(input_error("Cutting preflight is blocked after GRBL Check").into());
    }
    let controller = arbiter.refresh_status().await?;
    ensure_idle(&controller)?;
    let work_position = observed_work_position(&controller, &report.hardware.device)
        .ok_or_else(|| input_error("controller exposes neither WPos nor a usable WCS offset"))?;
    println!(
        "READY: WPos X {:+.3}, Y {:+.3}, Z {:+.3} mm; active WCS {}; manual spindle",
        work_position.x,
        work_position.y,
        work_position.z,
        active_wcs(&report.hardware.device).unwrap_or("unknown"),
    );

    let preparation = arbiter
        .authorize_first_cut(
            program.clone(),
            FirstCutConfirmation {
                intent: ProgramRunIntent::Cutting,
                execution_options: ProgramExecutionOptions::default(),
                stock_secured: true,
                tool_secured: true,
                tool_removed: false,
                xyz_zero_verified: true,
                safe_z_verified: true,
                manual_spindle_running: true,
                manual_spindle_off: false,
                probe_removed: true,
                path_clear: true,
                power_control_reachable: true,
            },
        )
        .await?;
    let started = arbiter
        .start_program_run(program.clone(), preparation.authorization.id)
        .await?;
    println!(
        "CUT START: {} sender lines, RX window {} bytes, authorization #{}",
        started.total_lines, started.rx_buffer_capacity, preparation.authorization.id
    );
    let deadline_seconds = (program.summary.estimated_total_time_seconds * 3.0 + 60.0)
        .ceil()
        .clamp(180.0, 3_600.0) as u64;
    let completed = monitor_sender(
        arbiter,
        &mut sender_updates,
        "CUT",
        Duration::from_secs(deadline_seconds),
        true,
    )
    .await?;

    let final_controller = arbiter.refresh_status().await?;
    ensure_idle(&final_controller)?;
    let final_work = observed_work_position(&final_controller, &report.hardware.device)
        .ok_or_else(|| input_error("final WPos is unavailable"))?;
    println!(
        "PASS: CUT completed {}/{} in {:.1} s; final WPos X {:+.3}, Y {:+.3}, Z {:+.3} mm",
        completed.acknowledged_lines,
        completed.total_lines,
        completed.elapsed_seconds,
        final_work.x,
        final_work.y,
        final_work.z,
    );
    Ok(())
}

async fn monitor_sender(
    arbiter: &CommandArbiter,
    updates: &mut tokio::sync::watch::Receiver<SenderSnapshot>,
    label: &str,
    timeout: Duration,
    physical: bool,
) -> Result<SenderSnapshot, Box<dyn Error>> {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut last_reported = usize::MAX;
    loop {
        let snapshot = updates.borrow_and_update().clone();
        let report_progress = last_reported == usize::MAX
            || snapshot.acknowledged_lines == snapshot.total_lines
            || snapshot.acknowledged_lines >= last_reported.saturating_add(25);
        if report_progress {
            println!(
                "{label}: {:?} {}/{} ({:.1}%){}; ACK age {:.2} s",
                snapshot.state,
                snapshot.acknowledged_lines,
                snapshot.total_lines,
                snapshot.progress * 100.0,
                snapshot
                    .current_source_line
                    .map(|line| format!(" at L{line}"))
                    .unwrap_or_default(),
                snapshot.seconds_since_acknowledgement,
            );
            last_reported = snapshot.acknowledged_lines;
        }
        match snapshot.state {
            SenderState::Completed => return Ok(snapshot),
            SenderState::Failed | SenderState::Cancelled => {
                if physical {
                    emergency_stop(arbiter).await;
                }
                return Err(input_error(format!(
                    "{label} ended as {:?}: {}",
                    snapshot.state,
                    snapshot.last_error.as_deref().unwrap_or("no error detail")
                ))
                .into());
            }
            _ => {}
        }

        tokio::select! {
            changed = updates.changed() => {
                if changed.is_err() {
                    if physical {
                        emergency_stop(arbiter).await;
                    }
                    return Err(input_error("sender event stream closed").into());
                }
            }
            _ = tokio::signal::ctrl_c() => {
                if physical {
                    emergency_stop(arbiter).await;
                } else {
                    let _ = arbiter.cancel_dry_run().await;
                }
                return Err(input_error(format!("{label} interrupted")).into());
            }
            _ = tokio::time::sleep_until(deadline) => {
                if physical {
                    emergency_stop(arbiter).await;
                } else {
                    let _ = arbiter.cancel_dry_run().await;
                }
                return Err(input_error(format!("{label} exceeded {} seconds", timeout.as_secs())).into());
            }
        }
    }
}

async fn emergency_stop(arbiter: &CommandArbiter) {
    let _ = arbiter.feed_hold().await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    if let Ok(challenge) = arbiter.request_soft_reset().await {
        let _ = arbiter.confirm_soft_reset(challenge.id).await;
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Args, io::Error> {
    let mut args = args.into_iter();
    let port = args.next().ok_or_else(|| input_error(USAGE))?;
    let program_path = PathBuf::from(args.next().ok_or_else(|| input_error(USAGE))?);
    let flags = args.collect::<Vec<_>>();
    let exact_flags = flags.len() == REQUIRED_FLAGS.len()
        && REQUIRED_FLAGS
            .iter()
            .all(|required| flags.iter().any(|flag| flag == required));
    if !exact_flags {
        return Err(input_error(format!(
            "all physical cutting confirmations are required; {USAGE}"
        )));
    }
    Ok(Args { port, program_path })
}

fn validate_program_envelope(
    program: &GcodeProgram,
    profile: &HardwareProfile,
) -> Result<(), io::Error> {
    let bounds = program
        .summary
        .bounds
        .ok_or_else(|| input_error("program has no motion bounds"))?;
    let travel = profile
        .travel_mm
        .ok_or_else(|| input_error("hardware profile has no travel envelope"))?;
    let inside_xy = bounds.min.x >= 0.0
        && bounds.min.y >= 0.0
        && bounds.max.x <= travel.x
        && bounds.max.y <= travel.y;
    if !inside_xy {
        return Err(input_error(format!(
            "program XY bounds {:.3}..{:.3} x {:.3}..{:.3} mm exceed machine travel {:.3} x {:.3} mm",
            bounds.min.x, bounds.max.x, bounds.min.y, bounds.max.y, travel.x, travel.y
        )));
    }
    if program.features.has_probe_cycle || program.features.has_machine_coordinate_move {
        return Err(input_error(
            "hardware cutting utility rejects probing and machine-coordinate motion",
        ));
    }
    Ok(())
}

fn observed_work_position(
    snapshot: &ControllerSnapshot,
    inspection: &DeviceInspection,
) -> Option<Position> {
    snapshot.machine.work_position.or_else(|| {
        let machine = snapshot.machine.machine_position?;
        let offset = snapshot
            .machine
            .work_coordinate_offset
            .or_else(|| inspection_work_coordinate_offset(inspection))?;
        Some(Position {
            x: machine.x - offset.x,
            y: machine.y - offset.y,
            z: machine.z - offset.z,
            a: match (machine.a, offset.a) {
                (Some(machine), Some(offset)) => Some(machine - offset),
                _ => None,
            },
        })
    })
}

fn inspection_work_coordinate_offset(inspection: &DeviceInspection) -> Option<Position> {
    let active = active_wcs(inspection)?;
    let work = parse_parameter_position(inspection.parameters.get(active)?)?;
    let g92 = inspection
        .parameters
        .get("G92")
        .and_then(|value| parse_parameter_position(value))
        .unwrap_or_default();
    let tool_length = inspection
        .parameters
        .get("TLO")
        .and_then(|value| value.split(',').next())
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    Some(Position {
        x: work.x + g92.x,
        y: work.y + g92.y,
        z: work.z + g92.z + tool_length,
        a: match (work.a, g92.a) {
            (Some(work), Some(g92)) => Some(work + g92),
            (Some(work), None) => Some(work),
            _ => None,
        },
    })
}

fn active_wcs(inspection: &DeviceInspection) -> Option<&str> {
    inspection.modal_state.iter().find_map(|value| {
        matches!(
            value.as_str(),
            "G54" | "G55" | "G56" | "G57" | "G58" | "G59"
        )
        .then_some(value.as_str())
    })
}

fn parse_parameter_position(value: &str) -> Option<Position> {
    let values = value
        .split(',')
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if !(3..=4).contains(&values.len()) {
        return None;
    }
    Some(Position {
        x: values[0],
        y: values[1],
        z: values[2],
        a: values.get(3).copied(),
    })
}

fn print_preflight(report: &millo_run::RunPreflightReport) {
    println!(
        "PREFLIGHT: ready={}, blockers={}, cautions={}",
        report.ready, report.blocker_count, report.caution_count
    );
    for check in &report.checks {
        if check.level != RunPreflightLevel::Pass {
            println!("  {:?} {}: {}", check.level, check.title, check.detail);
        }
    }
}

fn ensure_idle(snapshot: &ControllerSnapshot) -> Result<(), io::Error> {
    if snapshot.machine.mode == MachineMode::Idle {
        Ok(())
    } else {
        Err(input_error(format!(
            "controller must be Idle, got {:?}",
            snapshot.machine.mode
        )))
    }
}

fn hardware_profile() -> HardwareProfile {
    HardwareProfile {
        name: "LUNYEE CNC".to_owned(),
        axes: vec!["X".to_owned(), "Y".to_owned(), "Z".to_owned()],
        travel_mm: Some(MachineTravel {
            x: 500.0,
            y: 500.0,
            z: 200.0,
        }),
        max_jog_distance_mm: 50.0,
        spindle_control: SpindleControl::Manual,
        homing_installed: false,
        limit_switches_installed: false,
        probe_installed: false,
        probe_mode: ProbeWorkflowMode::Off,
        emergency_stop_installed: false,
    }
}

fn input_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_every_physical_cut_confirmation() {
        let mut confirmed = vec!["/dev/cu.fixture".to_owned(), "job.nc".to_owned()];
        confirmed.extend(REQUIRED_FLAGS.map(str::to_owned));
        let args = parse_args(confirmed).unwrap();
        assert_eq!(args.port, "/dev/cu.fixture");
        assert_eq!(args.program_path, PathBuf::from("job.nc"));

        let mut incomplete = vec!["/dev/cu.fixture".to_owned(), "job.nc".to_owned()];
        incomplete.extend(REQUIRED_FLAGS[..7].iter().map(|flag| (*flag).to_owned()));
        assert!(parse_args(incomplete).is_err());
    }

    #[test]
    fn rejects_a_program_outside_the_machine_xy_envelope() {
        let program = parse_program(ProgramParseRequest {
            source_name: "outside.nc".to_owned(),
            source: "G21 G90 G94\nG0 X501 Y1 Z3\nG1 Z-0.2 F80".to_owned(),
        })
        .unwrap();

        assert!(validate_program_envelope(&program, &hardware_profile()).is_err());
    }
}
