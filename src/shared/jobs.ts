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

export interface GeneratedImageJob {
  readonly sourceName: string;
  readonly source: string;
  readonly vectorSvg: string;
  readonly program: GcodeProgram;
  readonly summary: ImageJobSummary;
}

export interface GeneratedGcodeSaveOutcome {
  readonly path: string;
  readonly bytesWritten: number;
}

export interface PublishedJob {
  readonly sequence: number;
  readonly job: GeneratedImageJob;
}
