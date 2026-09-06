use super::*;
use millo_sketch::{GeneratedSketchJob, SketchJobRequest};

#[tauri::command]
pub async fn generate_sketch_job(
    request: SketchJobRequest,
    state: State<'_, AppState>,
) -> Result<GeneratedSketchJob, String> {
    let tools = state.tools.lock().await.state().tools;
    let context = json!({ "sourceName": request.source_name, "shapes": request.shapes.len(), "stock": request.stock });
    let result = background_compute::run("Sketch CAM task failed", move || {
        millo_sketch::generate_sketch_job(request, &tools)
    })
    .await;
    audit_operation(
        &state.audit,
        AuditCategory::Program,
        "program.sketch_job_generated",
        "Sketch G-code generated and reparsed",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn parse_gcode_program(
    request: ProgramParseRequest,
    options: Option<ProgramParseOptions>,
) -> Result<GcodeProgram, String> {
    background_compute::run("G-code parser task failed", move || {
        parse_program_with_options(request, options.unwrap_or_default())
    })
    .await
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
    let result = background_compute::run("Image job generation task failed", move || {
        generate_image_job_core(request)
    })
    .await;
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
    let result = background_compute::run("Surfacing job generation task failed", move || {
        generate_surfacing_job_core(request, &tool)
    })
    .await;
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
    let result = background_compute::run("PCB inspection task failed", move || {
        inspect_pcb_core(request)
    })
    .await;
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
    let result = background_compute::run("PCB generation task failed", move || {
        generate_pcb_job_core(request, &tools)
    })
    .await;
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

pub(super) async fn save_validated_gcode(
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
    background_compute::run("G-code validation task failed", move || {
        parse_program(validation).map_err(|error| format!("G-code is invalid: {error}"))
    })
    .await?;

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
    let rotary_state = if request.rotary_clearance_confirmed {
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
        verified_rotary_restart_state(
            &snapshot,
            &inspection.device,
            request.initial_work_a_degrees,
            true,
        )?
    } else {
        None
    };
    let result = background_compute::run("Selected-run planner task failed", move || {
        prepare_selected_run_with_rotary(request, rotary_state)
    })
    .await;
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

#[cfg(test)]
pub(super) fn prepare_selected_run(
    request: SelectedRunPreparationRequest,
) -> Result<SafeStartPackage, String> {
    prepare_selected_run_with_rotary(request, None)
}

fn prepare_selected_run_with_rotary(
    request: SelectedRunPreparationRequest,
    rotary: Option<millo_restart::RotaryRestartState>,
) -> Result<SafeStartPackage, String> {
    let document = program_documents::resolve_program_blocking(
        request.request.clone(),
        ProgramParseOptions {
            block_delete: request.execution_options.block_delete,
        },
    )
    .map_err(|error| error.to_string())?;
    let program = &document.program;
    let package = millo_restart::build_safe_start_with_rotary(
        program,
        &document.source,
        SafeStartRequest {
            selected_source_line: request.selected_source_line,
            safe_z_mm: request.safe_z_mm,
            intent: match request.intent {
                ProgramRunIntent::AirRun => SafeStartIntent::AirRun,
                ProgramRunIntent::Cutting => SafeStartIntent::Cutting,
            },
        },
        rotary,
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
