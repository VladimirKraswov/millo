import { describe, expect, it } from "vitest";

import type { SenderState } from "../../shared/dryRun";
import {
  physicalSenderActionLayout,
  senderActionLayout,
  senderRunIsVisibleForProgram,
} from "./operatorLayoutModel";

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

  it("keeps physical run controls explicit through pause and interruption", () => {
    expect(physicalSenderActionLayout("running")).toEqual({
      primary: "pause",
      stopVisible: true,
    });
    expect(physicalSenderActionLayout("paused")).toEqual({
      primary: "resume",
      stopVisible: true,
    });
    expect(physicalSenderActionLayout("toolChange")).toEqual({
      primary: "toolChange",
      stopVisible: true,
    });
    expect(physicalSenderActionLayout("cancelled")).toEqual({
      primary: "resolveInterruption",
      stopVisible: false,
    });
    expect(physicalSenderActionLayout("completed")).toEqual({
      primary: "prepareRerun",
      stopVisible: false,
    });
  });

  it("does not resurrect a terminal run after the UI consumed its sequence", () => {
    const terminal = { runSequence: 42, sourceName: "engraving.nc" };

    expect(senderRunIsVisibleForProgram(terminal, "engraving.nc", undefined)).toBe(true);
    expect(senderRunIsVisibleForProgram(terminal, "engraving.nc", 42)).toBe(false);
    expect(senderRunIsVisibleForProgram(terminal, "next.nc", undefined)).toBe(false);
  });
});
