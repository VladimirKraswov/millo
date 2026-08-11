use std::{future::Future, time::Instant};

use millo_controller::{
    Controller, ControllerConfig, ControllerError, RealtimeCommand, UnhomedSetting,
};
use millo_domain::{
    CommandResponse, ConnectionState, ControllerSnapshot, DeviceInspection, HardwareInspection,
    HardwareProfile, JogPadStepOutcome, JogPadStepRequest, MachineMode, OperatorConfirmation,
    Position, ResetChallenge, StepJogReceipt, StepJogRequest, TestJogPreparation, WorkAxis,
    WorkCoordinateSystem, WorkZeroOutcome, WorkZeroRequest,
};
use millo_dry_run::DryRunPlan;
use millo_gcode::GcodeProgram;
use millo_grbl::{
    active_work_coordinate_system, build_device_inspection, work_coordinate_parameter,
};
use millo_readiness::assess;
use millo_run::{RunPreflightReport, assess_real_run_preflight};
use millo_safety::{SafetyError, SafetyManager};
use millo_sender::{Sender, SenderError, SenderSnapshot, SenderState};
use millo_settings::{
    ControllerSettingEditRequest, SettingsError, VerifiedSettingUpdate, setting_values_equal,
    validate_setting_edit,
};
use millo_transport::BoxedTransport;
use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::{MissedTickBehavior, interval},
};

const REQUEST_CAPACITY: usize = 32;
pub const JOG_PAD_STEPS_MM: [f64; 2] = [0.01, 0.1];
pub const JOG_PAD_FEED_MM_PER_MIN: f64 = 10.0;
const WORK_ZERO_TOLERANCE_MM: f64 = 0.002;

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
    #[error("jog pad distance {0} mm is not one of the fixed presets")]
    UnsupportedJogPadDistance(f64),
    #[error("work zero requires explicit operator position confirmation")]
    WorkZeroConfirmationRequired,
    #[error("active work coordinate system is not one of G54-G59")]
    ActiveWorkCoordinateSystemUnavailable,
    #[error("work zero verification failed: {0}")]
    WorkZeroVerification(String),
    #[error(transparent)]
    Sender(#[from] SenderError),
    #[error("dry run is disabled for the active transport")]
    DryRunTransportUnavailable,
    #[error("real-run preflight requires the serial transport target")]
    RealRunTransportUnavailable,
    #[error("machine profile can be changed only while disconnected, current state is {0:?}")]
    ProfileChangeUnavailable(ConnectionState),
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
            safety: SafetyManager::default(),
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

    pub async fn inspect_device(&self) -> Result<HardwareInspection, ArbiterError> {
        self.call(|response| Request::InspectDevice { response })
            .await
    }

    pub async fn preflight_real_run(
        &self,
        program: GcodeProgram,
    ) -> Result<RunPreflightReport, ArbiterError> {
        self.call(|response| Request::PreflightRealRun { program, response })
            .await
    }

    pub async fn feed_hold(&self) -> Result<ControllerSnapshot, ArbiterError> {
        self.send_realtime(RealtimeCommand::FeedHold).await
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

    pub async fn send_realtime(
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
    InspectDevice {
        response: oneshot::Sender<Result<HardwareInspection, ArbiterError>>,
    },
    PreflightRealRun {
        program: GcodeProgram,
        response: oneshot::Sender<Result<RunPreflightReport, ArbiterError>>,
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
}

struct ActorState {
    controller: Controller<BoxedTransport>,
    config: ControllerConfig,
    hardware_profile: HardwareProfile,
    execution_target: ExecutionTarget,
    sender: Sender,
    safety: SafetyManager,
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
                    let _ = actor.controller.lifecycle_tick().await;
                    actor.safety.observe(&actor.controller.snapshot(), Instant::now());
                    publish(&actor.snapshots, &actor.controller);
                }
            }
            _ = tokio::task::yield_now(), if actor.sender.is_dispatchable() => {
                execute_sender_step(
                    &mut actor.controller,
                    &mut actor.sender,
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
        safety,
        snapshots,
        sender_snapshots,
    } = actor;
    match request {
        Request::ReplaceTransport {
            transport,
            execution_target: replacement_target,
            response,
        } => {
            let _ = controller.disconnect().await;
            safety.invalidate_test_jog();
            cancel_active_sender(sender, sender_snapshots);
            *controller = Controller::with_config(transport, *config);
            *execution_target = replacement_target;
            publish(snapshots, controller);
            let _ = response.send(Ok(controller.snapshot()));
        }
        Request::Connect { response } => {
            safety.invalidate_test_jog();
            cancel_active_sender(sender, sender_snapshots);
            let result = controller.connect().await.map_err(ArbiterError::from);
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::SetHardwareProfile { profile, response } => {
            let connection = controller.snapshot().connection;
            let result = if connection == ConnectionState::Disconnected {
                safety.invalidate_test_jog();
                *hardware_profile = profile;
                Ok(hardware_profile.clone())
            } else {
                Err(ArbiterError::ProfileChangeUnavailable(connection))
            };
            let _ = response.send(result);
        }
        Request::BindHardwareProfile { profile, response } => {
            let result = ensure_profile_binding_available(&controller.snapshot()).map(|()| {
                safety.invalidate_test_jog();
                *hardware_profile = profile;
                hardware_profile.clone()
            });
            let _ = response.send(result);
        }
        Request::UpdateControllerSetting { request, response } => {
            safety.invalidate_test_jog();
            let result = execute_controller_setting_update(controller, request).await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::Disconnect { response } => {
            safety.invalidate_test_jog();
            cancel_active_sender(sender, sender_snapshots);
            let result = controller.disconnect().await.map_err(ArbiterError::from);
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::RefreshStatus { response } => {
            let result = controller
                .refresh_status()
                .await
                .map_err(ArbiterError::from);
            safety.observe(&controller.snapshot(), Instant::now());
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::AcknowledgeReset { response } => {
            let result = Ok(controller.acknowledge_reset());
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
        Request::PreflightRealRun { program, response } => {
            let result = if *execution_target != ExecutionTarget::Serial {
                Err(ArbiterError::RealRunTransportUnavailable)
            } else {
                execute_real_run_preflight(controller, hardware_profile, program).await
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::Realtime { command, response } => {
            if command != RealtimeCommand::Status {
                safety.invalidate_test_jog();
            }
            if command == RealtimeCommand::FeedHold
                && sender.snapshot().state == SenderState::Running
            {
                let _ = sender.pause();
                publish_sender(sender_snapshots, sender);
            }
            if command == RealtimeCommand::SoftReset {
                cancel_active_sender(sender, sender_snapshots);
            }
            let result = controller
                .send_realtime(command)
                .await
                .map_err(ArbiterError::from);
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::BeginSoftReset { response } => {
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
            cancel_active_sender(sender, sender_snapshots);
            let result = safety
                .confirm_soft_reset(challenge_id, Instant::now())
                .map_err(ArbiterError::from);
            let result = match result {
                Ok(()) => controller
                    .send_realtime(RealtimeCommand::SoftReset)
                    .await
                    .map_err(ArbiterError::from),
                Err(error) => Err(error),
            };
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::PrepareTestJog {
            confirmation,
            response,
        } => {
            let result = prepare_test_jog(controller, hardware_profile, safety, confirmation).await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::StepJog { request, response } => {
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
            let result = execute_jog_pad_step(controller, hardware_profile, safety, request).await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::CancelJog { response } => {
            safety.invalidate_test_jog();
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
            safety.invalidate_test_jog();
            let result = configure_unhomed_operation(controller).await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::SetWorkZero { request, response } => {
            safety.invalidate_test_jog();
            let result = execute_set_work_zero(controller, request).await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::StartDryRun { plan, response } => {
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
            let result = sender.pause().map_err(ArbiterError::from);
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::ResumeDryRun { response } => {
            let result = ensure_stable_idle(&controller.snapshot())
                .and_then(|()| sender.resume().map_err(ArbiterError::from));
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
        Request::CancelDryRun { response } => {
            let result = sender.cancel().map_err(ArbiterError::from);
            publish_sender(sender_snapshots, sender);
            let _ = response.send(result);
        }
    }
}

async fn execute_real_run_preflight(
    controller: &mut Controller<BoxedTransport>,
    hardware_profile: &HardwareProfile,
    program: GcodeProgram,
) -> Result<RunPreflightReport, ArbiterError> {
    controller.refresh_status().await?;
    ensure_stable_idle(&controller.snapshot())?;
    let device = controller.inspect_device().await?;
    let snapshot = controller.refresh_status().await?;
    let readiness = assess(hardware_profile, &device, &snapshot);
    let hardware = HardwareInspection { device, readiness };
    Ok(assess_real_run_preflight(&program, hardware, &snapshot))
}

async fn execute_sender_step(
    controller: &mut Controller<BoxedTransport>,
    sender: &mut Sender,
    snapshots: &watch::Sender<ControllerSnapshot>,
    sender_snapshots: &watch::Sender<SenderSnapshot>,
) {
    if let Err(error) = ensure_stable_idle(&controller.snapshot()) {
        sender.fail(error.to_string());
        publish_sender(sender_snapshots, sender);
        return;
    }
    let Some(line) = sender.next_line() else {
        publish_sender(sender_snapshots, sender);
        return;
    };
    publish_sender(sender_snapshots, sender);

    match controller.execute_dry_run_line(&line).await {
        Ok(_) => {
            let _ = sender.acknowledge_ok();
        }
        Err(error) => {
            let _ = sender.acknowledge_error(error.to_string());
        }
    }
    publish(snapshots, controller);
    publish_sender(sender_snapshots, sender);
}

fn cancel_active_sender(sender: &mut Sender, snapshots: &watch::Sender<SenderSnapshot>) {
    if matches!(
        sender.snapshot().state,
        SenderState::Ready | SenderState::Running | SenderState::Paused
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
        .expect("validated setting must exist")
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
    if !is_supported_jog_pad_distance(request.distance_mm) {
        return Err(ArbiterError::UnsupportedJogPadDistance(request.distance_mm));
    }

    let preparation =
        prepare_test_jog(controller, hardware_profile, safety, request.confirmation).await?;
    let Some(authorization) = preparation.authorization else {
        return Ok(JogPadStepOutcome {
            inspection: preparation.inspection,
            receipt: None,
        });
    };
    let step = StepJogRequest {
        authorization_id: authorization.id,
        axis: request.axis,
        distance_mm: request.distance_mm,
        feed_mm_per_min: JOG_PAD_FEED_MM_PER_MIN,
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

fn is_supported_jog_pad_distance(distance_mm: f64) -> bool {
    distance_mm.is_finite()
        && JOG_PAD_STEPS_MM
            .iter()
            .any(|preset| distance_mm.abs() == *preset)
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

    use millo_dry_run::build_dry_run_plan;
    use millo_gcode::{ProgramParseRequest, parse_program};
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

    fn work_zero_request(axis: WorkAxis, position_confirmed: bool) -> WorkZeroRequest {
        WorkZeroRequest {
            axis,
            position_confirmed,
        }
    }

    fn dry_run_plan(source: &str) -> DryRunPlan {
        let program = parsed_program(source);
        build_dry_run_plan(&program).unwrap()
    }

    fn parsed_program(source: &str) -> GcodeProgram {
        parse_program(ProgramParseRequest {
            source_name: "sender-fixture.nc".to_owned(),
            source: source.to_owned(),
        })
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
                distance_mm: 1.01,
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
    async fn jog_pad_rechecks_motion_and_uses_only_its_fixed_feed() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let first = arbiter
            .jog_pad_step(JogPadStepRequest {
                confirmation: operator_confirmation(),
                axis: millo_domain::JogAxis::Y,
                distance_mm: 0.1,
            })
            .await
            .unwrap();
        assert_eq!(first.receipt.unwrap().command, "$J=G91 G21 Y0.100 F10.000");

        let blocked_while_moving = arbiter
            .jog_pad_step(JogPadStepRequest {
                confirmation: operator_confirmation(),
                axis: millo_domain::JogAxis::Z,
                distance_mm: 0.01,
            })
            .await
            .unwrap();
        assert!(blocked_while_moving.receipt.is_none());
        assert!(!blocked_while_moving.inspection.readiness.test_jog_ready);

        for _ in 0..4 {
            if arbiter.refresh_status().await.unwrap().machine.mode == MachineMode::Idle {
                break;
            }
        }
        let second = arbiter
            .jog_pad_step(JogPadStepRequest {
                confirmation: operator_confirmation(),
                axis: millo_domain::JogAxis::Z,
                distance_mm: -0.01,
            })
            .await
            .unwrap();
        assert_eq!(
            second.receipt.unwrap().command,
            "$J=G91 G21 Z-0.010 F10.000"
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
                b"$J=G91 G21 Y0.100 F10.000\n".to_vec(),
                b"$J=G91 G21 Z-0.010 F10.000\n".to_vec()
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn jog_pad_rejects_non_preset_distance_before_controller_io() {
        let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let error = arbiter
            .jog_pad_step(JogPadStepRequest {
                confirmation: operator_confirmation(),
                axis: millo_domain::JogAxis::X,
                distance_mm: 0.5,
            })
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            ArbiterError::UnsupportedJogPadDistance(distance) if distance == 0.5
        ));
        assert!(control.writes().is_empty());
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
    async fn real_run_preflight_is_serial_only_and_performs_read_only_fresh_queries() {
        let (arbiter, control, worker) = serial_preflight_arbiter();
        let task = tokio::spawn(worker);
        arbiter.connect().await.unwrap();

        let report = arbiter
            .preflight_real_run(parsed_program("G21 G90 G94\nG0 Z2\nG1 X2 F20\nM5"))
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
            .preflight_real_run(parsed_program("G21 G90\nG1 X1 F10"))
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
            .preflight_real_run(parsed_program("G21 G90 G94\nM3 S1000\nG1 X1 F10"))
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
        assert_eq!(completed.acknowledged_lines, 5);
        assert_eq!(completed.total_lines, 5);
        assert_eq!(
            control.writes(),
            vec![
                b"?".to_vec(),
                b"M5\n".to_vec(),
                b"M9\n".to_vec(),
                b"G21 G90\n".to_vec(),
                b"G0 X1\n".to_vec(),
                b"G1 X2 F10\n".to_vec(),
            ]
        );
        task.abort();
    }

    #[tokio::test]
    async fn mock_actor_stops_at_the_exact_rejected_program_line() {
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
        assert!(
            failed
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("code Some(20)"))
        );
        assert_eq!(
            control.writes(),
            vec![
                b"?".to_vec(),
                b"M5\n".to_vec(),
                b"M9\n".to_vec(),
                b"G21\n".to_vec(),
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
