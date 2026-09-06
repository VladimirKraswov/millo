import { describe, expect, it, vi } from "vitest";
import {
  createUiExtensionRegistry,
  uiSlots,
} from "../../platform/extensions/UiExtensionRegistry";
import { InMemoryPluginLoader } from "../../platform/plugins/InMemoryPluginLoader";
import { CapabilityGrantStore } from "../../platform/plugins/CapabilityGrantStore";
import { ToolLibraryService } from "../../platform/tooling/ToolLibraryService";
import { previewToolLibraryGateway } from "../../features/tool-library/previewToolLibraryGateway";
import {
  createQuickSketchPlugin,
  QUICK_SKETCH_PLUGIN_ID,
} from "./createQuickSketchPlugin";

describe("bundled sketch plugin", () => {
  it("requires explicit capabilities and removes its menu contribution on unload", async () => {
    const uiRegistry = createUiExtensionRegistry();
    const tools = new ToolLibraryService(previewToolLibraryGateway);
    const jobs = {
      generateImage: vi.fn(),
      generateSurfacing: vi.fn(),
      inspectPcb: vi.fn(),
      generatePcb: vi.fn(),
      generateSketch: vi.fn(),
      saveSketchProject: vi.fn(),
      open: vi.fn(),
      save: vi.fn(),
    };
    const grants = new CapabilityGrantStore([
      {
        pluginId: QUICK_SKETCH_PLUGIN_ID,
        capabilities: ["ui.contribute", "jobs.create", "tools.read"],
      },
    ]);
    const loader = new InMemoryPluginLoader({
      uiRegistry,
      tools,
      jobs,
      grants,
    });
    await loader.load(createQuickSketchPlugin());
    expect(
      uiRegistry
        .list(uiSlots.workspaceTools)
        .some((c) => c.owner === QUICK_SKETCH_PLUGIN_ID),
    ).toBe(true);
    await loader.unload(QUICK_SKETCH_PLUGIN_ID);
    expect(uiRegistry.list(uiSlots.workspaceTools)).toHaveLength(0);
    const denied = new InMemoryPluginLoader({
      uiRegistry,
      tools,
      jobs,
      grants: new CapabilityGrantStore([]),
    });
    await expect(denied.load(createQuickSketchPlugin())).rejects.toThrow(
      "missing required capabilities",
    );
  });
});
