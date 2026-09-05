use super::*;

#[tokio::test]
async fn test_jog_preparation_runs_a_fresh_inspection_each_time() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();

    let first = arbiter
        .prepare_test_jog(operator_confirmation())
        .await
        .unwrap();
    let second = arbiter
        .prepare_test_jog(operator_confirmation())
        .await
        .unwrap();

    assert!(first.authorization.is_some());
    assert!(second.authorization.is_some());
    assert_ne!(
        first.authorization.unwrap().id,
        second.authorization.unwrap().id
    );
    assert_eq!(
        control.writes(),
        vec![
            b"?".to_vec(),
            b"?".to_vec(),
            b"$I\n".to_vec(),
            b"$$\n".to_vec(),
            b"$G\n".to_vec(),
            b"$#\n".to_vec(),
            b"?".to_vec(),
            b"$I\n".to_vec(),
            b"$$\n".to_vec(),
            b"$G\n".to_vec(),
            b"$#\n".to_vec(),
        ]
    );
    task.abort();
}

#[tokio::test]
async fn consumes_authorization_before_writing_one_typed_step_jog() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();
    let authorization = arbiter
        .prepare_test_jog(operator_confirmation())
        .await
        .unwrap()
        .authorization
        .unwrap();

    let receipt = arbiter
        .step_jog(StepJogRequest {
            authorization_id: authorization.id,
            axis: millo_domain::JogAxis::X,
            distance_mm: 0.1,
            feed_mm_per_min: 50.0,
        })
        .await
        .unwrap();
    let reused = arbiter
        .step_jog(StepJogRequest {
            authorization_id: authorization.id,
            axis: millo_domain::JogAxis::X,
            distance_mm: 0.1,
            feed_mm_per_min: 50.0,
        })
        .await
        .unwrap_err();

    assert_eq!(receipt.command, "$J=G91 G21 X0.100 F50.000");
    assert!(matches!(
        reused,
        ArbiterError::Safety(SafetyError::TestJogAuthorizationMissing)
    ));
    assert_eq!(
        control.writes().last(),
        Some(&b"$J=G91 G21 X0.100 F50.000\n".to_vec())
    );
    assert_eq!(
        control
            .writes()
            .iter()
            .filter(|write| write.starts_with(b"$J="))
            .count(),
        1
    );
    task.abort();
}

#[tokio::test]
async fn failed_jog_validation_still_consumes_the_authorization() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();
    let authorization = arbiter
        .prepare_test_jog(operator_confirmation())
        .await
        .unwrap()
        .authorization
        .unwrap();

    let invalid = arbiter
        .step_jog(StepJogRequest {
            authorization_id: authorization.id,
            axis: millo_domain::JogAxis::Z,
            distance_mm: MAX_STEP_JOG_DISTANCE_MM + 0.01,
            feed_mm_per_min: 50.0,
        })
        .await
        .unwrap_err();
    let retry = arbiter
        .step_jog(StepJogRequest {
            authorization_id: authorization.id,
            axis: millo_domain::JogAxis::Z,
            distance_mm: 0.1,
            feed_mm_per_min: 50.0,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        invalid,
        ArbiterError::Controller(ControllerError::JogValidation(_))
    ));
    assert!(matches!(
        retry,
        ArbiterError::Safety(SafetyError::TestJogAuthorizationMissing)
    ));
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"$J="))
    );
    task.abort();
}

#[tokio::test]
async fn jog_cancel_is_available_only_for_reported_jog_state() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();

    assert!(matches!(
        arbiter.cancel_jog().await.unwrap_err(),
        ArbiterError::JogCancelUnavailable(MachineMode::Idle)
    ));

    let authorization = arbiter
        .prepare_test_jog(operator_confirmation())
        .await
        .unwrap()
        .authorization
        .unwrap();
    arbiter
        .step_jog(StepJogRequest {
            authorization_id: authorization.id,
            axis: millo_domain::JogAxis::Y,
            distance_mm: -1.0,
            feed_mm_per_min: 10.0,
        })
        .await
        .unwrap();
    assert_eq!(
        arbiter.refresh_status().await.unwrap().machine.mode,
        MachineMode::Jog
    );

    arbiter.cancel_jog().await.unwrap();
    assert_eq!(control.writes().last(), Some(&vec![0x85]));
    assert_eq!(
        wait_for_controller_idle(&arbiter).await.machine.mode,
        MachineMode::Idle
    );
    task.abort();
}

#[tokio::test]
async fn continuous_jog_is_bounded_and_cancelled_by_realtime_byte() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let receipt = arbiter
        .start_continuous_jog(ContinuousJogRequest {
            confirmation: operator_confirmation(),
            axis: millo_domain::JogAxis::X,
            direction: 1,
            feed_mm_per_min: 300.0,
        })
        .await
        .unwrap();
    assert_eq!(receipt.boundary_source, JogBoundarySource::ProfileDistance);
    assert_eq!(receipt.bounded_distance, 50.0);
    assert_eq!(receipt.command, "$J=G91 G21 X50.000 F300.000");

    arbiter.cancel_jog().await.unwrap();
    assert_eq!(control.writes().last(), Some(&vec![0x85]));
    assert_eq!(
        wait_for_controller_idle(&arbiter).await.machine.mode,
        MachineMode::Idle
    );
    task.abort();
}

#[tokio::test]
async fn optional_a_axis_uses_its_own_profile_limits() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let mut profile = HardwareProfile::first_machine();
    profile.axes.push("A".to_owned());
    profile.rotary_axis = Some(millo_domain::RotaryAxisProfile {
        travel_degrees: 360.0,
        max_jog_degrees: 30.0,
        max_feed_degrees_per_min: 720.0,
    });
    profile.max_jog_distance_mm = 1.0;
    let task = tokio::spawn(worker);
    arbiter.set_hardware_profile(profile).await.unwrap();
    arbiter.connect().await.unwrap();

    let outcome = arbiter
        .jog_pad_step(JogPadStepRequest {
            confirmation: operator_confirmation(),
            axis: millo_domain::JogAxis::A,
            distance_mm: 5.0,
            feed_mm_per_min: 360.0,
        })
        .await
        .unwrap();
    assert_eq!(
        outcome.receipt.unwrap().command,
        "$J=G91 G21 A5.000 F360.000"
    );
    assert!(
        control
            .writes()
            .iter()
            .any(|write| write == b"$J=G91 G21 A5.000 F360.000\n")
    );
    task.abort();
}

#[tokio::test]
async fn y_and_z_steps_each_require_a_fresh_authorization() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();

    let mut authorization_ids = Vec::new();
    for axis in [millo_domain::JogAxis::Y, millo_domain::JogAxis::Z] {
        let authorization = arbiter
            .prepare_test_jog(operator_confirmation())
            .await
            .unwrap()
            .authorization
            .unwrap();
        authorization_ids.push(authorization.id);

        arbiter
            .step_jog(StepJogRequest {
                authorization_id: authorization.id,
                axis,
                distance_mm: 0.1,
                feed_mm_per_min: 100.0,
            })
            .await
            .unwrap();

        assert_eq!(
            arbiter.refresh_status().await.unwrap().machine.mode,
            MachineMode::Jog
        );
        assert_eq!(
            wait_for_controller_idle(&arbiter).await.machine.mode,
            MachineMode::Idle
        );
    }

    assert_ne!(authorization_ids[0], authorization_ids[1]);
    let snapshot = arbiter.snapshot();
    let position = snapshot.machine.machine_position.unwrap();
    assert_eq!(position.x, 0.0);
    assert_eq!(position.y, 0.1);
    assert_eq!(position.z, 0.1);

    let jog_writes = control
        .writes()
        .into_iter()
        .filter(|write| write.starts_with(b"$J="))
        .collect::<Vec<_>>();
    assert_eq!(
        jog_writes,
        vec![
            b"$J=G91 G21 Y0.100 F100.000\n".to_vec(),
            b"$J=G91 G21 Z0.100 F100.000\n".to_vec()
        ]
    );
    task.abort();
}

#[tokio::test]
async fn jog_pad_rechecks_motion_and_forwards_selected_feed() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let first = arbiter
        .jog_pad_step(JogPadStepRequest {
            confirmation: operator_confirmation(),
            axis: millo_domain::JogAxis::Y,
            distance_mm: 0.1,
            feed_mm_per_min: 300.0,
        })
        .await
        .unwrap();
    assert_eq!(first.receipt.unwrap().command, "$J=G91 G21 Y0.100 F300.000");

    let blocked_while_moving = arbiter
        .jog_pad_step(JogPadStepRequest {
            confirmation: operator_confirmation(),
            axis: millo_domain::JogAxis::Z,
            distance_mm: 0.01,
            feed_mm_per_min: 100.0,
        })
        .await
        .unwrap();
    assert!(blocked_while_moving.receipt.is_none());
    assert!(!blocked_while_moving.inspection.readiness.test_jog_ready);

    wait_for_controller_idle(&arbiter).await;
    let second = arbiter
        .jog_pad_step(JogPadStepRequest {
            confirmation: operator_confirmation(),
            axis: millo_domain::JogAxis::Z,
            distance_mm: -0.01,
            feed_mm_per_min: 100.0,
        })
        .await
        .unwrap();
    assert_eq!(
        second.receipt.unwrap().command,
        "$J=G91 G21 Z-0.010 F100.000"
    );

    let writes = control.writes();
    assert_eq!(
        writes
            .iter()
            .filter(|write| write.as_slice() == b"$I\n")
            .count(),
        3
    );
    assert_eq!(
        writes
            .into_iter()
            .filter(|write| write.starts_with(b"$J="))
            .collect::<Vec<_>>(),
        vec![
            b"$J=G91 G21 Y0.100 F300.000\n".to_vec(),
            b"$J=G91 G21 Z-0.010 F100.000\n".to_vec()
        ]
    );
    task.abort();
}

#[tokio::test]
async fn jog_pad_rejects_distance_above_machine_profile_before_controller_io() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let error = arbiter
        .jog_pad_step(JogPadStepRequest {
            confirmation: operator_confirmation(),
            axis: millo_domain::JogAxis::X,
            distance_mm: 50.01,
            feed_mm_per_min: 100.0,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ArbiterError::JogPadDistanceExceedsProfile {
            axis: millo_domain::JogAxis::X,
            requested,
            maximum,
        } if requested == 50.01 && maximum == 50.0
    ));
    assert!(control.writes().is_empty());
    task.abort();
}

#[tokio::test]
async fn jog_pad_rejects_distance_above_selected_axis_travel() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let mut profile = HardwareProfile::first_machine();
    profile.travel_mm = Some(millo_domain::MachineTravel {
        x: 300.0,
        y: 180.0,
        z: 20.0,
    });
    profile.max_jog_distance_mm = 50.0;
    let task = tokio::spawn(worker);
    arbiter.set_hardware_profile(profile).await.unwrap();
    arbiter.connect().await.unwrap();

    let error = arbiter
        .jog_pad_step(JogPadStepRequest {
            confirmation: operator_confirmation(),
            axis: millo_domain::JogAxis::Z,
            distance_mm: 20.01,
            feed_mm_per_min: 100.0,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ArbiterError::JogPadDistanceExceedsProfile {
            axis: millo_domain::JogAxis::Z,
            requested,
            maximum,
        } if requested == 20.01 && maximum == 20.0
    ));
    assert!(control.writes().is_empty());
    task.abort();
}

#[tokio::test]
async fn jog_pad_rejects_feed_above_selected_axis_rate() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let error = arbiter
        .jog_pad_step(JogPadStepRequest {
            confirmation: operator_confirmation(),
            axis: millo_domain::JogAxis::X,
            distance_mm: 1.0,
            feed_mm_per_min: 1_001.0,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ArbiterError::JogPadFeedExceedsAxisRate {
            axis: millo_domain::JogAxis::X,
            requested,
            maximum,
        } if requested == 1_001.0 && maximum == 1_000.0
    ));
    assert!(
        control
            .writes()
            .into_iter()
            .all(|write| !write.starts_with(b"$J="))
    );
    task.abort();
}

#[tokio::test]
async fn return_to_work_zero_uses_absolute_jog_without_mutating_the_offset() {
    let transport = MockTransport::with_status(
        "<Idle|MPos:10.000,20.000,3.000|WPos:10.000,20.000,3.000|FS:0,0>",
    );
    let control = transport.control();
    control.set_active_wcs(55);
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            status_timeout: Duration::from_millis(20),
            command_timeout: Duration::from_millis(50),
            failures_before_recovery: 2,
        },
        HardwareProfile::first_machine(),
    );
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let outcome = arbiter
        .return_to_work_zero(return_to_zero_request(WorkAxis::Z))
        .await
        .unwrap();

    assert_eq!(outcome.coordinate_system, WorkCoordinateSystem::G55);
    assert_eq!(outcome.command, "$J=G90 G21 Z0.000 F100.000");
    assert!(
        control
            .writes()
            .contains(&b"$J=G90 G21 Z0.000 F100.000\n".to_vec())
    );
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"G10 L20"))
    );
    task.abort();
}

#[tokio::test]
async fn safe_return_raises_z_then_returns_xy_and_z_without_mutating_work_zero() {
    let transport = MockTransport::with_status(
        "<Idle|MPos:12.000,8.000,-0.200|WPos:12.000,8.000,-0.200|FS:0,0>",
    );
    let control = transport.control();
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            status_timeout: Duration::from_millis(20),
            command_timeout: Duration::from_millis(50),
            failures_before_recovery: 2,
        },
        HardwareProfile::first_machine(),
    );
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let outcome = arbiter
        .return_to_work_origin(ReturnToWorkOriginRequest {
            clearance_z_mm: 2.0,
            xy_feed_mm_per_min: 300.0,
            z_feed_mm_per_min: 100.0,
        })
        .await
        .unwrap();

    assert_eq!(outcome.coordinate_system, WorkCoordinateSystem::G54);
    assert_eq!(
        outcome.commands,
        vec![
            "$J=G90 G21 Z2.000 F100.000",
            "$J=G90 G21 X0.000 Y0.000 F300.000",
            "$J=G90 G21 Z0.000 F100.000",
        ]
    );
    assert_eq!(
        outcome.snapshot.machine.work_position,
        Some(Position {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            a: None
        })
    );
    assert!(
        !control
            .writes()
            .iter()
            .any(|write| write.starts_with(b"G10 L20"))
    );
    task.abort();
}
