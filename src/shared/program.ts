export interface ProgramParseRequest {
  readonly sourceName: string;
  readonly source: string;
}

export interface ProgramParseOptions {
  readonly blockDelete: boolean;
}

export interface ProgramPoint {
  readonly x: number;
  readonly y: number;
  readonly z: number;
}

export interface ProgramBounds {
  readonly min: ProgramPoint;
  readonly max: ProgramPoint;
  readonly size: ProgramPoint;
}

export type ToolpathKind =
  | "rapid"
  | "linear"
  | "arcClockwise"
  | "arcCounterclockwise";

export interface ToolpathSegment {
  readonly sourceLine: number;
  readonly optionalBlock?: boolean;
  readonly kind: ToolpathKind;
  readonly points: readonly ProgramPoint[];
  readonly distanceMm: number;
  readonly feedRateMmPerMin?: number;
  readonly estimatedDurationSeconds?: number;
}

export type ProgramWarningSeverity = "warning" | "safety" | "error";

export type ProgramWarningCode =
  | "unclosed-comment"
  | "unexpected-comment-close"
  | "invalid-token"
  | "duplicate-word"
  | "optional-block"
  | "optional-block-unsupported"
  | "checksum-validated"
  | "checksum-invalid"
  | "checksum-unsupported"
  | "unsupported-g-code"
  | "unsupported-m-code"
  | "unsupported-word"
  | "unsupported-plane"
  | "coordinate-system-ignored"
  | "unsafe-machine-command"
  | "spindle-activation"
  | "spindle-speed"
  | "tool-change"
  | "arc-definition"
  | "dwell-definition"
  | "feed-rate"
  | "modal-group-conflict"
  | "preview-limit";

export interface ProgramWarning {
  readonly sourceLine: number;
  readonly severity: ProgramWarningSeverity;
  readonly code: ProgramWarningCode;
  readonly message: string;
}

export interface ProgramLine {
  readonly sourceLine: number;
  readonly source: string;
  readonly normalized: string;
  readonly executable: boolean;
  readonly optionalBlock?: boolean;
  readonly blockDeleted?: boolean;
  readonly checksum?: number;
  readonly warningCount: number;
}

export interface ProgramFeatures {
  readonly usesImperialUnits: boolean;
  readonly usesIncrementalDistance: boolean;
  readonly hasSpindleActivation: boolean;
  readonly hasSpindleSpeed: boolean;
  readonly hasToolChange: boolean;
  readonly hasProbeCycle: boolean;
  readonly hasMachineCoordinateMove: boolean;
}

export interface ProgramSummary {
  readonly lineCount: number;
  readonly executableLineCount: number;
  readonly motionCount: number;
  readonly rapidDistanceMm: number;
  readonly cuttingDistanceMm: number;
  readonly estimatedMotionTimeSeconds: number;
  readonly dwellTimeSeconds: number;
  readonly estimatedTotalTimeSeconds: number;
  readonly timeEstimateComplete: boolean;
  readonly bounds?: ProgramBounds;
  readonly previewComplete: boolean;
  readonly dryRunEligible: boolean;
}

export interface GcodeProgram {
  readonly sourceName: string;
  readonly blockDeleteEnabled?: boolean;
  readonly lines: readonly ProgramLine[];
  readonly warnings: readonly ProgramWarning[];
  readonly features: ProgramFeatures;
  readonly summary: ProgramSummary;
  readonly toolpath: readonly ToolpathSegment[];
}
