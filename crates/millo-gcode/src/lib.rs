use std::collections::BTreeSet;
use std::f64::consts::{PI, TAU};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_SOURCE_NAME_BYTES: usize = 255;
pub const MAX_SOURCE_LINES: usize = 200_000;
pub const MAX_PREVIEW_POINTS: usize = 500_000;
const ARC_MAX_ANGLE_RAD: f64 = PI / 36.0;
const ARC_MAX_CHORD_MM: f64 = 0.5;
const POSITION_EPSILON_MM: f64 = 1e-9;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramParseRequest {
    pub source_name: String,
    pub source: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramParseOptions {
    pub block_delete: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramPoint {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramBounds {
    pub min: ProgramPoint,
    pub max: ProgramPoint,
    pub size: ProgramPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolpathKind {
    Rapid,
    Linear,
    ArcClockwise,
    ArcCounterclockwise,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolpathSegment {
    pub source_line: usize,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional_block: bool,
    pub kind: ToolpathKind,
    pub points: Vec<ProgramPoint>,
    pub distance_mm: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feed_rate_mm_per_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_duration_seconds: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProgramWarningSeverity {
    Warning,
    Safety,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProgramWarningCode {
    UnclosedComment,
    UnexpectedCommentClose,
    InvalidToken,
    DuplicateWord,
    OptionalBlock,
    OptionalBlockUnsupported,
    ChecksumValidated,
    ChecksumInvalid,
    ChecksumUnsupported,
    UnsupportedGCode,
    UnsupportedMCode,
    UnsupportedWord,
    UnsupportedPlane,
    CoordinateSystemIgnored,
    UnsafeMachineCommand,
    SpindleActivation,
    SpindleSpeed,
    ToolChange,
    ArcDefinition,
    DwellDefinition,
    FeedRate,
    ModalGroupConflict,
    PreviewLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramWarning {
    pub source_line: usize,
    pub severity: ProgramWarningSeverity,
    pub code: ProgramWarningCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramLine {
    pub source_line: usize,
    pub source: String,
    pub normalized: String,
    pub executable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub optional_block: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub block_deleted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<u8>,
    pub warning_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProgramMotionMode {
    None,
    Rapid,
    Linear,
    ArcClockwise,
    ArcCounterclockwise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProgramUnitMode {
    Millimeters,
    Inches,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProgramDistanceMode {
    Absolute,
    Incremental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProgramFeedMode {
    InverseTime,
    UnitsPerMinute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProgramPlane {
    Xy,
    Xz,
    Yz,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProgramSpindleMode {
    Off,
    Clockwise,
    Counterclockwise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProgramWorkCoordinateSystem {
    G54,
    G55,
    G56,
    G57,
    G58,
    G59,
}

/// Parser state immediately before one executable source block.
///
/// This is intentionally not serialized to the webview. It exists for the
/// native recovery planner, which must reconstruct modal state without
/// replaying earlier motion or side effects.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramExecutionCheckpoint {
    pub source_line: usize,
    pub position: ProgramPoint,
    pub motion: ProgramMotionMode,
    pub units: ProgramUnitMode,
    pub distance: ProgramDistanceMode,
    pub arc_distance: ProgramDistanceMode,
    pub feed_mode: ProgramFeedMode,
    pub feed_rate: Option<f64>,
    pub plane: ProgramPlane,
    pub work_coordinate_system: ProgramWorkCoordinateSystem,
    pub selected_tool: Option<u8>,
    pub spindle_mode: ProgramSpindleMode,
    pub spindle_speed: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramFeatures {
    pub uses_imperial_units: bool,
    pub uses_incremental_distance: bool,
    pub has_spindle_activation: bool,
    pub has_spindle_speed: bool,
    pub has_tool_change: bool,
    pub has_probe_cycle: bool,
    pub has_machine_coordinate_move: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramSummary {
    pub line_count: usize,
    pub executable_line_count: usize,
    pub motion_count: usize,
    pub rapid_distance_mm: f64,
    pub cutting_distance_mm: f64,
    pub estimated_motion_time_seconds: f64,
    pub dwell_time_seconds: f64,
    pub estimated_total_time_seconds: f64,
    pub time_estimate_complete: bool,
    pub bounds: Option<ProgramBounds>,
    pub preview_complete: bool,
    pub dry_run_eligible: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GcodeProgram {
    pub source_name: String,
    pub block_delete_enabled: bool,
    pub lines: Vec<ProgramLine>,
    pub warnings: Vec<ProgramWarning>,
    pub features: ProgramFeatures,
    pub summary: ProgramSummary,
    pub toolpath: Vec<ToolpathSegment>,
    #[serde(default, skip_serializing)]
    pub execution_checkpoints: Vec<ProgramExecutionCheckpoint>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProgramParseError {
    #[error("G-code source name is required")]
    MissingSourceName,
    #[error("G-code source name exceeds the {max_bytes} byte limit")]
    SourceNameTooLong { max_bytes: usize },
    #[error("G-code source is empty")]
    EmptySource,
    #[error("G-code source exceeds the {max_bytes} byte limit")]
    SourceTooLarge { max_bytes: usize },
    #[error("G-code source exceeds the {max_lines} line limit")]
    TooManyLines { max_lines: usize },
}

#[derive(Debug, Clone)]
struct Word {
    letter: char,
    value: f64,
    lexeme: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MotionMode {
    None,
    Rapid,
    Linear,
    ArcClockwise,
    ArcCounterclockwise,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UnitMode {
    Millimeters,
    Inches,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DistanceMode {
    Absolute,
    Incremental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArcDistanceMode {
    Absolute,
    Incremental,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FeedMode {
    InverseTime,
    UnitsPerMinute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Plane {
    Xy,
    Xz,
    Yz,
}

struct Parser {
    block_delete: bool,
    position: ProgramPoint,
    motion: MotionMode,
    units: UnitMode,
    distance: DistanceMode,
    arc_distance: ArcDistanceMode,
    feed_mode: FeedMode,
    feed_rate: Option<f64>,
    plane: Plane,
    lines: Vec<ProgramLine>,
    warnings: Vec<ProgramWarning>,
    features: ProgramFeatures,
    toolpath: Vec<ToolpathSegment>,
    bounds: BoundsAccumulator,
    preview_points: usize,
    preview_complete: bool,
    rapid_distance_mm: f64,
    cutting_distance_mm: f64,
    estimated_motion_time_seconds: f64,
    dwell_time_seconds: f64,
    time_estimate_complete: bool,
    execution_checkpoints: Vec<ProgramExecutionCheckpoint>,
    work_coordinate_system: ProgramWorkCoordinateSystem,
    selected_tool: Option<u8>,
    spindle_mode: ProgramSpindleMode,
    spindle_speed: Option<f64>,
}

impl Default for Parser {
    fn default() -> Self {
        Self {
            block_delete: false,
            position: ProgramPoint::default(),
            motion: MotionMode::Rapid,
            units: UnitMode::Millimeters,
            distance: DistanceMode::Absolute,
            arc_distance: ArcDistanceMode::Incremental,
            feed_mode: FeedMode::UnitsPerMinute,
            feed_rate: None,
            plane: Plane::Xy,
            lines: Vec::new(),
            warnings: Vec::new(),
            features: ProgramFeatures::default(),
            toolpath: Vec::new(),
            bounds: BoundsAccumulator::default(),
            preview_points: 0,
            preview_complete: true,
            rapid_distance_mm: 0.0,
            cutting_distance_mm: 0.0,
            estimated_motion_time_seconds: 0.0,
            dwell_time_seconds: 0.0,
            time_estimate_complete: true,
            execution_checkpoints: Vec::new(),
            work_coordinate_system: ProgramWorkCoordinateSystem::G54,
            selected_tool: None,
            spindle_mode: ProgramSpindleMode::Off,
            spindle_speed: None,
        }
    }
}

#[derive(Default)]
struct BoundsAccumulator {
    min: Option<ProgramPoint>,
    max: Option<ProgramPoint>,
}

impl BoundsAccumulator {
    fn include(&mut self, point: ProgramPoint) {
        match (&mut self.min, &mut self.max) {
            (Some(min), Some(max)) => {
                min.x = min.x.min(point.x);
                min.y = min.y.min(point.y);
                min.z = min.z.min(point.z);
                max.x = max.x.max(point.x);
                max.y = max.y.max(point.y);
                max.z = max.z.max(point.z);
            }
            _ => {
                self.min = Some(point);
                self.max = Some(point);
            }
        }
    }

    fn finish(self) -> Option<ProgramBounds> {
        let min = self.min?;
        let max = self.max?;
        Some(ProgramBounds {
            min,
            max,
            size: ProgramPoint {
                x: max.x - min.x,
                y: max.y - min.y,
                z: max.z - min.z,
            },
        })
    }
}

pub fn parse_program(request: ProgramParseRequest) -> Result<GcodeProgram, ProgramParseError> {
    parse_program_with_options(request, ProgramParseOptions::default())
}

pub fn parse_program_with_options(
    request: ProgramParseRequest,
    options: ProgramParseOptions,
) -> Result<GcodeProgram, ProgramParseError> {
    let source_name = request.source_name.trim();
    if source_name.is_empty() {
        return Err(ProgramParseError::MissingSourceName);
    }
    if source_name.len() > MAX_SOURCE_NAME_BYTES {
        return Err(ProgramParseError::SourceNameTooLong {
            max_bytes: MAX_SOURCE_NAME_BYTES,
        });
    }
    if request.source.len() > MAX_SOURCE_BYTES {
        return Err(ProgramParseError::SourceTooLarge {
            max_bytes: MAX_SOURCE_BYTES,
        });
    }
    if request.source.trim().is_empty() {
        return Err(ProgramParseError::EmptySource);
    }
    let line_count = request.source.lines().count();
    if line_count > MAX_SOURCE_LINES {
        return Err(ProgramParseError::TooManyLines {
            max_lines: MAX_SOURCE_LINES,
        });
    }

    let mut parser = Parser {
        block_delete: options.block_delete,
        ..Parser::default()
    };
    for (index, raw) in request.source.lines().enumerate() {
        parser.parse_line(index + 1, raw.trim_end_matches('\r'));
    }

    let has_blocker = parser
        .warnings
        .iter()
        .any(|warning| warning.severity != ProgramWarningSeverity::Warning);
    let summary = ProgramSummary {
        line_count,
        executable_line_count: parser.lines.iter().filter(|line| line.executable).count(),
        motion_count: parser.toolpath.len(),
        rapid_distance_mm: parser.rapid_distance_mm,
        cutting_distance_mm: parser.cutting_distance_mm,
        estimated_motion_time_seconds: parser.estimated_motion_time_seconds,
        dwell_time_seconds: parser.dwell_time_seconds,
        estimated_total_time_seconds: parser.estimated_motion_time_seconds
            + parser.dwell_time_seconds,
        time_estimate_complete: parser.time_estimate_complete,
        bounds: parser.bounds.finish(),
        preview_complete: parser.preview_complete,
        dry_run_eligible: parser.preview_complete && !has_blocker,
    };

    Ok(GcodeProgram {
        source_name: source_name.to_owned(),
        block_delete_enabled: options.block_delete,
        lines: parser.lines,
        warnings: parser.warnings,
        features: parser.features,
        summary,
        toolpath: parser.toolpath,
        execution_checkpoints: parser.execution_checkpoints,
    })
}

impl Parser {
    fn parse_line(&mut self, source_line: usize, source: &str) {
        let warning_start = self.warnings.len();
        let (checksummed_code, checksum) =
            validate_checksum(source, source_line, &mut self.warnings);
        let code = strip_comments(&checksummed_code, source_line, &mut self.warnings);
        let (code, optional_block) = strip_optional_block(&code, source_line, &mut self.warnings);
        let words = if code.trim() == "%" {
            Vec::new()
        } else {
            tokenize(&code, source_line, &mut self.warnings)
        };
        if checksum.is_some() && words.first().is_none_or(|word| word.letter != 'N') {
            self.preview_complete = false;
            self.warn(
                source_line,
                ProgramWarningSeverity::Error,
                ProgramWarningCode::ChecksumInvalid,
                "a checksummed block must begin with an N line number",
            );
        }
        let normalized = words
            .iter()
            .map(|word| word.lexeme.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let has_program_number = words.iter().any(|word| word.letter == 'O');
        let executable = words.iter().any(|word| !matches!(word.letter, 'N' | 'O'));

        if has_program_number && executable {
            self.preview_complete = false;
            self.warn(
                source_line,
                ProgramWarningSeverity::Error,
                ProgramWarningCode::UnsupportedWord,
                "O program numbers must be on a metadata-only line",
            );
        }

        let block_deleted = executable && optional_block && self.block_delete;
        if executable && !block_deleted {
            self.execution_checkpoints
                .push(self.execution_checkpoint(source_line));
            self.apply_block(source_line, &words, optional_block);
        }

        self.lines.push(ProgramLine {
            source_line,
            source: source.to_owned(),
            normalized,
            executable,
            optional_block,
            block_deleted,
            checksum,
            warning_count: self.warnings.len() - warning_start,
        });
    }

    fn execution_checkpoint(&self, source_line: usize) -> ProgramExecutionCheckpoint {
        ProgramExecutionCheckpoint {
            source_line,
            position: self.position,
            motion: match self.motion {
                MotionMode::None => ProgramMotionMode::None,
                MotionMode::Rapid => ProgramMotionMode::Rapid,
                MotionMode::Linear => ProgramMotionMode::Linear,
                MotionMode::ArcClockwise => ProgramMotionMode::ArcClockwise,
                MotionMode::ArcCounterclockwise => ProgramMotionMode::ArcCounterclockwise,
            },
            units: match self.units {
                UnitMode::Millimeters => ProgramUnitMode::Millimeters,
                UnitMode::Inches => ProgramUnitMode::Inches,
            },
            distance: match self.distance {
                DistanceMode::Absolute => ProgramDistanceMode::Absolute,
                DistanceMode::Incremental => ProgramDistanceMode::Incremental,
            },
            arc_distance: match self.arc_distance {
                ArcDistanceMode::Absolute => ProgramDistanceMode::Absolute,
                ArcDistanceMode::Incremental => ProgramDistanceMode::Incremental,
            },
            feed_mode: match self.feed_mode {
                FeedMode::InverseTime => ProgramFeedMode::InverseTime,
                FeedMode::UnitsPerMinute => ProgramFeedMode::UnitsPerMinute,
            },
            feed_rate: self.feed_rate,
            plane: match self.plane {
                Plane::Xy => ProgramPlane::Xy,
                Plane::Xz => ProgramPlane::Xz,
                Plane::Yz => ProgramPlane::Yz,
            },
            work_coordinate_system: self.work_coordinate_system,
            selected_tool: self.selected_tool,
            spindle_mode: self.spindle_mode,
            spindle_speed: self.spindle_speed,
        }
    }

    fn apply_block(&mut self, source_line: usize, words: &[Word], optional_block: bool) {
        let mut block_motion = self.motion;
        let mut skip_motion = false;
        let mut dwell = false;
        let mut seen = BTreeSet::new();
        let mut modal_groups = BTreeSet::new();
        let mut m_modal_groups = BTreeSet::new();

        for word in words {
            if !seen.insert(word.letter)
                && matches!(
                    word.letter,
                    'X' | 'Y' | 'Z' | 'I' | 'J' | 'K' | 'R' | 'F' | 'S'
                )
            {
                self.warn(
                    source_line,
                    ProgramWarningSeverity::Warning,
                    ProgramWarningCode::DuplicateWord,
                    format!(
                        "{} appears more than once; the final value is used",
                        word.letter
                    ),
                );
            }

            match word.letter {
                'G' => {
                    if let Some(group) = g_modal_group(word.value)
                        && !modal_groups.insert(group)
                    {
                        skip_motion = true;
                        self.preview_complete = false;
                        self.warn(
                            source_line,
                            ProgramWarningSeverity::Error,
                            ProgramWarningCode::ModalGroupConflict,
                            format!("more than one G-code from modal group {group} in one block"),
                        );
                    }
                    self.apply_g_code(
                        source_line,
                        word.value,
                        &mut block_motion,
                        &mut skip_motion,
                        &mut dwell,
                    )
                }
                'M' => {
                    if let Some(group) = m_modal_group(word.value)
                        && !m_modal_groups.insert(group)
                    {
                        skip_motion = true;
                        self.preview_complete = false;
                        self.warn(
                            source_line,
                            ProgramWarningSeverity::Error,
                            ProgramWarningCode::ModalGroupConflict,
                            format!("more than one M-code from modal group {group} in one block"),
                        );
                    }
                    self.apply_m_code(source_line, word.value)
                }
                'S' => {
                    self.spindle_speed = Some(word.value);
                    if word.value.abs() > f64::EPSILON {
                        self.features.has_spindle_speed = true;
                        self.warn(
                            source_line,
                            ProgramWarningSeverity::Safety,
                            ProgramWarningCode::SpindleSpeed,
                            "spindle speed is recorded but will be blocked by dry run",
                        );
                    }
                }
                'T' if word.value < 0.0
                    || word.value.fract().abs() > f64::EPSILON
                    || word.value > u8::MAX as f64 =>
                {
                    skip_motion = true;
                    self.preview_complete = false;
                    self.warn(
                        source_line,
                        ProgramWarningSeverity::Error,
                        ProgramWarningCode::UnsupportedWord,
                        "T tool number must be an integer from 0 to 255",
                    );
                }
                'T' => self.selected_tool = Some(word.value as u8),
                'N' | 'O' | 'X' | 'Y' | 'Z' | 'I' | 'J' | 'K' | 'R' | 'F' | 'P' | 'L' | 'H'
                | 'D' | 'Q' => {}
                letter => {
                    skip_motion = true;
                    self.preview_complete = false;
                    self.warn(
                        source_line,
                        ProgramWarningSeverity::Error,
                        ProgramWarningCode::UnsupportedWord,
                        format!("{letter} words are not supported by the preview parser"),
                    );
                }
            }
        }
        self.motion = block_motion;

        let unit_scale = match self.units {
            UnitMode::Millimeters => 1.0,
            UnitMode::Inches => 25.4,
        };
        let last_raw = |letter| {
            words
                .iter()
                .rev()
                .find(|word| word.letter == letter)
                .map(|word| word.value)
        };
        let last = |letter| last_raw(letter).map(|value| value * unit_scale);
        let block_feed = last_raw('F');
        if let Some(feed) = block_feed {
            if feed <= 0.0 {
                self.feed_rate = None;
                self.time_estimate_complete = false;
                self.warn(
                    source_line,
                    ProgramWarningSeverity::Error,
                    ProgramWarningCode::FeedRate,
                    "feed rate must be greater than zero",
                );
            } else {
                self.feed_rate = Some(match self.feed_mode {
                    FeedMode::UnitsPerMinute => feed * unit_scale,
                    FeedMode::InverseTime => feed,
                });
            }
        }

        if dwell {
            match last_raw('P') {
                Some(seconds) if seconds >= 0.0 => self.dwell_time_seconds += seconds,
                _ => {
                    self.time_estimate_complete = false;
                    self.warn(
                        source_line,
                        ProgramWarningSeverity::Error,
                        ProgramWarningCode::DwellDefinition,
                        "G4 dwell requires a non-negative P value in seconds",
                    );
                }
            }
            if words
                .iter()
                .any(|word| matches!(word.letter, 'X' | 'Y' | 'Z' | 'I' | 'J' | 'K' | 'R'))
            {
                self.preview_complete = false;
                self.warn(
                    source_line,
                    ProgramWarningSeverity::Error,
                    ProgramWarningCode::DwellDefinition,
                    "G4 dwell cannot contain motion or arc words",
                );
            }
            return;
        }

        let x = last('X');
        let y = last('Y');
        let z = last('Z');
        let i = last('I');
        let j = last('J');
        let k = last('K');
        let radius = last('R');
        let has_axis = x.is_some() || y.is_some() || z.is_some();
        let arc_definition = i.is_some() || j.is_some() || k.is_some() || radius.is_some();
        let is_arc = matches!(
            block_motion,
            MotionMode::ArcClockwise | MotionMode::ArcCounterclockwise
        );
        let has_g10 = words
            .iter()
            .any(|word| word.letter == 'G' && code_is(word.value, 10.0));
        let mut invalid_context = BTreeSet::new();
        for word in words {
            let invalid = match word.letter {
                'I' | 'J' | 'K' | 'R' => !is_arc,
                'P' | 'L' => !has_g10,
                'H' | 'D' | 'Q' => true,
                _ => false,
            };
            if invalid && invalid_context.insert(word.letter) {
                skip_motion = true;
                self.preview_complete = false;
                self.warn(
                    source_line,
                    ProgramWarningSeverity::Error,
                    ProgramWarningCode::UnsupportedWord,
                    format!(
                        "{} is not valid for the active command in this block",
                        word.letter
                    ),
                );
            }
        }
        if is_arc && arc_definition && !has_axis {
            self.preview_complete = false;
            self.warn(
                source_line,
                ProgramWarningSeverity::Error,
                ProgramWarningCode::ArcDefinition,
                "GRBL arcs require at least one explicit X, Y, or Z target word",
            );
            return;
        }
        if !has_axis {
            return;
        }

        if block_motion == MotionMode::None {
            self.preview_complete = false;
            self.warn(
                source_line,
                ProgramWarningSeverity::Error,
                ProgramWarningCode::UnsupportedGCode,
                "axis words require an active G0, G1, G2, or G3 motion mode",
            );
            return;
        }

        let end = ProgramPoint {
            x: resolve_axis(self.position.x, x, self.distance),
            y: resolve_axis(self.position.y, y, self.distance),
            z: resolve_axis(self.position.z, z, self.distance),
        };
        if skip_motion {
            self.preview_complete = false;
            self.position = end;
            return;
        }

        let points = match block_motion {
            MotionMode::None => unreachable!("motion cancellation is handled above"),
            MotionMode::Rapid | MotionMode::Linear => vec![self.position, end],
            MotionMode::ArcClockwise | MotionMode::ArcCounterclockwise => {
                let (offset_u, offset_v, invalid_offset) = plane_offsets(self.plane, i, j, k);
                if invalid_offset || radius.is_some() && (i.is_some() || j.is_some() || k.is_some())
                {
                    self.preview_complete = false;
                    self.warn(
                        source_line,
                        ProgramWarningSeverity::Error,
                        ProgramWarningCode::ArcDefinition,
                        "arc must use only its plane offsets and cannot mix I/J/K with R",
                    );
                    self.position = end;
                    return;
                }
                match sample_arc(
                    self.position,
                    end,
                    ArcDefinition {
                        plane: self.plane,
                        offset_u,
                        offset_v,
                        radius,
                        clockwise: block_motion == MotionMode::ArcClockwise,
                        distance_mode: self.arc_distance,
                    },
                ) {
                    Some(points) => points,
                    None => {
                        self.preview_complete = false;
                        self.warn(
                            source_line,
                            ProgramWarningSeverity::Error,
                            ProgramWarningCode::ArcDefinition,
                            "arc requires a valid plane-specific I/J/K center or R radius",
                        );
                        self.position = end;
                        return;
                    }
                }
            }
        };
        self.position = end;
        if points.windows(2).all(|pair| same_point(pair[0], pair[1])) {
            return;
        }
        if self.preview_points + points.len() > MAX_PREVIEW_POINTS {
            if self.preview_complete {
                self.warn(
                    source_line,
                    ProgramWarningSeverity::Error,
                    ProgramWarningCode::PreviewLimit,
                    format!("preview exceeds the {MAX_PREVIEW_POINTS} point limit"),
                );
            }
            self.preview_complete = false;
            return;
        }

        let distance_mm = polyline_distance(&points);
        for point in &points {
            self.bounds.include(*point);
        }
        self.preview_points += points.len();
        let kind = match block_motion {
            MotionMode::None => unreachable!("motion cancellation is handled above"),
            MotionMode::Rapid => ToolpathKind::Rapid,
            MotionMode::Linear => ToolpathKind::Linear,
            MotionMode::ArcClockwise => ToolpathKind::ArcClockwise,
            MotionMode::ArcCounterclockwise => ToolpathKind::ArcCounterclockwise,
        };
        if kind == ToolpathKind::Rapid {
            self.rapid_distance_mm += distance_mm;
            self.time_estimate_complete = false;
        } else {
            self.cutting_distance_mm += distance_mm;
        }
        let (feed_rate_mm_per_min, estimated_duration_seconds) = match block_motion {
            MotionMode::None | MotionMode::Rapid => (None, None),
            MotionMode::Linear | MotionMode::ArcClockwise | MotionMode::ArcCounterclockwise => {
                match (self.feed_mode, self.feed_rate) {
                    (FeedMode::UnitsPerMinute, Some(feed)) if feed > 0.0 => {
                        (Some(feed), Some(distance_mm / feed * 60.0))
                    }
                    (FeedMode::InverseTime, Some(feed)) if feed > 0.0 && block_feed.is_some() => {
                        (None, Some(60.0 / feed))
                    }
                    _ => {
                        self.time_estimate_complete = false;
                        self.warn(
                            source_line,
                            ProgramWarningSeverity::Error,
                            ProgramWarningCode::FeedRate,
                            match self.feed_mode {
                                FeedMode::UnitsPerMinute => {
                                    "cutting motion requires a positive modal F feed rate"
                                }
                                FeedMode::InverseTime => {
                                    "every G93 motion block requires its own positive F value"
                                }
                            },
                        );
                        (None, None)
                    }
                }
            }
        };
        if let Some(seconds) = estimated_duration_seconds {
            self.estimated_motion_time_seconds += seconds;
        }
        self.toolpath.push(ToolpathSegment {
            source_line,
            optional_block,
            kind,
            points,
            distance_mm,
            feed_rate_mm_per_min,
            estimated_duration_seconds,
        });
    }

    fn apply_g_code(
        &mut self,
        source_line: usize,
        value: f64,
        motion: &mut MotionMode,
        skip_motion: &mut bool,
        dwell: &mut bool,
    ) {
        if code_is(value, 0.0) {
            *motion = MotionMode::Rapid;
        } else if code_is(value, 1.0) {
            *motion = MotionMode::Linear;
        } else if code_is(value, 2.0) {
            *motion = MotionMode::ArcClockwise;
        } else if code_is(value, 3.0) {
            *motion = MotionMode::ArcCounterclockwise;
        } else if code_is(value, 17.0) {
            self.plane = Plane::Xy;
        } else if code_is(value, 18.0) {
            self.plane = Plane::Xz;
        } else if code_is(value, 19.0) {
            self.plane = Plane::Yz;
        } else if code_is(value, 20.0) {
            if self.units != UnitMode::Inches {
                self.feed_rate = None;
            }
            self.units = UnitMode::Inches;
            self.features.uses_imperial_units = true;
        } else if code_is(value, 21.0) {
            if self.units != UnitMode::Millimeters {
                self.feed_rate = None;
            }
            self.units = UnitMode::Millimeters;
        } else if code_is(value, 90.0) {
            self.distance = DistanceMode::Absolute;
        } else if code_is(value, 91.0) {
            self.distance = DistanceMode::Incremental;
            self.features.uses_incremental_distance = true;
        } else if code_is(value, 90.1) {
            self.arc_distance = ArcDistanceMode::Absolute;
            *skip_motion = true;
            self.warn(
                source_line,
                ProgramWarningSeverity::Error,
                ProgramWarningCode::UnsupportedGCode,
                "G90.1 absolute arc centers can be previewed but are not supported by GRBL 1.1",
            );
        } else if code_is(value, 91.1) {
            self.arc_distance = ArcDistanceMode::Incremental;
        } else if code_is(value, 93.0) {
            if self.feed_mode != FeedMode::InverseTime {
                self.feed_rate = None;
            }
            self.feed_mode = FeedMode::InverseTime;
        } else if code_is(value, 94.0) {
            if self.feed_mode != FeedMode::UnitsPerMinute {
                self.feed_rate = None;
            }
            self.feed_mode = FeedMode::UnitsPerMinute;
        } else if code_is(value, 95.0) {
            *skip_motion = true;
            self.warn(
                source_line,
                ProgramWarningSeverity::Error,
                ProgramWarningCode::UnsupportedGCode,
                "G95 units-per-revolution feed is not supported by GRBL 1.1",
            );
        } else if code_is(value, 40.0) || code_is(value, 49.0) {
            // Common modal cancel commands leave nominal preview geometry unchanged.
        } else if code_is(value, 80.0) {
            *motion = MotionMode::None;
        } else if code_is(value, 61.0) {
            // Path control affects execution, not nominal preview geometry.
        } else if code_is(value, 64.0) {
            *skip_motion = true;
            self.warn(
                source_line,
                ProgramWarningSeverity::Error,
                ProgramWarningCode::UnsupportedGCode,
                "G64 is not supported by GRBL 1.1",
            );
        } else if (54.0..=59.0).contains(&value) && value.fract().abs() < f64::EPSILON {
            self.work_coordinate_system = match value as u8 {
                54 => ProgramWorkCoordinateSystem::G54,
                55 => ProgramWorkCoordinateSystem::G55,
                56 => ProgramWorkCoordinateSystem::G56,
                57 => ProgramWorkCoordinateSystem::G57,
                58 => ProgramWorkCoordinateSystem::G58,
                _ => ProgramWorkCoordinateSystem::G59,
            };
            self.warn(
                source_line,
                ProgramWarningSeverity::Warning,
                ProgramWarningCode::CoordinateSystemIgnored,
                format!(
                    "G{value:.0} is recorded but work offsets are not applied to local preview"
                ),
            );
        } else if code_is(value, 4.0) {
            *dwell = true;
        } else if code_is(value, 10.0)
            || code_is(value, 28.0)
            || code_is(value, 30.0)
            || code_is(value, 53.0)
            || code_is(value, 92.0)
            || (38.0..39.0).contains(&value)
        {
            *skip_motion = true;
            if (38.0..39.0).contains(&value) {
                self.features.has_probe_cycle = true;
            }
            if code_is(value, 53.0) {
                self.features.has_machine_coordinate_move = true;
            }
            self.warn(
                source_line,
                ProgramWarningSeverity::Safety,
                ProgramWarningCode::UnsafeMachineCommand,
                format!("G{value} is excluded from preview and future dry run"),
            );
        } else {
            *skip_motion = true;
            self.warn(
                source_line,
                ProgramWarningSeverity::Error,
                ProgramWarningCode::UnsupportedGCode,
                format!("G{value} is not supported by the preview parser"),
            );
        }
    }

    fn apply_m_code(&mut self, source_line: usize, value: f64) {
        if code_is(value, 3.0) || code_is(value, 4.0) {
            self.spindle_mode = if code_is(value, 3.0) {
                ProgramSpindleMode::Clockwise
            } else {
                ProgramSpindleMode::Counterclockwise
            };
            self.features.has_spindle_activation = true;
            self.warn(
                source_line,
                ProgramWarningSeverity::Safety,
                ProgramWarningCode::SpindleActivation,
                format!("M{value:.0} spindle activation will be blocked by dry run"),
            );
        } else if code_is(value, 6.0) {
            self.features.has_tool_change = true;
            self.warn(
                source_line,
                ProgramWarningSeverity::Safety,
                ProgramWarningCode::ToolChange,
                "M6 requires a host-managed operator tool-change barrier",
            );
        } else if code_is(value, 7.0) || code_is(value, 8.0) {
            self.warn(
                source_line,
                ProgramWarningSeverity::Safety,
                ProgramWarningCode::UnsafeMachineCommand,
                format!("M{value:.0} coolant activation will be blocked by dry run"),
            );
        } else if code_is(value, 5.0)
            || code_is(value, 9.0)
            || code_is(value, 0.0)
            || code_is(value, 1.0)
            || code_is(value, 2.0)
            || code_is(value, 30.0)
        {
            if code_is(value, 5.0) {
                self.spindle_mode = ProgramSpindleMode::Off;
            }
            // Safe for geometry parsing; sender semantics are intentionally absent.
        } else {
            self.warn(
                source_line,
                ProgramWarningSeverity::Safety,
                ProgramWarningCode::UnsupportedMCode,
                format!("M{value:.0} is unknown and will be blocked by dry run"),
            );
        }
    }

    fn warn(
        &mut self,
        source_line: usize,
        severity: ProgramWarningSeverity,
        code: ProgramWarningCode,
        message: impl Into<String>,
    ) {
        self.warnings.push(ProgramWarning {
            source_line,
            severity,
            code,
            message: message.into(),
        });
    }
}

fn strip_comments(source: &str, source_line: usize, warnings: &mut Vec<ProgramWarning>) -> String {
    let mut output = String::with_capacity(source.len());
    let mut in_comment = false;
    for character in source.chars() {
        match character {
            ';' if !in_comment => break,
            '(' if !in_comment => in_comment = true,
            ')' if in_comment => in_comment = false,
            ')' => warnings.push(ProgramWarning {
                source_line,
                severity: ProgramWarningSeverity::Warning,
                code: ProgramWarningCode::UnexpectedCommentClose,
                message: "unexpected ')' outside a comment".to_owned(),
            }),
            _ if !in_comment => output.push(character),
            _ => {}
        }
    }
    if in_comment {
        warnings.push(ProgramWarning {
            source_line,
            severity: ProgramWarningSeverity::Error,
            code: ProgramWarningCode::UnclosedComment,
            message: "parenthesized comment is not closed".to_owned(),
        });
    }
    output
}

fn validate_checksum(
    source: &str,
    source_line: usize,
    warnings: &mut Vec<ProgramWarning>,
) -> (String, Option<u8>) {
    let mut in_comment = false;
    let mut separator = None;
    for (index, character) in source.char_indices() {
        match character {
            ';' if !in_comment => break,
            '(' if !in_comment => in_comment = true,
            ')' if in_comment => in_comment = false,
            '*' if !in_comment => {
                if separator.is_some() {
                    warnings.push(ProgramWarning {
                        source_line,
                        severity: ProgramWarningSeverity::Error,
                        code: ProgramWarningCode::ChecksumInvalid,
                        message: "a checksummed block contains more than one '*' separator"
                            .to_owned(),
                    });
                    return (source[..separator.unwrap_or(index)].to_owned(), None);
                }
                separator = Some(index);
            }
            _ => {}
        }
    }
    let Some(separator) = separator else {
        return (source.to_owned(), None);
    };

    let payload = &source[..separator];
    let supplied = source[separator + 1..].trim();
    let parsed = supplied.parse::<u16>().ok().filter(|value| *value <= 255);
    let Some(supplied) = parsed.map(|value| value as u8) else {
        warnings.push(ProgramWarning {
            source_line,
            severity: ProgramWarningSeverity::Error,
            code: ProgramWarningCode::ChecksumInvalid,
            message: "checksum must be a final decimal byte from 0 to 255".to_owned(),
        });
        return (payload.to_owned(), None);
    };
    let computed = payload
        .as_bytes()
        .iter()
        .fold(0u8, |checksum, byte| checksum ^ byte);
    if computed == supplied {
        warnings.push(ProgramWarning {
            source_line,
            severity: ProgramWarningSeverity::Warning,
            code: ProgramWarningCode::ChecksumValidated,
            message: format!("checksum {supplied} validated before normalization"),
        });
    } else {
        warnings.push(ProgramWarning {
            source_line,
            severity: ProgramWarningSeverity::Error,
            code: ProgramWarningCode::ChecksumInvalid,
            message: format!("checksum mismatch: source declares {supplied}, computed {computed}"),
        });
    }
    (payload.to_owned(), Some(supplied))
}

fn strip_optional_block(
    code: &str,
    source_line: usize,
    warnings: &mut Vec<ProgramWarning>,
) -> (String, bool) {
    let Some(non_whitespace) = code.find(|character: char| !character.is_whitespace()) else {
        return (code.to_owned(), false);
    };
    if code[non_whitespace..].starts_with('/') {
        warnings.push(ProgramWarning {
            source_line,
            severity: ProgramWarningSeverity::Warning,
            code: ProgramWarningCode::OptionalBlock,
            message: "optional block is preserved for the Block Delete run option".to_owned(),
        });
        let mut normalized = code.to_owned();
        normalized.remove(non_whitespace);
        (normalized, true)
    } else {
        (code.to_owned(), false)
    }
}

fn tokenize(code: &str, source_line: usize, warnings: &mut Vec<ProgramWarning>) -> Vec<Word> {
    let characters = code.chars().collect::<Vec<_>>();
    let mut words = Vec::new();
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if character.is_whitespace() || character == '%' {
            index += 1;
            continue;
        }
        if character == '/' {
            warnings.push(ProgramWarning {
                source_line,
                severity: ProgramWarningSeverity::Error,
                code: ProgramWarningCode::OptionalBlockUnsupported,
                message: "'/' is valid only as the first non-whitespace block character".to_owned(),
            });
            index += 1;
            continue;
        }
        if character == '*' {
            warnings.push(ProgramWarning {
                source_line,
                severity: ProgramWarningSeverity::Error,
                code: ProgramWarningCode::ChecksumUnsupported,
                message: "'*' is valid only as one final checksum separator".to_owned(),
            });
            break;
        }
        if !character.is_ascii_alphabetic() {
            warnings.push(ProgramWarning {
                source_line,
                severity: ProgramWarningSeverity::Error,
                code: ProgramWarningCode::InvalidToken,
                message: format!("unexpected token '{character}'"),
            });
            index += 1;
            continue;
        }

        let letter = character.to_ascii_uppercase();
        index += 1;
        while index < characters.len() && characters[index].is_whitespace() {
            index += 1;
        }
        let number_start = index;
        if index < characters.len() && matches!(characters[index], '+' | '-') {
            index += 1;
        }
        let mut digits = 0;
        let mut decimal_points = 0;
        while index < characters.len() {
            if characters[index].is_ascii_digit() {
                digits += 1;
                index += 1;
            } else if characters[index] == '.' && decimal_points == 0 {
                decimal_points += 1;
                index += 1;
            } else {
                break;
            }
        }
        if digits == 0 {
            warnings.push(ProgramWarning {
                source_line,
                severity: ProgramWarningSeverity::Error,
                code: ProgramWarningCode::InvalidToken,
                message: format!("{letter} word has no numeric value"),
            });
            continue;
        }
        let number = characters[number_start..index].iter().collect::<String>();
        match number.parse::<f64>() {
            Ok(value) if value.is_finite() => words.push(Word {
                letter,
                value,
                lexeme: format!("{letter}{number}"),
            }),
            _ => warnings.push(ProgramWarning {
                source_line,
                severity: ProgramWarningSeverity::Error,
                code: ProgramWarningCode::InvalidToken,
                message: format!("{letter}{number} is not a finite number"),
            }),
        }
    }
    words
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn resolve_axis(current: f64, value: Option<f64>, distance: DistanceMode) -> f64 {
    match (value, distance) {
        (Some(value), DistanceMode::Absolute) => value,
        (Some(value), DistanceMode::Incremental) => current + value,
        (None, _) => current,
    }
}

fn code_is(value: f64, expected: f64) -> bool {
    (value - expected).abs() < 1e-6
}

fn g_modal_group(value: f64) -> Option<u8> {
    if [0.0, 1.0, 2.0, 3.0, 80.0]
        .into_iter()
        .any(|code| code_is(value, code))
        || (38.0..39.0).contains(&value)
    {
        Some(1)
    } else if [17.0, 18.0, 19.0]
        .into_iter()
        .any(|code| code_is(value, code))
    {
        Some(2)
    } else if code_is(value, 90.0) || code_is(value, 91.0) {
        Some(3)
    } else if code_is(value, 90.1) || code_is(value, 91.1) {
        Some(4)
    } else if [93.0, 94.0, 95.0]
        .into_iter()
        .any(|code| code_is(value, code))
    {
        Some(5)
    } else if code_is(value, 20.0) || code_is(value, 21.0) {
        Some(6)
    } else if [4.0, 10.0, 28.0, 30.0, 53.0, 92.0]
        .into_iter()
        .any(|code| code_is(value, code))
    {
        Some(0)
    } else if code_is(value, 40.0) {
        Some(7)
    } else if code_is(value, 49.0) {
        Some(8)
    } else if (54.0..=59.0).contains(&value) && value.fract().abs() < f64::EPSILON {
        Some(12)
    } else if code_is(value, 61.0) {
        Some(13)
    } else {
        None
    }
}

fn m_modal_group(value: f64) -> Option<u8> {
    if [0.0, 1.0, 2.0, 30.0]
        .into_iter()
        .any(|code| code_is(value, code))
    {
        Some(4)
    } else if [3.0, 4.0, 5.0].into_iter().any(|code| code_is(value, code)) {
        Some(7)
    } else if [7.0, 8.0, 9.0].into_iter().any(|code| code_is(value, code)) {
        Some(8)
    } else {
        None
    }
}

fn same_point(left: ProgramPoint, right: ProgramPoint) -> bool {
    (left.x - right.x).abs() <= POSITION_EPSILON_MM
        && (left.y - right.y).abs() <= POSITION_EPSILON_MM
        && (left.z - right.z).abs() <= POSITION_EPSILON_MM
}

fn plane_offsets(
    plane: Plane,
    i: Option<f64>,
    j: Option<f64>,
    k: Option<f64>,
) -> (Option<f64>, Option<f64>, bool) {
    match plane {
        Plane::Xy => (i, j, k.is_some()),
        Plane::Xz => (k, i, j.is_some()),
        Plane::Yz => (j, k, i.is_some()),
    }
}

fn plane_components(point: ProgramPoint, plane: Plane) -> (f64, f64, f64) {
    match plane {
        Plane::Xy => (point.x, point.y, point.z),
        Plane::Xz => (point.z, point.x, point.y),
        Plane::Yz => (point.y, point.z, point.x),
    }
}

fn point_from_plane(u: f64, v: f64, linear: f64, plane: Plane) -> ProgramPoint {
    match plane {
        Plane::Xy => ProgramPoint {
            x: u,
            y: v,
            z: linear,
        },
        Plane::Xz => ProgramPoint {
            x: v,
            y: linear,
            z: u,
        },
        Plane::Yz => ProgramPoint {
            x: linear,
            y: u,
            z: v,
        },
    }
}

struct ArcDefinition {
    plane: Plane,
    offset_u: Option<f64>,
    offset_v: Option<f64>,
    radius: Option<f64>,
    clockwise: bool,
    distance_mode: ArcDistanceMode,
}

fn sample_arc(
    start: ProgramPoint,
    end: ProgramPoint,
    definition: ArcDefinition,
) -> Option<Vec<ProgramPoint>> {
    let (start_u, start_v, start_linear) = plane_components(start, definition.plane);
    let (end_u, end_v, end_linear) = plane_components(end, definition.plane);
    let (center_u, center_v) = if definition.offset_u.is_some() || definition.offset_v.is_some() {
        match definition.distance_mode {
            ArcDistanceMode::Incremental => (
                start_u + definition.offset_u.unwrap_or(0.0),
                start_v + definition.offset_v.unwrap_or(0.0),
            ),
            ArcDistanceMode::Absolute => (definition.offset_u?, definition.offset_v?),
        }
    } else {
        center_from_radius(
            start_u,
            start_v,
            end_u,
            end_v,
            definition.radius?,
            definition.clockwise,
        )?
    };
    let arc_radius = (start_u - center_u).hypot(start_v - center_v);
    if arc_radius <= POSITION_EPSILON_MM {
        return None;
    }
    let end_radius = (end_u - center_u).hypot(end_v - center_v);
    if (arc_radius - end_radius).abs() > arc_radius.max(1.0) * 0.002 {
        return None;
    }

    let start_angle = (start_v - center_v).atan2(start_u - center_u);
    let end_angle = (end_v - center_v).atan2(end_u - center_u);
    let full_circle = (start_u - end_u).abs() <= POSITION_EPSILON_MM
        && (start_v - end_v).abs() <= POSITION_EPSILON_MM;
    let sweep = directed_sweep(start_angle, end_angle, definition.clockwise, full_circle);
    if sweep <= POSITION_EPSILON_MM {
        return None;
    }
    let angle_steps = (sweep / ARC_MAX_ANGLE_RAD).ceil() as usize;
    let chord_steps = (arc_radius * sweep / ARC_MAX_CHORD_MM).ceil() as usize;
    let steps = angle_steps.max(chord_steps).clamp(2, MAX_PREVIEW_POINTS);
    let mut points = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        let progress = step as f64 / steps as f64;
        let angle = if definition.clockwise {
            start_angle - sweep * progress
        } else {
            start_angle + sweep * progress
        };
        points.push(point_from_plane(
            center_u + arc_radius * angle.cos(),
            center_v + arc_radius * angle.sin(),
            start_linear + (end_linear - start_linear) * progress,
            definition.plane,
        ));
    }
    if let Some(first) = points.first_mut() {
        *first = start;
    }
    if let Some(last) = points.last_mut() {
        *last = end;
    }
    Some(points)
}

fn center_from_radius(
    start_u: f64,
    start_v: f64,
    end_u: f64,
    end_v: f64,
    signed_radius: f64,
    clockwise: bool,
) -> Option<(f64, f64)> {
    let dx = end_u - start_u;
    let dy = end_v - start_v;
    let chord = dx.hypot(dy);
    let radius = signed_radius.abs();
    if chord <= POSITION_EPSILON_MM || radius + POSITION_EPSILON_MM < chord / 2.0 {
        return None;
    }
    let midpoint = ((start_u + end_u) / 2.0, (start_v + end_v) / 2.0);
    let height = (radius * radius - chord * chord / 4.0).max(0.0).sqrt();
    let perpendicular = (-dy / chord, dx / chord);
    let candidates = [
        (
            midpoint.0 + perpendicular.0 * height,
            midpoint.1 + perpendicular.1 * height,
        ),
        (
            midpoint.0 - perpendicular.0 * height,
            midpoint.1 - perpendicular.1 * height,
        ),
    ];
    let wants_major = signed_radius < 0.0;
    candidates.into_iter().min_by(|left, right| {
        let score = |center: &(f64, f64)| {
            let start_angle = (start_v - center.1).atan2(start_u - center.0);
            let end_angle = (end_v - center.1).atan2(end_u - center.0);
            let sweep = directed_sweep(start_angle, end_angle, clockwise, false);
            let is_major = sweep > PI + 1e-6;
            if is_major == wants_major { 0 } else { 1 }
        };
        score(left).cmp(&score(right))
    })
}

fn directed_sweep(start_angle: f64, end_angle: f64, clockwise: bool, full: bool) -> f64 {
    if full {
        TAU
    } else if clockwise {
        (start_angle - end_angle).rem_euclid(TAU)
    } else {
        (end_angle - start_angle).rem_euclid(TAU)
    }
}

fn polyline_distance(points: &[ProgramPoint]) -> f64 {
    points
        .windows(2)
        .map(|pair| {
            let dx = pair[1].x - pair[0].x;
            let dy = pair[1].y - pair[0].y;
            let dz = pair[1].z - pair[0].z;
            (dx * dx + dy * dy + dz * dz).sqrt()
        })
        .sum()
}
