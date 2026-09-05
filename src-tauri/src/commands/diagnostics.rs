use super::*;

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
pub fn report_ui_error(
    component: String,
    message: String,
    stack: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if component.len() > 640 || message.len() > 16_000 || stack.len() > 32_000 {
        return Err("UI diagnostic exceeds the size limit".to_owned());
    }
    state.audit.record(
        AuditLevel::Error,
        AuditCategory::Ui,
        "ui.render_failed",
        &message,
        json!({ "component": component, "stack": stack }),
    );
    Ok(())
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
