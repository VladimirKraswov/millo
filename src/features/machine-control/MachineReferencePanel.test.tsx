import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { emptySnapshot } from "../../shared/machine";
import { MachineReferencePanel } from "./MachineReferencePanel";

describe("MachineReferencePanel", () => {
  it("makes a session-scoped homing reference explicit", () => {
    const markup = renderToStaticMarkup(
      <MachineReferencePanel
        desktopRuntime
        disabled={false}
        homingInstalled
        onError={vi.fn()}
        onSnapshot={vi.fn()}
        snapshot={{
          ...emptySnapshot,
          connection: "connected",
          machine: { ...emptySnapshot.machine, mode: "idle", reportedMode: "Idle" },
          homing: { state: "homed", sequence: 3 },
        }}
      />,
    );

    expect(markup).toContain("Базирован в этой сессии");
    expect(markup).toContain('class="machine-reference is-homed"');
  });

  it("distinguishes an invalidated reference from a never-homed machine", () => {
    const markup = renderToStaticMarkup(
      <MachineReferencePanel
        desktopRuntime
        disabled={false}
        homingInstalled
        onError={vi.fn()}
        onSnapshot={vi.fn()}
        snapshot={{
          ...emptySnapshot,
          connection: "connected",
          homing: { state: "invalidated", sequence: 3 },
        }}
      />,
    );

    expect(markup).toContain("Базирование утрачено после reset/reconnect");
  });
});
