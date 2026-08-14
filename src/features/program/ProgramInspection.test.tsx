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
    expect(markup).toContain("Диагностика");
    expect(markup).toContain("Проверка");
    expect(markup).toContain("program-warning");
  });

  it("presents a host-managed tool change as an expected operation", () => {
    const program = {
      ...previewFixtureProgram,
      warnings: [{
        sourceLine: 3,
        severity: "safety" as const,
        code: "tool-change" as const,
        message: "M6 requires a host-managed operator tool-change barrier",
      }],
    };
    const markup = renderToStaticMarkup(
      <ProgramInspection
        diagnosticView="warnings"
        motionSourceLines={new Set()}
        onOpenChange={() => undefined}
        onSelectSourceLine={() => undefined}
        onView={() => undefined}
        open
        program={program}
        realRunTarget
      />,
    );

    expect(markup).toContain("1 смена инструмента");
    expect(markup).toContain("Смена инструмента");
    expect(markup).toContain("M6 не отправляется в GRBL");
    expect(markup).toContain("is-managed");
    expect(markup).not.toContain("M6 requires a host-managed");
  });
});
