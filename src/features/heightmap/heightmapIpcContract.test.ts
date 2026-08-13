import { describe, expect, it } from "vitest";

import startRequestFixture from "../../../fixtures/heightmap/start-request.json";
import type { HeightmapStartRequest } from "../../shared/heightmap";

describe("heightmap IPC contract", () => {
  it("uses the exact camelCase field names expected by Rust serde", () => {
    expect(["directSurface", "fixedPlate"]).toContain(
      startRequestFixture.plan.contactMode,
    );
    const request = startRequestFixture as HeightmapStartRequest;

    expect(request.plan.originXMm).toBe(-15.85);
    expect(request.plan.originYMm).toBe(-6.22);
    expect(request.plan.clearanceZMm).toBe(2);
    expect(Object.keys(request.plan)).not.toContain("originXmm");
    expect(Object.keys(request.plan)).not.toContain("clearanceZmm");
  });
});
