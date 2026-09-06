use crate::{geometry::distance, planner::PlannedOperation, *};
use millo_gcode::{MAX_SOURCE_BYTES, ProgramParseRequest, parse_program};
use std::{collections::BTreeMap, fmt::Write};

pub(crate) fn generate(
    request: &SketchJobRequest,
    plan: &[PlannedOperation<'_>],
) -> Result<GeneratedSketchJob, SketchError> {
    let stock = &request.stock;
    let mut out = Output(String::from(
        "; Millo Sketch CAM: active work system, Z0 = top of stock\nG21 G90 G94 G17\nM5\nM9\n",
    ));
    out.line(format_args!("G0 Z{:.4}", stock.safe_z_mm))?;
    let mut tool_numbers = BTreeMap::new();
    let mut current_tool = None;
    let mut operations = Vec::new();
    let mut preview = Vec::new();
    let mut warnings = Vec::new();
    let mut tab_paths = Vec::new();
    let mut tool_change_count = 0;
    for operation in plan {
        let shape = operation.shape;
        let op = &shape.operation;
        let tool = operation.tool;
        let next_number = tool_numbers.len() + 1;
        let tool_number = *tool_numbers.entry(&tool.id).or_insert(next_number);
        out.line(format_args!("; {}", comment(&shape.name)))?;
        if current_tool != Some(&tool.id) {
            out.line(format_args!("G0 Z{:.4}", stock.safe_z_mm))?;
            if current_tool.is_some() {
                out.line(format_args!("M5\nM9\nT{tool_number} M6"))?;
                tool_change_count += 1;
            } else {
                out.line(format_args!("T{tool_number}"))?;
            }
            out.line(format_args!("; Tool: {}", comment(&tool.name)))?;
            current_tool = Some(&tool.id);
        }
        if stock.spindle_mode == SketchSpindleMode::Controller {
            out.line(format_args!("S{} M3", op.spindle_rpm))?;
        } else {
            out.line(format_args!("; Manual spindle: {} rpm", op.spindle_rpm))?;
        }
        if op.through
            && op.tabs.count == 0
            && matches!(
                op.kind,
                SketchOperationKind::Inside | SketchOperationKind::Outside
            )
        {
            warnings.push(format!(
                "{}: сквозной контур без перемычек. Закрепите и деталь, и выпадающую часть",
                shape.name
            ));
        }
        if matches!(shape.geometry, SketchGeometry::Rectangle { radius, .. } if radius < tool.diameter_mm / 2.0)
            && matches!(
                op.kind,
                SketchOperationKind::Inside | SketchOperationKind::Pocket
            )
        {
            warnings.push(format!(
                "{}: внутренние углы останутся с радиусом не меньше {:.2} mm",
                shape.name,
                tool.diameter_mm / 2.0
            ));
        }
        for pass in 1..=operation.passes {
            let depth = (pass as f64 * op.stepdown_mm).min(operation.depth);
            out.line(format_args!("; Pass {pass}/{}", operation.passes))?;
            for path in &operation.paths {
                let first = path[0];
                out.line(format_args!(
                    "G0 Z{:.4}\nG0 X{:.4} Y{:.4}",
                    stock.safe_z_mm, first.x, first.y
                ))?;
                out.line(format_args!(
                    "G1 Z{:.4} F{:.3}",
                    -depth, op.plunge_mm_per_min
                ))?;
                if path.len() > 1 {
                    let tabs = trace(&mut out, path, operation, stock, depth)?;
                    if pass == operation.passes {
                        tab_paths.extend(tabs.into_iter().map(|points| SketchPreviewPath {
                            shape_id: shape.id.clone(),
                            points,
                        }));
                    }
                }
                // Drill uses the same bounded depth passes, with a full chip-clear retract.
                out.line(format_args!("G0 Z{:.4}", stock.safe_z_mm))?;
            }
        }
        operations.push(SketchOperationSummary {
            shape_id: shape.id.clone(),
            name: shape.name.clone(),
            tool_id: tool.id.clone(),
            tool_number,
            depth_mm: operation.depth,
            pass_count: operation.passes,
            path_count: operation.paths.len(),
        });
        preview.extend(operation.paths.iter().map(|points| SketchPreviewPath {
            shape_id: shape.id.clone(),
            points: points.clone(),
        }));
    }
    out.line(format_args!("G0 Z{:.4}\nM5\nM9\nM30", stock.safe_z_mm))?;
    let source = out.0;
    let source_name = filename(&request.source_name);
    let program = parse_program(ProgramParseRequest {
        source_name: source_name.clone(),
        source: source.clone(),
    })
    .map_err(|e| SketchError(format!("Ошибка проверки G-code: {e}")))?;
    Ok(GeneratedSketchJob {
        source_name,
        source,
        program,
        summary: SketchJobSummary {
            operations,
            tool_change_count,
            paths: preview,
            tab_paths,
            warnings,
        },
    })
}

fn trace(
    out: &mut Output,
    path: &[SketchPoint],
    operation: &PlannedOperation<'_>,
    stock: &SketchStock,
    depth: f64,
) -> Result<Vec<Vec<SketchPoint>>, SketchError> {
    let op = &operation.shape.operation;
    let perimeter: f64 = (0..path.len())
        .map(|i| distance(path[i], path[(i + 1) % path.len()]))
        .sum();
    let width = op.tabs.width_mm + operation.tool.diameter_mm;
    let tab_top = -(stock.thickness_mm - op.tabs.height_mm);
    let active_tabs = op.tabs.count > 0 && -depth < tab_top;
    let intervals: Vec<(f64, f64)> = if active_tabs {
        (0..op.tabs.count)
            .map(|i| {
                let center = perimeter * (f64::from(i) + 0.5) / f64::from(op.tabs.count);
                (center - width / 2.0, center + width / 2.0)
            })
            .collect()
    } else {
        vec![]
    };
    let mut travelled = 0.0;
    let mut current_z = -depth;
    let mut tabs = Vec::new();
    for i in 0..path.len() {
        let a = path[i];
        let b = path[(i + 1) % path.len()];
        let length = distance(a, b);
        if length <= 0.000_01 {
            continue;
        }
        let end = travelled + length;
        let mut stops = vec![travelled, end];
        for (start, finish) in &intervals {
            for t in [start, finish] {
                if *t > travelled && *t < end {
                    stops.push(*t);
                }
            }
        }
        stops.sort_by(f64::total_cmp);
        for pair in stops.windows(2) {
            let mid = (pair[0] + pair[1]) / 2.0;
            let z = if intervals.iter().any(|(a, b)| mid > *a && mid < *b) {
                tab_top
            } else {
                -depth
            };
            if (z - current_z).abs() > 0.000_01 {
                out.line(format_args!("G1 Z{z:.4} F{:.3}", op.plunge_mm_per_min))?;
                current_z = z;
            }
            let t = ((pair[1] - travelled) / length).clamp(0.0, 1.0);
            if z > -depth {
                let start = ((pair[0] - travelled) / length).clamp(0.0, 1.0);
                tabs.push(vec![
                    SketchPoint {
                        x: a.x + (b.x - a.x) * start,
                        y: a.y + (b.y - a.y) * start,
                    },
                    SketchPoint {
                        x: a.x + (b.x - a.x) * t,
                        y: a.y + (b.y - a.y) * t,
                    },
                ]);
            }
            out.line(format_args!(
                "G1 X{:.4} Y{:.4} F{:.3}",
                a.x + (b.x - a.x) * t,
                a.y + (b.y - a.y) * t,
                op.feed_mm_per_min
            ))?;
        }
        travelled = end;
    }
    Ok(tabs)
}

struct Output(String);
impl Output {
    fn line(&mut self, line: std::fmt::Arguments<'_>) -> Result<(), SketchError> {
        writeln!(self.0, "{line}").expect("String write");
        require(
            self.0.len() <= MAX_SOURCE_BYTES,
            "Программа слишком большая; уменьшите число проходов",
        )
    }
}

fn comment(value: &str) -> String {
    value
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, '(' | ')'))
        .take(120)
        .collect()
}

pub(crate) fn filename(value: &str) -> String {
    let stem = value.trim().strip_suffix(".nc").unwrap_or(value.trim());
    let name: String = stem
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .take(100)
        .collect();
    format!(
        "{}.nc",
        if name.trim_matches('.').is_empty() {
            "sketch"
        } else {
            name.trim_matches('.')
        }
    )
}
