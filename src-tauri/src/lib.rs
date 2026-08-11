mod commands;

use commands::{
    AppState, acknowledge_reset, active_transport, cancel_jog, confirm_soft_reset,
    connect_transport, controller_snapshot, disconnect, feed_hold, inspect_device, jog_pad_step,
    list_transports, mock_clear_alarm, mock_start_run, mock_trigger_alarm, mock_trigger_disconnect,
    mock_trigger_reset, mock_trigger_timeout, prepare_test_jog, refresh_status, request_soft_reset,
    step_jog,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            controller_snapshot,
            list_transports,
            active_transport,
            connect_transport,
            refresh_status,
            inspect_device,
            feed_hold,
            request_soft_reset,
            confirm_soft_reset,
            prepare_test_jog,
            step_jog,
            jog_pad_step,
            cancel_jog,
            disconnect,
            acknowledge_reset,
            mock_trigger_reset,
            mock_start_run,
            mock_trigger_alarm,
            mock_clear_alarm,
            mock_trigger_timeout,
            mock_trigger_disconnect
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Millo");
}
