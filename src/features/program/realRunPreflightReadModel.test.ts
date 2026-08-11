import { describe, expect, it } from "vitest";

import type { RunPreflightReport } from "../../shared/realRun";
import { realRunPreflightControls } from "./realRunPreflightReadModel";

const report = (ready: boolean): RunPreflightReport => ({
    sourceName: "fixture.nc",
    programFingerprint: "fixture-sha256",
    ready,
    blockerCount: ready ? 0 : 1,
    cautionCount: 3,
    pollSequence: 12,
    hardware: {
      device: {
        settings: {},
        modalState: [],
        parameters: {},
        responses: [],
      },
      readiness: {
        profile: {
          name: "fixture",
          axes: ["X", "Y", "Z"],
          spindleControl: "manual",
          homingInstalled: false,
          limitSwitchesInstalled: false,
          probeInstalled: false,
          emergencyStopInstalled: false,
        },
        testJogReady: true,
        probeReady: false,
        blockerCount: 0,
        cautionCount: 0,
        checks: [],
      },
    },
    checks: [],
    programBlockers: [],
    totalProgramBlockers: 0,
  });

const available = {
  serialAvailable: true,
  gatewayAvailable: true,
  checking: false,
};

describe("realRunPreflightControls", () => {
  it("offers only a check action for an available serial target", () => {
    expect(realRunPreflightControls(undefined, available)).toEqual({
      canCheck: true,
      status: "unchecked",
      statusLabel: "Not checked",
    });
  });

  it("reports ready and blocked backend outcomes without creating a start action", () => {
    expect(realRunPreflightControls(report(true), available)).toMatchObject({
      canCheck: true,
      status: "ready",
    });
    expect(realRunPreflightControls(report(false), available)).toMatchObject({
      canCheck: true,
      status: "blocked",
    });
  });

  it("disables repeated checks while the fresh transaction is running", () => {
    expect(
      realRunPreflightControls(undefined, { ...available, checking: true }),
    ).toMatchObject({ canCheck: false, status: "checking" });
  });

  it("fails closed without both serial state and a typed gateway", () => {
    expect(
      realRunPreflightControls(report(true), {
        ...available,
        serialAvailable: false,
      }),
    ).toMatchObject({ canCheck: false, status: "unavailable" });
    expect(
      realRunPreflightControls(report(true), {
        ...available,
        gatewayAvailable: false,
      }),
    ).toMatchObject({ canCheck: false, status: "unavailable" });
  });
});
