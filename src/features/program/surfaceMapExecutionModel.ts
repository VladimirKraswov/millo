import type { ProgramBounds } from "../../shared/program";
import type { StoredSurfaceMap, SurfaceSession } from "../../shared/heightmap";

export interface SurfaceMapExecutionView {
  readonly map: StoredSurfaceMap;
  readonly enabled: boolean;
  readonly usable: boolean;
  readonly coversProgram: boolean;
  readonly zRangeMm: number;
  readonly detail: string;
}

const within = (value: number, minimum: number, maximum: number): boolean =>
  value >= minimum - 1e-6 && value <= maximum + 1e-6;

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
  const usable = session.coordinateBindingStale === false;
  const detail = !usable
    ? `Карта #${map.mapId} снята до изменения рабочего нуля · снимите новую карту`
    : coversProgram
      ? `Карта #${map.mapId} · ${grid} · ${dimensions} · перепад ${zRange.toFixed(3)} mm`
      : `Карта #${map.mapId} не покрывает всю траекторию задания`;

  return {
    map,
    enabled: session?.applicationEnabled === true,
    usable,
    coversProgram,
    zRangeMm: zRange,
    detail,
  };
}
