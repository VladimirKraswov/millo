import type {
  SketchAxis,
  SketchGeometry,
  SketchJobRequest,
} from "../../shared/sketch";
import { resolveSketch } from "./sketchConstraints";

export type SketchDimensionTarget =
  | {
      readonly shapeId: string;
      readonly kind: "size";
      readonly axis: SketchAxis;
    }
  | {
      readonly shapeId: string;
      readonly kind: "offset";
      readonly axis: SketchAxis;
    };

export function localGeometryBounds(g: SketchGeometry) {
  if (g.kind === "polygon")
    return {
      minX: Math.min(...g.points.map((p) => p.x)),
      maxX: Math.max(...g.points.map((p) => p.x)),
      minY: Math.min(...g.points.map((p) => p.y)),
      maxY: Math.max(...g.points.map((p) => p.y)),
    };
  const w = g.kind === "circle" ? g.diameter : g.width;
  const h = g.kind === "circle" ? g.diameter : g.height;
  return { minX: -w / 2, maxX: w / 2, minY: -h / 2, maxY: h / 2 };
}

export function resizeSketchGeometry(
  g: SketchGeometry,
  axis: SketchAxis,
  value: number,
): SketchGeometry {
  if (!Number.isFinite(value) || value < 0.1 || value > 10_000)
    throw new Error("Размер должен быть от 0,1 до 10000 мм");
  if (g.kind === "circle") return { ...g, diameter: value };
  if (g.kind === "rectangle")
    return {
      ...g,
      [axis === "x" ? "width" : "height"]: value,
      radius: Math.min(g.radius, value / 2),
    };
  const bounds = localGeometryBounds(g);
  const min = axis === "x" ? bounds.minX : bounds.minY;
  const max = axis === "x" ? bounds.maxX : bounds.maxY;
  if (max - min < 1e-9) throw new Error("У контура нет размера по этой оси");
  const center = (min + max) / 2,
    scale = value / (max - min);
  const points = g.points.map((p) => ({
    ...p,
    [axis]: center + (p[axis] - center) * scale,
  }));
  if (points.some((p) => Math.abs(p.x) > 10_000 || Math.abs(p.y) > 10_000))
    throw new Error("Вершина выходит за допустимый диапазон координат");
  return { ...g, points };
}

/** The canvas and inspector share the same geometry; a dimension edit is one undo step. */
export function applySketchDimension(
  doc: SketchJobRequest,
  target: SketchDimensionTarget,
  value: number,
): SketchJobRequest {
  const shape = doc.shapes.find((s) => s.id === target.shapeId);
  if (!shape) throw new Error("Фигура больше не существует");
  let next = shape;
  if (target.kind === "size") {
    next = {
      ...shape,
      geometry: resizeSketchGeometry(shape.geometry, target.axis, value),
    };
  } else {
    const constraint = shape.constraints?.[target.axis];
    if (shape.locked) throw new Error("Сначала разблокируйте положение фигуры");
    if (!constraint) throw new Error("Размерная связь больше не существует");
    if (!Number.isFinite(value) || Math.abs(value) > 10_000)
      throw new Error("Смещение должно быть от −10000 до 10000 мм");
    next = {
      ...shape,
      constraints: {
        ...shape.constraints,
        [target.axis]: { ...constraint, offsetMm: value },
      },
    };
  }
  return resolveSketch({
    ...doc,
    shapes: doc.shapes.map((s) => (s.id === shape.id ? next : s)),
  });
}
