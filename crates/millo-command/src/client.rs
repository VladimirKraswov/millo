use super::*;

#[derive(Clone)]
pub struct CommandArbiter {
    requests: mpsc::Sender<Request>,
    snapshots: watch::Receiver<ControllerSnapshot>,
    sender_snapshots: watch::Receiver<SenderSnapshot>,
    heightmap_snapshots: watch::Receiver<HeightmapOperationSnapshot>,
}

impl CommandArbiter {
    pub fn new(
        transport: BoxedTransport,
        config: ControllerConfig,
        hardware_profile: HardwareProfile,
    ) -> (Self, impl Future<Output = ()> + Send + 'static) {
        Self::new_with_execution_target(
            transport,
            config,
            hardware_profile,
            ExecutionTarget::Disabled,
        )
    }

    pub fn new_with_execution_target(
        transport: BoxedTransport,
        config: ControllerConfig,
        hardware_profile: HardwareProfile,
        execution_target: ExecutionTarget,
    ) -> (Self, impl Future<Output = ()> + Send + 'static) {
        let controller = Controller::with_config(transport, config);
        let initial_snapshot = controller.snapshot();
        let sender = Sender::default();
        let (requests, request_rx) = mpsc::channel(REQUEST_CAPACITY);
        let (snapshot_tx, snapshots) = watch::channel(initial_snapshot);
        let (sender_snapshot_tx, sender_snapshots) = watch::channel(sender.snapshot());
        let (heightmap_snapshot_tx, heightmap_snapshots) =
            watch::channel(HeightmapOperationSnapshot::default());
        let actor = ActorState {
            controller,
            config,
            hardware_profile,
            execution_target,
            sender,
            sender_dispatch_enabled: true,
            safety: SafetyManager::default(),
            first_cut: FirstCutGate::default(),
            program_check: ProgramCheckGate::default(),
            pending_program_check: None,
            verified_z_datum: None,
            active_homing: None,
            homing_sequence: 0,
            machine_envelope: None,
            active_continuous_jog: None,
            active_z_probe: None,
            prepared_heightmap: None,
            active_heightmap: None,
            heightmap_sequence: 0,
            snapshots: snapshot_tx,
            sender_snapshots: sender_snapshot_tx,
            heightmap_snapshots: heightmap_snapshot_tx,
        };
        let worker = run_actor(actor, request_rx);

        (
            Self {
                requests,
                snapshots,
                sender_snapshots,
                heightmap_snapshots,
            },
            worker,
        )
    }

    pub fn snapshot(&self) -> ControllerSnapshot {
        self.snapshots.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<ControllerSnapshot> {
        self.snapshots.clone()
    }

    pub fn sender_snapshot(&self) -> SenderSnapshot {
        self.sender_snapshots.borrow().clone()
    }

    pub fn subscribe_sender(&self) -> watch::Receiver<SenderSnapshot> {
        self.sender_snapshots.clone()
    }

    pub fn heightmap_snapshot(&self) -> HeightmapOperationSnapshot {
        self.heightmap_snapshots.borrow().clone()
    }

    pub fn subscribe_heightmap(&self) -> watch::Receiver<HeightmapOperationSnapshot> {
        self.heightmap_snapshots.clone()
    }

    pub async fn replace_transport(
        &self,
        transport: BoxedTransport,
    ) -> Result<ControllerSnapshot, ArbiterError> {
        self.replace_transport_with_execution_target(transport, ExecutionTarget::Disabled)
            .await
    }

    pub async fn replace_transport_with_execution_target(
        &self,
        transport: BoxedTransport,
        execution_target: ExecutionTarget,
    ) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::ReplaceTransport {
            transport,
            execution_target,
            response,
        })
        .await
    }

    pub async fn connect(&self) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::Connect { response }).await
    }

    pub async fn set_hardware_profile(
        &self,
        profile: HardwareProfile,
    ) -> Result<HardwareProfile, ArbiterError> {
        self.call(|response| Request::SetHardwareProfile { profile, response })
            .await
    }

    pub async fn bind_hardware_profile(
        &self,
        profile: HardwareProfile,
    ) -> Result<HardwareProfile, ArbiterError> {
        self.call(|response| Request::BindHardwareProfile { profile, response })
            .await
    }

    pub async fn update_controller_setting(
        &self,
        request: ControllerSettingEditRequest,
    ) -> Result<VerifiedSettingUpdate, ArbiterError> {
        self.call(|response| Request::UpdateControllerSetting { request, response })
            .await
    }

    pub async fn disconnect(&self) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::Disconnect { response }).await
    }

    pub async fn refresh_status(&self) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::RefreshStatus { response })
            .await
    }

    pub async fn acknowledge_reset(&self) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::AcknowledgeReset { response })
            .await
    }

    pub async fn unlock_alarm(
        &self,
        operator_confirmed: bool,
    ) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::UnlockAlarm {
            operator_confirmed,
            response,
        })
        .await
    }

    pub async fn inspect_device(&self) -> Result<HardwareInspection, ArbiterError> {
        self.call(|response| Request::InspectDevice { response })
            .await
    }

    pub async fn execute_operator_console(
        &self,
        command: String,
        policy: OperatorConsolePolicy,
    ) -> Result<OperatorConsoleExchange, ArbiterError> {
        self.call(|response| Request::ExecuteOperatorConsole {
            command,
            policy,
            response,
        })
        .await
    }

    pub async fn preflight_real_run(
        &self,
        program: GcodeProgram,
        intent: ProgramRunIntent,
    ) -> Result<RunPreflightReport, ArbiterError> {
        self.preflight_real_run_with_options(program, intent, ProgramExecutionOptions::default())
            .await
    }

    pub async fn preflight_real_run_with_options(
        &self,
        program: GcodeProgram,
        intent: ProgramRunIntent,
        execution_options: ProgramExecutionOptions,
    ) -> Result<RunPreflightReport, ArbiterError> {
        self.preflight_real_run_with_heightmap(program, intent, execution_options, None)
            .await
    }

    pub async fn preflight_real_run_with_heightmap(
        &self,
        program: GcodeProgram,
        intent: ProgramRunIntent,
        execution_options: ProgramExecutionOptions,
        heightmap: Option<Heightmap>,
    ) -> Result<RunPreflightReport, ArbiterError> {
        self.call(|response| Request::PreflightRealRun {
            program,
            intent,
            execution_options,
            heightmap,
            response,
        })
        .await
    }

    pub async fn authorize_first_cut(
        &self,
        program: GcodeProgram,
        confirmation: FirstCutConfirmation,
    ) -> Result<FirstCutPreparation, ArbiterError> {
        self.authorize_first_cut_with_heightmap(program, confirmation, None)
            .await
    }

    pub async fn authorize_first_cut_with_heightmap(
        &self,
        program: GcodeProgram,
        confirmation: FirstCutConfirmation,
        heightmap: Option<Heightmap>,
    ) -> Result<FirstCutPreparation, ArbiterError> {
        self.call(|response| Request::AuthorizeFirstCut {
            program,
            confirmation,
            heightmap,
            require_check_certificate: true,
            response,
        })
        .await
    }

    #[cfg(test)]
    pub(super) async fn authorize_first_cut_fixture(
        &self,
        program: GcodeProgram,
        confirmation: FirstCutConfirmation,
    ) -> Result<FirstCutPreparation, ArbiterError> {
        self.call(|response| Request::AuthorizeFirstCut {
            program,
            confirmation,
            heightmap: None,
            require_check_certificate: false,
            response,
        })
        .await
    }

    pub async fn start_program_run(
        &self,
        program: GcodeProgram,
        authorization_id: u64,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.start_program_run_with_heightmap(program, authorization_id, None, true)
            .await
    }

    pub async fn start_program_run_with_heightmap(
        &self,
        program: GcodeProgram,
        authorization_id: u64,
        heightmap: Option<Heightmap>,
        dispatch_immediately: bool,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::StartProgramRun {
            program,
            authorization_id,
            heightmap,
            dispatch_immediately,
            response,
        })
        .await
    }

    pub async fn prepare_program_run(
        &self,
        program: GcodeProgram,
        authorization_id: u64,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.start_program_run_with_heightmap(program, authorization_id, None, false)
            .await
    }

    pub async fn prepare_program_run_with_heightmap(
        &self,
        program: GcodeProgram,
        authorization_id: u64,
        heightmap: Option<Heightmap>,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.start_program_run_with_heightmap(program, authorization_id, heightmap, false)
            .await
    }

    pub async fn commit_prepared_program_run(
        &self,
        run_sequence: u64,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::CommitPreparedProgramRun {
            run_sequence,
            response,
        })
        .await
    }

    pub async fn discard_prepared_program_run(
        &self,
        run_sequence: u64,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::DiscardPreparedProgramRun {
            run_sequence,
            response,
        })
        .await
    }

    pub async fn start_check_run(
        &self,
        program: GcodeProgram,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.start_check_run_with_options(program, ProgramExecutionOptions::default())
            .await
    }

    pub async fn start_check_run_with_options(
        &self,
        program: GcodeProgram,
        execution_options: ProgramExecutionOptions,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.start_check_run_with_heightmap(program, execution_options, None)
            .await
    }

    pub async fn start_check_run_with_heightmap(
        &self,
        program: GcodeProgram,
        execution_options: ProgramExecutionOptions,
        heightmap: Option<Heightmap>,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::StartCheckRun {
            program,
            execution_options,
            heightmap,
            response,
        })
        .await
    }

    pub async fn resume_program_run(&self) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::ResumeProgramRun { response })
            .await
    }

    pub async fn pause_program_run(&self) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::PauseProgramRun { response })
            .await
    }

    pub async fn abort_program_run(&self) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::AbortProgramRun { response })
            .await
    }

    pub async fn complete_tool_change(
        &self,
        confirmation: ToolChangeConfirmation,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::CompleteToolChange {
            confirmation,
            response,
        })
        .await
    }

    pub async fn feed_hold(&self) -> Result<ControllerSnapshot, ArbiterError> {
        self.send_realtime(RealtimeCommand::FeedHold).await
    }

    pub async fn adjust_feed_override(
        &self,
        adjustment: OverrideAdjustment,
    ) -> Result<ControllerSnapshot, ArbiterError> {
        self.send_realtime(RealtimeCommand::FeedOverride(adjustment))
            .await
    }

    pub async fn set_rapid_override(
        &self,
        target: RapidOverrideTarget,
    ) -> Result<ControllerSnapshot, ArbiterError> {
        self.send_realtime(RealtimeCommand::RapidOverride(target))
            .await
    }

    pub async fn adjust_spindle_override(
        &self,
        adjustment: OverrideAdjustment,
    ) -> Result<ControllerSnapshot, ArbiterError> {
        self.send_realtime(RealtimeCommand::SpindleOverride(adjustment))
            .await
    }

    pub async fn request_soft_reset(&self) -> Result<ResetChallenge, ArbiterError> {
        self.call(|response| Request::BeginSoftReset { response })
            .await
    }

    pub async fn confirm_soft_reset(
        &self,
        challenge_id: u64,
    ) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::ConfirmSoftReset {
            challenge_id,
            response,
        })
        .await
    }

    pub async fn prepare_test_jog(
        &self,
        confirmation: OperatorConfirmation,
    ) -> Result<TestJogPreparation, ArbiterError> {
        self.call(|response| Request::PrepareTestJog {
            confirmation,
            response,
        })
        .await
    }

    pub async fn step_jog(&self, request: StepJogRequest) -> Result<StepJogReceipt, ArbiterError> {
        self.call(|response| Request::StepJog { request, response })
            .await
    }

    pub async fn jog_pad_step(
        &self,
        request: JogPadStepRequest,
    ) -> Result<JogPadStepOutcome, ArbiterError> {
        self.call(|response| Request::JogPadStep { request, response })
            .await
    }

    pub async fn start_homing(
        &self,
        request: HomingRequest,
    ) -> Result<HomingStartOutcome, ArbiterError> {
        self.call(|response| Request::StartHoming { request, response })
            .await
    }

    pub async fn start_continuous_jog(
        &self,
        request: ContinuousJogRequest,
    ) -> Result<ContinuousJogReceipt, ArbiterError> {
        self.call(|response| Request::StartContinuousJog { request, response })
            .await
    }

    pub async fn cancel_jog(&self) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::CancelJog { response }).await
    }

    pub async fn select_work_coordinate_system(
        &self,
        coordinate_system: WorkCoordinateSystem,
    ) -> Result<WorkCoordinateSelectionOutcome, ArbiterError> {
        self.call(|response| Request::SelectWorkCoordinateSystem {
            coordinate_system,
            response,
        })
        .await
    }

    pub async fn set_machine_output(
        &self,
        request: MachineOutputRequest,
    ) -> Result<MachineOutputOutcome, ArbiterError> {
        self.call(|response| Request::SetMachineOutput { request, response })
            .await
    }

    pub async fn configure_unhomed_operation(&self) -> Result<UnhomedConfiguration, ArbiterError> {
        self.call(|response| Request::ConfigureUnhomedOperation { response })
            .await
    }

    pub async fn set_work_zero(
        &self,
        request: WorkZeroRequest,
    ) -> Result<WorkZeroOutcome, ArbiterError> {
        self.call(|response| Request::SetWorkZero { request, response })
            .await
    }

    pub async fn return_to_work_zero(
        &self,
        request: ReturnToWorkZeroRequest,
    ) -> Result<ReturnToWorkZeroOutcome, ArbiterError> {
        self.call(|response| Request::ReturnToWorkZero { request, response })
            .await
    }

    pub async fn return_to_work_origin(
        &self,
        request: ReturnToWorkOriginRequest,
    ) -> Result<ReturnToWorkOriginOutcome, ArbiterError> {
        self.call(|response| Request::ReturnToWorkOrigin { request, response })
            .await
    }

    pub async fn probe_z(&self, request: ZProbeRequest) -> Result<ZProbeOutcome, ArbiterError> {
        match self
            .call(|response| Request::ProbeZ { request, response })
            .await
        {
            Err(error) if probe_start_can_settle(&error) => {
                self.wait_for_probe_start_idle().await?;
                self.call(|response| Request::ProbeZ { request, response })
                    .await
            }
            result => result,
        }
    }

    #[cfg(test)]
    pub(super) async fn start_heightmap(
        &self,
        request: HeightmapStartRequest,
    ) -> Result<HeightmapOperationSnapshot, ArbiterError> {
        let prepared = self.prepare_heightmap(request).await?;
        match self
            .commit_prepared_heightmap(prepared.operation_sequence)
            .await
        {
            Ok(snapshot) => Ok(snapshot),
            Err(error) => {
                let _ = self
                    .discard_prepared_heightmap(prepared.operation_sequence)
                    .await;
                Err(error)
            }
        }
    }

    pub async fn prepare_heightmap(
        &self,
        request: HeightmapStartRequest,
    ) -> Result<HeightmapOperationSnapshot, ArbiterError> {
        match self
            .call(|response| Request::PrepareHeightmap {
                request: request.clone(),
                response,
            })
            .await
        {
            Err(error) if probe_start_can_settle(&error) => {
                self.wait_for_probe_start_idle().await?;
                self.call(|response| Request::PrepareHeightmap { request, response })
                    .await
            }
            result => result,
        }
    }

    pub async fn prepare_resume_heightmap(
        &self,
        map: Heightmap,
        request: HeightmapResumeRequest,
    ) -> Result<HeightmapOperationSnapshot, ArbiterError> {
        match self
            .call(|response| Request::PrepareResumeHeightmap {
                map: map.clone(),
                request,
                response,
            })
            .await
        {
            Err(error) if probe_start_can_settle(&error) => {
                self.wait_for_probe_start_idle().await?;
                self.call(|response| Request::PrepareResumeHeightmap {
                    map,
                    request,
                    response,
                })
                .await
            }
            result => result,
        }
    }

    async fn wait_for_probe_start_idle(&self) -> Result<ControllerSnapshot, ArbiterError> {
        let started = Instant::now();
        loop {
            if sender_is_active(&self.sender_snapshot())
                || matches!(
                    self.heightmap_snapshot().state,
                    HeightmapOperationState::Running | HeightmapOperationState::Paused
                )
            {
                return Err(ArbiterError::MachineOperationBusy);
            }

            let current = self.snapshot();
            if current.connection != ConnectionState::Connected
                || current.alarm.is_some()
                || current.reset_notice.is_some()
            {
                return Err(probe_start_blocked(&current));
            }

            let snapshot = self.refresh_status().await?;
            if snapshot.connection != ConnectionState::Connected
                || snapshot.alarm.is_some()
                || snapshot.reset_notice.is_some()
            {
                return Err(probe_start_blocked(&snapshot));
            }

            match snapshot.machine.mode {
                MachineMode::Idle => return Ok(snapshot),
                MachineMode::Run | MachineMode::Jog
                    if started.elapsed() < PROBE_START_SETTLE_TIMEOUT =>
                {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                MachineMode::Run | MachineMode::Jog => {
                    return Err(ArbiterError::ProbeStartSettleTimeout {
                        timeout_ms: PROBE_START_SETTLE_TIMEOUT
                            .as_millis()
                            .try_into()
                            .unwrap_or(u64::MAX),
                        last_mode: snapshot.machine.mode,
                    });
                }
                _ => return Err(probe_start_blocked(&snapshot)),
            }
        }
    }

    pub async fn commit_prepared_heightmap(
        &self,
        operation_sequence: u64,
    ) -> Result<HeightmapOperationSnapshot, ArbiterError> {
        self.call(|response| Request::CommitPreparedHeightmap {
            operation_sequence,
            response,
        })
        .await
    }

    pub async fn discard_prepared_heightmap(
        &self,
        operation_sequence: u64,
    ) -> Result<(), ArbiterError> {
        self.call(|response| Request::DiscardPreparedHeightmap {
            operation_sequence,
            response,
        })
        .await
    }

    pub async fn pause_heightmap(&self) -> Result<HeightmapOperationSnapshot, ArbiterError> {
        self.call(|response| Request::PauseHeightmap { response })
            .await
    }

    pub async fn resume_heightmap(&self) -> Result<HeightmapOperationSnapshot, ArbiterError> {
        self.call(|response| Request::ResumeHeightmap { response })
            .await
    }

    pub async fn cancel_heightmap(&self) -> Result<HeightmapOperationSnapshot, ArbiterError> {
        self.call(|response| Request::CancelHeightmap { response })
            .await
    }

    pub async fn start_dry_run(&self, plan: DryRunPlan) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::StartDryRun { plan, response })
            .await
    }

    pub async fn pause_dry_run(&self) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::PauseDryRun { response })
            .await
    }

    pub async fn resume_dry_run(&self) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::ResumeDryRun { response })
            .await
    }

    pub async fn cancel_dry_run(&self) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::CancelDryRun { response })
            .await
    }

    #[cfg(test)]
    pub(super) async fn start_serial_run_fixture(
        &self,
        program: GcodeProgram,
        authorization_id: u64,
        dispatch_immediately: bool,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::StartProgramRun {
            program,
            authorization_id,
            heightmap: None,
            dispatch_immediately,
            response,
        })
        .await
    }

    #[cfg(test)]
    pub(super) async fn release_serial_run_fixture(&self) -> Result<SenderSnapshot, ArbiterError> {
        self.commit_prepared_program_run(self.sender_snapshot().run_sequence)
            .await
    }

    pub(super) async fn send_realtime(
        &self,
        command: RealtimeCommand,
    ) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::Realtime { command, response })
            .await
    }

    async fn call<T>(
        &self,
        request: impl FnOnce(oneshot::Sender<Result<T, ArbiterError>>) -> Request,
    ) -> Result<T, ArbiterError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.requests
            .send(request(response_tx))
            .await
            .map_err(|_| ArbiterError::Closed)?;
        response_rx
            .await
            .map_err(|_| ArbiterError::ResponseDropped)?
    }
}
