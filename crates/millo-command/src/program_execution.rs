// Sender transactions execute only on the owning actor thread.
use super::*;

pub(super) struct RealRunPreflightContext<'a> {
    pub(super) intent: ProgramRunIntent,
    pub(super) execution_options: ProgramExecutionOptions,
    pub(super) heightmap: Option<&'a Heightmap>,
    pub(super) require_check_certificate: bool,
}

pub(super) async fn execute_real_run_preflight(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    program_check: &mut ProgramCheckGate,
    program: Arc<GcodeProgram>,
    context: RealRunPreflightContext<'_>,
) -> Result<RunPreflightReport, ArbiterError> {
    let RealRunPreflightContext {
        intent,
        execution_options,
        heightmap,
        require_check_certificate,
    } = context;
    controller.refresh_status().await?;
    ensure_stable_idle(&controller.snapshot())?;
    let device = controller.inspect_device().await?;
    let snapshot = controller.refresh_status().await?;
    let readiness = assess(hardware_profile, &device, &snapshot);
    let hardware = HardwareInspection { device, readiness };
    let binding = ProgramCheckBinding::from_program(&program, execution_options);
    let mut report = assess_real_run_preflight_with_options(
        &program,
        hardware,
        &snapshot,
        intent,
        execution_options,
    );
    apply_heightmap_preflight(&mut report, &program, intent, execution_options, heightmap);
    apply_rotary_preflight(&mut report, &program, hardware_profile, &snapshot);
    let current = controller.refresh_status().await?;
    ensure_unchanged_program_reference(&snapshot, &current)?;
    report.poll_sequence = current.poll_sequence;
    if require_check_certificate
        && (intent == ProgramRunIntent::Cutting
            || requires_safe_start_check(&program)
            || program.features.uses_rotary_a)
    {
        apply_program_check_requirement(
            &mut report,
            program_check.validate(&binding, &current, Instant::now()),
        );
    }
    Ok(report)
}

pub(super) fn requires_safe_start_check(program: &GcodeProgram) -> bool {
    program.source_name.starts_with("safe-start-L")
        && program
            .lines
            .first()
            .is_some_and(|line| line.source.trim().starts_with("(Millo safe start from L"))
}

pub(super) fn apply_program_check_requirement(
    report: &mut RunPreflightReport,
    result: Result<millo_run::ProgramCheckCertificate, ProgramCheckCertificateError>,
) {
    match result {
        Ok(certificate) => report.checks.push(RunPreflightCheck {
            id: "grbl-check-certificate".to_owned(),
            level: RunPreflightLevel::Pass,
            title: "GRBL Check certificate".to_owned(),
            detail: format!(
                "Check #{} validated this exact program and execution options in the current controller session",
                certificate.sequence
            ),
            source_line: None,
        }),
        Err(error) => {
            report.ready = false;
            report.blocker_count = report.blocker_count.saturating_add(1);
            report.checks.push(RunPreflightCheck {
                id: "grbl-check-certificate".to_owned(),
                level: RunPreflightLevel::Blocker,
                title: "GRBL Check required".to_owned(),
                detail: error.to_string(),
                source_line: None,
            });
        }
    }
}

pub(super) fn apply_heightmap_preflight(
    report: &mut RunPreflightReport,
    program: &GcodeProgram,
    intent: ProgramRunIntent,
    execution_options: ProgramExecutionOptions,
    heightmap: Option<&Heightmap>,
) {
    if execution_options.surface_map_id.is_none() {
        return;
    }
    let policy = match intent {
        ProgramRunIntent::AirRun => ProgramRunPolicy::AirRun,
        ProgramRunIntent::Cutting => ProgramRunPolicy::Cutting,
    };
    match build_program_run_plan_with_heightmap(program, policy, execution_options, heightmap) {
        Ok(plan) => {
            report.checks.push(RunPreflightCheck {
                id: "heightmap-compensation".to_owned(),
                level: RunPreflightLevel::Pass,
                title: "Heightmap compensation".to_owned(),
                detail: format!(
                    "Map #{} covers the program; {} compensated sender block(s) prepared",
                    execution_options.surface_map_id.unwrap_or_default(),
                    plan.lines().len()
                ),
                source_line: None,
            });
            if let Some(quality) = heightmap.and_then(|map| map.surface_quality().ok())
                && quality.suspicious_neighbor_jump
            {
                report.checks.push(RunPreflightCheck {
                    id: "heightmap-surface-quality".to_owned(),
                    level: RunPreflightLevel::Caution,
                    title: "Heightmap contains a sharp neighboring jump".to_owned(),
                    detail: format!(
                        "Largest neighboring change is {:.3} mm while the median is {:.3} mm; verify probe contact, workpiece coverage and unchanged cutter stick-out",
                        quality.maximum_neighbor_delta_mm,
                        quality.median_neighbor_delta_mm,
                    ),
                    source_line: None,
                });
            }
        }
        Err(error) => {
            report.ready = false;
            report.blocker_count = report.blocker_count.saturating_add(1);
            report.checks.push(RunPreflightCheck {
                id: "heightmap-compensation".to_owned(),
                level: RunPreflightLevel::Blocker,
                title: "Heightmap compensation".to_owned(),
                detail: error.to_string(),
                source_line: error
                    .blockers()
                    .first()
                    .and_then(|blocker| blocker.source_line),
            });
        }
    }
}

pub(super) struct FirstCutAuthorizationContext<'a> {
    pub(super) program: Arc<GcodeProgram>,
    pub(super) confirmation: FirstCutConfirmation,
    pub(super) heightmap: Option<&'a Heightmap>,
    pub(super) require_check_certificate: bool,
}

pub(super) async fn execute_first_cut_authorization(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    first_cut: &mut FirstCutGate,
    program_check: &mut ProgramCheckGate,
    context: FirstCutAuthorizationContext<'_>,
) -> Result<FirstCutPreparation, ArbiterError> {
    let FirstCutAuthorizationContext {
        program,
        confirmation,
        heightmap,
        require_check_certificate,
    } = context;
    let report = execute_real_run_preflight(
        controller,
        hardware_profile,
        program_check,
        program,
        RealRunPreflightContext {
            intent: confirmation.intent,
            execution_options: confirmation.execution_options,
            heightmap,
            require_check_certificate,
        },
    )
    .await?;
    let authorization = first_cut.authorize(
        confirmation,
        &report,
        &controller.snapshot(),
        Instant::now(),
    )?;
    Ok(FirstCutPreparation {
        report,
        authorization,
    })
}

pub(super) async fn execute_authorized_program_run_start(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    first_cut: &mut FirstCutGate,
    sender: &mut Sender,
    program: Arc<GcodeProgram>,
    authorization_id: u64,
    heightmap: Option<&Heightmap>,
) -> Result<SenderSnapshot, ArbiterError> {
    let sender_state = sender.snapshot().state;
    if matches!(
        sender_state,
        SenderState::Running
            | SenderState::Paused
            | SenderState::ToolChange
            | SenderState::Draining
    ) {
        return Err(SenderError::Busy(sender_state).into());
    }
    let fingerprint = program_fingerprint(&program);
    if program.features.uses_rotary_a {
        let inspection = controller.inspect_device().await?;
        let current = controller.refresh_status().await?;
        ensure_stable_idle(&current)?;
        validate_rotary_program(&program, hardware_profile, &inspection, &current)?;
    }
    let snapshot = controller.refresh_status().await?;
    ensure_stable_idle(&snapshot)?;
    let authorization =
        first_cut.consume(authorization_id, &fingerprint, &snapshot, Instant::now())?;
    let policy = match authorization.intent {
        ProgramRunIntent::AirRun => ProgramRunPolicy::AirRun,
        ProgramRunIntent::Cutting => ProgramRunPolicy::Cutting,
    };
    let plan = build_program_run_plan_with_heightmap(
        &program,
        policy,
        authorization.execution_options,
        heightmap,
    )?;
    let current = controller.refresh_status().await?;
    ensure_unchanged_program_reference(&snapshot, &current)?;
    sender.configure_rx_buffer_capacity(usable_rx_buffer_capacity(
        authorization.reported_rx_buffer_bytes,
    ))?;
    match authorization.intent {
        ProgramRunIntent::AirRun => sender.load_air_run(plan)?,
        ProgramRunIntent::Cutting => sender.load_cut_run(plan)?,
    };
    sender.start().map_err(ArbiterError::from)
}

pub(super) fn ensure_unchanged_program_reference(
    before: &ControllerSnapshot,
    after: &ControllerSnapshot,
) -> Result<(), ArbiterError> {
    ensure_stable_idle(after)?;
    if before.reset_count != after.reset_count || before.reconnect_count != after.reconnect_count {
        return Err(FirstCutAuthorizationError::ControllerSessionChanged.into());
    }
    if before.machine.machine_position != after.machine.machine_position
        || before.machine.work_position != after.machine.work_position
        || before.machine.work_coordinate_offset != after.machine.work_coordinate_offset
    {
        return Err(FirstCutAuthorizationError::ControllerPositionChanged.into());
    }
    Ok(())
}

pub(super) async fn execute_check_run_start(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    sender: &mut Sender,
    program: &GcodeProgram,
    execution_options: ProgramExecutionOptions,
    heightmap: Option<&Heightmap>,
) -> Result<SenderSnapshot, ArbiterError> {
    let sender_state = sender.snapshot().state;
    if matches!(
        sender_state,
        SenderState::Running
            | SenderState::Paused
            | SenderState::ToolChange
            | SenderState::Draining
    ) {
        return Err(SenderError::Busy(sender_state).into());
    }

    let plan = build_program_run_plan_with_heightmap(
        program,
        ProgramRunPolicy::Cutting,
        execution_options,
        heightmap,
    )?;
    let initial = controller.refresh_status().await?;
    ensure_stable_idle(&initial)?;
    let inspection = controller.inspect_device().await?;
    let final_idle = controller.refresh_status().await?;
    ensure_stable_idle(&final_idle)?;
    validate_rotary_program(program, hardware_profile, &inspection, &final_idle)?;

    sender.configure_rx_buffer_capacity(usable_rx_buffer_capacity(
        inspection
            .controller_capabilities
            .as_ref()
            .and_then(|capabilities| capabilities.rx_buffer_bytes),
    ))?;
    sender.load_check_run(plan)?;
    if let Err(error) = controller.set_check_mode(true).await {
        let _ = sender.cancel();
        return Err(error.into());
    }
    match sender.start() {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            let _ = controller.set_check_mode(false).await;
            Err(error.into())
        }
    }
}

pub(super) async fn execute_program_run_resume(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
) -> Result<SenderSnapshot, ArbiterError> {
    let sender_state = sender.snapshot().state;
    if sender_state != SenderState::Paused {
        return Err(SenderError::InvalidTransition {
            action: "resume",
            state: sender_state,
        }
        .into());
    }
    let snapshot = match refresh_paused_sender_status(controller, sender).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            handle_sender_controller_failure(
                controller,
                sender,
                &error,
                "program resume status failed",
            )
            .await;
            return Err(error.into());
        }
    };
    if snapshot.connection != ConnectionState::Connected
        || snapshot.reset_notice.is_some()
        || snapshot.alarm.is_some()
        || snapshot.machine.mode == MachineMode::Alarm
    {
        sender.fail_with(SenderFailure::new(
            SenderFailureKind::UnsafeState,
            "controller became unsafe while resuming the program",
        ));
        return Err(SafetyError::UnsafeControllerState.into());
    }
    match snapshot.machine.mode {
        MachineMode::Hold => {
            if let Err(error) = controller.send_realtime(RealtimeCommand::CycleStart).await {
                handle_sender_controller_failure(
                    controller,
                    sender,
                    &error,
                    "program resume could not be delivered",
                )
                .await;
                return Err(error.into());
            }
        }
        MachineMode::Idle => {}
        mode => return Err(ArbiterError::ProgramRunResumeUnavailable(mode)),
    }
    sender.resume().map_err(ArbiterError::from)
}

async fn refresh_paused_sender_status(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
) -> Result<ControllerSnapshot, ControllerError> {
    if !sender.has_in_flight() {
        return controller.refresh_status().await;
    }
    let timeout = controller.status_timeout();
    match tokio::time::timeout(timeout, async {
        controller.request_interleaved_status().await?;
        // Status and terminal replies share a FIFO. Account for every ACK that
        // precedes the requested status instead of letting refresh_status discard it.
        loop {
            let Some(line) = sender.oldest_in_flight() else {
                return controller.refresh_status().await;
            };
            match controller
                .poll_program_response(&line, SENDER_RESPONSE_SLICE)
                .await?
            {
                ProgramResponsePoll::Terminal(_) => {
                    sender.acknowledge_ok().map_err(|_| {
                        ControllerError::ProgramResponseState(
                            "resume acknowledgement has no sender line",
                        )
                    })?;
                }
                ProgramResponsePoll::StatusObserved => return Ok(controller.snapshot()),
                ProgramResponsePoll::Pending => {}
            }
        }
    })
    .await
    {
        Ok(result) => result,
        Err(_) => Err(ControllerError::StatusTimeout {
            timeout_ms: timeout.as_millis().min(u128::from(u64::MAX)) as u64,
        }),
    }
}

pub(super) fn ensure_active_physical_sender(
    sender: &Sender,
    action: &'static str,
) -> Result<SenderState, ArbiterError> {
    let snapshot = sender.snapshot();
    if !matches!(
        snapshot.mode,
        Some(millo_sender::SenderMode::AirRun | millo_sender::SenderMode::CutRun)
    ) {
        return Err(match action {
            "pause" => ArbiterError::ProgramRunPauseUnavailable(snapshot.state),
            _ => ArbiterError::ProgramRunStopUnavailable(snapshot.state),
        });
    }
    Ok(snapshot.state)
}

pub(super) async fn execute_program_run_pause(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
) -> Result<SenderSnapshot, ArbiterError> {
    let state = ensure_active_physical_sender(sender, "pause")?;
    if !matches!(state, SenderState::Running | SenderState::Draining) {
        return Err(ArbiterError::ProgramRunPauseUnavailable(state));
    }
    if let Err(error) = controller.send_realtime(RealtimeCommand::FeedHold).await {
        handle_sender_controller_failure(
            controller,
            sender,
            &error,
            "program pause could not be delivered",
        )
        .await;
        return Err(error.into());
    }
    sender.pause().map_err(ArbiterError::from)
}

pub(super) async fn execute_program_run_abort(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
) -> Result<SenderSnapshot, ArbiterError> {
    let state = ensure_active_physical_sender(sender, "stop")?;
    if !matches!(
        state,
        SenderState::Running
            | SenderState::Paused
            | SenderState::ToolChange
            | SenderState::Draining
    ) {
        return Err(ArbiterError::ProgramRunStopUnavailable(state));
    }
    if let Err(error) = controller.abort_program_stream().await {
        sender.fail_with(controller_sender_failure(
            &error,
            "operator stop could not be delivered",
        ));
        if controller_failure_requires_manual_reconnect(&error) {
            let _ = controller.disconnect().await;
        }
        return Err(error.into());
    }
    sender.cancel().map_err(ArbiterError::from)
}

pub(super) async fn execute_tool_change_completion(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    sender: &mut Sender,
    confirmation: ToolChangeConfirmation,
    rotary_reference: Option<&RotaryRunReference>,
) -> Result<SenderSnapshot, ArbiterError> {
    let active = sender.snapshot();
    if active.state != SenderState::ToolChange {
        return Err(ArbiterError::ToolChangeUnavailable(active.state));
    }
    if active.current_source_line != Some(confirmation.source_line)
        || active.requested_tool != confirmation.requested_tool
    {
        return Err(ArbiterError::ToolChangeMismatch);
    }
    let missing = confirmation.missing();
    if !missing.is_empty() {
        return Err(ArbiterError::ToolChangeConfirmationIncomplete(missing));
    }

    let initial = controller.refresh_status().await?;
    if initial.machine.mode != MachineMode::Idle {
        return Err(ArbiterError::ToolChangeControllerUnavailable(
            initial.machine.mode,
        ));
    }
    ensure_stable_idle(&initial)?;
    let inspection = controller.inspect_device().await?;
    active_work_coordinate_system(&inspection.modal_state)
        .ok_or(ArbiterError::ActiveWorkCoordinateSystemUnavailable)?;
    let final_snapshot = controller.refresh_status().await?;
    if final_snapshot.machine.mode != MachineMode::Idle {
        return Err(ArbiterError::ToolChangeControllerUnavailable(
            final_snapshot.machine.mode,
        ));
    }
    ensure_stable_idle(&final_snapshot)?;

    if let Some(reference) = rotary_reference {
        validate_rotary_capability(hardware_profile, &inspection, &final_snapshot)?;
        reference.verify(&active, &inspection, &final_snapshot)?;
    }

    sender.complete_tool_change().map_err(ArbiterError::from)
}

pub(super) fn invalidate_authorizations(safety: &mut SafetyManager, first_cut: &mut FirstCutGate) {
    safety.invalidate_test_jog();
    first_cut.invalidate();
}

pub(super) async fn reconcile_physical_sender(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
    sender_snapshots: &watch::Sender<SenderSnapshot>,
) {
    let snapshot = controller.snapshot();
    sender.observe_executing_line_number(snapshot.machine.line_number);
    let sender_state = sender.snapshot().state;
    if !matches!(
        sender_state,
        SenderState::Running
            | SenderState::Paused
            | SenderState::ToolChange
            | SenderState::Draining
    ) {
        return;
    }
    if snapshot.connection != ConnectionState::Connected
        || snapshot.alarm.is_some()
        || snapshot.reset_notice.is_some()
        || snapshot.machine.mode == MachineMode::Alarm
    {
        sender.fail_with(SenderFailure::new(
            SenderFailureKind::UnsafeState,
            "controller became unavailable while waiting for physical motion to finish",
        ));
    } else if sender.snapshot().mode == Some(millo_sender::SenderMode::CheckRun)
        && snapshot.machine.mode != MachineMode::Check
    {
        sender.fail_with(SenderFailure::new(
            SenderFailureKind::UnsafeState,
            format!(
                "controller left GRBL Check mode during validation: {:?}",
                snapshot.machine.mode
            ),
        ));
    } else {
        match (sender_state, snapshot.machine.mode) {
            (
                SenderState::Running | SenderState::Draining,
                MachineMode::Hold | MachineMode::Door,
            ) => {
                let _ = sender.pause();
            }
            (SenderState::Draining, MachineMode::Idle) => {
                if sender.deferred_program_end().is_some() {
                    match sender.dispatch_deferred_program_end() {
                        Ok(line) => {
                            if let Err(error) = controller.write_program_line(&line).await {
                                sender.fail_dispatched_line_with(
                                    line,
                                    controller_sender_failure(&error, "program-end write failed"),
                                );
                                let _ = controller.abort_program_stream().await;
                                if controller_failure_requires_manual_reconnect(&error) {
                                    let _ = controller.disconnect().await;
                                }
                            }
                        }
                        Err(error) => {
                            sender.fail(error.to_string());
                        }
                    }
                }
                if sender.snapshot().state == SenderState::Draining
                    && sender.deferred_program_end().is_none()
                    && !sender.has_in_flight()
                {
                    let _ = sender.complete_draining();
                }
            }
            _ => {}
        }
    }
    publish_sender(sender_snapshots, sender);
}

pub(super) async fn fail_and_quarantine_physical_sender(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
    error: &ControllerError,
    context: &str,
    sender_snapshots: &watch::Sender<SenderSnapshot>,
) {
    handle_sender_controller_failure(controller, sender, error, context).await;
    publish_sender(sender_snapshots, sender);
}

async fn handle_sender_controller_failure(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
    error: &ControllerError,
    context: &str,
) {
    let physical_run = physical_sender_active(sender);
    if matches!(
        sender.snapshot().state,
        SenderState::Running
            | SenderState::Paused
            | SenderState::ToolChange
            | SenderState::Draining
    ) {
        sender.fail_with(controller_sender_failure(error, context));
    }
    if physical_run {
        let stop = controller.abort_program_stream().await;
        if controller_failure_requires_manual_reconnect(error) || stop.is_err() {
            let _ = controller.disconnect().await;
        }
    }
}

pub(super) fn physical_sender_active(sender: &Sender) -> bool {
    matches!(
        sender.snapshot().mode,
        Some(millo_sender::SenderMode::AirRun | millo_sender::SenderMode::CutRun)
    ) && matches!(
        sender.snapshot().state,
        SenderState::Running
            | SenderState::Paused
            | SenderState::ToolChange
            | SenderState::Draining
    )
}

pub(super) fn controller_failure_requires_manual_reconnect(error: &ControllerError) -> bool {
    matches!(
        error,
        ControllerError::CommandTimeout { .. }
            | ControllerError::StatusTimeout { .. }
            | ControllerError::Transport(_)
            | ControllerError::NotReady(_)
    )
}

pub(super) fn controller_sender_failure(error: &ControllerError, context: &str) -> SenderFailure {
    let (kind, code, detail) = match error {
        ControllerError::CommandRejected {
            command,
            completion,
            code,
        } => {
            let (kind, label) = match completion {
                CommandCompletion::Error => (SenderFailureKind::GrblError, "GRBL error"),
                CommandCompletion::Alarm => (SenderFailureKind::Alarm, "GRBL alarm"),
                CommandCompletion::Reset => (SenderFailureKind::Reset, "GRBL reset"),
                CommandCompletion::Ok => (SenderFailureKind::Internal, "unexpected GRBL response"),
            };
            let code_text = code.map_or_else(String::new, |value| format!(" {value}"));
            (
                kind,
                *code,
                format!("{label}{code_text} while executing '{command}'"),
            )
        }
        ControllerError::CommandTimeout { timeout_ms } => (
            SenderFailureKind::Timeout,
            None,
            format!("controller command timed out after {timeout_ms} ms"),
        ),
        ControllerError::StatusTimeout { timeout_ms } => (
            SenderFailureKind::Timeout,
            None,
            format!("controller status timed out after {timeout_ms} ms"),
        ),
        ControllerError::Transport(TransportError::NotConnected) => (
            SenderFailureKind::Disconnected,
            None,
            "transport disconnected".to_owned(),
        ),
        ControllerError::Transport(transport) => {
            (SenderFailureKind::Transport, None, transport.to_string())
        }
        ControllerError::NotReady(_) => (SenderFailureKind::UnsafeState, None, error.to_string()),
        _ => (SenderFailureKind::Internal, None, error.to_string()),
    };
    SenderFailure::new(kind, format!("{context}: {detail}")).with_grbl_code(code)
}

pub(super) async fn execute_sender_step(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
    program_check: &mut ProgramCheckGate,
    pending_program_check: &mut Option<ProgramCheckBinding>,
    snapshots: &watch::Sender<ControllerSnapshot>,
    sender_snapshots: &watch::Sender<SenderSnapshot>,
) {
    if let Err(error) = ensure_sender_dispatch_ready(sender, &controller.snapshot()) {
        sender.fail_with(SenderFailure::new(
            SenderFailureKind::UnsafeState,
            error.to_string(),
        ));
        settle_program_check(controller, sender, program_check, pending_program_check).await;
        publish(snapshots, controller);
        publish_sender(sender_snapshots, sender);
        return;
    }
    while let Some(line) = sender.next_line() {
        if let Err(error) = controller.write_program_line(&line).await {
            let physical_run = matches!(
                sender.snapshot().mode,
                Some(millo_sender::SenderMode::AirRun | millo_sender::SenderMode::CutRun)
            );
            sender.fail_dispatched_line_with(
                line,
                controller_sender_failure(&error, "program write failed"),
            );
            if physical_run {
                let _ = controller.abort_program_stream().await;
                if controller_failure_requires_manual_reconnect(&error) {
                    let _ = controller.disconnect().await;
                }
            }
            settle_program_check(controller, sender, program_check, pending_program_check).await;
            publish(snapshots, controller);
            publish_sender(sender_snapshots, sender);
            return;
        }
        publish_sender(sender_snapshots, sender);
    }

    if let Some(line) = sender.oldest_in_flight() {
        match controller
            .poll_program_response(&line, SENDER_RESPONSE_SLICE)
            .await
        {
            Ok(ProgramResponsePoll::Terminal(_)) => {
                let _ = sender.acknowledge_ok();
                if line.kind() == DryRunLineKind::ProgramEnd
                    && sender.snapshot().state == SenderState::Draining
                    && !sender.has_in_flight()
                    && sender.deferred_program_end().is_none()
                    && controller.snapshot().machine.mode == MachineMode::Idle
                {
                    let _ = sender.complete_draining();
                }
            }
            Ok(ProgramResponsePoll::StatusObserved) => {
                reconcile_physical_sender(controller, sender, sender_snapshots).await;
            }
            Ok(ProgramResponsePoll::Pending) => {}
            Err(error) => {
                let physical_run = matches!(
                    sender.snapshot().mode,
                    Some(millo_sender::SenderMode::AirRun | millo_sender::SenderMode::CutRun)
                );
                let failure = controller_sender_failure(&error, "program response failed");
                let _ = sender.acknowledge_failure(failure);
                if physical_run {
                    let _ = controller.abort_program_stream().await;
                    if controller_failure_requires_manual_reconnect(&error) {
                        let _ = controller.disconnect().await;
                    }
                }
            }
        }
    }
    settle_program_check(controller, sender, program_check, pending_program_check).await;
    publish(snapshots, controller);
    publish_sender(sender_snapshots, sender);
}

pub(super) fn ensure_sender_dispatch_ready(
    sender: &Sender,
    snapshot: &ControllerSnapshot,
) -> Result<(), ArbiterError> {
    if snapshot.connection != ConnectionState::Connected
        || snapshot.alarm.is_some()
        || snapshot.reset_notice.is_some()
    {
        return Err(SafetyError::UnsafeControllerState.into());
    }
    let mode_ready = match sender.snapshot().mode {
        Some(millo_sender::SenderMode::AirRun | millo_sender::SenderMode::CutRun) => {
            matches!(
                snapshot.machine.mode,
                MachineMode::Idle | MachineMode::Run | MachineMode::Hold
            ) || (sender.snapshot().state == SenderState::Paused
                && snapshot.machine.mode == MachineMode::Door)
        }
        Some(millo_sender::SenderMode::CheckRun) => snapshot.machine.mode == MachineMode::Check,
        _ => snapshot.machine.mode == MachineMode::Idle,
    };
    if mode_ready {
        Ok(())
    } else {
        Err(SafetyError::UnsafeControllerState.into())
    }
}

pub(super) async fn settle_program_check(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
    program_check: &mut ProgramCheckGate,
    pending_program_check: &mut Option<ProgramCheckBinding>,
) {
    let sender_snapshot = sender.snapshot();
    if sender_snapshot.mode != Some(millo_sender::SenderMode::CheckRun)
        || !matches!(
            sender_snapshot.state,
            SenderState::Completed | SenderState::Failed | SenderState::Cancelled
        )
    {
        return;
    }

    let reset_count_before_cleanup = controller.snapshot().reset_count;
    let safely_idle = if controller.snapshot().connection != ConnectionState::Connected {
        false
    } else if controller.snapshot().machine.mode == MachineMode::Check {
        match controller.set_check_mode(false).await {
            Ok(snapshot) => snapshot.machine.mode == MachineMode::Idle,
            Err(error) => {
                sender.fail(format!("failed to leave GRBL Check mode: {error}"));
                false
            }
        }
    } else {
        controller.snapshot().machine.mode == MachineMode::Idle
    };
    let safely_idle = if safely_idle && controller.snapshot().reset_notice.is_some() {
        let expected_transition_reset =
            controller.snapshot().reset_count == reset_count_before_cleanup.saturating_add(1);
        if expected_transition_reset {
            controller.acknowledge_reset();
            controller.refresh_status().await.is_ok()
                && ensure_stable_idle(&controller.snapshot()).is_ok()
        } else {
            false
        }
    } else {
        safely_idle
    };

    let completed = sender.snapshot().state == SenderState::Completed;
    if completed && safely_idle {
        if let Some(binding) = pending_program_check.take()
            && let Err(error) = program_check.issue(binding, &controller.snapshot(), Instant::now())
        {
            let snapshot = controller.snapshot();
            sender.fail(format!(
                "failed to issue GRBL Check certificate: {error}; connection={:?}, mode={:?}, reset={}, alarm={}",
                snapshot.connection,
                snapshot.machine.mode,
                snapshot.reset_notice.is_some(),
                snapshot.alarm.is_some(),
            ));
        }
    } else {
        *pending_program_check = None;
        program_check.invalidate();
    }
}

pub(super) async fn cancel_check_run(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
    program_check: &mut ProgramCheckGate,
    pending_program_check: &mut Option<ProgramCheckBinding>,
    snapshots: &watch::Sender<SenderSnapshot>,
) {
    if sender.snapshot().mode != Some(millo_sender::SenderMode::CheckRun) {
        return;
    }
    cancel_active_sender(sender, snapshots);
    settle_program_check(controller, sender, program_check, pending_program_check).await;
    publish_sender(snapshots, sender);
}

pub(super) fn cancel_active_sender(sender: &mut Sender, snapshots: &watch::Sender<SenderSnapshot>) {
    if matches!(
        sender.snapshot().state,
        SenderState::Ready
            | SenderState::Running
            | SenderState::Paused
            | SenderState::ToolChange
            | SenderState::Draining
    ) {
        let _ = sender.cancel();
        publish_sender(snapshots, sender);
    }
}
