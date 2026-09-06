import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { createUiExtensionRegistry, uiSlots } from "../../platform/extensions/UiExtensionRegistry";
import { GeneratedJobStore } from "../../platform/jobs/GeneratedJobStore";
import { JobCreationService } from "../../platform/jobs/JobCreationService";
import type { ImageJobGateway } from "../../platform/jobs/ImageJobGateway";
import { CapabilityGrantStore } from "../../platform/plugins/CapabilityGrantStore";
import { InMemoryPluginLoader } from "../../platform/plugins/InMemoryPluginLoader";
import { ToolLibraryService } from "../../platform/tooling/ToolLibraryService";
import { previewToolLibraryGateway } from "../../features/tool-library/previewToolLibraryGateway";
import { createSpoilboardSurfacingPlugin, SPOILBOARD_SURFACING_PLUGIN_ID } from "./createSpoilboardSurfacingPlugin";

describe("Spoilboard surfacing bundled plugin", () => {
  it("registers through UI, jobs and read-only tool capabilities", async () => {
    const registry = createUiExtensionRegistry();
    const tooling = new ToolLibraryService(previewToolLibraryGateway);
    await tooling.initialize();
    const gateway: ImageJobGateway = {
      generate: vi.fn(),
      generateSurfacing: vi.fn(),
      inspectPcb: vi.fn(),
      generatePcb: vi.fn(),
      generateSketch: vi.fn(),
      saveSketchProject: vi.fn(),
      save: vi.fn(),
    };
    const loader = new InMemoryPluginLoader({
      uiRegistry: registry,
      jobs: new JobCreationService(gateway, new GeneratedJobStore()),
      tools: tooling,
      grants: new CapabilityGrantStore([{
        pluginId: SPOILBOARD_SURFACING_PLUGIN_ID,
        capabilities: ["ui.contribute", "jobs.create", "tools.read"],
      }]),
    });

    await loader.load(createSpoilboardSurfacingPlugin());
    const contribution = registry.list(uiSlots.workspaceTools)
      .find((entry) => entry.owner === SPOILBOARD_SURFACING_PLUGIN_ID);
    const markup = renderToStaticMarkup(
      contribution?.extension.kind === "global" ? contribution.extension.render() : null,
    );

    expect(markup).toContain("Выравнивание");
    expect(loader.list()[0]?.grantedCapabilities).toEqual([
      "ui.contribute",
      "jobs.create",
      "tools.read",
    ]);
  });
});
