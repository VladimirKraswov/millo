import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { JobReadinessPanel } from "./JobReadinessPanel";

describe("JobReadinessPanel", () => {
  it("renders the four operator facts and exactly one contextual primary action", () => {
    const markup = renderToStaticMarkup(
      <JobReadinessPanel
        busy={false}
        details={{
          machine: "Millo fixture · Idle",
          file: "24 строки · 0 замечаний",
          origin: "G54 · X 0.000 · Y 0.000 · Z 0.000",
          validation: "Еще не выполнялась",
        }}
        intent="cutting"
        onIntent={() => undefined}
        onOpenOrigin={() => undefined}
        onPrimary={() => undefined}
        view={{
          primaryAction: "runPreflight",
          primaryDisabled: false,
          primaryLabel: "Проверить готовность",
          steps: [
            { id: "machine", state: "ready" },
            { id: "file", state: "ready" },
            { id: "origin", state: "ready" },
            { id: "validation", state: "action" },
          ],
        }}
      />,
    );

    expect(markup).toContain("Готовность к запуску");
    expect(markup).toContain("Рабочий ноль");
    expect(markup).toContain("Проверить готовность");
    expect(markup.match(/job-primary-action/g)).toHaveLength(1);
  });
});
