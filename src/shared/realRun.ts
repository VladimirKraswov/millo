import type { HardwareInspection } from "./machine";
import type { ProgramBounds, ProgramParseRequest } from "./program";

export type ProgramRunIntent = "airRun" | "cutting";

export interface ProgramExecutionOptions {
  readonly optionalStop: boolean;
  readonly blockDelete: boolean;
}

export const defaultProgramExecutionOptions: ProgramExecutionOptions = {
  optionalStop: false,
  blockDelete: false,
};

export type RunPreflightLevel = "pass" | "caution" | "blocker";

export interface RunPreflightCheck {
  readonly id: string;
  readonly level: RunPreflightLevel;
  readonly title: string;
  readonly detail: string;
  readonly sourceLine?: number;
}
export interface RunProgramBlocker {
  readonly kind: string;
  readonly message: string;
  readonly sourceLine?: number;
}

export interface RunPreflightReport {
  readonly sourceName: string;
  readonly programFingerprint: string;
  readonly intent: ProgramRunIntent;
  readonly executionOptions: ProgramExecutionOptions;
  readonly ready: boolean;
  readonly blockerCount: number;
  readonly cautionCount: number;
  readonly pollSequence: number;
  readonly bounds?: ProgramBounds;
  readonly hardware: HardwareInspection;
  readonly checks: readonly RunPreflightCheck[];
  readonly programBlockers: readonly RunProgramBlocker[];
  readonly totalProgramBlockers: number;
}

export interface FirstCutConfirmation {
  readonly intent: ProgramRunIntent;
  readonly executionOptions: ProgramExecutionOptions;
  readonly stockSecured: boolean;
  readonly toolSecured: boolean;
  readonly toolRemoved: boolean;
  readonly xyzZeroVerified: boolean;
  readonly safeZVerified: boolean;
  readonly manualSpindleRunning: boolean;
  readonly manualSpindleOff: boolean;
  readonly pathClear: boolean;
  readonly powerControlReachable: boolean;
}

export interface FirstCutAuthorization {
  readonly id: number;
  readonly expiresInMs: number;
  readonly sourceName: string;
  readonly programFingerprint: string;
  readonly pollSequence: number;
  readonly intent: ProgramRunIntent;
  readonly executionOptions: ProgramExecutionOptions;
}

export interface FirstCutPreparation {
  readonly report: RunPreflightReport;
  readonly authorization: FirstCutAuthorization;
}

export interface ToolChangeConfirmation {
  readonly sourceLine: number;
  readonly requestedTool?: number;
  readonly toolSecured: boolean;
  readonly zZeroVerified: boolean;
  readonly safeZVerified: boolean;
  readonly pathClear: boolean;
  readonly manualSpindleRunning: boolean;
  readonly powerControlReachable: boolean;
}

export interface RealRunPreflightGateway {
  preflight(
    request: ProgramParseRequest,
    intent: ProgramRunIntent,
    executionOptions: ProgramExecutionOptions,
  ): Promise<RunPreflightReport>;
  authorizeFirstCut(
    request: ProgramParseRequest,
    confirmation: FirstCutConfirmation,
  ): Promise<FirstCutPreparation>;
  startProgram(
    request: ProgramParseRequest,
    authorizationId: number,
    executionOptions: ProgramExecutionOptions,
  ): Promise<import("./dryRun").SenderSnapshot>;
  startCheck(
    request: ProgramParseRequest,
    executionOptions: ProgramExecutionOptions,
  ): Promise<import("./dryRun").SenderSnapshot>;
  resumeProgram(): Promise<import("./dryRun").SenderSnapshot>;
  completeToolChange(
    confirmation: ToolChangeConfirmation,
  ): Promise<import("./dryRun").SenderSnapshot>;
}
