use super::*;

pub(super) async fn poll_active_homing(actor: &mut ActorState) {
    let Some(active) = actor.active_homing.as_ref() else {
        return;
    };
    if active.started.elapsed() > active.timeout + HOMING_SETTLE_TIMEOUT {
        let _ = actor
            .controller
            .send_realtime(RealtimeCommand::FeedHold)
            .await;
        actor
            .controller
            .mark_homing_failed("Homing watchdog expired; Feed Hold was sent");
        actor.active_homing = None;
        actor.machine_envelope = None;
        publish(&actor.snapshots, &actor.controller);
        return;
    }

    if let Some(settling_since) = active.settling_since {
        match actor.controller.refresh_status().await {
            Ok(snapshot) if snapshot.machine.mode == MachineMode::Idle => {
                let position = match snapshot.machine.machine_position {
                    Some(position) => position,
                    None => {
                        actor.controller.mark_homing_failed(
                            "Homing completed without a machine-coordinate position",
                        );
                        actor.active_homing = None;
                        actor.machine_envelope = None;
                        publish(&actor.snapshots, &actor.controller);
                        return;
                    }
                };
                let active = actor
                    .active_homing
                    .take()
                    .expect("homing state disappeared");
                actor.machine_envelope = Some(machine_envelope_after_homing(position, &active));
                actor.controller.mark_homing_completed();
            }
            Ok(snapshot) if settling_since.elapsed() < HOMING_SETTLE_TIMEOUT => {
                if matches!(
                    snapshot.machine.mode,
                    MachineMode::Alarm | MachineMode::Door
                ) {
                    actor
                        .controller
                        .mark_homing_failed(format!("Homing ended in {:?}", snapshot.machine.mode));
                    actor.active_homing = None;
                    actor.machine_envelope = None;
                }
            }
            Ok(_) => {
                actor.controller.mark_homing_failed(format!(
                    "Homing did not settle to Idle within {} ms",
                    HOMING_SETTLE_TIMEOUT.as_millis()
                ));
                actor.active_homing = None;
                actor.machine_envelope = None;
            }
            Err(error) => {
                actor.controller.mark_homing_failed(error.to_string());
                actor.active_homing = None;
                actor.machine_envelope = None;
            }
        }
        publish(&actor.snapshots, &actor.controller);
        return;
    }

    match actor.controller.poll_homing(SENDER_RESPONSE_SLICE).await {
        Ok(ProgramResponsePoll::Pending) => {}
        Ok(ProgramResponsePoll::StatusObserved) => {
            publish(&actor.snapshots, &actor.controller);
        }
        Ok(ProgramResponsePoll::Terminal(_)) => {
            if let Some(active) = actor.active_homing.as_mut() {
                active.settling_since = Some(Instant::now());
            }
            publish(&actor.snapshots, &actor.controller);
        }
        Err(error) => {
            let _ = actor
                .controller
                .send_realtime(RealtimeCommand::FeedHold)
                .await;
            actor.controller.mark_homing_failed(error.to_string());
            actor.active_homing = None;
            actor.machine_envelope = None;
            publish(&actor.snapshots, &actor.controller);
        }
    }
}

pub(super) fn machine_envelope_after_homing(
    position: Position,
    active: &ActiveHoming,
) -> MachineEnvelope {
    let positions = [position.x, position.y, position.z];
    let ranges = std::array::from_fn(|axis| {
        let usable = (active.travel_mm[axis] - 2.0 * active.pull_off_mm).max(0.0);
        if active.direction_mask & (1 << axis) == 0 {
            (positions[axis] - usable, positions[axis])
        } else {
            (positions[axis], positions[axis] + usable)
        }
    });
    MachineEnvelope { ranges }
}

pub(super) async fn begin_homing(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    request: HomingRequest,
    sequence: u64,
) -> Result<(HomingStartOutcome, ActiveHoming), ArbiterError> {
    if !request.operator_confirmed {
        return Err(ArbiterError::HomingConfirmationRequired);
    }
    if !hardware_profile.homing_installed {
        return Err(ArbiterError::HomingNotInstalled);
    }

    let snapshot = controller.refresh_status().await?;
    if snapshot.reset_notice.is_some()
        || snapshot.alarm.is_some() && snapshot.machine.mode != MachineMode::Alarm
        || !matches!(
            snapshot.machine.mode,
            MachineMode::Idle | MachineMode::Alarm
        )
    {
        return Err(ArbiterError::HomingUnavailable(snapshot.machine.mode));
    }
    let inspection = controller.inspect_device().await?;
    if setting_flag(&inspection, "$22") != Some(true) {
        return Err(ArbiterError::HomingDisabled);
    }
    let travel_mm = [
        positive_device_setting(&inspection, "$130")?,
        positive_device_setting(&inspection, "$131")?,
        positive_device_setting(&inspection, "$132")?,
    ];
    let direction_mask = inspection
        .settings
        .get("$23")
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0);
    let pull_off_mm = positive_device_setting(&inspection, "$27").unwrap_or(1.0);
    let timeout = homing_timeout(&inspection, travel_mm);

    controller.begin_homing(timeout).await?;
    controller.mark_homing_started(sequence, timeout);
    let outcome = HomingStartOutcome {
        command: "$H".to_owned(),
        timeout_ms: timeout.as_millis().try_into().unwrap_or(u64::MAX),
        snapshot: controller.snapshot(),
    };
    Ok((
        outcome,
        ActiveHoming {
            started: Instant::now(),
            timeout,
            settling_since: None,
            travel_mm,
            direction_mask,
            pull_off_mm,
        },
    ))
}

pub(super) fn homing_timeout(inspection: &DeviceInspection, travel_mm: [f64; 3]) -> Duration {
    let seek = positive_device_setting(inspection, "$25").unwrap_or(500.0);
    let locate = positive_device_setting(inspection, "$24").unwrap_or(25.0);
    let travel = travel_mm.into_iter().sum::<f64>();
    let seconds = travel / seek * 60.0 + travel.min(30.0) / locate * 60.0 + 15.0;
    Duration::try_from_secs_f64(seconds)
        .unwrap_or(HOMING_MAX_TIMEOUT)
        .clamp(HOMING_MIN_TIMEOUT, HOMING_MAX_TIMEOUT)
}
