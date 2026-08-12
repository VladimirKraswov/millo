import type {
  Heightmap,
  HeightmapPlan,
  HeightmapPlanRequest,
  ProbePoint,
} from "../../shared/heightmap";
import type { ProgramBounds } from "../../shared/program";

export type HeightmapDensity = "sparse" | "normal" | "precise" | "custom";

const densitySpacing: Record<Exclude<HeightmapDensity, "custom">, number> = {
  sparse: 20,
  normal: 10,
  precise: 5,
};

const axisPoints = (span: number, spacing: number): number =>
  Math.max(2, Math.min(101, Math.ceil(span / spacing) + 1));

export const applyDensity = (
  request: HeightmapPlanRequest,
  density: Exclude<HeightmapDensity, "custom">,
): HeightmapPlanRequest => ({
  ...request,
  columns: axisPoints(request.widthMm, densitySpacing[density]),
  rows: axisPoints(request.heightMm, densitySpacing[density]),
});

export const perimeterFromProgram = (
  request: HeightmapPlanRequest,
  bounds: ProgramBounds,
  marginMm: number,
): HeightmapPlanRequest => {
  const margin = Number.isFinite(marginMm) ? Math.max(0, marginMm) : 0;
  return {
    ...request,
    originXmm: bounds.min.x - margin,
    originYmm: bounds.min.y - margin,
    widthMm: Math.max(0.01, bounds.size.x + margin * 2),
    heightMm: Math.max(0.01, bounds.size.y + margin * 2),
  };
};

export const buildHeightmapPlan = (request: HeightmapPlanRequest): HeightmapPlan => {
  const spacing = {
    xMm: request.widthMm / Math.max(1, request.columns - 1),
    yMm: request.heightMm / Math.max(1, request.rows - 1),
  };
  const points: ProbePoint[] = [];
  for (let row = 0; row < request.rows; row += 1) {
    const columns = Array.from({ length: request.columns }, (_, index) => index);
    if (row % 2 === 1) columns.reverse();
    for (const column of columns) {
      points.push({
        sequence: points.length,
        row,
        column,
        xMm: request.originXmm + spacing.xMm * column,
        yMm: request.originYmm + spacing.yMm * row,
      });
    }
  }
  return { schemaVersion: 1, request, spacing, points };
};

export const estimateHeightmapSeconds = (request: HeightmapPlanRequest): number => {
  const plan = buildHeightmapPlan(request);
  const xyDistance = plan.points.slice(1).reduce((total, point, index) => {
    const previous = plan.points[index];
    return total + Math.hypot(point.xMm - previous.xMm, point.yMm - previous.yMm);
  }, 0);
  const probes = plan.points.length;
  return xyDistance / request.travelFeedMmPerMin * 60 + probes * (
    request.maxProbeDepthMm / request.probeFeedMmPerMin * 60 +
    (request.clearanceZmm + request.maxProbeDepthMm) /
      request.retractFeedMmPerMin * 60
  );
};

export const validateHeightmapRequest = (
  request: HeightmapPlanRequest,
  travel?: { readonly x: number; readonly y: number; readonly z: number },
): string | undefined => {
  const values = Object.values(request).filter((value): value is number => typeof value === "number");
  if (values.some((value) => !Number.isFinite(value))) return "Все размеры должны быть числами";
  if (request.widthMm <= 0 || request.heightMm <= 0) return "Периметр должен иметь положительный размер";
  if (request.columns < 2 || request.rows < 2) return "Нужно не менее двух точек по каждой оси";
  if (!Number.isInteger(request.columns) || !Number.isInteger(request.rows)) return "Количество точек должно быть целым";
  if (request.columns > 101 || request.rows > 101 || request.columns * request.rows > 10_000) return "Сетка слишком плотная: максимум 10 000 точек";
  if (travel && (request.widthMm > travel.x || request.heightMm > travel.y)) return "Периметр больше рабочего поля выбранного станка";
  if (travel && request.clearanceZmm > travel.z) return "Безопасная Z больше хода станка";
  if (request.clearanceZmm <= 0 || request.maxProbeDepthMm <= 0) return "Безопасная Z и глубина поиска должны быть положительными";
  if (request.contactMode === "directSurface" && request.contactOffsetMm !== 0) return "Для прямого контакта смещение равно 0";
  if (request.contactMode === "fixedPlate" && (request.contactOffsetMm < 0.01 || request.contactOffsetMm > 100)) return "Толщина сплошной пластины должна быть от 0.01 до 100 mm";
  return undefined;
};

export const heightmapMatrix = (map: Heightmap): Array<Array<number | undefined>> => {
  const matrix = Array.from({ length: map.plan.request.rows }, () =>
    Array<number | undefined>(map.plan.request.columns).fill(undefined),
  );
  for (const sample of map.samples) {
    if (sample?.triggered) matrix[sample.point.row][sample.point.column] = sample.zMm;
  }
  return matrix;
};

export const heightColor = (z: number, minimum: number, maximum: number): string => {
  const range = Math.max(maximum - minimum, 1e-9);
  const ratio = Math.max(0, Math.min(1, (z - minimum) / range));
  const hue = 205 - ratio * 170;
  return `hsl(${hue.toFixed(0)} 78% 58%)`;
};
