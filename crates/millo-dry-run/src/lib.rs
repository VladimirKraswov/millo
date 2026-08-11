use std::collections::{BTreeMap, BTreeSet};

use millo_gcode::{GcodeProgram, ProgramWarningCode, ProgramWarningSeverity};
use serde::Serialize;
use thiserror::Error;

pub const MAX_DRY_RUN_COMMAND_BYTES: usize = 255;
const SAFETY_PREAMBLE: [&str; 2] = ["M5", "M9"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProgramRunPolicy {
    #[default]
    AirRun,
    Cutting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DryRunBlockerKind {
    SpindleActivation,
    SpindleSpeed,
    CoolantActivation,
    ProbeCycle,
    ToolChange,
    MachineCoordinateMotion,
    CoordinateMutation,
    UnsupportedProgram,
    IncompletePreview,
    CommandTooLong,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DryRunBlocker {
    pub source_line: Option<usize>,
    pub kind: DryRunBlockerKind,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DryRunLineKind {
    SafetyPreamble,
    Program,
    ProgramPause,
    ToolChange,
    ProgramEnd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DryRunLine {
    source_line: Option<usize>,
    command: String,
    kind: DryRunLineKind,
    tool_number: Option<u8>,
    estimated_duration_ms: Option<u64>,
}

impl DryRunLine {
    pub fn source_line(&self) -> Option<usize> {
        self.source_line
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn kind(&self) -> DryRunLineKind {
        self.kind
    }

    pub fn tool_number(&self) -> Option<u8> {
        self.tool_number
    }

    pub fn estimated_duration_ms(&self) -> Option<u64> {
        self.estimated_duration_ms
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DryRunPlan {
    source_name: String,
    source_line_count: usize,
    lines: Vec<DryRunLine>,
    estimated_total_ms: u64,
    time_estimate_complete: bool,
}

impl DryRunPlan {
    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn source_line_count(&self) -> usize {
        self.source_line_count
    }

    pub fn lines(&self) -> &[DryRunLine] {
        &self.lines
    }

    pub fn estimated_total_ms(&self) -> u64 {
        self.estimated_total_ms
    }

    pub fn time_estimate_complete(&self) -> bool {
        self.time_estimate_complete
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DryRunPolicyError {
    #[error("dry run policy rejected the program with {0} blocker(s)")]
    Rejected(usize, Vec<DryRunBlocker>),
    #[error("dry run requires at least one executable program line")]
    EmptyProgram,
}

impl DryRunPolicyError {
    pub fn blockers(&self) -> &[DryRunBlocker] {
        match self {
            Self::Rejected(_, blockers) => blockers,
            Self::EmptyProgram => &[],
        }
    }
}

pub fn build_dry_run_plan(program: &GcodeProgram) -> Result<DryRunPlan, DryRunPolicyError> {
    build_program_run_plan(program, ProgramRunPolicy::AirRun)
}

pub fn build_program_run_plan(
    program: &GcodeProgram,
    policy: ProgramRunPolicy,
) -> Result<DryRunPlan, DryRunPolicyError> {
    let mut blockers = Vec::new();
    let mut seen = BTreeSet::new();

    if !program.summary.preview_complete {
        add_blocker(
            &mut blockers,
            &mut seen,
            None,
            DryRunBlockerKind::IncompletePreview,
            "preview is incomplete; execution is fail-closed",
        );
    }

    for line in program.lines.iter().filter(|line| line.executable) {
        inspect_normalized_line(
            line.source_line,
            &line.normalized,
            policy,
            &mut blockers,
            &mut seen,
        );
        if line.normalized.len() > MAX_DRY_RUN_COMMAND_BYTES {
            add_blocker(
                &mut blockers,
                &mut seen,
                Some(line.source_line),
                DryRunBlockerKind::CommandTooLong,
                format!(
                    "normalized command exceeds the {MAX_DRY_RUN_COMMAND_BYTES} byte sender limit"
                ),
            );
        }
    }

    for warning in &program.warnings {
        if warning.severity == ProgramWarningSeverity::Warning {
            continue;
        }
        if warning_allowed_by_policy(program, warning, policy) {
            continue;
        }
        let kind = match warning.code {
            ProgramWarningCode::SpindleActivation => DryRunBlockerKind::SpindleActivation,
            ProgramWarningCode::SpindleSpeed => DryRunBlockerKind::SpindleSpeed,
            ProgramWarningCode::ToolChange => DryRunBlockerKind::ToolChange,
            ProgramWarningCode::UnsafeMachineCommand => {
                if seen
                    .iter()
                    .any(|(line, _)| *line == Some(warning.source_line))
                {
                    continue;
                }
                DryRunBlockerKind::UnsupportedProgram
            }
            _ => DryRunBlockerKind::UnsupportedProgram,
        };
        add_blocker(
            &mut blockers,
            &mut seen,
            Some(warning.source_line),
            kind,
            warning.message.clone(),
        );
    }

    if !blockers.is_empty() {
        return Err(DryRunPolicyError::Rejected(blockers.len(), blockers));
    }

    let line_timings = line_timings(program);
    let mut program_lines = Vec::new();
    let mut selected_tool = None;
    for line in program
        .lines
        .iter()
        .filter(|line| line.executable && !line.normalized.is_empty())
    {
        let kind = classify_line(&line.normalized);
        if let Some(tool_number) = tool_number(&line.normalized) {
            selected_tool = Some(tool_number);
            if kind == DryRunLineKind::ToolChange {
                program_lines.push(DryRunLine {
                    source_line: Some(line.source_line),
                    command: format!("T{tool_number}"),
                    kind: DryRunLineKind::Program,
                    tool_number: None,
                    estimated_duration_ms: Some(0),
                });
            }
        }
        program_lines.push(DryRunLine {
            source_line: Some(line.source_line),
            command: line.normalized.clone(),
            kind,
            tool_number: if kind == DryRunLineKind::ToolChange {
                selected_tool
            } else {
                None
            },
            estimated_duration_ms: line_timings.get(&line.source_line).copied().flatten(),
        });
        if kind == DryRunLineKind::ProgramEnd {
            break;
        }
    }
    if program_lines.is_empty() {
        return Err(DryRunPolicyError::EmptyProgram);
    }

    let mut lines = Vec::with_capacity(SAFETY_PREAMBLE.len() + program_lines.len());
    lines.extend(SAFETY_PREAMBLE.map(|command| DryRunLine {
        source_line: None,
        command: command.to_owned(),
        kind: DryRunLineKind::SafetyPreamble,
        tool_number: None,
        estimated_duration_ms: Some(0),
    }));
    lines.extend(program_lines);

    let estimated_total_ms = lines
        .iter()
        .filter_map(DryRunLine::estimated_duration_ms)
        .fold(0u64, u64::saturating_add);
    let time_estimate_complete = program.summary.time_estimate_complete
        && lines
            .iter()
            .all(|line| line.estimated_duration_ms().is_some());

    Ok(DryRunPlan {
        source_name: program.source_name.clone(),
        source_line_count: program.summary.line_count,
        lines,
        estimated_total_ms,
        time_estimate_complete,
    })
}

fn line_timings(program: &GcodeProgram) -> BTreeMap<usize, Option<u64>> {
    let mut timings = program
        .lines
        .iter()
        .filter(|line| line.executable)
        .map(|line| (line.source_line, Some(0u64)))
        .collect::<BTreeMap<_, _>>();

    for segment in &program.toolpath {
        let entry = timings.entry(segment.source_line).or_insert(Some(0));
        match (entry.as_mut(), segment.estimated_duration_seconds) {
            (Some(total), Some(seconds)) => {
                *total = total.saturating_add(seconds_to_millis(seconds));
            }
            _ => *entry = None,
        }
    }
    for line in &program.lines {
        if let Some(seconds) = dwell_seconds(&line.normalized)
            && let Some(Some(total)) = timings.get_mut(&line.source_line)
        {
            *total = total.saturating_add(seconds_to_millis(seconds));
        }
    }
    timings
}

fn dwell_seconds(normalized: &str) -> Option<f64> {
    let words = normalized
        .split_whitespace()
        .filter_map(split_word)
        .collect::<Vec<_>>();
    words
        .iter()
        .any(|(letter, value)| *letter == 'G' && code_is(*value, 4.0))
        .then(|| {
            words
                .iter()
                .rev()
                .find(|(letter, _)| *letter == 'P')
                .map(|(_, value)| *value)
        })
        .flatten()
}

fn seconds_to_millis(seconds: f64) -> u64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        0
    } else {
        (seconds * 1_000.0).round().min(u64::MAX as f64) as u64
    }
}

fn inspect_normalized_line(
    source_line: usize,
    normalized: &str,
    policy: ProgramRunPolicy,
    blockers: &mut Vec<DryRunBlocker>,
    seen: &mut BTreeSet<(Option<usize>, DryRunBlockerKind)>,
) {
    for word in normalized.split_whitespace() {
        let Some((letter, value)) = split_word(word) else {
            continue;
        };
        match letter {
            'M' if (code_is(value, 3.0) || code_is(value, 4.0))
                && policy == ProgramRunPolicy::AirRun =>
            {
                add_blocker(
                    blockers,
                    seen,
                    Some(source_line),
                    DryRunBlockerKind::SpindleActivation,
                    "M3/M4 spindle activation is forbidden by dry-run policy",
                )
            }
            'M' if code_is(value, 7.0) || code_is(value, 8.0) => add_blocker(
                blockers,
                seen,
                Some(source_line),
                DryRunBlockerKind::CoolantActivation,
                "M7/M8 coolant activation is forbidden by dry-run policy",
            ),
            'M' if code_is(value, 6.0) && policy == ProgramRunPolicy::AirRun => add_blocker(
                blockers,
                seen,
                Some(source_line),
                DryRunBlockerKind::ToolChange,
                "M6 tool change is forbidden by dry-run policy",
            ),
            'S' if value.abs() > f64::EPSILON && policy == ProgramRunPolicy::AirRun => add_blocker(
                blockers,
                seen,
                Some(source_line),
                DryRunBlockerKind::SpindleSpeed,
                "non-zero spindle speed is forbidden by dry-run policy",
            ),
            'G' if (38.0..39.0).contains(&value) => add_blocker(
                blockers,
                seen,
                Some(source_line),
                DryRunBlockerKind::ProbeCycle,
                "G38.x probing is forbidden by dry-run policy",
            ),
            'G' if code_is(value, 28.0) || code_is(value, 30.0) || code_is(value, 53.0) => {
                add_blocker(
                    blockers,
                    seen,
                    Some(source_line),
                    DryRunBlockerKind::MachineCoordinateMotion,
                    "machine/reference-coordinate movement is forbidden by dry-run policy",
                )
            }
            'G' if code_is(value, 10.0) || code_is(value, 92.0) => add_blocker(
                blockers,
                seen,
                Some(source_line),
                DryRunBlockerKind::CoordinateMutation,
                "coordinate mutation is forbidden by dry-run policy",
            ),
            _ => {}
        }
    }
    if policy == ProgramRunPolicy::Cutting && has_code(normalized, 'M', 6.0) {
        for word in normalized.split_whitespace() {
            let Some((letter, value)) = split_word(word) else {
                continue;
            };
            let allowed = letter == 'N' || letter == 'T' || (letter == 'M' && code_is(value, 6.0));
            if !allowed {
                add_blocker(
                    blockers,
                    seen,
                    Some(source_line),
                    DryRunBlockerKind::ToolChange,
                    "M6 must be isolated from motion, spindle, coolant, and coordinate words",
                );
                break;
            }
        }
    }
}

fn warning_allowed_by_policy(
    _program: &GcodeProgram,
    warning: &millo_gcode::ProgramWarning,
    policy: ProgramRunPolicy,
) -> bool {
    policy == ProgramRunPolicy::Cutting
        && matches!(
            warning.code,
            ProgramWarningCode::SpindleActivation
                | ProgramWarningCode::SpindleSpeed
                | ProgramWarningCode::ToolChange
        )
}

fn classify_line(normalized: &str) -> DryRunLineKind {
    let mut pause = false;
    for word in normalized.split_whitespace() {
        let Some(('M', value)) = split_word(word) else {
            continue;
        };
        if code_is(value, 2.0) || code_is(value, 30.0) {
            return DryRunLineKind::ProgramEnd;
        }
        if code_is(value, 6.0) {
            return DryRunLineKind::ToolChange;
        }
        if code_is(value, 0.0) || code_is(value, 1.0) {
            pause = true;
        }
    }
    if pause {
        DryRunLineKind::ProgramPause
    } else {
        DryRunLineKind::Program
    }
}

fn has_code(normalized: &str, letter: char, expected: f64) -> bool {
    normalized.split_whitespace().any(|word| {
        split_word(word).is_some_and(|(actual_letter, value)| {
            actual_letter == letter && code_is(value, expected)
        })
    })
}

fn tool_number(normalized: &str) -> Option<u8> {
    normalized.split_whitespace().rev().find_map(|word| {
        let ('T', value) = split_word(word)? else {
            return None;
        };
        (value >= 0.0 && value <= u8::MAX as f64 && value.fract().abs() < f64::EPSILON)
            .then_some(value as u8)
    })
}

fn split_word(word: &str) -> Option<(char, f64)> {
    let mut characters = word.chars();
    let letter = characters.next()?.to_ascii_uppercase();
    let value = characters.as_str().parse().ok()?;
    Some((letter, value))
}

fn code_is(value: f64, expected: f64) -> bool {
    (value - expected).abs() < 1e-9
}

fn add_blocker(
    blockers: &mut Vec<DryRunBlocker>,
    seen: &mut BTreeSet<(Option<usize>, DryRunBlockerKind)>,
    source_line: Option<usize>,
    kind: DryRunBlockerKind,
    message: impl Into<String>,
) {
    if seen.insert((source_line, kind)) {
        blockers.push(DryRunBlocker {
            source_line,
            kind,
            message: message.into(),
        });
    }
}

#[cfg(test)]
mod tests {
    use millo_gcode::{ProgramParseRequest, parse_program};

    use super::*;

    fn parse(source: &str) -> GcodeProgram {
        parse_program(ProgramParseRequest {
            source_name: "fixture.nc".to_owned(),
            source: source.to_owned(),
        })
        .unwrap()
    }

    #[test]
    fn builds_only_from_normalized_lines_and_adds_an_off_preamble() {
        let plan = build_dry_run_plan(&parse("G21 G90 ; units\nG0 X1\nM5 M9")).unwrap();

        assert_eq!(
            plan.lines()
                .iter()
                .map(DryRunLine::command)
                .collect::<Vec<_>>(),
            vec!["M5", "M9", "G21 G90", "G0 X1", "M5 M9"]
        );
        assert_eq!(plan.lines()[0].kind(), DryRunLineKind::SafetyPreamble);
        assert_eq!(plan.lines()[2].source_line(), Some(1));
    }

    #[test]
    fn carries_deterministic_per_line_timing_and_marks_rapid_as_a_lower_bound() {
        let timed = build_dry_run_plan(&parse(
            "G21 G90 G94\nG1 X60 F60\nG4 P0.250\nG1 X90 F30\nM30",
        ))
        .unwrap();
        assert_eq!(timed.estimated_total_ms(), 120_250);
        assert!(timed.time_estimate_complete());
        assert_eq!(
            timed
                .lines()
                .iter()
                .find(|line| line.source_line() == Some(2))
                .and_then(DryRunLine::estimated_duration_ms),
            Some(60_000)
        );
        assert_eq!(
            timed
                .lines()
                .iter()
                .find(|line| line.source_line() == Some(3))
                .and_then(DryRunLine::estimated_duration_ms),
            Some(250)
        );

        let rapid = build_dry_run_plan(&parse("G21 G90 G94\nG0 X10\nG1 X20 F60\nM30")).unwrap();
        assert_eq!(rapid.estimated_total_ms(), 10_000);
        assert!(!rapid.time_estimate_complete());
        assert_eq!(
            rapid
                .lines()
                .iter()
                .find(|line| line.source_line() == Some(2))
                .and_then(DryRunLine::estimated_duration_ms),
            None
        );
    }

    #[test]
    fn blocks_each_operator_critical_command_family() {
        let program = parse("M3\nS1000\nM8\nG38.2 Z-1 F10\nM6\nG53 G0 X0\nG10 L20 P1 X0");
        let error = build_dry_run_plan(&program).unwrap_err();
        let kinds = error
            .blockers()
            .iter()
            .map(|blocker| blocker.kind)
            .collect::<BTreeSet<_>>();

        assert!(kinds.contains(&DryRunBlockerKind::SpindleActivation));
        assert!(kinds.contains(&DryRunBlockerKind::SpindleSpeed));
        assert!(kinds.contains(&DryRunBlockerKind::CoolantActivation));
        assert!(kinds.contains(&DryRunBlockerKind::ProbeCycle));
        assert!(kinds.contains(&DryRunBlockerKind::ToolChange));
        assert!(kinds.contains(&DryRunBlockerKind::MachineCoordinateMotion));
        assert!(kinds.contains(&DryRunBlockerKind::CoordinateMutation));
    }

    #[test]
    fn cutting_policy_accepts_spindle_words_but_keeps_other_guards() {
        let plan = build_program_run_plan(
            &parse("G21 G90 G94\nS12000 M3\nG1 X1 F50\nM5"),
            ProgramRunPolicy::Cutting,
        )
        .unwrap();
        assert!(
            plan.lines()
                .iter()
                .any(|line| line.command() == "S12000 M3")
        );

        let rejected = build_program_run_plan(
            &parse("G21 G90 G94\nM8\nG1 X1 F50"),
            ProgramRunPolicy::Cutting,
        )
        .unwrap_err();
        assert!(
            rejected
                .blockers()
                .iter()
                .any(|blocker| blocker.kind == DryRunBlockerKind::CoolantActivation)
        );
    }

    #[test]
    fn cutting_policy_turns_m6_into_an_isolated_host_barrier() {
        let plan = build_program_run_plan(
            &parse("G21 G90 G94\nT2 M6\nG1 X1 F50\nT7\nM6\nM30"),
            ProgramRunPolicy::Cutting,
        )
        .unwrap();
        let lines = plan.lines();

        let first_change = lines
            .iter()
            .position(|line| line.kind() == DryRunLineKind::ToolChange)
            .unwrap();
        assert_eq!(lines[first_change - 1].command(), "T2");
        assert_eq!(lines[first_change].command(), "T2 M6");
        assert_eq!(lines[first_change].tool_number(), Some(2));

        let changes = lines
            .iter()
            .filter(|line| line.kind() == DryRunLineKind::ToolChange)
            .collect::<Vec<_>>();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[1].command(), "M6");
        assert_eq!(changes[1].tool_number(), Some(7));
    }

    #[test]
    fn m6_remains_blocked_in_air_runs_and_cannot_share_a_cutting_block() {
        let air = build_program_run_plan(&parse("T2 M6"), ProgramRunPolicy::AirRun).unwrap_err();
        assert!(
            air.blockers()
                .iter()
                .any(|blocker| blocker.kind == DryRunBlockerKind::ToolChange)
        );

        for source in ["M6 G0 X1", "M6 S12000", "M6 M5", "M6 P1"] {
            let error =
                build_program_run_plan(&parse(source), ProgramRunPolicy::Cutting).unwrap_err();
            assert!(error.blockers().iter().any(|blocker| {
                blocker.kind == DryRunBlockerKind::ToolChange
                    && blocker.message.contains("must be isolated")
            }));
        }
    }

    #[test]
    fn classifies_program_pause_and_stops_the_plan_at_program_end() {
        let plan = build_program_run_plan(
            &parse("G21\nM0\nG1 X1 F10\nM30\nG1 X99"),
            ProgramRunPolicy::Cutting,
        )
        .unwrap();
        assert_eq!(plan.lines()[3].kind(), DryRunLineKind::ProgramPause);
        assert_eq!(plan.lines()[5].kind(), DryRunLineKind::ProgramEnd);
        assert!(
            !plan
                .lines()
                .iter()
                .any(|line| line.command().contains("X99"))
        );
    }

    #[test]
    fn hardware_square_compiles_to_a_spindle_safe_air_run_plan() {
        let plan = build_program_run_plan(
            &parse(include_str!(
                "../../../fixtures/programs/air-square-20mm.nc"
            )),
            ProgramRunPolicy::AirRun,
        )
        .unwrap();

        assert_eq!(plan.source_name(), "fixture.nc");
        assert_eq!(plan.lines()[0].command(), "M5");
        assert_eq!(plan.lines()[1].command(), "M9");
        assert!(plan.lines().iter().all(|line| {
            line.command()
                .split_whitespace()
                .all(|word| word != "M3" && word != "M4" && !word.starts_with('S'))
        }));
        assert_eq!(plan.lines().last().unwrap().command(), "M30");
    }

    #[test]
    fn parser_safety_errors_cannot_be_bypassed_by_summary_flags() {
        let mut program = parse("M80\nG1 X1");
        program.summary.dry_run_eligible = true;

        let error = build_dry_run_plan(&program).unwrap_err();

        assert!(
            error
                .blockers()
                .iter()
                .any(|blocker| blocker.kind == DryRunBlockerKind::UnsupportedProgram)
        );
    }

    #[test]
    fn blocks_incomplete_preview_and_grbl_lines_over_the_sender_limit() {
        let incomplete = build_dry_run_plan(&parse("G2 X1 Y1")).unwrap_err();
        assert!(
            incomplete
                .blockers()
                .iter()
                .any(|blocker| blocker.kind == DryRunBlockerKind::IncompletePreview)
        );

        let oversized_source = format!("G1 X1 {}", "N1 ".repeat(100));
        let oversized = build_dry_run_plan(&parse(&oversized_source)).unwrap_err();
        assert!(
            oversized
                .blockers()
                .iter()
                .any(|blocker| blocker.kind == DryRunBlockerKind::CommandTooLong)
        );
    }
}
