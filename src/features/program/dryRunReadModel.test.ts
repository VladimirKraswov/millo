import { describe, expect, it } from "vitest";

import { idleSenderSnapshot, type SenderSnapshot } from "../../shared/dryRun";
import {
  senderFailureSummary,
  senderHeartbeat,
  senderTiming,
} from "./dryRunReadModel";

const sender = (overrides: Partial<SenderSnapshot>): SenderSnapshot => ({
  ...idleSenderSnapshot,
  ...overrides,
});

describe("sender diagnostics read model", () => {
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

  it("renders typed sender failures without parsing controller text", () => {
    expect(
      senderFailureSummary(
        sender({
          lastError: "legacy text that may change",
          failure: {
            kind: "grblError",
            message: "program response failed: GRBL error 33",
            grblCode: 33,
            sourceLine: 12,
            command: "G2 X1 Y1 I0 J1",
          },
        }),
      ),
    ).toBe("GRBL error 33 · L12");
    expect(senderFailureSummary(sender({ lastError: "legacy" }))).toBe(
      "legacy",
    );
  });

  it("formats acknowledgement heartbeat and shutdown evidence", () => {
    expect(
      senderHeartbeat(
        sender({
          acknowledgedLines: 42,
          progressSequence: 42,
          lastAcknowledgedSourceLine: 38,
          secondsSinceAcknowledgement: 0.36,
          shutdownCommandsAcknowledged: true,
        }),
      ),
    ).toEqual({
      sequence: 42,
      lastLine: "L38",
      age: "0.4s",
      shutdownAcknowledged: true,
    });
    expect(
      senderHeartbeat(
        sender({
          acknowledgedLines: 2,
          secondsSinceAcknowledgement: Number.POSITIVE_INFINITY,
        }),
      ),
    ).toMatchObject({ sequence: 2, lastLine: "Guard", age: "0.0s" });
  });
});
