import { describe, expect, it, vi } from "vitest";

import type { ControllerSnapshot } from "../../shared/machine";
import { emptySnapshot } from "../../shared/machine";
import { MachineSnapshotStore } from "./MachineStateSource";

function snapshot(pollSequence: number): ControllerSnapshot {
  return {
    ...emptySnapshot,
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
});
