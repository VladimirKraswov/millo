import { describe, expect, it } from "vitest";

import type { PcbInspection, PcbJobSettings } from "../../shared/jobs";
import type { CuttingTool } from "../../shared/tooling";
import {
  closestTool,
  drillMappings,
  inferPcbRole,
  initialPcbOperations,
  isPcbDrillingTool,
  readablePcbError,
  toolsForDrillGroup,
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
  drillGroups: [{ key: "drill::T1", sourceName: "board.drl", sourceToolNumber: 1, diameterMm: 0.8, hitCount: 2, slotCount: 0 }],
  drillHits: [],
  drillSlots: [],
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
    expect(inferPcbRole("board.gts")).toBe("ignore");
    expect(inferPcbRole("generic.gbr", "%TF.FileFunction,Profile,NP*%")).toBe("outline");
    expect(inferPcbRole("generic.gbr", "%TF.FileFunction,Plated,1,2,PTH*%")).toBe("drill");
    expect(inferPcbRole("generic.gbr", "%TF.FileFunction,Soldermask,Top*%")).toBe("ignore");
    expect(inferPcbRole("generic.txt", "M48\nMETRIC,LZ\nT1C0.8\n%")).toBe("drill");
    expect(inferPcbRole("generic.txt", "; CAD export\nM48\nMETRIC,LZ\nT1C0.8\n%")).toBe("drill");
  });

  it("keeps an explicit drill mapping and otherwise chooses the nearest diameter", () => {
    const tools = [tool("small", 0.8), tool("large", 1.2)];
    expect(closestTool(tools, 1.1)?.id).toBe("large");
    const mappings = drillMappings([
      { key: "drill::T1", sourceName: "drill.drl", sourceToolNumber: 1, diameterMm: 0.8, hitCount: 2, slotCount: 0 },
      { key: "drill::T2", sourceName: "drill.drl", sourceToolNumber: 2, diameterMm: 1.2, hitCount: 1, slotCount: 0 },
    ], tools, new Map([["drill::T1", "small"]]));
    expect(mappings.get("drill::T1")).toBe("small");
    expect(mappings.get("drill::T2")).toBe("large");
  });

  it("keeps the UI drilling choices aligned with the Rust job policy", () => {
    expect(isPcbDrillingTool(tool("flat", 0.8))).toBe(true);
    expect(isPcbDrillingTool({ ...tool("v-bit", 0.1), kind: "vBit" })).toBe(false);
  });

  it("uses a milling tool no wider than an Excellon slot", () => {
    const group = { key: "drill::T3", sourceName: "drill.drl", sourceToolNumber: 3, diameterMm: 1, hitCount: 0, slotCount: 1 };
    const flat = tool("flat", 0.8);
    const wide = tool("wide", 1.2);
    const drill = { ...tool("drill", 1), kind: "drill" as const };
    expect(toolsForDrillGroup(group, [flat, wide, drill]).map((candidate) => candidate.id)).toEqual(["flat"]);
  });

  it("does not turn a loaded outline into an implicit cutting operation", () => {
    const initial = { ...settings(), drilling: { ...settings().drilling, enabled: false } };
    const configured = initialPcbOperations(initial, [file("copper"), file("drill"), file("outline")]);
    expect(configured.isolation.enabled).toBe(true);
    expect(configured.drilling.enabled).toBe(true);
    expect(configured.outline.enabled).toBe(false);
    expect(configured.marking.enabled).toBe(false);
  });

  it("asks for a drill source before asking for tool mappings", () => {
    expect(validatePcbWorkflow([file("copper")], inspection, settings(), [tool("drill-08", 0.8)]))
      .toBe("Для сверловки добавьте Excellon или Gerber X2 drill");
  });

  it("accepts a mapped drill group and names a missing mapping precisely", () => {
    const tools = [tool("drill-08", 0.8)];
    expect(validatePcbWorkflow([file("copper"), file("drill")], inspection, settings(), tools))
      .toBeUndefined();
    expect(validatePcbWorkflow([file("copper"), file("drill")], inspection, settings(""), tools))
      .toBe("Выберите инструмент для каждой группы отверстий и пазов");
  });

  it("turns parser failures into actionable operator messages", () => {
    expect(readablePcbError(new Error(
      "Gerber file board.gbr uses unsupported feature: incremental coordinates",
    ))).toBe("Gerber «board.gbr»: экспортируйте координаты в абсолютном режиме");
    expect(readablePcbError(new Error(
      "Excellon file holes.drl is invalid: missing unit",
    ))).toBe("Не удалось прочитать файл сверловки «holes.drl»: missing unit");
  });
});
