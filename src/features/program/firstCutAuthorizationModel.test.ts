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
const availableContext = {
  report: clearReport,
  gatewayAvailable: true,
  busy: false,
} as const;

describe("firstCutAuthorizationControls", () => {
  it("expands one operator readiness decision into intent-specific typed facts", () => {
    const airRun = setFirstCutReadiness(emptyFirstCutConfirmation, true);
    expect(airRun.toolRemoved).toBe(true);
    expect(airRun.manualSpindleOff).toBe(true);
    expect(airRun.stockSecured).toBe(false);
    expect(firstCutAuthorizationControls(airRun, availableContext).complete).toBe(true);

    const cutting = setFirstCutReadiness({ ...emptyFirstCutConfirmation, intent: "cutting" }, true);
    expect(cutting.stockSecured).toBe(true);
    expect(cutting.toolSecured).toBe(true);
    expect(cutting.manualSpindleRunning).toBe(true);
    expect(cutting.toolRemoved).toBe(false);
  });

  it("requires the physical confirmations for the selected intent", () => {
    expect(
      firstCutAuthorizationControls(emptyFirstCutConfirmation, availableContext),
    ).toEqual({ completedCount: 0, totalCount: 6, complete: false, canAuthorize: false });

    expect(
      firstCutAuthorizationControls(
        { ...complete, powerControlReachable: false },
        availableContext,
      ),
    ).toEqual({ completedCount: 6, totalCount: 7, complete: false, canAuthorize: false });
  });

  it.each([
    ["blocked report", { ...availableContext, report: { ...clearReport, ready: false } }],
    ["missing gateway", { ...availableContext, gatewayAvailable: false }],
    ["busy host", { ...availableContext, busy: true }],
  ])("fails closed for %s", (_label, context) => {
    expect(firstCutAuthorizationControls(complete, context).canAuthorize).toBe(false);
  });

  it("enables only the authorization action after every gate is complete", () => {
    expect(
      firstCutAuthorizationControls(complete, availableContext),
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
      firstCutAuthorizationControls(airRun, availableContext),
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
      firstCutAuthorizationControls(withHeightmap, availableContext),
    ).toEqual({ completedCount: 7, totalCount: 8, complete: false, canAuthorize: false });

    expect(
      firstCutAuthorizationControls(
        { ...withHeightmap, probeRemoved: true },
        availableContext,
      ),
    ).toEqual({ completedCount: 8, totalCount: 8, complete: true, canAuthorize: true });
  });
});
