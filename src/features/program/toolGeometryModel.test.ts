import { describe, expect, it } from "vitest";

import type { CuttingTool } from "../../shared/tooling";
import { toolRenderProfile } from "./toolGeometryModel";

describe("toolRenderProfile", () => {
  it("keeps conical tip proportions while bounding scene-only lengths", () => {
    const profile = toolRenderProfile({
      kind: "vBit",
      diameterMm: 3.175,
      tipDiameterMm: 0.1,
      shankDiameterMm: 3.175,
      cuttingLengthMm: 20,
      fluteCount: 2,
      includedAngleDegrees: 20,
      spindleRpm: 12_000,
    } as CuttingTool, 50);

    expect(profile.kind).toBe("vBit");
    expect(profile.diameterMm).toBeCloseTo(3.175);
    expect(profile.tipDiameterMm).toBeCloseTo(0.1);
    expect(profile.cuttingLengthMm).toBeLessThanOrEqual(17.5);
    expect(profile.angularSpeedRadPerSecond).toBe(18);
  });

  it("provides a visible generic cutter without inventing library identity", () => {
    const profile = toolRenderProfile(undefined, 30);

    expect(profile.kind).toBe("flatEndMill");
    expect(profile.diameterMm).toBeCloseTo(3.175);
    expect(profile.fluteCount).toBe(2);
  });
});
