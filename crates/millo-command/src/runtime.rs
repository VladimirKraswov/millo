use super::*;

pub(super) async fn run_actor(mut actor: ActorState, mut requests: mpsc::Receiver<Request>) {
    let mut ticker = interval(actor.config.poll_interval);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    ticker.tick().await;

    loop {
        tokio::select! {
            biased;
            request = requests.recv() => {
                let Some(request) = request else {
                    break;
                };
                if machine_operation_active(&actor)
                    && !request_allowed_during_machine_operation(&actor, &request)
                {
                    reject_request_during_machine_operation(request);
                } else {
                    handle_request(request, &mut actor).await;
                }
            }
            _ = ticker.tick() => {
                if matches!(
                    actor.controller.snapshot().connection,
                    ConnectionState::Connected | ConnectionState::Recovering
                ) {
                    if actor.sender.has_in_flight()
                        && actor.controller.snapshot().connection == ConnectionState::Connected
                    {
                        if let Err(error) = actor.controller.request_interleaved_status().await {
                            fail_and_quarantine_physical_sender(
                                &mut actor.controller,
                                &mut actor.sender,
                                &error,
                                "controller status request failed during program run",
                                &actor.sender_snapshots,
                            ).await;
                        }
                    } else if machine_operation_active(&actor) {
                        continue;
                    } else {
                        let lifecycle = actor.controller.lifecycle_tick().await;
                        actor.safety.observe(&actor.controller.snapshot(), Instant::now());
                        actor.first_cut.observe(&actor.controller.snapshot(), Instant::now());
                        actor.program_check.observe(&actor.controller.snapshot(), Instant::now());
                        match lifecycle {
                            Ok(_) => reconcile_physical_sender(
                                &mut actor.controller,
                                &mut actor.sender,
                                &actor.sender_snapshots,
                            ).await,
                            Err(error) => {
                                fail_and_quarantine_physical_sender(
                                    &mut actor.controller,
                                    &mut actor.sender,
                                    &error,
                                    "controller polling failed during program run",
                                    &actor.sender_snapshots,
                                ).await
                            }
                        }
                    }
                    publish(&actor.snapshots, &actor.controller);
                    publish_sender(&actor.sender_snapshots, &actor.sender);
                }
            }
            _ = tokio::task::yield_now(), if actor.sender_dispatch_enabled && actor.sender.has_in_flight() => {
                execute_sender_step(
                    &mut actor.controller,
                    &mut actor.sender,
                    &mut actor.program_check,
                    &mut actor.pending_program_check,
                    &actor.snapshots,
                    &actor.sender_snapshots,
                )
                .await;
            }
            _ = tokio::task::yield_now(), if actor.sender_dispatch_enabled && actor.sender.is_dispatchable() => {
                execute_sender_step(
                    &mut actor.controller,
                    &mut actor.sender,
                    &mut actor.program_check,
                    &mut actor.pending_program_check,
                    &actor.snapshots,
                    &actor.sender_snapshots,
                )
                .await;
            }
            _ = tokio::time::sleep(MACHINE_OPERATION_STEP_INTERVAL), if actor.active_z_probe.is_some() => {
                poll_active_z_probe(&mut actor).await;
            }
            _ = tokio::time::sleep(MACHINE_OPERATION_STEP_INTERVAL), if actor.active_homing.is_some() => {
                poll_active_homing(&mut actor).await;
            }
            _ = tokio::time::sleep(MACHINE_OPERATION_STEP_INTERVAL), if actor.active_continuous_jog.is_some() => {
                poll_active_continuous_jog(&mut actor).await;
            }
            _ = tokio::time::sleep(MACHINE_OPERATION_STEP_INTERVAL), if actor.active_heightmap.is_some() => {
                poll_active_heightmap(&mut actor).await;
            }
        }
        observe_rotary_tool_change(&mut actor).await;
    }
}

pub(super) fn publish(
    snapshots: &watch::Sender<ControllerSnapshot>,
    controller: &Controller<BoxedTransport>,
) {
    snapshots.send_replace(controller.snapshot());
}

pub(super) fn publish_sender(snapshots: &watch::Sender<SenderSnapshot>, sender: &Sender) {
    snapshots.send_replace(sender.snapshot());
}
