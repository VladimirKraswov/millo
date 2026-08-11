use std::{
    collections::VecDeque,
    future::pending,
    sync::{Arc, Mutex, MutexGuard},
};

use async_trait::async_trait;
use millo_transport::{Transport, TransportError};

const DEFAULT_STATUS: &str = "<Idle|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:0,0>";

#[derive(Debug, Clone)]
enum MockRead {
    Line(String),
    Stall,
    Disconnect,
}

#[derive(Debug)]
struct MockState {
    connected: bool,
    status_line: String,
    planned_cycles: VecDeque<VecDeque<MockRead>>,
    planned_queries: VecDeque<VecDeque<MockRead>>,
    active_reads: VecDeque<MockRead>,
    writes: Vec<Vec<u8>>,
    jog_polls_remaining: u32,
}

#[derive(Debug, Clone)]
pub struct MockControl {
    state: Arc<Mutex<MockState>>,
}

impl MockControl {
    pub fn set_status(&self, status_line: impl Into<String>) {
        self.lock().status_line = status_line.into();
    }

    pub fn queue_reset(&self, version: &str) {
        let mut state = self.lock();
        let status_line = state.status_line.clone();
        state.planned_cycles.push_back(VecDeque::from([
            MockRead::Line(format!("Grbl {version} ['$' for help]")),
            MockRead::Line(status_line),
        ]));
    }

    pub fn queue_alarm(&self, code: u16) {
        let mut state = self.lock();
        let alarm_status = "<Alarm|MPos:0.000,0.000,0.000|FS:0,0>".to_owned();
        state.status_line.clone_from(&alarm_status);
        state.planned_cycles.push_back(VecDeque::from([
            MockRead::Line(format!("ALARM:{code}")),
            MockRead::Line(alarm_status),
        ]));
    }

    pub fn clear_alarm(&self) {
        self.set_status(DEFAULT_STATUS);
    }

    pub fn queue_stall(&self) {
        self.lock()
            .planned_cycles
            .push_back(VecDeque::from([MockRead::Stall]));
    }

    pub fn queue_disconnect(&self) {
        self.lock()
            .planned_cycles
            .push_back(VecDeque::from([MockRead::Disconnect]));
    }

    pub fn queue_query_error(&self, code: u16) {
        self.lock()
            .planned_queries
            .push_back(VecDeque::from([MockRead::Line(format!("error:{code}"))]));
    }

    pub fn queue_query_alarm(&self, code: u16) {
        self.lock()
            .planned_queries
            .push_back(VecDeque::from([MockRead::Line(format!("ALARM:{code}"))]));
    }

    pub fn writes(&self) -> Vec<Vec<u8>> {
        self.lock().writes.clone()
    }

    fn lock(&self) -> MutexGuard<'_, MockState> {
        self.state.lock().expect("mock transport lock poisoned")
    }
}

#[derive(Debug, Clone)]
pub struct MockTransport {
    control: MockControl,
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::with_status(DEFAULT_STATUS)
    }
}

impl MockTransport {
    pub fn with_status(status_line: impl Into<String>) -> Self {
        Self {
            control: MockControl {
                state: Arc::new(Mutex::new(MockState {
                    connected: false,
                    status_line: status_line.into(),
                    planned_cycles: VecDeque::new(),
                    planned_queries: VecDeque::new(),
                    active_reads: VecDeque::new(),
                    writes: Vec::new(),
                    jog_polls_remaining: 0,
                })),
            },
        }
    }

    pub fn control(&self) -> MockControl {
        self.control.clone()
    }

    fn lock(&self) -> MutexGuard<'_, MockState> {
        self.control.lock()
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn connect(&mut self) -> Result<(), TransportError> {
        self.lock().connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        let mut state = self.lock();
        state.connected = false;
        state.active_reads.clear();
        Ok(())
    }

    async fn write(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let mut state = self.lock();
        if !state.connected {
            return Err(TransportError::NotConnected);
        }

        state.writes.push(data.to_vec());
        if data == b"?" {
            let cycle = if let Some(cycle) = state.planned_cycles.pop_front() {
                cycle
            } else {
                let status_line = state.status_line.clone();
                if status_line.starts_with("<Jog") {
                    state.jog_polls_remaining = state.jog_polls_remaining.saturating_sub(1);
                    if state.jog_polls_remaining == 0 {
                        state.status_line = status_with_mode(&status_line, "Idle", 0.0);
                    }
                }
                VecDeque::from([MockRead::Line(status_line)])
            };
            state.active_reads.extend(cycle);
        } else if data == b"!" {
            if state.status_line.starts_with("<Run") || state.status_line.starts_with("<Jog") {
                state.status_line = state
                    .status_line
                    .replacen("<Run", "<Hold:0", 1)
                    .replacen("<Jog", "<Hold:0", 1);
                state.jog_polls_remaining = 0;
            }
        } else if data == b"\x18" {
            state.status_line = DEFAULT_STATUS.to_owned();
            state.jog_polls_remaining = 0;
            state
                .active_reads
                .push_back(MockRead::Line("Grbl 1.1h ['$' for help]".to_owned()));
        } else if data == [0x85] {
            if state.status_line.starts_with("<Jog") {
                state.status_line = status_with_mode(&state.status_line, "Idle", 0.0);
                state.jog_polls_remaining = 0;
            }
        } else if let Some(jog) = parse_step_jog(data) {
            let mut position = status_position(&state.status_line).unwrap_or([0.0; 3]);
            position[jog.axis] += jog.distance_mm;
            state.status_line = format_status("Jog", position, jog.feed_mm_per_min);
            state.jog_polls_remaining = mock_jog_status_polls(jog);
            state
                .active_reads
                .push_back(MockRead::Line("ok".to_owned()));
        } else if let Some(default_response) = device_query_response(data) {
            let response = state
                .planned_queries
                .pop_front()
                .unwrap_or(default_response);
            state.active_reads.extend(response);
        }
        Ok(())
    }

    async fn read_line(&mut self) -> Result<String, TransportError> {
        let read = {
            let mut state = self.lock();
            if !state.connected {
                return Err(TransportError::NotConnected);
            }
            state
                .active_reads
                .pop_front()
                .ok_or(TransportError::NoData)?
        };

        match read {
            MockRead::Line(line) => Ok(line),
            MockRead::Stall => pending::<Result<String, TransportError>>().await,
            MockRead::Disconnect => {
                self.lock().connected = false;
                Err(TransportError::NotConnected)
            }
        }
    }

    fn is_connected(&self) -> bool {
        self.lock().connected
    }
}

fn mock_jog_status_polls(jog: MockJog) -> u32 {
    const MOCK_POLL_SECONDS: f64 = 0.25;
    let duration_seconds = jog.distance_mm.abs() / jog.feed_mm_per_min * 60.0;
    (duration_seconds / MOCK_POLL_SECONDS)
        .ceil()
        .clamp(1.0, 24.0) as u32
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MockJog {
    axis: usize,
    distance_mm: f64,
    feed_mm_per_min: f64,
}

fn parse_step_jog(data: &[u8]) -> Option<MockJog> {
    let command = std::str::from_utf8(data)
        .ok()?
        .trim_end_matches(['\r', '\n']);
    let words: Vec<_> = command
        .strip_prefix("$J=G91 G21 ")?
        .split_whitespace()
        .collect();
    if words.len() != 2 {
        return None;
    }

    let axis = match words[0].as_bytes().first()? {
        b'X' => 0,
        b'Y' => 1,
        b'Z' => 2,
        _ => return None,
    };
    let distance_mm = words[0].get(1..)?.parse().ok()?;
    let feed_mm_per_min = words[1].strip_prefix('F')?.parse().ok()?;
    Some(MockJog {
        axis,
        distance_mm,
        feed_mm_per_min,
    })
}

fn status_position(status: &str) -> Option<[f64; 3]> {
    let values = status
        .split('|')
        .find_map(|field| field.strip_prefix("MPos:"))?
        .split(',')
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (values.len() >= 3).then(|| [values[0], values[1], values[2]])
}

fn status_with_mode(status: &str, mode: &str, feed_mm_per_min: f64) -> String {
    format_status(
        mode,
        status_position(status).unwrap_or([0.0; 3]),
        feed_mm_per_min,
    )
}

fn format_status(mode: &str, position: [f64; 3], feed_mm_per_min: f64) -> String {
    let [x, y, z] = position;
    format!(
        "<{mode}|MPos:{x:.3},{y:.3},{z:.3}|WPos:{x:.3},{y:.3},{z:.3}|FS:{feed_mm_per_min:.3},0>"
    )
}

fn lines(values: &[&str]) -> VecDeque<MockRead> {
    values
        .iter()
        .map(|line| MockRead::Line((*line).to_owned()))
        .collect()
}

fn device_query_response(command: &[u8]) -> Option<VecDeque<MockRead>> {
    match command {
        b"$I\n" => Some(lines(&[
            "[VER:1.1h.20240101:Millo Mock]",
            "[OPT:V,15,128]",
            "ok",
        ])),
        b"$$\n" => Some(lines(&[
            "$0=10",
            "$6=0",
            "$10=3",
            "$20=0",
            "$21=0",
            "$22=0",
            "$30=12000",
            "$31=0",
            "$32=0",
            "$100=250.000",
            "$101=250.000",
            "$102=250.000",
            "$110=1000.000",
            "$111=1000.000",
            "$112=500.000",
            "$120=50.000",
            "$121=50.000",
            "$122=30.000",
            "$130=300.000",
            "$131=180.000",
            "$132=80.000",
            "ok",
        ])),
        b"$G\n" => Some(lines(&["[GC:G0 G54 G17 G21 G90 G94 M5 M9 T0 F0 S0]", "ok"])),
        b"$#\n" => Some(lines(&[
            "[G54:0.000,0.000,0.000]",
            "[G92:0.000,0.000,0.000]",
            "[TLO:0.000]",
            "[PRB:0.000,0.000,0.000:0]",
            "ok",
        ])),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn responds_to_realtime_status_query() {
        let mut transport = MockTransport::default();
        let control = transport.control();
        transport.connect().await.unwrap();
        transport.write(b"?").await.unwrap();

        let response = transport.read_line().await.unwrap();

        assert_eq!(response, DEFAULT_STATUS);
        assert_eq!(control.writes(), vec![b"?".to_vec()]);
    }

    #[tokio::test]
    async fn can_script_an_unresponsive_controller() {
        let mut transport = MockTransport::default();
        let control = transport.control();
        control.queue_stall();
        transport.connect().await.unwrap();
        transport.write(b"?").await.unwrap();

        let result = tokio::time::timeout(Duration::from_millis(5), transport.read_line()).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn reset_cycle_keeps_banner_and_status_in_order() {
        let mut transport = MockTransport::default();
        let control = transport.control();
        control.queue_reset("1.1h");
        transport.connect().await.unwrap();
        transport.write(b"?").await.unwrap();

        assert_eq!(
            transport.read_line().await.unwrap(),
            "Grbl 1.1h ['$' for help]"
        );
        assert_eq!(transport.read_line().await.unwrap(), DEFAULT_STATUS);
    }

    #[tokio::test]
    async fn alarm_persists_until_control_clears_it() {
        let mut transport = MockTransport::default();
        let control = transport.control();
        control.queue_alarm(3);
        transport.connect().await.unwrap();
        transport.write(b"?").await.unwrap();
        assert_eq!(transport.read_line().await.unwrap(), "ALARM:3");
        assert!(transport.read_line().await.unwrap().starts_with("<Alarm|"));

        transport.write(b"?").await.unwrap();
        assert!(transport.read_line().await.unwrap().starts_with("<Alarm|"));

        control.clear_alarm();
        transport.write(b"?").await.unwrap();
        assert_eq!(transport.read_line().await.unwrap(), DEFAULT_STATUS);
    }

    #[tokio::test]
    async fn answers_device_inspector_queries_in_command_order() {
        let mut transport = MockTransport::default();
        transport.connect().await.unwrap();
        transport.write(b"$I\n").await.unwrap();

        assert!(transport.read_line().await.unwrap().starts_with("[VER:"));
        assert!(transport.read_line().await.unwrap().starts_with("[OPT:"));
        assert_eq!(transport.read_line().await.unwrap(), "ok");
    }

    #[tokio::test]
    async fn feed_hold_changes_a_running_mock_before_the_next_status() {
        let mut transport = MockTransport::with_status(
            "<Run|MPos:1.000,2.000,3.000|WPos:1.000,2.000,3.000|FS:120,0>",
        );
        transport.connect().await.unwrap();

        transport.write(b"!").await.unwrap();
        transport.write(b"?").await.unwrap();

        assert!(transport.read_line().await.unwrap().starts_with("<Hold:0|"));
    }

    #[tokio::test]
    async fn soft_reset_emits_a_banner_and_returns_to_idle() {
        let mut transport = MockTransport::with_status(
            "<Alarm|MPos:1.000,2.000,3.000|WPos:1.000,2.000,3.000|FS:0,0>",
        );
        transport.connect().await.unwrap();

        transport.write(b"\x18").await.unwrap();
        assert_eq!(
            transport.read_line().await.unwrap(),
            "Grbl 1.1h ['$' for help]"
        );
        transport.write(b"?").await.unwrap();
        assert_eq!(transport.read_line().await.unwrap(), DEFAULT_STATUS);
    }

    #[tokio::test]
    async fn step_jog_changes_exactly_one_axis_and_completes() {
        let mut transport = MockTransport::default();
        transport.connect().await.unwrap();

        transport
            .write(b"$J=G91 G21 Y-0.100 F50.000\n")
            .await
            .unwrap();
        assert_eq!(transport.read_line().await.unwrap(), "ok");

        transport.write(b"?").await.unwrap();
        assert_eq!(
            transport.read_line().await.unwrap(),
            "<Jog|MPos:0.000,-0.100,0.000|WPos:0.000,-0.100,0.000|FS:50.000,0>"
        );
        transport.write(b"?").await.unwrap();
        assert_eq!(
            transport.read_line().await.unwrap(),
            "<Idle|MPos:0.000,-0.100,0.000|WPos:0.000,-0.100,0.000|FS:0.000,0>"
        );
    }

    #[tokio::test]
    async fn realtime_jog_cancel_returns_the_mock_to_idle() {
        let mut transport = MockTransport::default();
        transport.connect().await.unwrap();
        transport
            .write(b"$J=G91 G21 X1.000 F100.000\n")
            .await
            .unwrap();
        assert_eq!(transport.read_line().await.unwrap(), "ok");

        transport.write(&[0x85]).await.unwrap();
        transport.write(b"?").await.unwrap();

        assert_eq!(
            transport.read_line().await.unwrap(),
            "<Idle|MPos:1.000,0.000,0.000|WPos:1.000,0.000,0.000|FS:0.000,0>"
        );
    }

    #[tokio::test]
    async fn slow_mock_jog_remains_active_for_its_bounded_duration() {
        let mut transport = MockTransport::default();
        transport.connect().await.unwrap();
        transport
            .write(b"$J=G91 G21 Z1.000 F10.000\n")
            .await
            .unwrap();
        assert_eq!(transport.read_line().await.unwrap(), "ok");

        for _ in 0..23 {
            transport.write(b"?").await.unwrap();
            assert!(transport.read_line().await.unwrap().starts_with("<Jog|"));
        }
        transport.write(b"?").await.unwrap();
        assert!(transport.read_line().await.unwrap().starts_with("<Jog|"));
        transport.write(b"?").await.unwrap();
        assert!(transport.read_line().await.unwrap().starts_with("<Idle|"));
    }
}
