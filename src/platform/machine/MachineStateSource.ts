import type { ControllerSnapshot } from "../../shared/machine";
import { notifyListeners } from "../state/notifyListeners";

export type DeepReadonly<T> = T extends (...args: never[]) => unknown
  ? T
  : T extends readonly (infer Item)[]
    ? readonly DeepReadonly<Item>[]
    : T extends object
      ? { readonly [Key in keyof T]: DeepReadonly<T[Key]> }
      : T;

export type ReadonlyControllerSnapshot = DeepReadonly<ControllerSnapshot>;
export type MachineStateListener = (
  snapshot: ReadonlyControllerSnapshot,
) => void;

export interface MachineStateSource {
  current(): ReadonlyControllerSnapshot;
  subscribe(listener: MachineStateListener): () => void;
}

export class MachineSnapshotStore implements MachineStateSource {
  private snapshot: ReadonlyControllerSnapshot;
  private readonly listeners = new Set<MachineStateListener>();

  constructor(
    initialSnapshot: ControllerSnapshot,
    private readonly onListenerError: (error: unknown) => void = console.error,
  ) {
    this.snapshot = freezeSnapshot(initialSnapshot);
  }

  current = (): ReadonlyControllerSnapshot => this.snapshot;

  subscribe = (listener: MachineStateListener): (() => void) => {
    this.listeners.add(listener);
    let active = true;
    return () => {
      if (!active) return;
      active = false;
      this.listeners.delete(listener);
    };
  };

  publish = (snapshot: ControllerSnapshot): void => {
    this.snapshot = freezeSnapshot(snapshot);
    const current = this.snapshot;
    notifyListeners(this.listeners, current, this.onListenerError);
  };
}

function freezeSnapshot(
  snapshot: ControllerSnapshot,
): ReadonlyControllerSnapshot {
  return freezeTree(structuredClone(snapshot));
}

function freezeTree<T>(value: T): DeepReadonly<T> {
  if (value !== null && typeof value === "object") {
    for (const child of Object.values(value)) freezeTree(child);
    Object.freeze(value);
  }
  return value as DeepReadonly<T>;
}
