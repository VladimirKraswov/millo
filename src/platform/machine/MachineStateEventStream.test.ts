import { describe, expect, it, vi } from "vitest";

import type { ControllerSnapshot } from "../../shared/machine";
import { emptySnapshot } from "../../shared/machine";
import {
  bindMachineStateStream,
  type MachineStateEventStream,
} from "./MachineStateEventStream";
import { MachineSnapshotStore } from "./MachineStateSource";

function snapshot(pollSequence: number): ControllerSnapshot {
  return {
    ...emptySnapshot,
    connection: "connected",
    machine: {
      ...emptySnapshot.machine,
      mode: "idle",
      reportedMode: "Idle",
    },
    pollSequence,
  };
}

function deferred<T>(): {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
} {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((complete) => {
    resolve = complete;
  });
  return { promise, resolve };
}

const flushPromises = (): Promise<void> =>
  new Promise((resolve) => queueMicrotask(resolve));

describe("bindMachineStateStream", () => {
  it("publishes the initial Tauri snapshot and subsequent events", async () => {
    let listener: ((value: ControllerSnapshot) => void) | undefined;
    const cleanup = vi.fn();
    const stream: MachineStateEventStream = {
      readCurrent: async () => snapshot(1),
      listen: async (next) => {
        listener = next;
        return cleanup;
      },
    };
    const store = new MachineSnapshotStore(emptySnapshot);
    const dispose = bindMachineStateStream({ stream, store });

    await flushPromises();
    expect(store.current().pollSequence).toBe(1);

    listener?.(snapshot(2));
    expect(store.current().pollSequence).toBe(2);

    dispose();
    expect(cleanup).toHaveBeenCalledOnce();
  });

  it("does not overwrite an early event with a late initial response", async () => {
    const initial = deferred<ControllerSnapshot>();
    const stream: MachineStateEventStream = {
      readCurrent: () => initial.promise,
      listen: async (listener) => {
        listener(snapshot(2));
        return () => undefined;
      },
    };
    const store = new MachineSnapshotStore(emptySnapshot);
    const dispose = bindMachineStateStream({ stream, store });

    initial.resolve(snapshot(1));
    await flushPromises();

    expect(store.current().pollSequence).toBe(2);
    dispose();
  });

  it("cleans up a listener that resolves after the binding is disposed", async () => {
    const initial = deferred<ControllerSnapshot>();
    const listening = deferred<() => void>();
    const cleanup = vi.fn();
    let listener: ((value: ControllerSnapshot) => void) | undefined;
    const stream: MachineStateEventStream = {
      readCurrent: () => initial.promise,
      listen: (next) => {
        listener = next;
        return listening.promise;
      },
    };
    const store = new MachineSnapshotStore(emptySnapshot);
    const dispose = bindMachineStateStream({ stream, store });

    dispose();
    listener?.(snapshot(2));
    initial.resolve(snapshot(1));
    listening.resolve(cleanup);
    await flushPromises();

    expect(store.current().pollSequence).toBe(0);
    expect(cleanup).toHaveBeenCalledOnce();
  });
});
