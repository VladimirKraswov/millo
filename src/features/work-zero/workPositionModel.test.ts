import { describe, expect, it } from "vitest";

import { emptySnapshot, type HardwareInspection } from "../../shared/machine";
import { resolveWorkPosition } from "./workPositionModel";

const inspection: HardwareInspection = {
  device: {
    modalState: ["G0", "G55", "G21"],
    parameters: {
      G55: "100.000,20.000,-5.000",
      G92: "1.000,2.000,3.000",
      TLO: "4.000",
    },
    responses: [],
    settings: {},
  },
  readiness: {
    blockerCount: 0,
    cautionCount: 0,
    probeReady: false,
    testJogReady: true,
    checks: [],
    profile: {
      axes: ["X", "Y", "Z"],
      emergencyStopInstalled: false,
      homingInstalled: false,
      limitSwitchesInstalled: false,
      name: "Fixture",
    probeInstalled: false,
    probeMode: "off",
      spindleControl: "manual",
    },
  },
};

describe("resolveWorkPosition", () => {
  it("prefers the controller-reported work position", () => {
    const view = resolveWorkPosition(
      {
        ...emptySnapshot,
        machine: {
          ...emptySnapshot.machine,
          machinePosition: { x: 50, y: 50, z: 50 },
          workPosition: { x: 1, y: 2, z: 3 },
        },
      },
      inspection,
    );

    expect(view).toEqual({
      coordinateSystem: "G55",
      position: { x: 1, y: 2, z: 3 },
    });
  });

  it("derives work position from G5x, G92 and TLO when GRBL reports MPos", () => {
    const view = resolveWorkPosition(
      {
        ...emptySnapshot,
        machine: {
          ...emptySnapshot.machine,
          machinePosition: { x: 111, y: 32, z: 12 },
        },
      },
      inspection,
    );

    expect(view).toEqual({
      coordinateSystem: "G55",
      position: { x: 10, y: 10, z: 10 },
    });
  });
});
