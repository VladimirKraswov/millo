use super::*;

#[tauri::command]
pub async fn list_transports() -> Result<Vec<TransportDescriptor>, String> {
    let serial_ports = tokio::task::spawn_blocking(available_serial_ports)
        .await
        .map_err(|error| format!("serial discovery task failed: {error}"))?
        .map_err(|error| error.to_string())?;

    Ok(serial_ports.into_iter().map(serial_descriptor).collect())
}

#[tauri::command]
pub async fn active_transport(state: State<'_, AppState>) -> Result<TransportDescriptor, String> {
    Ok(state.active_transport.lock().await.clone())
}

#[tauri::command]
pub async fn controller_snapshot(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    Ok(state.arbiter.snapshot())
}

#[tauri::command]
pub async fn connect_transport(
    transport_id: String,
    baud_rate: u32,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ConnectOutcome, String> {
    let _transition = state.transition_lock.lock().await;
    state.audit.record(
        AuditLevel::Info,
        AuditCategory::Transport,
        "transport.connect.requested",
        "Controller connection requested",
        json!({ "transportId": &transport_id, "baudRate": baud_rate }),
    );
    state.start_event_bridge(app).await;
    let replacement = match resolve_transport(&transport_id, baud_rate).await {
        Ok(replacement) => replacement,
        Err(error) => {
            state.audit.record(
                AuditLevel::Error,
                AuditCategory::Transport,
                "transport.resolve.failed",
                &error,
                json!({ "transportId": &transport_id, "baudRate": baud_rate }),
            );
            return Err(error);
        }
    };
    let descriptor = replacement.descriptor.clone();

    if let Err(error) = state
        .arbiter
        .replace_transport_with_execution_target(
            replacement.transport,
            replacement.execution_target,
        )
        .await
        .map_err(|error| error.to_string())
    {
        state.audit.record(
            AuditLevel::Error,
            AuditCategory::Transport,
            "transport.replace.failed",
            &error,
            json!({ "transportId": &transport_id, "baudRate": baud_rate }),
        );
        return Err(error);
    }
    *state.settings_session.lock().await = None;
    *state.active_transport.lock().await = descriptor.clone();

    let result = async {
        state
            .arbiter
            .connect()
            .await
            .map_err(|error| error.to_string())?;
        let snapshot = state
            .arbiter
            .refresh_status()
            .await
            .map_err(|error| error.to_string())?;
        state
            .arbiter
            .bind_hardware_profile(HardwareProfile::first_machine())
            .await
            .map_err(|error| error.to_string())?;
        let initial_inspection = state
            .arbiter
            .inspect_device()
            .await
            .map_err(|error| error.to_string())?;
        let fingerprint = machine_fingerprint(&descriptor, &initial_inspection.device);
        let connection = MachineConnectionPreset {
            transport_id: descriptor.id.clone(),
            baud_rate,
            fingerprint: Some(fingerprint.clone()),
        };
        let profile_match = {
            let profiles = state.profiles.lock().await.state();
            match_machine_profile(
                &profiles,
                &fingerprint,
                &descriptor.id,
                &initial_inspection.device,
            )?
        };
        let mut profile_id = None;
        let mut archive = None;
        if let Some(profile) = profile_match.as_ref() {
            state
                .arbiter
                .bind_hardware_profile(profile.hardware_profile())
                .await
                .map_err(|error| error.to_string())?;
            let travel = build_settings_snapshot(&initial_inspection.device, 1)
                .travel_mm()
                .ok_or_else(|| {
                    "controller did not report valid $130/$131/$132 travel".to_owned()
                })?;
            let profiles = state
                .profiles
                .lock()
                .await
                .record_controller_observation(
                    &profile.id,
                    travel,
                    connection.clone(),
                    detected_controller(&initial_inspection.device),
                )
                .map_err(|error| error.to_string())?;
            let refreshed_profile = profiles
                .profiles
                .iter()
                .find(|candidate| candidate.id == profile.id)
                .ok_or_else(|| "observed profile disappeared from the profile store".to_owned())?;
            let temporary_session = ActiveControllerSettings {
                inspection: initial_inspection.device.clone(),
                fingerprint: fingerprint.clone(),
                connection: connection.clone(),
                profile_id: Some(profile.id.clone()),
                archive: None,
                revision: 1,
            };
            archive = begin_settings_archive(&state, refreshed_profile, &temporary_session)?;
            profile_id = Some(profile.id.clone());
        }

        let inspection = if profile_id.is_some() {
            state
                .arbiter
                .inspect_device()
                .await
                .map_err(|error| error.to_string())?
        } else {
            initial_inspection
        };
        let onboarding_draft = if descriptor.kind == TransportKind::Serial && profile_id.is_none() {
            Some(
                MachineProfileDraft::from_grbl_inspection(
                    suggested_machine_name(&descriptor, &inspection.device),
                    &inspection.device,
                    connection.clone(),
                )
                .map_err(|error| error.to_string())?,
            )
        } else {
            None
        };
        if let Some(settings_archive) = archive.as_mut() {
            settings_archive
                .record_observation(&inspection.device)
                .map_err(|error| error.to_string())?;
        }
        let active = ActiveControllerSettings {
            inspection: inspection.device.clone(),
            fingerprint,
            connection,
            profile_id,
            archive,
            revision: 1,
        };
        let settings = settings_state(&active);
        *state.settings_session.lock().await = Some(active);
        Ok(ConnectOutcome {
            snapshot,
            inspection,
            settings,
            profiles: state.profiles.lock().await.state(),
            onboarding_draft,
        })
    }
    .await;

    match result {
        Ok(outcome) => {
            state.audit.record(
                AuditLevel::Info,
                AuditCategory::Transport,
                "transport.connect.completed",
                "Controller connected and synchronized",
                json!({
                    "transport": descriptor,
                    "firmwareVersion": &outcome.inspection.device.firmware_version,
                    "firmwareBuildInfo": &outcome.inspection.device.firmware_build_info,
                    "profileId": &outcome.settings.profile_id,
                    "machineMode": outcome.snapshot.machine.mode,
                }),
            );
            Ok(outcome)
        }
        Err(connection_error) => {
            *state.settings_session.lock().await = None;
            match state.arbiter.disconnect().await {
                Ok(_) => {
                    state.audit.record(
                        AuditLevel::Error,
                        AuditCategory::Transport,
                        "transport.connect.failed",
                        &connection_error,
                        json!({ "transport": descriptor }),
                    );
                    Err(connection_error)
                }
                Err(cleanup_error) => {
                    let error = format!(
                        "{connection_error}; connection cleanup also failed: {cleanup_error}"
                    );
                    state.audit.record(
                        AuditLevel::Critical,
                        AuditCategory::Transport,
                        "transport.connect_cleanup.failed",
                        &error,
                        json!({ "transport": descriptor }),
                    );
                    Err(error)
                }
            }
        }
    }
}

#[tauri::command]
pub async fn refresh_status(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    let result = state
        .arbiter
        .refresh_status()
        .await
        .map_err(|error| error.to_string());
    audit_operation(
        &state.audit,
        AuditCategory::Controller,
        "controller.status_refresh",
        "Fresh GRBL status received",
        Value::Null,
        &result,
    );
    result
}

#[tauri::command]
pub async fn inspect_device(state: State<'_, AppState>) -> Result<HardwareInspection, String> {
    let inspection = match state
        .arbiter
        .inspect_device()
        .await
        .map_err(|error| error.to_string())
    {
        Ok(inspection) => inspection,
        Err(error) => {
            state.audit.record(
                AuditLevel::Error,
                AuditCategory::Controller,
                "controller.inspection.failed",
                &error,
                Value::Null,
            );
            return Err(error);
        }
    };
    if let Some(active) = state.settings_session.lock().await.as_mut() {
        active.inspection = inspection.device.clone();
        active.revision = active.revision.saturating_add(1);
        if let Some(archive) = active.archive.as_mut() {
            archive
                .record_observation(&inspection.device)
                .map_err(|error| error.to_string())?;
        }
    }
    state.audit.record(
        AuditLevel::Info,
        AuditCategory::Controller,
        "controller.inspection.completed",
        "GRBL identity, settings, modal state, and coordinates synchronized",
        serde_json::to_value(&inspection).unwrap_or(Value::Null),
    );
    Ok(inspection)
}

#[tauri::command]
pub async fn execute_operator_console(
    command: String,
    state: State<'_, AppState>,
) -> Result<OperatorConsoleExchange, String> {
    let _transition = state.transition_lock.lock().await;
    let context = json!({ "command": command.trim() });
    let policy = if state
        .preferences
        .lock()
        .await
        .preferences()
        .safe_command_mode
    {
        OperatorConsolePolicy::SafeOnly
    } else {
        OperatorConsolePolicy::Expert
    };
    let result = state
        .arbiter
        .execute_operator_console(command, policy)
        .await
        .map_err(|error| error.to_string());
    match &result {
        Ok(exchange) if exchange.completion == CommandCompletion::Ok => state.audit.record(
            AuditLevel::Info,
            AuditCategory::Controller,
            "controller.operator_console.completed",
            if exchange.kind == millo_domain::OperatorConsoleCommandKind::Raw {
                "Expert operator console command completed"
            } else {
                "Read-only operator console query completed"
            },
            context,
        ),
        Ok(exchange) => state.audit.record(
            AuditLevel::Warning,
            AuditCategory::Controller,
            "controller.operator_console.rejected",
            if exchange.kind == millo_domain::OperatorConsoleCommandKind::Raw {
                "GRBL rejected an expert operator console command"
            } else {
                "GRBL rejected a read-only operator console query"
            },
            json!({
                "context": context,
                "completion": exchange.completion,
                "code": exchange.code,
            }),
        ),
        Err(error) => state.audit.record(
            AuditLevel::Warning,
            AuditCategory::Controller,
            "controller.operator_console.blocked",
            error,
            json!({ "context": context }),
        ),
    };
    result
}

pub(super) async fn resolve_transport(
    transport_id: &str,
    baud_rate: u32,
) -> Result<ResolvedTransport, String> {
    let port_name = serial_port_name(transport_id)?;
    let available = tokio::task::spawn_blocking(available_serial_ports)
        .await
        .map_err(|error| format!("serial discovery task failed: {error}"))?
        .map_err(|error| error.to_string())?;
    let port = available
        .into_iter()
        .find(|port| port.port_name == port_name)
        .ok_or_else(|| format!("serial port is no longer available: {port_name}"))?;
    let config =
        SerialConfig::new(&port.port_name, baud_rate).map_err(|error| error.to_string())?;

    Ok(ResolvedTransport {
        transport: Box::new(SerialTransport::new(config)),
        descriptor: serial_descriptor(port),
        execution_target: ExecutionTarget::Serial,
    })
}

pub(super) fn disconnected_descriptor() -> TransportDescriptor {
    TransportDescriptor {
        id: String::new(),
        kind: TransportKind::Serial,
        label: "Serial controller".to_owned(),
        detail: None,
        port_name: None,
        likely_grbl: false,
        match_reason: None,
        vendor_id: None,
        product_id: None,
        manufacturer: None,
        product: None,
        serial_number: None,
    }
}

pub(super) fn serial_descriptor(port: SerialPortDescriptor) -> TransportDescriptor {
    let match_reason = grbl_match_reason(&port).map(str::to_owned);
    let label = port.product.as_ref().map_or_else(
        || port.port_name.clone(),
        |product| format!("{product} · {}", port.port_name),
    );
    let detail = match port.kind {
        SerialPortKind::Usb => port
            .product
            .clone()
            .or(port.manufacturer.clone())
            .or_else(|| {
                Some(format!(
                    "USB {:04X}:{:04X}",
                    port.vendor_id.unwrap_or_default(),
                    port.product_id.unwrap_or_default()
                ))
            }),
        SerialPortKind::Bluetooth => Some("Bluetooth serial port".to_owned()),
        SerialPortKind::Pci => Some("PCI serial port".to_owned()),
        SerialPortKind::Unknown => port
            .manufacturer
            .clone()
            .or_else(|| Some("Serial port".to_owned())),
    };

    TransportDescriptor {
        id: format!("{SERIAL_TRANSPORT_PREFIX}{}", port.port_name),
        kind: TransportKind::Serial,
        label,
        detail,
        port_name: Some(port.port_name),
        likely_grbl: match_reason.is_some(),
        match_reason,
        vendor_id: port.vendor_id,
        product_id: port.product_id,
        manufacturer: port.manufacturer,
        product: port.product,
        serial_number: port.serial_number,
    }
}

pub(super) fn grbl_match_reason(port: &SerialPortDescriptor) -> Option<&'static str> {
    let searchable = [
        Some(port.port_name.as_str()),
        port.manufacturer.as_deref(),
        port.product.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
    .to_ascii_lowercase();

    if ["grbl", "fluidnc", "cnc", "woodpecker", "xpro"]
        .iter()
        .any(|needle| searchable.contains(needle))
    {
        return Some("GRBL/CNC metadata");
    }

    if port.kind != SerialPortKind::Usb {
        return None;
    }

    if [
        "arduino",
        "usbserial",
        "usbmodem",
        "ch340",
        "ch341",
        "cp210",
        "ftdi",
        "usb serial",
        "usb2.0-serial",
    ]
    .iter()
    .any(|needle| searchable.contains(needle))
    {
        return Some("Common CNC USB serial interface");
    }

    match port.vendor_id {
        Some(0x0403 | 0x10C4 | 0x1A86 | 0x2341 | 0x2A03 | 0x303A) => {
            Some("Known controller or USB-UART vendor")
        }
        _ => None,
    }
}

pub(super) fn serial_port_name(transport_id: &str) -> Result<&str, String> {
    transport_id
        .strip_prefix(SERIAL_TRANSPORT_PREFIX)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("unknown transport: {transport_id}"))
}
