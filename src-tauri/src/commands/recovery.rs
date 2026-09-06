use super::*;

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
    pub(super) fn missing(self) -> Vec<&'static str> {
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
    let inspection = state
        .arbiter
        .inspect_device()
        .await
        .map_err(|error| error.to_string())?;
    let snapshot = state
        .arbiter
        .refresh_status()
        .await
        .map_err(|error| error.to_string())?;
    if !snapshot.is_stable_idle() {
        return Err("program recovery requires fresh Connected + Idle state".to_owned());
    }
    let rotary_state = if snapshot
        .machine
        .work_position
        .and_then(|position| position.a)
        .is_some()
    {
        verified_rotary_restart_state(
            &snapshot,
            &inspection.device,
            snapshot
                .machine
                .work_position
                .and_then(|position| position.a),
            request.path_clear && request.work_zero_restored,
        )?
    } else {
        None
    };
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
            .prepare_with_rotary(
                request.recovery_id,
                request.safe_z_mm,
                request.continuity,
                rotary_state,
            )
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
