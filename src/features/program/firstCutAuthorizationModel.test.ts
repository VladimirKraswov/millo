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
  stockSecured: true,
  toolSecured: true,
  xyzZeroVerified: true,
  safeZVerified: true,
  manualSpindleRunning: true,
  powerControlReachable: true,
};

const clearReport = { ready: true } as RunPreflightReport;

describe("firstCutAuthorizationControls", () => {
  it("requires all six independent physical confirmations", () => {
    expect(
      firstCutAuthorizationControls(emptyFirstCutConfirmation, {
        report: clearReport,
        gatewayAvailable: true,
        busy: false,
      }),
    ).toEqual({ completedCount: 0, complete: false, canAuthorize: false });

    expect(
      firstCutAuthorizationControls(
        { ...complete, powerControlReachable: false },
        { report: clearReport, gatewayAvailable: true, busy: false },
      ),
    ).toEqual({ completedCount: 5, complete: false, canAuthorize: false });
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
    ).toEqual({ completedCount: 6, complete: true, canAuthorize: true });
  });
});
