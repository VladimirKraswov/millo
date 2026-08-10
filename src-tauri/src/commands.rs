use gantryon_controller::Controller;
use gantryon_domain::ControllerSnapshot;
use gantryon_mock::MockTransport;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

pub struct AppState {
    controller: Mutex<Controller<MockTransport>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            controller: Mutex::new(Controller::new(MockTransport::default())),
        }
    }
}

#[tauri::command]
pub async fn controller_snapshot(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    Ok(state.controller.lock().await.snapshot())
}

#[tauri::command]
pub async fn connect_mock(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ControllerSnapshot, String> {
    let mut controller = state.controller.lock().await;
    controller
        .connect()
        .await
        .map_err(|error| error.to_string())?;
    let snapshot = controller
        .refresh_status()
        .await
        .map_err(|error| error.to_string())?;
    drop(controller);

    publish_snapshot(&app, &snapshot)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn refresh_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ControllerSnapshot, String> {
    let mut controller = state.controller.lock().await;
    let snapshot = controller
        .refresh_status()
        .await
        .map_err(|error| error.to_string())?;
    drop(controller);

    publish_snapshot(&app, &snapshot)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn disconnect(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ControllerSnapshot, String> {
    let mut controller = state.controller.lock().await;
    let snapshot = controller
        .disconnect()
        .await
        .map_err(|error| error.to_string())?;
    drop(controller);

    publish_snapshot(&app, &snapshot)?;
    Ok(snapshot)
}

fn publish_snapshot(app: &AppHandle, snapshot: &ControllerSnapshot) -> Result<(), String> {
    app.emit("machine-state", snapshot)
        .map_err(|error| error.to_string())
}
