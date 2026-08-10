mod commands;

use commands::{
    AppState, acknowledge_reset, connect_mock, controller_snapshot, disconnect, mock_clear_alarm,
    mock_trigger_alarm, mock_trigger_disconnect, mock_trigger_reset, mock_trigger_timeout,
    refresh_status,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            controller_snapshot,
            connect_mock,
            refresh_status,
            disconnect,
            acknowledge_reset,
            mock_trigger_reset,
            mock_trigger_alarm,
            mock_clear_alarm,
            mock_trigger_timeout,
            mock_trigger_disconnect
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Millo");
}
