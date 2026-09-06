import type {
  GcodeProgram,
  ProgramBounds,
  ProgramPoint,
  ProgramRotaryMotion,
  ToolpathSegment,
} from "../../shared/program";
import { adjustCuttingZ } from "./depthCorrectionModel";
import { programSourceIndex } from "./programSourceIndex";

export interface ToolpathReadModel {
  readonly rapidPositions: Float32Array;
  readonly rapidSourceLines: readonly number[];
  readonly cuttingPositions: Float32Array;
  readonly cuttingSourceLines: readonly number[];
  readonly rotaryPositions: Float32Array;
  readonly rotarySourceLines: readonly number[];
  readonly center: ProgramPoint;
  readonly gridSize: number;
  readonly gridZ: number;
  readonly frameRadius: number;
  readonly pointCount: number;
}

export interface ToolpathHighlightReadModel {
  readonly positions: Float32Array;
  readonly segmentCount: number;
  readonly pointCount: number;
}

export interface ToolPositionReadModel {
  readonly scenePosition: ProgramPoint;
  readonly gridProjection: ProgramPoint;
  readonly overProgram: boolean;
}

export function buildRotarySelectionReadModel(
  program: GcodeProgram,
  sourceLine: number | undefined,
  selectedToolpath?: readonly ToolpathSegment[],
): readonly ProgramRotaryMotion[] {
  if (sourceLine === undefined) return [];
  return (selectedToolpath ?? programSourceIndex(program).motions.get(sourceLine) ?? [])
    .flatMap((segment) => segment.sourceLine === sourceLine && segment.rotary ? [segment.rotary] : []);
}

export const formatRotaryDegrees = (degrees: number | undefined): string =>
  degrees !== undefined && Number.isFinite(degrees)
    ? `${degrees.toFixed(3)}°`
    : "--";

export function buildToolpathReadModel(
  program: GcodeProgram,
  cuttingDepthAdjustmentMm = 0,
): ToolpathReadModel {
  let bounds = program.summary.bounds;
  let rapidVertexCount = 0;
  let cuttingVertexCount = 0;
  let minZ = Infinity;
  let maxZ = -Infinity;
  for (const segment of program.toolpath) {
    const vertices = Math.max(0, segment.points.length - 1) * 2;
    if (segment.kind === "rapid") rapidVertexCount += vertices;
    else cuttingVertexCount += vertices;
    if (bounds && Math.abs(cuttingDepthAdjustmentMm) >= Number.EPSILON) {
      for (const point of segment.points) {
        const z = adjustCuttingZ(point.z, segment.kind, cuttingDepthAdjustmentMm);
        minZ = Math.min(minZ, z);
        maxZ = Math.max(maxZ, z);
      }
    }
  }
  if (bounds && minZ !== Infinity) {
    const min = { ...bounds.min, z: minZ };
    const max = { ...bounds.max, z: maxZ };
    bounds = { min, max, size: { ...bounds.size, z: maxZ - minZ } };
  }
  const center: ProgramPoint = bounds
    ? {
        x: (bounds.min.x + bounds.max.x) / 2,
        y: (bounds.min.y + bounds.max.y) / 2,
        z: (bounds.min.z + bounds.max.z) / 2,
      }
    : { x: 0, y: 0, z: 0 };
  const rapid = new Float32Array(rapidVertexCount * 3);
  const rapidSourceLines: number[] = [];
  const cutting = new Float32Array(cuttingVertexCount * 3);
  const cuttingSourceLines: number[] = [];
  let pointCount = 0;
  let rapidOffset = 0;
  let cuttingOffset = 0;
  const rotaryPositions: number[] = [];
  const rotarySourceLines: number[] = [];

  for (const segment of program.toolpath) {
    const anchor = segment.points[0];
    if (anchor && segment.rotary &&
      segment.rotary.startDegrees !== segment.rotary.endDegrees &&
      segment.points.every((point) =>
        point.x === anchor.x && point.y === anchor.y && point.z === anchor.z)) {
      rotaryPositions.push(
        anchor.x - center.x, anchor.y - center.y,
        adjustCuttingZ(anchor.z, segment.kind, cuttingDepthAdjustmentMm) - center.z,
      );
      rotarySourceLines.push(segment.sourceLine);
    }
    const positions = segment.kind === "rapid" ? rapid : cutting;
    const sourceLines =
      segment.kind === "rapid" ? rapidSourceLines : cuttingSourceLines;
    for (let index = 1; index < segment.points.length; index += 1) {
      const start = segment.points[index - 1];
      const end = segment.points[index];
      let offset = segment.kind === "rapid" ? rapidOffset : cuttingOffset;
      positions[offset++] = start.x - center.x;
      positions[offset++] = start.y - center.y;
      positions[offset++] = adjustCuttingZ(start.z, segment.kind, cuttingDepthAdjustmentMm) - center.z;
      positions[offset++] = end.x - center.x;
      positions[offset++] = end.y - center.y;
      positions[offset++] = adjustCuttingZ(end.z, segment.kind, cuttingDepthAdjustmentMm) - center.z;
      if (segment.kind === "rapid") rapidOffset = offset;
      else cuttingOffset = offset;
      sourceLines.push(segment.sourceLine);
      pointCount += 2;
    }
  }

  const span = bounds
    ? Math.max(bounds.size.x, bounds.size.y, bounds.size.z, 10)
    : 10;
  const gridSize = Math.ceil((span * 1.3) / 10) * 10;
  return {
    rapidPositions: rapid,
    rapidSourceLines,
    cuttingPositions: cutting,
    cuttingSourceLines,
    rotaryPositions: new Float32Array(rotaryPositions),
    rotarySourceLines,
    center,
    gridSize,
    gridZ: (bounds?.min.z ?? 0) - center.z,
    frameRadius: Math.max(span * 0.72, 7),
    pointCount,
  };
}

export function sourceLineForIntersection(
  sourceLines: readonly number[],
  vertexIndex: number | undefined,
): number | undefined {
  if (vertexIndex === undefined || vertexIndex < 0) return undefined;
  return sourceLines[Math.floor(vertexIndex / 2)];
}

export function buildToolpathHighlightReadModel(
  program: GcodeProgram,
  sourceLine: number | undefined,
  center: ProgramPoint,
  cuttingDepthAdjustmentMm = 0,
  selectedToolpath?: readonly ToolpathSegment[],
): ToolpathHighlightReadModel {
  if (sourceLine === undefined) {
    return {
      positions: new Float32Array(),
      segmentCount: 0,
      pointCount: 0,
    };
  }

  const positions: number[] = [];
  let segmentCount = 0;
  let pointCount = 0;
  for (const segment of selectedToolpath ?? programSourceIndex(program).motions.get(sourceLine) ?? []) {
    if (segment.sourceLine !== sourceLine) continue;
    segmentCount += 1;
    for (let index = 1; index < segment.points.length; index += 1) {
      const adjust = (point: ProgramPoint): ProgramPoint => ({
        ...point,
        z: adjustCuttingZ(point.z, segment.kind, cuttingDepthAdjustmentMm),
      });
      const start = adjust(segment.points[index - 1]);
      const end = adjust(segment.points[index]);
      positions.push(
        start.x - center.x,
        start.y - center.y,
        start.z - center.z,
        end.x - center.x,
        end.y - center.y,
        end.z - center.z,
      );
      pointCount += 2;
    }
  }

  return {
    positions: new Float32Array(positions),
    segmentCount,
    pointCount,
  };
}

export function buildToolPositionReadModel(
  position: ProgramPoint,
  model: Pick<ToolpathReadModel, "center" | "gridZ">,
  bounds?: ProgramBounds,
): ToolPositionReadModel {
  const scenePosition = {
    x: position.x - model.center.x,
    y: position.y - model.center.y,
    z: position.z - model.center.z,
  };
  return {
    scenePosition,
    gridProjection: {
      x: scenePosition.x,
      y: scenePosition.y,
      z: model.gridZ,
    },
    overProgram:
      bounds !== undefined &&
      position.x >= bounds.min.x &&
      position.x <= bounds.max.x &&
      position.y >= bounds.min.y &&
      position.y <= bounds.max.y,
  };
}
