import { describe, expect, it } from "vitest";

import {
  heightmapSampleLabel,
  heightmapVisualScale,
  shouldLabelHeightmapSample,
} from "./heightmapSceneModel";

describe("heightmapSceneModel", () => {
  it("keeps small measured grids fully labelled", () => {
    expect(Array.from({ length: 36 }, (_, sequence) =>
      shouldLabelHeightmapSample(sequence, 36))).toEqual(Array(36).fill(true));
  });

  it("thins dense labels but always keeps endpoints and the active point", () => {
    expect(shouldLabelHeightmapSample(1, 100)).toBe(false);
    expect(shouldLabelHeightmapSample(0, 100)).toBe(true);
    expect(shouldLabelHeightmapSample(99, 100)).toBe(true);
    expect(shouldLabelHeightmapSample(1, 100, 1)).toBe(true);
  });

  it("formats signed heights and bounds visual exaggeration", () => {
    expect(heightmapSampleLabel(-0.9754)).toBe("-0.975 mm");
    expect(heightmapSampleLabel(0.021)).toBe("+0.021 mm");
    expect(heightmapVisualScale([-0.9, 0.1], 50).exaggeration).toBe(4);
    expect(heightmapVisualScale([0, 0], 50).exaggeration).toBe(50);
  });
});
