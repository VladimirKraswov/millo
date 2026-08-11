use std::{
    fs, io,
    path::{Path, PathBuf},
};

use millo_domain::{DeviceInspection, HardwareProfile, MachineTravel, SpindleControl};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SCHEMA_VERSION: u16 = 1;
const MAX_PROFILES: usize = 64;
const MAX_NAME_BYTES: usize = 80;
const MAX_TRAVEL_MM: f64 = 100_000.0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineConnectionPreset {
    pub transport_id: String,
    pub baud_rate: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedController {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_build_info: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineProfile {
    pub id: String,
    pub name: String,
    pub travel_mm: MachineTravel,
    pub spindle_control: SpindleControl,
    pub homing_installed: bool,
    pub limit_switches_installed: bool,
    pub probe_installed: bool,
    pub emergency_stop_installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<MachineConnectionPreset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_controller: Option<DetectedController>,
}

impl MachineProfile {
    pub fn hardware_profile(&self) -> HardwareProfile {
        HardwareProfile {
            name: self.name.clone(),
            axes: vec!["X".to_owned(), "Y".to_owned(), "Z".to_owned()],
            travel_mm: Some(self.travel_mm),
            spindle_control: self.spindle_control,
            homing_installed: self.homing_installed,
            limit_switches_installed: self.limit_switches_installed,
            probe_installed: self.probe_installed,
            emergency_stop_installed: self.emergency_stop_installed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineProfileDraft {
    pub name: String,
    pub travel_mm: MachineTravel,
    #[serde(default)]
    pub spindle_control: SpindleControl,
    #[serde(default)]
    pub homing_installed: bool,
    #[serde(default)]
    pub limit_switches_installed: bool,
    #[serde(default)]
    pub probe_installed: bool,
    #[serde(default)]
    pub emergency_stop_installed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<MachineConnectionPreset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_controller: Option<DetectedController>,
}

impl MachineProfileDraft {
    pub fn from_grbl_inspection(
        suggested_name: impl Into<String>,
        inspection: &DeviceInspection,
        connection: MachineConnectionPreset,
    ) -> Result<Self, ProfileError> {
        Ok(Self {
            name: suggested_name.into(),
            travel_mm: MachineTravel {
                x: positive_setting(inspection, "$130")?,
                y: positive_setting(inspection, "$131")?,
                z: positive_setting(inspection, "$132")?,
            },
            spindle_control: SpindleControl::Manual,
            homing_installed: enabled_setting(inspection, "$22"),
            limit_switches_installed: enabled_setting(inspection, "$21"),
            probe_installed: false,
            emergency_stop_installed: false,
            connection: Some(connection),
            detected_controller: Some(DetectedController {
                firmware_version: inspection.firmware_version.clone(),
                firmware_build_info: inspection.firmware_build_info.clone(),
            }),
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineProfileState {
    pub profiles: Vec<MachineProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_profile_id: Option<String>,
}

impl MachineProfileState {
    pub fn selected(&self) -> Option<&MachineProfile> {
        let selected = self.selected_profile_id.as_deref()?;
        self.profiles.iter().find(|profile| profile.id == selected)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredProfiles {
    schema_version: u16,
    next_id: u64,
    profiles: Vec<MachineProfile>,
    selected_profile_id: Option<String>,
}

impl Default for StoredProfiles {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            next_id: 1,
            profiles: Vec::new(),
            selected_profile_id: None,
        }
    }
}

#[derive(Debug)]
pub struct MachineProfileStore {
    path: Option<PathBuf>,
    document: StoredProfiles,
}

impl MachineProfileStore {
    pub fn in_memory() -> Self {
        Self {
            path: None,
            document: StoredProfiles::default(),
        }
    }

    pub fn load(path: impl Into<PathBuf>) -> Result<Self, ProfileError> {
        let path = path.into();
        if !path.exists() {
            return Ok(Self {
                path: Some(path),
                document: StoredProfiles::default(),
            });
        }

        let bytes = fs::read(&path).map_err(|source| ProfileError::Io {
            path: path.clone(),
            source,
        })?;
        let document: StoredProfiles =
            serde_json::from_slice(&bytes).map_err(ProfileError::InvalidFile)?;
        validate_document(&document)?;
        Ok(Self {
            path: Some(path),
            document,
        })
    }

    pub fn state(&self) -> MachineProfileState {
        MachineProfileState {
            profiles: self.document.profiles.clone(),
            selected_profile_id: self.document.selected_profile_id.clone(),
        }
    }

    pub fn create_and_select(
        &mut self,
        draft: MachineProfileDraft,
    ) -> Result<MachineProfileState, ProfileError> {
        validate_draft(&draft)?;
        if self.document.profiles.len() >= MAX_PROFILES {
            return Err(ProfileError::ProfileLimit(MAX_PROFILES));
        }
        if self
            .document
            .profiles
            .iter()
            .any(|profile| profile.name.eq_ignore_ascii_case(draft.name.trim()))
        {
            return Err(ProfileError::DuplicateName(draft.name.trim().to_owned()));
        }

        let mut next = self.document.clone();
        let profile = MachineProfile {
            id: format!("machine-{:04}", next.next_id),
            name: draft.name.trim().to_owned(),
            travel_mm: draft.travel_mm,
            spindle_control: draft.spindle_control,
            homing_installed: draft.homing_installed,
            limit_switches_installed: draft.limit_switches_installed,
            probe_installed: draft.probe_installed,
            emergency_stop_installed: draft.emergency_stop_installed,
            connection: draft.connection,
            detected_controller: draft.detected_controller,
        };
        next.next_id = next
            .next_id
            .checked_add(1)
            .ok_or(ProfileError::IdExhausted)?;
        next.selected_profile_id = Some(profile.id.clone());
        next.profiles.push(profile);
        self.commit(next)?;
        Ok(self.state())
    }

    pub fn select(&mut self, profile_id: &str) -> Result<MachineProfileState, ProfileError> {
        if !self
            .document
            .profiles
            .iter()
            .any(|profile| profile.id == profile_id)
        {
            return Err(ProfileError::UnknownProfile(profile_id.to_owned()));
        }
        let mut next = self.document.clone();
        next.selected_profile_id = Some(profile_id.to_owned());
        self.commit(next)?;
        Ok(self.state())
    }

    fn commit(&mut self, next: StoredProfiles) -> Result<(), ProfileError> {
        if let Some(path) = &self.path {
            save_document(path, &next)?;
        }
        self.document = next;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("machine name is required")]
    MissingName,
    #[error("machine name must not exceed {MAX_NAME_BYTES} bytes")]
    NameTooLong,
    #[error("{0} travel must be finite and between 0 and {MAX_TRAVEL_MM} mm")]
    InvalidTravel(&'static str),
    #[error("machine profile already exists: {0}")]
    DuplicateName(String),
    #[error("machine profile limit reached: {0}")]
    ProfileLimit(usize),
    #[error("unknown machine profile: {0}")]
    UnknownProfile(String),
    #[error("machine profile id space is exhausted")]
    IdExhausted,
    #[error("unsupported machine-profile schema version: {0}")]
    UnsupportedSchema(u16),
    #[error("selected machine profile does not exist: {0}")]
    InvalidSelection(String),
    #[error("GRBL setting is missing or invalid: {0}")]
    InvalidControllerSetting(String),
    #[error("invalid machine-profile file: {0}")]
    InvalidFile(serde_json::Error),
    #[error("machine-profile I/O failed for {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

fn validate_document(document: &StoredProfiles) -> Result<(), ProfileError> {
    if document.schema_version != SCHEMA_VERSION {
        return Err(ProfileError::UnsupportedSchema(document.schema_version));
    }
    if document.profiles.len() > MAX_PROFILES {
        return Err(ProfileError::ProfileLimit(MAX_PROFILES));
    }
    for profile in &document.profiles {
        validate_draft(&MachineProfileDraft {
            name: profile.name.clone(),
            travel_mm: profile.travel_mm,
            spindle_control: profile.spindle_control,
            homing_installed: profile.homing_installed,
            limit_switches_installed: profile.limit_switches_installed,
            probe_installed: profile.probe_installed,
            emergency_stop_installed: profile.emergency_stop_installed,
            connection: profile.connection.clone(),
            detected_controller: profile.detected_controller.clone(),
        })?;
    }
    if let Some(selected) = &document.selected_profile_id
        && !document
            .profiles
            .iter()
            .any(|profile| profile.id == *selected)
    {
        return Err(ProfileError::InvalidSelection(selected.clone()));
    }
    Ok(())
}

fn validate_draft(draft: &MachineProfileDraft) -> Result<(), ProfileError> {
    let name = draft.name.trim();
    if name.is_empty() {
        return Err(ProfileError::MissingName);
    }
    if name.len() > MAX_NAME_BYTES {
        return Err(ProfileError::NameTooLong);
    }
    validate_travel("X", draft.travel_mm.x)?;
    validate_travel("Y", draft.travel_mm.y)?;
    validate_travel("Z", draft.travel_mm.z)
}

fn validate_travel(axis: &'static str, value: f64) -> Result<(), ProfileError> {
    if value.is_finite() && value > 0.0 && value <= MAX_TRAVEL_MM {
        Ok(())
    } else {
        Err(ProfileError::InvalidTravel(axis))
    }
}

fn positive_setting(inspection: &DeviceInspection, key: &str) -> Result<f64, ProfileError> {
    inspection
        .settings
        .get(key)
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0 && *value <= MAX_TRAVEL_MM)
        .ok_or_else(|| ProfileError::InvalidControllerSetting(key.to_owned()))
}

fn enabled_setting(inspection: &DeviceInspection, key: &str) -> bool {
    inspection
        .settings
        .get(key)
        .is_some_and(|value| value == "1")
}

fn save_document(path: &Path, document: &StoredProfiles) -> Result<(), ProfileError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ProfileError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(document).map_err(ProfileError::InvalidFile)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|source| ProfileError::Io {
        path: temporary.clone(),
        source,
    })?;
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(_) if path.exists() => {
            fs::remove_file(path).map_err(|source| ProfileError::Io {
                path: path.to_path_buf(),
                source,
            })?;
            fs::rename(&temporary, path).map_err(|source| ProfileError::Io {
                path: path.to_path_buf(),
                source,
            })
        }
        Err(source) => Err(ProfileError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static TEST_ID: AtomicU64 = AtomicU64::new(1);

    fn draft(name: &str) -> MachineProfileDraft {
        MachineProfileDraft {
            name: name.to_owned(),
            travel_mm: MachineTravel {
                x: 300.0,
                y: 180.0,
                z: 80.0,
            },
            spindle_control: SpindleControl::Manual,
            homing_installed: false,
            limit_switches_installed: false,
            probe_installed: false,
            emergency_stop_installed: false,
            connection: None,
            detected_controller: None,
        }
    }

    fn test_path() -> PathBuf {
        std::env::temp_dir().join(format!(
            "millo-profile-{}-{}.json",
            std::process::id(),
            TEST_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn creates_selects_and_reloads_a_profile() {
        let path = test_path();
        let mut store = MachineProfileStore::load(&path).unwrap();
        let created = store.create_and_select(draft("  Bench router  ")).unwrap();

        assert_eq!(created.selected_profile_id.as_deref(), Some("machine-0001"));
        assert_eq!(created.selected().unwrap().name, "Bench router");
        assert_eq!(created.selected().unwrap().hardware_profile().axes.len(), 3);

        let reloaded = MachineProfileStore::load(&path).unwrap().state();
        assert_eq!(reloaded, created);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_missing_dimensions_and_duplicate_names() {
        let mut store = MachineProfileStore::in_memory();
        let mut invalid = draft("Router");
        invalid.travel_mm.z = 0.0;
        assert!(matches!(
            store.create_and_select(invalid),
            Err(ProfileError::InvalidTravel("Z"))
        ));

        store.create_and_select(draft("Router")).unwrap();
        assert!(matches!(
            store.create_and_select(draft("router")),
            Err(ProfileError::DuplicateName(_))
        ));
    }

    #[test]
    fn derives_only_controller_backed_fields_from_grbl() {
        let inspection = DeviceInspection {
            firmware_version: Some("1.1f.20230316".to_owned()),
            firmware_build_info: Some("LUNYEE".to_owned()),
            settings: BTreeMap::from([
                ("$21".to_owned(), "0".to_owned()),
                ("$22".to_owned(), "1".to_owned()),
                ("$130".to_owned(), "301.500".to_owned()),
                ("$131".to_owned(), "181.000".to_owned()),
                ("$132".to_owned(), "45.000".to_owned()),
            ]),
            ..DeviceInspection::default()
        };

        let detected = MachineProfileDraft::from_grbl_inspection(
            "Detected GRBL",
            &inspection,
            MachineConnectionPreset {
                transport_id: "serial:/dev/cu.test".to_owned(),
                baud_rate: 115_200,
            },
        )
        .unwrap();

        assert_eq!(detected.travel_mm.x, 301.5);
        assert!(detected.homing_installed);
        assert!(!detected.limit_switches_installed);
        assert!(!detected.probe_installed);
        assert!(!detected.emergency_stop_installed);
        assert_eq!(
            detected
                .detected_controller
                .unwrap()
                .firmware_version
                .as_deref(),
            Some("1.1f.20230316")
        );
    }

    #[test]
    fn rejects_a_corrupt_selected_profile_reference() {
        let path = test_path();
        fs::write(
            &path,
            r#"{"schemaVersion":1,"nextId":2,"profiles":[],"selectedProfileId":"missing"}"#,
        )
        .unwrap();

        assert!(matches!(
            MachineProfileStore::load(&path),
            Err(ProfileError::InvalidSelection(value)) if value == "missing"
        ));
        let _ = fs::remove_file(path);
    }
}
