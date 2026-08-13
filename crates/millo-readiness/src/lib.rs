use millo_domain::{
    CommandCompletion, ControllerSnapshot, DeviceInspection, HardwareProfile, ReadinessCheck,
    ReadinessLevel, ReadinessReport, SpindleControl,
};

const EXPECTED_QUERIES: [&str; 4] = ["$I", "$$", "$G", "$#"];
const AXES: [(&str, &str); 3] = [("X", "0"), ("Y", "1"), ("Z", "2")];

pub fn assess(
    profile: &HardwareProfile,
    inspection: &DeviceInspection,
    snapshot: &ControllerSnapshot,
) -> ReadinessReport {
    let checks = vec![
        controller_state(snapshot),
        query_integrity(inspection),
        firmware_identity(inspection),
        axis_group(
            inspection,
            "axis-steps",
            "Axis calibration",
            "$10",
            "steps/mm",
        ),
        axis_group(
            inspection,
            "axis-rates",
            "Maximum axis rates",
            "$11",
            "mm/min",
        ),
        axis_group(
            inspection,
            "axis-acceleration",
            "Axis acceleration",
            "$12",
            "mm/s^2",
        ),
        axis_group(inspection, "axis-travel", "Configured travel", "$13", "mm"),
        unhomed_operation(profile, inspection),
        milling_mode(inspection),
        modal_units(inspection),
        spindle_state(profile, inspection),
        probe_input(profile, inspection),
        emergency_stop(profile),
    ];

    let blocker_count = checks
        .iter()
        .filter(|check| check.level == ReadinessLevel::Blocker)
        .count();
    let caution_count = checks
        .iter()
        .filter(|check| check.level == ReadinessLevel::Caution)
        .count();

    ReadinessReport {
        profile: profile.clone(),
        test_jog_ready: blocker_count == 0,
        probe_ready: false,
        blocker_count,
        caution_count,
        checks,
    }
}

fn controller_state(snapshot: &ControllerSnapshot) -> ReadinessCheck {
    let ready = snapshot.is_stable_idle();

    check(
        "controller-state",
        if ready {
            ReadinessLevel::Pass
        } else {
            ReadinessLevel::Blocker
        },
        "Controller state",
        if ready {
            "Connected and idle"
        } else {
            "A stable Idle state without alarm or reset notice is required"
        },
        Some(format!(
            "connection={:?}, mode={}",
            snapshot.connection, snapshot.machine.reported_mode
        )),
    )
}

fn query_integrity(inspection: &DeviceInspection) -> ReadinessCheck {
    let failed: Vec<&str> = EXPECTED_QUERIES
        .iter()
        .copied()
        .filter(|command| {
            !inspection.responses.iter().any(|response| {
                response.command == *command && response.completion == CommandCompletion::Ok
            })
        })
        .collect();

    check(
        "inspection-queries",
        if failed.is_empty() {
            ReadinessLevel::Pass
        } else {
            ReadinessLevel::Blocker
        },
        "Read-only inspection",
        if failed.is_empty() {
            "All controller queries completed"
        } else {
            "One or more controller queries are missing or failed"
        },
        Some(if failed.is_empty() {
            EXPECTED_QUERIES.join(" ")
        } else {
            format!("failed: {}", failed.join(" "))
        }),
    )
}

fn firmware_identity(inspection: &DeviceInspection) -> ReadinessCheck {
    let version = inspection.firmware_version.as_deref();
    check(
        "firmware",
        if version.is_some() {
            ReadinessLevel::Pass
        } else {
            ReadinessLevel::Blocker
        },
        "GRBL firmware",
        if version.is_some() {
            "Firmware identity received"
        } else {
            "Firmware identity is unavailable"
        },
        version.map(str::to_owned),
    )
}

fn axis_group(
    inspection: &DeviceInspection,
    id: &str,
    title: &str,
    setting_prefix: &str,
    unit: &str,
) -> ReadinessCheck {
    let mut values = Vec::new();
    let mut valid = true;

    for (axis, suffix) in AXES {
        let key = format!("{setting_prefix}{suffix}");
        match positive_setting(inspection, &key) {
            Some(value) => values.push(format!("{axis}={value}")),
            None => {
                valid = false;
                values.push(format!("{axis}=missing"));
            }
        }
    }

    check(
        id,
        if valid {
            ReadinessLevel::Pass
        } else {
            ReadinessLevel::Blocker
        },
        title,
        if valid {
            "All XYZ values are finite and positive"
        } else {
            "Valid values are required for every XYZ axis"
        },
        Some(format!("{} {unit}", values.join(" · "))),
    )
}

fn unhomed_operation(profile: &HardwareProfile, inspection: &DeviceInspection) -> ReadinessCheck {
    let soft_limits = binary_setting(inspection, "$20");
    let hard_limits = binary_setting(inspection, "$21");
    let homing = binary_setting(inspection, "$22");
    let contradictory = (!profile.homing_installed && homing != Some(false))
        || (!profile.limit_switches_installed && hard_limits != Some(false))
        || (!profile.homing_installed && soft_limits != Some(false));

    check(
        "unhomed-operation",
        if contradictory {
            ReadinessLevel::Blocker
        } else {
            ReadinessLevel::Caution
        },
        "Unhomed operation",
        if contradictory {
            "Limit or homing settings contradict the selected hardware profile"
        } else {
            "Coordinates are not a verified physical envelope"
        },
        Some(format!(
            "$20={} · $21={} · $22={}",
            binary_label(soft_limits),
            binary_label(hard_limits),
            binary_label(homing)
        )),
    )
}

fn milling_mode(inspection: &DeviceInspection) -> ReadinessCheck {
    let laser_mode = binary_setting(inspection, "$32");
    check(
        "milling-mode",
        if laser_mode == Some(false) {
            ReadinessLevel::Pass
        } else {
            ReadinessLevel::Blocker
        },
        "Milling mode",
        if laser_mode == Some(false) {
            "Laser mode is disabled"
        } else {
            "$32 must be 0 for the milling profile"
        },
        Some(format!("$32={}", binary_label(laser_mode))),
    )
}

fn modal_units(inspection: &DeviceInspection) -> ReadinessCheck {
    let millimetres = has_modal(inspection, "G21");
    let incremental = has_modal(inspection, "G91");
    check(
        "modal-units",
        if millimetres {
            if incremental {
                ReadinessLevel::Caution
            } else {
                ReadinessLevel::Pass
            }
        } else {
            ReadinessLevel::Blocker
        },
        "Modal units",
        if !millimetres {
            "Millimetre mode G21 is required for the first test"
        } else if incremental {
            "G91 is active; a future jog must declare its own units and distance mode"
        } else {
            "Millimetres and absolute mode are active"
        },
        Some(inspection.modal_state.join(" ")),
    )
}

fn spindle_state(profile: &HardwareProfile, inspection: &DeviceInspection) -> ReadinessCheck {
    let stopped = has_modal(inspection, "M5");
    let manual = profile.spindle_control == SpindleControl::Manual;
    check(
        "spindle",
        if stopped {
            if manual {
                ReadinessLevel::Caution
            } else {
                ReadinessLevel::Pass
            }
        } else {
            ReadinessLevel::Blocker
        },
        "Spindle workflow",
        if !stopped {
            "The controller modal state must report M5"
        } else if manual {
            "The operator must keep the physical spindle switched off during test jog"
        } else {
            "The controller reports spindle stop"
        },
        Some(if manual {
            "manual spindle · M5".to_owned()
        } else {
            "controller spindle · M5".to_owned()
        }),
    )
}

fn probe_input(profile: &HardwareProfile, inspection: &DeviceInspection) -> ReadinessCheck {
    let invert = binary_setting(inspection, "$6");
    let last_probe = inspection
        .parameters
        .get("PRB")
        .map(String::as_str)
        .unwrap_or("missing");

    check(
        "probe-input",
        if !profile.probe_installed || (invert.is_some() && last_probe != "missing") {
            ReadinessLevel::Caution
        } else {
            ReadinessLevel::Blocker
        },
        "Probe input",
        if !profile.probe_installed {
            "No probe is declared in the selected machine profile"
        } else if invert.is_some() && last_probe != "missing" {
            "Electrical polarity still requires a dedicated stationary probe test"
        } else {
            "Probe configuration or state is unavailable"
        },
        Some(format!(
            "installed={} · $6={} · PRB={last_probe}",
            profile.probe_installed,
            binary_label(invert)
        )),
    )
}

fn emergency_stop(profile: &HardwareProfile) -> ReadinessCheck {
    check(
        "emergency-stop",
        if profile.emergency_stop_installed {
            ReadinessLevel::Pass
        } else {
            ReadinessLevel::Caution
        },
        "Emergency stop",
        if profile.emergency_stop_installed {
            "Physical emergency stop is present"
        } else {
            "No physical emergency stop is installed"
        },
        None,
    )
}

fn positive_setting(inspection: &DeviceInspection, key: &str) -> Option<f64> {
    inspection
        .settings
        .get(key)?
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}

fn binary_setting(inspection: &DeviceInspection, key: &str) -> Option<bool> {
    match inspection.settings.get(key).map(String::as_str) {
        Some("0") => Some(false),
        Some("1") => Some(true),
        _ => None,
    }
}

fn binary_label(value: Option<bool>) -> &'static str {
    match value {
        Some(false) => "0",
        Some(true) => "1",
        None => "missing",
    }
}

fn has_modal(inspection: &DeviceInspection, code: &str) -> bool {
    inspection.modal_state.iter().any(|value| value == code)
}

fn check(
    id: &str,
    level: ReadinessLevel,
    title: &str,
    detail: &str,
    evidence: Option<String>,
) -> ReadinessCheck {
    ReadinessCheck {
        id: id.to_owned(),
        level,
        title: title.to_owned(),
        detail: detail.to_owned(),
        evidence,
    }
}
