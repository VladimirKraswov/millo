import type { HeightmapOperationSnapshot, HeightmapPlanRequest, SurfaceSession } from "../../shared/heightmap";

export const defaultHeightmapRequest = (): HeightmapPlanRequest => ({
  originXMm: 0,
  originYMm: 0,
  widthMm: 50,
  heightMm: 50,
  columns: 6,
  rows: 6,
  clearanceZMm: 2,
  maxProbeDepthMm: 3,
  probeFeedMmPerMin: 25,
  travelFeedMmPerMin: 300,
  retractFeedMmPerMin: 100,
  contactMode: "directSurface",
  contactOffsetMm: 0,
});

export const emptyHeightmapOperation: HeightmapOperationSnapshot = {
  operationSequence: 0,
  state: "idle",
  progress: { measured: 0, triggered: 0, total: 0, complete: false },
};

export const emptySurfaceSession: SurfaceSession = {
  schemaVersion: 1,
  revision: 0,
  nextMapId: 1,
  applicationEnabled: false,
  requiresSetupConfirmation: false,
  coordinateBindingStale: false,
};
