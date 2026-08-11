mod commands;

use commands::{
    AppState, acknowledge_reset, active_transport, adjust_feed_override, adjust_spindle_override,
    authorize_first_cut, cancel_dry_run, cancel_jog, complete_tool_change, confirm_soft_reset,
    connect_transport, controller_settings, controller_snapshot, create_machine_profile,
    detect_machine_profile, disconnect, feed_hold, inspect_device, jog_pad_step, list_transports,
    machine_profiles, mock_clear_alarm, mock_start_run, mock_trigger_alarm,
    mock_trigger_disconnect, mock_trigger_reset, mock_trigger_timeout, parse_gcode_program,
    pause_dry_run, preflight_real_run, prepare_test_jog, refresh_status, request_soft_reset,
    resume_dry_run, resume_program_run, rollback_controller_setting, select_machine_profile,
    sender_run_history, sender_snapshot, set_rapid_override, set_work_zero, start_check_run,
    start_mock_dry_run, start_program_run, step_jog, update_controller_setting,
    update_machine_local_settings,
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
            update_machine_local_settings,
            select_machine_profile,
            detect_machine_profile,
            list_transports,
            active_transport,
            connect_transport,
            controller_settings,
            update_controller_setting,
            rollback_controller_setting,
            refresh_status,
            inspect_device,
            feed_hold,
            adjust_feed_override,
            set_rapid_override,
            adjust_spindle_override,
            request_soft_reset,
            confirm_soft_reset,
            prepare_test_jog,
            step_jog,
            jog_pad_step,
            set_work_zero,
            parse_gcode_program,
            preflight_real_run,
            authorize_first_cut,
            start_program_run,
            start_check_run,
            resume_program_run,
            complete_tool_change,
            sender_snapshot,
            sender_run_history,
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
