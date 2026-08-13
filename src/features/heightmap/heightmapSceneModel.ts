import type { HeightmapPlanRequest } from "../../shared/heightmap";

export interface HeightmapVisualScale {
  readonly exaggeration: number;
  readonly maximum: number;
  readonly minimum: number;
  readonly range: number;
}

export interface HeightmapSceneBounds {
  readonly centerX: number;
  readonly centerY: number;
  readonly maximumX: number;
  readonly maximumY: number;
  readonly minimumX: number;
  readonly minimumY: number;
  readonly span: number;
}

export const heightmapSceneBounds = (
  request: HeightmapPlanRequest,
  measuredRequest?: HeightmapPlanRequest,
): HeightmapSceneBounds => {
  const minimumX = Math.min(
    request.originXMm,
    measuredRequest?.originXMm ?? request.originXMm,
  );
  const minimumY = Math.min(
    request.originYMm,
    measuredRequest?.originYMm ?? request.originYMm,
  );
  const maximumX = Math.max(
    request.originXMm + request.widthMm,
    measuredRequest
      ? measuredRequest.originXMm + measuredRequest.widthMm
      : request.originXMm + request.widthMm,
  );
  const maximumY = Math.max(
    request.originYMm + request.heightMm,
    measuredRequest
      ? measuredRequest.originYMm + measuredRequest.heightMm
      : request.originYMm + request.heightMm,
  );

  return {
    centerX: (minimumX + maximumX) / 2,
    centerY: (minimumY + maximumY) / 2,
    maximumX,
    maximumY,
    minimumX,
    minimumY,
    span: Math.max(maximumX - minimumX, maximumY - minimumY, 10),
  };
};

export const heightmapCameraScope = (
  view: "top" | "iso",
  bounds: HeightmapSceneBounds,
): string => [
  view,
  bounds.minimumX,
  bounds.minimumY,
  bounds.maximumX,
  bounds.maximumY,
].map((value) => typeof value === "number" ? value.toFixed(4) : value).join(":");

export const heightmapVisualScale = (
  values: readonly number[],
  sceneSpan: number,
): HeightmapVisualScale => {
  const minimum = values.length > 0 ? Math.min(...values) : 0;
  const maximum = values.length > 0 ? Math.max(...values) : 0;
  const range = Math.max(maximum - minimum, 0.001);
  return {
    exaggeration: Math.min(50, Math.max(1, sceneSpan * 0.08 / range)),
    maximum,
    minimum,
    range,
  };
};

export const heightmapSampleLabel = (zMm: number): string =>
  `${zMm >= 0 ? "+" : ""}${zMm.toFixed(3)} mm`;

export const shouldLabelHeightmapSample = (
  sequence: number,
  total: number,
  currentSequence?: number,
): boolean => {
  if (sequence === currentSequence || total <= 49) return true;
  const stride = Math.max(1, Math.ceil(total / 36));
  return sequence === 0 || sequence === total - 1 || sequence % stride === 0;
};
