import { describe, expect, it } from "vitest";

import type { SenderState } from "../../shared/dryRun";
import { isSenderActive, isSenderTerminal } from "./senderStateModel";

const states: readonly SenderState[] = [
  "idle",
  "ready",
  "running",
  "paused",
  "toolChange",
  "draining",
  "completed",
  "failed",
  "cancelled",
];

describe("senderStateModel", () => {
  it.each(states.map((state) => [state, ["running", "paused", "toolChange", "draining"].includes(state)]))(
    "classifies %s activity",
    (state, expected) => expect(isSenderActive(state)).toBe(expected),
  );

  it.each(states.map((state) => [state, ["completed", "failed", "cancelled"].includes(state)]))(
    "classifies %s terminal state",
    (state, expected) => expect(isSenderTerminal(state)).toBe(expected),
  );
});
