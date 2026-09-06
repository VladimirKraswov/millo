import { describe, expect, it, vi } from "vitest";

import type { ToolLibraryState } from "../../shared/tooling";
import { newToolDraft } from "../../shared/tooling";
import type { ToolLibraryGateway } from "./ToolLibraryGateway";
import { ToolLibraryService } from "./ToolLibraryService";

const state = (revision: number): ToolLibraryState => ({
  revision,
  tools: revision === 0 ? [] : [{
    id: "tool-0001",
    ...newToolDraft(),
    factoryPreset: false,
  }],
});

describe("ToolLibraryService", () => {
  it("ignores a late load or mutation response older than the current native revision", async () => {
    let finishLoad!: (state: ToolLibraryState) => void;
    const load = new Promise<ToolLibraryState>((resolve) => { finishLoad = resolve; });
    const gateway: ToolLibraryGateway = {
      load: vi.fn(() => load),
      create: vi.fn(async () => state(2)),
      update: vi.fn(async () => state(1)),
      delete: vi.fn(), restorePresets: vi.fn(),
    };
    const service = new ToolLibraryService(gateway);
    const listener = vi.fn();
    service.subscribe(listener);
    const loading = service.initialize();
    const newest = await service.create(newToolDraft());
    finishLoad(state(1));
    expect(await loading).toBe(newest);
    expect(await service.update("tool-0001", newToolDraft())).toBe(newest);
    expect(service.current()).toBe(newest);
    expect(listener).toHaveBeenCalledOnce();
  });

  it("clones partially frozen input and isolates failing observers and diagnostics", async () => {
    const reference = { manufacturer: "Fixture", product: "Cutter", url: "https://example.com/tool" };
    const input = { ...state(1), tools: [Object.freeze({ ...state(1).tools[0], reference })] };
    const gateway: ToolLibraryGateway = {
      load: vi.fn(async () => input), create: vi.fn(), update: vi.fn(), delete: vi.fn(), restorePresets: vi.fn(),
    };
    const report = vi.fn(() => { throw new Error("report failed"); });
    const service = new ToolLibraryService(gateway, report);
    service.subscribe(() => { throw new Error("plugin failed"); });
    const listener = vi.fn();
    service.subscribe(listener);
    const current = await service.initialize();
    expect(current.tools[0]).not.toBe(input.tools[0]);
    expect(current.tools[0].reference).not.toBe(reference);
    expect(Object.isFrozen(current.tools[0].reference)).toBe(true);
    expect(Object.isFrozen(reference)).toBe(false);
    expect(listener).toHaveBeenCalledWith(current);
    expect(report).toHaveBeenCalledOnce();
    expect(Object.isFrozen(input.tools)).toBe(false);
  });

  it("publishes immutable native snapshots after every mutation", async () => {
    const gateway: ToolLibraryGateway = {
      load: vi.fn(async () => state(1)),
      create: vi.fn(async () => state(2)),
      update: vi.fn(async () => state(3)),
      delete: vi.fn(async () => state(4)),
      restorePresets: vi.fn(async () => state(5)),
    };
    const service = new ToolLibraryService(gateway);
    const listener = vi.fn();
    service.subscribe(listener);

    await service.initialize();
    await service.create(newToolDraft());

    expect(service.current().revision).toBe(2);
    expect(listener).toHaveBeenCalledTimes(2);
    expect(Object.isFrozen(service.current())).toBe(true);
    expect(Object.isFrozen(service.current().tools)).toBe(true);
    expect(gateway.create).toHaveBeenCalledWith(newToolDraft());
  });
});
