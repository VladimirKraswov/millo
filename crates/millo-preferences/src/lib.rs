use std::{
    fs, io,
    path::{Path, PathBuf},
};

use millo_storage::{backup_path, write_atomically};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationPreferences {
    pub safe_command_mode: bool,
}

impl Default for ApplicationPreferences {
    fn default() -> Self {
        Self {
            safe_command_mode: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationPreferencesUpdate {
    pub safe_command_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredPreferences {
    schema_version: u16,
    preferences: ApplicationPreferences,
}

impl Default for StoredPreferences {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            preferences: ApplicationPreferences::default(),
        }
    }
}

pub struct ApplicationPreferencesStore {
    path: Option<PathBuf>,
    document: StoredPreferences,
}

impl ApplicationPreferencesStore {
    pub fn load(path: impl Into<PathBuf>) -> Result<Self, PreferencesError> {
        let path = path.into();
        let document = match read_document(&path) {
            Ok(Some(document)) => document,
            Ok(None) => StoredPreferences::default(),
            Err(primary) => match read_document(&backup_path(&path)) {
                Ok(Some(document)) => {
                    persist(&path, &document)?;
                    document
                }
                Ok(None) => return Err(primary),
                Err(backup) => {
                    return Err(PreferencesError::CorruptCopies {
                        primary: primary.to_string(),
                        backup: backup.to_string(),
                    });
                }
            },
        };
        validate(&document)?;
        Ok(Self {
            path: Some(path),
            document,
        })
    }

    pub fn in_memory() -> Self {
        Self {
            path: None,
            document: StoredPreferences::default(),
        }
    }

    pub fn preferences(&self) -> ApplicationPreferences {
        self.document.preferences
    }

    pub fn update(
        &mut self,
        update: ApplicationPreferencesUpdate,
    ) -> Result<ApplicationPreferences, PreferencesError> {
        let next = StoredPreferences {
            schema_version: SCHEMA_VERSION,
            preferences: ApplicationPreferences {
                safe_command_mode: update.safe_command_mode,
            },
        };
        if let Some(path) = &self.path {
            persist(path, &next)?;
        }
        self.document = next;
        Ok(self.document.preferences)
    }
}

#[derive(Debug, Error)]
pub enum PreferencesError {
    #[error("unsupported application-preferences schema version: {0}")]
    UnsupportedSchema(u16),
    #[error("invalid application-preferences file: {0}")]
    InvalidFile(serde_json::Error),
    #[error(
        "application-preferences primary and backup are corrupt: primary: {primary}; backup: {backup}"
    )]
    CorruptCopies { primary: String, backup: String },
    #[error("application-preferences I/O failed for {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

fn validate(document: &StoredPreferences) -> Result<(), PreferencesError> {
    if document.schema_version != SCHEMA_VERSION {
        return Err(PreferencesError::UnsupportedSchema(document.schema_version));
    }
    Ok(())
}

fn read_document(path: &Path) -> Result<Option<StoredPreferences>, PreferencesError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(PreferencesError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    let document = serde_json::from_slice(&bytes).map_err(PreferencesError::InvalidFile)?;
    validate(&document)?;
    Ok(Some(document))
}

fn persist(path: &Path, document: &StoredPreferences) -> Result<(), PreferencesError> {
    let bytes = serde_json::to_vec_pretty(document).map_err(PreferencesError::InvalidFile)?;
    write_atomically(path, &bytes).map_err(|source| PreferencesError::Io {
        path: path.to_path_buf(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static TEST_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn defaults_to_safe_command_mode() {
        assert!(
            ApplicationPreferencesStore::in_memory()
                .preferences()
                .safe_command_mode
        );
    }

    #[test]
    fn persists_expert_mode_and_restores_it() {
        let path = test_path("persist");
        let mut store = ApplicationPreferencesStore::load(&path).unwrap();
        store
            .update(ApplicationPreferencesUpdate {
                safe_command_mode: false,
            })
            .unwrap();

        let restored = ApplicationPreferencesStore::load(&path).unwrap();
        assert!(!restored.preferences().safe_command_mode);
        cleanup(&path);
    }

    #[test]
    fn restores_the_previous_valid_copy_when_primary_is_corrupt() {
        let path = test_path("backup");
        let mut store = ApplicationPreferencesStore::load(&path).unwrap();
        store
            .update(ApplicationPreferencesUpdate {
                safe_command_mode: false,
            })
            .unwrap();
        store
            .update(ApplicationPreferencesUpdate {
                safe_command_mode: true,
            })
            .unwrap();
        fs::write(&path, b"not json").unwrap();

        let restored = ApplicationPreferencesStore::load(&path).unwrap();
        assert!(!restored.preferences().safe_command_mode);
        cleanup(&path);
    }

    fn test_path(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!(
                "millo-preferences-{}-{timestamp}-{}",
                std::process::id(),
                TEST_ID.fetch_add(1, Ordering::Relaxed)
            ))
            .join(format!("{label}.json"))
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(backup_path(path));
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}
