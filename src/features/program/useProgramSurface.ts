import { useEffect, useMemo, useState } from "react";
import { bindSnapshotStream } from "../../platform/state/bindSnapshotStream";
import type { SurfaceSession } from "../../shared/heightmap";
import type {
  ProgramExecutionOptions,
  RunPreflightReport,
} from "../../shared/realRun";
import type { ProgramWorkspaceProps } from "./programWorkspaceTypes";
import { surfaceMapExecutionView } from "./surfaceMapExecutionModel";

import type { Dispatch, SetStateAction } from "react";
import type { GcodeProgram } from "../../shared/program";
interface ProgramSurfaceOptions {
  heightmapGateway: ProgramWorkspaceProps["heightmapGateway"];
  machineProfileId?: string;
  program?: GcodeProgram;
  programExecutionOptions: ProgramExecutionOptions;
  setProgramExecutionOptions: Dispatch<SetStateAction<ProgramExecutionOptions>>;
  setRealRunReport: Dispatch<SetStateAction<RunPreflightReport | undefined>>;
  setError: Dispatch<SetStateAction<string | undefined>>;
  senderActive: boolean;
}
export function useProgramSurface({
  heightmapGateway,
  machineProfileId,
  program,
  programExecutionOptions,
  setProgramExecutionOptions,
  setRealRunReport,
  setError,
  senderActive,
}: ProgramSurfaceOptions) {
  const [surfaceSession, setSurfaceSession] = useState<SurfaceSession>();
  const [surfaceMapBusy, setSurfaceMapBusy] = useState(false);
  useEffect(() => {
    if (!heightmapGateway) return;
    const accept = (session: SurfaceSession) => {
      setSurfaceSession(session);
      setProgramExecutionOptions((current) => {
        const usable =
          session.applicationEnabled &&
          session.coordinateBindingStale === false &&
          session.active?.machineProfileId === machineProfileId;
        const surfaceMapId = usable ? session.active?.mapId : undefined;
        return current.surfaceMapId === surfaceMapId
          ? current
          : { ...current, surfaceMapId };
      });
      setRealRunReport(undefined);
    };
    return bindSnapshotStream({
      stream: {
        readCurrent: () => heightmapGateway.getSession(),
        listen: (handler) => heightmapGateway.subscribeSession(handler),
      },
      onSnapshot: accept,
      onError: (reason) => setError(String(reason)),
    });
  }, [heightmapGateway, machineProfileId]);

  const surfaceMap = useMemo(
    () =>
      surfaceMapExecutionView(
        surfaceSession,
        machineProfileId,
        program?.summary.bounds,
      ),
    [machineProfileId, program?.summary.bounds, surfaceSession],
  );
  const setSurfaceMapApplication = async (
    enabled: boolean,
  ): Promise<ProgramExecutionOptions> => {
    const reject = (message: string): never => {
      setError(message);
      throw new Error(message);
    };
    if (!heightmapGateway || !surfaceMap) {
      return reject("Карта высот недоступна для текущего задания");
    }
    if (surfaceMapBusy || senderActive) {
      reject("Нельзя изменить карту высот во время другого действия");
    }
    if (enabled && !surfaceMap.coversProgram) {
      reject(
        "Карта высот не покрывает траекторию задания. Снимите карту по периметру файла.",
      );
    }
    if (enabled && !surfaceMap.usable) {
      reject(
        "Рабочий ноль изменился после измерения карты. Сначала снимите новую карту высот.",
      );
    }
    setSurfaceMapBusy(true);
    setError(undefined);
    setRealRunReport(undefined);
    try {
      const session = await heightmapGateway.setApplication(enabled, enabled);
      const surfaceMapId = enabled ? session.active?.mapId : undefined;
      if (enabled && surfaceMapId === undefined) {
        throw new Error("Контроллер не подтвердил активную карту высот");
      }
      const executionOptions = {
        ...programExecutionOptions,
        surfaceMapId,
      };
      setSurfaceSession(session);
      setProgramExecutionOptions(executionOptions);
      return executionOptions;
    } catch (reason) {
      setError(String(reason));
      throw reason;
    } finally {
      setSurfaceMapBusy(false);
    }
  };

  return {
    surfaceSession,
    surfaceMap,
    surfaceMapBusy,
    setSurfaceMapApplication,
  };
}
