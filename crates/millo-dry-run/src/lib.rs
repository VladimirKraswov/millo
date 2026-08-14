use std::collections::{BTreeMap, BTreeSet};

use millo_gcode::{
    GcodeProgram, ProgramBounds, ProgramDistanceMode, ProgramFeedMode, ProgramPlane, ProgramPoint,
    ProgramUnitMode, ProgramWarningCode, ProgramWarningSeverity, ToolpathKind, ToolpathSegment,
};
use millo_heightmap::Heightmap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_DRY_RUN_COMMAND_BYTES: usize = 255;
pub const MAX_COMPENSATED_PROGRAM_LINES: usize = 200_000;
pub const MAX_CUTTING_DEPTH_ADJUSTMENT_UM: i32 = 10_000;
const SAFETY_PREAMBLE: [&str; 2] = ["M5", "M9"];
const SAFETY_EPILOGUE: [&str; 2] = ["M5", "M9"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProgramRunPolicy {
    #[default]
    AirRun,
    Cutting,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramExecutionOptions {
    pub optional_stop: bool,
    pub block_delete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_map_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cutting_depth_adjustment_um: Option<i32>,
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
    HeightmapCompensation,
    CuttingDepthAdjustment,
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
    SafetyEpilogue,
    Program,
    ProgramPause,
    OptionalPause,
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

    pub fn wire_command(&self) -> String {
        match self.source_line {
            Some(source_line) => numbered_command(source_line, &self.command),
            None => self.command.clone(),
        }
    }

    pub fn wire_command_len(&self) -> usize {
        match self.source_line {
            Some(source_line) => numbered_command_len(source_line, &self.command),
            None => self.command.len(),
        }
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
    execution_options: ProgramExecutionOptions,
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

    pub fn execution_options(&self) -> ProgramExecutionOptions {
        self.execution_options
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
    build_program_run_plan_with_options(program, policy, ProgramExecutionOptions::default())
}

pub fn build_program_run_plan_with_options(
    program: &GcodeProgram,
    policy: ProgramRunPolicy,
    execution_options: ProgramExecutionOptions,
) -> Result<DryRunPlan, DryRunPolicyError> {
    let plan = build_untransformed_program_run_plan(program, policy, execution_options)?;
    let depth_adjustment_mm = validate_cutting_depth_adjustment(program, execution_options)?;
    if depth_adjustment_mm.is_none_or(|adjustment| adjustment.abs() < f64::EPSILON) {
        Ok(plan)
    } else {
        transform_plan(program, plan, None, depth_adjustment_mm)
    }
}

fn build_untransformed_program_run_plan(
    program: &GcodeProgram,
    policy: ProgramRunPolicy,
    execution_options: ProgramExecutionOptions,
) -> Result<DryRunPlan, DryRunPolicyError> {
    let mut blockers = Vec::new();
    let mut seen = BTreeSet::new();

    if program.block_delete_enabled != execution_options.block_delete {
        add_blocker(
            &mut blockers,
            &mut seen,
            None,
            DryRunBlockerKind::UnsupportedProgram,
            "program parse and sender disagree about Block Delete",
        );
    }

    if !program.summary.preview_complete {
        add_blocker(
            &mut blockers,
            &mut seen,
            None,
            DryRunBlockerKind::IncompletePreview,
            "preview is incomplete; execution is fail-closed",
        );
    }

    for line in program
        .lines
        .iter()
        .filter(|line| line.executable && !line.block_deleted)
    {
        inspect_normalized_line(
            line.source_line,
            &line.normalized,
            policy,
            &mut blockers,
            &mut seen,
        );
        if numbered_command_len(line.source_line, &line.normalized) > MAX_DRY_RUN_COMMAND_BYTES {
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
        .filter(|line| line.executable && !line.block_deleted && !line.normalized.is_empty())
    {
        let kind = classify_line(&line.normalized);
        if kind == DryRunLineKind::OptionalPause && !execution_options.optional_stop {
            continue;
        }
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

    let program_end = program_lines
        .last()
        .is_some_and(|line| line.kind == DryRunLineKind::ProgramEnd)
        .then(|| program_lines.pop())
        .flatten();
    let mut lines = Vec::with_capacity(
        SAFETY_PREAMBLE.len()
            + program_lines.len()
            + SAFETY_EPILOGUE.len()
            + usize::from(program_end.is_some()),
    );
    lines.extend(SAFETY_PREAMBLE.map(|command| DryRunLine {
        source_line: None,
        command: command.to_owned(),
        kind: DryRunLineKind::SafetyPreamble,
        tool_number: None,
        estimated_duration_ms: Some(0),
    }));
    lines.extend(program_lines);
    lines.extend(SAFETY_EPILOGUE.map(|command| DryRunLine {
        source_line: None,
        command: command.to_owned(),
        kind: DryRunLineKind::SafetyEpilogue,
        tool_number: None,
        estimated_duration_ms: Some(0),
    }));
    lines.extend(program_end);

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
        execution_options,
    })
}

pub fn build_program_run_plan_with_heightmap(
    program: &GcodeProgram,
    policy: ProgramRunPolicy,
    execution_options: ProgramExecutionOptions,
    heightmap: Option<&Heightmap>,
) -> Result<DryRunPlan, DryRunPolicyError> {
    let plan = build_untransformed_program_run_plan(program, policy, execution_options)?;
    let depth_adjustment_mm = validate_cutting_depth_adjustment(program, execution_options)?;
    let heightmap = match (execution_options.surface_map_id, heightmap) {
        (None, None) => None,
        (Some(_), Some(heightmap)) => Some(heightmap),
        (Some(_), None) => Err(heightmap_rejection(
            None,
            "selected heightmap is unavailable",
        ))?,
        (None, Some(_)) => Err(heightmap_rejection(
            None,
            "heightmap data was supplied without an explicit map selection",
        ))?,
    };
    if heightmap.is_none()
        && depth_adjustment_mm.is_none_or(|adjustment| adjustment.abs() < f64::EPSILON)
    {
        Ok(plan)
    } else {
        transform_plan(program, plan, heightmap, depth_adjustment_mm)
    }
}

fn transform_plan(
    program: &GcodeProgram,
    plan: DryRunPlan,
    heightmap: Option<&Heightmap>,
    depth_adjustment_mm: Option<f64>,
) -> Result<DryRunPlan, DryRunPolicyError> {
    validate_transformation_contract(program, heightmap.is_some(), depth_adjustment_mm.is_some())?;
    if let Some(heightmap) = heightmap {
        validate_heightmap_contract(heightmap)?;
    }
    let mut segments = BTreeMap::<usize, Vec<&ToolpathSegment>>::new();
    for segment in &program.toolpath {
        segments
            .entry(segment.source_line)
            .or_default()
            .push(segment);
    }

    let clearance_z = heightmap
        .map(|map| map.plan.request.clearance_z_mm.max(0.001))
        .unwrap_or(1.0);
    let max_step_mm = heightmap
        .map(|map| (map.plan.spacing.x_mm.min(map.plan.spacing.y_mm) / 2.0).clamp(0.25, 1.0));
    let transform = TrajectoryTransform {
        heightmap,
        depth_adjustment_mm,
        clearance_z,
        max_step_mm,
    };
    let mut transformed = Vec::with_capacity(plan.lines.len());
    let mut known_x = false;
    let mut known_y = false;

    for line in plan.lines {
        let Some(source_line) = line.source_line else {
            transformed.push(line);
            continue;
        };
        let Some(source) = program
            .lines
            .iter()
            .find(|item| item.source_line == source_line)
        else {
            transformed.push(line);
            continue;
        };
        let axes = explicit_axes(&source.normalized);
        let line_segments = segments.get(&source_line).cloned().unwrap_or_default();
        if line_segments.is_empty() {
            transformed.push(line);
            known_x |= axes.0;
            known_y |= axes.1;
            continue;
        }

        let safe_z_only_before_xy = heightmap.is_some()
            && !known_x
            && !known_y
            && !axes.0
            && !axes.1
            && line_segments.iter().all(|segment| {
                segment.kind == ToolpathKind::Rapid
                    && segment
                        .points
                        .last()
                        .is_some_and(|point| point.z >= clearance_z)
            });
        if safe_z_only_before_xy {
            transformed.push(line);
            continue;
        }

        let needs_surface = heightmap.is_some()
            && line_segments.iter().any(|segment| {
                segment
                    .points
                    .iter()
                    .any(|point| compensation_weight(point.z, clearance_z) > 0.0)
            });
        if needs_surface && (!known_x || !known_y) {
            return Err(heightmap_rejection(
                Some(source_line),
                "heightmap compensation requires explicit safe-Z positioning of X and Y before the first surface-level move",
            ));
        }
        let establishes_x = known_x || axes.0;
        let establishes_y = known_y || axes.1;

        if let Some(prefix) = non_motion_prefix(&source.normalized) {
            push_compensated_line(
                &mut transformed,
                source_line,
                prefix,
                DryRunLineKind::Program,
                Some(0),
            )?;
        }
        for segment in line_segments {
            append_compensated_segment(
                &mut transformed,
                segment,
                transform,
                establishes_x,
                establishes_y,
            )?;
        }
        known_x = establishes_x;
        known_y = establishes_y;
    }

    let estimated_total_ms = transformed
        .iter()
        .filter_map(DryRunLine::estimated_duration_ms)
        .fold(0u64, u64::saturating_add);
    let time_estimate_complete = transformed
        .iter()
        .all(|line| line.estimated_duration_ms().is_some());
    Ok(DryRunPlan {
        source_name: plan.source_name,
        source_line_count: plan.source_line_count,
        lines: transformed,
        estimated_total_ms,
        time_estimate_complete,
        execution_options: plan.execution_options,
    })
}

fn validate_transformation_contract(
    program: &GcodeProgram,
    heightmap_enabled: bool,
    depth_adjustment_enabled: bool,
) -> Result<(), DryRunPolicyError> {
    for checkpoint in &program.execution_checkpoints {
        let supported = checkpoint.units == ProgramUnitMode::Millimeters
            && checkpoint.distance == ProgramDistanceMode::Absolute
            && checkpoint.feed_mode == ProgramFeedMode::UnitsPerMinute
            && (!heightmap_enabled || checkpoint.plane == ProgramPlane::Xy);
        if !supported {
            let kind = if heightmap_enabled {
                DryRunBlockerKind::HeightmapCompensation
            } else {
                DryRunBlockerKind::CuttingDepthAdjustment
            };
            let requirement = if heightmap_enabled {
                "G21, G90, G94 and G17"
            } else {
                "G21, G90 and G94"
            };
            return Err(transformation_rejection(
                Some(checkpoint.source_line),
                kind,
                &format!("trajectory transformation requires {requirement} for every motion block"),
            ));
        }
    }
    debug_assert!(heightmap_enabled || depth_adjustment_enabled);
    Ok(())
}

fn validate_heightmap_contract(heightmap: &Heightmap) -> Result<(), DryRunPolicyError> {
    if heightmap.coordinate_binding.is_none() {
        return Err(heightmap_rejection(
            None,
            "heightmap has no work-coordinate binding",
        ));
    }
    let progress = heightmap.progress();
    if !progress.complete || progress.triggered != progress.total {
        return Err(heightmap_rejection(
            None,
            "heightmap is incomplete or contains a probe miss",
        ));
    }
    Ok(())
}

fn validate_cutting_depth_adjustment(
    program: &GcodeProgram,
    execution_options: ProgramExecutionOptions,
) -> Result<Option<f64>, DryRunPolicyError> {
    let Some(adjustment_um) = execution_options.cutting_depth_adjustment_um else {
        return Ok(None);
    };
    if adjustment_um.unsigned_abs() > MAX_CUTTING_DEPTH_ADJUSTMENT_UM as u32 {
        return Err(depth_adjustment_rejection(
            None,
            "cutting depth adjustment exceeds the 10 mm safety limit",
        ));
    }
    if deepest_cutting_z(program).is_none() {
        return Err(depth_adjustment_rejection(
            None,
            "cutting depth adjustment requires a cutting move below work Z0",
        ));
    }
    let adjustment_mm = f64::from(adjustment_um) / 1_000.0;
    Ok(Some(adjustment_mm))
}

pub fn deepest_cutting_z(program: &GcodeProgram) -> Option<f64> {
    program
        .toolpath
        .iter()
        .filter(|segment| segment.kind != ToolpathKind::Rapid)
        .flat_map(|segment| segment.points.iter().map(|point| point.z))
        .filter(|z| *z < -1e-9)
        .reduce(f64::min)
}

pub fn program_bounds_with_execution_options(
    program: &GcodeProgram,
    execution_options: ProgramExecutionOptions,
) -> Option<ProgramBounds> {
    let original = program.summary.bounds?;
    let adjustment_mm = execution_options
        .cutting_depth_adjustment_um
        .map(|value| f64::from(value) / 1_000.0)
        .unwrap_or(0.0);
    if adjustment_mm.abs() < f64::EPSILON {
        return Some(original);
    }
    let (min_z, max_z) = program
        .toolpath
        .iter()
        .flat_map(|segment| {
            segment.points.iter().map(move |point| ProgramPoint {
                z: adjusted_cutting_z(point.z, segment.kind, adjustment_mm),
                ..*point
            })
        })
        .map(|point| point.z)
        .fold(None, |bounds, z| {
            Some(match bounds {
                Some((min_z, max_z)) => (f64::min(min_z, z), f64::max(max_z, z)),
                None => (z, z),
            })
        })?;
    let min = ProgramPoint {
        z: min_z,
        ..original.min
    };
    let max = ProgramPoint {
        z: max_z,
        ..original.max
    };
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

#[derive(Clone, Copy)]
struct TrajectoryTransform<'a> {
    heightmap: Option<&'a Heightmap>,
    depth_adjustment_mm: Option<f64>,
    clearance_z: f64,
    max_step_mm: Option<f64>,
}

fn append_compensated_segment(
    output: &mut Vec<DryRunLine>,
    segment: &ToolpathSegment,
    transform: TrajectoryTransform<'_>,
    known_x: bool,
    known_y: bool,
) -> Result<(), DryRunPolicyError> {
    let TrajectoryTransform {
        heightmap,
        depth_adjustment_mm,
        clearance_z,
        max_step_mm,
    } = transform;
    let rapid = segment.kind == ToolpathKind::Rapid;
    for pair in segment.points.windows(2) {
        let start = pair[0];
        let end = pair[1];
        let compensation_needed = heightmap.is_some()
            && (compensation_weight(start.z, clearance_z) > 0.0
                || compensation_weight(end.z, clearance_z) > 0.0);
        let xy_distance = (end.x - start.x).hypot(end.y - start.y);
        let z_distance = (end.z - start.z).abs();
        let parts = if compensation_needed {
            ((xy_distance.max(z_distance) / max_step_mm.unwrap_or(1.0)).ceil() as usize).max(1)
        } else {
            1
        };
        for part in 1..=parts {
            if output.len() >= MAX_COMPENSATED_PROGRAM_LINES {
                return Err(heightmap_rejection(
                    Some(segment.source_line),
                    "heightmap segmentation exceeds the 200000-line sender limit",
                ));
            }
            let mix = part as f64 / parts as f64;
            let point = lerp_point(start, end, mix);
            let nominal_z =
                adjusted_cutting_z(point.z, segment.kind, depth_adjustment_mm.unwrap_or(0.0));
            let weight = heightmap
                .map(|_| compensation_weight(nominal_z, clearance_z))
                .unwrap_or(0.0);
            let surface_z = if let Some(heightmap) = heightmap.filter(|_| weight > 0.0) {
                heightmap
                    .interpolate_delta_z(point.x, point.y)
                    .map_err(|_| {
                        heightmap_rejection(
                            Some(segment.source_line),
                            "program motion leaves the measured heightmap perimeter",
                        )
                    })?
            } else {
                0.0
            };
            let corrected_z = nominal_z + surface_z * weight;
            let mut command = format!("G90 G21 G94 {}", if rapid { "G0" } else { "G1" });
            if known_x {
                command.push_str(&format!(" X{:.4}", point.x));
            }
            if known_y {
                command.push_str(&format!(" Y{:.4}", point.y));
            }
            command.push_str(&format!(" Z{corrected_z:.4}"));
            if !rapid && let Some(feed) = segment.feed_rate_mm_per_min {
                command.push_str(&format!(" F{feed:.3}"));
            }
            let duration = if rapid {
                None
            } else {
                segment.feed_rate_mm_per_min.map(|feed| {
                    let previous_mix = (part - 1) as f64 / parts as f64;
                    let previous = lerp_point(start, end, previous_mix);
                    let previous_nominal_z = adjusted_cutting_z(
                        previous.z,
                        segment.kind,
                        depth_adjustment_mm.unwrap_or(0.0),
                    );
                    let previous_weight = heightmap
                        .map(|_| compensation_weight(previous_nominal_z, clearance_z))
                        .unwrap_or(0.0);
                    let previous_surface =
                        if let Some(heightmap) = heightmap.filter(|_| previous_weight > 0.0) {
                            heightmap
                                .interpolate_delta_z(previous.x, previous.y)
                                .unwrap_or(0.0)
                        } else {
                            0.0
                        };
                    let previous_z = previous_nominal_z + previous_surface * previous_weight;
                    let distance = (point.x - previous.x)
                        .hypot(point.y - previous.y)
                        .hypot(corrected_z - previous_z);
                    seconds_to_millis(distance / feed * 60.0)
                })
            };
            push_compensated_line(
                output,
                segment.source_line,
                command,
                DryRunLineKind::Program,
                duration,
            )?;
        }
    }
    Ok(())
}

fn adjusted_cutting_z(z: f64, kind: ToolpathKind, adjustment_mm: f64) -> f64 {
    if kind != ToolpathKind::Rapid && z < -1e-9 {
        z + adjustment_mm
    } else {
        z
    }
}

fn push_compensated_line(
    output: &mut Vec<DryRunLine>,
    source_line: usize,
    command: String,
    kind: DryRunLineKind,
    estimated_duration_ms: Option<u64>,
) -> Result<(), DryRunPolicyError> {
    if numbered_command_len(source_line, &command) > MAX_DRY_RUN_COMMAND_BYTES {
        return Err(heightmap_rejection(
            Some(source_line),
            "heightmap command exceeds the sender line-length limit",
        ));
    }
    output.push(DryRunLine {
        source_line: Some(source_line),
        command,
        kind,
        tool_number: None,
        estimated_duration_ms,
    });
    Ok(())
}

fn heightmap_rejection(source_line: Option<usize>, message: &str) -> DryRunPolicyError {
    transformation_rejection(
        source_line,
        DryRunBlockerKind::HeightmapCompensation,
        message,
    )
}

fn depth_adjustment_rejection(source_line: Option<usize>, message: &str) -> DryRunPolicyError {
    transformation_rejection(
        source_line,
        DryRunBlockerKind::CuttingDepthAdjustment,
        message,
    )
}

fn transformation_rejection(
    source_line: Option<usize>,
    kind: DryRunBlockerKind,
    message: &str,
) -> DryRunPolicyError {
    DryRunPolicyError::Rejected(
        1,
        vec![DryRunBlocker {
            source_line,
            kind,
            message: message.to_owned(),
        }],
    )
}

fn explicit_axes(normalized: &str) -> (bool, bool) {
    let mut x = false;
    let mut y = false;
    for word in normalized.split_whitespace().filter_map(split_word) {
        x |= word.0 == 'X';
        y |= word.0 == 'Y';
    }
    (x, y)
}

fn non_motion_prefix(normalized: &str) -> Option<String> {
    let words = normalized
        .split_whitespace()
        .filter(|word| {
            let Some((letter, value)) = split_word(word) else {
                return true;
            };
            !(matches!(letter, 'N' | 'X' | 'Y' | 'Z' | 'I' | 'J' | 'K' | 'R' | 'F')
                || letter == 'G'
                    && [0.0, 1.0, 2.0, 3.0]
                        .iter()
                        .any(|code| code_is(value, *code)))
        })
        .collect::<Vec<_>>()
        .join(" ");
    (!words.is_empty()).then_some(words)
}

fn compensation_weight(nominal_z: f64, clearance_z: f64) -> f64 {
    if nominal_z <= 0.0 {
        1.0
    } else if nominal_z >= clearance_z {
        0.0
    } else {
        1.0 - nominal_z / clearance_z
    }
}

fn lerp_point(start: ProgramPoint, end: ProgramPoint, mix: f64) -> ProgramPoint {
    ProgramPoint {
        x: start.x + (end.x - start.x) * mix,
        y: start.y + (end.y - start.y) * mix,
        z: start.z + (end.z - start.z) * mix,
    }
}

fn numbered_command(source_line: usize, normalized: &str) -> String {
    let body = normalized
        .split_whitespace()
        .filter(|word| !matches!(split_word(word), Some(('N', _))))
        .collect::<Vec<_>>()
        .join(" ");
    format!("N{source_line} {body}")
}

fn numbered_command_len(source_line: usize, normalized: &str) -> usize {
    numbered_command(source_line, normalized).len()
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
    for pause_code in [0.0, 1.0] {
        if !has_code(normalized, 'M', pause_code) {
            continue;
        }
        for word in normalized.split_whitespace() {
            let Some((letter, value)) = split_word(word) else {
                continue;
            };
            let allowed = letter == 'N' || (letter == 'M' && code_is(value, pause_code));
            if !allowed {
                add_blocker(
                    blockers,
                    seen,
                    Some(source_line),
                    DryRunBlockerKind::UnsupportedProgram,
                    format!("M{pause_code:.0} must be isolated so its host pause is unambiguous"),
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
    let mut pause = None;
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
            pause = Some(if code_is(value, 0.0) {
                DryRunLineKind::ProgramPause
            } else {
                DryRunLineKind::OptionalPause
            });
        }
    }
    pause.unwrap_or(DryRunLineKind::Program)
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
    use millo_domain::{Position, WorkCoordinateSystem};
    use millo_gcode::{
        ProgramParseOptions, ProgramParseRequest, parse_program, parse_program_with_options,
    };
    use millo_heightmap::{HeightmapPlanRequest, plan_heightmap};

    use super::*;

    fn parse(source: &str) -> GcodeProgram {
        parse_program(ProgramParseRequest {
            source_name: "fixture.nc".to_owned(),
            source: source.to_owned(),
        })
        .unwrap()
    }

    fn parse_with_block_delete(source: &str) -> GcodeProgram {
        parse_program_with_options(
            ProgramParseRequest {
                source_name: "fixture.nc".to_owned(),
                source: source.to_owned(),
            },
            ProgramParseOptions { block_delete: true },
        )
        .unwrap()
    }

    fn completed_heightmap() -> Heightmap {
        let plan = plan_heightmap(
            HeightmapPlanRequest {
                origin_x_mm: 0.0,
                origin_y_mm: 0.0,
                width_mm: 10.0,
                height_mm: 10.0,
                columns: 2,
                rows: 2,
                clearance_z_mm: 2.0,
                ..HeightmapPlanRequest::default()
            },
            None,
        )
        .unwrap();
        let mut map = Heightmap::new(plan);
        map.bind_coordinates(
            WorkCoordinateSystem::G54,
            Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                a: None,
            },
        );
        for point in map.plan.points.clone() {
            // A simple plane: work surface falls 1 mm from X0 to X10.
            map.record_sample(point.sequence, -point.x_mm / 10.0, true)
                .unwrap();
        }
        map
    }

    #[test]
    fn builds_only_from_normalized_lines_and_adds_an_off_preamble() {
        let plan = build_dry_run_plan(&parse("G21 G90 ; units\nG0 X1\nM5 M9")).unwrap();

        assert_eq!(
            plan.lines()
                .iter()
                .map(DryRunLine::command)
                .collect::<Vec<_>>(),
            vec!["M5", "M9", "G21 G90", "G0 X1", "M5 M9", "M5", "M9"]
        );
        assert_eq!(plan.lines()[0].kind(), DryRunLineKind::SafetyPreamble);
        assert_eq!(plan.lines()[2].source_line(), Some(1));
    }

    #[test]
    fn wire_commands_replace_file_numbers_with_source_line_numbers() {
        let plan = build_dry_run_plan(&parse("N900 G21 G90\nN950 G0 X1")).unwrap();
        let commands = plan
            .lines()
            .iter()
            .map(DryRunLine::wire_command)
            .collect::<Vec<_>>();

        assert_eq!(commands[0], "M5");
        assert_eq!(commands[2], "N1 G21 G90");
        assert_eq!(commands[3], "N2 G0 X1");
        assert!(commands.iter().all(|command| !command.contains("N900")));
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
        assert_eq!(plan.lines()[5].kind(), DryRunLineKind::SafetyEpilogue);
        assert_eq!(plan.lines()[6].kind(), DryRunLineKind::SafetyEpilogue);
        assert_eq!(plan.lines()[7].kind(), DryRunLineKind::ProgramEnd);
        assert!(
            !plan
                .lines()
                .iter()
                .any(|line| line.command().contains("X99"))
        );
    }

    #[test]
    fn optional_stop_and_block_delete_are_independent_host_options() {
        let source = "G21 G90 G94\n/G1 X1 F10\nM1\nG1 X2 F10\nM30";
        let default = build_program_run_plan(&parse(source), ProgramRunPolicy::Cutting).unwrap();
        assert!(
            default
                .lines()
                .iter()
                .any(|line| line.command() == "G1 X1 F10")
        );
        assert!(!default.lines().iter().any(|line| line.command() == "M1"));

        let options = ProgramExecutionOptions {
            optional_stop: true,
            block_delete: true,
            ..ProgramExecutionOptions::default()
        };
        let configured = build_program_run_plan_with_options(
            &parse_with_block_delete(source),
            ProgramRunPolicy::Cutting,
            options,
        )
        .unwrap();
        assert!(
            !configured
                .lines()
                .iter()
                .any(|line| line.command() == "G1 X1 F10")
        );
        assert!(configured.lines().iter().any(|line| {
            line.command() == "M1" && line.kind() == DryRunLineKind::OptionalPause
        }));
        assert_eq!(configured.execution_options(), options);
    }

    #[test]
    fn rejects_ambiguous_pause_blocks_and_parse_option_mismatch() {
        for source in ["M0 G1 X1 F10", "M1 S1000"] {
            let error =
                build_program_run_plan(&parse(source), ProgramRunPolicy::Cutting).unwrap_err();
            assert!(error.blockers().iter().any(|blocker| {
                blocker.kind == DryRunBlockerKind::UnsupportedProgram
                    && blocker.message.contains("must be isolated")
            }));
        }

        let mismatch = build_program_run_plan_with_options(
            &parse_with_block_delete("G21\n/G1 X1 F10"),
            ProgramRunPolicy::Cutting,
            ProgramExecutionOptions::default(),
        )
        .unwrap_err();
        assert!(mismatch.blockers().iter().any(|blocker| {
            blocker.kind == DryRunBlockerKind::UnsupportedProgram
                && blocker.message.contains("disagree")
        }));
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

        let oversized_source = format!("G1 X1 F1.{}", "0".repeat(260));
        let oversized = build_dry_run_plan(&parse(&oversized_source)).unwrap_err();
        assert!(
            oversized
                .blockers()
                .iter()
                .any(|blocker| blocker.kind == DryRunBlockerKind::CommandTooLong)
        );
    }

    #[test]
    fn cutting_depth_adjustment_moves_only_negative_cutting_z() {
        let program =
            parse("G21 G90 G94 G17\nG0 Z3\nG1 Z-0.2 F100\nG1 X10 Z-0.1\nG1 Z0\nG0 Z3\nM30");
        let options = ProgramExecutionOptions {
            cutting_depth_adjustment_um: Some(-100),
            ..ProgramExecutionOptions::default()
        };
        let plan =
            build_program_run_plan_with_options(&program, ProgramRunPolicy::Cutting, options)
                .unwrap();
        let commands = plan
            .lines()
            .iter()
            .map(DryRunLine::command)
            .collect::<Vec<_>>();

        assert!(commands.iter().any(|command| command.contains("Z-0.3000")));
        assert!(commands.iter().any(|command| command.contains("Z-0.2000")));
        assert!(commands.iter().any(|command| command.contains("Z0.0000")));
        assert!(
            commands
                .iter()
                .filter(|command| command.contains("Z3.0000"))
                .count()
                >= 2
        );
        assert_eq!(plan.execution_options(), options);

        let bounds = program_bounds_with_execution_options(&program, options).unwrap();
        assert!((bounds.min.z + 0.3).abs() < 1e-9);
        assert!((bounds.max.z - 3.0).abs() < 1e-9);
    }

    #[test]
    fn positive_offset_is_added_exactly_without_deriving_a_target_depth() {
        let options = ProgramExecutionOptions {
            cutting_depth_adjustment_um: Some(100),
            ..ProgramExecutionOptions::default()
        };
        let plan = build_program_run_plan_with_options(
            &parse("G21 G90 G94 G17\nG0 Z2\nG1 Z-0.2 F100\nG1 Z-0.05\nG0 Z2"),
            ProgramRunPolicy::Cutting,
            options,
        )
        .unwrap();
        let commands = plan
            .lines()
            .iter()
            .map(DryRunLine::command)
            .collect::<Vec<_>>();

        assert!(commands.iter().any(|command| command.contains("Z-0.1000")));
        assert!(commands.iter().any(|command| command.contains("Z0.0500")));
    }

    #[test]
    fn cutting_depth_adjustment_is_bounded_and_requires_negative_cutting_geometry() {
        let excessive = ProgramExecutionOptions {
            cutting_depth_adjustment_um: Some(-10_001),
            ..ProgramExecutionOptions::default()
        };
        let error = build_program_run_plan_with_options(
            &parse("G21 G90 G94\nG1 Z-0.2 F100"),
            ProgramRunPolicy::Cutting,
            excessive,
        )
        .unwrap_err();
        assert_eq!(
            error.blockers()[0].kind,
            DryRunBlockerKind::CuttingDepthAdjustment
        );

        let no_depth = ProgramExecutionOptions {
            cutting_depth_adjustment_um: Some(0),
            ..ProgramExecutionOptions::default()
        };
        let error = build_program_run_plan_with_options(
            &parse("G21 G90 G94\nG1 X1 Z0 F100"),
            ProgramRunPolicy::Cutting,
            no_depth,
        )
        .unwrap_err();
        assert!(error.blockers()[0].message.contains("below work Z0"));
    }

    #[test]
    fn heightmap_compensation_interpolates_surface_z_and_preserves_safe_z() {
        let options = ProgramExecutionOptions {
            surface_map_id: Some(7),
            ..ProgramExecutionOptions::default()
        };
        let plan = build_program_run_plan_with_heightmap(
            &parse("G21 G90 G94 G17\nG0 Z2\nG0 X0 Y0\nG1 Z-0.2 F30\nG1 X10 Y0 F100\nG0 Z2\nM30"),
            ProgramRunPolicy::Cutting,
            options,
            Some(&completed_heightmap()),
        )
        .unwrap();
        let commands = plan
            .lines()
            .iter()
            .map(DryRunLine::command)
            .collect::<Vec<_>>();

        assert!(
            commands
                .iter()
                .any(|command| { command.contains("X10.0000") && command.contains("Z-1.2000") })
        );
        assert!(commands.contains(&"G0 Z2"));
        assert_eq!(plan.execution_options().surface_map_id, Some(7));
    }

    #[test]
    fn heightmap_compensation_is_applied_after_cutting_depth_adjustment() {
        let options = ProgramExecutionOptions {
            surface_map_id: Some(7),
            cutting_depth_adjustment_um: Some(-100),
            ..ProgramExecutionOptions::default()
        };
        let plan = build_program_run_plan_with_heightmap(
            &parse("G21 G90 G94 G17\nG0 Z2\nG0 X0 Y0\nG1 Z-0.2 F30\nG1 X10 Y0 F100"),
            ProgramRunPolicy::Cutting,
            options,
            Some(&completed_heightmap()),
        )
        .unwrap();

        assert!(plan.lines().iter().any(|line| {
            line.command().contains("X10.0000") && line.command().contains("Z-1.3000")
        }));
    }

    #[test]
    fn heightmap_compensation_fails_closed_outside_the_measured_area() {
        let options = ProgramExecutionOptions {
            surface_map_id: Some(7),
            ..ProgramExecutionOptions::default()
        };
        let error = build_program_run_plan_with_heightmap(
            &parse("G21 G90 G94 G17\nG0 Z2\nG0 X0 Y0\nG1 Z-0.2 F30\nG1 X11 F100"),
            ProgramRunPolicy::Cutting,
            options,
            Some(&completed_heightmap()),
        )
        .unwrap_err();

        assert!(error.blockers().iter().any(|blocker| {
            blocker.kind == DryRunBlockerKind::HeightmapCompensation
                && blocker.message.contains("perimeter")
        }));
    }

    #[test]
    fn selected_heightmap_requires_complete_data_and_explicit_selection() {
        let plan = plan_heightmap(
            HeightmapPlanRequest {
                width_mm: 10.0,
                height_mm: 10.0,
                columns: 2,
                rows: 2,
                ..HeightmapPlanRequest::default()
            },
            None,
        )
        .unwrap();
        let incomplete = Heightmap::new(plan);
        let mut incomplete = incomplete;
        incomplete.bind_coordinates(
            WorkCoordinateSystem::G54,
            Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                a: None,
            },
        );
        let program = parse("G21 G90 G94 G17\nG0 X0 Y0 Z2\nG1 Z-0.2 F30");
        let selected = ProgramExecutionOptions {
            surface_map_id: Some(2),
            ..ProgramExecutionOptions::default()
        };

        let missing = build_program_run_plan_with_heightmap(
            &program,
            ProgramRunPolicy::Cutting,
            selected,
            None,
        )
        .unwrap_err();
        assert!(missing.blockers()[0].message.contains("unavailable"));

        let incomplete = build_program_run_plan_with_heightmap(
            &program,
            ProgramRunPolicy::Cutting,
            selected,
            Some(&incomplete),
        )
        .unwrap_err();
        assert!(incomplete.blockers()[0].message.contains("incomplete"));
    }

    #[test]
    fn compensation_rejects_a_surface_move_from_the_parsers_implicit_origin() {
        let options = ProgramExecutionOptions {
            surface_map_id: Some(7),
            ..ProgramExecutionOptions::default()
        };
        let error = build_program_run_plan_with_heightmap(
            &parse("G21 G90 G94 G17\nG1 X5 Y5 Z-0.2 F30"),
            ProgramRunPolicy::Cutting,
            options,
            Some(&completed_heightmap()),
        )
        .unwrap_err();

        assert!(error.blockers()[0].message.contains("safe-Z positioning"));
    }
}
