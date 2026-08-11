import { describe, expect, it } from "vitest";

import type {
  FirstCutConfirmation,
  RunPreflightReport,
} from "../../shared/realRun";
import {
  emptyFirstCutConfirmation,
  firstCutAuthorizationControls,
} from "./firstCutAuthorizationModel";

const complete: FirstCutConfirmation = {
  intent: "cutting",
  stockSecured: true,
  toolSecured: true,
  toolRemoved: false,
  xyzZeroVerified: true,
  safeZVerified: true,
  manualSpindleRunning: true,
  manualSpindleOff: false,
  pathClear: true,
  powerControlReachable: true,
};

const clearReport = { ready: true } as RunPreflightReport;

describe("firstCutAuthorizationControls", () => {
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
});
