import { describe, expect, it } from "vitest";

import { previewFixtureAirSquareProgram } from "./previewFixtureAirSquare";
import {
  adjustCuttingZ,
  depthAdjustmentFromDraft,
  deepestCuttingZ,
  depthAdjustmentUm,
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
  it("starts disabled with a zero offset without exposing a derived target", () => {
    expect(deepestCuttingZ(engravingProgram)).toBe(-0.2);
    expect(depthCorrectionView(engravingProgram, undefined)).toMatchObject({
      available: true,
      enabled: false,
      adjustmentMm: 0,
      minimumAdjustmentMm: -10,
      maximumAdjustmentMm: 10,
    });
  });

  it("stores the signed offset as exact micrometres", () => {
    expect(depthAdjustmentUm(-0.1)).toBe(-100);
    expect(depthAdjustmentUm(0.125)).toBe(125);
    expect(depthCorrectionView(engravingProgram, -100).adjustmentMm).toBeCloseTo(-0.1);
  });

  it("allows an operator to compose a negative decimal before committing it", () => {
    expect(depthAdjustmentFromDraft("-")).toBeUndefined();
    expect(depthAdjustmentFromDraft("-0.")).toBe(-0);
    expect(depthAdjustmentFromDraft("-0.1")).toBe(-0.1);
    expect(depthAdjustmentFromDraft("")).toBeUndefined();
  });

  it("adds the exact offset to negative cutting points only", () => {
    expect(adjustCuttingZ(3, "rapid", -0.1)).toBe(3);
    expect(adjustCuttingZ(0, "linear", -0.1)).toBe(0);
    expect(adjustCuttingZ(-0.2, "linear", -0.1)).toBeCloseTo(-0.3);
    expect(adjustCuttingZ(-0.05, "linear", 0.1)).toBeCloseTo(0.05);
  });

  it("rejects offsets outside the guarded range", () => {
    expect(() => depthAdjustmentUm(-10.001)).toThrow("±10 мм");
    expect(() => depthAdjustmentUm(Number.NaN)).toThrow("должно быть числом");
  });
});
