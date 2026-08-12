import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { emptySnapshot } from "../../shared/machine";
import { WorkZeroPanel } from "./WorkZeroPanel";

describe("WorkZeroPanel", () => {
  it("offers a prominent XYZ action and secondary single-axis controls", () => {
    const markup = renderToStaticMarkup(
      <WorkZeroPanel
        desktopRuntime
        gateway={{
          returnToZero: async () => {
            throw new Error("not used during server render");
          },
          setZero: async () => {
            throw new Error("not used during server render");
          },
        }}
        onError={() => undefined}
        onSnapshot={() => undefined}
        snapshot={{
          ...emptySnapshot,
          connection: "connected",
          machine: { ...emptySnapshot.machine, mode: "idle", reportedMode: "Idle" },
        }}
      />,
    );

    expect(markup).toContain("Установить XYZ = 0");
    expect(markup).toContain("Только X");
    expect(markup).toContain("Только Y");
    expect(markup).toContain("Только Z");
    expect(markup).toContain("Вернуться к сохранённому нулю");
    expect(markup).toContain("К Z0");
  });
});
