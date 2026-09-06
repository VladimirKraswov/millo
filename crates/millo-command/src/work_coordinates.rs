use super::*;

const WORK_ZERO_TOLERANCE_DEGREES: f64 = 0.01;

pub(super) async fn execute_set_work_zero(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    request: WorkZeroRequest,
) -> Result<WorkZeroOutcome, ArbiterError> {
    if !request.position_confirmed {
        return Err(ArbiterError::WorkZeroConfirmationRequired);
    }

    controller.refresh_status().await?;
    ensure_stable_idle(&controller.snapshot())?;

    let rotary_context = if request.axis == WorkAxis::A {
        let initial = controller.snapshot();
        let inspection = controller.inspect_device().await?;
        let current = controller.refresh_status().await?;
        ensure_stable_idle(&current)?;
        verify_zero_epoch(&initial, &current)?;
        super::rotary_program::validate_rotary_capability(hardware_profile, &inspection, &current)?;
        Some((inspection, current))
    } else {
        None
    };

    let modal_response = controller
        .query_device(millo_controller::DeviceQuery::ModalState)
        .await?;
    let modal = build_device_inspection(vec![modal_response]);
    let coordinate_system = active_work_coordinate_system(&modal.modal_state)
        .ok_or(ArbiterError::ActiveWorkCoordinateSystemUnavailable)?;

    ensure_stable_idle(&controller.snapshot())?;
    if let Some((inspection, initial)) = &rotary_context {
        verify_zero_epoch(initial, &controller.snapshot())?;
        verified_rotary_work_axis(&controller.snapshot(), inspection, coordinate_system)?;
    }
    let command_response = controller
        .set_work_zero(request.axis, coordinate_system)
        .await?;
    let parameter_response = controller
        .query_device(millo_controller::DeviceQuery::Parameters)
        .await?;
    let parameters = build_device_inspection(vec![parameter_response]);
    let parameter_name = work_coordinate_parameter(coordinate_system);
    let parameter_value = parameters
        .parameters
        .get(parameter_name)
        .cloned()
        .ok_or_else(|| {
            ArbiterError::WorkZeroVerification(format!("$# did not return {parameter_name}"))
        })?;
    let valid_parameter = if request.axis == WorkAxis::A {
        parse_xyza_parameter(&parameter_value).is_some()
    } else {
        parse_xyz_parameter(&parameter_value).is_some()
    };
    if !valid_parameter {
        return Err(ArbiterError::WorkZeroVerification(format!(
            "$# returned malformed {parameter_name}: {parameter_value}"
        )));
    }

    let snapshot = controller.refresh_status().await?;
    ensure_stable_idle(&snapshot)?;
    if let Some((_, initial)) = &rotary_context {
        verify_zero_epoch(initial, &snapshot)?;
    }
    let work_position =
        verified_work_axis(&snapshot, &parameters, coordinate_system, request.axis)?;
    let (tolerance, units) = if request.axis == WorkAxis::A {
        (WORK_ZERO_TOLERANCE_DEGREES, "degrees")
    } else {
        (WORK_ZERO_TOLERANCE_MM, "mm")
    };
    if !work_position.is_finite() || work_position.abs() > tolerance {
        return Err(ArbiterError::WorkZeroVerification(format!(
            "expected {:?}=0 in {parameter_name}, read {work_position:.3} {units}",
            request.axis
        )));
    }

    Ok(WorkZeroOutcome {
        axis: request.axis,
        coordinate_system,
        command: command_response.command,
        parameter_value,
        work_position,
        snapshot,
    })
}

pub(super) async fn execute_return_to_work_zero(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    request: ReturnToWorkZeroRequest,
) -> Result<ReturnToWorkZeroOutcome, ArbiterError> {
    if request.axis == WorkAxis::A {
        return Err(ControllerError::JogValidation(
            millo_grbl::JogValidationError::RotaryClearanceRequired,
        )
        .into());
    }
    validate_jog_pad_motion(0.01, request.feed_mm_per_min)?;
    let snapshot = controller.refresh_status().await?;
    ensure_stable_idle(&snapshot)?;
    let work_position = snapshot
        .machine
        .work_position
        .ok_or(ArbiterError::WorkPositionUnavailable)?;
    if !matches!(request.axis, WorkAxis::Z) && work_position.z <= 0.0 {
        return Err(ArbiterError::ReturnToZeroNeedsClearance(request.axis));
    }

    let inspection = controller.inspect_device().await?;
    let coordinate_system = active_work_coordinate_system(&inspection.modal_state)
        .ok_or(ArbiterError::ActiveWorkCoordinateSystemUnavailable)?;
    ensure_stable_idle(&controller.snapshot())?;

    let jog_axis = work_axis_to_jog_axis(request.axis);
    let current = work_axis_value(work_position, request.axis).abs();
    let maximum = axis_travel_limit(hardware_profile, jog_axis);
    if current > maximum {
        return Err(ArbiterError::ReturnToZeroDistanceExceedsProfile {
            axis: request.axis,
            requested: current,
            maximum,
        });
    }
    if let Some(maximum) = axis_max_rate(&inspection, jog_axis)
        && request.feed_mm_per_min > maximum
    {
        return Err(ArbiterError::JogPadFeedExceedsAxisRate {
            axis: jog_axis,
            requested: request.feed_mm_per_min,
            maximum,
        });
    }

    let response = controller.return_to_work_zero(request).await?;
    let snapshot = controller.refresh_status().await?;
    Ok(ReturnToWorkZeroOutcome {
        axis: request.axis,
        coordinate_system,
        command: response.command,
        snapshot,
    })
}

pub(super) async fn execute_return_to_work_origin(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    request: ReturnToWorkOriginRequest,
) -> Result<ReturnToWorkOriginOutcome, ArbiterError> {
    validate_jog_pad_motion(0.01, request.xy_feed_mm_per_min)?;
    validate_jog_pad_motion(0.01, request.z_feed_mm_per_min)?;
    if !request.clearance_z_mm.is_finite() || request.clearance_z_mm < 0.1 {
        return Err(ArbiterError::JogPadDistanceOutOfRange);
    }

    let initial = controller.refresh_status().await?;
    ensure_stable_idle(&initial)?;
    let mut position = verified_heightmap_work_position(&initial)?;
    let inspection = controller.inspect_device().await?;
    let coordinate_system = active_work_coordinate_system(&inspection.modal_state)
        .ok_or(ArbiterError::ActiveWorkCoordinateSystemUnavailable)?;
    ensure_stable_idle(&controller.snapshot())?;

    for (axis, distance) in [
        (millo_domain::JogAxis::X, position.x.abs()),
        (millo_domain::JogAxis::Y, position.y.abs()),
        (
            millo_domain::JogAxis::Z,
            (request.clearance_z_mm - position.z)
                .max(0.0)
                .max(position.z.abs()),
        ),
    ] {
        let maximum = axis_travel_limit(hardware_profile, axis);
        if distance > maximum {
            return Err(ArbiterError::JogPadDistanceExceedsProfile {
                axis,
                requested: distance,
                maximum,
            });
        }
    }
    for (axis, feed) in [
        (millo_domain::JogAxis::X, request.xy_feed_mm_per_min),
        (millo_domain::JogAxis::Y, request.xy_feed_mm_per_min),
        (millo_domain::JogAxis::Z, request.z_feed_mm_per_min),
    ] {
        if let Some(maximum) = axis_max_rate(&inspection, axis)
            && feed > maximum
        {
            return Err(ArbiterError::JogPadFeedExceedsAxisRate {
                axis,
                requested: feed,
                maximum,
            });
        }
    }

    let mut commands = Vec::with_capacity(3);
    let transit_z = position.z.max(request.clearance_z_mm);
    if (position.z - transit_z).abs() > WORK_ZERO_TOLERANCE_MM {
        let response = controller
            .move_to_work_position(None, None, Some(transit_z), request.z_feed_mm_per_min)
            .await?;
        commands.push(response.command);
        position = wait_for_work_position(
            controller,
            position.x,
            position.y,
            transit_z,
            transit_z - position.z,
            request.z_feed_mm_per_min,
        )
        .await?;
    }

    if position.x.abs() > WORK_ZERO_TOLERANCE_MM || position.y.abs() > WORK_ZERO_TOLERANCE_MM {
        let distance = position.x.hypot(position.y);
        let response = controller
            .move_to_work_position(Some(0.0), Some(0.0), None, request.xy_feed_mm_per_min)
            .await?;
        commands.push(response.command);
        position = wait_for_work_position(
            controller,
            0.0,
            0.0,
            transit_z,
            distance,
            request.xy_feed_mm_per_min,
        )
        .await?;
    }

    if position.z.abs() > WORK_ZERO_TOLERANCE_MM {
        let distance = position.z.abs();
        let response = controller
            .move_to_work_position(None, None, Some(0.0), request.z_feed_mm_per_min)
            .await?;
        commands.push(response.command);
        let _ = wait_for_work_position(
            controller,
            0.0,
            0.0,
            0.0,
            distance,
            request.z_feed_mm_per_min,
        )
        .await?;
    }

    let snapshot = controller.snapshot();
    let verified = verified_heightmap_work_position(&snapshot)?;
    if verified.x.abs() > HEIGHTMAP_POSITION_TOLERANCE_MM
        || verified.y.abs() > HEIGHTMAP_POSITION_TOLERANCE_MM
        || verified.z.abs() > HEIGHTMAP_POSITION_TOLERANCE_MM
    {
        return Err(ArbiterError::WorkZeroVerification(format!(
            "safe return ended at X{:.3} Y{:.3} Z{:.3}, expected work origin",
            verified.x, verified.y, verified.z
        )));
    }
    Ok(ReturnToWorkOriginOutcome {
        coordinate_system,
        commands,
        snapshot,
    })
}

pub(super) async fn wait_for_work_position(
    controller: &mut Controller<BoxedTransport>,
    expected_x: f64,
    expected_y: f64,
    expected_z: f64,
    distance_mm: f64,
    feed_mm_per_min: f64,
) -> Result<Position, ArbiterError> {
    let timeout = bounded_motion_timeout(distance_mm, feed_mm_per_min);
    let started = Instant::now();
    loop {
        let snapshot = controller.refresh_status().await?;
        match snapshot.machine.mode {
            MachineMode::Idle => {
                let actual = verified_heightmap_work_position(&snapshot)?;
                verify_heightmap_axis("X", expected_x, actual.x)?;
                verify_heightmap_axis("Y", expected_y, actual.y)?;
                verify_heightmap_axis("Z", expected_z, actual.z)?;
                return Ok(actual);
            }
            MachineMode::Jog | MachineMode::Run if started.elapsed() < timeout => {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            MachineMode::Jog | MachineMode::Run => {
                return Err(ArbiterError::ZProbeRetractTimeout(
                    timeout.as_millis().try_into().unwrap_or(u64::MAX),
                ));
            }
            _ => ensure_stable_idle(&snapshot)?,
        }
    }
}

pub(super) async fn query_parameters(
    controller: &mut Controller<BoxedTransport>,
) -> Result<DeviceInspection, ArbiterError> {
    let response = controller
        .query_device(millo_controller::DeviceQuery::Parameters)
        .await?;
    Ok(build_device_inspection(vec![response]))
}

pub(super) async fn execute_select_work_coordinate_system(
    controller: &mut Controller<BoxedTransport>,
    coordinate_system: WorkCoordinateSystem,
) -> Result<WorkCoordinateSelectionOutcome, ArbiterError> {
    ensure_stable_idle(&controller.refresh_status().await?)?;
    let response = controller
        .select_work_coordinate_system(coordinate_system)
        .await?;
    let modal = controller
        .query_device(millo_controller::DeviceQuery::ModalState)
        .await?;
    let inspection = build_device_inspection(vec![modal]);
    let actual = active_work_coordinate_system(&inspection.modal_state);
    if actual != Some(coordinate_system) {
        return Err(ArbiterError::WorkCoordinateSelectionVerification {
            expected: coordinate_system,
            actual,
        });
    }
    let snapshot = controller.refresh_status().await?;
    ensure_stable_idle(&snapshot)?;
    Ok(WorkCoordinateSelectionOutcome {
        coordinate_system,
        command: response.command,
        snapshot,
    })
}

pub(super) fn work_axis_to_jog_axis(axis: WorkAxis) -> millo_domain::JogAxis {
    match axis {
        WorkAxis::X => millo_domain::JogAxis::X,
        WorkAxis::Y => millo_domain::JogAxis::Y,
        WorkAxis::Z => millo_domain::JogAxis::Z,
        WorkAxis::A => millo_domain::JogAxis::A,
    }
}

pub(super) fn work_axis_value(position: Position, axis: WorkAxis) -> f64 {
    match axis {
        WorkAxis::X => position.x,
        WorkAxis::Y => position.y,
        WorkAxis::Z => position.z,
        WorkAxis::A => position.a.unwrap_or(f64::NAN),
    }
}

pub(super) fn verified_work_axis(
    snapshot: &ControllerSnapshot,
    parameters: &DeviceInspection,
    coordinate_system: WorkCoordinateSystem,
    axis: WorkAxis,
) -> Result<f64, ArbiterError> {
    if axis == WorkAxis::A {
        return verified_rotary_work_axis(snapshot, parameters, coordinate_system);
    }
    if let Some(work_position) = snapshot.machine.work_position {
        return Ok(position_axis(work_position, axis));
    }
    if let (Some(machine_position), Some(offset)) = (
        snapshot.machine.machine_position,
        snapshot.machine.work_coordinate_offset,
    ) {
        return Ok(position_axis(machine_position, axis) - position_axis(offset, axis));
    }

    let machine_position = snapshot.machine.machine_position.ok_or_else(|| {
        ArbiterError::WorkZeroVerification(
            "status did not return WPos or enough data to derive it".to_owned(),
        )
    })?;
    let parameter_name = work_coordinate_parameter(coordinate_system);
    let wcs = parameters
        .parameters
        .get(parameter_name)
        .and_then(|value| parse_xyz_parameter(value))
        .ok_or_else(|| {
            ArbiterError::WorkZeroVerification(format!(
                "$# did not return a valid {parameter_name} position"
            ))
        })?;
    let g92 = parameters
        .parameters
        .get("G92")
        .and_then(|value| parse_xyz_parameter(value))
        .unwrap_or([0.0; 3]);
    let tool_length = parameters
        .parameters
        .get("TLO")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    let axis_index = work_axis_index(axis);
    let tool_offset = if axis == WorkAxis::Z {
        tool_length
    } else {
        0.0
    };
    Ok(position_axis(machine_position, axis) - wcs[axis_index] - g92[axis_index] - tool_offset)
}

pub(super) fn verified_heightmap_work_position(
    snapshot: &ControllerSnapshot,
) -> Result<Position, ArbiterError> {
    snapshot
        .machine
        .work_position
        .or_else(|| {
            snapshot
                .machine
                .machine_position
                .zip(snapshot.machine.work_coordinate_offset)
                .map(|(machine, offset)| Position {
                    x: machine.x - offset.x,
                    y: machine.y - offset.y,
                    z: machine.z - offset.z,
                    a: None,
                })
        })
        .ok_or(ArbiterError::WorkPositionUnavailable)
}

pub(super) fn effective_work_coordinate_offset(snapshot: &ControllerSnapshot) -> Option<Position> {
    snapshot.machine.work_coordinate_offset.or_else(|| {
        snapshot
            .machine
            .machine_position
            .zip(snapshot.machine.work_position)
            .map(|(machine, work)| Position {
                x: machine.x - work.x,
                y: machine.y - work.y,
                z: machine.z - work.z,
                a: None,
            })
    })
}

pub(super) fn effective_work_coordinate_offset_with_parameters(
    snapshot: &ControllerSnapshot,
    parameters: &DeviceInspection,
    coordinate_system: WorkCoordinateSystem,
) -> Result<Position, ArbiterError> {
    if let Some(offset) = effective_work_coordinate_offset(snapshot) {
        return Ok(offset);
    }
    let parameter_name = work_coordinate_parameter(coordinate_system);
    let wcs = parameters
        .parameters
        .get(parameter_name)
        .and_then(|value| parse_xyz_parameter(value))
        .ok_or_else(|| {
            ArbiterError::WorkZeroVerification(format!(
                "$# did not return a valid {parameter_name} offset"
            ))
        })?;
    let g92 = parameters
        .parameters
        .get("G92")
        .and_then(|value| parse_xyz_parameter(value))
        .unwrap_or([0.0; 3]);
    let tlo = parameters
        .parameters
        .get("TLO")
        .and_then(|value| value.parse::<f64>().ok())
        .unwrap_or(0.0);
    Ok(Position {
        x: wcs[0] + g92[0],
        y: wcs[1] + g92[1],
        z: wcs[2] + g92[2] + tlo,
        a: None,
    })
}

pub(super) fn positions_within(left: Position, right: Position, tolerance_mm: f64) -> bool {
    (left.x - right.x).abs() <= tolerance_mm
        && (left.y - right.y).abs() <= tolerance_mm
        && (left.z - right.z).abs() <= tolerance_mm
}

pub(super) fn verified_z_datum_from_snapshot(
    coordinate_system: WorkCoordinateSystem,
    snapshot: &ControllerSnapshot,
) -> Option<VerifiedZDatum> {
    effective_work_coordinate_offset(snapshot).map(|work_coordinate_offset| VerifiedZDatum {
        binding: HeightmapCoordinateBinding {
            coordinate_system,
            work_coordinate_offset,
        },
        reset_count: snapshot.reset_count,
        reconnect_count: snapshot.reconnect_count,
    })
}

pub(super) fn verify_heightmap_axis(
    axis: &'static str,
    expected: f64,
    actual: f64,
) -> Result<(), ArbiterError> {
    if (actual - expected).abs() <= HEIGHTMAP_POSITION_TOLERANCE_MM {
        Ok(())
    } else {
        Err(ArbiterError::HeightmapPositionVerification {
            axis,
            expected,
            actual,
        })
    }
}

pub(super) fn parse_xyz_parameter(value: &str) -> Option<[f64; 3]> {
    let values = value
        .split(',')
        .take(3)
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() == 3).then(|| [values[0], values[1], values[2]])
}

pub(super) fn position_axis(position: Position, axis: WorkAxis) -> f64 {
    match axis {
        WorkAxis::X => position.x,
        WorkAxis::Y => position.y,
        WorkAxis::Z => position.z,
        WorkAxis::A => position.a.unwrap_or(f64::NAN),
    }
}

fn verify_zero_epoch(
    before: &ControllerSnapshot,
    after: &ControllerSnapshot,
) -> Result<(), ArbiterError> {
    if before.reset_count != after.reset_count || before.reconnect_count != after.reconnect_count {
        return Err(ArbiterError::WorkZeroVerification(
            "Controller reset or reconnected during rotary zero verification.".to_owned(),
        ));
    }
    Ok(())
}

fn parse_xyza_parameter(value: &str) -> Option<[f64; 4]> {
    let values = value
        .split(',')
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if values.len() != 4 || !values.iter().all(|value| value.is_finite()) {
        return None;
    }
    Some([values[0], values[1], values[2], values[3]])
}

fn verified_rotary_work_axis(
    snapshot: &ControllerSnapshot,
    parameters: &DeviceInspection,
    coordinate_system: WorkCoordinateSystem,
) -> Result<f64, ArbiterError> {
    let invalid = || {
        ArbiterError::WorkZeroVerification("A requires finite, consistent four-coordinate status and $# offsets; missing A is not zero.".to_owned())
    };
    let offset = |name: &str| {
        parameters
            .parameters
            .get(name)
            .and_then(|value| parse_xyza_parameter(value))
            .ok_or_else(invalid)
    };
    let wcs = offset(work_coordinate_parameter(coordinate_system))?;
    let g92 = offset("G92")?;
    let machine_a = snapshot
        .machine
        .machine_position
        .and_then(|position| position.a)
        .filter(|a| a.is_finite())
        .ok_or_else(invalid)?;
    let total_offset = wcs[3] + g92[3];
    let derived = machine_a - total_offset;
    if !derived.is_finite() || !total_offset.is_finite() {
        return Err(invalid());
    }
    for (position, expected) in [
        (snapshot.machine.work_position, derived),
        (snapshot.machine.work_coordinate_offset, total_offset),
    ] {
        let a = position
            .and_then(|position| position.a)
            .filter(|a| a.is_finite())
            .ok_or_else(invalid)?;
        if (a - expected).abs() > WORK_ZERO_TOLERANCE_DEGREES {
            return Err(invalid());
        }
    }
    Ok(derived)
}

#[cfg(test)]
mod rotary_zero_tests {
    use super::*;
    use millo_mock::MockTransport;

    fn profile() -> HardwareProfile {
        let mut profile = HardwareProfile::first_machine();
        profile.axes.push("A".to_owned());
        profile.rotary_axis = Some(millo_domain::RotaryAxisProfile {
            travel_degrees: 720.0,
            max_jog_degrees: 30.0,
            max_feed_degrees_per_min: 720.0,
        });
        profile
    }

    fn config() -> ControllerConfig {
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            status_timeout: Duration::from_millis(100),
            command_timeout: Duration::from_millis(200),
            failures_before_recovery: 2,
        }
    }

    #[tokio::test]
    async fn zero_a_is_verified_in_active_wcs_without_moving_or_zeroing_xyz() {
        let transport = MockTransport::rotary();
        let control = transport.control();
        control.set_status("<Idle|MPos:10,20,30,90|WPos:10,20,30,90|FS:0,0>");
        control.set_active_wcs(55);
        let (arbiter, worker) = CommandArbiter::new(Box::new(transport), config(), profile());
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        let outcome = arbiter
            .set_work_zero(WorkZeroRequest {
                axis: WorkAxis::A,
                position_confirmed: true,
            })
            .await
            .unwrap();
        assert_eq!(outcome.command, "G10 L20 P2 A0");
        assert_eq!(outcome.parameter_value, "0.000,0.000,0.000,90.000");
        assert_eq!(outcome.work_position, 0.0);
        assert_eq!(
            outcome.snapshot.machine.machine_position,
            Some(Position {
                x: 10.0,
                y: 20.0,
                z: 30.0,
                a: Some(90.0)
            })
        );
        let writes = control.writes();
        let changes = writes
            .iter()
            .filter(|write| write.starts_with(b"G") || write.starts_with(b"$J="))
            .collect::<Vec<_>>();
        assert_eq!(changes, vec![&b"G10 L20 P2 A0\n".to_vec()]);
        task.abort();
    }

    #[tokio::test]
    async fn zero_a_rejects_disabled_profile_and_stock_xyz_before_offset_write() {
        for (transport, hardware) in [
            (MockTransport::rotary(), HardwareProfile::first_machine()),
            (MockTransport::default(), profile()),
        ] {
            let control = transport.control();
            let (arbiter, worker) = CommandArbiter::new(Box::new(transport), config(), hardware);
            let task = tokio::spawn(worker);
            arbiter.connect().await.unwrap();
            assert!(matches!(
                arbiter
                    .set_work_zero(WorkZeroRequest {
                        axis: WorkAxis::A,
                        position_confirmed: true
                    })
                    .await,
                Err(ArbiterError::RotaryProgramUnavailable(_))
            ));
            assert!(
                !control
                    .writes()
                    .iter()
                    .any(|write| write.starts_with(b"G10") || write.starts_with(b"$J="))
            );
            task.abort();
        }
    }

    #[tokio::test]
    async fn zero_a_confirmation_and_return_clearance_fail_before_io() {
        let transport = MockTransport::rotary();
        let control = transport.control();
        control.set_status("<Idle|MPos:0,0,10,90|WPos:0,0,10,90|FS:0,0>");
        let (arbiter, worker) = CommandArbiter::new(Box::new(transport), config(), profile());
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        assert!(matches!(
            arbiter
                .set_work_zero(WorkZeroRequest {
                    axis: WorkAxis::A,
                    position_confirmed: false
                })
                .await,
            Err(ArbiterError::WorkZeroConfirmationRequired)
        ));
        assert!(matches!(
            arbiter
                .return_to_work_zero(ReturnToWorkZeroRequest {
                    axis: WorkAxis::A,
                    feed_mm_per_min: 360.0
                })
                .await,
            Err(ArbiterError::Controller(ControllerError::JogValidation(
                millo_grbl::JogValidationError::RotaryClearanceRequired
            )))
        ));
        assert!(control.writes().is_empty());
        task.abort();
    }

    #[tokio::test]
    async fn xyz_zero_requests_do_not_change_a_and_missing_post_reset_a_is_rejected() {
        let transport = MockTransport::rotary();
        let control = transport.control();
        control.set_status("<Idle|MPos:10,20,30,90|WPos:10,20,30,90|FS:0,0>");
        let (arbiter, worker) = CommandArbiter::new(Box::new(transport), config(), profile());
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        for axis in [WorkAxis::X, WorkAxis::Y, WorkAxis::Z] {
            let outcome = arbiter
                .set_work_zero(WorkZeroRequest {
                    axis,
                    position_confirmed: true,
                })
                .await
                .unwrap();
            assert_eq!(
                outcome.snapshot.machine.work_position.unwrap().a,
                Some(90.0)
            );
        }
        assert!(
            !control
                .writes()
                .iter()
                .any(|write| write.starts_with(b"G10") && write.contains(&b'A'))
        );
        task.abort();
        let mut before = ControllerSnapshot::default();
        let mut after = before.clone();
        after.reset_count += 1;
        assert!(verify_zero_epoch(&before, &after).is_err());
        before.reconnect_count += 1;
        assert!(verify_zero_epoch(&before, &after).is_err());
        assert!(
            verified_rotary_work_axis(
                &after,
                &DeviceInspection::default(),
                WorkCoordinateSystem::G54
            )
            .is_err()
        );
    }

    fn verified_fixture() -> (ControllerSnapshot, DeviceInspection) {
        let mut snapshot = ControllerSnapshot::default();
        snapshot.machine.machine_position = Some(Position {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            a: Some(90.0),
        });
        snapshot.machine.work_position = Some(Position {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            a: Some(0.0),
        });
        snapshot.machine.work_coordinate_offset = snapshot.machine.machine_position;
        let mut inspection = DeviceInspection::default();
        inspection
            .parameters
            .insert("G54".to_owned(), "0,0,0,90".to_owned());
        inspection
            .parameters
            .insert("G92".to_owned(), "0,0,0,0".to_owned());
        inspection.responses.push(CommandResponse {
            command: "$I".to_owned(),
            completion: CommandCompletion::Ok,
            lines: vec!["[AXS:4:XYZA]".to_owned(), "[FIRMWARE:grblHAL]".to_owned()],
            code: None,
        });
        (snapshot, inspection)
    }

    #[test]
    fn zero_a_rejects_missing_non_finite_or_inconsistent_parameter_evidence() {
        let (snapshot, inspection) = verified_fixture();
        assert_eq!(
            verified_rotary_work_axis(&snapshot, &inspection, WorkCoordinateSystem::G54).unwrap(),
            0.0
        );
        for malformed in [
            "0,0,0",
            "0,0,0,NaN",
            "0,0,0,inf",
            "0,0,0,90,0",
            "0,0,0,0",
            "NaN,0,0,90",
        ] {
            let mut invalid = inspection.clone();
            invalid
                .parameters
                .insert("G54".to_owned(), malformed.to_owned());
            assert!(
                verified_rotary_work_axis(&snapshot, &invalid, WorkCoordinateSystem::G54).is_err(),
                "{malformed}"
            );
        }
        let mut missing = inspection.clone();
        missing.parameters.remove("G92");
        assert!(verified_rotary_work_axis(&snapshot, &missing, WorkCoordinateSystem::G54).is_err());
        let mut missing = snapshot;
        missing.machine.work_position.as_mut().unwrap().a = None;
        assert!(
            verified_rotary_work_axis(&missing, &inspection, WorkCoordinateSystem::G54).is_err()
        );
    }

    #[test]
    fn angular_capability_requires_external_grblhal_bit_one_and_exact_identity() {
        let (snapshot, mut inspection) = verified_fixture();
        for mask in ["0", "2", "8", "NaN", "1.5", "-1"] {
            inspection
                .settings
                .insert("$376".to_owned(), mask.to_owned());
            assert!(
                super::super::rotary_program::validate_rotary_capability(
                    &profile(),
                    &inspection,
                    &snapshot
                )
                .is_err(),
                "{mask}"
            );
        }
        inspection
            .settings
            .insert("$376".to_owned(), "1".to_owned());
        assert!(
            super::super::rotary_program::validate_rotary_capability(
                &profile(),
                &inspection,
                &snapshot
            )
            .is_ok()
        );
        inspection.responses[0].lines[1] = "[VER:1.1h:User named grblHAL FluidNC]".to_owned();
        assert!(
            super::super::rotary_program::validate_rotary_capability(
                &profile(),
                &inspection,
                &snapshot
            )
            .is_err()
        );
        inspection.responses[0].lines[1] = "[VER:3.9 FluidNC build:machine]".to_owned();
        assert!(
            super::super::rotary_program::validate_rotary_capability(
                &profile(),
                &inspection,
                &snapshot
            )
            .is_ok()
        );
        let mut invalid = profile();
        invalid.rotary_axis.as_mut().unwrap().travel_degrees = f64::NAN;
        assert!(
            super::super::rotary_program::validate_rotary_capability(
                &invalid,
                &inspection,
                &snapshot
            )
            .is_err()
        );
    }
}
