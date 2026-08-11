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

export interface RealRunPreflightGateway {
  preflight(request: ProgramParseRequest): Promise<RunPreflightReport>;
}
