import {
  clearSurfaceSession,
  getHeightmapSnapshot,
  getSurfaceSession,
  onHeightmapState,
  onSurfaceSession,
  pauseHeightmap,
  resumeHeightmap,
  setHeightmapApplication,
  startHeightmap,
} from "../../api/controller";
import type { HeightmapGateway } from "./HeightmapGateway";

export const tauriHeightmapGateway: HeightmapGateway = {
  clear: clearSurfaceSession,
  getOperation: getHeightmapSnapshot,
  getSession: getSurfaceSession,
  pause: pauseHeightmap,
  resume: resumeHeightmap,
  setApplication: setHeightmapApplication,
  start: startHeightmap,
  subscribeOperation: onHeightmapState,
  subscribeSession: onSurfaceSession,
};
