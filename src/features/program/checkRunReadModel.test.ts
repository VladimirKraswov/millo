import { describe, expect, it } from "vitest";

import { idleSenderSnapshot } from "../../shared/dryRun";
import { canStartCheckRun, type CheckRunContext } from "./checkRunReadModel";

const ready: CheckRunContext = {
  gatewayAvailable: true,
  loading: false,
  programLoaded: true,
  serialAvailable: true,
};

describe("GRBL Check controls", () => {
  it("requires a loaded program, serial target, and typed gateway", () => {
    expect(canStartCheckRun(idleSenderSnapshot, ready)).toBe(true);
    expect(
      canStartCheckRun(idleSenderSnapshot, { ...ready, programLoaded: false }),
    ).toBe(false);
    expect(
      canStartCheckRun(idleSenderSnapshot, { ...ready, serialAvailable: false }),
    ).toBe(false);
    expect(
      canStartCheckRun(idleSenderSnapshot, { ...ready, gatewayAvailable: false }),
    ).toBe(false);
  });

  it("cannot replace an active sender", () => {
    for (const state of ["running", "paused", "draining"] as const) {
      expect(canStartCheckRun({ ...idleSenderSnapshot, state }, ready)).toBe(false);
    }
  });
});
