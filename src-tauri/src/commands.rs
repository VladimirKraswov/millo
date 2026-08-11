use millo_command::CommandArbiter;
use millo_controller::ControllerConfig;
use millo_domain::{
    ControllerSnapshot, HardwareInspection, HardwareProfile, OperatorConfirmation, ResetChallenge,
    TestJogPreparation,
};
use millo_mock::{MockControl, MockTransport};
use millo_serial::{
    SerialConfig, SerialPortDescriptor, SerialPortKind, SerialTransport,
    available_ports as available_serial_ports,
};
use millo_transport::BoxedTransport;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tokio::{sync::Mutex, task::JoinHandle};

const MOCK_TRANSPORT_ID: &str = "mock";
const SERIAL_TRANSPORT_PREFIX: &str = "serial:";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportKind {
    Mock,
    Serial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransportDescriptor {
    pub id: String,
    pub kind: TransportKind,
    pub label: String,
    pub detail: Option<String>,
    pub port_name: Option<String>,
    pub likely_grbl: bool,
    pub match_reason: Option<String>,
}

struct ResolvedTransport {
    transport: BoxedTransport,
    descriptor: TransportDescriptor,
    mock: Option<MockControl>,
}

impl ResolvedTransport {
    fn mock() -> Self {
        let transport = MockTransport::default();
        let mock = transport.control();
        Self {
            transport: Box::new(transport),
            descriptor: mock_descriptor(),
            mock: Some(mock),
        }
    }
}

pub struct AppState {
    arbiter: CommandArbiter,
    active_transport: Mutex<TransportDescriptor>,
    mock: Mutex<Option<MockControl>>,
    transition_lock: Mutex<()>,
    event_task: Mutex<Option<JoinHandle<()>>>,
}

impl Default for AppState {
    fn default() -> Self {
        let initial = ResolvedTransport::mock();
        let descriptor = initial.descriptor;
        let mock = initial.mock;
        let (arbiter, worker) = CommandArbiter::new(
            initial.transport,
            ControllerConfig::default(),
            HardwareProfile::first_machine(),
        );
        tauri::async_runtime::spawn(worker);

        Self {
            arbiter,
            active_transport: Mutex::new(descriptor),
            mock: Mutex::new(mock),
            transition_lock: Mutex::new(()),
            event_task: Mutex::new(None),
        }
    }
}

impl AppState {
    async fn start_event_bridge(&self, app: AppHandle) {
        let mut event_task = self.event_task.lock().await;
        if event_task.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }

        let mut snapshots = self.arbiter.subscribe();
        *event_task = Some(tokio::spawn(async move {
            while snapshots.changed().await.is_ok() {
                let snapshot = snapshots.borrow_and_update().clone();
                let _ = app.emit("machine-state", snapshot);
            }
        }));
    }
}

#[tauri::command]
pub async fn list_transports() -> Result<Vec<TransportDescriptor>, String> {
    let serial_ports = tokio::task::spawn_blocking(available_serial_ports)
        .await
        .map_err(|error| format!("serial discovery task failed: {error}"))?
        .map_err(|error| error.to_string())?;

    let mut transports = vec![mock_descriptor()];
    transports.extend(serial_ports.into_iter().map(serial_descriptor));
    Ok(transports)
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
) -> Result<ControllerSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    state.start_event_bridge(app).await;
    let replacement = resolve_transport(&transport_id, baud_rate).await?;

    state
        .arbiter
        .replace_transport(replacement.transport)
        .await
        .map_err(|error| error.to_string())?;
    *state.active_transport.lock().await = replacement.descriptor;
    *state.mock.lock().await = replacement.mock;

    state
        .arbiter
        .connect()
        .await
        .map_err(|error| error.to_string())?;
    state
        .arbiter
        .refresh_status()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn refresh_status(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    state
        .arbiter
        .refresh_status()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn inspect_device(state: State<'_, AppState>) -> Result<HardwareInspection, String> {
    state
        .arbiter
        .inspect_device()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    let _transition = state.transition_lock.lock().await;
    state
        .arbiter
        .disconnect()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn acknowledge_reset(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    state
        .arbiter
        .acknowledge_reset()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn feed_hold(state: State<'_, AppState>) -> Result<ControllerSnapshot, String> {
    state
        .arbiter
        .feed_hold()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn request_soft_reset(state: State<'_, AppState>) -> Result<ResetChallenge, String> {
    state
        .arbiter
        .request_soft_reset()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn confirm_soft_reset(
    challenge_id: u64,
    state: State<'_, AppState>,
) -> Result<ControllerSnapshot, String> {
    state
        .arbiter
        .confirm_soft_reset(challenge_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn prepare_test_jog(
    confirmation: OperatorConfirmation,
    state: State<'_, AppState>,
) -> Result<TestJogPreparation, String> {
    state
        .arbiter
        .prepare_test_jog(confirmation)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn mock_trigger_reset(state: State<'_, AppState>) -> Result<(), String> {
    active_mock(&state).await?.queue_reset("1.1h");
    Ok(())
}

#[tauri::command]
pub async fn mock_start_run(state: State<'_, AppState>) -> Result<(), String> {
    active_mock(&state)
        .await?
        .set_status("<Run|MPos:1.000,2.000,3.000|WPos:1.000,2.000,3.000|FS:120,0>");
    Ok(())
}

#[tauri::command]
pub async fn mock_trigger_alarm(code: u16, state: State<'_, AppState>) -> Result<(), String> {
    active_mock(&state).await?.queue_alarm(code);
    Ok(())
}

#[tauri::command]
pub async fn mock_clear_alarm(state: State<'_, AppState>) -> Result<(), String> {
    active_mock(&state).await?.clear_alarm();
    Ok(())
}

#[tauri::command]
pub async fn mock_trigger_timeout(state: State<'_, AppState>) -> Result<(), String> {
    let mock = active_mock(&state).await?;
    mock.queue_stall();
    mock.queue_stall();
    Ok(())
}

#[tauri::command]
pub async fn mock_trigger_disconnect(state: State<'_, AppState>) -> Result<(), String> {
    active_mock(&state).await?.queue_disconnect();
    Ok(())
}

async fn active_mock(state: &State<'_, AppState>) -> Result<MockControl, String> {
    state
        .mock
        .lock()
        .await
        .clone()
        .ok_or_else(|| "mock scenarios require the Mock GRBL transport".to_owned())
}

async fn resolve_transport(
    transport_id: &str,
    baud_rate: u32,
) -> Result<ResolvedTransport, String> {
    if transport_id == MOCK_TRANSPORT_ID {
        return Ok(ResolvedTransport::mock());
    }

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
        mock: None,
    })
}

fn mock_descriptor() -> TransportDescriptor {
    TransportDescriptor {
        id: MOCK_TRANSPORT_ID.to_owned(),
        kind: TransportKind::Mock,
        label: "Mock GRBL".to_owned(),
        detail: Some("Deterministic test controller".to_owned()),
        port_name: None,
        likely_grbl: true,
        match_reason: Some("Built-in test controller".to_owned()),
    }
}

fn serial_descriptor(port: SerialPortDescriptor) -> TransportDescriptor {
    let match_reason = grbl_match_reason(&port).map(str::to_owned);
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
        SerialPortKind::Unknown => Some("Serial port".to_owned()),
    };

    TransportDescriptor {
        id: format!("{SERIAL_TRANSPORT_PREFIX}{}", port.port_name),
        kind: TransportKind::Serial,
        label: port.port_name.clone(),
        detail,
        port_name: Some(port.port_name),
        likely_grbl: match_reason.is_some(),
        match_reason,
    }
}

fn grbl_match_reason(port: &SerialPortDescriptor) -> Option<&'static str> {
    if port.kind != SerialPortKind::Usb {
        return None;
    }

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

fn serial_port_name(transport_id: &str) -> Result<&str, String> {
    transport_id
        .strip_prefix(SERIAL_TRANSPORT_PREFIX)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| format!("unknown transport: {transport_id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_transport_id_preserves_the_native_port_name() {
        assert_eq!(
            serial_port_name("serial:/dev/cu.usbserial-1420").unwrap(),
            "/dev/cu.usbserial-1420"
        );
        assert!(serial_port_name("serial:").is_err());
        assert!(serial_port_name("network:localhost").is_err());
    }

    #[test]
    fn usb_descriptor_keeps_device_identity() {
        let descriptor = serial_descriptor(SerialPortDescriptor {
            port_name: "/dev/cu.usbmodem101".to_owned(),
            kind: SerialPortKind::Usb,
            vendor_id: Some(0x2341),
            product_id: Some(0x0043),
            manufacturer: Some("Arduino".to_owned()),
            product: Some("Uno".to_owned()),
            serial_number: None,
        });

        assert_eq!(descriptor.kind, TransportKind::Serial);
        assert_eq!(descriptor.id, "serial:/dev/cu.usbmodem101");
        assert_eq!(descriptor.label, "/dev/cu.usbmodem101");
        assert_eq!(descriptor.detail.as_deref(), Some("Uno"));
        assert!(descriptor.likely_grbl);
        assert_eq!(
            descriptor.match_reason.as_deref(),
            Some("Common CNC USB serial interface")
        );
    }

    #[test]
    fn grbl_filter_rejects_non_usb_and_unidentified_ports() {
        let bluetooth = SerialPortDescriptor {
            port_name: "/dev/cu.Bluetooth-Incoming-Port".to_owned(),
            kind: SerialPortKind::Bluetooth,
            vendor_id: None,
            product_id: None,
            manufacturer: None,
            product: None,
            serial_number: None,
        };
        let unidentified_usb = SerialPortDescriptor {
            port_name: "COM8".to_owned(),
            kind: SerialPortKind::Usb,
            vendor_id: Some(0x9999),
            product_id: Some(0x0001),
            manufacturer: Some("Measurement Devices Inc.".to_owned()),
            product: Some("Lab interface".to_owned()),
            serial_number: None,
        };

        assert_eq!(grbl_match_reason(&bluetooth), None);
        assert_eq!(grbl_match_reason(&unidentified_usb), None);
    }

    #[test]
    fn grbl_filter_accepts_common_bridges_and_explicit_metadata() {
        let ch340 = SerialPortDescriptor {
            port_name: "COM4".to_owned(),
            kind: SerialPortKind::Usb,
            vendor_id: Some(0x1A86),
            product_id: Some(0x7523),
            manufacturer: None,
            product: None,
            serial_number: None,
        };
        let fluidnc = SerialPortDescriptor {
            port_name: "COM6".to_owned(),
            kind: SerialPortKind::Usb,
            vendor_id: Some(0x9999),
            product_id: Some(0x0002),
            manufacturer: None,
            product: Some("FluidNC controller".to_owned()),
            serial_number: None,
        };

        assert_eq!(
            grbl_match_reason(&ch340),
            Some("Known controller or USB-UART vendor")
        );
        assert_eq!(grbl_match_reason(&fluidnc), Some("GRBL/CNC metadata"));
    }
}
