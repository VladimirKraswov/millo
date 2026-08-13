import { describe, expect, it } from "vitest";

import type {
  FirstCutConfirmation,
  RunPreflightReport,
} from "../../shared/realRun";
import {
  emptyFirstCutConfirmation,
  firstCutAuthorizationControls,
  setFirstCutReadiness,
} from "./firstCutAuthorizationModel";

const complete: FirstCutConfirmation = {
  intent: "cutting",
  executionOptions: { optionalStop: false, blockDelete: false },
  stockSecured: true,
  toolSecured: true,
  toolRemoved: false,
  xyzZeroVerified: true,
  safeZVerified: true,
  manualSpindleRunning: true,
  manualSpindleOff: false,
  probeRemoved: true,
  pathClear: true,
  powerControlReachable: true,
};

const clearReport = { ready: true } as RunPreflightReport;

describe("firstCutAuthorizationControls", () => {
  it("expands one operator readiness decision into intent-specific typed facts", () => {
    const airRun = setFirstCutReadiness(emptyFirstCutConfirmation, true);
    expect(airRun.toolRemoved).toBe(true);
    expect(airRun.manualSpindleOff).toBe(true);
    expect(airRun.stockSecured).toBe(false);
    expect(firstCutAuthorizationControls(airRun, {
      report: clearReport,
      gatewayAvailable: true,
      busy: false,
    }).complete).toBe(true);

    const cutting = setFirstCutReadiness({ ...emptyFirstCutConfirmation, intent: "cutting" }, true);
    expect(cutting.stockSecured).toBe(true);
    expect(cutting.toolSecured).toBe(true);
    expect(cutting.manualSpindleRunning).toBe(true);
    expect(cutting.toolRemoved).toBe(false);
  });

  it("requires the physical confirmations for the selected intent", () => {
    expect(
      firstCutAuthorizationControls(emptyFirstCutConfirmation, {
        report: clearReport,
        gatewayAvailable: true,
        busy: false,
      }),
    ).toEqual({ completedCount: 0, totalCount: 6, complete: false, canAuthorize: false });

    expect(
      firstCutAuthorizationControls(
        { ...complete, powerControlReachable: false },
        { report: clearReport, gatewayAvailable: true, busy: false },
      ),
    ).toEqual({ completedCount: 6, totalCount: 7, complete: false, canAuthorize: false });
  });

  it("fails closed for stale, blocked, missing-gateway and busy states", () => {
    expect(
      firstCutAuthorizationControls(complete, {
        report: { ...clearReport, ready: false },
        gatewayAvailable: true,
        busy: false,
      }).canAuthorize,
    ).toBe(false);
    expect(
      firstCutAuthorizationControls(complete, {
        report: clearReport,
        gatewayAvailable: false,
        busy: false,
      }).canAuthorize,
    ).toBe(false);
    expect(
      firstCutAuthorizationControls(complete, {
        report: clearReport,
        gatewayAvailable: true,
        busy: true,
      }).canAuthorize,
    ).toBe(false);
  });

  it("enables only the authorization action after every gate is complete", () => {
    expect(
      firstCutAuthorizationControls(complete, {
        report: clearReport,
        gatewayAvailable: true,
        busy: false,
      }),
    ).toEqual({ completedCount: 7, totalCount: 7, complete: true, canAuthorize: true });
  });

  it("uses tool-removed and spindle-off checks for an air run", () => {
    const airRun: FirstCutConfirmation = {
      ...emptyFirstCutConfirmation,
      intent: "airRun",
      toolRemoved: true,
      manualSpindleOff: true,
      probeRemoved: false,
      xyzZeroVerified: true,
      safeZVerified: true,
      pathClear: true,
      powerControlReachable: true,
    };

    expect(
      firstCutAuthorizationControls(airRun, {
        report: clearReport,
        gatewayAvailable: true,
        busy: false,
      }),
    ).toEqual({ completedCount: 6, totalCount: 6, complete: true, canAuthorize: true });
  });

  it("requires the probe plate and wire to be removed for a heightmap cutting run", () => {
    const withHeightmap: FirstCutConfirmation = {
      ...complete,
      executionOptions: {
        ...complete.executionOptions,
        surfaceMapId: 4,
      },
      probeRemoved: false,
    };

    expect(
      firstCutAuthorizationControls(withHeightmap, {
        report: clearReport,
        gatewayAvailable: true,
        busy: false,
      }),
    ).toEqual({ completedCount: 7, totalCount: 8, complete: false, canAuthorize: false });

    expect(
      firstCutAuthorizationControls(
        { ...withHeightmap, probeRemoved: true },
        { report: clearReport, gatewayAvailable: true, busy: false },
      ),
    ).toEqual({ completedCount: 8, totalCount: 8, complete: true, canAuthorize: true });
  });
});
