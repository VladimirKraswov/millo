use super::*;

pub(super) async fn execute_set_work_zero(
    controller: &mut Controller<BoxedTransport>,
    request: WorkZeroRequest,
) -> Result<WorkZeroOutcome, ArbiterError> {
    if !request.position_confirmed {
        return Err(ArbiterError::WorkZeroConfirmationRequired);
    }

    controller.refresh_status().await?;
    ensure_stable_idle(&controller.snapshot())?;

    let modal_response = controller
        .query_device(millo_controller::DeviceQuery::ModalState)
        .await?;
    let modal = build_device_inspection(vec![modal_response]);
    let coordinate_system = active_work_coordinate_system(&modal.modal_state)
        .ok_or(ArbiterError::ActiveWorkCoordinateSystemUnavailable)?;

    ensure_stable_idle(&controller.snapshot())?;
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
    parse_xyz_parameter(&parameter_value).ok_or_else(|| {
        ArbiterError::WorkZeroVerification(format!(
            "$# returned malformed {parameter_name}: {parameter_value}"
        ))
    })?;

    let snapshot = controller.refresh_status().await?;
    ensure_stable_idle(&snapshot)?;
    let work_position =
        verified_work_axis(&snapshot, &parameters, coordinate_system, request.axis)?;
    if work_position.abs() > WORK_ZERO_TOLERANCE_MM {
        return Err(ArbiterError::WorkZeroVerification(format!(
            "expected {:?}=0 in {parameter_name}, read {work_position:.3} mm",
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
    }
}

pub(super) fn work_axis_value(position: Position, axis: WorkAxis) -> f64 {
    match axis {
        WorkAxis::X => position.x,
        WorkAxis::Y => position.y,
        WorkAxis::Z => position.z,
    }
}

pub(super) fn verified_work_axis(
    snapshot: &ControllerSnapshot,
    parameters: &DeviceInspection,
    coordinate_system: WorkCoordinateSystem,
    axis: WorkAxis,
) -> Result<f64, ArbiterError> {
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
    }
}
