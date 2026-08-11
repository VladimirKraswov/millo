use millo_dry_run::{DryRunLine, DryRunPlan, MAX_DRY_RUN_COMMAND_BYTES};
use serde::Serialize;
use thiserror::Error;

pub const MAX_SENDER_LINES: usize = 200_002;
pub const MAX_SENDER_BYTES: usize = 2 * 1024 * 1024;

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
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SenderSnapshot {
    pub state: SenderState,
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
}

pub struct Sender {
    limits: SenderLimits,
    plan: Option<DryRunPlan>,
    state: SenderState,
    acknowledged_lines: usize,
    in_flight: Option<DryRunLine>,
    last_line: Option<DryRunLine>,
    last_error: Option<String>,
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
            acknowledged_lines: 0,
            in_flight: None,
            last_line: None,
            last_error: None,
        }
    }

    pub fn load(&mut self, plan: DryRunPlan) -> Result<SenderSnapshot, SenderError> {
        if matches!(self.state, SenderState::Running | SenderState::Paused) {
            return Err(SenderError::Busy(self.state));
        }
        self.validate_plan(&plan)?;
        self.plan = Some(plan);
        self.state = SenderState::Ready;
        self.acknowledged_lines = 0;
        self.in_flight = None;
        self.last_line = None;
        self.last_error = None;
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
        if self.state != SenderState::Running {
            return Err(SenderError::InvalidTransition {
                action: "pause",
                state: self.state,
            });
        }
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
        self.state = SenderState::Running;
        Ok(self.snapshot())
    }

    pub fn cancel(&mut self) -> Result<SenderSnapshot, SenderError> {
        if !matches!(
            self.state,
            SenderState::Ready | SenderState::Running | SenderState::Paused
        ) {
            return Err(SenderError::InvalidTransition {
                action: "cancel",
                state: self.state,
            });
        }
        self.state = SenderState::Cancelled;
        self.in_flight = None;
        Ok(self.snapshot())
    }

    pub fn fail(&mut self, error: impl Into<String>) -> SenderSnapshot {
        self.last_line = self.in_flight.take().or_else(|| self.last_line.take());
        self.last_error = Some(error.into());
        self.state = SenderState::Failed;
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
            self.state = SenderState::Completed;
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
        self.last_line = Some(line);
        self.acknowledged_lines = self.acknowledged_lines.saturating_add(1);
        if self
            .plan
            .as_ref()
            .is_some_and(|plan| self.acknowledged_lines == plan.lines().len())
        {
            self.state = SenderState::Completed;
        }
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
}

#[cfg(test)]
mod tests {
    use millo_dry_run::build_dry_run_plan;
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

    #[test]
    fn never_dispatches_a_second_line_before_acknowledgement() {
        let mut sender = Sender::default();
        sender.load(plan("G21\nG0 X1")).unwrap();
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
}
