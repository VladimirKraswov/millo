use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionState {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Recovering,
    Faulted,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MachineMode {
    #[default]
    Unknown,
    Idle,
    Run,
    Hold,
    Jog,
    Alarm,
    Door,
    Check,
    Home,
    Sleep,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineState {
    pub mode: MachineMode,
    pub reported_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub substate: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub machine_position: Option<Position>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_position: Option<Position>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_coordinate_offset: Option<Position>,
    pub feed_rate: f64,
    pub spindle_speed: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetNotice {
    pub banner: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub sequence: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<u16>,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerSnapshot {
    pub connection: ConnectionState,
    pub machine: MachineState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_notice: Option<ResetNotice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alarm: Option<AlarmState>,
    pub consecutive_failures: u32,
    pub reconnect_count: u32,
    pub poll_sequence: u64,
    pub reset_count: u64,
    pub poll_interval_ms: u64,
    pub status_timeout_ms: u64,
    pub failure_threshold: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CommandCompletion {
    Ok,
    Error,
    Alarm,
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandResponse {
    pub command: String,
    pub completion: CommandCompletion,
    pub lines: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<u16>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInspection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_build_info: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub firmware_options: Option<String>,
    pub settings: BTreeMap<String, String>,
    pub modal_state: Vec<String>,
    pub parameters: BTreeMap<String, String>,
    pub responses: Vec<CommandResponse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpindleControl {
    Manual,
    Controller,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfile {
    pub name: String,
    pub axes: Vec<String>,
    pub spindle_control: SpindleControl,
    pub homing_installed: bool,
    pub limit_switches_installed: bool,
    pub emergency_stop_installed: bool,
}

impl HardwareProfile {
    pub fn first_machine() -> Self {
        Self {
            name: "First XYZ router".to_owned(),
            axes: vec!["X".to_owned(), "Y".to_owned(), "Z".to_owned()],
            spindle_control: SpindleControl::Manual,
            homing_installed: false,
            limit_switches_installed: false,
            emergency_stop_installed: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReadinessLevel {
    Pass,
    Caution,
    Blocker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessCheck {
    pub id: String,
    pub level: ReadinessLevel,
    pub title: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessReport {
    pub profile: HardwareProfile,
    pub test_jog_ready: bool,
    pub probe_ready: bool,
    pub blocker_count: usize,
    pub caution_count: usize,
    pub checks: Vec<ReadinessCheck>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareInspection {
    pub device: DeviceInspection,
    pub readiness: ReadinessReport,
}
