import type { ProgramParseRequest } from "./program";

export type SenderState =
  | "idle"
  | "ready"
  | "running"
  | "paused"
  | "draining"
  | "completed"
  | "failed"
  | "cancelled";

export type SenderMode = "mockDryRun" | "checkRun" | "airRun" | "cutRun";

export interface SenderSnapshot {
  readonly state: SenderState;
  readonly mode?: SenderMode;
  readonly sourceName?: string;
  readonly totalLines: number;
  readonly dispatchedLines: number;
  readonly acknowledgedLines: number;
  readonly inFlightLines: number;
  readonly rxBufferBytes: number;
  readonly rxBufferCapacity: number;
  readonly currentSourceLine?: number;
  readonly currentCommand?: string;
  readonly lastError?: string;
  readonly progress: number;
  readonly elapsedSeconds: number;
  readonly estimatedCompletedSeconds: number;
  readonly estimatedRemainingSeconds: number;
  readonly estimatedTotalSeconds: number;
  readonly timeEstimateComplete: boolean;
}

export interface DryRunGateway {
  snapshot(): Promise<SenderSnapshot>;
  start(request: ProgramParseRequest): Promise<SenderSnapshot>;
  pause(): Promise<SenderSnapshot>;
  resume(): Promise<SenderSnapshot>;
  cancel(): Promise<SenderSnapshot>;
  subscribe(listener: (snapshot: SenderSnapshot) => void): Promise<() => void>;
}

export const idleSenderSnapshot: SenderSnapshot = {
  state: "idle",
  totalLines: 0,
  dispatchedLines: 0,
  acknowledgedLines: 0,
  inFlightLines: 0,
  rxBufferBytes: 0,
  rxBufferCapacity: 127,
  progress: 0,
  elapsedSeconds: 0,
  estimatedCompletedSeconds: 0,
  estimatedRemainingSeconds: 0,
  estimatedTotalSeconds: 0,
  timeEstimateComplete: false,
};
