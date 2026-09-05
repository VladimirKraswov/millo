use super::*;

#[tokio::test]
async fn heightmap_mode_calibration_accepts_run_reported_for_jog_retract() {
    let transport = MockTransport::with_status(
        "<Idle|MPos:10.000,20.000,5.000|WPos:10.000,20.000,5.000|FS:0,0>",
    );
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    control.set_jog_reports_run(true);
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

    assert_eq!(outcome.zero_command, "G10 L20 P1 Z0.000");
    assert!((outcome.final_work_z - 1.0).abs() <= 0.01);
    task.abort();
}

#[tokio::test]
async fn prepared_heightmap_does_not_move_until_committed_and_can_be_discarded() {
    let transport =
        MockTransport::with_status("<Idle|MPos:0.000,0.000,10.000|WPos:0.000,0.000,10.000|FS:0,0>");
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
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

    let prepared = arbiter
        .prepare_heightmap(heightmap_request())
        .await
        .unwrap();
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }

    assert_eq!(prepared.state, HeightmapOperationState::Running);
    assert_eq!(
        arbiter.heightmap_snapshot().state,
        HeightmapOperationState::Idle
    );
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| { write.starts_with(b"$J=") || write.starts_with(b"G38.") })
    );

    assert!(matches!(
        arbiter
            .commit_prepared_heightmap(prepared.operation_sequence + 1)
            .await,
        Err(ArbiterError::PreparedHeightmapMismatch { .. })
    ));
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| { write.starts_with(b"$J=") || write.starts_with(b"G38.") })
    );

    arbiter
        .discard_prepared_heightmap(prepared.operation_sequence)
        .await
        .unwrap();
    assert_eq!(
        arbiter.heightmap_snapshot().state,
        HeightmapOperationState::Idle
    );
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| { write.starts_with(b"$J=") || write.starts_with(b"G38.") })
    );
    task.abort();
}

#[tokio::test]
async fn heightmap_start_waits_for_a_previous_motion_to_reach_idle() {
    let transport = MockTransport::with_status(
        "<Run|MPos:0.000,0.000,10.000|WPos:0.000,0.000,10.000|FS:100,0>",
    );
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
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
        settle_control.set_status("<Idle|MPos:0.000,0.000,10.000|WPos:0.000,0.000,10.000|FS:0,0>");
    });
    let prepared = arbiter
        .prepare_heightmap(heightmap_request())
        .await
        .unwrap();
    settle.await.unwrap();

    assert_eq!(prepared.state, HeightmapOperationState::Running);
    assert!(
        control
            .writes()
            .iter()
            .filter(|write| write.as_slice() == b"?")
            .count()
            >= 2
    );
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"$J=") || write.starts_with(b"G38."))
    );
    arbiter
        .discard_prepared_heightmap(prepared.operation_sequence)
        .await
        .unwrap();
    task.abort();
}

#[tokio::test]
async fn heightmap_probes_a_serpentine_grid_and_establishes_z_zero_once() {
    let transport =
        MockTransport::with_status("<Idle|MPos:0.000,0.000,10.000|WPos:0.000,0.000,10.000|FS:0,0>");
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    control.set_probe_settle_status_polls(2);
    control.set_jog_reports_run(true);
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
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

    arbiter.start_heightmap(heightmap_request()).await.unwrap();

    let completed = wait_for_heightmap(&arbiter, HeightmapOperationState::Completed).await;
    assert_eq!(completed.progress.measured, 4);
    assert_eq!(completed.progress.triggered, 4);
    assert!(completed.progress.complete);
    let writes = control.writes();
    let xy_moves = writes
        .iter()
        .filter(|write| write.starts_with(b"$J=G91 G21 X"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        xy_moves,
        vec![
            b"$J=G91 G21 X10.000 Y20.000 F1000.000\n".to_vec(),
            b"$J=G91 G21 X2.000 Y0.000 F1000.000\n".to_vec(),
            b"$J=G91 G21 X0.000 Y2.000 F1000.000\n".to_vec(),
            b"$J=G91 G21 X-2.000 Y0.000 F1000.000\n".to_vec(),
            b"$J=G91 G21 X-10.000 Y-22.000 F1000.000\n".to_vec(),
        ]
    );
    assert_eq!(
        writes
            .iter()
            .filter(|write| write.starts_with(b"G91 G21 G94 G38.3"))
            .count(),
        4
    );
    assert_eq!(
        writes
            .iter()
            .filter(|write| write.starts_with(b"G10 L20 P1 Z"))
            .count(),
        1
    );
    assert_eq!(
        arbiter
            .snapshot()
            .machine
            .work_position
            .map(|position| (position.x, position.y)),
        Some((0.0, 0.0))
    );
    assert!(
        !writes
            .iter()
            .any(|write| write.starts_with(b"$J=G90 G21 Z"))
    );
    task.abort();
}

#[tokio::test]
async fn heightmap_reuses_probe_established_z_zero_after_xy_zeroing() {
    let transport = MockTransport::with_status(
        "<Idle|MPos:10.000,20.000,5.000|WPos:10.000,20.000,5.000|FS:0,0>",
    );
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
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
    let mut calibration = z_probe_request();
    calibration.settings.mode = ProbeWorkflowMode::Heightmap;
    calibration.settings.plate_thickness_mm = 0.0;

    arbiter.probe_z(calibration).await.unwrap();
    arbiter
        .set_work_zero(work_zero_request(WorkAxis::X, true))
        .await
        .unwrap();
    control.set_probe_trigger_distance(Some(2.0));
    arbiter.start_heightmap(heightmap_request()).await.unwrap();
    let completed = wait_for_heightmap(&arbiter, HeightmapOperationState::Completed).await;

    assert!(completed.progress.complete);
    assert_eq!(
        control
            .writes()
            .iter()
            .filter(|write| write.starts_with(b"G10 L20 P1 Z"))
            .count(),
        1,
        "the map must retain the probe-established Z0 instead of writing it again",
    );
    task.abort();
}

#[tokio::test]
async fn heightmap_probe_miss_keeps_the_draft_and_returns_to_safe_idle_without_reset() {
    let transport =
        MockTransport::with_status("<Idle|MPos:0.000,0.000,2.000|WPos:0.000,0.000,2.000|FS:0,0>");
    let control = transport.control();
    control.set_probe_trigger_distance(None);
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
    let (arbiter, worker) =
        CommandArbiter::new(Box::new(transport), ControllerConfig::default(), profile);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    arbiter.start_heightmap(heightmap_request()).await.unwrap();
    let failed = wait_for_heightmap(&arbiter, HeightmapOperationState::Failed).await;

    assert_eq!(failed.progress.measured, 0);
    assert_eq!(failed.current_sequence, Some(0));
    assert!(failed.error.as_deref().unwrap().contains("did not contact"));
    assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);
    assert!(arbiter.snapshot().reset_notice.is_none());
    let writes = control.writes();
    assert!(!writes.iter().any(|write| write.as_slice() == [0x18]));
    assert!(
        writes
            .iter()
            .any(|write| { String::from_utf8_lossy(write).contains("G38.3 Z-4.000") })
    );
    task.abort();
}

#[tokio::test]
async fn resumed_heightmap_keeps_saved_samples_and_probes_only_the_missing_suffix() {
    let transport = MockTransport::with_status(
        "<Idle|MPos:12.000,20.000,2.000|WPos:12.000,20.000,2.000|FS:0,0>",
    );
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
    let (arbiter, worker) =
        CommandArbiter::new(Box::new(transport), ControllerConfig::default(), profile);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let request = heightmap_request();
    let mut map = Heightmap::new(plan_heightmap(request.plan, None).unwrap());
    map.bind_coordinates(
        WorkCoordinateSystem::G54,
        Position {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            a: None,
        },
    );
    map.record_sample(0, 0.1, true).unwrap();
    map.record_sample(1, 0.0, true).unwrap();
    let prepared = arbiter
        .prepare_resume_heightmap(
            map,
            HeightmapResumeRequest {
                max_probe_depth_mm: 4.0,
                setup_confirmed: true,
                contact_available_at_every_point: true,
            },
        )
        .await
        .unwrap();

    assert_eq!(prepared.progress.measured, 2);
    assert_eq!(prepared.current_sequence, Some(2));
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"G38.3"))
    );
    arbiter
        .commit_prepared_heightmap(prepared.operation_sequence)
        .await
        .unwrap();
    let completed = wait_for_heightmap(&arbiter, HeightmapOperationState::Completed).await;

    assert_eq!(completed.progress.measured, 4);
    assert_eq!(completed.map.unwrap().samples[0].unwrap().z_mm, 0.1);
    let writes = control.writes();
    assert_eq!(
        writes
            .iter()
            .filter(|write| write.starts_with(b"G91 G21 G94 G38.3"))
            .count(),
        2
    );
    assert!(
        writes
            .iter()
            .any(|write| { String::from_utf8_lossy(write).contains("G38.3 Z-6.000") })
    );
    task.abort();
}

#[tokio::test]
async fn heightmap_derives_work_position_after_sparse_reset_status() {
    let transport = MockTransport::with_status("<Idle|MPos:50.000,10.000,-5.000|FS:0,0>");
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    control.set_jog_reports_run(true);
    let mut profile = HardwareProfile::first_machine();
    profile.travel_mm = Some(millo_domain::MachineTravel {
        x: 500.0,
        y: 500.0,
        z: 200.0,
    });
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
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

    arbiter.start_heightmap(heightmap_request()).await.unwrap();

    let completed = wait_for_heightmap(&arbiter, HeightmapOperationState::Completed).await;
    assert_eq!(completed.progress.measured, 4);
    let writes = control.writes();
    let parameters_index = writes
        .iter()
        .position(|write| write.as_slice() == b"$#\n")
        .expect("heightmap must query offsets for sparse GRBL status");
    let first_motion_index = writes
        .iter()
        .position(|write| write.starts_with(b"$J="))
        .expect("heightmap must dispatch safe-Z motion");
    assert!(parameters_index < first_motion_index);
    task.abort();
}

#[test]
fn measured_surface_can_raise_but_never_lower_the_transit_plane() {
    let request = heightmap_request().plan;
    assert!((heightmap_transit_work_z(request, -1.5) - 2.0).abs() <= f64::EPSILON);
    assert!((heightmap_transit_work_z(request, 3.25) - 5.25).abs() <= f64::EPSILON);
}

#[tokio::test]
async fn fixed_plate_heightmap_raises_above_the_plate_surface() {
    let transport =
        MockTransport::with_status("<Idle|MPos:0.000,0.000,25.000|WPos:0.000,0.000,25.000|FS:0,0>");
    let control = transport.control();
    control.set_probe_trigger_distance(Some(2.0));
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
    let (arbiter, worker) =
        CommandArbiter::new(Box::new(transport), ControllerConfig::default(), profile);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    let mut request = heightmap_request();
    request.plan.contact_mode = HeightmapContactMode::FixedPlate;
    request.plan.contact_offset_mm = 19.1;

    arbiter.start_heightmap(request).await.unwrap();
    let completed = wait_for_heightmap(&arbiter, HeightmapOperationState::Completed).await;

    assert_eq!(completed.progress.measured, 4);
    assert!(
        control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"$J=G91 G21 Z"))
    );
    task.abort();
}

#[tokio::test]
async fn heightmap_failure_stops_without_sending_recovery_motion() {
    let transport =
        MockTransport::with_status("<Idle|MPos:0.000,0.000,10.000|WPos:0.000,0.000,10.000|FS:0,0>");
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    control.set_probe_delay(20);
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
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

    arbiter.start_heightmap(heightmap_request()).await.unwrap();
    control.queue_query_error(20);

    let failed = wait_for_heightmap(&arbiter, HeightmapOperationState::Failed).await;
    assert_eq!(failed.progress.measured, 0);
    let writes = control.writes();
    assert_eq!(
        writes
            .iter()
            .filter(|write| write.starts_with(b"$J=G91 G21 Z"))
            .count(),
        0,
    );
    assert!(
        !writes
            .iter()
            .any(|write| write.starts_with(b"$J=G90 G21 Z"))
    );
    assert_eq!(writes[writes.len() - 2], vec![b'!']);
    assert_eq!(writes.last(), Some(&vec![0x18]));
    assert!(!writes.iter().any(|write| write.starts_with(b"G10 L20")));
    task.abort();
}

#[tokio::test]
async fn heightmap_quarantines_a_controller_that_overshoots_relative_z() {
    let transport =
        MockTransport::with_status("<Idle|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:0,0>");
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    control.set_jog_distance_scale(35.0);
    control.set_virtual_motion_enabled(false);
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
    let (arbiter, worker) =
        CommandArbiter::new(Box::new(transport), ControllerConfig::default(), profile);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    arbiter.start_heightmap(heightmap_request()).await.unwrap();
    let failed = wait_for_heightmap(&arbiter, HeightmapOperationState::Failed).await;

    assert!(
        failed.error.as_deref().is_some_and(|error| {
            error.contains("heightmap Z movement ended") && error.contains("expected 2.000")
        }),
        "unexpected failure: {:?}",
        failed.error
    );
    let writes = control.writes();
    assert_eq!(
        writes
            .iter()
            .filter(|write| write.starts_with(b"G38.3"))
            .count(),
        0,
    );
    assert_eq!(writes[writes.len() - 2], vec![b'!']);
    assert_eq!(writes.last(), Some(&vec![0x18]));
    assert!(!writes.iter().any(|write| write.starts_with(b"$J=G90")));
    task.abort();
}

#[tokio::test]
async fn heightmap_link_loss_reports_that_emergency_stop_was_not_delivered() {
    let transport =
        MockTransport::with_status("<Idle|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:0,0>");
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            failures_before_recovery: 1,
            ..ControllerConfig::default()
        },
        profile,
    );
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    arbiter.start_heightmap(heightmap_request()).await.unwrap();
    control.drop_link();
    let failed = wait_for_heightmap(&arbiter, HeightmapOperationState::Failed).await;

    assert!(failed.error.as_deref().is_some_and(|error| {
        error.contains("emergency stop delivery failed")
            && error.contains("Stop could not be delivered")
    }));
    assert!(!control.writes().iter().any(|write| write == b"!"));
    assert!(!control.writes().iter().any(|write| write == &[0x18]));
    task.abort();
}

#[tokio::test]
async fn soft_reset_cancels_heightmap_before_another_probe_point() {
    let transport =
        MockTransport::with_status("<Idle|MPos:0.000,0.000,10.000|WPos:0.000,0.000,10.000|FS:0,0>");
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    control.set_probe_delay(100);
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
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

    arbiter.start_heightmap(heightmap_request()).await.unwrap();
    tokio::task::yield_now().await;
    arbiter
        .send_realtime(RealtimeCommand::SoftReset)
        .await
        .unwrap();

    let cancelled = wait_for_heightmap(&arbiter, HeightmapOperationState::Cancelled).await;
    assert_eq!(cancelled.current_sequence, None);
    assert!(
        cancelled
            .error
            .as_deref()
            .is_some_and(|error| error.contains("reset"))
    );
    assert!(
        control
            .writes()
            .iter()
            .filter(|write| write.starts_with(b"G91 G21 G94 G38.3"))
            .count()
            <= 1
    );
    task.abort();
}

#[tokio::test]
async fn dedicated_heightmap_cancel_preempts_the_operation_and_stays_terminal() {
    let transport =
        MockTransport::with_status("<Idle|MPos:0.000,0.000,10.000|WPos:0.000,0.000,10.000|FS:0,0>");
    let control = transport.control();
    control.set_probe_trigger_distance(Some(1.0));
    control.set_probe_delay(100);
    let mut profile = HardwareProfile::first_machine();
    profile.probe_installed = true;
    profile.probe_mode = ProbeWorkflowMode::Heightmap;
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            ..ControllerConfig::default()
        },
        profile,
    );
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.start_heightmap(heightmap_request()).await.unwrap();

    let cancelled = arbiter.cancel_heightmap().await.unwrap();
    assert_eq!(cancelled.state, HeightmapOperationState::Cancelled);
    assert_eq!(cancelled.current_sequence, None);
    let writes_after_cancel = control.writes();
    assert_eq!(
        writes_after_cancel[writes_after_cancel.len() - 2],
        vec![b'!']
    );
    assert_eq!(writes_after_cancel.last(), Some(&vec![0x18]));

    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(control.writes(), writes_after_cancel);
    assert_eq!(
        arbiter.heightmap_snapshot().state,
        HeightmapOperationState::Cancelled
    );
    task.abort();
}
