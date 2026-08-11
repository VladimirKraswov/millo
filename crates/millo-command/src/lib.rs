use std::{future::Future, time::Instant};

use millo_controller::{
    Controller, ControllerConfig, ControllerError, RealtimeCommand, UnhomedSetting,
};
use millo_domain::{
    CommandResponse, ConnectionState, ControllerSnapshot, DeviceInspection, HardwareInspection,
    HardwareProfile, JogPadStepOutcome, JogPadStepRequest, MachineMode, OperatorConfirmation,
    ResetChallenge, StepJogReceipt, StepJogRequest, TestJogPreparation,
};
use millo_readiness::assess;
use millo_safety::{SafetyError, SafetyManager};
use millo_transport::BoxedTransport;
use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::{MissedTickBehavior, interval},
};

const REQUEST_CAPACITY: usize = 32;
pub const JOG_PAD_STEPS_MM: [f64; 2] = [0.01, 0.1];
pub const JOG_PAD_FEED_MM_PER_MIN: f64 = 10.0;

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
}

impl CommandArbiter {
    pub fn new(
        transport: BoxedTransport,
        config: ControllerConfig,
        hardware_profile: HardwareProfile,
    ) -> (Self, impl Future<Output = ()> + Send + 'static) {
        let controller = Controller::with_config(transport, config);
        let initial_snapshot = controller.snapshot();
        let (requests, request_rx) = mpsc::channel(REQUEST_CAPACITY);
        let (snapshot_tx, snapshots) = watch::channel(initial_snapshot);
        let worker = run_actor(
            controller,
            config,
            hardware_profile,
            request_rx,
            snapshot_tx,
        );

        (
            Self {
                requests,
                snapshots,
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

    pub async fn replace_transport(
        &self,
        transport: BoxedTransport,
    ) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::ReplaceTransport {
            transport,
            response,
        })
        .await
    }

    pub async fn connect(&self) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::Connect { response }).await
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
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
    },
    Connect {
        response: oneshot::Sender<Result<ControllerSnapshot, ArbiterError>>,
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
}

async fn run_actor(
    mut controller: Controller<BoxedTransport>,
    config: ControllerConfig,
    hardware_profile: HardwareProfile,
    mut requests: mpsc::Receiver<Request>,
    snapshots: watch::Sender<ControllerSnapshot>,
) {
    let mut safety = SafetyManager::default();
    let mut ticker = interval(config.poll_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker.tick().await;

    loop {
        tokio::select! {
            biased;
            request = requests.recv() => {
                let Some(request) = request else {
                    break;
                };
                handle_request(
                    request,
                    &mut controller,
                    config,
                    &hardware_profile,
                    &mut safety,
                    &snapshots,
                )
                .await;
            }
            _ = ticker.tick() => {
                if matches!(
                    controller.snapshot().connection,
                    ConnectionState::Connected | ConnectionState::Recovering
                ) {
                    let _ = controller.lifecycle_tick().await;
                    safety.observe(&controller.snapshot(), Instant::now());
                    publish(&snapshots, &controller);
                }
            }
        }
    }
}

async fn handle_request(
    request: Request,
    controller: &mut Controller<BoxedTransport>,
    config: ControllerConfig,
    hardware_profile: &HardwareProfile,
    safety: &mut SafetyManager,
    snapshots: &watch::Sender<ControllerSnapshot>,
) {
    match request {
        Request::ReplaceTransport {
            transport,
            response,
        } => {
            let _ = controller.disconnect().await;
            safety.invalidate_test_jog();
            *controller = Controller::with_config(transport, config);
            publish(snapshots, controller);
            let _ = response.send(Ok(controller.snapshot()));
        }
        Request::Connect { response } => {
            safety.invalidate_test_jog();
            let result = controller.connect().await.map_err(ArbiterError::from);
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::Disconnect { response } => {
            safety.invalidate_test_jog();
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
        Request::Realtime { command, response } => {
            if command != RealtimeCommand::Status {
                safety.invalidate_test_jog();
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

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
}
