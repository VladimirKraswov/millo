import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { emptySnapshot } from "../../shared/machine";
import { MachineSetupPanel } from "./MachineSetupPanel";

const snapshot = {
  ...emptySnapshot,
  connection: "connected" as const,
  machine: { ...emptySnapshot.machine, mode: "idle" as const, reportedMode: "Idle" },
};

const renderPanel = (floodCoolantControl: boolean, mistCoolantControl: boolean) =>
  renderToStaticMarkup(
    <MachineSetupPanel
      activeCoordinateSystem="g56"
      disabled={false}
      floodCoolantControl={floodCoolantControl}
      mistCoolantControl={mistCoolantControl}
      onError={vi.fn()}
      onSnapshot={vi.fn()}
      snapshot={snapshot}
      spindleControl="manual"
    />,
  );

describe("MachineSetupPanel", () => {
  it("shows all WCS choices and marks the active one", () => {
    const markup = renderPanel(false, false);

    for (const wcs of ["G54", "G55", "G56", "G57", "G58", "G59"]) {
      expect(markup).toContain(`>${wcs}</button>`);
    }
    expect(markup).toMatch(/aria-pressed="true"[^>]*>G56<\/button>/);
  });

  it("renders only coolant outputs declared by the machine profile", () => {
    const disabled = renderPanel(false, false);
    const floodOnly = renderPanel(true, false);

    expect(disabled).not.toContain(">M8</button>");
    expect(disabled).not.toContain(">M7</button>");
    expect(floodOnly).toContain(">M8</button>");
    expect(floodOnly).not.toContain(">M7</button>");
  });
});
