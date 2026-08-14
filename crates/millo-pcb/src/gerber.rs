use std::io::{BufReader, Cursor};

use clipper2::{EndType, JoinType, Path, Paths, Point};
use lib_gerber_edit::{
    gerber::GerberLayerData,
    gerber_types::{
        Aperture, Command, CoordinateMode, DCode, ExtendedCode, FunctionCode, GCode,
        InterpolationMode, Mirroring, Operation, Polarity, QuadrantMode,
    },
    layer::LayerType,
};

use crate::{
    PcbError, PcbLayerRole, PcbPoint,
    geometry::{CamPath, CamPaths, LayerGeometry, circle, combine, polygon, rectangle},
};

pub(crate) fn parse_gerber(
    source_name: &str,
    role: PcbLayerRole,
    bytes: &[u8],
) -> Result<LayerGeometry, PcbError> {
    let normalized = normalize_legacy_coordinate_commands(source_name, bytes)?;
    let layer =
        GerberLayerData::from_type(layer_type(role), BufReader::new(Cursor::new(normalized)))
            .map_err(|error| PcbError::InvalidGerber(source_name.to_owned(), error.to_string()))?;
    if layer.coordinate_format.coordinate_mode == CoordinateMode::Incremental {
        return Err(PcbError::UnsupportedGerberFeature(
            source_name.to_owned(),
            "incremental coordinates".to_owned(),
        ));
    }
    if let Some(error) = layer.parse_errors.first() {
        return Err(PcbError::InvalidGerber(
            source_name.to_owned(),
            error.clone(),
        ));
    }
    if layer.header.iter().any(|command| {
        matches!(
            command,
            Command::ExtendedCode(
                ExtendedCode::ScaleImage(_)
                    | ExtendedCode::OffsetImage(_)
                    | ExtendedCode::RotateImage(_)
                    | ExtendedCode::ImagePolarity(_)
                    | ExtendedCode::AxisSelect(_)
            )
        )
    }) {
        return Err(PcbError::UnsupportedGerberFeature(
            source_name.to_owned(),
            "deprecated image transform".to_owned(),
        ));
    }

    let mut image = CamPaths::default();
    let mut selected_aperture = None;
    let mut current = PcbPoint::default();
    let mut interpolation = InterpolationMode::Linear;
    let mut quadrant_mode = QuadrantMode::Single;
    let mut polarity = Polarity::Dark;
    let mut region = false;
    let mut region_path: Vec<Point<clipper2::Milli>> = Vec::new();

    for command in &layer.commands {
        match command {
            Command::ExtendedCode(ExtendedCode::CoordinateFormat(format))
                if format.coordinate_mode == CoordinateMode::Incremental =>
            {
                return Err(PcbError::UnsupportedGerberFeature(
                    source_name.to_owned(),
                    "incremental coordinates".to_owned(),
                ));
            }
            Command::ExtendedCode(ExtendedCode::LoadPolarity(next)) => polarity = *next,
            Command::ExtendedCode(ExtendedCode::LoadMirroring(Mirroring::None)) => {}
            Command::ExtendedCode(ExtendedCode::LoadRotation(value)) if value.rotation == 0.0 => {}
            Command::ExtendedCode(ExtendedCode::LoadScaling(value)) if value.scale == 1.0 => {}
            Command::ExtendedCode(
                ExtendedCode::LoadMirroring(_)
                | ExtendedCode::LoadRotation(_)
                | ExtendedCode::LoadScaling(_)
                | ExtendedCode::StepAndRepeat(_)
                | ExtendedCode::ApertureBlock(_),
            ) => {
                return Err(PcbError::UnsupportedGerberFeature(
                    source_name.to_owned(),
                    "LM/LR/LS/SR/AB transform".to_owned(),
                ));
            }
            Command::FunctionCode(FunctionCode::DCode(DCode::SelectAperture(code))) => {
                selected_aperture = Some(*code);
            }
            Command::FunctionCode(FunctionCode::GCode(GCode::InterpolationMode(mode))) => {
                interpolation = *mode;
            }
            Command::FunctionCode(FunctionCode::GCode(GCode::QuadrantMode(mode))) => {
                quadrant_mode = *mode;
            }
            Command::FunctionCode(FunctionCode::GCode(GCode::CoordinateMode(
                CoordinateMode::Incremental,
            ))) => {
                return Err(PcbError::UnsupportedGerberFeature(
                    source_name.to_owned(),
                    "incremental coordinates".to_owned(),
                ));
            }
            Command::FunctionCode(FunctionCode::GCode(GCode::RegionMode(enabled))) => {
                if !enabled && region && region_path.len() >= 3 {
                    image = combine(
                        image,
                        Paths::new(vec![Path::new(std::mem::take(&mut region_path))]),
                        polarity == Polarity::Dark,
                    )?;
                }
                region = *enabled;
                if region {
                    region_path.clear();
                }
            }
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(operation))) => {
                match operation {
                    Operation::Move(coordinates) => {
                        current = modal_point(current, coordinates.as_ref());
                        if region {
                            if region_path.len() >= 3 {
                                image = combine(
                                    image,
                                    Paths::new(vec![Path::new(std::mem::take(&mut region_path))]),
                                    polarity == Polarity::Dark,
                                )?;
                            }
                            region_path = vec![Point::new(current.x_mm, current.y_mm)];
                        }
                    }
                    Operation::Flash(coordinates) => {
                        current = modal_point(current, coordinates.as_ref());
                        let aperture = selected_aperture
                            .and_then(|code| layer.apertures.get(&code))
                            .ok_or_else(|| PcbError::MissingAperture(source_name.to_owned()))?;
                        let primitive = flash(aperture, current, source_name)?;
                        image = combine(image, primitive, polarity == Polarity::Dark)?;
                    }
                    Operation::Interpolate(coordinates, offset) => {
                        let next = modal_point(current, coordinates.as_ref());
                        if interpolation != InterpolationMode::Linear
                            && quadrant_mode != QuadrantMode::Multi
                        {
                            return Err(PcbError::UnsupportedGerberFeature(
                                source_name.to_owned(),
                                "single-quadrant circular interpolation".to_owned(),
                            ));
                        }
                        let points =
                            flatten_interpolation(current, next, offset.as_ref(), interpolation);
                        if region {
                            if region_path.is_empty() {
                                region_path.push(Point::new(current.x_mm, current.y_mm));
                            }
                            region_path.extend(
                                points
                                    .iter()
                                    .skip(1)
                                    .map(|point| Point::new(point.x_mm, point.y_mm)),
                            );
                        } else {
                            let aperture = selected_aperture
                                .and_then(|code| layer.apertures.get(&code))
                                .ok_or_else(|| PcbError::MissingAperture(source_name.to_owned()))?;
                            let centerline = CamPath::new(
                                points
                                    .iter()
                                    .map(|point| Point::new(point.x_mm, point.y_mm))
                                    .collect(),
                            );
                            let primitive = stroke(aperture, centerline, source_name)?;
                            image = combine(image, primitive, polarity == Polarity::Dark)?;
                        }
                        current = next;
                    }
                }
            }
            _ => {}
        }
    }
    if region_path.len() >= 3 {
        image = combine(
            image,
            Paths::new(vec![Path::new(region_path)]),
            polarity == Polarity::Dark,
        )?;
    }
    if image.is_empty() {
        return Err(PcbError::EmptyLayer(source_name.to_owned()));
    }
    Ok(LayerGeometry {
        source_name: source_name.to_owned(),
        role,
        paths: image,
    })
}

fn normalize_legacy_coordinate_commands(
    source_name: &str,
    bytes: &[u8],
) -> Result<Vec<u8>, PcbError> {
    const ABSOLUTE: &[u8] = b"G90*";
    const INCREMENTAL: &[u8] = b"G91*";
    const ABSOLUTE_COMMENT: &[u8] = b"G04 Millo accepted legacy G90 absolute mode*";

    let mut normalized = Vec::with_capacity(bytes.len());
    let mut copied_until = 0usize;
    let mut command_start = 0usize;
    let mut in_extended = false;

    for (index, byte) in bytes.iter().copied().enumerate() {
        if byte == b'%' {
            in_extended = !in_extended;
            if !in_extended {
                command_start = index + 1;
            }
            continue;
        }
        if in_extended || byte != b'*' {
            continue;
        }

        let command = &bytes[command_start..=index];
        let leading_whitespace = command
            .iter()
            .position(|byte| !byte.is_ascii_whitespace())
            .unwrap_or(command.len());
        let trimmed = &command[leading_whitespace..];
        if trimmed == INCREMENTAL {
            return Err(PcbError::UnsupportedGerberFeature(
                source_name.to_owned(),
                "incremental coordinates".to_owned(),
            ));
        }
        if trimmed == ABSOLUTE {
            normalized.extend_from_slice(&bytes[copied_until..command_start + leading_whitespace]);
            normalized.extend_from_slice(ABSOLUTE_COMMENT);
            copied_until = index + 1;
        }
        command_start = index + 1;
    }
    normalized.extend_from_slice(&bytes[copied_until..]);
    Ok(normalized)
}

fn layer_type(role: PcbLayerRole) -> LayerType {
    match role {
        PcbLayerRole::Copper => LayerType::Top,
        PcbLayerRole::Outline => LayerType::Dimensions,
        PcbLayerRole::Marking => LayerType::SilkScreenTop,
        PcbLayerRole::Drill => LayerType::Drill,
    }
}

fn modal_point(
    current: PcbPoint,
    coordinates: Option<&lib_gerber_edit::gerber_types::Coordinates>,
) -> PcbPoint {
    let Some(coordinates) = coordinates else {
        return current;
    };
    PcbPoint {
        x_mm: coordinates.x.map(f64::from).unwrap_or(current.x_mm),
        y_mm: coordinates.y.map(f64::from).unwrap_or(current.y_mm),
    }
}

fn flash(aperture: &Aperture, center: PcbPoint, source_name: &str) -> Result<CamPaths, PcbError> {
    let (outer, hole) = match aperture {
        Aperture::Circle(value) => (
            Paths::new(vec![circle(center, value.diameter / 2.0)]),
            value.hole_diameter,
        ),
        Aperture::Rectangle(value) => (
            Paths::new(vec![rectangle(center, value.x, value.y)]),
            value.hole_diameter,
        ),
        Aperture::Obround(value) => {
            let base = if value.x >= value.y {
                let half = (value.x - value.y) / 2.0;
                let line: CamPath = vec![
                    (center.x_mm - half, center.y_mm),
                    (center.x_mm + half, center.y_mm),
                ]
                .into();
                line.inflate(value.y / 2.0, JoinType::Round, EndType::Round, 2.0)
            } else {
                let half = (value.y - value.x) / 2.0;
                let line: CamPath = vec![
                    (center.x_mm, center.y_mm - half),
                    (center.x_mm, center.y_mm + half),
                ]
                .into();
                line.inflate(value.x / 2.0, JoinType::Round, EndType::Round, 2.0)
            };
            (base, value.hole_diameter)
        }
        Aperture::Polygon(value) => (
            Paths::new(vec![polygon(
                center,
                value.diameter / 2.0,
                usize::from(value.vertices),
                value.rotation.unwrap_or(0.0).to_radians(),
            )]),
            value.hole_diameter,
        ),
        Aperture::Macro(name, _) => {
            return Err(PcbError::UnsupportedGerberFeature(
                source_name.to_owned(),
                format!("aperture macro {name}"),
            ));
        }
    };
    match hole {
        Some(diameter) if diameter > 0.0 => combine(
            outer,
            Paths::new(vec![circle(center, diameter / 2.0)]),
            false,
        ),
        _ => Ok(outer),
    }
}

fn stroke(
    aperture: &Aperture,
    centerline: CamPath,
    source_name: &str,
) -> Result<CamPaths, PcbError> {
    let paths = match aperture {
        Aperture::Circle(value) => {
            centerline.inflate(value.diameter / 2.0, JoinType::Round, EndType::Round, 2.0)
        }
        Aperture::Rectangle(value) => {
            centerline.minkowski_sum(rectangle(PcbPoint::default(), value.x, value.y), false)
        }
        Aperture::Obround(_) => {
            let kernel = flash(aperture, PcbPoint::default(), source_name)?;
            let kernel = kernel
                .first()
                .ok_or_else(|| PcbError::EmptyLayer(source_name.to_owned()))?;
            centerline.minkowski_sum(kernel.clone(), false)
        }
        Aperture::Polygon(value) => centerline.minkowski_sum(
            polygon(
                PcbPoint::default(),
                value.diameter / 2.0,
                usize::from(value.vertices),
                value.rotation.unwrap_or(0.0).to_radians(),
            ),
            false,
        ),
        Aperture::Macro(name, _) => {
            return Err(PcbError::UnsupportedGerberFeature(
                source_name.to_owned(),
                format!("aperture macro {name}"),
            ));
        }
    };
    Ok(paths.simplify(0.002, false))
}

fn flatten_interpolation(
    start: PcbPoint,
    end: PcbPoint,
    offset: Option<&lib_gerber_edit::gerber_types::CoordinateOffset>,
    mode: InterpolationMode,
) -> Vec<PcbPoint> {
    if mode == InterpolationMode::Linear || offset.is_none() {
        return vec![start, end];
    }
    let offset = offset.expect("checked above");
    let center = PcbPoint {
        x_mm: start.x_mm + offset.x.map(f64::from).unwrap_or(0.0),
        y_mm: start.y_mm + offset.y.map(f64::from).unwrap_or(0.0),
    };
    let radius = ((start.x_mm - center.x_mm).powi(2) + (start.y_mm - center.y_mm).powi(2)).sqrt();
    if radius <= 1e-9 {
        return vec![start, end];
    }
    let start_angle = (start.y_mm - center.y_mm).atan2(start.x_mm - center.x_mm);
    let end_angle = (end.y_mm - center.y_mm).atan2(end.x_mm - center.x_mm);
    let mut sweep = end_angle - start_angle;
    match mode {
        InterpolationMode::ClockwiseCircular => {
            while sweep >= 0.0 {
                sweep -= std::f64::consts::TAU;
            }
        }
        InterpolationMode::CounterclockwiseCircular => {
            while sweep <= 0.0 {
                sweep += std::f64::consts::TAU;
            }
        }
        InterpolationMode::Linear => {}
    }
    let segment_count = ((sweep.abs() * radius / 0.04).ceil() as usize).clamp(2, 512);
    let mut points = Vec::with_capacity(segment_count + 1);
    for index in 0..=segment_count {
        if index == segment_count {
            points.push(end);
        } else {
            let angle = start_angle + sweep * index as f64 / segment_count as f64;
            points.push(PcbPoint {
                x_mm: center.x_mm + radius * angle.cos(),
                y_mm: center.y_mm + radius * angle.sin(),
            });
        }
    }
    points
}
