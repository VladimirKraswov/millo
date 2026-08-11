import type { ProgramParseRequest } from "./program";

export type SenderState =
  | "idle"
  | "ready"
  | "running"
  | "paused"
  | "toolChange"
  | "draining"
  | "completed"
  | "failed"
  | "cancelled";

export type SenderMode = "mockDryRun" | "checkRun" | "airRun" | "cutRun";

export type SenderFailureKind =
  | "grblError"
  | "alarm"
  | "reset"
  | "timeout"
  | "disconnected"
  | "transport"
  | "unsafeState"
  | "internal";

export interface SenderFailure {
  readonly kind: SenderFailureKind;
  readonly message: string;
  readonly grblCode?: number;
  readonly sourceLine?: number;
  readonly command?: string;
}

export interface SenderSnapshot {
  readonly runSequence: number;
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
  readonly requestedTool?: number;
  readonly progressSequence?: number;
  readonly lastAcknowledgedSourceLine?: number;
  readonly lastAcknowledgedCommand?: string;
  readonly secondsSinceAcknowledgement?: number;
  readonly shutdownCommandsAcknowledged?: boolean;
  readonly lastError?: string;
  readonly failure?: SenderFailure;
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
  runSequence: 0,
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
