import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { FirstCutAuthorizationDialog } from "./FirstCutAuthorizationDialog";

describe("FirstCutAuthorizationDialog", () => {
  it("shows the processing mode and signed Z offset before motion starts", () => {
    const markup = renderToStaticMarkup(
      <FirstCutAuthorizationDialog
        depthCorrection={{ adjustmentMm: -0.1 }}
        executionOptions={{
          blockDelete: false,
          cuttingDepthAdjustmentUm: -100,
          optionalStop: false,
        }}
        intent="cutting"
        onAuthorize={async () => {
          throw new Error("not called while rendering");
        }}
        onAuthorized={() => undefined}
        onClose={() => undefined}
        onStart={async () => {
          throw new Error("not called while rendering");
        }}
        onStarted={() => undefined}
        open
        startingToolNumber={2}
      />,
    );

    expect(markup).toContain("Обработка");
    expect(markup).toContain("Коррекция глубины");
    expect(markup).toContain("ΔZ −0.100 мм");
    expect(markup).toContain("Начать обработку");
    expect(markup).toContain("Стартовый инструмент");
    expect(markup).toContain("T2");
  });

  it("uses an explicit motion-check label for a non-cutting run", () => {
    const markup = renderToStaticMarkup(
      <FirstCutAuthorizationDialog
        executionOptions={{ blockDelete: false, optionalStop: false }}
        intent="airRun"
        onAuthorize={async () => {
          throw new Error("not called while rendering");
        }}
        onAuthorized={() => undefined}
        onClose={() => undefined}
        onStart={async () => {
          throw new Error("not called while rendering");
        }}
        onStarted={() => undefined}
        open
      />,
    );

    expect(markup).toContain("Проверка движения");
    expect(markup).toContain("Начать проверку движения");
    expect(markup).not.toContain("Коррекция глубины");
  });

  it("warns explicitly when a usable heightmap exists but is disabled", () => {
    const markup = renderToStaticMarkup(
      <FirstCutAuthorizationDialog
        executionOptions={{ blockDelete: false, optionalStop: false }}
        intent="cutting"
        onAuthorize={async () => {
          throw new Error("not called while rendering");
        }}
        onAuthorized={() => undefined}
        onClose={() => undefined}
        onStart={async () => {
          throw new Error("not called while rendering");
        }}
        onStarted={() => undefined}
        open
        surfaceMap={{
          mapId: 5,
          enabled: false,
          usable: true,
          coversProgram: true,
          zRangeMm: 0.367,
          detail: "Карта #5 · 5×5 · 60.0×50.0 mm · перепад 0.367 mm",
          busy: false,
          onApply: async () => undefined,
        }}
      />,
    );

    expect(markup).toContain("Карта #5 найдена, но не применяется");
    expect(markup).toContain("Без компенсации перепад поверхности до 0.367 мм");
    expect(markup).toContain("Компенсировать траекторию по карте высот");
    expect(markup).toContain("Начать обработку без карты");
    expect(markup).toContain("first-cut-surface-map is-warning");
  });

  it("shows a positive status when the checked map is part of the run options", () => {
    const markup = renderToStaticMarkup(
      <FirstCutAuthorizationDialog
        executionOptions={{ blockDelete: false, optionalStop: false, surfaceMapId: 5 }}
        intent="cutting"
        onAuthorize={async () => {
          throw new Error("not called while rendering");
        }}
        onAuthorized={() => undefined}
        onClose={() => undefined}
        onStart={async () => {
          throw new Error("not called while rendering");
        }}
        onStarted={() => undefined}
        open
        surfaceMap={{
          mapId: 5,
          enabled: true,
          usable: true,
          coversProgram: true,
          zRangeMm: 0.367,
          detail: "Карта #5 · 5×5 · 60.0×50.0 mm · перепад 0.367 mm",
          busy: false,
          onApply: async () => undefined,
        }}
      />,
    );

    expect(markup).toContain("Карта #5 применяется");
    expect(markup).toContain("first-cut-surface-map is-enabled");
    expect(markup).toContain("Щуп и провода убраны");
    expect(markup).toContain("Начать обработку");
    expect(markup).not.toContain("Начать обработку без карты");
  });
});
