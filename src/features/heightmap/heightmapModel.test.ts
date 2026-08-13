import { describe, expect, it } from "vitest";

import type { Heightmap } from "../../shared/heightmap";
import type { ProgramBounds } from "../../shared/program";
import { defaultHeightmapRequest } from "./heightmapDefaults";
import {
  applyDensity,
  buildHeightmapPlan,
  estimateHeightmapSeconds,
  heightColor,
  heightmapCalibrationPlateThickness,
  heightmapSafeWorkZ,
  heightmapSurfaceVariation,
  heightmapMatrix,
  describeHeightmapFailure,
  perimeterFromProgram,
  validateHeightmapRequest,
  withHeightmapSurfaceVariation,
} from "./heightmapModel";

describe("heightmapModel", () => {
  it("derives a padded perimeter from the loaded job", () => {
    const bounds: ProgramBounds = {
      min: { x: -14.85, y: -5.22, z: -0.1 },
      max: { x: 14.7, y: 5.2, z: 2 },
      size: { x: 29.55, y: 10.42, z: 2.1 },
    };
    const next = perimeterFromProgram(defaultHeightmapRequest(), bounds, 1);
    expect(next.originXMm).toBe(-15.85);
    expect(next.originYMm).toBe(-6.22);
    expect(next.widthMm).toBe(31.55);
    expect(next.heightMm).toBe(12.42);
  });

  it("keeps probing density independent from interpolation rendering", () => {
    const next = applyDensity(
      { ...defaultHeightmapRequest(), widthMm: 50, heightMm: 30 },
      "precise",
    );
    expect(next.columns).toBe(11);
    expect(next.rows).toBe(7);
    expect(buildHeightmapPlan(next).points).toHaveLength(77);
  });

  it("estimates the same bounded probe and full retract path as Rust", () => {
    const request = {
      ...defaultHeightmapRequest(),
      widthMm: 10,
      heightMm: 10,
      columns: 2,
      rows: 2,
      clearanceZMm: 2,
      maxProbeDepthMm: 3,
      probeFeedMmPerMin: 30,
      travelFeedMmPerMin: 60,
      retractFeedMmPerMin: 60,
    };
    // XY is 30 s; each of four points probes for 6 s and retracts for 5 s.
    expect(estimateHeightmapSeconds(request)).toBe(74);
  });

  it("maps serpentine samples back into a readable numeric matrix", () => {
    const plan = buildHeightmapPlan({ ...defaultHeightmapRequest(), columns: 2, rows: 2 });
    const map: Heightmap = {
      schemaVersion: 1,
      plan,
      samples: plan.points.map((point) => ({ point, zMm: point.sequence * 0.1, triggered: true })),
    };
    const matrix = heightmapMatrix(map);
    expect(matrix[0]).toEqual([0, 0.1]);
    expect(matrix[1][0]).toBeCloseTo(0.3);
    expect(matrix[1][1]).toBeCloseTo(0.2);
  });

  it("rejects maps beyond the machine travel and uses a stable low-to-high color scale", () => {
    expect(validateHeightmapRequest(
      { ...defaultHeightmapRequest(), widthMm: 301 },
      { x: 300, y: 180, z: 80 },
    )).toContain("больше рабочего поля");
    expect(heightColor(-0.2, -0.2, 0.2)).toContain("205");
    expect(heightColor(0.2, -0.2, 0.2)).toContain("35");
  });

  it("mirrors Rust coordinate and feed limits before dispatch", () => {
    expect(validateHeightmapRequest({
      ...defaultHeightmapRequest(),
      originXMm: 100_001,
    })).toContain("периметра X");
    expect(validateHeightmapRequest({
      ...defaultHeightmapRequest(),
      probeFeedMmPerMin: 1_001,
    })).toContain("Подача щупа");
    expect(validateHeightmapRequest({
      ...defaultHeightmapRequest(),
      travelFeedMmPerMin: 5,
    })).toContain("Подача перехода");
  });

  it("derives a safe Z above a fixed plate and keeps variation operator-facing", () => {
    const request = {
      ...defaultHeightmapRequest(),
      contactMode: "fixedPlate" as const,
      contactOffsetMm: 19.1,
      clearanceZMm: 2,
    };
    expect(heightmapSafeWorkZ(request)).toBe(21.1);
    expect(heightmapCalibrationPlateThickness(request, 3)).toBe(19.1);
    expect(heightmapCalibrationPlateThickness(defaultHeightmapRequest(), 3)).toBe(3);
    const next = withHeightmapSurfaceVariation(request, 1.5);
    expect(next.maxProbeDepthMm).toBe(3.5);
    expect(heightmapSurfaceVariation(next)).toBe(1.5);
  });

  it("translates a bounded probe miss into an actionable operator message", () => {
    expect(describeHeightmapFailure("probe did not contact the surface", 12)).toContain("12.0 mm");
    expect(describeHeightmapFailure("ALARM:5", 3)).toContain("подведите фрезу ближе");
  });
});
