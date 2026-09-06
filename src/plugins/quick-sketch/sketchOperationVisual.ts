import type {
  SketchPoint,
  SketchShape,
  SketchStock,
} from "../../shared/sketch";
import {
  effectiveCuttingDiameterMm,
  type CuttingTool,
} from "../../shared/tooling";
import { compatibleTools, shapePoints } from "./sketchModel";

export interface SketchCutterVisual {
  readonly center: SketchPoint;
  readonly contact?: SketchPoint;
  readonly diameterMm?: number;
  readonly warning?: string;
}
const distance = (a: SketchPoint, b: SketchPoint) =>
  Math.hypot(a.x - b.x, a.y - b.y);
function edgeDistance(p: SketchPoint, a: SketchPoint, b: SketchPoint) {
  const dx = b.x - a.x,
    dy = b.y - a.y,
    length2 = dx * dx + dy * dy;
  const t = length2
    ? Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / length2))
    : 0;
  return distance(p, { x: a.x + t * dx, y: a.y + t * dy });
}
function inside(point: SketchPoint, points: readonly SketchPoint[]) {
  let result = false;
  for (let i = 0, j = points.length - 1; i < points.length; j = i++) {
    const a = points[i],
      b = points[j];
    if (
      a.y > point.y !== b.y > point.y &&
      point.x < ((b.x - a.x) * (point.y - a.y)) / (b.y - a.y) + a.x
    )
      result = !result;
  }
  return result;
}

/** A local tangency glyph, never a CAM offset or a collision/clearance guarantee.
 * Actual paths still come exclusively from the Rust/Clipper planner. */
export function sketchCutterVisual(
  shape: SketchShape,
  stock: SketchStock,
  tool?: CuttingTool,
): SketchCutterVisual {
  const origin = { x: shape.xMm, y: shape.yMm };
  if (!tool || !compatibleTools([tool], shape.operation.kind).length)
    return { center: origin, warning: "Выберите подходящий инструмент" };
  const depth = shape.operation.through
    ? stock.thicknessMm + stock.breakthroughMm
    : shape.operation.depthMm;
  const diameterMm = effectiveCuttingDiameterMm(tool, depth);
  if (
    diameterMm === undefined ||
    !Number.isFinite(diameterMm) ||
    diameterMm <= 0
  )
    return { center: origin, warning: "Неизвестен рабочий диаметр фрезы" };
  const r = diameterMm / 2,
    kind = shape.operation.kind;
  if (kind === "drill")
    return {
      center: origin,
      diameterMm,
      warning:
        shape.geometry.kind !== "circle" ||
        Math.abs(shape.geometry.diameter - tool.diameterMm) > 0.01
          ? "Диаметр отверстия должен совпадать со сверлом"
          : undefined,
    };
  const side = kind === "outside" ? 1 : kind === "engrave" ? 0 : -1;
  if (shape.geometry.kind === "circle") {
    const radius = shape.geometry.diameter / 2;
    const a = (shape.rotationDegrees * Math.PI) / 180;
    const point = (offset: number) => ({
      x: origin.x + offset * Math.cos(a),
      y: origin.y + offset * Math.sin(a),
    });
    if (side < 0 && r > radius + 1e-6)
      return { center: origin, diameterMm, warning: "Фреза шире отверстия" };
    return {
      center: point(radius + side * r),
      contact: point(radius),
      diameterMm,
    };
  }
  const points = shapePoints(shape);
  const edges = points.map((a, i) => ({
    a,
    b: points[(i + 1) % points.length],
  }));
  const signedArea = edges.reduce(
    (sum, { a, b }) => sum + a.x * b.y - b.x * a.y,
    0,
  );
  const winding = signedArea >= 0 ? 1 : -1;
  // Prefer a long straight edge; bound work for imported, highly concave outlines.
  const candidates = [...edges]
    .sort((a, b) => distance(b.a, b.b) - distance(a.a, a.b))
    .slice(0, 32);
  for (const { a, b } of candidates) {
    const length = distance(a, b);
    if (length < 1e-9) continue;
    const contact = { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
    const center = {
      x: contact.x + (side * r * winding * (b.y - a.y)) / length,
      y: contact.y - (side * r * winding * (b.x - a.x)) / length,
    };
    if (
      side === 0 ||
      (inside(center, points) === side < 0 &&
        edges.every((e) => edgeDistance(center, e.a, e.b) >= r - 0.002))
    )
      return { center, contact, diameterMm };
  }
  return {
    center: origin,
    diameterMm,
    warning:
      "Положение фрезы не показано: проверьте контур расчётом траектории",
  };
}
