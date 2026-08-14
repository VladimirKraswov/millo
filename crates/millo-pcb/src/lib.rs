mod excellon;
mod gcode;
mod geometry;
mod gerber;
mod model;

use std::collections::BTreeMap;

use base64::Engine;
use thiserror::Error;

pub use gcode::generate_pcb_job;
pub use model::*;

use geometry::{BoardGeometry, transform_board};

const MAX_FILES: usize = 16;
const MAX_ENCODED_FILE_BYTES: usize = 24 * 1024 * 1024;
const MAX_DECODED_TOTAL_BYTES: usize = 12 * 1024 * 1024;
const MAX_PREVIEW_POINTS: usize = 200_000;

#[derive(Debug, Error, PartialEq)]
pub enum PcbError {
    #[error("PCB job requires at least one source file")]
    MissingFiles,
    #[error("PCB job accepts at most {0} source files")]
    TooManyFiles(usize),
    #[error("PCB source name is invalid: {0}")]
    InvalidSourceName(String),
    #[error("PCB source is too large: {0}")]
    SourceTooLarge(String),
    #[error("PCB source is not valid base64: {0}")]
    InvalidBase64(String),
    #[error("Gerber file {0} is invalid: {1}")]
    InvalidGerber(String, String),
    #[error("Excellon file {0} is invalid: {1}")]
    InvalidExcellon(String, String),
    #[error("Excellon file {0} uses unsupported feature: {1}")]
    UnsupportedExcellonFeature(String, String),
    #[error("Gerber file {0} uses unsupported feature: {1}")]
    UnsupportedGerberFeature(String, String),
    #[error("Gerber file {0} selects no aperture before drawing")]
    MissingAperture(String),
    #[error("PCB geometry operation failed: {0}")]
    Geometry(String),
    #[error("PCB layer has no drawable geometry: {0}")]
    EmptyLayer(String),
    #[error("Excellon file {0} contains a drill hit before a tool selection")]
    DrillWithoutTool(String),
    #[error("Excellon file {0} references unknown tool T{1}")]
    UnknownDrillTool(String, u32),
    #[error("PCB preview exceeds the {0}-point limit")]
    PreviewTooComplex(usize),
    #[error("PCB transform is invalid")]
    InvalidTransform,
    #[error("PCB operation requires the {0} layer")]
    MissingLayer(&'static str),
    #[error("unknown PCB drill group: {0}")]
    UnknownDrillGroup(String),
    #[error("PCB drilling requires a tool mapping for each enabled group")]
    MissingDrillMappings,
    #[error("unknown cutting tool: {0}")]
    UnknownTool(String),
    #[error("tool {tool} cannot be used for PCB {operation}")]
    IncompatibleTool {
        operation: &'static str,
        tool: String,
    },
    #[error("PCB job has no enabled operations")]
    NoOperations,
    #[error("invalid PCB setting: {0}")]
    InvalidSetting(&'static str),
    #[error("generated PCB G-code exceeds {0} bytes")]
    GcodeTooLarge(usize),
    #[error("generated PCB G-code failed validation: {0}")]
    InvalidGeneratedGcode(String),
}

pub fn inspect_pcb(request: PcbInspectRequest) -> Result<PcbInspection, PcbError> {
    let (geometry, _) = parse_board(&request)?;
    Ok(inspection_from_geometry(&geometry))
}

pub(crate) fn parse_board(request: &PcbInspectRequest) -> Result<(BoardGeometry, usize), PcbError> {
    validate_request(request)?;
    let mut board = BoardGeometry::default();
    let mut total_bytes = 0usize;
    for file in &request.files {
        if file.source_base64.len() > MAX_ENCODED_FILE_BYTES {
            return Err(PcbError::SourceTooLarge(file.source_name.clone()));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(file.source_base64.as_bytes())
            .map_err(|_| PcbError::InvalidBase64(file.source_name.clone()))?;
        total_bytes = total_bytes.saturating_add(bytes.len());
        if total_bytes > MAX_DECODED_TOTAL_BYTES {
            return Err(PcbError::SourceTooLarge(file.source_name.clone()));
        }
        if file.role == PcbLayerRole::Drill {
            board
                .drills
                .extend(excellon::parse_drills(&file.source_name, &bytes)?);
        } else {
            board
                .layers
                .push(gerber::parse_gerber(&file.source_name, file.role, &bytes)?);
        }
    }
    transform_board(&mut board, request.transform);
    let point_count = board
        .layers
        .iter()
        .flat_map(|layer| layer.paths.iter())
        .map(|path| path.len())
        .sum::<usize>()
        .saturating_add(board.drills.len());
    if point_count > MAX_PREVIEW_POINTS {
        return Err(PcbError::PreviewTooComplex(MAX_PREVIEW_POINTS));
    }
    Ok((board, total_bytes))
}

pub(crate) fn inspection_from_geometry(board: &BoardGeometry) -> PcbInspection {
    let bounds = geometry::raw_bounds(board).unwrap_or_default();
    let paths = board
        .layers
        .iter()
        .flat_map(|layer| {
            layer.paths.iter().map(|path| PcbPreviewPath {
                role: layer.role,
                closed: true,
                points: path.iter().map(PcbPoint::from).collect(),
            })
        })
        .collect::<Vec<_>>();
    let point_count = paths.iter().map(|path| path.points.len()).sum::<usize>();
    debug_assert!(
        point_count <= MAX_PREVIEW_POINTS,
        "validated before inspection"
    );
    let drill_hits = board
        .drills
        .iter()
        .map(|drill| PcbDrillHit {
            group_key: drill.group_key.clone(),
            point: drill.point,
        })
        .collect::<Vec<_>>();
    let mut grouped = BTreeMap::<String, PcbDrillGroup>::new();
    for drill in &board.drills {
        grouped
            .entry(drill.group_key.clone())
            .and_modify(|group| {
                group.hit_count += 1;
            })
            .or_insert_with(|| PcbDrillGroup {
                key: drill.group_key.clone(),
                source_name: drill.source_name.clone(),
                source_tool_number: drill.source_tool_number,
                diameter_mm: drill.diameter_mm,
                hit_count: 1,
            });
    }
    let mut files = BTreeMap::<(String, PcbLayerRole), usize>::new();
    for layer in &board.layers {
        *files
            .entry((layer.source_name.clone(), layer.role))
            .or_default() += layer.paths.len();
    }
    for drill in &board.drills {
        *files
            .entry((drill.source_name.clone(), PcbLayerRole::Drill))
            .or_default() += 1;
    }
    PcbInspection {
        bounds,
        paths,
        drill_hits,
        drill_groups: grouped.into_values().collect(),
        files: files
            .into_iter()
            .map(|((source_name, role), primitive_count)| PcbFileSummary {
                source_name,
                role,
                primitive_count,
            })
            .collect(),
        warnings: board.warnings.clone(),
    }
}

fn validate_request(request: &PcbInspectRequest) -> Result<(), PcbError> {
    if request.files.is_empty() {
        return Err(PcbError::MissingFiles);
    }
    if request.files.len() > MAX_FILES {
        return Err(PcbError::TooManyFiles(MAX_FILES));
    }
    if !request.transform.offset_x_mm.is_finite()
        || !request.transform.offset_y_mm.is_finite()
        || request.transform.offset_x_mm.abs() > 100_000.0
        || request.transform.offset_y_mm.abs() > 100_000.0
        || request.transform.rotation_quarter_turns > 3
    {
        return Err(PcbError::InvalidTransform);
    }
    for file in &request.files {
        let name = file.source_name.trim();
        if name.is_empty() || name.len() > 255 || name.contains(['/', '\\']) {
            return Err(PcbError::InvalidSourceName(file.source_name.clone()));
        }
    }
    Ok(())
}
