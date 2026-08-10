mod commands;

use commands::{AppState, connect_mock, controller_snapshot, disconnect, refresh_status};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            controller_snapshot,
            connect_mock,
            refresh_status,
            disconnect
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Gantryon");
}
