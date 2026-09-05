use super::*;

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
pub async fn start_homing(
    request: HomingRequest,
    state: State<'_, AppState>,
) -> Result<HomingStartOutcome, String> {
    let context = serde_json::to_value(request).unwrap_or(Value::Null);
    let result = async {
        ensure_machine_bound(&state).await?;
        state
            .arbiter
            .start_homing(request)
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Safety,
        "controller.homing.start",
        "GRBL homing lifecycle started",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn start_continuous_jog(
    request: ContinuousJogRequest,
    state: State<'_, AppState>,
) -> Result<ContinuousJogReceipt, String> {
    let context = serde_json::to_value(request).unwrap_or(Value::Null);
    let result = async {
        ensure_machine_bound(&state).await?;
        state
            .arbiter
            .start_continuous_jog(request)
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Controller,
        "controller.jog.continuous.start",
        "Bounded continuous jog started",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn select_work_coordinate_system(
    coordinate_system: WorkCoordinateSystem,
    state: State<'_, AppState>,
) -> Result<WorkCoordinateSelectionOutcome, String> {
    ensure_machine_bound(&state).await?;
    state
        .arbiter
        .select_work_coordinate_system(coordinate_system)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_machine_output(
    request: MachineOutputRequest,
    state: State<'_, AppState>,
) -> Result<MachineOutputOutcome, String> {
    let context = serde_json::to_value(request).unwrap_or(Value::Null);
    let result = async {
        ensure_machine_bound(&state).await?;
        state
            .arbiter
            .set_machine_output(request)
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Controller,
        "controller.output.set",
        "Machine output command verified",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn set_work_zero(
    request: WorkZeroRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<WorkZeroOutcome, String> {
    let context = serde_json::to_value(request).unwrap_or(Value::Null);
    let result = async {
        ensure_machine_bound(&state).await?;
        if !request.position_confirmed {
            return state
                .arbiter
                .set_work_zero(request)
                .await
                .map_err(|error| error.to_string());
        }
        // Every surface sample is expressed in the current work coordinate
        // frame, so any G10 zero write invalidates automatic application.
        let session = state
            .surface_session
            .lock()
            .map_err(|error| format!("surface session lock poisoned: {error}"))?
            .disarm_for_coordinate_change()
            .map_err(|error| {
                format!("could not disarm heightmap before changing work zero: {error}")
            })?;
        let _ = app.emit("surface-session", session);
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
pub async fn return_to_work_origin(
    request: ReturnToWorkOriginRequest,
    state: State<'_, AppState>,
) -> Result<ReturnToWorkOriginOutcome, String> {
    let context = serde_json::to_value(request).unwrap_or(Value::Null);
    let result = async {
        ensure_machine_bound(&state).await?;
        state
            .arbiter
            .return_to_work_origin(request)
            .await
            .map_err(|error| error.to_string())
    }
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Controller,
        "controller.return_to_work_origin",
        "Safe return to work origin completed",
        context,
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
