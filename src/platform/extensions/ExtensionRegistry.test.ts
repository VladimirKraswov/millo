import { describe, expect, it, vi } from "vitest";

import { ExtensionRegistry } from "./ExtensionRegistry";

type TestSlot = "left" | "right";

describe("ExtensionRegistry", () => {
  it("orders active contributions deterministically", () => {
    const registry = new ExtensionRegistry<TestSlot, string>();
    registry.register({
      id: "plugin.second",
      owner: "plugin",
      slot: "right",
      order: 20,
      extension: "second",
    });
    registry.register({
      id: "plugin.first",
      owner: "plugin",
      slot: "right",
      order: 10,
      extension: "first",
    });
    registry.register({
      id: "plugin.other-slot",
      owner: "plugin",
      slot: "left",
      extension: "left",
    });

    expect(registry.list("right").map(({ id }) => id)).toEqual([
      "plugin.first",
      "plugin.second",
    ]);
  });

  it("reveals a replaced contribution again when replacement unloads", () => {
    const registry = new ExtensionRegistry<TestSlot, string>();
    registry.register({
      id: "core.panel",
      owner: "core",
      slot: "right",
      extension: "core",
    });
    const replacement = registry.register({
      id: "plugin.panel",
      owner: "plugin",
      slot: "right",
      replaces: ["core.panel"],
      extension: "plugin",
    });

    expect(registry.list("right").map(({ id }) => id)).toEqual([
      "plugin.panel",
    ]);
    replacement.dispose();
    expect(registry.list("right").map(({ id }) => id)).toEqual(["core.panel"]);
  });

  it("unloads every contribution owned by one plugin in one revision", () => {
    const registry = new ExtensionRegistry<TestSlot, string>();
    const listener = vi.fn();
    registry.subscribe(listener);
    registry.register({
      id: "plugin.left",
      owner: "plugin",
      slot: "left",
      extension: "left",
    });
    registry.register({
      id: "plugin.right",
      owner: "plugin",
      slot: "right",
      extension: "right",
    });
    listener.mockClear();

    expect(registry.unregisterOwner("plugin")).toBe(2);
    expect(listener).toHaveBeenCalledOnce();
    expect(registry.list("left")).toEqual([]);
    expect(registry.list("right")).toEqual([]);
  });

  it("rejects duplicate IDs and self replacement", () => {
    const registry = new ExtensionRegistry<TestSlot, string>();
    registry.register({
      id: "core.panel",
      owner: "core",
      slot: "right",
      extension: "core",
    });

    expect(() =>
      registry.register({
        id: "core.panel",
        owner: "plugin",
        slot: "right",
        extension: "duplicate",
      }),
    ).toThrow("already registered");
    expect(() =>
      registry.register({
        id: "plugin.loop",
        owner: "plugin",
        slot: "right",
        replaces: ["plugin.loop"],
        extension: "loop",
      }),
    ).toThrow("cannot replace itself");
  });
});
