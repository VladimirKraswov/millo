import {
  cancelHeightmap,
  clearSurfaceSession,
  discardHeightmapDraft,
  getHeightmapSnapshot,
  getSurfaceSession,
  onHeightmapState,
  onSurfaceSession,
  pauseHeightmap,
  resumeHeightmap,
  resumeHeightmapDraft,
  setHeightmapApplication,
  startHeightmap,
} from "../../api/controller";
import type { HeightmapGateway } from "./HeightmapGateway";

export const tauriHeightmapGateway: HeightmapGateway = {
  cancel: cancelHeightmap,
  clear: clearSurfaceSession,
  discardDraft: discardHeightmapDraft,
  getOperation: getHeightmapSnapshot,
  getSession: getSurfaceSession,
  pause: pauseHeightmap,
  resume: resumeHeightmap,
  resumeDraft: resumeHeightmapDraft,
  setApplication: setHeightmapApplication,
  start: startHeightmap,
  subscribeOperation: onHeightmapState,
  subscribeSession: onSurfaceSession,
};
