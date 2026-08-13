import { renderToStaticMarkup } from "react-dom/server";
import type { ComponentProps } from "react";
import { describe, expect, it } from "vitest";

import { JobReadinessPanel } from "./JobReadinessPanel";

type PanelProps = ComponentProps<typeof JobReadinessPanel>;

const readySteps: PanelProps["view"]["steps"] = [
  { id: "machine", state: "ready" },
  { id: "file", state: "ready" },
  { id: "origin", state: "ready" },
  { id: "validation", state: "ready" },
];

const baseProps: PanelProps = {
  busy: false,
  details: { machine: "Idle", file: "ok", origin: "G54", validation: "ok" },
  intent: "cutting",
  onIntent: () => undefined,
  onOpenOrigin: () => undefined,
  onPrimary: () => undefined,
  view: {
    primaryAction: "startProgram",
    primaryDisabled: false,
    primaryLabel: "Запустить программу",
    steps: readySteps,
  },
};

const renderPanel = (overrides: Partial<PanelProps> = {}): string =>
  renderToStaticMarkup(<JobReadinessPanel {...baseProps} {...overrides} />);

describe("JobReadinessPanel", () => {
  it("renders the four operator facts and exactly one contextual primary action", () => {
    const markup = renderPanel({
      details: {
        machine: "Millo fixture · Idle",
        file: "24 строки · 0 замечаний",
        origin: "G54 · X 0.000 · Y 0.000 · Z 0.000",
        validation: "Еще не выполнялась",
      },
      view: {
        primaryAction: "runPreflight",
        primaryDisabled: false,
        primaryLabel: "Проверить готовность",
        steps: [...readySteps.slice(0, 3), { id: "validation", state: "action" }],
      },
    });

    expect(markup).toContain("Готовность к запуску");
    expect(markup).toContain("Рабочий ноль");
    expect(markup).toContain("Проверить готовность");
    expect(markup).toContain("title=\"Еще не выполнялась\"");
    expect(markup.match(/job-primary-action/g)).toHaveLength(1);
  });

  it("makes an unsynchronized machine explanation and action discoverable", () => {
    const machine = "Профиль не синхронизирован с подключённым контроллером";
    const markup = renderPanel({
      details: { ...baseProps.details, machine },
      view: {
        primaryAction: "syncMachine",
        primaryDisabled: false,
        primaryLabel: "Определить подключённый станок",
        steps: [{ id: "machine", state: "action" }, ...readySteps.slice(1)],
      },
    });

    expect(markup).toContain("Определить подключённый станок");
    expect(markup).toContain(`title="${machine}"`);
  });

  it("names the final action after the selected execution intent", () => {
    const markup = renderPanel();

    expect(markup).toContain("Начать обработку");
    expect(markup).not.toContain(">Запустить программу<");
  });

  it("shows heightmap application beside the job launch when a map exists", () => {
    const markup = renderPanel({
      onSurfaceMap: () => undefined,
      surfaceMap: {
        checked: true,
        detail: "Карта #3 · 6×6 · 50.0×50.0 mm · перепад 0.120 mm",
        disabled: false,
        warning: false,
      },
    });

    expect(markup).toContain("Компенсировать по карте");
    expect(markup).toContain("role=\"switch\"");
    expect(markup).toContain("Карта #3");
  });

  it("shows file and target depth only for processing mode", () => {
    const markup = renderPanel({
      depthCorrection: {
        available: true,
        enabled: true,
        fileDepthMm: -0.2,
        targetDepthMm: -0.3,
        minimumTargetMm: -10.2,
        maximumTargetMm: 0,
      },
    });

    expect(markup).toContain("Коррекция глубины");
    expect(markup).toContain("Файл -0.200 мм · итог -0.300 мм");
    expect(markup).toContain("Итоговая глубина обработки");
    expect(markup).toContain("Обработка");
  });

  it.each([
    ["airRun", "Запустить проверку движения"],
    ["cutting", "Начать обработку"],
  ] as const)("names the %s execution mode", (intent, expected) => {
    expect(renderPanel({ intent })).toContain(expected);
  });
});
