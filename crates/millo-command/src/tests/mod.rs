mod controller;
mod coordinates;
mod execution;
mod heightmap;
mod homing;
mod jog;
mod probe;

use std::time::Duration;

use millo_dry_run::{build_dry_run_plan, build_program_run_plan};
use millo_gcode::{
    ProgramParseOptions, ProgramParseRequest, parse_program, parse_program_with_options,
};
use millo_mock::MockTransport;

use super::*;

fn test_arbiter(
    poll_interval: Duration,
) -> (
    CommandArbiter,
    millo_mock::MockControl,
    impl Future<Output = ()> + Send + 'static,
) {
    let transport = MockTransport::default();
    let control = transport.control();
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig {
            poll_interval,
            status_timeout: Duration::from_millis(20),
            command_timeout: Duration::from_millis(50),
            failures_before_recovery: 2,
        },
        HardwareProfile::first_machine(),
    );
    (arbiter, control, worker)
}

fn operator_confirmation() -> OperatorConfirmation {
    OperatorConfirmation {
        spindle_off: true,
        tool_clear: true,
        power_control_reachable: true,
    }
}

fn first_cut_confirmation() -> FirstCutConfirmation {
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
    }
}

fn air_run_confirmation() -> FirstCutConfirmation {
    FirstCutConfirmation {
        intent: ProgramRunIntent::AirRun,
        execution_options: ProgramExecutionOptions::default(),
        stock_secured: false,
        tool_secured: false,
        tool_removed: true,
        xyz_zero_verified: true,
        safe_z_verified: true,
        manual_spindle_running: false,
        manual_spindle_off: true,
        probe_removed: true,
        path_clear: true,
        power_control_reachable: true,
    }
}

fn tool_change_confirmation(
    source_line: usize,
    requested_tool: Option<u8>,
) -> ToolChangeConfirmation {
    ToolChangeConfirmation {
        source_line,
        requested_tool,
        tool_secured: true,
        z_zero_verified: true,
        safe_z_verified: true,
        path_clear: true,
        manual_spindle_running: true,
        power_control_reachable: true,
    }
}

fn work_zero_request(axis: WorkAxis, position_confirmed: bool) -> WorkZeroRequest {
    WorkZeroRequest {
        axis,
        position_confirmed,
    }
}

fn return_to_zero_request(axis: WorkAxis) -> ReturnToWorkZeroRequest {
    ReturnToWorkZeroRequest {
        axis,
        feed_mm_per_min: if matches!(axis, WorkAxis::Z) {
            100.0
        } else {
            300.0
        },
    }
}

fn z_probe_request() -> ZProbeRequest {
    ZProbeRequest {
        settings: ZProbeSettings {
            mode: ProbeWorkflowMode::WorkZero,
            plate_thickness_mm: 19.1,
            max_travel_mm: 10.0,
            probe_feed_mm_per_min: 25.0,
            retract_mm: 1.0,
            retract_feed_mm_per_min: 1_000.0,
        },
        setup_confirmed: true,
    }
}

fn heightmap_request() -> HeightmapStartRequest {
    HeightmapStartRequest {
        plan: millo_heightmap::HeightmapPlanRequest {
            origin_x_mm: 10.0,
            origin_y_mm: 20.0,
            width_mm: 2.0,
            height_mm: 2.0,
            columns: 2,
            rows: 2,
            clearance_z_mm: 2.0,
            max_probe_depth_mm: 2.0,
            probe_feed_mm_per_min: 100.0,
            travel_feed_mm_per_min: 1_000.0,
            retract_feed_mm_per_min: 1_000.0,
            ..millo_heightmap::HeightmapPlanRequest::default()
        },
        setup_confirmed: true,
        contact_available_at_every_point: true,
    }
}

fn dry_run_plan(source: &str) -> DryRunPlan {
    let program = parsed_program(source);
    build_dry_run_plan(&program).unwrap()
}

async fn authorize_and_start_serial_fixture(
    arbiter: &CommandArbiter,
    source: &str,
    dispatch_immediately: bool,
) -> (FirstCutPreparation, SenderSnapshot) {
    let preparation = arbiter
        .authorize_first_cut_fixture(parsed_program(source), first_cut_confirmation())
        .await
        .unwrap();
    let started = arbiter
        .start_serial_run_fixture(
            parsed_program(source),
            preparation.authorization.id,
            dispatch_immediately,
        )
        .await
        .unwrap();
    (preparation, started)
}

fn parsed_program(source: &str) -> GcodeProgram {
    parse_program(ProgramParseRequest {
        source_name: "sender-fixture.nc".to_owned(),
        source: source.to_owned(),
    })
    .unwrap()
}

fn safe_start_program(source: &str) -> GcodeProgram {
    parse_program(ProgramParseRequest {
        source_name: "safe-start-L42-original.nc".to_owned(),
        source: source.to_owned(),
    })
    .unwrap()
}

fn parsed_program_with_options(
    source: &str,
    execution_options: ProgramExecutionOptions,
) -> GcodeProgram {
    parse_program_with_options(
        ProgramParseRequest {
            source_name: "sender-fixture.nc".to_owned(),
            source: source.to_owned(),
        },
        ProgramParseOptions {
            block_delete: execution_options.block_delete,
        },
    )
    .unwrap()
}

fn mock_dry_run_arbiter() -> (
    CommandArbiter,
    millo_mock::MockControl,
    impl Future<Output = ()> + Send + 'static,
) {
    let transport = MockTransport::default();
    let control = transport.control();
    let (arbiter, worker) = CommandArbiter::new_with_execution_target(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            status_timeout: Duration::from_millis(20),
            command_timeout: Duration::from_millis(50),
            failures_before_recovery: 2,
        },
        HardwareProfile::first_machine(),
        ExecutionTarget::Mock,
    );
    (arbiter, control, worker)
}

fn disabled_execution_arbiter() -> (
    CommandArbiter,
    millo_mock::MockControl,
    impl Future<Output = ()> + Send + 'static,
) {
    let transport = MockTransport::default();
    let control = transport.control();
    control.set_virtual_motion_enabled(false);
    let (arbiter, worker) = CommandArbiter::new_with_execution_target(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            status_timeout: Duration::from_millis(20),
            command_timeout: Duration::from_millis(50),
            failures_before_recovery: 2,
        },
        HardwareProfile::first_machine(),
        ExecutionTarget::Disabled,
    );
    (arbiter, control, worker)
}

fn serial_preflight_arbiter() -> (
    CommandArbiter,
    millo_mock::MockControl,
    impl Future<Output = ()> + Send + 'static,
) {
    serial_preflight_arbiter_with_poll(Duration::from_secs(60))
}

fn serial_preflight_arbiter_with_poll(
    poll_interval: Duration,
) -> (
    CommandArbiter,
    millo_mock::MockControl,
    impl Future<Output = ()> + Send + 'static,
) {
    let transport = MockTransport::default();
    let control = transport.control();
    control.set_virtual_motion_enabled(false);
    let (arbiter, worker) = CommandArbiter::new_with_execution_target(
        Box::new(transport),
        ControllerConfig {
            poll_interval,
            status_timeout: Duration::from_millis(20),
            command_timeout: Duration::from_millis(50),
            failures_before_recovery: 2,
        },
        HardwareProfile::first_machine(),
        ExecutionTarget::Serial,
    );
    (arbiter, control, worker)
}

fn serial_preflight_arbiter_for_realtime_preemption() -> (
    CommandArbiter,
    millo_mock::MockControl,
    impl Future<Output = ()> + Send + 'static,
) {
    let transport = MockTransport::default();
    let control = transport.control();
    control.set_virtual_motion_enabled(false);
    let (arbiter, worker) = CommandArbiter::new_with_execution_target(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            status_timeout: Duration::from_millis(20),
            // The delayed response is intentional. Keep the command alive
            // even when the parallel workspace test runner is under load.
            command_timeout: Duration::from_secs(1),
            failures_before_recovery: 2,
        },
        HardwareProfile::first_machine(),
        ExecutionTarget::Serial,
    );
    (arbiter, control, worker)
}

async fn wait_for_sender(arbiter: &CommandArbiter, expected: SenderState) -> SenderSnapshot {
    let mut snapshots = arbiter.subscribe_sender();
    tokio::time::timeout(Duration::from_millis(200), async {
        loop {
            let snapshot = snapshots.borrow_and_update().clone();
            if snapshot.state == expected {
                return snapshot;
            }
            snapshots.changed().await.unwrap();
        }
    })
    .await
    .unwrap()
}

async fn wait_for_controller_idle(arbiter: &CommandArbiter) -> ControllerSnapshot {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = arbiter.refresh_status().await.unwrap();
            if snapshot.machine.mode == MachineMode::Idle {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("virtual motion did not settle to Idle")
}

async fn wait_for_homing_state(
    arbiter: &CommandArbiter,
    expected: HomingState,
) -> ControllerSnapshot {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let snapshot = arbiter.snapshot();
        if snapshot.homing.state == expected {
            return snapshot;
        }
        assert!(
            Instant::now() < deadline,
            "homing state did not reach {expected:?}"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn wait_for_heightmap(
    arbiter: &CommandArbiter,
    expected: HeightmapOperationState,
) -> HeightmapOperationSnapshot {
    let mut snapshots = arbiter.subscribe_heightmap();
    // Virtual motion uses wall-clock travel time; timer tick counts differ by OS.
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let snapshot = snapshots.borrow_and_update().clone();
            if snapshot.state == expected {
                return snapshot;
            }
            assert!(
                !matches!(
                    snapshot.state,
                    HeightmapOperationState::Completed
                        | HeightmapOperationState::Failed
                        | HeightmapOperationState::Cancelled
                ),
                "heightmap terminated before {expected:?}: {snapshot:?}"
            );
            snapshots.changed().await.expect("heightmap stream closed");
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "heightmap did not reach {expected:?}: {:?}",
            snapshots.borrow()
        )
    })
}
