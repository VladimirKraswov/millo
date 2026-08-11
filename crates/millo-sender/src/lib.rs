use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use millo_dry_run::{DryRunLine, DryRunLineKind, DryRunPlan, MAX_DRY_RUN_COMMAND_BYTES};
use serde::Serialize;
use thiserror::Error;

pub const MAX_SENDER_LINES: usize = 200_002;
pub const MAX_SENDER_BYTES: usize = 2 * 1024 * 1024;
pub const DEFAULT_GRBL_RX_BUFFER_BYTES: usize = 127;
pub const MAX_GRBL_RX_BUFFER_BYTES: usize = 4095;

pub fn usable_rx_buffer_capacity(reported_bytes: Option<u16>) -> usize {
    reported_bytes
        .map(|bytes| usize::from(bytes.saturating_sub(1)))
        .filter(|bytes| *bytes > 0)
        .map(|bytes| bytes.min(MAX_GRBL_RX_BUFFER_BYTES))
        .unwrap_or(DEFAULT_GRBL_RX_BUFFER_BYTES)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SenderMode {
    MockDryRun,
    CheckRun,
    AirRun,
    CutRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SenderLimits {
    pub max_lines: usize,
    pub max_bytes: usize,
    pub max_command_bytes: usize,
    pub rx_buffer_bytes: usize,
}

impl Default for SenderLimits {
    fn default() -> Self {
        Self {
            max_lines: MAX_SENDER_LINES,
            max_bytes: MAX_SENDER_BYTES,
            max_command_bytes: MAX_DRY_RUN_COMMAND_BYTES,
            rx_buffer_bytes: DEFAULT_GRBL_RX_BUFFER_BYTES,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SenderState {
    #[default]
    Idle,
    Ready,
    Running,
    Paused,
    Draining,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SenderFailureKind {
    GrblError,
    Alarm,
    Reset,
    Timeout,
    Disconnected,
    Transport,
    UnsafeState,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SenderFailure {
    pub kind: SenderFailureKind,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grbl_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

impl SenderFailure {
    pub fn new(kind: SenderFailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            grbl_code: None,
            source_line: None,
            command: None,
        }
    }

    pub fn with_grbl_code(mut self, code: Option<u16>) -> Self {
        self.grbl_code = code;
        self
    }

    fn attach_line(mut self, line: Option<&DryRunLine>) -> Self {
        if let Some(line) = line {
            self.source_line = line.source_line();
            self.command = Some(line.command().to_owned());
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SenderSnapshot {
    pub state: SenderState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<SenderMode>,
    pub source_name: Option<String>,
    pub total_lines: usize,
    pub dispatched_lines: usize,
    pub acknowledged_lines: usize,
    pub in_flight_lines: usize,
    pub rx_buffer_bytes: usize,
    pub rx_buffer_capacity: usize,
    pub current_source_line: Option<usize>,
    pub current_command: Option<String>,
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<SenderFailure>,
    pub progress: f64,
    pub elapsed_seconds: f64,
    pub estimated_completed_seconds: f64,
    pub estimated_remaining_seconds: f64,
    pub estimated_total_seconds: f64,
    pub time_estimate_complete: bool,
}

impl Default for SenderSnapshot {
    fn default() -> Self {
        Self {
            state: SenderState::Idle,
            mode: None,
            source_name: None,
            total_lines: 0,
            dispatched_lines: 0,
            acknowledged_lines: 0,
            in_flight_lines: 0,
            rx_buffer_bytes: 0,
            rx_buffer_capacity: DEFAULT_GRBL_RX_BUFFER_BYTES,
            current_source_line: None,
            current_command: None,
            last_error: None,
            failure: None,
            progress: 0.0,
            elapsed_seconds: 0.0,
            estimated_completed_seconds: 0.0,
            estimated_remaining_seconds: 0.0,
            estimated_total_seconds: 0.0,
            time_estimate_complete: false,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SenderError {
    #[error("sender cannot load a new plan while it is {0:?}")]
    Busy(SenderState),
    #[error("sender cannot {action} while it is {state:?}")]
    InvalidTransition {
        action: &'static str,
        state: SenderState,
    },
    #[error("sender plan has {actual} lines; limit is {limit}")]
    TooManyLines { actual: usize, limit: usize },
    #[error("sender plan has {actual} command bytes; limit is {limit}")]
    PlanTooLarge { actual: usize, limit: usize },
    #[error("sender command has {actual} bytes; limit is {limit}")]
    CommandTooLong { actual: usize, limit: usize },
    #[error("sender command requires {actual} RX bytes; GRBL buffer capacity is {limit}")]
    CommandExceedsRxBuffer { actual: usize, limit: usize },
    #[error("GRBL RX buffer capacity must be between 1 and {max}, got {actual}")]
    InvalidRxBufferCapacity { actual: usize, max: usize },
    #[error("sender has no command awaiting acknowledgement")]
    NoCommandInFlight,
    #[error("sender cannot complete while a command is awaiting acknowledgement")]
    CommandInFlight,
}

pub struct Sender {
    limits: SenderLimits,
    plan: Option<DryRunPlan>,
    state: SenderState,
    mode: Option<SenderMode>,
    dispatched_lines: usize,
    acknowledged_lines: usize,
    in_flight: VecDeque<DryRunLine>,
    in_flight_bytes: usize,
    deferred_program_end: Option<DryRunLine>,
    last_line: Option<DryRunLine>,
    last_error: Option<String>,
    failure: Option<SenderFailure>,
    paused_from: Option<SenderState>,
    estimated_completed_ms: u64,
    started_at: Option<Instant>,
    paused_at: Option<Instant>,
    paused_duration: Duration,
    finished_elapsed: Option<Duration>,
}

impl Default for Sender {
    fn default() -> Self {
        Self::with_limits(SenderLimits::default())
    }
}

impl Sender {
    pub fn with_limits(limits: SenderLimits) -> Self {
        Self {
            limits,
            plan: None,
            state: SenderState::Idle,
            mode: None,
            dispatched_lines: 0,
            acknowledged_lines: 0,
            in_flight: VecDeque::new(),
            in_flight_bytes: 0,
            deferred_program_end: None,
            last_line: None,
            last_error: None,
            failure: None,
            paused_from: None,
            estimated_completed_ms: 0,
            started_at: None,
            paused_at: None,
            paused_duration: Duration::ZERO,
            finished_elapsed: None,
        }
    }

    pub fn load(&mut self, plan: DryRunPlan) -> Result<SenderSnapshot, SenderError> {
        self.load_with_mode(plan, SenderMode::MockDryRun)
    }

    pub fn configure_rx_buffer_capacity(
        &mut self,
        capacity: usize,
    ) -> Result<SenderSnapshot, SenderError> {
        if matches!(
            self.state,
            SenderState::Ready | SenderState::Running | SenderState::Paused | SenderState::Draining
        ) {
            return Err(SenderError::Busy(self.state));
        }
        if !(1..=MAX_GRBL_RX_BUFFER_BYTES).contains(&capacity) {
            return Err(SenderError::InvalidRxBufferCapacity {
                actual: capacity,
                max: MAX_GRBL_RX_BUFFER_BYTES,
            });
        }
        self.limits.rx_buffer_bytes = capacity;
        Ok(self.snapshot())
    }

    pub fn load_air_run(&mut self, plan: DryRunPlan) -> Result<SenderSnapshot, SenderError> {
        self.load_with_mode(plan, SenderMode::AirRun)
    }

    pub fn load_check_run(&mut self, plan: DryRunPlan) -> Result<SenderSnapshot, SenderError> {
        self.load_with_mode(plan, SenderMode::CheckRun)
    }

    pub fn load_cut_run(&mut self, plan: DryRunPlan) -> Result<SenderSnapshot, SenderError> {
        self.load_with_mode(plan, SenderMode::CutRun)
    }

    fn load_with_mode(
        &mut self,
        plan: DryRunPlan,
        mode: SenderMode,
    ) -> Result<SenderSnapshot, SenderError> {
        if matches!(
            self.state,
            SenderState::Running | SenderState::Paused | SenderState::Draining
        ) {
            return Err(SenderError::Busy(self.state));
        }
        self.validate_plan(&plan)?;
        self.plan = Some(plan);
        self.state = SenderState::Ready;
        self.mode = Some(mode);
        self.dispatched_lines = 0;
        self.acknowledged_lines = 0;
        self.in_flight.clear();
        self.in_flight_bytes = 0;
        self.deferred_program_end = None;
        self.last_line = None;
        self.last_error = None;
        self.failure = None;
        self.paused_from = None;
        self.estimated_completed_ms = 0;
        self.started_at = None;
        self.paused_at = None;
        self.paused_duration = Duration::ZERO;
        self.finished_elapsed = None;
        Ok(self.snapshot())
    }

    pub fn start(&mut self) -> Result<SenderSnapshot, SenderError> {
        if self.state != SenderState::Ready {
            return Err(SenderError::InvalidTransition {
                action: "start",
                state: self.state,
            });
        }
        self.state = SenderState::Running;
        self.started_at = Some(Instant::now());
        Ok(self.snapshot())
    }

    pub fn pause(&mut self) -> Result<SenderSnapshot, SenderError> {
        if !matches!(self.state, SenderState::Running | SenderState::Draining) {
            return Err(SenderError::InvalidTransition {
                action: "pause",
                state: self.state,
            });
        }
        self.paused_from = Some(self.state);
        self.state = SenderState::Paused;
        self.pause_clock();
        Ok(self.snapshot())
    }

    pub fn resume(&mut self) -> Result<SenderSnapshot, SenderError> {
        if self.state != SenderState::Paused {
            return Err(SenderError::InvalidTransition {
                action: "resume",
                state: self.state,
            });
        }
        self.state = self.paused_from.take().unwrap_or(SenderState::Running);
        self.resume_clock();
        Ok(self.snapshot())
    }

    pub fn cancel(&mut self) -> Result<SenderSnapshot, SenderError> {
        if !matches!(
            self.state,
            SenderState::Ready | SenderState::Running | SenderState::Paused | SenderState::Draining
        ) {
            return Err(SenderError::InvalidTransition {
                action: "cancel",
                state: self.state,
            });
        }
        self.freeze_clock();
        self.state = SenderState::Cancelled;
        self.in_flight.clear();
        self.in_flight_bytes = 0;
        self.deferred_program_end = None;
        self.paused_from = None;
        Ok(self.snapshot())
    }

    pub fn fail(&mut self, error: impl Into<String>) -> SenderSnapshot {
        self.fail_with(SenderFailure::new(SenderFailureKind::Internal, error))
    }

    pub fn fail_with(&mut self, failure: SenderFailure) -> SenderSnapshot {
        self.freeze_clock();
        self.last_line = self
            .in_flight
            .pop_front()
            .or_else(|| self.deferred_program_end.take())
            .or_else(|| self.last_line.take());
        self.in_flight.clear();
        self.in_flight_bytes = 0;
        let failure = failure.attach_line(self.last_line.as_ref());
        self.last_error = Some(failure.message.clone());
        self.failure = Some(failure);
        self.state = SenderState::Failed;
        self.paused_from = None;
        self.snapshot()
    }

    pub fn fail_dispatched_line(
        &mut self,
        line: DryRunLine,
        error: impl Into<String>,
    ) -> SenderSnapshot {
        self.fail_dispatched_line_with(
            line,
            SenderFailure::new(SenderFailureKind::Transport, error),
        )
    }

    pub fn fail_dispatched_line_with(
        &mut self,
        line: DryRunLine,
        failure: SenderFailure,
    ) -> SenderSnapshot {
        self.freeze_clock();
        self.last_line = Some(line);
        self.in_flight.clear();
        self.in_flight_bytes = 0;
        self.deferred_program_end = None;
        let failure = failure.attach_line(self.last_line.as_ref());
        self.last_error = Some(failure.message.clone());
        self.failure = Some(failure);
        self.state = SenderState::Failed;
        self.paused_from = None;
        self.snapshot()
    }

    pub fn is_dispatchable(&self) -> bool {
        self.state == SenderState::Running
    }

    pub fn has_in_flight(&self) -> bool {
        !self.in_flight.is_empty()
    }

    pub fn needs_io(&self) -> bool {
        self.has_in_flight() || self.is_dispatchable()
    }

    pub fn oldest_in_flight(&self) -> Option<DryRunLine> {
        self.in_flight.front().cloned()
    }

    pub fn next_line(&mut self) -> Option<DryRunLine> {
        if !self.is_dispatchable() {
            return None;
        }
        if self.mode == Some(SenderMode::CheckRun) && !self.in_flight.is_empty() {
            return None;
        }
        let plan = self.plan.as_ref()?;
        if self.dispatched_lines >= plan.lines().len() {
            if self.in_flight.is_empty() {
                self.state = self.finished_state();
            }
            return None;
        }
        if self.in_flight.back().is_some_and(|line| {
            matches!(
                line.kind(),
                DryRunLineKind::ProgramPause | DryRunLineKind::ProgramEnd
            )
        }) {
            return None;
        }
        let line = plan.lines()[self.dispatched_lines].clone();
        if self.requires_motion_drain() && line.kind() == DryRunLineKind::ProgramEnd {
            if self.in_flight.is_empty() {
                self.dispatched_lines = self.dispatched_lines.saturating_add(1);
                self.deferred_program_end = Some(line);
                self.state = SenderState::Draining;
            }
            return None;
        }
        let line_bytes = command_rx_bytes(&line);
        if self.in_flight_bytes.saturating_add(line_bytes) > self.limits.rx_buffer_bytes {
            return None;
        }
        self.dispatched_lines = self.dispatched_lines.saturating_add(1);
        self.in_flight_bytes = self.in_flight_bytes.saturating_add(line_bytes);
        self.in_flight.push_back(line.clone());
        Some(line)
    }

    pub fn acknowledge_ok(&mut self) -> Result<SenderSnapshot, SenderError> {
        let line = self
            .in_flight
            .pop_front()
            .ok_or(SenderError::NoCommandInFlight)?;
        self.in_flight_bytes = self.in_flight_bytes.saturating_sub(command_rx_bytes(&line));
        let line_kind = line.kind();
        if let Some(duration_ms) = line.estimated_duration_ms() {
            self.estimated_completed_ms = self.estimated_completed_ms.saturating_add(duration_ms);
        }
        self.last_line = Some(line);
        self.acknowledged_lines = self.acknowledged_lines.saturating_add(1);
        if line_kind == DryRunLineKind::ProgramPause && self.mode != Some(SenderMode::CheckRun) {
            self.paused_from = Some(SenderState::Running);
            self.state = SenderState::Paused;
            self.pause_clock();
        } else if line_kind == DryRunLineKind::ProgramEnd
            || self.plan.as_ref().is_some_and(|plan| {
                self.acknowledged_lines == plan.lines().len() && self.in_flight.is_empty()
            })
        {
            self.state = self.finished_state();
            if self.state == SenderState::Completed {
                self.freeze_clock();
            }
        }
        Ok(self.snapshot())
    }

    pub fn dispatch_deferred_program_end(&mut self) -> Result<DryRunLine, SenderError> {
        let Some(line) = self.deferred_program_end.take() else {
            return Err(SenderError::NoCommandInFlight);
        };
        if self.state != SenderState::Draining || !self.in_flight.is_empty() {
            self.deferred_program_end = Some(line);
            return Err(SenderError::InvalidTransition {
                action: "dispatch deferred program end",
                state: self.state,
            });
        }
        self.in_flight_bytes = command_rx_bytes(&line);
        self.in_flight.push_back(line.clone());
        Ok(line)
    }

    pub fn deferred_program_end(&self) -> Option<DryRunLine> {
        self.deferred_program_end.clone()
    }

    pub fn complete_draining(&mut self) -> Result<SenderSnapshot, SenderError> {
        if self.state != SenderState::Draining {
            return Err(SenderError::InvalidTransition {
                action: "complete draining",
                state: self.state,
            });
        }
        if !self.in_flight.is_empty() || self.deferred_program_end.is_some() {
            return Err(SenderError::CommandInFlight);
        }
        self.freeze_clock();
        self.state = SenderState::Completed;
        Ok(self.snapshot())
    }

    pub fn acknowledge_error(
        &mut self,
        error: impl Into<String>,
    ) -> Result<SenderSnapshot, SenderError> {
        self.acknowledge_failure(SenderFailure::new(SenderFailureKind::GrblError, error))
    }

    pub fn acknowledge_failure(
        &mut self,
        failure: SenderFailure,
    ) -> Result<SenderSnapshot, SenderError> {
        let line = self
            .in_flight
            .pop_front()
            .ok_or(SenderError::NoCommandInFlight)?;
        self.freeze_clock();
        self.in_flight_bytes = self.in_flight_bytes.saturating_sub(command_rx_bytes(&line));
        self.last_line = Some(line);
        self.in_flight.clear();
        self.in_flight_bytes = 0;
        self.deferred_program_end = None;
        let failure = failure.attach_line(self.last_line.as_ref());
        self.last_error = Some(failure.message.clone());
        self.failure = Some(failure);
        self.state = SenderState::Failed;
        Ok(self.snapshot())
    }

    pub fn snapshot(&self) -> SenderSnapshot {
        let total_lines = self.plan.as_ref().map_or(0, |plan| plan.lines().len());
        let current = self
            .in_flight
            .front()
            .or(self.deferred_program_end.as_ref())
            .or(self.last_line.as_ref());
        let estimated_total_ms = self.plan.as_ref().map_or(0, DryRunPlan::estimated_total_ms);
        SenderSnapshot {
            state: self.state,
            mode: self.mode,
            source_name: self.plan.as_ref().map(|plan| plan.source_name().to_owned()),
            total_lines,
            dispatched_lines: self.dispatched_lines,
            acknowledged_lines: self.acknowledged_lines,
            in_flight_lines: self.in_flight.len(),
            rx_buffer_bytes: self.in_flight_bytes,
            rx_buffer_capacity: self.limits.rx_buffer_bytes,
            current_source_line: current.and_then(DryRunLine::source_line),
            current_command: current.map(|line| line.command().to_owned()),
            last_error: self.last_error.clone(),
            failure: self.failure.clone(),
            progress: if total_lines == 0 {
                0.0
            } else {
                self.acknowledged_lines as f64 / total_lines as f64
            },
            elapsed_seconds: self.elapsed_at(Instant::now()).as_secs_f64(),
            estimated_completed_seconds: self.estimated_completed_ms as f64 / 1_000.0,
            estimated_remaining_seconds: estimated_total_ms
                .saturating_sub(self.estimated_completed_ms)
                as f64
                / 1_000.0,
            estimated_total_seconds: estimated_total_ms as f64 / 1_000.0,
            time_estimate_complete: self
                .plan
                .as_ref()
                .is_some_and(DryRunPlan::time_estimate_complete),
        }
    }

    fn pause_clock(&mut self) {
        if self.started_at.is_some() && self.paused_at.is_none() && self.finished_elapsed.is_none()
        {
            self.paused_at = Some(Instant::now());
        }
    }

    fn resume_clock(&mut self) {
        if let Some(paused_at) = self.paused_at.take() {
            self.paused_duration = self
                .paused_duration
                .saturating_add(Instant::now().saturating_duration_since(paused_at));
        }
    }

    fn freeze_clock(&mut self) {
        if self.finished_elapsed.is_none() {
            self.finished_elapsed = Some(self.elapsed_at(Instant::now()));
            self.paused_at = None;
        }
    }

    fn elapsed_at(&self, now: Instant) -> Duration {
        if let Some(elapsed) = self.finished_elapsed {
            return elapsed;
        }
        let Some(started_at) = self.started_at else {
            return Duration::ZERO;
        };
        self.paused_at
            .unwrap_or(now)
            .saturating_duration_since(started_at)
            .saturating_sub(self.paused_duration)
    }

    fn validate_plan(&self, plan: &DryRunPlan) -> Result<(), SenderError> {
        if plan.lines().len() > self.limits.max_lines {
            return Err(SenderError::TooManyLines {
                actual: plan.lines().len(),
                limit: self.limits.max_lines,
            });
        }
        let mut total_bytes = 0usize;
        for line in plan.lines() {
            let bytes = line.command().len();
            if bytes > self.limits.max_command_bytes {
                return Err(SenderError::CommandTooLong {
                    actual: bytes,
                    limit: self.limits.max_command_bytes,
                });
            }
            let rx_bytes = bytes.saturating_add(1);
            if rx_bytes > self.limits.rx_buffer_bytes {
                return Err(SenderError::CommandExceedsRxBuffer {
                    actual: rx_bytes,
                    limit: self.limits.rx_buffer_bytes,
                });
            }
            total_bytes = total_bytes.saturating_add(bytes);
        }
        if total_bytes > self.limits.max_bytes {
            return Err(SenderError::PlanTooLarge {
                actual: total_bytes,
                limit: self.limits.max_bytes,
            });
        }
        Ok(())
    }

    fn finished_state(&self) -> SenderState {
        if self.requires_motion_drain() {
            SenderState::Draining
        } else {
            SenderState::Completed
        }
    }

    fn requires_motion_drain(&self) -> bool {
        matches!(self.mode, Some(SenderMode::AirRun | SenderMode::CutRun))
    }
}

fn command_rx_bytes(line: &DryRunLine) -> usize {
    line.command().len().saturating_add(1)
}

#[cfg(test)]
mod tests {
    use millo_dry_run::{ProgramRunPolicy, build_dry_run_plan, build_program_run_plan};
    use millo_gcode::{ProgramParseRequest, parse_program};

    use super::*;

    fn plan(source: &str) -> DryRunPlan {
        let program = parse_program(ProgramParseRequest {
            source_name: "sender.nc".to_owned(),
            source: source.to_owned(),
        })
        .unwrap();
        build_dry_run_plan(&program).unwrap()
    }

    fn cutting_plan(source: &str) -> DryRunPlan {
        let program = parse_program(ProgramParseRequest {
            source_name: "sender.nc".to_owned(),
            source: source.to_owned(),
        })
        .unwrap();
        build_program_run_plan(&program, ProgramRunPolicy::Cutting).unwrap()
    }

    #[test]
    fn fills_the_grbl_rx_buffer_without_exceeding_it() {
        let mut sender = Sender::default();
        let loaded = sender
            .load(plan(
                "G21\nG0 X1\nG0 X2\nG0 X3\nG0 X4\nG0 X5\nG0 X6\nG0 X7\nG0 X8",
            ))
            .unwrap();
        assert_eq!(loaded.mode, Some(SenderMode::MockDryRun));
        sender.start().unwrap();

        let mut dispatched = Vec::new();
        while let Some(line) = sender.next_line() {
            dispatched.push(line.command().to_owned());
        }

        let snapshot = sender.snapshot();
        assert!(dispatched.len() > 1);
        assert_eq!(snapshot.in_flight_lines, dispatched.len());
        assert!(snapshot.rx_buffer_bytes <= snapshot.rx_buffer_capacity);
        assert_eq!(snapshot.acknowledged_lines, 0);
    }

    #[test]
    fn derives_and_applies_a_bounded_window_from_reported_grbl_capacity() {
        assert_eq!(usable_rx_buffer_capacity(Some(128)), 127);
        assert_eq!(usable_rx_buffer_capacity(Some(256)), 255);
        assert_eq!(
            usable_rx_buffer_capacity(Some(1)),
            DEFAULT_GRBL_RX_BUFFER_BYTES
        );
        assert_eq!(
            usable_rx_buffer_capacity(Some(u16::MAX)),
            MAX_GRBL_RX_BUFFER_BYTES
        );

        let mut sender = Sender::default();
        let configured = sender.configure_rx_buffer_capacity(255).unwrap();
        assert_eq!(configured.rx_buffer_capacity, 255);
        sender.load(plan("G21")).unwrap();
        assert_eq!(
            sender.configure_rx_buffer_capacity(127),
            Err(SenderError::Busy(SenderState::Ready))
        );
    }

    #[test]
    fn correlates_error_with_the_in_flight_source_line_and_stops() {
        let mut sender = Sender::default();
        sender.load(plan("G21\nG0 X1")).unwrap();
        sender.start().unwrap();
        sender.next_line();
        sender.acknowledge_ok().unwrap();
        sender.next_line();
        sender.acknowledge_ok().unwrap();
        let line = sender.next_line().unwrap();
        assert_eq!(line.source_line(), Some(1));

        let snapshot = sender
            .acknowledge_failure(
                SenderFailure::new(SenderFailureKind::GrblError, "GRBL error 20")
                    .with_grbl_code(Some(20)),
            )
            .unwrap();

        assert_eq!(snapshot.state, SenderState::Failed);
        assert_eq!(snapshot.current_source_line, Some(1));
        assert_eq!(snapshot.acknowledged_lines, 2);
        let failure = snapshot.failure.unwrap();
        assert_eq!(failure.kind, SenderFailureKind::GrblError);
        assert_eq!(failure.grbl_code, Some(20));
        assert_eq!(failure.source_line, Some(1));
        assert_eq!(failure.command.as_deref(), Some("G21"));
        assert!(sender.next_line().is_none());
    }

    #[test]
    fn pause_resume_and_cancel_are_explicit_transitions() {
        let mut sender = Sender::default();
        sender.load(plan("G0 X1")).unwrap();
        sender.start().unwrap();
        assert_eq!(sender.pause().unwrap().state, SenderState::Paused);
        assert!(sender.next_line().is_none());
        assert_eq!(sender.resume().unwrap().state, SenderState::Running);
        assert_eq!(sender.cancel().unwrap().state, SenderState::Cancelled);
        assert!(sender.resume().is_err());
    }

    #[test]
    fn program_pause_is_an_acknowledged_barrier() {
        let mut sender = Sender::default();
        sender
            .load_cut_run(cutting_plan("G21\nM0\nG1 X1 F10"))
            .unwrap();
        sender.start().unwrap();

        loop {
            let line = sender.next_line().unwrap();
            if line.command() == "M0" {
                break;
            }
            sender.acknowledge_ok().unwrap();
        }
        let paused = sender.acknowledge_ok().unwrap();
        assert_eq!(paused.state, SenderState::Paused);
        assert!(sender.next_line().is_none());

        sender.resume().unwrap();
        assert_eq!(sender.next_line().unwrap().command(), "G1 X1 F10");
    }

    #[test]
    fn snapshot_tracks_plan_timing_and_excludes_paused_wall_time() {
        let mut sender = Sender::default();
        let loaded = sender
            .load(plan("G21 G90 G94\nG1 X60 F60\nG4 P0.250\nG1 X90 F30\nM30"))
            .unwrap();
        assert_eq!(loaded.estimated_total_seconds, 120.25);
        assert_eq!(loaded.estimated_remaining_seconds, 120.25);
        assert!(loaded.time_estimate_complete);

        sender.start().unwrap();
        std::thread::sleep(Duration::from_millis(8));
        let paused = sender.pause().unwrap();
        std::thread::sleep(Duration::from_millis(12));
        let still_paused = sender.snapshot();
        assert!((still_paused.elapsed_seconds - paused.elapsed_seconds).abs() < 0.003);

        sender.resume().unwrap();
        loop {
            let line = sender.next_line().unwrap();
            let source_line = line.source_line();
            let snapshot = sender.acknowledge_ok().unwrap();
            if source_line == Some(2) {
                assert_eq!(snapshot.estimated_completed_seconds, 60.0);
                assert_eq!(snapshot.estimated_remaining_seconds, 60.25);
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(8));
        let cancelled = sender.cancel().unwrap();
        std::thread::sleep(Duration::from_millis(8));
        assert_eq!(sender.snapshot().elapsed_seconds, cancelled.elapsed_seconds);
        assert!(cancelled.elapsed_seconds >= paused.elapsed_seconds);
    }

    #[test]
    fn program_end_enters_physical_draining() {
        let mut sender = Sender::default();
        sender
            .load_cut_run(cutting_plan("G21\nG1 X1 F10\nM30\nG1 X99"))
            .unwrap();
        sender.start().unwrap();
        while sender.snapshot().state == SenderState::Running {
            if sender.next_line().is_some() {
                sender.acknowledge_ok().unwrap();
            }
        }

        let snapshot = sender.snapshot();
        assert_eq!(snapshot.state, SenderState::Draining);
        assert_eq!(snapshot.current_command.as_deref(), Some("M30"));
        assert_eq!(snapshot.acknowledged_lines + 1, snapshot.total_lines);
    }

    #[test]
    fn check_run_acknowledges_program_end_without_motion_draining() {
        let mut sender = Sender::default();
        sender
            .load_check_run(plan("G21 G90 G94\nM0\nG1 X1 F10\nM1\nM30"))
            .unwrap();
        sender.start().unwrap();

        let mut checked_pauses = 0;
        while sender.snapshot().state == SenderState::Running {
            let line = sender.next_line().unwrap();
            assert!(sender.next_line().is_none());
            sender.acknowledge_ok().unwrap();
            if line.kind() == DryRunLineKind::ProgramPause {
                checked_pauses += 1;
                assert_eq!(sender.snapshot().state, SenderState::Running);
            }
            if line.kind() == DryRunLineKind::ProgramEnd {
                break;
            }
        }

        let completed = sender.snapshot();
        assert_eq!(completed.mode, Some(SenderMode::CheckRun));
        assert_eq!(completed.state, SenderState::Completed);
        assert_eq!(completed.acknowledged_lines, completed.total_lines);
        assert_eq!(checked_pauses, 2);
        assert!(sender.deferred_program_end().is_none());
    }

    #[test]
    fn physical_program_end_can_wait_for_the_motion_planner_to_drain() {
        let mut sender = Sender::default();
        sender
            .load_cut_run(cutting_plan("G21\nG1 X1 F10\nM30"))
            .unwrap();
        sender.start().unwrap();

        while sender.snapshot().state == SenderState::Running {
            if sender.next_line().is_some() {
                sender.acknowledge_ok().unwrap();
            }
        }

        let draining = sender.snapshot();
        assert_eq!(draining.state, SenderState::Draining);
        assert_eq!(draining.current_command.as_deref(), Some("M30"));
        assert_eq!(draining.acknowledged_lines + 1, draining.total_lines);
        assert_eq!(
            sender.complete_draining(),
            Err(SenderError::CommandInFlight)
        );

        assert_eq!(
            sender.dispatch_deferred_program_end().unwrap().command(),
            "M30"
        );
        sender.acknowledge_ok().unwrap();
        assert!(sender.deferred_program_end().is_none());
        assert_eq!(
            sender.complete_draining().unwrap().state,
            SenderState::Completed
        );
    }

    #[test]
    fn rejects_a_plan_outside_configured_bounds() {
        let mut sender = Sender::with_limits(SenderLimits {
            max_lines: 2,
            max_bytes: 1024,
            max_command_bytes: 255,
            rx_buffer_bytes: DEFAULT_GRBL_RX_BUFFER_BYTES,
        });

        assert_eq!(
            sender.load(plan("G0 X1")).unwrap_err(),
            SenderError::TooManyLines {
                actual: 3,
                limit: 2
            }
        );
    }

    #[test]
    fn completes_only_after_every_line_has_ok() {
        let mut sender = Sender::default();
        sender.load(plan("G21\nG0 X1")).unwrap();
        sender.start().unwrap();
        while sender.snapshot().state == SenderState::Running {
            sender.next_line().unwrap();
            sender.acknowledge_ok().unwrap();
        }

        let snapshot = sender.snapshot();
        assert_eq!(snapshot.state, SenderState::Completed);
        assert_eq!(snapshot.acknowledged_lines, snapshot.total_lines);
        assert_eq!(snapshot.progress, 1.0);
    }

    #[test]
    fn program_run_waits_for_a_fresh_idle_after_every_ack() {
        let mut sender = Sender::default();
        sender
            .load_air_run(plan("G21 G90 G94\nG1 X20 F60"))
            .unwrap();
        sender.start().unwrap();
        while sender.snapshot().state == SenderState::Running {
            sender.next_line().unwrap();
            sender.acknowledge_ok().unwrap();
        }

        assert_eq!(sender.snapshot().state, SenderState::Draining);
        assert_eq!(
            sender.complete_draining().unwrap().state,
            SenderState::Completed
        );
    }

    #[test]
    fn hold_and_resume_preserve_the_dispatch_or_drain_phase() {
        let mut dispatching = Sender::default();
        dispatching.load_cut_run(plan("G0 X1")).unwrap();
        dispatching.start().unwrap();
        dispatching.pause().unwrap();
        assert_eq!(dispatching.resume().unwrap().state, SenderState::Running);

        while dispatching.snapshot().state == SenderState::Running {
            dispatching.next_line().unwrap();
            dispatching.acknowledge_ok().unwrap();
        }
        dispatching.pause().unwrap();
        assert_eq!(dispatching.resume().unwrap().state, SenderState::Draining);
    }

    #[test]
    fn acknowledgements_release_fifo_bytes_and_errors_keep_the_exact_oldest_line() {
        let mut sender = Sender::with_limits(SenderLimits {
            rx_buffer_bytes: 20,
            ..SenderLimits::default()
        });
        sender.load(plan("G21\nG0 X1\nG0 X2\nG0 X3")).unwrap();
        sender.start().unwrap();

        while sender.next_line().is_some() {}
        let filled = sender.snapshot();
        assert!(filled.in_flight_lines > 1);
        let oldest = sender.oldest_in_flight().unwrap();
        let before_bytes = filled.rx_buffer_bytes;

        sender.acknowledge_ok().unwrap();
        assert!(sender.snapshot().rx_buffer_bytes < before_bytes);
        assert_ne!(sender.oldest_in_flight(), Some(oldest));
        let failed_line = sender.oldest_in_flight().unwrap();
        let failed = sender.acknowledge_error("error:20").unwrap();
        assert_eq!(failed.state, SenderState::Failed);
        assert_eq!(failed.current_source_line, failed_line.source_line());
        assert_eq!(failed.in_flight_lines, 0);
        assert_eq!(failed.rx_buffer_bytes, 0);
    }

    #[test]
    fn write_failure_keeps_the_exact_line_that_could_not_be_dispatched() {
        let mut sender = Sender::default();
        sender.load(plan("G21\nG0 X1\nG0 X2\nG0 X3")).unwrap();
        sender.start().unwrap();
        let first = sender.next_line().unwrap();
        let second = sender.next_line().unwrap();
        assert_ne!(first.command(), second.command());

        let failed = sender.fail_dispatched_line(second, "write failed");

        assert_eq!(failed.state, SenderState::Failed);
        assert_eq!(failed.current_command.as_deref(), Some("M9"));
        assert_eq!(failed.in_flight_lines, 0);
        assert_eq!(failed.rx_buffer_bytes, 0);
    }

    #[test]
    fn pause_barrier_is_the_last_buffered_line() {
        let mut sender = Sender::default();
        sender
            .load_cut_run(cutting_plan("G21\nG1 X1 F10\nM0\nG1 X2"))
            .unwrap();
        sender.start().unwrap();

        let mut commands = Vec::new();
        while let Some(line) = sender.next_line() {
            commands.push(line.command().to_owned());
        }

        assert_eq!(commands.last().map(String::as_str), Some("M0"));
        assert!(!commands.iter().any(|command| command.contains("X2")));
    }

    #[test]
    fn rejects_a_single_line_larger_than_the_rx_buffer() {
        let mut sender = Sender::with_limits(SenderLimits {
            rx_buffer_bytes: 8,
            ..SenderLimits::default()
        });

        assert_eq!(
            sender.load(plan("G0 X1234")).unwrap_err(),
            SenderError::CommandExceedsRxBuffer {
                actual: 9,
                limit: 8,
            }
        );
    }
}
