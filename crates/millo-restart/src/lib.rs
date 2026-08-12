use millo_gcode::{
    GcodeProgram, ProgramDistanceMode, ProgramExecutionCheckpoint, ProgramFeedMode,
    ProgramMotionMode, ProgramParseRequest, ProgramPlane, ProgramPoint, ProgramSpindleMode,
    ProgramUnitMode, ProgramWorkCoordinateSystem, ToolpathKind,
};
use serde::Serialize;
use thiserror::Error;

const CLEARANCE_EPSILON_MM: f64 = 0.002;
const MAX_SAFE_Z_MM: f64 = 10_000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafeStartIntent {
    AirRun,
    Cutting,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SafeStartRequest {
    pub selected_source_line: usize,
    pub safe_z_mm: f64,
    pub intent: SafeStartIntent,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeStartPackage {
    pub original_source_name: String,
    pub selected_source_line: usize,
    pub restart_source_line: usize,
    pub restart_position: ProgramPoint,
    pub safe_z_mm: f64,
    pub minimum_safe_z_mm: f64,
    pub replayed_executable_lines: usize,
    pub remaining_executable_lines: usize,
    pub work_coordinate_system: ProgramWorkCoordinateSystem,
    pub selected_tool: Option<u8>,
    pub spindle_mode: ProgramSpindleMode,
    pub request: ProgramParseRequest,
}

#[derive(Debug, Error, PartialEq)]
pub enum SafeStartError {
    #[error("selected source line {0} is not executable")]
    LineNotExecutable(usize),
    #[error("selected source line {0} has no preview motion")]
    LineHasNoMotion(usize),
    #[error("selected source line {0} is after M2/M30 and is not reachable")]
    LineAfterProgramEnd(usize),
    #[error("selected source line {0} has no parser checkpoint")]
    CheckpointUnavailable(usize),
    #[error("no clearance-height rapid entry exists before source line {0}")]
    SafeEntryUnavailable(usize),
    #[error("safe Z must be finite and between {minimum:.3} and {maximum:.3} mm")]
    InvalidSafeZ { minimum: f64, maximum: f64 },
    #[error("the selected line has no executable remainder")]
    EmptyRemainder,
}

pub fn build_safe_start(
    program: &GcodeProgram,
    source: &str,
    request: SafeStartRequest,
) -> Result<SafeStartPackage, SafeStartError> {
    let selected_line = program
        .lines
        .iter()
        .find(|line| line.source_line == request.selected_source_line)
        .filter(|line| line.executable && !line.block_deleted)
        .ok_or(SafeStartError::LineNotExecutable(
            request.selected_source_line,
        ))?;
    if !program
        .toolpath
        .iter()
        .any(|segment| segment.source_line == selected_line.source_line)
    {
        return Err(SafeStartError::LineHasNoMotion(
            request.selected_source_line,
        ));
    }
    if program.lines.iter().any(|line| {
        line.source_line < selected_line.source_line
            && line.executable
            && !line.block_deleted
            && has_program_end(&line.normalized)
    }) {
        return Err(SafeStartError::LineAfterProgramEnd(
            request.selected_source_line,
        ));
    }

    let minimum_safe_z_mm = program.summary.bounds.map(|bounds| bounds.max.z).ok_or(
        SafeStartError::LineHasNoMotion(request.selected_source_line),
    )?;
    if !request.safe_z_mm.is_finite()
        || request.safe_z_mm + CLEARANCE_EPSILON_MM < minimum_safe_z_mm
        || request.safe_z_mm > MAX_SAFE_Z_MM
    {
        return Err(SafeStartError::InvalidSafeZ {
            minimum: minimum_safe_z_mm,
            maximum: MAX_SAFE_Z_MM,
        });
    }

    let selected_checkpoint = checkpoint(program, request.selected_source_line)?;
    let anchor = if selected_checkpoint.position.z + CLEARANCE_EPSILON_MM >= minimum_safe_z_mm {
        selected_checkpoint
    } else {
        program
            .toolpath
            .iter()
            .filter(|segment| segment.source_line <= request.selected_source_line)
            .filter(|segment| segment.kind == ToolpathKind::Rapid)
            .filter(|segment| {
                segment
                    .points
                    .first()
                    .is_some_and(|point| point.z + CLEARANCE_EPSILON_MM >= minimum_safe_z_mm)
            })
            .filter_map(|segment| checkpoint(program, segment.source_line).ok())
            .next_back()
            .ok_or(SafeStartError::SafeEntryUnavailable(
                request.selected_source_line,
            ))?
    };

    let remaining_executable_lines = program
        .lines
        .iter()
        .filter(|line| {
            line.source_line >= anchor.source_line && line.executable && !line.block_deleted
        })
        .count();
    if remaining_executable_lines == 0 {
        return Err(SafeStartError::EmptyRemainder);
    }
    let replayed_executable_lines = program
        .lines
        .iter()
        .filter(|line| {
            line.source_line >= anchor.source_line
                && line.source_line < request.selected_source_line
                && line.executable
                && !line.block_deleted
        })
        .count();

    let generated_source = safe_start_source(
        program,
        source,
        anchor,
        request.safe_z_mm,
        request.intent,
        request.selected_source_line,
    );
    Ok(SafeStartPackage {
        original_source_name: program.source_name.clone(),
        selected_source_line: request.selected_source_line,
        restart_source_line: anchor.source_line,
        restart_position: anchor.position,
        safe_z_mm: request.safe_z_mm,
        minimum_safe_z_mm,
        replayed_executable_lines,
        remaining_executable_lines,
        work_coordinate_system: anchor.work_coordinate_system,
        selected_tool: anchor.selected_tool,
        spindle_mode: anchor.spindle_mode,
        request: ProgramParseRequest {
            source_name: safe_start_source_name(request.selected_source_line, &program.source_name),
            source: generated_source,
        },
    })
}

fn has_program_end(normalized: &str) -> bool {
    normalized.split_whitespace().any(|word| {
        let Some(value) = word
            .strip_prefix('M')
            .and_then(|value| value.parse::<f64>().ok())
        else {
            return false;
        };
        (value - 2.0).abs() < f64::EPSILON || (value - 30.0).abs() < f64::EPSILON
    })
}

fn checkpoint(
    program: &GcodeProgram,
    source_line: usize,
) -> Result<ProgramExecutionCheckpoint, SafeStartError> {
    program
        .execution_checkpoints
        .iter()
        .find(|checkpoint| checkpoint.source_line == source_line)
        .copied()
        .ok_or(SafeStartError::CheckpointUnavailable(source_line))
}

fn safe_start_source(
    program: &GcodeProgram,
    source: &str,
    anchor: ProgramExecutionCheckpoint,
    safe_z_mm: f64,
    intent: SafeStartIntent,
    selected_source_line: usize,
) -> String {
    let mut lines = vec![
        format!(
            "(Millo safe start from L{selected_source_line} of {})",
            program.source_name
        ),
        "M5".to_owned(),
        "M9".to_owned(),
        "G21 G90 G94".to_owned(),
        wcs_word(anchor.work_coordinate_system).to_owned(),
        format!("G0 Z{safe_z_mm:.4}"),
        format!("G0 X{:.4} Y{:.4}", anchor.position.x, anchor.position.y),
    ];
    if (safe_z_mm - anchor.position.z).abs() > CLEARANCE_EPSILON_MM {
        lines.push(format!("G0 Z{:.4}", anchor.position.z));
    }
    if let Some(tool) = anchor.selected_tool {
        lines.push(format!("T{tool}"));
        if intent == SafeStartIntent::Cutting {
            lines.push("M6".to_owned());
        }
    }
    if intent == SafeStartIntent::Cutting {
        if let Some(speed) = anchor.spindle_speed {
            lines.push(format!("S{speed:.3}"));
        }
        match anchor.spindle_mode {
            ProgramSpindleMode::Clockwise => lines.push("M3".to_owned()),
            ProgramSpindleMode::Counterclockwise => lines.push("M4".to_owned()),
            ProgramSpindleMode::Off => {}
        }
    }
    lines.push(modal_restore(anchor));
    lines.extend(
        source
            .lines()
            .skip(anchor.source_line.saturating_sub(1))
            .map(str::to_owned),
    );
    lines.join("\n")
}

pub fn modal_restore(checkpoint: ProgramExecutionCheckpoint) -> String {
    let units = match checkpoint.units {
        ProgramUnitMode::Millimeters => "G21",
        ProgramUnitMode::Inches => "G20",
    };
    let distance = match checkpoint.distance {
        ProgramDistanceMode::Absolute => "G90",
        ProgramDistanceMode::Incremental => "G91",
    };
    let arc_distance = match checkpoint.arc_distance {
        ProgramDistanceMode::Absolute => "G90.1",
        ProgramDistanceMode::Incremental => "G91.1",
    };
    let feed_mode = match checkpoint.feed_mode {
        ProgramFeedMode::InverseTime => "G93",
        ProgramFeedMode::UnitsPerMinute => "G94",
    };
    let plane = match checkpoint.plane {
        ProgramPlane::Xy => "G17",
        ProgramPlane::Xz => "G18",
        ProgramPlane::Yz => "G19",
    };
    let motion = match checkpoint.motion {
        ProgramMotionMode::None => "G80",
        ProgramMotionMode::Rapid => "G0",
        ProgramMotionMode::Linear => "G1",
        ProgramMotionMode::ArcClockwise => "G2",
        ProgramMotionMode::ArcCounterclockwise => "G3",
    };
    let mut words = vec![units, distance, arc_distance, feed_mode, plane, motion]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if let Some(feed) = checkpoint.feed_rate {
        let native_feed = match (checkpoint.units, checkpoint.feed_mode) {
            (ProgramUnitMode::Inches, ProgramFeedMode::UnitsPerMinute) => feed / 25.4,
            _ => feed,
        };
        words.push(format!("F{native_feed:.4}"));
    }
    words.join(" ")
}

pub const fn wcs_word(wcs: ProgramWorkCoordinateSystem) -> &'static str {
    match wcs {
        ProgramWorkCoordinateSystem::G54 => "G54",
        ProgramWorkCoordinateSystem::G55 => "G55",
        ProgramWorkCoordinateSystem::G56 => "G56",
        ProgramWorkCoordinateSystem::G57 => "G57",
        ProgramWorkCoordinateSystem::G58 => "G58",
        ProgramWorkCoordinateSystem::G59 => "G59",
    }
}

fn safe_start_source_name(selected_source_line: usize, original: &str) -> String {
    let name = format!("safe-start-L{selected_source_line}-{original}");
    if name.len() <= millo_gcode::MAX_SOURCE_NAME_BYTES {
        name
    } else {
        format!("safe-start-L{selected_source_line}.nc")
    }
}

#[cfg(test)]
mod tests {
    use millo_gcode::{ProgramParseOptions, parse_program_with_options};

    use super::*;

    const SOURCE: &str = "G21 G90 G94 G17 G55\nT2 M6\nS12000 M3\nG0 Z5\nG0 X0 Y0\nG1 Z-0.2 F100\nG1 X10\nG0 Z5\nG0 X20 Y0\nG1 Z-0.2 F100\nG1 X30\nM30";

    fn parsed() -> GcodeProgram {
        parse_program_with_options(
            ProgramParseRequest {
                source_name: "long-job.nc".to_owned(),
                source: SOURCE.to_owned(),
            },
            ProgramParseOptions::default(),
        )
        .unwrap()
    }

    #[test]
    fn rewinds_to_the_latest_clearance_entry_and_restores_state() {
        let package = build_safe_start(
            &parsed(),
            SOURCE,
            SafeStartRequest {
                selected_source_line: 11,
                safe_z_mm: 8.0,
                intent: SafeStartIntent::Cutting,
            },
        )
        .unwrap();

        assert_eq!(package.selected_source_line, 11);
        assert_eq!(package.restart_source_line, 9);
        assert_eq!(
            package.restart_position,
            ProgramPoint {
                x: 10.0,
                y: 0.0,
                z: 5.0
            }
        );
        assert_eq!(package.replayed_executable_lines, 2);
        assert_eq!(package.selected_tool, Some(2));
        assert_eq!(
            package.work_coordinate_system,
            ProgramWorkCoordinateSystem::G55
        );
        let lines = package.request.source.lines().collect::<Vec<_>>();
        assert_eq!(
            &lines[1..13],
            &[
                "M5",
                "M9",
                "G21 G90 G94",
                "G55",
                "G0 Z8.0000",
                "G0 X10.0000 Y0.0000",
                "G0 Z5.0000",
                "T2",
                "M6",
                "S12000.000",
                "M3",
                "G21 G90 G91.1 G94 G17 G0 F100.0000",
            ]
        );
        assert_eq!(lines[13], "G0 X20 Y0");
        assert_eq!(lines.last(), Some(&"M30"));
    }

    #[test]
    fn air_run_restores_modal_and_tool_selection_without_spindle_or_m6() {
        let package = build_safe_start(
            &parsed(),
            SOURCE,
            SafeStartRequest {
                selected_source_line: 11,
                safe_z_mm: 6.0,
                intent: SafeStartIntent::AirRun,
            },
        )
        .unwrap();

        assert!(package.request.source.contains("\nT2\n"));
        assert!(!package.request.source.contains("\nM6\n"));
        assert!(!package.request.source.contains("\nM3\n"));
        assert!(!package.request.source.contains("S12000.000"));
    }

    #[test]
    fn rejects_non_motion_lines_and_unsafe_clearance() {
        assert_eq!(
            build_safe_start(
                &parsed(),
                SOURCE,
                SafeStartRequest {
                    selected_source_line: 3,
                    safe_z_mm: 8.0,
                    intent: SafeStartIntent::Cutting,
                },
            )
            .unwrap_err(),
            SafeStartError::LineHasNoMotion(3)
        );
        assert_eq!(
            build_safe_start(
                &parsed(),
                SOURCE,
                SafeStartRequest {
                    selected_source_line: 11,
                    safe_z_mm: 4.0,
                    intent: SafeStartIntent::Cutting,
                },
            )
            .unwrap_err(),
            SafeStartError::InvalidSafeZ {
                minimum: 5.0,
                maximum: 10_000.0,
            }
        );
    }

    #[test]
    fn refuses_to_invent_a_descent_when_no_clearance_entry_exists() {
        let source = "G21 G90 G94 G17\nG1 Z-1 F50\nG1 X10";
        let program = parse_program_with_options(
            ProgramParseRequest {
                source_name: "unsafe.nc".to_owned(),
                source: source.to_owned(),
            },
            ProgramParseOptions::default(),
        )
        .unwrap();

        assert_eq!(
            build_safe_start(
                &program,
                source,
                SafeStartRequest {
                    selected_source_line: 3,
                    safe_z_mm: 1.0,
                    intent: SafeStartIntent::Cutting,
                },
            )
            .unwrap_err(),
            SafeStartError::SafeEntryUnavailable(3)
        );
    }

    #[test]
    fn rejects_preview_motion_that_is_unreachable_after_program_end() {
        let source = "G21 G90 G94 G17\nG0 Z5\nG1 X1 F50\nM30\nG0 Z5\nG1 X10 F50";
        let program = parse_program_with_options(
            ProgramParseRequest {
                source_name: "ended.nc".to_owned(),
                source: source.to_owned(),
            },
            ProgramParseOptions::default(),
        )
        .unwrap();

        assert_eq!(
            build_safe_start(
                &program,
                source,
                SafeStartRequest {
                    selected_source_line: 6,
                    safe_z_mm: 7.0,
                    intent: SafeStartIntent::Cutting,
                },
            )
            .unwrap_err(),
            SafeStartError::LineAfterProgramEnd(6)
        );
    }
}
