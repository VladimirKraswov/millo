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
    expect(markup).toContain("title=\"Еще не выполнялась\"");
    expect(markup.match(/job-primary-action/g)).toHaveLength(1);
  });

  it("makes an unsynchronized machine explanation and action discoverable", () => {
    const markup = renderToStaticMarkup(
      <JobReadinessPanel
        busy={false}
        details={{
          machine: "Профиль не синхронизирован с подключённым контроллером",
          file: "ok",
          origin: "G54",
          validation: "Еще не выполнялась",
        }}
        intent="cutting"
        onIntent={() => undefined}
        onOpenOrigin={() => undefined}
        onPrimary={() => undefined}
        view={{
          primaryAction: "syncMachine",
          primaryDisabled: false,
          primaryLabel: "Определить подключённый станок",
          steps: [
            { id: "machine", state: "action" },
            { id: "file", state: "ready" },
            { id: "origin", state: "ready" },
            { id: "validation", state: "action" },
          ],
        }}
      />,
    );

    expect(markup).toContain("Определить подключённый станок");
    expect(markup).toContain(
      "title=\"Профиль не синхронизирован с подключённым контроллером\"",
    );
  });

  it("names the final action after the selected execution intent", () => {
    const markup = renderToStaticMarkup(
      <JobReadinessPanel
        busy={false}
        details={{ machine: "Idle", file: "ok", origin: "G54", validation: "ok" }}
        intent="cutting"
        onIntent={() => undefined}
        onOpenOrigin={() => undefined}
        onPrimary={() => undefined}
        view={{
          primaryAction: "startProgram",
          primaryDisabled: false,
          primaryLabel: "Запустить программу",
          steps: [
            { id: "machine", state: "ready" },
            { id: "file", state: "ready" },
            { id: "origin", state: "ready" },
            { id: "validation", state: "ready" },
          ],
        }}
      />,
    );

    expect(markup).toContain("Начать гравировку");
    expect(markup).not.toContain(">Запустить программу<");
  });

  it("shows heightmap application beside the job launch when a map exists", () => {
    const markup = renderToStaticMarkup(
      <JobReadinessPanel
        busy={false}
        details={{ machine: "Idle", file: "ok", origin: "G54", validation: "ok" }}
        intent="cutting"
        onIntent={() => undefined}
        onOpenOrigin={() => undefined}
        onPrimary={() => undefined}
        onSurfaceMap={() => undefined}
        surfaceMap={{
          checked: true,
          detail: "Карта #3 · 6×6 · 50.0×50.0 mm · перепад 0.120 mm",
          disabled: false,
          warning: false,
        }}
        view={{
          primaryAction: "startProgram",
          primaryDisabled: false,
          primaryLabel: "Запустить программу",
          steps: [
            { id: "machine", state: "ready" },
            { id: "file", state: "ready" },
            { id: "origin", state: "ready" },
            { id: "validation", state: "ready" },
          ],
        }}
      />,
    );

    expect(markup).toContain("Компенсировать по карте");
    expect(markup).toContain("role=\"switch\"");
    expect(markup).toContain("Карта #3");
  });
});
