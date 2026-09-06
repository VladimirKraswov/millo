import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { emptySnapshot, type Position } from "../../shared/machine";
import { resolveWorkPosition } from "../work-zero/workPositionModel";
import { previewFixtureProgram } from "./previewFixtureProgram";
import { ToolpathPreview } from "./ToolpathPreview";

const renderPosition = (position: Position, rotaryProgram = false) => renderToStaticMarkup(
  <ToolpathPreview
    program={rotaryProgram ? {
      ...previewFixtureProgram,
      features: { ...previewFixtureProgram.features, usesRotaryA: true },
    } : previewFixtureProgram}
    toolPosition={position}
    toolVisualization={{ state: "selected", showCutter: true, spinning: false }}
    view="iso"
  />,
);

describe("ToolpathPreview rotary telemetry", () => {
  it("displays live controller A degrees carried through the existing work position", () => {
    const work = resolveWorkPosition({
      ...emptySnapshot,
      machine: {
        ...emptySnapshot.machine,
        machinePosition: { x: 105, y: 12, z: 8, a: -810.25 },
        workCoordinateOffset: { x: 100, y: 10, z: 5 },
      },
    });
    const markup = renderPosition(work.position!);
    expect(markup).toContain("Текущее положение оси A");
    expect(markup).toContain("A -810.250°");
    expect(markup).toContain("X 5.000");
    expect(markup).toContain("Y 2.000");
    expect(markup).toContain("Z 3.000");
    expect(markup).not.toContain("is-spinning");
  });

  it("shows zero A, but keeps a truly XYZ-only readout unchanged", () => {
    expect(renderPosition({ x: 0, y: 0, z: 0, a: 0 })).toContain("A 0.000°");
    const markup = renderPosition({ x: 0, y: 0, z: 0 });
    expect(markup).not.toContain("Текущее положение оси A");
    expect(markup).not.toContain("has-rotary");
  });

  it("marks unavailable or invalid A telemetry as unknown in a rotary program", () => {
    expect(renderPosition({ x: 0, y: 0, z: 0 }, true)).toContain("A --");
    expect(renderPosition({ x: 0, y: 0, z: 0, a: NaN }, true)).toContain("A --");
  });
});
