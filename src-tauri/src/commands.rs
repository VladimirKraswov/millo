mod tooling;
pub use tooling::*;
mod recovery;
pub use recovery::*;
mod diagnostics;
pub use diagnostics::*;
mod surface;
pub use surface::*;
mod scripts;
pub use scripts::*;
mod program_run;
pub use program_run::*;
mod machine_control;
pub use machine_control::*;
mod profiles;
pub use profiles::*;
mod settings;
pub use settings::*;
mod connection;
pub use connection::*;

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
use millo_command::{CommandArbiter, ExecutionTarget, OperatorConsolePolicy};
use millo_controller::ControllerConfig;
use millo_domain::{
    CommandCompletion, ContinuousJogReceipt, ContinuousJogRequest, ControllerSnapshot,
    DeviceInspection, HardwareInspection, HardwareProfile, HomingRequest, HomingStartOutcome,
    JogPadStepOutcome, JogPadStepRequest, MachineOutputOutcome, MachineOutputRequest,
    OperatorConfirmation, OperatorConsoleExchange, OverrideAdjustment, RapidOverrideTarget,
    ResetChallenge, ReturnToWorkOriginOutcome, ReturnToWorkOriginRequest, ReturnToWorkZeroOutcome,
    ReturnToWorkZeroRequest, StepJogReceipt, StepJogRequest, TestJogPreparation,
    WorkCoordinateSelectionOutcome, WorkCoordinateSystem, WorkZeroOutcome, WorkZeroRequest,
    ZProbeOutcome, ZProbeRequest,
};
use millo_dry_run::{
    ProgramExecutionOptions, ProgramRunPolicy, build_program_run_plan_with_options,
};
use millo_gcode::{
    GcodeProgram, ProgramParseOptions, ProgramParseRequest, parse_program,
    parse_program_with_options,
};
use millo_grbl::active_work_coordinate_system;
use millo_heightmap::{
    HeightmapOperationSnapshot, HeightmapOperationState, HeightmapResumeRequest,
    HeightmapStartRequest, SurfaceSession, SurfaceSessionStore,
};
use millo_journal::{RunJournal, RunJournalEntry};
use millo_pcb::{
    GeneratedPcbJob, PcbInspectRequest, PcbInspection, PcbJobRequest,
    generate_pcb_job as generate_pcb_job_core, inspect_pcb as inspect_pcb_core,
};
use millo_preferences::{
    ApplicationPreferences, ApplicationPreferencesStore, ApplicationPreferencesUpdate,
};
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
    InstalledScriptPlugin, ScriptAction, ScriptCapability, ScriptPluginStore, ScriptRuntime,
    action_capability, generated_job, parse_package, read_package,
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
use millo_transport::{BoxedTransport, DisconnectedTransport};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use tokio::{
    sync::{Mutex, mpsc},
    task::JoinHandle,
};

mod program_io;
mod sketch;
pub use sketch::*;
mod script_command_model;

pub use program_io::*;

pub use script_command_model::{
    ScriptPluginDeleteRequest, ScriptPluginEnableRequest, ScriptPluginExecutionOutcome,
    ScriptPluginExecutionRequest, ScriptPluginExportRequest, ScriptPluginSourceRequest,
};
use script_command_model::{ensure_script_motion_confirmed, jog_axis, work_axis};

const SERIAL_TRANSPORT_PREFIX: &str = "serial:";

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

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

pub struct AppState {
    arbiter: CommandArbiter,
    audit: AuditLog,
    profiles: Mutex<MachineProfileStore>,
    tools: Mutex<ToolLibraryStore>,
    active_transport: Mutex<TransportDescriptor>,
    transition_lock: Mutex<()>,
    event_task: Mutex<Option<JoinHandle<()>>>,
    settings_root: Option<PathBuf>,
    settings_session: Mutex<Option<ActiveControllerSettings>>,
    run_journal: Arc<StdMutex<RunJournal>>,
    program_recovery: Arc<StdMutex<ProgramRecoveryStore>>,
    script_plugins: Mutex<ScriptPluginStore>,
    script_execution: Mutex<()>,
    surface_session: Arc<StdMutex<SurfaceSessionStore>>,
    preferences: Mutex<ApplicationPreferencesStore>,
}

struct PersistentStores {
    profiles: MachineProfileStore,
    tools: ToolLibraryStore,
    run_journal: RunJournal,
    program_recovery: ProgramRecoveryStore,
    script_plugins: ScriptPluginStore,
    surface_session: SurfaceSessionStore,
    preferences: ApplicationPreferencesStore,
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
        let surface_session_path = profile_path
            .parent()
            .map(|parent| parent.join("surface-session.json"));
        let preferences_path = profile_path
            .parent()
            .map(|parent| parent.join("application-preferences.json"));
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
        let surface_session = surface_session_path
            .map(SurfaceSessionStore::load)
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_else(SurfaceSessionStore::in_memory);
        let preferences = match preferences_path
            .map(ApplicationPreferencesStore::load)
            .transpose()
        {
            Ok(Some(store)) => store,
            Ok(None) => ApplicationPreferencesStore::in_memory(),
            Err(error) => {
                audit.record(
                    AuditLevel::Critical,
                    AuditCategory::Storage,
                    "storage.application_preferences_initialization_failed",
                    error.to_string(),
                    json!({
                        "fallback": "safeDefaultsInMemory",
                        "safeCommandMode": true,
                    }),
                );
                ApplicationPreferencesStore::in_memory()
            }
        };
        let state = Self::from_stores(
            PersistentStores {
                profiles,
                tools,
                run_journal: journal,
                program_recovery: recovery,
                script_plugins,
                surface_session,
                preferences,
            },
            settings_root,
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

    fn from_stores(
        stores: PersistentStores,
        settings_root: Option<PathBuf>,
        audit: AuditLog,
    ) -> Self {
        let descriptor = disconnected_descriptor();
        let hardware_profile = stores
            .profiles
            .state()
            .selected()
            .map(|profile| profile.hardware_profile())
            .unwrap_or_else(HardwareProfile::first_machine);
        let (arbiter, worker) = CommandArbiter::new_with_execution_target(
            Box::new(DisconnectedTransport),
            ControllerConfig::default(),
            hardware_profile,
            ExecutionTarget::Disabled,
        );
        tauri::async_runtime::spawn(worker);

        Self {
            arbiter,
            audit,
            profiles: Mutex::new(stores.profiles),
            tools: Mutex::new(stores.tools),
            active_transport: Mutex::new(descriptor),
            transition_lock: Mutex::new(()),
            event_task: Mutex::new(None),
            settings_root,
            settings_session: Mutex::new(None),
            run_journal: Arc::new(StdMutex::new(stores.run_journal)),
            program_recovery: Arc::new(StdMutex::new(stores.program_recovery)),
            script_plugins: Mutex::new(stores.script_plugins),
            script_execution: Mutex::new(()),
            surface_session: Arc::new(StdMutex::new(stores.surface_session)),
            preferences: Mutex::new(stores.preferences),
        }
    }

    async fn start_event_bridge(&self, app: AppHandle) {
        let mut event_task = self.event_task.lock().await;
        if event_task.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }

        let mut snapshots = self.arbiter.subscribe();
        let mut sender_snapshots = self.arbiter.subscribe_sender();
        let mut heightmap_snapshots = self.arbiter.subscribe_heightmap();
        let persistence_sender = start_run_persistence_worker(
            Arc::clone(&self.run_journal),
            Arc::clone(&self.program_recovery),
            self.audit.clone(),
        );
        let audit = self.audit.clone();
        let surface_session = Arc::clone(&self.surface_session);
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
                        if let Err(error) = app.emit("sender-state", snapshot.clone()) {
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
                    changed = heightmap_snapshots.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        let snapshot = heightmap_snapshots.borrow_and_update().clone();
                        let now = unix_time_ms();
                        let session = match surface_session.lock() {
                            Ok(mut store) => {
                                let result = match snapshot.state {
                                    HeightmapOperationState::Idle => Ok(store.session()),
                                    HeightmapOperationState::Completed => store
                                        .checkpoint(snapshot.clone(), now)
                                        .and_then(|_| store.activate_completed(snapshot.operation_sequence, now)),
                                    _ => store.checkpoint(snapshot.clone(), now),
                                };
                                match result {
                                    Ok(session) => Some(session),
                                    Err(error) => {
                                        audit.record(
                                            AuditLevel::Critical,
                                            AuditCategory::Storage,
                                            "storage.heightmap_checkpoint_failed",
                                            error.to_string(),
                                            json!({ "operationSequence": snapshot.operation_sequence }),
                                        );
                                        None
                                    }
                                }
                            }
                            Err(error) => {
                                audit.record(
                                    AuditLevel::Critical,
                                    AuditCategory::Storage,
                                    "storage.heightmap_lock_poisoned",
                                    error.to_string(),
                                    json!({ "operationSequence": snapshot.operation_sequence }),
                                );
                                None
                            }
                        };
                        audit.record(
                            if snapshot.state == HeightmapOperationState::Failed {
                                AuditLevel::Error
                            } else {
                                AuditLevel::Info
                            },
                            AuditCategory::Controller,
                            "heightmap.snapshot",
                            format!("Heightmap {:?}: {}/{}", snapshot.state, snapshot.progress.measured, snapshot.progress.total),
                            serde_json::to_value(&snapshot).unwrap_or(Value::Null),
                        );
                        if let Err(error) = app.emit("heightmap-state", snapshot) {
                            audit.record(
                                AuditLevel::Error,
                                AuditCategory::Ui,
                                "ui.heightmap_state_emit_failed",
                                error.to_string(),
                                Value::Null,
                            );
                        }
                        if let Some(session) = session {
                            if let Err(error) = app.emit("surface-session", session) {
                                audit.record(
                                    AuditLevel::Error,
                                    AuditCategory::Ui,
                                    "ui.surface_session_emit_failed",
                                    error.to_string(),
                                    Value::Null,
                                );
                            }
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
        assert_eq!(descriptor.label, "Uno · /dev/cu.usbmodem101");
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
        let external_grbl = SerialPortDescriptor {
            port_name: "/dev/ttys003".to_owned(),
            kind: SerialPortKind::Unknown,
            vendor_id: None,
            product_id: None,
            manufacturer: Some("Millo".to_owned()),
            product: Some("Millo VMC-3 GRBL Controller".to_owned()),
            serial_number: Some("MILLO-VMC3-0001".to_owned()),
        };

        assert_eq!(
            grbl_match_reason(&ch340),
            Some("Known controller or USB-UART vendor")
        );
        assert_eq!(grbl_match_reason(&fluidnc), Some("GRBL/CNC metadata"));
        assert_eq!(grbl_match_reason(&external_grbl), Some("GRBL/CNC metadata"));
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
            rotary_axis: None,
            max_jog_distance_mm: 50.0,
            spindle_control: millo_domain::SpindleControl::Manual,
            flood_coolant_control: false,
            mist_coolant_control: false,
            homing_installed: false,
            limit_switches_installed: false,
            probe_installed: false,
            probe_settings: millo_domain::ZProbeSettings::default(),
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
