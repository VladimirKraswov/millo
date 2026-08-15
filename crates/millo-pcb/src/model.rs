use millo_gcode::GcodeProgram;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PcbLayerRole {
    Copper,
    Drill,
    Outline,
    Marking,
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbSourceFile {
    pub source_name: String,
    pub source_base64: String,
    pub role: PcbLayerRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbTransform {
    pub offset_x_mm: f64,
    pub offset_y_mm: f64,
    pub rotation_quarter_turns: u8,
    pub mirror_x: bool,
}

impl Default for PcbTransform {
    fn default() -> Self {
        Self {
            offset_x_mm: 0.0,
            offset_y_mm: 0.0,
            rotation_quarter_turns: 0,
            mirror_x: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbInspectRequest {
    pub files: Vec<PcbSourceFile>,
    #[serde(default)]
    pub transform: PcbTransform,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbPoint {
    pub x_mm: f64,
    pub y_mm: f64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbBounds {
    pub min_x_mm: f64,
    pub min_y_mm: f64,
    pub max_x_mm: f64,
    pub max_y_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbPreviewPath {
    pub role: PcbLayerRole,
    pub closed: bool,
    pub points: Vec<PcbPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbDrillHit {
    pub group_key: String,
    pub point: PcbPoint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbDrillSlot {
    pub group_key: String,
    pub start: PcbPoint,
    pub end: PcbPoint,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbDrillGroup {
    pub key: String,
    pub source_name: String,
    pub source_tool_number: u32,
    pub diameter_mm: f64,
    pub hit_count: usize,
    pub slot_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbFileSummary {
    pub source_name: String,
    pub role: PcbLayerRole,
    pub primitive_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbInspection {
    pub bounds: PcbBounds,
    pub paths: Vec<PcbPreviewPath>,
    pub drill_hits: Vec<PcbDrillHit>,
    pub drill_slots: Vec<PcbDrillSlot>,
    pub drill_groups: Vec<PcbDrillGroup>,
    pub files: Vec<PcbFileSummary>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbIsolationSettings {
    pub enabled: bool,
    pub tool_id: String,
    pub depth_mm: f64,
    pub clearance_mm: f64,
    pub passes: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbDrillToolMapping {
    pub group_key: String,
    pub tool_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbDrillingSettings {
    pub enabled: bool,
    pub depth_mm: f64,
    pub mappings: Vec<PcbDrillToolMapping>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbOutlineSettings {
    pub enabled: bool,
    pub tool_id: String,
    pub depth_mm: f64,
    pub depth_per_pass_mm: f64,
    pub tab_count: u8,
    pub tab_width_mm: f64,
    pub tab_height_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbMarkingSettings {
    pub enabled: bool,
    pub tool_id: String,
    pub depth_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbJobSettings {
    pub safe_z_mm: f64,
    pub surface_z_mm: f64,
    pub isolation: PcbIsolationSettings,
    pub drilling: PcbDrillingSettings,
    pub outline: PcbOutlineSettings,
    pub marking: PcbMarkingSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbJobRequest {
    pub source_name: String,
    pub board: PcbInspectRequest,
    pub settings: PcbJobSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbOperationSummary {
    pub kind: String,
    pub tool_id: String,
    pub tool_name: String,
    pub motion_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcbJobSummary {
    pub bounds: PcbBounds,
    pub operations: Vec<PcbOperationSummary>,
    pub tool_count: usize,
    pub tool_change_count: usize,
    pub warning_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedPcbJob {
    pub source_name: String,
    pub source: String,
    pub program: GcodeProgram,
    pub inspection: PcbInspection,
    pub summary: PcbJobSummary,
}
