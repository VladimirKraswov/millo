use super::*;

pub(super) fn request_conflicts_with_sender(sender: &Sender, request: &Request) -> bool {
    if !sender_is_active(&sender.snapshot()) {
        return false;
    }
    match request {
        Request::InspectDevice { .. }
        | Request::PreflightRealRun { .. }
        | Request::AuthorizeFirstCut { .. }
        | Request::UpdateControllerSetting { .. }
        | Request::ConfigureUnhomedOperation { .. }
        | Request::UnlockAlarm { .. } => true,
        // Drained setup operations remain available at a pause/tool-change
        // barrier, but their synchronous readers must never consume stream ACKs.
        Request::PrepareTestJog { .. }
        | Request::StepJog { .. }
        | Request::JogPadStep { .. }
        | Request::SetWorkZero { .. }
        | Request::ReturnToWorkZero { .. } => sender.has_in_flight(),
        _ => false,
    }
}

pub(super) fn request_allowed_during_machine_operation(
    actor: &ActorState,
    request: &Request,
) -> bool {
    let common = matches!(
        request,
        Request::Realtime {
            command: RealtimeCommand::Status
                | RealtimeCommand::FeedHold
                | RealtimeCommand::SoftReset,
            ..
        } | Request::BeginSoftReset { .. }
            | Request::ConfirmSoftReset { .. }
            | Request::CommitPreparedHeightmap { .. }
            | Request::DiscardPreparedHeightmap { .. }
            | Request::PauseHeightmap { .. }
            | Request::ResumeHeightmap { .. }
            | Request::CancelHeightmap { .. }
    );
    common
        || ((actor.active_homing.is_some() || actor.active_continuous_jog.is_some())
            && matches!(request, Request::Disconnect { .. }))
        || (actor.active_continuous_jog.is_some()
            && matches!(
                request,
                Request::CancelJog { .. } | Request::RefreshStatus { .. }
            ))
}

pub(super) fn reject_request_during_machine_operation(request: Request) {
    match request {
        Request::ReplaceTransport { response, .. }
        | Request::Connect { response }
        | Request::Disconnect { response }
        | Request::RefreshStatus { response }
        | Request::AcknowledgeReset { response }
        | Request::UnlockAlarm { response, .. }
        | Request::Realtime { response, .. }
        | Request::ConfirmSoftReset { response, .. }
        | Request::CancelJog { response } => send_machine_operation_busy(response),
        Request::SetHardwareProfile { response, .. }
        | Request::BindHardwareProfile { response, .. } => send_machine_operation_busy(response),
        Request::UpdateControllerSetting { response, .. } => send_machine_operation_busy(response),
        Request::InspectDevice { response } => send_machine_operation_busy(response),
        Request::ExecuteOperatorConsole { response, .. } => send_machine_operation_busy(response),
        Request::PreflightRealRun { response, .. } => send_machine_operation_busy(response),
        Request::AuthorizeFirstCut { response, .. } => send_machine_operation_busy(response),
        Request::StartProgramRun { response, .. }
        | Request::StartCheckRun { response, .. }
        | Request::ResumeProgramRun { response }
        | Request::PauseProgramRun { response }
        | Request::AbortProgramRun { response }
        | Request::CompleteToolChange { response, .. }
        | Request::StartDryRun { response, .. }
        | Request::PauseDryRun { response }
        | Request::ResumeDryRun { response }
        | Request::CancelDryRun { response }
        | Request::CommitPreparedProgramRun { response, .. }
        | Request::DiscardPreparedProgramRun { response, .. } => {
            send_machine_operation_busy(response)
        }
        Request::BeginSoftReset { response } => send_machine_operation_busy(response),
        Request::PrepareTestJog { response, .. } => send_machine_operation_busy(response),
        Request::StepJog { response, .. } => send_machine_operation_busy(response),
        Request::JogPadStep { response, .. } => send_machine_operation_busy(response),
        Request::StartHoming { response, .. } => send_machine_operation_busy(response),
        Request::StartContinuousJog { response, .. } => send_machine_operation_busy(response),
        Request::ConfigureUnhomedOperation { response } => send_machine_operation_busy(response),
        Request::SelectWorkCoordinateSystem { response, .. } => {
            send_machine_operation_busy(response)
        }
        Request::SetMachineOutput { response, .. } => send_machine_operation_busy(response),
        Request::SetWorkZero { response, .. } => send_machine_operation_busy(response),
        Request::ReturnToWorkZero { response, .. } => send_machine_operation_busy(response),
        Request::ReturnToWorkOrigin { response, .. } => send_machine_operation_busy(response),
        Request::ProbeZ { response, .. } => send_machine_operation_busy(response),
        Request::PrepareHeightmap { response, .. }
        | Request::PrepareResumeHeightmap { response, .. }
        | Request::CommitPreparedHeightmap { response, .. }
        | Request::PauseHeightmap { response }
        | Request::ResumeHeightmap { response }
        | Request::CancelHeightmap { response } => send_machine_operation_busy(response),
        Request::DiscardPreparedHeightmap { response, .. } => send_machine_operation_busy(response),
    }
}

pub(super) fn send_machine_operation_busy<T>(response: oneshot::Sender<Result<T, ArbiterError>>) {
    let _ = response.send(Err(ArbiterError::MachineOperationBusy));
}

pub(super) fn machine_operation_active(actor: &ActorState) -> bool {
    actor.active_homing.is_some()
        || actor.active_continuous_jog.is_some()
        || actor.active_z_probe.is_some()
        || actor.prepared_heightmap.is_some()
        || actor.active_heightmap.is_some()
}

pub(super) fn ensure_stable_idle(snapshot: &ControllerSnapshot) -> Result<(), ArbiterError> {
    if snapshot.is_stable_idle() {
        Ok(())
    } else {
        Err(SafetyError::UnsafeControllerState.into())
    }
}

pub(super) fn ensure_profile_binding_available(
    snapshot: &ControllerSnapshot,
) -> Result<(), ArbiterError> {
    if snapshot.connection != ConnectionState::Disconnected {
        Ok(())
    } else {
        Err(SafetyError::UnsafeControllerState.into())
    }
}

pub(super) fn sender_is_active(snapshot: &SenderSnapshot) -> bool {
    matches!(
        snapshot.state,
        SenderState::Ready
            | SenderState::Running
            | SenderState::Paused
            | SenderState::ToolChange
            | SenderState::Draining
    )
}

pub(super) fn bounded_motion_timeout(distance_mm: f64, feed_mm_per_min: f64) -> Duration {
    let motion_seconds =
        if distance_mm.is_finite() && feed_mm_per_min.is_finite() && feed_mm_per_min > 0.0 {
            distance_mm / feed_mm_per_min * 60.0
        } else {
            0.0
        };
    Duration::from_secs_f64(motion_seconds.max(0.0)) + MOTION_SETTLE_MARGIN
}
