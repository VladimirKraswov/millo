import type {
  HeightmapOperationSnapshot,
  HeightmapStartRequest,
  SurfaceSession,
} from "../../shared/heightmap";

export interface HeightmapGateway {
  clear(): Promise<SurfaceSession>;
  getOperation(): Promise<HeightmapOperationSnapshot>;
  getSession(): Promise<SurfaceSession>;
  pause(): Promise<HeightmapOperationSnapshot>;
  resume(): Promise<HeightmapOperationSnapshot>;
  setApplication(enabled: boolean, setupConfirmed: boolean): Promise<SurfaceSession>;
  start(request: HeightmapStartRequest, machineProfileId: string): Promise<HeightmapOperationSnapshot>;
  subscribeOperation(handler: (snapshot: HeightmapOperationSnapshot) => void): Promise<() => void>;
  subscribeSession(handler: (session: SurfaceSession) => void): Promise<() => void>;
}
