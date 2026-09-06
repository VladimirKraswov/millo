import type { CuttingTool } from "../../shared/tooling";
import type {
  SketchGeometry,
  SketchJobRequest,
  SketchOperation,
  SketchOperationKind,
  SketchPoint,
  SketchShape,
} from "../../shared/sketch";

export type DrawMode = "select" | "pan" | "rectangle" | "circle" | "polygon";
export const operationLabels: Record<SketchOperationKind, string> = {
  pocket: "Карман / отверстие",
  inside: "Вырез внутри",
  outside: "Деталь по внешнему краю",
  engrave: "Линия по контуру",
  drill: "Сверление",
};
export const emptySketch = (): SketchJobRequest => ({
  sourceName: "Мой чертёж.nc",
  shapes: [],
  stock: {
    widthMm: 200,
    heightMm: 140,
    thicknessMm: 3,
    safeZMm: 5,
    breakthroughMm: 0.1,
    spindleMode: "manual",
  },
});
export function compatibleTools(
  tools: readonly CuttingTool[],
  kind: SketchOperationKind,
) {
  return tools.filter((tool) =>
    kind === "drill"
      ? tool.kind === "drill"
      : kind === "engrave"
        ? ["flatEndMill", "engraving", "vBit"].includes(tool.kind)
        : tool.kind === "flatEndMill",
  );
}
export function preferredTool(tools: readonly CuttingTool[]) {
  const mills = compatibleTools(tools, "pocket");
  return (
    mills.find((t) => t.diameterMm >= 2 && t.diameterMm <= 3.2) ?? mills[0]
  );
}
export function toolSettings(
  tool?: CuttingTool,
): Pick<
  SketchOperation,
  | "toolId"
  | "stepdownMm"
  | "stepoverPercent"
  | "feedMmPerMin"
  | "plungeMmPerMin"
  | "spindleRpm"
> {
  return {
    toolId: tool?.id ?? "",
    stepdownMm: Math.min(tool?.stepdownMm ?? 0.5, 0.5),
    stepoverPercent: Math.min(tool?.stepoverPercent ?? 35, 50),
    feedMmPerMin: tool?.feedMmPerMin ?? 400,
    plungeMmPerMin: tool?.plungeMmPerMin ?? 100,
    spindleRpm: tool?.spindleRpm ?? 10_000,
  };
}
export function createShape(
  geometry: SketchGeometry,
  x: number,
  y: number,
  tool?: CuttingTool,
  index = 1,
): SketchShape {
  const circle = geometry.kind === "circle";
  return {
    id: crypto.randomUUID(),
    name: `${circle ? "Отверстие" : "Контур"} ${index}`,
    xMm: x,
    yMm: y,
    rotationDegrees: 0,
    geometry,
    operation: {
      kind: circle ? "pocket" : "outside",
      through: true,
      depthMm: 1,
      ...toolSettings(tool),
      tabs: { count: circle ? 0 : 4, widthMm: 3, heightMm: 0.6 },
    },
  };
}
export function changeOperation(
  shape: SketchShape,
  kind: SketchOperationKind,
  tools: readonly CuttingTool[],
): SketchShape {
  const available = compatibleTools(tools, kind);
  const tool =
    available.find((t) => t.id === shape.operation.toolId) ?? available[0];
  const through = kind === "outside" || kind === "inside" || kind === "drill";
  return {
    ...shape,
    operation: {
      ...shape.operation,
      ...toolSettings(tool),
      kind,
      through,
      tabs: {
        ...shape.operation.tabs,
        count: kind === "inside" || kind === "outside" ? 4 : 0,
      },
    },
  };
}
export function shapePoints(shape: SketchShape): SketchPoint[] {
  const g = shape.geometry;
  let local: SketchPoint[];
  if (g.kind === "polygon") local = [...g.points];
  else if (g.kind === "circle")
    local = Array.from({ length: 96 }, (_, i) => ({
      x: (g.diameter / 2) * Math.cos((i * Math.PI) / 48),
      y: (g.diameter / 2) * Math.sin((i * Math.PI) / 48),
    }));
  else if (g.radius <= 0)
    local = [
      { x: -g.width / 2, y: -g.height / 2 },
      { x: g.width / 2, y: -g.height / 2 },
      { x: g.width / 2, y: g.height / 2 },
      { x: -g.width / 2, y: g.height / 2 },
    ];
  else {
    const r = Math.min(g.radius, g.width / 2, g.height / 2);
    local = [
      [g.width / 2 - r, g.height / 2 - r],
      [-g.width / 2 + r, g.height / 2 - r],
      [-g.width / 2 + r, -g.height / 2 + r],
      [g.width / 2 - r, -g.height / 2 + r],
    ].flatMap(([cx, cy], corner) =>
      Array.from({ length: 17 }, (_, i) => {
        const a = ((corner + i / 16) * Math.PI) / 2;
        return { x: cx + r * Math.cos(a), y: cy + r * Math.sin(a) };
      }),
    );
  }
  const a = (shape.rotationDegrees * Math.PI) / 180;
  return local.map((p) => ({
    x: shape.xMm + p.x * Math.cos(a) - p.y * Math.sin(a),
    y: shape.yMm + p.x * Math.sin(a) + p.y * Math.cos(a),
  }));
}
export const svgPoints = (points: readonly SketchPoint[]) =>
  points.map((p) => `${p.x},${-p.y}`).join(" ");
export const snap = (value: number, step: number) =>
  step > 0 ? Math.round(value / step) * step : value;

export interface FanTemplate {
  readonly opening: number;
  readonly pitch: number;
  readonly hole: number;
  readonly plate: number;
}
export function fanShapes(
  options: FanTemplate,
  center: SketchPoint,
  tool?: CuttingTool,
): SketchShape[] {
  const circle = createShape(
    { kind: "circle", diameter: options.opening },
    center.x,
    center.y,
    tool,
  );
  const opening = {
    ...circle,
    name: "Воздуховод",
    operation: {
      ...circle.operation,
      kind: "inside" as const,
      tabs: { count: 4, widthMm: 3, heightMm: 0.6 },
    },
  };
  const holes = [-1, 1]
    .flatMap((x) =>
      [-1, 1].map((y) =>
        createShape(
          { kind: "circle", diameter: options.hole },
          center.x + (x * options.pitch) / 2,
          center.y + (y * options.pitch) / 2,
          tool,
        ),
      ),
    )
    .map((s, i) => ({ ...s, name: `Крепление ${i + 1}` }));
  const plate = createShape(
    {
      kind: "rectangle",
      width: options.plate,
      height: options.plate,
      radius: 4,
    },
    center.x,
    center.y,
    tool,
  );
  return [...holes, opening, { ...plate, name: "Панель" }];
}
export function grilleShapes(
  center: SketchPoint,
  tool?: CuttingTool,
): SketchShape[] {
  return Array.from({ length: 5 }, (_, i) => {
    const s = createShape(
      { kind: "rectangle", width: 60, height: 5, radius: 2.5 },
      center.x,
      center.y + (i - 2) * 10,
      tool,
      i + 1,
    );
    return {
      ...s,
      name: `Прорезь ${i + 1}`,
      operation: {
        ...s.operation,
        kind: "pocket" as const,
        tabs: { ...s.operation.tabs, count: 0 },
      },
    };
  });
}

export function validateSketch(
  document: SketchJobRequest,
  tools: readonly CuttingTool[],
): string | undefined {
  if (!document.shapes.length) return "Нет фигур";
  if (document.shapes.length > 200) return "Не больше 200 фигур";
  if (
    Object.values(document.stock).some(
      (v) => typeof v === "number" && !Number.isFinite(v),
    )
  )
    return "Заполните размеры заготовки";
  for (const s of document.shapes) {
    const tool = tools.find((t) => t.id === s.operation.toolId);
    if (!tool || !compatibleTools(tools, s.operation.kind).includes(tool))
      return `${s.name}: выберите подходящий инструмент`;
    if (
      [
        s.xMm,
        s.yMm,
        s.rotationDegrees,
        ...Object.values(s.operation),
        ...Object.values(s.geometry),
      ].some((v) => typeof v === "number" && !Number.isFinite(v))
    )
      return `${s.name}: заполните числовые поля`;
    if (
      s.operation.kind === "drill" &&
      (s.geometry.kind !== "circle" ||
        Math.abs(s.geometry.diameter - tool.diameterMm) > 0.01)
    )
      return `${s.name}: диаметр круга должен совпадать со сверлом`;
  }
  return undefined;
}
