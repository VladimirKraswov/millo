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
            let cycle = state
                .planned_cycles
                .pop_front()
                .unwrap_or_else(|| VecDeque::from([MockRead::Line(state.status_line.clone())]));
            state.active_reads.extend(cycle);
        } else if data == b"!" {
            if state.status_line.starts_with("<Run") || state.status_line.starts_with("<Jog") {
                state.status_line = state
                    .status_line
                    .replacen("<Run", "<Hold:0", 1)
                    .replacen("<Jog", "<Hold:0", 1);
            }
        } else if data == b"\x18" {
            state.status_line = DEFAULT_STATUS.to_owned();
            state
                .active_reads
                .push_back(MockRead::Line("Grbl 1.1h ['$' for help]".to_owned()));
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
}
