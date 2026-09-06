use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use millo_command::{ArbiterError, CommandArbiter, ExecutionTarget};
use millo_controller::ControllerConfig;
use millo_domain::{
    ConnectionState, HardwareProfile, MachineMode, RotaryAxisProfile, WorkAxis, WorkZeroRequest,
};
use millo_gcode::{GcodeProgram, ProgramParseRequest, parse_program};
use millo_mock::{MockControl, MockTransport};
use millo_run::{
    FirstCutConfirmation, ProgramRunIntent, RunPreflightLevel, ToolChangeConfirmation,
};
use millo_sender::{SenderMode, SenderSnapshot, SenderState};

const PROGRAM: &str = "G21 G90 G93\nG1 X20 Y10 Z5 A90 F6\nM30";

fn program(source: &str) -> GcodeProgram {
    parse_program(ProgramParseRequest {
        source_name: "rotary-command.nc".to_owned(),
        source: source.to_owned(),
    })
    .unwrap()
}

fn profile() -> HardwareProfile {
    let mut profile = HardwareProfile::first_machine();
    profile.axes.push("A".to_owned());
    profile.rotary_axis = Some(RotaryAxisProfile {
        travel_degrees: 720.0,
        max_jog_degrees: 30.0,
        max_feed_degrees_per_min: 720.0,
    });
    profile
}

fn air_confirmation() -> FirstCutConfirmation {
    FirstCutConfirmation {
        intent: ProgramRunIntent::AirRun,
        tool_removed: true,
        xyz_zero_verified: true,
        safe_z_verified: true,
        manual_spindle_off: true,
        probe_removed: true,
        path_clear: true,
        power_control_reachable: true,
        ..FirstCutConfirmation::default()
    }
}

struct Rig {
    arbiter: CommandArbiter,
    control: MockControl,
    worker: tokio::task::JoinHandle<()>,
}

impl Rig {
    async fn new(transport: MockTransport, profile: HardwareProfile) -> Self {
        Self::with_target(transport, profile, ExecutionTarget::Serial).await
    }

    async fn with_target(
        transport: MockTransport,
        profile: HardwareProfile,
        target: ExecutionTarget,
    ) -> Self {
        let control = transport.control();
        // Exercise the production serial execution policy on an in-memory transport only.
        let (arbiter, worker) = CommandArbiter::new_with_execution_target(
            Box::new(transport),
            ControllerConfig {
                poll_interval: Duration::from_secs(60),
                status_timeout: Duration::from_millis(200),
                command_timeout: Duration::from_millis(500),
                failures_before_recovery: 2,
            },
            profile,
            target,
        );
        let rig = Self {
            arbiter,
            control,
            worker: tokio::spawn(worker),
        };
        rig.arbiter.connect().await.unwrap();
        rig
    }

    async fn start_air_run(&self) {
        self.check(program(PROGRAM)).await;
        let preparation = self
            .arbiter
            .authorize_first_cut(program(PROGRAM), air_confirmation())
            .await
            .unwrap();
        assert!(preparation.report.ready, "{:?}", preparation.report.checks);
        let started = self
            .arbiter
            .start_program_run(program(PROGRAM), preparation.authorization.id)
            .await
            .unwrap();
        assert_eq!(started.mode, Some(SenderMode::AirRun));
        self.wait(SenderState::Draining).await;
    }

    async fn wait(&self, expected: SenderState) -> SenderSnapshot {
        self.wait_for(expected, Duration::from_secs(3)).await
    }

    async fn check(&self, program: impl Into<Arc<GcodeProgram>>) {
        self.arbiter.start_check_run(program).await.unwrap();
        self.wait(SenderState::Completed).await;
    }

    async fn wait_for(&self, expected: SenderState, timeout: Duration) -> SenderSnapshot {
        let mut updates = self.arbiter.subscribe_sender();
        tokio::time::timeout(timeout, async {
            loop {
                let snapshot = updates.borrow_and_update().clone();
                if snapshot.state == expected {
                    return snapshot;
                }
                assert!(
                    !matches!(snapshot.state, SenderState::Failed | SenderState::Cancelled),
                    "{snapshot:?}"
                );
                updates.changed().await.unwrap();
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "expected {expected:?}: {:?}",
                self.arbiter.sender_snapshot()
            )
        })
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.worker.abort();
    }
}

fn assert_no_execution(writes: &[Vec<u8>]) {
    assert!(!writes.iter().any(|line| line == b"$C\n"), "{writes:?}");
    assert!(
        !writes
            .iter()
            .any(|line| line.ends_with(b"\n") && !line.starts_with(b"$")),
        "{writes:?}"
    );
}

#[tokio::test]
async fn stock_xyz_rejects_rotary_preflight_check_and_air_run_before_any_motion_or_check_mode() {
    let rig = Rig::new(MockTransport::default(), profile()).await;
    let report = rig
        .arbiter
        .preflight_real_run(program(PROGRAM), ProgramRunIntent::AirRun)
        .await
        .unwrap();
    assert!(!report.ready);
    assert!(report.checks.iter().any(
        |check| check.id == "rotary-a-capability" && check.level == RunPreflightLevel::Blocker
    ));
    assert!(matches!(
        rig.arbiter.start_check_run(program(PROGRAM)).await,
        Err(ArbiterError::RotaryProgramUnavailable(_))
    ));
    assert!(
        rig.arbiter
            .authorize_first_cut(program(PROGRAM), air_confirmation())
            .await
            .is_err()
    );
    assert!(matches!(
        rig.arbiter.start_program_run(program(PROGRAM), 1).await,
        Err(ArbiterError::RotaryProgramUnavailable(_))
    ));
    assert_no_execution(&rig.control.writes());
}

#[tokio::test]
async fn virtual_xyza_preflight_and_check_preserve_four_axis_position_then_certify_cutting() {
    let rig = Rig::new(MockTransport::rotary(), profile()).await;
    let before = rig
        .arbiter
        .refresh_status()
        .await
        .unwrap()
        .machine
        .machine_position;
    let air = rig
        .arbiter
        .preflight_real_run(program(PROGRAM), ProgramRunIntent::AirRun)
        .await
        .unwrap();
    assert!(!air.ready, "rotary air run must also require Check");
    assert!(
        air.checks
            .iter()
            .any(|check| check.id == "grbl-check-certificate"
                && check.level == RunPreflightLevel::Blocker)
    );
    assert!(
        rig.arbiter
            .authorize_first_cut(program(PROGRAM), air_confirmation())
            .await
            .is_err()
    );
    assert_no_execution(&rig.control.writes());
    assert!(
        air.checks.iter().any(
            |check| check.id == "rotary-a-capability" && check.level == RunPreflightLevel::Pass
        )
    );
    let cutting = rig
        .arbiter
        .preflight_real_run(program(PROGRAM), ProgramRunIntent::Cutting)
        .await
        .unwrap();
    assert!(
        cutting
            .checks
            .iter()
            .any(|check| check.id == "grbl-check-certificate"
                && check.level == RunPreflightLevel::Blocker)
    );
    let started = rig.arbiter.start_check_run(program(PROGRAM)).await.unwrap();
    assert_eq!(started.mode, Some(SenderMode::CheckRun));
    let completed = rig.wait(SenderState::Completed).await;
    assert_eq!(completed.acknowledged_lines, completed.total_lines);
    rig.control.advance_program(Duration::from_secs(60));
    let after = rig.arbiter.refresh_status().await.unwrap();
    assert_eq!(after.machine.mode, MachineMode::Idle);
    assert_eq!(after.machine.machine_position, before);
    let air = rig
        .arbiter
        .preflight_real_run(program(PROGRAM), ProgramRunIntent::AirRun)
        .await
        .unwrap();
    assert!(air.ready, "{:?}", air.checks);
    let report = rig
        .arbiter
        .preflight_real_run(program(PROGRAM), ProgramRunIntent::Cutting)
        .await
        .unwrap();
    assert!(report.ready, "{:?}", report.checks);
    assert!(report.checks.iter().any(
        |check| check.id == "grbl-check-certificate" && check.level == RunPreflightLevel::Pass
    ));
    assert_eq!(
        rig.control
            .writes()
            .iter()
            .filter(|line| line.as_slice() == b"$C\n")
            .count(),
        2
    );
    assert!(
        rig.control
            .writes()
            .iter()
            .any(|line| String::from_utf8_lossy(line).contains("A90"))
    );
}

#[tokio::test]
async fn virtual_air_run_executes_coordinated_xyza_and_survives_hold_resume() {
    let rig = Rig::new(MockTransport::rotary(), profile()).await;
    rig.start_air_run().await;
    rig.control.advance_program(Duration::from_secs(2));
    let running = rig.arbiter.refresh_status().await.unwrap();
    let partial = running.machine.machine_position.unwrap();
    assert!(partial.a.unwrap() > 0.0 && partial.a.unwrap() < 90.0);
    assert!((partial.x / 20.0 - partial.a.unwrap() / 90.0).abs() < 0.001);
    assert!((partial.y / 10.0 - partial.a.unwrap() / 90.0).abs() < 0.001);
    assert!((partial.z / 5.0 - partial.a.unwrap() / 90.0).abs() < 0.001);

    rig.arbiter.feed_hold().await.unwrap();
    rig.wait(SenderState::Paused).await;
    rig.control.advance_program(Duration::from_secs(2));
    let held = rig
        .arbiter
        .refresh_status()
        .await
        .unwrap()
        .machine
        .machine_position;
    rig.control.advance_program(Duration::from_secs(60));
    assert_eq!(
        rig.arbiter
            .refresh_status()
            .await
            .unwrap()
            .machine
            .machine_position,
        held
    );
    rig.arbiter.resume_program_run().await.unwrap();
    rig.control.advance_program(Duration::from_secs(60));
    let finished = rig.arbiter.refresh_status().await.unwrap();
    rig.wait(SenderState::Completed).await;
    let end = finished.machine.machine_position.unwrap();
    assert_eq!((end.x, end.y, end.z, end.a), (20.0, 10.0, 5.0, Some(90.0)));
    let writes = rig.control.writes();
    assert!(writes.contains(&b"!".to_vec()));
    assert!(writes.contains(&b"~".to_vec()));
    assert_eq!(
        writes
            .iter()
            .filter(|line| line.as_slice() == b"$C\n")
            .count(),
        2
    );
    assert!(!writes.iter().any(|line| {
        String::from_utf8_lossy(line)
            .split_whitespace()
            .any(|word| matches!(word, "M3" | "M4") || word.starts_with('S'))
    }));
}

#[tokio::test]
async fn rotary_air_run_reset_cancels_queue_preserves_angle_and_never_resumes_implicitly() {
    let rig = Rig::new(MockTransport::rotary(), profile()).await;
    rig.start_air_run().await;
    rig.control.advance_program(Duration::from_secs(2));
    rig.arbiter.feed_hold().await.unwrap();
    rig.control.advance_program(Duration::from_secs(2));
    let held = rig
        .arbiter
        .refresh_status()
        .await
        .unwrap()
        .machine
        .machine_position;
    let challenge = rig.arbiter.request_soft_reset().await.unwrap();
    rig.arbiter.confirm_soft_reset(challenge.id).await.unwrap();
    assert_eq!(rig.arbiter.sender_snapshot().state, SenderState::Cancelled);
    rig.control.advance_program(Duration::from_secs(60));
    let after = rig.arbiter.refresh_status().await.unwrap();
    assert_eq!(after.machine.machine_position, held);
    assert!(after.reset_count > 0);
    assert!(rig.arbiter.resume_program_run().await.is_err());
    let writes = rig.control.writes();
    let reset = writes.iter().rposition(|line| line == b"\x18").unwrap();
    assert_no_execution(&writes[reset + 1..]);
    assert!(
        !writes
            .iter()
            .any(|line| String::from_utf8_lossy(line).contains("M30"))
    );
}

#[tokio::test]
async fn rotary_air_run_link_loss_stops_sender_and_reconnect_does_not_replay_a() {
    let rig = Rig::new(MockTransport::rotary(), profile()).await;
    rig.start_air_run().await;
    rig.control.advance_program(Duration::from_secs(1));
    rig.control.drop_link();
    assert!(rig.arbiter.refresh_status().await.is_err());
    assert_eq!(
        rig.arbiter.snapshot().connection,
        ConnectionState::Disconnected
    );
    assert_eq!(rig.arbiter.sender_snapshot().state, SenderState::Failed);
    let lost = rig.control.writes().len();
    assert!(rig.arbiter.resume_program_run().await.is_err());
    rig.arbiter.connect().await.unwrap();
    rig.arbiter.refresh_status().await.unwrap();
    assert_eq!(rig.arbiter.sender_snapshot().state, SenderState::Failed);
    assert_no_execution(&rig.control.writes()[lost..]);
}

#[tokio::test]
async fn authorization_rechecks_current_axis_evidence_before_sending_a() {
    let rig = Rig::new(MockTransport::rotary(), profile()).await;
    rig.check(program(PROGRAM)).await;
    let preparation = rig
        .arbiter
        .authorize_first_cut(program(PROGRAM), air_confirmation())
        .await
        .unwrap();
    let authorized = rig.control.writes().len();
    rig.control.set_virtual_motion_enabled(false);
    rig.control
        .set_status("<Idle|MPos:0,0,0|WPos:0,0,0|WCO:0,0,0|FS:0,0>");
    assert!(matches!(
        rig.arbiter
            .start_program_run(program(PROGRAM), preparation.authorization.id)
            .await,
        Err(ArbiterError::RotaryProgramUnavailable(_))
    ));
    assert_no_execution(&rig.control.writes()[authorized..]);
}

#[tokio::test]
async fn disabled_profile_and_unimplemented_rotary_arcs_fail_before_check_mode() {
    let rig = Rig::new(MockTransport::rotary(), HardwareProfile::first_machine()).await;
    assert!(matches!(
        rig.arbiter.start_check_run(program(PROGRAM)).await,
        Err(ArbiterError::RotaryProgramUnavailable(_))
    ));
    assert_no_execution(&rig.control.writes());
    let rig = Rig::new(MockTransport::rotary(), profile()).await;
    let arc = program("G21 G90 G93\nG2 X10 Y0 I5 J0 A90 F6");
    assert!(matches!(
        rig.arbiter.start_check_run(arc.clone()).await,
        Err(ArbiterError::RotaryProgramUnavailable(_))
    ));
    let report = rig
        .arbiter
        .preflight_real_run(arc, ProgramRunIntent::AirRun)
        .await
        .unwrap();
    assert!(report.checks.iter().any(
        |check| check.id == "rotary-a-capability" && check.level == RunPreflightLevel::Blocker
    ));
    assert_no_execution(&rig.control.writes());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "million-line release benchmark; run explicitly with --release --ignored --nocapture"]
async fn million_line_rotary_actor_benchmark() {
    const LINES: usize = 1_000_000;
    let mut source = String::with_capacity(LINES * 26);
    source.push_str("G21 G90 G93\n");
    for line in 0..LINES - 2 {
        source.push_str(if line % 2 == 0 {
            "G1 X1 Y1 Z1 A1 F600\n"
        } else {
            "G1 X0 Y0 Z0 A0 F600\n"
        });
    }
    source.push_str("M30\n");
    let clock = Instant::now();
    let parsed = Arc::new(
        parse_program(ProgramParseRequest {
            source_name: "million-line-rotary.nc".to_owned(),
            source,
        })
        .unwrap(),
    );
    let parse_time = clock.elapsed();
    assert_eq!(parsed.lines.len(), LINES);
    assert!(parsed.features.uses_rotary_a);
    eprintln!("million-line rotary: parse={parse_time:?}");
    let rig = Rig::with_target(MockTransport::rotary(), profile(), ExecutionTarget::Mock).await;

    let clock = Instant::now();
    rig.arbiter
        .start_check_run(Arc::clone(&parsed))
        .await
        .unwrap();
    rig.wait_for(SenderState::Completed, Duration::from_secs(300))
        .await;
    let check_time = clock.elapsed();
    let checked = rig.control.writes().len();
    eprintln!("million-line rotary: check={check_time:?}");

    let clock = Instant::now();
    let preflight = rig
        .arbiter
        .preflight_real_run(Arc::clone(&parsed), ProgramRunIntent::AirRun)
        .await
        .unwrap();
    let preflight_time = clock.elapsed();
    assert!(preflight.ready, "{:?}", preflight.checks);
    eprintln!("million-line rotary: preflight={preflight_time:?}");
    let clock = Instant::now();
    let authorized = rig
        .arbiter
        .authorize_first_cut(Arc::clone(&parsed), air_confirmation())
        .await
        .unwrap();
    let authorize_time = clock.elapsed();
    eprintln!("million-line rotary: authorize={authorize_time:?}");
    let clock = Instant::now();
    let prepared = rig
        .arbiter
        .prepare_program_run(Arc::clone(&parsed), authorized.authorization.id)
        .await
        .unwrap();
    let prepare_time = clock.elapsed();
    assert_eq!(prepared.state, SenderState::Running);
    assert_eq!(prepared.mode, Some(SenderMode::AirRun));
    assert_no_execution(&rig.control.writes()[checked..]);
    eprintln!("million-line rotary: prepare={prepare_time:?}");

    let clock = Instant::now();
    rig.arbiter
        .commit_prepared_program_run(prepared.run_sequence)
        .await
        .unwrap();
    let commit_time = clock.elapsed();
    tokio::time::timeout(Duration::from_secs(3), async {
        while rig.arbiter.sender_snapshot().acknowledged_lines < 8 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("sender must actually dispatch rotary blocks");
    let clock = Instant::now();
    tokio::time::timeout(Duration::from_secs(3), rig.arbiter.feed_hold())
        .await
        .unwrap()
        .unwrap();
    let hold_time = clock.elapsed();
    rig.wait(SenderState::Paused).await;
    let accepted = rig.arbiter.sender_snapshot();
    assert!(accepted.acknowledged_lines < accepted.total_lines);
    let clock = Instant::now();
    let challenge = rig.arbiter.request_soft_reset().await.unwrap();
    tokio::time::timeout(
        Duration::from_secs(3),
        rig.arbiter.confirm_soft_reset(challenge.id),
    )
    .await
    .unwrap()
    .unwrap();
    let reset_time = clock.elapsed();
    assert_eq!(rig.arbiter.sender_snapshot().state, SenderState::Cancelled);
    assert!(rig.arbiter.resume_program_run().await.is_err());
    let writes = rig.control.writes();
    assert!(
        writes[checked..]
            .iter()
            .any(|line| String::from_utf8_lossy(line).contains("A1"))
    );
    let reset = writes.iter().rposition(|line| line == b"\x18").unwrap();
    assert_no_execution(&writes[reset + 1..]);
    eprintln!(
        "million-line rotary: lines={LINES} parse={parse_time:?} check={check_time:?} preflight={preflight_time:?} authorize={authorize_time:?} prepare={prepare_time:?} commit={commit_time:?} hold={hold_time:?} reset={reset_time:?} accepted_before_hold={}/{}",
        accepted.acknowledged_lines, accepted.total_lines
    );
}

#[tokio::test]
async fn prepared_rotary_commit_rejects_changed_idle_a_reference() {
    let rig = Rig::new(MockTransport::rotary(), profile()).await;
    rig.check(program(PROGRAM)).await;
    let checked = rig.control.writes().len();
    let preparation = rig
        .arbiter
        .authorize_first_cut(program(PROGRAM), air_confirmation())
        .await
        .unwrap();
    let prepared = rig
        .arbiter
        .prepare_program_run(program(PROGRAM), preparation.authorization.id)
        .await
        .unwrap();
    assert_no_execution(&rig.control.writes()[checked..]);
    rig.control
        .set_status("<Idle|MPos:0,0,0,10|WPos:0,0,0,10|WCO:0,0,0,0|FS:0,0>");
    assert!(
        rig.arbiter
            .commit_prepared_program_run(prepared.run_sequence)
            .await
            .is_err(),
        "commit must compare the prepared reference, not only check fresh Idle"
    );
    assert_no_execution(&rig.control.writes()[checked..]);
}

#[tokio::test]
async fn rotary_m6_keeps_a_and_wcs_reference_but_allows_typed_z_zero() {
    let source = "G21 G90 G93\nG1 X2 Z5 A10 F6\nT2 M6\nG1 X4 A20 F6\nM30";
    let rig = Rig::new(MockTransport::rotary(), profile()).await;
    rig.check(program(source)).await;
    let preparation = rig
        .arbiter
        .authorize_first_cut(
            program(source),
            FirstCutConfirmation {
                intent: ProgramRunIntent::Cutting,
                stock_secured: true,
                tool_secured: true,
                xyz_zero_verified: true,
                safe_z_verified: true,
                manual_spindle_running: true,
                probe_removed: true,
                path_clear: true,
                power_control_reachable: true,
                ..FirstCutConfirmation::default()
            },
        )
        .await
        .unwrap();
    rig.arbiter
        .start_program_run(program(source), preparation.authorization.id)
        .await
        .unwrap();
    rig.wait(SenderState::Draining).await;
    rig.control.advance_program(Duration::from_secs(60));
    rig.arbiter.refresh_status().await.unwrap();
    let barrier = rig.wait(SenderState::ToolChange).await;
    assert_eq!(barrier.current_source_line, Some(3));
    assert_eq!(barrier.requested_tool, Some(2));
    assert!(rig.arbiter.resume_program_run().await.is_err());
    let confirmation = ToolChangeConfirmation {
        source_line: 3,
        requested_tool: Some(2),
        tool_secured: true,
        z_zero_verified: true,
        safe_z_verified: true,
        path_clear: true,
        manual_spindle_running: true,
        power_control_reachable: true,
    };
    let barrier_writes = rig.control.writes().len();
    rig.control
        .set_status("<Idle|MPos:2,0,5,11|WPos:2,0,5,11|WCO:0,0,0,0|FS:0,0>");
    assert!(matches!(
        rig.arbiter.complete_tool_change(confirmation).await,
        Err(ArbiterError::RotaryProgramUnavailable(_))
    ));
    assert_no_execution(&rig.control.writes()[barrier_writes..]);
    rig.control
        .set_status("<Idle|MPos:2,0,5,10|WPos:2,0,5,10|WCO:0,0,0,0|FS:0,0>");
    rig.arbiter
        .set_work_zero(WorkZeroRequest {
            axis: WorkAxis::Z,
            position_confirmed: true,
        })
        .await
        .unwrap();
    let resumed = rig
        .arbiter
        .complete_tool_change(confirmation)
        .await
        .unwrap();
    assert_eq!(resumed.state, SenderState::Running);
    rig.wait(SenderState::Draining).await;
    rig.control.advance_program(Duration::from_secs(60));
    let finished = rig.arbiter.refresh_status().await.unwrap();
    rig.wait(SenderState::Completed).await;
    assert_eq!(finished.machine.machine_position.unwrap().a, Some(20.0));
    assert_eq!(
        finished.machine.work_coordinate_offset.unwrap().a,
        Some(0.0)
    );
    assert_eq!(finished.machine.work_coordinate_offset.unwrap().z, 5.0);
    assert!(!rig.control.writes().iter().any(|line| {
        String::from_utf8_lossy(line)
            .split_whitespace()
            .any(|word| word == "M6")
    }));
}
