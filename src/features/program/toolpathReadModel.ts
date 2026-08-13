import type {
  GcodeProgram,
  ProgramBounds,
  ProgramPoint,
} from "../../shared/program";

export interface ToolpathReadModel {
  readonly rapidPositions: Float32Array;
  readonly rapidSourceLines: readonly number[];
  readonly cuttingPositions: Float32Array;
  readonly cuttingSourceLines: readonly number[];
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

export function buildToolpathReadModel(
  program: GcodeProgram,
  cuttingDepthAdjustmentMm = 0,
): ToolpathReadModel {
  const adjustedPoint = (
    point: ProgramPoint,
    kind: GcodeProgram["toolpath"][number]["kind"],
  ): ProgramPoint => ({
    ...point,
    z: kind !== "rapid" && point.z < -1e-9
      ? Math.min(0, point.z + cuttingDepthAdjustmentMm)
      : point.z,
  });
  const adjustedPoints = program.toolpath.flatMap((segment) =>
    segment.points.map((point) => adjustedPoint(point, segment.kind)),
  );
  const bounds = Math.abs(cuttingDepthAdjustmentMm) < Number.EPSILON
    ? program.summary.bounds
    : adjustedPoints.length === 0 || !program.summary.bounds
    ? program.summary.bounds
    : (() => {
        const minZ = adjustedPoints.reduce((minimum, point) => Math.min(minimum, point.z), Infinity);
        const min = { ...program.summary.bounds.min, z: minZ };
        const max = program.summary.bounds.max;
        return {
          min,
          max,
          size: { x: max.x - min.x, y: max.y - min.y, z: max.z - min.z },
        };
      })();
  const center: ProgramPoint = bounds
    ? {
        x: (bounds.min.x + bounds.max.x) / 2,
        y: (bounds.min.y + bounds.max.y) / 2,
        z: (bounds.min.z + bounds.max.z) / 2,
      }
    : { x: 0, y: 0, z: 0 };
  const rapid: number[] = [];
  const rapidSourceLines: number[] = [];
  const cutting: number[] = [];
  const cuttingSourceLines: number[] = [];
  let pointCount = 0;

  for (const segment of program.toolpath) {
    const positions = segment.kind === "rapid" ? rapid : cutting;
    const sourceLines =
      segment.kind === "rapid" ? rapidSourceLines : cuttingSourceLines;
    for (let index = 1; index < segment.points.length; index += 1) {
      const start = adjustedPoint(segment.points[index - 1], segment.kind);
      const end = adjustedPoint(segment.points[index], segment.kind);
      positions.push(
        start.x - center.x,
        start.y - center.y,
        start.z - center.z,
        end.x - center.x,
        end.y - center.y,
        end.z - center.z,
      );
      sourceLines.push(segment.sourceLine);
      pointCount += 2;
    }
  }

  const span = bounds
    ? Math.max(bounds.size.x, bounds.size.y, bounds.size.z, 10)
    : 10;
  const gridSize = Math.ceil((span * 1.3) / 10) * 10;
  return {
    rapidPositions: new Float32Array(rapid),
    rapidSourceLines,
    cuttingPositions: new Float32Array(cutting),
    cuttingSourceLines,
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
      const adjust = (point: ProgramPoint): ProgramPoint => ({
        ...point,
        z: segment.kind !== "rapid" && point.z < -1e-9
          ? Math.min(0, point.z + cuttingDepthAdjustmentMm)
          : point.z,
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
