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
});
