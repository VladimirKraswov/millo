use millo_dry_run::{DryRunLine, DryRunLineKind, DryRunPlan, MAX_DRY_RUN_COMMAND_BYTES};
use serde::Serialize;
use thiserror::Error;

pub const MAX_SENDER_LINES: usize = 200_002;
pub const MAX_SENDER_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SenderMode {
    MockDryRun,
    AirRun,
    CutRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SenderLimits {
    pub max_lines: usize,
    pub max_bytes: usize,
    pub max_command_bytes: usize,
}

impl Default for SenderLimits {
    fn default() -> Self {
        Self {
            max_lines: MAX_SENDER_LINES,
            max_bytes: MAX_SENDER_BYTES,
            max_command_bytes: MAX_DRY_RUN_COMMAND_BYTES,
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SenderSnapshot {
    pub state: SenderState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<SenderMode>,
    pub source_name: Option<String>,
    pub total_lines: usize,
    pub acknowledged_lines: usize,
    pub current_source_line: Option<usize>,
    pub current_command: Option<String>,
    pub last_error: Option<String>,
    pub progress: f64,
}

impl Default for SenderSnapshot {
    fn default() -> Self {
        Self {
            state: SenderState::Idle,
            mode: None,
            source_name: None,
            total_lines: 0,
            acknowledged_lines: 0,
            current_source_line: None,
            current_command: None,
            last_error: None,
            progress: 0.0,
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
    acknowledged_lines: usize,
    in_flight: Option<DryRunLine>,
    last_line: Option<DryRunLine>,
    last_error: Option<String>,
    paused_from: Option<SenderState>,
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
            acknowledged_lines: 0,
            in_flight: None,
            last_line: None,
            last_error: None,
            paused_from: None,
        }
    }

    pub fn load(&mut self, plan: DryRunPlan) -> Result<SenderSnapshot, SenderError> {
        self.load_with_mode(plan, SenderMode::MockDryRun)
    }

    pub fn load_air_run(&mut self, plan: DryRunPlan) -> Result<SenderSnapshot, SenderError> {
        self.load_with_mode(plan, SenderMode::AirRun)
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
        self.acknowledged_lines = 0;
        self.in_flight = None;
        self.last_line = None;
        self.last_error = None;
        self.paused_from = None;
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
        self.state = SenderState::Cancelled;
        self.in_flight = None;
        self.paused_from = None;
        Ok(self.snapshot())
    }

    pub fn fail(&mut self, error: impl Into<String>) -> SenderSnapshot {
        self.last_line = self.in_flight.take().or_else(|| self.last_line.take());
        self.last_error = Some(error.into());
        self.state = SenderState::Failed;
        self.paused_from = None;
        self.snapshot()
    }

    pub fn is_dispatchable(&self) -> bool {
        self.state == SenderState::Running && self.in_flight.is_none()
    }

    pub fn next_line(&mut self) -> Option<DryRunLine> {
        if !self.is_dispatchable() {
            return None;
        }
        let plan = self.plan.as_ref()?;
        if self.acknowledged_lines >= plan.lines().len() {
            self.state = self.finished_state();
            return None;
        }
        let line = plan.lines()[self.acknowledged_lines].clone();
        self.in_flight = Some(line.clone());
        Some(line)
    }

    pub fn acknowledge_ok(&mut self) -> Result<SenderSnapshot, SenderError> {
        let line = self
            .in_flight
            .take()
            .ok_or(SenderError::NoCommandInFlight)?;
        let line_kind = line.kind();
        self.last_line = Some(line);
        self.acknowledged_lines = self.acknowledged_lines.saturating_add(1);
        if line_kind == DryRunLineKind::ProgramPause {
            self.paused_from = Some(SenderState::Running);
            self.state = SenderState::Paused;
        } else if line_kind == DryRunLineKind::ProgramEnd
            || self
                .plan
                .as_ref()
                .is_some_and(|plan| self.acknowledged_lines == plan.lines().len())
        {
            self.state = self.finished_state();
        }
        Ok(self.snapshot())
    }

    pub fn defer_program_end(&mut self) -> Result<SenderSnapshot, SenderError> {
        let Some(line) = self.in_flight.as_ref() else {
            return Err(SenderError::NoCommandInFlight);
        };
        if self.state != SenderState::Running || line.kind() != DryRunLineKind::ProgramEnd {
            return Err(SenderError::InvalidTransition {
                action: "defer program end",
                state: self.state,
            });
        }
        self.state = SenderState::Draining;
        Ok(self.snapshot())
    }

    pub fn deferred_program_end(&self) -> Option<DryRunLine> {
        self.in_flight
            .as_ref()
            .filter(|line| {
                self.state == SenderState::Draining && line.kind() == DryRunLineKind::ProgramEnd
            })
            .cloned()
    }

    pub fn complete_draining(&mut self) -> Result<SenderSnapshot, SenderError> {
        if self.state != SenderState::Draining {
            return Err(SenderError::InvalidTransition {
                action: "complete draining",
                state: self.state,
            });
        }
        if self.in_flight.is_some() {
            return Err(SenderError::CommandInFlight);
        }
        self.state = SenderState::Completed;
        Ok(self.snapshot())
    }

    pub fn acknowledge_error(
        &mut self,
        error: impl Into<String>,
    ) -> Result<SenderSnapshot, SenderError> {
        let line = self
            .in_flight
            .take()
            .ok_or(SenderError::NoCommandInFlight)?;
        self.last_line = Some(line);
        self.last_error = Some(error.into());
        self.state = SenderState::Failed;
        Ok(self.snapshot())
    }

    pub fn snapshot(&self) -> SenderSnapshot {
        let total_lines = self.plan.as_ref().map_or(0, |plan| plan.lines().len());
        let current = self.in_flight.as_ref().or(self.last_line.as_ref());
        SenderSnapshot {
            state: self.state,
            mode: self.mode,
            source_name: self.plan.as_ref().map(|plan| plan.source_name().to_owned()),
            total_lines,
            acknowledged_lines: self.acknowledged_lines,
            current_source_line: current.and_then(DryRunLine::source_line),
            current_command: current.map(|line| line.command().to_owned()),
            last_error: self.last_error.clone(),
            progress: if total_lines == 0 {
                0.0
            } else {
                self.acknowledged_lines as f64 / total_lines as f64
            },
        }
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
        if matches!(self.mode, Some(SenderMode::AirRun | SenderMode::CutRun)) {
            SenderState::Draining
        } else {
            SenderState::Completed
        }
    }
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
    fn never_dispatches_a_second_line_before_acknowledgement() {
        let mut sender = Sender::default();
        let loaded = sender.load(plan("G21\nG0 X1")).unwrap();
        assert_eq!(loaded.mode, Some(SenderMode::MockDryRun));
        sender.start().unwrap();

        assert_eq!(sender.next_line().unwrap().command(), "M5");
        assert!(sender.next_line().is_none());
        sender.acknowledge_ok().unwrap();
        assert_eq!(sender.next_line().unwrap().command(), "M9");
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

        let snapshot = sender.acknowledge_error("error:20").unwrap();

        assert_eq!(snapshot.state, SenderState::Failed);
        assert_eq!(snapshot.current_source_line, Some(1));
        assert_eq!(snapshot.acknowledged_lines, 2);
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
    fn program_end_enters_physical_draining() {
        let mut sender = Sender::default();
        sender
            .load_cut_run(cutting_plan("G21\nG1 X1 F10\nM30\nG1 X99"))
            .unwrap();
        sender.start().unwrap();
        while sender.snapshot().state == SenderState::Running {
            sender.next_line().unwrap();
            sender.acknowledge_ok().unwrap();
        }

        let snapshot = sender.snapshot();
        assert_eq!(snapshot.state, SenderState::Draining);
        assert_eq!(snapshot.current_command.as_deref(), Some("M30"));
        assert_eq!(snapshot.acknowledged_lines, snapshot.total_lines);
    }

    #[test]
    fn physical_program_end_can_wait_for_the_motion_planner_to_drain() {
        let mut sender = Sender::default();
        sender
            .load_cut_run(cutting_plan("G21\nG1 X1 F10\nM30"))
            .unwrap();
        sender.start().unwrap();

        loop {
            let line = sender.next_line().unwrap();
            if line.kind() == DryRunLineKind::ProgramEnd {
                break;
            }
            sender.acknowledge_ok().unwrap();
        }

        let draining = sender.defer_program_end().unwrap();
        assert_eq!(draining.state, SenderState::Draining);
        assert_eq!(draining.current_command.as_deref(), Some("M30"));
        assert_eq!(draining.acknowledged_lines + 1, draining.total_lines);
        assert_eq!(
            sender.complete_draining(),
            Err(SenderError::CommandInFlight)
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
}
