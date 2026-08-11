use std::{
    collections::{BTreeMap, VecDeque},
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
    DelaySlice,
    Disconnect,
}

#[derive(Debug)]
struct MockState {
    connected: bool,
    status_line: String,
    planned_cycles: VecDeque<VecDeque<MockRead>>,
    planned_queries: VecDeque<VecDeque<MockRead>>,
    planned_settings: VecDeque<VecDeque<MockRead>>,
    planned_program: VecDeque<VecDeque<MockRead>>,
    active_reads: VecDeque<MockRead>,
    writes: Vec<Vec<u8>>,
    jog_polls_remaining: u32,
    settings: BTreeMap<u16, String>,
    active_wcs: usize,
    work_offsets: [[f64; 3]; 6],
    firmware_options: String,
    overrides: [u16; 3],
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

    pub fn queue_setting_error(&self, code: u16) {
        self.lock()
            .planned_settings
            .push_back(VecDeque::from([MockRead::Line(format!("error:{code}"))]));
    }

    pub fn queue_program_error(&self, code: u16) {
        self.lock()
            .planned_program
            .push_back(VecDeque::from([MockRead::Line(format!("error:{code}"))]));
    }

    pub fn queue_program_ok(&self) {
        self.lock()
            .planned_program
            .push_back(VecDeque::from([MockRead::Line("ok".to_owned())]));
    }

    pub fn queue_program_stall(&self) {
        self.lock()
            .planned_program
            .push_back(VecDeque::from([MockRead::Stall]));
    }

    pub fn queue_program_delay(&self, read_slices: usize) {
        let mut response = (0..read_slices)
            .map(|_| MockRead::DelaySlice)
            .collect::<VecDeque<_>>();
        response.push_back(MockRead::Line("ok".to_owned()));
        self.lock().planned_program.push_back(response);
    }

    pub fn queue_program_alarm(&self, code: u16) {
        self.lock()
            .planned_program
            .push_back(VecDeque::from([MockRead::Line(format!("ALARM:{code}"))]));
    }

    pub fn queue_program_reset(&self, version: &str) {
        self.lock()
            .planned_program
            .push_back(VecDeque::from([MockRead::Line(format!(
                "Grbl {version} ['$' for help]"
            ))]));
    }

    pub fn queue_program_disconnect(&self) {
        self.lock()
            .planned_program
            .push_back(VecDeque::from([MockRead::Disconnect]));
    }

    pub fn writes(&self) -> Vec<Vec<u8>> {
        self.lock().writes.clone()
    }

    pub fn set_setting(&self, number: u16, value: impl Into<String>) {
        self.lock().settings.insert(number, value.into());
    }

    pub fn set_active_wcs(&self, coordinate_system: u8) {
        assert!((54..=59).contains(&coordinate_system));
        self.lock().active_wcs = usize::from(coordinate_system - 54);
    }

    pub fn set_firmware_options(&self, value: impl Into<String>) {
        self.lock().firmware_options = value.into();
    }

    fn lock(&self) -> MutexGuard<'_, MockState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
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
                    planned_settings: VecDeque::new(),
                    planned_program: VecDeque::new(),
                    active_reads: VecDeque::new(),
                    writes: Vec::new(),
                    jog_polls_remaining: 0,
                    settings: default_settings(),
                    active_wcs: 0,
                    work_offsets: [[0.0; 3]; 6],
                    firmware_options: "V,15,128".to_owned(),
                    overrides: [100, 100, 100],
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
            let reset_banner_pending = state.active_reads.front().is_some_and(
                |read| matches!(read, MockRead::Line(line) if line.starts_with("Grbl ")),
            );
            if state.active_reads.is_empty() || reset_banner_pending {
                state.active_reads.extend(cycle);
            } else {
                for read in cycle.into_iter().rev() {
                    state.active_reads.push_front(read);
                }
            }
        } else if data == b"!" {
            if state.status_line.starts_with("<Run") || state.status_line.starts_with("<Jog") {
                state.status_line = state
                    .status_line
                    .replacen("<Run", "<Hold:0", 1)
                    .replacen("<Jog", "<Hold:0", 1);
                state.jog_polls_remaining = 0;
            }
        } else if data == b"~" {
            if state.status_line.starts_with("<Hold") {
                state.status_line = state.status_line.replacen("<Hold:0", "<Run", 1);
            }
        } else if data == b"\x18" {
            state.status_line = DEFAULT_STATUS.to_owned();
            state.overrides = [100, 100, 100];
            state.jog_polls_remaining = 0;
            state.active_reads.clear();
            state
                .active_reads
                .push_back(MockRead::Line("Grbl 1.1h ['$' for help]".to_owned()));
        } else if data == [0x85] {
            if state.status_line.starts_with("<Jog") {
                state.status_line = status_with_mode(&state.status_line, "Idle", 0.0);
                state.jog_polls_remaining = 0;
            }
        } else if let Some((index, value)) = override_update(data, state.overrides) {
            state.overrides[index] = value;
            state.status_line = status_with_overrides(&state.status_line, state.overrides);
        } else if data == b"$X\n" {
            if state.status_line.starts_with("<Alarm") {
                state.status_line = status_with_mode(&state.status_line, "Idle", 0.0);
                state
                    .active_reads
                    .push_back(MockRead::Line("ok".to_owned()));
            } else {
                state
                    .active_reads
                    .push_back(MockRead::Line("error:9".to_owned()));
            }
        } else if data == b"$C\n" {
            let mode = status_mode(&state.status_line).unwrap_or("Unknown");
            if mode == "Idle" {
                state.status_line = status_with_mode(&state.status_line, "Check", 0.0);
                state
                    .active_reads
                    .push_back(MockRead::Line("ok".to_owned()));
            } else if mode == "Check" {
                state.status_line = status_with_mode(&state.status_line, "Idle", 0.0);
                state
                    .active_reads
                    .push_back(MockRead::Line("ok".to_owned()));
            } else {
                state
                    .active_reads
                    .push_back(MockRead::Line("error:8".to_owned()));
            }
        } else if let Some(jog) = parse_step_jog(data) {
            let mut position = status_position(&state.status_line).unwrap_or([0.0; 3]);
            position[jog.axis] += jog.distance_mm;
            let work_position = subtract_position(position, state.work_offsets[state.active_wcs]);
            state.status_line = format_status("Jog", position, work_position, jog.feed_mm_per_min);
            state.jog_polls_remaining = mock_jog_status_polls(jog);
            state
                .active_reads
                .push_back(MockRead::Line("ok".to_owned()));
        } else if let Some(work_zero) = parse_work_zero(data) {
            let machine_position = status_position(&state.status_line).unwrap_or([0.0; 3]);
            state.work_offsets[work_zero.coordinate_system][work_zero.axis] =
                machine_position[work_zero.axis];
            if work_zero.coordinate_system == state.active_wcs {
                let work_position =
                    subtract_position(machine_position, state.work_offsets[state.active_wcs]);
                let mode = status_mode(&state.status_line).unwrap_or("Idle").to_owned();
                let feed = status_feed(&state.status_line).unwrap_or(0.0);
                state.status_line = format_status(&mode, machine_position, work_position, feed);
            }
            state
                .active_reads
                .push_back(MockRead::Line("ok".to_owned()));
        } else if let Some((number, value)) = parse_setting_write(data) {
            let response = state
                .planned_settings
                .pop_front()
                .unwrap_or_else(|| lines(&["ok"]));
            if response
                .back()
                .is_some_and(|read| matches!(read, MockRead::Line(line) if line == "ok"))
            {
                state.settings.insert(number, value);
            }
            state.active_reads.extend(response);
        } else if let Some(default_response) = device_query_response(data, &state) {
            let response = state
                .planned_queries
                .pop_front()
                .unwrap_or(default_response);
            state.active_reads.extend(response);
        } else if is_program_line(data) {
            let response = state
                .planned_program
                .pop_front()
                .unwrap_or_else(|| lines(&["ok"]));
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
            match state.active_reads.front() {
                Some(MockRead::Stall) => MockRead::Stall,
                Some(_) => state
                    .active_reads
                    .pop_front()
                    .ok_or(TransportError::NoData)?,
                None => return Err(TransportError::NoData),
            }
        };

        match read {
            MockRead::Line(line) => Ok(line),
            MockRead::Stall | MockRead::DelaySlice => {
                pending::<Result<String, TransportError>>().await
            }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MockWorkZero {
    coordinate_system: usize,
    axis: usize,
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

fn parse_work_zero(data: &[u8]) -> Option<MockWorkZero> {
    let command = std::str::from_utf8(data)
        .ok()?
        .trim_end_matches(['\r', '\n']);
    let words: Vec<_> = command.split_whitespace().collect();
    if words.len() != 4 || words[0] != "G10" || words[1] != "L20" {
        return None;
    }
    let parameter = words[2].strip_prefix('P')?.parse::<usize>().ok()?;
    if !(1..=6).contains(&parameter) {
        return None;
    }
    let axis = match words[3].as_bytes().first()? {
        b'X' => 0,
        b'Y' => 1,
        b'Z' => 2,
        _ => return None,
    };
    (words[3].get(1..)?.parse::<f64>().ok()? == 0.0).then_some(MockWorkZero {
        coordinate_system: parameter - 1,
        axis,
    })
}

fn parse_setting_write(data: &[u8]) -> Option<(u16, String)> {
    let command = std::str::from_utf8(data)
        .ok()?
        .trim_end_matches(['\r', '\n']);
    let (number, value) = command.strip_prefix('$')?.split_once('=')?;
    let number = number.parse().ok()?;
    (!value.is_empty()).then(|| (number, value.to_owned()))
}

fn is_program_line(data: &[u8]) -> bool {
    let Ok(command) = std::str::from_utf8(data) else {
        return false;
    };
    let command = command.trim_end_matches(['\r', '\n']);
    !command.is_empty() && !command.starts_with('$') && data.ends_with(b"\n")
}

fn status_position(status: &str) -> Option<[f64; 3]> {
    status_named_position(status, "MPos")
}

fn status_named_position(status: &str, name: &str) -> Option<[f64; 3]> {
    let values = status
        .split('|')
        .find_map(|field| field.strip_prefix(&format!("{name}:")))?
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
        status_named_position(status, "WPos").unwrap_or([0.0; 3]),
        feed_mm_per_min,
    )
}

fn status_mode(status: &str) -> Option<&str> {
    status.strip_prefix('<')?.split('|').next()
}

fn status_feed(status: &str) -> Option<f64> {
    status
        .split('|')
        .find_map(|field| field.strip_prefix("FS:"))?
        .split(',')
        .next()?
        .parse()
        .ok()
}

fn override_update(data: &[u8], current: [u16; 3]) -> Option<(usize, u16)> {
    let (index, value) = match data {
        [0x90] => (0, 100),
        [0x91] => (0, current[0].saturating_add(10).min(200)),
        [0x92] => (0, current[0].saturating_sub(10).max(10)),
        [0x93] => (0, current[0].saturating_add(1).min(200)),
        [0x94] => (0, current[0].saturating_sub(1).max(10)),
        [0x95] => (1, 100),
        [0x96] => (1, 50),
        [0x97] => (1, 25),
        [0x99] => (2, 100),
        [0x9a] => (2, current[2].saturating_add(10).min(200)),
        [0x9b] => (2, current[2].saturating_sub(10).max(10)),
        [0x9c] => (2, current[2].saturating_add(1).min(200)),
        [0x9d] => (2, current[2].saturating_sub(1).max(10)),
        _ => return None,
    };
    Some((index, value))
}

fn status_with_overrides(status: &str, overrides: [u16; 3]) -> String {
    let override_field = format!("Ov:{},{},{}", overrides[0], overrides[1], overrides[2]);
    let mut fields = status
        .trim_start_matches('<')
        .trim_end_matches('>')
        .split('|')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if let Some(field) = fields.iter_mut().find(|field| field.starts_with("Ov:")) {
        *field = override_field;
    } else {
        fields.push(override_field);
    }
    format!("<{}>", fields.join("|"))
}

fn subtract_position(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    [left[0] - right[0], left[1] - right[1], left[2] - right[2]]
}

fn format_status(
    mode: &str,
    machine_position: [f64; 3],
    work_position: [f64; 3],
    feed_mm_per_min: f64,
) -> String {
    let [mx, my, mz] = machine_position;
    let [wx, wy, wz] = work_position;
    format!(
        "<{mode}|MPos:{mx:.3},{my:.3},{mz:.3}|WPos:{wx:.3},{wy:.3},{wz:.3}|FS:{feed_mm_per_min:.3},0>"
    )
}

fn lines(values: &[&str]) -> VecDeque<MockRead> {
    values
        .iter()
        .map(|line| MockRead::Line((*line).to_owned()))
        .collect()
}

fn default_settings() -> BTreeMap<u16, String> {
    [
        (0, "10"),
        (6, "0"),
        (10, "3"),
        (20, "0"),
        (21, "0"),
        (22, "0"),
        (30, "12000"),
        (31, "0"),
        (32, "0"),
        (100, "250.000"),
        (101, "250.000"),
        (102, "250.000"),
        (110, "1000.000"),
        (111, "1000.000"),
        (112, "500.000"),
        (120, "50.000"),
        (121, "50.000"),
        (122, "30.000"),
        (130, "300.000"),
        (131, "180.000"),
        (132, "80.000"),
    ]
    .into_iter()
    .map(|(number, value)| (number, value.to_owned()))
    .collect()
}

fn device_query_response(command: &[u8], state: &MockState) -> Option<VecDeque<MockRead>> {
    match command {
        b"$I\n" => Some(lines(&[
            "[VER:1.1h.20240101:Millo Mock]",
            &format!("[OPT:{}]", state.firmware_options),
            "ok",
        ])),
        b"$$\n" => {
            let mut response = state
                .settings
                .iter()
                .map(|(number, value)| MockRead::Line(format!("${number}={value}")))
                .collect::<VecDeque<_>>();
            response.push_back(MockRead::Line("ok".to_owned()));
            Some(response)
        }
        b"$G\n" => Some(lines(&[
            &format!(
                "[GC:G0 G{} G17 G21 G90 G94 M5 M9 T0 F0 S0]",
                state.active_wcs + 54
            ),
            "ok",
        ])),
        b"$#\n" => {
            let mut response = state
                .work_offsets
                .iter()
                .enumerate()
                .map(|(index, [x, y, z])| {
                    MockRead::Line(format!("[G{}:{x:.3},{y:.3},{z:.3}]", index + 54))
                })
                .collect::<VecDeque<_>>();
            response.extend(lines(&[
                "[G92:0.000,0.000,0.000]",
                "[TLO:0.000]",
                "[PRB:0.000,0.000,0.000:0]",
                "ok",
            ]));
            Some(response)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn recovers_mock_state_after_a_fixture_thread_panics() {
        let control = MockTransport::default().control();
        let fixture = control.clone();
        let panicked = std::thread::spawn(move || {
            let _guard = fixture.state.lock().unwrap();
            panic!("fixture panic");
        })
        .join();

        assert!(panicked.is_err());
        control.set_status(DEFAULT_STATUS);
        assert!(control.writes().is_empty());
    }

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
    async fn setting_writes_persist_only_after_ok() {
        let mut transport = MockTransport::default();
        let control = transport.control();
        control.set_setting(21, "1");
        transport.connect().await.unwrap();

        transport.write(b"$21=0\n").await.unwrap();
        assert_eq!(transport.read_line().await.unwrap(), "ok");
        transport.write(b"$$\n").await.unwrap();
        let mut settings = Vec::new();
        loop {
            let line = transport.read_line().await.unwrap();
            if line == "ok" {
                break;
            }
            settings.push(line);
        }
        assert!(settings.contains(&"$21=0".to_owned()));

        control.queue_setting_error(2);
        transport.write(b"$22=1\n").await.unwrap();
        assert_eq!(transport.read_line().await.unwrap(), "error:2");
        transport.write(b"$$\n").await.unwrap();
        let mut homing = None;
        loop {
            let line = transport.read_line().await.unwrap();
            if line == "ok" {
                break;
            }
            if let Some(value) = line.strip_prefix("$22=") {
                homing = Some(value.to_owned());
            }
        }
        assert_eq!(homing.as_deref(), Some("0"));
    }

    #[tokio::test]
    async fn check_mode_toggles_only_between_idle_and_check() {
        let mut transport = MockTransport::default();
        transport.connect().await.unwrap();

        transport.write(b"$C\n").await.unwrap();
        assert_eq!(transport.read_line().await.unwrap(), "ok");
        transport.write(b"?").await.unwrap();
        assert!(transport.read_line().await.unwrap().starts_with("<Check|"));

        transport.write(b"$C\n").await.unwrap();
        assert_eq!(transport.read_line().await.unwrap(), "ok");
        transport.write(b"?").await.unwrap();
        assert!(transport.read_line().await.unwrap().starts_with("<Idle|"));

        let mut running = MockTransport::with_status(
            "<Run|MPos:0.000,0.000,0.000|WPos:0.000,0.000,0.000|FS:10,0>",
        );
        running.connect().await.unwrap();
        running.write(b"$C\n").await.unwrap();
        assert_eq!(running.read_line().await.unwrap(), "error:8");
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
    async fn applies_typed_grbl_override_bytes_to_status_telemetry() {
        let mut transport = MockTransport::default();
        transport.connect().await.unwrap();

        for byte in [0x91, 0x93, 0x97, 0x9b, 0x9d] {
            transport.write(&[byte]).await.unwrap();
        }
        transport.write(b"?").await.unwrap();

        let status = transport.read_line().await.unwrap();
        assert!(status.contains("|Ov:111,25,89>"), "{status}");
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
    async fn work_zero_changes_active_wcs_and_work_position_without_motion() {
        let mut transport = MockTransport::with_status(
            "<Idle|MPos:10.000,20.000,30.000|WPos:10.000,20.000,30.000|FS:0,0>",
        );
        let control = transport.control();
        control.set_active_wcs(55);
        transport.connect().await.unwrap();

        transport.write(b"G10 L20 P2 Y0\n").await.unwrap();
        assert_eq!(transport.read_line().await.unwrap(), "ok");
        transport.write(b"?").await.unwrap();
        assert_eq!(
            transport.read_line().await.unwrap(),
            "<Idle|MPos:10.000,20.000,30.000|WPos:10.000,0.000,30.000|FS:0.000,0>"
        );
        transport.write(b"$#\n").await.unwrap();
        let mut parameters = Vec::new();
        loop {
            let line = transport.read_line().await.unwrap();
            if line == "ok" {
                break;
            }
            parameters.push(line);
        }

        assert!(parameters.contains(&"[G55:0.000,20.000,0.000]".to_owned()));
        assert_eq!(
            status_position(&control.lock().status_line),
            Some([10.0, 20.0, 30.0])
        );
    }

    #[tokio::test]
    async fn acknowledges_program_lines_and_can_inject_a_correlated_error() {
        let mut transport = MockTransport::default();
        let control = transport.control();
        transport.connect().await.unwrap();

        transport.write(b"G21 G90\n").await.unwrap();
        assert_eq!(transport.read_line().await.unwrap(), "ok");
        control.queue_program_error(20);
        transport.write(b"G1 X1 F10\n").await.unwrap();
        assert_eq!(transport.read_line().await.unwrap(), "error:20");

        assert_eq!(
            control.writes(),
            vec![b"G21 G90\n".to_vec(), b"G1 X1 F10\n".to_vec()]
        );
    }

    #[tokio::test]
    async fn realtime_status_precedes_a_delayed_program_acknowledgement() {
        let mut transport = MockTransport::default();
        let control = transport.control();
        control.queue_program_delay(1);
        transport.connect().await.unwrap();

        transport.write(b"G1 X1 F10\n").await.unwrap();
        transport.write(b"?").await.unwrap();

        assert!(transport.read_line().await.unwrap().starts_with("<Idle|"));
        let delayed = tokio::time::timeout(Duration::from_millis(2), transport.read_line()).await;
        assert!(delayed.is_err());
        assert_eq!(transport.read_line().await.unwrap(), "ok");
    }

    #[tokio::test]
    async fn can_inject_an_alarm_as_the_terminal_program_response() {
        let mut transport = MockTransport::default();
        let control = transport.control();
        control.queue_program_alarm(2);
        transport.connect().await.unwrap();

        transport.write(b"G1 X1\n").await.unwrap();

        assert_eq!(transport.read_line().await.unwrap(), "ALARM:2");
    }

    #[tokio::test]
    async fn soft_reset_flushes_pending_program_responses() {
        let mut transport = MockTransport::default();
        let control = transport.control();
        control.queue_program_stall();
        control.queue_program_ok();
        transport.connect().await.unwrap();

        transport.write(b"G1 X1\n").await.unwrap();
        transport.write(b"G1 X2\n").await.unwrap();
        transport.write(b"\x18").await.unwrap();

        assert_eq!(
            transport.read_line().await.unwrap(),
            "Grbl 1.1h ['$' for help]"
        );
        assert_eq!(
            transport.read_line().await.unwrap_err(),
            TransportError::NoData
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
