import { describe, expect, it } from "vitest";

import { jobReadinessModel, type JobReadinessInput } from "./jobReadinessModel";

const readyInput: JobReadinessInput = {
  alarm: false,
  connection: "connected",
  machineBound: true,
  machineSyncing: false,
  machineMode: "idle",
  parserEligible: true,
  preflightStatus: "ready",
  resetPending: false,
  recoveryStatus: "clear",
  requiresGrblCheck: false,
  workPositionAvailable: true,
};

describe("jobReadinessModel", () => {
  it("leads a disconnected operator to the connection action", () => {
    const view = jobReadinessModel({
      ...readyInput,
      connection: "disconnected",
    });

    expect(view.primaryAction).toBe("connect");
    expect(view.steps[0]).toEqual({ id: "machine", state: "action" });
  });

  it("prioritizes Alarm unlock over every program action", () => {
    const view = jobReadinessModel({
      ...readyInput,
      alarm: true,
      machineMode: "alarm",
      preflightStatus: "unchecked",
      workPositionAvailable: false,
    });

    expect(view.primaryAction).toBe("unlock");
    expect(view.primaryLabel).toBe("Разблокировать станок");
  });

  it("asks for a work zero before controller validation", () => {
    const view = jobReadinessModel({
      ...readyInput,
      preflightStatus: "unchecked",
      workPositionAvailable: false,
    });

    expect(view.primaryAction).toBe("setWorkZero");
    expect(view.steps[2]).toEqual({ id: "origin", state: "action" });
  });

  it("shows connected profile synchronization as progress instead of an error", () => {
    const view = jobReadinessModel({
      ...readyInput,
      machineBound: false,
      machineSyncing: true,
    });

    expect(view.steps[0]).toEqual({ id: "machine", state: "pending" });
    expect(view.primaryAction).toBe("syncMachine");
    expect(view.primaryLabel).toBe("Синхронизируем профиль...");
    expect(view.primaryDisabled).toBe(true);
  });

  it("offers a recovery action when a connected controller is not bound", () => {
    const view = jobReadinessModel({
      ...readyInput,
      machineBound: false,
    });

    expect(view.steps[0]).toEqual({ id: "machine", state: "action" });
    expect(view.primaryAction).toBe("syncMachine");
    expect(view.primaryLabel).toBe("Определить подключённый станок");
    expect(view.primaryDisabled).toBe(false);
  });

  it("routes a missing cutting certificate to GRBL Check", () => {
    const view = jobReadinessModel({
      ...readyInput,
      preflightStatus: "blocked",
      requiresGrblCheck: true,
    });

    expect(view.primaryAction).toBe("runGrblCheck");
  });

  it("blocks a new start until persisted recovery is resolved", () => {
    const view = jobReadinessModel({
      ...readyInput,
      recoveryStatus: "outstanding",
    });

    expect(view.primaryAction).toBe("resolveRecovery");
    expect(view.steps[3]).toEqual({ id: "validation", state: "blocked" });
  });

  it("does not expose Start while recovery evidence is still loading", () => {
    const view = jobReadinessModel({
      ...readyInput,
      recoveryStatus: "checking",
    });

    expect(view.primaryAction).toBe("resolveRecovery");
    expect(view.primaryDisabled).toBe(true);
  });

  it("exposes one start action only after all readiness facts pass", () => {
    const view = jobReadinessModel(readyInput);

    expect(view.primaryAction).toBe("startProgram");
    expect(view.primaryDisabled).toBe(false);
    expect(view.steps.every((item) => item.state === "ready")).toBe(true);
  });
});
