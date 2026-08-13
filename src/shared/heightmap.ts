export type HeightmapContactMode = "directSurface" | "fixedPlate";

export interface HeightmapPlanRequest {
  originXMm: number;
  originYMm: number;
  widthMm: number;
  heightMm: number;
  columns: number;
  rows: number;
  clearanceZMm: number;
  maxProbeDepthMm: number;
  probeFeedMmPerMin: number;
  travelFeedMmPerMin: number;
  retractFeedMmPerMin: number;
  contactMode: HeightmapContactMode;
  contactOffsetMm: number;
}

export interface HeightmapSpacing {
  xMm: number;
  yMm: number;
}

export interface ProbePoint {
  sequence: number;
  row: number;
  column: number;
  xMm: number;
  yMm: number;
}

export interface HeightmapPlan {
  schemaVersion: number;
  request: HeightmapPlanRequest;
  spacing: HeightmapSpacing;
  points: ProbePoint[];
}

export interface ProbeSample {
  point: ProbePoint;
  zMm: number;
  triggered: boolean;
}

export interface Heightmap {
  schemaVersion: number;
  plan: HeightmapPlan;
  samples: Array<ProbeSample | null>;
}

export interface HeightmapProgress {
  measured: number;
  triggered: number;
  total: number;
  complete: boolean;
}

export type HeightmapOperationState =
  | "idle"
  | "running"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export interface HeightmapOperationSnapshot {
  operationSequence: number;
  state: HeightmapOperationState;
  map?: Heightmap;
  currentSequence?: number;
  progress: HeightmapProgress;
  error?: string;
}

export interface HeightmapStartRequest {
  plan: HeightmapPlanRequest;
  setupConfirmed: boolean;
  contactAvailableAtEveryPoint: boolean;
}

export interface StoredSurfaceMap {
  mapId: number;
  machineProfileId: string;
  createdAtUnixMs: number;
  map: Heightmap;
}

export interface PendingSurfaceMap {
  machineProfileId: string;
  updatedAtUnixMs: number;
  operation: HeightmapOperationSnapshot;
}

export interface SurfaceSession {
  schemaVersion: number;
  revision: number;
  nextMapId: number;
  active?: StoredSurfaceMap;
  pending?: PendingSurfaceMap;
  applicationEnabled: boolean;
  requiresSetupConfirmation: boolean;
}
