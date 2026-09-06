import type {
  SketchAnchor,
  SketchAxis,
  SketchAxisConstraint,
  SketchJobRequest,
  SketchPoint,
  SketchShape,
} from "../../shared/sketch";

export function anchorOffset(
  shape: SketchShape,
  axis: SketchAxis,
  anchor: SketchAnchor,
): number {
  if (anchor === "center") return 0;
  const g = shape.geometry,
    a = (shape.rotationDegrees * Math.PI) / 180;
  const c = Math.cos(a),
    s = Math.sin(a);
  const project = (p: SketchPoint) =>
    axis === "x" ? p.x * c - p.y * s : p.x * s + p.y * c;
  let min: number, max: number;
  if (g.kind === "polygon") {
    if (typeof anchor === "number") {
      if (!Number.isInteger(anchor) || !g.points[anchor])
        throw new Error("Опорная вершина не найдена");
      return project(g.points[anchor]);
    }
    const values = g.points.map(project);
    min = Math.min(...values);
    max = Math.max(...values);
  } else {
    if (typeof anchor === "number")
      throw new Error("Вершину можно выбрать только у многоугольника");
    const extent =
      g.kind === "circle"
        ? g.diameter / 2
        : axis === "x"
          ? (g.width / 2 - g.radius) * Math.abs(c) +
            (g.height / 2 - g.radius) * Math.abs(s) +
            g.radius
          : (g.width / 2 - g.radius) * Math.abs(s) +
            (g.height / 2 - g.radius) * Math.abs(c) +
            g.radius;
    min = -extent;
    max = extent;
  }
  return anchor === "min" ? min : max;
}
export function anchorPoint(
  shape: SketchShape,
  anchor: SketchAnchor,
): SketchPoint {
  return {
    x: shape.xMm + anchorOffset(shape, "x", anchor),
    y: shape.yMm + anchorOffset(shape, "y", anchor),
  };
}
export function referencePoint(
  doc: SketchJobRequest,
  constraint: SketchAxisConstraint,
): SketchPoint {
  if (constraint.referenceId !== undefined) {
    const ref = doc.shapes.find((s) => s.id === constraint.referenceId);
    if (!ref) throw new Error("Опорная фигура не найдена");
    return anchorPoint(ref, constraint.referenceAnchor);
  }
  if (typeof constraint.referenceAnchor === "number")
    throw new Error("У листа нет индексированных вершин");
  const factor =
    constraint.referenceAnchor === "min"
      ? 0
      : constraint.referenceAnchor === "center"
        ? 0.5
        : 1;
  return { x: doc.stock.widthMm * factor, y: doc.stock.heightMm * factor };
}
export function preservePosition(
  doc: SketchJobRequest,
  shape: SketchShape,
  axis: SketchAxis,
  constraint: SketchAxisConstraint,
): SketchAxisConstraint {
  const offsetMm =
    anchorPoint(shape, constraint.ownAnchor)[axis] -
    referencePoint(doc, constraint)[axis];
  return { ...constraint, offsetMm: Math.round(offsetMm * 1e6) / 1e6 };
}

/** Exact directed dimensions, not a general nonlinear CAD solver. Rust resolves
 * the same contract again before CAM; stored x/y are only a display cache. */
export function resolveSketch(document: SketchJobRequest): SketchJobRequest {
  const shapes = [...document.shapes],
    ids = new Map(shapes.map((s, i) => [s.id, i]));
  if (ids.size !== shapes.length || shapes.length > 200)
    throw new Error("Некорректный список фигур");
  for (const axis of ["x", "y"] as const) {
    const state = new Map<number, number>();
    const visit = (i: number) => {
      if (state.get(i) === 2) return;
      if (state.get(i) === 1)
        throw new Error(
          `Циклическая размерная связь: ${shapes[i].name} · ${axis.toUpperCase()}`,
        );
      state.set(i, 1);
      const constraint = shapes[i].constraints?.[axis];
      if (constraint) {
        if (
          !Number.isFinite(constraint.offsetMm) ||
          Math.abs(constraint.offsetMm) > 10_000
        )
          throw new Error("Размерная связь: допустимо от −10000 до 10000 мм");
        if (constraint.referenceId !== undefined) {
          const other = ids.get(constraint.referenceId);
          if (other === undefined)
            throw new Error(`${shapes[i].name}: опорная фигура не найдена`);
          visit(other);
        }
        const value =
          referencePoint({ ...document, shapes }, constraint)[axis] +
          constraint.offsetMm -
          anchorOffset(shapes[i], axis, constraint.ownAnchor);
        if (!Number.isFinite(value) || Math.abs(value) > 10_000)
          throw new Error("Рассчитанная координата вне допустимого диапазона");
        const key = axis === "x" ? "xMm" : "yMm";
        if (Math.abs(shapes[i][key] - value) > 1e-9)
          shapes[i] = { ...shapes[i], [key]: value };
      }
      state.set(i, 2);
    };
    shapes.forEach((_, i) => visit(i));
  }
  return shapes.every((s, i) => s === document.shapes[i])
    ? document
    : { ...document, shapes };
}
export function removeSketchShapes(
  doc: SketchJobRequest,
  ids: readonly string[],
): SketchJobRequest {
  const resolved = resolveSketch(doc);
  return {
    ...resolved,
    shapes: resolved.shapes
      .filter((s) => !ids.includes(s.id))
      .map((s) => ({
        ...s,
        constraints: Object.fromEntries(
          Object.entries(s.constraints ?? {}).filter(
            ([, c]) => c && !ids.includes(c.referenceId ?? ""),
          ),
        ),
      })),
  };
}
export function arrangeShapes(
  doc: SketchJobRequest,
  ids: readonly string[],
  referenceId: string,
  axis: SketchAxis,
  step?: number,
): SketchJobRequest {
  let index = 0;
  const cross = axis === "x" ? "y" : "x";
  return resolveSketch({
    ...doc,
    shapes: doc.shapes.map((shape) => {
      if (!ids.includes(shape.id) || shape.id === referenceId) return shape;
      if (shape.locked)
        throw new Error(`${shape.name}: сначала разблокируйте положение`);
      index++;
      const center = {
        referenceId,
        referenceAnchor: "center" as const,
        ownAnchor: "center" as const,
        offsetMm: 0,
      };
      return {
        ...shape,
        constraints: {
          ...shape.constraints,
          [axis]: {
            ...center,
            offsetMm: step === undefined ? 0 : index * step,
          },
          ...(step === undefined ? {} : { [cross]: center }),
        },
      };
    }),
  });
}
export function moveSketchShape(
  shape: SketchShape,
  point: SketchPoint,
): SketchShape {
  if (shape.locked) return shape;
  return {
    ...shape,
    xMm: shape.constraints?.x ? shape.xMm : point.x,
    yMm: shape.constraints?.y ? shape.yMm : point.y,
  };
}
