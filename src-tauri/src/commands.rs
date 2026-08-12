use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex},
    time::{Instant, SystemTime},
};

use millo_audit::{
    AuditCategory, AuditExportFormat, AuditExportOutcome, AuditLevel, AuditLog, AuditLogSnapshot,
};
use millo_cam::{
    GeneratedImageJob, GeneratedSurfacingJob, ImageJobRequest, SurfacingJobRequest,
    generate_image_job as generate_image_job_core,
    generate_surfacing_job as generate_surfacing_job_core,
};
use millo_command::{CommandArbiter, ExecutionTarget};
use millo_controller::ControllerConfig;
use millo_domain::{
    ControllerSnapshot, DeviceInspection, HardwareInspection, HardwareProfile, JogAxis,
    JogPadStepOutcome, JogPadStepRequest, OperatorConfirmation, OverrideAdjustment,
    RapidOverrideTarget, ResetChallenge, ReturnToWorkZeroOutcome, ReturnToWorkZeroRequest,
    StepJogReceipt, StepJogRequest, TestJogPreparation, WorkAxis, WorkZeroOutcome, WorkZeroRequest,
};
use millo_dry_run::{
    DryRunPlan, DryRunPolicyError, ProgramExecutionOptions, ProgramRunPolicy, build_dry_run_plan,
    build_program_run_plan_with_options,
};
use millo_gcode::{
    GcodeProgram, ProgramParseOptions, ProgramParseRequest, parse_program,
    parse_program_with_options,
};
use millo_journal::{RunJournal, RunJournalEntry};
use millo_mock::{MockControl, MockTransport};
use millo_profile::{
    DetectedController, IdentityConfidence, MachineConnectionPreset, MachineFingerprint,
    MachineLocalSettingsUpdate, MachineProfile, MachineProfileDraft, MachineProfileState,
    MachineProfileStore,
};
use millo_recovery::{
    ProgramRecoveryCandidate, ProgramRecoveryPackage, ProgramRecoveryStore, RecoveryContinuity,
    RecoverySeed,
};
use millo_restart::{SafeStartIntent, SafeStartPackage, SafeStartRequest, build_safe_start};
use millo_run::{
    FirstCutConfirmation, FirstCutPreparation, ProgramRunIntent, RunPreflightReport,
    ToolChangeConfirmation, program_fingerprint,
};
use millo_script::{
    InstalledScriptPlugin, ScriptAction, ScriptAxis, ScriptCapability, ScriptGeneratedJob,
    ScriptNoticeTone, ScriptPluginStore, ScriptRuntime, action_capability, generated_job,
    parse_package, read_package,
};
use millo_sender::{SenderMode, SenderSnapshot};
use millo_serial::{
    SerialConfig, SerialPortDescriptor, SerialPortKind, SerialTransport,
    available_ports as available_serial_ports,
};
use millo_settings::{
    ControllerSettingEditRequest, ControllerSettingsSnapshot, MachineSettingsArchive,
    build_settings_snapshot,
};
use millo_tooling::{CuttingToolDraft, ToolLibraryState, ToolLibraryStore};
use millo_transport::BoxedTransport;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tokio::{
    sync::{Mutex, mpsc},
    task::JoinHandle,
};

const MOCK_TRANSPORT_ID: &str = "mock";
const SERIAL_TRANSPORT_PREFIX: &str = "serial:";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedGcodeSaveRequest {
    pub source_name: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedGcodeSaveOutcome {
    pub path: String,
    pub bytes_written: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginSourceRequest {
    pub package_json: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginEnableRequest {
    pub plugin_id: String,
    pub digest: String,
    pub enabled: bool,
    pub granted_capabilities: Vec<ScriptCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginDeleteRequest {
    pub plugin_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginExportRequest {
    pub plugin_id: String,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPluginExecutionRequest {
    pub plugin_id: String,
    pub digest: String,
    pub command_id: String,
    pub input: Value,
    #[serde(default)]
    pub operator_confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ScriptPluginExecutionOutcome {
    Job {
        job: ScriptGeneratedJob,
    },
    Machine {
        action: String,
        message: String,
        snapshot: ControllerSnapshot,
    },
    Notice {
        title: String,
        message: String,
        tone: ScriptNoticeTone,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedRunPreparationRequest {
    pub request: ProgramParseRequest,
    pub selected_source_line: usize,
    pub safe_z_mm: f64,
    pub intent: ProgramRunIntent,
    pub execution_options: ProgramExecutionOptions,
}

fn audit_operation<T>(
    audit: &AuditLog,
    category: AuditCategory,
    operation: &str,
    success_message: &str,
    data: Value,
    result: &Result<T, String>,
) {
    match result {
        Ok(_) => {
            audit.record(
                AuditLevel::Info,
                category,
                format!("{operation}.completed"),
                success_message,
                data,
            );
        }
        Err(error) => {
            audit.record(
                AuditLevel::Error,
                category,
                format!("{operation}.failed"),
                error,
                json!({ "context": data }),
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportKind {
    Mock,
    Serial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportDescriptor {
    pub id: String,
    pub kind: TransportKind,
    pub label: String,
    pub detail: Option<String>,
    pub port_name: Option<String>,
    pub likely_grbl: bool,
    pub match_reason: Option<String>,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerSettingsState {
    pub snapshot: ControllerSettingsSnapshot,
    pub session_baseline: BTreeMap<String, String>,
    pub previous_baseline: Option<BTreeMap<String, String>>,
    pub revision_count: usize,
    pub profile_id: Option<String>,
    pub fingerprint: MachineFingerprint,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectOutcome {
    pub snapshot: ControllerSnapshot,
    pub inspection: HardwareInspection,
    pub settings: ControllerSettingsState,
    pub profiles: MachineProfileState,
    pub onboarding_draft: Option<MachineProfileDraft>,
}

struct ResolvedTransport {
    transport: BoxedTransport,
    descriptor: TransportDescriptor,
    mock: Option<MockControl>,
    execution_target: ExecutionTarget,
}

struct ActiveControllerSettings {
    inspection: DeviceInspection,
    fingerprint: MachineFingerprint,
    connection: MachineConnectionPreset,
    profile_id: Option<String>,
    archive: Option<MachineSettingsArchive>,
    revision: u64,
}

impl ResolvedTransport {
    fn mock() -> Self {
        let transport = MockTransport::default();
        let mock = transport.control();
        Self {
            transport: Box::new(transport),
            descriptor: mock_descriptor(),
            mock: Some(mock),
            execution_target: ExecutionTarget::Mock,
        }
    }
}

pub struct AppState {
    arbiter: CommandArbiter,
    audit: AuditLog,
    profiles: Mutex<MachineProfileStore>,
    tools: Mutex<ToolLibraryStore>,
    active_transport: Mutex<TransportDescriptor>,
    mock: Mutex<Option<MockControl>>,
    transition_lock: Mutex<()>,
    event_task: Mutex<Option<JoinHandle<()>>>,
    settings_root: Option<PathBuf>,
    settings_session: Mutex<Option<ActiveControllerSettings>>,
    run_journal: Arc<StdMutex<RunJournal>>,
    program_recovery: Arc<StdMutex<ProgramRecoveryStore>>,
    script_plugins: Mutex<ScriptPluginStore>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::from_profile_store(
            MachineProfileStore::in_memory(),
            ToolLibraryStore::in_memory(),
            None,
            RunJournal::in_memory(),
            ProgramRecoveryStore::in_memory(),
            ScriptPluginStore::in_memory().expect("bundled script plugin must be valid"),
            AuditLog::in_memory(),
        )
    }
}

impl AppState {
    pub fn load(profile_path: impl Into<PathBuf>) -> Result<Self, String> {
        let profile_path = profile_path.into();
        let settings_root = profile_path.parent().map(|parent| parent.join("machines"));
        let journal_path = profile_path
            .parent()
            .map(|parent| parent.join("sender-runs.json"));
        let recovery_path = profile_path
            .parent()
            .map(|parent| parent.join("active-program-recovery.json"));
        let tool_library_path = profile_path
            .parent()
            .map(|parent| parent.join("cutting-tools.json"));
        let script_plugins_path = profile_path
            .parent()
            .map(|parent| parent.join("script-plugins.json"));
        let audit = match profile_path
            .parent()
            .map(|parent| AuditLog::persistent(parent.join("logs")))
        {
            Some(Ok(audit)) => audit,
            Some(Err(error)) => {
                let audit = AuditLog::in_memory();
                audit.record(
                    AuditLevel::Critical,
                    AuditCategory::Storage,
                    "storage.audit_initialization_failed",
                    error.to_string(),
                    json!({ "fallback": "inMemory" }),
                );
                audit
            }
            None => AuditLog::in_memory(),
        };
        let profiles =
            MachineProfileStore::load(profile_path).map_err(|error| error.to_string())?;
        let tools = tool_library_path
            .map(ToolLibraryStore::load)
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_else(ToolLibraryStore::in_memory);
        let journal = journal_path
            .map(RunJournal::load)
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_else(RunJournal::in_memory);
        let recovery = recovery_path
            .map(ProgramRecoveryStore::load)
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_else(ProgramRecoveryStore::in_memory);
        let script_plugins = match script_plugins_path.map(ScriptPluginStore::load).transpose() {
            Ok(Some(store)) => store,
            Ok(None) => ScriptPluginStore::in_memory().map_err(|error| error.to_string())?,
            Err(error) => {
                audit.record(
                    AuditLevel::Critical,
                    AuditCategory::Storage,
                    "storage.script_plugin_initialization_failed",
                    error.to_string(),
                    json!({ "fallback": "bundledOnlyInMemory" }),
                );
                ScriptPluginStore::in_memory().map_err(|error| error.to_string())?
            }
        };
        let state = Self::from_profile_store(
            profiles,
            tools,
            settings_root,
            journal,
            recovery,
            script_plugins,
            audit,
        );
        state.audit.record(
            AuditLevel::Info,
            AuditCategory::Application,
            "application.started",
            "Millo desktop backend started",
            json!({ "version": env!("CARGO_PKG_VERSION") }),
        );
        Ok(state)
    }

    fn from_profile_store(
        profiles: MachineProfileStore,
        tools: ToolLibraryStore,
        settings_root: Option<PathBuf>,
        run_journal: RunJournal,
        program_recovery: ProgramRecoveryStore,
        script_plugins: ScriptPluginStore,
        audit: AuditLog,
    ) -> Self {
        let initial = ResolvedTransport::mock();
        let descriptor = initial.descriptor;
        let mock = initial.mock;
        let hardware_profile = profiles
            .state()
            .selected()
            .map(|profile| profile.hardware_profile())
            .unwrap_or_else(HardwareProfile::first_machine);
        let (arbiter, worker) = CommandArbiter::new_with_execution_target(
            initial.transport,
            ControllerConfig::default(),
            hardware_profile,
            initial.execution_target,
        );
        tauri::async_runtime::spawn(worker);

        Self {
            arbiter,
            audit,
            profiles: Mutex::new(profiles),
            tools: Mutex::new(tools),
            active_transport: Mutex::new(descriptor),
            mock: Mutex::new(mock),
            transition_lock: Mutex::new(()),
            event_task: Mutex::new(None),
            settings_root,
            settings_session: Mutex::new(None),
            run_journal: Arc::new(StdMutex::new(run_journal)),
            program_recovery: Arc::new(StdMutex::new(program_recovery)),
            script_plugins: Mutex::new(script_plugins),
        }
    }

    async fn start_event_bridge(&self, app: AppHandle) {
        let mut event_task = self.event_task.lock().await;
        if event_task.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }

        let mut snapshots = self.arbiter.subscribe();
        let mut sender_snapshots = self.arbiter.subscribe_sender();
        let persistence_sender = start_run_persistence_worker(
            Arc::clone(&self.run_journal),
            Arc::clone(&self.program_recovery),
            self.audit.clone(),
        );
        let audit = self.audit.clone();
        *event_task = Some(tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = snapshots.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        let snapshot = snapshots.borrow_and_update().clone();
                        let level = if snapshot.alarm.is_some() || snapshot.last_error.is_some() {
                            AuditLevel::Error
                        } else if snapshot.reset_notice.is_some() {
                            AuditLevel::Warning
                        } else {
                            AuditLevel::Debug
                        };
                        audit.record(
                            level,
                            AuditCategory::Controller,
                            "controller.snapshot",
                            format!("{:?} / {:?}", snapshot.connection, snapshot.machine.mode),
                            serde_json::to_value(&snapshot).unwrap_or(Value::Null),
                        );
                        if let Err(error) = app.emit("machine-state", snapshot) {
                            audit.record(
                                AuditLevel::Error,
                                AuditCategory::Ui,
                                "ui.machine_state_emit_failed",
                                error.to_string(),
                                Value::Null,
                            );
                        }
                    }
                    changed = sender_snapshots.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        let snapshot = sender_snapshots.borrow_and_update().clone();
                        let level = if snapshot.failure.is_some() {
                            AuditLevel::Error
                        } else if matches!(snapshot.state, millo_sender::SenderState::ToolChange) {
                            AuditLevel::Warning
                        } else if matches!(
                            snapshot.state,
                            millo_sender::SenderState::Completed
                                | millo_sender::SenderState::Failed
                                | millo_sender::SenderState::Cancelled
                        ) {
                            AuditLevel::Info
                        } else {
                            AuditLevel::Debug
                        };
                        audit.record(
                            level,
                            AuditCategory::Sender,
                            "sender.snapshot",
                            format!("{:?} at source line {:?}", snapshot.state, snapshot.current_source_line),
                            serde_json::to_value(&snapshot).unwrap_or(Value::Null),
                        );
                        if let Err(error) = app.emit("dry-run-state", snapshot.clone()) {
                            audit.record(
                                AuditLevel::Error,
                                AuditCategory::Ui,
                                "ui.sender_state_emit_failed",
                                error.to_string(),
                                Value::Null,
                            );
                        }
                        if let Some(sender) = persistence_sender.as_ref()
                            && sender.send(snapshot).await.is_err()
                        {
                            audit.record(
                                AuditLevel::Critical,
                                AuditCategory::Storage,
                                "storage.sender_journal_worker_stopped",
                                "Sender journal worker stopped unexpectedly",
                                Value::Null,
                            );
                        }
                    }
                }
            }
        }));
    }
}

fn start_run_persistence_worker(
    journal: Arc<StdMutex<RunJournal>>,
    recovery: Arc<StdMutex<ProgramRecoveryStore>>,
    audit: AuditLog,
) -> Option<mpsc::Sender<SenderSnapshot>> {
    let (sender, mut snapshots) = mpsc::channel::<SenderSnapshot>(128);
    let worker_audit = audit.clone();
    let worker = std::thread::Builder::new()
        .name("millo-run-journal".to_owned())
        .spawn(move || {
            while let Some(snapshot) = snapshots.blocking_recv() {
                match journal.lock() {
                    Ok(mut journal) => {
                        if let Err(error) =
                            journal.observe(&snapshot, SystemTime::now(), Instant::now())
                        {
                            worker_audit.record(
                                AuditLevel::Error,
                                AuditCategory::Storage,
                                "storage.sender_journal_failed",
                                error.to_string(),
                                json!({ "runSequence": snapshot.run_sequence }),
                            );
                        }
                    }
                    Err(error) => {
                        worker_audit.record(
                            AuditLevel::Critical,
                            AuditCategory::Storage,
                            "storage.sender_journal_lock_poisoned",
                            error.to_string(),
                            Value::Null,
                        );
                    }
                }
                match recovery.lock() {
                    Ok(mut recovery) => {
                        if let Err(error) =
                            recovery.observe(&snapshot, SystemTime::now(), Instant::now())
                        {
                            worker_audit.record(
                                AuditLevel::Error,
                                AuditCategory::Storage,
                                "storage.program_recovery_failed",
                                error.to_string(),
                                json!({ "runSequence": snapshot.run_sequence }),
                            );
                        }
                    }
                    Err(error) => {
                        worker_audit.record(
                            AuditLevel::Critical,
                            AuditCategory::Storage,
                            "storage.program_recovery_lock_poisoned",
                            error.to_string(),
                            Value::Null,
                        );
                    }
                }
            }
        });
    match worker {
        Ok(_) => Some(sender),
        Err(error) => {
            audit.record(
                AuditLevel::Critical,
                AuditCategory::Storage,
                "storage.persistence_worker_start_failed",
                error.to_string(),
                Value::Null,
            );
            None
        }
    }
}

#[tauri::command]
pub async fn sender_run_history(
    state: State<'_, AppState>,
) -> Result<Vec<RunJournalEntry>, String> {
    let journal = Arc::clone(&state.run_journal);
    tokio::task::spawn_blocking(move || {
        journal
            .lock()
            .map(|journal| journal.entries().to_vec())
            .map_err(|error| format!("sender journal lock poisoned: {error}"))
    })
    .await
    .map_err(|error| format!("sender journal history task failed: {error}"))?
}

#[tauri::command]
pub async fn diagnostic_log_snapshot(
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<AuditLogSnapshot, String> {
    Ok(state.audit.snapshot(limit.unwrap_or(500).clamp(1, 2_000)))
}

#[tauri::command]
pub async fn export_diagnostic_log(
    format: AuditExportFormat,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<AuditExportOutcome>, String> {
    let (selection, selected) = tokio::sync::oneshot::channel();
    let (file_name, filter_name, extensions): (&str, &str, &[&str]) = match format {
        AuditExportFormat::JsonLines => ("millo-diagnostic-log.jsonl", "JSON Lines", &["jsonl"]),
        AuditExportFormat::Text => ("millo-diagnostic-log.log", "Text log", &["log", "txt"]),
    };
    app.dialog()
        .file()
        .set_file_name(file_name)
        .add_filter(filter_name, extensions)
        .save_file(move |path| {
            let _ = selection.send(path);
        });
    let Some(path) = selected
        .await
        .map_err(|_| "diagnostic log save dialog closed unexpectedly".to_owned())?
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|error| error.to_string())?;
    let audit = state.audit.clone();
    let export_path = path.clone();
    let outcome = tokio::task::spawn_blocking(move || audit.export(export_path, format))
        .await
        .map_err(|error| format!("diagnostic log export task failed: {error}"))?
        .map_err(|error| error.to_string())?;
    state.audit.record(
        AuditLevel::Info,
        AuditCategory::Storage,
        "storage.audit_exported",
        "Diagnostic log exported",
        json!({
            "path": path,
            "format": format,
            "entryCount": outcome.entry_count,
        }),
    );
    Ok(Some(outcome))
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramRecoveryPreparationRequest {
    pub recovery_id: u64,
    pub safe_z_mm: f64,
    pub continuity: RecoveryContinuity,
    pub machine_reference_restored: bool,
    pub work_zero_restored: bool,
    pub motion_power_restored: bool,
    pub restart_point_inspected: bool,
    pub path_clear: bool,
    pub power_control_reachable: bool,
}

impl ProgramRecoveryPreparationRequest {
    fn missing(self) -> Vec<&'static str> {
        [
            (!self.machine_reference_restored)
                .then_some("machine reference restored after power loss"),
            (!self.work_zero_restored).then_some("work zero restored"),
            (!self.motion_power_restored).then_some("motion power and physical position verified"),
            (!self.restart_point_inspected).then_some("restart point inspected in preview"),
            (!self.path_clear).then_some("clearance route and repeated path are clear"),
            (!self.power_control_reachable).then_some("machine power control reachable"),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
}

#[tauri::command]
pub async fn program_recovery_candidate(
    state: State<'_, AppState>,
) -> Result<Option<ProgramRecoveryCandidate>, String> {
    let snapshot = state.arbiter.sender_snapshot();
    let recovery = Arc::clone(&state.program_recovery);
    tokio::task::spawn_blocking(move || {
        let mut recovery = recovery
            .lock()
            .map_err(|error| format!("program recovery lock poisoned: {error}"))?;
        recovery
            .observe(&snapshot, SystemTime::now(), Instant::now())
            .map_err(|error| error.to_string())?;
        recovery.candidate().map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("program recovery candidate task failed: {error}"))?
}

#[tauri::command]
pub async fn prepare_program_recovery(
    request: ProgramRecoveryPreparationRequest,
    state: State<'_, AppState>,
) -> Result<ProgramRecoveryPackage, String> {
    let _transition = state.transition_lock.lock().await;
    let missing = request.missing();
    if !missing.is_empty() {
        return Err(format!(
            "program recovery confirmation is incomplete: {missing:?}"
        ));
    }
    ensure_machine_bound(&state).await?;
    if state.active_transport.lock().await.kind != TransportKind::Serial {
        return Err("program recovery requires an active serial transport".to_owned());
    }
    let snapshot = state
        .arbiter
        .refresh_status()
        .await
        .map_err(|error| error.to_string())?;
    if snapshot.connection != millo_domain::ConnectionState::Connected
        || snapshot.machine.mode != millo_domain::MachineMode::Idle
        || snapshot.alarm.is_some()
        || snapshot.reset_notice.is_some()
    {
        return Err("program recovery requires fresh Connected + Idle state".to_owned());
    }
    state
        .arbiter
        .inspect_device()
        .await
        .map_err(|error| error.to_string())?;
    let fingerprint = state
        .settings_session
        .lock()
        .await
        .as_ref()
        .map(|session| session.fingerprint.key.clone())
        .ok_or_else(|| "controller settings have not been synchronized".to_owned())?;
    let recovery = Arc::clone(&state.program_recovery);
    tokio::task::spawn_blocking(move || {
        let mut recovery = recovery
            .lock()
            .map_err(|error| format!("program recovery lock poisoned: {error}"))?;
        if !recovery.machine_matches(request.recovery_id, &fingerprint) {
            return Err("interrupted job belongs to a different controller".to_owned());
        }
        recovery
            .prepare(request.recovery_id, request.safe_z_mm, request.continuity)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("program recovery preparation task failed: {error}"))?
}

#[tauri::command]
pub async fn dismiss_program_recovery(
    recovery_id: u64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let recovery = Arc::clone(&state.program_recovery);
    tokio::task::spawn_blocking(move || {
        recovery
            .lock()
            .map_err(|error| format!("program recovery lock poisoned: {error}"))?
            .dismiss(recovery_id)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("program recovery dismissal task failed: {error}"))?
}

#[tauri::command]
pub async fn machine_profiles(state: State<'_, AppState>) -> Result<MachineProfileState, String> {
    Ok(state.profiles.lock().await.state())
}

#[tauri::command]
pub async fn tool_library(state: State<'_, AppState>) -> Result<ToolLibraryState, String> {
    Ok(state.tools.lock().await.state())
}

#[tauri::command]
pub async fn create_cutting_tool(
    draft: CuttingToolDraft,
    state: State<'_, AppState>,
) -> Result<ToolLibraryState, String> {
    let context = json!({ "name": &draft.name, "kind": draft.kind });
    let result = state
        .tools
        .lock()
        .await
        .create(draft)
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Storage,
        "storage.tool_created",
        "Cutting tool added to the library",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn update_cutting_tool(
    tool_id: String,
    draft: CuttingToolDraft,
    state: State<'_, AppState>,
) -> Result<ToolLibraryState, String> {
    let context = json!({ "toolId": &tool_id, "name": &draft.name, "kind": draft.kind });
    let result = state
        .tools
        .lock()
        .await
        .update(&tool_id, draft)
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Storage,
        "storage.tool_updated",
        "Cutting tool updated",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn delete_cutting_tool(
    tool_id: String,
    state: State<'_, AppState>,
) -> Result<ToolLibraryState, String> {
    let context = json!({ "toolId": &tool_id });
    let result = state
        .tools
        .lock()
        .await
        .delete(&tool_id)
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Storage,
        "storage.tool_deleted",
        "Cutting tool removed from the library",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn restore_cutting_tool_presets(
    state: State<'_, AppState>,
) -> Result<ToolLibraryState, String> {
    let result = state
        .tools
        .lock()
        .await
        .restore_missing_presets()
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Storage,
        "storage.tool_presets_restored",
        "Missing cutting-tool presets restored",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn create_machine_profile(
    mut draft: MachineProfileDraft,
    state: State<'_, AppState>,
) -> Result<MachineProfileState, String> {
    let _transition = state.transition_lock.lock().await;
    let connected =
        state.arbiter.snapshot().connection != millo_domain::ConnectionState::Disconnected;
    if connected {
        let session = state.settings_session.lock().await;
        let session = session
            .as_ref()
            .ok_or_else(|| "controller settings have not been synchronized".to_owned())?;
        if session.profile_id.is_some() {
            return Err(
                "the connected controller is already bound to a machine profile".to_owned(),
            );
        }
        let snapshot = build_settings_snapshot(&session.inspection, session.revision);
        draft.travel_mm = snapshot
            .travel_mm()
            .ok_or_else(|| "controller did not report valid $130/$131/$132 travel".to_owned())?;
        draft.connection = Some(session.connection.clone());
        draft.detected_controller = Some(detected_controller(&session.inspection));
    } else {
        ensure_profile_change_available(&state)?;
    }
    let next = state
        .profiles
        .lock()
        .await
        .create_and_select(draft)
        .map_err(|error| error.to_string())?;
    let profile = next
        .selected()
        .ok_or_else(|| "profile store did not select the newly created profile".to_owned())?
        .hardware_profile();
    if connected {
        state
            .arbiter
            .bind_hardware_profile(profile)
            .await
            .map_err(|error| error.to_string())?;
        let selected = next
            .selected()
            .ok_or_else(|| "newly created profile lost its selection".to_owned())?;
        let mut session = state.settings_session.lock().await;
        let active = session
            .as_mut()
            .ok_or_else(|| "controller settings session ended during onboarding".to_owned())?;
        active.profile_id = Some(selected.id.clone());
        active.archive = begin_settings_archive(&state, selected, active)?;
    } else {
        state
            .arbiter
            .set_hardware_profile(profile)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(next)
}

#[tauri::command]
pub async fn update_machine_local_settings(
    profile_id: String,
    update: MachineLocalSettingsUpdate,
    state: State<'_, AppState>,
) -> Result<MachineProfileState, String> {
    let _transition = state.transition_lock.lock().await;
    let connected =
        state.arbiter.snapshot().connection != millo_domain::ConnectionState::Disconnected;
    if connected {
        let session = state.settings_session.lock().await;
        if session
            .as_ref()
            .and_then(|active| active.profile_id.as_deref())
            != Some(profile_id.as_str())
        {
            return Err(
                "only the profile bound to the connected controller can be edited".to_owned(),
            );
        }
    }
    let next = state
        .profiles
        .lock()
        .await
        .update_local_settings(&profile_id, update)
        .map_err(|error| error.to_string())?;
    let profile = next
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "updated profile disappeared from the profile store".to_owned())?
        .hardware_profile();
    if connected {
        state
            .arbiter
            .bind_hardware_profile(profile)
            .await
            .map_err(|error| error.to_string())?;
    } else if next.selected_profile_id.as_deref() == Some(profile_id.as_str()) {
        state
            .arbiter
            .set_hardware_profile(profile)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(next)
}

#[tauri::command]
pub async fn select_machine_profile(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<MachineProfileState, String> {
    let _transition = state.transition_lock.lock().await;
    ensure_profile_change_available(&state)?;
    let next = state
        .profiles
        .lock()
        .await
        .select(&profile_id)
        .map_err(|error| error.to_string())?;
    let profile = next
        .selected()
        .ok_or_else(|| "profile store did not retain the requested selection".to_owned())?
        .hardware_profile();
    state
        .arbiter
        .set_hardware_profile(profile)
        .await
        .map_err(|error| error.to_string())?;
    Ok(next)
}

#[tauri::command]
pub async fn detect_machine_profile(
    transport_id: String,
    baud_rate: u32,
    state: State<'_, AppState>,
) -> Result<MachineProfileDraft, String> {
    let _transition = state.transition_lock.lock().await;
    ensure_profile_change_available(&state)?;
    let resolved = resolve_transport(&transport_id, baud_rate).await?;
    let descriptor = resolved.descriptor.clone();
    let (arbiter, worker) = CommandArbiter::new_with_execution_target(
        resolved.transport,
        ControllerConfig::default(),
        HardwareProfile::first_machine(),
        resolved.execution_target,
    );
    let worker = tokio::spawn(worker);

    let result = async {
        arbiter.connect().await.map_err(|error| error.to_string())?;
        let snapshot = arbiter
            .refresh_status()
            .await
            .map_err(|error| error.to_string())?;
        if snapshot.reset_notice.is_some() {
            arbiter
                .acknowledge_reset()
                .await
                .map_err(|error| error.to_string())?;
        }
        let inspection = arbiter
            .inspect_device()
            .await
            .map_err(|error| error.to_string())?;
        let fingerprint = machine_fingerprint(&descriptor, &inspection.device);
        MachineProfileDraft::from_grbl_inspection(
            suggested_machine_name(&descriptor, &inspection.device),
            &inspection.device,
            MachineConnectionPreset {
                transport_id: descriptor.id.clone(),
                baud_rate,
                fingerprint: Some(fingerprint),
            },
        )
        .map_err(|error| error.to_string())
    }
    .await;

    let _ = arbiter.disconnect().await;
    worker.abort();
    result
}

#[tauri::command]
pub async fn list_transports() -> Result<Vec<TransportDescriptor>, String> {
    let serial_ports = tokio::task::spawn_blocking(available_serial_ports)
        .await
        .map_err(|error| format!("serial discovery task failed: {error}"))?
        .map_err(|error| error.to_string())?;

    let mut transports = vec![mock_descriptor()];
    transports.extend(serial_ports.into_iter().map(serial_descriptor));
    Ok(transports)
}

#[tauri::command]
pub async fn active_transport(state: State<'_, AppState>) -> Result<TransportDescriptor, String> {
    Ok(state.active_transport.lock().await.clone())
}

#[tauri::command]
pub async fn controller_snapshot(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    Ok(state.arbiter.snapshot())
}

#[tauri::command]
pub async fn connect_transport(
    transport_id: String,
    baud_rate: u32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ConnectOutcome, String> {
    let _transition = state.transition_lock.lock().await;
    state.audit.record(
        AuditLevel::Info,
        AuditCategory::Transport,
        "transport.connect.requested",
        "Controller connection requested",
        json!({ "transportId": &transport_id, "baudRate": baud_rate }),
    );
    state.start_event_bridge(app).await;
    let replacement = match resolve_transport(&transport_id, baud_rate).await {
        Ok(replacement) => replacement,
        Err(error) => {
            state.audit.record(
                AuditLevel::Error,
                AuditCategory::Transport,
                "transport.resolve.failed",
                &error,
                json!({ "transportId": &transport_id, "baudRate": baud_rate }),
            );
            return Err(error);
        }
    };
    let descriptor = replacement.descriptor.clone();

    if let Err(error) = state
        .arbiter
        .replace_transport_with_execution_target(
            replacement.transport,
            replacement.execution_target,
        )
        .await
        .map_err(|error| error.to_string())
    {
        state.audit.record(
            AuditLevel::Error,
            AuditCategory::Transport,
            "transport.replace.failed",
            &error,
            json!({ "transportId": &transport_id, "baudRate": baud_rate }),
        );
        return Err(error);
    }
    *state.settings_session.lock().await = None;
    *state.active_transport.lock().await = descriptor.clone();
    *state.mock.lock().await = replacement.mock;

    let result = async {
        state
            .arbiter
            .connect()
            .await
            .map_err(|error| error.to_string())?;
        let snapshot = state
            .arbiter
            .refresh_status()
            .await
            .map_err(|error| error.to_string())?;
        state
            .arbiter
            .bind_hardware_profile(HardwareProfile::first_machine())
            .await
            .map_err(|error| error.to_string())?;
        let initial_inspection = state
            .arbiter
            .inspect_device()
            .await
            .map_err(|error| error.to_string())?;
        let fingerprint = machine_fingerprint(&descriptor, &initial_inspection.device);
        let connection = MachineConnectionPreset {
            transport_id: descriptor.id.clone(),
            baud_rate,
            fingerprint: Some(fingerprint.clone()),
        };
        let profile_match = if descriptor.kind == TransportKind::Serial {
            let profiles = state.profiles.lock().await.state();
            match_machine_profile(
                &profiles,
                &fingerprint,
                &descriptor.id,
                &initial_inspection.device,
            )?
        } else {
            None
        };

        let mut profile_id = None;
        let mut archive = None;
        if let Some(profile) = profile_match.as_ref() {
            state
                .arbiter
                .bind_hardware_profile(profile.hardware_profile())
                .await
                .map_err(|error| error.to_string())?;
            let travel = build_settings_snapshot(&initial_inspection.device, 1)
                .travel_mm()
                .ok_or_else(|| {
                    "controller did not report valid $130/$131/$132 travel".to_owned()
                })?;
            let profiles = state
                .profiles
                .lock()
                .await
                .record_controller_observation(
                    &profile.id,
                    travel,
                    connection.clone(),
                    detected_controller(&initial_inspection.device),
                )
                .map_err(|error| error.to_string())?;
            let refreshed_profile = profiles
                .profiles
                .iter()
                .find(|candidate| candidate.id == profile.id)
                .ok_or_else(|| "observed profile disappeared from the profile store".to_owned())?;
            let temporary_session = ActiveControllerSettings {
                inspection: initial_inspection.device.clone(),
                fingerprint: fingerprint.clone(),
                connection: connection.clone(),
                profile_id: Some(profile.id.clone()),
                archive: None,
                revision: 1,
            };
            archive = begin_settings_archive(&state, refreshed_profile, &temporary_session)?;
            profile_id = Some(profile.id.clone());
        }

        let inspection = if profile_id.is_some() {
            state
                .arbiter
                .inspect_device()
                .await
                .map_err(|error| error.to_string())?
        } else {
            initial_inspection
        };
        let onboarding_draft = if descriptor.kind == TransportKind::Serial && profile_id.is_none() {
            Some(
                MachineProfileDraft::from_grbl_inspection(
                    suggested_machine_name(&descriptor, &inspection.device),
                    &inspection.device,
                    connection.clone(),
                )
                .map_err(|error| error.to_string())?,
            )
        } else {
            None
        };
        if let Some(settings_archive) = archive.as_mut() {
            settings_archive
                .record_observation(&inspection.device)
                .map_err(|error| error.to_string())?;
        }
        let active = ActiveControllerSettings {
            inspection: inspection.device.clone(),
            fingerprint,
            connection,
            profile_id,
            archive,
            revision: 1,
        };
        let settings = settings_state(&active);
        *state.settings_session.lock().await = Some(active);
        Ok(ConnectOutcome {
            snapshot,
            inspection,
            settings,
            profiles: state.profiles.lock().await.state(),
            onboarding_draft,
        })
    }
    .await;

    match result {
        Ok(outcome) => {
            state.audit.record(
                AuditLevel::Info,
                AuditCategory::Transport,
                "transport.connect.completed",
                "Controller connected and synchronized",
                json!({
                    "transport": descriptor,
                    "firmwareVersion": &outcome.inspection.device.firmware_version,
                    "firmwareBuildInfo": &outcome.inspection.device.firmware_build_info,
                    "profileId": &outcome.settings.profile_id,
                    "machineMode": outcome.snapshot.machine.mode,
                }),
            );
            Ok(outcome)
        }
        Err(connection_error) => {
            *state.settings_session.lock().await = None;
            match state.arbiter.disconnect().await {
                Ok(_) => {
                    state.audit.record(
                        AuditLevel::Error,
                        AuditCategory::Transport,
                        "transport.connect.failed",
                        &connection_error,
                        json!({ "transport": descriptor }),
                    );
                    Err(connection_error)
                }
                Err(cleanup_error) => {
                    let error = format!(
                        "{connection_error}; connection cleanup also failed: {cleanup_error}"
                    );
                    state.audit.record(
                        AuditLevel::Critical,
                        AuditCategory::Transport,
                        "transport.connect_cleanup.failed",
                        &error,
                        json!({ "transport": descriptor }),
                    );
                    Err(error)
                }
            }
        }
    }
}

fn ensure_profile_change_available(state: &AppState) -> Result<(), String> {
    let connection = state.arbiter.snapshot().connection;
    if connection == millo_domain::ConnectionState::Disconnected {
        Ok(())
    } else {
        Err(format!(
            "machine profiles can be changed only while disconnected, current state is {connection:?}"
        ))
    }
}

async fn apply_controller_setting(
    state: &AppState,
    request: ControllerSettingEditRequest,
) -> Result<ControllerSettingsState, String> {
    ensure_machine_bound(state).await?;
    let expected_revision = request
        .expected_revision
        .ok_or_else(|| "controller setting edit is missing its source revision".to_owned())?;
    if request.expected_value.is_none() {
        return Err("controller setting edit is missing its source value".to_owned());
    }
    {
        let session = state.settings_session.lock().await;
        let active = session
            .as_ref()
            .ok_or_else(|| "connect and synchronize a controller first".to_owned())?;
        if active.revision != expected_revision {
            return Err(format!(
                "controller settings changed: expected revision {expected_revision}, current revision is {}",
                active.revision
            ));
        }
    }

    let verified = state
        .arbiter
        .update_controller_setting(request)
        .await
        .map_err(|error| error.to_string())?;
    let mut session = state.settings_session.lock().await;
    let active = session
        .as_mut()
        .ok_or_else(|| "controller settings session ended during verification".to_owned())?;
    if active.revision != expected_revision {
        return Err("controller settings changed while the write was in flight".to_owned());
    }
    active.inspection = verified.inspection;
    active.revision = active.revision.saturating_add(1);
    if let Some(archive) = active.archive.as_mut() {
        archive
            .record_observation(&active.inspection)
            .map_err(|error| error.to_string())?;
    }

    let profile_to_bind = if let Some(profile_id) = active.profile_id.as_deref() {
        if let Some(travel) =
            build_settings_snapshot(&active.inspection, active.revision).travel_mm()
        {
            let profiles = state
                .profiles
                .lock()
                .await
                .record_controller_observation(
                    profile_id,
                    travel,
                    active.connection.clone(),
                    detected_controller(&active.inspection),
                )
                .map_err(|error| error.to_string())?;
            profiles
                .profiles
                .iter()
                .find(|profile| profile.id == profile_id)
                .map(MachineProfile::hardware_profile)
        } else {
            None
        }
    } else {
        None
    };
    let next = settings_state(active);
    drop(session);
    if let Some(profile) = profile_to_bind {
        state
            .arbiter
            .bind_hardware_profile(profile)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(next)
}

async fn ensure_machine_bound(state: &AppState) -> Result<(), String> {
    if state.active_transport.lock().await.kind == TransportKind::Mock {
        return Ok(());
    }
    if state
        .settings_session
        .lock()
        .await
        .as_ref()
        .and_then(|session| session.profile_id.as_ref())
        .is_some()
    {
        Ok(())
    } else {
        Err(
            "the connected controller must be identified and bound to a machine profile first"
                .to_owned(),
        )
    }
}

fn settings_state(active: &ActiveControllerSettings) -> ControllerSettingsState {
    let (session_baseline, previous_baseline, revision_count) = active
        .archive
        .as_ref()
        .map(|archive| {
            let state = archive.state();
            (
                state.active.baseline.clone(),
                state
                    .revisions
                    .last()
                    .map(|revision| revision.values.clone()),
                state.revisions.len(),
            )
        })
        .unwrap_or_else(|| (active.inspection.settings.clone(), None, 0));
    ControllerSettingsState {
        snapshot: build_settings_snapshot(&active.inspection, active.revision),
        session_baseline,
        previous_baseline,
        revision_count,
        profile_id: active.profile_id.clone(),
        fingerprint: active.fingerprint.clone(),
    }
}

fn begin_settings_archive(
    state: &AppState,
    profile: &MachineProfile,
    active: &ActiveControllerSettings,
) -> Result<Option<MachineSettingsArchive>, String> {
    let Some(root) = state.settings_root.as_ref() else {
        return Ok(None);
    };
    MachineSettingsArchive::begin(
        root.join(format!("{}.settings.json", profile.id)),
        profile.id.clone(),
        active.fingerprint.key.clone(),
        &active.inspection,
    )
    .map(Some)
    .map_err(|error| error.to_string())
}

fn detected_controller(inspection: &DeviceInspection) -> DetectedController {
    DetectedController {
        firmware_version: inspection.firmware_version.clone(),
        firmware_build_info: inspection.firmware_build_info.clone(),
    }
}

fn match_machine_profile(
    profiles: &MachineProfileState,
    fingerprint: &MachineFingerprint,
    transport_id: &str,
    inspection: &DeviceInspection,
) -> Result<Option<MachineProfile>, String> {
    let exact = profiles
        .profiles
        .iter()
        .filter(|profile| {
            profile
                .connection
                .as_ref()
                .and_then(|connection| connection.fingerprint.as_ref())
                .is_some_and(|stored| stored.key == fingerprint.key)
        })
        .cloned()
        .collect::<Vec<_>>();
    if exact.len() > 1 {
        return Err("multiple machine profiles have the same controller fingerprint".to_owned());
    }
    if let Some(profile) = exact.into_iter().next() {
        return Ok(Some(profile));
    }

    let legacy = profiles
        .profiles
        .iter()
        .filter(|profile| {
            let Some(connection) = profile.connection.as_ref() else {
                return false;
            };
            if connection.fingerprint.is_some() || connection.transport_id != transport_id {
                return false;
            }
            match (
                profile
                    .detected_controller
                    .as_ref()
                    .and_then(|controller| controller.firmware_version.as_deref()),
                inspection.firmware_version.as_deref(),
            ) {
                (Some(stored), Some(observed)) => stored == observed,
                _ => true,
            }
        })
        .cloned()
        .collect::<Vec<_>>();
    if legacy.len() > 1 {
        return Err("the serial device matches more than one legacy machine profile".to_owned());
    }
    Ok(legacy.into_iter().next())
}

fn machine_fingerprint(
    descriptor: &TransportDescriptor,
    inspection: &DeviceInspection,
) -> MachineFingerprint {
    if descriptor.kind == TransportKind::Mock {
        return MachineFingerprint {
            key: "mock:built-in-grbl".to_owned(),
            confidence: IdentityConfidence::Synthetic,
            label: "Built-in Mock GRBL".to_owned(),
        };
    }
    let vendor = descriptor.vendor_id.unwrap_or_default();
    let product = descriptor.product_id.unwrap_or_default();
    if let Some(serial) = descriptor
        .serial_number
        .as_deref()
        .map(str::trim)
        .filter(|serial| !serial.is_empty() && *serial != "0")
    {
        return MachineFingerprint {
            key: format!("usb:{vendor:04x}:{product:04x}:{}", identity_token(serial)),
            confidence: IdentityConfidence::Strong,
            label: format!("USB {vendor:04X}:{product:04X} · {serial}"),
        };
    }
    let product_name = descriptor
        .product
        .as_deref()
        .or(descriptor.detail.as_deref())
        .unwrap_or("serial");
    let firmware = inspection.firmware_version.as_deref().unwrap_or("unknown");
    MachineFingerprint {
        key: format!(
            "port:{vendor:04x}:{product:04x}:{}:{}",
            identity_token(product_name),
            identity_token(descriptor.port_name.as_deref().unwrap_or(&descriptor.id))
        ),
        confidence: IdentityConfidence::PortBound,
        label: format!(
            "{} · {} · {}",
            product_name,
            firmware,
            descriptor.port_name.as_deref().unwrap_or(&descriptor.label)
        ),
    }
}

fn identity_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_'))
        .flat_map(char::to_lowercase)
        .collect()
}

fn suggested_machine_name(
    descriptor: &TransportDescriptor,
    inspection: &millo_domain::DeviceInspection,
) -> String {
    let source = descriptor
        .detail
        .as_deref()
        .filter(|value| !matches!(*value, "Serial port" | "Bluetooth serial port"))
        .or(inspection.firmware_build_info.as_deref())
        .or(inspection.firmware_version.as_deref())
        .unwrap_or("GRBL machine");
    let normalized = source.replace(['_', '-'], " ");
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[tauri::command]
pub async fn refresh_status(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    let result = state
        .arbiter
        .refresh_status()
        .await
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Controller,
        "controller.status_refresh",
        "Fresh GRBL status received",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn inspect_device(state: State<'_, AppState>) -> Result<HardwareInspection, String> {
    let inspection = match state
        .arbiter
        .inspect_device()
        .await
        .map_err(|error| error.to_string())
    {
        Ok(inspection) => inspection,
        Err(error) => {
            state.audit.record(
                AuditLevel::Error,
                AuditCategory::Controller,
                "controller.inspection.failed",
                &error,
                Value::Null,
            );
            return Err(error);
        }
    };
    if let Some(active) = state.settings_session.lock().await.as_mut() {
        active.inspection = inspection.device.clone();
        active.revision = active.revision.saturating_add(1);
        if let Some(archive) = active.archive.as_mut() {
            archive
                .record_observation(&inspection.device)
                .map_err(|error| error.to_string())?;
        }
    }
    state.audit.record(
        AuditLevel::Info,
        AuditCategory::Controller,
        "controller.inspection.completed",
        "GRBL identity, settings, modal state, and coordinates synchronized",
        serde_json::to_value(&inspection).unwrap_or(Value::Null),
    );
    Ok(inspection)
}

#[tauri::command]
pub async fn controller_settings(
    state: State<'_, AppState>,
) -> Result<ControllerSettingsState, String> {
    let session = state.settings_session.lock().await;
    session
        .as_ref()
        .map(settings_state)
        .ok_or_else(|| "connect and synchronize a controller first".to_owned())
}

#[tauri::command]
pub async fn update_controller_setting(
    request: ControllerSettingEditRequest,
    state: State<'_, AppState>,
) -> Result<ControllerSettingsState, String> {
    let _transition = state.transition_lock.lock().await;
    let context = serde_json::to_value(&request).unwrap_or(Value::Null);
    let result = apply_controller_setting(&state, request).await;
    audit_operation(
        &state.audit,
        AuditCategory::Controller,
        "controller.setting_write",
        "Controller setting written and read back",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn rollback_controller_setting(
    key: String,
    expected_revision: u64,
    state: State<'_, AppState>,
) -> Result<ControllerSettingsState, String> {
    let _transition = state.transition_lock.lock().await;
    let (value, expected_value) = {
        let session = state.settings_session.lock().await;
        let active = session
            .as_ref()
            .ok_or_else(|| "connect and synchronize a controller first".to_owned())?;
        let current = active
            .inspection
            .settings
            .get(&key)
            .ok_or_else(|| format!("controller did not report setting {key}"))?;
        let baseline = active
            .archive
            .as_ref()
            .and_then(|archive| archive.state().baseline_value(&key))
            .unwrap_or(current);
        (baseline.to_owned(), current.to_owned())
    };
    apply_controller_setting(
        &state,
        ControllerSettingEditRequest {
            key,
            value,
            confirmed: true,
            expected_value: Some(expected_value),
            expected_revision: Some(expected_revision),
        },
    )
    .await
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    let result = state
        .arbiter
        .disconnect()
        .await
        .map_err(|error| error.to_string());
    *state.settings_session.lock().await = None;
    audit_operation(
        &state.audit,
        AuditCategory::Transport,
        "transport.disconnect",
        "Controller transport disconnected",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn acknowledge_reset(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    state
        .arbiter
        .acknowledge_reset()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn unlock_alarm(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    let result = state
        .arbiter
        .unlock_alarm(true)
        .await
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Safety,
        "safety.alarm_unlock",
        "GRBL Alarm unlocked and Idle verified",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn feed_hold(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    let result = state
        .arbiter
        .feed_hold()
        .await
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Safety,
        "safety.feed_hold",
        "Realtime Feed Hold sent",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn adjust_feed_override(
    adjustment: OverrideAdjustment,
    state: State<'_, AppState>,
) -> Result<ControllerSnapshot, String> {
    state
        .arbiter
        .adjust_feed_override(adjustment)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_rapid_override(
    target: RapidOverrideTarget,
    state: State<'_, AppState>,
) -> Result<ControllerSnapshot, String> {
    state
        .arbiter
        .set_rapid_override(target)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn adjust_spindle_override(
    adjustment: OverrideAdjustment,
    state: State<'_, AppState>,
) -> Result<ControllerSnapshot, String> {
    state
        .arbiter
        .adjust_spindle_override(adjustment)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn request_soft_reset(state: State<'_, AppState>) -> Result<ResetChallenge, String> {
    let result = state
        .arbiter
        .request_soft_reset()
        .await
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Safety,
        "safety.soft_reset_challenge",
        "Soft Reset challenge issued",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn confirm_soft_reset(
    challenge_id: u64,
    state: State<'_, AppState>,
) -> Result<ControllerSnapshot, String> {
    let result = state
        .arbiter
        .confirm_soft_reset(challenge_id)
        .await
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Safety,
        "safety.soft_reset",
        "Soft Reset sent and controller banner observed",
        json!({ "challengeId": challenge_id }),
        &result,
    );
    result
}

#[tauri::command]
pub async fn prepare_test_jog(
    confirmation: OperatorConfirmation,
    state: State<'_, AppState>,
) -> Result<TestJogPreparation, String> {
    ensure_machine_bound(&state).await?;
    state
        .arbiter
        .prepare_test_jog(confirmation)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn step_jog(
    request: StepJogRequest,
    state: State<'_, AppState>,
) -> Result<StepJogReceipt, String> {
    ensure_machine_bound(&state).await?;
    state
        .arbiter
        .step_jog(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn jog_pad_step(
    request: JogPadStepRequest,
    state: State<'_, AppState>,
) -> Result<JogPadStepOutcome, String> {
    let context = serde_json::to_value(request).unwrap_or(Value::Null);
    let result = async {
        ensure_machine_bound(&state).await?;
        state
            .arbiter
            .jog_pad_step(request)
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Controller,
        "controller.jog_step",
        "Guarded jog step accepted",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn set_work_zero(
    request: WorkZeroRequest,
    state: State<'_, AppState>,
) -> Result<WorkZeroOutcome, String> {
    let context = serde_json::to_value(request).unwrap_or(Value::Null);
    let result = async {
        ensure_machine_bound(&state).await?;
        state
            .arbiter
            .set_work_zero(request)
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Controller,
        "controller.work_zero",
        "Work zero written and verified through $#",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn return_to_work_zero(
    request: ReturnToWorkZeroRequest,
    state: State<'_, AppState>,
) -> Result<ReturnToWorkZeroOutcome, String> {
    let context = serde_json::to_value(request).unwrap_or(Value::Null);
    let result = async {
        ensure_machine_bound(&state).await?;
        state
            .arbiter
            .return_to_work_zero(request)
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Controller,
        "controller.return_to_work_zero",
        "Absolute work-zero jog accepted",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn parse_gcode_program(
    request: ProgramParseRequest,
    options: Option<ProgramParseOptions>,
) -> Result<GcodeProgram, String> {
    tokio::task::spawn_blocking(move || {
        parse_program_with_options(request, options.unwrap_or_default())
    })
    .await
    .map_err(|error| format!("G-code parser task failed: {error}"))?
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn script_plugins(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledScriptPlugin>, String> {
    Ok(state.script_plugins.lock().await.list())
}

#[tauri::command]
pub async fn import_script_plugin(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<InstalledScriptPlugin>, String> {
    let (selection, selected) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .add_filter("Millo plugin", &["millo-plugin", "json"])
        .pick_file(move |path| {
            let _ = selection.send(path);
        });
    let Some(path) = selected
        .await
        .map_err(|_| "plugin open dialog closed unexpectedly".to_owned())?
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|error| error.to_string())?;
    let read_path = path.clone();
    let package = tokio::task::spawn_blocking(move || read_package(&read_path))
        .await
        .map_err(|error| format!("plugin import task failed: {error}"))?
        .map_err(|error| error.to_string())?;
    let installed = state
        .script_plugins
        .lock()
        .await
        .install_external(package)
        .map_err(|error| error.to_string())?;
    state.audit.record(
        AuditLevel::Info,
        AuditCategory::Application,
        "plugin.imported",
        "External script plugin imported in disabled state",
        json!({
            "pluginId": &installed.package.manifest.id,
            "digest": &installed.digest,
            "path": path,
        }),
    );
    Ok(Some(installed))
}

#[tauri::command]
pub async fn save_script_plugin(
    request: ScriptPluginSourceRequest,
    state: State<'_, AppState>,
) -> Result<InstalledScriptPlugin, String> {
    let package = parse_package(&request.package_json).map_err(|error| error.to_string())?;
    let installed = state
        .script_plugins
        .lock()
        .await
        .install_external(package)
        .map_err(|error| error.to_string())?;
    state.audit.record(
        AuditLevel::Info,
        AuditCategory::Application,
        "plugin.saved",
        "External script plugin validated and saved in disabled state",
        json!({
            "pluginId": &installed.package.manifest.id,
            "digest": &installed.digest,
        }),
    );
    Ok(installed)
}

#[tauri::command]
pub async fn export_script_plugin(
    request: ScriptPluginExportRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let package = {
        let store = state.script_plugins.lock().await;
        let installed = store
            .get(&request.plugin_id)
            .ok_or_else(|| format!("plugin is not installed: {}", request.plugin_id))?;
        if installed.digest != request.digest {
            return Err("plugin digest changed; reopen it before export".to_owned());
        }
        installed.package.clone()
    };
    let file_name = format!("{}.millo-plugin", package.manifest.id);
    let package_json = millo_script::package_json(&package).map_err(|error| error.to_string())?;
    let (selection, selected) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(file_name)
        .add_filter("Millo plugin", &["millo-plugin"])
        .save_file(move |path| {
            let _ = selection.send(path);
        });
    let Some(path) = selected
        .await
        .map_err(|_| "plugin save dialog closed unexpectedly".to_owned())?
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|error| error.to_string())?;
    let output_path = path.clone();
    tokio::task::spawn_blocking(move || std::fs::write(output_path, package_json))
        .await
        .map_err(|error| format!("plugin export task failed: {error}"))?
        .map_err(|error| format!("failed to export plugin: {error}"))?;
    state.audit.record(
        AuditLevel::Info,
        AuditCategory::Storage,
        "plugin.exported",
        "Script plugin package exported",
        json!({
            "pluginId": request.plugin_id,
            "digest": request.digest,
            "path": path,
        }),
    );
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn configure_script_plugin(
    request: ScriptPluginEnableRequest,
    state: State<'_, AppState>,
) -> Result<InstalledScriptPlugin, String> {
    let installed = state
        .script_plugins
        .lock()
        .await
        .set_enabled(
            &request.plugin_id,
            &request.digest,
            request.enabled,
            request.granted_capabilities,
        )
        .map_err(|error| error.to_string())?;
    state.audit.record(
        AuditLevel::Info,
        AuditCategory::Application,
        if installed.enabled {
            "plugin.enabled"
        } else {
            "plugin.disabled"
        },
        if installed.enabled {
            "Script plugin enabled with reviewed capabilities"
        } else {
            "Script plugin disabled"
        },
        json!({
            "pluginId": &installed.package.manifest.id,
            "digest": &installed.digest,
            "capabilities": &installed.granted_capabilities,
        }),
    );
    Ok(installed)
}

#[tauri::command]
pub async fn delete_script_plugin(
    request: ScriptPluginDeleteRequest,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let removed = state
        .script_plugins
        .lock()
        .await
        .remove(&request.plugin_id)
        .map_err(|error| error.to_string())?;
    if removed {
        state.audit.record(
            AuditLevel::Info,
            AuditCategory::Application,
            "plugin.deleted",
            "External script plugin deleted",
            json!({ "pluginId": request.plugin_id }),
        );
    }
    Ok(removed)
}

#[tauri::command]
pub async fn execute_script_plugin(
    request: ScriptPluginExecutionRequest,
    state: State<'_, AppState>,
) -> Result<ScriptPluginExecutionOutcome, String> {
    let installed = {
        let store = state.script_plugins.lock().await;
        store
            .get(&request.plugin_id)
            .cloned()
            .ok_or_else(|| format!("plugin is not installed: {}", request.plugin_id))?
    };
    if installed.digest != request.digest {
        return Err("plugin digest changed; reopen and review it".to_owned());
    }
    if !installed.enabled {
        return Err(format!("plugin is disabled: {}", request.plugin_id));
    }
    let command = installed
        .package
        .commands
        .iter()
        .find(|command| command.id == request.command_id)
        .ok_or_else(|| format!("plugin command is not declared: {}", request.command_id))?;
    if let Some(capability) = command
        .required_capabilities
        .iter()
        .find(|capability| !installed.granted_capabilities.contains(capability))
    {
        return Err(format!("plugin capability was not granted: {capability:?}"));
    }
    let machine = if installed
        .granted_capabilities
        .contains(&ScriptCapability::MachineRead)
    {
        serde_json::to_value(state.arbiter.snapshot()).unwrap_or(Value::Null)
    } else {
        Value::Null
    };
    let package = installed.package.clone();
    let command_id = request.command_id.clone();
    let input = request.input.clone();
    let action = tokio::task::spawn_blocking(move || {
        ScriptRuntime::execute(&package, &command_id, input, machine)
    })
    .await
    .map_err(|error| format!("script runtime task failed: {error}"))?
    .map_err(|error| error.to_string())?;
    if let Some(capability) = action_capability(&action)
        && !installed.granted_capabilities.contains(&capability)
    {
        return Err(format!("plugin capability was not granted: {capability:?}"));
    }

    let action_name = match &action {
        ScriptAction::CreateProgram { .. } => "createProgram",
        ScriptAction::Jog { .. } => "jog",
        ScriptAction::SetZero { .. } => "setZero",
        ScriptAction::ReturnZero { .. } => "returnZero",
        ScriptAction::Notice { .. } => "notice",
    };
    state.audit.record(
        AuditLevel::Info,
        AuditCategory::Application,
        "plugin.command_executed",
        "Script command returned a validated action",
        json!({
            "pluginId": &request.plugin_id,
            "commandId": &request.command_id,
            "digest": &request.digest,
            "action": action_name,
        }),
    );

    match action {
        ScriptAction::CreateProgram { .. } => {
            let job = tokio::task::spawn_blocking(move || generated_job(&action))
                .await
                .map_err(|error| format!("script G-code parser task failed: {error}"))?
                .map_err(|error| error.to_string())?;
            Ok(ScriptPluginExecutionOutcome::Job { job })
        }
        ScriptAction::Notice {
            title,
            message,
            tone,
        } => Ok(ScriptPluginExecutionOutcome::Notice {
            title,
            message,
            tone,
        }),
        ScriptAction::Jog {
            axis,
            distance_mm,
            feed_mm_per_min,
        } => {
            ensure_script_motion_confirmed(request.operator_confirmed)?;
            ensure_machine_bound(&state).await?;
            state
                .arbiter
                .jog_pad_step(JogPadStepRequest {
                    confirmation: OperatorConfirmation {
                        spindle_off: true,
                        tool_clear: true,
                        power_control_reachable: true,
                    },
                    axis: jog_axis(axis),
                    distance_mm,
                    feed_mm_per_min,
                })
                .await
                .map_err(|error| error.to_string())?;
            Ok(ScriptPluginExecutionOutcome::Machine {
                action: "jog".to_owned(),
                message: format!("{:?} moved {distance_mm:.3} mm", axis),
                snapshot: state.arbiter.snapshot(),
            })
        }
        ScriptAction::SetZero { axis } => {
            ensure_script_motion_confirmed(request.operator_confirmed)?;
            ensure_machine_bound(&state).await?;
            let outcome = state
                .arbiter
                .set_work_zero(WorkZeroRequest {
                    axis: work_axis(axis),
                    position_confirmed: true,
                })
                .await
                .map_err(|error| error.to_string())?;
            Ok(ScriptPluginExecutionOutcome::Machine {
                action: "setZero".to_owned(),
                message: format!("{:?} work zero set and verified", axis),
                snapshot: outcome.snapshot,
            })
        }
        ScriptAction::ReturnZero {
            axis,
            feed_mm_per_min,
        } => {
            ensure_script_motion_confirmed(request.operator_confirmed)?;
            ensure_machine_bound(&state).await?;
            let outcome = state
                .arbiter
                .return_to_work_zero(ReturnToWorkZeroRequest {
                    axis: work_axis(axis),
                    feed_mm_per_min,
                })
                .await
                .map_err(|error| error.to_string())?;
            Ok(ScriptPluginExecutionOutcome::Machine {
                action: "returnZero".to_owned(),
                message: format!("{:?} returned to work zero", axis),
                snapshot: outcome.snapshot,
            })
        }
    }
}

fn ensure_script_motion_confirmed(confirmed: bool) -> Result<(), String> {
    if confirmed {
        Ok(())
    } else {
        Err("operator confirmation is required for a plugin machine action".to_owned())
    }
}

fn jog_axis(axis: ScriptAxis) -> JogAxis {
    match axis {
        ScriptAxis::X => JogAxis::X,
        ScriptAxis::Y => JogAxis::Y,
        ScriptAxis::Z => JogAxis::Z,
    }
}

fn work_axis(axis: ScriptAxis) -> WorkAxis {
    match axis {
        ScriptAxis::X => WorkAxis::X,
        ScriptAxis::Y => WorkAxis::Y,
        ScriptAxis::Z => WorkAxis::Z,
    }
}

#[tauri::command]
pub async fn generate_image_job(
    request: ImageJobRequest,
    state: State<'_, AppState>,
) -> Result<GeneratedImageJob, String> {
    let context = json!({
        "sourceName": &request.source_name,
        "format": request.format,
        "encodedBytes": request.source_base64.len(),
        "settings": &request.settings,
    });
    let result = tokio::task::spawn_blocking(move || generate_image_job_core(request))
        .await
        .map_err(|error| format!("image job generation task failed: {error}"))?
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Program,
        "program.image_job_generated",
        "Image job generated and reparsed",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn generate_surfacing_job(
    request: SurfacingJobRequest,
    state: State<'_, AppState>,
) -> Result<GeneratedSurfacingJob, String> {
    let tool = state
        .tools
        .lock()
        .await
        .get(&request.tool_id)
        .cloned()
        .ok_or_else(|| format!("unknown cutting tool: {}", request.tool_id))?;
    let context = json!({
        "sourceName": &request.source_name,
        "toolId": &request.tool_id,
        "settings": &request.settings,
    });
    let result = tokio::task::spawn_blocking(move || generate_surfacing_job_core(request, &tool))
        .await
        .map_err(|error| format!("surfacing job generation task failed: {error}"))?
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Program,
        "program.surfacing_job_generated",
        "Surfacing job generated and reparsed",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn save_generated_gcode(
    request: GeneratedGcodeSaveRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<GeneratedGcodeSaveOutcome>, String> {
    save_validated_gcode(
        ProgramParseRequest {
            source_name: request.source_name,
            source: request.source,
        },
        &app,
        &state.audit,
        "storage.generated_gcode_saved",
        "Generated G-code saved",
    )
    .await
}

#[tauri::command]
pub async fn save_gcode_program(
    request: ProgramParseRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<GeneratedGcodeSaveOutcome>, String> {
    save_validated_gcode(
        request,
        &app,
        &state.audit,
        "storage.gcode_program_saved",
        "G-code program saved",
    )
    .await
}

async fn save_validated_gcode(
    request: ProgramParseRequest,
    app: &AppHandle,
    audit: &AuditLog,
    audit_operation: &'static str,
    audit_message: &'static str,
) -> Result<Option<GeneratedGcodeSaveOutcome>, String> {
    let source_name = request.source_name.trim();
    if !valid_program_gcode_name(&request.source_name) {
        return Err("G-code file name is invalid".to_owned());
    }
    parse_program(request.clone()).map_err(|error| format!("G-code is invalid: {error}"))?;

    let (selection, selected) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(source_name)
        .add_filter("G-code", &["nc", "ngc", "gcode", "tap", "cnc"])
        .save_file(move |path| {
            let _ = selection.send(path);
        });
    let Some(path) = selected
        .await
        .map_err(|_| "G-code save dialog closed unexpectedly".to_owned())?
    else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|error| error.to_string())?;
    let bytes_written = request.source.len();
    let output_path = path.clone();
    tokio::task::spawn_blocking(move || std::fs::write(output_path, request.source))
        .await
        .map_err(|error| format!("G-code save task failed: {error}"))?
        .map_err(|error| format!("failed to save G-code: {error}"))?;
    let outcome = GeneratedGcodeSaveOutcome {
        path: path.to_string_lossy().into_owned(),
        bytes_written,
    };
    audit.record(
        AuditLevel::Info,
        AuditCategory::Storage,
        audit_operation,
        audit_message,
        json!({ "path": &outcome.path, "bytesWritten": outcome.bytes_written }),
    );
    Ok(Some(outcome))
}

fn valid_program_gcode_name(value: &str) -> bool {
    let trimmed = value.trim();
    let extension = std::path::Path::new(trimmed)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    !trimmed.is_empty()
        && value == trimmed
        && !trimmed.contains('/')
        && !trimmed.contains('\\')
        && matches!(extension.as_str(), "nc" | "ngc" | "gcode" | "tap" | "cnc")
}

#[tauri::command]
pub async fn prepare_selected_program_run(
    request: SelectedRunPreparationRequest,
    state: State<'_, AppState>,
) -> Result<SafeStartPackage, String> {
    let context = json!({
        "sourceName": &request.request.source_name,
        "selectedSourceLine": request.selected_source_line,
        "safeZMm": request.safe_z_mm,
        "intent": request.intent,
        "executionOptions": request.execution_options,
    });
    let result = tokio::task::spawn_blocking(move || prepare_selected_run(request))
        .await
        .map_err(|error| format!("selected-run planner task failed: {error}"))?;
    audit_operation(
        &state.audit,
        AuditCategory::Program,
        "program.selected_run.prepare",
        "Safe selected-line program prepared",
        context,
        &result,
    );
    result
}

fn prepare_selected_run(
    request: SelectedRunPreparationRequest,
) -> Result<SafeStartPackage, String> {
    let program = parse_program_with_options(
        request.request.clone(),
        ProgramParseOptions {
            block_delete: request.execution_options.block_delete,
        },
    )
    .map_err(|error| error.to_string())?;
    let package = build_safe_start(
        &program,
        &request.request.source,
        SafeStartRequest {
            selected_source_line: request.selected_source_line,
            safe_z_mm: request.safe_z_mm,
            intent: match request.intent {
                ProgramRunIntent::AirRun => SafeStartIntent::AirRun,
                ProgramRunIntent::Cutting => SafeStartIntent::Cutting,
            },
        },
    )
    .map_err(|error| error.to_string())?;
    let prepared = parse_program_with_options(
        package.request.clone(),
        ProgramParseOptions {
            block_delete: request.execution_options.block_delete,
        },
    )
    .map_err(|error| format!("prepared selected-run source is invalid: {error}"))?;
    build_program_run_plan_with_options(
        &prepared,
        match request.intent {
            ProgramRunIntent::AirRun => ProgramRunPolicy::AirRun,
            ProgramRunIntent::Cutting => ProgramRunPolicy::Cutting,
        },
        request.execution_options,
    )
    .map_err(|error| format!("prepared selected-run policy failed: {error}"))?;
    Ok(package)
}

#[tauri::command]
pub async fn preflight_real_run(
    request: ProgramParseRequest,
    intent: ProgramRunIntent,
    execution_options: ProgramExecutionOptions,
    state: State<'_, AppState>,
) -> Result<RunPreflightReport, String> {
    let _transition = state.transition_lock.lock().await;
    let context = json!({
        "sourceName": &request.source_name,
        "sourceBytes": request.source.len(),
        "intent": intent,
        "executionOptions": execution_options,
    });
    let result = async {
        ensure_machine_bound(&state).await?;
        if state.active_transport.lock().await.kind != TransportKind::Serial {
            return Err("real-run preflight requires an active serial transport".to_owned());
        }
        let program = tokio::task::spawn_blocking(move || {
            parse_program_with_options(
                request,
                ProgramParseOptions {
                    block_delete: execution_options.block_delete,
                },
            )
        })
        .await
        .map_err(|error| format!("real-run parser task failed: {error}"))?
        .map_err(|error| error.to_string())?;
        state
            .arbiter
            .preflight_real_run_with_options(program, intent, execution_options)
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Program,
        "program.preflight",
        "Program preflight completed",
        context,
        &result,
    );
    if let Ok(report) = &result {
        state.audit.record(
            if report.ready {
                AuditLevel::Info
            } else {
                AuditLevel::Warning
            },
            AuditCategory::Program,
            "program.preflight.report",
            if report.ready {
                "Program is ready for operator authorization"
            } else {
                "Program preflight is blocked"
            },
            serde_json::to_value(report).unwrap_or(Value::Null),
        );
    }
    result
}

#[tauri::command]
pub async fn authorize_first_cut(
    request: ProgramParseRequest,
    confirmation: FirstCutConfirmation,
    state: State<'_, AppState>,
) -> Result<FirstCutPreparation, String> {
    let _transition = state.transition_lock.lock().await;
    let context = json!({
        "sourceName": &request.source_name,
        "sourceBytes": request.source.len(),
        "confirmation": &confirmation,
    });
    let result = async {
        ensure_machine_bound(&state).await?;
        if state.active_transport.lock().await.kind != TransportKind::Serial {
            return Err("first-cut authorization requires an active serial transport".to_owned());
        }
        let execution_options = confirmation.execution_options;
        let program = tokio::task::spawn_blocking(move || {
            parse_program_with_options(
                request,
                ProgramParseOptions {
                    block_delete: execution_options.block_delete,
                },
            )
        })
        .await
        .map_err(|error| format!("first-cut parser task failed: {error}"))?
        .map_err(|error| error.to_string())?;
        state
            .arbiter
            .authorize_first_cut(program, confirmation)
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Safety,
        "safety.program_authorization",
        "One-use program authorization issued",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn start_program_run(
    request: ProgramParseRequest,
    authorization_id: u64,
    execution_options: ProgramExecutionOptions,
    state: State<'_, AppState>,
) -> Result<SenderSnapshot, String> {
    let context = json!({
        "sourceName": &request.source_name,
        "sourceBytes": request.source.len(),
        "authorizationId": authorization_id,
        "executionOptions": execution_options,
    });
    state.audit.record(
        AuditLevel::Info,
        AuditCategory::Program,
        "program.run.requested",
        "Program execution requested",
        context.clone(),
    );
    let result = start_program_run_impl(request, authorization_id, execution_options, &state).await;
    audit_operation(
        &state.audit,
        AuditCategory::Program,
        "program.run",
        "Program sender started",
        context,
        &result,
    );
    result
}

async fn start_program_run_impl(
    request: ProgramParseRequest,
    authorization_id: u64,
    execution_options: ProgramExecutionOptions,
    state: &AppState,
) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    ensure_machine_bound(state).await?;
    if state.active_transport.lock().await.kind != TransportKind::Serial {
        return Err("program run requires an active serial transport".to_owned());
    }
    let (machine_fingerprint, profile_id) = state
        .settings_session
        .lock()
        .await
        .as_ref()
        .map(|session| (session.fingerprint.key.clone(), session.profile_id.clone()))
        .ok_or_else(|| "controller settings have not been synchronized".to_owned())?;
    let source = request.clone();
    let program = tokio::task::spawn_blocking(move || {
        parse_program_with_options(
            request,
            ProgramParseOptions {
                block_delete: execution_options.block_delete,
            },
        )
    })
    .await
    .map_err(|error| format!("program-run parser task failed: {error}"))?
    .map_err(|error| error.to_string())?;
    let stored_source_name = program.source_name.clone();
    let fingerprint = program_fingerprint(&program);
    let prepared = state
        .arbiter
        .prepare_program_run(program, authorization_id)
        .await
        .map_err(|error| error.to_string())?;
    let intent = match prepared.mode {
        Some(SenderMode::AirRun) => ProgramRunIntent::AirRun,
        Some(SenderMode::CutRun) => ProgramRunIntent::Cutting,
        _ => {
            let _ = state
                .arbiter
                .discard_prepared_program_run(prepared.run_sequence)
                .await;
            return Err("prepared sender did not retain a physical run intent".to_owned());
        }
    };
    let controller = state.arbiter.snapshot();
    let seed = RecoverySeed {
        machine_fingerprint,
        profile_id,
        source_name: stored_source_name,
        source: source.source,
        program_fingerprint: fingerprint,
        intent,
        execution_options,
        run_sequence: prepared.run_sequence,
        start_machine_position: controller.machine.machine_position,
        start_work_position: controller.machine.work_position,
        start_work_coordinate_offset: controller.machine.work_coordinate_offset,
    };
    let recovery = Arc::clone(&state.program_recovery);
    let prepared_for_store = prepared.clone();
    let arm_task = tokio::task::spawn_blocking(move || {
        recovery
            .lock()
            .map_err(|error| format!("program recovery lock poisoned: {error}"))?
            .arm(seed, &prepared_for_store, SystemTime::now(), Instant::now())
            .map_err(|error| error.to_string())
    })
    .await;
    let arm_result = match arm_task {
        Ok(result) => result,
        Err(error) => Err(format!("program recovery arm task failed: {error}")),
    };
    let candidate = match arm_result {
        Ok(candidate) => candidate,
        Err(error) => {
            let _ = state
                .arbiter
                .discard_prepared_program_run(prepared.run_sequence)
                .await;
            return Err(format!(
                "program run was not dispatched because recovery evidence could not be persisted: {error}"
            ));
        }
    };
    match state
        .arbiter
        .commit_prepared_program_run(prepared.run_sequence)
        .await
    {
        Ok(snapshot) => {
            match state.program_recovery.lock() {
                Ok(mut recovery) => {
                    if let Err(error) = recovery.commit_arm(candidate.id) {
                        eprintln!("program recovery arm commit bookkeeping failed: {error}");
                    }
                }
                Err(error) => eprintln!("program recovery lock poisoned after commit: {error}"),
            }
            Ok(snapshot)
        }
        Err(error) => {
            let recovery = Arc::clone(&state.program_recovery);
            let rollback = tokio::task::spawn_blocking(move || {
                recovery
                    .lock()
                    .map_err(|lock| format!("program recovery lock poisoned: {lock}"))?
                    .rollback_arm(candidate.id)
                    .map_err(|rollback| rollback.to_string())
            })
            .await;
            let _ = state
                .arbiter
                .discard_prepared_program_run(prepared.run_sequence)
                .await;
            match rollback {
                Ok(Ok(())) => Err(error.to_string()),
                Ok(Err(rollback)) => Err(format!(
                    "{error}; prepared recovery rollback also failed: {rollback}"
                )),
                Err(rollback) => Err(format!(
                    "{error}; prepared recovery rollback task failed: {rollback}"
                )),
            }
        }
    }
}

#[tauri::command]
pub async fn start_check_run(
    request: ProgramParseRequest,
    execution_options: ProgramExecutionOptions,
    state: State<'_, AppState>,
) -> Result<SenderSnapshot, String> {
    let context = json!({
        "sourceName": &request.source_name,
        "sourceBytes": request.source.len(),
        "executionOptions": execution_options,
    });
    let result = start_check_run_impl(request, execution_options, &state).await;
    audit_operation(
        &state.audit,
        AuditCategory::Program,
        "program.check_run",
        "GRBL Check sender started",
        context,
        &result,
    );
    result
}

async fn start_check_run_impl(
    request: ProgramParseRequest,
    execution_options: ProgramExecutionOptions,
    state: &AppState,
) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    ensure_machine_bound(state).await?;
    if state.active_transport.lock().await.kind != TransportKind::Serial {
        return Err("GRBL Check requires an active serial transport".to_owned());
    }
    let program = tokio::task::spawn_blocking(move || {
        parse_program_with_options(
            request,
            ProgramParseOptions {
                block_delete: execution_options.block_delete,
            },
        )
    })
    .await
    .map_err(|error| format!("check-run parser task failed: {error}"))?
    .map_err(|error| error.to_string())?;
    state
        .arbiter
        .start_check_run_with_options(program, execution_options)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn pause_program_run(state: State<'_, AppState>) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    let result = async {
        ensure_machine_bound(&state).await?;
        if state.active_transport.lock().await.kind != TransportKind::Serial {
            return Err("program pause requires an active serial transport".to_owned());
        }
        state
            .arbiter
            .pause_program_run()
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Sender,
        "sender.pause",
        "Physical sender paused",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn resume_program_run(state: State<'_, AppState>) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    let result = async {
        ensure_machine_bound(&state).await?;
        if state.active_transport.lock().await.kind != TransportKind::Serial {
            return Err("program resume requires an active serial transport".to_owned());
        }
        state
            .arbiter
            .resume_program_run()
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Sender,
        "sender.resume",
        "Physical sender resumed",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn abort_program_run(state: State<'_, AppState>) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    let result = async {
        ensure_machine_bound(&state).await?;
        if state.active_transport.lock().await.kind != TransportKind::Serial {
            return Err("program stop requires an active serial transport".to_owned());
        }
        state
            .arbiter
            .abort_program_run()
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Safety,
        "sender.abort",
        "Physical sender stopped with Feed Hold and Soft Reset",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn complete_tool_change(
    confirmation: ToolChangeConfirmation,
    state: State<'_, AppState>,
) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    let context = serde_json::to_value(confirmation).unwrap_or(Value::Null);
    let result = async {
        ensure_machine_bound(&state).await?;
        if state.active_transport.lock().await.kind != TransportKind::Serial {
            return Err("tool change requires an active serial transport".to_owned());
        }
        state
            .arbiter
            .complete_tool_change(confirmation)
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Safety,
        "safety.tool_change",
        "Tool change confirmed and sender resumed",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn sender_snapshot(state: State<'_, AppState>) -> Result<SenderSnapshot, String> {
    Ok(state.arbiter.sender_snapshot())
}

#[tauri::command]
pub async fn start_mock_dry_run(
    request: ProgramParseRequest,
    state: State<'_, AppState>,
) -> Result<SenderSnapshot, String> {
    if state.active_transport.lock().await.kind != TransportKind::Mock {
        return Err("dry run is currently available only on Mock GRBL".to_owned());
    }
    let plan = tokio::task::spawn_blocking(move || prepare_dry_run(request))
        .await
        .map_err(|error| format!("dry-run policy task failed: {error}"))??;
    state
        .arbiter
        .start_dry_run(plan)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn pause_dry_run(state: State<'_, AppState>) -> Result<SenderSnapshot, String> {
    let result = state
        .arbiter
        .pause_dry_run()
        .await
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Sender,
        "sender.pause",
        "Sender paused",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn resume_dry_run(state: State<'_, AppState>) -> Result<SenderSnapshot, String> {
    let result = state
        .arbiter
        .resume_dry_run()
        .await
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Sender,
        "sender.resume",
        "Sender resumed",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn cancel_dry_run(state: State<'_, AppState>) -> Result<SenderSnapshot, String> {
    let result = state
        .arbiter
        .cancel_dry_run()
        .await
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Sender,
        "sender.cancel",
        "Sender cancelled",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn cancel_jog(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    let result = state
        .arbiter
        .cancel_jog()
        .await
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Safety,
        "safety.jog_cancel",
        "Realtime Jog Cancel sent",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn mock_trigger_reset(state: State<'_, AppState>) -> Result<(), String> {
    active_mock(&state).await?.queue_reset("1.1h");
    Ok(())
}

#[tauri::command]
pub async fn mock_start_run(state: State<'_, AppState>) -> Result<(), String> {
    active_mock(&state)
        .await?
        .set_status("<Run|MPos:1.000,2.000,3.000|WPos:1.000,2.000,3.000|FS:120,0>");
    Ok(())
}

#[tauri::command]
pub async fn mock_trigger_alarm(code: u16, state: State<'_, AppState>) -> Result<(), String> {
    active_mock(&state).await?.queue_alarm(code);
    Ok(())
}

#[tauri::command]
pub async fn mock_clear_alarm(state: State<'_, AppState>) -> Result<(), String> {
    active_mock(&state).await?.clear_alarm();
    Ok(())
}

#[tauri::command]
pub async fn mock_trigger_timeout(state: State<'_, AppState>) -> Result<(), String> {
    let mock = active_mock(&state).await?;
    mock.queue_stall();
    mock.queue_stall();
    Ok(())
}

#[tauri::command]
pub async fn mock_trigger_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    active_mock(&state).await?.queue_disconnect();
    Ok(())
}

async fn active_mock(state: &State<'_, AppState>) -> Result<MockControl, String> {
    state
        .mock
        .lock()
        .await
        .clone()
        .ok_or_else(|| "mock scenarios require the Mock GRBL transport".to_owned())
}

async fn resolve_transport(
    transport_id: &str,
    baud_rate: u32,
) -> Result<ResolvedTransport, String> {
    if transport_id == MOCK_TRANSPORT_ID {
        return Ok(ResolvedTransport::mock());
    }

    let port_name = serial_port_name(transport_id)?;
    let available = tokio::task::spawn_blocking(available_serial_ports)
        .await
        .map_err(|error| format!("serial discovery task failed: {error}"))?
        .map_err(|error| error.to_string())?;
    let port = available
        .into_iter()
        .find(|port| port.port_name == port_name)
        .ok_or_else(|| format!("serial port is no longer available: {port_name}"))?;
    let config =
        SerialConfig::new(&port.port_name, baud_rate).map_err(|error| error.to_string())?;

    Ok(ResolvedTransport {
        transport: Box::new(SerialTransport::new(config)),
        descriptor: serial_descriptor(port),
        mock: None,
        execution_target: ExecutionTarget::Serial,
    })
}

fn prepare_dry_run(request: ProgramParseRequest) -> Result<DryRunPlan, String> {
    let program = parse_program(request).map_err(|error| error.to_string())?;
    build_dry_run_plan(&program).map_err(format_dry_run_policy_error)
}

fn format_dry_run_policy_error(error: DryRunPolicyError) -> String {
    match error {
        DryRunPolicyError::Rejected(_, blockers) => blockers
            .into_iter()
            .map(|blocker| {
                let location = blocker
                    .source_line
                    .map_or_else(|| "program".to_owned(), |line| format!("line {line}"));
                format!("{location}: {}", blocker.message)
            })
            .collect::<Vec<_>>()
            .join("; "),
        DryRunPolicyError::EmptyProgram => error.to_string(),
    }
}

fn mock_descriptor() -> TransportDescriptor {
    TransportDescriptor {
        id: MOCK_TRANSPORT_ID.to_owned(),
        kind: TransportKind::Mock,
        label: "Mock GRBL".to_owned(),
        detail: Some("Deterministic test controller".to_owned()),
        port_name: None,
        likely_grbl: true,
        match_reason: Some("Built-in test controller".to_owned()),
        vendor_id: None,
        product_id: None,
        manufacturer: None,
        product: None,
        serial_number: None,
    }
}

fn serial_descriptor(port: SerialPortDescriptor) -> TransportDescriptor {
    let match_reason = grbl_match_reason(&port).map(str::to_owned);
    let detail = match port.kind {
        SerialPortKind::Usb => port
            .product
            .clone()
            .or(port.manufacturer.clone())
            .or_else(|| {
                Some(format!(
                    "USB {:04X}:{:04X}",
                    port.vendor_id.unwrap_or_default(),
                    port.product_id.unwrap_or_default()
                ))
            }),
        SerialPortKind::Bluetooth => Some("Bluetooth serial port".to_owned()),
        SerialPortKind::Pci => Some("PCI serial port".to_owned()),
        SerialPortKind::Unknown => Some("Serial port".to_owned()),
    };

    TransportDescriptor {
        id: format!("{SERIAL_TRANSPORT_PREFIX}{}", port.port_name),
        kind: TransportKind::Serial,
        label: port.port_name.clone(),
        detail,
        port_name: Some(port.port_name),
        likely_grbl: match_reason.is_some(),
        match_reason,
        vendor_id: port.vendor_id,
        product_id: port.product_id,
        manufacturer: port.manufacturer,
        product: port.product,
        serial_number: port.serial_number,
    }
}

fn grbl_match_reason(port: &SerialPortDescriptor) -> Option<&'static str> {
    if port.kind != SerialPortKind::Usb {
        return None;
    }

    let searchable = [
        Some(port.port_name.as_str()),
        port.manufacturer.as_deref(),
        port.product.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_ascii_lowercase();

    if ["grbl", "fluidnc", "cnc", "woodpecker", "xpro"]
        .iter()
        .any(|needle| searchable.contains(needle))
    {
        return Some("GRBL/CNC metadata");
    }

    if [
        "arduino",
        "usbserial",
        "usbmodem",
        "ch340",
        "ch341",
        "cp210",
        "ftdi",
        "usb serial",
        "usb2.0-serial",
    ]
    .iter()
    .any(|needle| searchable.contains(needle))
    {
        return Some("Common CNC USB serial interface");
    }

    match port.vendor_id {
        Some(0x0403 | 0x10C4 | 0x1A86 | 0x2341 | 0x2A03 | 0x303A) => {
            Some("Known controller or USB-UART vendor")
        }
        _ => None,
    }
}

fn serial_port_name(transport_id: &str) -> Result<&str, String> {
    transport_id
        .strip_prefix(SERIAL_TRANSPORT_PREFIX)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("unknown transport: {transport_id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_operation_preserves_structured_failure_context() {
        let audit = AuditLog::in_memory();
        let result: Result<(), String> = Err("ALARM:2 on source line 18".to_owned());
        audit_operation(
            &audit,
            AuditCategory::Sender,
            "sender.test",
            "unused",
            json!({ "sourceLine": 18, "command": "G1 Z-0.200 F80" }),
            &result,
        );

        let mut entries = audit.snapshot(10).entries;
        let entry = entries.pop().unwrap();
        assert_eq!(entry.level, AuditLevel::Error);
        assert_eq!(entry.event, "sender.test.failed");
        assert_eq!(entry.data["context"]["sourceLine"], 18);
        assert_eq!(entry.data["context"]["command"], "G1 Z-0.200 F80");
    }

    #[test]
    fn program_export_accepts_only_leaf_program_names() {
        assert!(valid_program_gcode_name("engraving.nc"));
        assert!(valid_program_gcode_name("engraving.GCODE"));
        assert!(valid_program_gcode_name("engraving.tap"));
        assert!(valid_program_gcode_name("engraving.cnc"));
        assert!(!valid_program_gcode_name("../engraving.nc"));
        assert!(!valid_program_gcode_name("folder\\engraving.nc"));
        assert!(!valid_program_gcode_name("engraving.svg"));
        assert!(!valid_program_gcode_name(" engraving.nc"));
    }

    #[test]
    fn recovery_preparation_requires_every_operator_confirmation() {
        let incomplete = ProgramRecoveryPreparationRequest {
            recovery_id: 1,
            safe_z_mm: 5.0,
            continuity: RecoveryContinuity::MotionPowerLostOrUnknown,
            machine_reference_restored: false,
            work_zero_restored: false,
            motion_power_restored: false,
            restart_point_inspected: false,
            path_clear: false,
            power_control_reachable: false,
        };
        assert_eq!(incomplete.missing().len(), 6);
        assert!(
            ProgramRecoveryPreparationRequest {
                machine_reference_restored: true,
                work_zero_restored: true,
                motion_power_restored: true,
                restart_point_inspected: true,
                path_clear: true,
                power_control_reachable: true,
                ..incomplete
            }
            .missing()
            .is_empty()
        );
    }

    #[tokio::test]
    async fn run_persistence_worker_processes_snapshots_off_the_async_task() {
        let journal = Arc::new(StdMutex::new(RunJournal::in_memory()));
        let recovery = Arc::new(StdMutex::new(ProgramRecoveryStore::in_memory()));
        let sender = start_run_persistence_worker(
            Arc::clone(&journal),
            Arc::clone(&recovery),
            AuditLog::in_memory(),
        )
        .unwrap();
        let snapshot = SenderSnapshot {
            run_sequence: 1,
            source_name: Some("worker.nc".to_owned()),
            mode: Some(millo_sender::SenderMode::CutRun),
            state: millo_sender::SenderState::Running,
            total_lines: 1,
            ..SenderSnapshot::default()
        };

        sender.send(snapshot).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if journal.lock().unwrap().entries().len() == 1 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn tauri_parser_adapter_returns_a_typed_preview_without_machine_state() {
        let program = parse_gcode_program(
            ProgramParseRequest {
                source_name: "adapter.nc".to_owned(),
                source: "G21 G90\nG0 X0 Y0 Z2\nG1 Z0 F50".to_owned(),
            },
            None,
        )
        .await
        .unwrap();

        assert_eq!(program.source_name, "adapter.nc");
        assert_eq!(program.summary.motion_count, 2);
        assert!(program.summary.preview_complete);
    }

    #[tokio::test]
    async fn tauri_parser_adapter_applies_block_delete_to_preview_geometry() {
        let request = ProgramParseRequest {
            source_name: "optional.nc".to_owned(),
            source: "G21 G90 G94\n/G91\nG1 X10 F10\nG1 X20".to_owned(),
        };
        let included = parse_gcode_program(request.clone(), None).await.unwrap();
        let deleted =
            parse_gcode_program(request, Some(ProgramParseOptions { block_delete: true }))
                .await
                .unwrap();

        assert_eq!(included.summary.bounds.unwrap().max.x, 30.0);
        assert_eq!(deleted.summary.bounds.unwrap().max.x, 20.0);
        assert!(deleted.lines[1].block_deleted);
    }

    #[test]
    fn dry_run_adapter_reparses_source_and_rejects_spindle_commands() {
        let error = prepare_dry_run(ProgramParseRequest {
            source_name: "unsafe.nc".to_owned(),
            source: "G21\nM3 S1000\nG1 X1".to_owned(),
        })
        .unwrap_err();

        assert!(error.contains("line 2"));
        assert!(error.contains("spindle"));
    }

    #[test]
    fn dry_run_adapter_returns_an_opaque_policy_plan_for_safe_source() {
        let plan = prepare_dry_run(ProgramParseRequest {
            source_name: "safe.nc".to_owned(),
            source: "G21 G90\nG0 X1\nG1 X2 F10".to_owned(),
        })
        .unwrap();

        assert_eq!(plan.source_name(), "safe.nc");
        assert_eq!(plan.lines().len(), 7);
        assert_eq!(plan.lines().first().unwrap().command(), "M5");
        assert_eq!(plan.lines().get(1).unwrap().command(), "M9");
        assert_eq!(plan.lines().get(5).unwrap().command(), "M5");
        assert_eq!(plan.lines().last().unwrap().command(), "M9");
    }

    #[test]
    fn selected_run_adapter_builds_a_policy_valid_exact_remainder() {
        let source = "G21 G90 G94 G17 G54\nG0 Z5\nG0 X0 Y0\nG1 Z-0.2 F100\nG1 X10\nG0 Z5\nG0 X20 Y0\nG1 Z-0.2 F100\nG1 X30\nM30";
        let package = prepare_selected_run(SelectedRunPreparationRequest {
            request: ProgramParseRequest {
                source_name: "two-features.nc".to_owned(),
                source: source.to_owned(),
            },
            selected_source_line: 9,
            safe_z_mm: 8.0,
            intent: ProgramRunIntent::Cutting,
            execution_options: ProgramExecutionOptions::default(),
        })
        .unwrap();

        assert_eq!(package.selected_source_line, 9);
        assert_eq!(package.restart_source_line, 7);
        assert_eq!(package.replayed_executable_lines, 2);
        assert!(package.request.source_name.starts_with("safe-start-L9-"));
        assert!(package.request.source.contains("G0 Z8.0000"));
        assert!(package.request.source.ends_with("G1 X30\nM30"));
    }

    #[test]
    fn serial_transport_id_preserves_the_native_port_name() {
        assert_eq!(
            serial_port_name("serial:/dev/cu.usbserial-1420").unwrap(),
            "/dev/cu.usbserial-1420"
        );
        assert!(serial_port_name("serial:").is_err());
        assert!(serial_port_name("network:localhost").is_err());
    }

    #[test]
    fn usb_descriptor_keeps_device_identity() {
        let descriptor = serial_descriptor(SerialPortDescriptor {
            port_name: "/dev/cu.usbmodem101".to_owned(),
            kind: SerialPortKind::Usb,
            vendor_id: Some(0x2341),
            product_id: Some(0x0043),
            manufacturer: Some("Arduino".to_owned()),
            product: Some("Uno".to_owned()),
            serial_number: None,
        });

        assert_eq!(descriptor.kind, TransportKind::Serial);
        assert_eq!(descriptor.id, "serial:/dev/cu.usbmodem101");
        assert_eq!(descriptor.label, "/dev/cu.usbmodem101");
        assert_eq!(descriptor.detail.as_deref(), Some("Uno"));
        assert!(descriptor.likely_grbl);
        assert_eq!(
            descriptor.match_reason.as_deref(),
            Some("Common CNC USB serial interface")
        );
    }

    #[test]
    fn grbl_filter_rejects_non_usb_and_unidentified_ports() {
        let bluetooth = SerialPortDescriptor {
            port_name: "/dev/cu.Bluetooth-Incoming-Port".to_owned(),
            kind: SerialPortKind::Bluetooth,
            vendor_id: None,
            product_id: None,
            manufacturer: None,
            product: None,
            serial_number: None,
        };
        let unidentified_usb = SerialPortDescriptor {
            port_name: "COM8".to_owned(),
            kind: SerialPortKind::Usb,
            vendor_id: Some(0x9999),
            product_id: Some(0x0001),
            manufacturer: Some("Measurement Devices Inc.".to_owned()),
            product: Some("Lab interface".to_owned()),
            serial_number: None,
        };

        assert_eq!(grbl_match_reason(&bluetooth), None);
        assert_eq!(grbl_match_reason(&unidentified_usb), None);
    }

    #[test]
    fn grbl_filter_accepts_common_bridges_and_explicit_metadata() {
        let ch340 = SerialPortDescriptor {
            port_name: "COM4".to_owned(),
            kind: SerialPortKind::Usb,
            vendor_id: Some(0x1A86),
            product_id: Some(0x7523),
            manufacturer: None,
            product: None,
            serial_number: None,
        };
        let fluidnc = SerialPortDescriptor {
            port_name: "COM6".to_owned(),
            kind: SerialPortKind::Usb,
            vendor_id: Some(0x9999),
            product_id: Some(0x0002),
            manufacturer: None,
            product: Some("FluidNC controller".to_owned()),
            serial_number: None,
        };

        assert_eq!(
            grbl_match_reason(&ch340),
            Some("Known controller or USB-UART vendor")
        );
        assert_eq!(grbl_match_reason(&fluidnc), Some("GRBL/CNC metadata"));
    }

    #[test]
    fn detected_name_prefers_and_normalizes_usb_product_metadata() {
        let descriptor = TransportDescriptor {
            id: "serial:/dev/cu.test".to_owned(),
            kind: TransportKind::Serial,
            label: "/dev/cu.test".to_owned(),
            detail: Some("LUNYEE_4axis_Control".to_owned()),
            port_name: Some("/dev/cu.test".to_owned()),
            likely_grbl: true,
            match_reason: Some("GRBL/CNC metadata".to_owned()),
            vendor_id: Some(0x0483),
            product_id: Some(0x5740),
            manufacturer: Some("tomeko net".to_owned()),
            product: Some("LUNYEE_4axis_Control".to_owned()),
            serial_number: None,
        };
        let inspection = millo_domain::DeviceInspection {
            firmware_build_info: Some("fallback".to_owned()),
            ..Default::default()
        };

        assert_eq!(
            suggested_machine_name(&descriptor, &inspection),
            "LUNYEE 4axis Control"
        );
    }

    #[test]
    fn fingerprint_prefers_a_real_usb_serial_and_rejects_zero_as_identity() {
        let mut descriptor = serial_descriptor(SerialPortDescriptor {
            port_name: "/dev/cu.usbmodem101".to_owned(),
            kind: SerialPortKind::Usb,
            vendor_id: Some(0x0483),
            product_id: Some(0x5740),
            manufacturer: Some("tomeko net".to_owned()),
            product: Some("LUNYEE_4axis_Control".to_owned()),
            serial_number: Some("ABC-123".to_owned()),
        });
        let inspection = DeviceInspection {
            firmware_version: Some("1.1f".to_owned()),
            ..Default::default()
        };

        let strong = machine_fingerprint(&descriptor, &inspection);
        assert_eq!(strong.confidence, IdentityConfidence::Strong);
        assert_eq!(strong.key, "usb:0483:5740:abc123");

        descriptor.serial_number = Some("0".to_owned());
        let fallback = machine_fingerprint(&descriptor, &inspection);
        assert_eq!(fallback.confidence, IdentityConfidence::PortBound);
        assert!(fallback.key.contains("usbmodem101"));

        let upgraded = DeviceInspection {
            firmware_version: Some("1.1h".to_owned()),
            ..Default::default()
        };
        assert_eq!(
            machine_fingerprint(&descriptor, &upgraded).key,
            fallback.key
        );
    }

    #[test]
    fn profile_matching_migrates_one_legacy_port_but_rejects_ambiguity() {
        let inspection = DeviceInspection {
            firmware_version: Some("1.1f".to_owned()),
            ..Default::default()
        };
        let fingerprint = MachineFingerprint {
            key: "port:test".to_owned(),
            confidence: IdentityConfidence::PortBound,
            label: "Test controller".to_owned(),
        };
        let profile = MachineProfile {
            id: "machine-0001".to_owned(),
            name: "Router".to_owned(),
            travel_mm: millo_domain::MachineTravel {
                x: 300.0,
                y: 200.0,
                z: 80.0,
            },
            max_jog_distance_mm: 50.0,
            spindle_control: millo_domain::SpindleControl::Manual,
            homing_installed: false,
            limit_switches_installed: false,
            probe_installed: false,
            emergency_stop_installed: false,
            connection: Some(MachineConnectionPreset {
                transport_id: "serial:/dev/cu.test".to_owned(),
                baud_rate: 115_200,
                fingerprint: None,
            }),
            detected_controller: Some(DetectedController {
                firmware_version: Some("1.1f".to_owned()),
                firmware_build_info: None,
            }),
        };
        let one = MachineProfileState {
            profiles: vec![profile.clone()],
            selected_profile_id: None,
        };
        assert_eq!(
            match_machine_profile(&one, &fingerprint, "serial:/dev/cu.test", &inspection,)
                .unwrap()
                .unwrap()
                .id,
            "machine-0001"
        );

        let mut second = profile;
        second.id = "machine-0002".to_owned();
        second.name = "Other router".to_owned();
        let ambiguous = MachineProfileState {
            profiles: vec![one.profiles[0].clone(), second],
            selected_profile_id: None,
        };
        assert!(
            match_machine_profile(&ambiguous, &fingerprint, "serial:/dev/cu.test", &inspection,)
                .is_err()
        );
    }
}
