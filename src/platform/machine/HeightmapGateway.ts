import type {
  HeightmapOperationSnapshot,
  HeightmapResumeRequest,
  HeightmapStartRequest,
  SurfaceSession,
} from "../../shared/heightmap";

export interface HeightmapGateway {
  cancel(): Promise<HeightmapOperationSnapshot>;
  clear(): Promise<SurfaceSession>;
  discardDraft(): Promise<SurfaceSession>;
  getOperation(): Promise<HeightmapOperationSnapshot>;
  getSession(): Promise<SurfaceSession>;
  pause(): Promise<HeightmapOperationSnapshot>;
  resume(): Promise<HeightmapOperationSnapshot>;
  resumeDraft(request: HeightmapResumeRequest, machineProfileId: string): Promise<HeightmapOperationSnapshot>;
  setApplication(enabled: boolean, setupConfirmed: boolean): Promise<SurfaceSession>;
  start(request: HeightmapStartRequest, machineProfileId: string): Promise<HeightmapOperationSnapshot>;
  subscribeOperation(handler: (snapshot: HeightmapOperationSnapshot) => void): Promise<() => void>;
  subscribeSession(handler: (session: SurfaceSession) => void): Promise<() => void>;
}
