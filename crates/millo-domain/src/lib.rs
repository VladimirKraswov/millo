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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OverrideAdjustment {
    Reset,
    IncreaseTen,
    DecreaseTen,
    IncreaseOne,
    DecreaseOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RapidOverrideTarget {
    Full,
    Half,
    Quarter,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerBufferState {
    pub planner_available: u16,
    pub rx_available: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerOverrides {
    pub feed_percent: u16,
    pub rapid_percent: u16,
    pub spindle_percent: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerPins {
    pub raw: String,
    pub x_limit: bool,
    pub y_limit: bool,
    pub z_limit: bool,
    pub a_limit: bool,
    pub b_limit: bool,
    pub c_limit: bool,
    pub probe: bool,
    pub door: bool,
    pub hold: bool,
    pub soft_reset: bool,
    pub cycle_start: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerAccessories {
    pub raw: String,
    pub spindle_clockwise: bool,
    pub spindle_counterclockwise: bool,
    pub flood_coolant: bool,
    pub mist_coolant: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub buffer_state: Option<ControllerBufferState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overrides: Option<ControllerOverrides>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pins: Option<ControllerPins>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessories: Option<ControllerAccessories>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_number: Option<u64>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller_capabilities: Option<ControllerCapabilities>,
    pub settings: BTreeMap<String, String>,
    pub modal_state: Vec<String>,
    pub parameters: BTreeMap<String, String>,
    pub responses: Vec<CommandResponse>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ControllerCapabilities {
    pub option_flags: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planner_buffer_blocks: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rx_buffer_bytes: Option<u16>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SpindleControl {
    #[default]
    Manual,
    Controller,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineTravel {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

pub const DEFAULT_MAX_JOG_DISTANCE_MM: f64 = 50.0;

pub const fn default_max_jog_distance_mm() -> f64 {
    DEFAULT_MAX_JOG_DISTANCE_MM
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfile {
    pub name: String,
    pub axes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub travel_mm: Option<MachineTravel>,
    #[serde(default = "default_max_jog_distance_mm")]
    pub max_jog_distance_mm: f64,
    pub spindle_control: SpindleControl,
    pub homing_installed: bool,
    pub limit_switches_installed: bool,
    pub probe_installed: bool,
    #[serde(default)]
    pub probe_mode: ProbeWorkflowMode,
    pub emergency_stop_installed: bool,
}

impl HardwareProfile {
    pub fn first_machine() -> Self {
        Self {
            name: "First XYZ router".to_owned(),
            axes: vec!["X".to_owned(), "Y".to_owned(), "Z".to_owned()],
            travel_mm: None,
            max_jog_distance_mm: DEFAULT_MAX_JOG_DISTANCE_MM,
            spindle_control: SpindleControl::Manual,
            homing_installed: false,
            limit_switches_installed: false,
            probe_installed: false,
            probe_mode: ProbeWorkflowMode::Off,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperatorConfirmation {
    pub spindle_off: bool,
    pub tool_clear: bool,
    pub power_control_reachable: bool,
}

impl OperatorConfirmation {
    pub fn is_complete(self) -> bool {
        self.spindle_off && self.tool_clear && self.power_control_reachable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetChallenge {
    pub id: u64,
    pub expires_in_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestJogAuthorization {
    pub id: u64,
    pub expires_in_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestJogPreparation {
    pub inspection: HardwareInspection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization: Option<TestJogAuthorization>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JogAxis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepJogRequest {
    pub authorization_id: u64,
    pub axis: JogAxis,
    pub distance_mm: f64,
    pub feed_mm_per_min: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepJogReceipt {
    pub command: String,
    pub axis: JogAxis,
    pub distance_mm: f64,
    pub feed_mm_per_min: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JogPadStepRequest {
    pub confirmation: OperatorConfirmation,
    pub axis: JogAxis,
    pub distance_mm: f64,
    pub feed_mm_per_min: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JogPadStepOutcome {
    pub inspection: HardwareInspection,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<StepJogReceipt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkAxis {
    X,
    Y,
    Z,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkCoordinateSystem {
    G54,
    G55,
    G56,
    G57,
    G58,
    G59,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkZeroRequest {
    pub axis: WorkAxis,
    pub position_confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkZeroOutcome {
    pub axis: WorkAxis,
    pub coordinate_system: WorkCoordinateSystem,
    pub command: String,
    pub parameter_value: String,
    pub work_position: f64,
    pub snapshot: ControllerSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnToWorkZeroRequest {
    pub axis: WorkAxis,
    pub feed_mm_per_min: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnToWorkZeroOutcome {
    pub axis: WorkAxis,
    pub coordinate_system: WorkCoordinateSystem,
    pub command: String,
    pub snapshot: ControllerSnapshot,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnToWorkOriginRequest {
    pub clearance_z_mm: f64,
    pub xy_feed_mm_per_min: f64,
    pub z_feed_mm_per_min: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnToWorkOriginOutcome {
    pub coordinate_system: WorkCoordinateSystem,
    pub commands: Vec<String>,
    pub snapshot: ControllerSnapshot,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProbeWorkflowMode {
    #[default]
    Off,
    WorkZero,
    Heightmap,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZProbeSettings {
    pub mode: ProbeWorkflowMode,
    pub plate_thickness_mm: f64,
    pub max_travel_mm: f64,
    pub probe_feed_mm_per_min: f64,
    pub retract_mm: f64,
    pub retract_feed_mm_per_min: f64,
}

impl Default for ZProbeSettings {
    fn default() -> Self {
        Self {
            mode: ProbeWorkflowMode::Off,
            plate_thickness_mm: 0.0,
            max_travel_mm: 10.0,
            probe_feed_mm_per_min: 25.0,
            retract_mm: 3.0,
            retract_feed_mm_per_min: 100.0,
        }
    }
}

impl<'de> Deserialize<'de> for ZProbeSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Stored {
            mode: Option<ProbeWorkflowMode>,
            use_for_work_zero: Option<bool>,
            plate_thickness_mm: Option<f64>,
            max_travel_mm: Option<f64>,
            probe_feed_mm_per_min: Option<f64>,
            retract_mm: Option<f64>,
            retract_feed_mm_per_min: Option<f64>,
        }
        let stored = Stored::deserialize(deserializer)?;
        let defaults = Self::default();
        Ok(Self {
            mode: stored.mode.unwrap_or_else(|| {
                if stored.use_for_work_zero.unwrap_or(false) {
                    ProbeWorkflowMode::WorkZero
                } else {
                    ProbeWorkflowMode::Off
                }
            }),
            plate_thickness_mm: stored
                .plate_thickness_mm
                .unwrap_or(defaults.plate_thickness_mm),
            max_travel_mm: stored.max_travel_mm.unwrap_or(defaults.max_travel_mm),
            probe_feed_mm_per_min: stored
                .probe_feed_mm_per_min
                .unwrap_or(defaults.probe_feed_mm_per_min),
            retract_mm: stored.retract_mm.unwrap_or(defaults.retract_mm),
            retract_feed_mm_per_min: stored
                .retract_feed_mm_per_min
                .unwrap_or(defaults.retract_feed_mm_per_min),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZProbeRequest {
    pub settings: ZProbeSettings,
    pub setup_confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZProbeOutcome {
    pub coordinate_system: WorkCoordinateSystem,
    pub probe_command: String,
    pub zero_command: String,
    pub retract_command: String,
    pub contact_machine_position: Position,
    pub final_work_z: f64,
    pub snapshot: ControllerSnapshot,
}

#[cfg(test)]
mod z_probe_settings_tests {
    use super::*;

    #[test]
    fn legacy_work_zero_flag_migrates_to_the_typed_probe_mode() {
        let settings: ZProbeSettings = serde_json::from_str(
            r#"{
                "useForWorkZero": true,
                "plateThicknessMm": 19.1,
                "maxTravelMm": 10.0,
                "probeFeedMmPerMin": 25.0,
                "retractMm": 3.0,
                "retractFeedMmPerMin": 100.0
            }"#,
        )
        .unwrap();

        assert_eq!(settings.mode, ProbeWorkflowMode::WorkZero);
        assert_eq!(settings.plate_thickness_mm, 19.1);
    }

    #[test]
    fn explicit_heightmap_mode_wins_over_the_legacy_flag() {
        let settings: ZProbeSettings = serde_json::from_str(
            r#"{
                "mode": "heightmap",
                "useForWorkZero": true,
                "plateThicknessMm": 19.1
            }"#,
        )
        .unwrap();

        assert_eq!(settings.mode, ProbeWorkflowMode::Heightmap);
    }
}
