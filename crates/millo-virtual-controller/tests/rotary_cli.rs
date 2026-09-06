#![cfg(unix)]

use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    time::Duration,
};

use millo_serial::{SerialConfig, SerialTransport};
use millo_transport::Transport;

struct ControllerProcess(Child);

impl Drop for ControllerProcess {
    fn drop(&mut self) {
        // Let the executable drop its discovery registration before PTY reuse.
        unsafe {
            libc::kill(self.0.id() as libc::pid_t, libc::SIGINT);
        }
        for _ in 0..200 {
            if self.0.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn response(transport: &mut SerialTransport) -> String {
    tokio::time::timeout(Duration::from_secs(3), transport.read_line())
        .await
        .unwrap()
        .unwrap()
}

async fn query(transport: &mut SerialTransport) -> String {
    transport.write(b"?").await.unwrap();
    response(transport).await
}

fn position(frame: &str) -> Vec<f64> {
    frame
        .split('|')
        .find_map(|field| field.strip_prefix("MPos:"))
        .unwrap()
        .split(',')
        .map(|value| value.parse().unwrap())
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executable_rotary_option_runs_xyz_a_through_its_own_serial_pty() {
    let mut child = ControllerProcess(
        Command::new(env!("CARGO_BIN_EXE_millo-virtual-controller"))
            .arg("--rotary")
            .stdout(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let mut port = String::new();
    BufReader::new(child.0.stdout.take().unwrap())
        .read_line(&mut port)
        .unwrap();
    let mut serial = SerialTransport::new(SerialConfig::new(port.trim(), 115_200).unwrap());
    serial.connect().await.unwrap();
    serial.write(b"$I\n").await.unwrap();
    let mut identity = Vec::new();
    loop {
        let line = response(&mut serial).await;
        if line == "ok" {
            break;
        }
        identity.push(line);
    }
    assert!(identity.contains(&"[FIRMWARE:MilloVirtual]".to_owned()));
    assert!(identity.contains(&"[AXS:4:XYZA]".to_owned()));
    assert_eq!(position(&query(&mut serial).await), [0.0; 4]);

    serial.write(b"N1 G20 G90 G93 G1 X0.1 A").await.unwrap();
    serial.write(b"90 F30\r\n").await.unwrap();
    assert_eq!(response(&mut serial).await, "ok");
    tokio::time::sleep(Duration::from_millis(300)).await;
    let moving = query(&mut serial).await;
    let p = position(&moving);
    assert!(moving.starts_with("<Run|"), "{moving}");
    assert!(p[0] > 0.0 && p[0] < 2.54 && p[3] > 0.0 && p[3] < 90.0);
    assert!((p[0] / 2.54 - p[3] / 90.0).abs() < 0.001);
    serial.write(b"!").await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    let held = query(&mut serial).await;
    assert!(held.starts_with("<Hold:0|"), "{held}");
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(position(&query(&mut serial).await), position(&held));
    serial.write(b"~").await.unwrap();
    tokio::time::sleep(Duration::from_secs(3)).await;
    let complete = query(&mut serial).await;
    assert!(complete.starts_with("<Idle|"), "{complete}");
    assert_eq!(position(&complete), [2.54, 0.0, 0.0, 90.0]);

    serial
        .write(b"G1 X100 A720 F1\nG1 X200 A1440 F1\nG1 A999")
        .await
        .unwrap();
    assert_eq!(response(&mut serial).await, "ok");
    assert_eq!(response(&mut serial).await, "ok");
    serial.write(b"\x18").await.unwrap();
    assert!(response(&mut serial).await.starts_with("Grbl "));
    serial.write(b"G21 G91 G1 A10 F600\n").await.unwrap();
    assert_eq!(response(&mut serial).await, "ok");
    tokio::time::sleep(Duration::from_secs(2)).await;
    let after_reset = query(&mut serial).await;
    assert!(after_reset.starts_with("<Idle|"), "{after_reset}");
    let p = position(&after_reset);
    assert!((p[3] - 100.0).abs() < 1.0, "{after_reset}");
    serial.disconnect().await.unwrap();
}
