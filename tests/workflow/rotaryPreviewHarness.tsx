import { useState } from "react";
import { createRoot } from "react-dom/client";
import { ProgramPreviewStage } from "../../src/features/program/ProgramPreviewStage";
import { buildToolpathReadModel } from "../../src/features/program/toolpathReadModel";
import { previewFixtureProgram } from "../../src/features/program/previewFixtureProgram";
import type { GcodeProgram } from "../../src/shared/program";
import "../../src/styles.css";
import "../../src/app/workspace/workspace.css";

const anchor = { x: 10, y: 23, z: 0 };
const toolpath: GcodeProgram["toolpath"] = [
  ...previewFixtureProgram.toolpath,
  { sourceLine: 99, kind: "linear", points: [anchor, anchor], distanceMm: 0,
    rotary: { startDegrees: -90, endDegrees: 720 } },
];
export let toolpathReads = 0;
const program: GcodeProgram = {
  ...previewFixtureProgram,
  document: { id: "sampled", sourceBytes: 4_000_000, pageSize: 200,
    previewSampled: true, warningCount: 0, blockingWarningCount: 0, toolSelections: [],
    errorCount: 0, managedToolChangeCount: 0, toolSelectionCoverageLine: 0 },
  features: { ...previewFixtureProgram.features, usesRotaryA: true },
  lines: [...previewFixtureProgram.lines, {
    sourceLine: 99, source: "G1 A720 F120", normalized: "G1 A720 F120",
    executable: true, warningCount: 0,
  }, { sourceLine: 100, source: "G1 X18 Y12 A1080", normalized: "G1 X18 Y12 A1080",
    executable: true, warningCount: 0 }],
  summary: { ...previewFixtureProgram.summary,
    bounds: { min: { x: 0, y: 0, z: 0 }, max: { x: 20, y: 23, z: 4 }, size: { x: 20, y: 23, z: 4 } },
    rotaryBounds: { minDegrees: -90, maxDegrees: 720, sizeDegrees: 810 }, rotaryTravelDegrees: 1080,
  },
  get toolpath() { toolpathReads += 1; return toolpath; },
};
const model = buildToolpathReadModel(program);
export let updateAngle: (angle: number | undefined) => void;
export let changeView: () => void;
export let selectDetail: () => void;

function Preview() {
  const [angle, setAngle] = useState<number | undefined>(-810.25);
  const [view, setView] = useState<"top" | "iso">("top");
  const [selected, setSelected] = useState<number>();
  const [detail, setDetail] = useState<GcodeProgram["toolpath"]>();
  updateAngle = setAngle;
  changeView = () => setView((current) => current === "top" ? "iso" : "top");
  selectDetail = () => {
    setSelected(100);
    setDetail([{ sourceLine: 100, kind: "linear", distanceMm: 22,
      points: [{ x: 0, y: 0, z: 0 }, { x: 18, y: 12, z: 0 }],
      rotary: { startDegrees: 720, endDegrees: 1080 } }]);
  };
  return <main className="workstation" style={{ padding: 12, display: "block", height: "auto", minHeight: 0 }}>
    <div style={{ height: 540, display: "grid" }}>
      <ProgramPreviewStage
        cuttingDepthAdjustmentMm={0} onClearSelection={() => setSelected(undefined)}
        onSafeStart={() => {}} onSelectSourceLine={setSelected} program={program}
        safeStartAvailable={false} selectedMotionCount={selected === undefined ? 0 : 1}
        selectedProgramLine={program.lines.find((line) => line.sourceLine === selected)}
        selectedSourceLine={selected} toolPosition={{ x: 2, y: 2, z: 2, a: angle }}
        selectedToolpath={detail}
        toolVisualization={{ state: "selected", showCutter: true, spinning: false }} view={view}
      />
    </div>
  </main>;
}

export function mount() {
  const host = document.createElement("div");
  document.body.replaceChildren(host);
  createRoot(host).render(<Preview />);
}

export function markerPosition() {
  const canvas = document.querySelector(".toolpath-canvas canvas")!;
  const rect = canvas.getBoundingClientRect();
  const aspect = rect.width / rect.height;
  const halfHeight = aspect >= 1 ? model.frameRadius : model.frameRadius / aspect;
  return { x: rect.x + rect.width / 2,
    y: rect.y + rect.height / 2 * (1 - (anchor.y - model.center.y) / halfHeight) };
}
