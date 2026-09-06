use super::*;

pub(super) fn verified_rotary_restart_state(
    snapshot: &ControllerSnapshot,
    inspection: &DeviceInspection,
    initial_work_a_degrees: Option<f64>,
    clearance_confirmed: bool,
) -> Result<Option<millo_restart::RotaryRestartState>, String> {
    if !clearance_confirmed {
        return Ok(None);
    }
    if !snapshot.is_stable_idle()
        || millo_grbl::rotary_axis_evidence(Some(inspection), Some(&snapshot.machine)).is_none()
    {
        return Err("Для повторного входа A нужны свежие Idle и координаты XYZA.".to_owned());
    }
    let missing =
        || "Нет подтверждённого угла A, смещения WCO A или начального угла программы.".to_owned();
    let work_a_degrees = snapshot
        .machine
        .work_position
        .and_then(|position| position.a)
        .filter(|angle| angle.is_finite())
        .ok_or_else(missing)?;
    let work_offset_a_degrees = snapshot
        .machine
        .work_coordinate_offset
        .and_then(|position| position.a)
        .filter(|angle| angle.is_finite())
        .ok_or_else(missing)?;
    let initial_work_a_degrees = initial_work_a_degrees
        .filter(|angle| angle.is_finite())
        .ok_or_else(missing)?;
    let work_coordinate_system =
        match active_work_coordinate_system(&inspection.modal_state).ok_or_else(missing)? {
            WorkCoordinateSystem::G54 => millo_gcode::ProgramWorkCoordinateSystem::G54,
            WorkCoordinateSystem::G55 => millo_gcode::ProgramWorkCoordinateSystem::G55,
            WorkCoordinateSystem::G56 => millo_gcode::ProgramWorkCoordinateSystem::G56,
            WorkCoordinateSystem::G57 => millo_gcode::ProgramWorkCoordinateSystem::G57,
            WorkCoordinateSystem::G58 => millo_gcode::ProgramWorkCoordinateSystem::G58,
            WorkCoordinateSystem::G59 => millo_gcode::ProgramWorkCoordinateSystem::G59,
        };
    Ok(Some(millo_restart::RotaryRestartState {
        work_a_degrees,
        work_offset_a_degrees,
        reference_work_offset_a_degrees: work_offset_a_degrees,
        initial_work_a_degrees,
        work_coordinate_system,
        clearance_confirmed,
    }))
}
