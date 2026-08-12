import { describe, expect, it } from "vitest";

import {
  emptyToolChangeConfirmation,
  setToolChangeReadiness,
  toolChangeConfirmationProgress,
} from "./toolChangeConfirmationModel";

describe("tool change confirmation", () => {
  it("expands one operator decision into every resume-critical fact", () => {
    const confirmation = setToolChangeReadiness(
      emptyToolChangeConfirmation(18, 4),
      true,
    );
    expect(toolChangeConfirmationProgress(confirmation).complete).toBe(true);
    expect(confirmation.sourceLine).toBe(18);
    expect(confirmation.requestedTool).toBe(4);
  });

  it("binds the operator facts to the active source line and tool", () => {
    const confirmation = emptyToolChangeConfirmation(18, 4);

    expect(confirmation.sourceLine).toBe(18);
    expect(confirmation.requestedTool).toBe(4);
    expect(toolChangeConfirmationProgress(confirmation)).toEqual({
      completed: 0,
      total: 6,
      complete: false,
    });
  });

  it("becomes complete only when every resume-critical fact is true", () => {
    expect(
      toolChangeConfirmationProgress({
        ...emptyToolChangeConfirmation(18, 4),
        toolSecured: true,
        zZeroVerified: true,
        safeZVerified: true,
        pathClear: true,
        manualSpindleRunning: true,
        powerControlReachable: true,
      }),
    ).toEqual({ completed: 6, total: 6, complete: true });
  });
});
