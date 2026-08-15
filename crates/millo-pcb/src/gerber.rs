use std::io::{BufReader, Cursor};

use clipper2::{Path, Paths, Point};
use lib_gerber_edit::{
    gerber::GerberLayerData,
    gerber_types::{
        Aperture, Command, CommentContent, CoordinateMode, DCode, ExtendedCode, FileAttribute,
        FilePolarity, FunctionCode, GCode, InterpolationMode, Mirroring, Operation, Polarity,
        QuadrantMode, StandardComment, StepAndRepeat,
    },
    layer::LayerType,
};

use crate::{
    PcbError, PcbLayerRole, PcbPoint, aperture,
    geometry::{CamPath, CamPaths, DrillFeature, DrillGeometry, LayerGeometry, combine},
};

pub(crate) fn looks_like_gerber(bytes: &[u8]) -> bool {
    bytes
        .windows(3)
        .any(|window| matches!(window, b"%FS" | b"%MO" | b"%AD"))
}

pub(crate) fn parse_gerber_drills(
    source_name: &str,
    bytes: &[u8],
) -> Result<Vec<DrillGeometry>, PcbError> {
    let layer = load_layer(source_name, PcbLayerRole::Drill, bytes)?;
    let mut selected_aperture = None;
    let mut current = PcbPoint::default();
    let mut interpolation = InterpolationMode::Linear;
    let mut quadrant_mode = QuadrantMode::Single;
    let mut repeat = None;
    let mut drills = Vec::new();
    for command in &layer.commands {
        match command {
            Command::ExtendedCode(ExtendedCode::StepAndRepeat(StepAndRepeat::Open {
                repeat_x,
                repeat_y,
                distance_x,
                distance_y,
            })) => {
                repeat = Some(checked_repeat(
                    *repeat_x,
                    *repeat_y,
                    *distance_x,
                    *distance_y,
                    source_name,
                )?);
            }
            Command::ExtendedCode(ExtendedCode::StepAndRepeat(StepAndRepeat::Close)) => {
                repeat = None;
            }
            Command::FunctionCode(FunctionCode::GCode(GCode::InterpolationMode(mode))) => {
                interpolation = *mode;
            }
            Command::FunctionCode(FunctionCode::GCode(GCode::QuadrantMode(mode))) => {
                quadrant_mode = *mode;
            }
            Command::FunctionCode(FunctionCode::DCode(DCode::SelectAperture(code))) => {
                selected_aperture = Some(*code);
            }
            Command::FunctionCode(FunctionCode::DCode(DCode::Operation(operation))) => {
                if let Operation::Move(coordinates) = operation {
                    current = modal_point(current, coordinates.as_ref());
                    continue;
                }
                let aperture_code = selected_aperture
                    .ok_or_else(|| PcbError::MissingAperture(source_name.to_owned()))?;
                let aperture = layer
                    .apertures
                    .get(&aperture_code)
                    .ok_or_else(|| PcbError::MissingAperture(source_name.to_owned()))?;
                let diameter_mm = match aperture {
                    Aperture::Circle(circle) if circle.hole_diameter.is_none() => circle.diameter,
                    _ => {
                        return Err(PcbError::UnsupportedGerberFeature(
                            source_name.to_owned(),
                            "Gerber drill data requires a solid circular aperture".to_owned(),
                        ));
                    }
                };
                let source_tool_number = u32::try_from(aperture_code).map_err(|_| {
                    PcbError::UnsupportedGerberFeature(
                        source_name.to_owned(),
                        "negative drill aperture code".to_owned(),
                    )
                })?;
                let group_key = format!("{}::D{}", source_name, aperture_code);
                match operation {
                    Operation::Flash(coordinates) => {
                        current = modal_point(current, coordinates.as_ref());
                        push_repeated_drill(
                            &mut drills,
                            &group_key,
                            source_name,
                            source_tool_number,
                            diameter_mm,
                            DrillFeature::Hit(current),
                            repeat,
                        );
                    }
                    Operation::Interpolate(coordinates, offset) => {
                        let end = modal_point(current, coordinates.as_ref());
                        let points = flatten_interpolation(
                            current,
                            end,
                            offset.as_ref(),
                            interpolation,
                            quadrant_mode,
                            source_name,
                        )?;
                        for points in points.windows(2) {
                            push_repeated_drill(
                                &mut drills,
                                &group_key,
                                source_name,
                                source_tool_number,
                                diameter_mm,
                                DrillFeature::Slot {
                                    start: points[0],
                                    end: points[1],
                                },
                                repeat,
                            );
                        }
                        current = end;
                    }
                    Operation::Move(_) => unreachable!("handled above"),
                }
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

pub(crate) fn parse_gerber(
    source_name: &str,
    role: PcbLayerRole,
    bytes: &[u8],
) -> Result<LayerGeometry, PcbError> {
    let layer = load_layer(source_name, role, bytes)?;
    render_gerber(source_name, role, layer)
}

fn load_layer(
    source_name: &str,
    role: PcbLayerRole,
    bytes: &[u8],
) -> Result<GerberLayerData, PcbError> {
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
    if layer.header.iter().chain(&layer.commands).any(|command| {
        matches!(
            command,
            Command::ExtendedCode(ExtendedCode::FileAttribute(FileAttribute::FilePolarity(
                FilePolarity::Negative
            ))) | Command::FunctionCode(FunctionCode::GCode(GCode::Comment(
                CommentContent::Standard(StandardComment::FileAttribute(
                    FileAttribute::FilePolarity(FilePolarity::Negative)
                ))
            )))
        )
    }) {
        return Err(PcbError::UnsupportedGerberFeature(
            source_name.to_owned(),
            "negative file polarity without a finite image boundary".to_owned(),
        ));
    }

    Ok(layer)
}

fn render_gerber(
    source_name: &str,
    role: PcbLayerRole,
    layer: GerberLayerData,
) -> Result<LayerGeometry, PcbError> {
    let mut image = CamPaths::default();
    let mut selected_aperture = None;
    let mut current = PcbPoint::default();
    let mut interpolation = InterpolationMode::Linear;
    let mut quadrant_mode = QuadrantMode::Single;
    let mut polarity = Polarity::Dark;
    let mut repeat = None;
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
            Command::ExtendedCode(ExtendedCode::StepAndRepeat(StepAndRepeat::Open {
                repeat_x,
                repeat_y,
                distance_x,
                distance_y,
            })) => {
                repeat = Some(checked_repeat(
                    *repeat_x,
                    *repeat_y,
                    *distance_x,
                    *distance_y,
                    source_name,
                )?);
            }
            Command::ExtendedCode(ExtendedCode::StepAndRepeat(StepAndRepeat::Close)) => {
                repeat = None;
            }
            Command::ExtendedCode(
                ExtendedCode::LoadMirroring(_)
                | ExtendedCode::LoadRotation(_)
                | ExtendedCode::LoadScaling(_)
                | ExtendedCode::ApertureBlock(_),
            ) => {
                return Err(PcbError::UnsupportedGerberFeature(
                    source_name.to_owned(),
                    "aperture transform LM/LR/LS or aperture block AB".to_owned(),
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
                    image = combine_repeated(
                        image,
                        Paths::new(vec![Path::new(std::mem::take(&mut region_path))]),
                        polarity == Polarity::Dark,
                        repeat,
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
                                image = combine_repeated(
                                    image,
                                    Paths::new(vec![Path::new(std::mem::take(&mut region_path))]),
                                    polarity == Polarity::Dark,
                                    repeat,
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
                        let primitive =
                            aperture::flash(aperture, &layer.macros, current, source_name)?;
                        image =
                            combine_repeated(image, primitive, polarity == Polarity::Dark, repeat)?;
                    }
                    Operation::Interpolate(coordinates, offset) => {
                        let next = modal_point(current, coordinates.as_ref());
                        let points = flatten_interpolation(
                            current,
                            next,
                            offset.as_ref(),
                            interpolation,
                            quadrant_mode,
                            source_name,
                        )?;
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
                            let primitive = aperture::stroke(aperture, centerline, source_name)?;
                            image = combine_repeated(
                                image,
                                primitive,
                                polarity == Polarity::Dark,
                                repeat,
                            )?;
                        }
                        current = next;
                    }
                }
            }
            _ => {}
        }
    }
    if region_path.len() >= 3 {
        image = combine_repeated(
            image,
            Paths::new(vec![Path::new(region_path)]),
            polarity == Polarity::Dark,
            repeat,
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

#[derive(Debug, Clone, Copy)]
struct Repeat {
    x: u32,
    y: u32,
    distance_x_mm: f64,
    distance_y_mm: f64,
}

fn checked_repeat(
    x: u32,
    y: u32,
    distance_x_mm: f64,
    distance_y_mm: f64,
    source_name: &str,
) -> Result<Repeat, PcbError> {
    let copies = u64::from(x) * u64::from(y);
    if x == 0
        || y == 0
        || copies > 10_000
        || !distance_x_mm.is_finite()
        || !distance_y_mm.is_finite()
    {
        return Err(PcbError::UnsupportedGerberFeature(
            source_name.to_owned(),
            "invalid or excessive step-and-repeat".to_owned(),
        ));
    }
    Ok(Repeat {
        x,
        y,
        distance_x_mm,
        distance_y_mm,
    })
}

fn push_repeated_drill(
    drills: &mut Vec<DrillGeometry>,
    group_key: &str,
    source_name: &str,
    source_tool_number: u32,
    diameter_mm: f64,
    feature: DrillFeature,
    repeat: Option<Repeat>,
) {
    let repeat = repeat.unwrap_or(Repeat {
        x: 1,
        y: 1,
        distance_x_mm: 0.0,
        distance_y_mm: 0.0,
    });
    for y in 0..repeat.y {
        for x in 0..repeat.x {
            let x_mm = f64::from(x) * repeat.distance_x_mm;
            let y_mm = f64::from(y) * repeat.distance_y_mm;
            let translate = |point: PcbPoint| PcbPoint {
                x_mm: point.x_mm + x_mm,
                y_mm: point.y_mm + y_mm,
            };
            let feature = match feature {
                DrillFeature::Hit(point) => DrillFeature::Hit(translate(point)),
                DrillFeature::Slot { start, end } => DrillFeature::Slot {
                    start: translate(start),
                    end: translate(end),
                },
            };
            drills.push(DrillGeometry {
                group_key: group_key.to_owned(),
                source_name: source_name.to_owned(),
                source_tool_number,
                diameter_mm,
                feature,
            });
        }
    }
}

fn combine_repeated(
    mut image: CamPaths,
    primitive: CamPaths,
    dark: bool,
    repeat: Option<Repeat>,
) -> Result<CamPaths, PcbError> {
    let repeat = repeat.unwrap_or(Repeat {
        x: 1,
        y: 1,
        distance_x_mm: 0.0,
        distance_y_mm: 0.0,
    });
    for y in 0..repeat.y {
        for x in 0..repeat.x {
            let translated = translate_paths(
                &primitive,
                f64::from(x) * repeat.distance_x_mm,
                f64::from(y) * repeat.distance_y_mm,
            );
            image = combine(image, translated, dark)?;
        }
    }
    Ok(image)
}

fn translate_paths(paths: &CamPaths, x_mm: f64, y_mm: f64) -> CamPaths {
    Paths::new(
        paths
            .iter()
            .map(|path| {
                Path::new(
                    path.iter()
                        .map(|point| Point::new(point.x() + x_mm, point.y() + y_mm))
                        .collect(),
                )
            })
            .collect(),
    )
}

fn normalize_legacy_coordinate_commands(
    source_name: &str,
    bytes: &[u8],
) -> Result<Vec<u8>, PcbError> {
    const ABSOLUTE_COMMENT: &[u8] = b"G04 Millo accepted legacy G90 absolute mode*";
    const METRIC_COMMENT: &[u8] = b"G04 Millo accepted legacy G71 metric mode*";
    const INCH_COMMENT: &[u8] = b"G04 Millo accepted legacy G70 inch mode*";

    let mut normalized = Vec::with_capacity(bytes.len());
    let mut copied_until = 0usize;
    let mut command_start = 0usize;
    let mut in_extended = false;
    let modern_metric = bytes.windows(b"%MOMM".len()).any(|value| value == b"%MOMM");
    let modern_inch = bytes.windows(b"%MOIN".len()).any(|value| value == b"%MOIN");

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
        if legacy_suffix(trimmed, b"G91").is_some() {
            return Err(PcbError::UnsupportedGerberFeature(
                source_name.to_owned(),
                "incremental coordinates".to_owned(),
            ));
        }
        let replacement = legacy_suffix(trimmed, b"G90")
            .map(|suffix| legacy_replacement(suffix, ABSOLUTE_COMMENT).to_vec())
            .or_else(|| {
                legacy_suffix(trimmed, b"G70").map(|suffix| {
                    legacy_unit_replacement(suffix, b"%MOIN*%", modern_inch, INCH_COMMENT)
                })
            })
            .or_else(|| {
                legacy_suffix(trimmed, b"G71").map(|suffix| {
                    legacy_unit_replacement(suffix, b"%MOMM*%", modern_metric, METRIC_COMMENT)
                })
            });
        if (legacy_suffix(trimmed, b"G70").is_some() && modern_metric)
            || (legacy_suffix(trimmed, b"G71").is_some() && modern_inch)
        {
            return Err(PcbError::UnsupportedGerberFeature(
                source_name.to_owned(),
                "conflicting legacy and extended units".to_owned(),
            ));
        }
        if let Some(replacement) = replacement {
            normalized.extend_from_slice(&bytes[copied_until..command_start + leading_whitespace]);
            normalized.extend_from_slice(&replacement);
            copied_until = index + 1;
        }
        command_start = index + 1;
    }
    normalized.extend_from_slice(&bytes[copied_until..]);
    Ok(normalized)
}

fn legacy_suffix<'a>(command: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    let suffix = command.strip_prefix(prefix)?;
    (suffix == b"*" || suffix.starts_with(b"D")).then_some(suffix)
}

fn legacy_replacement<'a>(suffix: &'a [u8], comment: &'a [u8]) -> &'a [u8] {
    if suffix == b"*" { comment } else { suffix }
}

fn legacy_unit_replacement(
    suffix: &[u8],
    extended_code: &[u8],
    already_declared: bool,
    comment: &[u8],
) -> Vec<u8> {
    if already_declared {
        return legacy_replacement(suffix, comment).to_vec();
    }
    let mut replacement = extended_code.to_vec();
    if suffix != b"*" {
        replacement.push(b'\n');
        replacement.extend_from_slice(suffix);
    }
    replacement
}

fn layer_type(role: PcbLayerRole) -> LayerType {
    match role {
        PcbLayerRole::Copper => LayerType::Top,
        PcbLayerRole::Outline => LayerType::Dimensions,
        PcbLayerRole::Marking => LayerType::SilkScreenTop,
        PcbLayerRole::Drill => LayerType::Drill,
        PcbLayerRole::Ignore => LayerType::UndefinedGerber,
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

fn flatten_interpolation(
    start: PcbPoint,
    end: PcbPoint,
    offset: Option<&lib_gerber_edit::gerber_types::CoordinateOffset>,
    mode: InterpolationMode,
    quadrant_mode: QuadrantMode,
    source_name: &str,
) -> Result<Vec<PcbPoint>, PcbError> {
    if mode == InterpolationMode::Linear {
        return Ok(vec![start, end]);
    }
    let offset = offset.ok_or_else(|| {
        PcbError::UnsupportedGerberFeature(
            source_name.to_owned(),
            "circular interpolation without I/J offset".to_owned(),
        )
    })?;
    let offset = PcbPoint {
        x_mm: offset.x.map(f64::from).unwrap_or(0.0),
        y_mm: offset.y.map(f64::from).unwrap_or(0.0),
    };
    let center = match quadrant_mode {
        QuadrantMode::Multi => PcbPoint {
            x_mm: start.x_mm + offset.x_mm,
            y_mm: start.y_mm + offset.y_mm,
        },
        QuadrantMode::Single => resolve_single_quadrant_center(start, end, offset, mode)
            .ok_or_else(|| {
                PcbError::UnsupportedGerberFeature(
                    source_name.to_owned(),
                    "ambiguous single-quadrant circular interpolation".to_owned(),
                )
            })?,
    };
    let radius = ((start.x_mm - center.x_mm).powi(2) + (start.y_mm - center.y_mm).powi(2)).sqrt();
    if radius <= 1e-9 {
        return Err(PcbError::UnsupportedGerberFeature(
            source_name.to_owned(),
            "zero-radius circular interpolation".to_owned(),
        ));
    }
    let start_angle = (start.y_mm - center.y_mm).atan2(start.x_mm - center.x_mm);
    let end_angle = (end.y_mm - center.y_mm).atan2(end.x_mm - center.x_mm);
    let sweep = directed_sweep(start_angle, end_angle, mode);
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
    Ok(points)
}

fn resolve_single_quadrant_center(
    start: PcbPoint,
    end: PcbPoint,
    offset: PcbPoint,
    mode: InterpolationMode,
) -> Option<PcbPoint> {
    let i = offset.x_mm.abs();
    let j = offset.y_mm.abs();
    let mut candidates = Vec::with_capacity(4);
    for x_sign in [-1.0, 1.0] {
        for y_sign in [-1.0, 1.0] {
            let center = PcbPoint {
                x_mm: start.x_mm + x_sign * i,
                y_mm: start.y_mm + y_sign * j,
            };
            let start_radius = (start.x_mm - center.x_mm).hypot(start.y_mm - center.y_mm);
            let end_radius = (end.x_mm - center.x_mm).hypot(end.y_mm - center.y_mm);
            let radius_error = (start_radius - end_radius).abs();
            let tolerance = (start_radius * 0.001).max(0.005);
            let start_angle = (start.y_mm - center.y_mm).atan2(start.x_mm - center.x_mm);
            let end_angle = (end.y_mm - center.y_mm).atan2(end.x_mm - center.x_mm);
            let sweep = directed_sweep(start_angle, end_angle, mode);
            if start_radius > 1e-9
                && radius_error <= tolerance
                && sweep.abs() <= std::f64::consts::FRAC_PI_2 + 1e-6
            {
                candidates.push((radius_error, sweep.abs(), center));
            }
        }
    }
    candidates.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
    });
    candidates.first().map(|candidate| candidate.2)
}

fn directed_sweep(start_angle: f64, end_angle: f64, mode: InterpolationMode) -> f64 {
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
    sweep
}
