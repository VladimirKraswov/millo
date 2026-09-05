use super::*;

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
        let heightmap = selected_surface_map_for_active_profile(execution_options, &state)
            .await?
            .map(|stored| stored.map);
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
            .preflight_real_run_with_heightmap(program, intent, execution_options, heightmap)
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
        let execution_options = confirmation.execution_options;
        let heightmap = selected_surface_map_for_active_profile(execution_options, &state)
            .await?
            .map(|stored| stored.map);
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
            .authorize_first_cut_with_heightmap(program, confirmation, heightmap)
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

pub(super) async fn start_program_run_impl(
    request: ProgramParseRequest,
    authorization_id: u64,
    execution_options: ProgramExecutionOptions,
    state: &AppState,
) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    ensure_machine_bound(state).await?;
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
    let heightmap = if execution_options.surface_map_id.is_some() {
        let selected_profile_id = profile_id.as_deref().ok_or_else(|| {
            "heightmap compensation requires the controller to be linked to a machine profile"
                .to_owned()
        })?;
        selected_surface_map(execution_options, state, selected_profile_id)?
            .map(|stored| stored.map)
    } else {
        None
    };
    let stored_source_name = program.source_name.clone();
    let fingerprint = program_fingerprint(&program);
    let prepared = state
        .arbiter
        .prepare_program_run_with_heightmap(program, authorization_id, heightmap)
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

pub(super) fn selected_surface_map(
    options: ProgramExecutionOptions,
    state: &AppState,
    profile_id: &str,
) -> Result<Option<millo_heightmap::StoredSurfaceMap>, String> {
    let Some(requested_map_id) = options.surface_map_id else {
        return Ok(None);
    };
    let session = state
        .surface_session
        .lock()
        .map_err(|error| format!("surface session lock poisoned: {error}"))?
        .session();
    if !session.application_enabled {
        return Err("heightmap application is not enabled for this workpiece".to_owned());
    }
    if session.coordinate_binding_stale {
        return Err(
            "heightmap work-coordinate binding is stale; measure a new map after setting work zero"
                .to_owned(),
        );
    }
    let active = session
        .active
        .ok_or_else(|| "selected heightmap is no longer available".to_owned())?;
    if active.map_id != requested_map_id {
        return Err(format!(
            "heightmap changed after preparation: expected #{requested_map_id}, active #{}",
            active.map_id
        ));
    }
    if active.machine_profile_id != profile_id {
        return Err("heightmap belongs to a different machine profile".to_owned());
    }
    Ok(Some(active))
}

pub(super) async fn selected_surface_map_for_active_profile(
    options: ProgramExecutionOptions,
    state: &AppState,
) -> Result<Option<millo_heightmap::StoredSurfaceMap>, String> {
    if options.surface_map_id.is_none() {
        return Ok(None);
    }
    let profile_id = state
        .settings_session
        .lock()
        .await
        .as_ref()
        .and_then(|session| session.profile_id.clone())
        .ok_or_else(|| {
            "heightmap compensation requires the controller to be linked to a machine profile"
                .to_owned()
        })?;
    let selected = selected_surface_map(options, state, &profile_id)?;
    let Some(selected) = selected else {
        return Ok(None);
    };
    let binding = selected.map.coordinate_binding.ok_or_else(|| {
        "heightmap has no work-coordinate binding; measure a new map after setting work zero"
            .to_owned()
    })?;
    let snapshot = state
        .arbiter
        .refresh_status()
        .await
        .map_err(|error| error.to_string())?;
    let inspection = state
        .arbiter
        .inspect_device()
        .await
        .map_err(|error| error.to_string())?;
    let coordinate_system = active_work_coordinate_system(&inspection.device.modal_state)
        .ok_or_else(|| {
            "controller did not report an active G54-G59 work coordinate system".to_owned()
        })?;
    let offset = snapshot
        .machine
        .work_coordinate_offset
        .or_else(|| {
            snapshot
                .machine
                .machine_position
                .zip(snapshot.machine.work_position)
                .map(|(machine, work)| millo_domain::Position {
                    x: machine.x - work.x,
                    y: machine.y - work.y,
                    z: machine.z - work.z,
                    a: None,
                })
        })
        .ok_or_else(|| {
            "controller did not report enough position data to verify the heightmap".to_owned()
        })?;
    let bound = binding.work_coordinate_offset;
    let same_offset = (bound.x - offset.x).abs() <= 0.01
        && (bound.y - offset.y).abs() <= 0.01
        && (bound.z - offset.z).abs() <= 0.01;
    if binding.coordinate_system != coordinate_system || !same_offset {
        let _ = state
            .surface_session
            .lock()
            .map_err(|error| format!("surface session lock poisoned: {error}"))?
            .disarm_for_coordinate_change();
        return Err(
            "work zero or G54-G59 changed after the heightmap was measured; measure a new map"
                .to_owned(),
        );
    }
    Ok(Some(selected))
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

pub(super) async fn start_check_run_impl(
    request: ProgramParseRequest,
    execution_options: ProgramExecutionOptions,
    state: &AppState,
) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    ensure_machine_bound(state).await?;
    let heightmap = selected_surface_map_for_active_profile(execution_options, state)
        .await?
        .map(|stored| stored.map);
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
        .start_check_run_with_heightmap(program, execution_options, heightmap)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn pause_program_run(state: State<'_, AppState>) -> Result<SenderSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    let result = async {
        ensure_machine_bound(&state).await?;
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
