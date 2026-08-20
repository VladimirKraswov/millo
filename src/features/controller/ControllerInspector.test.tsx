import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { HardwareInspection } from "../../shared/machine";
import { ControllerInspector } from "./ControllerInspector";

const inspection: HardwareInspection = {
  device: {
    firmwareVersion: "1.1f",
    firmwareBuildInfo: "fixture",
    firmwareOptions: "VMZ",
    settings: { "$21": "0" },
    modalState: ["G21", "G90"],
    parameters: { G54: "1.000,2.000,3.000" },
    responses: [{ command: "$I", completion: "ok", lines: [] }],
  },
  readiness: {
    profile: {
      name: "Fixture",
      axes: ["x", "y", "z"],
      spindleControl: "manual",
      floodCoolantControl: false,
      mistCoolantControl: false,
      homingInstalled: false,
      limitSwitchesInstalled: false,
      probeInstalled: false,
      probeMode: "off",
      emergencyStopInstalled: false,
    },
    testJogReady: true,
    probeReady: false,
    blockerCount: 0,
    cautionCount: 0,
    checks: [],
  },
};

describe("ControllerInspector", () => {
  it("keeps read-only controller identity and register groups visible", () => {
    const markup = renderToStaticMarkup(
      <ControllerInspector
        busy={false}
        connected
        inspecting={false}
        inspection={inspection}
        onRead={() => undefined}
      />,
    );

    expect(markup).toContain("1.1f");
    expect(markup).toContain("Настройки контроллера");
    expect(markup).toContain("Системы координат");
    expect(markup).toContain("$21");
    expect(markup).toContain("G54");
  });

  it("disables inspection while disconnected", () => {
    const markup = renderToStaticMarkup(
      <ControllerInspector
        busy={false}
        connected={false}
        inspecting={false}
        onRead={() => undefined}
      />,
    );

    expect(markup).toContain("disabled=\"\"");
    expect(markup).toContain("Профиль контроллера не считан");
  });
});
