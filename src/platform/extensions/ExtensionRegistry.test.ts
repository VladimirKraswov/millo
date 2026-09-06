import { describe, expect, it, vi } from "vitest";

import { ExtensionRegistry } from "./ExtensionRegistry";

type TestSlot = "left" | "right";

describe("ExtensionRegistry", () => {
  it("does not let a stale registration remove its replacement after reload", () => {
    const registry = new ExtensionRegistry<TestSlot, string>();
    const contribution = { id: "plugin.panel", owner: "plugin", slot: "left" as const, extension: "old" };
    const old = registry.register(contribution);
    registry.unregisterOwner("plugin");
    registry.register({ ...contribution, extension: "new" });
    const revision = registry.getSnapshot();
    old.dispose();
    expect(registry.list("left").map(({ extension }) => extension)).toEqual(["new"]);
    expect(registry.getSnapshot()).toBe(revision);
  });

  it("isolates observer and diagnostic failures without losing registrations", () => {
    const report = vi.fn(() => { throw new Error("diagnostic failed"); });
    const registry = new ExtensionRegistry<TestSlot, string>(report);
    registry.subscribe(() => { throw new Error("observer failed"); });
    const listener = vi.fn();
    registry.subscribe(listener);
    const registration = registry.register({ id: "plugin.panel", owner: "plugin", slot: "left", extension: "panel" });
    expect(registry.list("left")).toHaveLength(1);
    registration.dispose();
    expect(registry.list("left")).toHaveLength(0);
    expect(report).toHaveBeenCalledTimes(2);
    expect(listener).toHaveBeenCalledTimes(2);
  });

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

  it("rejects ambiguous replacement lists", () => {
    const registry = new ExtensionRegistry<"slot", string>();

    expect(() =>
      registry.register({
        id: "plugin.panel",
        owner: "plugin",
        slot: "slot",
        replaces: ["core.panel", "core.panel"],
        extension: "panel",
      }),
    ).toThrow("replacements contain duplicates");
    expect(() =>
      registry.register({
        id: "plugin.panel",
        owner: "plugin",
        slot: "slot",
        replaces: [""],
        extension: "panel",
      }),
    ).toThrow("replacement ids must be non-empty");
  });

  it("rejects indirect replacement cycles without hiding the working panel", () => {
    const registry = new ExtensionRegistry<TestSlot, string>();
    registry.register({ id: "a", owner: "plugin", slot: "left", replaces: ["b"], extension: "A" });
    registry.register({ id: "b", owner: "plugin", slot: "left", replaces: ["c"], extension: "B" });
    const revision = registry.getSnapshot();
    expect(() => registry.register({ id: "c", owner: "plugin", slot: "left", replaces: ["a"], extension: "C" })).toThrow("cycle");
    expect(registry.getSnapshot()).toBe(revision);
    expect(registry.list("left").map(({ id }) => id)).toEqual(["a"]);
  });
});
