export interface ProgramParseRequest {
  readonly sourceName: string;
  readonly source: string;
  readonly programId?: string;
  readonly parseOptions?: ProgramParseOptions;
}

export interface ProgramDocumentMetadata {
  readonly id: string;
  readonly sourceBytes: number;
  readonly pageSize: number;
  readonly previewSampled: boolean;
  readonly warningCount: number;
  readonly blockingWarningCount: number;
  readonly errorCount: number;
  readonly managedToolChangeCount: number;
  readonly deepestCuttingZ?: number | null;
  readonly toolSelections: readonly { readonly sourceLine: number; readonly tool?: number | null }[];
  readonly toolSelectionCoverageLine: number;
  readonly initialToolNumber?: number | null;
}

export interface ProgramLinePage {
  readonly programId: string;
  readonly startIndex: number;
  readonly totalLines: number;
  readonly lines: readonly ProgramLine[];
}

export interface ProgramLineDetail {
  readonly programId: string;
  readonly line: ProgramLine;
  readonly toolpath: readonly ToolpathSegment[];
}

export interface ProgramParseOptions {
  readonly blockDelete: boolean;
}

export interface ProgramSaveOutcome {
  readonly path: string;
  readonly bytesWritten: number;
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

export interface ProgramRotaryBounds {
  readonly minDegrees: number;
  readonly maxDegrees: number;
  readonly sizeDegrees: number;
}

export interface ProgramRotaryMotion {
  // Unwrapped A degrees, independent of XYZ units and spindle rotation.
  readonly startDegrees: number;
  readonly endDegrees: number;
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
  readonly rotary?: ProgramRotaryMotion;
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
  | "preview-limit"
  | "rotary-timing-unavailable";

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
  readonly usesRotaryA?: boolean;
  readonly usesRotaryArc?: boolean;
  readonly usesInverseTimeFeed?: boolean;
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
  readonly rotaryBounds?: ProgramRotaryBounds;
  readonly rotaryTravelDegrees?: number;
  readonly previewComplete: boolean;
  readonly dryRunEligible: boolean;
}

export interface GcodeProgram {
  readonly document?: ProgramDocumentMetadata;
  readonly sourceName: string;
  readonly blockDeleteEnabled?: boolean;
  readonly lines: readonly ProgramLine[];
  readonly warnings: readonly ProgramWarning[];
  readonly features: ProgramFeatures;
  readonly summary: ProgramSummary;
  readonly toolpath: readonly ToolpathSegment[];
}
