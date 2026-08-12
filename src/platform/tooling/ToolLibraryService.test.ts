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
