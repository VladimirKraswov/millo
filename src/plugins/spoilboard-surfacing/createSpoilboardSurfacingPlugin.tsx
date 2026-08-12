import { uiSlots } from "../../platform/extensions/UiExtensionRegistry";
import type { InMemoryPluginModule } from "../../platform/plugins/InMemoryPluginLoader";
import {
  PLUGIN_API_VERSION,
  PLUGIN_MANIFEST_VERSION,
} from "../../platform/plugins/PluginManifest";
import { SpoilboardSurfacingPlugin } from "./SpoilboardSurfacingPlugin";

export const SPOILBOARD_SURFACING_PLUGIN_ID = "io.millo.spoilboard-surfacing";

interface SpoilboardSurfacingPluginOptions {
  readonly initialOpen?: boolean;
}

export function createSpoilboardSurfacingPlugin(
  options: SpoilboardSurfacingPluginOptions = {},
): InMemoryPluginModule {
  return {
    manifest: {
      manifestVersion: PLUGIN_MANIFEST_VERSION,
      apiVersion: PLUGIN_API_VERSION,
      id: SPOILBOARD_SURFACING_PLUGIN_ID,
      name: "Spoilboard Surfacing",
      version: "0.1.0",
      capabilities: {
        required: ["ui.contribute", "jobs.create", "tools.read"],
        optional: [],
      },
    },
    activate(context) {
      if (!context.ui || !context.jobs || !context.tools) {
        throw new Error("surfacing plugin capabilities are unavailable");
      }
      const registration = context.ui.register({
        id: `${SPOILBOARD_SURFACING_PLUGIN_ID}.launcher`,
        slot: uiSlots.workspaceTools,
        order: 30,
        render: () => (
          <SpoilboardSurfacingPlugin
            initialOpen={options.initialOpen}
            jobs={context.jobs!}
            tools={context.tools!}
          />
        ),
      });
      return () => registration.dispose();
    },
  };
}
