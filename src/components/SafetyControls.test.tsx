import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { createUiExtensionRegistry } from "../platform/extensions/UiExtensionRegistry";
import type { ControllerSnapshot } from "../shared/machine";
import { emptySnapshot } from "../shared/machine";
import { SafetyControls } from "./SafetyControls";

const snapshot = (mode: "idle" | "jog"): ControllerSnapshot => ({
  ...emptySnapshot,
  connection: "connected",
  machine: {
    ...emptySnapshot.machine,
    mode,
    reportedMode: mode === "jog" ? "Jog" : "Idle",
  },
});

const renderControls = (mode: "idle" | "jog"): string =>
  renderToStaticMarkup(
    <SafetyControls
      desktopRuntime
      extensionRegistry={createUiExtensionRegistry()}
      machineBound
      machineGateway={{ jogPadStep: vi.fn() }}
      maxJogDistanceMm={50}
      maxJogFeedMmPerMin={1_000}
      useProbeForZ={false}
      onError={vi.fn()}
      onInspection={vi.fn()}
      onOpenMotionSettings={vi.fn()}
      onSnapshot={vi.fn()}
      snapshot={snapshot(mode)}
      workCoordinateGateway={{ setZero: vi.fn(), returnToZero: vi.fn() }}
    />,
  );

describe("SafetyControls layout", () => {
  it("keeps all three safety action slots mounted when jog starts", () => {
    const idle = renderControls("idle");
    const jog = renderControls("jog");
    const cancelTag = (markup: string) =>
      markup.match(/<button[^>]*class="jog-cancel-action"[^>]*>/)?.[0];

    for (const markup of [idle, jog]) {
      expect(markup).toContain('class="hold-action"');
      expect(markup).toContain('class="reset-action"');
      expect(markup).toContain('class="jog-cancel-action"');
    }
    expect(cancelTag(idle)).toContain("disabled");
    expect(cancelTag(jog)).not.toContain("disabled");
  });
});
