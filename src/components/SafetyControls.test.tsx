import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import type { ControllerSnapshot } from "../shared/machine";
import { emptySnapshot } from "../shared/machine";
import { RealtimeControls } from "../features/machine-control/RealtimeControls";

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
    <RealtimeControls
      desktopRuntime
      onError={vi.fn()}
      onSnapshot={vi.fn()}
      snapshot={snapshot(mode)}
    />,
  );

describe("RealtimeControls layout", () => {
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
