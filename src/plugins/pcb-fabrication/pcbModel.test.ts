import { describe, expect, it } from "vitest";

import type { CuttingTool } from "../../shared/tooling";
import { closestTool, drillMappings, inferPcbRole } from "./pcbModel";

const tool = (id: string, diameterMm: number): CuttingTool => ({
  id,
  name: id,
  description: id,
  kind: "flatEndMill",
  diameterMm,
  shankDiameterMm: diameterMm,
  cuttingLengthMm: 2,
  fluteCount: 1,
  feedMmPerMin: 100,
  plungeMmPerMin: 50,
  spindleRpm: 10_000,
  stepdownMm: 0.2,
  stepoverPercent: 30,
  factoryPreset: false,
});

describe("PCB plugin model", () => {
  it("infers common EasyEDA layer roles", () => {
    expect(inferPcbRole("board.GTL")).toBe("copper");
    expect(inferPcbRole("board.gko")).toBe("outline");
    expect(inferPcbRole("board.drl")).toBe("drill");
    expect(inferPcbRole("board.gto")).toBe("marking");
  });

  it("keeps an explicit drill mapping and otherwise chooses the nearest diameter", () => {
    const tools = [tool("small", 0.8), tool("large", 1.2)];
    expect(closestTool(tools, 1.1)?.id).toBe("large");
    const mappings = drillMappings([
      { key: "drill::T1", sourceName: "drill.drl", sourceToolNumber: 1, diameterMm: 0.8, hitCount: 2 },
      { key: "drill::T2", sourceName: "drill.drl", sourceToolNumber: 2, diameterMm: 1.2, hitCount: 1 },
    ], tools, new Map([["drill::T1", "large"]]));
    expect(mappings.get("drill::T1")).toBe("large");
    expect(mappings.get("drill::T2")).toBe("large");
  });
});
