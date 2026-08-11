import type { HardwareInspection } from "./machine";
import type { ProgramBounds, ProgramParseRequest } from "./program";

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
  readonly stockSecured: boolean;
  readonly toolSecured: boolean;
  readonly xyzZeroVerified: boolean;
  readonly safeZVerified: boolean;
  readonly manualSpindleRunning: boolean;
  readonly powerControlReachable: boolean;
}

export interface FirstCutAuthorization {
  readonly id: number;
  readonly expiresInMs: number;
  readonly sourceName: string;
  readonly programFingerprint: string;
  readonly pollSequence: number;
}

export interface FirstCutPreparation {
  readonly report: RunPreflightReport;
  readonly authorization: FirstCutAuthorization;
}

export interface RealRunPreflightGateway {
  preflight(request: ProgramParseRequest): Promise<RunPreflightReport>;
  authorizeFirstCut(
    request: ProgramParseRequest,
    confirmation: FirstCutConfirmation,
  ): Promise<FirstCutPreparation>;
}
