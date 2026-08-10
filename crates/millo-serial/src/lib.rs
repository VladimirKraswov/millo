use std::collections::BTreeMap;

use async_trait::async_trait;
use millo_transport::{Transport, TransportError};
use thiserror::Error;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio_serial::{SerialPortBuilderExt, SerialPortType, SerialStream};

type SerialReader = BufReader<ReadHalf<SerialStream>>;
type SerialWriter = WriteHalf<SerialStream>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialConfig {
    pub port_name: String,
    pub baud_rate: u32,
}

impl SerialConfig {
    pub fn new(port_name: impl Into<String>, baud_rate: u32) -> Result<Self, SerialConfigError> {
        let port_name = port_name.into();
        if port_name.trim().is_empty() {
            return Err(SerialConfigError::EmptyPortName);
        }
        if baud_rate == 0 {
            return Err(SerialConfigError::InvalidBaudRate(baud_rate));
        }

        Ok(Self {
            port_name,
            baud_rate,
        })
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SerialConfigError {
    #[error("serial port name must not be empty")]
    EmptyPortName,
    #[error("serial baud rate must be greater than zero, got {0}")]
    InvalidBaudRate(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerialPortKind {
    Usb,
    Bluetooth,
    Pci,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerialPortDescriptor {
    pub port_name: String,
    pub kind: SerialPortKind,
    pub vendor_id: Option<u16>,
    pub product_id: Option<u16>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub serial_number: Option<String>,
}

pub fn available_ports() -> Result<Vec<SerialPortDescriptor>, TransportError> {
    let ports = tokio_serial::available_ports()
        .map_err(|error| TransportError::Io(error.to_string()))?
        .into_iter()
        .map(|port| {
            let mut descriptor = SerialPortDescriptor {
                port_name: port.port_name,
                kind: SerialPortKind::Unknown,
                vendor_id: None,
                product_id: None,
                manufacturer: None,
                product: None,
                serial_number: None,
            };

            match port.port_type {
                SerialPortType::UsbPort(info) => {
                    descriptor.kind = SerialPortKind::Usb;
                    descriptor.vendor_id = Some(info.vid);
                    descriptor.product_id = Some(info.pid);
                    descriptor.manufacturer = info.manufacturer;
                    descriptor.product = info.product;
                    descriptor.serial_number = info.serial_number;
                }
                SerialPortType::BluetoothPort => {
                    descriptor.kind = SerialPortKind::Bluetooth;
                }
                SerialPortType::PciPort => {
                    descriptor.kind = SerialPortKind::Pci;
                }
                SerialPortType::Unknown => {}
            }

            descriptor
        })
        .collect::<Vec<_>>();
    Ok(deduplicate_native_ports(ports))
}

fn deduplicate_native_ports(ports: Vec<SerialPortDescriptor>) -> Vec<SerialPortDescriptor> {
    let mut unique = BTreeMap::<String, SerialPortDescriptor>::new();

    for port in ports {
        let key = native_device_key(&port.port_name);
        match unique.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(port);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if native_port_priority(&port.port_name)
                    < native_port_priority(&entry.get().port_name)
                {
                    entry.insert(port);
                }
            }
        }
    }

    let mut ports = unique.into_values().collect::<Vec<_>>();
    ports.sort_by(|left, right| left.port_name.cmp(&right.port_name));
    ports
}

fn native_device_key(port_name: &str) -> String {
    port_name
        .strip_prefix("/dev/cu.")
        .or_else(|| port_name.strip_prefix("/dev/tty."))
        .map(|device| format!("macos:{device}"))
        .unwrap_or_else(|| format!("native:{port_name}"))
}

fn native_port_priority(port_name: &str) -> u8 {
    if port_name.starts_with("/dev/cu.") {
        0
    } else if port_name.starts_with("/dev/tty.") {
        1
    } else {
        0
    }
}

pub struct SerialTransport {
    config: SerialConfig,
    reader: Option<SerialReader>,
    writer: Option<SerialWriter>,
}

impl SerialTransport {
    pub fn new(config: SerialConfig) -> Self {
        Self {
            config,
            reader: None,
            writer: None,
        }
    }

    pub fn config(&self) -> &SerialConfig {
        &self.config
    }
}

#[async_trait]
impl Transport for SerialTransport {
    async fn connect(&mut self) -> Result<(), TransportError> {
        self.reader = None;
        self.writer = None;

        let stream = tokio_serial::new(&self.config.port_name, self.config.baud_rate)
            .open_native_async()
            .map_err(|error| TransportError::Io(error.to_string()))?;
        let (reader, writer) = tokio::io::split(stream);
        self.reader = Some(BufReader::new(reader));
        self.writer = Some(writer);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        self.reader = None;
        self.writer = None;
        Ok(())
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let writer = self.writer.as_mut().ok_or(TransportError::NotConnected)?;
        writer
            .write_all(data)
            .await
            .map_err(|error| TransportError::Io(error.to_string()))?;
        writer
            .flush()
            .await
            .map_err(|error| TransportError::Io(error.to_string()))
    }

    async fn read_line(&mut self) -> Result<String, TransportError> {
        let reader = self.reader.as_mut().ok_or(TransportError::NotConnected)?;
        read_serial_line(reader).await
    }

    fn is_connected(&self) -> bool {
        self.reader.is_some() && self.writer.is_some()
    }
}

async fn read_serial_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
) -> Result<String, TransportError> {
    let mut bytes = Vec::new();
    let count = reader
        .read_until(b'\n', &mut bytes)
        .await
        .map_err(|error| TransportError::Io(error.to_string()))?;
    if count == 0 {
        return Err(TransportError::NotConnected);
    }

    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes.pop();
    }
    String::from_utf8(bytes).map_err(|error| TransportError::Io(error.to_string()))
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncWriteExt, BufReader};

    use super::*;

    #[test]
    fn validates_serial_configuration() {
        assert_eq!(
            SerialConfig::new("  ", 115_200).unwrap_err(),
            SerialConfigError::EmptyPortName
        );
        assert_eq!(
            SerialConfig::new("/dev/tty.test", 0).unwrap_err(),
            SerialConfigError::InvalidBaudRate(0)
        );
        assert_eq!(
            SerialConfig::new("/dev/tty.test", 115_200).unwrap(),
            SerialConfig {
                port_name: "/dev/tty.test".to_owned(),
                baud_rate: 115_200,
            }
        );
    }

    #[tokio::test]
    async fn reads_a_fragmented_crlf_line() {
        let (mut device, host) = tokio::io::duplex(64);
        let writer = tokio::spawn(async move {
            device.write_all(b"<Idle|MPos:0").await.unwrap();
            tokio::task::yield_now().await;
            device.write_all(b",0,0>\r\n").await.unwrap();
        });
        let mut reader = BufReader::new(host);

        let line = read_serial_line(&mut reader).await.unwrap();

        writer.await.unwrap();
        assert_eq!(line, "<Idle|MPos:0,0,0>");
    }

    #[tokio::test]
    async fn treats_end_of_stream_as_a_disconnection() {
        let (device, host) = tokio::io::duplex(16);
        drop(device);
        let mut reader = BufReader::new(host);

        assert_eq!(
            read_serial_line(&mut reader).await.unwrap_err(),
            TransportError::NotConnected
        );
    }

    #[tokio::test]
    async fn rejects_io_before_connect() {
        let config = SerialConfig::new("/dev/tty.test", 115_200).unwrap();
        let mut transport = SerialTransport::new(config);

        assert_eq!(
            transport.write(b"?").await.unwrap_err(),
            TransportError::NotConnected
        );
        assert_eq!(
            transport.read_line().await.unwrap_err(),
            TransportError::NotConnected
        );
    }

    #[test]
    fn collapses_macos_callout_and_tty_paths_to_the_callout_port() {
        let ports = vec![
            test_port("/dev/tty.usbmodem11101"),
            test_port("/dev/cu.usbmodem11101"),
            test_port("/dev/cu.usbserial-210"),
        ];

        let unique = deduplicate_native_ports(ports);

        assert_eq!(
            unique
                .iter()
                .map(|port| port.port_name.as_str())
                .collect::<Vec<_>>(),
            vec!["/dev/cu.usbmodem11101", "/dev/cu.usbserial-210"]
        );
    }

    #[test]
    fn preserves_unpaired_and_non_macos_port_names() {
        let ports = vec![
            test_port("/dev/tty.usbmodem-without-callout"),
            test_port("COM4"),
            test_port("COM5"),
        ];

        let unique = deduplicate_native_ports(ports);

        assert_eq!(unique.len(), 3);
        assert!(
            unique
                .iter()
                .any(|port| port.port_name == "/dev/tty.usbmodem-without-callout")
        );
        assert!(unique.iter().any(|port| port.port_name == "COM4"));
        assert!(unique.iter().any(|port| port.port_name == "COM5"));
    }

    fn test_port(port_name: &str) -> SerialPortDescriptor {
        SerialPortDescriptor {
            port_name: port_name.to_owned(),
            kind: SerialPortKind::Usb,
            vendor_id: Some(0x2341),
            product_id: Some(0x0043),
            manufacturer: Some("Test vendor".to_owned()),
            product: Some("Test device".to_owned()),
            serial_number: None,
        }
    }
}
