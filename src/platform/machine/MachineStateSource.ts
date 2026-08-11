import type { ControllerSnapshot, Position } from "../../shared/machine";

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

  constructor(initialSnapshot: ControllerSnapshot) {
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

  publish(snapshot: ControllerSnapshot): void {
    this.snapshot = freezeSnapshot(snapshot);
    for (const listener of [...this.listeners]) listener(this.snapshot);
  }
}

function freezeSnapshot(
  snapshot: ControllerSnapshot,
): ReadonlyControllerSnapshot {
  const machine = Object.freeze({
    ...snapshot.machine,
    machinePosition: freezePosition(snapshot.machine.machinePosition),
    workPosition: freezePosition(snapshot.machine.workPosition),
    workCoordinateOffset: freezePosition(snapshot.machine.workCoordinateOffset),
  });
  return Object.freeze({
    ...snapshot,
    machine,
    resetNotice: snapshot.resetNotice
      ? Object.freeze({ ...snapshot.resetNotice })
      : undefined,
    alarm: snapshot.alarm ? Object.freeze({ ...snapshot.alarm }) : undefined,
  });
}

function freezePosition(position?: Position): Readonly<Position> | undefined {
  return position ? Object.freeze({ ...position }) : undefined;
}
