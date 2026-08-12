use std::{
    future::Future,
    time::{Duration, Instant},
};

use millo_controller::{
    Controller, ControllerConfig, ControllerError, ProgramResponsePoll, RealtimeCommand,
    UnhomedSetting,
};
use millo_domain::{
    CommandCompletion, CommandResponse, ConnectionState, ControllerSnapshot, DeviceInspection,
    HardwareInspection, HardwareProfile, JogPadStepOutcome, JogPadStepRequest, MachineMode,
    OperatorConfirmation, OverrideAdjustment, Position, RapidOverrideTarget, ResetChallenge,
    ReturnToWorkZeroOutcome, ReturnToWorkZeroRequest, StepJogReceipt, StepJogRequest,
    TestJogPreparation, WorkAxis, WorkCoordinateSystem, WorkZeroOutcome, WorkZeroRequest,
};
use millo_dry_run::{
    DryRunLineKind, DryRunPlan, DryRunPolicyError, ProgramExecutionOptions, ProgramRunPolicy,
    build_program_run_plan_with_options,
};
use millo_gcode::GcodeProgram;
use millo_grbl::{
    MAX_STEP_JOG_DISTANCE_MM, MAX_STEP_JOG_FEED_MM_PER_MIN, MIN_STEP_JOG_DISTANCE_MM,
    MIN_STEP_JOG_FEED_MM_PER_MIN, active_work_coordinate_system, build_device_inspection,
    work_coordinate_parameter,
};
use millo_readiness::assess;
use millo_run::program_fingerprint;
use millo_run::{
    FirstCutAuthorizationError, FirstCutConfirmation, FirstCutGate, FirstCutPreparation,
    ProgramCheckBinding, ProgramCheckCertificateError, ProgramCheckGate, ProgramRunIntent,
    RunPreflightCheck, RunPreflightLevel, RunPreflightReport, ToolChangeConfirmation,
    assess_real_run_preflight_with_options,
};
use millo_safety::{SafetyError, SafetyManager};
use millo_sender::{
    Sender, SenderError, SenderFailure, SenderFailureKind, SenderSnapshot, SenderState,
    usable_rx_buffer_capacity,
};
use millo_settings::{
    ControllerSettingEditRequest, SettingsError, VerifiedSettingUpdate, setting_values_equal,
    validate_setting_edit,
};
use millo_transport::{BoxedTransport, TransportError};
use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::{MissedTickBehavior, interval},
};

const REQUEST_CAPACITY: usize = 32;
const WORK_ZERO_TOLERANCE_MM: f64 = 0.002;
const SENDER_RESPONSE_SLICE: Duration = Duration::from_millis(10);

#[derive(Debug, Error)]
pub enum ArbiterError {
    #[error(transparent)]
    Controller(#[from] ControllerError),
    #[error(transparent)]
    Safety(#[from] SafetyError),
    #[error("command arbiter is no longer running")]
    Closed,
    #[error("command arbiter dropped a response")]
    ResponseDropped,
    #[error("jog cancel requires Jog state, current mode is {0:?}")]
    JogCancelUnavailable(MachineMode),
    #[error("unhomed configuration verification failed: {0}")]
    ConfigurationVerification(String),
    #[error("jog distance must be between 0.01 and 100000 mm")]
    JogPadDistanceOutOfRange,
    #[error("jog feed must be between 10 and 100000 mm/min")]
    JogPadFeedOutOfRange,
    #[error("jog feed {requested:.0} mm/min exceeds {axis:?} maximum rate {maximum:.0} mm/min")]
    JogPadFeedExceedsAxisRate {
        axis: millo_domain::JogAxis,
        requested: f64,
        maximum: f64,
    },
    #[error("jog distance {requested:.3} mm exceeds the {axis:?} profile limit {maximum:.3} mm")]
    JogPadDistanceExceedsProfile {
        axis: millo_domain::JogAxis,
        requested: f64,
        maximum: f64,
    },
    #[error("work zero requires explicit operator position confirmation")]
    WorkZeroConfirmationRequired,
    #[error("active work coordinate system is not one of G54-G59")]
    ActiveWorkCoordinateSystemUnavailable,
    #[error("alarm unlock requires explicit operator confirmation")]
    UnlockConfirmationRequired,
    #[error("work zero verification failed: {0}")]
    WorkZeroVerification(String),
    #[error("current work position is unavailable")]
    WorkPositionUnavailable,
    #[error("raise work Z above zero before returning {0:?} to zero")]
    ReturnToZeroNeedsClearance(WorkAxis),
    #[error("return distance {requested:.3} mm exceeds the {axis:?} travel {maximum:.3} mm")]
    ReturnToZeroDistanceExceedsProfile {
        axis: WorkAxis,
        requested: f64,
        maximum: f64,
    },
    #[error(transparent)]
    Sender(#[from] SenderError),
    #[error("dry run is disabled for the active transport")]
    DryRunTransportUnavailable,
    #[error("real-run preflight requires the serial transport target")]
    RealRunTransportUnavailable,
    #[error("GRBL Check run requires the serial transport target")]
    CheckRunTransportUnavailable,
    #[error("program run can resume only from GRBL Hold or Idle, current mode is {0:?}")]
    ProgramRunResumeUnavailable(MachineMode),
    #[error("physical program pause is unavailable while sender is {0:?}")]
    ProgramRunPauseUnavailable(SenderState),
    #[error("physical program stop is unavailable while sender is {0:?}")]
    ProgramRunStopUnavailable(SenderState),
    #[error("tool change can be completed only at an active M6 barrier, sender is {0:?}")]
    ToolChangeUnavailable(SenderState),
    #[error("tool-change confirmation does not match the active source line or requested tool")]
    ToolChangeMismatch,
    #[error("tool-change confirmation is incomplete: {0:?}")]
    ToolChangeConfirmationIncomplete(Vec<&'static str>),
    #[error("tool change can continue only from fresh GRBL Idle, current mode is {0:?}")]
    ToolChangeControllerUnavailable(MachineMode),
    #[error("a physical program run can be stopped only with Feed Hold followed by Soft Reset")]
    ProgramRunStopRequiresReset,
    #[error("prepared program run {expected} does not match active run {actual}")]
    PreparedRunMismatch { expected: u64, actual: u64 },
    #[error("prepared program run is unavailable while sender is {0:?}")]
    PreparedRunUnavailable(SenderState),
    #[error("prepared program run has already been committed to dispatch")]
    PreparedRunAlreadyCommitted,
    #[error(transparent)]
    FirstCut(#[from] FirstCutAuthorizationError),
    #[error(transparent)]
    RunPolicy(#[from] DryRunPolicyError),
    #[error("machine profile can be changed only while disconnected, current state is {0:?}")]
    ProfileChangeUnavailable(ConnectionState),
    #[error("transport can be replaced only while disconnected, current state is {0:?}")]
    TransportReplacementUnavailable(ConnectionState),
    #[error("connect requires a disconnected controller, current state is {0:?}")]
    ConnectUnavailable(ConnectionState),
    #[error(transparent)]
    Settings(#[from] SettingsError),
    #[error(
        "controller setting verification failed for {key}: requested {requested}, read {stored}"
    )]
    SettingVerification {
        key: String,
        requested: String,
        stored: String,
    },
    #[error("validated controller setting {0} disappeared from the inspection snapshot")]
    SettingSourceMissing(String),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ExecutionTarget {
    #[default]
    Disabled,
    Mock,
    Serial,
}

#[derive(Debug, Clone)]
pub struct UnhomedConfiguration {
    pub before: DeviceInspection,
    pub after: DeviceInspection,
    pub writes: Vec<CommandResponse>,
}

#[derive(Clone)]
pub struct CommandArbiter {
    requests: mpsc::Sender<Request>,
    snapshots: watch::Receiver<ControllerSnapshot>,
    sender_snapshots: watch::Receiver<SenderSnapshot>,
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
            snapshots: snapshot_tx,
            sender_snapshots: sender_snapshot_tx,
        };
        let worker = run_actor(actor, request_rx);

        (
            Self {
                requests,
                snapshots,
                sender_snapshots,
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
        self.call(|response| Request::PreflightRealRun {
            program,
            intent,
            execution_options,
            response,
        })
        .await
    }

    pub async fn authorize_first_cut(
        &self,
        program: GcodeProgram,
        confirmation: FirstCutConfirmation,
    ) -> Result<FirstCutPreparation, ArbiterError> {
        self.call(|response| Request::AuthorizeFirstCut {
            program,
            confirmation,
            require_check_certificate: true,
            response,
        })
        .await
    }

    #[cfg(test)]
    async fn authorize_first_cut_fixture(
        &self,
        program: GcodeProgram,
        confirmation: FirstCutConfirmation,
    ) -> Result<FirstCutPreparation, ArbiterError> {
        self.call(|response| Request::AuthorizeFirstCut {
            program,
            confirmation,
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
        self.call(|response| Request::StartProgramRun {
            program,
            authorization_id,
            dispatch_immediately: true,
            response,
        })
        .await
    }

    pub async fn prepare_program_run(
        &self,
        program: GcodeProgram,
        authorization_id: u64,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::StartProgramRun {
            program,
            authorization_id,
            dispatch_immediately: false,
            response,
        })
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
        self.call(|response| Request::StartCheckRun {
            program,
            execution_options,
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

    pub async fn cancel_jog(&self) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::CancelJog { response }).await
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
    async fn start_serial_run_fixture(
        &self,
        program: GcodeProgram,
        authorization_id: u64,
        dispatch_immediately: bool,
    ) -> Result<SenderSnapshot, ArbiterError> {
        self.call(|response| Request::StartProgramRun {
            program,
            authorization_id,
            dispatch_immediately,
            response,
        })
        .await
    }

    #[cfg(test)]
    async fn release_serial_run_fixture(&self) -> Result<SenderSnapshot, ArbiterError> {
        self.commit_prepared_program_run(self.sender_snapshot().run_sequence)
            .await
    }

    async fn send_realtime(
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

enum Request {
    ReplaceTransport {
        transport: BoxedTransport,
        execution_target: ExecutionTarget,
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    Connect {
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    SetHardwareProfile {
        profile: HardwareProfile,
        response: oneshot::Sender<Result<HardwareProfile, ArbiterError>>,
    },
    BindHardwareProfile {
        profile: HardwareProfile,
        response: oneshot::Sender<Result<HardwareProfile, ArbiterError>>,
    },
    UpdateControllerSetting {
        request: ControllerSettingEditRequest,
        response: oneshot::Sender<Result<VerifiedSettingUpdate, ArbiterError>>,
    },
    Disconnect {
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    RefreshStatus {
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    AcknowledgeReset {
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    UnlockAlarm {
        operator_confirmed: bool,
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    InspectDevice {
        response: oneshot::Sender<Result<HardwareInspection, ArbiterError>>,
    },
    PreflightRealRun {
        program: GcodeProgram,
        intent: ProgramRunIntent,
        execution_options: ProgramExecutionOptions,
        response: oneshot::Sender<Result<RunPreflightReport, ArbiterError>>,
    },
    AuthorizeFirstCut {
        program: GcodeProgram,
        confirmation: FirstCutConfirmation,
        require_check_certificate: bool,
        response: oneshot::Sender<Result<FirstCutPreparation, ArbiterError>>,
    },
    StartProgramRun {
        program: GcodeProgram,
        authorization_id: u64,
        dispatch_immediately: bool,
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    StartCheckRun {
        program: GcodeProgram,
        execution_options: ProgramExecutionOptions,
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    ResumeProgramRun {
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    PauseProgramRun {
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    AbortProgramRun {
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    CompleteToolChange {
        confirmation: ToolChangeConfirmation,
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    Realtime {
        command: RealtimeCommand,
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    BeginSoftReset {
        response: oneshot::Sender<Result<ResetChallenge, ArbiterError>>,
    },
    ConfirmSoftReset {
        challenge_id: u64,
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    PrepareTestJog {
        confirmation: OperatorConfirmation,
        response: oneshot::Sender<Result<TestJogPreparation, ArbiterError>>,
    },
    StepJog {
        request: StepJogRequest,
        response: oneshot::Sender<Result<StepJogReceipt, ArbiterError>>,
    },
    JogPadStep {
        request: JogPadStepRequest,
        response: oneshot::Sender<Result<JogPadStepOutcome, ArbiterError>>,
    },
    CancelJog {
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    ConfigureUnhomedOperation {
        response: oneshot::Sender<Result<UnhomedConfiguration, ArbiterError>>,
    },
    SetWorkZero {
        request: WorkZeroRequest,
        response: oneshot::Sender<Result<WorkZeroOutcome, ArbiterError>>,
    },
    ReturnToWorkZero {
        request: ReturnToWorkZeroRequest,
        response: oneshot::Sender<Result<ReturnToWorkZeroOutcome, ArbiterError>>,
    },
    StartDryRun {
        plan: DryRunPlan,
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    PauseDryRun {
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    ResumeDryRun {
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    CancelDryRun {
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    CommitPreparedProgramRun {
        run_sequence: u64,
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
    DiscardPreparedProgramRun {
        run_sequence: u64,
        response: oneshot::Sender<Result<SenderSnapshot, ArbiterError>>,
    },
}

struct ActorState {
    controller: Controller<BoxedTransport>,
    config: ControllerConfig,
    hardware_profile: HardwareProfile,
    execution_target: ExecutionTarget,
    sender: Sender,
    sender_dispatch_enabled: bool,
    safety: SafetyManager,
    first_cut: FirstCutGate,
    program_check: ProgramCheckGate,
    pending_program_check: Option<ProgramCheckBinding>,
    snapshots: watch::Sender<ControllerSnapshot>,
    sender_snapshots: watch::Sender<SenderSnapshot>,
}

async fn run_actor(mut actor: ActorState, mut requests: mpsc::Receiver<Request>) {
    let mut ticker = interval(actor.config.poll_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker.tick().await;

    loop {
        tokio::select! {
            biased;
            request = requests.recv() => {
                let Some(request) = request else {
                    break;
                };
                handle_request(request, &mut actor).await;
            }
            _ = ticker.tick() => {
                if matches!(
                    actor.controller.snapshot().connection,
                    ConnectionState::Connected | ConnectionState::Recovering
                ) {
                    if actor.sender.has_in_flight()
                        && actor.controller.snapshot().connection == ConnectionState::Connected
                    {
                        if let Err(error) = actor.controller.request_interleaved_status().await {
                            fail_and_quarantine_physical_sender(
                                &mut actor.controller,
                                &mut actor.sender,
                                &error,
                                "controller status request failed during program run",
                                &actor.sender_snapshots,
                            ).await;
                        }
                    } else {
                        let lifecycle = actor.controller.lifecycle_tick().await;
                        actor.safety.observe(&actor.controller.snapshot(), Instant::now());
                        actor.first_cut.observe(&actor.controller.snapshot(), Instant::now());
                        actor.program_check.observe(&actor.controller.snapshot(), Instant::now());
                        match lifecycle {
                            Ok(_) => reconcile_physical_sender(
                                &mut actor.controller,
                                &mut actor.sender,
                                &actor.sender_snapshots,
                            ).await,
                            Err(error) => {
                                fail_and_quarantine_physical_sender(
                                    &mut actor.controller,
                                    &mut actor.sender,
                                    &error,
                                    "controller polling failed during program run",
                                    &actor.sender_snapshots,
                                ).await
                            }
                        }
                    }
                    publish(&actor.snapshots, &actor.controller);
                    publish_sender(&actor.sender_snapshots, &actor.sender);
                }
            }
            _ = tokio::task::yield_now(), if actor.sender_dispatch_enabled && actor.sender.has_in_flight() => {
                execute_sender_step(
                    &mut actor.controller,
                    &mut actor.sender,
                    &mut actor.program_check,
                    &mut actor.pending_program_check,
                    &actor.snapshots,
                    &actor.sender_snapshots,
                )
                .await;
            }
            _ = tokio::task::yield_now(), if actor.sender_dispatch_enabled && actor.sender.is_dispatchable() => {
                execute_sender_step(
                    &mut actor.controller,
                    &mut actor.sender,
                    &mut actor.program_check,
                    &mut actor.pending_program_check,
                    &actor.snapshots,
                    &actor.sender_snapshots,
                )
                .await;
            }
        }
    }
}

async fn handle_request(request: Request, actor: &mut ActorState) {
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
        snapshots,
        sender_snapshots,
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
                *hardware_profile = profile;
                hardware_profile.clone()
            });
            let _ = response.send(result);
        }
        Request::UpdateControllerSetting { request, response } => {
            invalidate_authorizations(safety, first_cut);
            program_check.invalidate();
            let result = execute_controller_setting_update(controller, request).await;
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
        Request::PreflightRealRun {
            program,
            intent,
            execution_options,
            response,
        } => {
            first_cut.invalidate();
            let result = if *execution_target != ExecutionTarget::Serial {
                Err(ArbiterError::RealRunTransportUnavailable)
            } else {
                execute_real_run_preflight(
                    controller,
                    hardware_profile,
                    program_check,
                    program,
                    intent,
                    execution_options,
                    true,
                )
                .await
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::AuthorizeFirstCut {
            program,
            confirmation,
            require_check_certificate,
            response,
        } => {
            first_cut.invalidate();
            let result = if *execution_target != ExecutionTarget::Serial {
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
                    program,
                    confirmation,
                    require_check_certificate,
                )
                .await
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::StartProgramRun {
            program,
            authorization_id,
            dispatch_immediately,
            response,
        } => {
            let result = if *execution_target != ExecutionTarget::Serial {
                Err(ArbiterError::RealRunTransportUnavailable)
            } else {
                execute_authorized_program_run_start(
                    controller,
                    first_cut,
                    sender,
                    program,
                    authorization_id,
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
            response,
        } => {
            *sender_dispatch_enabled = true;
            let binding = ProgramCheckBinding::from_program(&program, execution_options);
            let result = if *execution_target != ExecutionTarget::Serial {
                Err(ArbiterError::CheckRunTransportUnavailable)
            } else {
                execute_check_run_start(controller, sender, &program, execution_options).await
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
            let result = if *execution_target != ExecutionTarget::Serial {
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
            let result = if *execution_target != ExecutionTarget::Serial {
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
            let result = if *execution_target != ExecutionTarget::Serial {
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
            let result = if *execution_target != ExecutionTarget::Serial {
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
            }
            let controller_result = if command == RealtimeCommand::Status && sender.has_in_flight()
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
                    let controller_result =
                        controller.send_realtime(RealtimeCommand::SoftReset).await;
                    match &controller_result {
                        Ok(_) => cancel_active_sender(sender, sender_snapshots),
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
        Request::CancelJog { response } => {
            invalidate_authorizations(safety, first_cut);
            let mode = controller.snapshot().machine.mode;
            let result = if mode == MachineMode::Jog {
                controller
                    .send_realtime(RealtimeCommand::JogCancel)
                    .await
                    .map_err(ArbiterError::from)
            } else {
                Err(ArbiterError::JogCancelUnavailable(mode))
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
            let result = execute_set_work_zero(controller, request).await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::ReturnToWorkZero { request, response } => {
            invalidate_authorizations(safety, first_cut);
            let result = execute_return_to_work_zero(controller, hardware_profile, request).await;
            publish(snapshots, controller);
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

async fn execute_real_run_preflight(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    program_check: &mut ProgramCheckGate,
    program: GcodeProgram,
    intent: ProgramRunIntent,
    execution_options: ProgramExecutionOptions,
    require_check_certificate: bool,
) -> Result<RunPreflightReport, ArbiterError> {
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
    if require_check_certificate
        && (intent == ProgramRunIntent::Cutting || requires_safe_start_check(&program))
    {
        apply_program_check_requirement(
            &mut report,
            program_check.validate(&binding, &snapshot, Instant::now()),
        );
    }
    Ok(report)
}

fn requires_safe_start_check(program: &GcodeProgram) -> bool {
    program.source_name.starts_with("safe-start-L")
        && program
            .lines
            .first()
            .is_some_and(|line| line.source.trim().starts_with("(Millo safe start from L"))
}

fn apply_program_check_requirement(
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

async fn execute_first_cut_authorization(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    first_cut: &mut FirstCutGate,
    program_check: &mut ProgramCheckGate,
    program: GcodeProgram,
    confirmation: FirstCutConfirmation,
    require_check_certificate: bool,
) -> Result<FirstCutPreparation, ArbiterError> {
    let report = execute_real_run_preflight(
        controller,
        hardware_profile,
        program_check,
        program,
        confirmation.intent,
        confirmation.execution_options,
        require_check_certificate,
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

async fn execute_authorized_program_run_start(
    controller: &mut Controller<BoxedTransport>,
    first_cut: &mut FirstCutGate,
    sender: &mut Sender,
    program: GcodeProgram,
    authorization_id: u64,
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
    let snapshot = controller.refresh_status().await?;
    ensure_stable_idle(&snapshot)?;
    let authorization =
        first_cut.consume(authorization_id, &fingerprint, &snapshot, Instant::now())?;
    let policy = match authorization.intent {
        ProgramRunIntent::AirRun => ProgramRunPolicy::AirRun,
        ProgramRunIntent::Cutting => ProgramRunPolicy::Cutting,
    };
    let plan =
        build_program_run_plan_with_options(&program, policy, authorization.execution_options)?;
    sender.configure_rx_buffer_capacity(usable_rx_buffer_capacity(
        authorization.reported_rx_buffer_bytes,
    ))?;
    match authorization.intent {
        ProgramRunIntent::AirRun => sender.load_air_run(plan)?,
        ProgramRunIntent::Cutting => sender.load_cut_run(plan)?,
    };
    sender.start().map_err(ArbiterError::from)
}

async fn execute_check_run_start(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
    program: &GcodeProgram,
    execution_options: ProgramExecutionOptions,
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

    let plan =
        build_program_run_plan_with_options(program, ProgramRunPolicy::Cutting, execution_options)?;
    let initial = controller.refresh_status().await?;
    ensure_stable_idle(&initial)?;
    let inspection = controller.inspect_device().await?;
    let final_idle = controller.refresh_status().await?;
    ensure_stable_idle(&final_idle)?;

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

async fn execute_program_run_resume(
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
    let snapshot = controller.refresh_status().await?;
    match snapshot.machine.mode {
        MachineMode::Hold => {
            controller
                .send_realtime(RealtimeCommand::CycleStart)
                .await?;
        }
        MachineMode::Idle => {}
        mode => return Err(ArbiterError::ProgramRunResumeUnavailable(mode)),
    }
    sender.resume().map_err(ArbiterError::from)
}

fn ensure_active_physical_sender(
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

async fn execute_program_run_pause(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
) -> Result<SenderSnapshot, ArbiterError> {
    let state = ensure_active_physical_sender(sender, "pause")?;
    if !matches!(state, SenderState::Running | SenderState::Draining) {
        return Err(ArbiterError::ProgramRunPauseUnavailable(state));
    }
    controller.send_realtime(RealtimeCommand::FeedHold).await?;
    sender.pause().map_err(ArbiterError::from)
}

async fn execute_program_run_abort(
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

async fn execute_tool_change_completion(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
    confirmation: ToolChangeConfirmation,
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

    sender.complete_tool_change().map_err(ArbiterError::from)
}

fn invalidate_authorizations(safety: &mut SafetyManager, first_cut: &mut FirstCutGate) {
    safety.invalidate_test_jog();
    first_cut.invalidate();
}

async fn reconcile_physical_sender(
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
            (SenderState::Running, MachineMode::Hold | MachineMode::Door) => {
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

async fn fail_and_quarantine_physical_sender(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
    error: &ControllerError,
    context: &str,
    sender_snapshots: &watch::Sender<SenderSnapshot>,
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
        publish_sender(sender_snapshots, sender);
    }
    if physical_run && controller_failure_requires_manual_reconnect(error) {
        let _ = controller.disconnect().await;
    }
}

fn physical_sender_active(sender: &Sender) -> bool {
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

fn controller_failure_requires_manual_reconnect(error: &ControllerError) -> bool {
    matches!(
        error,
        ControllerError::CommandTimeout { .. }
            | ControllerError::StatusTimeout { .. }
            | ControllerError::Transport(_)
            | ControllerError::NotReady(_)
    )
}

fn controller_sender_failure(error: &ControllerError, context: &str) -> SenderFailure {
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

async fn execute_sender_step(
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

fn ensure_sender_dispatch_ready(
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
        Some(millo_sender::SenderMode::AirRun | millo_sender::SenderMode::CutRun) => matches!(
            snapshot.machine.mode,
            MachineMode::Idle | MachineMode::Run | MachineMode::Hold
        ),
        Some(millo_sender::SenderMode::CheckRun) => snapshot.machine.mode == MachineMode::Check,
        _ => snapshot.machine.mode == MachineMode::Idle,
    };
    if mode_ready {
        Ok(())
    } else {
        Err(SafetyError::UnsafeControllerState.into())
    }
}

async fn settle_program_check(
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

async fn cancel_check_run(
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

fn cancel_active_sender(sender: &mut Sender, snapshots: &watch::Sender<SenderSnapshot>) {
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

async fn execute_set_work_zero(
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

async fn execute_return_to_work_zero(
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

fn work_axis_to_jog_axis(axis: WorkAxis) -> millo_domain::JogAxis {
    match axis {
        WorkAxis::X => millo_domain::JogAxis::X,
        WorkAxis::Y => millo_domain::JogAxis::Y,
        WorkAxis::Z => millo_domain::JogAxis::Z,
    }
}

fn work_axis_value(position: Position, axis: WorkAxis) -> f64 {
    match axis {
        WorkAxis::X => position.x,
        WorkAxis::Y => position.y,
        WorkAxis::Z => position.z,
    }
}

fn verified_work_axis(
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

fn parse_xyz_parameter(value: &str) -> Option<[f64; 3]> {
    let values = value
        .split(',')
        .take(3)
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() == 3).then(|| [values[0], values[1], values[2]])
}

fn position_axis(position: Position, axis: WorkAxis) -> f64 {
    match axis {
        WorkAxis::X => position.x,
        WorkAxis::Y => position.y,
        WorkAxis::Z => position.z,
    }
}

const fn work_axis_index(axis: WorkAxis) -> usize {
    match axis {
        WorkAxis::X => 0,
        WorkAxis::Y => 1,
        WorkAxis::Z => 2,
    }
}

async fn configure_unhomed_operation(
    controller: &mut Controller<BoxedTransport>,
) -> Result<UnhomedConfiguration, ArbiterError> {
    ensure_stable_idle(&controller.snapshot())?;
    let before = controller.inspect_device().await?;
    let mut writes = Vec::with_capacity(2);

    if before.settings.get("$21").map(String::as_str) != Some("0") {
        ensure_stable_idle(&controller.snapshot())?;
        writes.push(
            controller
                .disable_unhomed_setting(UnhomedSetting::HardLimits)
                .await?,
        );
    }
    if before.settings.get("$22").map(String::as_str) != Some("0") {
        ensure_stable_idle(&controller.snapshot())?;
        writes.push(
            controller
                .disable_unhomed_setting(UnhomedSetting::Homing)
                .await?,
        );
    }

    ensure_stable_idle(&controller.snapshot())?;
    let after = controller.inspect_device().await?;
    for key in ["$21", "$22"] {
        if after.settings.get(key).map(String::as_str) != Some("0") {
            return Err(ArbiterError::ConfigurationVerification(format!(
                "expected {key}=0, read {:?}",
                after.settings.get(key)
            )));
        }
    }

    Ok(UnhomedConfiguration {
        before,
        after,
        writes,
    })
}

async fn execute_controller_setting_update(
    controller: &mut Controller<BoxedTransport>,
    request: ControllerSettingEditRequest,
) -> Result<VerifiedSettingUpdate, ArbiterError> {
    controller.refresh_status().await?;
    ensure_stable_idle(&controller.snapshot())?;
    let before = controller.inspect_device().await?;
    let setting = validate_setting_edit(request, &before)?;
    let before_value = before
        .settings
        .get(setting.key())
        .ok_or_else(|| ArbiterError::SettingSourceMissing(setting.key().to_owned()))?
        .clone();
    ensure_stable_idle(&controller.snapshot())?;
    let write = controller.write_setting(&setting).await?;
    controller.refresh_status().await?;
    ensure_stable_idle(&controller.snapshot())?;
    let inspection = controller.inspect_device().await?;
    let stored_value = inspection
        .settings
        .get(setting.key())
        .cloned()
        .unwrap_or_else(|| "missing".to_owned());
    if !setting_values_equal(setting.value(), &stored_value) {
        return Err(ArbiterError::SettingVerification {
            key: setting.key().to_owned(),
            requested: setting.value().to_owned(),
            stored: stored_value,
        });
    }
    Ok(VerifiedSettingUpdate {
        key: setting.key().to_owned(),
        before_value,
        stored_value,
        write,
        inspection,
    })
}

fn ensure_stable_idle(snapshot: &ControllerSnapshot) -> Result<(), ArbiterError> {
    if snapshot.connection == ConnectionState::Connected
        && snapshot.machine.mode == MachineMode::Idle
        && snapshot.alarm.is_none()
        && snapshot.reset_notice.is_none()
    {
        Ok(())
    } else {
        Err(SafetyError::UnsafeControllerState.into())
    }
}

fn ensure_profile_binding_available(snapshot: &ControllerSnapshot) -> Result<(), ArbiterError> {
    if snapshot.connection != ConnectionState::Disconnected {
        Ok(())
    } else {
        Err(SafetyError::UnsafeControllerState.into())
    }
}

async fn prepare_test_jog(
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

async fn execute_jog_pad_step(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    safety: &mut SafetyManager,
    request: JogPadStepRequest,
) -> Result<JogPadStepOutcome, ArbiterError> {
    validate_jog_pad_motion(request.distance_mm, request.feed_mm_per_min)?;
    let distance_limit =
        axis_travel_limit(hardware_profile, request.axis).min(hardware_profile.max_jog_distance_mm);
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
    if let Some(maximum) = axis_max_rate(&preparation.inspection.device, request.axis)
        && request.feed_mm_per_min > maximum
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

fn validate_jog_pad_motion(distance_mm: f64, feed_mm_per_min: f64) -> Result<(), ArbiterError> {
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

fn axis_max_rate(device: &DeviceInspection, axis: millo_domain::JogAxis) -> Option<f64> {
    let key = match axis {
        millo_domain::JogAxis::X => "$110",
        millo_domain::JogAxis::Y => "$111",
        millo_domain::JogAxis::Z => "$112",
    };
    device
        .settings
        .get(key)
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn axis_travel_limit(profile: &HardwareProfile, axis: millo_domain::JogAxis) -> f64 {
    let Some(travel) = profile.travel_mm else {
        return profile.max_jog_distance_mm;
    };
    match axis {
        millo_domain::JogAxis::X => travel.x,
        millo_domain::JogAxis::Y => travel.y,
        millo_domain::JogAxis::Z => travel.z,
    }
}

fn publish(snapshots: &watch::Sender<ControllerSnapshot>, controller: &Controller<BoxedTransport>) {
    snapshots.send_replace(controller.snapshot());
}

fn publish_sender(snapshots: &watch::Sender<SenderSnapshot>, sender: &Sender) {
    snapshots.send_replace(sender.snapshot());
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use millo_dry_run::{build_dry_run_plan, build_program_run_plan};
    use millo_gcode::{
        ProgramParseOptions, ProgramParseRequest, parse_program, parse_program_with_options,
    };
    use millo_mock::MockTransport;

    use super::*;

    fn test_arbiter(
        poll_interval: Duration,
    ) -> (
        CommandArbiter,
        millo_mock::MockControl,
        impl Future<Output = ()> + Send + 'static,
    ) {
        let transport = MockTransport::default();
        let control = transport.control();
        let (arbiter, worker) = CommandArbiter::new(
            Box::new(transport),
            ControllerConfig {
                poll_interval,
                status_timeout: Duration::from_millis(20),
                command_timeout: Duration::from_millis(50),
                failures_before_recovery: 2,
            },
            HardwareProfile::first_machine(),
        );
        (arbiter, control, worker)
    }

    fn operator_confirmation() -> OperatorConfirmation {
        OperatorConfirmation {
            spindle_off: true,
            tool_clear: true,
            power_control_reachable: true,
        }
    }

    fn first_cut_confirmation() -> FirstCutConfirmation {
        FirstCutConfirmation {
            intent: ProgramRunIntent::Cutting,
            execution_options: ProgramExecutionOptions::default(),
            stock_secured: true,
            tool_secured: true,
            tool_removed: false,
            xyz_zero_verified: true,
            safe_z_verified: true,
            manual_spindle_running: true,
            manual_spindle_off: false,
            path_clear: true,
            power_control_reachable: true,
        }
    }

    fn air_run_confirmation() -> FirstCutConfirmation {
        FirstCutConfirmation {
            intent: ProgramRunIntent::AirRun,
            execution_options: ProgramExecutionOptions::default(),
            stock_secured: false,
            tool_secured: false,
            tool_removed: true,
            xyz_zero_verified: true,
            safe_z_verified: true,
            manual_spindle_running: false,
            manual_spindle_off: true,
            path_clear: true,
            power_control_reachable: true,
        }
    }

    fn tool_change_confirmation(
        source_line: usize,
        requested_tool: Option<u8>,
    ) -> ToolChangeConfirmation {
        ToolChangeConfirmation {
            source_line,
            requested_tool,
            tool_secured: true,
            z_zero_verified: true,
            safe_z_verified: true,
            path_clear: true,
            manual_spindle_running: true,
            power_control_reachable: true,
        }
    }

    fn work_zero_request(axis: WorkAxis, position_confirmed: bool) -> WorkZeroRequest {
        WorkZeroRequest {
            axis,
            position_confirmed,
        }
    }

    fn return_to_zero_request(axis: WorkAxis) -> ReturnToWorkZeroRequest {
        ReturnToWorkZeroRequest {
            axis,
            feed_mm_per_min: if matches!(axis, WorkAxis::Z) {
                100.0
            } else {
                300.0
            },
        }
    }

    fn dry_run_plan(source: &str) -> DryRunPlan {
        let program = parsed_program(source);
        build_dry_run_plan(&program).unwrap()
    }

    async fn authorize_and_start_serial_fixture(
        arbiter: &CommandArbiter,
        source: &str,
        dispatch_immediately: bool,
    ) -> (FirstCutPreparation, SenderSnapshot) {
        let preparation = arbiter
            .authorize_first_cut_fixture(parsed_program(source), first_cut_confirmation())
            .await
            .unwrap();
        let started = arbiter
            .start_serial_run_fixture(
                parsed_program(source),
                preparation.authorization.id,
                dispatch_immediately,
            )
            .await
            .unwrap();
        (preparation, started)
    }

    fn parsed_program(source: &str) -> GcodeProgram {
        parse_program(ProgramParseRequest {
            source_name: "sender-fixture.nc".to_owned(),
            source: source.to_owned(),
        })
        .unwrap()
    }

    fn safe_start_program(source: &str) -> GcodeProgram {
        parse_program(ProgramParseRequest {
            source_name: "safe-start-L42-original.nc".to_owned(),
            source: source.to_owned(),
        })
        .unwrap()
    }

    fn parsed_program_with_options(
        source: &str,
        execution_options: ProgramExecutionOptions,
    ) -> GcodeProgram {
        parse_program_with_options(
            ProgramParseRequest {
                source_name: "sender-fixture.nc".to_owned(),
                source: source.to_owned(),
            },
            ProgramParseOptions {
                block_delete: execution_options.block_delete,
            },
        )
        .unwrap()
    }

    fn mock_dry_run_arbiter() -> (
        CommandArbiter,
        millo_mock::MockControl,
        impl Future<Output = ()> + Send + 'static,
    ) {
        let transport = MockTransport::default();
        let control = transport.control();
        let (arbiter, worker) = CommandArbiter::new_with_execution_target(
            Box::new(transport),
            ControllerConfig {
                poll_interval: Duration::from_secs(60),
                status_timeout: Duration::from_millis(20),
                command_timeout: Duration::from_millis(50),
                failures_before_recovery: 2,
            },
            HardwareProfile::first_machine(),
            ExecutionTarget::Mock,
        );
        (arbiter, control, worker)
    }

    fn serial_preflight_arbiter() -> (
        CommandArbiter,
        millo_mock::MockControl,
        impl Future<Output = ()> + Send + 'static,
    ) {
        serial_preflight_arbiter_with_poll(Duration::from_secs(60))
    }

    fn serial_preflight_arbiter_with_poll(
        poll_interval: Duration,
    ) -> (
        CommandArbiter,
        millo_mock::MockControl,
        impl Future<Output = ()> + Send + 'static,
    ) {
        let transport = MockTransport::default();
        let control = transport.control();
        let (arbiter, worker) = CommandArbiter::new_with_execution_target(
            Box::new(transport),
            ControllerConfig {
                poll_interval,
                status_timeout: Duration::from_millis(20),
                command_timeout: Duration::from_millis(50),
                failures_before_recovery: 2,
            },
            HardwareProfile::first_machine(),
            ExecutionTarget::Serial,
        );
        (arbiter, control, worker)
    }

    async fn wait_for_sender(arbiter: &CommandArbiter, expected: SenderState) -> SenderSnapshot {
        let mut snapshots = arbiter.subscribe_sender();
        tokio::time::timeout(Duration::from_millis(200), async {
            loop {
                let snapshot = snapshots.borrow_and_update().clone();
                if snapshot.state == expected {
                    return snapshot;
                }
                snapshots.changed().await.unwrap();
            }
        })
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn serializes_status_and_inspector_commands_through_one_worker() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        arbiter.refresh_status().await.unwrap();
        let inspection = arbiter.inspect_device().await.unwrap();

        assert_eq!(inspection.device.responses.len(), 4);
        assert!(inspection.readiness.test_jog_ready);
        assert_eq!(
            control.writes(),
            vec![
                b"?".to_vec(),
                b"$I\n".to_vec(),
                b"$$\n".to_vec(),
                b"$G\n".to_vec(),
                b"$#\n".to_vec(),
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn changes_the_hardware_profile_only_while_disconnected() {
        let (arbiter, _, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        let mut profile = HardwareProfile::first_machine();
        profile.name = "Selected bench router".to_owned();
        profile.travel_mm = Some(millo_domain::MachineTravel {
            x: 300.0,
            y: 180.0,
            z: 45.0,
        });

        let selected = arbiter.set_hardware_profile(profile.clone()).await.unwrap();
        assert_eq!(selected, profile);

        arbiter.connect().await.unwrap();
        let error = arbiter
            .set_hardware_profile(HardwareProfile::first_machine())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            ArbiterError::ProfileChangeUnavailable(ConnectionState::Connected)
        ));
        let inspection = arbiter.inspect_device().await.unwrap();
        assert_eq!(inspection.readiness.profile.name, "Selected bench router");
        assert_eq!(inspection.readiness.profile.travel_mm, profile.travel_mm);
        task.abort();
    }

    #[tokio::test]
    async fn binds_an_identified_profile_while_an_idle_reset_banner_is_visible() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        control.queue_reset("1.1h");
        arbiter.refresh_status().await.unwrap();
        let mut profile = HardwareProfile::first_machine();
        profile.name = "Identified router".to_owned();

        arbiter.bind_hardware_profile(profile).await.unwrap();
        let inspection = arbiter.inspect_device().await.unwrap();

        assert_eq!(inspection.readiness.profile.name, "Identified router");
        assert!(arbiter.snapshot().reset_notice.is_some());
        task.abort();
    }

    #[tokio::test]
    async fn profile_binding_is_local_context_and_does_not_write_during_run() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        control.set_status("<Run|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:10,0>");
        arbiter.refresh_status().await.unwrap();
        let writes_before_binding = control.writes();
        let mut profile = HardwareProfile::first_machine();
        profile.name = "Bound while externally running".to_owned();

        arbiter.bind_hardware_profile(profile).await.unwrap();

        assert_eq!(control.writes(), writes_before_binding);
        assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Run);
        task.abort();
    }

    #[tokio::test]
    async fn writes_and_rereads_one_confirmed_controller_setting() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let update = arbiter
            .update_controller_setting(ControllerSettingEditRequest {
                key: "$120".to_owned(),
                value: "600".to_owned(),
                confirmed: true,
                expected_value: Some("50".to_owned()),
                expected_revision: Some(7),
            })
            .await
            .unwrap();

        assert_eq!(update.before_value, "50.000");
        assert_eq!(update.stored_value, "600");
        assert_eq!(
            control.writes(),
            vec![
                b"?".to_vec(),
                b"$I\n".to_vec(),
                b"$$\n".to_vec(),
                b"$G\n".to_vec(),
                b"$#\n".to_vec(),
                b"$120=600\n".to_vec(),
                b"?".to_vec(),
                b"$I\n".to_vec(),
                b"$$\n".to_vec(),
                b"$G\n".to_vec(),
                b"$#\n".to_vec(),
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn actor_owns_periodic_lifecycle_polling() {
        let (arbiter, _control, worker) = test_arbiter(Duration::from_millis(5));
        let task = tokio::spawn(worker);
        let mut snapshots = arbiter.subscribe();
        arbiter.connect().await.unwrap();

        tokio::time::timeout(Duration::from_millis(100), async {
            loop {
                snapshots.changed().await.unwrap();
                if snapshots.borrow().poll_sequence > 0 {
                    break;
                }
            }
        })
        .await
        .unwrap();

        assert_eq!(arbiter.snapshot().connection, ConnectionState::Connected);
        task.abort();
    }

    #[tokio::test]
    async fn realtime_and_line_requests_share_the_same_queue() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        arbiter
            .send_realtime(RealtimeCommand::FeedHold)
            .await
            .unwrap();
        arbiter.inspect_device().await.unwrap();

        assert_eq!(control.writes()[0], b"!".to_vec());
        assert_eq!(control.writes()[1], b"$I\n".to_vec());
        task.abort();
    }

    #[tokio::test]
    async fn realtime_status_request_consumes_its_status_frame() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let snapshot = arbiter
            .send_realtime(RealtimeCommand::Status)
            .await
            .unwrap();

        assert_eq!(snapshot.poll_sequence, 1);
        assert_eq!(control.writes(), vec![b"?".to_vec()]);
        task.abort();
    }

    #[tokio::test]
    async fn test_jog_preparation_runs_a_fresh_inspection_each_time() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        arbiter.refresh_status().await.unwrap();

        let first = arbiter
            .prepare_test_jog(operator_confirmation())
            .await
            .unwrap();
        let second = arbiter
            .prepare_test_jog(operator_confirmation())
            .await
            .unwrap();

        assert!(first.authorization.is_some());
        assert!(second.authorization.is_some());
        assert_ne!(
            first.authorization.unwrap().id,
            second.authorization.unwrap().id
        );
        assert_eq!(
            control.writes(),
            vec![
                b"?".to_vec(),
                b"?".to_vec(),
                b"$I\n".to_vec(),
                b"$$\n".to_vec(),
                b"$G\n".to_vec(),
                b"$#\n".to_vec(),
                b"?".to_vec(),
                b"$I\n".to_vec(),
                b"$$\n".to_vec(),
                b"$G\n".to_vec(),
                b"$#\n".to_vec(),
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn incomplete_operator_confirmation_does_not_touch_the_controller() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        let mut incomplete = operator_confirmation();
        incomplete.tool_clear = false;

        let error = arbiter.prepare_test_jog(incomplete).await.unwrap_err();

        assert!(matches!(
            error,
            ArbiterError::Safety(SafetyError::IncompleteOperatorConfirmation)
        ));
        assert!(control.writes().is_empty());
        task.abort();
    }

    #[tokio::test]
    async fn soft_reset_requires_and_consumes_an_actor_challenge() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        let challenge = arbiter.request_soft_reset().await.unwrap();

        arbiter.confirm_soft_reset(challenge.id).await.unwrap();
        let reused = arbiter.confirm_soft_reset(challenge.id).await.unwrap_err();

        assert!(matches!(
            reused,
            ArbiterError::Safety(SafetyError::ResetChallengeMissing)
        ));
        assert_eq!(control.writes(), vec![b"\x18".to_vec()]);
        task.abort();
    }

    #[tokio::test]
    async fn invalid_soft_reset_confirmation_cannot_cancel_an_active_sender() {
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X2 F20", false).await;

        let error = arbiter.confirm_soft_reset(u64::MAX).await.unwrap_err();

        assert!(matches!(
            error,
            ArbiterError::Safety(SafetyError::ResetChallengeMissing)
        ));
        assert_eq!(arbiter.sender_snapshot().state, SenderState::Running);
        assert!(!control.writes().iter().any(|write| write == b"\x18"));
        task.abort();
    }

    #[tokio::test]
    async fn connected_actor_rejects_reconnect_and_transport_replacement() {
        let (arbiter, _, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X2 F20", false).await;

        let reconnect = arbiter.connect().await.unwrap_err();
        let replacement = arbiter
            .replace_transport(Box::new(MockTransport::default()))
            .await
            .unwrap_err();

        assert!(matches!(
            reconnect,
            ArbiterError::ConnectUnavailable(ConnectionState::Connected)
        ));
        assert!(matches!(
            replacement,
            ArbiterError::TransportReplacementUnavailable(ConnectionState::Connected)
        ));
        assert_eq!(arbiter.sender_snapshot().state, SenderState::Running);
        task.abort();
    }

    #[tokio::test]
    async fn mock_pause_and_resume_cannot_change_a_physical_sender() {
        let (arbiter, _, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X2 F20", false).await;

        assert!(matches!(
            arbiter.pause_dry_run().await.unwrap_err(),
            ArbiterError::DryRunTransportUnavailable
        ));
        assert!(matches!(
            arbiter.resume_dry_run().await.unwrap_err(),
            ArbiterError::DryRunTransportUnavailable
        ));
        assert_eq!(arbiter.sender_snapshot().state, SenderState::Running);
        task.abort();
    }

    #[tokio::test]
    async fn alarm_unlock_requires_confirmation_and_verifies_idle_in_the_actor() {
        let transport = MockTransport::with_status(
            "<Alarm|MPos:1.000,2.000,3.000|WPos:1.000,2.000,3.000|FS:0,0>",
        );
        let control = transport.control();
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

        assert!(matches!(
            arbiter.unlock_alarm(false).await.unwrap_err(),
            ArbiterError::UnlockConfirmationRequired
        ));
        assert!(control.writes().is_empty());

        let unlocked = arbiter.unlock_alarm(true).await.unwrap();
        assert_eq!(unlocked.machine.mode, MachineMode::Idle);
        assert!(unlocked.alarm.is_none());
        assert_eq!(
            control.writes(),
            vec![b"?".to_vec(), b"$X\n".to_vec(), b"?".to_vec()]
        );
        task.abort();
    }

    #[tokio::test]
    async fn alarm_returns_a_fresh_blocked_report_without_authorization() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        control.queue_alarm(3);
        arbiter.refresh_status().await.unwrap();

        let preparation = arbiter
            .prepare_test_jog(operator_confirmation())
            .await
            .unwrap();

        assert!(preparation.authorization.is_none());
        assert!(!preparation.inspection.readiness.test_jog_ready);
        assert!(preparation.inspection.readiness.blocker_count > 0);
        task.abort();
    }

    #[tokio::test]
    async fn consumes_authorization_before_writing_one_typed_step_jog() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        arbiter.refresh_status().await.unwrap();
        let authorization = arbiter
            .prepare_test_jog(operator_confirmation())
            .await
            .unwrap()
            .authorization
            .unwrap();

        let receipt = arbiter
            .step_jog(StepJogRequest {
                authorization_id: authorization.id,
                axis: millo_domain::JogAxis::X,
                distance_mm: 0.1,
                feed_mm_per_min: 50.0,
            })
            .await
            .unwrap();
        let reused = arbiter
            .step_jog(StepJogRequest {
                authorization_id: authorization.id,
                axis: millo_domain::JogAxis::X,
                distance_mm: 0.1,
                feed_mm_per_min: 50.0,
            })
            .await
            .unwrap_err();

        assert_eq!(receipt.command, "$J=G91 G21 X0.100 F50.000");
        assert!(matches!(
            reused,
            ArbiterError::Safety(SafetyError::TestJogAuthorizationMissing)
        ));
        assert_eq!(
            control.writes().last(),
            Some(&b"$J=G91 G21 X0.100 F50.000\n".to_vec())
        );
        assert_eq!(
            control
                .writes()
                .iter()
                .filter(|write| write.starts_with(b"$J="))
                .count(),
            1
        );
        task.abort();
    }

    #[tokio::test]
    async fn failed_jog_validation_still_consumes_the_authorization() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        arbiter.refresh_status().await.unwrap();
        let authorization = arbiter
            .prepare_test_jog(operator_confirmation())
            .await
            .unwrap()
            .authorization
            .unwrap();

        let invalid = arbiter
            .step_jog(StepJogRequest {
                authorization_id: authorization.id,
                axis: millo_domain::JogAxis::Z,
                distance_mm: MAX_STEP_JOG_DISTANCE_MM + 0.01,
                feed_mm_per_min: 50.0,
            })
            .await
            .unwrap_err();
        let retry = arbiter
            .step_jog(StepJogRequest {
                authorization_id: authorization.id,
                axis: millo_domain::JogAxis::Z,
                distance_mm: 0.1,
                feed_mm_per_min: 50.0,
            })
            .await
            .unwrap_err();

        assert!(matches!(
            invalid,
            ArbiterError::Controller(ControllerError::JogValidation(_))
        ));
        assert!(matches!(
            retry,
            ArbiterError::Safety(SafetyError::TestJogAuthorizationMissing)
        ));
        assert!(
            !control
                .writes()
                .iter()
                .any(|write| write.starts_with(b"$J="))
        );
        task.abort();
    }

    #[tokio::test]
    async fn jog_cancel_is_available_only_for_reported_jog_state() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        arbiter.refresh_status().await.unwrap();

        assert!(matches!(
            arbiter.cancel_jog().await.unwrap_err(),
            ArbiterError::JogCancelUnavailable(MachineMode::Idle)
        ));

        let authorization = arbiter
            .prepare_test_jog(operator_confirmation())
            .await
            .unwrap()
            .authorization
            .unwrap();
        arbiter
            .step_jog(StepJogRequest {
                authorization_id: authorization.id,
                axis: millo_domain::JogAxis::Y,
                distance_mm: -1.0,
                feed_mm_per_min: 10.0,
            })
            .await
            .unwrap();
        assert_eq!(
            arbiter.refresh_status().await.unwrap().machine.mode,
            MachineMode::Jog
        );

        arbiter.cancel_jog().await.unwrap();
        assert_eq!(control.writes().last(), Some(&vec![0x85]));
        assert_eq!(
            arbiter.refresh_status().await.unwrap().machine.mode,
            MachineMode::Idle
        );
        task.abort();
    }

    #[tokio::test]
    async fn y_and_z_steps_each_require_a_fresh_authorization() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        arbiter.refresh_status().await.unwrap();

        let mut authorization_ids = Vec::new();
        for axis in [millo_domain::JogAxis::Y, millo_domain::JogAxis::Z] {
            let authorization = arbiter
                .prepare_test_jog(operator_confirmation())
                .await
                .unwrap()
                .authorization
                .unwrap();
            authorization_ids.push(authorization.id);

            arbiter
                .step_jog(StepJogRequest {
                    authorization_id: authorization.id,
                    axis,
                    distance_mm: 0.1,
                    feed_mm_per_min: 100.0,
                })
                .await
                .unwrap();

            assert_eq!(
                arbiter.refresh_status().await.unwrap().machine.mode,
                MachineMode::Jog
            );
            assert_eq!(
                arbiter.refresh_status().await.unwrap().machine.mode,
                MachineMode::Idle
            );
        }

        assert_ne!(authorization_ids[0], authorization_ids[1]);
        let snapshot = arbiter.snapshot();
        let position = snapshot.machine.machine_position.unwrap();
        assert_eq!(position.x, 0.0);
        assert_eq!(position.y, 0.1);
        assert_eq!(position.z, 0.1);

        let jog_writes = control
            .writes()
            .into_iter()
            .filter(|write| write.starts_with(b"$J="))
            .collect::<Vec<_>>();
        assert_eq!(
            jog_writes,
            vec![
                b"$J=G91 G21 Y0.100 F100.000\n".to_vec(),
                b"$J=G91 G21 Z0.100 F100.000\n".to_vec()
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn jog_pad_rechecks_motion_and_forwards_selected_feed() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let first = arbiter
            .jog_pad_step(JogPadStepRequest {
                confirmation: operator_confirmation(),
                axis: millo_domain::JogAxis::Y,
                distance_mm: 0.1,
                feed_mm_per_min: 300.0,
            })
            .await
            .unwrap();
        assert_eq!(first.receipt.unwrap().command, "$J=G91 G21 Y0.100 F300.000");

        let blocked_while_moving = arbiter
            .jog_pad_step(JogPadStepRequest {
                confirmation: operator_confirmation(),
                axis: millo_domain::JogAxis::Z,
                distance_mm: 0.01,
                feed_mm_per_min: 100.0,
            })
            .await
            .unwrap();
        assert!(blocked_while_moving.receipt.is_none());
        assert!(!blocked_while_moving.inspection.readiness.test_jog_ready);

        for _ in 0..6 {
            if arbiter.refresh_status().await.unwrap().machine.mode == MachineMode::Idle {
                break;
            }
        }
        let second = arbiter
            .jog_pad_step(JogPadStepRequest {
                confirmation: operator_confirmation(),
                axis: millo_domain::JogAxis::Z,
                distance_mm: -0.01,
                feed_mm_per_min: 100.0,
            })
            .await
            .unwrap();
        assert_eq!(
            second.receipt.unwrap().command,
            "$J=G91 G21 Z-0.010 F100.000"
        );

        let writes = control.writes();
        assert_eq!(
            writes
                .iter()
                .filter(|write| write.as_slice() == b"$I\n")
                .count(),
            3
        );
        assert_eq!(
            writes
                .into_iter()
                .filter(|write| write.starts_with(b"$J="))
                .collect::<Vec<_>>(),
            vec![
                b"$J=G91 G21 Y0.100 F300.000\n".to_vec(),
                b"$J=G91 G21 Z-0.010 F100.000\n".to_vec()
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn jog_pad_rejects_distance_above_machine_profile_before_controller_io() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let error = arbiter
            .jog_pad_step(JogPadStepRequest {
                confirmation: operator_confirmation(),
                axis: millo_domain::JogAxis::X,
                distance_mm: 50.01,
                feed_mm_per_min: 100.0,
            })
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ArbiterError::JogPadDistanceExceedsProfile {
                axis: millo_domain::JogAxis::X,
                requested,
                maximum,
            } if requested == 50.01 && maximum == 50.0
        ));
        assert!(control.writes().is_empty());
        task.abort();
    }

    #[tokio::test]
    async fn jog_pad_rejects_distance_above_selected_axis_travel() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let mut profile = HardwareProfile::first_machine();
        profile.travel_mm = Some(millo_domain::MachineTravel {
            x: 300.0,
            y: 180.0,
            z: 20.0,
        });
        profile.max_jog_distance_mm = 50.0;
        let task = tokio::spawn(worker);
        arbiter.set_hardware_profile(profile).await.unwrap();
        arbiter.connect().await.unwrap();

        let error = arbiter
            .jog_pad_step(JogPadStepRequest {
                confirmation: operator_confirmation(),
                axis: millo_domain::JogAxis::Z,
                distance_mm: 20.01,
                feed_mm_per_min: 100.0,
            })
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ArbiterError::JogPadDistanceExceedsProfile {
                axis: millo_domain::JogAxis::Z,
                requested,
                maximum,
            } if requested == 20.01 && maximum == 20.0
        ));
        assert!(control.writes().is_empty());
        task.abort();
    }

    #[tokio::test]
    async fn jog_pad_rejects_feed_above_selected_axis_rate() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let error = arbiter
            .jog_pad_step(JogPadStepRequest {
                confirmation: operator_confirmation(),
                axis: millo_domain::JogAxis::X,
                distance_mm: 1.0,
                feed_mm_per_min: 1_001.0,
            })
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ArbiterError::JogPadFeedExceedsAxisRate {
                axis: millo_domain::JogAxis::X,
                requested,
                maximum,
            } if requested == 1_001.0 && maximum == 1_000.0
        ));
        assert!(
            control
                .writes()
                .into_iter()
                .all(|write| !write.starts_with(b"$J="))
        );
        task.abort();
    }

    #[tokio::test]
    async fn disables_and_verifies_unhomed_controller_settings() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        control.set_setting(21, "1");
        control.set_setting(22, "1");
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        arbiter.refresh_status().await.unwrap();

        let result = arbiter.configure_unhomed_operation().await.unwrap();

        assert_eq!(result.before.settings.get("$21").unwrap(), "1");
        assert_eq!(result.before.settings.get("$22").unwrap(), "1");
        assert_eq!(result.after.settings.get("$21").unwrap(), "0");
        assert_eq!(result.after.settings.get("$22").unwrap(), "0");
        assert_eq!(result.writes.len(), 2);
        assert_eq!(
            control
                .writes()
                .into_iter()
                .filter(|write| write.starts_with(b"$21=") || write.starts_with(b"$22="))
                .collect::<Vec<_>>(),
            vec![b"$21=0\n".to_vec(), b"$22=0\n".to_vec()]
        );
        task.abort();
    }

    #[tokio::test]
    async fn stops_configuration_after_the_first_rejected_setting() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        control.set_setting(21, "1");
        control.set_setting(22, "1");
        control.queue_setting_error(2);
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        arbiter.refresh_status().await.unwrap();

        let error = arbiter.configure_unhomed_operation().await.unwrap_err();

        assert!(matches!(
            error,
            ArbiterError::Controller(ControllerError::CommandRejected { .. })
        ));
        let setting_writes = control
            .writes()
            .into_iter()
            .filter(|write| write.starts_with(b"$21=") || write.starts_with(b"$22="))
            .collect::<Vec<_>>();
        assert_eq!(setting_writes, vec![b"$21=0\n".to_vec()]);
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
    async fn return_to_work_zero_uses_absolute_jog_without_mutating_the_offset() {
        let transport = MockTransport::with_status(
            "<Idle|MPos:10.000,20.000,3.000|WPos:10.000,20.000,3.000|FS:0,0>",
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

        let outcome = arbiter
            .return_to_work_zero(return_to_zero_request(WorkAxis::Z))
            .await
            .unwrap();

        assert_eq!(outcome.coordinate_system, WorkCoordinateSystem::G55);
        assert_eq!(outcome.command, "$J=G90 G21 Z0.000 F100.000");
        assert!(
            control
                .writes()
                .contains(&b"$J=G90 G21 Z0.000 F100.000\n".to_vec())
        );
        assert!(
            !control
                .writes()
                .iter()
                .any(|write| write.starts_with(b"G10 L20"))
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

    #[tokio::test]
    async fn real_run_preflight_is_serial_only_and_performs_read_only_fresh_queries() {
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let report = arbiter
            .preflight_real_run(
                parsed_program("G21 G90 G94\nG0 Z2\nG1 X2 F20\nM5"),
                ProgramRunIntent::AirRun,
            )
            .await
            .unwrap();

        assert!(report.ready);
        assert_eq!(report.poll_sequence, 2);
        assert_eq!(
            control.writes(),
            vec![
                b"?".to_vec(),
                b"$I\n".to_vec(),
                b"$$\n".to_vec(),
                b"$G\n".to_vec(),
                b"$#\n".to_vec(),
                b"?".to_vec(),
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn real_run_preflight_rejects_non_serial_target_before_controller_io() {
        let (arbiter, control, worker) = mock_dry_run_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let error = arbiter
            .preflight_real_run(
                parsed_program("G21 G90\nG1 X1 F10"),
                ProgramRunIntent::AirRun,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, ArbiterError::RealRunTransportUnavailable));
        assert!(control.writes().is_empty());
        task.abort();
    }

    #[tokio::test]
    async fn unsafe_real_run_preflight_never_dispatches_a_program_line() {
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let report = arbiter
            .preflight_real_run(
                parsed_program("G21 G90 G94\nM3 S1000\nG1 X1 F10"),
                ProgramRunIntent::AirRun,
            )
            .await
            .unwrap();

        assert!(!report.ready);
        assert_eq!(report.program_blockers[0].source_line, Some(2));
        assert!(control.writes().iter().all(|write| matches!(
            write.as_slice(),
            b"?" | b"$I\n" | b"$$\n" | b"$G\n" | b"$#\n"
        )));
        task.abort();
    }

    #[tokio::test]
    async fn cutting_preflight_accepts_spindle_syntax_but_requires_a_check_certificate() {
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let report = arbiter
            .preflight_real_run(
                parsed_program("G21 G90 G94\nM3 S1000\nG1 X1 F10\nM5"),
                ProgramRunIntent::Cutting,
            )
            .await
            .unwrap();

        assert!(!report.ready);
        assert_eq!(report.intent, ProgramRunIntent::Cutting);
        assert!(report.checks.iter().any(|check| {
            check.id == "grbl-check-certificate" && check.level == RunPreflightLevel::Blocker
        }));
        assert!(control.writes().iter().all(|write| matches!(
            write.as_slice(),
            b"?" | b"$I\n" | b"$$\n" | b"$G\n" | b"$#\n"
        )));
        task.abort();
    }

    #[tokio::test]
    async fn safe_selected_line_air_run_also_requires_its_exact_check_certificate() {
        let source =
            "(Millo safe start from L42 of original.nc)\nG21 G90 G94 G17\nG0 Z5\nG1 X1 F10\nM5";
        let (arbiter, _control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let blocked = arbiter
            .preflight_real_run(safe_start_program(source), ProgramRunIntent::AirRun)
            .await
            .unwrap();
        assert!(!blocked.ready);
        assert!(blocked.checks.iter().any(|check| {
            check.id == "grbl-check-certificate" && check.level == RunPreflightLevel::Blocker
        }));

        arbiter
            .start_check_run(safe_start_program(source))
            .await
            .unwrap();
        wait_for_sender(&arbiter, SenderState::Completed).await;
        let certified = arbiter
            .preflight_real_run(safe_start_program(source), ProgramRunIntent::AirRun)
            .await
            .unwrap();
        assert!(certified.ready);
        assert!(certified.checks.iter().any(|check| {
            check.id == "grbl-check-certificate" && check.level == RunPreflightLevel::Pass
        }));
        task.abort();
    }

    #[tokio::test]
    async fn first_cut_authorization_repeats_preflight_and_emits_no_motion() {
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let preparation = arbiter
            .authorize_first_cut_fixture(
                parsed_program("G21 G90 G94\nG0 Z2\nG1 X2 F20\nM5"),
                first_cut_confirmation(),
            )
            .await
            .unwrap();

        assert!(preparation.report.ready);
        assert_eq!(
            preparation.authorization.program_fingerprint,
            preparation.report.program_fingerprint
        );
        assert_eq!(preparation.authorization.poll_sequence, 2);
        assert_eq!(
            control.writes(),
            vec![
                b"?".to_vec(),
                b"$I\n".to_vec(),
                b"$$\n".to_vec(),
                b"$G\n".to_vec(),
                b"$#\n".to_vec(),
                b"?".to_vec(),
            ]
        );
        assert_eq!(arbiter.sender_snapshot().state, SenderState::Idle);
        task.abort();
    }

    #[tokio::test]
    async fn completed_check_certifies_the_exact_cutting_program_and_options() {
        let source = "G21 G90 G94\nM3 S1000\nG1 X1 F10\nM5";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        arbiter
            .start_check_run(parsed_program(source))
            .await
            .unwrap();
        let checked = wait_for_sender(&arbiter, SenderState::Completed).await;
        assert_eq!(checked.mode, Some(millo_sender::SenderMode::CheckRun));
        assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);

        let report = arbiter
            .preflight_real_run(parsed_program(source), ProgramRunIntent::Cutting)
            .await
            .unwrap();
        assert!(report.ready);
        assert!(report.checks.iter().any(|check| {
            check.id == "grbl-check-certificate" && check.level == RunPreflightLevel::Pass
        }));

        let changed_options = arbiter
            .preflight_real_run_with_options(
                parsed_program(source),
                ProgramRunIntent::Cutting,
                ProgramExecutionOptions {
                    optional_stop: true,
                    block_delete: false,
                },
            )
            .await
            .unwrap();
        assert!(!changed_options.ready);
        assert!(changed_options.checks.iter().any(|check| {
            check.id == "grbl-check-certificate" && check.detail.contains("semantics changed")
        }));
        assert_eq!(
            control
                .writes()
                .iter()
                .filter(|write| write.as_slice() == b"$C\n")
                .count(),
            2
        );
        task.abort();
    }

    #[tokio::test]
    async fn cancelled_check_exits_check_mode_without_issuing_a_certificate() {
        let source = "G21 G90 G94\nM3 S1000\nG1 X1 F10\nM5";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        control.queue_program_stall();

        let started = arbiter
            .start_check_run(parsed_program(source))
            .await
            .unwrap();
        assert_eq!(started.state, SenderState::Running);

        let cancelled = arbiter.cancel_dry_run().await.unwrap();
        assert_eq!(cancelled.state, SenderState::Cancelled);
        assert_eq!(cancelled.mode, Some(millo_sender::SenderMode::CheckRun));
        assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);

        let report = arbiter
            .preflight_real_run(parsed_program(source), ProgramRunIntent::Cutting)
            .await
            .unwrap();
        assert!(!report.ready);
        assert!(report.checks.iter().any(|check| {
            check.id == "grbl-check-certificate" && check.level == RunPreflightLevel::Blocker
        }));
        assert_eq!(
            control
                .writes()
                .iter()
                .filter(|write| write.as_slice() == b"$C\n")
                .count(),
            2
        );
        task.abort();
    }

    #[tokio::test]
    async fn incomplete_first_cut_confirmation_fails_before_controller_io() {
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        let mut confirmation = first_cut_confirmation();
        confirmation.stock_secured = false;

        let error = arbiter
            .authorize_first_cut(parsed_program("G21 G90 G94\nG1 X2 F20"), confirmation)
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ArbiterError::FirstCut(FirstCutAuthorizationError::IncompleteConfirmation { .. })
        ));
        assert!(control.writes().is_empty());
        task.abort();
    }

    #[tokio::test]
    async fn serial_fixture_consumes_one_lease_and_completes_only_after_every_ok() {
        let source = "G21 G90 G94\nG0 Z2\nG1 X2 F20\nM5";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        control.set_firmware_options("V,15,256");
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let (preparation, started) =
            authorize_and_start_serial_fixture(&arbiter, source, true).await;
        let draining = wait_for_sender(&arbiter, SenderState::Draining).await;
        assert_eq!(draining.acknowledged_lines, draining.total_lines);
        arbiter.refresh_status().await.unwrap();
        let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

        assert_eq!(started.mode, Some(millo_sender::SenderMode::CutRun));
        assert_eq!(started.rx_buffer_capacity, 255);
        assert_eq!(completed.acknowledged_lines, completed.total_lines);
        assert_eq!(completed.progress, 1.0);
        let writes_before_reuse = control.writes();
        let reuse = arbiter
            .start_serial_run_fixture(parsed_program(source), preparation.authorization.id, true)
            .await
            .unwrap_err();
        assert!(matches!(
            reuse,
            ArbiterError::FirstCut(FirstCutAuthorizationError::AuthorizationMissing)
        ));
        assert_eq!(control.writes().len(), writes_before_reuse.len() + 1);
        assert_eq!(control.writes().last(), Some(&b"?".to_vec()));
        task.abort();
    }

    #[tokio::test]
    async fn production_air_run_executes_the_authorized_file_and_rejects_plain_cancel() {
        let source = include_str!("../../../fixtures/programs/air-square-20mm.nc");
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        let preparation = arbiter
            .authorize_first_cut(parsed_program(source), air_run_confirmation())
            .await
            .unwrap();

        let started = arbiter
            .start_program_run(parsed_program(source), preparation.authorization.id)
            .await
            .unwrap();
        assert_eq!(started.mode, Some(millo_sender::SenderMode::AirRun));
        wait_for_sender(&arbiter, SenderState::Draining).await;
        assert!(matches!(
            arbiter.cancel_dry_run().await.unwrap_err(),
            ArbiterError::ProgramRunStopRequiresReset
        ));

        control.set_status("<Idle|MPos:2.000,0.000,0.000|WPos:2.000,0.000,0.000|FS:0,0>");
        arbiter.refresh_status().await.unwrap();
        wait_for_sender(&arbiter, SenderState::Completed).await;
        assert!(control.writes().iter().all(|write| {
            String::from_utf8_lossy(write)
                .split_whitespace()
                .all(|word| word != "M3" && word != "M4" && !word.starts_with('S'))
        }));
        task.abort();
    }

    #[tokio::test]
    async fn serial_tool_change_is_host_managed_verified_and_cannot_be_plain_resumed() {
        let source = "G21 G90 G94\nG1 X1 F20\nT2 M6\nG1 X2 F20\nM30";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        authorize_and_start_serial_fixture(&arbiter, source, true).await;
        let barrier = wait_for_sender(&arbiter, SenderState::ToolChange).await;
        assert_eq!(barrier.current_source_line, Some(3));
        assert_eq!(barrier.requested_tool, Some(2));
        assert!(control.writes().contains(&b"N3 T2\n".to_vec()));
        assert!(!control.writes().contains(&b"N3 T2 M6\n".to_vec()));

        let writes_before_resume = control.writes();
        assert!(matches!(
            arbiter.resume_program_run().await.unwrap_err(),
            ArbiterError::Sender(SenderError::InvalidTransition {
                action: "resume",
                state: SenderState::ToolChange,
            })
        ));
        assert_eq!(control.writes(), writes_before_resume);

        let mut incomplete = tool_change_confirmation(3, Some(2));
        incomplete.z_zero_verified = false;
        assert!(matches!(
            arbiter.complete_tool_change(incomplete).await.unwrap_err(),
            ArbiterError::ToolChangeConfirmationIncomplete(_)
        ));
        assert!(matches!(
            arbiter
                .complete_tool_change(tool_change_confirmation(4, Some(2)))
                .await
                .unwrap_err(),
            ArbiterError::ToolChangeMismatch
        ));

        let resumed = arbiter
            .complete_tool_change(tool_change_confirmation(3, Some(2)))
            .await
            .unwrap();
        assert_eq!(resumed.state, SenderState::Running);
        let draining = wait_for_sender(&arbiter, SenderState::Draining).await;
        assert_eq!(draining.current_command.as_deref(), Some("M30"));
        arbiter.refresh_status().await.unwrap();
        let completed = wait_for_sender(&arbiter, SenderState::Completed).await;
        assert_eq!(completed.acknowledged_lines, completed.total_lines);
        assert!(control.writes().contains(&b"N4 G1 X2 F20\n".to_vec()));
        assert!(!control.writes().iter().any(|write| {
            String::from_utf8_lossy(write)
                .split_whitespace()
                .any(|word| word == "M6")
        }));
        task.abort();
    }

    #[tokio::test]
    async fn check_run_validates_tool_number_and_skips_the_host_only_m6_barrier() {
        let source = "G21 G90 G94\nT5 M6\nG1 X1 F20\nM30";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        arbiter
            .start_check_run(parsed_program(source))
            .await
            .unwrap();
        let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

        assert_eq!(completed.acknowledged_lines, completed.total_lines);
        assert!(control.writes().contains(&b"N2 T5\n".to_vec()));
        assert!(!control.writes().contains(&b"N4 M30\n".to_vec()));
        assert!(!control.writes().iter().any(|write| {
            String::from_utf8_lossy(write)
                .split_whitespace()
                .any(|word| word == "M6")
        }));
        task.abort();
    }

    #[tokio::test]
    async fn serial_check_run_validates_complex_geometry_and_returns_to_idle() {
        let source = include_str!("../../../fixtures/programs/grbl-complex-check.nc");
        let program = parsed_program(source);
        let plan = build_program_run_plan(&program, ProgramRunPolicy::Cutting).unwrap();
        let expected_commands = plan
            .lines()
            .iter()
            .filter(|line| line.kind() != DryRunLineKind::ProgramEnd)
            .map(|line| format!("{}\n", line.wire_command()).into_bytes())
            .collect::<Vec<_>>();
        let (arbiter, control, worker) = serial_preflight_arbiter();
        control.set_firmware_options("V,35,254");
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let started = arbiter.start_check_run(program).await.unwrap();
        assert_eq!(started.mode, Some(millo_sender::SenderMode::CheckRun));
        assert_eq!(started.rx_buffer_capacity, 253);
        let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

        assert_eq!(completed.acknowledged_lines, completed.total_lines);
        assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);
        let writes = control.writes();
        assert_eq!(
            writes
                .iter()
                .filter(|write| write.as_slice() == b"$C\n")
                .count(),
            2
        );
        let actual_commands = writes
            .iter()
            .filter(|write| {
                write.ends_with(b"\n") && !write.starts_with(b"$") && write.as_slice() != b"$C\n"
            })
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(actual_commands, expected_commands);
        assert!(!writes.contains(&b"!".to_vec()));
        assert!(!writes.contains(&b"\x18".to_vec()));
        task.abort();
    }

    #[tokio::test]
    async fn serial_check_run_accepts_cutting_spindle_syntax_without_motion_authorization() {
        let source = include_str!("../../../fixtures/programs/grbl-cutting-check.nc");
        let program = parsed_program(source);
        assert!(program.features.has_spindle_activation);
        assert!(build_dry_run_plan(&program).is_err());
        let expected = build_program_run_plan(&program, ProgramRunPolicy::Cutting)
            .unwrap()
            .lines()
            .iter()
            .filter(|line| line.kind() != DryRunLineKind::ProgramEnd)
            .map(|line| format!("{}\n", line.wire_command()).into_bytes())
            .collect::<Vec<_>>();
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        arbiter.start_check_run(program).await.unwrap();
        let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

        assert_eq!(completed.acknowledged_lines, completed.total_lines);
        assert_eq!(completed.total_lines, expected.len() + 1);
        assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);
        let actual = control
            .writes()
            .into_iter()
            .filter(|write| {
                write.ends_with(b"\n") && !write.starts_with(b"$") && write.as_slice() != b"$C\n"
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert!(actual.iter().any(|line| {
            String::from_utf8_lossy(line)
                .split_whitespace()
                .collect::<Vec<_>>()
                .ends_with(&["S12000", "M3"])
        }));
        assert!(actual.iter().any(|line| {
            String::from_utf8_lossy(line)
                .split_whitespace()
                .collect::<Vec<_>>()
                .ends_with(&["S6000", "M4"])
        }));
        task.abort();
    }

    #[tokio::test]
    async fn serial_check_run_applies_bound_optional_stop_block_delete_and_checksums() {
        let source = include_str!("../../../fixtures/programs/grbl-stream-semantics-check.nc");
        let options = ProgramExecutionOptions {
            optional_stop: true,
            block_delete: true,
        };
        let program = parsed_program_with_options(source, options);
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        arbiter
            .start_check_run_with_options(program, options)
            .await
            .unwrap();
        let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

        assert_eq!(completed.acknowledged_lines, completed.total_lines);
        let commands = control
            .writes()
            .into_iter()
            .filter(|write| {
                write.ends_with(b"\n") && !write.starts_with(b"$") && write.as_slice() != b"$C\n"
            })
            .map(|write| String::from_utf8(write).unwrap())
            .collect::<Vec<_>>();
        assert!(commands.contains(&"N5 M1\n".to_owned()));
        assert!(!commands.iter().any(|line| line.starts_with("N3 ")));
        assert!(!commands.iter().any(|line| line.starts_with("N50 ")));
        assert!(commands.iter().all(|line| !line.contains('*')));
        assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);
        task.abort();
    }

    #[tokio::test]
    async fn serial_check_run_exits_check_mode_after_a_correlated_error() {
        let (arbiter, control, worker) = serial_preflight_arbiter();
        control.queue_program_ok();
        control.queue_program_ok();
        control.queue_program_error(33);
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        arbiter
            .start_check_run(parsed_program("G21 G90 G94\nG1 X1 F10"))
            .await
            .unwrap();
        let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

        assert_eq!(failed.current_source_line, Some(1));
        let failure = failed.failure.unwrap();
        assert_eq!(failure.kind, SenderFailureKind::GrblError);
        assert_eq!(failure.grbl_code, Some(33));
        assert_eq!(failure.source_line, Some(1));
        assert_eq!(failure.command.as_deref(), Some("G21 G90 G94"));
        assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);
        let writes = control.writes();
        assert_eq!(
            writes
                .iter()
                .filter(|write| write.as_slice() == b"$C\n")
                .count(),
            2
        );
        assert!(!writes.contains(&b"!".to_vec()));
        assert!(!writes.contains(&b"\x18".to_vec()));
        task.abort();
    }

    #[tokio::test]
    async fn check_run_rejects_mock_target_before_controller_io() {
        let (arbiter, control, worker) = mock_dry_run_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let error = arbiter
            .start_check_run(parsed_program("G21 G90 G94\nG1 X1 F10"))
            .await
            .unwrap_err();

        assert!(matches!(error, ArbiterError::CheckRunTransportUnavailable));
        assert!(control.writes().is_empty());
        task.abort();
    }

    #[tokio::test]
    async fn physical_program_end_waits_for_idle_and_survives_hold_resume() {
        let source = "G21 G90 G94\nG1 X2 F20\nM30";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, source, true).await;

        let draining = wait_for_sender(&arbiter, SenderState::Draining).await;
        assert_eq!(draining.current_command.as_deref(), Some("M30"));
        assert!(!control.writes().contains(&b"N3 M30\n".to_vec()));

        control.set_status("<Run|MPos:1.000,0.000,0.000|WPos:1.000,0.000,0.000|FS:20,0>");
        arbiter.refresh_status().await.unwrap();
        assert_eq!(arbiter.sender_snapshot().state, SenderState::Draining);
        assert!(!control.writes().contains(&b"N3 M30\n".to_vec()));

        arbiter.feed_hold().await.unwrap();
        assert_eq!(arbiter.sender_snapshot().state, SenderState::Paused);
        arbiter.resume_program_run().await.unwrap();
        assert_eq!(arbiter.sender_snapshot().state, SenderState::Draining);
        assert!(!control.writes().contains(&b"N3 M30\n".to_vec()));

        control.set_status("<Idle|MPos:2.000,0.000,0.000|WPos:2.000,0.000,0.000|FS:0,0>");
        arbiter.refresh_status().await.unwrap();
        let completed = wait_for_sender(&arbiter, SenderState::Completed).await;
        assert_eq!(completed.state, SenderState::Completed);
        assert_eq!(completed.acknowledged_lines, completed.total_lines);
        assert_eq!(
            control
                .writes()
                .iter()
                .filter(|write| write.as_slice() == b"N3 M30\n")
                .count(),
            1
        );
        task.abort();
    }

    #[tokio::test]
    async fn soft_reset_cancels_a_deferred_program_end_without_dispatching_it() {
        let source = "G21 G90 G94\nG1 X2 F20\nM30";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, source, true).await;
        wait_for_sender(&arbiter, SenderState::Draining).await;

        let challenge = arbiter.request_soft_reset().await.unwrap();
        arbiter.confirm_soft_reset(challenge.id).await.unwrap();

        assert_eq!(arbiter.sender_snapshot().state, SenderState::Cancelled);
        assert!(!control.writes().contains(&b"N3 M30\n".to_vec()));
        assert_eq!(control.writes().last(), Some(&b"\x18".to_vec()));
        task.abort();
    }

    #[tokio::test]
    async fn deferred_program_end_timeout_fails_the_correlated_line() {
        let source = "G21 G90 G94\nG1 X2 F20\nM30";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, source, true).await;
        wait_for_sender(&arbiter, SenderState::Draining).await;

        control.queue_program_stall();
        arbiter.refresh_status().await.unwrap();
        let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

        assert_eq!(failed.state, SenderState::Failed);
        assert_eq!(failed.current_command.as_deref(), Some("M30"));
        assert!(
            failed
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("timed out"))
        );
        task.abort();
    }

    #[tokio::test]
    async fn program_run_fails_on_alarm_after_all_lines_were_accepted() {
        let source = "G21 G90 G94\nG1 X2 F20";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, source, true).await;
        wait_for_sender(&arbiter, SenderState::Draining).await;

        control.set_status("<Alarm|MPos:1.000,0.000,0.000|WPos:1.000,0.000,0.000|FS:0,0>");
        arbiter.refresh_status().await.unwrap();

        assert_eq!(arbiter.sender_snapshot().state, SenderState::Failed);
        task.abort();
    }

    #[tokio::test]
    async fn program_run_fails_on_status_link_loss_while_draining() {
        let source = "G21 G90 G94\nG1 X2 F20";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, source, true).await;
        wait_for_sender(&arbiter, SenderState::Draining).await;

        control.queue_disconnect();
        assert!(arbiter.refresh_status().await.is_err());

        let failed = arbiter.sender_snapshot();
        assert_eq!(failed.state, SenderState::Failed);
        assert_eq!(arbiter.snapshot().connection, ConnectionState::Disconnected);
        assert!(
            failed
                .last_error
                .as_deref()
                .is_some_and(|value| value.contains("status failed"))
        );
        task.abort();
    }

    #[tokio::test]
    async fn serial_fixture_stops_on_correlated_error() {
        let source = "G21 G90 G94\nG1 X2 F20\nG1 X4 F20\nG1 X6 F20";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        control.queue_program_ok();
        control.queue_program_ok();
        control.queue_program_error(20);
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        authorize_and_start_serial_fixture(&arbiter, source, true).await;
        let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

        assert_eq!(failed.current_source_line, Some(1));
        assert_eq!(failed.acknowledged_lines, 2);
        let failure = failed.failure.unwrap();
        assert_eq!(failure.kind, SenderFailureKind::GrblError);
        assert_eq!(failure.grbl_code, Some(20));
        assert_eq!(failure.source_line, Some(1));
        let writes = control.writes();
        assert_eq!(
            writes[writes.len() - 2..],
            [b"!".to_vec(), b"\x18".to_vec()]
        );

        let recovered = arbiter.refresh_status().await.unwrap();
        assert_eq!(recovered.machine.mode, MachineMode::Idle);
        assert!(recovered.reset_notice.is_some());
        task.abort();
    }

    #[tokio::test]
    async fn serial_fixture_stops_on_alarm_and_keeps_alarm_state() {
        let source = "G21 G90 G94\nG1 X2 F20";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        control.queue_program_ok();
        control.queue_program_ok();
        control.queue_program_alarm(2);
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        authorize_and_start_serial_fixture(&arbiter, source, true).await;
        let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

        assert_eq!(failed.current_source_line, Some(1));
        assert_eq!(
            arbiter.snapshot().alarm.and_then(|alarm| alarm.code),
            Some(2)
        );
        task.abort();
    }

    #[tokio::test]
    async fn serial_fixture_hold_pauses_and_resume_continues_the_same_plan() {
        let source = "G21 G90 G94\nG0 Z2\nG1 X2 F20";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        let (_, started) = authorize_and_start_serial_fixture(&arbiter, source, false).await;

        assert_eq!(started.state, SenderState::Running);
        control.set_status("<Run|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:20,0>");
        let paused = arbiter.feed_hold().await.unwrap();
        assert_eq!(paused.connection, ConnectionState::Connected);
        assert_eq!(arbiter.sender_snapshot().state, SenderState::Paused);
        assert_eq!(control.writes().last(), Some(&b"!".to_vec()));

        arbiter.resume_program_run().await.unwrap();
        assert_eq!(control.writes().last(), Some(&b"~".to_vec()));
        arbiter.release_serial_run_fixture().await.unwrap();
        wait_for_sender(&arbiter, SenderState::Draining).await;
        control.set_status("<Idle|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:0,0>");
        arbiter.refresh_status().await.unwrap();
        let completed = wait_for_sender(&arbiter, SenderState::Completed).await;
        assert_eq!(completed.mode, Some(millo_sender::SenderMode::CutRun));
        task.abort();
    }

    #[tokio::test]
    async fn typed_program_pause_and_abort_stop_only_a_physical_sender() {
        let source = "G21 G90 G94\nG0 Z2\nG1 X2 F20";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, source, false).await;

        let paused = arbiter.pause_program_run().await.unwrap();
        assert_eq!(paused.state, SenderState::Paused);
        assert_eq!(control.writes().last(), Some(&b"!".to_vec()));

        let stopped = arbiter.abort_program_run().await.unwrap();
        assert_eq!(stopped.state, SenderState::Cancelled);
        assert_eq!(
            control.writes()[control.writes().len() - 2..],
            [b"!".to_vec(), b"\x18".to_vec()]
        );
        assert!(matches!(
            arbiter.abort_program_run().await.unwrap_err(),
            ArbiterError::ProgramRunStopUnavailable(SenderState::Cancelled)
        ));
        task.abort();
    }

    #[tokio::test]
    async fn prepared_physical_run_dispatches_only_after_matching_commit() {
        let source = "G21 G90 G94\nG0 Z2\nG1 X2 F20";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        let (_, prepared) = authorize_and_start_serial_fixture(&arbiter, source, false).await;

        tokio::task::yield_now().await;
        assert_eq!(prepared.state, SenderState::Running);
        assert!(!control.writes().iter().any(|write| write.starts_with(b"N")));
        assert!(matches!(
            arbiter
                .commit_prepared_program_run(prepared.run_sequence + 1)
                .await,
            Err(ArbiterError::PreparedRunMismatch { .. })
        ));
        assert!(!control.writes().iter().any(|write| write.starts_with(b"N")));

        let discarded = arbiter
            .discard_prepared_program_run(prepared.run_sequence)
            .await
            .unwrap();
        assert_eq!(discarded.state, SenderState::Cancelled);
        assert!(!control.writes().iter().any(|write| write.starts_with(b"N")));
        task.abort();
    }

    #[tokio::test]
    async fn feed_hold_preempts_a_delayed_program_response() {
        let source = "G21 G90 G94\nG1 X2 F20";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        control.queue_program_delay(20);
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, source, true).await;
        tokio::time::sleep(Duration::from_millis(15)).await;

        tokio::time::timeout(Duration::from_millis(30), arbiter.feed_hold())
            .await
            .expect("Feed Hold must preempt response waiting")
            .unwrap();

        assert_eq!(arbiter.sender_snapshot().state, SenderState::Paused);
        assert!(control.writes().contains(&b"!".to_vec()));
        let challenge = arbiter.request_soft_reset().await.unwrap();
        arbiter.confirm_soft_reset(challenge.id).await.unwrap();
        task.abort();
    }

    #[tokio::test]
    async fn realtime_overrides_preempt_sender_waiting_without_pausing_it() {
        let source = "G21 G90 G94\nG1 X2 F20";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        control.queue_program_delay(20);
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, source, true).await;
        tokio::time::sleep(Duration::from_millis(15)).await;

        arbiter
            .adjust_feed_override(OverrideAdjustment::IncreaseTen)
            .await
            .unwrap();
        arbiter
            .set_rapid_override(RapidOverrideTarget::Half)
            .await
            .unwrap();
        arbiter
            .adjust_spindle_override(OverrideAdjustment::DecreaseOne)
            .await
            .unwrap();
        let acknowledged_before_refresh = arbiter.sender_snapshot().acknowledged_lines;
        arbiter.refresh_status().await.unwrap();

        assert_eq!(arbiter.sender_snapshot().state, SenderState::Running);
        assert_eq!(
            arbiter.sender_snapshot().acknowledged_lines,
            acknowledged_before_refresh
        );
        let writes = control.writes();
        assert!(writes.contains(&vec![0x91]));
        assert!(writes.contains(&vec![0x96]));
        assert!(writes.contains(&vec![0x9d]));
        let challenge = arbiter.request_soft_reset().await.unwrap();
        arbiter.confirm_soft_reset(challenge.id).await.unwrap();
        task.abort();
    }

    #[tokio::test]
    async fn realtime_write_failure_quarantines_a_physical_sender() {
        let source = "G21 G90 G94\nG1 X2 F20";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        control.queue_program_delay(20);
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, source, true).await;
        tokio::time::sleep(Duration::from_millis(15)).await;
        control.drop_link();

        let error = arbiter.feed_hold().await.unwrap_err();

        assert!(matches!(
            error,
            ArbiterError::Controller(ControllerError::Transport(TransportError::NotConnected))
        ));
        let failed = arbiter.sender_snapshot();
        assert_eq!(failed.state, SenderState::Failed);
        assert_eq!(
            failed.failure.as_ref().map(|failure| failure.kind),
            Some(SenderFailureKind::Disconnected)
        );
        assert_eq!(arbiter.snapshot().connection, ConnectionState::Disconnected);
        let writes_after_failure = control.writes();
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(control.writes(), writes_after_failure);
        task.abort();
    }

    #[tokio::test]
    async fn physical_sender_uses_interleaved_status_as_a_liveness_heartbeat() {
        let transport = MockTransport::default();
        let control = transport.control();
        // Keep the acknowledgement pending long enough to observe a poll frame
        // deterministically, even when the test runner is under light load.
        control.queue_program_delay(80);
        let (arbiter, worker) = CommandArbiter::new_with_execution_target(
            Box::new(transport),
            ControllerConfig {
                poll_interval: Duration::from_millis(5),
                status_timeout: Duration::from_millis(20),
                command_timeout: Duration::from_millis(50),
                failures_before_recovery: 2,
            },
            HardwareProfile::first_machine(),
            ExecutionTarget::Serial,
        );
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X2 F20", true).await;
        control.set_status(
            "<Run|MPos:1.000,0.000,0.000|WPos:1.000,0.000,0.000|FS:20,0|Bf:12,200|Ov:80,50,90>",
        );

        tokio::time::timeout(Duration::from_millis(150), async {
            loop {
                let snapshot = arbiter.snapshot();
                if snapshot.machine.mode == MachineMode::Run
                    && snapshot
                        .machine
                        .overrides
                        .is_some_and(|overrides| overrides.feed_percent == 80)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("interleaved status should update live telemetry");

        tokio::time::sleep(Duration::from_millis(90)).await;
        assert_ne!(arbiter.sender_snapshot().state, SenderState::Failed);
        assert!(arbiter.sender_snapshot().in_flight_lines > 0);
        assert!(control.writes().contains(&b"?".to_vec()));
        let challenge = arbiter.request_soft_reset().await.unwrap();
        arbiter.confirm_soft_reset(challenge.id).await.unwrap();
        task.abort();
    }

    #[tokio::test]
    async fn serial_fixture_stops_when_controller_resets_during_a_program_line() {
        let source = "G21 G90 G94\nG1 X2 F20";
        let (arbiter, control, worker) = serial_preflight_arbiter();
        control.queue_program_ok();
        control.queue_program_ok();
        control.queue_program_reset("1.1h");
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, source, true).await;
        let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

        assert_eq!(failed.current_source_line, Some(1));
        assert_eq!(
            arbiter.snapshot().reset_notice.unwrap().version.as_deref(),
            Some("1.1h")
        );
        task.abort();
    }

    #[tokio::test]
    async fn serial_fixture_fails_closed_on_link_drop_during_a_program_line() {
        let source = "G21 G90 G94\nG1 X2 F20";
        let (arbiter, control, worker) =
            serial_preflight_arbiter_with_poll(Duration::from_millis(5));
        control.queue_program_ok();
        control.queue_program_ok();
        control.queue_program_disconnect();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        authorize_and_start_serial_fixture(&arbiter, source, true).await;
        let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

        assert_eq!(failed.current_source_line, Some(1));
        let failure = failed.failure.unwrap();
        assert_eq!(failure.kind, SenderFailureKind::Disconnected);
        assert_eq!(failure.source_line, Some(1));
        assert_eq!(failure.command.as_deref(), Some("G21 G90 G94"));
        assert_eq!(arbiter.snapshot().connection, ConnectionState::Disconnected);
        let writes_after_failure = control.writes();
        tokio::time::sleep(Duration::from_millis(30)).await;
        assert_eq!(control.writes(), writes_after_failure);

        arbiter.connect().await.unwrap();
        arbiter.refresh_status().await.unwrap();
        assert_eq!(arbiter.sender_snapshot().state, SenderState::Failed);
        assert!(
            !control.writes()[writes_after_failure.len()..]
                .iter()
                .any(|write| write.starts_with(b"N"))
        );
        task.abort();
    }

    #[tokio::test]
    async fn mock_actor_sends_one_policy_approved_line_per_acknowledgement() {
        let (arbiter, control, worker) = mock_dry_run_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        arbiter.refresh_status().await.unwrap();

        let started = arbiter
            .start_dry_run(dry_run_plan("G21 G90\nG0 X1\nG1 X2 F10"))
            .await
            .unwrap();
        let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

        assert_eq!(started.state, SenderState::Running);
        assert_eq!(completed.acknowledged_lines, 7);
        assert_eq!(completed.total_lines, 7);
        assert!(completed.shutdown_commands_acknowledged);
        assert_eq!(
            control.writes(),
            vec![
                b"?".to_vec(),
                b"M5\n".to_vec(),
                b"M9\n".to_vec(),
                b"N1 G21 G90\n".to_vec(),
                b"N2 G0 X1\n".to_vec(),
                b"N3 G1 X2 F10\n".to_vec(),
                b"M5\n".to_vec(),
                b"M9\n".to_vec(),
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn mock_actor_prefills_but_never_overruns_the_grbl_rx_buffer() {
        let source = (0..40)
            .map(|index| format!("G1 X{index} F100"))
            .collect::<Vec<_>>()
            .join("\n");
        let (arbiter, control, worker) = mock_dry_run_arbiter();
        control.queue_program_stall();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        arbiter.refresh_status().await.unwrap();
        arbiter.start_dry_run(dry_run_plan(&source)).await.unwrap();

        tokio::time::timeout(Duration::from_millis(40), async {
            loop {
                if control.writes().len() > 3 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        let writes = control.writes();
        let buffered = writes
            .iter()
            .filter(|write| write.as_slice() != b"?")
            .collect::<Vec<_>>();
        let buffered_bytes = buffered.iter().map(|write| write.len()).sum::<usize>();
        assert!(buffered.len() > 1);
        assert!(buffered_bytes <= millo_sender::DEFAULT_GRBL_RX_BUFFER_BYTES);
        let snapshot = arbiter.sender_snapshot();
        assert_eq!(snapshot.in_flight_lines, buffered.len());
        assert_eq!(snapshot.rx_buffer_bytes, buffered_bytes);

        let failed = wait_for_sender(&arbiter, SenderState::Failed).await;
        assert_eq!(failed.current_command.as_deref(), Some("M5"));
        task.abort();
    }

    #[tokio::test]
    async fn mock_actor_correlates_the_exact_rejected_fifo_line() {
        let (arbiter, control, worker) = mock_dry_run_arbiter();
        control.queue_program_ok();
        control.queue_program_ok();
        control.queue_program_error(20);
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        arbiter.refresh_status().await.unwrap();

        arbiter
            .start_dry_run(dry_run_plan("G21\nG0 X1"))
            .await
            .unwrap();
        let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

        assert_eq!(failed.current_source_line, Some(1));
        assert_eq!(failed.acknowledged_lines, 2);
        let failure = failed.failure.unwrap();
        assert_eq!(failure.kind, SenderFailureKind::GrblError);
        assert_eq!(failure.grbl_code, Some(20));
        assert_eq!(failure.source_line, Some(1));
        assert_eq!(
            control.writes(),
            vec![
                b"?".to_vec(),
                b"M5\n".to_vec(),
                b"M9\n".to_vec(),
                b"N1 G21\n".to_vec(),
                b"N2 G0 X1\n".to_vec(),
                b"M5\n".to_vec(),
                b"M9\n".to_vec(),
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn dry_run_is_rejected_when_the_actor_target_is_not_mock() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();
        arbiter.refresh_status().await.unwrap();

        let error = arbiter
            .start_dry_run(dry_run_plan("G0 X1"))
            .await
            .unwrap_err();

        assert!(matches!(error, ArbiterError::DryRunTransportUnavailable));
        assert_eq!(control.writes(), vec![b"?".to_vec()]);
        task.abort();
    }
}
