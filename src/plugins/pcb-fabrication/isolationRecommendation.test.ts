import { describe, expect, it } from "vitest";

import type { PcbCopperAnalysis } from "../../shared/jobs";
import type { CuttingTool } from "../../shared/tooling";
import {
  isolationToolGeometryWarning,
  recommendIsolation,
  recommendIsolationForTool,
} from "./isolationRecommendation";

const copper = (minimumIsolationGapMm?: number): PcbCopperAnalysis => ({
  contourCount: 12,
  minimumIsolationGapMm,
});

const engraver = (
  id: string,
  tipDiameterMm: number,
  includedAngleDegrees?: number,
): CuttingTool => ({
  id,
  name: id,
  description: id,
  kind: "engraving",
  diameterMm: 3.175,
  tipDiameterMm,
  shankDiameterMm: 3.175,
  cuttingLengthMm: 3,
  fluteCount: 1,
  includedAngleDegrees,
  feedMmPerMin: id === "20deg" ? 300 : 240,
  plungeMmPerMin: 60,
  spindleRpm: 18_000,
  stepdownMm: 0.05,
  stepoverPercent: 10,
  factoryPreset: true,
});

describe("PCB isolation recommendation", () => {
  it("selects the narrow known 20 degree cutter and derives a cut-through depth", () => {
    const recommendation = recommendIsolation([
      engraver("unknown", 0.2),
      engraver("90deg", 0.1, 90),
      engraver("20deg", 0.1, 20),
    ], copper(0.16));

    expect(recommendation?.tool.id).toBe("20deg");
    expect(recommendation?.depthMm).toBeCloseTo(0.05);
    expect(recommendation?.effectiveDiameterMm).toBeCloseTo(0.1176, 3);
    expect(recommendation?.feedMmPerMin).toBe(300);
    expect(recommendation?.warning).toBeUndefined();
  });

  it("marks a 90 degree cutter as unable to fit a fine copper gap", () => {
    const recommendation = recommendIsolationForTool(
      engraver("90deg", 0.1, 90),
      copper(0.16),
    );

    expect(recommendation?.depthMm).toBeCloseTo(0.05);
    expect(recommendation?.warning).toContain("не помещается");
  });

  it("does not invent an angle for an unmarked kit engraver", () => {
    const tool = engraver("unknown", 0.2);
    expect(recommendIsolationForTool(tool, copper(0.3))).toBeUndefined();
    expect(isolationToolGeometryWarning(tool)).toContain("не указан угол");
  });
});
