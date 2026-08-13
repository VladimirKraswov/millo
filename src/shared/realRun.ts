import type { HardwareInspection } from "./machine";
import type { ProgramBounds, ProgramParseRequest } from "./program";
import type {
  ProgramRecoveryCandidate,
  ProgramRecoveryPackage,
  ProgramRecoveryPreparationRequest,
} from "./recovery";

export type ProgramRunIntent = "airRun" | "cutting";

export interface ProgramExecutionOptions {
  readonly optionalStop: boolean;
  readonly blockDelete: boolean;
  readonly surfaceMapId?: number;
  readonly cuttingDepthAdjustmentUm?: number;
}

export type ProgramWorkCoordinateSystem =
  | "g54"
  | "g55"
  | "g56"
  | "g57"
  | "g58"
  | "g59";

export type ProgramSpindleMode = "off" | "clockwise" | "counterclockwise";

export interface SelectedRunPreparationRequest {
  readonly request: ProgramParseRequest;
  readonly selectedSourceLine: number;
  readonly safeZMm: number;
  readonly intent: ProgramRunIntent;
  readonly executionOptions: ProgramExecutionOptions;
}

export interface SafeStartPackage {
  readonly originalSourceName: string;
  readonly selectedSourceLine: number;
  readonly restartSourceLine: number;
  readonly restartPosition: { readonly x: number; readonly y: number; readonly z: number };
  readonly safeZMm: number;
  readonly minimumSafeZMm: number;
  readonly replayedExecutableLines: number;
  readonly remainingExecutableLines: number;
  readonly workCoordinateSystem: ProgramWorkCoordinateSystem;
  readonly selectedTool?: number;
  readonly spindleMode: ProgramSpindleMode;
  readonly request: ProgramParseRequest;
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
  readonly probeRemoved: boolean;
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
  prepareSelectedRun(
    request: SelectedRunPreparationRequest,
  ): Promise<SafeStartPackage>;
  recoveryCandidate(): Promise<ProgramRecoveryCandidate | null>;
  prepareRecovery(
    request: ProgramRecoveryPreparationRequest,
  ): Promise<ProgramRecoveryPackage>;
  dismissRecovery(recoveryId: number): Promise<void>;
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
  pauseProgram(): Promise<import("./dryRun").SenderSnapshot>;
  resumeProgram(): Promise<import("./dryRun").SenderSnapshot>;
  abortProgram(): Promise<import("./dryRun").SenderSnapshot>;
  completeToolChange(
    confirmation: ToolChangeConfirmation,
  ): Promise<import("./dryRun").SenderSnapshot>;
}
