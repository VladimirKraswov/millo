use super::*;

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
    let _execution = state.script_execution.lock().await;
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
    let _execution = state.script_execution.lock().await;
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
        let _execution = state.script_execution.lock().await;
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
    let _execution = state.script_execution.lock().await;
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
    let _execution = state.script_execution.lock().await;
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
    let _execution = state.script_execution.lock().await;
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
        ScriptAction::RawCommand { .. } => "rawCommand",
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
            Ok(ScriptPluginExecutionOutcome::Job { job: Box::new(job) })
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
                snapshot: Box::new(state.arbiter.snapshot()),
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
                snapshot: Box::new(outcome.snapshot),
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
                snapshot: Box::new(outcome.snapshot),
            })
        }
        ScriptAction::RawCommand { command } => {
            ensure_script_motion_confirmed(request.operator_confirmed)?;
            ensure_machine_bound(&state).await?;
            let _transition = state.transition_lock.lock().await;
            if state
                .preferences
                .lock()
                .await
                .preferences()
                .safe_command_mode
            {
                return Err(
                    "safe command mode is enabled; disable it in application settings before running a plugin raw command"
                        .to_owned(),
                );
            }
            let exchange = state
                .arbiter
                .execute_operator_console(command.clone(), OperatorConsolePolicy::Expert)
                .await
                .map_err(|error| error.to_string())?;
            state.audit.record(
                AuditLevel::Warning,
                AuditCategory::Controller,
                "plugin.raw_command.completed",
                "Plugin executed an expert GRBL command through the controller actor",
                json!({
                    "pluginId": request.plugin_id,
                    "commandId": request.command_id,
                    "command": command,
                    "completion": exchange.completion,
                    "code": exchange.code,
                }),
            );
            Ok(ScriptPluginExecutionOutcome::Machine {
                action: "rawCommand".to_owned(),
                message: if exchange.lines.is_empty() {
                    format!("{} · {:?}", exchange.command, exchange.completion)
                } else {
                    format!(
                        "{} · {:?} · {}",
                        exchange.command,
                        exchange.completion,
                        exchange.lines.join(" · ")
                    )
                },
                snapshot: Box::new(exchange.snapshot),
            })
        }
    }
}
