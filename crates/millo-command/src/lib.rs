use std::future::Future;

use millo_controller::{Controller, ControllerConfig, ControllerError, RealtimeCommand};
use millo_domain::{ConnectionState, ControllerSnapshot, HardwareInspection, HardwareProfile};
use millo_readiness::assess;
use millo_transport::BoxedTransport;
use thiserror::Error;
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::{MissedTickBehavior, interval},
};

const REQUEST_CAPACITY: usize = 32;

#[derive(Debug, Error)]
pub enum ArbiterError {
    #[error(transparent)]
    Controller(#[from] ControllerError),
    #[error("command arbiter is no longer running")]
    Closed,
    #[error("command arbiter dropped a response")]
    ResponseDropped,
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

    pub async fn send_realtime(
        &self,
        command: RealtimeCommand,
    ) -> Result<ControllerSnapshot, ArbiterError> {
        self.call(|response| Request::Realtime { command, response })
            .await
    }

    async fn call<T>(
        &self,
        request: impl FnOnce(oneshot::Sender<Result<T, ControllerError>>) -> Request,
    ) -> Result<T, ArbiterError> {
        let (response_tx, response_rx) = oneshot::channel();
        self.requests
            .send(request(response_tx))
            .await
            .map_err(|_| ArbiterError::Closed)?;
        response_rx
            .await
            .map_err(|_| ArbiterError::ResponseDropped)?
            .map_err(ArbiterError::from)
    }
}

enum Request {
    ReplaceTransport {
        transport: BoxedTransport,
        response: oneshot::Sender<Result<ControllerSnapshot, ControllerError>>,
    },
    Connect {
        response: oneshot::Sender<Result<ControllerSnapshot, ControllerError>>,
    },
    Disconnect {
        response: oneshot::Sender<Result<ControllerSnapshot, ControllerError>>,
    },
    RefreshStatus {
        response: oneshot::Sender<Result<ControllerSnapshot, ControllerError>>,
    },
    AcknowledgeReset {
        response: oneshot::Sender<Result<ControllerSnapshot, ControllerError>>,
    },
    InspectDevice {
        response: oneshot::Sender<Result<HardwareInspection, ControllerError>>,
    },
    Realtime {
        command: RealtimeCommand,
        response: oneshot::Sender<Result<ControllerSnapshot, ControllerError>>,
    },
}

async fn run_actor(
    mut controller: Controller<BoxedTransport>,
    config: ControllerConfig,
    hardware_profile: HardwareProfile,
    mut requests: mpsc::Receiver<Request>,
    snapshots: watch::Sender<ControllerSnapshot>,
) {
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
    snapshots: &watch::Sender<ControllerSnapshot>,
) {
    match request {
        Request::ReplaceTransport {
            transport,
            response,
        } => {
            let _ = controller.disconnect().await;
            *controller = Controller::with_config(transport, config);
            publish(snapshots, controller);
            let _ = response.send(Ok(controller.snapshot()));
        }
        Request::Connect { response } => {
            let result = controller.connect().await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::Disconnect { response } => {
            let result = controller.disconnect().await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::RefreshStatus { response } => {
            let result = controller.refresh_status().await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::AcknowledgeReset { response } => {
            let result = Ok(controller.acknowledge_reset());
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::InspectDevice { response } => {
            let result = controller.inspect_device().await.map(|device| {
                let readiness = assess(hardware_profile, &device, &controller.snapshot());
                HardwareInspection { device, readiness }
            });
            publish(snapshots, controller);
            let _ = response.send(result);
        }
        Request::Realtime { command, response } => {
            let result = controller.send_realtime(command).await;
            publish(snapshots, controller);
            let _ = response.send(result);
        }
    }
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
}
