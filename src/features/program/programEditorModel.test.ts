import { describe, expect, it } from "vitest";

import { previewFixtureProgram } from "./previewFixtureProgram";
import {
  PROGRAM_EDITOR_HISTORY_LIMIT,
  buildProcessedProgramSource,
  commitProgramEditorSource,
  createProgramEditorHistory,
  deleteProgramLines,
  insertProgramLine,
  processedProgramName,
  redoProgramEditorSource,
  selectedLineSpan,
  selectedTextOrLines,
  sourceLineAtOffset,
  sourceOffsetAtLine,
  tokenizeGcodeLine,
  undoProgramEditorSource,
} from "./programEditorModel";

describe("programEditorModel", () => {
  it("keeps bounded immutable undo and redo history", () => {
    let history = createProgramEditorHistory("G0 X0");
    history = commitProgramEditorSource(history, "G0 X1");
    history = commitProgramEditorSource(history, "G0 X2");

    history = undoProgramEditorSource(history);
    expect(history.source).toBe("G0 X1");
    history = undoProgramEditorSource(history);
    expect(history.source).toBe("G0 X0");
    history = redoProgramEditorSource(history);
    expect(history.source).toBe("G0 X1");

    for (let index = 0; index < PROGRAM_EDITOR_HISTORY_LIMIT + 20; index += 1) {
      history = commitProgramEditorSource(history, `G0 X${index}`);
    }
    expect(history.past).toHaveLength(PROGRAM_EDITOR_HISTORY_LIMIT);
  });

  it("finds whole selected rows and supports insert, delete, and line copy", () => {
    const source = "G21\nG0 X0\nG1 X10";
    expect(selectedLineSpan(source, { start: 5, end: 9 })).toEqual({
      start: 4,
      end: 10,
    });
    expect(selectedTextOrLines(source, { start: 6, end: 6 }).text).toBe("G0 X0\n");
    expect(insertProgramLine(source, { start: 12, end: 12 })).toEqual({
      source: "G21\nG0 X0\n\nG1 X10",
      selection: { start: 10, end: 10 },
    });
    expect(deleteProgramLines(source, { start: 5, end: 9 })).toEqual({
      source: "G21\nG1 X10",
      selection: { start: 4, end: 4 },
    });
    expect(deleteProgramLines("\nG1 X10", { start: 0, end: 0 })).toEqual({
      source: "G1 X10",
      selection: { start: 0, end: 0 },
    });
  });

  it("maps caret offsets to stable one-based source lines", () => {
    const source = "G21\nG0 X0\nG1 X10";
    expect(sourceLineAtOffset(source, 0)).toBe(1);
    expect(sourceLineAtOffset(source, 8)).toBe(2);
    expect(sourceOffsetAtLine(source, 3)).toBe(10);
    expect(sourceOffsetAtLine(source, 200)).toBe(source.length);
  });

  it("classifies G-code words, coordinates, parameters, and comments", () => {
    expect(tokenizeGcodeLine("/N10 G1 X2.5 Y-1 F300 ; cut")).toEqual([
      { kind: "optional", text: "/" },
      { kind: "line-number", text: "N10" },
      { kind: "plain", text: " " },
      { kind: "command", text: "G1" },
      { kind: "plain", text: " " },
      { kind: "axis", text: "X2.5" },
      { kind: "plain", text: " " },
      { kind: "axis", text: "Y-1" },
      { kind: "plain", text: " " },
      { kind: "parameter", text: "F300" },
      { kind: "plain", text: " " },
      { kind: "comment", text: "; cut" },
    ]);
    expect(tokenizeGcodeLine("G2 X1 I.5 J0 (arc)").map((token) => token.kind)).toEqual([
      "command",
      "plain",
      "axis",
      "plain",
      "arc",
      "plain",
      "arc",
      "plain",
      "comment",
    ]);
  });

  it("builds a deterministic processed copy and a non-destructive file name", () => {
    const program = {
      ...previewFixtureProgram,
      lines: [
        { ...previewFixtureProgram.lines[0], normalized: "G21 G90" },
        {
          ...previewFixtureProgram.lines[1],
          normalized: "G0 X10",
          blockDeleted: true,
        },
        { ...previewFixtureProgram.lines[2], normalized: "G1 X20 F100" },
      ],
    };
    expect(buildProcessedProgramSource(program)).toBe("G21 G90\nG1 X20 F100\n");
    expect(processedProgramName("part.ngc")).toBe("part-transformed.ngc");
    expect(processedProgramName("program")).toBe("program-transformed.nc");
  });
});
