export interface SnapshotStream<T> {
  readCurrent(): Promise<T>;
  listen(listener: (snapshot: T) => void): Promise<() => void>;
}

export interface SnapshotBindingOptions<T> {
  readonly stream: SnapshotStream<T>;
  readonly onSnapshot: (snapshot: T) => void;
  readonly onError?: (error: unknown) => void;
}

export function bindSnapshotStream<T>(
  options: SnapshotBindingOptions<T>,
): () => void {
  let active = true;
  let eventRevision = 0;
  let unlisten: (() => void) | undefined;

  const publish = (snapshot: T): void => {
    if (!active) return;
    try {
      options.onSnapshot(snapshot);
    } catch (error) {
      reportSafely(options.onError, error);
    }
  };

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

    // Subscribe before reading: otherwise an event between the two is lost.
    // An event received during listener setup already supplies initial state.
    if (!active || eventRevision > 0) return;
    const revisionBeforeInitialRead = eventRevision;
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
