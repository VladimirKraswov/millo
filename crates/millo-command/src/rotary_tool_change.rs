use super::*;

pub(super) struct RotaryRunReference {
    pub run_sequence: u64,
    barrier: Option<RotaryBarrier>,
}

struct RotaryBarrier {
    source_line: Option<usize>,
    coordinate_system: WorkCoordinateSystem,
    snapshot: ControllerSnapshot,
}

impl RotaryRunReference {
    pub fn new(run_sequence: u64) -> Self {
        Self {
            run_sequence,
            barrier: None,
        }
    }

    pub fn verify(
        &self,
        active: &SenderSnapshot,
        inspection: &DeviceInspection,
        current: &ControllerSnapshot,
    ) -> Result<(), ArbiterError> {
        let rejected = || {
            ArbiterError::RotaryProgramUnavailable("После смены инструмента восстановите прежние индекс A, ноль A и G54–G59. Z можно выставить заново.".to_owned())
        };
        let barrier = self.barrier.as_ref().ok_or_else(rejected)?;
        if self.run_sequence != active.run_sequence
            || barrier.source_line != active.current_source_line
            || active_work_coordinate_system(&inspection.modal_state)
                != Some(barrier.coordinate_system)
            || current.reset_count != barrier.snapshot.reset_count
            || current.reconnect_count != barrier.snapshot.reconnect_count
        {
            return Err(rejected());
        }
        for (before, after) in [
            (
                barrier.snapshot.machine.machine_position,
                current.machine.machine_position,
            ),
            (
                barrier.snapshot.machine.work_position,
                current.machine.work_position,
            ),
            (
                barrier.snapshot.machine.work_coordinate_offset,
                current.machine.work_coordinate_offset,
            ),
        ] {
            match (
                before.and_then(|position| position.a),
                after.and_then(|position| position.a),
            ) {
                (Some(a), Some(b)) if a.is_finite() && b.is_finite() && (a - b).abs() <= 0.01 => {}
                _ => return Err(rejected()),
            }
        }
        Ok(())
    }
}

pub(super) async fn observe_rotary_tool_change(actor: &mut ActorState) {
    let Some(reference) = actor.rotary_run_reference.as_mut() else {
        return;
    };
    let active = actor.sender.snapshot();
    if active.run_sequence != reference.run_sequence || !sender_is_active(&active) {
        actor.rotary_run_reference = None;
        return;
    }
    if active.state != SenderState::ToolChange {
        reference.barrier = None;
        return;
    }
    if reference
        .barrier
        .as_ref()
        .is_some_and(|barrier| barrier.source_line == active.current_source_line)
    {
        return;
    }
    let captured = async {
        let inspection = actor.controller.inspect_device().await?;
        let snapshot = actor.controller.refresh_status().await?;
        ensure_stable_idle(&snapshot)?;
        validate_rotary_capability(&actor.hardware_profile, &inspection, &snapshot)?;
        let coordinate_system = active_work_coordinate_system(&inspection.modal_state)
            .ok_or(ArbiterError::ActiveWorkCoordinateSystemUnavailable)?;
        Ok::<_, ArbiterError>(RotaryBarrier {
            source_line: active.current_source_line,
            coordinate_system,
            snapshot,
        })
    }
    .await;
    match captured {
        Ok(barrier) => {
            reference.barrier = Some(barrier);
        }
        Err(error) => {
            actor.sender.fail_with(SenderFailure::new(
                SenderFailureKind::UnsafeState,
                format!("Не удалось зафиксировать A перед сменой инструмента: {error}"),
            ));
            let _ = actor.controller.abort_program_stream().await;
        }
    }
    publish(&actor.snapshots, &actor.controller);
    publish_sender(&actor.sender_snapshots, &actor.sender);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotary_tool_change_preserves_index_and_wcs_but_allows_new_z_datum() {
        let mut snapshot = ControllerSnapshot::default();
        let position = Position {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            a: Some(90.0),
        };
        snapshot.machine.machine_position = Some(position);
        snapshot.machine.work_position = Some(position);
        snapshot.machine.work_coordinate_offset = Some(Position {
            a: Some(0.0),
            ..position
        });
        let mut active = Sender::default().snapshot();
        active.run_sequence = 1;
        active.current_source_line = Some(40);
        let reference = RotaryRunReference {
            run_sequence: 1,
            barrier: Some(RotaryBarrier {
                source_line: Some(40),
                coordinate_system: WorkCoordinateSystem::G54,
                snapshot: snapshot.clone(),
            }),
        };
        let mut inspection = DeviceInspection {
            modal_state: vec!["G54".into()],
            ..Default::default()
        };
        snapshot.machine.work_position.as_mut().unwrap().z = 0.0;
        snapshot.machine.work_coordinate_offset.as_mut().unwrap().z = 18.4;
        assert!(reference.verify(&active, &inspection, &snapshot).is_ok());
        for mutate in [
            |s: &mut ControllerSnapshot| s.machine.work_position.as_mut().unwrap().a = Some(0.0),
            |s: &mut ControllerSnapshot| {
                s.machine.machine_position.as_mut().unwrap().a = Some(91.0)
            },
            |s: &mut ControllerSnapshot| {
                s.machine.work_coordinate_offset.as_mut().unwrap().a = Some(90.0)
            },
            |s: &mut ControllerSnapshot| s.machine.work_position.as_mut().unwrap().a = None,
            |s: &mut ControllerSnapshot| s.reset_count += 1,
        ] {
            let mut changed = snapshot.clone();
            mutate(&mut changed);
            assert!(reference.verify(&active, &inspection, &changed).is_err());
        }
        inspection.modal_state = vec!["G55".into()];
        assert!(reference.verify(&active, &inspection, &snapshot).is_err());
    }
}
