import { describe, expect, it } from "vitest";

import type { Heightmap } from "../../shared/heightmap";
import type { ProgramBounds } from "../../shared/program";
import { defaultHeightmapRequest } from "./heightmapDefaults";
import {
  applyDensity,
  buildHeightmapPlan,
  heightColor,
  heightmapMatrix,
  perimeterFromProgram,
  validateHeightmapRequest,
} from "./heightmapModel";

describe("heightmapModel", () => {
  it("derives a padded perimeter from the loaded job", () => {
    const bounds: ProgramBounds = {
      min: { x: -14.85, y: -5.22, z: -0.1 },
      max: { x: 14.7, y: 5.2, z: 2 },
      size: { x: 29.55, y: 10.42, z: 2.1 },
    };
    const next = perimeterFromProgram(defaultHeightmapRequest(), bounds, 1);
    expect(next.originXmm).toBe(-15.85);
    expect(next.originYmm).toBe(-6.22);
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
});
