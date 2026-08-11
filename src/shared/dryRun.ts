import type { ProgramParseRequest } from "./program";

export type SenderState =
  | "idle"
  | "ready"
  | "running"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export interface SenderSnapshot {
  readonly state: SenderState;
  readonly sourceName?: string;
  readonly totalLines: number;
  readonly acknowledgedLines: number;
  readonly currentSourceLine?: number;
  readonly currentCommand?: string;
  readonly lastError?: string;
  readonly progress: number;
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
  acknowledgedLines: 0,
  progress: 0,
};

