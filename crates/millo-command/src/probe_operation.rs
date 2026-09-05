use super::*;

pub(super) async fn begin_z_probe(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    request: ZProbeRequest,
) -> Result<StartedZProbe, ArbiterError> {
    if !request.setup_confirmed {
        return Err(ArbiterError::ZProbeConfirmationRequired);
    }
    if !hardware_profile.probe_installed {
        return Err(ArbiterError::ZProbeNotInstalled);
    }
    if request.settings.mode == ProbeWorkflowMode::Off
        || request.settings.mode != hardware_profile.probe_mode
    {
        return Err(ArbiterError::ZProbeDisabled);
    }
    validate_z_probe_settings(request.settings)?;

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
    let restore_modal = restore_probe_modal_command(&modal.modal_state);

    ensure_probe_start_idle(&controller.snapshot())?;
    let (command, _) = controller
        .begin_z_probe(
            request.settings.max_travel_mm,
            request.settings.probe_feed_mm_per_min,
        )
        .await?;
    Ok(StartedZProbe {
        request,
        coordinate_system,
        restore_modal,
        command,
    })
}

pub(super) async fn complete_z_probe(
    controller: &mut Controller<BoxedTransport>,
    request: ZProbeRequest,
    coordinate_system: WorkCoordinateSystem,
    restore_modal: String,
    probe_response: CommandResponse,
) -> Result<ZProbeOutcome, ArbiterError> {
    let calibration = async {
        // GRBL may emit the terminal `ok` before the next status report has
        // transitioned from Run to Idle. Never query PRB or mutate WCS while
        // the probe motion is still settling. Keep this inside the calibrated
        // section so every failure still reaches the common cleanup below.
        wait_for_probe_motion_idle(controller).await?;
        let parameters = query_parameters(controller).await?;
        let contact_machine_position = parse_probe_position(&parameters)?;
        let zero_response = controller
            .set_work_value(
                WorkAxis::Z,
                coordinate_system,
                request.settings.plate_thickness_mm,
            )
            .await?;
        let offset_parameters = query_parameters(controller).await?;
        let zero_snapshot = controller.refresh_status().await?;
        ensure_stable_idle(&zero_snapshot)?;
        verify_probe_zero_snapshot(
            &zero_snapshot,
            &offset_parameters,
            coordinate_system,
            request.settings.plate_thickness_mm,
        )?;
        Ok::<_, ArbiterError>((contact_machine_position, zero_response))
    }
    .await;

    // A successful probe may leave the cutter touching the plate. A G38.3 miss
    // ends at the bounded search limit, so return the full search travel rather
    // than only the normal post-contact clearance.
    let retract_distance = if matches!(&calibration, Err(ArbiterError::ZProbeContactNotFound)) {
        request.settings.max_travel_mm
    } else {
        request.settings.retract_mm
    };
    let restore_result = controller.restore_modal_state(&restore_modal).await;
    let retract_result = controller
        .retract_z(retract_distance, request.settings.retract_feed_mm_per_min)
        .await;
    let settle_result = if retract_result.is_ok() {
        wait_for_idle_after_probe(controller, request.settings, retract_distance).await
    } else {
        Ok(controller.snapshot())
    };
    let (contact_machine_position, zero_response) = calibration?;
    restore_result?;
    let retract_response = retract_result?;
    let snapshot = settle_result?;
    let final_work_z = snapshot
        .machine
        .work_position
        .ok_or(ArbiterError::WorkPositionUnavailable)?
        .z;
    let expected = request.settings.plate_thickness_mm + request.settings.retract_mm;
    if (final_work_z - expected).abs() > 0.01 {
        return Err(ArbiterError::ZProbeVerification(format!(
            "expected final work Z {expected:.3} mm, read {final_work_z:.3} mm"
        )));
    }

    Ok(ZProbeOutcome {
        coordinate_system,
        probe_command: probe_response.command,
        zero_command: zero_response.command,
        retract_command: retract_response.command,
        contact_machine_position,
        final_work_z,
        snapshot,
    })
}

pub(super) async fn wait_for_probe_motion_idle(
    controller: &mut Controller<BoxedTransport>,
) -> Result<ControllerSnapshot, ArbiterError> {
    const SETTLE_TIMEOUT: Duration = Duration::from_secs(3);

    let started = Instant::now();
    loop {
        let snapshot = controller.refresh_status().await?;
        match snapshot.machine.mode {
            MachineMode::Idle => return Ok(snapshot),
            MachineMode::Run if started.elapsed() < SETTLE_TIMEOUT => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            MachineMode::Run => {
                return Err(ArbiterError::ZProbeSettleTimeout {
                    timeout_ms: SETTLE_TIMEOUT.as_millis().try_into().unwrap_or(u64::MAX),
                    last_mode: snapshot.machine.mode,
                });
            }
            _ => ensure_stable_idle(&snapshot)?,
        }
    }
}

pub(super) fn finish_active_z_probe(
    active: &mut Option<ActiveZProbe>,
    result: Result<ZProbeOutcome, ArbiterError>,
) {
    if let Some(active) = active.take() {
        let _ = active.response.send(result);
    }
}

pub(super) async fn poll_active_z_probe(actor: &mut ActorState) {
    let Some(command) = actor
        .active_z_probe
        .as_ref()
        .map(|active| active.command.clone())
    else {
        return;
    };
    match actor
        .controller
        .poll_z_probe(&command, SENDER_RESPONSE_SLICE)
        .await
    {
        Ok(ProgramResponsePoll::Pending) => {}
        Ok(ProgramResponsePoll::StatusObserved) => {
            publish(&actor.snapshots, &actor.controller);
        }
        Ok(ProgramResponsePoll::Terminal(probe_response)) => {
            let Some(active) = actor.active_z_probe.take() else {
                return;
            };
            let result = complete_z_probe(
                &mut actor.controller,
                active.request,
                active.coordinate_system,
                active.restore_modal,
                probe_response,
            )
            .await;
            if let Ok(outcome) = &result {
                actor.verified_z_datum =
                    verified_z_datum_from_snapshot(outcome.coordinate_system, &outcome.snapshot);
            }
            let _ = active.response.send(result);
            publish(&actor.snapshots, &actor.controller);
        }
        Err(error) => {
            finish_active_z_probe(&mut actor.active_z_probe, Err(error.into()));
            publish(&actor.snapshots, &actor.controller);
        }
    }
}

pub(super) fn validate_z_probe_settings(settings: ZProbeSettings) -> Result<(), ArbiterError> {
    let checks = [
        (
            settings.plate_thickness_mm.is_finite()
                && (0.0..=100.0).contains(&settings.plate_thickness_mm)
                && (settings.mode != ProbeWorkflowMode::WorkZero
                    || settings.plate_thickness_mm >= 0.01),
            "plate thickness must be 0-100 mm for a heightmap or 0.01-100 mm for work zero",
        ),
        (
            settings.max_travel_mm.is_finite() && (0.1..=100.0).contains(&settings.max_travel_mm),
            "search travel must be between 0.1 and 100 mm",
        ),
        (
            settings.probe_feed_mm_per_min.is_finite()
                && (1.0..=500.0).contains(&settings.probe_feed_mm_per_min),
            "probe feed must be between 1 and 500 mm/min",
        ),
        (
            settings.retract_mm.is_finite() && (0.1..=100.0).contains(&settings.retract_mm),
            "retract must be between 0.1 and 100 mm",
        ),
        (
            settings.retract_feed_mm_per_min.is_finite()
                && (1.0..=2_000.0).contains(&settings.retract_feed_mm_per_min),
            "retract feed must be between 1 and 2000 mm/min",
        ),
    ];
    checks
        .into_iter()
        .find_map(|(valid, message)| (!valid).then_some(message))
        .map_or(Ok(()), |message| {
            Err(ArbiterError::InvalidZProbeSettings(message))
        })
}

pub(super) fn parse_probe_position(
    parameters: &DeviceInspection,
) -> Result<Position, ArbiterError> {
    let raw = parameters
        .parameters
        .get("PRB")
        .ok_or_else(|| ArbiterError::ZProbeVerification("$# did not return PRB".to_owned()))?;
    let (position, success) = raw
        .rsplit_once(':')
        .ok_or_else(|| ArbiterError::ZProbeVerification(format!("malformed PRB value: {raw}")))?;
    if success != "1" {
        return Err(ArbiterError::ZProbeContactNotFound);
    }
    let [x, y, z] = parse_xyz_parameter(position).ok_or_else(|| {
        ArbiterError::ZProbeVerification(format!("malformed PRB position: {raw}"))
    })?;
    Ok(Position { x, y, z, a: None })
}

pub(super) fn verify_probe_zero_snapshot(
    snapshot: &ControllerSnapshot,
    parameters: &DeviceInspection,
    coordinate_system: WorkCoordinateSystem,
    expected_work_z: f64,
) -> Result<(), ArbiterError> {
    let actual = verified_work_axis(snapshot, parameters, coordinate_system, WorkAxis::Z)
        .map_err(|error| ArbiterError::ZProbeVerification(error.to_string()))?;
    if (actual - expected_work_z).abs() > WORK_ZERO_TOLERANCE_MM {
        return Err(ArbiterError::ZProbeVerification(format!(
            "expected current work Z {expected_work_z:.3} mm after G10, read {actual:.3} mm"
        )));
    }
    Ok(())
}

pub(super) fn derive_probe_work_z(
    parameters: &DeviceInspection,
    coordinate_system: WorkCoordinateSystem,
    contact: Position,
) -> Result<f64, ArbiterError> {
    let parameter_name = work_coordinate_parameter(coordinate_system);
    let offset = parameters
        .parameters
        .get(parameter_name)
        .and_then(|value| parse_xyz_parameter(value))
        .ok_or_else(|| {
            ArbiterError::ZProbeVerification(format!(
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
    Ok(contact.z - offset[2] - g92[2] - tlo)
}

pub(super) fn restore_probe_modal_command(modal: &[String]) -> String {
    let units = if modal.iter().any(|word| word == "G20") {
        "G20"
    } else {
        "G21"
    };
    let distance = if modal.iter().any(|word| word == "G91") {
        "G91"
    } else {
        "G90"
    };
    let feed = if modal.iter().any(|word| word == "G93") {
        "G93"
    } else {
        "G94"
    };
    format!("G0 {units} {distance} {feed}")
}

pub(super) fn probe_start_blocked(snapshot: &ControllerSnapshot) -> ArbiterError {
    ArbiterError::ProbeStartBlocked {
        connection: snapshot.connection,
        mode: snapshot.machine.mode,
        alarm_active: snapshot.alarm.is_some(),
        reset_pending: snapshot.reset_notice.is_some(),
    }
}

pub(super) fn ensure_probe_start_idle(snapshot: &ControllerSnapshot) -> Result<(), ArbiterError> {
    if snapshot.is_stable_idle() {
        Ok(())
    } else {
        Err(probe_start_blocked(snapshot))
    }
}

pub(super) fn probe_start_can_settle(error: &ArbiterError) -> bool {
    matches!(
        error,
        ArbiterError::ProbeStartBlocked {
            connection: ConnectionState::Connected,
            mode: MachineMode::Run | MachineMode::Jog,
            alarm_active: false,
            reset_pending: false,
        }
    )
}

pub(super) async fn wait_for_idle_after_probe(
    controller: &mut Controller<BoxedTransport>,
    settings: ZProbeSettings,
    retract_distance: f64,
) -> Result<ControllerSnapshot, ArbiterError> {
    let timeout =
        Duration::from_secs_f64(retract_distance / settings.retract_feed_mm_per_min * 60.0 + 3.0);
    let started = Instant::now();
    loop {
        let snapshot = controller.refresh_status().await?;
        match snapshot.machine.mode {
            MachineMode::Idle => return Ok(snapshot),
            MachineMode::Jog | MachineMode::Run if started.elapsed() < timeout => {
                tokio::time::sleep(Duration::from_millis(50)).await;
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
