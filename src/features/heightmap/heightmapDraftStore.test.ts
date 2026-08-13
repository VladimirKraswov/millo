import { describe, expect, it } from "vitest";

import { initialHeightmapDraft, loadHeightmapDraft, saveHeightmapDraft } from "./heightmapDraftStore";

const memoryStorage = () => {
  const values = new Map<string, string>();
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
  };
};

describe("heightmapDraftStore", () => {
  it("round-trips a machine-specific operator draft", () => {
    const storage = memoryStorage();
    const draft = { ...initialHeightmapDraft(), surfaceSearchMm: 18, zeroPlateThicknessMm: 19.1 };
    saveHeightmapDraft("machine-1", draft, storage);
    expect(loadHeightmapDraft("machine-1", storage)?.surfaceSearchMm).toBe(18);
    expect(loadHeightmapDraft("machine-1", storage)?.zeroPlateThicknessMm).toBe(19.1);
    expect(loadHeightmapDraft("machine-2", storage)).toBeUndefined();
  });

  it("ignores malformed persisted input", () => {
    const storage = memoryStorage();
    storage.setItem("millo.heightmap-draft.v2.machine-1", "{broken");
    expect(loadHeightmapDraft("machine-1", storage)).toBeUndefined();
  });

  it("loads older v2 drafts without preserving the removed shape preset", () => {
    const storage = memoryStorage();
    const legacy = { ...initialHeightmapDraft(), surfaceShape: "relief" };
    storage.setItem("millo.heightmap-draft.v2.machine-1", JSON.stringify(legacy));

    const loaded = loadHeightmapDraft("machine-1", storage);
    expect(loaded).toBeDefined();
    expect(loaded).not.toHaveProperty("surfaceShape");
  });
});
