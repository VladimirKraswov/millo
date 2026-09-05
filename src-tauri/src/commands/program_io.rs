use super::*;

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
pub async fn inspect_pcb_job(
    request: PcbInspectRequest,
    state: State<'_, AppState>,
) -> Result<PcbInspection, String> {
    let context = json!({
        "files": request.files.iter().map(|file| json!({
            "sourceName": &file.source_name,
            "role": file.role,
            "encodedBytes": file.source_base64.len(),
        })).collect::<Vec<_>>(),
        "transform": &request.transform,
    });
    let result = tokio::task::spawn_blocking(move || inspect_pcb_core(request))
        .await
        .map_err(|error| format!("PCB inspection task failed: {error}"))?
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Program,
        "program.pcb_inspected",
        "PCB sources inspected",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn generate_pcb_job(
    request: PcbJobRequest,
    state: State<'_, AppState>,
) -> Result<GeneratedPcbJob, String> {
    let tools = state.tools.lock().await.state().tools;
    let context = json!({
        "sourceName": &request.source_name,
        "files": request.board.files.iter().map(|file| json!({
            "sourceName": &file.source_name,
            "role": file.role,
            "encodedBytes": file.source_base64.len(),
        })).collect::<Vec<_>>(),
        "settings": &request.settings,
    });
    let result = tokio::task::spawn_blocking(move || generate_pcb_job_core(request, &tools))
        .await
        .map_err(|error| format!("PCB generation task failed: {error}"))?
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Program,
        "program.pcb_job_generated",
        "PCB G-code generated and reparsed",
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
    let validation = request.clone();
    tokio::task::spawn_blocking(move || parse_program(validation))
        .await
        .map_err(|error| format!("G-code validation task failed: {error}"))?
        .map_err(|error| format!("G-code is invalid: {error}"))?;

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
    tokio::task::spawn_blocking(move || {
        millo_storage::replace_file_atomically(&output_path, request.source.as_bytes())
    })
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

pub(super) fn valid_program_gcode_name(value: &str) -> bool {
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

pub(super) fn prepare_selected_run(
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
