use super::*;

#[tauri::command]
pub async fn tool_library(state: State<'_, AppState>) -> Result<ToolLibraryState, String> {
    Ok(state.tools.lock().await.state())
}

#[tauri::command]
pub async fn create_cutting_tool(
    draft: CuttingToolDraft,
    state: State<'_, AppState>,
) -> Result<ToolLibraryState, String> {
    let context = json!({ "name": &draft.name, "kind": draft.kind });
    let result = state
        .tools
        .lock()
        .await
        .create(draft)
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Storage,
        "storage.tool_created",
        "Cutting tool added to the library",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn update_cutting_tool(
    tool_id: String,
    draft: CuttingToolDraft,
    state: State<'_, AppState>,
) -> Result<ToolLibraryState, String> {
    let context = json!({ "toolId": &tool_id, "name": &draft.name, "kind": draft.kind });
    let result = state
        .tools
        .lock()
        .await
        .update(&tool_id, draft)
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Storage,
        "storage.tool_updated",
        "Cutting tool updated",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn delete_cutting_tool(
    tool_id: String,
    state: State<'_, AppState>,
) -> Result<ToolLibraryState, String> {
    let context = json!({ "toolId": &tool_id });
    let result = state
        .tools
        .lock()
        .await
        .delete(&tool_id)
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Storage,
        "storage.tool_deleted",
        "Cutting tool removed from the library",
        context,
        &result,
    );
    result
}

#[tauri::command]
pub async fn restore_cutting_tool_presets(
    state: State<'_, AppState>,
) -> Result<ToolLibraryState, String> {
    let result = state
        .tools
        .lock()
        .await
        .restore_missing_presets()
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Storage,
        "storage.tool_presets_restored",
        "Missing cutting-tool presets restored",
        Value::Null,
        &result,
    );
    result
}
