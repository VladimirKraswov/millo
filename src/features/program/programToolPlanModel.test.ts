import { describe, expect, it } from "vitest";

import type { ProgramDocumentMetadata, ProgramLine } from "../../shared/program";
import {
  initialProgramToolNumber,
  programToolNumberAtSourceLine,
} from "./programToolPlanModel";

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

  it("tracks the selected tool at an executing source line", () => {
    const multiTool = program([
      "T1",
      "G1 X1 F100",
      "T2 M6",
      "G1 X2 F100",
    ]);

    expect(programToolNumberAtSourceLine(multiTool, 2)).toBe(1);
    expect(programToolNumberAtSourceLine(multiTool, 3)).toBe(2);
    expect(programToolNumberAtSourceLine(multiTool, 4)).toBe(2);
  });

  it("uses full native tool metadata beyond the first page, without guessing beyond its coverage", () => {
    const document: ProgramDocumentMetadata = {
      id: "large", sourceBytes: 10000, pageSize: 512, previewSampled: true,
      warningCount: 0, blockingWarningCount: 0, errorCount: 0, managedToolChangeCount: 2,
      initialToolNumber: 7, toolSelections: [{ sourceLine: 601, tool: 7 }, { sourceLine: 900000, tool: 2 }],
      toolSelectionCoverageLine: 1000000,
    };
    const large = { ...program(["(header)"]), document };
    expect(initialProgramToolNumber(large)).toBe(7);
    expect(programToolNumberAtSourceLine(large, 899999)).toBe(7);
    expect(programToolNumberAtSourceLine(large, 900000)).toBe(2);
    expect(programToolNumberAtSourceLine(large, 1000001)).toBeUndefined();
  });
});
