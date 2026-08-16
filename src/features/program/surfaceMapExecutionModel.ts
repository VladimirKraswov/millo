import type { ProgramBounds } from "../../shared/program";
import type { StoredSurfaceMap, SurfaceSession } from "../../shared/heightmap";

export interface SurfaceMapExecutionView {
  readonly map: StoredSurfaceMap;
  readonly enabled: boolean;
  readonly usable: boolean;
  readonly coversProgram: boolean;
  readonly zRangeMm: number;
  readonly maximumNeighborDeltaMm: number;
  readonly medianNeighborDeltaMm: number;
  readonly suspiciousNeighborJump: boolean;
  readonly detail: string;
}

const within = (value: number, minimum: number, maximum: number): boolean =>
  value >= minimum - 1e-6 && value <= maximum + 1e-6;

interface SurfaceQuality {
  readonly maximumNeighborDeltaMm: number;
  readonly medianNeighborDeltaMm: number;
  readonly suspiciousNeighborJump: boolean;
}

const surfaceQuality = (map: StoredSurfaceMap): SurfaceQuality => {
  const columns = map.map.plan.request.columns;
  const rows = map.map.plan.request.rows;
  const byCell = new Map<string, number>();
  map.map.samples.forEach((sample) => {
    if (sample?.triggered) byCell.set(`${sample.point.row}:${sample.point.column}`, sample.zMm);
  });
  const deltas: number[] = [];
  for (let row = 0; row < rows; row += 1) {
    for (let column = 0; column < columns; column += 1) {
      const current = byCell.get(`${row}:${column}`);
      if (current === undefined) continue;
      const right = byCell.get(`${row}:${column + 1}`);
      const below = byCell.get(`${row + 1}:${column}`);
      if (right !== undefined) deltas.push(Math.abs(current - right));
      if (below !== undefined) deltas.push(Math.abs(current - below));
    }
  }
  deltas.sort((left, right) => left - right);
  const middle = Math.floor(deltas.length / 2);
  const medianNeighborDeltaMm = deltas.length === 0
    ? 0
    : deltas.length % 2 === 1
      ? deltas[middle]
      : (deltas[middle - 1] + deltas[middle]) / 2;
  const maximumNeighborDeltaMm = deltas.at(-1) ?? 0;
  return {
    maximumNeighborDeltaMm,
    medianNeighborDeltaMm,
    suspiciousNeighborJump: maximumNeighborDeltaMm >= 0.5 &&
      maximumNeighborDeltaMm >= Math.max(0.01, medianNeighborDeltaMm) * 8,
  };
};

export function surfaceMapExecutionView(
  session: SurfaceSession | undefined,
  machineProfileId: string | undefined,
  bounds: ProgramBounds | undefined,
): SurfaceMapExecutionView | undefined {
  const map = session?.active;
  if (!map || !machineProfileId || map.machineProfileId !== machineProfileId) return undefined;

  const area = map.map.plan.request;
  const coversProgram = !bounds || (
    within(bounds.min.x, area.originXMm, area.originXMm + area.widthMm) &&
    within(bounds.max.x, area.originXMm, area.originXMm + area.widthMm) &&
    within(bounds.min.y, area.originYMm, area.originYMm + area.heightMm) &&
    within(bounds.max.y, area.originYMm, area.originYMm + area.heightMm)
  );
  const measured = map.map.samples.filter(
    (sample) => sample?.triggered,
  );
  const zValues = measured.map((sample) => sample!.zMm);
  const zRange = zValues.length > 0
    ? Math.max(...zValues) - Math.min(...zValues)
    : 0;
  const grid = `${map.map.plan.request.columns}×${map.map.plan.request.rows}`;
  const dimensions = `${area.widthMm.toFixed(1)}×${area.heightMm.toFixed(1)} mm`;
  const quality = surfaceQuality(map);
  const usable = session.coordinateBindingStale === false;
  const detail = !usable
    ? `Карта #${map.mapId} снята до изменения рабочего нуля · снимите новую карту`
    : coversProgram
      ? `Карта #${map.mapId} · ${grid} · ${dimensions} · перепад ${zRange.toFixed(3)} mm${
        quality.suspiciousNeighborJump
          ? ` · резкий скачок ${quality.maximumNeighborDeltaMm.toFixed(3)} mm`
          : ""
      }`
      : `Карта #${map.mapId} не покрывает всю траекторию задания`;

  return {
    map,
    enabled: session?.applicationEnabled === true,
    usable,
    coversProgram,
    zRangeMm: zRange,
    ...quality,
    detail,
  };
}
