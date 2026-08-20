use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use millo_gcode::{GcodeProgram, ProgramParseRequest, parse_program};
use millo_storage::{backup_path, write_atomically};
use rhai::{Dynamic, Engine, Scope};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const SCRIPT_PACKAGE_VERSION: u16 = 1;
pub const SCRIPT_API_VERSION: u16 = 1;
pub const MAX_SCRIPT_BYTES: usize = 256 * 1024;
pub const MAX_PLUGIN_COMMANDS: usize = 64;
pub const MAX_INSTALLED_PLUGINS: usize = 128;
const MAX_PLUGIN_STORE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_COMMAND_FIELDS: usize = 32;
const MAX_FIELD_TEXT_BYTES: usize = 4 * 1024;
const STORE_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ScriptCapability {
    #[serde(rename = "ui.contribute")]
    UiContribute,
    #[serde(rename = "machine.read")]
    MachineRead,
    #[serde(rename = "machine.jog")]
    MachineJog,
    #[serde(rename = "machine.coordinates")]
    MachineCoordinates,
    #[serde(rename = "machine.commands")]
    MachineCommands,
    #[serde(rename = "jobs.create")]
    JobsCreate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptCapabilityDeclaration {
    pub required: Vec<ScriptCapability>,
    #[serde(default)]
    pub optional: Vec<ScriptCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginManifest {
    pub manifest_version: u16,
    pub api_version: u16,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: ScriptCapabilityDeclaration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptCommandSurface {
    WorkspaceTools,
    MachinePanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptFieldKind {
    Number,
    Boolean,
    Text,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptCommandField {
    pub id: String,
    pub label: String,
    pub kind: ScriptFieldKind,
    pub default_value: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginCommand {
    pub id: String,
    pub title: String,
    pub description: String,
    pub icon: String,
    pub surface: ScriptCommandSurface,
    #[serde(default)]
    pub fields: Vec<ScriptCommandField>,
    #[serde(default)]
    pub required_capabilities: Vec<ScriptCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginPackage {
    pub package_version: u16,
    pub manifest: ScriptPluginManifest,
    pub commands: Vec<ScriptPluginCommand>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledScriptPlugin {
    pub package: ScriptPluginPackage,
    pub digest: String,
    pub enabled: bool,
    pub bundled: bool,
    pub granted_capabilities: Vec<ScriptCapability>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScriptPluginStoreFile {
    version: u16,
    plugins: Vec<InstalledScriptPlugin>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScriptAction {
    CreateProgram {
        source_name: String,
        source: String,
    },
    Jog {
        axis: ScriptAxis,
        distance_mm: f64,
        feed_mm_per_min: f64,
    },
    SetZero {
        axis: ScriptAxis,
    },
    ReturnZero {
        axis: ScriptAxis,
        feed_mm_per_min: f64,
    },
    RawCommand {
        command: String,
    },
    Notice {
        title: String,
        message: String,
        #[serde(default)]
        tone: ScriptNoticeTone,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptAxis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptNoticeTone {
    #[default]
    Info,
    Success,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptGeneratedJob {
    pub source_name: String,
    pub source: String,
    pub program: GcodeProgram,
}

#[derive(Debug, Error)]
pub enum ScriptPluginError {
    #[error("plugin package is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("unsupported plugin package version: {0}")]
    UnsupportedPackageVersion(u16),
    #[error("unsupported script API version: {0}")]
    UnsupportedApiVersion(u16),
    #[error("invalid plugin package: {0}")]
    InvalidPackage(String),
    #[error("script could not be compiled: {0}")]
    Compile(String),
    #[error("script command failed: {0}")]
    Runtime(String),
    #[error("script action is invalid: {0}")]
    InvalidAction(String),
    #[error("plugin is not installed: {0}")]
    NotInstalled(String),
    #[error("plugin is disabled: {0}")]
    Disabled(String),
    #[error("plugin command is not declared: {0}")]
    UnknownCommand(String),
    #[error("plugin capability was not granted: {0:?}")]
    CapabilityDenied(ScriptCapability),
    #[error("plugin digest changed; review and grant it again")]
    DigestMismatch,
    #[error("plugin storage failed: {0}")]
    Storage(String),
}

pub fn parse_package(json: &str) -> Result<ScriptPluginPackage, ScriptPluginError> {
    let package: ScriptPluginPackage = serde_json::from_str(json)?;
    validate_package(&package)?;
    Ok(package)
}

pub fn package_json(package: &ScriptPluginPackage) -> Result<String, ScriptPluginError> {
    validate_package(package)?;
    serde_json::to_string_pretty(package).map_err(ScriptPluginError::from)
}

pub fn package_digest(package: &ScriptPluginPackage) -> Result<String, ScriptPluginError> {
    let bytes = serde_json::to_vec(package)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

pub fn validate_package(package: &ScriptPluginPackage) -> Result<(), ScriptPluginError> {
    if package.package_version != SCRIPT_PACKAGE_VERSION {
        return Err(ScriptPluginError::UnsupportedPackageVersion(
            package.package_version,
        ));
    }
    if package.manifest.manifest_version != 1 {
        return Err(ScriptPluginError::InvalidPackage(format!(
            "unsupported manifest version: {}",
            package.manifest.manifest_version
        )));
    }
    if package.manifest.api_version != SCRIPT_API_VERSION {
        return Err(ScriptPluginError::UnsupportedApiVersion(
            package.manifest.api_version,
        ));
    }
    if !valid_plugin_id(&package.manifest.id) {
        return Err(ScriptPluginError::InvalidPackage(
            "plugin id must be lowercase dot- or dash-separated segments".to_owned(),
        ));
    }
    validate_text("plugin name", &package.manifest.name, 100)?;
    validate_text("plugin version", &package.manifest.version, 64)?;
    validate_text("plugin description", &package.manifest.description, 500)?;
    if !valid_semver(&package.manifest.version) {
        return Err(ScriptPluginError::InvalidPackage(
            "plugin version must use semantic versioning".to_owned(),
        ));
    }
    if package.source.len() > MAX_SCRIPT_BYTES {
        return Err(ScriptPluginError::InvalidPackage(format!(
            "script exceeds {MAX_SCRIPT_BYTES} bytes"
        )));
    }
    if package.source.trim().is_empty() {
        return Err(ScriptPluginError::InvalidPackage(
            "script source is empty".to_owned(),
        ));
    }
    if package.commands.is_empty() || package.commands.len() > MAX_PLUGIN_COMMANDS {
        return Err(ScriptPluginError::InvalidPackage(format!(
            "plugin must declare 1..={MAX_PLUGIN_COMMANDS} commands"
        )));
    }

    let required = capability_set(&package.manifest.capabilities.required, "required")?;
    let optional = capability_set(&package.manifest.capabilities.optional, "optional")?;
    if let Some(duplicate) = required.intersection(&optional).next() {
        return Err(ScriptPluginError::InvalidPackage(format!(
            "capability is both required and optional: {duplicate:?}"
        )));
    }
    if !required.contains(&ScriptCapability::UiContribute) {
        return Err(ScriptPluginError::InvalidPackage(
            "ui.contribute must be a required capability for command plugins".to_owned(),
        ));
    }
    let declared = required.union(&optional).copied().collect::<BTreeSet<_>>();
    let mut command_ids = BTreeSet::new();
    for command in &package.commands {
        if !valid_local_id(&command.id) || !command_ids.insert(command.id.as_str()) {
            return Err(ScriptPluginError::InvalidPackage(format!(
                "invalid or duplicate command id: {}",
                command.id
            )));
        }
        validate_text("command title", &command.title, 100)?;
        validate_text("command description", &command.description, 500)?;
        validate_text("command icon", &command.icon, 64)?;
        let command_capabilities = capability_set(
            &command.required_capabilities,
            &format!("command {} required", command.id),
        )?;
        if !command_capabilities.is_subset(&declared) {
            return Err(ScriptPluginError::InvalidPackage(format!(
                "command {} requests an undeclared capability",
                command.id
            )));
        }
        if command.fields.len() > MAX_COMMAND_FIELDS {
            return Err(ScriptPluginError::InvalidPackage(format!(
                "command {} has too many fields",
                command.id
            )));
        }
        let mut field_ids = BTreeSet::new();
        for field in &command.fields {
            if !valid_local_id(&field.id) || !field_ids.insert(field.id.as_str()) {
                return Err(ScriptPluginError::InvalidPackage(format!(
                    "invalid or duplicate field id: {}",
                    field.id
                )));
            }
            validate_text("field label", &field.label, 100)?;
            if let Some(unit) = &field.unit {
                validate_text("field unit", unit, 24)?;
            }
            if field.min.is_some_and(|value| !value.is_finite())
                || field.max.is_some_and(|value| !value.is_finite())
                || matches!((field.min, field.max), (Some(min), Some(max)) if min > max)
            {
                return Err(ScriptPluginError::InvalidPackage(format!(
                    "field {} has invalid bounds",
                    field.id
                )));
            }
            if field
                .step
                .is_some_and(|step| !step.is_finite() || step <= 0.0)
            {
                return Err(ScriptPluginError::InvalidPackage(format!(
                    "field {} has an invalid step",
                    field.id
                )));
            }
            if field.kind != ScriptFieldKind::Number
                && (field.min.is_some() || field.max.is_some() || field.step.is_some())
            {
                return Err(ScriptPluginError::InvalidPackage(format!(
                    "field {} has numeric constraints but is not a number",
                    field.id
                )));
            }
            validate_field_value(field, &field.default_value)?;
        }
        if let Some(reason) = &command.unavailable_reason {
            validate_text("unavailable reason", reason, 500)?;
        }
    }

    ScriptRuntime::engine()
        .compile(&package.source)
        .map_err(|error| ScriptPluginError::Compile(error.to_string()))?;
    Ok(())
}

pub struct ScriptRuntime;

impl ScriptRuntime {
    pub fn execute(
        package: &ScriptPluginPackage,
        command_id: &str,
        input: Value,
        machine: Value,
    ) -> Result<ScriptAction, ScriptPluginError> {
        validate_package(package)?;
        let command = package
            .commands
            .iter()
            .find(|command| command.id == command_id)
            .ok_or_else(|| ScriptPluginError::UnknownCommand(command_id.to_owned()))?;
        if let Some(reason) = &command.unavailable_reason {
            return Err(ScriptPluginError::InvalidAction(reason.clone()));
        }
        let engine = Self::engine();
        let ast = engine
            .compile(&package.source)
            .map_err(|error| ScriptPluginError::Compile(error.to_string()))?;
        let input = normalize_input(command, input)?;
        let input = rhai::serde::to_dynamic(input)
            .map_err(|error| ScriptPluginError::Runtime(error.to_string()))?;
        let machine = rhai::serde::to_dynamic(machine)
            .map_err(|error| ScriptPluginError::Runtime(error.to_string()))?;
        let mut scope = Scope::new();
        let result = engine
            .call_fn::<Dynamic>(
                &mut scope,
                &ast,
                "run",
                (command_id.to_owned(), input, machine),
            )
            .map_err(|error| ScriptPluginError::Runtime(error.to_string()))?;
        let action: ScriptAction = rhai::serde::from_dynamic(&result)
            .map_err(|error| ScriptPluginError::InvalidAction(error.to_string()))?;
        validate_action(&action)?;
        if let Some(capability) = action_capability(&action)
            && !command.required_capabilities.contains(&capability)
        {
            return Err(ScriptPluginError::InvalidAction(format!(
                "command {} returned an action without declaring {capability:?}",
                command.id
            )));
        }
        Ok(action)
    }

    fn engine() -> Engine {
        let mut engine = Engine::new();
        engine.set_max_operations(50_000);
        engine.set_max_call_levels(32);
        engine.set_max_expr_depths(128, 64);
        engine.set_max_string_size(2 * 1024 * 1024);
        engine.set_max_array_size(2_048);
        engine.set_max_map_size(2_048);
        engine.set_allow_shadowing(false);
        engine.set_fail_on_invalid_map_property(true);
        engine.disable_symbol("eval");
        engine.disable_symbol("import");
        engine
    }
}

pub fn action_capability(action: &ScriptAction) -> Option<ScriptCapability> {
    match action {
        ScriptAction::CreateProgram { .. } => Some(ScriptCapability::JobsCreate),
        ScriptAction::Jog { .. } => Some(ScriptCapability::MachineJog),
        ScriptAction::SetZero { .. } | ScriptAction::ReturnZero { .. } => {
            Some(ScriptCapability::MachineCoordinates)
        }
        ScriptAction::RawCommand { .. } => Some(ScriptCapability::MachineCommands),
        ScriptAction::Notice { .. } => None,
    }
}

pub fn generated_job(action: &ScriptAction) -> Result<ScriptGeneratedJob, ScriptPluginError> {
    let ScriptAction::CreateProgram {
        source_name,
        source,
    } = action
    else {
        return Err(ScriptPluginError::InvalidAction(
            "action does not create a program".to_owned(),
        ));
    };
    let program = parse_program(ProgramParseRequest {
        source_name: source_name.clone(),
        source: source.clone(),
    })
    .map_err(|error| ScriptPluginError::InvalidAction(error.to_string()))?;
    Ok(ScriptGeneratedJob {
        source_name: source_name.clone(),
        source: source.clone(),
        program,
    })
}

#[derive(Debug)]
pub struct ScriptPluginStore {
    path: Option<PathBuf>,
    plugins: BTreeMap<String, InstalledScriptPlugin>,
}

impl ScriptPluginStore {
    pub fn in_memory() -> Result<Self, ScriptPluginError> {
        Self::with_plugins(None, Vec::new())
    }

    pub fn load(path: impl Into<PathBuf>) -> Result<Self, ScriptPluginError> {
        let path = path.into();
        let backup = backup_path(&path);
        let (plugins, recovered) = match read_store_file(&path) {
            Ok(Some(file)) => (file.plugins, false),
            Ok(None) => match read_store_file(&backup)? {
                Some(file) => (file.plugins, true),
                None => (Vec::new(), false),
            },
            Err(primary_error) => match read_store_file(&backup) {
                Ok(Some(file)) => (file.plugins, true),
                Ok(None) | Err(_) => return Err(primary_error),
            },
        };
        if recovered {
            remove_file_if_present(&path)?;
        }
        Self::with_plugins(Some(path), plugins)
    }

    fn with_plugins(
        path: Option<PathBuf>,
        plugins: Vec<InstalledScriptPlugin>,
    ) -> Result<Self, ScriptPluginError> {
        let mut installed = BTreeMap::new();
        if plugins.len() > MAX_INSTALLED_PLUGINS {
            return Err(ScriptPluginError::Storage(format!(
                "plugin store exceeds the {MAX_INSTALLED_PLUGINS} package limit"
            )));
        }
        for plugin in plugins {
            validate_installed_plugin(&plugin)?;
            let id = plugin.package.manifest.id.clone();
            if installed.insert(id.clone(), plugin).is_some() {
                return Err(ScriptPluginError::Storage(format!(
                    "duplicate plugin in store: {id}"
                )));
            }
        }
        let mut store = Self {
            path,
            plugins: installed,
        };
        store.install_bundled(default_macro_package()?)?;
        Ok(store)
    }

    pub fn list(&self) -> Vec<InstalledScriptPlugin> {
        self.plugins.values().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<&InstalledScriptPlugin> {
        self.plugins.get(id)
    }

    pub fn install_external(
        &mut self,
        package: ScriptPluginPackage,
    ) -> Result<InstalledScriptPlugin, ScriptPluginError> {
        validate_package(&package)?;
        if self
            .plugins
            .get(&package.manifest.id)
            .is_some_and(|plugin| plugin.bundled)
        {
            return Err(ScriptPluginError::InvalidPackage(
                "bundled plugin ids are reserved".to_owned(),
            ));
        }
        if !self.plugins.contains_key(&package.manifest.id)
            && self.plugins.len() >= MAX_INSTALLED_PLUGINS
        {
            return Err(ScriptPluginError::Storage(format!(
                "plugin store reached the {MAX_INSTALLED_PLUGINS} package limit"
            )));
        }
        let digest = package_digest(&package)?;
        let installed = InstalledScriptPlugin {
            package,
            digest,
            enabled: false,
            bundled: false,
            granted_capabilities: Vec::new(),
        };
        let mut candidate = self.plugins.clone();
        candidate.insert(installed.package.manifest.id.clone(), installed.clone());
        self.commit(candidate)?;
        Ok(installed)
    }

    pub fn set_enabled(
        &mut self,
        id: &str,
        digest: &str,
        enabled: bool,
        grants: Vec<ScriptCapability>,
    ) -> Result<InstalledScriptPlugin, ScriptPluginError> {
        let mut candidate = self.plugins.clone();
        let plugin = candidate
            .get_mut(id)
            .ok_or_else(|| ScriptPluginError::NotInstalled(id.to_owned()))?;
        if plugin.digest != digest {
            return Err(ScriptPluginError::DigestMismatch);
        }
        let declared = plugin
            .package
            .manifest
            .capabilities
            .required
            .iter()
            .chain(plugin.package.manifest.capabilities.optional.iter())
            .copied()
            .collect::<BTreeSet<_>>();
        let granted = grants.into_iter().collect::<BTreeSet<_>>();
        if !granted.is_subset(&declared) {
            return Err(ScriptPluginError::InvalidPackage(
                "grant contains an undeclared capability".to_owned(),
            ));
        }
        if enabled
            && !plugin
                .package
                .manifest
                .capabilities
                .required
                .iter()
                .all(|capability| granted.contains(capability))
        {
            return Err(ScriptPluginError::InvalidPackage(
                "all required capabilities must be granted before enabling".to_owned(),
            ));
        }
        plugin.enabled = enabled;
        plugin.granted_capabilities = granted.into_iter().collect();
        let result = plugin.clone();
        self.commit(candidate)?;
        Ok(result)
    }

    pub fn remove(&mut self, id: &str) -> Result<bool, ScriptPluginError> {
        if self.plugins.get(id).is_some_and(|plugin| plugin.bundled) {
            return Err(ScriptPluginError::InvalidPackage(
                "bundled plugins cannot be deleted".to_owned(),
            ));
        }
        let mut candidate = self.plugins.clone();
        let removed = candidate.remove(id).is_some();
        if removed {
            self.commit(candidate)?;
        }
        Ok(removed)
    }

    fn install_bundled(&mut self, package: ScriptPluginPackage) -> Result<(), ScriptPluginError> {
        validate_package(&package)?;
        let digest = package_digest(&package)?;
        let id = package.manifest.id.clone();
        let required = package.manifest.capabilities.required.clone();
        let mut candidate = self.plugins.clone();
        match candidate.get(&id) {
            Some(existing) if existing.digest == digest => {
                candidate.insert(
                    id,
                    InstalledScriptPlugin {
                        package,
                        digest,
                        enabled: existing.enabled,
                        bundled: true,
                        granted_capabilities: existing.granted_capabilities.clone(),
                    },
                );
            }
            Some(existing) => {
                candidate.insert(
                    id,
                    InstalledScriptPlugin {
                        package,
                        digest,
                        enabled: existing.enabled,
                        bundled: true,
                        granted_capabilities: required,
                    },
                );
            }
            None => {
                candidate.insert(
                    id,
                    InstalledScriptPlugin {
                        package,
                        digest,
                        enabled: true,
                        bundled: true,
                        granted_capabilities: required,
                    },
                );
            }
        }
        self.commit(candidate)
    }

    fn commit(
        &mut self,
        candidate: BTreeMap<String, InstalledScriptPlugin>,
    ) -> Result<(), ScriptPluginError> {
        self.persist(&candidate)?;
        self.plugins = candidate;
        Ok(())
    }

    fn persist(
        &self,
        plugins: &BTreeMap<String, InstalledScriptPlugin>,
    ) -> Result<(), ScriptPluginError> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let file = ScriptPluginStoreFile {
            version: STORE_VERSION,
            plugins: plugins.values().cloned().collect(),
        };
        let bytes = serde_json::to_vec_pretty(&file)?;
        write_atomically(path, &bytes)
            .map_err(|error| ScriptPluginError::Storage(error.to_string()))
    }
}

fn validate_installed_plugin(plugin: &InstalledScriptPlugin) -> Result<(), ScriptPluginError> {
    validate_package(&plugin.package)?;
    if package_digest(&plugin.package)? != plugin.digest {
        return Err(ScriptPluginError::Storage(format!(
            "stored digest does not match plugin package: {}",
            plugin.package.manifest.id
        )));
    }
    let declared = plugin
        .package
        .manifest
        .capabilities
        .required
        .iter()
        .chain(plugin.package.manifest.capabilities.optional.iter())
        .copied()
        .collect::<BTreeSet<_>>();
    let granted = plugin
        .granted_capabilities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if granted.len() != plugin.granted_capabilities.len() || !granted.is_subset(&declared) {
        return Err(ScriptPluginError::Storage(format!(
            "stored grants are invalid: {}",
            plugin.package.manifest.id
        )));
    }
    if plugin.enabled
        && !plugin
            .package
            .manifest
            .capabilities
            .required
            .iter()
            .all(|capability| granted.contains(capability))
    {
        return Err(ScriptPluginError::Storage(format!(
            "enabled plugin is missing a required grant: {}",
            plugin.package.manifest.id
        )));
    }
    Ok(())
}

pub fn default_macro_package() -> Result<ScriptPluginPackage, ScriptPluginError> {
    serde_json::from_str(include_str!("../defaults/operator-macros.millo-plugin"))
        .map_err(ScriptPluginError::from)
}

pub fn read_package(path: &Path) -> Result<ScriptPluginPackage, ScriptPluginError> {
    let bytes = fs::read(path).map_err(|error| ScriptPluginError::Storage(error.to_string()))?;
    if bytes.len() > MAX_SCRIPT_BYTES * 2 {
        return Err(ScriptPluginError::InvalidPackage(
            "plugin package is too large".to_owned(),
        ));
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| ScriptPluginError::InvalidPackage(error.to_string()))?;
    parse_package(text)
}

fn validate_action(action: &ScriptAction) -> Result<(), ScriptPluginError> {
    match action {
        ScriptAction::CreateProgram {
            source_name,
            source,
        } => {
            validate_action_text("generated source name", source_name, 240)?;
            if Path::new(source_name)
                .file_name()
                .and_then(|value| value.to_str())
                != Some(source_name.as_str())
                || source_name.chars().any(char::is_control)
            {
                return Err(ScriptPluginError::InvalidAction(
                    "generated source name must be a plain file name".to_owned(),
                ));
            }
            if source.len() > millo_gcode::MAX_SOURCE_BYTES || source.trim().is_empty() {
                return Err(ScriptPluginError::InvalidAction(
                    "generated G-code is empty or too large".to_owned(),
                ));
            }
        }
        ScriptAction::Jog {
            distance_mm,
            feed_mm_per_min,
            ..
        } => {
            if !distance_mm.is_finite() || *distance_mm == 0.0 || distance_mm.abs() > 50.0 {
                return Err(ScriptPluginError::InvalidAction(
                    "jog distance must be non-zero and within 50 mm".to_owned(),
                ));
            }
            validate_feed(*feed_mm_per_min)?;
        }
        ScriptAction::ReturnZero {
            feed_mm_per_min, ..
        } => validate_feed(*feed_mm_per_min)?,
        ScriptAction::SetZero { .. } => {}
        ScriptAction::RawCommand { command } => {
            let command = command.trim();
            if command.is_empty()
                || command.len() > 255
                || !command.is_ascii()
                || command.chars().any(char::is_control)
                || matches!(command, "!" | "~")
            {
                return Err(ScriptPluginError::InvalidAction(
                    "raw command must be one printable ASCII GRBL line up to 255 bytes; use typed realtime controls for Hold and Resume"
                        .to_owned(),
                ));
            }
        }
        ScriptAction::Notice { title, message, .. } => {
            validate_action_text("notice title", title, 100)?;
            validate_action_text("notice message", message, 1_000)?;
        }
    }
    Ok(())
}

fn normalize_input(
    command: &ScriptPluginCommand,
    input: Value,
) -> Result<Value, ScriptPluginError> {
    let provided = input.as_object().ok_or_else(|| {
        ScriptPluginError::InvalidAction("command input must be an object".to_owned())
    })?;
    if let Some(key) = provided.keys().find(|key| {
        !command
            .fields
            .iter()
            .any(|field| field.id.as_str() == key.as_str())
    }) {
        return Err(ScriptPluginError::InvalidAction(format!(
            "unknown command input: {key}"
        )));
    }
    let mut normalized = serde_json::Map::new();
    for field in &command.fields {
        let value = provided.get(&field.id).unwrap_or(&field.default_value);
        validate_field_value(field, value)?;
        normalized.insert(field.id.clone(), value.clone());
    }
    Ok(Value::Object(normalized))
}

fn validate_field_value(
    field: &ScriptCommandField,
    value: &Value,
) -> Result<(), ScriptPluginError> {
    match field.kind {
        ScriptFieldKind::Number => {
            let number = value
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    ScriptPluginError::InvalidAction(format!(
                        "field {} must be a finite number",
                        field.id
                    ))
                })?;
            if field.min.is_some_and(|min| number < min)
                || field.max.is_some_and(|max| number > max)
            {
                return Err(ScriptPluginError::InvalidAction(format!(
                    "field {} is outside its allowed range",
                    field.id
                )));
            }
        }
        ScriptFieldKind::Boolean if !value.is_boolean() => {
            return Err(ScriptPluginError::InvalidAction(format!(
                "field {} must be a boolean",
                field.id
            )));
        }
        ScriptFieldKind::Text => {
            let text = value.as_str().ok_or_else(|| {
                ScriptPluginError::InvalidAction(format!("field {} must be text", field.id))
            })?;
            if text.len() > MAX_FIELD_TEXT_BYTES {
                return Err(ScriptPluginError::InvalidAction(format!(
                    "field {} exceeds {MAX_FIELD_TEXT_BYTES} bytes",
                    field.id
                )));
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_feed(feed: f64) -> Result<(), ScriptPluginError> {
    if !feed.is_finite() || !(10.0..=100_000.0).contains(&feed) {
        return Err(ScriptPluginError::InvalidAction(
            "feed must be between 10 and 100000 mm/min".to_owned(),
        ));
    }
    Ok(())
}

fn capability_set(
    capabilities: &[ScriptCapability],
    label: &str,
) -> Result<BTreeSet<ScriptCapability>, ScriptPluginError> {
    let set = capabilities.iter().copied().collect::<BTreeSet<_>>();
    if set.len() != capabilities.len() {
        return Err(ScriptPluginError::InvalidPackage(format!(
            "{label} capabilities contain duplicates"
        )));
    }
    Ok(set)
}

fn validate_text(label: &str, value: &str, max: usize) -> Result<(), ScriptPluginError> {
    if value.trim() != value || value.is_empty() || value.len() > max {
        return Err(ScriptPluginError::InvalidPackage(format!(
            "{label} must be a non-empty trimmed string up to {max} bytes"
        )));
    }
    Ok(())
}

fn validate_action_text(label: &str, value: &str, max: usize) -> Result<(), ScriptPluginError> {
    if value.trim() != value || value.is_empty() || value.len() > max {
        return Err(ScriptPluginError::InvalidAction(format!(
            "{label} must be a non-empty trimmed string up to {max} bytes"
        )));
    }
    Ok(())
}

fn valid_plugin_id(value: &str) -> bool {
    value.contains('.')
        && value.len() <= 100
        && value.split(['.', '-']).all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        })
}

fn valid_local_id(value: &str) -> bool {
    value
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase())
        && value.len() <= 64
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

fn valid_semver(value: &str) -> bool {
    let core = value.split_once('-').map_or(value, |(core, _)| core);
    let mut parts = core.split('.');
    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(a), Some(b), Some(c), None)
            if [a, b, c].iter().all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
    )
}

fn read_store_file(path: &Path) -> Result<Option<ScriptPluginStoreFile>, ScriptPluginError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ScriptPluginError::Storage(error.to_string())),
    };
    if metadata.len() > MAX_PLUGIN_STORE_BYTES {
        return Err(ScriptPluginError::Storage(format!(
            "plugin store exceeds {MAX_PLUGIN_STORE_BYTES} bytes"
        )));
    }
    let bytes = fs::read(path).map_err(|error| ScriptPluginError::Storage(error.to_string()))?;
    let file: ScriptPluginStoreFile = serde_json::from_slice(&bytes)
        .map_err(|error| ScriptPluginError::Storage(error.to_string()))?;
    if file.version != STORE_VERSION {
        return Err(ScriptPluginError::Storage(format!(
            "unsupported plugin store version: {}",
            file.version
        )));
    }
    Ok(Some(file))
}

fn remove_file_if_present(path: &Path) -> Result<(), ScriptPluginError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ScriptPluginError::Storage(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    static TEST_ID: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn bundled_macros_compile_and_create_a_parseable_boundary_job() {
        let package = default_macro_package().unwrap();
        validate_package(&package).unwrap();
        let action = ScriptRuntime::execute(
            &package,
            "boundary-check",
            serde_json::json!({
                "originXMm": 0.0,
                "originYMm": 0.0,
                "widthMm": 20.0,
                "heightMm": 20.0,
                "safeZMm": 3.0,
                "feedMmPerMin": 300.0
            }),
            serde_json::json!({ "connection": "connected", "mode": "idle" }),
        )
        .unwrap();
        let job = generated_job(&action).unwrap();

        assert_eq!(job.source_name, "boundary-check-20.0x20.0.nc");
        assert!(job.source.contains("M5\nM9"));
        assert_eq!(job.program.summary.bounds.unwrap().size.x, 20.0);
    }

    #[test]
    fn runtime_rejects_an_unbounded_script() {
        let mut package = default_macro_package().unwrap();
        package.source = "fn run(command, input, machine) { loop {} }".to_owned();
        let error = ScriptRuntime::execute(
            &package,
            "boundary-check",
            serde_json::json!({}),
            Value::Null,
        )
        .unwrap_err();

        assert!(matches!(error, ScriptPluginError::Runtime(_)));
        assert!(error.to_string().contains("operations"));
    }

    #[test]
    fn external_package_is_disabled_and_loses_old_grants_after_update() {
        let mut store = ScriptPluginStore::in_memory().unwrap();
        let mut package = default_macro_package().unwrap();
        package.manifest.id = "community.example".to_owned();
        package.manifest.name = "Example".to_owned();
        let installed = store.install_external(package.clone()).unwrap();
        assert!(!installed.enabled);
        assert!(installed.granted_capabilities.is_empty());

        let enabled = store
            .set_enabled(
                "community.example",
                &installed.digest,
                true,
                package.manifest.capabilities.required.clone(),
            )
            .unwrap();
        assert!(enabled.enabled);

        package.manifest.version = "1.0.1".to_owned();
        let updated = store.install_external(package).unwrap();
        assert!(!updated.enabled);
        assert!(updated.granted_capabilities.is_empty());
        assert_ne!(updated.digest, installed.digest);
    }

    #[test]
    fn digest_must_match_when_capabilities_are_granted() {
        let mut store = ScriptPluginStore::in_memory().unwrap();
        let error = store
            .set_enabled(
                "millo.operator-macros",
                "stale",
                true,
                vec![ScriptCapability::UiContribute],
            )
            .unwrap_err();
        assert!(matches!(error, ScriptPluginError::DigestMismatch));
    }

    #[test]
    fn input_schema_is_enforced_before_script_execution() {
        let package = default_macro_package().unwrap();
        let error = ScriptRuntime::execute(
            &package,
            "raise-z",
            serde_json::json!({ "distanceMm": 500.0, "feedMmPerMin": 300.0 }),
            Value::Null,
        )
        .unwrap_err();

        assert!(error.to_string().contains("outside its allowed range"));
    }

    #[test]
    fn dynamic_eval_is_not_available_to_external_scripts() {
        let mut package = default_macro_package().unwrap();
        package.source = "fn run(command, input, machine) { eval(\"40 + 2\") }".to_owned();
        let error = validate_package(&package).unwrap_err();

        assert!(matches!(error, ScriptPluginError::Compile(_)));
    }

    #[test]
    fn stored_package_integrity_is_checked_before_it_can_be_enabled() {
        let package = default_macro_package().unwrap();
        let error = ScriptPluginStore::with_plugins(
            None,
            vec![InstalledScriptPlugin {
                package,
                digest: "tampered".to_owned(),
                enabled: true,
                bundled: true,
                granted_capabilities: vec![ScriptCapability::UiContribute],
            }],
        )
        .unwrap_err();

        assert!(error.to_string().contains("stored digest does not match"));
    }

    #[test]
    fn package_rejects_hidden_authority_and_invalid_field_metadata() {
        let mut package = default_macro_package().unwrap();
        package.manifest.capabilities.required = vec![ScriptCapability::JobsCreate];
        assert!(
            validate_package(&package)
                .unwrap_err()
                .to_string()
                .contains("ui.contribute")
        );

        let mut package = default_macro_package().unwrap();
        package.commands[0].required_capabilities =
            vec![ScriptCapability::JobsCreate, ScriptCapability::JobsCreate];
        assert!(
            validate_package(&package)
                .unwrap_err()
                .to_string()
                .contains("duplicates")
        );

        let mut package = default_macro_package().unwrap();
        package.commands[0].fields[0].min = Some(f64::NAN);
        assert!(
            validate_package(&package)
                .unwrap_err()
                .to_string()
                .contains("invalid bounds")
        );
    }

    #[test]
    fn runtime_rejects_path_like_job_names_and_oversized_text_input() {
        let mut package = default_macro_package().unwrap();
        package.source = r#"fn run(command, input, machine) {
            #{ kind: "createProgram", sourceName: "../outside.nc", source: "G21\nM30" }
        }"#
        .to_owned();
        let error = ScriptRuntime::execute(
            &package,
            "boundary-check",
            serde_json::json!({}),
            Value::Null,
        )
        .unwrap_err();
        assert!(matches!(error, ScriptPluginError::InvalidAction(_)));
        assert!(error.to_string().contains("plain file name"));

        let mut package = default_macro_package().unwrap();
        package.commands[0].fields = vec![ScriptCommandField {
            id: "note".to_owned(),
            label: "Note".to_owned(),
            kind: ScriptFieldKind::Text,
            default_value: Value::String("ok".to_owned()),
            min: None,
            max: None,
            step: None,
            unit: None,
        }];
        let error = ScriptRuntime::execute(
            &package,
            "boundary-check",
            serde_json::json!({ "note": "x".repeat(MAX_FIELD_TEXT_BYTES + 1) }),
            Value::Null,
        )
        .unwrap_err();
        assert!(error.to_string().contains("exceeds"));
    }

    #[test]
    fn runtime_requires_the_command_to_declare_its_returned_action_capability() {
        let mut package = default_macro_package().unwrap();
        package.commands[1].required_capabilities.clear();
        let error = ScriptRuntime::execute(
            &package,
            "raise-z",
            serde_json::json!({ "distanceMm": 1.0, "feedMmPerMin": 100.0 }),
            Value::Null,
        )
        .unwrap_err();

        assert!(matches!(error, ScriptPluginError::InvalidAction(_)));
        assert!(error.to_string().contains("MachineJog"));
    }

    #[test]
    fn raw_commands_require_the_explicit_capability_and_one_printable_line() {
        let mut package = default_macro_package().unwrap();
        package
            .manifest
            .capabilities
            .optional
            .push(ScriptCapability::MachineCommands);
        package.commands[0].fields.clear();
        package.commands[0].required_capabilities = vec![ScriptCapability::MachineCommands];
        package.source = r#"fn run(command, input, machine) {
            #{ kind: "rawCommand", command: "$SD/Job.nc" }
        }"#
        .to_owned();

        validate_package(&package).unwrap();
        let action = ScriptRuntime::execute(
            &package,
            &package.commands[0].id,
            serde_json::json!({}),
            Value::Null,
        )
        .unwrap();
        assert_eq!(
            action,
            ScriptAction::RawCommand {
                command: "$SD/Job.nc".to_owned(),
            }
        );
        assert_eq!(
            action_capability(&action),
            Some(ScriptCapability::MachineCommands)
        );

        for command in ["!", "~", "G0 X1\nG0 X2", "\u{18}"] {
            assert!(matches!(
                validate_action(&ScriptAction::RawCommand {
                    command: command.to_owned(),
                }),
                Err(ScriptPluginError::InvalidAction(_))
            ));
        }
    }

    #[test]
    fn persistent_store_recovers_from_backup_and_keeps_grants_transactional() {
        let path = test_path("recovery");
        let mut store = ScriptPluginStore::load(&path).unwrap();
        let mut package = default_macro_package().unwrap();
        package.manifest.id = "community.recovery".to_owned();
        package.manifest.name = "Recovery".to_owned();
        let first = store.install_external(package.clone()).unwrap();
        package.manifest.version = "1.0.1".to_owned();
        store.install_external(package).unwrap();
        fs::write(&path, b"corrupt primary").unwrap();

        let recovered = ScriptPluginStore::load(&path).unwrap();
        assert_eq!(
            recovered.get("community.recovery").unwrap().digest,
            first.digest
        );

        let blocked_path = test_path("transactional");
        let mut blocked = ScriptPluginStore::load(&blocked_path).unwrap();
        let blocked_parent = blocked_path.parent().unwrap();
        let _ = fs::remove_file(&blocked_path);
        let _ = fs::remove_file(backup_path(&blocked_path));
        fs::remove_dir(blocked_parent).unwrap();
        fs::write(blocked_parent, b"file blocks directory creation").unwrap();
        let mut package = default_macro_package().unwrap();
        package.manifest.id = "community.transactional".to_owned();
        package.manifest.name = "Transactional".to_owned();
        assert!(matches!(
            blocked.install_external(package),
            Err(ScriptPluginError::Storage(_))
        ));
        assert!(blocked.get("community.transactional").is_none());

        cleanup(&path);
        let _ = fs::remove_file(blocked_parent);
    }

    fn test_path(label: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!(
                "millo-script-{}-{timestamp}-{}",
                std::process::id(),
                TEST_ID.fetch_add(1, Ordering::Relaxed)
            ))
            .join(format!("{label}.json"))
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(backup_path(path));
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir_all(parent);
        }
    }
}
