import { describe, expect, it } from "vitest";

import type { ProgramExecutionOptions } from "../../shared/realRun";
import {
  executionOptionsForNewProgram,
  sameExecutionOptions,
} from "./executionOptionsModel";

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

describe("executionOptionsForNewProgram", () => {
  it("does not carry a job-specific depth offset into a newly loaded file", () => {
    expect(executionOptionsForNewProgram(baseline)).toEqual({
      optionalStop: false,
      blockDelete: false,
      surfaceMapId: 7,
      cuttingDepthAdjustmentUm: undefined,
    });
  });
});
