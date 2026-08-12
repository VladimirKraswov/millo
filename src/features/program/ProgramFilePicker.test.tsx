import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { ProgramFilePicker } from "./ProgramFilePicker";

describe("ProgramFilePicker", () => {
  it("renders the empty workspace action as an explicit G-code command", () => {
    const markup = renderToStaticMarkup(
      <ProgramFilePicker
        disabled={false}
        loading={false}
        onSelect={vi.fn()}
        variant="empty"
      />,
    );

    expect(markup).toContain("Открыть G-code");
    expect(markup).toContain("program-file-picker is-empty");
    expect(markup).toContain('accept=".nc,.ngc,.gcode,.tap,.cnc"');
  });

  it("uses a stable replacement action after a program is loaded", () => {
    const markup = renderToStaticMarkup(
      <ProgramFilePicker
        disabled
        loading={false}
        onSelect={vi.fn()}
        variant="toolbar"
      />,
    );

    expect(markup).toContain("Заменить файл");
    expect(markup).toContain("program-file-picker is-toolbar");
    expect(markup).toContain("disabled");
  });
});
