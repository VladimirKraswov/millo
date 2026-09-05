import { describe, expect, it, vi } from "vitest";

import type { ControllerSnapshot } from "../../shared/machine";
import { emptySnapshot } from "../../shared/machine";
import { MachineSnapshotStore } from "./MachineStateSource";

function snapshot(pollSequence: number): ControllerSnapshot {
  return {
    ...emptySnapshot,
    homing: { ...emptySnapshot.homing },
    connection: "connected",
    machine: {
      ...emptySnapshot.machine,
      mode: "idle",
      reportedMode: "Idle",
      machinePosition: { x: pollSequence, y: 2, z: 3 },
    },
    pollSequence,
  };
}

describe("MachineSnapshotStore", () => {
  it("clones and deeply freezes snapshots at the host boundary", () => {
    const input: ControllerSnapshot = {
      ...snapshot(1),
      machine: {
        ...snapshot(1).machine,
        workPosition: { x: 4, y: 5, z: 6 },
        workCoordinateOffset: { x: 7, y: 8, z: 9 },
        bufferState: { plannerAvailable: 15, rxAvailable: 128 },
        overrides: { feedPercent: 100, rapidPercent: 100, spindlePercent: 100 },
      },
      resetNotice: { banner: "Grbl 1.1f", sequence: 1 },
      alarm: { code: 3, message: "Reset while in motion" },
    };
    const store = new MachineSnapshotStore(input);
    const current = store.current();

    input.machine.reportedMode = "Mutated";
    input.machine.machinePosition!.x = 99;
    input.resetNotice!.banner = "Mutated";
    input.alarm!.message = "Mutated";
    input.homing.state = "homed";
    input.machine.bufferState!.rxAvailable = 0;
    input.machine.overrides!.feedPercent = 200;

    expect(current.machine.reportedMode).toBe("Idle");
    expect(current.machine.machinePosition?.x).toBe(1);
    expect(current.resetNotice?.banner).toBe("Grbl 1.1f");
    expect(current.alarm?.message).toBe("Reset while in motion");
    expect(Object.isFrozen(current)).toBe(true);
    expect(Object.isFrozen(current.machine)).toBe(true);
    expect(Object.isFrozen(current.machine.machinePosition)).toBe(true);
    expect(Object.isFrozen(current.machine.workPosition)).toBe(true);
    expect(Object.isFrozen(current.machine.workCoordinateOffset)).toBe(true);
    expect(Object.isFrozen(current.resetNotice)).toBe(true);
    expect(Object.isFrozen(current.alarm)).toBe(true);
    expect(current.homing.state).toBe("unreferenced");
    expect(current.machine.bufferState?.rxAvailable).toBe(128);
    expect(current.machine.overrides?.feedPercent).toBe(100);
    expect(Object.isFrozen(current.homing)).toBe(true);
    expect(Object.isFrozen(current.machine.overrides)).toBe(true);
    expect(() => {
      (current as ControllerSnapshot).pollSequence = 99;
    }).toThrow();
  });

  it("publishes future snapshots until an idempotent unsubscribe", () => {
    const store = new MachineSnapshotStore(snapshot(0));
    const listener = vi.fn();
    const unsubscribe = store.subscribe(listener);

    store.publish(snapshot(1));
    unsubscribe();
    unsubscribe();
    store.publish(snapshot(2));

    expect(listener).toHaveBeenCalledOnce();
    expect(listener.mock.calls[0]?.[0].pollSequence).toBe(1);
  });

  it("delivers state when a subscriber or its error reporter throws", () => {
    const store = new MachineSnapshotStore(snapshot(0), () => { throw new Error("reporter"); });
    const listener = vi.fn();
    store.subscribe(() => { throw new Error("plugin"); });
    store.subscribe(listener);
    expect(() => store.publish(snapshot(2))).not.toThrow();
    expect(listener.mock.calls[0]?.[0].pollSequence).toBe(2);
  });

  it("does not call an observer unloaded during the current dispatch", () => {
    const store = new MachineSnapshotStore(snapshot(0));
    const listener = vi.fn();
    store.subscribe(() => unsubscribe());
    const unsubscribe = store.subscribe(listener);
    store.publish(snapshot(1));
    expect(listener).not.toHaveBeenCalled();
  });
});
