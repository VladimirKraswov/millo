use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use millo_sender::{SenderFailure, SenderMode, SenderSnapshot, SenderState};
use millo_storage::{backup_path, write_atomically};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const JOURNAL_SCHEMA_VERSION: u16 = 1;
const DEFAULT_MAX_ENTRIES: usize = 100;
const ACK_CHECKPOINT_INTERVAL: usize = 250;
const TIME_CHECKPOINT_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecoveryDisposition {
    CheckpointOnly,
    NotRequired,
    RestartBlocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunJournalEntry {
    pub application_session: u64,
    pub run_sequence: u64,
    pub source_name: String,
    pub mode: SenderMode,
    pub state: SenderState,
    pub created_at_unix_ms: u64,
    pub updated_at_unix_ms: u64,
    pub total_lines: usize,
    pub dispatched_lines: usize,
    pub acknowledged_lines: usize,
    pub last_acknowledged_source_line: Option<usize>,
    pub last_acknowledged_command: Option<String>,
    pub current_source_line: Option<usize>,
    pub current_command: Option<String>,
    pub shutdown_commands_acknowledged: bool,
    pub elapsed_seconds: f64,
    pub failure: Option<SenderFailure>,
    pub recovery: RecoveryDisposition,
    pub recovery_detail: String,
}

impl RunJournalEntry {
    fn from_snapshot(
        application_session: u64,
        snapshot: &SenderSnapshot,
        now_unix_ms: u64,
    ) -> Option<Self> {
        Some(Self {
            application_session,
            run_sequence: snapshot.run_sequence,
            source_name: snapshot.source_name.clone()?,
            mode: snapshot.mode?,
            state: snapshot.state,
            created_at_unix_ms: now_unix_ms,
            updated_at_unix_ms: now_unix_ms,
            total_lines: snapshot.total_lines,
            dispatched_lines: snapshot.dispatched_lines,
            acknowledged_lines: snapshot.acknowledged_lines,
            last_acknowledged_source_line: snapshot.last_acknowledged_source_line,
            last_acknowledged_command: snapshot.last_acknowledged_command.clone(),
            current_source_line: snapshot.current_source_line,
            current_command: snapshot.current_command.clone(),
            shutdown_commands_acknowledged: snapshot.shutdown_commands_acknowledged,
            elapsed_seconds: snapshot.elapsed_seconds,
            failure: snapshot.failure.clone(),
            recovery: recovery_disposition(snapshot.state),
            recovery_detail: recovery_detail(snapshot.state).to_owned(),
        })
    }

    fn update(&mut self, snapshot: &SenderSnapshot, now_unix_ms: u64) {
        self.state = snapshot.state;
        self.updated_at_unix_ms = now_unix_ms;
        self.total_lines = snapshot.total_lines;
        self.dispatched_lines = snapshot.dispatched_lines;
        self.acknowledged_lines = snapshot.acknowledged_lines;
        self.last_acknowledged_source_line = snapshot.last_acknowledged_source_line;
        self.last_acknowledged_command = snapshot.last_acknowledged_command.clone();
        self.current_source_line = snapshot.current_source_line;
        self.current_command = snapshot.current_command.clone();
        self.shutdown_commands_acknowledged = snapshot.shutdown_commands_acknowledged;
        self.elapsed_seconds = snapshot.elapsed_seconds;
        self.failure = snapshot.failure.clone();
        self.recovery = recovery_disposition(snapshot.state);
        self.recovery_detail = recovery_detail(snapshot.state).to_owned();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JournalFile {
    schema_version: u16,
    entries: Vec<RunJournalEntry>,
}

struct LoadedJournal {
    entries: Vec<RunJournalEntry>,
    recovered_from_backup: bool,
}

#[derive(Debug, Clone)]
struct PersistedCheckpoint {
    run_sequence: u64,
    state: SenderState,
    acknowledged_lines: usize,
    persisted_at: Instant,
    terminal: bool,
}

#[derive(Debug, Error)]
pub enum RunJournalError {
    #[error("sender journal I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("sender journal JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported sender journal schema {0}")]
    UnsupportedSchema(u16),
}

pub struct RunJournal {
    path: Option<PathBuf>,
    max_entries: usize,
    application_session: u64,
    entries: Vec<RunJournalEntry>,
    checkpoint: Option<PersistedCheckpoint>,
}

impl RunJournal {
    pub fn in_memory() -> Self {
        Self::new(None, DEFAULT_MAX_ENTRIES, SystemTime::now())
    }

    pub fn load(path: impl Into<PathBuf>) -> Result<Self, RunJournalError> {
        let path = path.into();
        let loaded = load_file_with_backup(&path)?;
        let mut journal = Self::new(Some(path), DEFAULT_MAX_ENTRIES, SystemTime::now());
        journal.entries = loaded
            .as_ref()
            .map(|loaded| loaded.entries.clone())
            .unwrap_or_default();
        journal.trim();
        if loaded.is_some_and(|loaded| loaded.recovered_from_backup) {
            journal.remove_corrupt_primary()?;
            journal.persist()?;
        }
        Ok(journal)
    }

    fn new(path: Option<PathBuf>, max_entries: usize, now: SystemTime) -> Self {
        Self {
            path,
            max_entries,
            application_session: unix_micros(now),
            entries: Vec::new(),
            checkpoint: None,
        }
    }

    pub fn entries(&self) -> &[RunJournalEntry] {
        &self.entries
    }

    pub fn observe(
        &mut self,
        snapshot: &SenderSnapshot,
        wall_clock: SystemTime,
        monotonic: Instant,
    ) -> Result<bool, RunJournalError> {
        if snapshot.run_sequence == 0
            || snapshot.mode.is_none()
            || snapshot.source_name.is_none()
            || matches!(snapshot.state, SenderState::Idle | SenderState::Ready)
        {
            return Ok(false);
        }

        let now_unix_ms = unix_millis(wall_clock);
        let index = self.entries.iter().position(|entry| {
            entry.application_session == self.application_session
                && entry.run_sequence == snapshot.run_sequence
        });
        let new_run = index.is_none();
        let index = if let Some(index) = index {
            self.entries[index].update(snapshot, now_unix_ms);
            index
        } else {
            let Some(entry) =
                RunJournalEntry::from_snapshot(self.application_session, snapshot, now_unix_ms)
            else {
                return Ok(false);
            };
            self.entries.push(entry);
            self.entries.len() - 1
        };

        let terminal = is_terminal(snapshot.state);
        let should_persist = new_run
            || self.checkpoint.as_ref().is_none_or(|checkpoint| {
                checkpoint.run_sequence != snapshot.run_sequence
                    || checkpoint.state != snapshot.state
                    || snapshot
                        .acknowledged_lines
                        .saturating_sub(checkpoint.acknowledged_lines)
                        >= ACK_CHECKPOINT_INTERVAL
                    || (!terminal
                        && monotonic.duration_since(checkpoint.persisted_at)
                            >= TIME_CHECKPOINT_INTERVAL)
                    || (terminal && !checkpoint.terminal)
            });
        if !should_persist {
            return Ok(false);
        }

        self.entries[index].updated_at_unix_ms = now_unix_ms;
        self.trim();
        self.persist()?;
        self.checkpoint = Some(PersistedCheckpoint {
            run_sequence: snapshot.run_sequence,
            state: snapshot.state,
            acknowledged_lines: snapshot.acknowledged_lines,
            persisted_at: monotonic,
            terminal,
        });
        Ok(true)
    }

    fn trim(&mut self) {
        if self.entries.len() > self.max_entries {
            self.entries
                .drain(0..self.entries.len().saturating_sub(self.max_entries));
        }
    }

    fn persist(&self) -> Result<(), RunJournalError> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };
        let bytes = serde_json::to_vec_pretty(&JournalFile {
            schema_version: JOURNAL_SCHEMA_VERSION,
            entries: self.entries.clone(),
        })?;
        write_atomically(path, &bytes)?;
        Ok(())
    }

    fn remove_corrupt_primary(&self) -> Result<(), RunJournalError> {
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

fn load_file_with_backup(path: &Path) -> Result<Option<LoadedJournal>, RunJournalError> {
    let mut parse_error = None;
    for (index, candidate) in [path.to_path_buf(), backup_path(path)]
        .into_iter()
        .enumerate()
    {
        if !candidate.exists() {
            continue;
        }
        let bytes = fs::read(candidate)?;
        let file: JournalFile = match serde_json::from_slice(&bytes) {
            Ok(file) => file,
            Err(error) => {
                parse_error = Some(error);
                continue;
            }
        };
        if file.schema_version != JOURNAL_SCHEMA_VERSION {
            return Err(RunJournalError::UnsupportedSchema(file.schema_version));
        }
        return Ok(Some(LoadedJournal {
            entries: file.entries,
            recovered_from_backup: index == 1,
        }));
    }
    if let Some(error) = parse_error {
        return Err(error.into());
    }
    Ok(None)
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

fn is_terminal(state: SenderState) -> bool {
    matches!(
        state,
        SenderState::Completed | SenderState::Failed | SenderState::Cancelled
    )
}

fn recovery_disposition(state: SenderState) -> RecoveryDisposition {
    match state {
        SenderState::Completed => RecoveryDisposition::NotRequired,
        SenderState::Failed | SenderState::Cancelled => RecoveryDisposition::RestartBlocked,
        _ => RecoveryDisposition::CheckpointOnly,
    }
}

fn recovery_detail(state: SenderState) -> &'static str {
    match state {
        SenderState::Completed => "Program completed; no recovery is required",
        SenderState::Failed | SenderState::Cancelled => {
            "Automatic restart from a line is blocked; re-establish machine position, modal state and a safe approach before creating a new authorization"
        }
        _ => "Diagnostic checkpoint only; it is not an executable resume token",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(sequence: u64, state: SenderState, acknowledged: usize) -> SenderSnapshot {
        SenderSnapshot {
            run_sequence: sequence,
            state,
            mode: Some(SenderMode::CutRun),
            source_name: Some("complex.nc".to_owned()),
            total_lines: 1_000,
            dispatched_lines: acknowledged.saturating_add(10),
            acknowledged_lines: acknowledged,
            last_acknowledged_source_line: Some(acknowledged),
            last_acknowledged_command: Some(format!("G1 X{acknowledged}")),
            ..SenderSnapshot::default()
        }
    }

    #[test]
    fn checkpoints_are_throttled_but_terminal_state_is_always_persisted() {
        let now = Instant::now();
        let wall = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let mut journal = RunJournal::new(None, 100, wall);

        assert!(
            journal
                .observe(&snapshot(1, SenderState::Running, 0), wall, now)
                .unwrap()
        );
        assert!(
            !journal
                .observe(
                    &snapshot(1, SenderState::Running, 20),
                    wall + Duration::from_millis(50),
                    now + Duration::from_millis(50),
                )
                .unwrap()
        );
        assert!(
            journal
                .observe(
                    &snapshot(1, SenderState::Running, ACK_CHECKPOINT_INTERVAL),
                    wall + Duration::from_secs(1),
                    now + Duration::from_secs(1),
                )
                .unwrap()
        );
        let mut completed = snapshot(1, SenderState::Completed, 1_000);
        completed.shutdown_commands_acknowledged = true;
        assert!(
            journal
                .observe(
                    &completed,
                    wall + Duration::from_secs(2),
                    now + Duration::from_secs(2),
                )
                .unwrap()
        );
        assert_eq!(
            journal.entries()[0].recovery,
            RecoveryDisposition::NotRequired
        );
        assert!(journal.entries()[0].shutdown_commands_acknowledged);
        assert!(
            !journal
                .observe(
                    &completed,
                    wall + Duration::from_secs(20),
                    now + Duration::from_secs(20),
                )
                .unwrap()
        );
    }

    #[test]
    fn failed_runs_retain_the_exact_line_but_never_become_resume_tokens() {
        let now = Instant::now();
        let wall = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let mut failed = snapshot(7, SenderState::Failed, 42);
        failed.current_source_line = Some(43);
        failed.current_command = Some("G2 X10 Y10 I5 J0".to_owned());
        failed.failure = Some(SenderFailure {
            kind: millo_sender::SenderFailureKind::GrblError,
            message: "error:33".to_owned(),
            grbl_code: Some(33),
            source_line: Some(43),
            command: failed.current_command.clone(),
        });
        let mut journal = RunJournal::new(None, 100, wall);

        journal.observe(&failed, wall, now).unwrap();

        let entry = &journal.entries()[0];
        assert_eq!(entry.current_source_line, Some(43));
        assert_eq!(entry.recovery, RecoveryDisposition::RestartBlocked);
        assert!(entry.recovery_detail.contains("Automatic restart"));
    }

    #[test]
    fn persistent_journal_is_bounded_and_recovers_from_the_backup() {
        let unique = format!(
            "millo-run-journal-{}-{}.json",
            std::process::id(),
            unix_micros(SystemTime::now())
        );
        let path = std::env::temp_dir().join(unique);
        let wall = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let now = Instant::now();
        let mut journal = RunJournal::new(Some(path.clone()), 2, wall);
        for sequence in 1..=3 {
            journal
                .observe(
                    &snapshot(sequence, SenderState::Completed, 1_000),
                    wall + Duration::from_secs(sequence),
                    now + Duration::from_secs(sequence),
                )
                .unwrap();
        }
        assert_eq!(journal.entries().len(), 2);

        fs::write(&path, b"corrupt").unwrap();
        let loaded = RunJournal::load(&path).unwrap();
        assert!(!loaded.entries().is_empty());

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup_path(&path));
        let _ = fs::remove_file(millo_storage::temporary_path(&path));
    }

    #[test]
    fn corrupt_primary_and_backup_do_not_silently_erase_history() {
        let unique = format!(
            "millo-corrupt-run-journal-{}-{}.json",
            std::process::id(),
            unix_micros(SystemTime::now())
        );
        let path = std::env::temp_dir().join(unique);
        fs::write(&path, b"corrupt primary").unwrap();
        fs::write(backup_path(&path), b"corrupt backup").unwrap();

        assert!(matches!(
            RunJournal::load(&path),
            Err(RunJournalError::Json(_))
        ));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup_path(&path));
    }
}
