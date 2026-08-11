use std::{
    collections::BTreeMap,
    fs, io,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use millo_domain::{CommandResponse, DeviceInspection, MachineTravel};
use millo_storage::{backup_path, write_atomically};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_VALUE_BYTES: usize = 32;
const ARCHIVE_SCHEMA_VERSION: u16 = 1;
const MAX_REVISIONS: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingGroup {
    Interface,
    Pins,
    Safety,
    Homing,
    Spindle,
    Calibration,
    Motion,
    Travel,
    Advanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingKind {
    Boolean,
    Integer,
    Decimal,
    Mask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerSettingValue {
    pub key: String,
    pub value: String,
    pub title: String,
    pub group: SettingGroup,
    pub kind: SettingKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub known: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerSettingsSnapshot {
    pub revision: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_build_info: Option<String>,
    pub values: Vec<ControllerSettingValue>,
}

impl ControllerSettingsSnapshot {
    pub fn value(&self, key: &str) -> Option<&str> {
        self.values
            .iter()
            .find(|setting| setting.key == key)
            .map(|setting| setting.value.as_str())
    }

    pub fn travel_mm(&self) -> Option<MachineTravel> {
        Some(MachineTravel {
            x: positive_value(self.value("$130")?)?,
            y: positive_value(self.value("$131")?)?,
            z: positive_value(self.value("$132")?)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerSettingEditRequest {
    pub key: String,
    pub value: String,
    pub confirmed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedSettingUpdate {
    pub key: String,
    pub before_value: String,
    pub stored_value: String,
    pub write: CommandResponse,
    pub inspection: DeviceInspection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSettingWrite {
    key: String,
    value: String,
    command: String,
}

impl ValidatedSettingWrite {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn command(&self) -> &str {
        &self.command
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SettingsError {
    #[error("controller setting change requires explicit confirmation")]
    ConfirmationRequired,
    #[error("controller did not report setting {0}")]
    UnknownSetting(String),
    #[error("controller setting {key} changed externally: expected {expected}, read {actual}")]
    StaleControllerValue {
        key: String,
        expected: String,
        actual: String,
    },
    #[error("invalid controller setting key: {0}")]
    InvalidKey(String),
    #[error("invalid value for {key}: {value}")]
    InvalidValue { key: String, value: String },
}

pub fn build_settings_snapshot(
    inspection: &DeviceInspection,
    revision: u64,
) -> ControllerSettingsSnapshot {
    let mut values = inspection
        .settings
        .iter()
        .filter_map(|(key, value)| {
            let number = setting_number(key)?;
            let definition = definition(number);
            Some(ControllerSettingValue {
                key: key.clone(),
                value: value.clone(),
                title: definition.map_or_else(
                    || format!("Firmware setting {number}"),
                    |known| known.title.to_owned(),
                ),
                group: definition.map_or(SettingGroup::Advanced, |known| known.group),
                kind: definition.map_or(SettingKind::Decimal, |known| known.kind),
                unit: definition.and_then(|known| known.unit).map(str::to_owned),
                known: definition.is_some(),
            })
        })
        .collect::<Vec<_>>();
    values.sort_by_key(|setting| setting_number(&setting.key).unwrap_or(u16::MAX));
    ControllerSettingsSnapshot {
        revision,
        firmware_version: inspection.firmware_version.clone(),
        firmware_build_info: inspection.firmware_build_info.clone(),
        values,
    }
}

pub fn validate_setting_edit(
    request: ControllerSettingEditRequest,
    current: &DeviceInspection,
) -> Result<ValidatedSettingWrite, SettingsError> {
    if !request.confirmed {
        return Err(SettingsError::ConfirmationRequired);
    }
    let number = setting_number(&request.key)
        .ok_or_else(|| SettingsError::InvalidKey(request.key.clone()))?;
    let current_value = current
        .settings
        .get(&request.key)
        .ok_or_else(|| SettingsError::UnknownSetting(request.key.clone()))?;
    if let Some(expected) = request.expected_value.as_deref()
        && !setting_values_equal(expected, current_value)
    {
        return Err(SettingsError::StaleControllerValue {
            key: request.key,
            expected: expected.to_owned(),
            actual: current_value.clone(),
        });
    }
    let value = request.value.trim();
    let kind = definition(number).map_or(SettingKind::Decimal, |known| known.kind);
    let valid = !value.is_empty()
        && value.len() <= MAX_VALUE_BYTES
        && match kind {
            SettingKind::Boolean => matches!(value, "0" | "1"),
            SettingKind::Integer => value.chars().all(|character| character.is_ascii_digit()),
            SettingKind::Mask => value
                .parse::<u16>()
                .is_ok_and(|parsed| parsed <= u8::MAX.into()),
            SettingKind::Decimal => valid_non_negative_decimal(value),
        }
        && (!(130..=132).contains(&number)
            || value.parse::<f64>().is_ok_and(|parsed| parsed > 0.0));
    if !valid {
        return Err(SettingsError::InvalidValue {
            key: request.key,
            value: request.value,
        });
    }
    Ok(ValidatedSettingWrite {
        key: request.key.clone(),
        value: value.to_owned(),
        command: format!("{}={value}", request.key),
    })
}

pub fn setting_values_equal(requested: &str, stored: &str) -> bool {
    match (requested.parse::<f64>(), stored.parse::<f64>()) {
        (Ok(requested), Ok(stored)) => (requested - stored).abs() <= 0.000_001,
        _ => requested == stored,
    }
}

#[derive(Debug, Clone, Copy)]
struct SettingDefinition {
    title: &'static str,
    group: SettingGroup,
    kind: SettingKind,
    unit: Option<&'static str>,
}

const fn known(
    title: &'static str,
    group: SettingGroup,
    kind: SettingKind,
    unit: Option<&'static str>,
) -> SettingDefinition {
    SettingDefinition {
        title,
        group,
        kind,
        unit,
    }
}

fn definition(number: u16) -> Option<SettingDefinition> {
    use SettingGroup::{Calibration, Homing, Interface, Motion, Pins, Safety, Spindle, Travel};
    use SettingKind::{Boolean, Decimal, Integer, Mask};
    Some(match number {
        0 => known("Step pulse time", Interface, Integer, Some("us")),
        1 => known("Step idle delay", Interface, Integer, Some("ms")),
        2 => known("Step pulse invert", Pins, Mask, None),
        3 => known("Step direction invert", Pins, Mask, None),
        4 => known("Invert step enable pin", Pins, Boolean, None),
        5 => known("Invert limit pins", Pins, Boolean, None),
        6 => known("Invert probe pin", Pins, Boolean, None),
        10 => known("Status report options", Interface, Mask, None),
        11 => known("Junction deviation", Motion, Decimal, Some("mm")),
        12 => known("Arc tolerance", Motion, Decimal, Some("mm")),
        13 => known("Report in inches", Interface, Boolean, None),
        20 => known("Soft limits", Safety, Boolean, None),
        21 => known("Hard limits", Safety, Boolean, None),
        22 => known("Homing cycle", Homing, Boolean, None),
        23 => known("Homing direction invert", Homing, Mask, None),
        24 => known("Homing feed", Homing, Decimal, Some("mm/min")),
        25 => known("Homing seek", Homing, Decimal, Some("mm/min")),
        26 => known("Homing debounce", Homing, Integer, Some("ms")),
        27 => known("Homing pull-off", Homing, Decimal, Some("mm")),
        30 => known("Maximum spindle speed", Spindle, Decimal, Some("rpm")),
        31 => known("Minimum spindle speed", Spindle, Decimal, Some("rpm")),
        32 => known("Laser mode", Spindle, Boolean, None),
        100 => known(
            "X steps per millimeter",
            Calibration,
            Decimal,
            Some("step/mm"),
        ),
        101 => known(
            "Y steps per millimeter",
            Calibration,
            Decimal,
            Some("step/mm"),
        ),
        102 => known(
            "Z steps per millimeter",
            Calibration,
            Decimal,
            Some("step/mm"),
        ),
        110 => known("X maximum rate", Motion, Decimal, Some("mm/min")),
        111 => known("Y maximum rate", Motion, Decimal, Some("mm/min")),
        112 => known("Z maximum rate", Motion, Decimal, Some("mm/min")),
        120 => known("X acceleration", Motion, Decimal, Some("mm/s^2")),
        121 => known("Y acceleration", Motion, Decimal, Some("mm/s^2")),
        122 => known("Z acceleration", Motion, Decimal, Some("mm/s^2")),
        130 => known("X maximum travel", Travel, Decimal, Some("mm")),
        131 => known("Y maximum travel", Travel, Decimal, Some("mm")),
        132 => known("Z maximum travel", Travel, Decimal, Some("mm")),
        _ => return None,
    })
}

fn setting_number(key: &str) -> Option<u16> {
    key.strip_prefix('$')?.parse().ok()
}

fn valid_non_negative_decimal(value: &str) -> bool {
    let mut decimal_points = 0;
    let lexical = value.chars().all(|character| {
        if character == '.' {
            decimal_points += 1;
            decimal_points <= 1
        } else {
            character.is_ascii_digit()
        }
    });
    lexical
        && value
            .parse::<f64>()
            .is_ok_and(|parsed| parsed.is_finite() && (0.0..=1_000_000_000.0).contains(&parsed))
}

fn positive_value(value: &str) -> Option<f64> {
    value
        .parse::<f64>()
        .ok()
        .filter(|parsed| parsed.is_finite() && *parsed > 0.0)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsRevision {
    pub id: u64,
    pub captured_at_unix_ms: u64,
    pub values: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSettingsSession {
    pub baseline: BTreeMap<String, String>,
    pub current: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineSettingsArchiveState {
    pub machine_profile_id: String,
    pub identity_fingerprint: String,
    pub active: ActiveSettingsSession,
    pub revisions: Vec<SettingsRevision>,
}

impl MachineSettingsArchiveState {
    pub fn baseline_value(&self, key: &str) -> Option<&str> {
        self.active.baseline.get(key).map(String::as_str)
    }

    pub fn previous_value(&self, key: &str) -> Option<&str> {
        self.revisions
            .last()
            .and_then(|revision| revision.values.get(key))
            .map(String::as_str)
    }
}

#[derive(Debug)]
pub struct MachineSettingsArchive {
    path: PathBuf,
    state: MachineSettingsArchiveState,
}

impl MachineSettingsArchive {
    pub fn begin(
        path: impl Into<PathBuf>,
        machine_profile_id: impl Into<String>,
        identity_fingerprint: impl Into<String>,
        inspection: &DeviceInspection,
    ) -> Result<Self, ArchiveError> {
        let path = path.into();
        let machine_profile_id = machine_profile_id.into();
        let identity_fingerprint = identity_fingerprint.into();
        let observed = inspection.settings.clone();
        let backup = backup_path(&path);
        let mut recovered_from_backup = false;
        let mut state = if path.exists() || backup.exists() {
            let stored = if path.exists() {
                match load_archive(&path) {
                    Ok(stored) => stored,
                    Err(ArchiveError::InvalidFile(primary)) if backup.exists() => {
                        match load_archive(&backup) {
                            Ok(stored) => {
                                recovered_from_backup = true;
                                stored
                            }
                            Err(ArchiveError::InvalidFile(backup)) => {
                                return Err(ArchiveError::CorruptCopies { primary, backup });
                            }
                            Err(error) => return Err(error),
                        }
                    }
                    Err(error) => return Err(error),
                }
            } else {
                recovered_from_backup = true;
                load_archive(&backup)?
            };
            if stored.schema_version != ARCHIVE_SCHEMA_VERSION {
                return Err(ArchiveError::UnsupportedSchema(stored.schema_version));
            }
            if stored.state.machine_profile_id != machine_profile_id {
                return Err(ArchiveError::ProfileMismatch {
                    expected: machine_profile_id,
                    actual: stored.state.machine_profile_id,
                });
            }
            stored.state
        } else {
            MachineSettingsArchiveState {
                machine_profile_id,
                identity_fingerprint: identity_fingerprint.clone(),
                active: ActiveSettingsSession {
                    baseline: observed.clone(),
                    current: observed.clone(),
                },
                revisions: Vec::new(),
            }
        };

        if state.active.baseline != state.active.current || state.active.current != observed {
            let baseline = state.active.baseline.clone();
            let duplicate = state
                .revisions
                .last()
                .is_some_and(|revision| revision.values == baseline);
            if !duplicate {
                state.revisions.push(SettingsRevision {
                    id: next_revision_id(&state.revisions),
                    captured_at_unix_ms: unix_time_ms(),
                    values: baseline,
                });
                if state.revisions.len() > MAX_REVISIONS {
                    state.revisions.remove(0);
                }
            }
        }
        state.identity_fingerprint = identity_fingerprint;
        state.active = ActiveSettingsSession {
            baseline: observed.clone(),
            current: observed,
        };

        let archive = Self { path, state };
        if recovered_from_backup {
            archive.remove_corrupt_primary()?;
        }
        archive.persist()?;
        Ok(archive)
    }

    pub fn state(&self) -> &MachineSettingsArchiveState {
        &self.state
    }

    pub fn record_verified_change(
        &mut self,
        key: &str,
        before: &str,
        after: &str,
    ) -> Result<(), ArchiveError> {
        let current = self
            .state
            .active
            .current
            .get(key)
            .ok_or_else(|| ArchiveError::UnknownSetting(key.to_owned()))?;
        if !setting_values_equal(current, before) {
            return Err(ArchiveError::StaleSetting {
                key: key.to_owned(),
                expected: current.clone(),
                actual: before.to_owned(),
            });
        }
        self.state
            .active
            .current
            .insert(key.to_owned(), after.to_owned());
        self.persist()
    }

    pub fn record_observation(
        &mut self,
        inspection: &DeviceInspection,
    ) -> Result<(), ArchiveError> {
        self.state.active.current = inspection.settings.clone();
        self.persist()
    }

    fn persist(&self) -> Result<(), ArchiveError> {
        let bytes = serde_json::to_vec_pretty(&StoredSettingsArchive {
            schema_version: ARCHIVE_SCHEMA_VERSION,
            state: self.state.clone(),
        })
        .map_err(ArchiveError::InvalidFile)?;
        write_atomically(&self.path, &bytes).map_err(|source| ArchiveError::Io {
            path: self.path.clone(),
            source,
        })
    }

    fn remove_corrupt_primary(&self) -> Result<(), ArchiveError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(ArchiveError::Io {
                path: self.path.clone(),
                source,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredSettingsArchive {
    schema_version: u16,
    state: MachineSettingsArchiveState,
}

#[derive(Debug, Error)]
pub enum ArchiveError {
    #[error("unsupported settings archive schema: {0}")]
    UnsupportedSchema(u16),
    #[error("settings archive belongs to {actual}, expected {expected}")]
    ProfileMismatch { expected: String, actual: String },
    #[error("settings archive does not contain {0}")]
    UnknownSetting(String),
    #[error("stale settings cache for {key}: expected {expected}, got {actual}")]
    StaleSetting {
        key: String,
        expected: String,
        actual: String,
    },
    #[error("invalid settings archive: {0}")]
    InvalidFile(serde_json::Error),
    #[error("settings primary and backup are corrupt: primary: {primary}; backup: {backup}")]
    CorruptCopies {
        primary: serde_json::Error,
        backup: serde_json::Error,
    },
    #[error("settings archive I/O failed for {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

fn load_archive(path: &std::path::Path) -> Result<StoredSettingsArchive, ArchiveError> {
    let bytes = fs::read(path).map_err(|source| ArchiveError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(ArchiveError::InvalidFile)
}

fn next_revision_id(revisions: &[SettingsRevision]) -> u64 {
    revisions
        .last()
        .map_or(1, |revision| revision.id.saturating_add(1))
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static TEST_ID: AtomicU64 = AtomicU64::new(1);

    fn inspection() -> DeviceInspection {
        DeviceInspection {
            firmware_version: Some("1.1f".to_owned()),
            settings: BTreeMap::from([
                ("$21".to_owned(), "0".to_owned()),
                ("$100".to_owned(), "1600.000".to_owned()),
                ("$130".to_owned(), "500.000".to_owned()),
                ("$131".to_owned(), "500.000".to_owned()),
                ("$132".to_owned(), "200.000".to_owned()),
                ("$200".to_owned(), "7.5".to_owned()),
            ]),
            ..Default::default()
        }
    }

    #[test]
    fn catalogs_every_reported_setting_and_retains_unknown_firmware_keys() {
        let snapshot = build_settings_snapshot(&inspection(), 3);

        assert_eq!(snapshot.revision, 3);
        assert_eq!(snapshot.values.len(), 6);
        assert_eq!(snapshot.value("$100"), Some("1600.000"));
        assert_eq!(snapshot.travel_mm().unwrap().z, 200.0);
        let unknown = snapshot
            .values
            .iter()
            .find(|setting| setting.key == "$200")
            .unwrap();
        assert_eq!(unknown.group, SettingGroup::Advanced);
        assert!(!unknown.known);
    }

    #[test]
    fn validates_only_confirmed_numeric_settings_reported_by_the_controller() {
        let current = inspection();
        assert_eq!(
            validate_setting_edit(
                ControllerSettingEditRequest {
                    key: "$21".to_owned(),
                    value: "1".to_owned(),
                    confirmed: false,
                    expected_value: None,
                    expected_revision: None,
                },
                &current,
            ),
            Err(SettingsError::ConfirmationRequired)
        );
        assert!(matches!(
            validate_setting_edit(
                ControllerSettingEditRequest {
                    key: "$999".to_owned(),
                    value: "1".to_owned(),
                    confirmed: true,
                    expected_value: None,
                    expected_revision: None,
                },
                &current,
            ),
            Err(SettingsError::UnknownSetting(_))
        ));
        assert!(matches!(
            validate_setting_edit(
                ControllerSettingEditRequest {
                    key: "$21".to_owned(),
                    value: "2".to_owned(),
                    confirmed: true,
                    expected_value: None,
                    expected_revision: None,
                },
                &current,
            ),
            Err(SettingsError::InvalidValue { .. })
        ));

        let write = validate_setting_edit(
            ControllerSettingEditRequest {
                key: "$100".to_owned(),
                value: " 1601.25 ".to_owned(),
                confirmed: true,
                expected_value: Some("1600".to_owned()),
                expected_revision: None,
            },
            &current,
        )
        .unwrap();
        assert_eq!(write.command(), "$100=1601.25");
    }

    #[test]
    fn compares_controller_number_formatting_semantically() {
        assert!(setting_values_equal("500", "500.000"));
        assert!(!setting_values_equal("500.1", "500.000"));
    }

    #[test]
    fn rejects_zero_machine_travel() {
        let current = inspection();
        assert!(matches!(
            validate_setting_edit(
                ControllerSettingEditRequest {
                    key: "$130".to_owned(),
                    value: "0".to_owned(),
                    confirmed: true,
                    expected_value: None,
                    expected_revision: Some(1),
                },
                &current,
            ),
            Err(SettingsError::InvalidValue { .. })
        ));
    }

    #[test]
    fn rejects_a_write_when_the_fresh_controller_value_changed_externally() {
        let current = inspection();
        assert!(matches!(
            validate_setting_edit(
                ControllerSettingEditRequest {
                    key: "$100".to_owned(),
                    value: "1700".to_owned(),
                    confirmed: true,
                    expected_value: Some("1500".to_owned()),
                    expected_revision: Some(4),
                },
                &current,
            ),
            Err(SettingsError::StaleControllerValue { .. })
        ));
    }

    #[test]
    fn keeps_the_connection_baseline_through_multiple_verified_changes() {
        let path = test_path();
        let mut first = inspection();
        first
            .settings
            .insert("$120".to_owned(), "500.000".to_owned());
        let mut archive =
            MachineSettingsArchive::begin(&path, "machine-0001", "port:test", &first).unwrap();

        archive
            .record_verified_change("$120", "500.000", "600.000")
            .unwrap();
        archive
            .record_verified_change("$120", "600.000", "800.000")
            .unwrap();
        assert_eq!(archive.state().baseline_value("$120"), Some("500.000"));
        assert_eq!(
            archive
                .state()
                .active
                .current
                .get("$120")
                .map(String::as_str),
            Some("800.000")
        );

        let mut reconnected = first;
        reconnected
            .settings
            .insert("$120".to_owned(), "800.000".to_owned());
        let archive =
            MachineSettingsArchive::begin(&path, "machine-0001", "port:test", &reconnected)
                .unwrap();
        assert_eq!(archive.state().baseline_value("$120"), Some("800.000"));
        assert_eq!(archive.state().previous_value("$120"), Some("500.000"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn records_only_a_fresh_controller_observation_as_current_truth() {
        let path = test_path();
        let first = inspection();
        let mut archive =
            MachineSettingsArchive::begin(&path, "machine-0001", "port:test", &first).unwrap();
        let mut observed = first;
        observed
            .settings
            .insert("$100".to_owned(), "1700.000".to_owned());

        archive.record_observation(&observed).unwrap();

        assert_eq!(archive.state().baseline_value("$100"), Some("1600.000"));
        assert_eq!(
            archive
                .state()
                .active
                .current
                .get("$100")
                .map(String::as_str),
            Some("1700.000")
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn recovers_and_repairs_settings_from_the_last_valid_backup() {
        let path = test_path();
        let first = inspection();
        let mut archive =
            MachineSettingsArchive::begin(&path, "machine-0001", "port:test", &first).unwrap();
        archive.record_observation(&first).unwrap();
        fs::write(&path, b"corrupt").unwrap();

        let recovered =
            MachineSettingsArchive::begin(&path, "machine-0001", "port:test", &first).unwrap();

        assert_eq!(recovered.state().baseline_value("$100"), Some("1600.000"));
        assert!(serde_json::from_slice::<StoredSettingsArchive>(&fs::read(&path).unwrap()).is_ok());
        assert!(backup_path(&path).exists());

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup_path(&path));
    }

    #[test]
    fn reports_when_both_settings_copies_are_corrupt() {
        let path = test_path();
        fs::write(&path, b"corrupt primary").unwrap();
        fs::write(backup_path(&path), b"corrupt backup").unwrap();

        assert!(matches!(
            MachineSettingsArchive::begin(&path, "machine-0001", "port:test", &inspection()),
            Err(ArchiveError::CorruptCopies { .. })
        ));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(backup_path(&path));
    }

    fn test_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "millo-settings-{}-{}.json",
            std::process::id(),
            TEST_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
