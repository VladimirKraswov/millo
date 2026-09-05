use super::*;

pub(super) async fn handle_request(request: Request, actor: &mut ActorState) {
    let ActorState {
        controller,
        config,
        hardware_profile,
        execution_target,
        sender,
        sender_dispatch_enabled,
        safety,
        first_cut,
        program_check,
        pending_program_check,
        verified_z_datum,
        active_homing,
        homing_sequence,
        machine_envelope,
        active_continuous_jog,
        active_z_probe,
        prepared_heightmap,
        active_heightmap,
        heightmap_sequence,
        snapshots,
        sender_snapshots,
        heightmap_snapshots,
    } = actor;
    match request {
        Request::ReplaceTransport {
            transport,
            execution_target: replacement_target,
            response,
        } => {
            let connection = controller.snapshot().connection;
            if connection != ConnectionState::Disconnected {
                let _ = response.send(Err(ArbiterError::TransportReplacementUnavailable(
                    connection,
                )));
                return;
            }
            cancel_check_run(
                controller,
                sender,
                program_check,
                pending_program_check,
                sender_snapshots,
            )
            .await;
            invalidate_authorizations(safety, first_cut);
            program_check.invalidate();
            *pending_program_check = None;
            *verified_z_datum = None;
            *active_homing = None;
            *machine_envelope = None;
            *active_continuous_jog = None;
            cancel_active_sender(sender, sender_snapshots);
            *sender_dispatch_enabled = true;
            *controller = Controller::with_config(transport, *config);
            *execution_target = replacement_target;
            publish(snapshots, controller);
            let _ = response.send(Ok(controller.snapshot()));
        }
        Request::Connect { response } => {
            let connection = controller.snapshot().connection;
            let result = if connection == ConnectionState::Disconnected {
                invalidate_authorizations(safety, first_cut);
                program_check.invalidate();
                *pending_program_check = None;
                *verified_z_datum = None;
                *active_homing = None;
                *machine_envelope = None;
                *active_continuous_jog = None;
                cancel_active_sender(sender, sender_snapshots);
                *sender_dispatch_enabled = true;
                controller.connect().await.map_err(ArbiterError::from)
            } else {
                Err(ArbiterError::ConnectUnavailable(connection))
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::SetHardwareProfile { profile, response } => {
            let connection = controller.snapshot().connection;
            let result = if connection == ConnectionState::Disconnected {
                invalidate_authorizations(safety, first_cut);
                *verified_z_datum = None;
                *machine_envelope = None;
                controller.invalidate_machine_reference("Machine profile changed");
                *hardware_profile = profile;
                Ok(hardware_profile.clone())
            } else {
                Err(ArbiterError::ProfileChangeUnavailable(connection))
            };
            let _ = response.send(result);
        }
        Request::BindHardwareProfile { profile, response } => {
            let result = ensure_profile_binding_available(&controller.snapshot()).map(|()| {
                invalidate_authorizations(safety, first_cut);
                *verified_z_datum = None;
                *machine_envelope = None;
                controller.invalidate_machine_reference("Machine profile changed");
                *hardware_profile = profile;
                hardware_profile.clone()
            });
            let _ = response.send(result);
        }
        Request::UpdateControllerSetting { request, response } => {
            invalidate_authorizations(safety, first_cut);
            program_check.invalidate();
            let result = execute_controller_setting_update(controller, request).await;
            if result.is_ok() {
                *machine_envelope = None;
                controller.invalidate_machine_reference("Controller settings changed");
            }
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::Disconnect { response } => {
            invalidate_authorizations(safety, first_cut);
            cancel_check_run(
                controller,
                sender,
                program_check,
                pending_program_check,
                sender_snapshots,
            )
            .await;
            program_check.invalidate();
            *pending_program_check = None;
            *verified_z_datum = None;
            *active_homing = None;
            *machine_envelope = None;
            if active_continuous_jog.is_some() {
                let _ = controller.send_realtime(RealtimeCommand::JogCancel).await;
                *active_continuous_jog = None;
            }
            cancel_active_sender(sender, sender_snapshots);
            *sender_dispatch_enabled = true;
            let result = controller.disconnect().await.map_err(ArbiterError::from);
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::RefreshStatus { response } => {
            let interleaved = sender.has_in_flight();
            let controller_result = if interleaved {
                controller
                    .request_interleaved_status()
                    .await
                    .map(|()| controller.snapshot())
            } else {
                controller.refresh_status().await
            };
            safety.observe(&controller.snapshot(), Instant::now());
            first_cut.observe(&controller.snapshot(), Instant::now());
            program_check.observe(&controller.snapshot(), Instant::now());
            match &controller_result {
                Ok(_) if !interleaved => {
                    reconcile_physical_sender(controller, sender, sender_snapshots).await
                }
                Ok(_) => {}
                Err(error) => {
                    fail_and_quarantine_physical_sender(
                        controller,
                        sender,
                        error,
                        "controller status failed during program run",
                        sender_snapshots,
                    )
                    .await
                }
            }
            publish(snapshots, controller);
            let _ = response.send(controller_result.map_err(ArbiterError::from));
        }
        Request::AcknowledgeReset { response } => {
            invalidate_authorizations(safety, first_cut);
            program_check.invalidate();
            *pending_program_check = None;
            let result = Ok(controller.acknowledge_reset());
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::UnlockAlarm {
            operator_confirmed,
            response,
        } => {
            invalidate_authorizations(safety, first_cut);
            let result = if operator_confirmed {
                controller.unlock_alarm().await.map_err(ArbiterError::from)
            } else {
                Err(ArbiterError::UnlockConfirmationRequired)
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::InspectDevice { response } => {
            let result = controller
                .inspect_device()
                .await
                .map(|device| {
                    let readiness = assess(hardware_profile, &device, &controller.snapshot());
                    HardwareInspection { device, readiness }
                })
                .map_err(ArbiterError::from);
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::ExecuteOperatorConsole {
            command,
            policy,
            response,
        } => {
            let reset_count = controller.snapshot().reset_count;
            let expert_command = policy == OperatorConsolePolicy::Expert
                && matches!(
                    operator_console::ConsoleCommand::parse(&command, true),
                    Ok(operator_console::ConsoleCommand::Raw(_))
                );
            let result = if sender_is_active(&sender.snapshot()) {
                Err(ArbiterError::MachineOperationBusy)
            } else {
                if expert_command {
                    invalidate_authorizations(safety, first_cut);
                    program_check.invalidate();
                    *pending_program_check = None;
                    *verified_z_datum = None;
                    *active_homing = None;
                    *machine_envelope = None;
                }
                let mut result = execute_operator_console(controller, &command, policy).await;
                if expert_command {
                    controller.invalidate_machine_reference("Expert console command executed");
                    let invalidated_snapshot = controller.snapshot();
                    if let Ok(exchange) = &mut result {
                        exchange.snapshot = invalidated_snapshot;
                    }
                }
                result
            };
            if controller.snapshot().reset_count != reset_count {
                invalidate_authorizations(safety, first_cut);
                program_check.invalidate();
                *pending_program_check = None;
                *verified_z_datum = None;
                *machine_envelope = None;
            }
            safety.observe(&controller.snapshot(), Instant::now());
            first_cut.observe(&controller.snapshot(), Instant::now());
            program_check.observe(&controller.snapshot(), Instant::now());
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::PreflightRealRun {
            program,
            intent,
            execution_options,
            heightmap,
            response,
        } => {
            first_cut.invalidate();
            let result = if !execution_target.supports_machine_execution() {
                Err(ArbiterError::RealRunTransportUnavailable)
            } else {
                execute_real_run_preflight(
                    controller,
                    hardware_profile,
                    program_check,
                    program,
                    RealRunPreflightContext {
                        intent,
                        execution_options,
                        heightmap: heightmap.as_ref(),
                        require_check_certificate: true,
                    },
                )
                .await
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::AuthorizeFirstCut {
            program,
            confirmation,
            heightmap,
            require_check_certificate,
            response,
        } => {
            first_cut.invalidate();
            let result = if !execution_target.supports_machine_execution() {
                Err(ArbiterError::RealRunTransportUnavailable)
            } else if !confirmation.is_complete() {
                Err(FirstCutAuthorizationError::IncompleteConfirmation {
                    missing: confirmation.missing(),
                }
                .into())
            } else {
                execute_first_cut_authorization(
                    controller,
                    hardware_profile,
                    first_cut,
                    program_check,
                    FirstCutAuthorizationContext {
                        program,
                        confirmation,
                        heightmap: heightmap.as_ref(),
                        require_check_certificate,
                    },
                )
                .await
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::StartProgramRun {
            program,
            authorization_id,
            heightmap,
            dispatch_immediately,
            response,
        } => {
            let result = if !execution_target.supports_machine_execution() {
                Err(ArbiterError::RealRunTransportUnavailable)
            } else {
                execute_authorized_program_run_start(
                    controller,
                    first_cut,
                    sender,
                    program,
                    authorization_id,
                    heightmap.as_ref(),
                )
                .await
            };
            *sender_dispatch_enabled = dispatch_immediately && result.is_ok();
            publish(snapshots, controller);
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::StartCheckRun {
            program,
            execution_options,
            heightmap,
            response,
        } => {
            *sender_dispatch_enabled = true;
            let binding = ProgramCheckBinding::from_program(&program, execution_options);
            let result = if !execution_target.supports_machine_execution() {
                Err(ArbiterError::CheckRunTransportUnavailable)
            } else {
                execute_check_run_start(
                    controller,
                    sender,
                    &program,
                    execution_options,
                    heightmap.as_ref(),
                )
                .await
            };
            if result.is_ok() {
                program_check.invalidate();
                *pending_program_check = Some(binding);
            }
            publish(snapshots, controller);
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::ResumeProgramRun { response } => {
            let result = if !execution_target.supports_machine_execution() {
                Err(ArbiterError::RealRunTransportUnavailable)
            } else {
                execute_program_run_resume(controller, sender).await
            };
            publish(snapshots, controller);
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::PauseProgramRun { response } => {
            invalidate_authorizations(safety, first_cut);
            let result = if !execution_target.supports_machine_execution() {
                Err(ArbiterError::RealRunTransportUnavailable)
            } else {
                execute_program_run_pause(controller, sender).await
            };
            publish(snapshots, controller);
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::AbortProgramRun { response } => {
            invalidate_authorizations(safety, first_cut);
            program_check.invalidate();
            *pending_program_check = None;
            let result = if !execution_target.supports_machine_execution() {
                Err(ArbiterError::RealRunTransportUnavailable)
            } else {
                execute_program_run_abort(controller, sender).await
            };
            if result.is_ok() {
                *sender_dispatch_enabled = true;
            }
            publish(snapshots, controller);
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::CompleteToolChange {
            confirmation,
            response,
        } => {
            let result = if !execution_target.supports_machine_execution() {
                Err(ArbiterError::RealRunTransportUnavailable)
            } else {
                execute_tool_change_completion(controller, sender, confirmation).await
            };
            publish(snapshots, controller);
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::Realtime { command, response } => {
            if command != RealtimeCommand::Status {
                invalidate_authorizations(safety, first_cut);
            }
            if command == RealtimeCommand::SoftReset {
                program_check.invalidate();
                *pending_program_check = None;
                *verified_z_datum = None;
                *active_homing = None;
                *machine_envelope = None;
                *active_continuous_jog = None;
            }
            let controller_result = if command == RealtimeCommand::Status
                && (sender.has_in_flight()
                    || active_z_probe.is_some()
                    || active_homing.is_some()
                    || active_continuous_jog.is_some()
                    || prepared_heightmap.is_some()
                    || active_heightmap.is_some())
            {
                controller
                    .request_interleaved_status()
                    .await
                    .map(|()| controller.snapshot())
            } else {
                controller.send_realtime(command).await
            };
            match &controller_result {
                Ok(_) if command == RealtimeCommand::SoftReset => {
                    finish_active_z_probe(active_z_probe, Err(ArbiterError::ZProbeReset));
                    *prepared_heightmap = None;
                    cancel_active_heightmap(
                        active_heightmap,
                        heightmap_snapshots,
                        "Controller reset",
                    );
                    cancel_active_sender(sender, sender_snapshots);
                }
                Ok(_)
                    if command == RealtimeCommand::FeedHold
                        && matches!(
                            sender.snapshot().state,
                            SenderState::Running | SenderState::Draining
                        ) =>
                {
                    let _ = sender.pause();
                    publish_sender(sender_snapshots, sender);
                }
                Ok(_) => {}
                Err(error) => {
                    fail_and_quarantine_physical_sender(
                        controller,
                        sender,
                        error,
                        "realtime command failed during program run",
                        sender_snapshots,
                    )
                    .await;
                }
            }
            publish(snapshots, controller);
            let _ = response.send(controller_result.map_err(ArbiterError::from));
        }
        Request::BeginSoftReset { response } => {
            invalidate_authorizations(safety, first_cut);
            let result = if controller.snapshot().connection == ConnectionState::Connected {
                Ok(safety.request_soft_reset(Instant::now()))
            } else {
                Err(ControllerError::NotReady(controller.snapshot().connection).into())
            };
            let _ = response.send(result);
        }
        Request::ConfirmSoftReset {
            challenge_id,
            response,
        } => {
            let result = match safety
                .confirm_soft_reset(challenge_id, Instant::now())
                .map_err(ArbiterError::from)
            {
                Ok(()) => {
                    first_cut.invalidate();
                    program_check.invalidate();
                    *pending_program_check = None;
                    *verified_z_datum = None;
                    *active_homing = None;
                    *machine_envelope = None;
                    *active_continuous_jog = None;
                    let controller_result =
                        controller.send_realtime(RealtimeCommand::SoftReset).await;
                    match &controller_result {
                        Ok(_) => {
                            finish_active_z_probe(active_z_probe, Err(ArbiterError::ZProbeReset));
                            *prepared_heightmap = None;
                            cancel_active_heightmap(
                                active_heightmap,
                                heightmap_snapshots,
                                "Controller reset",
                            );
                            cancel_active_sender(sender, sender_snapshots)
                        }
                        Err(error) => {
                            fail_and_quarantine_physical_sender(
                                controller,
                                sender,
                                error,
                                "soft reset could not be delivered",
                                sender_snapshots,
                            )
                            .await
                        }
                    }
                    controller_result.map_err(ArbiterError::from)
                }
                Err(error) => Err(error),
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::PrepareTestJog {
            confirmation,
            response,
        } => {
            first_cut.invalidate();
            let result = prepare_test_jog(controller, hardware_profile, safety, confirmation).await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::StepJog { request, response } => {
            first_cut.invalidate();
            let result = safety
                .consume_test_jog(
                    request.authorization_id,
                    &controller.snapshot(),
                    Instant::now(),
                )
                .map_err(ArbiterError::from);
            let result = match result {
                Ok(()) => controller
                    .step_jog(request)
                    .await
                    .map_err(ArbiterError::from),
                Err(error) => Err(error),
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::JogPadStep { request, response } => {
            first_cut.invalidate();
            let result = execute_jog_pad_step(controller, hardware_profile, safety, request).await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::StartHoming { request, response } => {
            invalidate_authorizations(safety, first_cut);
            program_check.invalidate();
            *pending_program_check = None;
            *verified_z_datum = None;
            let result = if sender_is_active(&sender.snapshot()) {
                Err(ArbiterError::MachineOperationBusy)
            } else {
                *homing_sequence = homing_sequence.saturating_add(1);
                begin_homing(controller, hardware_profile, request, *homing_sequence).await
            };
            match result {
                Ok((outcome, active)) => {
                    *active_homing = Some(active);
                    publish(snapshots, controller);
                    let _ = response.send(Ok(outcome));
                }
                Err(error) => {
                    publish(snapshots, controller);
                    let _ = response.send(Err(error));
                }
            }
        }
        Request::StartContinuousJog { request, response } => {
            first_cut.invalidate();
            let result = if sender_is_active(&sender.snapshot()) || active_continuous_jog.is_some()
            {
                Err(ArbiterError::ContinuousJogActive)
            } else {
                begin_continuous_jog(
                    controller,
                    hardware_profile,
                    safety,
                    machine_envelope.as_ref(),
                    request,
                )
                .await
            };
            match result {
                Ok((receipt, active)) => {
                    *active_continuous_jog = Some(active);
                    publish(snapshots, controller);
                    let _ = response.send(Ok(receipt));
                }
                Err(error) => {
                    publish(snapshots, controller);
                    let _ = response.send(Err(error));
                }
            }
        }
        Request::CancelJog { response } => {
            invalidate_authorizations(safety, first_cut);
            let mode = controller.snapshot().machine.mode;
            let result = if mode == MachineMode::Jog || active_continuous_jog.is_some() {
                controller
                    .send_realtime(RealtimeCommand::JogCancel)
                    .await
                    .map_err(ArbiterError::from)
            } else {
                Err(ArbiterError::JogCancelUnavailable(mode))
            };
            if result.is_ok()
                && let Some(active) = active_continuous_jog.as_mut()
            {
                active.cancel_requested = true;
            }
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::SelectWorkCoordinateSystem {
            coordinate_system,
            response,
        } => {
            invalidate_authorizations(safety, first_cut);
            *verified_z_datum = None;
            let result = if sender_is_active(&sender.snapshot()) {
                Err(ArbiterError::MachineOperationBusy)
            } else {
                execute_select_work_coordinate_system(controller, coordinate_system).await
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::SetMachineOutput { request, response } => {
            invalidate_authorizations(safety, first_cut);
            let result = if sender_is_active(&sender.snapshot()) {
                Err(ArbiterError::MachineOperationBusy)
            } else {
                execute_machine_output(controller, hardware_profile, request).await
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::ConfigureUnhomedOperation { response } => {
            invalidate_authorizations(safety, first_cut);
            let result = configure_unhomed_operation(controller).await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::SetWorkZero { request, response } => {
            invalidate_authorizations(safety, first_cut);
            let previous_z_datum = *verified_z_datum;
            let axis = request.axis;
            let result = execute_set_work_zero(controller, request).await;
            if let Ok(outcome) = &result {
                let current =
                    verified_z_datum_from_snapshot(outcome.coordinate_system, &outcome.snapshot);
                let preserve_z_datum = axis == WorkAxis::Z
                    || previous_z_datum.is_some_and(|previous| {
                        current.is_some_and(|current| {
                            previous.reset_count == current.reset_count
                                && previous.reconnect_count == current.reconnect_count
                                && previous.binding.coordinate_system
                                    == current.binding.coordinate_system
                                && (previous.binding.work_coordinate_offset.z
                                    - current.binding.work_coordinate_offset.z)
                                    .abs()
                                    <= 0.01
                        })
                    });
                *verified_z_datum = if preserve_z_datum { current } else { None };
            }
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::ReturnToWorkZero { request, response } => {
            invalidate_authorizations(safety, first_cut);
            let result = execute_return_to_work_zero(controller, hardware_profile, request).await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::ReturnToWorkOrigin { request, response } => {
            invalidate_authorizations(safety, first_cut);
            let result = if sender_is_active(&sender.snapshot()) {
                Err(ArbiterError::MachineOperationBusy)
            } else {
                execute_return_to_work_origin(controller, hardware_profile, request).await
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::ProbeZ { request, response } => {
            invalidate_authorizations(safety, first_cut);
            let sender_busy = sender_is_active(&sender.snapshot());
            let result = if active_z_probe.is_some()
                || prepared_heightmap.is_some()
                || active_heightmap.is_some()
                || sender_busy
            {
                Err(ArbiterError::MachineOperationBusy)
            } else {
                begin_z_probe(controller, hardware_profile, request).await
            };
            match result {
                Ok(started) => {
                    *active_z_probe = Some(ActiveZProbe {
                        request: started.request,
                        coordinate_system: started.coordinate_system,
                        restore_modal: started.restore_modal,
                        command: started.command,
                        response,
                    });
                    publish(snapshots, controller);
                    return;
                }
                Err(error) => {
                    let _ = response.send(Err(error));
                }
            }
            publish(snapshots, controller);
        }
        Request::PrepareHeightmap { request, response } => {
            invalidate_authorizations(safety, first_cut);
            let sender_busy = sender_is_active(&sender.snapshot());
            let result = if active_z_probe.is_some()
                || prepared_heightmap.is_some()
                || active_heightmap.is_some()
                || sender_busy
            {
                Err(ArbiterError::MachineOperationBusy)
            } else {
                *heightmap_sequence = heightmap_sequence.saturating_add(1);
                begin_heightmap(
                    controller,
                    hardware_profile,
                    request,
                    *heightmap_sequence,
                    *verified_z_datum,
                )
                .await
            };
            match result {
                Ok(active) => {
                    let snapshot = heightmap_operation_snapshot(&active);
                    *prepared_heightmap = Some(active);
                    let _ = response.send(Ok(snapshot));
                }
                Err(error) => {
                    let _ = response.send(Err(error));
                }
            }
            publish(snapshots, controller);
        }
        Request::PrepareResumeHeightmap {
            map,
            request,
            response,
        } => {
            invalidate_authorizations(safety, first_cut);
            let sender_busy = sender_is_active(&sender.snapshot());
            let result = if active_z_probe.is_some()
                || prepared_heightmap.is_some()
                || active_heightmap.is_some()
                || sender_busy
            {
                Err(ArbiterError::MachineOperationBusy)
            } else {
                *heightmap_sequence = heightmap_sequence.saturating_add(1);
                begin_resumed_heightmap(
                    controller,
                    hardware_profile,
                    map,
                    request,
                    *heightmap_sequence,
                )
                .await
            };
            match result {
                Ok(active) => {
                    let snapshot = heightmap_operation_snapshot(&active);
                    *prepared_heightmap = Some(active);
                    let _ = response.send(Ok(snapshot));
                }
                Err(error) => {
                    let _ = response.send(Err(error));
                }
            }
            publish(snapshots, controller);
        }
        Request::CommitPreparedHeightmap {
            operation_sequence,
            response,
        } => {
            let result = match prepared_heightmap.as_ref() {
                Some(prepared) if prepared.operation_sequence != operation_sequence => {
                    Err(ArbiterError::PreparedHeightmapMismatch {
                        expected: operation_sequence,
                        actual: prepared.operation_sequence,
                    })
                }
                Some(_) => match prepared_heightmap.take() {
                    Some(active) => {
                        let snapshot = heightmap_operation_snapshot(&active);
                        *active_heightmap = Some(active);
                        let _ = heightmap_snapshots.send(snapshot.clone());
                        Ok(snapshot)
                    }
                    None => Err(ArbiterError::PreparedHeightmapUnavailable),
                },
                None => Err(ArbiterError::PreparedHeightmapUnavailable),
            };
            let _ = response.send(result);
        }
        Request::DiscardPreparedHeightmap {
            operation_sequence,
            response,
        } => {
            let result = match prepared_heightmap.as_ref() {
                Some(prepared) if prepared.operation_sequence != operation_sequence => {
                    Err(ArbiterError::PreparedHeightmapMismatch {
                        expected: operation_sequence,
                        actual: prepared.operation_sequence,
                    })
                }
                Some(_) => {
                    *prepared_heightmap = None;
                    Ok(())
                }
                None => Err(ArbiterError::PreparedHeightmapUnavailable),
            };
            let _ = response.send(result);
        }
        Request::PauseHeightmap { response } => {
            let result = async {
                let active = active_heightmap
                    .as_mut()
                    .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
                controller.send_realtime(RealtimeCommand::FeedHold).await?;
                active.paused = true;
                let snapshot = heightmap_operation_snapshot(active);
                let _ = heightmap_snapshots.send(snapshot.clone());
                Ok(snapshot)
            }
            .await;
            let _ = response.send(result);
        }
        Request::ResumeHeightmap { response } => {
            let result = async {
                let active = active_heightmap
                    .as_mut()
                    .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
                let state = controller.refresh_status().await?;
                if state.machine.mode == MachineMode::Hold {
                    controller
                        .send_realtime(RealtimeCommand::CycleStart)
                        .await?;
                } else if state.machine.mode != MachineMode::Idle {
                    ensure_stable_idle(&state)?;
                }
                active.paused = false;
                let snapshot = heightmap_operation_snapshot(active);
                let _ = heightmap_snapshots.send(snapshot.clone());
                Ok(snapshot)
            }
            .await;
            let _ = response.send(result);
        }
        Request::CancelHeightmap { response } => {
            let result = async {
                let active = active_heightmap
                    .take()
                    .ok_or(ArbiterError::HeightmapOperationUnavailable)?;
                let abort = controller.abort_program_stream().await;
                let mut snapshot = heightmap_operation_snapshot(&active);
                snapshot.current_sequence = None;
                match &abort {
                    Ok(_) => {
                        snapshot.state = HeightmapOperationState::Cancelled;
                        snapshot.error = Some("Stopped by operator".to_owned());
                    }
                    Err(error) => {
                        snapshot.state = HeightmapOperationState::Failed;
                        snapshot.error = Some(format!(
                            "Stop delivery failed; disconnect controller power if motion continues: {error}"
                        ));
                    }
                }
                let _ = heightmap_snapshots.send(snapshot.clone());
                publish(snapshots, controller);
                abort.map(|_| snapshot).map_err(ArbiterError::from)
            }
            .await;
            let _ = response.send(result);
        }
        Request::StartDryRun { plan, response } => {
            *sender_dispatch_enabled = true;
            let result = if *execution_target != ExecutionTarget::Mock {
                Err(ArbiterError::DryRunTransportUnavailable)
            } else {
                ensure_stable_idle(&controller.snapshot()).and_then(|()| {
                    sender.load(plan)?;
                    sender.start().map_err(ArbiterError::from)
                })
            };
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::PauseDryRun { response } => {
            let result = if *execution_target == ExecutionTarget::Mock {
                sender.pause().map_err(ArbiterError::from)
            } else {
                Err(ArbiterError::DryRunTransportUnavailable)
            };
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::ResumeDryRun { response } => {
            let result = if *execution_target == ExecutionTarget::Mock {
                ensure_sender_dispatch_ready(sender, &controller.snapshot())
                    .and_then(|()| sender.resume().map_err(ArbiterError::from))
            } else {
                Err(ArbiterError::DryRunTransportUnavailable)
            };
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::CancelDryRun { response } => {
            let result = if matches!(
                sender.snapshot().mode,
                Some(millo_sender::SenderMode::AirRun | millo_sender::SenderMode::CutRun)
            ) {
                Err(ArbiterError::ProgramRunStopRequiresReset)
            } else {
                sender.cancel().map_err(ArbiterError::from)
            };
            if result.is_ok() {
                settle_program_check(controller, sender, program_check, pending_program_check)
                    .await;
                publish(snapshots, controller);
            }
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::CommitPreparedProgramRun {
            run_sequence,
            response,
        } => {
            let active = sender.snapshot();
            let result = if active.run_sequence != run_sequence {
                Err(ArbiterError::PreparedRunMismatch {
                    expected: run_sequence,
                    actual: active.run_sequence,
                })
            } else if active.state != SenderState::Running {
                Err(ArbiterError::PreparedRunUnavailable(active.state))
            } else if *sender_dispatch_enabled {
                Err(ArbiterError::PreparedRunAlreadyCommitted)
            } else {
                *sender_dispatch_enabled = true;
                Ok(active)
            };
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::DiscardPreparedProgramRun {
            run_sequence,
            response,
        } => {
            let active = sender.snapshot();
            let result = if active.run_sequence != run_sequence {
                Err(ArbiterError::PreparedRunMismatch {
                    expected: run_sequence,
                    actual: active.run_sequence,
                })
            } else if active.state != SenderState::Running {
                Err(ArbiterError::PreparedRunUnavailable(active.state))
            } else if *sender_dispatch_enabled {
                Err(ArbiterError::PreparedRunAlreadyCommitted)
            } else {
                sender.cancel().map_err(ArbiterError::from)
            };
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
    }
}

pub(super) async fn execute_operator_console(
    controller: &mut Controller<BoxedTransport>,
    input: &str,
    policy: OperatorConsolePolicy,
) -> Result<OperatorConsoleExchange, ArbiterError> {
    match operator_console::ConsoleCommand::parse(input, policy == OperatorConsolePolicy::Expert)? {
        operator_console::ConsoleCommand::Status => {
            let snapshot = controller.refresh_status().await?;
            Ok(operator_console::status_exchange(snapshot))
        }
        operator_console::ConsoleCommand::Query { query, kind } => {
            let mode = controller.snapshot().machine.mode;
            if !matches!(mode, MachineMode::Idle | MachineMode::Alarm) {
                return Err(ArbiterError::OperatorConsoleQueryUnavailable(mode));
            }
            let response = controller.query_device(query).await?;
            Ok(operator_console::query_exchange(
                kind,
                response,
                controller.snapshot(),
            ))
        }
        operator_console::ConsoleCommand::Raw(command) => {
            let mode = controller.refresh_status().await?.machine.mode;
            if !matches!(mode, MachineMode::Idle | MachineMode::Alarm) {
                return Err(ArbiterError::OperatorConsoleQueryUnavailable(mode));
            }
            let response = controller.execute_console_line(&command).await?;
            let snapshot = controller.refresh_status().await?;
            Ok(operator_console::raw_exchange(response, snapshot))
        }
    }
}
