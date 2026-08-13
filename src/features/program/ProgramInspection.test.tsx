import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { previewFixtureProgram } from "./previewFixtureProgram";
import { ProgramInspection } from "./ProgramInspection";

describe("ProgramInspection", () => {
  it("keeps line, warning and optional preflight tabs in one disclosure", () => {
    const markup = renderToStaticMarkup(
      <ProgramInspection
        diagnosticView="warnings"
        motionSourceLines={new Set(previewFixtureProgram.toolpath.map(({ sourceLine }) => sourceLine))}
        onOpenChange={() => undefined}
        onSelectSourceLine={() => undefined}
        onView={() => undefined}
        open
        program={previewFixtureProgram}
        realRunTarget
        selectedSourceLine={previewFixtureProgram.warnings[0]?.sourceLine}
      />,
    );

    expect(markup).toContain("Программа и диагностика");
    expect(markup).toContain("Строки");
    expect(markup).toContain("Замечания");
    expect(markup).toContain("Проверка");
    expect(markup).toContain("program-warning");
  });
});
