import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { emptySnapshot } from "../../shared/machine";
import { previewHeightmapGateway } from "./previewHeightmapGateway";
import { HeightmapPanel } from "./HeightmapPanel";

describe("HeightmapPanel", () => {
  it("leads with direct surface finding and keeps optional plate details advanced", () => {
    const markup = renderToStaticMarkup(
      <HeightmapPanel
        desktopRuntime
        gateway={previewHeightmapGateway}
        machineProfileId="machine-0001"
        onAbort={async () => emptySnapshot}
        onError={() => undefined}
        onSaveMode={async () => undefined}
        onSnapshot={() => undefined}
        onUnlock={async () => emptySnapshot}
        snapshot={{
          ...emptySnapshot,
          connection: "connected",
          machine: { ...emptySnapshot.machine, mode: "idle", reportedMode: "Idle" },
        }}
        zProbeGateway={{ run: async () => { throw new Error("not used"); } }}
      />,
    );

    expect(markup).toContain("Найти поверхность и установить Z0");
    expect(markup).toContain("Плоская / PCB");
    expect(markup).toContain("Сильный рельеф");
    expect(markup).toContain("Это толщина съёмной шайбы, а не заготовки");
    expect(markup).not.toContain("Пластина для Z0");
  });
});
