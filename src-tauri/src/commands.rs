use std::sync::Arc;

use gantryon_controller::Controller;
use gantryon_domain::ControllerSnapshot;
use gantryon_mock::{MockControl, MockTransport};
use tauri::{AppHandle, Emitter, State};
use tokio::{
    sync::Mutex,
    task::JoinHandle,
    time::{MissedTickBehavior, interval},
};

pub struct AppState {
    controller: Arc<Mutex<Controller<MockTransport>>>,
    mock: MockControl,
    poll_task: Mutex<Option<JoinHandle<()>>>,
}

impl Default for AppState {
    fn default() -> Self {
        let transport = MockTransport::default();
        let mock = transport.control();
        Self {
            controller: Arc::new(Mutex::new(Controller::new(transport))),
            mock,
            poll_task: Mutex::new(None),
        }
    }
}

impl AppState {
    async fn start_polling(&self, app: AppHandle) {
        let mut poll_task = self.poll_task.lock().await;
        if poll_task.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }

        let controller = Arc::clone(&self.controller);
        let poll_interval = controller.lock().await.poll_interval();
        *poll_task = Some(tokio::spawn(async move {
            let mut ticker = interval(poll_interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
            ticker.tick().await;

            loop {
                ticker.tick().await;
                let snapshot = {
                    let mut controller = controller.lock().await;
                    let _ = controller.lifecycle_tick().await;
                    controller.snapshot()
                };
                let _ = app.emit("machine-state", snapshot);
            }
        }));
    }

    async fn stop_polling(&self) {
        if let Some(task) = self.poll_task.lock().await.take() {
            task.abort();
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
    state.stop_polling().await;
    let (sync_result, snapshot) = {
        let mut controller = state.controller.lock().await;
        controller
            .connect()
            .await
            .map_err(|error| error.to_string())?;
        let result = controller.refresh_status().await;
        (result, controller.snapshot())
    };

    publish_snapshot(&app, &snapshot)?;
    state.start_polling(app).await;
    sync_result.map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn refresh_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ControllerSnapshot, String> {
    let (result, snapshot) = {
        let mut controller = state.controller.lock().await;
        let result = controller.refresh_status().await;
        (result, controller.snapshot())
    };

    publish_snapshot(&app, &snapshot)?;
    result.map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn disconnect(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ControllerSnapshot, String> {
    state.stop_polling().await;
    let snapshot = state
        .controller
        .lock()
        .await
        .disconnect()
        .await
        .map_err(|error| error.to_string())?;

    publish_snapshot(&app, &snapshot)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn acknowledge_reset(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ControllerSnapshot, String> {
    let snapshot = state.controller.lock().await.acknowledge_reset();
    publish_snapshot(&app, &snapshot)?;
    Ok(snapshot)
}

#[tauri::command]
pub fn mock_trigger_reset(state: State<'_, AppState>) {
    state.mock.queue_reset("1.1h");
}

#[tauri::command]
pub fn mock_trigger_alarm(code: u16, state: State<'_, AppState>) {
    state.mock.queue_alarm(code);
}

#[tauri::command]
pub fn mock_clear_alarm(state: State<'_, AppState>) {
    state.mock.clear_alarm();
}

#[tauri::command]
pub fn mock_trigger_timeout(state: State<'_, AppState>) {
    state.mock.queue_stall();
    state.mock.queue_stall();
}

#[tauri::command]
pub fn mock_trigger_disconnect(state: State<'_, AppState>) {
    state.mock.queue_disconnect();
}

fn publish_snapshot(app: &AppHandle, snapshot: &ControllerSnapshot) -> Result<(), String> {
    app.emit("machine-state", snapshot)
        .map_err(|error| error.to_string())
}
