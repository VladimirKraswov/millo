use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use millo_dry_run::{DryRunLine, DryRunLineKind, DryRunPlan, MAX_DRY_RUN_COMMAND_BYTES};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_SENDER_LINES: usize = 2_000_004;
pub const MAX_SENDER_BYTES: usize = 128 * 1024 * 1024;
pub const DEFAULT_GRBL_RX_BUFFER_BYTES: usize = 127;
pub const MAX_GRBL_RX_BUFFER_BYTES: usize = 4095;

pub fn usable_rx_buffer_capacity(reported_bytes: Option<u16>) -> usize {
    reported_bytes
        .map(|bytes| usize::from(bytes.saturating_sub(1)))
        .filter(|bytes| *bytes > 0)
        .map(|bytes| bytes.min(MAX_GRBL_RX_BUFFER_BYTES))
        .unwrap_or(DEFAULT_GRBL_RX_BUFFER_BYTES)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SenderState {
    #[default]
    Idle,
    Ready,
    Running,
    Paused,
    #[serde(alias = "toolchange")]
    ToolChange,
    Draining,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    pub run_sequence: u64,
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
    pub requested_tool: Option<u8>,
    pub progress_sequence: u64,
    pub last_acknowledged_source_line: Option<usize>,
    pub last_acknowledged_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executing_source_line: Option<usize>,
    pub seconds_since_acknowledgement: f64,
    pub shutdown_commands_acknowledged: bool,
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
            run_sequence: 0,
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
            requested_tool: None,
            progress_sequence: 0,
            last_acknowledged_source_line: None,
            last_acknowledged_command: None,
            executing_source_line: None,
            seconds_since_acknowledgement: 0.0,
            shutdown_commands_acknowledged: false,
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
    run_sequence: u64,
    plan: Option<DryRunPlan>,
    state: SenderState,
    mode: Option<SenderMode>,
    dispatched_lines: usize,
    acknowledged_lines: usize,
    in_flight: VecDeque<DryRunLine>,
    in_flight_bytes: usize,
    deferred_program_end: Option<DryRunLine>,
    deferred_tool_change: Option<DryRunLine>,
    last_line: Option<DryRunLine>,
    last_acknowledged_line: Option<DryRunLine>,
    last_acknowledged_at: Option<Instant>,
    finished_acknowledgement_age: Option<Duration>,
    shutdown_acknowledged: usize,
    shutdown_total: usize,
    last_error: Option<String>,
    failure: Option<SenderFailure>,
    paused_from: Option<SenderState>,
    estimated_completed_ms: u64,
    started_at: Option<Instant>,
    paused_at: Option<Instant>,
    paused_duration: Duration,
    finished_elapsed: Option<Duration>,
    executing_source_line: Option<usize>,
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
            run_sequence: 0,
            plan: None,
            state: SenderState::Idle,
            mode: None,
            dispatched_lines: 0,
            acknowledged_lines: 0,
            in_flight: VecDeque::new(),
            in_flight_bytes: 0,
            deferred_program_end: None,
            deferred_tool_change: None,
            last_line: None,
            last_acknowledged_line: None,
            last_acknowledged_at: None,
            finished_acknowledgement_age: None,
            shutdown_acknowledged: 0,
            shutdown_total: 0,
            last_error: None,
            failure: None,
            paused_from: None,
            estimated_completed_ms: 0,
            started_at: None,
            paused_at: None,
            paused_duration: Duration::ZERO,
            finished_elapsed: None,
            executing_source_line: None,
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
            SenderState::Ready
                | SenderState::Running
                | SenderState::Paused
                | SenderState::Draining
                | SenderState::ToolChange
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
            SenderState::Running
                | SenderState::Paused
                | SenderState::Draining
                | SenderState::ToolChange
        ) {
            return Err(SenderError::Busy(self.state));
        }
        self.validate_plan(&plan)?;
        self.run_sequence = self.run_sequence.saturating_add(1);
        self.shutdown_total = plan
            .lines()
            .iter()
            .filter(|line| line.kind() == DryRunLineKind::SafetyEpilogue)
            .count();
        self.plan = Some(plan);
        self.state = SenderState::Ready;
        self.mode = Some(mode);
        self.dispatched_lines = 0;
        self.acknowledged_lines = 0;
        self.in_flight.clear();
        self.in_flight_bytes = 0;
        self.deferred_program_end = None;
        self.deferred_tool_change = None;
        self.last_line = None;
        self.last_acknowledged_line = None;
        self.last_acknowledged_at = None;
        self.finished_acknowledgement_age = None;
        self.shutdown_acknowledged = 0;
        self.last_error = None;
        self.failure = None;
        self.paused_from = None;
        self.estimated_completed_ms = 0;
        self.started_at = None;
        self.paused_at = None;
        self.paused_duration = Duration::ZERO;
        self.finished_elapsed = None;
        self.executing_source_line = None;
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
        let now = Instant::now();
        self.started_at = Some(now);
        self.last_acknowledged_at = Some(now);
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
            SenderState::Ready
                | SenderState::Running
                | SenderState::Paused
                | SenderState::ToolChange
                | SenderState::Draining
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
        self.deferred_tool_change = None;
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
            .or_else(|| self.deferred_tool_change.take())
            .or_else(|| self.last_line.take());
        self.in_flight.clear();
        self.in_flight_bytes = 0;
        self.deferred_program_end = None;
        self.deferred_tool_change = None;
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
        self.deferred_tool_change = None;
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

    pub fn complete_tool_change(&mut self) -> Result<SenderSnapshot, SenderError> {
        if self.state != SenderState::ToolChange {
            return Err(SenderError::InvalidTransition {
                action: "complete tool change",
                state: self.state,
            });
        }
        self.acknowledged_lines = self.acknowledged_lines.saturating_add(1);
        if let Some(line) = self.last_line.clone() {
            self.record_acknowledgement(&line);
        }
        self.state = SenderState::Running;
        self.resume_clock();
        Ok(self.snapshot())
    }

    pub fn has_in_flight(&self) -> bool {
        !self.in_flight.is_empty()
    }

    /// Records GRBL's `Ln` status, which identifies the block currently being
    /// executed rather than merely accepted into the RX/planner buffers.
    pub fn observe_executing_line_number(&mut self, line_number: Option<u64>) {
        let Some(line_number) = line_number.and_then(|value| usize::try_from(value).ok()) else {
            return;
        };
        let source_line_count = self.plan.as_ref().map_or(0, DryRunPlan::source_line_count);
        if (1..=source_line_count).contains(&line_number) {
            self.executing_source_line = Some(line_number);
        }
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
        loop {
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
                    DryRunLineKind::ProgramPause
                        | DryRunLineKind::OptionalPause
                        | DryRunLineKind::ProgramEnd
                )
            }) {
                return None;
            }
            let line = plan.lines()[self.dispatched_lines].clone();
            if line.kind() == DryRunLineKind::ToolChange
                || (self.mode == Some(SenderMode::CheckRun)
                    && line.kind() == DryRunLineKind::ProgramEnd)
            {
                if !self.in_flight.is_empty() {
                    return None;
                }
                self.dispatched_lines = self.dispatched_lines.saturating_add(1);
                self.last_line = Some(line);
                if self.mode == Some(SenderMode::CheckRun) {
                    self.acknowledged_lines = self.acknowledged_lines.saturating_add(1);
                    if let Some(line) = self.last_line.clone() {
                        self.record_acknowledgement(&line);
                    }
                    continue;
                }
                if self.requires_motion_drain() {
                    self.deferred_tool_change = self.last_line.take();
                    self.state = SenderState::Draining;
                } else {
                    self.state = SenderState::ToolChange;
                    self.pause_clock();
                }
                return None;
            }
            break;
        }
        let plan = self.plan.as_ref()?;
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
        self.record_acknowledgement(&line);
        self.last_line = Some(line);
        self.acknowledged_lines = self.acknowledged_lines.saturating_add(1);
        if matches!(
            line_kind,
            DryRunLineKind::ProgramPause | DryRunLineKind::OptionalPause
        ) && self.mode != Some(SenderMode::CheckRun)
        {
            self.paused_from = Some(SenderState::Running);
            self.state = SenderState::Paused;
            self.pause_clock();
        } else if line_kind == DryRunLineKind::ProgramEnd
            || self.plan.as_ref().is_some_and(|plan| {
                self.acknowledged_lines == plan.lines().len() && self.in_flight.is_empty()
            })
        {
            let finished = self.finished_state();
            if self.state == SenderState::Paused && self.requires_motion_drain() {
                self.paused_from = Some(finished);
            } else {
                self.state = finished;
            }
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
        if let Some(line) = self.deferred_tool_change.take() {
            self.last_line = Some(line);
            self.state = SenderState::ToolChange;
            self.pause_clock();
        } else {
            self.freeze_clock();
            self.state = SenderState::Completed;
        }
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
        self.deferred_tool_change = None;
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
            .or(self.deferred_tool_change.as_ref())
            .or(self.last_line.as_ref());
        let estimated_total_ms = self.plan.as_ref().map_or(0, DryRunPlan::estimated_total_ms);
        SenderSnapshot {
            run_sequence: self.run_sequence,
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
            requested_tool: current.and_then(DryRunLine::tool_number),
            progress_sequence: self.acknowledged_lines as u64,
            last_acknowledged_source_line: self
                .last_acknowledged_line
                .as_ref()
                .and_then(DryRunLine::source_line),
            last_acknowledged_command: self
                .last_acknowledged_line
                .as_ref()
                .map(|line| line.command().to_owned()),
            executing_source_line: self.executing_source_line,
            seconds_since_acknowledgement: self
                .acknowledgement_age_at(Instant::now())
                .as_secs_f64(),
            shutdown_commands_acknowledged: self.shutdown_total > 0
                && self.shutdown_acknowledged == self.shutdown_total,
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
            let now = Instant::now();
            self.finished_elapsed = Some(self.elapsed_at(now));
            self.finished_acknowledgement_age = Some(self.acknowledgement_age_at(now));
            self.paused_at = None;
        }
    }

    fn record_acknowledgement(&mut self, line: &DryRunLine) {
        self.last_acknowledged_line = Some(line.clone());
        self.last_acknowledged_at = Some(Instant::now());
        if line.kind() == DryRunLineKind::SafetyEpilogue {
            self.shutdown_acknowledged = self.shutdown_acknowledged.saturating_add(1);
        }
    }

    fn acknowledgement_age_at(&self, now: Instant) -> Duration {
        if let Some(age) = self.finished_acknowledgement_age {
            return age;
        }
        self.last_acknowledged_at
            .map(|at| now.saturating_duration_since(at))
            .unwrap_or(Duration::ZERO)
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
            let bytes = line.wire_command_len();
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
    line.wire_command_len().saturating_add(1)
}

#[cfg(test)]
mod tests {
    use millo_dry_run::{
        ProgramExecutionOptions, ProgramRunPolicy, build_dry_run_plan, build_program_run_plan,
        build_program_run_plan_with_options,
    };
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

    fn cutting_plan_with_options(
        source: &str,
        execution_options: ProgramExecutionOptions,
    ) -> DryRunPlan {
        let program = parse_program(ProgramParseRequest {
            source_name: "sender.nc".to_owned(),
            source: source.to_owned(),
        })
        .unwrap();
        build_program_run_plan_with_options(&program, ProgramRunPolicy::Cutting, execution_options)
            .unwrap()
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
    fn accepts_only_in_range_grbl_execution_line_numbers() {
        let mut sender = Sender::default();
        sender
            .load_cut_run(cutting_plan("G21\nG0 X1\nG1 X2 F10"))
            .unwrap();

        sender.observe_executing_line_number(Some(2));
        assert_eq!(sender.snapshot().executing_source_line, Some(2));
        sender.observe_executing_line_number(Some(99));
        sender.observe_executing_line_number(Some(0));
        sender.observe_executing_line_number(None);

        assert_eq!(sender.snapshot().executing_source_line, Some(2));
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
        assert_eq!(snapshot.progress_sequence, 2);
        assert_eq!(snapshot.last_acknowledged_command.as_deref(), Some("M9"));
        assert!(!snapshot.shutdown_commands_acknowledged);
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
    fn final_acknowledgement_preserves_hold_until_explicit_resume() {
        for program_end in [false, true] {
            let mut sender = Sender::default();
            sender
                .load_cut_run(cutting_plan(if program_end {
                    "G1 X1 F10\nM30"
                } else {
                    "G1 X1 F10"
                }))
                .unwrap();
            sender.start().unwrap();
            while sender.next_line().is_some() {
                if !program_end {
                    continue;
                }
                sender.acknowledge_ok().unwrap();
            }
            if program_end {
                sender.dispatch_deferred_program_end().unwrap();
            }
            sender.pause().unwrap();
            while sender.has_in_flight() {
                sender.acknowledge_ok().unwrap();
            }
            assert_eq!(sender.snapshot().state, SenderState::Paused);
            assert!(sender.complete_draining().is_err());
            assert_eq!(sender.resume().unwrap().state, SenderState::Draining);
            assert_eq!(
                sender.complete_draining().unwrap().state,
                SenderState::Completed
            );
        }
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
    fn enabled_optional_stop_pauses_while_the_default_plan_omits_m1() {
        let source = "G21\nM1\nG1 X1 F10";
        assert!(
            !cutting_plan(source)
                .lines()
                .iter()
                .any(|line| line.command() == "M1")
        );

        let mut sender = Sender::default();
        sender
            .load_cut_run(cutting_plan_with_options(
                source,
                ProgramExecutionOptions {
                    optional_stop: true,
                    block_delete: false,
                    ..ProgramExecutionOptions::default()
                },
            ))
            .unwrap();
        sender.start().unwrap();
        loop {
            let line = sender.next_line().unwrap();
            sender.acknowledge_ok().unwrap();
            if line.command() == "M1" {
                break;
            }
        }
        assert_eq!(sender.snapshot().state, SenderState::Paused);
    }

    #[test]
    fn tool_change_waits_for_an_empty_fifo_and_never_reaches_grbl() {
        let mut sender = Sender::default();
        sender
            .load_cut_run(cutting_plan("G21 G90 G94\nG1 X1 F10\nT2 M6\nG1 X2 F10"))
            .unwrap();
        sender.start().unwrap();

        let mut dispatched = Vec::new();
        while let Some(line) = sender.next_line() {
            dispatched.push(line.command().to_owned());
        }
        assert!(sender.has_in_flight());
        assert_eq!(sender.snapshot().state, SenderState::Running);
        assert!(!dispatched.iter().any(|command| command.contains("M6")));

        while sender.has_in_flight() {
            sender.acknowledge_ok().unwrap();
        }
        assert!(sender.next_line().is_none());
        assert_eq!(sender.snapshot().state, SenderState::Draining);
        sender.complete_draining().unwrap();
        let barrier = sender.snapshot();
        assert_eq!(barrier.state, SenderState::ToolChange);
        assert_eq!(barrier.current_source_line, Some(3));
        assert_eq!(barrier.current_command.as_deref(), Some("T2 M6"));
        assert_eq!(barrier.requested_tool, Some(2));
        assert_eq!(
            sender.resume(),
            Err(SenderError::InvalidTransition {
                action: "resume",
                state: SenderState::ToolChange,
            })
        );

        let acknowledged_before = barrier.acknowledged_lines;
        let resumed = sender.complete_tool_change().unwrap();
        assert_eq!(resumed.state, SenderState::Running);
        assert_eq!(resumed.acknowledged_lines, acknowledged_before + 1);
        assert_eq!(sender.next_line().unwrap().command(), "G1 X2 F10");
    }

    #[test]
    fn check_run_validates_tool_selection_but_skips_the_host_barrier() {
        let mut sender = Sender::default();
        sender
            .load_check_run(cutting_plan("G21 G90\nT4 M6\nG1 X1 F10\nM30"))
            .unwrap();
        sender.start().unwrap();

        let mut dispatched = Vec::new();
        while sender.snapshot().state == SenderState::Running {
            if let Some(line) = sender.next_line() {
                dispatched.push(line.command().to_owned());
                sender.acknowledge_ok().unwrap();
            }
        }

        assert_eq!(sender.snapshot().state, SenderState::Completed);
        assert!(dispatched.iter().any(|command| command == "T4"));
        assert!(!dispatched.iter().any(|command| command.contains("M6")));
        assert_eq!(
            sender.snapshot().acknowledged_lines,
            sender.snapshot().total_lines
        );
    }

    #[test]
    fn tool_change_time_is_excluded_from_sender_elapsed_time() {
        let mut sender = Sender::default();
        sender
            .load_cut_run(cutting_plan("G21\nG1 X1 F10\nT2 M6\nG1 X2 F10"))
            .unwrap();
        sender.start().unwrap();
        loop {
            if sender.next_line().is_some() {
                sender.acknowledge_ok().unwrap();
            } else if sender.snapshot().state == SenderState::Draining {
                sender.complete_draining().unwrap();
                break;
            }
        }

        let paused = sender.snapshot().elapsed_seconds;
        std::thread::sleep(Duration::from_millis(12));
        let still_paused = sender.snapshot().elapsed_seconds;
        assert!((still_paused - paused).abs() < 0.003);
        sender.complete_tool_change().unwrap();
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
        assert!(snapshot.shutdown_commands_acknowledged);
    }

    #[test]
    fn check_run_acknowledges_program_end_without_motion_draining() {
        let mut sender = Sender::default();
        sender
            .load_check_run(plan("G21 G90 G94\nM0\nG1 X1 F10\nM1\nM30"))
            .unwrap();
        sender.start().unwrap();

        let mut checked_pauses = 0;
        let mut sent_program_end = false;
        while sender.snapshot().state == SenderState::Running {
            let Some(line) = sender.next_line() else {
                break;
            };
            assert!(sender.next_line().is_none());
            sender.acknowledge_ok().unwrap();
            if line.kind() == DryRunLineKind::ProgramPause {
                checked_pauses += 1;
                assert_eq!(sender.snapshot().state, SenderState::Running);
            }
            if line.kind() == DryRunLineKind::ProgramEnd {
                sent_program_end = true;
            }
        }

        let completed = sender.snapshot();
        assert_eq!(completed.mode, Some(SenderMode::CheckRun));
        assert_eq!(completed.state, SenderState::Completed);
        assert_eq!(completed.acknowledged_lines, completed.total_lines);
        assert_eq!(checked_pauses, 1);
        assert!(!sent_program_end);
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
                actual: 5,
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
    fn acknowledgement_heartbeat_resets_on_ok_and_freezes_at_terminal_state() {
        let mut sender = Sender::default();
        sender.load(plan("G21\nG1 X1 F10")).unwrap();
        sender.start().unwrap();
        std::thread::sleep(Duration::from_millis(8));
        assert!(sender.snapshot().seconds_since_acknowledgement >= 0.005);

        let line = sender.next_line().unwrap();
        let acknowledged = sender.acknowledge_ok().unwrap();
        assert_eq!(acknowledged.progress_sequence, 1);
        assert_eq!(
            acknowledged.last_acknowledged_command.as_deref(),
            Some(line.command())
        );
        assert!(acknowledged.seconds_since_acknowledgement < 0.005);

        let cancelled = sender.cancel().unwrap();
        std::thread::sleep(Duration::from_millis(8));
        assert_eq!(
            sender.snapshot().seconds_since_acknowledgement,
            cancelled.seconds_since_acknowledgement
        );
    }

    #[test]
    fn streams_one_hundred_thousand_lines_with_constant_bounded_fifo_state() {
        let source = "G1X1F10\n".repeat(100_000);
        let plan = plan(&source);
        assert_eq!(plan.lines().len(), 100_004);
        let mut sender = Sender::default();
        sender.load(plan).unwrap();
        sender.start().unwrap();
        let mut peak_in_flight = 0;

        while sender.snapshot().state == SenderState::Running {
            while sender.next_line().is_some() {
                let snapshot = sender.snapshot();
                peak_in_flight = peak_in_flight.max(snapshot.in_flight_lines);
                assert!(snapshot.rx_buffer_bytes <= snapshot.rx_buffer_capacity);
            }
            if sender.has_in_flight() {
                sender.acknowledge_ok().unwrap();
            }
        }

        let completed = sender.snapshot();
        assert_eq!(completed.state, SenderState::Completed);
        assert_eq!(completed.acknowledged_lines, 100_004);
        assert!(completed.shutdown_commands_acknowledged);
        assert!(peak_in_flight < 32);
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
                actual: 12,
                limit: 8,
            }
        );
    }

    #[test]
    fn sender_state_json_matches_the_webview_contract_and_reads_legacy_toolchange() {
        assert_eq!(
            serde_json::to_string(&SenderState::ToolChange).unwrap(),
            "\"toolChange\""
        );
        assert_eq!(
            serde_json::from_str::<SenderState>("\"toolchange\"").unwrap(),
            SenderState::ToolChange
        );
    }
}
