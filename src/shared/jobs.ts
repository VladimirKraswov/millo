import type { GcodeProgram } from "./program";

export type ImageJobFormat = "svg" | "png";

export interface ImageJobSettings {
  readonly widthMm: number;
  readonly safeZMm: number;
  readonly surfaceZMm: number;
  readonly engravingDepthMm: number;
  readonly feedMmPerMin: number;
  readonly plungeMmPerMin: number;
  readonly curveToleranceMm: number;
  readonly rasterThresholdPercent: number;
  readonly traceSpecklePx: number;
  readonly traceCornerThresholdDegrees: number;
  readonly traceSegmentLengthPx: number;
  readonly invert: boolean;
}

export const defaultImageJobSettings: ImageJobSettings = Object.freeze({
  widthMm: 50,
  safeZMm: 3,
  surfaceZMm: 0,
  engravingDepthMm: 0.2,
  feedMmPerMin: 300,
  plungeMmPerMin: 100,
  curveToleranceMm: 0.08,
  rasterThresholdPercent: 50,
  traceSpecklePx: 4,
  traceCornerThresholdDegrees: 60,
  traceSegmentLengthPx: 4,
  invert: false,
});

export interface ImageJobRequest {
  readonly sourceName: string;
  readonly sourceBase64: string;
  readonly format: ImageJobFormat;
  readonly settings: ImageJobSettings;
}

export interface ImageJobSummary {
  readonly widthMm: number;
  readonly heightMm: number;
  readonly pathCount: number;
  readonly pointCount: number;
  readonly sourceWidthPx?: number;
  readonly sourceHeightPx?: number;
}

export interface GeneratedJob {
  readonly sourceName: string;
  readonly source: string;
  readonly program: GcodeProgram;
}

export interface GeneratedImageJob extends GeneratedJob {
  readonly vectorSvg: string;
  readonly summary: ImageJobSummary;
}

export type SurfacingRasterAxis = "x" | "y";

export interface SurfacingJobSettings {
  readonly originXMm: number;
  readonly originYMm: number;
  readonly widthMm: number;
  readonly heightMm: number;
  readonly edgeOverrunMm: number;
  readonly surfaceZMm: number;
  readonly removalMm: number;
  readonly depthPerPassMm: number;
  readonly safeZMm: number;
  readonly stepoverPercent: number;
  readonly feedMmPerMin: number;
  readonly plungeMmPerMin: number;
  readonly rasterAxis: SurfacingRasterAxis;
}

export interface SurfacingJobRequest {
  readonly sourceName: string;
  readonly toolId: string;
  readonly settings: SurfacingJobSettings;
}

export interface SurfacingJobSummary {
  readonly toolId: string;
  readonly toolName: string;
  readonly toolDiameterMm: number;
  readonly passCount: number;
  readonly rasterLineCount: number;
  readonly stepoverMm: number;
  readonly coveredWidthMm: number;
  readonly coveredHeightMm: number;
  readonly edgeOverrunMm: number;
  readonly removalMm: number;
  readonly spindleRpm: number;
}

export interface GeneratedSurfacingJob extends GeneratedJob {
  readonly summary: SurfacingJobSummary;
}

export type PcbLayerRole = "copper" | "drill" | "outline" | "marking";

export interface PcbSourceFile {
  readonly sourceName: string;
  readonly sourceBase64: string;
  readonly role: PcbLayerRole;
}

export interface PcbTransform {
  readonly offsetXMm: number;
  readonly offsetYMm: number;
  readonly rotationQuarterTurns: number;
  readonly mirrorX: boolean;
}

export interface PcbInspectRequest {
  readonly files: readonly PcbSourceFile[];
  readonly transform: PcbTransform;
}

export interface PcbPoint {
  readonly xMm: number;
  readonly yMm: number;
}

export interface PcbBounds {
  readonly minXMm: number;
  readonly minYMm: number;
  readonly maxXMm: number;
  readonly maxYMm: number;
  readonly widthMm: number;
  readonly heightMm: number;
}

export interface PcbPreviewPath {
  readonly role: PcbLayerRole;
  readonly closed: boolean;
  readonly points: readonly PcbPoint[];
}

export interface PcbDrillHit {
  readonly groupKey: string;
  readonly point: PcbPoint;
}

export interface PcbDrillGroup {
  readonly key: string;
  readonly sourceName: string;
  readonly sourceToolNumber: number;
  readonly diameterMm: number;
  readonly hitCount: number;
}

export interface PcbFileSummary {
  readonly sourceName: string;
  readonly role: PcbLayerRole;
  readonly primitiveCount: number;
}

export interface PcbInspection {
  readonly bounds: PcbBounds;
  readonly paths: readonly PcbPreviewPath[];
  readonly drillHits: readonly PcbDrillHit[];
  readonly drillGroups: readonly PcbDrillGroup[];
  readonly files: readonly PcbFileSummary[];
  readonly warnings: readonly string[];
}

export interface PcbIsolationSettings {
  readonly enabled: boolean;
  readonly toolId: string;
  readonly depthMm: number;
  readonly clearanceMm: number;
  readonly passes: number;
}

export interface PcbDrillToolMapping {
  readonly groupKey: string;
  readonly toolId: string;
}

export interface PcbDrillingSettings {
  readonly enabled: boolean;
  readonly depthMm: number;
  readonly mappings: readonly PcbDrillToolMapping[];
}

export interface PcbOutlineSettings {
  readonly enabled: boolean;
  readonly toolId: string;
  readonly depthMm: number;
  readonly depthPerPassMm: number;
  readonly tabCount: number;
  readonly tabWidthMm: number;
  readonly tabHeightMm: number;
}

export interface PcbMarkingSettings {
  readonly enabled: boolean;
  readonly toolId: string;
  readonly depthMm: number;
}

export interface PcbJobSettings {
  readonly safeZMm: number;
  readonly surfaceZMm: number;
  readonly isolation: PcbIsolationSettings;
  readonly drilling: PcbDrillingSettings;
  readonly outline: PcbOutlineSettings;
  readonly marking: PcbMarkingSettings;
}

export interface PcbJobRequest {
  readonly sourceName: string;
  readonly board: PcbInspectRequest;
  readonly settings: PcbJobSettings;
}

export interface PcbOperationSummary {
  readonly kind: string;
  readonly toolId: string;
  readonly toolName: string;
  readonly motionCount: number;
}

export interface PcbJobSummary {
  readonly bounds: PcbBounds;
  readonly operations: readonly PcbOperationSummary[];
  readonly toolChangeCount: number;
  readonly warningCount: number;
}

export interface GeneratedPcbJob extends GeneratedJob {
  readonly inspection: PcbInspection;
  readonly summary: PcbJobSummary;
}

export interface GeneratedGcodeSaveOutcome {
  readonly path: string;
  readonly bytesWritten: number;
}

export interface PublishedJob {
  readonly sequence: number;
  readonly job: GeneratedJob;
}
