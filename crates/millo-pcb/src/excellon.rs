use std::io::{BufReader, Cursor};

use lib_gerber_edit::{
    excellon_format::{Command, GeometricCode, InputMode, MachineCode, parse_excellon},
    gerber_types::Unit,
};

use crate::{
    PcbError, PcbPoint,
    geometry::{DrillFeature, DrillGeometry},
};

pub(crate) fn parse_drills(
    source_name: &str,
    bytes: &[u8],
) -> Result<Vec<DrillGeometry>, PcbError> {
    let layer = parse_excellon(BufReader::new(Cursor::new(bytes)))
        .map_err(|error| PcbError::InvalidExcellon(source_name.to_owned(), error.to_string()))?;
    if let Some(error) = layer
        .header
        .iter()
        .chain(&layer.commands)
        .filter_map(|command| command.as_ref().err())
        .next()
    {
        return Err(PcbError::InvalidExcellon(
            source_name.to_owned(),
            error.to_string(),
        ));
    }
    let mut current_tool = None;
    let mut current = PcbPoint::default();
    let mut incremental = layer
        .header
        .iter()
        .rev()
        .find_map(|command| match command {
            Ok(Command::Incremental(value)) => Some(*value),
            _ => None,
        })
        .unwrap_or(false);
    let mut drills = Vec::new();
    for command in layer.commands.iter().flatten() {
        match command {
            Command::Tool(tool) => current_tool = Some(*tool),
            Command::Geometric(GeometricCode::InputMode(mode)) => {
                incremental = *mode == InputMode::Incremental;
            }
            Command::Machine(MachineCode::Scale(_)) => {}
            Command::Coordinate(x, y, format) => {
                let x = x.map(|value| to_mm(value, format.unit));
                let y = y.map(|value| to_mm(value, format.unit));
                if incremental {
                    current.x_mm += x.unwrap_or(0.0);
                    current.y_mm += y.unwrap_or(0.0);
                } else {
                    current.x_mm = x.unwrap_or(current.x_mm);
                    current.y_mm = y.unwrap_or(current.y_mm);
                }
                let tool = current_tool
                    .ok_or_else(|| PcbError::DrillWithoutTool(source_name.to_owned()))?;
                let diameter = layer
                    .tools
                    .get(&tool)
                    .copied()
                    .ok_or_else(|| PcbError::UnknownDrillTool(source_name.to_owned(), tool))?;
                drills.push(DrillGeometry {
                    group_key: format!("{}::T{}", source_name, tool),
                    source_name: source_name.to_owned(),
                    source_tool_number: tool,
                    diameter_mm: to_mm(diameter, layer.unit.unit),
                    feature: DrillFeature::Hit(current),
                });
            }
            Command::Slot {
                from_x,
                from_y,
                to_x,
                to_y,
                fmt,
            } => {
                let from = slot_point(current, *from_x, *from_y, fmt.unit, incremental);
                let end_origin = if from_x.is_some() || from_y.is_some() {
                    from
                } else {
                    current
                };
                let end = slot_point(end_origin, *to_x, *to_y, fmt.unit, incremental);
                let tool = current_tool
                    .ok_or_else(|| PcbError::DrillWithoutTool(source_name.to_owned()))?;
                let diameter = layer
                    .tools
                    .get(&tool)
                    .copied()
                    .ok_or_else(|| PcbError::UnknownDrillTool(source_name.to_owned(), tool))?;
                drills.push(DrillGeometry {
                    group_key: format!("{}::T{}", source_name, tool),
                    source_name: source_name.to_owned(),
                    source_tool_number: tool,
                    diameter_mm: to_mm(diameter, layer.unit.unit),
                    feature: DrillFeature::Slot { start: from, end },
                });
                current = end;
            }
            _ => {}
        }
    }
    if drills.is_empty() {
        Err(PcbError::EmptyLayer(source_name.to_owned()))
    } else {
        Ok(drills)
    }
}

fn slot_point(
    current: PcbPoint,
    x: Option<f64>,
    y: Option<f64>,
    unit: Unit,
    incremental: bool,
) -> PcbPoint {
    let x = x.map(|value| to_mm(value, unit));
    let y = y.map(|value| to_mm(value, unit));
    if incremental {
        PcbPoint {
            x_mm: current.x_mm + x.unwrap_or(0.0),
            y_mm: current.y_mm + y.unwrap_or(0.0),
        }
    } else {
        PcbPoint {
            x_mm: x.unwrap_or(current.x_mm),
            y_mm: y.unwrap_or(current.y_mm),
        }
    }
}

fn to_mm(value: f64, unit: Unit) -> f64 {
    match unit {
        Unit::Millimeters => value,
        Unit::Inches => value * 25.4,
    }
}
