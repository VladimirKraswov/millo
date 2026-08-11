import { describe, expect, it } from "vitest";

import type { ProgramLine } from "../../shared/program";
import {
  PROGRAM_LINE_ROW_HEIGHT,
  buildProgramLineWindow,
  findProgramLineIndex,
} from "./programLineTableModel";

const lines = Array.from({ length: 1_000 }, (_, index): ProgramLine => ({
  sourceLine: index + 1,
  source: `G1 X${index}`,
  normalized: `G1 X${index}`,
  executable: true,
  warningCount: 0,
}));

describe("programLineTableModel", () => {
  it("keeps a large program in a small overscanned DOM window", () => {
    const window = buildProgramLineWindow(
      lines,
      PROGRAM_LINE_ROW_HEIGHT * 50,
      PROGRAM_LINE_ROW_HEIGHT * 3,
    );

    expect(window.startIndex).toBe(46);
    expect(window.endIndex).toBe(57);
    expect(window.lines).toHaveLength(11);
    expect(window.offsetPx).toBe(PROGRAM_LINE_ROW_HEIGHT * 46);
    expect(window.totalHeightPx).toBe(PROGRAM_LINE_ROW_HEIGHT * 1_000);
  });

  it("clamps invalid and bottom-edge scroll positions", () => {
    expect(buildProgramLineWindow(lines, Number.NaN, 0).startIndex).toBe(0);
    const bottom = buildProgramLineWindow(lines, Number.MAX_VALUE, 100);
    expect(bottom.endIndex).toBe(lines.length);
    expect(bottom.lines.length).toBeLessThanOrEqual(8);
  });

  it("finds sorted source lines without assuming a zero-based line number", () => {
    expect(findProgramLineIndex(lines, 1)).toBe(0);
    expect(findProgramLineIndex(lines, 700)).toBe(699);
    expect(findProgramLineIndex(lines, 1_001)).toBeUndefined();
    expect(findProgramLineIndex(lines, undefined)).toBeUndefined();
  });
});
