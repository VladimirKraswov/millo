import type { Position } from "./machine";
import type { SenderState } from "./dryRun";
import type { ProgramParseRequest } from "./program";
import type { ProgramExecutionOptions, ProgramRunIntent } from "./realRun";

export interface ProgramRecoveryCandidate {
  readonly id: number;
  readonly sourceName: string;
  readonly intent: ProgramRunIntent;
  readonly state: SenderState;
  readonly updatedAtUnixMs: number;
  readonly totalLines: number;
  readonly acknowledgedLines: number;
  readonly executingSourceLine?: number;
  readonly restartSourceLine?: number;
  readonly restartPosition?: Position;
  readonly minimumSafeZMm?: number;
  readonly ready: boolean;
  readonly detail: string;
}

export interface ProgramRecoveryPreparationRequest {
  readonly recoveryId: number;
  readonly safeZMm: number;
  readonly machineReferenceRestored: boolean;
  readonly workZeroRestored: boolean;
  readonly restartPointInspected: boolean;
  readonly pathClear: boolean;
  readonly powerControlReachable: boolean;
}

export interface ProgramRecoveryPackage {
  readonly recoveryId: number;
  readonly originalSourceName: string;
  readonly interruptedSourceLine: number;
  readonly restartSourceLine: number;
  readonly restartPosition: Position;
  readonly clearanceZMm: number;
  readonly repeatedSourceLines: number;
  readonly intent: ProgramRunIntent;
  readonly executionOptions: ProgramExecutionOptions;
  readonly request: ProgramParseRequest;
}
