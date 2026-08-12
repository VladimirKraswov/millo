use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const AUDIT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditLevel {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditCategory {
    Application,
    Transport,
    Controller,
    Sender,
    Safety,
    Program,
    Storage,
    Ui,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEntry {
    pub schema_version: u16,
    pub sequence: u64,
    pub session_id: String,
    pub timestamp_ms: u64,
    pub level: AuditLevel,
    pub category: AuditCategory,
    pub event: String,
    pub message: String,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLogSnapshot {
    pub entries: Vec<AuditEntry>,
    pub dropped_entries: u64,
    pub write_failures: u64,
    pub active_path: Option<PathBuf>,
    pub session_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuditExportFormat {
    JsonLines,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditExportOutcome {
    pub path: PathBuf,
    pub entry_count: usize,
}

#[derive(Debug, Clone)]
pub struct AuditConfig {
    pub recent_capacity: usize,
    pub queue_capacity: usize,
    pub max_file_bytes: u64,
    pub retained_files: usize,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            recent_capacity: 2_000,
            queue_capacity: 4_096,
            max_file_bytes: 5 * 1024 * 1024,
            retained_files: 4,
        }
    }
}

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("audit log I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("audit log JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("audit log writer is unavailable")]
    WriterUnavailable,
    #[error("audit log export response was lost")]
    ExportResponseLost,
    #[error("audit export cannot overwrite an active log file")]
    ActiveLogDestination,
}

#[derive(Clone)]
pub struct AuditLog {
    shared: Arc<Shared>,
}

struct Shared {
    sender: Mutex<Option<SyncSender<WriterCommand>>>,
    worker: Mutex<Option<thread::JoinHandle<()>>>,
    recent: Mutex<VecDeque<AuditEntry>>,
    recent_capacity: usize,
    sequence: AtomicU64,
    dropped_entries: Arc<AtomicU64>,
    write_failures: Arc<AtomicU64>,
    active_path: Option<PathBuf>,
    session_id: String,
}

impl Drop for Shared {
    fn drop(&mut self) {
        if let Ok(sender) = self.sender.get_mut() {
            sender.take();
        }
        if let Ok(worker) = self.worker.get_mut()
            && let Some(worker) = worker.take()
        {
            let _ = worker.join();
        }
    }
}

enum WriterCommand {
    Entry(AuditEntry),
    Export {
        path: PathBuf,
        format: AuditExportFormat,
        response: mpsc::Sender<Result<AuditExportOutcome, AuditError>>,
    },
}

impl AuditLog {
    pub fn in_memory() -> Self {
        Self::in_memory_with_config(AuditConfig::default())
    }

    pub fn in_memory_with_config(config: AuditConfig) -> Self {
        Self {
            shared: Arc::new(Shared {
                sender: Mutex::new(None),
                worker: Mutex::new(None),
                recent: Mutex::new(VecDeque::with_capacity(config.recent_capacity)),
                recent_capacity: config.recent_capacity,
                sequence: AtomicU64::new(1),
                dropped_entries: Arc::new(AtomicU64::new(0)),
                write_failures: Arc::new(AtomicU64::new(0)),
                active_path: None,
                session_id: session_id(),
            }),
        }
    }

    pub fn persistent(root: impl AsRef<Path>) -> Result<Self, AuditError> {
        Self::persistent_with_config(root, AuditConfig::default())
    }

    pub fn persistent_with_config(
        root: impl AsRef<Path>,
        config: AuditConfig,
    ) -> Result<Self, AuditError> {
        fs::create_dir_all(root.as_ref())?;
        let active_path = root.as_ref().join("millo-audit.jsonl");
        let recent =
            load_recent_files(&active_path, config.retained_files, config.recent_capacity)?;
        let next_sequence = recent
            .back()
            .map_or(1, |entry| entry.sequence.saturating_add(1));
        let (sender, receiver) = mpsc::sync_channel(config.queue_capacity);
        let dropped_entries = Arc::new(AtomicU64::new(0));
        let write_failures = Arc::new(AtomicU64::new(0));
        let worker_failures = Arc::clone(&write_failures);
        let worker_path = active_path.clone();
        let worker_config = config.clone();
        let worker = thread::Builder::new()
            .name("millo-audit-writer".to_owned())
            .spawn(move || {
                writer_loop(receiver, &worker_path, &worker_config, &worker_failures);
            })?;

        Ok(Self {
            shared: Arc::new(Shared {
                sender: Mutex::new(Some(sender)),
                worker: Mutex::new(Some(worker)),
                recent: Mutex::new(recent),
                recent_capacity: config.recent_capacity,
                sequence: AtomicU64::new(next_sequence),
                dropped_entries,
                write_failures,
                active_path: Some(active_path),
                session_id: session_id(),
            }),
        })
    }

    pub fn record(
        &self,
        level: AuditLevel,
        category: AuditCategory,
        event: impl Into<String>,
        message: impl Into<String>,
        data: Value,
    ) -> AuditEntry {
        let entry = AuditEntry {
            schema_version: AUDIT_SCHEMA_VERSION,
            sequence: self.shared.sequence.fetch_add(1, Ordering::Relaxed),
            session_id: self.shared.session_id.clone(),
            timestamp_ms: unix_time_ms(SystemTime::now()),
            level,
            category,
            event: bounded_text(event.into(), 96),
            message: bounded_text(message.into(), 1_024),
            data,
        };

        if let Ok(mut recent) = self.shared.recent.lock() {
            if recent.len() == self.shared.recent_capacity {
                recent.pop_front();
            }
            recent.push_back(entry.clone());
        }

        if let Ok(sender) = self.shared.sender.lock()
            && let Some(sender) = sender.as_ref()
        {
            match sender.try_send(WriterCommand::Entry(entry.clone())) {
                Ok(()) => {}
                Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
                    self.shared.dropped_entries.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        entry
    }

    pub fn snapshot(&self, limit: usize) -> AuditLogSnapshot {
        let entries = self
            .shared
            .recent
            .lock()
            .map(|recent| {
                let skip = recent.len().saturating_sub(limit);
                recent.iter().skip(skip).cloned().collect()
            })
            .unwrap_or_default();
        AuditLogSnapshot {
            entries,
            dropped_entries: self.shared.dropped_entries.load(Ordering::Relaxed),
            write_failures: self.shared.write_failures.load(Ordering::Relaxed),
            active_path: self.shared.active_path.clone(),
            session_id: self.shared.session_id.clone(),
        }
    }

    pub fn export(
        &self,
        path: impl Into<PathBuf>,
        format: AuditExportFormat,
    ) -> Result<AuditExportOutcome, AuditError> {
        let path = path.into();
        if self
            .shared
            .active_path
            .as_ref()
            .is_some_and(|active| active == &path)
        {
            return Err(AuditError::ActiveLogDestination);
        }
        let sender = self
            .shared
            .sender
            .lock()
            .map_err(|_| AuditError::WriterUnavailable)?
            .as_ref()
            .cloned();
        if let Some(sender) = sender {
            let (response, result) = mpsc::channel();
            sender
                .send(WriterCommand::Export {
                    path,
                    format,
                    response,
                })
                .map_err(|_| AuditError::WriterUnavailable)?;
            return result.recv().map_err(|_| AuditError::ExportResponseLost)?;
        }

        let entries = self.snapshot(usize::MAX).entries;
        export_entries(&path, format, entries.iter())
    }
}

fn writer_loop(
    receiver: Receiver<WriterCommand>,
    active_path: &Path,
    config: &AuditConfig,
    write_failures: &AtomicU64,
) {
    let mut writer = open_writer(active_path).ok();
    let mut bytes = fs::metadata(active_path).map_or(0, |metadata| metadata.len());

    while let Ok(command) = receiver.recv() {
        match command {
            WriterCommand::Entry(entry) => {
                let mut line = match serde_json::to_vec(&entry) {
                    Ok(line) => line,
                    Err(_) => {
                        write_failures.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                };
                line.push(b'\n');
                if bytes.saturating_add(line.len() as u64) > config.max_file_bytes {
                    if let Some(mut current) = writer.take() {
                        let _ = current.flush();
                    }
                    if rotate(active_path, config.retained_files).is_err() {
                        write_failures.fetch_add(1, Ordering::Relaxed);
                    }
                    writer = open_writer(active_path).ok();
                    bytes = 0;
                }
                if writer.is_none() {
                    writer = open_writer(active_path).ok();
                }
                let written = writer.as_mut().is_some_and(|writer| {
                    writer.write_all(&line).is_ok() && writer.flush().is_ok()
                });
                if written {
                    bytes = bytes.saturating_add(line.len() as u64);
                } else {
                    write_failures.fetch_add(1, Ordering::Relaxed);
                    writer = None;
                }
            }
            WriterCommand::Export {
                path,
                format,
                response,
            } => {
                if let Some(current) = writer.as_mut() {
                    let _ = current.flush();
                }
                let result = export_files(active_path, config.retained_files, &path, format);
                let _ = response.send(result);
            }
        }
    }
}

fn open_writer(path: &Path) -> Result<BufWriter<File>, AuditError> {
    Ok(BufWriter::new(
        OpenOptions::new().create(true).append(true).open(path)?,
    ))
}

fn rotate(active_path: &Path, retained_files: usize) -> Result<(), AuditError> {
    for index in (1..=retained_files).rev() {
        let destination = rotated_path(active_path, index);
        let source = if index == 1 {
            active_path.to_path_buf()
        } else {
            rotated_path(active_path, index - 1)
        };
        if destination.exists() {
            fs::remove_file(&destination)?;
        }
        if source.exists() {
            fs::rename(source, destination)?;
        }
    }
    Ok(())
}

fn rotated_path(active_path: &Path, index: usize) -> PathBuf {
    let name = active_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("millo-audit.jsonl");
    active_path.with_file_name(format!("{name}.{index}"))
}

fn export_files(
    active_path: &Path,
    retained_files: usize,
    destination: &Path,
    format: AuditExportFormat,
) -> Result<AuditExportOutcome, AuditError> {
    if destination == active_path
        || (1..=retained_files).any(|index| destination == rotated_path(active_path, index))
    {
        return Err(AuditError::ActiveLogDestination);
    }
    let mut entries = Vec::new();
    for index in (1..=retained_files).rev() {
        read_entries(&rotated_path(active_path, index), &mut entries)?;
    }
    read_entries(active_path, &mut entries)?;
    export_entries(destination, format, entries.iter())
}

fn export_entries<'a>(
    destination: &Path,
    format: AuditExportFormat,
    entries: impl Iterator<Item = &'a AuditEntry>,
) -> Result<AuditExportOutcome, AuditError> {
    let file = File::create(destination)?;
    let mut writer = BufWriter::new(file);
    let mut entry_count = 0;
    for entry in entries {
        match format {
            AuditExportFormat::JsonLines => serde_json::to_writer(&mut writer, entry)?,
            AuditExportFormat::Text => {
                write!(
                    writer,
                    "{} #{} {:?}/{:?} {}: {}",
                    entry.timestamp_ms,
                    entry.sequence,
                    entry.level,
                    entry.category,
                    entry.event,
                    entry.message
                )?;
                if !entry.data.is_null() {
                    write!(writer, " | {}", entry.data)?;
                }
            }
        }
        writer.write_all(b"\n")?;
        entry_count += 1;
    }
    writer.flush()?;
    Ok(AuditExportOutcome {
        path: destination.to_path_buf(),
        entry_count,
    })
}

fn read_entries(path: &Path, destination: &mut Vec<AuditEntry>) -> Result<(), AuditError> {
    if !path.exists() {
        return Ok(());
    }
    for line in BufReader::new(File::open(path)?).lines() {
        let line = line?;
        if let Ok(entry) = serde_json::from_str(&line) {
            destination.push(entry);
        }
    }
    Ok(())
}

fn load_recent_files(
    path: &Path,
    retained_files: usize,
    capacity: usize,
) -> Result<VecDeque<AuditEntry>, AuditError> {
    let mut entries = Vec::new();
    for index in (1..=retained_files).rev() {
        read_entries(&rotated_path(path, index), &mut entries)?;
    }
    read_entries(path, &mut entries)?;
    Ok(entries
        .into_iter()
        .rev()
        .take(capacity)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect())
}

fn bounded_text(mut value: String, max_chars: usize) -> String {
    if let Some((index, _)) = value.char_indices().nth(max_chars) {
        value.truncate(index);
    }
    value
}

fn session_id() -> String {
    format!("{}-{}", unix_time_ms(SystemTime::now()), std::process::id())
}

fn unix_time_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::{env, fs, time::SystemTime};

    use serde_json::json;

    use super::*;

    fn test_root(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "millo-audit-{name}-{}-{}",
            std::process::id(),
            unix_time_ms(SystemTime::now())
        ))
    }

    #[test]
    fn in_memory_log_is_bounded_and_monotonic() {
        let log = AuditLog::in_memory_with_config(AuditConfig {
            recent_capacity: 2,
            ..AuditConfig::default()
        });
        for index in 0..3 {
            log.record(
                AuditLevel::Info,
                AuditCategory::Application,
                "test.event",
                format!("event {index}"),
                json!({ "index": index }),
            );
        }

        let snapshot = log.snapshot(100);
        assert_eq!(snapshot.entries.len(), 2);
        assert_eq!(
            snapshot.entries[0].sequence + 1,
            snapshot.entries[1].sequence
        );
        assert_eq!(snapshot.entries[0].data["index"], 1);
    }

    #[test]
    fn persistent_log_rotates_restores_and_exports() {
        let root = test_root("persistent");
        let export_path = root.join("export.log");
        let config = AuditConfig {
            recent_capacity: 20,
            queue_capacity: 20,
            max_file_bytes: 320,
            retained_files: 2,
        };
        let log = AuditLog::persistent_with_config(&root, config.clone()).unwrap();
        for index in 0..8 {
            log.record(
                AuditLevel::Warning,
                AuditCategory::Sender,
                "sender.test",
                format!("line {index}"),
                json!({ "sourceLine": index }),
            );
        }
        let exported = log.export(&export_path, AuditExportFormat::Text).unwrap();
        assert!(exported.entry_count > 0);
        assert!(
            fs::read_to_string(&export_path)
                .unwrap()
                .contains("sender.test")
        );
        drop(log);

        let restored = AuditLog::persistent_with_config(&root, config).unwrap();
        let before = restored.snapshot(20).entries.last().unwrap().sequence;
        let next = restored.record(
            AuditLevel::Info,
            AuditCategory::Application,
            "application.restored",
            "restored",
            Value::Null,
        );
        assert!(next.sequence > before);
        drop(restored);
        fs::remove_dir_all(root).unwrap();
    }
}
