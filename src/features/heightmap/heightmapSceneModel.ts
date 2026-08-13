export interface HeightmapVisualScale {
  readonly exaggeration: number;
  readonly maximum: number;
  readonly minimum: number;
  readonly range: number;
}

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
