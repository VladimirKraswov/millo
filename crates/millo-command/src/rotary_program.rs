use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RotaryFirmware {
    GrblHal,
    FluidNc,
    MilloVirtual,
}

fn rotary_firmware(inspection: &DeviceInspection) -> Result<RotaryFirmware, ArbiterError> {
    let mut identities = [false; 3];
    for line in inspection
        .responses
        .iter()
        .filter(|response| {
            matches!(response.command.as_str(), "$I" | "$I+")
                && response.completion == CommandCompletion::Ok
        })
        .flat_map(|response| &response.lines)
    {
        identities[0] |= line == "[FIRMWARE:grblHAL]";
        identities[1] |= line
            .strip_prefix("[VER:")
            .and_then(|value| value.strip_suffix(']'))
            .and_then(|value| value.split_once(':'))
            .is_some_and(|(version, _)| version.split_whitespace().any(|word| word == "FluidNC"));
        identities[2] |= line == "[FIRMWARE:MilloVirtual]";
    }
    match identities {
        [true, false, false] => Ok(RotaryFirmware::GrblHal),
        [false, true, false] => Ok(RotaryFirmware::FluidNc),
        [false, false, true] => Ok(RotaryFirmware::MilloVirtual),
        _ => Err(ArbiterError::RotaryProgramUnavailable(
            "An unambiguous grblHAL, FluidNC or MilloVirtual firmware identity is required."
                .to_owned(),
        )),
    }
}

/// Shared non-motion capability check. Callers must supply freshly inspected
/// identity and current status from the same reset/reconnect epoch.
pub(super) fn validate_rotary_capability(
    profile: &HardwareProfile,
    inspection: &DeviceInspection,
    snapshot: &ControllerSnapshot,
) -> Result<(), ArbiterError> {
    let rejected = |detail: &str| ArbiterError::RotaryProgramUnavailable(detail.to_owned());
    let rotary = profile
        .rotary_axis
        .as_ref()
        .filter(|_| profile.axes.iter().any(|axis| axis == "A"))
        .ok_or_else(|| rejected("Rotary A must be enabled in the machine profile."))?;
    if ![
        rotary.travel_degrees,
        rotary.max_jog_degrees,
        rotary.max_feed_degrees_per_min,
    ]
    .into_iter()
    .all(|value| value.is_finite() && value > 0.0)
    {
        return Err(rejected(
            "Rotary profile limits must be finite and positive.",
        ));
    }
    if millo_grbl::rotary_axis_evidence(Some(inspection), Some(&snapshot.machine)).is_none() {
        return Err(rejected("Current controller evidence must confirm XYZA."));
    }
    let firmware = rotary_firmware(inspection)?;
    if firmware == RotaryFirmware::GrblHal
        && inspection
            .settings
            .get("$376")
            .and_then(|value| value.parse::<u32>().ok())
            .is_none_or(|mask| mask & 1 == 0)
    {
        return Err(rejected(
            "grblHAL $376 must report A as angular (external bit 0).",
        ));
    }
    if firmware == RotaryFirmware::MilloVirtual
        && millo_grbl::rotary_axis_evidence(Some(inspection), Some(&snapshot.machine))
            != Some(millo_grbl::RotaryAxisEvidence::ReportedAxes)
    {
        return Err(rejected(
            "MilloVirtual requires its explicit XYZA declaration.",
        ));
    }
    for value in [
        snapshot.machine.machine_position,
        snapshot.machine.work_position,
        snapshot.machine.work_coordinate_offset,
    ] {
        if value
            .and_then(|position| position.a)
            .is_none_or(|a| !a.is_finite())
        {
            return Err(rejected(
                "Current finite machine A, work A and work offset A are required.",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_rotary_program(
    program: &GcodeProgram,
    profile: &HardwareProfile,
    inspection: &DeviceInspection,
    snapshot: &ControllerSnapshot,
) -> Result<(), ArbiterError> {
    if !program.features.uses_rotary_a {
        return Ok(());
    }
    validate_rotary_capability(profile, inspection, snapshot)?;
    // The virtual firmware supports coordinated lines, but not rotary arcs.
    if program.features.uses_rotary_arc
        && rotary_firmware(inspection)? == RotaryFirmware::MilloVirtual
    {
        return Err(ArbiterError::RotaryProgramUnavailable(
            "MilloVirtual does not support coordinated rotary arcs.".to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn apply_rotary_preflight(
    report: &mut RunPreflightReport,
    program: &GcodeProgram,
    profile: &HardwareProfile,
    snapshot: &ControllerSnapshot,
) {
    if !program.features.uses_rotary_a {
        return;
    }
    let result = validate_rotary_program(program, profile, &report.hardware.device, snapshot);
    let (level, detail) = match result {
        Ok(()) => (RunPreflightLevel::Pass, "XYZA подтверждена; A задаётся в градусах. Проверьте индекс A и свободное вращение заготовки.".to_owned()),
        Err(error) => {
            report.ready = false;
            report.blocker_count += 1;
            (RunPreflightLevel::Blocker, error.to_string())
        }
    };
    report.checks.push(RunPreflightCheck {
        id: "rotary-a-capability".to_owned(),
        level,
        title: "Поворотная ось A".to_owned(),
        detail,
        source_line: None,
    });
}

#[cfg(test)]
#[path = "tests/rotary_capability.rs"]
mod tests;
