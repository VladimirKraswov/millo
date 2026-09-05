use super::*;

pub(super) async fn begin_heightmap(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    request: HeightmapStartRequest,
    operation_sequence: u64,
    verified_z_datum: Option<VerifiedZDatum>,
) -> Result<ActiveHeightmap, ArbiterError> {
    if !request.setup_confirmed {
        return Err(ArbiterError::HeightmapConfirmationRequired);
    }
    if !hardware_profile.probe_installed {
        return Err(ArbiterError::ZProbeNotInstalled);
    }
    if hardware_profile.probe_mode != ProbeWorkflowMode::Heightmap {
        return Err(ArbiterError::HeightmapModeDisabled);
    }
    if !request.contact_available_at_every_point {
        return Err(ArbiterError::HeightmapContactUnavailable);
    }
    let travel = hardware_profile
        .travel_mm
        .map(|travel| HeightmapTravelLimits {
            x_mm: travel.x,
            y_mm: travel.y,
        });
    let safe_work_z = heightmap_safe_work_z(request.plan);
    if hardware_profile
        .travel_mm
        .is_some_and(|travel| safe_work_z > travel.z)
    {
        return Err(HeightmapError::ExceedsTravel {
            axis: "Z",
            requested: safe_work_z,
            maximum: hardware_profile.travel_mm.map_or(0.0, |travel| travel.z),
        }
        .into());
    }
    let mut map = Heightmap::new(plan_heightmap(request.plan, travel)?);
    let before = controller.refresh_status().await?;
    ensure_probe_start_idle(&before)?;
    if before.machine.pins.as_ref().is_some_and(|pins| pins.probe) {
        return Err(ArbiterError::ZProbeInputAlreadyActive);
    }
    let modal_response = controller
        .query_device(millo_controller::DeviceQuery::ModalState)
        .await?;
    let modal = build_device_inspection(vec![modal_response]);
    let coordinate_system = active_work_coordinate_system(&modal.modal_state)
        .ok_or(ArbiterError::ActiveWorkCoordinateSystemUnavailable)?;
    let parameters = query_parameters(controller).await?;
    let current_offset =
        effective_work_coordinate_offset_with_parameters(&before, &parameters, coordinate_system)?;
    map.bind_coordinates(coordinate_system, current_offset);
    let reuse_verified_z_zero = verified_z_datum.is_some_and(|datum| {
        datum.reset_count == before.reset_count
            && datum.reconnect_count == before.reconnect_count
            && datum.binding.coordinate_system == coordinate_system
            && positions_within(datum.binding.work_coordinate_offset, current_offset, 0.01)
    });
    let current_work_x = verified_work_axis(&before, &parameters, coordinate_system, WorkAxis::X)?;
    let current_work_y = verified_work_axis(&before, &parameters, coordinate_system, WorkAxis::Y)?;
    let current_work_z = verified_work_axis(&before, &parameters, coordinate_system, WorkAxis::Z)?;
    let last_work_xy = Some((current_work_x, current_work_y));
    Ok(ActiveHeightmap {
        map,
        coordinate_system,
        restore_modal: restore_probe_modal_command(&modal.modal_state),
        next_sequence: 0,
        phase: HeightmapPhase::Raise,
        paused: false,
        operation_sequence,
        start_work_xy: last_work_xy,
        last_work_xy,
        last_work_z: Some(current_work_z),
        highest_measured_surface_z: 0.0,
        establish_z_zero_on_first_contact: !reuse_verified_z_zero,
    })
}

pub(super) async fn begin_resumed_heightmap(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    mut map: Heightmap,
    request: HeightmapResumeRequest,
    operation_sequence: u64,
) -> Result<ActiveHeightmap, ArbiterError> {
    if !request.setup_confirmed {
        return Err(ArbiterError::HeightmapConfirmationRequired);
    }
    if !hardware_profile.probe_installed {
        return Err(ArbiterError::ZProbeNotInstalled);
    }
    if hardware_profile.probe_mode != ProbeWorkflowMode::Heightmap {
        return Err(ArbiterError::HeightmapModeDisabled);
    }
    if !request.contact_available_at_every_point {
        return Err(ArbiterError::HeightmapContactUnavailable);
    }
    if !request.max_probe_depth_mm.is_finite()
        || !(0.1..=100.0).contains(&request.max_probe_depth_mm)
    {
        return Err(HeightmapError::InvalidSetting("maximum probe depth").into());
    }
    let next_sequence = map
        .samples
        .iter()
        .position(Option::is_none)
        .ok_or(HeightmapError::Incomplete)?;
    if map.samples[next_sequence + 1..].iter().any(Option::is_some) {
        return Err(ArbiterError::HeightmapProbe(
            "saved heightmap samples are not a contiguous prefix".to_owned(),
        ));
    }
    if map.samples[..next_sequence]
        .iter()
        .any(|sample| sample.as_ref().is_none_or(|sample| !sample.triggered))
    {
        return Err(ArbiterError::HeightmapProbe(
            "saved heightmap contains an unverified contact".to_owned(),
        ));
    }
    map.plan.request.max_probe_depth_mm = request.max_probe_depth_mm;
    let travel = hardware_profile
        .travel_mm
        .map(|travel| HeightmapTravelLimits {
            x_mm: travel.x,
            y_mm: travel.y,
        });
    let validated_plan = plan_heightmap(map.plan.request, travel)?;
    if validated_plan.points != map.plan.points || validated_plan.spacing != map.plan.spacing {
        return Err(ArbiterError::HeightmapProbe(
            "saved heightmap grid no longer matches its plan".to_owned(),
        ));
    }
    map.plan = validated_plan;
    let safe_work_z = heightmap_safe_work_z(map.plan.request);
    if hardware_profile
        .travel_mm
        .is_some_and(|travel| safe_work_z > travel.z)
    {
        return Err(HeightmapError::ExceedsTravel {
            axis: "Z",
            requested: safe_work_z,
            maximum: hardware_profile.travel_mm.map_or(0.0, |travel| travel.z),
        }
        .into());
    }
    let before = controller.refresh_status().await?;
    ensure_probe_start_idle(&before)?;
    if before.machine.pins.as_ref().is_some_and(|pins| pins.probe) {
        return Err(ArbiterError::ZProbeInputAlreadyActive);
    }
    let modal_response = controller
        .query_device(millo_controller::DeviceQuery::ModalState)
        .await?;
    let modal = build_device_inspection(vec![modal_response]);
    let coordinate_system = active_work_coordinate_system(&modal.modal_state)
        .ok_or(ArbiterError::ActiveWorkCoordinateSystemUnavailable)?;
    let binding = map.coordinate_binding.ok_or_else(|| {
        ArbiterError::HeightmapProbe(
            "saved heightmap has no work-coordinate binding; start a new measurement".to_owned(),
        )
    })?;
    let parameters = query_parameters(controller).await?;
    let current_offset =
        effective_work_coordinate_offset_with_parameters(&before, &parameters, coordinate_system)?;
    if binding.coordinate_system != coordinate_system
        || !positions_within(binding.work_coordinate_offset, current_offset, 0.01)
    {
        return Err(ArbiterError::HeightmapProbe(
            "work zero changed after this heightmap draft was measured; start a new measurement"
                .to_owned(),
        ));
    }
    let current_work_x = verified_work_axis(&before, &parameters, coordinate_system, WorkAxis::X)?;
    let current_work_y = verified_work_axis(&before, &parameters, coordinate_system, WorkAxis::Y)?;
    let current_work_z = verified_work_axis(&before, &parameters, coordinate_system, WorkAxis::Z)?;
    let highest_measured_surface_z = map
        .samples
        .iter()
        .filter_map(Option::as_ref)
        .map(|sample| sample.z_mm)
        .fold(0.0_f64, f64::max);
    let last_work_xy = Some((current_work_x, current_work_y));
    Ok(ActiveHeightmap {
        map,
        coordinate_system,
        restore_modal: restore_probe_modal_command(&modal.modal_state),
        next_sequence,
        phase: HeightmapPhase::Raise,
        paused: false,
        operation_sequence,
        start_work_xy: last_work_xy,
        last_work_xy,
        last_work_z: Some(current_work_z),
        highest_measured_surface_z,
        establish_z_zero_on_first_contact: false,
    })
}

pub(super) fn heightmap_operation_snapshot(active: &ActiveHeightmap) -> HeightmapOperationSnapshot {
    let mut snapshot =
        HeightmapOperationSnapshot::running(active.operation_sequence, active.map.clone());
    snapshot.state = if active.paused {
        HeightmapOperationState::Paused
    } else {
        HeightmapOperationState::Running
    };
    snapshot.current_sequence =
        (active.next_sequence < active.map.plan.points.len()).then_some(active.next_sequence);
    snapshot
}

pub(super) fn cancel_active_heightmap(
    active: &mut Option<ActiveHeightmap>,
    snapshots: &watch::Sender<HeightmapOperationSnapshot>,
    reason: &str,
) {
    if let Some(active) = active.take() {
        let mut snapshot = heightmap_operation_snapshot(&active);
        snapshot.state = HeightmapOperationState::Cancelled;
        snapshot.current_sequence = None;
        snapshot.error = Some(reason.to_owned());
        let _ = snapshots.send(snapshot);
    }
}

pub(super) async fn poll_active_heightmap(actor: &mut ActorState) {
    if actor
        .active_heightmap
        .as_ref()
        .is_none_or(|active| active.paused)
    {
        return;
    }
    if let Err(error) = execute_heightmap_phase(actor).await {
        if let Some(mut active) = actor.active_heightmap.take() {
            let expected_miss = matches!(&error, ArbiterError::ZProbeContactNotFound);
            let stop_error = if expected_miss {
                match recover_heightmap_probe_miss(&mut actor.controller, &mut active).await {
                    Ok(()) => None,
                    Err(recovery) => {
                        let quarantine = quarantine_failed_heightmap(&mut actor.controller).await;
                        Some(match quarantine {
                            Some(stop) => {
                                format!("{recovery}; emergency stop delivery also failed: {stop}")
                            }
                            None => recovery.to_string(),
                        })
                    }
                }
            } else {
                quarantine_failed_heightmap(&mut actor.controller).await
            };
            let mut snapshot = heightmap_operation_snapshot(&active);
            snapshot.state = HeightmapOperationState::Failed;
            snapshot.current_sequence = Some(active.next_sequence);
            snapshot.error = Some(match (expected_miss, stop_error) {
                (true, Some(stop)) => format!(
                    "{error}; automatic recovery to safe Z failed and motion was disabled: {stop}"
                ),
                (false, Some(stop)) => format!(
                    "{error}; automatic motion was disabled, but emergency stop delivery failed: {stop}"
                ),
                (_, None) => error.to_string(),
            });
            let _ = actor.heightmap_snapshots.send(snapshot);
        }
        publish(&actor.snapshots, &actor.controller);
    }
}

pub(super) async fn recover_heightmap_probe_miss(
    controller: &mut Controller<BoxedTransport>,
    active: &mut ActiveHeightmap,
) -> Result<(), ArbiterError> {
    let snapshot = controller.refresh_status().await?;
    ensure_stable_idle(&snapshot)?;
    let current_z = verified_heightmap_work_position(&snapshot)?.z;
    let request = active.map.plan.request;
    let target_z = heightmap_transit_work_z(request, active.highest_measured_surface_z);
    let distance = target_z - current_z;
    if distance > WORK_ZERO_TOLERANCE_MM {
        controller
            .move_heightmap_z(distance, request.retract_feed_mm_per_min)
            .await?;
        let timeout = bounded_motion_timeout(distance, request.retract_feed_mm_per_min);
        let started = Instant::now();
        loop {
            let snapshot = controller.refresh_status().await?;
            match snapshot.machine.mode {
                MachineMode::Idle => {
                    let actual_z = verified_heightmap_work_position(&snapshot)?.z;
                    verify_heightmap_axis("Z", target_z, actual_z)?;
                    active.last_work_z = Some(actual_z);
                    break;
                }
                MachineMode::Jog | MachineMode::Run if started.elapsed() < timeout => {}
                MachineMode::Jog | MachineMode::Run => {
                    return Err(ArbiterError::ZProbeRetractTimeout(
                        timeout.as_millis().try_into().unwrap_or(u64::MAX),
                    ));
                }
                _ => ensure_stable_idle(&snapshot)?,
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    controller
        .restore_modal_state(&active.restore_modal)
        .await?;
    Ok(())
}

pub(super) async fn quarantine_failed_heightmap(
    controller: &mut Controller<BoxedTransport>,
) -> Option<String> {
    if controller.snapshot().connection != ConnectionState::Connected {
        return Some(format!(
            "controller link is {:?}; Stop could not be delivered",
            controller.snapshot().connection
        ));
    }
    controller
        .abort_program_stream()
        .await
        .err()
        .map(|error| error.to_string())
}

pub(super) fn heightmap_safe_work_z(request: millo_heightmap::HeightmapPlanRequest) -> f64 {
    request.contact_offset_mm + request.clearance_z_mm
}

pub(super) fn heightmap_transit_work_z(
    request: millo_heightmap::HeightmapPlanRequest,
    highest_measured_surface_z: f64,
) -> f64 {
    highest_measured_surface_z.max(0.0) + request.contact_offset_mm + request.clearance_z_mm
}

pub(super) async fn execute_heightmap_phase(actor: &mut ActorState) -> Result<(), ArbiterError> {
    let phase = actor
        .active_heightmap
        .as_ref()
        .map(|active| match &active.phase {
            HeightmapPhase::Raise => HeightmapPhase::Raise,
            HeightmapPhase::WaitForRaise {
                started,
                timeout,
                target_z,
            } => HeightmapPhase::WaitForRaise {
                started: *started,
                timeout: *timeout,
                target_z: *target_z,
            },
            HeightmapPhase::MoveXy => HeightmapPhase::MoveXy,
            HeightmapPhase::WaitForXy {
                started,
                timeout,
                target_x,
                target_y,
            } => HeightmapPhase::WaitForXy {
                started: *started,
                timeout: *timeout,
                target_x: *target_x,
                target_y: *target_y,
            },
            HeightmapPhase::Probe => HeightmapPhase::Probe,
            HeightmapPhase::PollProbe { command } => HeightmapPhase::PollProbe {
                command: command.clone(),
            },
            HeightmapPhase::WaitForProbeIdle { started } => {
                HeightmapPhase::WaitForProbeIdle { started: *started }
            }
            HeightmapPhase::RecordProbe => HeightmapPhase::RecordProbe,
            HeightmapPhase::ReturnToStartXy => HeightmapPhase::ReturnToStartXy,
            HeightmapPhase::WaitForReturnXy {
                started,
                timeout,
                target_x,
                target_y,
            } => HeightmapPhase::WaitForReturnXy {
                started: *started,
                timeout: *timeout,
                target_x: *target_x,
                target_y: *target_y,
            },
            HeightmapPhase::Finalize => HeightmapPhase::Finalize,
        })
        .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
    match phase {
        HeightmapPhase::Raise => {
            let active = actor
                .active_heightmap
                .as_ref()
                .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
            let request = active.map.plan.request;
            let transit_z = heightmap_transit_work_z(request, active.highest_measured_surface_z);
            let current_z = active
                .last_work_z
                .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
            let raise_distance = (transit_z - current_z).max(0.0);
            if raise_distance <= WORK_ZERO_TOLERANCE_MM {
                actor
                    .active_heightmap
                    .as_mut()
                    .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                    .phase = if active.next_sequence >= active.map.plan.points.len() {
                    HeightmapPhase::ReturnToStartXy
                } else {
                    HeightmapPhase::MoveXy
                };
            } else {
                actor
                    .controller
                    .move_heightmap_z(raise_distance, request.retract_feed_mm_per_min)
                    .await?;
                let timeout =
                    bounded_motion_timeout(raise_distance, request.retract_feed_mm_per_min);
                actor
                    .active_heightmap
                    .as_mut()
                    .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                    .phase = HeightmapPhase::WaitForRaise {
                    started: Instant::now(),
                    timeout,
                    target_z: transit_z,
                };
            }
        }
        HeightmapPhase::WaitForRaise {
            started,
            timeout,
            target_z,
        } => {
            let snapshot = actor.controller.refresh_status().await?;
            match snapshot.machine.mode {
                MachineMode::Idle => {
                    let actual_z = verified_heightmap_work_position(&snapshot)?.z;
                    verify_heightmap_axis("Z", target_z, actual_z)?;
                    let active = actor
                        .active_heightmap
                        .as_mut()
                        .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
                    active.last_work_z = Some(actual_z);
                    active.phase = if active.next_sequence >= active.map.plan.points.len() {
                        HeightmapPhase::ReturnToStartXy
                    } else {
                        HeightmapPhase::MoveXy
                    };
                }
                MachineMode::Jog | MachineMode::Run if started.elapsed() < timeout => {}
                MachineMode::Jog | MachineMode::Run => {
                    return Err(ArbiterError::ZProbeRetractTimeout(
                        timeout.as_millis().try_into().unwrap_or(u64::MAX),
                    ));
                }
                MachineMode::Hold => {
                    actor
                        .active_heightmap
                        .as_mut()
                        .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                        .paused = true
                }
                _ => ensure_stable_idle(&snapshot)?,
            }
        }
        HeightmapPhase::MoveXy => {
            let active = actor
                .active_heightmap
                .as_ref()
                .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
            let point = *active
                .map
                .plan
                .points
                .get(active.next_sequence)
                .ok_or(HeightmapError::UnknownPoint(active.next_sequence))?;
            let feed = active.map.plan.request.travel_feed_mm_per_min;
            let current = actor.controller.snapshot().machine.work_position;
            let current_xy = current
                .map(|position| (position.x, position.y))
                .or(active.last_work_xy)
                .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
            let delta_x = point.x_mm - current_xy.0;
            let delta_y = point.y_mm - current_xy.1;
            let distance = delta_x.hypot(delta_y);
            if distance <= HEIGHTMAP_POSITION_TOLERANCE_MM {
                let active = actor
                    .active_heightmap
                    .as_mut()
                    .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
                active.last_work_xy = Some((point.x_mm, point.y_mm));
                active.phase = HeightmapPhase::Probe;
            } else {
                actor
                    .controller
                    .move_heightmap_xy(delta_x, delta_y, feed)
                    .await?;
                let active = actor
                    .active_heightmap
                    .as_mut()
                    .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
                active.phase = HeightmapPhase::WaitForXy {
                    started: Instant::now(),
                    timeout: bounded_motion_timeout(distance, feed),
                    target_x: point.x_mm,
                    target_y: point.y_mm,
                };
            }
        }
        HeightmapPhase::WaitForXy {
            started,
            timeout,
            target_x,
            target_y,
        } => {
            let snapshot = actor.controller.refresh_status().await?;
            match snapshot.machine.mode {
                MachineMode::Idle => {
                    let actual = verified_heightmap_work_position(&snapshot)?;
                    verify_heightmap_axis("X", target_x, actual.x)?;
                    verify_heightmap_axis("Y", target_y, actual.y)?;
                    let active = actor
                        .active_heightmap
                        .as_mut()
                        .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
                    active.last_work_xy = Some((actual.x, actual.y));
                    active.phase = HeightmapPhase::Probe;
                }
                MachineMode::Jog | MachineMode::Run if started.elapsed() < timeout => {}
                MachineMode::Jog | MachineMode::Run => {
                    return Err(ArbiterError::ZProbeSettleTimeout {
                        timeout_ms: timeout.as_millis().try_into().unwrap_or(u64::MAX),
                        last_mode: snapshot.machine.mode,
                    });
                }
                MachineMode::Hold => {
                    actor
                        .active_heightmap
                        .as_mut()
                        .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                        .paused = true
                }
                _ => ensure_stable_idle(&snapshot)?,
            }
        }
        HeightmapPhase::Probe => {
            let request = actor
                .active_heightmap
                .as_ref()
                .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                .map
                .plan
                .request;
            let status = actor.controller.refresh_status().await?;
            ensure_stable_idle(&status)?;
            if status.machine.pins.as_ref().is_some_and(|pins| pins.probe) {
                return Err(ArbiterError::ZProbeInputAlreadyActive);
            }
            let search_travel_mm = request.clearance_z_mm + request.max_probe_depth_mm;
            let (command, _) = actor
                .controller
                .begin_z_probe(search_travel_mm, request.probe_feed_mm_per_min)
                .await?;
            actor
                .active_heightmap
                .as_mut()
                .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                .phase = HeightmapPhase::PollProbe { command };
        }
        HeightmapPhase::PollProbe { command } => {
            match actor
                .controller
                .poll_z_probe(&command, SENDER_RESPONSE_SLICE)
                .await?
            {
                ProgramResponsePoll::Pending => {}
                ProgramResponsePoll::StatusObserved => publish(&actor.snapshots, &actor.controller),
                ProgramResponsePoll::Terminal(_) => {
                    actor
                        .active_heightmap
                        .as_mut()
                        .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                        .phase = HeightmapPhase::WaitForProbeIdle {
                        started: Instant::now(),
                    }
                }
            }
        }
        HeightmapPhase::WaitForProbeIdle { started } => {
            const SETTLE_TIMEOUT: Duration = Duration::from_secs(3);

            let snapshot = actor.controller.refresh_status().await?;
            match snapshot.machine.mode {
                MachineMode::Idle => {
                    actor
                        .active_heightmap
                        .as_mut()
                        .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                        .phase = HeightmapPhase::RecordProbe;
                }
                MachineMode::Run if started.elapsed() < SETTLE_TIMEOUT => {}
                MachineMode::Run => {
                    return Err(ArbiterError::ZProbeSettleTimeout {
                        timeout_ms: SETTLE_TIMEOUT.as_millis().try_into().unwrap_or(u64::MAX),
                        last_mode: snapshot.machine.mode,
                    });
                }
                _ => ensure_stable_idle(&snapshot)?,
            }
        }
        HeightmapPhase::RecordProbe => {
            let (coordinate_system, contact_offset_mm, establishes_z_zero) = {
                let active = actor
                    .active_heightmap
                    .as_ref()
                    .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
                (
                    active.coordinate_system,
                    active.map.plan.request.contact_offset_mm,
                    active.establish_z_zero_on_first_contact
                        && active.next_sequence == 0
                        && active.map.samples.iter().all(Option::is_none),
                )
            };
            let mut parameters = query_parameters(&mut actor.controller).await?;
            let contact = parse_probe_position(&parameters)?;
            if establishes_z_zero {
                actor
                    .controller
                    .set_work_value(WorkAxis::Z, coordinate_system, contact_offset_mm)
                    .await?;
                parameters = query_parameters(&mut actor.controller).await?;
                let zero_snapshot = actor.controller.refresh_status().await?;
                ensure_stable_idle(&zero_snapshot)?;
                verify_probe_zero_snapshot(
                    &zero_snapshot,
                    &parameters,
                    coordinate_system,
                    contact_offset_mm,
                )?;
                let binding_offset = effective_work_coordinate_offset(&zero_snapshot)
                    .ok_or(ArbiterError::WorkPositionUnavailable)?;
                actor
                    .active_heightmap
                    .as_mut()
                    .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                    .map
                    .bind_coordinates(coordinate_system, binding_offset);
                actor.verified_z_datum =
                    verified_z_datum_from_snapshot(coordinate_system, &zero_snapshot);
            }
            let contact_work_z = derive_probe_work_z(&parameters, coordinate_system, contact)?;
            let surface_z = contact_work_z - contact_offset_mm;
            let active = actor
                .active_heightmap
                .as_mut()
                .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
            active
                .map
                .record_sample(active.next_sequence, surface_z, true)?;
            active.highest_measured_surface_z = active.highest_measured_surface_z.max(surface_z);
            active.last_work_z = Some(contact_work_z);
            active.next_sequence += 1;
            active.phase = HeightmapPhase::Raise;
            let _ = actor
                .heightmap_snapshots
                .send(heightmap_operation_snapshot(active));
        }
        HeightmapPhase::ReturnToStartXy => {
            let active = actor
                .active_heightmap
                .as_ref()
                .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
            let Some((target_x, target_y)) = active.start_work_xy else {
                actor
                    .active_heightmap
                    .as_mut()
                    .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                    .phase = HeightmapPhase::Finalize;
                return Ok(());
            };
            let (current_x, current_y) = active
                .last_work_xy
                .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
            let delta_x = target_x - current_x;
            let delta_y = target_y - current_y;
            let distance = delta_x.hypot(delta_y);
            if distance <= WORK_ZERO_TOLERANCE_MM {
                let active = actor
                    .active_heightmap
                    .as_mut()
                    .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
                active.last_work_xy = Some((target_x, target_y));
                active.phase = HeightmapPhase::Finalize;
            } else {
                let feed = active.map.plan.request.travel_feed_mm_per_min;
                actor
                    .controller
                    .move_heightmap_xy(delta_x, delta_y, feed)
                    .await?;
                actor
                    .active_heightmap
                    .as_mut()
                    .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                    .phase = HeightmapPhase::WaitForReturnXy {
                    started: Instant::now(),
                    timeout: bounded_motion_timeout(distance, feed),
                    target_x,
                    target_y,
                };
            }
        }
        HeightmapPhase::WaitForReturnXy {
            started,
            timeout,
            target_x,
            target_y,
        } => {
            let snapshot = actor.controller.refresh_status().await?;
            match snapshot.machine.mode {
                MachineMode::Idle => {
                    let actual = verified_heightmap_work_position(&snapshot)?;
                    verify_heightmap_axis("X", target_x, actual.x)?;
                    verify_heightmap_axis("Y", target_y, actual.y)?;
                    let active = actor
                        .active_heightmap
                        .as_mut()
                        .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
                    active.last_work_xy = Some((actual.x, actual.y));
                    active.phase = HeightmapPhase::Finalize;
                }
                MachineMode::Jog | MachineMode::Run if started.elapsed() < timeout => {}
                MachineMode::Jog | MachineMode::Run => {
                    return Err(ArbiterError::ZProbeSettleTimeout {
                        timeout_ms: timeout.as_millis().try_into().unwrap_or(u64::MAX),
                        last_mode: snapshot.machine.mode,
                    });
                }
                MachineMode::Hold => {
                    actor
                        .active_heightmap
                        .as_mut()
                        .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                        .paused = true;
                }
                _ => ensure_stable_idle(&snapshot)?,
            }
        }
        HeightmapPhase::Finalize => {
            let restore = actor
                .active_heightmap
                .as_ref()
                .ok_or(ArbiterError::HeightmapOperationUnavailable)?
                .restore_modal
                .clone();
            actor.controller.restore_modal_state(&restore).await?;
            let active = actor
                .active_heightmap
                .take()
                .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
            let mut snapshot = heightmap_operation_snapshot(&active);
            snapshot.state = HeightmapOperationState::Completed;
            snapshot.current_sequence = None;
            let _ = actor.heightmap_snapshots.send(snapshot);
        }
    }
    publish(&actor.snapshots, &actor.controller);
    Ok(())
}
