use crate::{
    geometry::{self, Contour},
    *,
};
use millo_tooling::{CuttingTool, ToolKind};
use std::collections::BTreeSet;

pub(crate) struct PlannedOperation<'a> {
    pub shape: &'a SketchShape,
    pub tool: &'a CuttingTool,
    pub paths: Vec<Vec<SketchPoint>>,
    pub depth: f64,
    pub passes: usize,
}

pub(crate) fn plan<'a>(
    request: &'a SketchJobRequest,
    tools: &'a [CuttingTool],
) -> Result<Vec<PlannedOperation<'a>>, SketchError> {
    let s = &request.stock;
    range(s.width_mm, 1.0, 10_000.0, "Ширина листа")?;
    range(s.height_mm, 1.0, 10_000.0, "Высота листа")?;
    range(s.thickness_mm, 0.05, 100.0, "Толщина материала")?;
    range(s.safe_z_mm, 0.5, 100.0, "Безопасный Z над материалом")?;
    range(s.breakthrough_mm, 0.0, 1.0, "Выход в подложку")?;
    require(!request.source_name.trim().is_empty(), "Назовите чертёж")?;
    require(
        request.source_name.chars().count() <= 100,
        "Название чертежа длиннее 100 символов",
    )?;
    require(
        (1..=MAX_SHAPES).contains(&request.shapes.len()),
        "Добавьте от 1 до 200 фигур",
    )?;
    let mut ids = BTreeSet::new();
    let mut operations = Vec::new();
    let mut contours = Vec::new();
    let mut total_points: usize = 0;
    for shape in &request.shapes {
        require(
            shape.id.len() <= 100 && shape.name.chars().count() <= 120,
            "Слишком длинное имя или ID фигуры",
        )?;
        require(
            !shape.id.is_empty() && ids.insert(&shape.id),
            "ID фигур должны быть уникальны",
        )?;
        let result = plan_shape(shape, request, tools);
        let (operation, contour) =
            result.map_err(|e| SketchError(format!("{}: {}", shape.name, e.0)))?;
        total_points = total_points.saturating_add(
            operation
                .paths
                .iter()
                .map(Vec::len)
                .sum::<usize>()
                .saturating_mul(operation.passes),
        );
        require(
            total_points <= MAX_POINTS,
            "Слишком много проходов: увеличьте шаг или уменьшите чертёж",
        )?;
        contours.push(contour);
        operations.push(operation);
    }
    for a in 0..contours.len() {
        for b in a + 1..contours.len() {
            let overlap = geometry::overlap(&contours[a], &contours[b])?;
            if overlap < 0.005 {
                continue;
            }
            let enclosed = |outer: usize, inner: usize| {
                operations[outer].shape.operation.kind == SketchOperationKind::Outside
                    && contours[outer].signed_area().abs()
                        > contours[inner].signed_area().abs() + 0.005
                    && (overlap - contours[inner].signed_area().abs()).abs() < 0.005
            };
            require(
                enclosed(a, b) || enclosed(b, a),
                format!(
                    "{} / {}: области пересекаются; разнесите их. Вложенные карманы пока не поддерживаются",
                    operations[a].shape.name, operations[b].shape.name
                ),
            )?;
        }
    }
    // Keep internal features supported by stock until all pockets/holes are finished.
    // Nested exterior parts are cut from smallest to largest, never by tool grouping alone.
    operations.sort_by(|a, b| {
        let stage = |op: &PlannedOperation<'_>| match op.shape.operation.kind {
            SketchOperationKind::Outside => 2,
            SketchOperationKind::Inside => 1,
            _ => 0,
        };
        stage(a).cmp(&stage(b)).then_with(|| {
            if a.shape.operation.kind == SketchOperationKind::Outside {
                path_area(&a.paths[0]).total_cmp(&path_area(&b.paths[0]))
            } else {
                std::cmp::Ordering::Equal
            }
        })
    });
    Ok(operations)
}

fn plan_shape<'a>(
    shape: &'a SketchShape,
    request: &SketchJobRequest,
    tools: &'a [CuttingTool],
) -> Result<(PlannedOperation<'a>, Contour), SketchError> {
    let op = &shape.operation;
    require(
        !(op.kind == SketchOperationKind::Engrave && op.through),
        "Линия по контуру требует заданной глубины, а не сквозного реза",
    )?;
    let tool = tools
        .iter()
        .find(|t| t.id == op.tool_id)
        .ok_or_else(|| SketchError("Выберите инструмент из библиотеки".into()))?;
    let permitted = match op.kind {
        SketchOperationKind::Drill => tool.kind == ToolKind::Drill,
        SketchOperationKind::Engrave => matches!(
            tool.kind,
            ToolKind::FlatEndMill | ToolKind::VBit | ToolKind::Engraving
        ),
        _ => tool.kind == ToolKind::FlatEndMill,
    };
    require(
        permitted,
        "Для кармана и реза нужна плоская концевая фреза, для сверления нужно сверло",
    )?;
    range(tool.diameter_mm, 0.05, 100.0, "Диаметр инструмента")?;
    let depth = if op.through {
        request.stock.thickness_mm + request.stock.breakthrough_mm
    } else {
        op.depth_mm
    };
    range(
        depth,
        0.01,
        tool.cutting_length_mm
            .min(request.stock.thickness_mm + request.stock.breakthrough_mm),
        "Глубина реза",
    )?;
    range(op.stepdown_mm, 0.01, 10.0, "Съём за проход")?;
    range(op.stepover_percent, 5.0, 50.0, "Боковой шаг, % диаметра")?;
    range(op.feed_mm_per_min, 1.0, 30_000.0, "Подача XY")?;
    range(op.plunge_mm_per_min, 1.0, 10_000.0, "Подача Z")?;
    require(
        (1_000..=100_000).contains(&op.spindle_rpm),
        "Обороты: от 1000 до 100000 rpm",
    )?;
    let passes = (depth / op.stepdown_mm).ceil() as usize;
    require(
        passes <= MAX_PASSES,
        "Больше 200 проходов по Z; увеличьте съём за проход",
    )?;
    let contour = geometry::contour(shape)?;
    for p in contour.iter() {
        require(
            p.x() >= -0.001
                && p.y() >= -0.001
                && p.x() <= request.stock.width_mm + 0.001
                && p.y() <= request.stock.height_mm + 0.001,
            "Фигура выходит за лист",
        )?;
    }
    let radius = tool
        .cutting_diameter_at_depth_mm(depth)
        .filter(|d| d.is_finite() && *d > 0.0)
        .ok_or_else(|| SketchError("Неизвестна геометрия режущей части".into()))?
        / 2.0;
    let mut paths = Vec::new();
    match op.kind {
        SketchOperationKind::Drill => {
            let SketchGeometry::Circle { diameter } = shape.geometry else {
                return Err(SketchError(
                    "Сверление возможно только в центре круга".into(),
                ));
            };
            require(
                (diameter - tool.diameter_mm).abs() <= 0.01,
                "Диаметр сверла должен совпадать с отверстием; для большего отверстия выберите карман и фрезу",
            )?;
            paths.push(vec![SketchPoint {
                x: shape.x_mm,
                y: shape.y_mm,
            }]);
        }
        SketchOperationKind::Engrave => paths.push(geometry::points(&contour)),
        SketchOperationKind::Inside | SketchOperationKind::Outside => {
            let delta = if op.kind == SketchOperationKind::Inside {
                -radius
            } else {
                radius
            };
            let offset = geometry::offset(&contour, delta);
            require(
                offset.len() == 1,
                "Фреза не помещается в контур или разделяет его; выберите меньший диаметр",
            )?;
            paths.extend(offset.iter().map(geometry::points));
        }
        SketchOperationKind::Pocket => {
            let step = radius * 2.0 * op.stepover_percent / 100.0;
            for ring in 0..=2_000 {
                let offset = geometry::offset(&contour, -(radius + ring as f64 * step));
                if offset.is_empty() {
                    break;
                }
                require(ring < 2_000, "Карманы: превышен предел 2000 колец")?;
                paths.extend(offset.iter().map(geometry::points));
                require(
                    paths.iter().map(Vec::len).sum::<usize>() <= MAX_POINTS / passes,
                    "Карманы: превышен предел точек",
                )?;
            }
            require(
                !paths.is_empty(),
                "Фреза не помещается в карман; выберите меньшую фрезу",
            )?;
            // Start in the centre and expand, with a safe retract between disconnected loops.
            paths.reverse();
        }
    }
    for p in paths.iter().flatten() {
        require(
            p.x - radius >= -0.01
                && p.y - radius >= -0.01
                && p.x + radius <= request.stock.width_mm + 0.01
                && p.y + radius <= request.stock.height_mm + 0.01,
            "Режущая часть выходит за лист. Оставьте припуск у края или увеличьте размер заготовки",
        )?;
    }
    if op.tabs.count > 0 {
        require(
            op.through
                && matches!(
                    op.kind,
                    SketchOperationKind::Inside | SketchOperationKind::Outside
                ),
            "Перемычки доступны только для сквозного реза по контуру",
        )?;
        require(
            (2..=16).contains(&op.tabs.count),
            "Перемычек должно быть от 2 до 16",
        )?;
        range(op.tabs.width_mm, 0.5, 50.0, "Ширина перемычки")?;
        range(
            op.tabs.height_mm,
            0.05,
            request.stock.thickness_mm,
            "Высота перемычки от низа листа",
        )?;
        let perimeter: f64 = paths[0]
            .iter()
            .enumerate()
            .map(|(i, p)| geometry::distance(*p, paths[0][(i + 1) % paths[0].len()]))
            .sum();
        require(
            f64::from(op.tabs.count) * (op.tabs.width_mm + tool.diameter_mm) < perimeter * 0.75,
            "Перемычки занимают слишком большую часть контура",
        )?;
    }
    Ok((
        PlannedOperation {
            shape,
            tool,
            paths,
            depth,
            passes,
        },
        contour,
    ))
}

fn path_area(path: &[SketchPoint]) -> f64 {
    path.iter()
        .enumerate()
        .map(|(i, a)| {
            let b = path[(i + 1) % path.len()];
            a.x * b.y - a.y * b.x
        })
        .sum::<f64>()
        .abs()
        / 2.0
}
