import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { FirstCutAuthorizationDialog } from "./FirstCutAuthorizationDialog";

describe("FirstCutAuthorizationDialog", () => {
  it("shows the processing mode and the effective depth before motion starts", () => {
    const markup = renderToStaticMarkup(
      <FirstCutAuthorizationDialog
        depthCorrection={{ fileDepthMm: -0.2, targetDepthMm: -0.3 }}
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
      />,
    );

    expect(markup).toContain("Обработка");
    expect(markup).toContain("Коррекция глубины");
    expect(markup).toContain("−0.200 → −0.300 мм");
    expect(markup).toContain("Начать обработку");
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
