import { describe, expect, it } from "vitest";

import type { SenderState } from "../../shared/dryRun";
import { senderActionLayout } from "./operatorLayoutModel";

describe("operator layout model", () => {
  it("keeps one primary and one cancel slot through every sender state", () => {
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

    expect(states.map((state) => [state, senderActionLayout(state)])).toEqual([
      ["idle", { primary: "start", cancelVisible: false }],
      ["ready", { primary: "start", cancelVisible: false }],
      ["running", { primary: "pause", cancelVisible: true }],
      ["paused", { primary: "resume", cancelVisible: true }],
      ["toolChange", { primary: "none", cancelVisible: true }],
      ["draining", { primary: "none", cancelVisible: true }],
      ["completed", { primary: "start", cancelVisible: false }],
      ["failed", { primary: "start", cancelVisible: false }],
      ["cancelled", { primary: "start", cancelVisible: false }],
    ]);
  });
});
