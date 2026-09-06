import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { GcodeProgram } from "../../shared/program";

import { previewFixtureFirstCutProgram } from "./previewFixtureFirstCut";
import { ProgramPreviewStage } from "./ProgramPreviewStage";

describe("ProgramPreviewStage", () => {
  it("renders stable program metrics and the selected safe-start context", () => {
    const selectedProgramLine = previewFixtureFirstCutProgram.lines[3];
    const markup = renderToStaticMarkup(
      <ProgramPreviewStage
        cuttingDepthAdjustmentMm={-0.1}
        onClearSelection={() => undefined}
        onSafeStart={() => undefined}
        onSelectSourceLine={() => undefined}
        program={previewFixtureFirstCutProgram}
        safeStartAvailable
        selectedMotionCount={1}
        selectedProgramLine={selectedProgramLine}
        selectedSourceLine={selectedProgramLine?.sourceLine}
        toolVisualization={{
          state: "selected",
          showCutter: true,
          spinning: false,
        }}
        view="iso"
      />,
    );

    expect(markup).toContain("Загрузка траектории");
    expect(markup).toContain("С этого участка");
    expect(markup).toContain("1 сегмент траектории");
    expect(markup).toContain("Размер XYZ");
    expect(markup).not.toContain("program-rotary-metrics");
    expect(markup).not.toContain("preview-rotary-selection");
  });

  const renderProgram = (program: GcodeProgram, selectedToolpath?: GcodeProgram["toolpath"]) => renderToStaticMarkup(
    <ProgramPreviewStage
      cuttingDepthAdjustmentMm={0}
      onClearSelection={() => undefined}
      onSafeStart={() => undefined}
      onSelectSourceLine={() => undefined}
      program={program}
      safeStartAvailable={false}
      selectedMotionCount={1}
      selectedProgramLine={program.lines[3]}
      selectedSourceLine={program.lines[3].sourceLine}
      selectedToolpath={selectedToolpath}
      toolVisualization={{ state: "selected", showCutter: true, spinning: false }}
      view="iso"
    />,
  );

  it("shows unwrapped program ranges, angular travel and selected A endpoints", () => {
    const program: GcodeProgram = {
      ...previewFixtureFirstCutProgram,
      features: { ...previewFixtureFirstCutProgram.features, usesRotaryA: true },
      summary: {
        ...previewFixtureFirstCutProgram.summary,
        rotaryBounds: { minDegrees: -90, maxDegrees: 720, sizeDegrees: 810 },
        rotaryTravelDegrees: 1080,
      },
      toolpath: [{
        ...previewFixtureFirstCutProgram.toolpath[0],
        sourceLine: previewFixtureFirstCutProgram.lines[3].sourceLine,
        rotary: { startDegrees: -90, endDegrees: 720 },
      }],
    };
    const markup = renderProgram(program);
    expect(markup).toContain("Проекция XYZ");
    expect(markup).toContain("Диапазон A");
    expect(markup).toContain("-90.000° … 720.000°");
    expect(markup).toContain("Путь A");
    expect(markup).toContain("1080.000°");
    expect(markup).toContain("Начало -90.000°");
    expect(markup).toContain("Конец 720.000°");
  });

  it("does not invent rotary bounds or travel for incomplete preview metadata", () => {
    const markup = renderProgram({
      ...previewFixtureFirstCutProgram,
      features: { ...previewFixtureFirstCutProgram.features, usesRotaryA: true },
    });
    expect(markup).toContain("Проекция XYZ");
    expect(markup).toContain("-- … --");
    expect(markup).not.toContain("0.000°");
    expect(markup).not.toContain("preview-rotary-selection");
  });

  it("labels sampled geometry and reads exact selected A rather than the overview", () => {
    const program: GcodeProgram = {
      ...previewFixtureFirstCutProgram,
      document: {
        id: "large", sourceBytes: 4_000_000, pageSize: 200, previewSampled: true,
        warningCount: 0, blockingWarningCount: 0, toolSelections: [],
        errorCount: 0, managedToolChangeCount: 0, toolSelectionCoverageLine: 0,
      },
      features: { ...previewFixtureFirstCutProgram.features, usesRotaryA: true },
    };
    const markup = renderProgram(program, [{
      sourceLine: program.lines[3].sourceLine, kind: "linear", distanceMm: 0,
      points: [{ x: 0, y: 0, z: 0 }, { x: 0, y: 0, z: 0 }],
      rotary: { startDegrees: 720, endDegrees: 1080 },
    }]);
    expect(markup).toContain("Обзорная траектория");
    expect(markup).toContain("выполнение использует все строки");
    expect(markup).toContain("Начало 720.000°");
    expect(markup).toContain("Конец 1080.000°");
    expect(renderProgram(previewFixtureFirstCutProgram)).not.toContain("Обзорная траектория");
  });
});
