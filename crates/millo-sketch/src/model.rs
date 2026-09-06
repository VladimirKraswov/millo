use millo_gcode::GcodeProgram;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SketchPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum SketchGeometry {
    Rectangle {
        width: f64,
        height: f64,
        radius: f64,
    },
    Circle {
        diameter: f64,
    },
    Polygon {
        points: Vec<SketchPoint>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SketchOperationKind {
    Pocket,
    Inside,
    Outside,
    Engrave,
    Drill,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SketchTabs {
    pub count: u8,
    pub width_mm: f64,
    pub height_mm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SketchOperation {
    pub kind: SketchOperationKind,
    pub tool_id: String,
    pub through: bool,
    pub depth_mm: f64,
    pub stepdown_mm: f64,
    pub stepover_percent: f64,
    pub feed_mm_per_min: f64,
    pub plunge_mm_per_min: f64,
    pub spindle_rpm: u32,
    pub tabs: SketchTabs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SketchShape {
    pub id: String,
    pub name: String,
    pub x_mm: f64,
    pub y_mm: f64,
    pub rotation_degrees: f64,
    pub geometry: SketchGeometry,
    pub operation: SketchOperation,
    #[serde(default)]
    pub constraints: SketchConstraints,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SketchAnchorName {
    Min,
    Center,
    Max,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SketchAnchor {
    Named(SketchAnchorName),
    Vertex(usize),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SketchAxisConstraint {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_id: Option<String>,
    pub reference_anchor: SketchAnchor,
    pub own_anchor: SketchAnchor,
    pub offset_mm: f64,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SketchConstraints {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub x: Option<SketchAxisConstraint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub y: Option<SketchAxisConstraint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SketchSpindleMode {
    Manual,
    Controller,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SketchStock {
    pub width_mm: f64,
    pub height_mm: f64,
    pub thickness_mm: f64,
    pub safe_z_mm: f64,
    pub breakthrough_mm: f64,
    pub spindle_mode: SketchSpindleMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SketchJobRequest {
    pub source_name: String,
    pub stock: SketchStock,
    pub shapes: Vec<SketchShape>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SketchOperationSummary {
    pub shape_id: String,
    pub name: String,
    pub tool_id: String,
    pub tool_number: usize,
    pub depth_mm: f64,
    pub pass_count: usize,
    pub path_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SketchPreviewPath {
    pub shape_id: String,
    pub points: Vec<SketchPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SketchJobSummary {
    pub operations: Vec<SketchOperationSummary>,
    pub tool_change_count: usize,
    pub paths: Vec<SketchPreviewPath>,
    pub tab_paths: Vec<SketchPreviewPath>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedSketchJob {
    pub source_name: String,
    pub source: String,
    pub program: GcodeProgram,
    pub summary: SketchJobSummary,
}
