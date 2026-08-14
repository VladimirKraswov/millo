use std::fmt::Write;

use base64::Engine;
use image::{GenericImageView, ImageReader, Limits};
use millo_gcode::{GcodeProgram, MAX_SOURCE_BYTES, ProgramParseRequest, parse_program};
use millo_tooling::CuttingTool;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use usvg::tiny_skia_path::{PathSegment, Point, Transform};

const MAX_ENCODED_SOURCE_BYTES: usize = 12 * 1024 * 1024;
const MAX_DECODED_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 4_096;
const MAX_IMAGE_PIXELS: u64 = 8 * 1024 * 1024;
const MAX_GEOMETRY_POINTS: usize = 120_000;
const MAX_VECTOR_SVG_BYTES: usize = 8 * 1024 * 1024;
const MAX_RECURSION_DEPTH: u8 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImageJobFormat {
    Svg,
    Png,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageJobSettings {
    pub width_mm: f64,
    pub safe_z_mm: f64,
    pub surface_z_mm: f64,
    pub engraving_depth_mm: f64,
    pub feed_mm_per_min: f64,
    pub plunge_mm_per_min: f64,
    pub curve_tolerance_mm: f64,
    pub raster_threshold_percent: u8,
    pub trace_speckle_px: usize,
    pub trace_corner_threshold_degrees: i32,
    pub trace_segment_length_px: f64,
    pub invert: bool,
}

impl Default for ImageJobSettings {
    fn default() -> Self {
        Self {
            width_mm: 50.0,
            safe_z_mm: 3.0,
            surface_z_mm: 0.0,
            engraving_depth_mm: 0.2,
            feed_mm_per_min: 300.0,
            plunge_mm_per_min: 100.0,
            curve_tolerance_mm: 0.08,
            raster_threshold_percent: 50,
            trace_speckle_px: 4,
            trace_corner_threshold_degrees: 60,
            trace_segment_length_px: 4.0,
            invert: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageJobRequest {
    pub source_name: String,
    pub source_base64: String,
    pub format: ImageJobFormat,
    pub settings: ImageJobSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageJobSummary {
    pub width_mm: f64,
    pub height_mm: f64,
    pub path_count: usize,
    pub point_count: usize,
    pub source_width_px: Option<u32>,
    pub source_height_px: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImageJob {
    pub source_name: String,
    pub source: String,
    pub vector_svg: String,
    pub program: GcodeProgram,
    pub summary: ImageJobSummary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SurfacingRasterAxis {
    X,
    Y,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfacingJobSettings {
    pub origin_x_mm: f64,
    pub origin_y_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    pub edge_overrun_mm: f64,
    pub surface_z_mm: f64,
    pub removal_mm: f64,
    pub depth_per_pass_mm: f64,
    pub safe_z_mm: f64,
    pub stepover_percent: f64,
    pub feed_mm_per_min: f64,
    pub plunge_mm_per_min: f64,
    pub raster_axis: SurfacingRasterAxis,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfacingJobRequest {
    pub source_name: String,
    pub tool_id: String,
    pub settings: SurfacingJobSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurfacingJobSummary {
    pub tool_id: String,
    pub tool_name: String,
    pub tool_diameter_mm: f64,
    pub pass_count: usize,
    pub raster_line_count: usize,
    pub stepover_mm: f64,
    pub covered_width_mm: f64,
    pub covered_height_mm: f64,
    pub edge_overrun_mm: f64,
    pub removal_mm: f64,
    pub spindle_rpm: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedSurfacingJob {
    pub source_name: String,
    pub source: String,
    pub program: GcodeProgram,
    pub summary: SurfacingJobSummary,
}

#[derive(Debug, Error, PartialEq)]
pub enum SurfacingJobError {
    #[error("surfacing source name is required")]
    MissingSourceName,
    #[error("tool {0} cannot produce a flat surface")]
    IncompatibleTool(String),
    #[error("surfacing area must be at least one tool diameter on each axis")]
    AreaSmallerThanTool,
    #[error("edge overrun cannot exceed the tool radius")]
    EdgeOverrunExceedsRadius,
    #[error("invalid surfacing setting: {0}")]
    InvalidSetting(&'static str),
    #[error("surfacing plan exceeds the {max_lines} raster-line limit")]
    TooManyRasterLines { max_lines: usize },
    #[error("generated G-code exceeds the {max_bytes} byte limit")]
    GcodeTooLarge { max_bytes: usize },
    #[error("generated G-code failed validation: {0}")]
    InvalidGeneratedGcode(String),
}

const MAX_SURFACING_RASTER_LINES: usize = 20_000;

pub fn generate_surfacing_job(
    request: SurfacingJobRequest,
    tool: &CuttingTool,
) -> Result<GeneratedSurfacingJob, SurfacingJobError> {
    validate_surfacing_settings(&request.settings)?;
    if request.source_name.trim().is_empty() {
        return Err(SurfacingJobError::MissingSourceName);
    }
    if request.tool_id != tool.id || !tool.supports_surfacing() {
        return Err(SurfacingJobError::IncompatibleTool(tool.name.clone()));
    }
    if request.settings.width_mm < tool.diameter_mm || request.settings.height_mm < tool.diameter_mm
    {
        return Err(SurfacingJobError::AreaSmallerThanTool);
    }
    let radius = tool.diameter_mm / 2.0;
    if request.settings.edge_overrun_mm > radius {
        return Err(SurfacingJobError::EdgeOverrunExceedsRadius);
    }

    let stepover_mm = tool.diameter_mm * request.settings.stepover_percent / 100.0;
    if !stepover_mm.is_finite() || stepover_mm <= 0.0 {
        return Err(SurfacingJobError::InvalidSetting("stepoverPercent"));
    }
    let cross_span = match request.settings.raster_axis {
        SurfacingRasterAxis::X => {
            request.settings.height_mm - tool.diameter_mm + request.settings.edge_overrun_mm * 2.0
        }
        SurfacingRasterAxis::Y => {
            request.settings.width_mm - tool.diameter_mm + request.settings.edge_overrun_mm * 2.0
        }
    };
    let raster_offsets = bounded_raster_offsets(cross_span, stepover_mm)?;
    let pass_count = (request.settings.removal_mm / request.settings.depth_per_pass_mm)
        .ceil()
        .max(1.0) as usize;
    let total_lines = raster_offsets.len().saturating_mul(pass_count);
    if total_lines > MAX_SURFACING_RASTER_LINES {
        return Err(SurfacingJobError::TooManyRasterLines {
            max_lines: MAX_SURFACING_RASTER_LINES,
        });
    }

    let source_name = gcode_name(request.source_name.trim());
    let mut source = String::with_capacity(total_lines.saturating_mul(72));
    let settings = &request.settings;
    let min_x = settings.origin_x_mm + radius - settings.edge_overrun_mm;
    let max_x = settings.origin_x_mm + settings.width_mm - radius + settings.edge_overrun_mm;
    let min_y = settings.origin_y_mm + radius - settings.edge_overrun_mm;
    let max_y = settings.origin_y_mm + settings.height_mm - radius + settings.edge_overrun_mm;
    writeln!(
        &mut source,
        "(Millo surfacing job: {})",
        comment_text(&source_name)
    )
    .unwrap();
    writeln!(&mut source, "(Tool: {})", comment_text(&tool.name)).unwrap();
    writeln!(
        &mut source,
        "(Diameter {} mm; recommended spindle {} rpm)",
        number(tool.diameter_mm),
        tool.spindle_rpm
    )
    .unwrap();
    writeln!(
        &mut source,
        "(Area origin X{} Y{}; first cutter center X{} Y{})",
        number(settings.origin_x_mm),
        number(settings.origin_y_mm),
        number(min_x),
        number(min_y)
    )
    .unwrap();
    writeln!(
        &mut source,
        "(Safe approach: retract Z{} before rapid XY)",
        number(settings.safe_z_mm)
    )
    .unwrap();
    writeln!(&mut source, "G21 G90 G94 G17").unwrap();
    writeln!(&mut source, "M5").unwrap();
    writeln!(&mut source, "M9").unwrap();
    writeln!(&mut source, "G0 Z{}", number(settings.safe_z_mm)).unwrap();

    for pass in 1..=pass_count {
        let removed = (settings.depth_per_pass_mm * pass as f64).min(settings.removal_mm);
        let depth = settings.surface_z_mm - removed;
        let start = raster_point(
            settings.raster_axis,
            min_x,
            max_x,
            min_y,
            max_y,
            raster_offsets[0],
            false,
        );
        writeln!(
            &mut source,
            "(Pass {pass}/{pass_count}; Z {})",
            number(depth)
        )
        .unwrap();
        writeln!(&mut source, "G0 X{} Y{}", number(start.0), number(start.1)).unwrap();
        writeln!(
            &mut source,
            "G1 Z{} F{}",
            number(depth),
            number(settings.plunge_mm_per_min)
        )
        .unwrap();
        for (index, offset) in raster_offsets.iter().copied().enumerate() {
            let end = raster_point(
                settings.raster_axis,
                min_x,
                max_x,
                min_y,
                max_y,
                offset,
                index % 2 == 0,
            );
            writeln!(
                &mut source,
                "G1 X{} Y{} F{}",
                number(end.0),
                number(end.1),
                number(settings.feed_mm_per_min)
            )
            .unwrap();
            if let Some(next_offset) = raster_offsets.get(index + 1) {
                let cross = raster_point(
                    settings.raster_axis,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                    *next_offset,
                    index % 2 == 0,
                );
                writeln!(
                    &mut source,
                    "G1 X{} Y{} F{}",
                    number(cross.0),
                    number(cross.1),
                    number(settings.feed_mm_per_min)
                )
                .unwrap();
            }
            if source.len() > MAX_SOURCE_BYTES {
                return Err(SurfacingJobError::GcodeTooLarge {
                    max_bytes: MAX_SOURCE_BYTES,
                });
            }
        }
        writeln!(&mut source, "G0 Z{}", number(settings.safe_z_mm)).unwrap();
    }
    writeln!(&mut source, "M5").unwrap();
    writeln!(&mut source, "M9").unwrap();
    writeln!(&mut source, "M30").unwrap();
    if source.len() > MAX_SOURCE_BYTES {
        return Err(SurfacingJobError::GcodeTooLarge {
            max_bytes: MAX_SOURCE_BYTES,
        });
    }
    let program = parse_program(ProgramParseRequest {
        source_name: source_name.clone(),
        source: source.clone(),
    })
    .map_err(|error| SurfacingJobError::InvalidGeneratedGcode(error.to_string()))?;

    Ok(GeneratedSurfacingJob {
        source_name,
        source,
        program,
        summary: SurfacingJobSummary {
            tool_id: tool.id.clone(),
            tool_name: tool.name.clone(),
            tool_diameter_mm: tool.diameter_mm,
            pass_count,
            raster_line_count: total_lines,
            stepover_mm,
            covered_width_mm: settings.width_mm,
            covered_height_mm: settings.height_mm,
            edge_overrun_mm: settings.edge_overrun_mm,
            removal_mm: settings.removal_mm,
            spindle_rpm: tool.spindle_rpm,
        },
    })
}

fn validate_surfacing_settings(settings: &SurfacingJobSettings) -> Result<(), SurfacingJobError> {
    surfacing_range(settings.origin_x_mm, -100_000.0, 100_000.0, "originXMm")?;
    surfacing_range(settings.origin_y_mm, -100_000.0, 100_000.0, "originYMm")?;
    surfacing_range(settings.width_mm, 0.1, 100_000.0, "widthMm")?;
    surfacing_range(settings.height_mm, 0.1, 100_000.0, "heightMm")?;
    surfacing_range(settings.edge_overrun_mm, 0.0, 250.0, "edgeOverrunMm")?;
    surfacing_range(settings.surface_z_mm, -10_000.0, 10_000.0, "surfaceZMm")?;
    surfacing_range(settings.removal_mm, 0.001, 100.0, "removalMm")?;
    surfacing_range(settings.depth_per_pass_mm, 0.001, 20.0, "depthPerPassMm")?;
    surfacing_range(settings.safe_z_mm, -10_000.0, 10_000.0, "safeZMm")?;
    surfacing_range(settings.stepover_percent, 1.0, 95.0, "stepoverPercent")?;
    surfacing_range(settings.feed_mm_per_min, 1.0, 100_000.0, "feedMmPerMin")?;
    surfacing_range(settings.plunge_mm_per_min, 1.0, 50_000.0, "plungeMmPerMin")?;
    if settings.safe_z_mm <= settings.surface_z_mm
        || settings.depth_per_pass_mm > settings.removal_mm
    {
        return Err(SurfacingJobError::InvalidSetting("Z envelope"));
    }
    Ok(())
}

fn surfacing_range(
    value: f64,
    minimum: f64,
    maximum: f64,
    name: &'static str,
) -> Result<(), SurfacingJobError> {
    if value.is_finite() && (minimum..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(SurfacingJobError::InvalidSetting(name))
    }
}

fn bounded_raster_offsets(span: f64, step: f64) -> Result<Vec<f64>, SurfacingJobError> {
    if span <= f64::EPSILON {
        return Ok(vec![0.0]);
    }
    let count = (span / step).ceil() as usize + 1;
    if count > MAX_SURFACING_RASTER_LINES {
        return Err(SurfacingJobError::TooManyRasterLines {
            max_lines: MAX_SURFACING_RASTER_LINES,
        });
    }
    let mut offsets = (0..count.saturating_sub(1))
        .map(|index| (index as f64 * step).min(span))
        .collect::<Vec<_>>();
    offsets.push(span);
    offsets.dedup_by(|left, right| (*left - *right).abs() < 0.000_001);
    Ok(offsets)
}

#[allow(clippy::too_many_arguments)]
fn raster_point(
    axis: SurfacingRasterAxis,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
    offset: f64,
    high: bool,
) -> (f64, f64) {
    match axis {
        SurfacingRasterAxis::X => (if high { max_x } else { min_x }, min_y + offset),
        SurfacingRasterAxis::Y => (min_x + offset, if high { max_y } else { min_y }),
    }
}

#[derive(Debug, Error, PartialEq)]
pub enum ImageJobError {
    #[error("image source name is required")]
    MissingSourceName,
    #[error("encoded image exceeds the {max_bytes} byte limit")]
    EncodedSourceTooLarge { max_bytes: usize },
    #[error("image data is not valid base64")]
    InvalidBase64,
    #[error("decoded image exceeds the {max_bytes} byte limit")]
    SourceTooLarge { max_bytes: usize },
    #[error("invalid image job setting: {0}")]
    InvalidSetting(&'static str),
    #[error("SVG could not be parsed: {0}")]
    InvalidSvg(String),
    #[error("PNG could not be decoded: {0}")]
    InvalidPng(String),
    #[error("raster image exceeds the {max_pixels} pixel limit")]
    RasterTooLarge { max_pixels: u64 },
    #[error("vectorized SVG exceeds the {max_bytes} byte limit")]
    VectorSvgTooLarge { max_bytes: usize },
    #[error("image contains no engravable geometry")]
    EmptyGeometry,
    #[error("generated geometry exceeds the {max_points} point limit")]
    GeometryTooComplex { max_points: usize },
    #[error("generated G-code exceeds the {max_bytes} byte limit")]
    GcodeTooLarge { max_bytes: usize },
    #[error("generated G-code failed validation: {0}")]
    InvalidGeneratedGcode(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MillPoint {
    x: f64,
    y: f64,
}

type MillPath = Vec<MillPoint>;

#[derive(Debug)]
struct Geometry {
    paths: Vec<MillPath>,
    width_mm: f64,
    height_mm: f64,
    source_width_px: Option<u32>,
    source_height_px: Option<u32>,
}

pub fn generate_image_job(request: ImageJobRequest) -> Result<GeneratedImageJob, ImageJobError> {
    validate_settings(&request.settings)?;
    let source_name = request.source_name.trim();
    if source_name.is_empty() {
        return Err(ImageJobError::MissingSourceName);
    }
    if request.source_base64.len() > MAX_ENCODED_SOURCE_BYTES {
        return Err(ImageJobError::EncodedSourceTooLarge {
            max_bytes: MAX_ENCODED_SOURCE_BYTES,
        });
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(request.source_base64)
        .map_err(|_| ImageJobError::InvalidBase64)?;
    if bytes.len() > MAX_DECODED_SOURCE_BYTES {
        return Err(ImageJobError::SourceTooLarge {
            max_bytes: MAX_DECODED_SOURCE_BYTES,
        });
    }

    let (geometry, vector_svg) = match request.format {
        ImageJobFormat::Svg => {
            let geometry = svg_geometry(&bytes, &request.settings)?;
            let vector_svg = String::from_utf8(bytes)
                .map_err(|error| ImageJobError::InvalidSvg(error.to_string()))?;
            (geometry, vector_svg)
        }
        ImageJobFormat::Png => png_geometry(&bytes, &request.settings)?,
    };
    let point_count = geometry.paths.iter().map(Vec::len).sum();
    if point_count > MAX_GEOMETRY_POINTS {
        return Err(ImageJobError::GeometryTooComplex {
            max_points: MAX_GEOMETRY_POINTS,
        });
    }

    let gcode_name = gcode_name(source_name);
    let source = write_gcode(&geometry.paths, &request.settings, &gcode_name)?;
    let program = parse_program(ProgramParseRequest {
        source_name: gcode_name.clone(),
        source: source.clone(),
    })
    .map_err(|error| ImageJobError::InvalidGeneratedGcode(error.to_string()))?;

    Ok(GeneratedImageJob {
        source_name: gcode_name,
        source,
        vector_svg,
        program,
        summary: ImageJobSummary {
            width_mm: geometry.width_mm,
            height_mm: geometry.height_mm,
            path_count: geometry.paths.len(),
            point_count,
            source_width_px: geometry.source_width_px,
            source_height_px: geometry.source_height_px,
        },
    })
}

fn validate_settings(settings: &ImageJobSettings) -> Result<(), ImageJobError> {
    validate_range(settings.width_mm, 0.1, 5_000.0, "widthMm")?;
    validate_range(settings.safe_z_mm, -1_000.0, 1_000.0, "safeZMm")?;
    validate_range(settings.surface_z_mm, -1_000.0, 1_000.0, "surfaceZMm")?;
    validate_range(settings.engraving_depth_mm, 0.001, 10.0, "engravingDepthMm")?;
    validate_range(settings.feed_mm_per_min, 1.0, 20_000.0, "feedMmPerMin")?;
    validate_range(settings.plunge_mm_per_min, 1.0, 10_000.0, "plungeMmPerMin")?;
    validate_range(settings.curve_tolerance_mm, 0.005, 2.0, "curveToleranceMm")?;
    if !(1..=99).contains(&settings.raster_threshold_percent) {
        return Err(ImageJobError::InvalidSetting("rasterThresholdPercent"));
    }
    if !(1..=64).contains(&settings.trace_speckle_px) {
        return Err(ImageJobError::InvalidSetting("traceSpecklePx"));
    }
    if !(1..=180).contains(&settings.trace_corner_threshold_degrees) {
        return Err(ImageJobError::InvalidSetting("traceCornerThresholdDegrees"));
    }
    validate_range(
        settings.trace_segment_length_px,
        1.0,
        32.0,
        "traceSegmentLengthPx",
    )?;
    if settings.safe_z_mm <= settings.surface_z_mm {
        return Err(ImageJobError::InvalidSetting(
            "safeZMm must be above surfaceZMm",
        ));
    }
    Ok(())
}

fn validate_range(
    value: f64,
    minimum: f64,
    maximum: f64,
    name: &'static str,
) -> Result<(), ImageJobError> {
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        return Err(ImageJobError::InvalidSetting(name));
    }
    Ok(())
}

fn svg_geometry(bytes: &[u8], settings: &ImageJobSettings) -> Result<Geometry, ImageJobError> {
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default())
        .map_err(|error| ImageJobError::InvalidSvg(error.to_string()))?;
    let mut source_paths = Vec::new();
    let document_width = f64::from(tree.size().width()).max(f64::EPSILON);
    let source_tolerance = settings.curve_tolerance_mm / (settings.width_mm / document_width);
    let mut point_count = 0;
    collect_svg_group(
        tree.root(),
        source_tolerance,
        &mut source_paths,
        &mut point_count,
    )?;
    normalize_paths(source_paths, settings.width_mm)
}

fn collect_svg_group(
    group: &usvg::Group,
    tolerance: f64,
    output: &mut Vec<MillPath>,
    point_count: &mut usize,
) -> Result<(), ImageJobError> {
    for node in group.children() {
        match node {
            usvg::Node::Group(child) => collect_svg_group(child, tolerance, output, point_count)?,
            usvg::Node::Path(path) if path.is_visible() => {
                flatten_svg_path(
                    path.data(),
                    path.abs_transform(),
                    tolerance,
                    output,
                    point_count,
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn flatten_svg_path(
    path: &usvg::tiny_skia_path::Path,
    transform: Transform,
    tolerance: f64,
    output: &mut Vec<MillPath>,
    point_count: &mut usize,
) -> Result<(), ImageJobError> {
    let mut current = MillPath::new();
    let mut cursor = MillPoint { x: 0.0, y: 0.0 };
    let mut start = cursor;
    for segment in path.segments() {
        match segment {
            PathSegment::MoveTo(point) => {
                finish_path(&mut current, output, point_count)?;
                cursor = transformed(point, transform);
                start = cursor;
                current.push(cursor);
            }
            PathSegment::LineTo(point) => {
                cursor = transformed(point, transform);
                push_distinct(&mut current, cursor);
            }
            PathSegment::QuadTo(control, end) => {
                let control = transformed(control, transform);
                let end = transformed(end, transform);
                flatten_quadratic(cursor, control, end, tolerance, 0, &mut current)?;
                cursor = end;
            }
            PathSegment::CubicTo(control_a, control_b, end) => {
                let control_a = transformed(control_a, transform);
                let control_b = transformed(control_b, transform);
                let end = transformed(end, transform);
                flatten_cubic(
                    cursor,
                    control_a,
                    control_b,
                    end,
                    tolerance,
                    0,
                    &mut current,
                )?;
                cursor = end;
            }
            PathSegment::Close => {
                push_distinct(&mut current, start);
                finish_path(&mut current, output, point_count)?;
                cursor = start;
            }
        }
    }
    finish_path(&mut current, output, point_count)?;
    Ok(())
}

fn transformed(point: Point, transform: Transform) -> MillPoint {
    let mut point = point;
    transform.map_point(&mut point);
    MillPoint {
        x: f64::from(point.x),
        y: f64::from(point.y),
    }
}

fn flatten_quadratic(
    start: MillPoint,
    control: MillPoint,
    end: MillPoint,
    tolerance: f64,
    depth: u8,
    output: &mut MillPath,
) -> Result<(), ImageJobError> {
    ensure_capacity(output.len())?;
    if depth >= MAX_RECURSION_DEPTH || point_line_distance(control, start, end) <= tolerance {
        push_distinct(output, end);
        return Ok(());
    }
    let start_control = midpoint(start, control);
    let control_end = midpoint(control, end);
    let middle = midpoint(start_control, control_end);
    flatten_quadratic(start, start_control, middle, tolerance, depth + 1, output)?;
    flatten_quadratic(middle, control_end, end, tolerance, depth + 1, output)
}

#[allow(clippy::too_many_arguments)]
fn flatten_cubic(
    start: MillPoint,
    control_a: MillPoint,
    control_b: MillPoint,
    end: MillPoint,
    tolerance: f64,
    depth: u8,
    output: &mut MillPath,
) -> Result<(), ImageJobError> {
    ensure_capacity(output.len())?;
    let flatness =
        point_line_distance(control_a, start, end).max(point_line_distance(control_b, start, end));
    if depth >= MAX_RECURSION_DEPTH || flatness <= tolerance {
        push_distinct(output, end);
        return Ok(());
    }
    let start_a = midpoint(start, control_a);
    let a_b = midpoint(control_a, control_b);
    let b_end = midpoint(control_b, end);
    let left_control_b = midpoint(start_a, a_b);
    let right_control_a = midpoint(a_b, b_end);
    let middle = midpoint(left_control_b, right_control_a);
    flatten_cubic(
        start,
        start_a,
        left_control_b,
        middle,
        tolerance,
        depth + 1,
        output,
    )?;
    flatten_cubic(
        middle,
        right_control_a,
        b_end,
        end,
        tolerance,
        depth + 1,
        output,
    )
}

fn finish_path(
    current: &mut MillPath,
    output: &mut Vec<MillPath>,
    point_count: &mut usize,
) -> Result<(), ImageJobError> {
    if current.len() >= 2 {
        *point_count = point_count.saturating_add(current.len());
        if *point_count > MAX_GEOMETRY_POINTS {
            return Err(ImageJobError::GeometryTooComplex {
                max_points: MAX_GEOMETRY_POINTS,
            });
        }
        output.push(std::mem::take(current));
    } else {
        current.clear();
    }
    Ok(())
}

fn push_distinct(path: &mut MillPath, point: MillPoint) {
    if path
        .last()
        .is_none_or(|last| distance(*last, point) > f64::EPSILON)
    {
        path.push(point);
    }
}

fn png_geometry(
    bytes: &[u8],
    settings: &ImageJobSettings,
) -> Result<(Geometry, String), ImageJobError> {
    let cursor = std::io::Cursor::new(bytes);
    let mut reader = ImageReader::with_format(cursor, image::ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| ImageJobError::InvalidPng(error.to_string()))?;
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return Err(ImageJobError::EmptyGeometry);
    }
    if u64::from(width).saturating_mul(u64::from(height)) > MAX_IMAGE_PIXELS {
        return Err(ImageJobError::RasterTooLarge {
            max_pixels: MAX_IMAGE_PIXELS,
        });
    }
    let rgba = image.to_rgba8();
    let threshold = f64::from(settings.raster_threshold_percent) / 100.0;
    let mut pixels = Vec::with_capacity(rgba.len());
    for pixel in rgba.pixels() {
        let pixel = pixel.0;
        let alpha = f64::from(pixel[3]) / 255.0;
        let luminance = (0.2126 * f64::from(pixel[0])
            + 0.7152 * f64::from(pixel[1])
            + 0.0722 * f64::from(pixel[2]))
            / 255.0;
        let composited = luminance * alpha + (1.0 - alpha);
        let active = if settings.invert {
            composited >= threshold
        } else {
            composited <= threshold
        };
        let value = if active { 0 } else { 255 };
        pixels.extend_from_slice(&[value, value, value, 255]);
    }
    let trace_image = vtracer::ColorImage {
        pixels,
        width: width as usize,
        height: height as usize,
    };
    let mut config = vtracer::Config::from_preset(vtracer::Preset::Bw);
    config.filter_speckle = settings.trace_speckle_px;
    config.corner_threshold = settings.trace_corner_threshold_degrees;
    config.length_threshold = settings.trace_segment_length_px;
    config.path_precision = Some(3);
    let vector_svg = config
        .build()
        .and_then(|pipeline| pipeline.to_svg(&trace_image))
        .map_err(|error| ImageJobError::InvalidPng(error.to_string()))?;
    if vector_svg.len() > MAX_VECTOR_SVG_BYTES {
        return Err(ImageJobError::VectorSvgTooLarge {
            max_bytes: MAX_VECTOR_SVG_BYTES,
        });
    }
    let mut geometry = svg_geometry(vector_svg.as_bytes(), settings)?;
    geometry.source_width_px = Some(width);
    geometry.source_height_px = Some(height);
    Ok((geometry, vector_svg))
}

fn normalize_paths(paths: Vec<MillPath>, target_width: f64) -> Result<Geometry, ImageJobError> {
    let mut points = paths.iter().flatten();
    let Some(first) = points.next().copied() else {
        return Err(ImageJobError::EmptyGeometry);
    };
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (first.x, first.x, first.y, first.y);
    let mut point_count = 1usize;
    for point in points {
        point_count += 1;
        if point_count > MAX_GEOMETRY_POINTS {
            return Err(ImageJobError::GeometryTooComplex {
                max_points: MAX_GEOMETRY_POINTS,
            });
        }
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        min_y = min_y.min(point.y);
        max_y = max_y.max(point.y);
    }
    let source_width = max_x - min_x;
    let source_height = max_y - min_y;
    if source_width <= f64::EPSILON || source_height <= f64::EPSILON {
        return Err(ImageJobError::EmptyGeometry);
    }
    let scale = target_width / source_width;
    let height_mm = source_height * scale;
    let paths = paths
        .into_iter()
        .map(|path| {
            path.into_iter()
                .map(|point| MillPoint {
                    x: (point.x - min_x) * scale,
                    y: (max_y - point.y) * scale,
                })
                .collect()
        })
        .collect();
    Ok(Geometry {
        paths,
        width_mm: target_width,
        height_mm,
        source_width_px: None,
        source_height_px: None,
    })
}

fn write_gcode(
    paths: &[MillPath],
    settings: &ImageJobSettings,
    source_name: &str,
) -> Result<String, ImageJobError> {
    let mut source = String::with_capacity(paths.len().saturating_mul(96));
    let depth = settings.surface_z_mm - settings.engraving_depth_mm;
    writeln!(
        &mut source,
        "(Millo image job: {})",
        comment_text(source_name)
    )
    .unwrap();
    writeln!(&mut source, "G21 G90 G94 G17").unwrap();
    writeln!(&mut source, "M5").unwrap();
    writeln!(&mut source, "M9").unwrap();
    writeln!(&mut source, "G0 Z{}", number(settings.safe_z_mm)).unwrap();
    for path in paths {
        let Some(first) = path.first() else { continue };
        writeln!(&mut source, "G0 X{} Y{}", number(first.x), number(first.y)).unwrap();
        writeln!(
            &mut source,
            "G1 Z{} F{}",
            number(depth),
            number(settings.plunge_mm_per_min)
        )
        .unwrap();
        for point in path.iter().skip(1) {
            writeln!(
                &mut source,
                "G1 X{} Y{} F{}",
                number(point.x),
                number(point.y),
                number(settings.feed_mm_per_min)
            )
            .unwrap();
            if source.len() > MAX_SOURCE_BYTES {
                return Err(ImageJobError::GcodeTooLarge {
                    max_bytes: MAX_SOURCE_BYTES,
                });
            }
        }
        writeln!(&mut source, "G0 Z{}", number(settings.safe_z_mm)).unwrap();
    }
    writeln!(&mut source, "G0 X0 Y0").unwrap();
    writeln!(&mut source, "M5").unwrap();
    writeln!(&mut source, "M9").unwrap();
    writeln!(&mut source, "M30").unwrap();
    if source.len() > MAX_SOURCE_BYTES {
        return Err(ImageJobError::GcodeTooLarge {
            max_bytes: MAX_SOURCE_BYTES,
        });
    }
    Ok(source)
}

fn gcode_name(source_name: &str) -> String {
    let stem = source_name
        .rsplit_once('.')
        .map_or(source_name, |(stem, _)| stem)
        .chars()
        .map(|character| match character {
            '(' | ')' | '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\n' | '\r' => '_',
            other => other,
        })
        .take(180)
        .collect::<String>();
    let stem = stem.trim().trim_matches('.');
    format!("{}.nc", if stem.is_empty() { "image-job" } else { stem })
}

fn comment_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !matches!(character, '(' | ')' | '\n' | '\r'))
        .take(180)
        .collect()
}

fn number(value: f64) -> String {
    let mut value = format!("{value:.4}");
    while value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    if value == "-0" { "0".to_owned() } else { value }
}

fn ensure_capacity(length: usize) -> Result<(), ImageJobError> {
    if length >= MAX_GEOMETRY_POINTS {
        Err(ImageJobError::GeometryTooComplex {
            max_points: MAX_GEOMETRY_POINTS,
        })
    } else {
        Ok(())
    }
}

fn midpoint(a: MillPoint, b: MillPoint) -> MillPoint {
    MillPoint {
        x: (a.x + b.x) / 2.0,
        y: (a.y + b.y) / 2.0,
    }
}

fn distance(a: MillPoint, b: MillPoint) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn point_line_distance(point: MillPoint, start: MillPoint, end: MillPoint) -> f64 {
    let line_length = distance(start, end);
    if line_length <= f64::EPSILON {
        return distance(point, start);
    }
    let area_twice =
        ((end.x - start.x) * (start.y - point.y) - (start.x - point.x) * (end.y - start.y)).abs();
    area_twice / line_length
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use image::{DynamicImage, ImageBuffer, ImageFormat, Luma};

    use super::*;

    fn encoded(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn surfacing_tool() -> CuttingTool {
        millo_tooling::factory_presets()
            .into_iter()
            .find(|tool| tool.id == "preset-carbide3d-mcfly")
            .unwrap()
    }

    fn surfacing_request() -> SurfacingJobRequest {
        SurfacingJobRequest {
            source_name: "spoilboard.nc".to_owned(),
            tool_id: "preset-carbide3d-mcfly".to_owned(),
            settings: SurfacingJobSettings {
                origin_x_mm: 0.0,
                origin_y_mm: 0.0,
                width_mm: 100.0,
                height_mm: 80.0,
                edge_overrun_mm: 0.0,
                surface_z_mm: 0.0,
                removal_mm: 0.4,
                depth_per_pass_mm: 0.2,
                safe_z_mm: 5.0,
                stepover_percent: 45.0,
                feed_mm_per_min: 1_000.0,
                plunge_mm_per_min: 250.0,
                raster_axis: SurfacingRasterAxis::X,
            },
        }
    }

    #[test]
    fn creates_valid_spindle_free_gcode_from_transformed_svg() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10">
          <g transform="translate(2 1)"><path d="M0 0 C5 0 5 8 10 8 L16 0 Z"/></g>
        </svg>"#;
        let result = generate_image_job(ImageJobRequest {
            source_name: "mark.svg".to_owned(),
            source_base64: encoded(svg),
            format: ImageJobFormat::Svg,
            settings: ImageJobSettings {
                width_mm: 40.0,
                ..ImageJobSettings::default()
            },
        })
        .unwrap();

        assert_eq!(result.source_name, "mark.nc");
        assert!((result.summary.width_mm - 40.0).abs() < 0.001);
        assert!(result.summary.path_count >= 1);
        assert!(result.summary.point_count > 4);
        assert!(result.source.contains("G1 Z-0.2 F100"));
        assert!(
            !result
                .source
                .lines()
                .any(|line| line == "M3" || line == "M4")
        );
        assert!(!result.program.features.has_spindle_activation);
        assert!(result.program.summary.dry_run_eligible);
        assert!(result.program.summary.bounds.unwrap().size.x >= 39.99);
    }

    #[test]
    fn vectorizes_png_to_svg_before_creating_gcode() {
        let image = ImageBuffer::from_fn(32, 24, |x, y| {
            if (4..=13).contains(&x) && (4..=18).contains(&y) {
                Luma([0u8])
            } else {
                Luma([255u8])
            }
        });
        let mut png = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageLuma8(image)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let result = generate_image_job(ImageJobRequest {
            source_name: "pixels.png".to_owned(),
            source_base64: encoded(png.get_ref()),
            format: ImageJobFormat::Png,
            settings: ImageJobSettings {
                width_mm: 8.0,
                trace_speckle_px: 1,
                ..ImageJobSettings::default()
            },
        })
        .unwrap();

        assert_eq!(result.summary.source_width_px, Some(32));
        assert_eq!(result.summary.source_height_px, Some(24));
        assert!(result.vector_svg.contains("<svg"));
        assert!(result.vector_svg.contains("<path"));
        assert!(result.summary.path_count >= 1);
        assert!(result.program.summary.motion_count > 0);
        assert!(result.program.warnings.is_empty());
    }

    #[test]
    fn rejects_invalid_settings_before_decoding() {
        let error = generate_image_job(ImageJobRequest {
            source_name: "bad.svg".to_owned(),
            source_base64: "not base64".to_owned(),
            format: ImageJobFormat::Svg,
            settings: ImageJobSettings {
                safe_z_mm: 0.0,
                ..ImageJobSettings::default()
            },
        })
        .unwrap_err();

        assert_eq!(
            error,
            ImageJobError::InvalidSetting("safeZMm must be above surfaceZMm")
        );
    }

    #[test]
    fn rejects_blank_raster() {
        let image = ImageBuffer::from_pixel(4, 4, Luma([255u8]));
        let mut png = std::io::Cursor::new(Vec::new());
        DynamicImage::ImageLuma8(image)
            .write_to(&mut png, ImageFormat::Png)
            .unwrap();
        let error = generate_image_job(ImageJobRequest {
            source_name: "blank.png".to_owned(),
            source_base64: encoded(png.get_ref()),
            format: ImageJobFormat::Png,
            settings: ImageJobSettings::default(),
        })
        .unwrap_err();

        assert_eq!(error, ImageJobError::EmptyGeometry);
    }

    #[test]
    fn bounds_aggregate_svg_geometry_during_collection() {
        let mut current = vec![MillPoint { x: 0.0, y: 0.0 }; MAX_GEOMETRY_POINTS + 1];
        let mut output = Vec::new();
        let mut point_count = 0;

        let error = finish_path(&mut current, &mut output, &mut point_count).unwrap_err();

        assert_eq!(
            error,
            ImageJobError::GeometryTooComplex {
                max_points: MAX_GEOMETRY_POINTS
            }
        );
        assert!(output.is_empty());
    }

    #[test]
    fn generated_file_name_is_a_bounded_leaf_name() {
        assert_eq!(gcode_name("../unsafe:name.svg"), "_unsafe_name.nc");
        assert!(gcode_name(&"a".repeat(400)).len() <= 183);
    }

    #[test]
    fn creates_a_bounded_multi_pass_spindle_free_surfacing_job() {
        let result = generate_surfacing_job(surfacing_request(), &surfacing_tool()).unwrap();

        assert_eq!(result.summary.pass_count, 2);
        assert_eq!(result.summary.tool_diameter_mm, 25.4);
        assert!(result.summary.raster_line_count >= 10);
        assert!(result.source.contains("(Pass 1/2; Z -0.2)"));
        assert!(result.source.contains("(Pass 2/2; Z -0.4)"));
        assert!(
            !result
                .source
                .lines()
                .any(|line| line == "M3" || line == "M4")
        );
        assert!(result.program.summary.dry_run_eligible);
        assert!(result.program.warnings.is_empty());
        let bounds = result.program.summary.bounds.unwrap();
        assert_eq!(bounds.min.x, 0.0);
        assert!((bounds.max.x - 87.3).abs() < 0.001);
    }

    #[test]
    fn surfacing_rejects_non_flat_tools_and_areas_smaller_than_the_cutter() {
        let ball = millo_tooling::factory_presets()
            .into_iter()
            .find(|tool| matches!(tool.kind, millo_tooling::ToolKind::BallNose))
            .unwrap();
        let mut request = surfacing_request();
        request.tool_id = ball.id.clone();
        assert!(matches!(
            generate_surfacing_job(request, &ball),
            Err(SurfacingJobError::IncompatibleTool(_))
        ));

        let mut request = surfacing_request();
        request.settings.width_mm = 20.0;
        assert_eq!(
            generate_surfacing_job(request, &surfacing_tool()).unwrap_err(),
            SurfacingJobError::AreaSmallerThanTool
        );
    }

    #[test]
    fn surfacing_finishes_the_cross_axis_without_exceeding_the_line_limit() {
        let result = generate_surfacing_job(surfacing_request(), &surfacing_tool()).unwrap();
        let bounds = result.program.summary.bounds.unwrap();
        assert!((bounds.max.y - 67.3).abs() < 0.001);

        let mut request = surfacing_request();
        request.settings.width_mm = 100_000.0;
        request.settings.height_mm = 100_000.0;
        request.settings.stepover_percent = 1.0;
        assert!(matches!(
            generate_surfacing_job(request, &surfacing_tool()),
            Err(SurfacingJobError::TooManyRasterLines { .. })
        ));
    }

    #[test]
    fn surfacing_edge_overrun_is_bounded_by_the_tool_radius() {
        let tool = surfacing_tool();
        let mut request = surfacing_request();
        request.settings.edge_overrun_mm = tool.diameter_mm / 2.0;
        let result = generate_surfacing_job(request, &tool).unwrap();
        let bounds = result.program.summary.bounds.unwrap();

        assert_eq!(result.summary.edge_overrun_mm, 12.7);
        assert!(
            result
                .source
                .contains("(Area origin X0 Y0; first cutter center X0 Y0)")
        );
        assert!(
            result
                .source
                .contains("G0 Z5\n(Pass 1/2; Z -0.2)\nG0 X0 Y0")
        );
        assert!((bounds.max.x - 100.0).abs() < 0.001);
        assert!((bounds.max.y - 80.0).abs() < 0.001);

        let mut request = surfacing_request();
        request.settings.edge_overrun_mm = tool.diameter_mm / 2.0 + 0.1;
        assert_eq!(
            generate_surfacing_job(request, &tool).unwrap_err(),
            SurfacingJobError::EdgeOverrunExceedsRadius
        );
    }
}
