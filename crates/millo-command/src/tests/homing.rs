use super::*;

#[test]
fn homing_timeout_handles_overflow_before_clamping_the_duration() {
    for (travel, seek, locate, expected) in [
        ([f64::MAX; 3], "500", "25", HOMING_MAX_TIMEOUT),
        ([1.0; 3], "1e-300", "25", HOMING_MAX_TIMEOUT),
        ([1.0; 3], "500", "1e-300", HOMING_MAX_TIMEOUT),
        ([1.0; 3], "1e300", "1e300", HOMING_MIN_TIMEOUT),
        ([100.0; 3], "500", "25", Duration::from_secs(123)),
    ] {
        let mut inspection = DeviceInspection::default();
        inspection
            .settings
            .insert("$25".to_owned(), seek.to_owned());
        inspection
            .settings
            .insert("$24".to_owned(), locate.to_owned());
        assert_eq!(homing_timeout(&inspection, travel), expected);
    }
}

#[tokio::test]
async fn extreme_homing_rate_does_not_panic_the_actor() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let mut profile = HardwareProfile::first_machine();
    profile.homing_installed = true;
    control.set_setting(22, "1");
    control.set_setting(25, "1e-300");
    let task = tokio::spawn(worker);
    arbiter.set_hardware_profile(profile).await.unwrap();
    arbiter.connect().await.unwrap();

    let started = arbiter
        .start_homing(HomingRequest {
            operator_confirmed: true,
        })
        .await
        .unwrap();

    assert_eq!(started.timeout_ms, HOMING_MAX_TIMEOUT.as_millis() as u64);
    arbiter
        .send_realtime(RealtimeCommand::SoftReset)
        .await
        .unwrap();
    task.abort();
}

#[tokio::test]
async fn homing_is_actor_owned_and_reset_invalidates_the_reference() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let mut profile = HardwareProfile::first_machine();
    profile.homing_installed = true;
    control.set_setting(22, "1");
    let task = tokio::spawn(worker);
    arbiter.set_hardware_profile(profile).await.unwrap();
    arbiter.connect().await.unwrap();

    let started = arbiter
        .start_homing(HomingRequest {
            operator_confirmed: true,
        })
        .await
        .unwrap();
    assert_eq!(started.command, "$H");
    assert_eq!(started.snapshot.homing.state, HomingState::Homing);

    let homed = wait_for_homing_state(&arbiter, HomingState::Homed).await;
    assert_eq!(homed.machine.mode, MachineMode::Idle);
    assert!(control.writes().iter().any(|write| write == b"$H\n"));

    let challenge = arbiter.request_soft_reset().await.unwrap();
    let reset = arbiter.confirm_soft_reset(challenge.id).await.unwrap();
    assert_eq!(reset.homing.state, HomingState::Invalidated);
    task.abort();
}

#[tokio::test]
async fn homed_continuous_jog_uses_the_machine_coordinate_envelope() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    let mut profile = HardwareProfile::first_machine();
    profile.homing_installed = true;
    control.set_setting(22, "1");
    let task = tokio::spawn(worker);
    arbiter.set_hardware_profile(profile).await.unwrap();
    arbiter.connect().await.unwrap();
    arbiter
        .start_homing(HomingRequest {
            operator_confirmed: true,
        })
        .await
        .unwrap();
    wait_for_homing_state(&arbiter, HomingState::Homed).await;

    let receipt = arbiter
        .start_continuous_jog(ContinuousJogRequest {
            confirmation: operator_confirmation(),
            axis: millo_domain::JogAxis::X,
            direction: -1,
            feed_mm_per_min: 300.0,
        })
        .await
        .unwrap();
    assert_eq!(
        receipt.boundary_source,
        JogBoundarySource::MachineCoordinates
    );
    assert!(receipt.bounded_distance > 297.0 && receipt.bounded_distance < 299.0);
    arbiter.cancel_jog().await.unwrap();
    task.abort();
}

#[tokio::test]
async fn disables_and_verifies_unhomed_controller_settings() {
    let (arbiter, control, worker) = test_arbiter(Duration::from_secs(60));
    control.set_setting(21, "1");
    control.set_setting(22, "1");
    let task = tokio::spawn(worker);
    arbiter.connect().await.unwrap();
    arbiter.refresh_status().await.unwrap();

    let result = arbiter.configure_unhomed_operation().await.unwrap();

    assert_eq!(result.before.settings.get("$21").unwrap(), "1");
    assert_eq!(result.before.settings.get("$22").unwrap(), "1");
    assert_eq!(result.after.settings.get("$21").unwrap(), "0");
    assert_eq!(result.after.settings.get("$22").unwrap(), "0");
    assert_eq!(result.writes.len(), 2);
    assert_eq!(
        control
            .writes()
            .into_iter()
            .filter(|write| write.starts_with(b"$21=") || write.starts_with(b"$22="))
            .collect::<Vec<_>>(),
        vec![b"$21=0\n".to_vec(), b"$22=0\n".to_vec()]
    );
    task.abort();
}
