use clipper2::{EndType, FillRule, JoinType, Milli, Path, Paths, intersect, union};

use crate::{SketchError, SketchGeometry, SketchPoint, SketchShape, range, require};

pub(crate) type Contours = Paths<Milli>;
pub(crate) type Contour = Path<Milli>;

pub(crate) fn contour(shape: &SketchShape) -> Result<Contour, SketchError> {
    range(shape.x_mm, 0.0, 10_000.0, "Центр X")?;
    range(shape.y_mm, 0.0, 10_000.0, "Центр Y")?;
    range(shape.rotation_degrees, -360.0, 360.0, "Поворот")?;
    let local: Vec<(f64, f64)> = match &shape.geometry {
        SketchGeometry::Circle { diameter } => {
            range(*diameter, 0.1, 10_000.0, "Диаметр")?;
            arc(0.0, 0.0, diameter / 2.0, 0.0, std::f64::consts::TAU)
        }
        SketchGeometry::Rectangle {
            width,
            height,
            radius,
        } => {
            range(*width, 0.1, 10_000.0, "Ширина")?;
            range(*height, 0.1, 10_000.0, "Высота")?;
            range(*radius, 0.0, width.min(*height) / 2.0, "Скругление")?;
            let (x, y) = (width / 2.0, height / 2.0);
            if *radius < 0.001 {
                vec![(-x, -y), (x, -y), (x, y), (-x, y)]
            } else {
                [
                    (x - radius, y - radius),
                    (-x + radius, y - radius),
                    (-x + radius, -y + radius),
                    (x - radius, -y + radius),
                ]
                .into_iter()
                .enumerate()
                .flat_map(|(i, (cx, cy))| {
                    arc(
                        cx,
                        cy,
                        *radius,
                        i as f64 * std::f64::consts::FRAC_PI_2,
                        std::f64::consts::FRAC_PI_2,
                    )
                })
                .collect()
            }
        }
        SketchGeometry::Polygon { points } => {
            require(
                (3..=256).contains(&points.len()),
                "Контур: нужно от 3 до 256 вершин",
            )?;
            for point in points {
                range(point.x, -10_000.0, 10_000.0, "Вершина X")?;
                range(point.y, -10_000.0, 10_000.0, "Вершина Y")?;
            }
            points.iter().map(|p| (p.x, p.y)).collect()
        }
    };
    let (sin, cos) = shape.rotation_degrees.to_radians().sin_cos();
    let input: Contour = local
        .into_iter()
        .map(|(x, y)| {
            (
                shape.x_mm + x * cos - y * sin,
                shape.y_mm + x * sin + y * cos,
            )
        })
        .collect::<Vec<_>>()
        .into();
    let area = input.signed_area().abs();
    require(area > 0.005, "Контур вырожден или пересекает сам себя")?;
    // Canonical winding and removal of intersecting edges before any offset.
    let normalized = union::<Milli>(Paths::new(vec![input]), Paths::default(), FillRule::NonZero)
        .map_err(|e| SketchError(e.to_string()))?;
    require(
        normalized.len() == 1 && (normalized.signed_area().abs() - area).abs() < 0.005,
        "Самопересекающийся контур: разделите его на отдельные фигуры",
    )?;
    Ok(normalized.iter().next().expect("one contour").clone())
}

fn arc(cx: f64, cy: f64, radius: f64, start: f64, sweep: f64) -> Vec<(f64, f64)> {
    // Chord error <= 0.005 mm, including large circles (no fixed segment cap).
    let angle = (1.0 - (0.005 / radius).min(1.0)).acos() * 2.0;
    let steps = (sweep / angle).ceil().max(4.0) as usize;
    (0..=steps)
        .map(|i| {
            let a = start + sweep * i as f64 / steps as f64;
            (cx + radius * a.cos(), cy + radius * a.sin())
        })
        .collect()
}

pub(crate) fn offset(path: &Contour, amount: f64) -> Contours {
    path.inflate(amount, JoinType::Round, EndType::Polygon, 2.0)
        .simplify(0.001, false)
}

pub(crate) fn points(path: &Contour) -> Vec<SketchPoint> {
    path.iter()
        .map(|p| SketchPoint { x: p.x(), y: p.y() })
        .collect()
}

pub(crate) fn overlap(a: &Contour, b: &Contour) -> Result<f64, SketchError> {
    intersect::<Milli>(
        Paths::new(vec![a.clone()]),
        Paths::new(vec![b.clone()]),
        FillRule::NonZero,
    )
    .map(|p| p.signed_area().abs())
    .map_err(|e| SketchError(e.to_string()))
}

pub(crate) fn distance(a: SketchPoint, b: SketchPoint) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}
