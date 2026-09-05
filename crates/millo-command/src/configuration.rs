use super::*;

pub(super) async fn execute_machine_output(
    controller: &mut Controller<BoxedTransport>,
    profile: &HardwareProfile,
    request: MachineOutputRequest,
) -> Result<MachineOutputOutcome, ArbiterError> {
    ensure_stable_idle(&controller.refresh_status().await?)?;
    let before = controller.inspect_device().await?;
    validate_machine_output_request(profile, &before, request)?;
    let responses = controller.set_machine_outputs(request).await?;
    let modal = controller
        .query_device(millo_controller::DeviceQuery::ModalState)
        .await?;
    let inspection = build_device_inspection(vec![modal]);
    verify_machine_output(request, &inspection.modal_state)?;
    let snapshot = controller.refresh_status().await?;
    Ok(MachineOutputOutcome {
        commands: responses
            .into_iter()
            .map(|response| response.command)
            .collect(),
        snapshot,
    })
}

pub(super) fn validate_machine_output_request(
    profile: &HardwareProfile,
    inspection: &DeviceInspection,
    request: MachineOutputRequest,
) -> Result<(), ArbiterError> {
    match request {
        MachineOutputRequest::SpindleOn { speed_rpm, .. } => {
            if profile.spindle_control != SpindleControl::Controller {
                return Err(ArbiterError::ControllerSpindleDisabled);
            }
            let minimum = inspection
                .settings
                .get("$31")
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or(0.0);
            let maximum = positive_device_setting(inspection, "$30")?;
            if !speed_rpm.is_finite() || !(minimum..=maximum).contains(&speed_rpm) {
                return Err(ArbiterError::SpindleSpeedOutOfRange { minimum, maximum });
            }
        }
        MachineOutputRequest::FloodCoolant(_) if !profile.flood_coolant_control => {
            return Err(ArbiterError::CoolantOutputDisabled("flood"));
        }
        MachineOutputRequest::MistCoolant(_) if !profile.mist_coolant_control => {
            return Err(ArbiterError::CoolantOutputDisabled("mist"));
        }
        MachineOutputRequest::SpindleOff
        | MachineOutputRequest::AllOff
        | MachineOutputRequest::FloodCoolant(_)
        | MachineOutputRequest::MistCoolant(_) => {}
    }
    Ok(())
}

pub(super) fn verify_machine_output(
    request: MachineOutputRequest,
    modal_state: &[String],
) -> Result<(), ArbiterError> {
    let has = |word: &str| modal_state.iter().any(|current| current == word);
    let valid = match request {
        MachineOutputRequest::SpindleOn { direction, .. } => match direction {
            millo_domain::SpindleDirection::Clockwise => has("M3"),
            millo_domain::SpindleDirection::Counterclockwise => has("M4"),
        },
        MachineOutputRequest::SpindleOff => has("M5"),
        MachineOutputRequest::FloodCoolant(true) => has("M8"),
        MachineOutputRequest::MistCoolant(true) => has("M7"),
        MachineOutputRequest::FloodCoolant(false) | MachineOutputRequest::MistCoolant(false) => {
            has("M9")
        }
        MachineOutputRequest::AllOff => has("M5") && has("M9"),
    };
    if valid {
        Ok(())
    } else {
        Err(ArbiterError::MachineOutputVerification(format!(
            "modal state does not reflect {request:?}: {}",
            modal_state.join(" ")
        )))
    }
}

pub(super) async fn configure_unhomed_operation(
    controller: &mut Controller<BoxedTransport>,
) -> Result<UnhomedConfiguration, ArbiterError> {
    ensure_stable_idle(&controller.snapshot())?;
    let before = controller.inspect_device().await?;
    let mut writes = Vec::with_capacity(2);

    if before.settings.get("$21").map(String::as_str) != Some("0") {
        ensure_stable_idle(&controller.snapshot())?;
        writes.push(
            controller
                .disable_unhomed_setting(UnhomedSetting::HardLimits)
                .await?,
        );
    }
    if before.settings.get("$22").map(String::as_str) != Some("0") {
        ensure_stable_idle(&controller.snapshot())?;
        writes.push(
            controller
                .disable_unhomed_setting(UnhomedSetting::Homing)
                .await?,
        );
    }

    ensure_stable_idle(&controller.snapshot())?;
    let after = controller.inspect_device().await?;
    for key in ["$21", "$22"] {
        if after.settings.get(key).map(String::as_str) != Some("0") {
            return Err(ArbiterError::ConfigurationVerification(format!(
                "expected {key}=0, read {:?}",
                after.settings.get(key)
            )));
        }
    }

    Ok(UnhomedConfiguration {
        before,
        after,
        writes,
    })
}

pub(super) async fn execute_controller_setting_update(
    controller: &mut Controller<BoxedTransport>,
    request: ControllerSettingEditRequest,
) -> Result<VerifiedSettingUpdate, ArbiterError> {
    controller.refresh_status().await?;
    ensure_stable_idle(&controller.snapshot())?;
    let before = controller.inspect_device().await?;
    let setting = validate_setting_edit(request, &before)?;
    let before_value = before
        .settings
        .get(setting.key())
        .ok_or_else(|| ArbiterError::SettingSourceMissing(setting.key().to_owned()))?
        .clone();
    ensure_stable_idle(&controller.snapshot())?;
    let write = controller.write_setting(&setting).await?;
    controller.refresh_status().await?;
    ensure_stable_idle(&controller.snapshot())?;
    let inspection = controller.inspect_device().await?;
    let stored_value = inspection
        .settings
        .get(setting.key())
        .cloned()
        .unwrap_or_else(|| "missing".to_owned());
    if !setting_values_equal(setting.value(), &stored_value) {
        return Err(ArbiterError::SettingVerification {
            key: setting.key().to_owned(),
            requested: setting.value().to_owned(),
            stored: stored_value,
        });
    }
    Ok(VerifiedSettingUpdate {
        key: setting.key().to_owned(),
        before_value,
        stored_value,
        write,
        inspection,
    })
}

pub(super) fn setting_flag(inspection: &DeviceInspection, key: &str) -> Option<bool> {
    match inspection.settings.get(key).map(String::as_str) {
        Some("0") => Some(false),
        Some("1") => Some(true),
        _ => None,
    }
}

pub(super) fn positive_device_setting(
    inspection: &DeviceInspection,
    key: &'static str,
) -> Result<f64, ArbiterError> {
    inspection
        .settings
        .get(key)
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| ArbiterError::ConfigurationVerification(format!("missing {key}")))
}
