use super::*;

pub(super) async fn apply_controller_setting(
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

pub(super) async fn ensure_machine_bound(state: &AppState) -> Result<(), String> {
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

pub(super) fn settings_state(active: &ActiveControllerSettings) -> ControllerSettingsState {
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

pub(super) fn begin_settings_archive(
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

#[tauri::command]
pub async fn application_preferences(
    state: State<'_, AppState>,
) -> Result<ApplicationPreferences, String> {
    Ok(state.preferences.lock().await.preferences())
}

#[tauri::command]
pub async fn update_application_preferences(
    update: ApplicationPreferencesUpdate,
    state: State<'_, AppState>,
) -> Result<ApplicationPreferences, String> {
    let _transition = state.transition_lock.lock().await;
    let preferences = state
        .preferences
        .lock()
        .await
        .update(update)
        .map_err(|error| error.to_string())?;
    state.audit.record(
        if preferences.safe_command_mode {
            AuditLevel::Info
        } else {
            AuditLevel::Warning
        },
        AuditCategory::Application,
        "application.safe_command_mode_changed",
        if preferences.safe_command_mode {
            "Safe command mode enabled"
        } else {
            "Safe command mode disabled; expert console and granted plugin commands are available"
        },
        json!({ "safeCommandMode": preferences.safe_command_mode }),
    );
    Ok(preferences)
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
