import type { ControllerSnapshot } from "../../shared/machine";
import type {
  MachineSnapshotStore,
  ReadonlyControllerSnapshot,
} from "./MachineStateSource";

export interface MachineStateEventStream {
  readCurrent(): Promise<ControllerSnapshot>;
  listen(listener: (snapshot: ControllerSnapshot) => void): Promise<() => void>;
}

export interface MachineStateStreamBindingOptions {
  readonly stream: MachineStateEventStream;
  readonly store: MachineSnapshotStore;
  readonly onSnapshot?: (snapshot: ReadonlyControllerSnapshot) => void;
  readonly onError?: (error: unknown) => void;
}

export function bindMachineStateStream(
  options: MachineStateStreamBindingOptions,
): () => void {
  let active = true;
  let eventRevision = 0;
  let unlisten: (() => void) | undefined;

  const publish = (snapshot: ControllerSnapshot): void => {
    if (!active) return;
    try {
      options.store.publish(snapshot);
      options.onSnapshot?.(options.store.current());
    } catch (error) {
      reportSafely(options.onError, error);
    }
  };

  const revisionBeforeInitialRead = eventRevision;
  void (async () => {
    try {
      const cleanup = await options.stream.listen((snapshot) => {
        eventRevision += 1;
        publish(snapshot);
      });
      if (active) {
        unlisten = cleanup;
      } else {
        cleanupSafely(cleanup, options.onError);
      }
    } catch (error) {
      if (active) reportSafely(options.onError, error);
    }
  })();

  void (async () => {
    try {
      const snapshot = await options.stream.readCurrent();
      if (eventRevision === revisionBeforeInitialRead) publish(snapshot);
    } catch (error) {
      if (active) reportSafely(options.onError, error);
    }
  })();

  return () => {
    if (!active) return;
    active = false;
    if (unlisten) cleanupSafely(unlisten, options.onError);
    unlisten = undefined;
  };
}

function cleanupSafely(
  cleanup: () => void,
  onError?: (error: unknown) => void,
): void {
  try {
    cleanup();
  } catch (error) {
    reportSafely(onError, error);
  }
}

function reportSafely(
  onError: ((error: unknown) => void) | undefined,
  error: unknown,
): void {
  try {
    onError?.(error);
  } catch {
    // Stream diagnostics must not disturb lifecycle cleanup.
  }
}
