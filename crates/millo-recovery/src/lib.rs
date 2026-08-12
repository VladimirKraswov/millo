use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use millo_domain::Position;
use millo_dry_run::ProgramExecutionOptions;
use millo_gcode::{
    GcodeProgram, ProgramDistanceMode, ProgramExecutionCheckpoint, ProgramFeedMode,
    ProgramMotionMode, ProgramParseOptions, ProgramParseRequest, ProgramPlane, ProgramPoint,
    ProgramSpindleMode, ProgramUnitMode, ProgramWorkCoordinateSystem, ToolpathKind,
    parse_program_with_options,
};
use millo_run::{ProgramRunIntent, program_fingerprint};
use millo_sender::{SenderMode, SenderSnapshot, SenderState};
use millo_storage::{backup_path, write_atomically};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const RECOVERY_SCHEMA_VERSION: u16 = 1;
const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(1);
const CLEARANCE_EPSILON_MM: f64 = 0.002;
const MAX_SAFE_Z_MM: f64 = 10_000.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverySeed {
    pub machine_fingerprint: String,
    pub profile_id: Option<String>,
    pub source_name: String,
    pub source: String,
    pub program_fingerprint: String,
    pub intent: ProgramRunIntent,
    pub execution_options: ProgramExecutionOptions,
    pub run_sequence: u64,
    pub start_machine_position: Option<Position>,
    pub start_work_position: Option<Position>,
    pub start_work_coordinate_offset: Option<Position>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryRecord {
    id: u64,
    machine_fingerprint: String,
    profile_id: Option<String>,
    source_name: String,
    source: String,
    program_fingerprint: String,
    intent: ProgramRunIntent,
    execution_options: ProgramExecutionOptions,
    run_sequence: u64,
    state: SenderState,
    created_at_unix_ms: u64,
    updated_at_unix_ms: u64,
    total_lines: usize,
    acknowledged_lines: usize,
    executing_source_line: Option<usize>,
    start_machine_position: Option<Position>,
    start_work_position: Option<Position>,
    start_work_coordinate_offset: Option<Position>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryFile {
    schema_version: u16,
    record: Option<RecoveryRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramRecoveryCandidate {
    pub id: u64,
    pub source_name: String,
    pub intent: ProgramRunIntent,
    pub state: SenderState,
    pub updated_at_unix_ms: u64,
    pub total_lines: usize,
    pub acknowledged_lines: usize,
    pub executing_source_line: Option<usize>,
    pub restart_source_line: Option<usize>,
    pub restart_position: Option<ProgramPoint>,
    pub ready: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramRecoveryPackage {
    pub recovery_id: u64,
    pub original_source_name: String,
    pub interrupted_source_line: usize,
    pub restart_source_line: usize,
    pub restart_position: ProgramPoint,
    pub clearance_z_mm: f64,
    pub repeated_source_lines: usize,
    pub intent: ProgramRunIntent,
    pub execution_options: ProgramExecutionOptions,
    pub request: ProgramParseRequest,
}

#[derive(Debug, Error)]
pub enum ProgramRecoveryError {
    #[error("program recovery I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("program recovery JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported program recovery schema {0}")]
    UnsupportedSchema(u16),
    #[error("there is no interrupted program recovery record")]
    Missing,
    #[error("program recovery record {actual} does not match requested record {expected}")]
    RecordMismatch { expected: u64, actual: u64 },
    #[error("the stored program no longer matches its recovery fingerprint")]
    ProgramChanged,
    #[error("GRBL did not report an executing source line before interruption")]
    ExecutingLineUnavailable,
    #[error("the interrupted source line is not present in the stored program")]
    CheckpointUnavailable,
    #[error("safe Z must be finite and between {minimum:.3} and {maximum:.3} mm")]
    InvalidSafeZ { minimum: f64, maximum: f64 },
    #[error("program recovery cannot arm a non-physical sender mode")]
    InvalidSenderMode,
    #[error("program recovery cannot arm sender run sequence zero")]
    InvalidRunSequence,
    #[error("program recovery source is invalid: {0}")]
    InvalidSource(String),
}

#[derive(Debug, Clone)]
struct PersistedCheckpoint {
    run_sequence: u64,
    state: SenderState,
    executing_source_line: Option<usize>,
    persisted_at: Instant,
}

pub struct ProgramRecoveryStore {
    path: Option<PathBuf>,
    record: Option<RecoveryRecord>,
    checkpoint: Option<PersistedCheckpoint>,
}

impl ProgramRecoveryStore {
    pub fn in_memory() -> Self {
        Self {
            path: None,
            record: None,
            checkpoint: None,
        }
    }

    pub fn load(path: impl Into<PathBuf>) -> Result<Self, ProgramRecoveryError> {
        let path = path.into();
        let loaded = load_with_backup(&path)?;
        let recovered_from_backup = loaded.as_ref().is_some_and(|loaded| loaded.1);
        let store = Self {
            path: Some(path),
            record: loaded.and_then(|loaded| loaded.0.record),
            checkpoint: None,
        };
        if recovered_from_backup {
            store.remove_corrupt_primary()?;
            store.persist()?;
        }
        Ok(store)
    }

    pub fn arm(
        &mut self,
        seed: RecoverySeed,
        snapshot: &SenderSnapshot,
        now: SystemTime,
        monotonic: Instant,
    ) -> Result<ProgramRecoveryCandidate, ProgramRecoveryError> {
        if seed.run_sequence == 0 || snapshot.run_sequence != seed.run_sequence {
            return Err(ProgramRecoveryError::InvalidRunSequence);
        }
        if !matches!(snapshot.mode, Some(SenderMode::AirRun | SenderMode::CutRun)) {
            return Err(ProgramRecoveryError::InvalidSenderMode);
        }
        let program = parse_seed(&seed)?;
        if program_fingerprint(&program) != seed.program_fingerprint {
            return Err(ProgramRecoveryError::ProgramChanged);
        }
        let now_unix_ms = unix_millis(now);
        self.record = Some(RecoveryRecord {
            id: unix_micros(now),
            machine_fingerprint: seed.machine_fingerprint,
            profile_id: seed.profile_id,
            source_name: seed.source_name,
            source: seed.source,
            program_fingerprint: seed.program_fingerprint,
            intent: seed.intent,
            execution_options: seed.execution_options,
            run_sequence: seed.run_sequence,
            state: snapshot.state,
            created_at_unix_ms: now_unix_ms,
            updated_at_unix_ms: now_unix_ms,
            total_lines: snapshot.total_lines,
            acknowledged_lines: snapshot.acknowledged_lines,
            executing_source_line: snapshot.executing_source_line,
            start_machine_position: seed.start_machine_position,
            start_work_position: seed.start_work_position,
            start_work_coordinate_offset: seed.start_work_coordinate_offset,
        });
        self.persist()?;
        self.checkpoint = Some(PersistedCheckpoint {
            run_sequence: seed.run_sequence,
            state: snapshot.state,
            executing_source_line: snapshot.executing_source_line,
            persisted_at: monotonic,
        });
        self.candidate()?.ok_or(ProgramRecoveryError::Missing)
    }

    pub fn observe(
        &mut self,
        snapshot: &SenderSnapshot,
        now: SystemTime,
        monotonic: Instant,
    ) -> Result<bool, ProgramRecoveryError> {
        let Some(record) = self.record.as_mut() else {
            return Ok(false);
        };
        if snapshot.run_sequence != record.run_sequence {
            return Ok(false);
        }
        record.state = snapshot.state;
        record.updated_at_unix_ms = unix_millis(now);
        record.total_lines = snapshot.total_lines;
        record.acknowledged_lines = snapshot.acknowledged_lines;
        if snapshot.executing_source_line.is_some() {
            record.executing_source_line = snapshot.executing_source_line;
        }
        let executing_source_line = record.executing_source_line;
        let terminal = is_terminal(snapshot.state);
        let should_persist = self.checkpoint.as_ref().is_none_or(|checkpoint| {
            checkpoint.run_sequence != snapshot.run_sequence
                || checkpoint.state != snapshot.state
                || (checkpoint.executing_source_line != executing_source_line
                    && monotonic.duration_since(checkpoint.persisted_at) >= CHECKPOINT_INTERVAL)
                || terminal
        });
        if !should_persist {
            return Ok(false);
        }
        self.persist()?;
        self.checkpoint = Some(PersistedCheckpoint {
            run_sequence: snapshot.run_sequence,
            state: snapshot.state,
            executing_source_line,
            persisted_at: monotonic,
        });
        Ok(true)
    }

    pub fn candidate(&self) -> Result<Option<ProgramRecoveryCandidate>, ProgramRecoveryError> {
        let Some(record) = self.record.as_ref() else {
            return Ok(None);
        };
        if record.state == SenderState::Completed {
            return Ok(None);
        }
        let base = |ready, detail, restart: Option<RecoveryAnchor>| ProgramRecoveryCandidate {
            id: record.id,
            source_name: record.source_name.clone(),
            intent: record.intent,
            state: record.state,
            updated_at_unix_ms: record.updated_at_unix_ms,
            total_lines: record.total_lines,
            acknowledged_lines: record.acknowledged_lines,
            executing_source_line: record.executing_source_line,
            restart_source_line: restart.as_ref().map(|anchor| anchor.source_line),
            restart_position: restart.map(|anchor| anchor.checkpoint.position),
            ready,
            detail,
        };
        let Some(executing) = record.executing_source_line else {
            return Ok(Some(base(
                false,
                "GRBL did not expose Ln execution telemetry; automatic line recovery is blocked"
                    .to_owned(),
                None,
            )));
        };
        let program = parse_record(record)?;
        if program_fingerprint(&program) != record.program_fingerprint {
            return Ok(Some(base(
                false,
                "Stored source fingerprint mismatch; recovery is blocked".to_owned(),
                None,
            )));
        }
        let anchor = recovery_anchor(&program, executing)?;
        Ok(Some(base(
            true,
            format!(
                "Restart from source line {} replays {} line(s) before the interrupted line",
                anchor.source_line,
                executing.saturating_sub(anchor.source_line)
            ),
            Some(anchor),
        )))
    }

    pub fn prepare(
        &self,
        recovery_id: u64,
        safe_z_mm: f64,
    ) -> Result<ProgramRecoveryPackage, ProgramRecoveryError> {
        let record = self.record.as_ref().ok_or(ProgramRecoveryError::Missing)?;
        if record.id != recovery_id {
            return Err(ProgramRecoveryError::RecordMismatch {
                expected: recovery_id,
                actual: record.id,
            });
        }
        let interrupted = record
            .executing_source_line
            .ok_or(ProgramRecoveryError::ExecutingLineUnavailable)?;
        let program = parse_record(record)?;
        if program_fingerprint(&program) != record.program_fingerprint {
            return Err(ProgramRecoveryError::ProgramChanged);
        }
        let anchor = recovery_anchor(&program, interrupted)?;
        let minimum_safe_z = program
            .summary
            .bounds
            .map_or(anchor.checkpoint.position.z, |bounds| bounds.max.z)
            .max(anchor.checkpoint.position.z);
        if !safe_z_mm.is_finite()
            || safe_z_mm + CLEARANCE_EPSILON_MM < minimum_safe_z
            || safe_z_mm > MAX_SAFE_Z_MM
        {
            return Err(ProgramRecoveryError::InvalidSafeZ {
                minimum: minimum_safe_z,
                maximum: MAX_SAFE_Z_MM,
            });
        }
        let request = ProgramParseRequest {
            source_name: recovery_source_name(record.id, &record.source_name),
            source: build_recovery_source(record, &anchor, safe_z_mm),
        };
        parse_program_with_options(
            request.clone(),
            ProgramParseOptions {
                block_delete: record.execution_options.block_delete,
            },
        )
        .map_err(|error| ProgramRecoveryError::InvalidSource(error.to_string()))?;
        Ok(ProgramRecoveryPackage {
            recovery_id: record.id,
            original_source_name: record.source_name.clone(),
            interrupted_source_line: interrupted,
            restart_source_line: anchor.source_line,
            restart_position: anchor.checkpoint.position,
            clearance_z_mm: safe_z_mm,
            repeated_source_lines: interrupted.saturating_sub(anchor.source_line),
            intent: record.intent,
            execution_options: record.execution_options,
            request,
        })
    }

    pub fn machine_matches(&self, recovery_id: u64, fingerprint: &str) -> bool {
        self.record.as_ref().is_some_and(|record| {
            record.id == recovery_id && record.machine_fingerprint == fingerprint
        })
    }

    pub fn dismiss(&mut self, recovery_id: u64) -> Result<(), ProgramRecoveryError> {
        let record = self.record.as_ref().ok_or(ProgramRecoveryError::Missing)?;
        if record.id != recovery_id {
            return Err(ProgramRecoveryError::RecordMismatch {
                expected: recovery_id,
                actual: record.id,
            });
        }
        self.record = None;
        self.checkpoint = None;
        self.persist()
    }

    fn persist(&self) -> Result<(), ProgramRecoveryError> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        let bytes = serde_json::to_vec_pretty(&RecoveryFile {
            schema_version: RECOVERY_SCHEMA_VERSION,
            record: self.record.clone(),
        })?;
        write_atomically(path, &bytes)?;
        Ok(())
    }

    fn remove_corrupt_primary(&self) -> Result<(), ProgramRecoveryError> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[derive(Debug, Clone)]
struct RecoveryAnchor {
    source_line: usize,
    checkpoint: ProgramExecutionCheckpoint,
}

fn recovery_anchor(
    program: &GcodeProgram,
    interrupted_source_line: usize,
) -> Result<RecoveryAnchor, ProgramRecoveryError> {
    let clearance_z = program
        .summary
        .bounds
        .map(|bounds| bounds.max.z)
        .ok_or(ProgramRecoveryError::CheckpointUnavailable)?;
    let preferred_line = program
        .toolpath
        .iter()
        .filter(|segment| segment.source_line <= interrupted_source_line)
        .filter(|segment| segment.kind == ToolpathKind::Rapid)
        .filter(|segment| {
            segment
                .points
                .first()
                .is_some_and(|point| point.z + CLEARANCE_EPSILON_MM >= clearance_z)
        })
        .map(|segment| segment.source_line)
        .next_back()
        .or_else(|| {
            program
                .toolpath
                .iter()
                .find(|segment| segment.source_line <= interrupted_source_line)
                .map(|segment| segment.source_line)
        })
        .ok_or(ProgramRecoveryError::CheckpointUnavailable)?;
    let checkpoint = program
        .execution_checkpoints
        .iter()
        .find(|checkpoint| checkpoint.source_line == preferred_line)
        .copied()
        .ok_or(ProgramRecoveryError::CheckpointUnavailable)?;
    Ok(RecoveryAnchor {
        source_line: preferred_line,
        checkpoint,
    })
}

fn build_recovery_source(
    record: &RecoveryRecord,
    anchor: &RecoveryAnchor,
    safe_z_mm: f64,
) -> String {
    let point = anchor.checkpoint.position;
    let mut lines = vec![
        format!("(Millo recovery {} for {})", record.id, record.source_name),
        "M5".to_owned(),
        "M9".to_owned(),
        "G21 G90 G94".to_owned(),
        wcs_word(anchor.checkpoint.work_coordinate_system).to_owned(),
        format!("G0 Z{safe_z_mm:.4}"),
        format!("G0 X{:.4} Y{:.4}", point.x, point.y),
        format!("G0 Z{:.4}", point.z),
    ];
    if let Some(tool) = anchor.checkpoint.selected_tool {
        lines.push(format!("T{tool}"));
    }
    if record.intent == ProgramRunIntent::Cutting {
        if let Some(speed) = anchor.checkpoint.spindle_speed {
            lines.push(format!("S{speed:.3}"));
        }
        match anchor.checkpoint.spindle_mode {
            ProgramSpindleMode::Clockwise => lines.push("M3".to_owned()),
            ProgramSpindleMode::Counterclockwise => lines.push("M4".to_owned()),
            ProgramSpindleMode::Off => {}
        }
    }
    lines.push(modal_restore(anchor.checkpoint));
    lines.extend(
        record
            .source
            .lines()
            .skip(anchor.source_line.saturating_sub(1))
            .map(str::to_owned),
    );
    lines.join("\n")
}

fn modal_restore(checkpoint: ProgramExecutionCheckpoint) -> String {
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

const fn wcs_word(wcs: ProgramWorkCoordinateSystem) -> &'static str {
    match wcs {
        ProgramWorkCoordinateSystem::G54 => "G54",
        ProgramWorkCoordinateSystem::G55 => "G55",
        ProgramWorkCoordinateSystem::G56 => "G56",
        ProgramWorkCoordinateSystem::G57 => "G57",
        ProgramWorkCoordinateSystem::G58 => "G58",
        ProgramWorkCoordinateSystem::G59 => "G59",
    }
}

fn recovery_source_name(id: u64, original: &str) -> String {
    let suffix = format!("recovery-{id}-{original}");
    if suffix.len() <= millo_gcode::MAX_SOURCE_NAME_BYTES {
        suffix
    } else {
        format!("recovery-{id}.nc")
    }
}

fn parse_seed(seed: &RecoverySeed) -> Result<GcodeProgram, ProgramRecoveryError> {
    parse_program_with_options(
        ProgramParseRequest {
            source_name: seed.source_name.clone(),
            source: seed.source.clone(),
        },
        ProgramParseOptions {
            block_delete: seed.execution_options.block_delete,
        },
    )
    .map_err(|error| ProgramRecoveryError::InvalidSource(error.to_string()))
}

fn parse_record(record: &RecoveryRecord) -> Result<GcodeProgram, ProgramRecoveryError> {
    parse_program_with_options(
        ProgramParseRequest {
            source_name: record.source_name.clone(),
            source: record.source.clone(),
        },
        ProgramParseOptions {
            block_delete: record.execution_options.block_delete,
        },
    )
    .map_err(|error| ProgramRecoveryError::InvalidSource(error.to_string()))
}

fn load_with_backup(path: &Path) -> Result<Option<(RecoveryFile, bool)>, ProgramRecoveryError> {
    let mut parse_error = None;
    for (index, candidate) in [path.to_path_buf(), backup_path(path)]
        .into_iter()
        .enumerate()
    {
        if !candidate.exists() {
            continue;
        }
        let bytes = fs::read(candidate)?;
        let file: RecoveryFile = match serde_json::from_slice(&bytes) {
            Ok(file) => file,
            Err(error) => {
                parse_error = Some(error);
                continue;
            }
        };
        if file.schema_version != RECOVERY_SCHEMA_VERSION {
            return Err(ProgramRecoveryError::UnsupportedSchema(file.schema_version));
        }
        return Ok(Some((file, index == 1)));
    }
    if let Some(error) = parse_error {
        return Err(error.into());
    }
    Ok(None)
}

fn is_terminal(state: SenderState) -> bool {
    matches!(
        state,
        SenderState::Completed | SenderState::Failed | SenderState::Cancelled
    )
}

fn unix_millis(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn unix_micros(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_micros()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use millo_dry_run::{ProgramRunPolicy, build_program_run_plan};
    use millo_sender::Sender;

    const SOURCE: &str = "G21 G90 G94 G17 G54\nG0 Z5\nG0 X0 Y0\nG1 Z-1 F100\nG1 X10\nG0 Z5\nG0 X20 Y0\nG1 Z-1 F100\nG1 X30\nM30";

    fn parsed() -> GcodeProgram {
        parse_program_with_options(
            ProgramParseRequest {
                source_name: "interrupted.nc".to_owned(),
                source: SOURCE.to_owned(),
            },
            ProgramParseOptions::default(),
        )
        .unwrap()
    }

    fn running_snapshot() -> SenderSnapshot {
        let plan = build_program_run_plan(&parsed(), ProgramRunPolicy::Cutting).unwrap();
        let mut sender = Sender::default();
        sender.load_cut_run(plan).unwrap();
        let mut snapshot = sender.start().unwrap();
        snapshot.executing_source_line = Some(9);
        snapshot.acknowledged_lines = 8;
        snapshot
    }

    fn seed(snapshot: &SenderSnapshot) -> RecoverySeed {
        let program = parsed();
        RecoverySeed {
            machine_fingerprint: "usb:1234:5678:serial".to_owned(),
            profile_id: Some("machine-1".to_owned()),
            source_name: program.source_name.clone(),
            source: SOURCE.to_owned(),
            program_fingerprint: program_fingerprint(&program),
            intent: ProgramRunIntent::Cutting,
            execution_options: ProgramExecutionOptions::default(),
            run_sequence: snapshot.run_sequence,
            start_machine_position: None,
            start_work_position: None,
            start_work_coordinate_offset: None,
        }
    }

    #[test]
    fn builds_a_conservative_clearance_restart_with_modal_restore() {
        let snapshot = running_snapshot();
        let mut store = ProgramRecoveryStore::in_memory();
        let wall = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let candidate = store
            .arm(seed(&snapshot), &snapshot, wall, Instant::now())
            .unwrap();

        assert!(candidate.ready);
        assert_eq!(candidate.executing_source_line, Some(9));
        assert_eq!(candidate.restart_source_line, Some(7));
        let package = store.prepare(candidate.id, 8.0).unwrap();
        assert_eq!(package.restart_source_line, 7);
        assert_eq!(package.restart_position.z, 5.0);
        assert!(package.request.source.contains("G0 Z8.0000"));
        assert!(package.request.source.contains("G21 G90 G91.1 G94 G17 G0"));
        assert!(package.request.source.ends_with("G1 X30\nM30"));
        assert!(!package.request.source.contains("G1 X10"));
    }

    #[test]
    fn blocks_recovery_without_ln_and_rejects_unsafe_clearance() {
        let mut snapshot = running_snapshot();
        snapshot.executing_source_line = None;
        let mut store = ProgramRecoveryStore::in_memory();
        let wall = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let candidate = store
            .arm(seed(&snapshot), &snapshot, wall, Instant::now())
            .unwrap();
        assert!(!candidate.ready);
        assert!(matches!(
            store.prepare(candidate.id, 8.0),
            Err(ProgramRecoveryError::ExecutingLineUnavailable)
        ));

        snapshot.executing_source_line = Some(9);
        let candidate = store
            .arm(seed(&snapshot), &snapshot, wall, Instant::now())
            .unwrap();
        assert!(matches!(
            store.prepare(candidate.id, 4.0),
            Err(ProgramRecoveryError::InvalidSafeZ { .. })
        ));
    }

    #[test]
    fn persists_exact_source_and_recovers_from_backup() {
        let unique = format!(
            "millo-recovery-{}-{}.json",
            std::process::id(),
            unix_micros(SystemTime::now())
        );
        let path = std::env::temp_dir().join(unique);
        let mut snapshot = running_snapshot();
        snapshot.executing_source_line = Some(8);
        let wall = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let mut store = ProgramRecoveryStore::load(&path).unwrap();
        let candidate = store
            .arm(seed(&snapshot), &snapshot, wall, Instant::now())
            .unwrap();
        let mut progressed = snapshot.clone();
        progressed.executing_source_line = Some(9);
        store
            .observe(
                &progressed,
                wall + Duration::from_secs(2),
                Instant::now() + Duration::from_secs(2),
            )
            .unwrap();
        fs::write(&path, b"corrupt").unwrap();

        let recovered = ProgramRecoveryStore::load(&path).unwrap();
        assert_eq!(recovered.candidate().unwrap().unwrap().id, candidate.id);

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup_path(&path));
        let _ = fs::remove_file(millo_storage::temporary_path(&path));
    }

    #[test]
    fn completed_runs_are_not_offered_and_dismissal_is_id_bound() {
        let snapshot = running_snapshot();
        let mut store = ProgramRecoveryStore::in_memory();
        let wall = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let candidate = store
            .arm(seed(&snapshot), &snapshot, wall, Instant::now())
            .unwrap();
        let mut completed = snapshot;
        completed.state = SenderState::Completed;
        store.observe(&completed, wall, Instant::now()).unwrap();
        assert!(store.candidate().unwrap().is_none());
        assert!(matches!(
            store.dismiss(candidate.id + 1),
            Err(ProgramRecoveryError::RecordMismatch { .. })
        ));
        store.dismiss(candidate.id).unwrap();
        assert!(store.candidate().unwrap().is_none());
    }
}
