use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

use clipper2::{EndType, JoinType, Milli, Path};
use millo_gcode::{MAX_SOURCE_BYTES, ProgramParseRequest, parse_program};
use millo_tooling::{CuttingTool, ToolKind};

use crate::{
    GeneratedPcbJob, PcbError, PcbJobRequest, PcbJobSummary, PcbLayerRole, PcbOperationSummary,
    geometry::{BoardGeometry, CamPaths},
    inspection_from_geometry, parse_board,
};

pub fn generate_pcb_job(
    request: PcbJobRequest,
    tools: &[CuttingTool],
) -> Result<GeneratedPcbJob, PcbError> {
    validate_settings(&request)?;
    let (geometry, _) = parse_board(&request.board)?;
    let inspection = inspection_from_geometry(&geometry);
    let tool_lookup = tools
        .iter()
        .map(|tool| (tool.id.as_str(), tool))
        .collect::<BTreeMap<_, _>>();
    if request.settings.drilling.enabled {
        let actual = geometry
            .drills
            .iter()
            .map(|drill| drill.group_key.as_str())
            .collect::<BTreeSet<_>>();
        let mapped = request
            .settings
            .drilling
            .mappings
            .iter()
            .map(|mapping| mapping.group_key.as_str())
            .collect::<BTreeSet<_>>();
        if actual.is_empty()
            || actual != mapped
            || mapped.len() != request.settings.drilling.mappings.len()
        {
            return Err(PcbError::MissingDrillMappings);
        }
    }
    let mut source = String::with_capacity(32_768);
    writeln!(source, "; Millo PCB CAM").unwrap();
    writeln!(source, "; Source layers: {}", request.board.files.len()).unwrap();
    writeln!(source, "G21 G90 G94 G17").unwrap();
    writeln!(source, "M5").unwrap();
    writeln!(source, "M9").unwrap();
    writeln!(source, "G0 Z{}", number(request.settings.safe_z_mm)).unwrap();

    let mut emitter = Emitter {
        source: &mut source,
        safe_z_mm: request.settings.safe_z_mm,
        surface_z_mm: request.settings.surface_z_mm,
        tool_numbers: BTreeMap::new(),
        current_tool_id: None,
        tool_change_count: 0,
        operations: Vec::new(),
    };

    if request.settings.isolation.enabled {
        let tool = resolve_tool(&tool_lookup, &request.settings.isolation.tool_id)?;
        require_tool(
            tool,
            "isolation",
            &[ToolKind::Engraving, ToolKind::VBit, ToolKind::FlatEndMill],
        )?;
        let copper = collect_role(&geometry, PcbLayerRole::Copper);
        if copper.is_empty() {
            return Err(PcbError::MissingLayer("copper"));
        }
        let mut paths = CamPaths::default();
        for pass in 0..request.settings.isolation.passes {
            let offset = tool.diameter_mm / 2.0
                + request.settings.isolation.clearance_mm
                + f64::from(pass) * tool.diameter_mm * 0.6;
            paths.append(
                copper
                    .inflate(offset, JoinType::Round, EndType::Polygon, 2.0)
                    .simplify(0.004, false),
            );
        }
        emitter.use_tool(tool, "Изоляция меди");
        let motions = emitter.engrave_paths(
            &paths,
            request.settings.isolation.depth_mm,
            tool.feed_mm_per_min,
            tool.plunge_mm_per_min,
        );
        emitter.operation("isolation", tool, motions);
    }

    if request.settings.drilling.enabled {
        for mapping in &request.settings.drilling.mappings {
            let hits = geometry
                .drills
                .iter()
                .filter(|drill| drill.group_key == mapping.group_key)
                .collect::<Vec<_>>();
            if hits.is_empty() {
                return Err(PcbError::UnknownDrillGroup(mapping.group_key.clone()));
            }
            let tool = resolve_tool(&tool_lookup, &mapping.tool_id)?;
            require_tool(
                tool,
                "drilling",
                &[ToolKind::Drill, ToolKind::FlatEndMill, ToolKind::Engraving],
            )?;
            emitter.use_tool(
                tool,
                &format!(
                    "Сверление {} · Ø{} mm",
                    mapping.group_key,
                    number(hits[0].diameter_mm)
                ),
            );
            let motions = emitter.drill_hits(
                hits.iter().map(|hit| hit.point),
                request.settings.drilling.depth_mm,
                tool.plunge_mm_per_min,
            );
            emitter.operation("drilling", tool, motions);
        }
    }

    if request.settings.marking.enabled {
        let tool = resolve_tool(&tool_lookup, &request.settings.marking.tool_id)?;
        require_tool(
            tool,
            "marking",
            &[ToolKind::Engraving, ToolKind::VBit, ToolKind::FlatEndMill],
        )?;
        let paths = collect_role(&geometry, PcbLayerRole::Marking);
        if paths.is_empty() {
            return Err(PcbError::MissingLayer("marking"));
        }
        emitter.use_tool(tool, "Маркировка");
        let motions = emitter.engrave_paths(
            &paths,
            request.settings.marking.depth_mm,
            tool.feed_mm_per_min,
            tool.plunge_mm_per_min,
        );
        emitter.operation("marking", tool, motions);
    }

    if request.settings.outline.enabled {
        let tool = resolve_tool(&tool_lookup, &request.settings.outline.tool_id)?;
        require_tool(tool, "outline", &[ToolKind::FlatEndMill])?;
        let outlines = collect_role(&geometry, PcbLayerRole::Outline);
        let principal = largest_path(&outlines).ok_or(PcbError::MissingLayer("outline"))?;
        let compensated = principal
            .inflate(
                tool.diameter_mm / 2.0,
                JoinType::Round,
                EndType::Polygon,
                2.0,
            )
            .simplify(0.004, false);
        let path = largest_path(&compensated).ok_or(PcbError::MissingLayer("outline"))?;
        emitter.use_tool(tool, "Контур платы");
        let motions = emitter.cut_outline(
            path,
            request.settings.outline.depth_mm,
            request.settings.outline.depth_per_pass_mm,
            request.settings.outline.tab_count,
            request.settings.outline.tab_width_mm,
            request.settings.outline.tab_height_mm,
            tool.feed_mm_per_min,
            tool.plunge_mm_per_min,
        );
        emitter.operation("outline", tool, motions);
    }

    writeln!(emitter.source, "G0 Z{}", number(request.settings.safe_z_mm)).unwrap();
    writeln!(emitter.source, "M5").unwrap();
    writeln!(emitter.source, "M9").unwrap();
    writeln!(emitter.source, "M30").unwrap();
    let operations = std::mem::take(&mut emitter.operations);
    let tool_count = emitter.tool_numbers.len();
    let tool_change_count = emitter.tool_change_count;
    drop(emitter);
    if source.len() > MAX_SOURCE_BYTES {
        return Err(PcbError::GcodeTooLarge(MAX_SOURCE_BYTES));
    }
    let source_name = gcode_name(&request.source_name);
    let program = parse_program(ProgramParseRequest {
        source_name: source_name.clone(),
        source: source.clone(),
    })
    .map_err(|error| PcbError::InvalidGeneratedGcode(error.to_string()))?;
    Ok(GeneratedPcbJob {
        source_name,
        source,
        program,
        summary: PcbJobSummary {
            bounds: inspection.bounds,
            operations,
            tool_count,
            tool_change_count,
            warning_count: inspection.warnings.len(),
        },
        inspection,
    })
}

struct Emitter<'a> {
    source: &'a mut String,
    safe_z_mm: f64,
    surface_z_mm: f64,
    tool_numbers: BTreeMap<String, usize>,
    current_tool_id: Option<String>,
    tool_change_count: usize,
    operations: Vec<PcbOperationSummary>,
}

impl Emitter<'_> {
    fn use_tool(&mut self, tool: &CuttingTool, label: &str) {
        writeln!(self.source, "; {label}").unwrap();
        if self.current_tool_id.as_deref() == Some(&tool.id) {
            return;
        }
        let initial_tool = self.current_tool_id.is_none();
        let next_number = self.tool_numbers.len() + 1;
        let tool_number = *self
            .tool_numbers
            .entry(tool.id.clone())
            .or_insert(next_number);
        if initial_tool {
            writeln!(self.source, "T{tool_number}").unwrap();
        } else {
            writeln!(self.source, "G0 Z{}", number(self.safe_z_mm)).unwrap();
            writeln!(self.source, "M5").unwrap();
            writeln!(self.source, "T{tool_number} M6").unwrap();
            self.tool_change_count += 1;
        }
        writeln!(
            self.source,
            "; Tool: {} · Ø{} mm",
            tool.name,
            number(tool.diameter_mm)
        )
        .unwrap();
        self.current_tool_id = Some(tool.id.clone());
    }

    fn operation(&mut self, kind: &str, tool: &CuttingTool, motion_count: usize) {
        self.operations.push(PcbOperationSummary {
            kind: kind.to_owned(),
            tool_id: tool.id.clone(),
            tool_name: tool.name.clone(),
            motion_count,
        });
    }

    fn engrave_paths(&mut self, paths: &CamPaths, depth_mm: f64, feed: f64, plunge: f64) -> usize {
        let mut motions = 0;
        for path in paths.iter().filter(|path| path.len() >= 2) {
            let first = path.iter().next().expect("path has points");
            writeln!(self.source, "G0 Z{}", number(self.safe_z_mm)).unwrap();
            writeln!(
                self.source,
                "G0 X{} Y{}",
                number(first.x()),
                number(first.y())
            )
            .unwrap();
            writeln!(
                self.source,
                "G1 Z{} F{}",
                number(self.surface_z_mm - depth_mm),
                number(plunge)
            )
            .unwrap();
            for point in path.iter().skip(1) {
                writeln!(
                    self.source,
                    "G1 X{} Y{} F{}",
                    number(point.x()),
                    number(point.y()),
                    number(feed)
                )
                .unwrap();
                motions += 1;
            }
            writeln!(
                self.source,
                "G1 X{} Y{} F{}",
                number(first.x()),
                number(first.y()),
                number(feed)
            )
            .unwrap();
            writeln!(self.source, "G0 Z{}", number(self.safe_z_mm)).unwrap();
            motions += 1;
        }
        motions
    }

    fn drill_hits(
        &mut self,
        hits: impl Iterator<Item = crate::PcbPoint>,
        depth_mm: f64,
        plunge: f64,
    ) -> usize {
        let mut motions = 0;
        for point in hits {
            writeln!(self.source, "G0 Z{}", number(self.safe_z_mm)).unwrap();
            writeln!(
                self.source,
                "G0 X{} Y{}",
                number(point.x_mm),
                number(point.y_mm)
            )
            .unwrap();
            writeln!(
                self.source,
                "G1 Z{} F{}",
                number(self.surface_z_mm - depth_mm),
                number(plunge)
            )
            .unwrap();
            writeln!(self.source, "G0 Z{}", number(self.safe_z_mm)).unwrap();
            motions += 1;
        }
        motions
    }

    #[allow(clippy::too_many_arguments)]
    fn cut_outline(
        &mut self,
        path: &Path<Milli>,
        depth_mm: f64,
        depth_per_pass_mm: f64,
        tab_count: u8,
        tab_width_mm: f64,
        tab_height_mm: f64,
        feed: f64,
        plunge: f64,
    ) -> usize {
        let points = path
            .iter()
            .map(|point| (point.x(), point.y()))
            .collect::<Vec<_>>();
        let points = densify_closed(
            &points,
            if tab_count > 0 {
                (tab_width_mm / 3.0).clamp(0.1, 1.0)
            } else {
                2.0
            },
        );
        if points.len() < 2 {
            return 0;
        }
        let perimeter = closed_perimeter(&points);
        let pass_count = (depth_mm / depth_per_pass_mm).ceil().max(1.0) as usize;
        let mut motions = 0;
        for pass in 1..=pass_count {
            let pass_depth = (pass as f64 * depth_per_pass_mm).min(depth_mm);
            let first = points[0];
            writeln!(self.source, "G0 Z{}", number(self.safe_z_mm)).unwrap();
            writeln!(self.source, "G0 X{} Y{}", number(first.0), number(first.1)).unwrap();
            writeln!(
                self.source,
                "G1 Z{} F{}",
                number(self.surface_z_mm - pass_depth),
                number(plunge)
            )
            .unwrap();
            let mut travelled = 0.0;
            let mut raised = false;
            for index in 1..=points.len() {
                let from = points[index - 1];
                let to = points[index % points.len()];
                let length = distance(from, to);
                let midpoint = travelled + length / 2.0;
                let tab = tab_count > 0
                    && pass_depth > (depth_mm - tab_height_mm).max(0.0)
                    && in_tab(midpoint, perimeter, tab_count, tab_width_mm);
                if tab != raised {
                    let z = if tab {
                        self.surface_z_mm - (depth_mm - tab_height_mm).max(0.0)
                    } else {
                        self.surface_z_mm - pass_depth
                    };
                    writeln!(self.source, "G1 Z{} F{}", number(z), number(plunge)).unwrap();
                    raised = tab;
                }
                writeln!(
                    self.source,
                    "G1 X{} Y{} F{}",
                    number(to.0),
                    number(to.1),
                    number(feed)
                )
                .unwrap();
                travelled += length;
                motions += 1;
            }
        }
        writeln!(self.source, "G0 Z{}", number(self.safe_z_mm)).unwrap();
        motions
    }
}

fn collect_role(geometry: &BoardGeometry, role: PcbLayerRole) -> CamPaths {
    let mut paths = CamPaths::default();
    for layer in geometry.layers.iter().filter(|layer| layer.role == role) {
        paths.append(layer.paths.clone());
    }
    paths
}

fn largest_path(paths: &CamPaths) -> Option<&Path<Milli>> {
    paths.iter().max_by(|left, right| {
        polygon_area(left)
            .abs()
            .total_cmp(&polygon_area(right).abs())
    })
}

fn polygon_area(path: &Path<Milli>) -> f64 {
    let points = path.iter().collect::<Vec<_>>();
    if points.len() < 3 {
        return 0.0;
    }
    points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            let next = points[(index + 1) % points.len()];
            point.x() * next.y() - next.x() * point.y()
        })
        .sum::<f64>()
        / 2.0
}

fn resolve_tool<'a>(
    tools: &BTreeMap<&str, &'a CuttingTool>,
    id: &str,
) -> Result<&'a CuttingTool, PcbError> {
    tools
        .get(id)
        .copied()
        .ok_or_else(|| PcbError::UnknownTool(id.to_owned()))
}

fn require_tool(
    tool: &CuttingTool,
    operation: &'static str,
    allowed: &[ToolKind],
) -> Result<(), PcbError> {
    if allowed.contains(&tool.kind) {
        Ok(())
    } else {
        Err(PcbError::IncompatibleTool {
            operation,
            tool: tool.name.clone(),
        })
    }
}

fn validate_settings(request: &PcbJobRequest) -> Result<(), PcbError> {
    let settings = &request.settings;
    validate_range("safeZMm", settings.safe_z_mm, -1_000.0, 10_000.0)?;
    validate_range("surfaceZMm", settings.surface_z_mm, -1_000.0, 1_000.0)?;
    if settings.safe_z_mm <= settings.surface_z_mm {
        return Err(PcbError::InvalidSetting("safeZMm"));
    }
    if !settings.isolation.enabled
        && !settings.drilling.enabled
        && !settings.outline.enabled
        && !settings.marking.enabled
    {
        return Err(PcbError::NoOperations);
    }
    if settings.isolation.enabled {
        validate_range(
            "isolation.depthMm",
            settings.isolation.depth_mm,
            0.001,
            10.0,
        )?;
        validate_range(
            "isolation.clearanceMm",
            settings.isolation.clearance_mm,
            0.0,
            10.0,
        )?;
        if !(1..=10).contains(&settings.isolation.passes) {
            return Err(PcbError::InvalidSetting("isolation.passes"));
        }
    }
    if settings.drilling.enabled {
        validate_range("drilling.depthMm", settings.drilling.depth_mm, 0.001, 100.0)?;
        if settings.drilling.mappings.is_empty() {
            return Err(PcbError::MissingDrillMappings);
        }
    }
    if settings.outline.enabled {
        validate_range("outline.depthMm", settings.outline.depth_mm, 0.001, 100.0)?;
        validate_range(
            "outline.depthPerPassMm",
            settings.outline.depth_per_pass_mm,
            0.001,
            settings.outline.depth_mm,
        )?;
        validate_range(
            "outline.tabWidthMm",
            settings.outline.tab_width_mm,
            0.1,
            50.0,
        )?;
        validate_range(
            "outline.tabHeightMm",
            settings.outline.tab_height_mm,
            0.0,
            settings.outline.depth_mm,
        )?;
        if settings.outline.tab_count > 16 {
            return Err(PcbError::InvalidSetting("outline.tabCount"));
        }
    }
    if settings.marking.enabled {
        validate_range("marking.depthMm", settings.marking.depth_mm, 0.001, 10.0)?;
    }
    Ok(())
}

fn validate_range(field: &'static str, value: f64, min: f64, max: f64) -> Result<(), PcbError> {
    if !value.is_finite() || value < min || value > max {
        Err(PcbError::InvalidSetting(field))
    } else {
        Ok(())
    }
}

fn closed_perimeter(points: &[(f64, f64)]) -> f64 {
    (0..points.len())
        .map(|index| distance(points[index], points[(index + 1) % points.len()]))
        .sum()
}

fn densify_closed(points: &[(f64, f64)], max_segment_mm: f64) -> Vec<(f64, f64)> {
    let mut result = Vec::new();
    for index in 0..points.len() {
        let from = points[index];
        let to = points[(index + 1) % points.len()];
        result.push(from);
        let segments = (distance(from, to) / max_segment_mm).ceil().max(1.0) as usize;
        for step in 1..segments {
            let ratio = step as f64 / segments as f64;
            result.push((
                from.0 + (to.0 - from.0) * ratio,
                from.1 + (to.1 - from.1) * ratio,
            ));
        }
    }
    result
}

fn distance(left: (f64, f64), right: (f64, f64)) -> f64 {
    ((left.0 - right.0).powi(2) + (left.1 - right.1).powi(2)).sqrt()
}

fn in_tab(distance: f64, perimeter: f64, count: u8, width: f64) -> bool {
    if perimeter <= 0.0 || count == 0 {
        return false;
    }
    (0..count).any(|index| {
        let center = perimeter * (f64::from(index) + 0.5) / f64::from(count);
        (distance - center).abs() <= width / 2.0
    })
}

fn gcode_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.to_ascii_lowercase().ends_with(".nc") {
        trimmed.to_owned()
    } else {
        format!("{trimmed}.nc")
    }
}

fn number(value: f64) -> String {
    let mut value = format!("{value:.4}");
    while value.contains('.') && value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    if value == "-0" { "0".to_owned() } else { value }
}
