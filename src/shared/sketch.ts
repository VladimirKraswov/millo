import type { GeneratedJob } from "./jobs";

export interface SketchPoint {
  readonly x: number;
  readonly y: number;
}
export type SketchGeometry =
  | {
      readonly kind: "rectangle";
      readonly width: number;
      readonly height: number;
      readonly radius: number;
    }
  | { readonly kind: "circle"; readonly diameter: number }
  | { readonly kind: "polygon"; readonly points: readonly SketchPoint[] };
export type SketchOperationKind =
  | "pocket"
  | "inside"
  | "outside"
  | "engrave"
  | "drill";
export interface SketchTabs {
  readonly count: number;
  readonly widthMm: number;
  readonly heightMm: number;
}
export interface SketchOperation {
  readonly kind: SketchOperationKind;
  readonly toolId: string;
  readonly through: boolean;
  readonly depthMm: number;
  readonly stepdownMm: number;
  readonly stepoverPercent: number;
  readonly feedMmPerMin: number;
  readonly plungeMmPerMin: number;
  readonly spindleRpm: number;
  readonly tabs: SketchTabs;
}
export interface SketchShape {
  readonly id: string;
  readonly name: string;
  readonly xMm: number;
  readonly yMm: number;
  readonly rotationDegrees: number;
  readonly geometry: SketchGeometry;
  readonly operation: SketchOperation;
}
export interface SketchStock {
  readonly widthMm: number;
  readonly heightMm: number;
  readonly thicknessMm: number;
  readonly safeZMm: number;
  readonly breakthroughMm: number;
  readonly spindleMode: "manual" | "controller";
}
export interface SketchJobRequest {
  readonly sourceName: string;
  readonly stock: SketchStock;
  readonly shapes: readonly SketchShape[];
}
export interface SketchProject {
  readonly version: 1;
  readonly document: SketchJobRequest;
}
export interface SketchOperationSummary {
  readonly shapeId: string;
  readonly name: string;
  readonly toolId: string;
  readonly toolNumber: number;
  readonly depthMm: number;
  readonly passCount: number;
  readonly pathCount: number;
}
export interface GeneratedSketchJob extends GeneratedJob {
  readonly summary: {
    readonly operations: readonly SketchOperationSummary[];
    readonly toolChangeCount: number;
    readonly paths: readonly {
      readonly shapeId: string;
      readonly points: readonly SketchPoint[];
    }[];
    readonly tabPaths: readonly {
      readonly shapeId: string;
      readonly points: readonly SketchPoint[];
    }[];
    readonly warnings: readonly string[];
  };
}
