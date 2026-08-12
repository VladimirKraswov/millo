import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { ProgramEditor } from "./ProgramEditor";
import { previewFixtureProgram } from "./previewFixtureProgram";

describe("ProgramEditor", () => {
  it("exposes a syntax editor, line operations, history, preview, and safe apply", () => {
    const markup = renderToStaticMarkup(
      <ProgramEditor
        blockDelete={false}
        document={{
          program: previewFixtureProgram,
          source: previewFixtureProgram.lines.map((line) => line.source).join("\n"),
        }}
        gateway={{
          parse: async () => previewFixtureProgram,
          save: async () => undefined,
        }}
        onApply={() => undefined}
        onClose={() => undefined}
      />,
    );

    expect(markup).toContain("Program Editor");
    expect(markup).toContain("aria-label=\"Исходный G-code\"");
    expect(markup).toContain("title=\"Отменить\"");
    expect(markup).toContain("title=\"Вырезать\"");
    expect(markup).toContain("title=\"Вставить строку\"");
    expect(markup).toContain("Обработанная копия");
    expect(markup).toContain("Применить к заданию");
  });
});
