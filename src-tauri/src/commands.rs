use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Arc, Mutex as StdMutex},
    time::{Instant, SystemTime},
};

use millo_command::{CommandArbiter, ExecutionTarget};
use millo_controller::ControllerConfig;
use millo_domain::{
    ControllerSnapshot, DeviceInspection, HardwareInspection, HardwareProfile, JogPadStepOutcome,
    JogPadStepRequest, OperatorConfirmation, OverrideAdjustment, RapidOverrideTarget,
    ResetChallenge, StepJogReceipt, StepJogRequest, TestJogPreparation, WorkZeroOutcome,
    WorkZeroRequest,
};
use millo_dry_run::{DryRunPlan, DryRunPolicyError, ProgramExecutionOptions, build_dry_run_plan};
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
use millo_run::{
    FirstCutConfirmation, FirstCutPreparation, ProgramRunIntent, RunPreflightReport,
    ToolChangeConfirmation,
};
use millo_sender::SenderSnapshot;
use millo_serial::{
    SerialConfig, SerialPortDescriptor, SerialPortKind, SerialTransport,
    available_ports as available_serial_ports,
};
use millo_settings::{
    ControllerSettingEditRequest, ControllerSettingsSnapshot, MachineSettingsArchive,
    build_settings_snapshot,
};
use millo_transport::BoxedTransport;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::{
    sync::{Mutex, mpsc},
    task::JoinHandle,
};

const MOCK_TRANSPORT_ID: &str = "mock";
const SERIAL_TRANSPORT_PREFIX: &str = "serial:";

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
    profiles: Mutex<MachineProfileStore>,
    active_transport: Mutex<TransportDescriptor>,
    mock: Mutex<Option<MockControl>>,
    transition_lock: Mutex<()>,
    event_task: Mutex<Option<JoinHandle<()>>>,
    settings_root: Option<PathBuf>,
    settings_session: Mutex<Option<ActiveControllerSettings>>,
    run_journal: Arc<StdMutex<RunJournal>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::from_profile_store(
            MachineProfileStore::in_memory(),
            None,
            RunJournal::in_memory(),
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
        let profiles =
            MachineProfileStore::load(profile_path).map_err(|error| error.to_string())?;
        let journal = journal_path
            .map(RunJournal::load)
            .transpose()
            .map_err(|error| error.to_string())?
            .unwrap_or_else(RunJournal::in_memory);
        Ok(Self::from_profile_store(profiles, settings_root, journal))
    }

    fn from_profile_store(
        profiles: MachineProfileStore,
        settings_root: Option<PathBuf>,
        run_journal: RunJournal,
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
            profiles: Mutex::new(profiles),
            active_transport: Mutex::new(descriptor),
            mock: Mutex::new(mock),
            transition_lock: Mutex::new(()),
            event_task: Mutex::new(None),
            settings_root,
            settings_session: Mutex::new(None),
            run_journal: Arc::new(StdMutex::new(run_journal)),
        }
    }

    async fn start_event_bridge(&self, app: AppHandle) {
        let mut event_task = self.event_task.lock().await;
        if event_task.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }

        let mut snapshots = self.arbiter.subscribe();
        let mut sender_snapshots = self.arbiter.subscribe_sender();
        let journal_sender = start_run_journal_worker(Arc::clone(&self.run_journal));
        *event_task = Some(tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = snapshots.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        let snapshot = snapshots.borrow_and_update().clone();
                        if let Err(error) = app.emit("machine-state", snapshot) {
                            eprintln!("machine-state event emission failed: {error}");
                        }
                    }
                    changed = sender_snapshots.changed() => {
                        if changed.is_err() {
                            break;
                        }
                        let snapshot = sender_snapshots.borrow_and_update().clone();
                        if let Err(error) = app.emit("dry-run-state", snapshot.clone()) {
                            eprintln!("dry-run-state event emission failed: {error}");
                        }
                        if let Some(sender) = journal_sender.as_ref()
                            && sender.send(snapshot).await.is_err()
                        {
                            eprintln!("sender journal worker stopped unexpectedly");
                        }
                    }
                }
            }
        }));
    }
}

fn start_run_journal_worker(
    journal: Arc<StdMutex<RunJournal>>,
) -> Option<mpsc::Sender<SenderSnapshot>> {
    let (sender, mut snapshots) = mpsc::channel::<SenderSnapshot>(128);
    let worker = std::thread::Builder::new()
        .name("millo-run-journal".to_owned())
        .spawn(move || {
            while let Some(snapshot) = snapshots.blocking_recv() {
                let mut journal = match journal.lock() {
                    Ok(journal) => journal,
                    Err(error) => {
                        eprintln!("sender journal lock poisoned: {error}");
                        break;
                    }
                };
                if let Err(error) = journal.observe(&snapshot, SystemTime::now(), Instant::now()) {
                    eprintln!("sender journal checkpoint failed: {error}");
                }
            }
        });
    match worker {
        Ok(_) => Some(sender),
        Err(error) => {
            eprintln!("sender journal worker could not start: {error}");
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
pub async fn machine_profiles(state: State<'_, AppState>) -> Result<MachineProfileState, String> {
    Ok(state.profiles.lock().await.state())
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
    state.start_event_bridge(app).await;
    let replacement = resolve_transport(&transport_id, baud_rate).await?;
    let descriptor = replacement.descriptor.clone();

    state
        .arbiter
        .replace_transport_with_execution_target(
            replacement.transport,
            replacement.execution_target,
        )
        .await
        .map_err(|error| error.to_string())?;
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
        Ok(outcome) => Ok(outcome),
        Err(connection_error) => {
            *state.settings_session.lock().await = None;
            match state.arbiter.disconnect().await {
                Ok(_) => Err(connection_error),
                Err(cleanup_error) => Err(format!(
                    "{connection_error}; connection cleanup also failed: {cleanup_error}"
                )),
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
    state
        .arbiter
        .refresh_status()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn inspect_device(state: State<'_, AppState>) -> Result<HardwareInspection, String> {
    let inspection = state
        .arbiter
        .inspect_device()
        .await
        .map_err(|error| error.to_string())?;
    if let Some(active) = state.settings_session.lock().await.as_mut() {
        active.inspection = inspection.device.clone();
        active.revision = active.revision.saturating_add(1);
        if let Some(archive) = active.archive.as_mut() {
            archive
                .record_observation(&inspection.device)
                .map_err(|error| error.to_string())?;
        }
    }
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
    apply_controller_setting(&state, request).await
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
pub async fn feed_hold(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    state
        .arbiter
        .feed_hold()
        .await
        .map_err(|error| error.to_string())
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
    state
        .arbiter
        .request_soft_reset()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn confirm_soft_reset(
    challenge_id: u64,
    state: State<'_, AppState>,
) -> Result<ControllerSnapshot, String> {
    state
        .arbiter
        .confirm_soft_reset(challenge_id)
        .await
        .map_err(|error| error.to_string())
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
    ensure_machine_bound(&state).await?;
    state
        .arbiter
        .jog_pad_step(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_work_zero(
    request: WorkZeroRequest,
    state: State<'_, AppState>,
) -> Result<WorkZeroOutcome, String> {
    ensure_machine_bound(&state).await?;
    state
        .arbiter
        .set_work_zero(request)
        .await
        .map_err(|error| error.to_string())
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
pub async fn preflight_real_run(
    request: ProgramParseRequest,
    intent: ProgramRunIntent,
    execution_options: ProgramExecutionOptions,
    state: State<'_, AppState>,
) -> Result<RunPreflightReport, String> {
    let _transition = state.transition_lock.lock().await;
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

#[tauri::command]
pub async fn authorize_first_cut(
    request: ProgramParseRequest,
    confirmation: FirstCutConfirmation,
    state: State<'_, AppState>,
) -> Result<FirstCutPreparation, String> {
    let _transition = state.transition_lock.lock().await;
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

#[tauri::command]
pub async fn start_program_run(
    request: ProgramParseRequest,
    authorization_id: u64,
    execution_options: ProgramExecutionOptions,
    state: State<'_, AppState>,
) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    ensure_machine_bound(&state).await?;
    if state.active_transport.lock().await.kind != TransportKind::Serial {
        return Err("program run requires an active serial transport".to_owned());
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
    .map_err(|error| format!("program-run parser task failed: {error}"))?
    .map_err(|error| error.to_string())?;
    state
        .arbiter
        .start_program_run(program, authorization_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn start_check_run(
    request: ProgramParseRequest,
    execution_options: ProgramExecutionOptions,
    state: State<'_, AppState>,
) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    ensure_machine_bound(&state).await?;
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
pub async fn resume_program_run(state: State<'_, AppState>) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
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

#[tauri::command]
pub async fn complete_tool_change(
    confirmation: ToolChangeConfirmation,
    state: State<'_, AppState>,
) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
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
    state
        .arbiter
        .pause_dry_run()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn resume_dry_run(state: State<'_, AppState>) -> Result<SenderSnapshot, String> {
    state
        .arbiter
        .resume_dry_run()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn cancel_dry_run(state: State<'_, AppState>) -> Result<SenderSnapshot, String> {
    state
        .arbiter
        .cancel_dry_run()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn cancel_jog(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    state
        .arbiter
        .cancel_jog()
        .await
        .map_err(|error| error.to_string())
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

    #[tokio::test]
    async fn run_journal_worker_processes_snapshots_off_the_async_task() {
        let journal = Arc::new(StdMutex::new(RunJournal::in_memory()));
        let sender = start_run_journal_worker(Arc::clone(&journal)).unwrap();
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
