import { describe, expect, it } from "vitest";

import { idleSenderSnapshot, type SenderSnapshot } from "../../shared/dryRun";
import { dryRunControls, senderTiming } from "./dryRunReadModel";

const context = {
  mockAvailable: true,
  policyEligible: true,
  loading: false,
};

const sender = (overrides: Partial<SenderSnapshot>): SenderSnapshot => ({
  ...idleSenderSnapshot,
  ...overrides,
});

describe("dryRunControls", () => {
  it("requires both Mock GRBL and a policy-eligible program", () => {
    expect(dryRunControls(idleSenderSnapshot, context).canStart).toBe(true);
    expect(
      dryRunControls(idleSenderSnapshot, { ...context, mockAvailable: false })
        .canStart,
    ).toBe(false);
    expect(
      dryRunControls(idleSenderSnapshot, { ...context, policyEligible: false })
        .canStart,
    ).toBe(false);
  });

  it("exposes only state-valid sender controls", () => {
    expect(dryRunControls(sender({ state: "running" }), context)).toMatchObject({
      canStart: false,
      canPause: true,
      canResume: false,
      canCancel: true,
    });
    expect(dryRunControls(sender({ state: "paused" }), context)).toMatchObject({
      canPause: false,
      canResume: true,
      canCancel: true,
    });
  });

  it("clamps untrusted progress for display", () => {
    expect(
      dryRunControls(sender({ progress: 1.4 }), context).progressPercent,
    ).toBe(100);
    expect(
      dryRunControls(sender({ progress: -0.2 }), context).progressPercent,
    ).toBe(0);
  });

  it("formats active time and distinguishes a lower-bound estimate", () => {
    expect(
      senderTiming(
        sender({
          elapsedSeconds: 65.4,
          estimatedRemainingSeconds: 3_661,
          timeEstimateComplete: false,
        }),
      ),
    ).toEqual({ elapsed: "1:05", estimateLabel: "ETA >=", remaining: "1:01:01" });
    expect(
      senderTiming(
        sender({ estimatedRemainingSeconds: 9.6, timeEstimateComplete: true }),
      ).estimateLabel,
    ).toBe("ETA");
  });
});
