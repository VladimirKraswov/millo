use super::*;

#[tokio::test]
async fn selects_and_verifies_each_supported_work_coordinate_system() {
    let (arbiter, _, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    for coordinate_system in [
        WorkCoordinateSystem::G54,
        WorkCoordinateSystem::G55,
        WorkCoordinateSystem::G56,
        WorkCoordinateSystem::G57,
        WorkCoordinateSystem::G58,
        WorkCoordinateSystem::G59,
    ] {
        let outcome = arbiter
            .select_work_coordinate_system(coordinate_system)
            .await
            .unwrap();
        assert_eq!(outcome.coordinate_system, coordinate_system);
        assert_eq!(
            outcome.command,
            work_coordinate_parameter(coordinate_system)
        );
    }
    task.abort();
}

#[tokio::test]
async fn work_zero_requires_confirmation_before_controller_io() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let error = arbiter
        .set_work_zero(work_zero_request(WorkAxis::X, false))
        .await
        .unwrap_err();

    assert!(matches!(error, ArbiterError::WorkZeroConfirmationRequired));
    assert!(control.writes().is_empty());
    task.abort();
}

#[tokio::test]
async fn work_zero_rechecks_idle_before_writing() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    control.set_status("<Run|MPos:1.000,2.000,3.000|WPos:1.000,2.000,3.000|FS:100,0>");
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let error = arbiter
        .set_work_zero(work_zero_request(WorkAxis::X, true))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ArbiterError::Safety(SafetyError::UnsafeControllerState)
    ));
    assert_eq!(control.writes(), vec![b"?".to_vec()]);
    task.abort();
}

#[tokio::test]
async fn work_zero_sets_and_verifies_each_axis_in_the_active_wcs() {
    let transport = MockTransport::with_status(
        "<Idle|MPos:10.000,20.000,30.000|WPos:10.000,20.000,30.000|FS:0,0>",
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

    for (axis, expected_parameter) in [
        (WorkAxis::X, "10.000,0.000,0.000"),
        (WorkAxis::Y, "10.000,20.000,0.000"),
        (WorkAxis::Z, "10.000,20.000,30.000"),
    ] {
        let outcome = arbiter
            .set_work_zero(work_zero_request(axis, true))
            .await
            .unwrap();
        assert_eq!(outcome.axis, axis);
        assert_eq!(outcome.coordinate_system, WorkCoordinateSystem::G55);
        assert_eq!(outcome.parameter_value, expected_parameter);
        assert!(outcome.work_position.abs() <= WORK_ZERO_TOLERANCE_MM);
        assert_eq!(
            outcome.snapshot.machine.machine_position,
            Some(Position {
                x: 10.0,
                y: 20.0,
                z: 30.0,
                a: None,
            })
        );
    }

    assert_eq!(
        control.writes(),
        vec![
            b"?".to_vec(),
            b"$G\n".to_vec(),
            b"G10 L20 P2 X0\n".to_vec(),
            b"$#\n".to_vec(),
            b"?".to_vec(),
            b"?".to_vec(),
            b"$G\n".to_vec(),
            b"G10 L20 P2 Y0\n".to_vec(),
            b"$#\n".to_vec(),
            b"?".to_vec(),
            b"?".to_vec(),
            b"$G\n".to_vec(),
            b"G10 L20 P2 Z0\n".to_vec(),
            b"$#\n".to_vec(),
            b"?".to_vec(),
        ]
    );
    task.abort();
}

#[tokio::test]
async fn lateral_return_to_zero_requires_positive_work_z_clearance() {
    let (arbiter, _control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let error = arbiter
        .return_to_work_zero(return_to_zero_request(WorkAxis::X))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ArbiterError::ReturnToZeroNeedsClearance(WorkAxis::X)
    ));
    task.abort();
}
