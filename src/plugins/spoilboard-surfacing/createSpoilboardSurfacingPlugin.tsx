import {
  createPluginManifest,
  definePlugin,
  type InMemoryPluginModule,
  uiSlots,
} from "../../plugin-sdk";
import { SpoilboardSurfacingPlugin } from "./SpoilboardSurfacingPlugin";

export const SPOILBOARD_SURFACING_PLUGIN_ID = "io.millo.spoilboard-surfacing";

interface SpoilboardSurfacingPluginOptions {
  readonly initialOpen?: boolean;
}

export function createSpoilboardSurfacingPlugin(
  options: SpoilboardSurfacingPluginOptions = {},
): InMemoryPluginModule {
  return definePlugin({
    manifest: createPluginManifest({
      id: SPOILBOARD_SURFACING_PLUGIN_ID,
      name: "Spoilboard Surfacing",
      version: "0.1.0",
      capabilities: {
        required: ["ui.contribute", "jobs.create", "tools.read"],
        optional: [],
      },
    }),
    activate(context) {
      if (!context.ui || !context.jobs || !context.tools) {
        throw new Error("surfacing plugin capabilities are unavailable");
      }
      const jobs = context.jobs;
      const tools = context.tools;
      const registration = context.ui.register({
        id: `${SPOILBOARD_SURFACING_PLUGIN_ID}.launcher`,
        slot: uiSlots.workspaceTools,
        order: 30,
        render: () => (
          <SpoilboardSurfacingPlugin
            initialOpen={options.initialOpen}
            jobs={jobs}
            tools={tools}
          />
        ),
      });
      return () => registration.dispose();
    },
  });
}
