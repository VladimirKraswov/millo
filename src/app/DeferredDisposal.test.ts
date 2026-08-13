import { describe, expect, it, vi } from "vitest";

import { DeferredDisposal } from "./DeferredDisposal";

const nextMicrotask = (): Promise<void> => Promise.resolve();

describe("DeferredDisposal", () => {
  it("keeps a resource alive across a StrictMode cleanup and immediate remount", async () => {
    const dispose = vi.fn();
    const lifecycle = new DeferredDisposal(dispose);
    const releaseFirstMount = lifecycle.mount();

    releaseFirstMount();
    const releaseSecondMount = lifecycle.mount();
    await nextMicrotask();

    expect(dispose).not.toHaveBeenCalled();
    releaseSecondMount();
    await nextMicrotask();
    expect(dispose).toHaveBeenCalledOnce();
  });

  it("reports synchronous disposal failures", async () => {
    const onError = vi.fn();
    const lifecycle = new DeferredDisposal(() => {
      throw new Error("dispose failed");
    }, onError);

    lifecycle.mount()();
    await nextMicrotask();

    expect(onError).toHaveBeenCalledWith(expect.objectContaining({
      message: "dispose failed",
    }));
  });
});
