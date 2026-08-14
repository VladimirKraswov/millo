use clipper2::{FillRule, Milli, Path, Paths, Point, difference, union};

use crate::{PcbBounds, PcbError, PcbLayerRole, PcbPoint, PcbTransform};

pub(crate) type CamPath = Path<Milli>;
pub(crate) type CamPaths = Paths<Milli>;

#[derive(Debug, Clone)]
pub(crate) struct LayerGeometry {
    pub source_name: String,
    pub role: PcbLayerRole,
    pub paths: CamPaths,
}

#[derive(Debug, Clone)]
pub(crate) struct DrillGeometry {
    pub group_key: String,
    pub source_name: String,
    pub source_tool_number: u32,
    pub diameter_mm: f64,
    pub point: PcbPoint,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct BoardGeometry {
    pub layers: Vec<LayerGeometry>,
    pub drills: Vec<DrillGeometry>,
    pub warnings: Vec<String>,
}

pub(crate) fn combine(
    current: CamPaths,
    primitive: CamPaths,
    dark: bool,
) -> Result<CamPaths, PcbError> {
    if primitive.is_empty() {
        return Ok(current);
    }
    if current.is_empty() {
        return Ok(if dark { primitive } else { current });
    }
    let result = if dark {
        union(current, primitive, FillRule::NonZero)
    } else {
        difference(current, primitive, FillRule::NonZero)
    };
    result.map_err(|error| PcbError::Geometry(error.to_string()))
}

pub(crate) fn circle(center: PcbPoint, radius_mm: f64) -> CamPath {
    let circumference = std::f64::consts::TAU * radius_mm.max(0.001);
    let segments = ((circumference / 0.04).ceil() as usize).clamp(16, 192);
    Path::new(
        (0..segments)
            .map(|index| {
                let angle = std::f64::consts::TAU * index as f64 / segments as f64;
                Point::new(
                    center.x_mm + radius_mm * angle.cos(),
                    center.y_mm + radius_mm * angle.sin(),
                )
            })
            .collect(),
    )
}

pub(crate) fn polygon(center: PcbPoint, radius_mm: f64, vertices: usize, rotation: f64) -> CamPath {
    Path::new(
        (0..vertices)
            .map(|index| {
                let angle = rotation + std::f64::consts::TAU * index as f64 / vertices as f64;
                Point::new(
                    center.x_mm + radius_mm * angle.cos(),
                    center.y_mm + radius_mm * angle.sin(),
                )
            })
            .collect(),
    )
}

pub(crate) fn rectangle(center: PcbPoint, width: f64, height: f64) -> CamPath {
    let half_x = width / 2.0;
    let half_y = height / 2.0;
    vec![
        (center.x_mm - half_x, center.y_mm - half_y),
        (center.x_mm + half_x, center.y_mm - half_y),
        (center.x_mm + half_x, center.y_mm + half_y),
        (center.x_mm - half_x, center.y_mm + half_y),
    ]
    .into()
}

pub(crate) fn transform_board(board: &mut BoardGeometry, transform: PcbTransform) -> PcbBounds {
    let initial = raw_bounds(board).unwrap_or_default();
    let quarter_turns = transform.rotation_quarter_turns % 4;
    let rotate = |point: PcbPoint| {
        let x = point.x_mm - initial.min_x_mm;
        let y = point.y_mm - initial.min_y_mm;
        match quarter_turns {
            1 => PcbPoint { x_mm: -y, y_mm: x },
            2 => PcbPoint { x_mm: -x, y_mm: -y },
            3 => PcbPoint { x_mm: y, y_mm: -x },
            _ => PcbPoint { x_mm: x, y_mm: y },
        }
    };
    let mut rotated_points = Vec::new();
    for layer in &board.layers {
        for path in layer.paths.iter() {
            rotated_points.extend(path.iter().map(|point| rotate(point.into())));
        }
    }
    rotated_points.extend(board.drills.iter().map(|drill| rotate(drill.point)));
    let rotated = bounds_for_points(rotated_points.iter().copied()).unwrap_or_default();
    let map = |point: PcbPoint| {
        let rotated_point = rotate(point);
        let x = rotated_point.x_mm - rotated.min_x_mm;
        let y = rotated_point.y_mm - rotated.min_y_mm;
        PcbPoint {
            x_mm: if transform.mirror_x {
                rotated.width_mm - x
            } else {
                x
            } + transform.offset_x_mm,
            y_mm: y + transform.offset_y_mm,
        }
    };
    for layer in &mut board.layers {
        layer.paths = CamPaths::new(
            layer
                .paths
                .iter()
                .map(|path| {
                    CamPath::new(
                        path.iter()
                            .map(|point| {
                                let point = map(point.into());
                                Point::new(point.x_mm, point.y_mm)
                            })
                            .collect(),
                    )
                })
                .collect(),
        );
    }
    for drill in &mut board.drills {
        drill.point = map(drill.point);
    }
    raw_bounds(board).unwrap_or_default()
}

pub(crate) fn raw_bounds(board: &BoardGeometry) -> Option<PcbBounds> {
    let layer_points = board.layers.iter().flat_map(|layer| {
        layer
            .paths
            .iter()
            .flat_map(|path| path.iter().map(PcbPoint::from))
    });
    bounds_for_points(layer_points.chain(board.drills.iter().map(|drill| drill.point)))
}

pub(crate) fn bounds_for_points(points: impl Iterator<Item = PcbPoint>) -> Option<PcbBounds> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for point in points {
        min_x = min_x.min(point.x_mm);
        min_y = min_y.min(point.y_mm);
        max_x = max_x.max(point.x_mm);
        max_y = max_y.max(point.y_mm);
    }
    min_x.is_finite().then_some(PcbBounds {
        min_x_mm: min_x,
        min_y_mm: min_y,
        max_x_mm: max_x,
        max_y_mm: max_y,
        width_mm: max_x - min_x,
        height_mm: max_y - min_y,
    })
}

impl From<&Point<Milli>> for PcbPoint {
    fn from(point: &Point<Milli>) -> Self {
        Self {
            x_mm: point.x(),
            y_mm: point.y(),
        }
    }
}
