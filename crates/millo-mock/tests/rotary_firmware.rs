use std::time::Duration;

use millo_mock::MockTransport;
use millo_transport::Transport;

async fn rotary() -> MockTransport {
    let mut transport = MockTransport::rotary();
    transport.connect().await.unwrap();
    transport
}

async fn send(transport: &mut MockTransport, command: &str, expected: &str) {
    transport
        .write(format!("{command}\n").as_bytes())
        .await
        .unwrap();
    assert_eq!(transport.read_line().await.unwrap(), expected, "{command}");
}

async fn query(transport: &mut MockTransport, command: &str) -> Vec<String> {
    transport
        .write(format!("{command}\n").as_bytes())
        .await
        .unwrap();
    let mut lines = Vec::new();
    loop {
        let line = transport.read_line().await.unwrap();
        if line == "ok" {
            return lines;
        }
        assert!(!line.starts_with("error:"), "{command}: {line}");
        lines.push(line);
    }
}

async fn status(transport: &mut MockTransport, seconds: f64) -> String {
    transport
        .control()
        .advance_program(Duration::from_secs_f64(seconds));
    transport.write(b"?").await.unwrap();
    transport.read_line().await.unwrap()
}

fn position(frame: &str, name: &str) -> Vec<f64> {
    frame
        .trim_end_matches('>')
        .split('|')
        .find_map(|field| field.strip_prefix(&format!("{name}:")))
        .unwrap()
        .split(',')
        .map(|value| value.parse().unwrap())
        .collect()
}

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected} +/- {tolerance}"
    );
}

#[tokio::test]
async fn default_mock_never_silently_accepts_an_a_word() {
    let mut transport = MockTransport::default();
    transport.connect().await.unwrap();
    for enabled in [true, false] {
        transport.control().set_virtual_motion_enabled(enabled);
        for command in [
            "G1 X10 A90 F60",
            "g91g1a-30f360",
            "$J=G91 G21 A5 F360",
            "G10 L20 P1 A0",
            "G0 A0",
            "G1 B1 F60",
        ] {
            send(&mut transport, command, "error:20").await;
        }
        assert_eq!(
            position(&status(&mut transport, 20.0).await, "MPos"),
            [0.0; 3]
        );
    }
    send(&mut transport, "G21 (A90) ; A180", "ok").await;
    assert!(
        !query(&mut transport, "$I")
            .await
            .iter()
            .any(|line| line.starts_with("[AXS:"))
    );
    assert!(
        query(&mut transport, "$#")
            .await
            .contains(&"[G54:0.000,0.000,0.000]".to_owned())
    );
}

#[tokio::test]
async fn rotary_identity_and_parameter_vectors_exist_before_any_jog() {
    let mut transport = rotary().await;
    assert!(
        query(&mut transport, "$I")
            .await
            .contains(&"[FIRMWARE:MilloVirtual]".to_owned())
    );
    assert!(
        query(&mut transport, "$I")
            .await
            .contains(&"[AXS:4:XYZA]".to_owned())
    );
    assert_eq!(
        position(&status(&mut transport, 0.0).await, "MPos"),
        [0.0; 4]
    );
    let parameters = query(&mut transport, "$#").await;
    for expected in [
        "[G54:0.000,0.000,0.000,0.000]",
        "[G92:0.000,0.000,0.000,0.000]",
        "[PRB:0.000,0.000,0.000,0.000:0]",
    ] {
        assert!(parameters.contains(&expected.to_owned()), "{parameters:?}");
    }
    let modal = query(&mut transport, "$G").await;
    assert!(modal[0].contains("G21 G90 G94"));
    assert_eq!(query(&mut transport, "$GC").await, modal);
}

#[tokio::test]
async fn queued_xyz_a_programs_obey_absolute_incremental_and_angular_inches() {
    let mut transport = rotary().await;
    send(&mut transport, "N1 G90 G21 G1 X10 Y5 Z-2 A90 F600", "ok").await;
    send(&mut transport, "N2 G91 X2 Y-1 Z1 A-30", "ok").await;
    send(&mut transport, "N3 F60 G20 X1 A720", "ok").await;
    let frame = status(&mut transport, 100.0).await;
    assert!(frame.starts_with("<Idle|"), "{frame}");
    assert!(frame.contains("Ln:3"));
    assert_eq!(position(&frame, "MPos"), [37.4, 4.0, -1.0, 780.0]);
    send(&mut transport, "G90 G0 A-450", "ok").await;
    assert_eq!(
        position(&status(&mut transport, 100.0).await, "MPos"),
        [37.4, 4.0, -1.0, -450.0]
    );
}

#[tokio::test]
async fn inverse_time_coordinates_all_axes_and_is_not_scaled_by_g20() {
    let mut transport = rotary().await;
    send(&mut transport, "F2 G20 G90 G93 G1 X1 A180", "ok").await;
    let halfway = status(&mut transport, 15.0).await;
    assert!(halfway.starts_with("<Run|"));
    let p = position(&halfway, "MPos");
    close(p[0], 12.7, 0.05);
    close(p[3], 90.0, 0.3);
    close(p[0] / 25.4, p[3] / 180.0, 0.0001);
    let modal = query(&mut transport, "$G").await;
    assert!(modal[0].contains("G20 G90 G93"));
    assert!(modal[0].contains("F2.000"));
    let done = status(&mut transport, 15.5).await;
    assert!(done.starts_with("<Idle|"));
    assert_eq!(position(&done, "MPos"), [25.4, 0.0, 0.0, 180.0]);
}

#[tokio::test]
async fn feed_validation_is_atomic_including_check_mode() {
    let mut transport = rotary().await;
    send(&mut transport, "G21 G90 G94 G1 X1 A10 F60", "ok").await;
    let before = query(&mut transport, "$G").await;
    for (command, error) in [
        ("G20 G91 G93 G1 X1 A90", "error:22"),
        ("G93 G1 A90 F0", "error:22"),
        ("G93 G1 A90 F-1", "error:4"),
        ("G1 A90 A180 F60", "error:25"),
        ("G2 X1 A90 I1 F60", "error:20"),
        ("G92 A90", "error:20"),
    ] {
        send(&mut transport, command, error).await;
        assert_eq!(query(&mut transport, "$G").await, before);
    }
    assert_eq!(
        position(&status(&mut transport, 10.0).await, "MPos"),
        [1.0, 0.0, 0.0, 10.0]
    );
    send(&mut transport, "G93 G1 A20 F2", "ok").await;
    send(&mut transport, "G1 A30", "error:22").await;
    send(&mut transport, "G94 G1 A30", "error:22").await;
    send(&mut transport, "G94", "ok").await;
    send(&mut transport, "G1 A30", "error:22").await;
    assert_eq!(
        position(&status(&mut transport, 31.0).await, "MPos")[3],
        20.0
    );
    send(&mut transport, "$C", "ok").await;
    send(&mut transport, "G93 G1 A90", "error:22").await;
    send(&mut transport, "G93 G1 X20 A90 F2", "ok").await;
    let checked = status(&mut transport, 60.0).await;
    assert!(checked.starts_with("<Check|"));
    assert_eq!(position(&checked, "MPos"), [1.0, 0.0, 0.0, 20.0]);
}

#[tokio::test]
async fn g94_mixed_motion_uses_linear_length_and_pure_a_uses_degrees() {
    let mut transport = rotary().await;
    send(&mut transport, "G94 G1 X10 A360 F60", "ok").await;
    let p = position(&status(&mut transport, 5.0).await, "MPos");
    close(p[0], 5.0, 0.1);
    close(p[3], 180.0, 3.0);
    close(p[0] / 10.0, p[3] / 360.0, 0.0001);
    let frame = status(&mut transport, 5.5).await;
    assert!(frame.starts_with("<Idle|"));
    assert_eq!(position(&frame, "MPos"), [10.0, 0.0, 0.0, 360.0]);
    for units in ["G20", "G21"] {
        send(&mut transport, &format!("{units} G91 G1 A360 F360"), "ok").await;
        let start = if units == "G20" { 360.0 } else { 720.0 };
        close(
            position(&status(&mut transport, 30.0).await, "MPos")[3],
            start + 180.0,
            0.2,
        );
        let done = status(&mut transport, 30.5).await;
        assert!(done.starts_with("<Idle|"));
        assert_eq!(position(&done, "MPos")[3], start + 360.0);
    }
}

#[tokio::test]
async fn a_rate_limits_slow_the_entire_coordinated_move() {
    let mut transport = rotary().await;
    transport.control().set_setting(113, "60");
    send(&mut transport, "G93 G1 X10 A360 F60", "ok").await;
    let p = position(&status(&mut transport, 60.0).await, "MPos");
    close(p[3], 60.0, 0.1);
    close(p[0] / 10.0, p[3] / 360.0, 0.0001);
    assert!(status(&mut transport, 1.0).await.starts_with("<Run|"));
    assert_eq!(
        position(&status(&mut transport, 310.0).await, "MPos"),
        [10.0, 0.0, 0.0, 360.0]
    );
}

#[tokio::test]
async fn hold_resume_and_reset_keep_xyz_and_a_together_and_flush_the_queue() {
    let mut transport = rotary().await;
    send(&mut transport, "G93 G1 X100 A360 F1", "ok").await;
    send(&mut transport, "G1 X200 A720 F1", "ok").await;
    let moving = position(&status(&mut transport, 10.0).await, "MPos");
    transport.write(b"!").await.unwrap();
    let held = status(&mut transport, 1.0).await;
    assert!(held.starts_with("<Hold:0|"), "{held}");
    let p = position(&held, "MPos");
    assert!(p[0] >= moving[0] && p[3] >= moving[3]);
    close(p[0] / 100.0, p[3] / 360.0, 0.0001);
    assert_eq!(position(&status(&mut transport, 100.0).await, "MPos"), p);
    transport.write(b"~").await.unwrap();
    let resumed = position(&status(&mut transport, 5.0).await, "MPos");
    assert!(resumed[0] > p[0] && resumed[3] > p[3]);
    transport.write(b"\x18").await.unwrap();
    assert!(transport.read_line().await.unwrap().starts_with("Grbl "));
    let reset = status(&mut transport, 0.0).await;
    assert!(reset.starts_with("<Idle|"));
    let stopped = position(&reset, "MPos");
    assert_eq!(
        position(&status(&mut transport, 1000.0).await, "MPos"),
        stopped
    );
    assert!(query(&mut transport, "$G").await[0].contains("G21 G90 G94"));
    send(&mut transport, "G91 G1 A10 F360", "ok").await;
    close(
        position(&status(&mut transport, 10.0).await, "MPos")[3],
        stopped[3] + 10.0,
        0.002,
    );
}

#[tokio::test]
async fn work_offsets_report_angular_values_and_apply_to_absolute_a() {
    let mut transport = rotary().await;
    send(&mut transport, "G0 A90", "ok").await;
    status(&mut transport, 10.0).await;
    send(&mut transport, "G10 L20 P1 A10", "ok").await;
    let frame = status(&mut transport, 0.0).await;
    assert_eq!(position(&frame, "MPos")[3], 90.0);
    assert_eq!(position(&frame, "WPos")[3], 10.0);
    assert!(
        query(&mut transport, "$#")
            .await
            .contains(&"[G54:0.000,0.000,0.000,80.000]".to_owned())
    );
    send(&mut transport, "G90 G20 G1 A0 F360", "ok").await;
    assert_eq!(
        position(&status(&mut transport, 10.0).await, "MPos")[3],
        80.0
    );
    send(&mut transport, "G53 G0 A0", "ok").await;
    assert_eq!(
        position(&status(&mut transport, 10.0).await, "MPos")[3],
        0.0
    );
    transport.write(b"\x18").await.unwrap();
    transport.read_line().await.unwrap();
    assert_eq!(
        position(&status(&mut transport, 0.0).await, "WPos")[3],
        -80.0
    );
    send(&mut transport, "G0 A0", "ok").await;
    assert_eq!(
        position(&status(&mut transport, 10.0).await, "MPos")[3],
        80.0
    );
}

#[tokio::test]
async fn rotary_jog_is_timed_and_cancel_does_not_cancel_a_program() {
    let mut transport = rotary().await;
    send(&mut transport, "$J=G91 G21 A360 F360", "ok").await;
    let jogging = status(&mut transport, 1.0).await;
    assert!(jogging.starts_with("<Jog|"));
    close(position(&jogging, "MPos")[3], 6.0, 0.2);
    transport.write(&[0x85]).await.unwrap();
    let cancelled = status(&mut transport, 1.0).await;
    assert!(cancelled.starts_with("<Idle|"));
    let p = position(&cancelled, "MPos");
    assert_eq!(position(&status(&mut transport, 100.0).await, "MPos"), p);
    send(&mut transport, "G90 G1 X10 A90 F60", "ok").await;
    transport.write(&[0x85]).await.unwrap();
    assert_eq!(
        position(&status(&mut transport, 20.0).await, "MPos"),
        [10.0, 0.0, 0.0, 90.0]
    );
}
