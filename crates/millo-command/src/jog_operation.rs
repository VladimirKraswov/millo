use super::*;

pub(super) async fn poll_active_continuous_jog(actor: &mut ActorState) {
    let deadline_reached = actor
        .active_continuous_jog
        .as_ref()
        .is_some_and(|active| !active.cancel_requested && Instant::now() >= active.deadline);
    if deadline_reached {
        let _ = actor
            .controller
            .send_realtime(RealtimeCommand::JogCancel)
            .await;
        if let Some(active) = actor.active_continuous_jog.as_mut() {
            active.cancel_requested = true;
        }
    }

    match actor.controller.refresh_status().await {
        Ok(snapshot) if snapshot.machine.mode == MachineMode::Idle => {
            actor.active_continuous_jog = None;
        }
        Ok(snapshot)
            if matches!(
                snapshot.machine.mode,
                MachineMode::Alarm | MachineMode::Door
            ) =>
        {
            actor.active_continuous_jog = None;
        }
        Ok(_) => {}
        Err(_) if actor.controller.snapshot().connection != ConnectionState::Connected => {
            actor.active_continuous_jog = None;
        }
        Err(_) => {}
    }
    publish(&actor.snapshots, &actor.controller);
}

pub(super) async fn prepare_test_jog(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    safety: &mut SafetyManager,
    confirmation: OperatorConfirmation,
) -> Result<TestJogPreparation, ArbiterError> {
    if !confirmation.is_complete() {
        return Err(SafetyError::IncompleteOperatorConfirmation.into());
    }

    controller.refresh_status().await?;
    let device = controller.inspect_device().await?;
    let snapshot = controller.snapshot();
    let readiness = assess(hardware_profile, &device, &snapshot);
    let inspection = HardwareInspection { device, readiness };
    let authorization = safety
        .authorize_test_jog(confirmation, &inspection, &snapshot, Instant::now())
        .ok();

    Ok(TestJogPreparation {
        inspection,
        authorization,
    })
}

pub(super) async fn execute_jog_pad_step(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    safety: &mut SafetyManager,
    request: JogPadStepRequest,
) -> Result<JogPadStepOutcome, ArbiterError> {
    ensure_jog_axis_available(hardware_profile, request.axis)?;
    validate_jog_pad_motion(request.distance_mm, request.feed_mm_per_min)?;
    let distance_limit = axis_jog_distance_limit(hardware_profile, request.axis);
    if request.distance_mm.abs() > distance_limit {
        return Err(ArbiterError::JogPadDistanceExceedsProfile {
            axis: request.axis,
            requested: request.distance_mm.abs(),
            maximum: distance_limit,
        });
    }

    let preparation =
        prepare_test_jog(controller, hardware_profile, safety, request.confirmation).await?;
    let Some(authorization) = preparation.authorization else {
        return Ok(JogPadStepOutcome {
            inspection: preparation.inspection,
            receipt: None,
        });
    };
    if let Some(maximum) = effective_axis_max_rate(
        &preparation.inspection.device,
        hardware_profile,
        request.axis,
    ) && request.feed_mm_per_min > maximum
    {
        return Err(ArbiterError::JogPadFeedExceedsAxisRate {
            axis: request.axis,
            requested: request.feed_mm_per_min,
            maximum,
        });
    }
    let step = StepJogRequest {
        authorization_id: authorization.id,
        axis: request.axis,
        distance_mm: request.distance_mm,
        feed_mm_per_min: request.feed_mm_per_min,
    };

    safety.consume_test_jog(
        step.authorization_id,
        &controller.snapshot(),
        Instant::now(),
    )?;
    let receipt = controller.step_jog(step).await?;
    Ok(JogPadStepOutcome {
        inspection: preparation.inspection,
        receipt: Some(receipt),
    })
}

pub(super) async fn begin_continuous_jog(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    safety: &mut SafetyManager,
    machine_envelope: Option<&MachineEnvelope>,
    request: ContinuousJogRequest,
) -> Result<(ContinuousJogReceipt, ActiveContinuousJog), ArbiterError> {
    ensure_jog_axis_available(hardware_profile, request.axis)?;
    if !matches!(request.direction, -1 | 1) {
        return Err(ArbiterError::ContinuousJogDirectionInvalid);
    }
    validate_jog_pad_motion(MIN_STEP_JOG_DISTANCE_MM, request.feed_mm_per_min)?;

    let preparation =
        prepare_test_jog(controller, hardware_profile, safety, request.confirmation).await?;
    let Some(authorization) = preparation.authorization else {
        return Err(SafetyError::ReadinessBlocked {
            blockers: preparation.inspection.readiness.blocker_count,
        }
        .into());
    };
    if let Some(maximum) = effective_axis_max_rate(
        &preparation.inspection.device,
        hardware_profile,
        request.axis,
    ) && request.feed_mm_per_min > maximum
    {
        return Err(ArbiterError::JogPadFeedExceedsAxisRate {
            axis: request.axis,
            requested: request.feed_mm_per_min,
            maximum,
        });
    }

    let snapshot = controller.snapshot();
    let (bounded_distance, boundary_source) = if snapshot.homing.state == HomingState::Homed
        && request.axis != millo_domain::JogAxis::A
    {
        let envelope = machine_envelope
            .ok_or(ArbiterError::ContinuousJogBoundaryReached { axis: request.axis })?;
        let axis_index = cartesian_axis_index(request.axis)
            .ok_or(ArbiterError::JogAxisUnavailable(request.axis))?;
        let position = snapshot
            .machine
            .machine_position
            .ok_or(ArbiterError::WorkPositionUnavailable)?;
        let coordinate = jog_axis_position(position, request.axis)?;
        let (lower, upper) = envelope.ranges[axis_index];
        let available = if request.direction > 0 {
            upper - coordinate
        } else {
            coordinate - lower
        } - MACHINE_BOUNDARY_MARGIN_MM;
        if available < MIN_STEP_JOG_DISTANCE_MM {
            return Err(ArbiterError::ContinuousJogBoundaryReached { axis: request.axis });
        }
        (
            available.min(MAX_STEP_JOG_DISTANCE_MM),
            JogBoundarySource::MachineCoordinates,
        )
    } else {
        (
            axis_jog_distance_limit(hardware_profile, request.axis).min(MAX_STEP_JOG_DISTANCE_MM),
            JogBoundarySource::ProfileDistance,
        )
    };

    safety.consume_test_jog(authorization.id, &snapshot, Instant::now())?;
    let signed_distance = f64::from(request.direction) * bounded_distance;
    let step = StepJogRequest {
        authorization_id: authorization.id,
        axis: request.axis,
        distance_mm: signed_distance,
        feed_mm_per_min: request.feed_mm_per_min,
    };
    let receipt = controller.step_jog(step).await?;
    let duration = Duration::from_secs_f64(bounded_distance / request.feed_mm_per_min * 60.0)
        + CONTINUOUS_JOG_WATCHDOG_MARGIN;
    Ok((
        ContinuousJogReceipt {
            command: receipt.command,
            axis: request.axis,
            direction: request.direction,
            bounded_distance,
            feed_mm_per_min: request.feed_mm_per_min,
            boundary_source,
        },
        ActiveContinuousJog {
            deadline: Instant::now() + duration,
            cancel_requested: false,
        },
    ))
}

pub(super) fn ensure_jog_axis_available(
    profile: &HardwareProfile,
    axis: millo_domain::JogAxis,
) -> Result<(), ArbiterError> {
    let label = match axis {
        millo_domain::JogAxis::X => "X",
        millo_domain::JogAxis::Y => "Y",
        millo_domain::JogAxis::Z => "Z",
        millo_domain::JogAxis::A => "A",
    };
    profile
        .axes
        .iter()
        .any(|configured| configured.eq_ignore_ascii_case(label))
        .then_some(())
        .ok_or(ArbiterError::JogAxisUnavailable(axis))
}

pub(super) fn cartesian_axis_index(axis: millo_domain::JogAxis) -> Option<usize> {
    match axis {
        millo_domain::JogAxis::X => Some(0),
        millo_domain::JogAxis::Y => Some(1),
        millo_domain::JogAxis::Z => Some(2),
        millo_domain::JogAxis::A => None,
    }
}

pub(super) fn jog_axis_position(
    position: Position,
    axis: millo_domain::JogAxis,
) -> Result<f64, ArbiterError> {
    match axis {
        millo_domain::JogAxis::X => Ok(position.x),
        millo_domain::JogAxis::Y => Ok(position.y),
        millo_domain::JogAxis::Z => Ok(position.z),
        millo_domain::JogAxis::A => position.a.ok_or(ArbiterError::JogAxisUnavailable(axis)),
    }
}

pub(super) fn validate_jog_pad_motion(
    distance_mm: f64,
    feed_mm_per_min: f64,
) -> Result<(), ArbiterError> {
    if !distance_mm.is_finite()
        || !(MIN_STEP_JOG_DISTANCE_MM..=MAX_STEP_JOG_DISTANCE_MM).contains(&distance_mm.abs())
    {
        return Err(ArbiterError::JogPadDistanceOutOfRange);
    }
    if !feed_mm_per_min.is_finite()
        || !(MIN_STEP_JOG_FEED_MM_PER_MIN..=MAX_STEP_JOG_FEED_MM_PER_MIN).contains(&feed_mm_per_min)
    {
        return Err(ArbiterError::JogPadFeedOutOfRange);
    }
    Ok(())
}

pub(super) fn axis_max_rate(device: &DeviceInspection, axis: millo_domain::JogAxis) -> Option<f64> {
    let key = match axis {
        millo_domain::JogAxis::X => "$110",
        millo_domain::JogAxis::Y => "$111",
        millo_domain::JogAxis::Z => "$112",
        millo_domain::JogAxis::A => "$113",
    };
    device
        .settings
        .get(key)
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
}

pub(super) fn effective_axis_max_rate(
    device: &DeviceInspection,
    profile: &HardwareProfile,
    axis: millo_domain::JogAxis,
) -> Option<f64> {
    axis_max_rate(device, axis).or_else(|| {
        (axis == millo_domain::JogAxis::A)
            .then(|| {
                profile
                    .rotary_axis
                    .map(|rotary| rotary.max_feed_degrees_per_min)
            })
            .flatten()
    })
}

pub(super) fn axis_travel_limit(profile: &HardwareProfile, axis: millo_domain::JogAxis) -> f64 {
    if axis == millo_domain::JogAxis::A {
        return profile
            .rotary_axis
            .map(|rotary| rotary.max_jog_degrees)
            .unwrap_or(0.0);
    }
    let Some(travel) = profile.travel_mm else {
        return profile.max_jog_distance_mm;
    };
    match axis {
        millo_domain::JogAxis::X => travel.x,
        millo_domain::JogAxis::Y => travel.y,
        millo_domain::JogAxis::Z => travel.z,
        millo_domain::JogAxis::A => unreachable!("A axis handled before Cartesian travel"),
    }
}

pub(super) fn axis_jog_distance_limit(
    profile: &HardwareProfile,
    axis: millo_domain::JogAxis,
) -> f64 {
    if axis == millo_domain::JogAxis::A {
        axis_travel_limit(profile, axis)
    } else {
        axis_travel_limit(profile, axis).min(profile.max_jog_distance_mm)
    }
}
