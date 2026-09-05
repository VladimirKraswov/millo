use super::*;

#[tauri::command]
pub async fn machine_profiles(state: State<'_, AppState>) -> Result<MachineProfileState, String> {
    Ok(state.profiles.lock().await.state())
}

#[tauri::command]
pub async fn create_machine_profile(
    mut draft: MachineProfileDraft,
    state: State<'_, AppState>,
) -> Result<MachineProfileState, String> {
    let _transition = state.transition_lock.lock().await;
    let connected =
        state.arbiter.snapshot().connection != millo_domain::ConnectionState::Disconnected;
    if connected {
        let session = state.settings_session.lock().await;
        let session = session
            .as_ref()
            .ok_or_else(|| "controller settings have not been synchronized".to_owned())?;
        if session.profile_id.is_some() {
            return Err(
                "the connected controller is already bound to a machine profile".to_owned(),
            );
        }
        let snapshot = build_settings_snapshot(&session.inspection, session.revision);
        draft.travel_mm = snapshot
            .travel_mm()
            .ok_or_else(|| "controller did not report valid $130/$131/$132 travel".to_owned())?;
        draft.connection = Some(session.connection.clone());
        draft.detected_controller = Some(detected_controller(&session.inspection));
    } else {
        ensure_profile_change_available(&state)?;
    }
    let next = state
        .profiles
        .lock()
        .await
        .create_and_select(draft)
        .map_err(|error| error.to_string())?;
    let profile = next
        .selected()
        .ok_or_else(|| "profile store did not select the newly created profile".to_owned())?
        .hardware_profile();
    if connected {
        state
            .arbiter
            .bind_hardware_profile(profile)
            .await
            .map_err(|error| error.to_string())?;
        let selected = next
            .selected()
            .ok_or_else(|| "newly created profile lost its selection".to_owned())?;
        let mut session = state.settings_session.lock().await;
        let active = session
            .as_mut()
            .ok_or_else(|| "controller settings session ended during onboarding".to_owned())?;
        active.profile_id = Some(selected.id.clone());
        active.archive = begin_settings_archive(&state, selected, active)?;
    } else {
        state
            .arbiter
            .set_hardware_profile(profile)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(next)
}

#[tauri::command]
pub async fn update_machine_local_settings(
    profile_id: String,
    update: MachineLocalSettingsUpdate,
    state: State<'_, AppState>,
) -> Result<MachineProfileState, String> {
    let _transition = state.transition_lock.lock().await;
    let connected =
        state.arbiter.snapshot().connection != millo_domain::ConnectionState::Disconnected;
    if connected {
        let session = state.settings_session.lock().await;
        if session
            .as_ref()
            .and_then(|active| active.profile_id.as_deref())
            != Some(profile_id.as_str())
        {
            return Err(
                "only the profile bound to the connected controller can be edited".to_owned(),
            );
        }
    }
    let next = state
        .profiles
        .lock()
        .await
        .update_local_settings(&profile_id, update)
        .map_err(|error| error.to_string())?;
    let profile = next
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "updated profile disappeared from the profile store".to_owned())?
        .hardware_profile();
    if connected {
        state
            .arbiter
            .bind_hardware_profile(profile)
            .await
            .map_err(|error| error.to_string())?;
    } else if next.selected_profile_id.as_deref() == Some(profile_id.as_str()) {
        state
            .arbiter
            .set_hardware_profile(profile)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(next)
}

#[tauri::command]
pub async fn select_machine_profile(
    profile_id: String,
    state: State<'_, AppState>,
) -> Result<MachineProfileState, String> {
    let _transition = state.transition_lock.lock().await;
    ensure_profile_change_available(&state)?;
    let next = state
        .profiles
        .lock()
        .await
        .select(&profile_id)
        .map_err(|error| error.to_string())?;
    let profile = next
        .selected()
        .ok_or_else(|| "profile store did not retain the requested selection".to_owned())?
        .hardware_profile();
    state
        .arbiter
        .set_hardware_profile(profile)
        .await
        .map_err(|error| error.to_string())?;
    Ok(next)
}

#[tauri::command]
pub async fn detect_machine_profile(
    transport_id: String,
    baud_rate: u32,
    state: State<'_, AppState>,
) -> Result<MachineProfileDraft, String> {
    let _transition = state.transition_lock.lock().await;
    ensure_profile_change_available(&state)?;
    let resolved = resolve_transport(&transport_id, baud_rate).await?;
    let descriptor = resolved.descriptor.clone();
    let (arbiter, worker) = CommandArbiter::new_with_execution_target(
        resolved.transport,
        ControllerConfig::default(),
        HardwareProfile::first_machine(),
        resolved.execution_target,
    );
    let worker = tokio::spawn(worker);

    let result = async {
        arbiter.connect().await.map_err(|error| error.to_string())?;
        let snapshot = arbiter
            .refresh_status()
            .await
            .map_err(|error| error.to_string())?;
        if snapshot.reset_notice.is_some() {
            arbiter
                .acknowledge_reset()
                .await
                .map_err(|error| error.to_string())?;
        }
        let inspection = arbiter
            .inspect_device()
            .await
            .map_err(|error| error.to_string())?;
        let fingerprint = machine_fingerprint(&descriptor, &inspection.device);
        MachineProfileDraft::from_grbl_inspection(
            suggested_machine_name(&descriptor, &inspection.device),
            &inspection.device,
            MachineConnectionPreset {
                transport_id: descriptor.id.clone(),
                baud_rate,
                fingerprint: Some(fingerprint),
            },
        )
        .map_err(|error| error.to_string())
    }
    .await;

    let _ = arbiter.disconnect().await;
    worker.abort();
    result
}

pub(super) fn ensure_profile_change_available(state: &AppState) -> Result<(), String> {
    let connection = state.arbiter.snapshot().connection;
    if connection == millo_domain::ConnectionState::Disconnected {
        Ok(())
    } else {
        Err(format!(
            "machine profiles can be changed only while disconnected, current state is {connection:?}"
        ))
    }
}

pub(super) fn match_machine_profile(
    profiles: &MachineProfileState,
    fingerprint: &MachineFingerprint,
    transport_id: &str,
    inspection: &DeviceInspection,
) -> Result<Option<MachineProfile>, String> {
    let exact = profiles
        .profiles
        .iter()
        .filter(|profile| {
            profile
                .connection
                .as_ref()
                .and_then(|connection| connection.fingerprint.as_ref())
                .is_some_and(|stored| stored.key == fingerprint.key)
        })
        .cloned()
        .collect::<Vec<_>>();
    if exact.len() > 1 {
        return Err("multiple machine profiles have the same controller fingerprint".to_owned());
    }
    if let Some(profile) = exact.into_iter().next() {
        return Ok(Some(profile));
    }

    let legacy = profiles
        .profiles
        .iter()
        .filter(|profile| {
            let Some(connection) = profile.connection.as_ref() else {
                return false;
            };
            if connection.fingerprint.is_some() || connection.transport_id != transport_id {
                return false;
            }
            match (
                profile
                    .detected_controller
                    .as_ref()
                    .and_then(|controller| controller.firmware_version.as_deref()),
                inspection.firmware_version.as_deref(),
            ) {
                (Some(stored), Some(observed)) => stored == observed,
                _ => true,
            }
        })
        .cloned()
        .collect::<Vec<_>>();
    if legacy.len() > 1 {
        return Err("the serial device matches more than one legacy machine profile".to_owned());
    }
    Ok(legacy.into_iter().next())
}

pub(super) fn machine_fingerprint(
    descriptor: &TransportDescriptor,
    inspection: &DeviceInspection,
) -> MachineFingerprint {
    let vendor = descriptor.vendor_id.unwrap_or_default();
    let product = descriptor.product_id.unwrap_or_default();
    if let Some(serial) = descriptor
        .serial_number
        .as_deref()
        .map(str::trim)
        .filter(|serial| !serial.is_empty() && *serial != "0")
    {
        return MachineFingerprint {
            key: format!("usb:{vendor:04x}:{product:04x}:{}", identity_token(serial)),
            confidence: IdentityConfidence::Strong,
            label: format!("USB {vendor:04X}:{product:04X} · {serial}"),
        };
    }
    let product_name = descriptor
        .product
        .as_deref()
        .or(descriptor.detail.as_deref())
        .unwrap_or("serial");
    let firmware = inspection.firmware_version.as_deref().unwrap_or("unknown");
    MachineFingerprint {
        key: format!(
            "port:{vendor:04x}:{product:04x}:{}:{}",
            identity_token(product_name),
            identity_token(descriptor.port_name.as_deref().unwrap_or(&descriptor.id))
        ),
        confidence: IdentityConfidence::PortBound,
        label: format!(
            "{} · {} · {}",
            product_name,
            firmware,
            descriptor.port_name.as_deref().unwrap_or(&descriptor.label)
        ),
    }
}

pub(super) fn identity_token(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '_'))
        .flat_map(char::to_lowercase)
        .collect()
}

pub(super) fn suggested_machine_name(
    descriptor: &TransportDescriptor,
    inspection: &millo_domain::DeviceInspection,
) -> String {
    let source = descriptor
        .detail
        .as_deref()
        .filter(|value| !matches!(*value, "Serial port" | "Bluetooth serial port"))
        .or(inspection.firmware_build_info.as_deref())
        .or(inspection.firmware_version.as_deref())
        .unwrap_or("GRBL machine");
    let normalized = source.replace(['_', '-'], " ");
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn detected_controller(inspection: &DeviceInspection) -> DetectedController {
    DetectedController {
        firmware_version: inspection.firmware_version.clone(),
        firmware_build_info: inspection.firmware_build_info.clone(),
    }
}
