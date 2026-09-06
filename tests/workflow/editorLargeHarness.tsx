import { createRoot } from "react-dom/client";
import { DialogHost } from "../../src/components/DialogSurface";
import { ProgramEditor } from "../../src/features/program/ProgramEditor";
import { buildProgramEditorLineIndex } from "../../src/features/program/programEditorPaging";
import { previewFixtureProgram } from "../../src/features/program/previewFixtureProgram";
import type { GcodeProgram, ProgramParseRequest } from "../../src/shared/program";
import type { LoadedProgram } from "../../src/features/program/ProgramLoader";
import "../../src/styles.css";
import "../../src/app/workspace/workspace.css";

export const originalSource = "G1 X1\n".repeat(1_000_000) + "M30";
export let currentSource = originalSource;
export let savedSource: string | undefined;
export let processedRequest: ProgramParseRequest | undefined;
export let processedSourceName: string | undefined;
export let detailRequest: ProgramParseRequest | undefined;
export let detailLine: number | undefined;
export let applied: LoadedProgram | undefined;
let revision = 0;

function parsed(source: string): GcodeProgram {
  const count = buildProgramEditorLineIndex(source).length;
  const lines = source.slice(0, 3072).split("\n").slice(0, 512).map((line, index) => ({
    sourceLine: index + 1, source: line, normalized: line, executable: true, warningCount: 0,
  }));
  return {
    ...previewFixtureProgram,
    sourceName: "million-lines.nc", lines,
    document: { id: `revision-${revision++}`, sourceBytes: source.length, pageSize: 512, previewSampled: true,
      warningCount: 0, blockingWarningCount: 0, errorCount: 0, managedToolChangeCount: 0,
      toolSelections: [], toolSelectionCoverageLine: count },
    summary: { ...previewFixtureProgram.summary, lineCount: count },
  };
}

export function mount(processedCapability = true) {
  const host = document.createElement("div");
  document.body.replaceChildren(host);
  createRoot(host).render(<DialogHost><ProgramEditor blockDelete={false}
    document={{ program: parsed(originalSource), source: originalSource }}
    gateway={{
      parse: async ({ source }) => { currentSource = source; return parsed(source); },
      save: async ({ source }) => { savedSource = source; return { path: "source.nc", bytesWritten: source.length }; },
      lineDetail: async (request, sourceLine) => {
        detailRequest = request; detailLine = sourceLine;
        return { programId: request.programId!, toolpath: [],
          line: { sourceLine, source: "G1 X1", normalized: "G1 X1", executable: true, warningCount: 0 } };
      },
      ...(processedCapability ? { saveProcessed: async (request: ProgramParseRequest, sourceName: string) => {
        processedRequest = request; processedSourceName = sourceName;
        return { path: "processed.nc", bytesWritten: currentSource.length };
      } } : {}),
    }} onApply={(document) => { applied = document; }} onClose={() => {}} />
  </DialogHost>);
}
