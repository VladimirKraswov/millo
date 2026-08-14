#![cfg(unix)]

use millo_serial::{SerialConfig, SerialTransport, available_ports};
use millo_transport::Transport;
use millo_virtual_controller::VirtualController;

#[tokio::test]
async fn external_controller_uses_serial_discovery_and_physical_jog_timing() {
    let virtual_controller = VirtualController::start().await.unwrap();
    let port_name = virtual_controller
        .port_name()
        .to_string_lossy()
        .into_owned();
    let discovered = available_ports().unwrap();
    let descriptor = discovered
        .iter()
        .find(|port| port.port_name == port_name)
        .unwrap();
    assert_eq!(
        descriptor.product.as_deref(),
        Some("Millo VMC-3 GRBL Controller")
    );

    let mut transport = SerialTransport::new(SerialConfig::new(&port_name, 115_200).unwrap());
    transport.connect().await.unwrap();
    transport.write(b"?").await.unwrap();
    assert!(transport.read_line().await.unwrap().starts_with("<Idle|"));
    transport.write(b"$I\n").await.unwrap();
    assert_eq!(
        transport.read_line().await.unwrap(),
        "[VER:1.1h.20260814:Millo VMC-3]"
    );
    assert!(transport.read_line().await.unwrap().starts_with("[OPT:"));
    assert_eq!(transport.read_line().await.unwrap(), "ok");

    transport.write(b"$J=G91 G21 X10 F600\n").await.unwrap();
    assert_eq!(transport.read_line().await.unwrap(), "ok");

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    transport.write(b"?").await.unwrap();
    let accelerating = transport.read_line().await.unwrap();
    let accelerating_x = status_number(&accelerating, "MPos:");
    let accelerating_feed = status_number(&accelerating, "FS:");
    assert!(accelerating.starts_with("<Jog|"));
    assert!(accelerating_x > 0.0 && accelerating_x < 10.0);
    assert!(accelerating_feed > 0.0 && accelerating_feed < 600.0);

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    transport.write(b"?").await.unwrap();
    let complete = transport.read_line().await.unwrap();
    assert!(complete.starts_with("<Idle|"));
    assert!((status_number(&complete, "MPos:") - 10.0).abs() <= 0.001);
    assert_eq!(status_number(&complete, "FS:"), 0.0);
}

fn status_number(status: &str, marker: &str) -> f64 {
    status
        .split(marker)
        .nth(1)
        .and_then(|value| value.split([',', '|']).next())
        .and_then(|value| value.parse().ok())
        .unwrap_or(f64::NAN)
}
