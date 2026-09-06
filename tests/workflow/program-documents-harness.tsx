import { useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import { PagedProgramLineTable } from "../../src/features/program/PagedProgramLineTable";
import { ProgramPreviewStage } from "../../src/features/program/ProgramPreviewStage";
import { previewFixtureProgram } from "../../src/features/program/previewFixtureProgram";
import { useProgramSelection } from "../../src/features/program/useProgramSelection";
import type { ProgramGateway } from "../../src/platform/program/ProgramGateway";
import type { GcodeProgram, ProgramLine, ProgramParseRequest, ToolpathSegment } from "../../src/shared/program";
import "../../src/styles.css";
import "../../src/app/workspace/workspace.css";
import "./program-documents-harness.css";

export const totalLines = 1_000_000;
const pageSize = 512;

function createDocument(id: "a" | "b") {
  const shift = id === "a" ? 0 : 2;
  const degrees = id === "a" ? 810 : -270;
  const corners = [
    { x: shift, y: shift, z: 0 }, { x: 20 + shift, y: shift, z: 0 },
    { x: 20 + shift, y: 15 + shift, z: 0 }, { x: shift, y: 15 + shift, z: 0 },
  ];
  const rows = corners.map((point) => `G1 X${point.x} Y${point.y} Z0 F240`);
  const block = `${rows.join("\n")}\n`;
  const offsets = rows.map((_, index) => rows.slice(0, index).reduce((sum, row) => sum + row.length + 1, 0));
  const prefix = block.repeat(Math.floor((totalLines - 1) / rows.length)) + `${rows.slice(0, 3).join("\n")}\n`;
  const lastSource = id === "a"
    ? "N1000000 G1 X18 Y12 Z-1 A810 F240 ; END-A"
    : "N1000000 G1 X2 Y14 Z-2 A-270 F120 ; END-B";
  const source = prefix + lastSource;
  // Address actual full-source slices without allocating a million-line array.
  const lineAt = (sourceLine: number): ProgramLine => {
    if (sourceLine < 1 || sourceLine > totalLines) throw new Error("Line out of range");
    const index = sourceLine - 1;
    const start = sourceLine === totalLines ? prefix.length : Math.floor(index / 4) * block.length + offsets[index % 4];
    const end = source.indexOf("\n", start);
    const raw = source.slice(start, end < 0 ? source.length : end);
    return { sourceLine, source: raw, normalized: raw.split(";")[0].trim(), executable: true, warningCount: 0 };
  };
  const endpoint = id === "a" ? { x: 18, y: 12, z: -1 } : { x: 2, y: 14, z: -2 };
  const detail: ToolpathSegment = {
    sourceLine: totalLines, kind: "linear", points: [corners[2], endpoint],
    distanceMm: Math.hypot(endpoint.x - corners[2].x, endpoint.y - corners[2].y, endpoint.z),
    rotary: { startDegrees: 0, endDegrees: degrees },
  };
  const program: GcodeProgram = {
    ...previewFixtureProgram, sourceName: `${id}-million.nc`, blockDeleteEnabled: id === "a",
    document: { id: `document-${id}`, sourceBytes: source.length, pageSize, previewSampled: true,
      warningCount: 0, blockingWarningCount: 0, errorCount: 0, managedToolChangeCount: 0,
      toolSelections: [], toolSelectionCoverageLine: totalLines },
    lines: Array.from({ length: pageSize }, (_, index) => lineAt(index + 1)), warnings: [],
    features: { ...previewFixtureProgram.features, hasSpindleActivation: false, usesRotaryA: true },
    summary: { ...previewFixtureProgram.summary, lineCount: totalLines,
      executableLineCount: totalLines, motionCount: totalLines - 1,
      rapidDistanceMm: 0, cuttingDistanceMm: 249999 * 70 + 35 + detail.distanceMm,
      bounds: { min: { x: shift, y: shift, z: endpoint.z }, max: corners[2],
        size: { x: 20, y: 15, z: -endpoint.z } },
      rotaryBounds: { minDegrees: Math.min(0, degrees), maxDegrees: Math.max(0, degrees), sizeDegrees: Math.abs(degrees) },
      rotaryTravelDegrees: Math.abs(degrees),
    },
    toolpath: corners.map((point, index): ToolpathSegment => ({
      sourceLine: index + 2, kind: "linear", points: [point, corners[(index + 1) % 4]],
      distanceMm: index % 2 === 0 ? 20 : 15,
    })),
  };
  return { program, source, lineAt, detail, lastSource };
}

const documents = { a: createDocument("a"), b: createDocument("b") };
type RequestKind = "page" | "detail";
export const requests: { kind: RequestKind; programId: string; sourceMatches: boolean; sourceLength: number;
  blockDelete: boolean | undefined; startIndex?: number; count?: number; sourceLine?: number }[] = [];
const pending: { kind: RequestKind; resolve: () => void; reject: (error: Error) => void }[] = [];
let held: "all" | "detail" | undefined;

function documentFor(request: ProgramParseRequest, kind: RequestKind, range: { startIndex?: number; count?: number; sourceLine?: number }) {
  const doc = request.programId === documents.a.program.document!.id ? documents.a : documents.b;
  const sourceMatches = request.source === doc.source;
  requests.push({ kind, programId: request.programId ?? "missing", sourceMatches,
    sourceLength: request.source.length, blockDelete: request.parseOptions?.blockDelete, ...range });
  if (!sourceMatches || request.sourceName !== doc.program.sourceName || request.programId !== doc.program.document!.id
    || request.parseOptions?.blockDelete !== doc.program.blockDeleteEnabled) throw new Error("Invalid native document recovery request");
  return doc;
}

function respond<T>(id: string, kind: RequestKind, result: T): Promise<T> {
  if (id !== "document-a" || !(held === "all" || held === kind)) return Promise.resolve(result);
  return new Promise<T>((resolve, reject) => pending.push({ kind, resolve: () => resolve(result), reject }));
}

const gateway: ProgramGateway = {
  async parse() { throw new Error("The document browser must not reparse source"); },
  linePage(request, startIndex, count) {
    const doc = documentFor(request, "page", { startIndex, count });
    if (count > pageSize) throw new Error("Unbounded page request");
    return respond(doc.program.document!.id, "page", { programId: doc.program.document!.id, startIndex, totalLines,
      lines: Array.from({ length: Math.min(count, totalLines - startIndex) }, (_, index) => doc.lineAt(startIndex + index + 1)) });
  },
  lineDetail(request, sourceLine) {
    const doc = documentFor(request, "detail", { sourceLine });
    return respond(doc.program.document!.id, "detail", { programId: doc.program.document!.id, line: doc.lineAt(sourceLine),
      toolpath: sourceLine === totalLines ? [doc.detail] : [] });
  },
};

export function holdRequests(kind: "all" | "detail") { held = kind; }
export function pendingCounts() {
  return { page: pending.filter((request) => request.kind === "page").length,
    detail: pending.filter((request) => request.kind === "detail").length };
}
export async function flushHeld(mode: "success" | "mixed") {
  held = undefined;
  const seen = new Set<RequestKind>();
  for (const request of pending.splice(0)) {
    if (mode === "mixed" && seen.has(request.kind)) request.reject(new Error(`STALE A ${request.kind}`));
    else request.resolve();
    seen.add(request.kind);
  }
  await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
}

export let replaceDocument: () => void;
export let snapshot: { programId: string; sourceLine?: number; line?: ProgramLine; toolpath?: readonly ToolpathSegment[] };

function DocumentBrowser() {
  const [key, setKey] = useState<"a" | "b">("a");
  const [selected, setSelected] = useState<number>();
  const doc = documents[key];
  const selection = useProgramSelection(doc.program, selected, gateway, doc.source);
  const motionSourceLines = useMemo(() => new Set(selection.sourceIndex!.motions.keys()), [selection.sourceIndex]);
  snapshot = { programId: doc.program.document!.id, sourceLine: selected,
    line: selection.selectedProgramLine, toolpath: selection.selectedToolpath };
  replaceDocument = () => { setSelected(undefined); setKey("b"); };
  return <main className="workstation program-documents-fixture">
    <h1>{doc.program.sourceName}</h1>
    <div className="program-documents-layout">
      <div className="program-documents-scene">
        <ProgramPreviewStage program={doc.program} cuttingDepthAdjustmentMm={0}
          onClearSelection={() => setSelected(undefined)} onSafeStart={() => {}}
          onSelectSourceLine={setSelected} safeStartAvailable={false}
          selectedSourceLine={selected} selectedProgramLine={selection.selectedProgramLine}
          selectedMotionCount={selection.selectedToolpath?.length ?? 0} selectedToolpath={selection.selectedToolpath}
          toolPosition={{ x: 2, y: 2, z: 2, a: 35 }}
          toolVisualization={{ state: "removed", showCutter: false, spinning: false }} view="iso" />
      </div>
      <section className="program-lines-panel program-documents-table" aria-label="Исходный документ">
        <PagedProgramLineTable program={doc.program} source={doc.source} gateway={gateway}
          motionSourceLines={motionSourceLines} selectedSourceLine={selected} onSelect={setSelected} />
      </section>
    </div>
    {selection.selectionError && <output role="alert">{selection.selectionError}</output>}
  </main>;
}

export function mount() {
  const host = document.createElement("div");
  document.body.replaceChildren(host);
  createRoot(host).render(<DocumentBrowser />);
}
