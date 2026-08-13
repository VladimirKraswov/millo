import { describe, expect, it } from "vitest";

import { previewFixtureAirSquareProgram } from "./previewFixtureAirSquare";
import {
  adjustCuttingZ,
  deepestCuttingZ,
  depthAdjustmentUmForTarget,
  depthCorrectionView,
} from "./depthCorrectionModel";

const engravingProgram = {
  ...previewFixtureAirSquareProgram,
  toolpath: [
    {
      sourceLine: 5,
      kind: "rapid" as const,
      points: [{ x: 0, y: 0, z: 3 }, { x: 0, y: 0, z: 1 }],
      distanceMm: 2,
    },
    {
      sourceLine: 6,
      kind: "linear" as const,
      points: [{ x: 0, y: 0, z: 0 }, { x: 2, y: 0, z: -0.2 }],
      distanceMm: 2,
      feedRateMmPerMin: 100,
    },
  ],
};

describe("depth correction model", () => {
  it("derives the default target from the deepest cutting point", () => {
    expect(deepestCuttingZ(engravingProgram)).toBe(-0.2);
    expect(depthCorrectionView(engravingProgram, undefined)).toMatchObject({
      available: true,
      enabled: false,
      fileDepthMm: -0.2,
      targetDepthMm: -0.2,
      adjustmentMm: 0,
    });
  });

  it("stores the target as an exact micrometre adjustment", () => {
    expect(depthAdjustmentUmForTarget(-0.2, -0.3)).toBe(-100);
    expect(depthCorrectionView(engravingProgram, -100).targetDepthMm).toBeCloseTo(-0.3);
  });

  it("does not move rapid, surface, or shallower-than-zero-safe points", () => {
    expect(adjustCuttingZ(3, "rapid", -0.1)).toBe(3);
    expect(adjustCuttingZ(0, "linear", -0.1)).toBe(0);
    expect(adjustCuttingZ(-0.2, "linear", -0.1)).toBeCloseTo(-0.3);
    expect(adjustCuttingZ(-0.05, "linear", 0.1)).toBe(0);
  });
});
