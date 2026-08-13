import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

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
        view="iso"
      />,
    );

    expect(markup).toContain("Загрузка траектории");
    expect(markup).toContain("С этого участка");
    expect(markup).toContain("1 сегмент траектории");
    expect(markup).toContain("Размер XYZ");
  });
});
