import type { ControllerSnapshot } from "../../shared/machine";
import { bindSnapshotStream, type SnapshotStream } from "../state/bindSnapshotStream";
import type { MachineSnapshotStore, ReadonlyControllerSnapshot } from "./MachineStateSource";

export type MachineStateEventStream = SnapshotStream<ControllerSnapshot>;

export interface MachineStateStreamBindingOptions {
  readonly stream: MachineStateEventStream;
  readonly store: MachineSnapshotStore;
  readonly onSnapshot?: (snapshot: ReadonlyControllerSnapshot) => void;
  readonly onError?: (error: unknown) => void;
}

export function bindMachineStateStream(options: MachineStateStreamBindingOptions): () => void {
  return bindSnapshotStream({
    stream: options.stream,
    onSnapshot: (snapshot) => {
      options.store.publish(snapshot);
      options.onSnapshot?.(options.store.current());
    },
    onError: options.onError,
  });
}
