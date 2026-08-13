import { describe, expect, it } from "vitest";

import {
  hasControllerSession,
  isControllerConnected,
  isControllerStableIdle,
} from "./controllerReadiness";
import { emptySnapshot, type ControllerSnapshot } from "./machine";

const snapshot = (overrides: Partial<ControllerSnapshot>): ControllerSnapshot => ({
  ...emptySnapshot,
  ...overrides,
  machine: { ...emptySnapshot.machine, ...overrides.machine },
});

describe("controller readiness", () => {
  it.each([
    ["disconnected", false, false],
    ["connecting", false, false],
    ["connected", true, true],
    ["recovering", false, true],
    ["faulted", false, false],
  ] as const)("classifies a %s connection", (connection, connected, session) => {
    const value = snapshot({ connection });
    expect(isControllerConnected(value)).toBe(connected);
    expect(hasControllerSession(value)).toBe(session);
  });

  it("accepts only a clean connected Idle snapshot for motion authorization", () => {
    const idle = snapshot({
      connection: "connected",
      machine: { ...emptySnapshot.machine, mode: "idle" },
    });

    expect(isControllerStableIdle(idle)).toBe(true);
    expect(isControllerStableIdle(snapshot({ ...idle, connection: "recovering" }))).toBe(false);
    expect(
      isControllerStableIdle({ ...idle, alarm: { code: 1, message: "Hard limit" } }),
    ).toBe(false);
    expect(
      isControllerStableIdle({
        ...idle,
        resetNotice: { banner: "Grbl 1.1h", sequence: 1 },
      }),
    ).toBe(false);
    expect(
      isControllerStableIdle({ ...idle, machine: { ...idle.machine, mode: "run" } }),
    ).toBe(false);
  });
});
