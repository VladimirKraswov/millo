import { describe, expect, it } from "vitest";

import type { ProgramExecutionOptions } from "../../shared/realRun";
import { sameExecutionOptions } from "./executionOptionsModel";

const baseline: ProgramExecutionOptions = {
  optionalStop: false,
  blockDelete: false,
  surfaceMapId: 7,
  cuttingDepthAdjustmentUm: -100,
};

describe("sameExecutionOptions", () => {
  it("accepts an exact execution contract", () => {
    expect(sameExecutionOptions(baseline, { ...baseline })).toBe(true);
  });

  it.each([
    { optionalStop: true },
    { blockDelete: true },
    { surfaceMapId: 8 },
    { cuttingDepthAdjustmentUm: 100 },
  ])("rejects a changed bound option: %o", (change) => {
    expect(sameExecutionOptions(baseline, { ...baseline, ...change })).toBe(false);
  });
});
