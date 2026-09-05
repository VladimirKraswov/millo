use super::*;

#[tauri::command]
pub async fn probe_z(
    request: ZProbeRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ZProbeOutcome, String> {
    let context = serde_json::to_value(request).unwrap_or(Value::Null);
    let result = async {
        ensure_machine_bound(&state).await?;
        if !request.setup_confirmed {
            return state
                .arbiter
                .probe_z(request)
                .await
                .map_err(|error| error.to_string());
        }
        // A Z-zero write changes the datum of every stored surface sample. The
        // session must be durably disarmed before any probe motion can begin.
        let session = state
            .surface_session
            .lock()
            .map_err(|error| format!("surface session lock poisoned: {error}"))?
            .disarm_for_coordinate_change()
            .map_err(|error| format!("could not disarm heightmap before Z probing: {error}"))?;
        let _ = app.emit("surface-session", session);
        state
            .arbiter
            .probe_z(request)
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Controller,
        "controller.z_probe",
        "Guarded Z probe completed and verified",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn surface_session(state: State<'_, AppState>) -> Result<SurfaceSession, String> {
    state
        .surface_session
        .lock()
        .map(|store| store.session())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn heightmap_snapshot(
    state: State<'_, AppState>,
) -> Result<HeightmapOperationSnapshot, String> {
    Ok(state.arbiter.heightmap_snapshot())
}

#[tauri::command]
pub async fn start_heightmap(
    request: HeightmapStartRequest,
    machine_profile_id: String,
    state: State<'_, AppState>,
) -> Result<HeightmapOperationSnapshot, String> {
    ensure_machine_bound(&state).await?;
    let selected = state
        .profiles
        .lock()
        .await
        .state()
        .selected_profile_id
        .ok_or_else(|| "select a machine profile before probing".to_owned())?;
    if selected != machine_profile_id {
        return Err("heightmap machine profile does not match the selected machine".to_owned());
    }
    let prepared = state
        .arbiter
        .prepare_heightmap(request)
        .await
        .map_err(|error| error.to_string())?;
    let persisted = state
        .surface_session
        .lock()
        .map_err(|error| error.to_string())
        .and_then(|mut store| {
            store
                .begin(machine_profile_id, prepared.clone(), unix_time_ms())
                .map_err(|error| error.to_string())
        });
    if let Err(error) = persisted {
        let cleanup = state
            .arbiter
            .discard_prepared_heightmap(prepared.operation_sequence)
            .await;
        return Err(match cleanup {
            Ok(()) => error,
            Err(cleanup) => format!("{error}; prepared heightmap cleanup also failed: {cleanup}"),
        });
    }

    match state
        .arbiter
        .commit_prepared_heightmap(prepared.operation_sequence)
        .await
    {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            let actor_cleanup = state
                .arbiter
                .discard_prepared_heightmap(prepared.operation_sequence)
                .await;
            let mut details = vec![error.to_string()];
            match actor_cleanup {
                Ok(()) => {
                    if let Err(cleanup) = state
                        .surface_session
                        .lock()
                        .map_err(|lock_error| lock_error.to_string())?
                        .discard_pending()
                    {
                        details.push(format!("storage cleanup failed: {cleanup}"));
                    }
                }
                Err(cleanup) => {
                    details.push(format!("actor cleanup failed: {cleanup}"));
                    details.push(
                        "the durable pending surface session was retained because commit outcome is uncertain"
                            .to_owned(),
                    );
                }
            }
            Err(details.join("; "))
        }
    }
}

#[tauri::command]
pub async fn resume_heightmap_draft(
    request: HeightmapResumeRequest,
    machine_profile_id: String,
    state: State<'_, AppState>,
) -> Result<HeightmapOperationSnapshot, String> {
    ensure_machine_bound(&state).await?;
    if state.arbiter.snapshot().reset_notice.is_some() {
        state
            .arbiter
            .acknowledge_reset()
            .await
            .map_err(|error| format!("controller resynchronization failed: {error}"))?;
    }
    let selected = state
        .profiles
        .lock()
        .await
        .state()
        .selected_profile_id
        .ok_or_else(|| "select a machine profile before resuming probing".to_owned())?;
    if selected != machine_profile_id {
        return Err("heightmap draft does not belong to the selected machine".to_owned());
    }
    let previous = state
        .surface_session
        .lock()
        .map_err(|error| error.to_string())?
        .session()
        .pending
        .ok_or_else(|| "there is no unfinished heightmap to resume".to_owned())?;
    if previous.machine_profile_id != machine_profile_id {
        return Err("heightmap draft does not belong to the selected machine".to_owned());
    }
    if !matches!(
        previous.operation.state,
        HeightmapOperationState::Running
            | HeightmapOperationState::Failed
            | HeightmapOperationState::Cancelled
    ) {
        return Err("only a stopped heightmap draft can be resumed".to_owned());
    }
    let map = previous
        .operation
        .map
        .clone()
        .ok_or_else(|| "unfinished heightmap has no saved samples".to_owned())?;
    let prepared = state
        .arbiter
        .prepare_resume_heightmap(map, request)
        .await
        .map_err(|error| error.to_string())?;
    let persisted = state
        .surface_session
        .lock()
        .map_err(|error| error.to_string())
        .and_then(|mut store| {
            store
                .resume_pending(&machine_profile_id, prepared.clone(), unix_time_ms())
                .map_err(|error| error.to_string())
        });
    if let Err(error) = persisted {
        let cleanup = state
            .arbiter
            .discard_prepared_heightmap(prepared.operation_sequence)
            .await;
        return Err(match cleanup {
            Ok(()) => error,
            Err(cleanup) => format!("{error}; prepared heightmap cleanup also failed: {cleanup}"),
        });
    }

    match state
        .arbiter
        .commit_prepared_heightmap(prepared.operation_sequence)
        .await
    {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => {
            let cleanup = state
                .arbiter
                .discard_prepared_heightmap(prepared.operation_sequence)
                .await;
            let mut details = vec![error.to_string()];
            match cleanup {
                Ok(()) => {
                    if let Err(restore) = state
                        .surface_session
                        .lock()
                        .map_err(|lock_error| lock_error.to_string())?
                        .restore_pending_after_failed_resume(prepared.operation_sequence, previous)
                    {
                        details.push(format!("draft restore failed: {restore}"));
                    }
                }
                Err(cleanup) => details.push(format!("actor cleanup failed: {cleanup}")),
            }
            Err(details.join("; "))
        }
    }
}

#[tauri::command]
pub async fn discard_heightmap_draft(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SurfaceSession, String> {
    let session = state
        .surface_session
        .lock()
        .map_err(|error| error.to_string())?
        .discard_pending()
        .map_err(|error| error.to_string())?;
    let _ = app.emit("surface-session", session.clone());
    Ok(session)
}

#[tauri::command]
pub async fn pause_heightmap(
    state: State<'_, AppState>,
) -> Result<HeightmapOperationSnapshot, String> {
    state
        .arbiter
        .pause_heightmap()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn resume_heightmap(
    state: State<'_, AppState>,
) -> Result<HeightmapOperationSnapshot, String> {
    state
        .arbiter
        .resume_heightmap()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn cancel_heightmap(
    state: State<'_, AppState>,
) -> Result<HeightmapOperationSnapshot, String> {
    state
        .arbiter
        .cancel_heightmap()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_heightmap_application(
    enabled: bool,
    setup_confirmed: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SurfaceSession, String> {
    let session = state
        .surface_session
        .lock()
        .map_err(|error| error.to_string())?
        .set_application_enabled(enabled, setup_confirmed)
        .map_err(|error| error.to_string())?;
    let _ = app.emit("surface-session", session.clone());
    Ok(session)
}

#[tauri::command]
pub async fn clear_surface_session(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SurfaceSession, String> {
    let mut store = state
        .surface_session
        .lock()
        .map_err(|error| error.to_string())?;
    store.discard_pending().map_err(|error| error.to_string())?;
    let session = store.forget_active().map_err(|error| error.to_string())?;
    let _ = app.emit("surface-session", session.clone());
    Ok(session)
}
