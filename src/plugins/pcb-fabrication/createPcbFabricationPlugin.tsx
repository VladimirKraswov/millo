import { lazy, Suspense } from "react";

import {
  createPluginManifest,
  definePlugin,
  type InMemoryPluginModule,
  uiSlots,
} from "../../plugin-sdk";

const PcbFabricationPlugin = lazy(async () => {
  const module = await import("./PcbFabricationPlugin");
  return { default: module.PcbFabricationPlugin };
});

export const PCB_FABRICATION_PLUGIN_ID = "io.millo.pcb-fabrication";

export function createPcbFabricationPlugin(
  options: { readonly initialOpen?: boolean } = {},
): InMemoryPluginModule {
  return definePlugin({
    manifest: createPluginManifest({
      id: PCB_FABRICATION_PLUGIN_ID,
      name: "PCB from Gerber",
      version: "0.1.0",
      capabilities: {
        required: ["ui.contribute", "jobs.create", "tools.read"],
        optional: [],
      },
    }),
    activate(context) {
      if (!context.ui || !context.jobs || !context.tools) {
        throw new Error("PCB plugin capabilities are unavailable");
      }
      const jobs = context.jobs;
      const tools = context.tools;
      const registration = context.ui.register({
        id: `${PCB_FABRICATION_PLUGIN_ID}.launcher`,
        slot: uiSlots.workspaceTools,
        order: 20,
        render: () => (
          <Suspense fallback={<button className="pcb-launcher" disabled type="button">Плата из Gerber</button>}>
            <PcbFabricationPlugin
              initialOpen={options.initialOpen}
              jobs={jobs}
              tools={tools}
            />
          </Suspense>
        ),
      });
      return () => registration.dispose();
    },
  });
}
