import {
  createPluginManifest,
  definePlugin,
  type InMemoryPluginModule,
  uiSlots,
} from "../../plugin-sdk";
import { ImageToGcodePlugin } from "./ImageToGcodePlugin";

export const IMAGE_TO_GCODE_PLUGIN_ID = "io.millo.image-to-gcode";

interface ImageToGcodePluginOptions {
  readonly initialOpen?: boolean;
}

export function createImageToGcodePlugin(
  options: ImageToGcodePluginOptions = {},
): InMemoryPluginModule {
  return definePlugin({
    manifest: createPluginManifest({
      id: IMAGE_TO_GCODE_PLUGIN_ID,
      name: "Image to G-code",
      version: "0.1.0",
      capabilities: {
        required: ["ui.contribute", "jobs.create"],
        optional: [],
      },
    }),
    activate(context) {
      if (!context.ui || !context.jobs) {
        throw new Error("Image to G-code requires UI and jobs.create capabilities");
      }
      const jobs = context.jobs;
      context.ui.register({
        id: `${IMAGE_TO_GCODE_PLUGIN_ID}.launcher`,
        slot: uiSlots.workspaceTools,
        order: 100,
        render: () => <ImageToGcodePlugin initialOpen={options.initialOpen} jobs={jobs} />,
      });
    },
  });
}
