use std::time::Duration;

use millo_domain::{
    AlarmState, CommandCompletion, CommandResponse, ConnectionState, ControllerSnapshot,
    DeviceInspection, MachineMode, MachineState, ResetNotice, StepJogReceipt, StepJogRequest,
    WorkAxis, WorkCoordinateSystem,
};
use millo_dry_run::DryRunLine;
use millo_grbl::{
    IncomingLine, JogValidationError, StatusParseError, build_device_inspection,
    encode_set_work_zero, encode_step_jog, parse_incoming_line,
};
use millo_settings::ValidatedSettingWrite;
use millo_transport::{Transport, TransportError};
use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub struct ControllerConfig {
    pub poll_interval: Duration,
    pub status_timeout: Duration,
    pub command_timeout: Duration,
    pub failures_before_recovery: u32,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            poll_interval: Duration::from_millis(250),
            status_timeout: Duration::from_millis(500),
            command_timeout: Duration::from_secs(2),
            failures_before_recovery: 2,
        }
    }
}

#[derive(Debug, Error)]
pub enum ControllerError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Status(#[from] StatusParseError),
    #[error("controller status timed out after {timeout_ms} ms")]
    StatusTimeout { timeout_ms: u64 },
    #[error("controller command timed out after {timeout_ms} ms")]
    CommandTimeout { timeout_ms: u64 },
    #[error("controller is not ready for polling: {0:?}")]
    NotReady(ConnectionState),
    #[error("controller rejected status request: {0}")]
    Device(String),
    #[error(transparent)]
    JogValidation(#[from] JogValidationError),
    #[error("controller rejected '{command}' with {completion:?} (code {code:?})")]
    CommandRejected {
        command: String,
        completion: CommandCompletion,
        code: Option<u16>,
    },
    #[error("alarm unlock is available only in Alarm, current mode is {0:?}")]
    UnlockUnavailable(MachineMode),
    #[error("alarm unlock verification expected Idle, got {0:?}")]
    UnlockVerification(MachineMode),
    #[error("cannot {action} GRBL Check mode from {mode:?}")]
    CheckModeUnavailable {
        action: &'static str,
        mode: MachineMode,
    },
    #[error("GRBL Check mode verification expected {expected:?}, got {actual:?}")]
    CheckModeVerification {
        expected: MachineMode,
        actual: MachineMode,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceQuery {
    BuildInfo,
    Settings,
    ModalState,
    Parameters,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnhomedSetting {
    HardLimits,
    Homing,
}

impl UnhomedSetting {
    const fn disable_command(self) -> &'static str {
        match self {
            Self::HardLimits => "$21=0",
            Self::Homing => "$22=0",
        }
    }
}

impl DeviceQuery {
    pub const ALL: [Self; 4] = [
        Self::BuildInfo,
        Self::Settings,
        Self::ModalState,
        Self::Parameters,
    ];

    pub const fn command(self) -> &'static str {
        match self {
            Self::BuildInfo => "$I",
            Self::Settings => "$$",
            Self::ModalState => "$G",
            Self::Parameters => "$#",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealtimeCommand {
    Status,
    FeedHold,
    CycleStart,
    JogCancel,
    SoftReset,
}

impl RealtimeCommand {
    const fn byte(self) -> u8 {
        match self {
            Self::Status => b'?',
            Self::FeedHold => b'!',
            Self::CycleStart => b'~',
            Self::JogCancel => 0x85,
            Self::SoftReset => 0x18,
        }
    }
}

pub struct Controller<T> {
    transport: T,
    config: ControllerConfig,
    snapshot: ControllerSnapshot,
}

impl<T: Transport> Controller<T> {
    pub fn new(transport: T) -> Self {
        Self::with_config(transport, ControllerConfig::default())
    }

    pub fn with_config(transport: T, config: ControllerConfig) -> Self {
        let config = ControllerConfig {
            failures_before_recovery: config.failures_before_recovery.max(1),
            ..config
        };
        let snapshot = ControllerSnapshot {
            poll_interval_ms: duration_ms(config.poll_interval),
            status_timeout_ms: duration_ms(config.status_timeout),
            failure_threshold: config.failures_before_recovery,
            ..ControllerSnapshot::default()
        };

        Self {
            transport,
            config,
            snapshot,
        }
    }

    pub fn snapshot(&self) -> ControllerSnapshot {
        self.snapshot.clone()
    }

    pub fn poll_interval(&self) -> Duration {
        self.config.poll_interval
    }

    pub async fn connect(&mut self) -> Result<ControllerSnapshot, ControllerError> {
        self.snapshot.connection = ConnectionState::Connecting;
        self.snapshot.last_error = None;
        self.snapshot.consecutive_failures = 0;
        self.snapshot.reconnect_count = 0;
        self.snapshot.poll_sequence = 0;
        self.snapshot.reset_count = 0;
        self.snapshot.machine = MachineState::default();
        self.snapshot.reset_notice = None;
        self.snapshot.alarm = None;

        if let Err(error) = self.transport.connect().await {
            self.snapshot.connection = ConnectionState::Faulted;
            self.snapshot.last_error = Some(error.to_string());
            return Err(error.into());
        }

        self.snapshot.connection = ConnectionState::Connected;
        Ok(self.snapshot())
    }

    pub async fn disconnect(&mut self) -> Result<ControllerSnapshot, ControllerError> {
        if let Err(error) = self.transport.disconnect().await {
            self.snapshot.connection = ConnectionState::Faulted;
            self.snapshot.last_error = Some(error.to_string());
            return Err(error.into());
        }

        self.snapshot.connection = ConnectionState::Disconnected;
        self.snapshot.machine = MachineState::default();
        self.snapshot.reset_notice = None;
        self.snapshot.alarm = None;
        self.snapshot.consecutive_failures = 0;
        self.snapshot.reconnect_count = 0;
        self.snapshot.poll_sequence = 0;
        self.snapshot.reset_count = 0;
        self.snapshot.last_error = None;
        Ok(self.snapshot())
    }

    pub fn acknowledge_reset(&mut self) -> ControllerSnapshot {
        self.snapshot.reset_notice = None;
        self.snapshot()
    }

    pub async fn refresh_status(&mut self) -> Result<ControllerSnapshot, ControllerError> {
        if self.snapshot.connection != ConnectionState::Connected {
            return Err(ControllerError::NotReady(self.snapshot.connection));
        }

        match self.request_status().await {
            Ok(()) => {
                self.record_poll_success();
                Ok(self.snapshot())
            }
            Err(error) => {
                self.record_poll_failure(&error);
                Err(error)
            }
        }
    }

    pub async fn lifecycle_tick(&mut self) -> Result<ControllerSnapshot, ControllerError> {
        match self.snapshot.connection {
            ConnectionState::Connected => self.refresh_status().await,
            ConnectionState::Recovering => self.recover().await,
            state => Err(ControllerError::NotReady(state)),
        }
    }

    pub async fn inspect_device(&mut self) -> Result<DeviceInspection, ControllerError> {
        let mut responses = Vec::with_capacity(DeviceQuery::ALL.len());
        for query in DeviceQuery::ALL {
            responses.push(self.query_device(query).await?);
        }
        Ok(build_device_inspection(responses))
    }

    pub async fn query_device(
        &mut self,
        query: DeviceQuery,
    ) -> Result<CommandResponse, ControllerError> {
        if self.snapshot.connection != ConnectionState::Connected {
            return Err(ControllerError::NotReady(self.snapshot.connection));
        }

        let timeout = self.config.command_timeout;
        let result =
            match tokio::time::timeout(timeout, self.line_command_inner(query.command())).await {
                Ok(result) => result,
                Err(_) => Err(ControllerError::CommandTimeout {
                    timeout_ms: duration_ms(timeout),
                }),
            };

        match result {
            Ok(response) => {
                self.snapshot.consecutive_failures = 0;
                self.snapshot.last_error = None;
                Ok(response)
            }
            Err(error) => {
                self.record_poll_failure(&error);
                Err(error)
            }
        }
    }

    pub async fn step_jog(
        &mut self,
        request: StepJogRequest,
    ) -> Result<StepJogReceipt, ControllerError> {
        let command = encode_step_jog(request)?;
        self.execute_acknowledged_line(&command).await?;
        Ok(StepJogReceipt {
            command,
            axis: request.axis,
            distance_mm: request.distance_mm,
            feed_mm_per_min: request.feed_mm_per_min,
        })
    }

    pub async fn disable_unhomed_setting(
        &mut self,
        setting: UnhomedSetting,
    ) -> Result<CommandResponse, ControllerError> {
        self.execute_acknowledged_line(setting.disable_command())
            .await
    }

    pub async fn write_setting(
        &mut self,
        setting: &ValidatedSettingWrite,
    ) -> Result<CommandResponse, ControllerError> {
        self.execute_acknowledged_line(setting.command()).await
    }

    pub async fn set_work_zero(
        &mut self,
        axis: WorkAxis,
        coordinate_system: WorkCoordinateSystem,
    ) -> Result<CommandResponse, ControllerError> {
        let command = encode_set_work_zero(axis, coordinate_system);
        self.execute_acknowledged_line(&command).await
    }

    pub async fn unlock_alarm(&mut self) -> Result<ControllerSnapshot, ControllerError> {
        let before = self.refresh_status().await?;
        if before.machine.mode != MachineMode::Alarm {
            return Err(ControllerError::UnlockUnavailable(before.machine.mode));
        }
        self.execute_acknowledged_line("$X").await?;
        let after = self.refresh_status().await?;
        if after.machine.mode != MachineMode::Idle || after.alarm.is_some() {
            return Err(ControllerError::UnlockVerification(after.machine.mode));
        }
        Ok(after)
    }

    pub async fn set_check_mode(
        &mut self,
        enabled: bool,
    ) -> Result<ControllerSnapshot, ControllerError> {
        let before = self.refresh_status().await?;
        let expected = if enabled {
            MachineMode::Check
        } else {
            MachineMode::Idle
        };
        if before.machine.mode == expected {
            return Ok(before);
        }
        let allowed = if enabled {
            before.machine.mode == MachineMode::Idle
        } else {
            before.machine.mode == MachineMode::Check
        };
        if !allowed {
            return Err(ControllerError::CheckModeUnavailable {
                action: if enabled { "enable" } else { "disable" },
                mode: before.machine.mode,
            });
        }

        self.execute_acknowledged_line("$C").await?;
        let after = self.refresh_status().await?;
        if after.machine.mode != expected {
            return Err(ControllerError::CheckModeVerification {
                expected,
                actual: after.machine.mode,
            });
        }
        Ok(after)
    }

    pub async fn execute_program_line(
        &mut self,
        line: &DryRunLine,
    ) -> Result<CommandResponse, ControllerError> {
        self.execute_acknowledged_line(line.command()).await
    }

    pub async fn write_program_line(&mut self, line: &DryRunLine) -> Result<(), ControllerError> {
        if self.snapshot.connection != ConnectionState::Connected {
            return Err(ControllerError::NotReady(self.snapshot.connection));
        }
        let timeout = self.config.command_timeout;
        match tokio::time::timeout(
            timeout,
            self.transport
                .write(format!("{}\n", line.command()).as_bytes()),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                let error = ControllerError::from(error);
                self.record_poll_failure(&error);
                Err(error)
            }
            Err(_) => {
                let error = ControllerError::CommandTimeout {
                    timeout_ms: duration_ms(timeout),
                };
                self.record_poll_failure(&error);
                Err(error)
            }
        }
    }

    pub async fn read_program_response(
        &mut self,
        line: &DryRunLine,
    ) -> Result<CommandResponse, ControllerError> {
        self.read_acknowledged_response(line.command()).await
    }

    pub async fn send_realtime(
        &mut self,
        command: RealtimeCommand,
    ) -> Result<ControllerSnapshot, ControllerError> {
        if command == RealtimeCommand::Status {
            return self.refresh_status().await;
        }
        if self.snapshot.connection != ConnectionState::Connected {
            return Err(ControllerError::NotReady(self.snapshot.connection));
        }
        self.transport.write(&[command.byte()]).await?;
        Ok(self.snapshot())
    }

    pub async fn abort_program_stream(&mut self) -> Result<ControllerSnapshot, ControllerError> {
        if self.snapshot.connection != ConnectionState::Connected {
            return Err(ControllerError::NotReady(self.snapshot.connection));
        }

        if let Err(error) = self
            .transport
            .write(&[RealtimeCommand::FeedHold.byte()])
            .await
        {
            let error = ControllerError::from(error);
            self.record_poll_failure(&error);
            return Err(error);
        }
        if let Err(error) = self
            .transport
            .write(&[RealtimeCommand::SoftReset.byte()])
            .await
        {
            let error = ControllerError::from(error);
            self.record_poll_failure(&error);
            return Err(error);
        }

        self.snapshot.consecutive_failures = 0;
        self.snapshot.last_error = None;
        Ok(self.snapshot())
    }

    async fn recover(&mut self) -> Result<ControllerSnapshot, ControllerError> {
        self.snapshot.connection = ConnectionState::Recovering;
        let _ = self.transport.disconnect().await;

        if let Err(error) = self.transport.connect().await {
            return self.record_recovery_failure(error.into());
        }

        if let Err(error) = self.request_status().await {
            return self.record_recovery_failure(error);
        }

        self.snapshot.connection = ConnectionState::Connected;
        self.snapshot.consecutive_failures = 0;
        self.snapshot.reconnect_count = self.snapshot.reconnect_count.saturating_add(1);
        self.snapshot.last_error = None;
        Ok(self.snapshot())
    }

    async fn request_status(&mut self) -> Result<(), ControllerError> {
        let timeout = self.config.status_timeout;
        match tokio::time::timeout(timeout, self.request_status_inner()).await {
            Ok(result) => result,
            Err(_) => Err(ControllerError::StatusTimeout {
                timeout_ms: duration_ms(timeout),
            }),
        }
    }

    async fn request_status_inner(&mut self) -> Result<(), ControllerError> {
        self.transport.write(b"?").await?;

        loop {
            let line = self.transport.read_line().await?;
            match parse_incoming_line(&line)? {
                IncomingLine::Status(state) => {
                    self.apply_status(*state);
                    return Ok(());
                }
                IncomingLine::ResetBanner { raw, version } => {
                    self.apply_reset_banner(raw, version);
                }
                IncomingLine::Alarm { code, raw } => {
                    self.apply_alarm(code, raw);
                }
                IncomingLine::Error { raw, .. } => {
                    return Err(ControllerError::Device(raw));
                }
                IncomingLine::Ok | IncomingLine::Message(_) => {}
            }
        }
    }

    async fn line_command_inner(
        &mut self,
        command: &str,
    ) -> Result<CommandResponse, ControllerError> {
        self.transport
            .write(format!("{command}\n").as_bytes())
            .await?;
        self.command_response_inner(command).await
    }

    async fn command_response_inner(
        &mut self,
        command: &str,
    ) -> Result<CommandResponse, ControllerError> {
        let mut lines = Vec::new();

        loop {
            let line = self.transport.read_line().await?;
            match parse_incoming_line(&line)? {
                IncomingLine::Status(state) => self.apply_status(*state),
                IncomingLine::ResetBanner { raw, version } => {
                    self.apply_reset_banner(raw.clone(), version);
                    lines.push(raw);
                    return Ok(CommandResponse {
                        command: command.to_owned(),
                        completion: CommandCompletion::Reset,
                        lines,
                        code: None,
                    });
                }
                IncomingLine::Alarm { code, raw } => {
                    self.apply_alarm(code, raw.clone());
                    lines.push(raw);
                    return Ok(CommandResponse {
                        command: command.to_owned(),
                        completion: CommandCompletion::Alarm,
                        lines,
                        code,
                    });
                }
                IncomingLine::Error { code, raw } => {
                    lines.push(raw);
                    return Ok(CommandResponse {
                        command: command.to_owned(),
                        completion: CommandCompletion::Error,
                        lines,
                        code,
                    });
                }
                IncomingLine::Ok => {
                    return Ok(CommandResponse {
                        command: command.to_owned(),
                        completion: CommandCompletion::Ok,
                        lines,
                        code: None,
                    });
                }
                IncomingLine::Message(line) if !line.is_empty() => lines.push(line),
                IncomingLine::Message(_) => {}
            }
        }
    }

    async fn execute_acknowledged_line(
        &mut self,
        command: &str,
    ) -> Result<CommandResponse, ControllerError> {
        if self.snapshot.connection != ConnectionState::Connected {
            return Err(ControllerError::NotReady(self.snapshot.connection));
        }

        let timeout = self.config.command_timeout;
        let response = match tokio::time::timeout(timeout, self.line_command_inner(command)).await {
            Ok(result) => match result {
                Ok(response) => response,
                Err(error) => {
                    self.record_poll_failure(&error);
                    return Err(error);
                }
            },
            Err(_) => {
                let error = ControllerError::CommandTimeout {
                    timeout_ms: duration_ms(timeout),
                };
                self.record_poll_failure(&error);
                return Err(error);
            }
        };

        if response.completion != CommandCompletion::Ok {
            return Err(ControllerError::CommandRejected {
                command: response.command,
                completion: response.completion,
                code: response.code,
            });
        }

        self.snapshot.consecutive_failures = 0;
        self.snapshot.last_error = None;
        Ok(response)
    }

    async fn read_acknowledged_response(
        &mut self,
        command: &str,
    ) -> Result<CommandResponse, ControllerError> {
        if self.snapshot.connection != ConnectionState::Connected {
            return Err(ControllerError::NotReady(self.snapshot.connection));
        }
        let timeout = self.config.command_timeout;
        let response =
            match tokio::time::timeout(timeout, self.command_response_inner(command)).await {
                Ok(result) => match result {
                    Ok(response) => response,
                    Err(error) => {
                        self.record_poll_failure(&error);
                        return Err(error);
                    }
                },
                Err(_) => {
                    let error = ControllerError::CommandTimeout {
                        timeout_ms: duration_ms(timeout),
                    };
                    self.record_poll_failure(&error);
                    return Err(error);
                }
            };
        if response.completion != CommandCompletion::Ok {
            return Err(ControllerError::CommandRejected {
                command: response.command,
                completion: response.completion,
                code: response.code,
            });
        }
        self.snapshot.consecutive_failures = 0;
        self.snapshot.last_error = None;
        Ok(response)
    }

    fn apply_status(&mut self, state: MachineState) {
        if state.mode == MachineMode::Alarm {
            self.snapshot.alarm.get_or_insert_with(|| AlarmState {
                code: None,
                message: "Controller reported Alarm state".to_owned(),
            });
        } else {
            self.snapshot.alarm = None;
        }
        self.snapshot.machine = state;
    }

    fn apply_reset_banner(&mut self, banner: String, version: Option<String>) {
        self.snapshot.reset_count = self.snapshot.reset_count.saturating_add(1);
        self.snapshot.machine = MachineState::default();
        self.snapshot.alarm = None;
        self.snapshot.reset_notice = Some(ResetNotice {
            banner,
            version,
            sequence: self.snapshot.reset_count,
        });
    }

    fn apply_alarm(&mut self, code: Option<u16>, message: String) {
        self.snapshot.machine.mode = MachineMode::Alarm;
        self.snapshot.machine.reported_mode = "Alarm".to_owned();
        self.snapshot.alarm = Some(AlarmState { code, message });
    }

    fn record_poll_success(&mut self) {
        self.snapshot.connection = ConnectionState::Connected;
        self.snapshot.consecutive_failures = 0;
        self.snapshot.poll_sequence = self.snapshot.poll_sequence.saturating_add(1);
        self.snapshot.last_error = None;
    }

    fn record_poll_failure(&mut self, error: &ControllerError) {
        self.snapshot.consecutive_failures = self.snapshot.consecutive_failures.saturating_add(1);
        self.snapshot.last_error = Some(error.to_string());

        if matches!(
            error,
            ControllerError::Transport(TransportError::NotConnected)
        ) || self.snapshot.consecutive_failures >= self.config.failures_before_recovery
        {
            self.snapshot.connection = ConnectionState::Recovering;
        }
    }

    fn record_recovery_failure(
        &mut self,
        error: ControllerError,
    ) -> Result<ControllerSnapshot, ControllerError> {
        self.snapshot.connection = ConnectionState::Recovering;
        self.snapshot.consecutive_failures = self.snapshot.consecutive_failures.saturating_add(1);
        self.snapshot.last_error = Some(error.to_string());
        Err(error)
    }
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().try_into().unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use millo_domain::{ConnectionState, MachineMode};
    use millo_mock::{MockControl, MockTransport};

    use super::*;

    fn test_controller() -> (Controller<MockTransport>, MockControl) {
        let transport = MockTransport::default();
        let control = transport.control();
        let controller = Controller::with_config(
            transport,
            ControllerConfig {
                poll_interval: Duration::from_millis(10),
                status_timeout: Duration::from_millis(5),
                command_timeout: Duration::from_millis(20),
                failures_before_recovery: 2,
            },
        );
        (controller, control)
    }

    #[tokio::test]
    async fn completes_the_mock_status_round_trip() {
        let transport = MockTransport::with_status(
            "<Run|MPos:10.000,20.000,-1.500|WPos:1.000,2.000,-1.500|FS:300,8000>",
        );
        let mut controller = Controller::new(transport);

        controller.connect().await.unwrap();
        let snapshot = controller.refresh_status().await.unwrap();

        assert_eq!(snapshot.connection, ConnectionState::Connected);
        assert_eq!(snapshot.machine.mode, MachineMode::Run);
        assert_eq!(snapshot.machine.machine_position.unwrap().x, 10.0);
        assert_eq!(snapshot.machine.feed_rate, 300.0);
        assert_eq!(snapshot.machine.spindle_speed, 8000.0);
        assert_eq!(snapshot.poll_sequence, 1);
    }

    #[tokio::test]
    async fn ignores_status_requests_before_connect() {
        let mut controller = Controller::new(MockTransport::default());

        let error = controller.refresh_status().await.unwrap_err();

        assert!(matches!(
            error,
            ControllerError::NotReady(ConnectionState::Disconnected)
        ));
        assert_eq!(
            controller.snapshot().connection,
            ConnectionState::Disconnected
        );
    }

    #[tokio::test]
    async fn captures_reset_banner_and_continues_to_status() {
        let (mut controller, control) = test_controller();
        controller.connect().await.unwrap();
        controller.refresh_status().await.unwrap();
        control.queue_reset("1.1h");

        let snapshot = controller.lifecycle_tick().await.unwrap();

        let notice = snapshot.reset_notice.unwrap();
        assert_eq!(notice.version.as_deref(), Some("1.1h"));
        assert_eq!(notice.sequence, 1);
        assert_eq!(snapshot.machine.mode, MachineMode::Idle);

        controller.acknowledge_reset();
        control.queue_reset("1.1h");
        let second_snapshot = controller.lifecycle_tick().await.unwrap();
        assert_eq!(second_snapshot.reset_notice.unwrap().sequence, 2);
        assert_eq!(second_snapshot.reset_count, 2);
    }

    #[tokio::test]
    async fn keeps_alarm_until_non_alarm_status_arrives() {
        let (mut controller, control) = test_controller();
        controller.connect().await.unwrap();
        controller.refresh_status().await.unwrap();
        control.queue_alarm(3);

        let alarm_snapshot = controller.lifecycle_tick().await.unwrap();
        assert_eq!(alarm_snapshot.machine.mode, MachineMode::Alarm);
        assert_eq!(alarm_snapshot.alarm.unwrap().code, Some(3));

        control.clear_alarm();
        let idle_snapshot = controller.lifecycle_tick().await.unwrap();
        assert_eq!(idle_snapshot.machine.mode, MachineMode::Idle);
        assert!(idle_snapshot.alarm.is_none());
    }

    #[tokio::test]
    async fn typed_alarm_unlock_requires_alarm_and_verifies_fresh_idle() {
        let transport = MockTransport::with_status(
            "<Alarm|MPos:1.000,2.000,3.000|WPos:1.000,2.000,3.000|FS:0,0>",
        );
        let control = transport.control();
        let mut controller = Controller::new(transport);
        controller.connect().await.unwrap();

        let unlocked = controller.unlock_alarm().await.unwrap();

        assert_eq!(unlocked.machine.mode, MachineMode::Idle);
        assert!(unlocked.alarm.is_none());
        assert_eq!(
            control.writes(),
            vec![b"?".to_vec(), b"$X\n".to_vec(), b"?".to_vec()]
        );
        assert!(matches!(
            controller.unlock_alarm().await.unwrap_err(),
            ControllerError::UnlockUnavailable(MachineMode::Idle)
        ));
    }

    #[tokio::test]
    async fn check_mode_is_a_verified_typed_idle_transition() {
        let transport = MockTransport::default();
        let control = transport.control();
        let mut controller = Controller::new(transport);
        controller.connect().await.unwrap();

        let enabled = controller.set_check_mode(true).await.unwrap();
        assert_eq!(enabled.machine.mode, MachineMode::Check);
        let disabled = controller.set_check_mode(false).await.unwrap();
        assert_eq!(disabled.machine.mode, MachineMode::Idle);
        assert_eq!(
            control.writes(),
            vec![
                b"?".to_vec(),
                b"$C\n".to_vec(),
                b"?".to_vec(),
                b"?".to_vec(),
                b"$C\n".to_vec(),
                b"?".to_vec(),
            ]
        );

        control.set_status("<Run|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:10,0>");
        assert!(matches!(
            controller.set_check_mode(true).await.unwrap_err(),
            ControllerError::CheckModeUnavailable {
                action: "enable",
                mode: MachineMode::Run,
            }
        ));
    }

    #[tokio::test]
    async fn recovers_after_repeated_status_timeouts() {
        let (mut controller, control) = test_controller();
        controller.connect().await.unwrap();
        controller.refresh_status().await.unwrap();
        control.queue_stall();
        control.queue_stall();

        assert!(matches!(
            controller.lifecycle_tick().await.unwrap_err(),
            ControllerError::StatusTimeout { .. }
        ));
        assert_eq!(controller.snapshot().connection, ConnectionState::Connected);
        assert_eq!(controller.snapshot().consecutive_failures, 1);

        assert!(matches!(
            controller.lifecycle_tick().await.unwrap_err(),
            ControllerError::StatusTimeout { .. }
        ));
        assert_eq!(
            controller.snapshot().connection,
            ConnectionState::Recovering
        );

        let recovered = controller.lifecycle_tick().await.unwrap();
        assert_eq!(recovered.connection, ConnectionState::Connected);
        assert_eq!(recovered.consecutive_failures, 0);
        assert_eq!(recovered.reconnect_count, 1);
        assert!(recovered.last_error.is_none());
    }

    #[tokio::test]
    async fn inspects_device_through_read_only_queries() {
        let mut controller = Controller::new(MockTransport::default());
        controller.connect().await.unwrap();

        let inspection = controller.inspect_device().await.unwrap();

        assert_eq!(
            inspection.firmware_version.as_deref(),
            Some("1.1h.20240101")
        );
        assert_eq!(
            inspection.settings.get("$30").map(String::as_str),
            Some("12000")
        );
        assert!(inspection.modal_state.contains(&"G54".to_owned()));
        assert_eq!(inspection.responses.len(), 4);
        assert!(
            inspection
                .responses
                .iter()
                .all(|response| response.completion == CommandCompletion::Ok)
        );
    }

    #[tokio::test]
    async fn routes_realtime_commands_as_single_bytes() {
        let transport = MockTransport::default();
        let control = transport.control();
        let mut controller = Controller::new(transport);
        controller.connect().await.unwrap();

        controller
            .send_realtime(RealtimeCommand::FeedHold)
            .await
            .unwrap();
        controller
            .send_realtime(RealtimeCommand::CycleStart)
            .await
            .unwrap();
        controller
            .send_realtime(RealtimeCommand::JogCancel)
            .await
            .unwrap();
        controller
            .send_realtime(RealtimeCommand::SoftReset)
            .await
            .unwrap();

        assert_eq!(
            control.writes(),
            vec![b"!".to_vec(), b"~".to_vec(), vec![0x85], vec![0x18]]
        );
    }

    #[tokio::test]
    async fn aborts_a_buffered_program_with_hold_then_soft_reset() {
        let transport = MockTransport::default();
        let control = transport.control();
        let mut controller = Controller::new(transport);
        controller.connect().await.unwrap();

        controller.abort_program_stream().await.unwrap();

        assert_eq!(control.writes(), vec![b"!".to_vec(), vec![0x18]]);
    }

    #[tokio::test]
    async fn sends_only_a_validated_typed_step_jog() {
        let (mut controller, control) = test_controller();
        controller.connect().await.unwrap();

        let receipt = controller
            .step_jog(StepJogRequest {
                authorization_id: 7,
                axis: millo_domain::JogAxis::Z,
                distance_mm: 0.1,
                feed_mm_per_min: 25.0,
            })
            .await
            .unwrap();

        assert_eq!(receipt.command, "$J=G91 G21 Z0.100 F25.000");
        assert_eq!(
            control.writes(),
            vec![b"$J=G91 G21 Z0.100 F25.000\n".to_vec()]
        );

        let invalid = controller
            .step_jog(StepJogRequest {
                authorization_id: 8,
                axis: millo_domain::JogAxis::X,
                distance_mm: 1.1,
                feed_mm_per_min: 25.0,
            })
            .await
            .unwrap_err();
        assert!(matches!(invalid, ControllerError::JogValidation(_)));
        assert_eq!(control.writes().len(), 1);
    }

    #[tokio::test]
    async fn sends_only_the_typed_work_zero_command() {
        let (mut controller, control) = test_controller();
        controller.connect().await.unwrap();

        let response = controller
            .set_work_zero(WorkAxis::Z, WorkCoordinateSystem::G57)
            .await
            .unwrap();

        assert_eq!(response.command, "G10 L20 P4 Z0");
        assert_eq!(control.writes(), vec![b"G10 L20 P4 Z0\n".to_vec()]);
    }

    #[tokio::test]
    async fn disables_only_the_two_typed_unhomed_settings() {
        let (mut controller, control) = test_controller();
        controller.connect().await.unwrap();

        let hard_limits = controller
            .disable_unhomed_setting(UnhomedSetting::HardLimits)
            .await
            .unwrap();
        let homing = controller
            .disable_unhomed_setting(UnhomedSetting::Homing)
            .await
            .unwrap();

        assert_eq!(hard_limits.command, "$21=0");
        assert_eq!(homing.command, "$22=0");
        assert_eq!(
            control.writes(),
            vec![b"$21=0\n".to_vec(), b"$22=0\n".to_vec()]
        );
    }

    #[tokio::test]
    async fn correlates_error_and_alarm_with_the_active_query() {
        let transport = MockTransport::default();
        let control = transport.control();
        let mut controller = Controller::new(transport);
        controller.connect().await.unwrap();

        control.queue_query_error(2);
        let rejected = controller
            .query_device(DeviceQuery::BuildInfo)
            .await
            .unwrap();
        assert_eq!(rejected.command, "$I");
        assert_eq!(rejected.completion, CommandCompletion::Error);
        assert_eq!(rejected.code, Some(2));

        control.queue_query_alarm(3);
        let alarmed = controller
            .query_device(DeviceQuery::Parameters)
            .await
            .unwrap();
        assert_eq!(alarmed.command, "$#");
        assert_eq!(alarmed.completion, CommandCompletion::Alarm);
        assert_eq!(alarmed.code, Some(3));
        assert_eq!(controller.snapshot().alarm.unwrap().code, Some(3));
    }
}
