import { describe, expect, it } from "vitest";

import type { PcbInspection, PcbJobSettings } from "../../shared/jobs";
import type { CuttingTool } from "../../shared/tooling";
import {
  closestTool,
  drillMappings,
  inferPcbRole,
  isPcbDrillingTool,
  validatePcbWorkflow,
  type LocalPcbFile,
} from "./pcbModel";

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

const file = (role: LocalPcbFile["role"]): LocalPcbFile => ({
  role,
  sizeBytes: 1,
  sourceBase64: "AA==",
  sourceName: role === "drill" ? "board.drl" : "board.gbl",
});

const settings = (drillToolId = "drill-08"): PcbJobSettings => ({
  safeZMm: 3,
  surfaceZMm: 0,
  isolation: { enabled: false, toolId: "", depthMm: 0.08, clearanceMm: 0.05, passes: 1 },
  drilling: { enabled: true, depthMm: 1.8, mappings: [{ groupKey: "drill::T1", toolId: drillToolId }] },
  outline: { enabled: false, toolId: "", depthMm: 1.7, depthPerPassMm: 0.4, tabCount: 4, tabWidthMm: 2, tabHeightMm: 0.4 },
  marking: { enabled: false, toolId: "", depthMm: 0.04 },
});

const inspection: PcbInspection = {
  bounds: { maxXMm: 10, maxYMm: 10, minXMm: 0, minYMm: 0, widthMm: 10, heightMm: 10 },
  drillGroups: [{ key: "drill::T1", sourceName: "board.drl", sourceToolNumber: 1, diameterMm: 0.8, hitCount: 2 }],
  drillHits: [],
  files: [],
  paths: [],
  warnings: [],
};

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

  it("keeps the UI drilling choices aligned with the Rust job policy", () => {
    expect(isPcbDrillingTool(tool("flat", 0.8))).toBe(true);
    expect(isPcbDrillingTool({ ...tool("v-bit", 0.1), kind: "vBit" })).toBe(false);
  });

  it("asks for Excellon before asking for drill tool mappings", () => {
    expect(validatePcbWorkflow([file("copper")], inspection, settings(), [tool("drill-08", 0.8)]))
      .toBe("Для сверловки добавьте Excellon (.drl, .xln или .txt)");
  });

  it("accepts a mapped drill group and names a missing mapping precisely", () => {
    const tools = [tool("drill-08", 0.8)];
    expect(validatePcbWorkflow([file("copper"), file("drill")], inspection, settings(), tools))
      .toBeUndefined();
    expect(validatePcbWorkflow([file("copper"), file("drill")], inspection, settings(""), tools))
      .toBe("Выберите сверло для каждой группы отверстий");
  });
});
