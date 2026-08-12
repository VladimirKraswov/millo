import { describe, expect, it } from "vitest";

import {
  createPluginManifest,
  definePlugin,
  uiSlots,
} from "./index";

describe("trusted plugin SDK", () => {
  it("creates a validated host-version manifest", () => {
    const manifest = createPluginManifest({
      id: "dev.millo.sdk-fixture",
      name: "SDK fixture",
      version: "1.0.0",
      capabilities: { required: ["ui.contribute"], optional: [] },
    });

    expect(manifest).toMatchObject({ manifestVersion: 1, apiVersion: 1 });
    expect(Object.isFrozen(manifest)).toBe(true);
  });

  it("validates a plugin at its definition boundary", () => {
    const plugin = definePlugin({
      manifest: createPluginManifest({
        id: "dev.millo.defined-fixture",
        name: "Defined fixture",
        version: "1.0.0",
        capabilities: { required: ["ui.contribute"], optional: [] },
      }),
      activate(context) {
        context.ui?.register({
          id: "dev.millo.defined-fixture.launcher",
          slot: uiSlots.workspaceTools,
          render: () => null,
        });
      },
    });

    expect(Object.isFrozen(plugin)).toBe(true);
  });
});
