import { describe, expect, it } from "vitest";

import { canPrepareSafeStart, suggestedSafeZ } from "./safeStartModel";

describe("safeStartModel", () => {
  it("suggests a stable two millimetre clearance above program geometry", () => {
    expect(suggestedSafeZ(5)).toBe(7);
    expect(suggestedSafeZ(-0.2)).toBe(1.8);
    expect(suggestedSafeZ(undefined)).toBe(2);
  });

  it("requires motion and a finite clearance at or above the program", () => {
    expect(
      canPrepareSafeStart({
        busy: false,
        minimumSafeZ: 5,
        motionCount: 1,
        safeZ: 7,
        sourceLine: 42,
      }),
    ).toBe(true);
    expect(
      canPrepareSafeStart({
        busy: false,
        minimumSafeZ: 5,
        motionCount: 1,
        safeZ: 4.9,
        sourceLine: 42,
      }),
    ).toBe(false);
    expect(
      canPrepareSafeStart({
        busy: false,
        minimumSafeZ: 5,
        motionCount: 0,
        safeZ: 7,
        sourceLine: 42,
      }),
    ).toBe(false);
  });
});
