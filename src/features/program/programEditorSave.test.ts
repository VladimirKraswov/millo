import { describe, expect, it, vi } from "vitest";
import { previewFixtureProgram } from "./previewFixtureProgram";
import { buildProcessedProgramSource } from "./programEditorModel";
import { saveProgramEditorDocument } from "./programEditorSave";

const program = {
  ...previewFixtureProgram,
  document: { id: "full-document", sourceBytes: 8_000_000, pageSize: 512, previewSampled: true,
    warningCount: 0, blockingWarningCount: 0, errorCount: 0, managedToolChangeCount: 0,
    toolSelections: [], toolSelectionCoverageLine: 0 },
};

describe("programEditorSave", () => {
  it("uses the native full-document transform instead of the 512 display rows", async () => {
    const saveProcessed = vi.fn(async () => ({ path: "part.nc", bytesWritten: 8_000_000 }));
    const save = vi.fn();
    const result = await saveProgramEditorDocument({ parse: vi.fn(), save, saveProcessed }, { program, source: "FULL SOURCE" }, true);
    expect(saveProcessed).toHaveBeenCalledWith(
      { programId: "full-document", sourceName: program.sourceName, source: "FULL SOURCE", parseOptions: { blockDelete: false } },
      "preview-fixture-transformed.nc",
    );
    expect(save).not.toHaveBeenCalled();
    expect(result?.bytesWritten).toBe(8_000_000);
  });

  it("fails closed when native processed save is unavailable", async () => {
    const save = vi.fn();
    await expect(saveProgramEditorDocument({ parse: vi.fn(), save }, { program, source: "FULL" }, true))
      .rejects.toThrow("полной обработанной копии");
    expect(save).not.toHaveBeenCalled();
    expect(() => buildProcessedProgramSource(program)).toThrow("постраничной");
  });

  it("carries the parsed block-delete policy for expired native document recovery", async () => {
    const saveProcessed = vi.fn(async () => undefined);
    await saveProgramEditorDocument({ parse: vi.fn(), saveProcessed }, {
      program: { ...program, blockDeleteEnabled: true }, source: "/G1 X10\nG1 X20",
    }, true);
    expect(saveProcessed).toHaveBeenCalledWith({
      programId: "full-document", sourceName: program.sourceName,
      source: "/G1 X10\nG1 X20", parseOptions: { blockDelete: true },
    }, "preview-fixture-transformed.nc");
  });

  it("normal saves retain the entire source, including text outside the visible page", async () => {
    const source = "G1 X1\r\n".repeat(1_000_000) + "M30";
    const save = vi.fn(async () => undefined);
    await saveProgramEditorDocument({ parse: vi.fn(), save }, { program, source }, false);
    expect(save).toHaveBeenCalledWith({ sourceName: program.sourceName, source });
    expect(save.mock.calls).toHaveLength(1);
  });
});
