import { useCallback, useEffect, useState } from "react";
import type { ControllerSnapshot, WorkCoordinateSystem, ZProbeOutcome } from "../../shared/machine";
import type { HeightmapGateway } from "../../platform/machine/HeightmapGateway";
import { bindSnapshotStream } from "../../platform/state/bindSnapshotStream";
import { reportUiError } from "../../api/audit";
import { heightmapHasCurrentZDatum } from "../heightmap/heightmapModel";
import { isProbeDatumCurrent, type ProbeEstablishedZDatum } from "./probeDatumModel";

export function useProbeDatum({ snapshot, coordinateSystem, profileId, gateway }: {
  snapshot: ControllerSnapshot;
  coordinateSystem: string;
  profileId?: string;
  gateway?: Pick<HeightmapGateway, "getSession" | "subscribeSession">;
}) {
  const [storedDatum, setStoredDatum] = useState<ProbeEstablishedZDatum>();
  const datum = isProbeDatumCurrent(storedDatum, snapshot, coordinateSystem, profileId)
    ? storedDatum : undefined;
  const { connection, resetCount, reconnectCount } = snapshot;
  const { x, y, z } = snapshot.machine.workCoordinateOffset ?? {};

  useEffect(() => {
    if (storedDatum && !datum) setStoredDatum(undefined);
  }, [storedDatum, datum]);

  useEffect(() => {
    if (!gateway || datum || connection !== "connected" || !profileId ||
      x === undefined || y === undefined || z === undefined) return;
    const offset = { x, y, z };
    const wcs = coordinateSystem.toLowerCase() as WorkCoordinateSystem;
    return bindSnapshotStream({
      stream: { readCurrent: () => gateway.getSession(), listen: (listener) => gateway.subscribeSession(listener) },
      onSnapshot: (session) => {
        const stored = session.active;
        if (stored?.machineProfileId !== profileId || !heightmapHasCurrentZDatum(
          stored.map, session.coordinateBindingStale, wcs, offset,
        )) return;
        setStoredDatum({
          profileId, coordinateSystem: wcs, resetCount, reconnectCount,
          source: "heightmap", workCoordinateOffsetZ: z,
        });
      },
      onError: (error) => reportUiError("Привязка Z", error, ""),
    });
    // Primitive dependencies keep equal telemetry packets from cancelling the read.
  }, [gateway, datum, connection, profileId, resetCount, reconnectCount, coordinateSystem, x, y, z]);

  const remember = useCallback((outcome: ZProbeOutcome, source: ProbeEstablishedZDatum["source"]) => {
    setStoredDatum({
      profileId,
      coordinateSystem: outcome.coordinateSystem,
      resetCount: outcome.snapshot.resetCount,
      reconnectCount: outcome.snapshot.reconnectCount,
      source,
      workCoordinateOffsetZ: outcome.snapshot.machine.workCoordinateOffset?.z,
    });
  }, [profileId]);

  return { datum, remember };
}
