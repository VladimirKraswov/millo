import type { ControllerSnapshot, WorkCoordinateSystem } from "../../shared/machine";

export interface ProbeEstablishedZDatum {
  readonly profileId?: string;
  readonly coordinateSystem: WorkCoordinateSystem;
  readonly resetCount: number;
  readonly reconnectCount: number;
  readonly source: "probe" | "heightmap";
  readonly workCoordinateOffsetZ?: number;
}

export function isProbeDatumCurrent(
  datum: ProbeEstablishedZDatum | undefined,
  snapshot: ControllerSnapshot,
  coordinateSystem: string,
  profileId?: string,
): datum is ProbeEstablishedZDatum {
  if (!datum || snapshot.connection !== "connected" ||
    datum.profileId !== profileId ||
    datum.resetCount !== snapshot.resetCount ||
    datum.reconnectCount !== snapshot.reconnectCount ||
    datum.coordinateSystem !== coordinateSystem.toLowerCase()) return false;
  const offset = snapshot.machine.workCoordinateOffset?.z;
  return datum.workCoordinateOffsetZ === undefined || offset === undefined ||
    (Number.isFinite(offset) && Number.isFinite(datum.workCoordinateOffsetZ) &&
      Math.abs(datum.workCoordinateOffsetZ - offset) <= 0.01);
}
