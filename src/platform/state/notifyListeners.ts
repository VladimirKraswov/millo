export function notifyListeners<T>(
  listeners: ReadonlySet<(value: T) => void>,
  value: T,
  onError: (error: unknown) => void = console.error,
): void {
  // Additions wait for the next publication; removals take effect immediately.
  for (const listener of [...listeners]) {
    if (!listeners.has(listener)) continue;
    try {
      listener(value);
    } catch (error) {
      try {
        onError(error);
      } catch {
        // Diagnostics cannot interrupt delivery to the remaining observers.
      }
    }
  }
}
