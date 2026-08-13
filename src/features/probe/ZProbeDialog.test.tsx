import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { emptySnapshot } from "../../shared/machine";
import { ZProbeDialog } from "./ZProbeDialog";

const heightmapGateway = {
  clear: async () => { throw new Error("not used"); },
  getOperation: async () => { throw new Error("not used"); },
  getSession: async () => { throw new Error("not used"); },
  pause: async () => { throw new Error("not used"); },
  resume: async () => { throw new Error("not used"); },
  setApplication: async () => { throw new Error("not used"); },
  start: async () => { throw new Error("not used"); },
  subscribeOperation: async () => () => undefined,
  subscribeSession: async () => () => undefined,
};

describe("ZProbeDialog", () => {
  it("explains the measured plate offset and blocks an already active input", () => {
    const markup = renderToStaticMarkup(
      <ZProbeDialog
        desktopRuntime
        gateway={{ run: async () => { throw new Error("not used"); } }}
        heightmapGateway={heightmapGateway}
        onAbort={async () => emptySnapshot}
        onClose={() => undefined}
        onError={() => undefined}
        onSaveSettings={async () => undefined}
        onSnapshot={() => undefined}
        onUnlock={async () => emptySnapshot}
        open
        profileId="machine-0001"
        probeInstalled
        settings={{
          mode: "workZero",
          plateThicknessMm: 19.1,
          maxTravelMm: 10,
          probeFeedMmPerMin: 25,
          retractMm: 3,
          retractFeedMmPerMin: 100,
        }}
        snapshot={{
          ...emptySnapshot,
          connection: "connected",
          machine: {
            ...emptySnapshot.machine,
            mode: "idle",
            reportedMode: "Idle",
            pins: {
              raw: "P",
              xLimit: false,
              yLimit: false,
              zLimit: false,
              aLimit: false,
              bLimit: false,
              cLimit: false,
              probe: true,
              door: false,
              hold: false,
              softReset: false,
              cycleStart: false,
            },
          },
        }}
      />,
    );

    expect(markup).toContain("Контакт замкнут");
    expect(markup).toContain("Z = 19.100 mm");
    expect(markup).toContain("Z = 22.100 mm");
    expect(markup).toContain("Вход P уже активен");
    expect(markup).toContain("Общая команда обнулит только X/Y");
    expect(markup).toContain("disabled");
  });

  it("keeps manual Z available while probe zeroing is disabled", () => {
    const markup = renderToStaticMarkup(
      <ZProbeDialog
        desktopRuntime
        gateway={{ run: async () => { throw new Error("not used"); } }}
        heightmapGateway={heightmapGateway}
        onAbort={async () => emptySnapshot}
        onClose={() => undefined}
        onError={() => undefined}
        onSaveSettings={async () => undefined}
        onSnapshot={() => undefined}
        onUnlock={async () => emptySnapshot}
        open
        profileId="machine-0001"
        probeInstalled
        settings={{
          mode: "off",
          plateThicknessMm: 0,
          maxTravelMm: 10,
          probeFeedMmPerMin: 25,
          retractMm: 3,
          retractFeedMmPerMin: 100,
        }}
        snapshot={{ ...emptySnapshot, connection: "connected" }}
      />,
    );

    expect(markup).toContain("Ручное обнуление Z доступно");
    expect(markup).not.toContain("Перед касанием введите измеренную толщину");
    expect(markup).not.toContain("Сохранить параметры");
  });
});
