use super::*;

struct FifoTransport {
    reads: std::collections::VecDeque<String>,
    writes: std::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
}

type IoFuture<'a, T> =
    std::pin::Pin<Box<dyn Future<Output = Result<T, TransportError>> + Send + 'a>>;

impl millo_transport::Transport for FifoTransport {
    fn connect<'a, 'b>(&'a mut self) -> IoFuture<'b, ()>
    where
        'a: 'b,
        Self: 'b,
    {
        Box::pin(async { Ok(()) })
    }
    fn disconnect<'a, 'b>(&'a mut self) -> IoFuture<'b, ()>
    where
        'a: 'b,
        Self: 'b,
    {
        Box::pin(async { Ok(()) })
    }
    fn write<'a, 'd, 'b>(&'a mut self, data: &'d [u8]) -> IoFuture<'b, ()>
    where
        'a: 'b,
        'd: 'b,
        Self: 'b,
    {
        Box::pin(async move {
            self.writes.lock().unwrap().push(data.to_vec());
            Ok(())
        })
    }
    fn read_line<'a, 'b>(&'a mut self) -> IoFuture<'b, String>
    where
        'a: 'b,
        Self: 'b,
    {
        Box::pin(async move {
            match self.reads.pop_front() {
                Some(line) => Ok(line),
                None => std::future::pending().await,
            }
        })
    }
    fn is_connected(&self) -> bool {
        true
    }
}

#[test]
fn inspector_admission_covers_ready_running_paused_draining_and_tool_change() {
    let mut sender = Sender::default();
    sender
        .load_cut_run(
            build_program_run_plan(
                &parsed_program("G21 G90 G94\nG1 X1 F10\nT2 M6\nG1 X2 F10"),
                ProgramRunPolicy::Cutting,
            )
            .unwrap(),
        )
        .unwrap();
    let (response, _) = oneshot::channel();
    let request = Request::InspectDevice { response };
    assert!(request_conflicts_with_sender(&sender, &request));
    sender.start().unwrap();
    assert!(request_conflicts_with_sender(&sender, &request));
    sender.pause().unwrap();
    assert!(request_conflicts_with_sender(&sender, &request));
    sender.resume().unwrap();
    while sender.next_line().is_some() {
        sender.acknowledge_ok().unwrap();
    }
    assert_eq!(sender.snapshot().state, SenderState::Draining);
    assert!(request_conflicts_with_sender(&sender, &request));
    sender.complete_draining().unwrap();
    assert_eq!(sender.snapshot().state, SenderState::ToolChange);
    assert!(request_conflicts_with_sender(&sender, &request));
    sender.cancel().unwrap();
    assert!(!request_conflicts_with_sender(&sender, &request));
}

#[tokio::test]
async fn ordinary_readers_reject_an_active_sender_even_with_a_cached_idle_status() {
    let source = "G21 G90 G94\nG1 X1 F10";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, source, false).await;
    assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);
    let before = control.writes();

    assert!(matches!(
        arbiter.inspect_device().await,
        Err(ArbiterError::MachineOperationBusy)
    ));
    assert!(matches!(
        arbiter.configure_unhomed_operation().await,
        Err(ArbiterError::MachineOperationBusy)
    ));
    assert!(matches!(
        arbiter.unlock_alarm(true).await,
        Err(ArbiterError::MachineOperationBusy)
    ));
    assert!(matches!(
        arbiter
            .preflight_real_run(parsed_program(source), ProgramRunIntent::AirRun)
            .await,
        Err(ArbiterError::MachineOperationBusy)
    ));
    assert!(matches!(
        arbiter
            .authorize_first_cut_fixture(parsed_program(source), first_cut_confirmation())
            .await,
        Err(ArbiterError::MachineOperationBusy)
    ));
    assert!(matches!(
        arbiter
            .update_controller_setting(ControllerSettingEditRequest {
                key: "$120".to_owned(),
                value: "600".to_owned(),
                confirmed: true,
                expected_value: Some("50".to_owned()),
                expected_revision: Some(7),
            })
            .await,
        Err(ArbiterError::MachineOperationBusy)
    ));

    assert_eq!(control.writes(), before);
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Running);
    arbiter.abort_program_run().await.unwrap();
    arbiter.refresh_status().await.unwrap();
    arbiter.acknowledge_reset().await.unwrap();
    assert_eq!(
        arbiter
            .inspect_device()
            .await
            .unwrap()
            .device
            .responses
            .len(),
        4
    );
    task.abort();
}

#[tokio::test]
async fn ordinary_readers_cannot_steal_a_paused_senders_acknowledgements() {
    let (arbiter, control, worker) = serial_preflight_arbiter_for_realtime_preemption();
    control.queue_program_delay(40);
    control.queue_program_error(20);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X1 F10", true).await;
    let mut snapshots = arbiter.subscribe_sender();
    tokio::time::timeout(Duration::from_secs(2), async {
        while snapshots.borrow().in_flight_lines == 0 {
            snapshots.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    arbiter.feed_hold().await.unwrap();
    assert!(arbiter.sender_snapshot().in_flight_lines > 0);
    let before = control.writes();

    assert!(matches!(
        arbiter.inspect_device().await,
        Err(ArbiterError::MachineOperationBusy)
    ));
    assert!(matches!(
        arbiter.prepare_test_jog(operator_confirmation()).await,
        Err(ArbiterError::MachineOperationBusy)
    ));
    assert!(matches!(
        arbiter
            .jog_pad_step(JogPadStepRequest {
                confirmation: operator_confirmation(),
                axis: millo_domain::JogAxis::Z,
                distance_mm: 0.01,
                feed_mm_per_min: 100.0,
            })
            .await,
        Err(ArbiterError::MachineOperationBusy)
    ));
    assert!(matches!(
        arbiter
            .step_jog(StepJogRequest {
                authorization_id: 1,
                axis: millo_domain::JogAxis::Z,
                distance_mm: 0.01,
                feed_mm_per_min: 100.0,
            })
            .await,
        Err(ArbiterError::MachineOperationBusy)
    ));
    assert!(matches!(
        arbiter
            .set_work_zero(work_zero_request(WorkAxis::Z, true))
            .await,
        Err(ArbiterError::MachineOperationBusy)
    ));
    assert!(matches!(
        arbiter
            .return_to_work_zero(return_to_zero_request(WorkAxis::Z))
            .await,
        Err(ArbiterError::MachineOperationBusy)
    ));
    assert_eq!(control.writes(), before);

    let failed = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = snapshots.borrow_and_update().clone();
            if snapshot.state == SenderState::Failed {
                return snapshot;
            }
            snapshots.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert_eq!(failed.acknowledged_lines, 1);
    let failure = failed.failure.unwrap();
    assert_eq!(failure.command.as_deref(), Some("M9"));
    assert_eq!(failure.grbl_code, Some(20));
    task.abort();
}

#[tokio::test]
async fn resume_accounts_for_acknowledgements_before_the_hold_status() {
    let transport = FifoTransport {
        reads: ["ok", "<Hold:0|MPos:0,0,0|FS:0,0>"]
            .map(str::to_owned)
            .into(),
        writes: Default::default(),
    };
    let writes = transport.writes.clone();
    let mut controller: Controller<BoxedTransport> = Controller::new(Box::new(transport));
    controller.connect().await.unwrap();
    let mut sender = Sender::default();
    sender
        .load_cut_run(
            build_program_run_plan(&parsed_program("G1 X1 F10"), ProgramRunPolicy::Cutting)
                .unwrap(),
        )
        .unwrap();
    sender.start().unwrap();
    let line = sender.next_line().unwrap();
    controller.write_program_line(&line).await.unwrap();
    sender.pause().unwrap();

    execute_program_run_resume(&mut controller, &mut sender)
        .await
        .unwrap();

    assert_eq!(sender.snapshot().acknowledged_lines, 1);
    assert!(!sender.has_in_flight());
    assert_eq!(sender.snapshot().state, SenderState::Running);
    assert_eq!(writes.lock().unwrap().last(), Some(&b"~".to_vec()));
}

#[tokio::test]
async fn resume_rejects_a_reset_banner_even_when_followed_by_idle() {
    let transport = FifoTransport {
        reads: ["Grbl 1.1h ['$' for help]", "<Idle|MPos:0,0,0|FS:0,0>"]
            .map(str::to_owned)
            .into(),
        writes: Default::default(),
    };
    let writes = transport.writes.clone();
    let mut controller: Controller<BoxedTransport> = Controller::new(Box::new(transport));
    controller.connect().await.unwrap();
    let mut sender = Sender::default();
    sender
        .load_cut_run(
            build_program_run_plan(&parsed_program("G1 X1 F10"), ProgramRunPolicy::Cutting)
                .unwrap(),
        )
        .unwrap();
    sender.start().unwrap();
    sender.pause().unwrap();

    assert!(
        execute_program_run_resume(&mut controller, &mut sender)
            .await
            .is_err()
    );
    assert_eq!(sender.snapshot().state, SenderState::Failed);
    assert!(!writes.lock().unwrap().contains(&b"~".to_vec()));
}

#[tokio::test]
async fn resume_aborts_on_a_correlated_error_before_the_hold_status() {
    let transport = FifoTransport {
        reads: ["ok", "error:20", "<Hold:0|MPos:0,0,0|FS:0,0>"]
            .map(str::to_owned)
            .into(),
        writes: Default::default(),
    };
    let writes = transport.writes.clone();
    let mut controller: Controller<BoxedTransport> = Controller::new(Box::new(transport));
    controller.connect().await.unwrap();
    let mut sender = Sender::default();
    sender
        .load_cut_run(
            build_program_run_plan(&parsed_program("G1 X1 F10"), ProgramRunPolicy::Cutting)
                .unwrap(),
        )
        .unwrap();
    sender.start().unwrap();
    for _ in 0..2 {
        controller
            .write_program_line(&sender.next_line().unwrap())
            .await
            .unwrap();
    }
    sender.pause().unwrap();

    assert!(
        execute_program_run_resume(&mut controller, &mut sender)
            .await
            .is_err()
    );

    let failure = sender.snapshot().failure.unwrap();
    assert_eq!(failure.kind, SenderFailureKind::GrblError);
    assert_eq!(failure.command.as_deref(), Some("M9"));
    assert_eq!(sender.snapshot().acknowledged_lines, 1);
    let writes = writes.lock().unwrap();
    assert!(!writes.contains(&b"~".to_vec()));
    assert_eq!(
        &writes[writes.len() - 2..],
        &[b"!".to_vec(), b"\x18".to_vec()]
    );
}

#[tokio::test]
async fn disconnect_aborts_a_physical_stream_before_closing_the_link() {
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X1 F10", false).await;
    let before = control.writes().len();

    arbiter.disconnect().await.unwrap();

    assert_eq!(
        &control.writes()[before..],
        &[b"!".to_vec(), b"\x18".to_vec()]
    );
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Cancelled);
    assert_eq!(arbiter.snapshot().connection, ConnectionState::Disconnected);
    task.abort();
}

#[tokio::test]
async fn typed_pause_delivery_failure_quarantines_the_sender() {
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X1 F10", false).await;
    control.drop_link();

    assert!(arbiter.pause_program_run().await.is_err());
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Failed);
    assert_eq!(arbiter.snapshot().connection, ConnectionState::Disconnected);
    task.abort();
}

#[tokio::test]
async fn externally_observed_hold_preserves_the_draining_phase() {
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X1 F10", true).await;
    wait_for_sender(&arbiter, SenderState::Draining).await;
    control.set_status("<Hold:0|MPos:0,0,0|FS:0,0>");

    arbiter.refresh_status().await.unwrap();

    assert_eq!(arbiter.sender_snapshot().state, SenderState::Paused);
    assert_eq!(
        arbiter.resume_program_run().await.unwrap().state,
        SenderState::Draining
    );
    task.abort();
}

#[tokio::test]
async fn door_status_pauses_dispatch_but_does_not_discard_pending_acknowledgements() {
    let transport = FifoTransport {
        reads: ["<Idle|MPos:0,0,0>", "<Door:0|MPos:0,0,0>", "ok"]
            .map(str::to_owned)
            .into(),
        writes: Default::default(),
    };
    let mut controller: Controller<BoxedTransport> = Controller::new(Box::new(transport));
    controller.connect().await.unwrap();
    controller.refresh_status().await.unwrap();
    let mut sender = Sender::default();
    sender
        .load_cut_run(
            build_program_run_plan(&parsed_program("G1 X1 F10"), ProgramRunPolicy::Cutting)
                .unwrap(),
        )
        .unwrap();
    sender.start().unwrap();
    let (snapshots, _) = watch::channel(controller.snapshot());
    let (sender_snapshots, _) = watch::channel(sender.snapshot());
    let mut program_check = ProgramCheckGate::default();
    let mut pending_check = None;

    for _ in 0..2 {
        execute_sender_step(
            &mut controller,
            &mut sender,
            &mut program_check,
            &mut pending_check,
            &snapshots,
            &sender_snapshots,
        )
        .await;
        assert_eq!(sender.snapshot().state, SenderState::Paused);
    }
    assert_eq!(sender.snapshot().acknowledged_lines, 1);
    assert!(sender.has_in_flight());
}

#[tokio::test]
async fn disconnect_reports_stop_delivery_failure_and_still_closes_the_link() {
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X1 F10", false).await;
    control.drop_link();

    assert!(arbiter.disconnect().await.is_err());
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Failed);
    assert_eq!(arbiter.snapshot().connection, ConnectionState::Disconnected);
    task.abort();
}

#[tokio::test]
async fn realtime_status_reconciles_reset_before_it_can_be_acknowledged_away() {
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X1 F10", true).await;
    wait_for_sender(&arbiter, SenderState::Draining).await;
    control.queue_reset("1.1h");

    arbiter
        .send_realtime(RealtimeCommand::Status)
        .await
        .unwrap();
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Failed);
    arbiter.acknowledge_reset().await.unwrap();
    arbiter.refresh_status().await.unwrap();
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Failed);
    task.abort();
}

#[tokio::test]
async fn invalid_soft_reset_confirmation_cannot_cancel_an_active_sender() {
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X2 F20", false).await;

    let error = arbiter.confirm_soft_reset(u64::MAX).await.unwrap_err();

    assert!(matches!(
        error,
        ArbiterError::Safety(SafetyError::ResetChallengeMissing)
    ));
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Running);
    assert!(!control.writes().iter().any(|write| write == b"\x18"));
    task.abort();
}

#[tokio::test]
async fn mock_pause_and_resume_cannot_change_a_physical_sender() {
    let (arbiter, _, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X2 F20", false).await;

    assert!(matches!(
        arbiter.pause_dry_run().await.unwrap_err(),
        ArbiterError::DryRunTransportUnavailable
    ));
    assert!(matches!(
        arbiter.resume_dry_run().await.unwrap_err(),
        ArbiterError::DryRunTransportUnavailable
    ));
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Running);
    task.abort();
}

#[tokio::test]
async fn real_run_preflight_rejects_a_disabled_execution_target_before_controller_io() {
    let (arbiter, control, worker) = disabled_execution_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let error = arbiter
        .preflight_real_run(
            parsed_program("G21 G90\nG1 X1 F10"),
            ProgramRunIntent::AirRun,
        )
        .await
        .unwrap_err();

    assert!(matches!(error, ArbiterError::RealRunTransportUnavailable));
    assert!(control.writes().is_empty());
    task.abort();
}

#[tokio::test]
async fn unsafe_real_run_preflight_never_dispatches_a_program_line() {
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let report = arbiter
        .preflight_real_run(
            parsed_program("G21 G90 G94\nM3 S1000\nG1 X1 F10"),
            ProgramRunIntent::AirRun,
        )
        .await
        .unwrap();

    assert!(!report.ready);
    assert_eq!(report.program_blockers[0].source_line, Some(2));
    assert!(control.writes().iter().all(|write| matches!(
        write.as_slice(),
        b"?" | b"$I\n" | b"$$\n" | b"$G\n" | b"$#\n"
    )));
    task.abort();
}

#[tokio::test]
async fn cutting_preflight_accepts_spindle_syntax_but_requires_a_check_certificate() {
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let report = arbiter
        .preflight_real_run(
            parsed_program("G21 G90 G94\nM3 S1000\nG1 X1 F10\nM5"),
            ProgramRunIntent::Cutting,
        )
        .await
        .unwrap();

    assert!(!report.ready);
    assert_eq!(report.intent, ProgramRunIntent::Cutting);
    assert!(report.checks.iter().any(|check| {
        check.id == "grbl-check-certificate" && check.level == RunPreflightLevel::Blocker
    }));
    assert!(control.writes().iter().all(|write| matches!(
        write.as_slice(),
        b"?" | b"$I\n" | b"$$\n" | b"$G\n" | b"$#\n"
    )));
    task.abort();
}

#[tokio::test]
async fn safe_selected_line_air_run_also_requires_its_exact_check_certificate() {
    let source =
        "(Millo safe start from L42 of original.nc)\nG21 G90 G94 G17\nG0 Z5\nG1 X1 F10\nM5";
    let (arbiter, _control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let blocked = arbiter
        .preflight_real_run(safe_start_program(source), ProgramRunIntent::AirRun)
        .await
        .unwrap();
    assert!(!blocked.ready);
    assert!(blocked.checks.iter().any(|check| {
        check.id == "grbl-check-certificate" && check.level == RunPreflightLevel::Blocker
    }));

    arbiter
        .start_check_run(safe_start_program(source))
        .await
        .unwrap();
    wait_for_sender(&arbiter, SenderState::Completed).await;
    let certified = arbiter
        .preflight_real_run(safe_start_program(source), ProgramRunIntent::AirRun)
        .await
        .unwrap();
    assert!(certified.ready);
    assert!(certified.checks.iter().any(|check| {
        check.id == "grbl-check-certificate" && check.level == RunPreflightLevel::Pass
    }));
    task.abort();
}

#[tokio::test]
async fn first_cut_authorization_repeats_preflight_and_emits_no_motion() {
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let preparation = arbiter
        .authorize_first_cut_fixture(
            parsed_program("G21 G90 G94\nG0 Z2\nG1 X2 F20\nM5"),
            first_cut_confirmation(),
        )
        .await
        .unwrap();

    assert!(preparation.report.ready);
    assert_eq!(
        preparation.authorization.program_fingerprint,
        preparation.report.program_fingerprint
    );
    assert_eq!(preparation.authorization.poll_sequence, 2);
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
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Idle);
    task.abort();
}

#[tokio::test]
async fn completed_check_certifies_the_exact_cutting_program_and_options() {
    let source = "G21 G90 G94\nM3 S1000\nG1 X1 F10\nM5";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    arbiter
        .start_check_run(parsed_program(source))
        .await
        .unwrap();
    let checked = wait_for_sender(&arbiter, SenderState::Completed).await;
    assert_eq!(checked.mode, Some(millo_sender::SenderMode::CheckRun));
    assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);

    let report = arbiter
        .preflight_real_run(parsed_program(source), ProgramRunIntent::Cutting)
        .await
        .unwrap();
    assert!(report.ready);
    assert!(report.checks.iter().any(|check| {
        check.id == "grbl-check-certificate" && check.level == RunPreflightLevel::Pass
    }));

    let changed_options = arbiter
        .preflight_real_run_with_options(
            parsed_program(source),
            ProgramRunIntent::Cutting,
            ProgramExecutionOptions {
                optional_stop: true,
                block_delete: false,
                ..ProgramExecutionOptions::default()
            },
        )
        .await
        .unwrap();
    assert!(!changed_options.ready);
    assert!(changed_options.checks.iter().any(|check| {
        check.id == "grbl-check-certificate" && check.detail.contains("options changed")
    }));
    assert_eq!(
        control
            .writes()
            .iter()
            .filter(|write| write.as_slice() == b"$C\n")
            .count(),
        2
    );
    task.abort();
}

#[tokio::test]
async fn cancelled_check_exits_check_mode_without_issuing_a_certificate() {
    let source = "G21 G90 G94\nM3 S1000\nG1 X1 F10\nM5";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    control.queue_program_stall();

    let started = arbiter
        .start_check_run(parsed_program(source))
        .await
        .unwrap();
    assert_eq!(started.state, SenderState::Running);

    let cancelled = arbiter.cancel_dry_run().await.unwrap();
    assert_eq!(cancelled.state, SenderState::Cancelled);
    assert_eq!(cancelled.mode, Some(millo_sender::SenderMode::CheckRun));
    assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);

    let report = arbiter
        .preflight_real_run(parsed_program(source), ProgramRunIntent::Cutting)
        .await
        .unwrap();
    assert!(!report.ready);
    assert!(report.checks.iter().any(|check| {
        check.id == "grbl-check-certificate" && check.level == RunPreflightLevel::Blocker
    }));
    assert_eq!(
        control
            .writes()
            .iter()
            .filter(|write| write.as_slice() == b"$C\n")
            .count(),
        2
    );
    task.abort();
}

#[tokio::test]
async fn incomplete_first_cut_confirmation_fails_before_controller_io() {
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    let mut confirmation = first_cut_confirmation();
    confirmation.stock_secured = false;

    let error = arbiter
        .authorize_first_cut(parsed_program("G21 G90 G94\nG1 X2 F20"), confirmation)
        .await
        .unwrap_err();

    assert!(matches!(
        error,
        ArbiterError::FirstCut(FirstCutAuthorizationError::IncompleteConfirmation { .. })
    ));
    assert!(control.writes().is_empty());
    task.abort();
}

#[tokio::test]
async fn serial_fixture_consumes_one_lease_and_completes_only_after_every_ok() {
    let source = "G21 G90 G94\nG0 Z2\nG1 X2 F20\nM5";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    control.set_firmware_options("V,15,256");
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let (preparation, started) = authorize_and_start_serial_fixture(&arbiter, source, true).await;
    let draining = wait_for_sender(&arbiter, SenderState::Draining).await;
    assert_eq!(draining.acknowledged_lines, draining.total_lines);
    arbiter.refresh_status().await.unwrap();
    let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

    assert_eq!(started.mode, Some(millo_sender::SenderMode::CutRun));
    assert_eq!(started.rx_buffer_capacity, 255);
    assert_eq!(completed.acknowledged_lines, completed.total_lines);
    assert_eq!(completed.progress, 1.0);
    let writes_before_reuse = control.writes();
    let reuse = arbiter
        .start_serial_run_fixture(parsed_program(source), preparation.authorization.id, true)
        .await
        .unwrap_err();
    assert!(matches!(
        reuse,
        ArbiterError::FirstCut(FirstCutAuthorizationError::AuthorizationMissing)
    ));
    assert_eq!(control.writes().len(), writes_before_reuse.len() + 1);
    assert_eq!(control.writes().last(), Some(&b"?".to_vec()));
    task.abort();
}

#[tokio::test]
async fn production_air_run_executes_the_authorized_file_and_rejects_plain_cancel() {
    let source = include_str!("../../../../fixtures/programs/air-square-20mm.nc");
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    let preparation = arbiter
        .authorize_first_cut(parsed_program(source), air_run_confirmation())
        .await
        .unwrap();

    let started = arbiter
        .start_program_run(parsed_program(source), preparation.authorization.id)
        .await
        .unwrap();
    assert_eq!(started.mode, Some(millo_sender::SenderMode::AirRun));
    wait_for_sender(&arbiter, SenderState::Draining).await;
    assert!(matches!(
        arbiter.cancel_dry_run().await.unwrap_err(),
        ArbiterError::ProgramRunStopRequiresReset
    ));

    control.set_status("<Idle|MPos:2.000,0.000,0.000|WPos:2.000,0.000,0.000|FS:0,0>");
    arbiter.refresh_status().await.unwrap();
    wait_for_sender(&arbiter, SenderState::Completed).await;
    assert!(control.writes().iter().all(|write| {
        String::from_utf8_lossy(write)
            .split_whitespace()
            .all(|word| word != "M3" && word != "M4" && !word.starts_with('S'))
    }));
    task.abort();
}

#[tokio::test]
async fn check_run_validates_tool_number_and_skips_the_host_only_m6_barrier() {
    let source = "G21 G90 G94\nT5 M6\nG1 X1 F20\nM30";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    arbiter
        .start_check_run(parsed_program(source))
        .await
        .unwrap();
    let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

    assert_eq!(completed.acknowledged_lines, completed.total_lines);
    assert!(control.writes().contains(&b"N2 T5\n".to_vec()));
    assert!(!control.writes().contains(&b"N4 M30\n".to_vec()));
    assert!(!control.writes().iter().any(|write| {
        String::from_utf8_lossy(write)
            .split_whitespace()
            .any(|word| word == "M6")
    }));
    task.abort();
}

#[tokio::test]
async fn serial_check_run_validates_complex_geometry_and_returns_to_idle() {
    let source = include_str!("../../../../fixtures/programs/grbl-complex-check.nc");
    let program = parsed_program(source);
    let plan = build_program_run_plan(&program, ProgramRunPolicy::Cutting).unwrap();
    let expected_commands = plan
        .lines()
        .iter()
        .filter(|line| line.kind() != DryRunLineKind::ProgramEnd)
        .map(|line| format!("{}\n", line.wire_command()).into_bytes())
        .collect::<Vec<_>>();
    let (arbiter, control, worker) = serial_preflight_arbiter();
    control.set_firmware_options("V,35,254");
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let started = arbiter.start_check_run(program).await.unwrap();
    assert_eq!(started.mode, Some(millo_sender::SenderMode::CheckRun));
    assert_eq!(started.rx_buffer_capacity, 253);
    let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

    assert_eq!(completed.acknowledged_lines, completed.total_lines);
    assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);
    let writes = control.writes();
    assert_eq!(
        writes
            .iter()
            .filter(|write| write.as_slice() == b"$C\n")
            .count(),
        2
    );
    let actual_commands = writes
        .iter()
        .filter(|write| {
            write.ends_with(b"\n") && !write.starts_with(b"$") && write.as_slice() != b"$C\n"
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(actual_commands, expected_commands);
    assert!(!writes.contains(&b"!".to_vec()));
    assert!(!writes.contains(&b"\x18".to_vec()));
    task.abort();
}

#[tokio::test]
async fn serial_check_run_accepts_cutting_spindle_syntax_without_motion_authorization() {
    let source = include_str!("../../../../fixtures/programs/grbl-cutting-check.nc");
    let program = parsed_program(source);
    assert!(program.features.has_spindle_activation);
    assert!(build_dry_run_plan(&program).is_err());
    let expected = build_program_run_plan(&program, ProgramRunPolicy::Cutting)
        .unwrap()
        .lines()
        .iter()
        .filter(|line| line.kind() != DryRunLineKind::ProgramEnd)
        .map(|line| format!("{}\n", line.wire_command()).into_bytes())
        .collect::<Vec<_>>();
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    arbiter.start_check_run(program).await.unwrap();
    let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

    assert_eq!(completed.acknowledged_lines, completed.total_lines);
    assert_eq!(completed.total_lines, expected.len() + 1);
    assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);
    let actual = control
        .writes()
        .into_iter()
        .filter(|write| {
            write.ends_with(b"\n") && !write.starts_with(b"$") && write.as_slice() != b"$C\n"
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
    assert!(actual.iter().any(|line| {
        String::from_utf8_lossy(line)
            .split_whitespace()
            .collect::<Vec<_>>()
            .ends_with(&["S12000", "M3"])
    }));
    assert!(actual.iter().any(|line| {
        String::from_utf8_lossy(line)
            .split_whitespace()
            .collect::<Vec<_>>()
            .ends_with(&["S6000", "M4"])
    }));
    task.abort();
}

#[tokio::test]
async fn serial_check_run_applies_bound_optional_stop_block_delete_and_checksums() {
    let source = include_str!("../../../../fixtures/programs/grbl-stream-semantics-check.nc");
    let options = ProgramExecutionOptions {
        optional_stop: true,
        block_delete: true,
        ..ProgramExecutionOptions::default()
    };
    let program = parsed_program_with_options(source, options);
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    arbiter
        .start_check_run_with_options(program, options)
        .await
        .unwrap();
    let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

    assert_eq!(completed.acknowledged_lines, completed.total_lines);
    let commands = control
        .writes()
        .into_iter()
        .filter(|write| {
            write.ends_with(b"\n") && !write.starts_with(b"$") && write.as_slice() != b"$C\n"
        })
        .map(|write| String::from_utf8(write).unwrap())
        .collect::<Vec<_>>();
    assert!(commands.contains(&"N5 M1\n".to_owned()));
    assert!(!commands.iter().any(|line| line.starts_with("N3 ")));
    assert!(!commands.iter().any(|line| line.starts_with("N50 ")));
    assert!(commands.iter().all(|line| !line.contains('*')));
    assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);
    task.abort();
}

#[tokio::test]
async fn serial_check_run_exits_check_mode_after_a_correlated_error() {
    let (arbiter, control, worker) = serial_preflight_arbiter();
    control.queue_program_ok();
    control.queue_program_ok();
    control.queue_program_error(33);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    arbiter
        .start_check_run(parsed_program("G21 G90 G94\nG1 X1 F10"))
        .await
        .unwrap();
    let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

    assert_eq!(failed.current_source_line, Some(1));
    let failure = failed.failure.unwrap();
    assert_eq!(failure.kind, SenderFailureKind::GrblError);
    assert_eq!(failure.grbl_code, Some(33));
    assert_eq!(failure.source_line, Some(1));
    assert_eq!(failure.command.as_deref(), Some("G21 G90 G94"));
    assert_eq!(arbiter.snapshot().machine.mode, MachineMode::Idle);
    let writes = control.writes();
    assert_eq!(
        writes
            .iter()
            .filter(|write| write.as_slice() == b"$C\n")
            .count(),
        2
    );
    assert!(!writes.contains(&b"!".to_vec()));
    assert!(!writes.contains(&b"\x18".to_vec()));
    task.abort();
}

#[tokio::test]
async fn mock_target_runs_the_same_grbl_check_workflow() {
    let (arbiter, control, worker) = mock_dry_run_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    let started = arbiter
        .start_check_run(parsed_program("G21 G90 G94\nG1 X1 F10"))
        .await
        .unwrap();
    let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

    assert_eq!(started.mode, Some(millo_sender::SenderMode::CheckRun));
    assert_eq!(completed.acknowledged_lines, completed.total_lines);
    assert_eq!(
        control
            .writes()
            .iter()
            .filter(|write| write.as_slice() == b"$C\n")
            .count(),
        2
    );
    task.abort();
}

#[tokio::test]
async fn mock_target_executes_an_authorized_program_and_reports_machine_motion() {
    let source = "G21 G90 G94\nG0 Z2\nG1 X20 Y10 Z-0.2 F300\nM30";
    let (arbiter, control, worker) = mock_dry_run_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    wait_for_sender(&arbiter, SenderState::Draining).await;
    control.advance_program(Duration::from_secs(60));
    let snapshot = arbiter.refresh_status().await.unwrap();
    let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

    assert_eq!(completed.acknowledged_lines, completed.total_lines);
    assert_eq!(snapshot.machine.machine_position.unwrap().x, 20.0);
    assert_eq!(snapshot.machine.machine_position.unwrap().y, 10.0);
    assert_eq!(snapshot.machine.machine_position.unwrap().z, -0.2);
    task.abort();
}

#[tokio::test]
async fn physical_program_end_waits_for_idle_and_survives_hold_resume() {
    let source = "G21 G90 G94\nG1 X2 F20\nM30";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, source, true).await;

    let draining = wait_for_sender(&arbiter, SenderState::Draining).await;
    assert_eq!(draining.current_command.as_deref(), Some("M30"));
    assert!(!control.writes().contains(&b"N3 M30\n".to_vec()));

    control.set_status("<Run|MPos:1.000,0.000,0.000|WPos:1.000,0.000,0.000|FS:20,0>");
    arbiter.refresh_status().await.unwrap();
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Draining);
    assert!(!control.writes().contains(&b"N3 M30\n".to_vec()));

    arbiter.feed_hold().await.unwrap();
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Paused);
    arbiter.resume_program_run().await.unwrap();
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Draining);
    assert!(!control.writes().contains(&b"N3 M30\n".to_vec()));

    control.set_status("<Idle|MPos:2.000,0.000,0.000|WPos:2.000,0.000,0.000|FS:0,0>");
    arbiter.refresh_status().await.unwrap();
    let completed = wait_for_sender(&arbiter, SenderState::Completed).await;
    assert_eq!(completed.state, SenderState::Completed);
    assert_eq!(completed.acknowledged_lines, completed.total_lines);
    assert_eq!(
        control
            .writes()
            .iter()
            .filter(|write| write.as_slice() == b"N3 M30\n")
            .count(),
        1
    );
    task.abort();
}

#[tokio::test]
async fn soft_reset_cancels_a_deferred_program_end_without_dispatching_it() {
    let source = "G21 G90 G94\nG1 X2 F20\nM30";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    wait_for_sender(&arbiter, SenderState::Draining).await;

    let challenge = arbiter.request_soft_reset().await.unwrap();
    arbiter.confirm_soft_reset(challenge.id).await.unwrap();

    assert_eq!(arbiter.sender_snapshot().state, SenderState::Cancelled);
    assert!(!control.writes().contains(&b"N3 M30\n".to_vec()));
    assert_eq!(control.writes().last(), Some(&b"\x18".to_vec()));
    task.abort();
}

#[tokio::test]
async fn deferred_program_end_timeout_fails_the_correlated_line() {
    let source = "G21 G90 G94\nG1 X2 F20\nM30";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    wait_for_sender(&arbiter, SenderState::Draining).await;

    control.queue_program_stall();
    arbiter.refresh_status().await.unwrap();
    let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

    assert_eq!(failed.state, SenderState::Failed);
    assert_eq!(failed.current_command.as_deref(), Some("M30"));
    assert!(
        failed
            .last_error
            .as_deref()
            .is_some_and(|error| error.contains("timed out"))
    );
    task.abort();
}

#[tokio::test]
async fn program_run_fails_on_alarm_after_all_lines_were_accepted() {
    let source = "G21 G90 G94\nG1 X2 F20";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    wait_for_sender(&arbiter, SenderState::Draining).await;

    control.set_status("<Alarm|MPos:1.000,0.000,0.000|WPos:1.000,0.000,0.000|FS:0,0>");
    arbiter.refresh_status().await.unwrap();

    assert_eq!(arbiter.sender_snapshot().state, SenderState::Failed);
    task.abort();
}

#[tokio::test]
async fn program_run_fails_on_status_link_loss_while_draining() {
    let source = "G21 G90 G94\nG1 X2 F20";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    wait_for_sender(&arbiter, SenderState::Draining).await;

    control.queue_disconnect();
    assert!(arbiter.refresh_status().await.is_err());

    let failed = arbiter.sender_snapshot();
    assert_eq!(failed.state, SenderState::Failed);
    assert_eq!(arbiter.snapshot().connection, ConnectionState::Disconnected);
    assert!(
        failed
            .last_error
            .as_deref()
            .is_some_and(|value| value.contains("status failed"))
    );
    task.abort();
}

#[tokio::test]
async fn serial_fixture_stops_on_correlated_error() {
    let source = "G21 G90 G94\nG1 X2 F20\nG1 X4 F20\nG1 X6 F20";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    control.queue_program_ok();
    control.queue_program_ok();
    control.queue_program_error(20);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

    assert_eq!(failed.current_source_line, Some(1));
    assert_eq!(failed.acknowledged_lines, 2);
    let failure = failed.failure.unwrap();
    assert_eq!(failure.kind, SenderFailureKind::GrblError);
    assert_eq!(failure.grbl_code, Some(20));
    assert_eq!(failure.source_line, Some(1));
    let writes = control.writes();
    assert_eq!(
        writes[writes.len() - 2..],
        [b"!".to_vec(), b"\x18".to_vec()]
    );

    let recovered = arbiter.refresh_status().await.unwrap();
    assert_eq!(recovered.machine.mode, MachineMode::Idle);
    assert!(recovered.reset_notice.is_some());
    task.abort();
}

#[tokio::test]
async fn serial_fixture_stops_on_alarm_and_keeps_alarm_state() {
    let source = "G21 G90 G94\nG1 X2 F20";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    control.queue_program_ok();
    control.queue_program_ok();
    control.queue_program_alarm(2);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();

    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

    assert_eq!(failed.current_source_line, Some(1));
    assert_eq!(
        arbiter.snapshot().alarm.and_then(|alarm| alarm.code),
        Some(2)
    );
    task.abort();
}

#[tokio::test]
async fn serial_fixture_hold_pauses_and_resume_continues_the_same_plan() {
    let source = "G21 G90 G94\nG0 Z2\nG1 X2 F20";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    let (_, started) = authorize_and_start_serial_fixture(&arbiter, source, false).await;

    assert_eq!(started.state, SenderState::Running);
    control.set_status("<Run|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:20,0>");
    let paused = arbiter.feed_hold().await.unwrap();
    assert_eq!(paused.connection, ConnectionState::Connected);
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Paused);
    assert_eq!(control.writes().last(), Some(&b"!".to_vec()));

    arbiter.resume_program_run().await.unwrap();
    assert_eq!(control.writes().last(), Some(&b"~".to_vec()));
    arbiter.release_serial_run_fixture().await.unwrap();
    wait_for_sender(&arbiter, SenderState::Draining).await;
    control.set_status("<Idle|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:0,0>");
    arbiter.refresh_status().await.unwrap();
    let completed = wait_for_sender(&arbiter, SenderState::Completed).await;
    assert_eq!(completed.mode, Some(millo_sender::SenderMode::CutRun));
    task.abort();
}

#[tokio::test]
async fn typed_program_pause_and_abort_stop_only_a_physical_sender() {
    let source = "G21 G90 G94\nG0 Z2\nG1 X2 F20";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, source, false).await;

    let paused = arbiter.pause_program_run().await.unwrap();
    assert_eq!(paused.state, SenderState::Paused);
    assert_eq!(control.writes().last(), Some(&b"!".to_vec()));

    let stopped = arbiter.abort_program_run().await.unwrap();
    assert_eq!(stopped.state, SenderState::Cancelled);
    assert_eq!(
        control.writes()[control.writes().len() - 2..],
        [b"!".to_vec(), b"\x18".to_vec()]
    );
    assert!(matches!(
        arbiter.abort_program_run().await.unwrap_err(),
        ArbiterError::ProgramRunStopUnavailable(SenderState::Cancelled)
    ));
    task.abort();
}

#[tokio::test]
async fn feed_hold_preempts_a_delayed_program_response() {
    let source = "G21 G90 G94\nG1 X2 F20";
    let (arbiter, control, worker) = serial_preflight_arbiter_for_realtime_preemption();
    control.queue_program_delay(20);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    tokio::time::sleep(Duration::from_millis(15)).await;

    tokio::time::timeout(Duration::from_millis(30), arbiter.feed_hold())
        .await
        .expect("Feed Hold must preempt response waiting")
        .unwrap();

    assert_eq!(arbiter.sender_snapshot().state, SenderState::Paused);
    assert!(control.writes().contains(&b"!".to_vec()));
    let challenge = arbiter.request_soft_reset().await.unwrap();
    arbiter.confirm_soft_reset(challenge.id).await.unwrap();
    task.abort();
}

#[tokio::test]
async fn realtime_overrides_preempt_sender_waiting_without_pausing_it() {
    let source = "G21 G90 G94\nG1 X2 F20";
    let (arbiter, control, worker) = serial_preflight_arbiter_for_realtime_preemption();
    control.queue_program_delay(20);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    tokio::time::sleep(Duration::from_millis(15)).await;

    arbiter
        .adjust_feed_override(OverrideAdjustment::IncreaseTen)
        .await
        .unwrap();
    arbiter
        .set_rapid_override(RapidOverrideTarget::Half)
        .await
        .unwrap();
    arbiter
        .adjust_spindle_override(OverrideAdjustment::DecreaseOne)
        .await
        .unwrap();
    let acknowledged_before_refresh = arbiter.sender_snapshot().acknowledged_lines;
    arbiter.refresh_status().await.unwrap();

    assert_eq!(arbiter.sender_snapshot().state, SenderState::Running);
    assert_eq!(
        arbiter.sender_snapshot().acknowledged_lines,
        acknowledged_before_refresh
    );
    let writes = control.writes();
    assert!(writes.contains(&vec![0x91]));
    assert!(writes.contains(&vec![0x96]));
    assert!(writes.contains(&vec![0x9d]));
    let challenge = arbiter.request_soft_reset().await.unwrap();
    arbiter.confirm_soft_reset(challenge.id).await.unwrap();
    task.abort();
}

#[tokio::test]
async fn realtime_write_failure_quarantines_a_physical_sender() {
    let source = "G21 G90 G94\nG1 X2 F20";
    let (arbiter, control, worker) = serial_preflight_arbiter_for_realtime_preemption();
    control.queue_program_delay(20);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    tokio::time::sleep(Duration::from_millis(15)).await;
    control.drop_link();

    let error = arbiter.feed_hold().await.unwrap_err();

    assert!(matches!(
        error,
        ArbiterError::Controller(ControllerError::Transport(TransportError::NotConnected))
    ));
    let failed = arbiter.sender_snapshot();
    assert_eq!(failed.state, SenderState::Failed);
    assert_eq!(
        failed.failure.as_ref().map(|failure| failure.kind),
        Some(SenderFailureKind::Disconnected)
    );
    assert_eq!(arbiter.snapshot().connection, ConnectionState::Disconnected);
    let writes_after_failure = control.writes();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(control.writes(), writes_after_failure);
    task.abort();
}

#[tokio::test]
async fn physical_sender_uses_interleaved_status_as_a_liveness_heartbeat() {
    let transport = MockTransport::default();
    let control = transport.control();
    control.set_virtual_motion_enabled(false);
    // Keep the acknowledgement pending long enough to observe a poll frame
    // deterministically, even when the test runner is under light load.
    control.queue_program_delay(80);
    let (arbiter, worker) = CommandArbiter::new_with_execution_target(
        Box::new(transport),
        ControllerConfig {
            poll_interval: Duration::from_millis(5),
            status_timeout: Duration::from_millis(20),
            command_timeout: Duration::from_millis(50),
            failures_before_recovery: 2,
        },
        HardwareProfile::first_machine(),
        ExecutionTarget::Serial,
    );
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, "G21 G90 G94\nG1 X2 F20", true).await;
    control.set_status(
        "<Run|MPos:1.000,0.000,0.000|WPos:1.000,0.000,0.000|FS:20,0|Bf:12,200|Ov:80,50,90>",
    );

    tokio::time::timeout(Duration::from_millis(150), async {
        loop {
            let snapshot = arbiter.snapshot();
            if snapshot.machine.mode == MachineMode::Run
                && snapshot
                    .machine
                    .overrides
                    .is_some_and(|overrides| overrides.feed_percent == 80)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("interleaved status should update live telemetry");

    tokio::time::sleep(Duration::from_millis(90)).await;
    assert_ne!(arbiter.sender_snapshot().state, SenderState::Failed);
    assert!(arbiter.sender_snapshot().in_flight_lines > 0);
    assert!(control.writes().contains(&b"?".to_vec()));
    let challenge = arbiter.request_soft_reset().await.unwrap();
    arbiter.confirm_soft_reset(challenge.id).await.unwrap();
    task.abort();
}

#[tokio::test]
async fn serial_fixture_stops_when_controller_resets_during_a_program_line() {
    let source = "G21 G90 G94\nG1 X2 F20";
    let (arbiter, control, worker) = serial_preflight_arbiter();
    control.queue_program_ok();
    control.queue_program_ok();
    control.queue_program_reset("1.1h");
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

    assert_eq!(failed.current_source_line, Some(1));
    assert_eq!(
        arbiter.snapshot().reset_notice.unwrap().version.as_deref(),
        Some("1.1h")
    );
    task.abort();
}

#[tokio::test]
async fn serial_fixture_fails_closed_on_link_drop_during_a_program_line() {
    let source = "G21 G90 G94\nG1 X2 F20";
    let (arbiter, control, worker) = serial_preflight_arbiter_with_poll(Duration::from_millis(5));
    control.queue_program_ok();
    control.queue_program_ok();
    control.queue_program_disconnect();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    authorize_and_start_serial_fixture(&arbiter, source, true).await;
    let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

    assert_eq!(failed.current_source_line, Some(1));
    let failure = failed.failure.unwrap();
    assert_eq!(failure.kind, SenderFailureKind::Disconnected);
    assert_eq!(failure.source_line, Some(1));
    assert_eq!(failure.command.as_deref(), Some("G21 G90 G94"));
    assert_eq!(arbiter.snapshot().connection, ConnectionState::Disconnected);
    let writes_after_failure = control.writes();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(control.writes(), writes_after_failure);

    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();
    assert_eq!(arbiter.sender_snapshot().state, SenderState::Failed);
    assert!(
        !control.writes()[writes_after_failure.len()..]
            .iter()
            .any(|write| write.starts_with(b"N"))
    );
    task.abort();
}

#[tokio::test]
async fn mock_actor_sends_one_policy_approved_line_per_acknowledgement() {
    let (arbiter, control, worker) = mock_dry_run_arbiter();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();

    let started = arbiter
        .start_dry_run(dry_run_plan("G21 G90\nG0 X1\nG1 X2 F10"))
        .await
        .unwrap();
    let completed = wait_for_sender(&arbiter, SenderState::Completed).await;

    assert_eq!(started.state, SenderState::Running);
    assert_eq!(completed.acknowledged_lines, 7);
    assert_eq!(completed.total_lines, 7);
    assert!(completed.shutdown_commands_acknowledged);
    assert_eq!(
        control.writes(),
        vec![
            b"?".to_vec(),
            b"M5\n".to_vec(),
            b"M9\n".to_vec(),
            b"N1 G21 G90\n".to_vec(),
            b"N2 G0 X1\n".to_vec(),
            b"N3 G1 X2 F10\n".to_vec(),
            b"M5\n".to_vec(),
            b"M9\n".to_vec(),
        ]
    );
    task.abort();
}

#[tokio::test]
async fn mock_actor_prefills_but_never_overruns_the_grbl_rx_buffer() {
    let source = (0..40)
        .map(|index| format!("G1 X{index} F100"))
        .collect::<Vec<_>>()
        .join("\n");
    let (arbiter, control, worker) = mock_dry_run_arbiter();
    control.queue_program_stall();
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();
    arbiter.start_dry_run(dry_run_plan(&source)).await.unwrap();

    tokio::time::timeout(Duration::from_millis(40), async {
        loop {
            if control.writes().len() > 3 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();

    let writes = control.writes();
    let buffered = writes
        .iter()
        .filter(|write| write.as_slice() != b"?")
        .collect::<Vec<_>>();
    let buffered_bytes = buffered.iter().map(|write| write.len()).sum::<usize>();
    assert!(buffered.len() > 1);
    assert!(buffered_bytes <= millo_sender::DEFAULT_GRBL_RX_BUFFER_BYTES);
    let snapshot = arbiter.sender_snapshot();
    assert_eq!(snapshot.in_flight_lines, buffered.len());
    assert_eq!(snapshot.rx_buffer_bytes, buffered_bytes);

    let failed = wait_for_sender(&arbiter, SenderState::Failed).await;
    assert_eq!(failed.current_command.as_deref(), Some("M5"));
    task.abort();
}

#[tokio::test]
async fn mock_actor_correlates_the_exact_rejected_fifo_line() {
    let (arbiter, control, worker) = mock_dry_run_arbiter();
    control.queue_program_ok();
    control.queue_program_ok();
    control.queue_program_error(20);
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();

    arbiter
        .start_dry_run(dry_run_plan("G21\nG0 X1"))
        .await
        .unwrap();
    let failed = wait_for_sender(&arbiter, SenderState::Failed).await;

    assert_eq!(failed.current_source_line, Some(1));
    assert_eq!(failed.acknowledged_lines, 2);
    let failure = failed.failure.unwrap();
    assert_eq!(failure.kind, SenderFailureKind::GrblError);
    assert_eq!(failure.grbl_code, Some(20));
    assert_eq!(failure.source_line, Some(1));
    assert_eq!(
        control.writes(),
        vec![
            b"?".to_vec(),
            b"M5\n".to_vec(),
            b"M9\n".to_vec(),
            b"N1 G21\n".to_vec(),
            b"N2 G0 X1\n".to_vec(),
            b"M5\n".to_vec(),
            b"M9\n".to_vec(),
        ]
    );
    task.abort();
}
