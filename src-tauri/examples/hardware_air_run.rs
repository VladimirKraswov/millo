use std::{error::Error, fs, io, path::PathBuf, time::Duration};

use millo_command::{CommandArbiter, ExecutionTarget};
use millo_controller::ControllerConfig;
use millo_domain::{
    DeviceInspection, HardwareProfile, MachineMode, MachineTravel, Position, ProbeWorkflowMode,
    SpindleControl, WorkAxis, WorkZeroRequest,
};
use millo_gcode::{GcodeProgram, ProgramParseRequest, parse_program};
use millo_run::{FirstCutConfirmation, ProgramRunIntent, RunPreflightLevel};
use millo_sender::SenderState;
use millo_serial::{SerialConfig, SerialTransport};

const INSPECT_ONLY_FLAG: &str = "--inspect-only";
const REQUIRED_RUN_FLAGS: [&str; 7] = [
    "--confirm-unlock",
    "--confirm-tool-removed",
    "--confirm-spindle-off",
    "--confirm-set-current-xyz-zero",
    "--confirm-safe-z",
    "--confirm-path-clear",
    "--confirm-power-control",
];
const USAGE: &str = "usage: hardware_air_run <serial-port> <program.nc> --inspect-only\n       hardware_air_run <serial-port> <program.nc> --confirm-unlock --confirm-tool-removed --confirm-spindle-off --confirm-set-current-xyz-zero --confirm-safe-z --confirm-path-clear --confirm-power-control";
const POSITION_TOLERANCE_MM: f64 = 0.02;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMode {
    InspectOnly,
    ConfirmedAirRun,
}

struct Args {
    port: String,
    program_path: PathBuf,
    mode: RunMode,
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
    ensure_square_fixture(&program)?;

    let transport = SerialTransport::new(SerialConfig::new(args.port.clone(), 115_200)?);
    let (arbiter, worker) = CommandArbiter::new_with_execution_target(
        Box::new(transport),
        ControllerConfig::default(),
        hardware_profile(),
        ExecutionTarget::Serial,
    );
    let worker = tokio::spawn(worker);

    let result = run(&arbiter, &args, program).await;
    let _ = arbiter.disconnect().await;
    worker.abort();
    result
}

async fn run(
    arbiter: &CommandArbiter,
    args: &Args,
    program: GcodeProgram,
) -> Result<(), Box<dyn Error>> {
    println!("Connecting read-only to {} at 115200 baud", args.port);
    arbiter.connect().await?;
    let mut controller = arbiter.refresh_status().await?;
    if controller.reset_notice.is_some() {
        controller = arbiter.acknowledge_reset().await?;
    }
    if controller.machine.mode == MachineMode::Alarm && args.mode == RunMode::ConfirmedAirRun {
        println!("Unlocking the confirmed alarm state with typed $X");
        controller = arbiter.unlock_alarm(true).await?;
        println!("Unlock verified: {:?}", controller.machine.mode);
    }
    if controller.machine.mode != MachineMode::Idle {
        return Err(input_error(format!(
            "controller must be Idle, got {:?}",
            controller.machine.mode
        ))
        .into());
    }

    let mut report = arbiter
        .preflight_real_run(program.clone(), ProgramRunIntent::AirRun)
        .await?;
    print_preflight(&report);
    if let Some(capabilities) = &report.hardware.device.controller_capabilities {
        println!(
            "GRBL capabilities: flags={}, planner={:?}, RX={:?} byte(s)",
            capabilities.option_flags,
            capabilities.planner_buffer_blocks,
            capabilities.rx_buffer_bytes
        );
    }
    if !report.ready {
        return Err(input_error("Air-run preflight is blocked").into());
    }
    let work_before = observed_work_position(&arbiter.snapshot(), &report.hardware.device)
        .ok_or_else(|| input_error("controller status has no work position"))?;
    println!(
        "Work position: X {:+.3}, Y {:+.3}, Z {:+.3} mm",
        work_before.x, work_before.y, work_before.z
    );

    if args.mode == RunMode::InspectOnly {
        println!("PASS: read-only preflight is clear; no program line was sent");
        return Ok(());
    }
    println!("Setting the current position as G54-G59 XYZ work zero");
    for axis in [WorkAxis::X, WorkAxis::Y, WorkAxis::Z] {
        let outcome = arbiter
            .set_work_zero(WorkZeroRequest {
                axis,
                position_confirmed: true,
            })
            .await?;
        println!(
            "  {:?}: {} verified at {:+.3} mm",
            axis, outcome.command, outcome.work_position
        );
    }
    report = arbiter
        .preflight_real_run(program.clone(), ProgramRunIntent::AirRun)
        .await?;
    if !report.ready {
        return Err(input_error("post-zero Air-run preflight is blocked").into());
    }
    let work_before = observed_work_position(&arbiter.snapshot(), &report.hardware.device)
        .ok_or_else(|| input_error("post-zero controller status has no work position"))?;
    ensure_work_zero(work_before)?;

    let preparation = arbiter
        .authorize_first_cut(
            program.clone(),
            FirstCutConfirmation {
                intent: ProgramRunIntent::AirRun,
                execution_options: millo_dry_run::ProgramExecutionOptions::default(),
                stock_secured: false,
                tool_secured: false,
                tool_removed: true,
                xyz_zero_verified: true,
                safe_z_verified: true,
                manual_spindle_running: false,
                manual_spindle_off: true,
                path_clear: true,
                power_control_reachable: true,
            },
        )
        .await?;
    let mut sender = arbiter.subscribe_sender();
    let started = arbiter
        .start_program_run(program, preparation.authorization.id)
        .await?;
    println!(
        "START: {:?}, {} sender line(s), RX window {} byte(s), authorization #{}",
        started.mode, started.total_lines, started.rx_buffer_capacity, preparation.authorization.id
    );

    let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
    loop {
        let snapshot = sender.borrow().clone();
        println!(
            "{:?}: {}/{}{}",
            snapshot.state,
            snapshot.acknowledged_lines,
            snapshot.total_lines,
            snapshot
                .current_source_line
                .map(|line| format!(" at L{line}"))
                .unwrap_or_default()
        );
        match snapshot.state {
            SenderState::Completed => break,
            SenderState::Failed | SenderState::Cancelled => {
                emergency_stop(arbiter).await;
                return Err(input_error(format!(
                    "Air run ended as {:?}: {}; Hold and Soft Reset requested",
                    snapshot.state,
                    snapshot.last_error.as_deref().unwrap_or("no error detail")
                ))
                .into());
            }
            _ => {}
        }

        tokio::select! {
            changed = sender.changed() => {
                if changed.is_err() {
                    emergency_stop(arbiter).await;
                    return Err(input_error("sender event stream closed; Hold and Soft Reset requested").into());
                }
            },
            _ = tokio::signal::ctrl_c() => {
                emergency_stop(arbiter).await;
                return Err(input_error("operator interrupted Air run; Hold and Soft Reset requested").into());
            }
            _ = tokio::time::sleep_until(deadline) => {
                emergency_stop(arbiter).await;
                return Err(input_error("Air run exceeded 120 seconds; Hold and Soft Reset requested").into());
            }
        }
    }

    let final_snapshot = arbiter.refresh_status().await?;
    let work_after = observed_work_position(&final_snapshot, &report.hardware.device)
        .ok_or_else(|| input_error("final status has no work position"))?;
    ensure_work_zero(work_after)?;
    println!(
        "PASS: Air run completed in Idle; final WPos X {:+.3}, Y {:+.3}, Z {:+.3} mm",
        work_after.x, work_after.y, work_after.z
    );
    Ok(())
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
    let mode = if flags.len() == 1 && flags[0] == INSPECT_ONLY_FLAG {
        RunMode::InspectOnly
    } else if flags.len() == REQUIRED_RUN_FLAGS.len()
        && REQUIRED_RUN_FLAGS
            .iter()
            .all(|required| flags.iter().any(|flag| flag == required))
    {
        RunMode::ConfirmedAirRun
    } else {
        return Err(input_error(format!(
            "inspect-only or all seven run confirmations are required; {USAGE}"
        )));
    };
    Ok(Args {
        port,
        program_path,
        mode,
    })
}

fn ensure_square_fixture(program: &GcodeProgram) -> Result<(), io::Error> {
    let bounds = program
        .summary
        .bounds
        .ok_or_else(|| input_error("program has no motion bounds"))?;
    let exact_square = close(bounds.min.x, 0.0)
        && close(bounds.min.y, 0.0)
        && close(bounds.max.x, 20.0)
        && close(bounds.max.y, 20.0)
        && close(bounds.size.z, 0.0)
        && program.summary.motion_count == 4;
    if !exact_square {
        return Err(input_error(
            "hardware fixture must contain exactly four motions in a 20 x 20 x 0 mm envelope",
        ));
    }
    Ok(())
}

fn ensure_work_zero(position: Position) -> Result<(), io::Error> {
    if close_to_zero(position.x) && close_to_zero(position.y) && close_to_zero(position.z) {
        Ok(())
    } else {
        Err(input_error(format!(
            "WPos must be within +/-{POSITION_TOLERANCE_MM:.2} mm of XYZ zero; got X {:+.3}, Y {:+.3}, Z {:+.3}",
            position.x, position.y, position.z
        )))
    }
}

fn observed_work_position(
    snapshot: &millo_domain::ControllerSnapshot,
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
    let active = inspection.modal_state.iter().find(|value| {
        matches!(
            value.as_str(),
            "G54" | "G55" | "G56" | "G57" | "G58" | "G59"
        )
    })?;
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
        "Preflight: ready={}, blockers={}, cautions={}, bounds={:.3} x {:.3} x {:.3} mm",
        report.ready,
        report.blocker_count,
        report.caution_count,
        report.bounds.map_or(0.0, |bounds| bounds.size.x),
        report.bounds.map_or(0.0, |bounds| bounds.size.y),
        report.bounds.map_or(0.0, |bounds| bounds.size.z),
    );
    for check in &report.checks {
        if check.level != RunPreflightLevel::Pass {
            println!("  {:?} {}: {}", check.level, check.title, check.detail);
        }
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

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= f64::EPSILON
}

fn close_to_zero(value: f64) -> bool {
    value.is_finite() && value.abs() <= POSITION_TOLERANCE_MM
}

fn input_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_inspect_only_or_every_physical_confirmation() {
        let inspect = parse_args([
            "/dev/cu.fixture".to_owned(),
            "square.nc".to_owned(),
            INSPECT_ONLY_FLAG.to_owned(),
        ])
        .unwrap();
        assert_eq!(inspect.mode, RunMode::InspectOnly);

        let mut confirmed = vec!["/dev/cu.fixture".to_owned(), "square.nc".to_owned()];
        confirmed.extend(REQUIRED_RUN_FLAGS.map(str::to_owned));
        assert_eq!(
            parse_args(confirmed).unwrap().mode,
            RunMode::ConfirmedAirRun
        );
        assert!(
            parse_args([
                "/dev/cu.fixture".to_owned(),
                "square.nc".to_owned(),
                REQUIRED_RUN_FLAGS[0].to_owned(),
            ])
            .is_err()
        );
    }

    #[test]
    fn enforces_the_exact_fixture_envelope_and_work_zero() {
        let program = parse_program(ProgramParseRequest {
            source_name: "air-square-20mm.nc".to_owned(),
            source: include_str!("../../fixtures/programs/air-square-20mm.nc").to_owned(),
        })
        .unwrap();
        ensure_square_fixture(&program).unwrap();
        ensure_work_zero(Position {
            x: 0.01,
            y: -0.01,
            z: 0.0,
            a: None,
        })
        .unwrap();
        assert!(
            ensure_work_zero(Position {
                x: 0.03,
                y: 0.0,
                z: 0.0,
                a: None,
            })
            .is_err()
        );

        let mut snapshot = millo_domain::ControllerSnapshot::default();
        snapshot.machine.machine_position = Some(Position {
            x: -10.0,
            y: 5.0,
            z: 1.0,
            a: None,
        });
        snapshot.machine.work_coordinate_offset = Some(Position {
            x: -10.0,
            y: 5.0,
            z: 1.0,
            a: None,
        });
        assert_eq!(
            observed_work_position(&snapshot, &DeviceInspection::default()),
            Some(Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                a: None,
            })
        );

        snapshot.machine.work_coordinate_offset = None;
        let mut inspection = DeviceInspection::default();
        inspection.modal_state = vec!["G0".to_owned(), "G54".to_owned()];
        inspection
            .parameters
            .insert("G54".to_owned(), "-10.000,5.000,1.000".to_owned());
        inspection
            .parameters
            .insert("G92".to_owned(), "0.000,0.000,0.000".to_owned());
        inspection
            .parameters
            .insert("TLO".to_owned(), "0.000".to_owned());
        assert_eq!(
            observed_work_position(&snapshot, &inspection),
            Some(Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                a: None,
            })
        );
    }
}
