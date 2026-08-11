import type { GcodeProgram, ProgramPoint } from "../../shared/program";

export interface ToolpathReadModel {
  readonly rapidPositions: Float32Array;
  readonly cuttingPositions: Float32Array;
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

export function buildToolpathReadModel(program: GcodeProgram): ToolpathReadModel {
  const bounds = program.summary.bounds;
  const center: ProgramPoint = bounds
    ? {
        x: (bounds.min.x + bounds.max.x) / 2,
        y: (bounds.min.y + bounds.max.y) / 2,
        z: (bounds.min.z + bounds.max.z) / 2,
      }
    : { x: 0, y: 0, z: 0 };
  const rapid: number[] = [];
  const cutting: number[] = [];
  let pointCount = 0;

  for (const segment of program.toolpath) {
    const positions = segment.kind === "rapid" ? rapid : cutting;
    for (let index = 1; index < segment.points.length; index += 1) {
      const start = segment.points[index - 1];
      const end = segment.points[index];
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

  const span = bounds
    ? Math.max(bounds.size.x, bounds.size.y, bounds.size.z, 10)
    : 10;
  const gridSize = Math.ceil((span * 1.3) / 10) * 10;
  return {
    rapidPositions: new Float32Array(rapid),
    cuttingPositions: new Float32Array(cutting),
    center,
    gridSize,
    gridZ: (bounds?.min.z ?? 0) - center.z,
    frameRadius: Math.max(span * 0.72, 7),
    pointCount,
  };
}

export function buildToolpathHighlightReadModel(
  program: GcodeProgram,
  sourceLine: number | undefined,
  center: ProgramPoint,
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
  for (const segment of program.toolpath) {
    if (segment.sourceLine !== sourceLine) continue;
    segmentCount += 1;
    for (let index = 1; index < segment.points.length; index += 1) {
      const start = segment.points[index - 1];
      const end = segment.points[index];
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
