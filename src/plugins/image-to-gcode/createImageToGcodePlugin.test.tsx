import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { createUiExtensionRegistry, uiSlots } from "../../platform/extensions/UiExtensionRegistry";
import type { ImageJobGateway } from "../../platform/jobs/ImageJobGateway";
import { GeneratedJobStore } from "../../platform/jobs/GeneratedJobStore";
import { JobCreationService } from "../../platform/jobs/JobCreationService";
import { CapabilityGrantStore } from "../../platform/plugins/CapabilityGrantStore";
import { InMemoryPluginLoader } from "../../platform/plugins/InMemoryPluginLoader";
import { createImageToGcodePlugin, IMAGE_TO_GCODE_PLUGIN_ID } from "./createImageToGcodePlugin";

describe("Image to G-code bundled plugin", () => {
  it("registers its launcher only through granted UI and job capabilities", async () => {
    const registry = createUiExtensionRegistry();
    const gateway: ImageJobGateway = {
      generate: vi.fn(),
      generateSurfacing: vi.fn(),
      save: vi.fn(),
    };
    const loader = new InMemoryPluginLoader({
      uiRegistry: registry,
      jobs: new JobCreationService(gateway, new GeneratedJobStore()),
      grants: new CapabilityGrantStore([
        { pluginId: IMAGE_TO_GCODE_PLUGIN_ID, capabilities: ["ui.contribute", "jobs.create"] },
      ]),
    });

    await loader.load(createImageToGcodePlugin());
    const contribution = registry.list(uiSlots.workspaceTools)[0];
    const markup = renderToStaticMarkup(
      contribution.extension.kind === "global" ? contribution.extension.render() : null,
    );

    expect(markup).toContain("Гравировка по изображению");
    expect(loader.list()[0]?.grantedCapabilities).toEqual(["ui.contribute", "jobs.create"]);
  });
});
