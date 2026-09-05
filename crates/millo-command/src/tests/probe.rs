use super::*;

#[tokio::test]
async fn z_probe_sets_contact_height_retracts_and_verifies_final_work_z() {
    let transport = MockTransport::with_status(
        "<Idle|MPos:10.000,20.000,10.000|WPos:10.000,20.000,10.000|FS:0,0>",
    );
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::WorkZero;
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            status_timeout: Duration::from_millis(20),
            command_timeout: Duration::from_millis(50),
            failures_before_recovery: 2,
        },
        profile,
    );
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let outcome = arbiter.probe_z(z_probe_request()).await.unwrap();

    assert_eq!(outcome.coordinate_system, WorkCoordinateSystem::G54);
    assert_eq!(outcome.contact_machine_position.z, 9.0);
    assert!((outcome.final_work_z - 20.1).abs() <= 0.01);
    assert_eq!(outcome.probe_command, "G91 G21 G94 G38.3 Z-10.000 F25.000");
    assert_eq!(outcome.zero_command, "G10 L20 P1 Z19.100");
    assert_eq!(outcome.retract_command, "$J=G91 G21 Z1.000 F1000.000");
    assert!(control.writes().contains(&b"G0 G21 G90 G94\n".to_vec()));
    task.abort();
}

#[tokio::test]
async fn z_probe_start_waits_for_a_previous_motion_to_reach_idle() {
    let transport = MockTransport::with_status(
        "<Run|MPos:10.000,20.000,10.000|WPos:10.000,20.000,10.000|FS:100,0>",
    );
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::WorkZero;
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            status_timeout: Duration::from_millis(20),
            command_timeout: Duration::from_millis(100),
            failures_before_recovery: 2,
        },
        profile,
    );
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let settle_control = control.clone();
    let settle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        settle_control
            .set_status("<Idle|MPos:10.000,20.000,10.000|WPos:10.000,20.000,10.000|FS:0,0>");
    });
    let outcome = arbiter.probe_z(z_probe_request()).await.unwrap();
    settle.await.unwrap();

    assert_eq!(outcome.contact_machine_position.z, 9.0);
    let writes = control.writes();
    let modal_index = writes
        .iter()
        .position(|write| write.as_slice() == b"$G\n")
        .unwrap();
    assert!(
        writes[..modal_index]
            .iter()
            .filter(|write| write.as_slice() == b"?")
            .count()
            >= 2
    );
    task.abort();
}

#[tokio::test]
async fn probe_start_wait_keeps_realtime_hold_available() {
    let transport = MockTransport::with_status(
        "<Run|MPos:10.000,20.000,10.000|WPos:10.000,20.000,10.000|FS:100,0>",
    );
    let control = transport.control();
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::WorkZero;
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            status_timeout: Duration::from_millis(20),
            command_timeout: Duration::from_millis(100),
            failures_before_recovery: 2,
        },
        profile,
    );
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let probe_arbiter = arbiter.clone();
    let probe = tokio::spawn(async move { probe_arbiter.probe_z(z_probe_request()).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    arbiter
        .send_realtime(RealtimeCommand::FeedHold)
        .await
        .unwrap();

    assert!(matches!(
        probe.await.unwrap(),
        Err(ArbiterError::ProbeStartBlocked {
            mode: MachineMode::Hold,
            ..
        })
    ));
    let writes = control.writes();
    assert!(writes.iter().any(|write| write.as_slice() == b"!"));
    assert!(!writes.iter().any(|write| write.starts_with(b"G38.")));
    task.abort();
}

#[tokio::test]
async fn probe_start_is_rejected_while_sender_is_active() {
    let (arbiter, control, worker) = mock_dry_run_arbiter();
    control.queue_program_stall();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();
    arbiter
        .start_dry_run(dry_run_plan("G21 G90\nG1 X10 F100"))
        .await
        .unwrap();

    let error = arbiter.probe_z(z_probe_request()).await.unwrap_err();

    assert!(matches!(error, ArbiterError::MachineOperationBusy));
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"G38."))
    );
    task.abort();
}

#[tokio::test]
async fn z_probe_waits_for_idle_after_terminal_ack_before_writing_work_zero() {
    let transport = MockTransport::with_status(
        "<Idle|MPos:10.000,20.000,5.000|WPos:10.000,20.000,5.000|FS:0,0>",
    );
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    control.set_probe_settle_status_polls(2);
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
    let (arbiter, worker) =
        CommandArbiter::new(Box::new(transport), ControllerConfig::default(), profile);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    let mut request = z_probe_request();
    request.settings.mode = ProbeWorkflowMode::Heightmap;
    request.settings.plate_thickness_mm = 0.0;

    let outcome = arbiter.probe_z(request).await.unwrap();

    assert!((outcome.final_work_z - 1.0).abs() <= 0.01);
    let writes = control.writes();
    let probe_index = writes
        .iter()
        .position(|write| write.starts_with(b"G91 G21 G94 G38.3"))
        .unwrap();
    let zero_index = writes
        .iter()
        .position(|write| write.starts_with(b"G10 L20 P1 Z0.000"))
        .unwrap();
    assert!(zero_index > probe_index);
    assert!(
        writes[probe_index + 1..zero_index]
            .iter()
            .filter(|write| write.as_slice() == b"?")
            .count()
            >= 2
    );
    task.abort();
}

#[tokio::test]
async fn z_probe_blocks_motion_when_probe_input_is_already_active() {
    let transport = MockTransport::with_status(
        "<Idle|MPos:0.000,0.000,10.000|WPos:0.000,0.000,10.000|FS:0,0|Pn:P>",
    );
    let control = transport.control();
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::WorkZero;
    let (arbiter, worker) =
        CommandArbiter::new(Box::new(transport), ControllerConfig::default(), profile);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let error = arbiter.probe_z(z_probe_request()).await.unwrap_err();

    assert!(matches!(error, ArbiterError::ZProbeInputAlreadyActive));
    assert_eq!(control.writes(), vec![b"?".to_vec()]);
    task.abort();
}

#[tokio::test]
async fn z_probe_requires_an_installed_probe_before_any_controller_io() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let error = arbiter.probe_z(z_probe_request()).await.unwrap_err();

    assert!(matches!(error, ArbiterError::ZProbeNotInstalled));
    assert!(control.writes().is_empty());
    task.abort();
}

#[tokio::test]
async fn z_probe_requires_work_zero_mode_before_any_controller_io() {
    let transport = MockTransport::default();
    let control = transport.control();
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::WorkZero;
    let (arbiter, worker) =
        CommandArbiter::new(Box::new(transport), ControllerConfig::default(), profile);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    let mut request = z_probe_request();
    request.settings.mode = ProbeWorkflowMode::Off;

    let error = arbiter.probe_z(request).await.unwrap_err();

    assert!(matches!(error, ArbiterError::ZProbeDisabled));
    assert!(control.writes().is_empty());
    task.abort();
}

#[tokio::test]
async fn z_probe_miss_returns_to_start_without_alarm_or_work_offset() {
    let transport =
        MockTransport::with_status("<Idle|MPos:0.000,0.000,10.000|WPos:0.000,0.000,10.000|FS:0,0>");
    let control = transport.control();
    control.set_probe_trigger_distance(None);
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::WorkZero;
    let (arbiter, worker) =
        CommandArbiter::new(Box::new(transport), ControllerConfig::default(), profile);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let error = arbiter.probe_z(z_probe_request()).await.unwrap_err();

    assert!(matches!(error, ArbiterError::ZProbeContactNotFound));
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"G10 "))
    );
    assert!(
        control
            .writes()
            .contains(&b"$J=G91 G21 Z10.000 F1000.000\n".to_vec())
    );
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"$X"))
    );
    task.abort();
}

#[tokio::test]
async fn soft_reset_preempts_a_probe_wait_and_prevents_offset_write() {
    let transport =
        MockTransport::with_status("<Idle|MPos:0.000,0.000,10.000|WPos:0.000,0.000,10.000|FS:0,0>");
    let control = transport.control();
    control.set_probe_delay(50);
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::WorkZero;
    let (arbiter, worker) =
        CommandArbiter::new(Box::new(transport), ControllerConfig::default(), profile);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let probe_actor = arbiter.clone();
    let probe_task = tokio::spawn(async move { probe_actor.probe_z(z_probe_request()).await });
    tokio::time::sleep(Duration::from_millis(5)).await;
    let challenge = arbiter.request_soft_reset().await.unwrap();
    arbiter.confirm_soft_reset(challenge.id).await.unwrap();

    assert!(matches!(
        probe_task.await.unwrap(),
        Err(ArbiterError::ZProbeReset)
    ));
    assert!(control.writes().contains(&b"\x18".to_vec()));
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"G10 "))
    );
    task.abort();
}

#[tokio::test]
async fn machine_commands_are_rejected_instead_of_replayed_after_probe() {
    let transport = MockTransport::with_status(
        "<Idle|MPos:10.000,20.000,10.000|WPos:10.000,20.000,10.000|FS:0,0>",
    );
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    control.set_probe_delay(100);
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::WorkZero;
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            status_timeout: Duration::from_millis(20),
            command_timeout: Duration::from_millis(100),
            failures_before_recovery: 2,
        },
        profile,
    );
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let probe_arbiter = arbiter.clone();
    let probe = tokio::spawn(async move { probe_arbiter.probe_z(z_probe_request()).await });
    for _ in 0..100 {
        if control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"G91 G21 G94 G38.3"))
        {
            break;
        }
        tokio::task::yield_now().await;
    }

    let error = arbiter
        .set_work_zero(work_zero_request(WorkAxis::X, true))
        .await
        .unwrap_err();
    assert!(matches!(error, ArbiterError::MachineOperationBusy));
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"G10 L20"))
    );

    arbiter
        .send_realtime(RealtimeCommand::SoftReset)
        .await
        .unwrap();
    assert!(matches!(
        probe.await.unwrap(),
        Err(ArbiterError::ZProbeReset)
    ));
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"G10 L20"))
    );
    task.abort();
}
