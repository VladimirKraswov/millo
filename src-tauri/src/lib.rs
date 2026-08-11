mod commands;

use commands::{
    AppState, acknowledge_reset, active_transport, cancel_dry_run, cancel_jog, confirm_soft_reset,
    connect_transport, controller_snapshot, create_machine_profile, detect_machine_profile,
    disconnect, feed_hold, inspect_device, jog_pad_step, list_transports, machine_profiles,
    mock_clear_alarm, mock_start_run, mock_trigger_alarm, mock_trigger_disconnect,
    mock_trigger_reset, mock_trigger_timeout, parse_gcode_program, pause_dry_run,
    preflight_real_run, prepare_test_jog, refresh_status, request_soft_reset, resume_dry_run,
    select_machine_profile, sender_snapshot, set_work_zero, start_mock_dry_run, step_jog,
};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let profile_path = app.path().app_config_dir()?.join("machine-profiles.json");
            app.manage(AppState::load(profile_path)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            controller_snapshot,
            machine_profiles,
            create_machine_profile,
            select_machine_profile,
            detect_machine_profile,
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
            set_work_zero,
            parse_gcode_program,
            preflight_real_run,
            sender_snapshot,
            start_mock_dry_run,
            pause_dry_run,
            resume_dry_run,
            cancel_dry_run,
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
