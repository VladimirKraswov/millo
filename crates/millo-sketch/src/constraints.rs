use crate::*;
use std::collections::BTreeMap;

/// Directed dimensions, solved independently on X and Y. Cached centres are never
/// trusted for constrained axes; cycles and dangling references fail before CAM.
pub fn resolve_sketch(mut document: SketchJobRequest) -> Result<SketchJobRequest, SketchError> {
    require(document.shapes.len() <= MAX_SHAPES, "Не больше 200 фигур")?;
    let ids: BTreeMap<_, _> = document
        .shapes
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.clone(), i))
        .collect();
    require(
        ids.len() == document.shapes.len(),
        "ID фигур должны быть уникальны",
    )?;
    for axis in 0..2 {
        let mut state = vec![0; document.shapes.len()];
        for index in 0..document.shapes.len() {
            resolve_axis(index, axis, &mut document, &ids, &mut state)?;
        }
    }
    Ok(document)
}

fn resolve_axis(
    index: usize,
    axis: usize,
    doc: &mut SketchJobRequest,
    ids: &BTreeMap<String, usize>,
    state: &mut [u8],
) -> Result<(), SketchError> {
    if state[index] == 2 {
        return Ok(());
    }
    require(
        state[index] != 1,
        format!(
            "Циклическая размерная связь: {} · {}",
            doc.shapes[index].name,
            if axis == 0 { "X" } else { "Y" }
        ),
    )?;
    state[index] = 1;
    let constraint = if axis == 0 {
        &doc.shapes[index].constraints.x
    } else {
        &doc.shapes[index].constraints.y
    }
    .clone();
    if let Some(c) = constraint {
        range(c.offset_mm, -10_000.0, 10_000.0, "Размерная связь")?;
        let target = if let Some(id) = &c.reference_id {
            let other = *ids.get(id).ok_or_else(|| {
                SketchError(format!(
                    "{}: опорная фигура не найдена",
                    doc.shapes[index].name
                ))
            })?;
            resolve_axis(other, axis, doc, ids, state)?;
            coordinate(&doc.shapes[other], axis)
                + anchor_offset(&doc.shapes[other], axis, c.reference_anchor)?
        } else {
            let size = if axis == 0 {
                doc.stock.width_mm
            } else {
                doc.stock.height_mm
            };
            range(size, 1.0, 10_000.0, "Размер листа")?;
            match c.reference_anchor {
                SketchAnchor::Named(SketchAnchorName::Min) => 0.0,
                SketchAnchor::Named(SketchAnchorName::Center) => size / 2.0,
                SketchAnchor::Named(SketchAnchorName::Max) => size,
                _ => return Err(SketchError("У листа нет индексированных вершин".into())),
            }
        };
        let value = target + c.offset_mm - anchor_offset(&doc.shapes[index], axis, c.own_anchor)?;
        range(value, -10_000.0, 10_000.0, "Рассчитанный центр")?;
        if axis == 0 {
            doc.shapes[index].x_mm = value;
        } else {
            doc.shapes[index].y_mm = value;
        }
    }
    state[index] = 2;
    Ok(())
}

fn coordinate(shape: &SketchShape, axis: usize) -> f64 {
    if axis == 0 { shape.x_mm } else { shape.y_mm }
}

fn anchor_offset(
    shape: &SketchShape,
    axis: usize,
    anchor: SketchAnchor,
) -> Result<f64, SketchError> {
    if anchor == SketchAnchor::Named(SketchAnchorName::Center) {
        return Ok(0.0);
    }
    range(shape.rotation_degrees, -360.0, 360.0, "Поворот")?;
    let (sin, cos) = shape.rotation_degrees.to_radians().sin_cos();
    let project = |p: &SketchPoint| {
        if axis == 0 {
            p.x * cos - p.y * sin
        } else {
            p.x * sin + p.y * cos
        }
    };
    let (min, max) = match &shape.geometry {
        SketchGeometry::Polygon { points } => {
            require(
                (3..=256).contains(&points.len()),
                "Контур: нужно от 3 до 256 вершин",
            )?;
            for p in points {
                range(p.x, -10_000.0, 10_000.0, "Вершина X")?;
                range(p.y, -10_000.0, 10_000.0, "Вершина Y")?;
            }
            if let SketchAnchor::Vertex(i) = anchor {
                return points
                    .get(i)
                    .map(project)
                    .ok_or_else(|| SketchError("Опорная вершина не найдена".into()));
            }
            points
                .iter()
                .map(project)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), v| {
                    (min.min(v), max.max(v))
                })
        }
        SketchGeometry::Circle { diameter } => {
            range(*diameter, 0.1, 10_000.0, "Диаметр")?;
            (-diameter / 2.0, diameter / 2.0)
        }
        SketchGeometry::Rectangle {
            width,
            height,
            radius,
        } => {
            range(*width, 0.1, 10_000.0, "Ширина")?;
            range(*height, 0.1, 10_000.0, "Высота")?;
            range(*radius, 0.0, width.min(*height) / 2.0, "Скругление")?;
            let x = width / 2.0 - radius;
            let y = height / 2.0 - radius;
            let extent = if axis == 0 {
                x * cos.abs() + y * sin.abs() + radius
            } else {
                x * sin.abs() + y * cos.abs() + radius
            };
            (-extent, extent)
        }
    };
    match anchor {
        SketchAnchor::Named(SketchAnchorName::Min) => Ok(min),
        SketchAnchor::Named(SketchAnchorName::Max) => Ok(max),
        _ => Err(SketchError(
            "Вершину можно выбрать только у многоугольника".into(),
        )),
    }
}
