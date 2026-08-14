import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { createUiExtensionRegistry, uiSlots } from "../../platform/extensions/UiExtensionRegistry";
import { GeneratedJobStore } from "../../platform/jobs/GeneratedJobStore";
import type { ImageJobGateway } from "../../platform/jobs/ImageJobGateway";
import { JobCreationService } from "../../platform/jobs/JobCreationService";
import { CapabilityGrantStore } from "../../platform/plugins/CapabilityGrantStore";
import { InMemoryPluginLoader } from "../../platform/plugins/InMemoryPluginLoader";
import { ToolLibraryService } from "../../platform/tooling/ToolLibraryService";
import { previewToolLibraryGateway } from "../../features/tool-library/previewToolLibraryGateway";
import { createPcbFabricationPlugin, PCB_FABRICATION_PLUGIN_ID } from "./createPcbFabricationPlugin";

describe("PCB fabrication bundled plugin", () => {
  it("registers a default workspace tool with only core job and tool capabilities", async () => {
    const registry = createUiExtensionRegistry();
    const tooling = new ToolLibraryService(previewToolLibraryGateway);
    await tooling.initialize();
    const gateway: ImageJobGateway = {
      generate: vi.fn(),
      generateSurfacing: vi.fn(),
      inspectPcb: vi.fn(),
      generatePcb: vi.fn(),
      save: vi.fn(),
    };
    const loader = new InMemoryPluginLoader({
      uiRegistry: registry,
      jobs: new JobCreationService(gateway, new GeneratedJobStore()),
      tools: tooling,
      grants: new CapabilityGrantStore([{
        pluginId: PCB_FABRICATION_PLUGIN_ID,
        capabilities: ["ui.contribute", "jobs.create", "tools.read"],
      }]),
    });

    await loader.load(createPcbFabricationPlugin());
    const contribution = registry.list(uiSlots.workspaceTools)
      .find((entry) => entry.owner === PCB_FABRICATION_PLUGIN_ID);
    const markup = renderToStaticMarkup(
      contribution?.extension.kind === "global" ? contribution.extension.render() : null,
    );

    expect(markup).toContain("Плата из Gerber");
    expect(loader.list()[0]?.grantedCapabilities).toEqual([
      "ui.contribute",
      "jobs.create",
      "tools.read",
    ]);
  });
});
