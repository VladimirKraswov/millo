import { lazy, Suspense } from "react";
import {
  createPluginManifest,
  definePlugin,
  uiSlots,
  type InMemoryPluginModule,
} from "../../plugin-sdk";

const QuickSketchPlugin = lazy(async () => ({
  default: (await import("./QuickSketchPlugin")).QuickSketchPlugin,
}));
export const QUICK_SKETCH_PLUGIN_ID = "io.millo.quick-sketch";
export function createQuickSketchPlugin(
  options: { readonly initialOpen?: boolean } = {},
): InMemoryPluginModule {
  return definePlugin({
    manifest: createPluginManifest({
      id: QUICK_SKETCH_PLUGIN_ID,
      name: "Чертёж и раскрой",
      version: "0.1.0",
      capabilities: {
        required: ["ui.contribute", "jobs.create", "tools.read"],
        optional: [],
      },
    }),
    activate(context) {
      if (!context.ui || !context.jobs || !context.tools)
        throw new Error("Sketch capabilities unavailable");
      const jobs = context.jobs,
        tools = context.tools;
      const contribution = context.ui.register({
        id: `${QUICK_SKETCH_PLUGIN_ID}.launcher`,
        slot: uiSlots.workspaceTools,
        order: 5,
        render: () => (
          <Suspense
            fallback={
              <button type="button" disabled>
                Чертёж и раскрой
              </button>
            }
          >
            <QuickSketchPlugin
              initialOpen={options.initialOpen}
              jobs={jobs}
              tools={tools}
            />
          </Suspense>
        ),
      });
      return () => contribution.dispose();
    },
  });
}
