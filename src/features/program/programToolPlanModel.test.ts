import { describe, expect, it } from "vitest";

import type { ProgramLine } from "../../shared/program";
import { initialProgramToolNumber } from "./programToolPlanModel";

const program = (normalized: readonly string[]) => ({
  lines: normalized.map((source, index): ProgramLine => ({
    executable: true,
    normalized: source,
    source,
    sourceLine: index + 1,
    warningCount: 0,
  })),
});

describe("program tool plan model", () => {
  it("finds the tool selected for the first cutting operation", () => {
    expect(initialProgramToolNumber(program([
      "G21 G90 G94",
      "G0 Z3",
      "T1 M6",
      "G1 X1 F100",
    ]))).toBe(1);
    expect(initialProgramToolNumber(program(["T7", "G0 X5", "G1 Z-0.1 F50"]))).toBe(7);
  });

  it("does not present a later tool as the startup tool", () => {
    expect(initialProgramToolNumber(program(["G1 X1 F100", "T2 M6"]))).toBeUndefined();
  });
});
