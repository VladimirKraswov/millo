import { describe, expect, it } from "vitest";

import type { GcodeProgram, ProgramWarning } from "../../shared/program";
import { previewFixtureFirstCutProgram } from "./previewFixtureFirstCut";
import {
  formatProgramDiagnostics,
  hasActionableProgramWarnings,
  programCanEnterPreflight,
  programDiagnosticsSummary,
  programWarningPresentation,
} from "./programDiagnosticsModel";

const toolChange: ProgramWarning = {
  sourceLine: 10,
  severity: "safety",
  code: "tool-change",
  message: "M6 requires a host-managed operator tool-change barrier",
};

function withWarnings(
  warnings: readonly ProgramWarning[],
  previewComplete = true,
): GcodeProgram {
  return {
    ...previewFixtureFirstCutProgram,
    warnings,
    summary: { ...previewFixtureFirstCutProgram.summary, previewComplete },
  };
}

describe("program diagnostics model", () => {
  it("treats host-managed M6 as an expected event, not a file defect", () => {
    const program = withWarnings([toolChange]);

    expect(programCanEnterPreflight(program)).toBe(true);
    expect(hasActionableProgramWarnings(program)).toBe(false);
    expect(programDiagnosticsSummary(program)).toEqual({
      actionableCount: 0,
      managedToolChangeCount: 1,
      totalCount: 1,
    });
    expect(formatProgramDiagnostics(programDiagnosticsSummary(program))).toBe(
      "1 смена инструмента",
    );
    expect(programWarningPresentation(toolChange)).toMatchObject({
      kind: "managed",
      title: "Смена инструмента",
    });
  });

  it("keeps parser errors actionable and blocks incomplete previews", () => {
    const error: ProgramWarning = {
      sourceLine: 4,
      severity: "error",
      code: "invalid-token",
      message: "invalid token",
    };
    const program = withWarnings([toolChange, error], false);

    expect(programCanEnterPreflight(program)).toBe(false);
    expect(hasActionableProgramWarnings(program)).toBe(true);
    expect(formatProgramDiagnostics(programDiagnosticsSummary(program))).toBe(
      "1 замечание · 1 смена инструмента",
    );
  });
});
