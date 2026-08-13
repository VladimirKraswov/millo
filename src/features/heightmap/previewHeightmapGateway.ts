import type { HeightmapGateway } from "../../platform/machine/HeightmapGateway";
import type { HeightmapOperationSnapshot, SurfaceSession } from "../../shared/heightmap";
import { defaultHeightmapRequest } from "./heightmapDefaults";
import { buildHeightmapPlan } from "./heightmapModel";

const request = {
  ...defaultHeightmapRequest(),
  originXMm: -16,
  originYMm: -7,
  widthMm: 32,
  heightMm: 14,
  columns: 7,
  rows: 4,
};
const plan = buildHeightmapPlan(request);
const map = {
  schemaVersion: 1,
  plan,
  samples: plan.points.map((point) => ({
    point,
    zMm: -0.18 + point.xMm * 0.006 + point.yMm * 0.011 + Math.sin(point.xMm * 0.28) * 0.025,
    triggered: true,
  })),
};
const operation: HeightmapOperationSnapshot = {
  operationSequence: 3,
  state: "completed",
  map,
  progress: { measured: plan.points.length, triggered: plan.points.length, total: plan.points.length, complete: true },
};
const session: SurfaceSession = {
  schemaVersion: 1,
  revision: 4,
  nextMapId: 2,
  active: { mapId: 1, machineProfileId: "machine-0001", createdAtUnixMs: Date.now(), map },
  applicationEnabled: false,
  requiresSetupConfirmation: false,
};

export const previewHeightmapGateway: HeightmapGateway = {
  cancel: async () => ({
    ...operation,
    state: "cancelled",
    error: "Stopped by operator",
  }),
  clear: async () => ({ ...session, active: undefined }),
  getOperation: async () => operation,
  getSession: async () => session,
  pause: async () => operation,
  resume: async () => operation,
  setApplication: async (enabled) => ({ ...session, applicationEnabled: enabled }),
  start: async () => operation,
  subscribeOperation: async () => () => undefined,
  subscribeSession: async () => () => undefined,
};
