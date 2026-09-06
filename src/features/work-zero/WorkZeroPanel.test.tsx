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
          returnToOrigin: async () => {
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
    expect(markup).not.toContain("Только A");
    expect(markup).toContain("Вернуться к сохранённому нулю");
    expect(markup).toContain("Вернуться в рабочий ноль");
  });

  it("offers separately confirmed A zero only with finite reported A and no rotary return", () => {
    const position = { x: 0, y: 0, z: 10, a: 90 };
    const markup = renderToStaticMarkup(<WorkZeroPanel desktopRuntime
      gateway={{ setZero: async () => { throw new Error("unused"); }, returnToZero: async () => { throw new Error("unused"); } }}
      onError={() => undefined} onSnapshot={() => undefined}
      snapshot={{ ...emptySnapshot, connection: "connected", machine: { ...emptySnapshot.machine, mode: "idle", reportedMode: "Idle",
        machinePosition: position, workPosition: position, workCoordinateOffset: { ...position, a: 0 } } }} />);
    expect(markup).toContain("Только A");
    expect(markup).toContain("Подтверждаю текущий угол A");
    expect(markup).toContain("90.000°");
    expect(markup).toContain("Установить XYZ = 0");
    expect(markup).not.toContain("Вернуть A");
  });

  it("keeps manual Z zero out of the combined action when probe mode is enabled", () => {
    const markup = renderToStaticMarkup(
      <WorkZeroPanel
        desktopRuntime
        gateway={{
          returnToZero: async () => { throw new Error("not used"); },
          returnToOrigin: async () => { throw new Error("not used"); },
          setZero: async () => { throw new Error("not used"); },
        }}
        onError={() => undefined}
        onSnapshot={() => undefined}
        snapshot={{
          ...emptySnapshot,
          connection: "connected",
          machine: { ...emptySnapshot.machine, mode: "idle", reportedMode: "Idle" },
        }}
        useProbeForZ
      />,
    );

    expect(markup).toContain("Установить только XY = 0");
    expect(markup).toContain("Z0 уже найден щупом");
  });
});
