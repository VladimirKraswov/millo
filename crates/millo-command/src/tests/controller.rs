use super::*;

#[tokio::test]
async fn serializes_status_and_inspector_commands_through_one_worker() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    arbiter.refresh_status().await.unwrap();
    let inspection = arbiter.inspect_device().await.unwrap();

    assert_eq!(inspection.device.responses.len(), 4);
    assert!(inspection.readiness.test_jog_ready);
    assert_eq!(
        control.writes(),
        vec![
            b"?".to_vec(),
            b"$I\n".to_vec(),
            b"$$\n".to_vec(),
            b"$G\n".to_vec(),
            b"$#\n".to_vec(),
        ]
    );
    task.abort();
}

#[tokio::test]
async fn safe_operator_console_serializes_only_the_read_only_allowlist() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let status = arbiter
        .execute_operator_console("?".to_owned(), OperatorConsolePolicy::SafeOnly)
        .await
        .unwrap();
    assert_eq!(status.command, "?");
    assert!(status.lines[0].starts_with("<Idle|"));

    for command in ["$I", "$$", "$G", "$#"] {
        let exchange = arbiter
            .execute_operator_console(command.to_owned(), OperatorConsolePolicy::SafeOnly)
            .await
            .unwrap();
        assert_eq!(exchange.command, command);
        assert_eq!(exchange.completion, CommandCompletion::Ok);
    }

    let writes_before_rejection = control.writes();
    for rejected in ["G0 X10", "$100=1", "$X", "$H", "M3 S1000", "!"] {
        assert!(matches!(
            arbiter
                .execute_operator_console(rejected.to_owned(), OperatorConsolePolicy::SafeOnly)
                .await,
            Err(ArbiterError::OperatorConsoleCommandRejected)
        ));
    }
    assert_eq!(control.writes(), writes_before_rejection);
    task.abort();
}

#[tokio::test]
async fn safe_operator_console_blocks_line_queries_while_machine_is_running() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    control.set_status("<Run|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:100,0>");
    arbiter.refresh_status().await.unwrap();
    let writes_before_query = control.writes();

    assert!(matches!(
        arbiter
            .execute_operator_console("$I".to_owned(), OperatorConsolePolicy::SafeOnly)
            .await,
        Err(ArbiterError::OperatorConsoleQueryUnavailable(
            MachineMode::Run
        ))
    ));
    assert_eq!(control.writes(), writes_before_query);
    task.abort();
}

#[tokio::test]
async fn expert_operator_console_serializes_one_arbitrary_line_through_the_actor() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let exchange = arbiter
        .execute_operator_console("G0 X1.25".to_owned(), OperatorConsolePolicy::Expert)
        .await
        .unwrap();

    assert_eq!(exchange.kind, millo_domain::OperatorConsoleCommandKind::Raw);
    assert_eq!(exchange.command, "G0 X1.25");
    assert_eq!(exchange.completion, CommandCompletion::Ok);
    assert!(control.writes().contains(&b"G0 X1.25\n".to_vec()));
    task.abort();
}

#[tokio::test]
async fn changes_the_hardware_profile_only_while_disconnected() {
    let (arbiter, _, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    let mut profile = HardwareProfile::first_machine();
    profile.name = "Selected bench router".to_owned();
    profile.travel_mm = Some(millo_domain::MachineTravel {
        x: 300.0,
        y: 180.0,
        z: 45.0,
    });

    let selected = arbiter.set_hardware_profile(profile.clone()).await.unwrap();
    assert_eq!(selected, profile);

    arbiter.connect().await.unwrap();
    let error = arbiter
        .set_hardware_profile(HardwareProfile::first_machine())
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        ArbiterError::ProfileChangeUnavailable(ConnectionState::Connected)
    ));
    let inspection = arbiter.inspect_device().await.unwrap();
    assert_eq!(inspection.readiness.profile.name, "Selected bench router");
    assert_eq!(inspection.readiness.profile.travel_mm, profile.travel_mm);
    task.abort();
}

#[tokio::test]
async fn binds_an_identified_profile_while_an_idle_reset_banner_is_visible() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    control.queue_reset("1.1h");
    arbiter.refresh_status().await.unwrap();
    let mut profile = HardwareProfile::first_machine();
    profile.name = "Identified router".to_owned();

    arbiter.bind_hardware_profile(profile).await.unwrap();
    let inspection = arbiter.inspect_device().await.unwrap();

    assert_eq!(inspection.readiness.profile.name, "Identified router");
    assert!(arbiter.snapshot().reset_notice.is_some());
    task.abort();
}

#[tokio::test]
async fn profile_binding_is_local_context_and_does_not_write_during_run() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    control.set_status("<Run|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:10,0>");
    arbiter.refresh_status().await.unwrap();
    let writes_before_binding = control.writes();
    let mut profile = HardwareProfile::first_machine();
    profile.name = "Bound while externally running".to_owned();

    arbiter.bind_hardware_profile(profile).await.unwrap();

    assert_eq!(control.writes(), writes_before_binding);
    assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Run);
    task.abort();
}

#[tokio::test]
async fn writes_and_rereads_one_confirmed_controller_setting() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let update = arbiter
        .update_controller_setting(ControllerSettingEditRequest {
            key: "$120".to_owned(),
            value: "600".to_owned(),
            confirmed: true,
            expected_value: Some("50".to_owned()),
            expected_revision: Some(7),
        })
        .await
        .unwrap();

    assert_eq!(update.before_value, "50.000");
    assert_eq!(update.stored_value, "600");
    assert_eq!(
        control.writes(),
        vec![
            b"?".to_vec(),
            b"$I\n".to_vec(),
            b"$$\n".to_vec(),
            b"$G\n".to_vec(),
            b"$#\n".to_vec(),
            b"$120=600\n".to_vec(),
            b"?".to_vec(),
            b"$I\n".to_vec(),
            b"$$\n".to_vec(),
            b"$G\n".to_vec(),
            b"$#\n".to_vec(),
        ]
    );
    task.abort();
}

#[tokio::test]
async fn actor_owns_periodic_lifecycle_polling() {
    let (arbiter, _control, worker) = test_arbiter(Duration::from_millis(5));
    let task = tokio::spawn(worker);
    let mut snapshots = arbiter.subscribe();
    arbiter.connect().await.unwrap();

    tokio::time::timeout(Duration::from_millis(100), async {
        loop {
            snapshots.changed().await.unwrap();
            if snapshots.borrow().poll_sequence > 0 {
                break;
            }
        }
    })
    .await
    .unwrap();

    assert_eq!(arbiter.snapshot().connection, ConnectionState::Connected);
    task.abort();
}

#[tokio::test]
async fn realtime_and_line_requests_share_the_same_queue() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    arbiter
        .send_realtime(RealtimeCommand::FeedHold)
        .await
        .unwrap();
    arbiter.inspect_device().await.unwrap();

    assert_eq!(control.writes()[0], b"!".to_vec());
    assert_eq!(control.writes()[1], b"$I\n".to_vec());
    task.abort();
}

#[tokio::test]
async fn realtime_status_request_consumes_its_status_frame() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let snapshot = arbiter
        .send_realtime(RealtimeCommand::Status)
        .await
        .unwrap();

    assert_eq!(snapshot.poll_sequence, 1);
    assert_eq!(control.writes(), vec![b"?".to_vec()]);
    task.abort();
}

#[tokio::test]
async fn incomplete_operator_confirmation_does_not_touch_the_controller() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    let mut incomplete = operator_confirmation();
    incomplete.tool_clear = false;

    let error = arbiter.prepare_test_jog(incomplete).await.unwrap_err();

    assert!(matches!(
        error,
        ArbiterError::Safety(SafetyError::IncompleteOperatorConfirmation)
    ));
    assert!(control.writes().is_empty());
    task.abort();
}

#[tokio::test]
async fn soft_reset_requires_and_consumes_an_actor_challenge() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    let challenge = arbiter.request_soft_reset().await.unwrap();

    arbiter.confirm_soft_reset(challenge.id).await.unwrap();
    let reused = arbiter.confirm_soft_reset(challenge.id).await.unwrap_err();

    assert!(matches!(
        reused,
        ArbiterError::Safety(SafetyError::ResetChallengeMissing)
    ));
    assert_eq!(control.writes(), vec![b"\x18".to_vec()]);
    task.abort();
}

#[tokio::test]
async fn connected_actor_rejects_reconnect_and_transport_replacement() {
    let (arbiter, _, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X2 F20", false).await;

    let reconnect = arbiter.connect().await.unwrap_err();
    let replacement = arbiter
        .replace_transport(Box::new(MockTransport::default()))
        .await
        .unwrap_err();

    assert!(matches!(
        reconnect,
        ArbiterError::ConnectUnavailable(ConnectionState::Connected)
    ));
    assert!(matches!(
        replacement,
        ArbiterError::TransportReplacementUnavailable(ConnectionState::Connected)
    ));
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Running);
    task.abort();
}

#[tokio::test]
async fn alarm_unlock_requires_confirmation_and_verifies_idle_in_the_actor() {
    let transport =
        MockTransport::with_status("<Alarm|MPos:1.000,2.000,3.000|WPos:1.000,2.000,3.000|FS:0,0>");
    let control = transport.control();
    let (arbiter, worker) = CommandArbiter::new(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_secs(60),
            status_timeout: Duration::from_millis(20),
            command_timeout: Duration::from_millis(50),
            failures_before_recovery: 2,
        },
        HardwareProfile::first_machine(),
    );
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    assert!(matches!(
        arbiter.unlock_alarm(false).await.unwrap_err(),
        ArbiterError::UnlockConfirmationRequired
    ));
    assert!(control.writes().is_empty());

    let unlocked = arbiter.unlock_alarm(true).await.unwrap();
    assert_eq!(unlocked.machine.mode, MachineMode::Idle);
    assert!(unlocked.alarm.is_none());
    assert_eq!(
        control.writes(),
        vec![b"?".to_vec(), b"$X\n".to_vec(), b"?".to_vec()]
    );
    task.abort();
}

#[tokio::test]
async fn alarm_returns_a_fresh_blocked_report_without_authorization() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    control.queue_alarm(3);
    arbiter.refresh_status().await.unwrap();

    let preparation = arbiter
        .prepare_test_jog(operator_confirmation())
        .await
        .unwrap();

    assert!(preparation.authorization.is_none());
    assert!(!preparation.inspection.readiness.test_jog_ready);
    assert!(preparation.inspection.readiness.blocker_count > 0);
    task.abort();
}

#[tokio::test]
async fn controller_managed_spindle_and_coolant_are_modal_verified() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let mut profile = HardwareProfile::first_machine();
    profile.spindle_control = SpindleControl::Controller;
    profile.flood_coolant_control = true;
    let task = tokio::spawn(worker);
    arbiter.set_hardware_profile(profile).await.unwrap();
    arbiter.connect().await.unwrap();

    let spindle = arbiter
        .set_machine_output(MachineOutputRequest::SpindleOn {
            direction: millo_domain::SpindleDirection::Clockwise,
            speed_rpm: 1_000.0,
        })
        .await
        .unwrap();
    assert_eq!(spindle.commands, ["S1000", "M3"]);
    let coolant = arbiter
        .set_machine_output(MachineOutputRequest::FloodCoolant(true))
        .await
        .unwrap();
    assert_eq!(coolant.commands, ["M8"]);
    let stopped = arbiter
        .set_machine_output(MachineOutputRequest::AllOff)
        .await
        .unwrap();
    assert_eq!(stopped.commands, ["M5", "M9"]);
    assert!(control.writes().iter().any(|write| write == b"M3\n"));
    task.abort();
}

#[tokio::test]
async fn undeclared_coolant_output_is_rejected_before_transport_io() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let error = arbiter
        .set_machine_output(MachineOutputRequest::FloodCoolant(true))
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ArbiterError::CoolantOutputDisabled("flood")
    ));
    assert!(!control.writes().iter().any(|write| write == b"M8\n"));
    task.abort();
}

#[tokio::test]
async fn stops_configuration_after_the_first_rejected_setting() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    control.set_setting(21, "1");
    control.set_setting(22, "1");
    control.queue_setting_error(2);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();

    let error = arbiter.configure_unhomed_operation().await.unwrap_err();

    assert!(matches!(
        error,
        ArbiterError::Controller(ControllerError::CommandRejected { .. })
    ));
    let setting_writes = control
        .writes()
        .into_iter()
        .filter(|write| write.starts_with(b"$21=") || write.starts_with(b"$22="))
        .collect::<Vec<_>>();
    assert_eq!(setting_writes, vec![b"$21=0\n".to_vec()]);
    task.abort();
}

#[test]
fn motion_timeout_includes_travel_time_and_settle_margin() {
    assert_eq!(bounded_motion_timeout(50.0, 500.0), Duration::from_secs(9));
}

#[tokio::test]
async fn machine_run_preflight_performs_read_only_fresh_queries() {
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let report = arbiter
        .preflight_real_run(
            parsed_program("G21 G90 G94\nG0 Z2\nG1 X2 F20\nM5"),
            ProgramRunIntent::AirRun,
        )
        .await
        .unwrap();

    assert!(report.ready);
    assert_eq!(report.poll_sequence, 2);
    assert_eq!(
        control.writes(),
        vec![
            b"?".to_vec(),
            b"$I\n".to_vec(),
            b"$$\n".to_vec(),
            b"$G\n".to_vec(),
            b"$#\n".to_vec(),
            b"?".to_vec(),
        ]
    );
    task.abort();
}

#[tokio::test]
async fn serial_tool_change_is_host_managed_verified_and_cannot_be_plain_resumed() {
    let source = "G21 G90 G94\nG1 X1 F20\nT2 M6\nG1 X2 F20\nM30";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    wait_for_sender(&arbiter, SenderState::Draining).await;
    control.set_status("<Run|MPos:0,0,0|FS:20,0>");
    arbiter.refresh_status().await.unwrap();
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Draining);
    assert!(
        arbiter
            .complete_tool_change(tool_change_confirmation(3, Some(2)))
            .await
            .is_err()
    );
    control.set_status("<Idle|MPos:1,0,0|FS:0,0>");
    arbiter.refresh_status().await.unwrap();
    let barrier = wait_for_sender(&arbiter, SenderState::ToolChange).await;
    assert_eq!(barrier.current_source_line, Some(3));
    assert_eq!(barrier.requested_tool, Some(2));
    assert!(control.writes().contains(&b"N3 T2\n".to_vec()));
    assert!(!control.writes().contains(&b"N3 T2 M6\n".to_vec()));

    let writes_before_resume = control.writes();
    assert!(matches!(
        arbiter.inspect_device().await,
        Err(ArbiterError::MachineOperationBusy)
    ));
    assert!(matches!(
        arbiter.resume_program_run().await.unwrap_err(),
        ArbiterError::Sender(SenderError::InvalidTransition {
            action: "resume",
            state: SenderState::ToolChange,
        })
    ));
    assert_eq!(control.writes(), writes_before_resume);

    let mut incomplete = tool_change_confirmation(3, Some(2));
    incomplete.z_zero_verified = false;
    assert!(matches!(
        arbiter.complete_tool_change(incomplete).await.unwrap_err(),
        ArbiterError::ToolChangeConfirmationIncomplete(_)
    ));
    assert!(matches!(
        arbiter
            .complete_tool_change(tool_change_confirmation(4, Some(2)))
            .await
            .unwrap_err(),
        ArbiterError::ToolChangeMismatch
    ));

    arbiter
        .set_work_zero(work_zero_request(WorkAxis::Z, true))
        .await
        .unwrap();
    assert_eq!(arbiter.sender_snapshot().state, SenderState::ToolChange);

    let resumed = arbiter
        .complete_tool_change(tool_change_confirmation(3, Some(2)))
        .await
        .unwrap();
    assert_eq!(resumed.state, SenderState::Running);
    let draining = wait_for_sender(&arbiter, SenderState::Draining).await;
    assert_eq!(draining.current_command.as_deref(), Some("M30"));
    arbiter.refresh_status().await.unwrap();
    let completed = wait_for_sender(&arbiter, SenderState::Completed).await;
    assert_eq!(completed.acknowledged_lines, completed.total_lines);
    assert!(control.writes().contains(&b"N4 G1 X2 F20\n".to_vec()));
    assert!(!control.writes().iter().any(|write| {
        String::from_utf8_lossy(write)
            .split_whitespace()
            .any(|word| word == "M6")
    }));
    task.abort();
}

#[tokio::test]
async fn prepared_physical_run_dispatches_only_after_matching_commit() {
    let source = "G21 G90 G94\nG0 Z2\nG1 X2 F20";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    let (_, prepared) = authorize_and_start_serial_fixture(&arbiter, source, false).await;

    tokio::task::yield_now().await;
    assert_eq!(prepared.state, SenderState::Running);
    assert!(!control.writes().iter().any(|write| write.starts_with(b"N")));
    assert!(matches!(
        arbiter
            .commit_prepared_program_run(prepared.run_sequence + 1)
            .await,
        Err(ArbiterError::PreparedRunMismatch { .. })
    ));
    assert!(!control.writes().iter().any(|write| write.starts_with(b"N")));

    let discarded = arbiter
        .discard_prepared_program_run(prepared.run_sequence)
        .await
        .unwrap();
    assert_eq!(discarded.state, SenderState::Cancelled);
    assert!(!control.writes().iter().any(|write| write.starts_with(b"N")));
    task.abort();
}

#[tokio::test]
async fn dry_run_is_rejected_when_the_actor_target_is_not_mock() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();

    let error = arbiter
        .start_dry_run(dry_run_plan("G0 X1"))
        .await
        .unwrap_err();

    assert!(matches!(error, ArbiterError::DryRunTransportUnavailable));
    assert_eq!(control.writes(), vec![b"?".to_vec()]);
    task.abort();
}
