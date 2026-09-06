import { describe, expect, it } from "vitest";
import {
  buildProgramEditorLineIndex, editProgramEditorPage, indexedSourceLine,
  pageOffsetToSource, programEditorPageAtLine, sourceOffsetToPage,
} from "./programEditorPaging";

describe("programEditorPaging", () => {
  it("indexes a million lines compactly and renders a bounded arbitrary page", () => {
    const source = "G1 X1\r\n".repeat(1_000_000) + "M30";
    const offsets = buildProgramEditorLineIndex(source);
    expect(offsets.length).toBe(1_000_001);
    expect(offsets.byteLength).toBe(4_000_004);
    expect(indexedSourceLine(offsets, 6_999_997)).toBe(1_000_000);
    const page = programEditorPageAtLine(source, offsets, 999_999);
    expect(page.firstLine).toBe(995_001);
    expect(page.lastLine).toBe(1_000_000);
    expect(page.offsets.length).toBe(5_000);
    expect(page.text.length).toBeLessThan(40_000);
    expect(programEditorPageAtLine(source, offsets, 1_000_001).text).toBe("M30");
  });

  it("maps CRLF page cursor offsets to source and edits only the changed text", () => {
    const source = "G0 X0\r\n".repeat(5_001) + "M30\r\n";
    const offsets = buildProgramEditorLineIndex(source);
    const page = programEditorPageAtLine(source, offsets, 5_001);
    expect(page.text).toBe("G0 X0\nM30\n");
    expect(pageOffsetToSource(page, offsets, 6)).toBe(page.start + 7);
    expect(sourceOffsetToPage(page, offsets, page.start + 7)).toBe(6);
    const next = "G0 X9\nG1 X10\nM30\n";
    const edit = editProgramEditorPage(source, offsets, page, next, { start: 12, end: 12 });
    expect(edit.source).toBe("G0 X0\r\n".repeat(5_000) + "G0 X9\r\nG1 X10\r\nM30\r\n");
    expect(edit.selection).toEqual({ start: page.start + 13, end: page.start + 13 });
  });

  it("keeps page-boundary separators while inserting and deleting lines", () => {
    const source = "G1 X1\n".repeat(5_005);
    const offsets = buildProgramEditorLineIndex(source);
    const page = programEditorPageAtLine(source, offsets, 1);
    expect(page.text.endsWith("\n")).toBe(false);
    const edit = editProgramEditorPage(source, offsets, page, page.text + "\nG1 X2", {
      start: page.text.length + 6, end: page.text.length + 6,
    });
    expect(edit.source).toBe(source.slice(0, page.end) + "\nG1 X2" + source.slice(page.end));
    const nextOffsets = buildProgramEditorLineIndex(edit.source);
    expect(indexedSourceLine(nextOffsets, edit.selection.start)).toBe(5_001);
    expect(programEditorPageAtLine(edit.source, nextOffsets, 5_001).text.startsWith("G1 X2\nG1 X1")).toBe(true);
    const deleted = editProgramEditorPage(source, offsets, page, "", { start: 0, end: 0 });
    expect(deleted.source).toBe(source.slice(page.end));
  });

  it("handles empty sources, trailing blank lines and clamped jumps", () => {
    expect([...buildProgramEditorLineIndex("")]).toEqual([0]);
    const source = "G21\n\n";
    const offsets = buildProgramEditorLineIndex(source);
    expect([...offsets]).toEqual([0, 4, 5]);
    expect(programEditorPageAtLine(source, offsets, Infinity).text).toBe(source);
    expect(programEditorPageAtLine(source, offsets, -1).firstLine).toBe(1);
  });
});
