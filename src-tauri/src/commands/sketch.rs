use super::*;
use millo_sketch::SketchJobRequest;

#[tauri::command]
pub async fn save_sketch_project(
    request: SketchJobRequest,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<GeneratedGcodeSaveOutcome>, String> {
    let request = millo_sketch::resolve_sketch(request).map_err(|e| e.to_string())?;
    let filename = millo_sketch::project_file_name(&request.source_name);
    if request.shapes.len() > 200 {
        return Err("В проекте больше 200 фигур".into());
    }
    // A project can be incomplete or reference tools absent on this computer.
    // Saving is deliberately independent of machining readiness.
    let bytes = serde_json::to_vec_pretty(&json!({ "version": 2, "document": request }))
        .map_err(|e| e.to_string())?;
    if bytes.len() > 512_000 {
        return Err("Проект больше 512 КБ".into());
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_file_name(filename)
        .add_filter("Millo Sketch", &["json"])
        .save_file(move |path| {
            let _ = tx.send(path);
        });
    let Some(path) = rx.await.map_err(|_| "Диалог сохранения закрыт")? else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|e| e.to_string())?;
    let destination = path.clone();
    let bytes_written = bytes.len();
    tokio::task::spawn_blocking(move || {
        millo_storage::replace_file_atomically(&destination, &bytes)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("Не удалось сохранить проект: {e}"))?;
    state.audit.record(
        AuditLevel::Info,
        AuditCategory::Storage,
        "storage.sketch_project_saved",
        "Sketch project saved",
        json!({"path":path,"bytesWritten":bytes_written}),
    );
    Ok(Some(GeneratedGcodeSaveOutcome {
        path: path.to_string_lossy().into_owned(),
        bytes_written,
    }))
}
