import { expect, it, vi } from "vitest";
import { bindSnapshotStream } from "./bindSnapshotStream";

const flush = () => new Promise<void>((resolve) => queueMicrotask(resolve));

it("does not roll back an event received while the initial request is pending", async () => {
  let publish!: (value: number) => void;
  let resolve!: (value: number) => void;
  const read = new Promise<number>((done) => { resolve = done; });
  const received = vi.fn();
  const dispose = bindSnapshotStream({
    stream: { listen: async (listener) => { publish = listener; return () => {}; }, readCurrent: () => read },
    onSnapshot: received,
  });
  await flush();
  publish(2);
  resolve(1);
  await flush();
  expect(received.mock.calls).toEqual([[2]]);
  dispose();
});

it("isolates observer and cleanup errors without an unhandled rejection", async () => {
  const onError = vi.fn(() => { throw new Error("reporting failed"); });
  const cleanup = vi.fn(() => { throw new Error("cleanup failed"); });
  const dispose = bindSnapshotStream({
    stream: { listen: async () => cleanup, readCurrent: async () => 1 },
    onSnapshot: () => { throw new Error("observer failed"); },
    onError,
  });
  await flush();
  await flush();
  expect(onError).toHaveBeenCalledTimes(1);
  expect(() => { dispose(); dispose(); }).not.toThrow();
  expect(onError).toHaveBeenCalledTimes(2);
  expect(cleanup).toHaveBeenCalledOnce();
});
