import type { InMemoryPluginModule } from "../platform/plugins/InMemoryPluginLoader";
import type { PluginManifestV1 } from "../platform/plugins/PluginManifest";
import {
  PLUGIN_API_VERSION,
  PLUGIN_MANIFEST_VERSION,
  validatePluginManifest,
} from "../platform/plugins/PluginManifest";

export { uiSlots } from "../platform/extensions/UiExtensionRegistry";
export { DialogSurface } from "../components/DialogSurface";
export type { SketchJobRequest, SketchShape, SketchOperation, SketchStock, SketchProject, GeneratedSketchJob } from "../shared/sketch";
export {
  PLUGIN_API_VERSION,
  PLUGIN_MANIFEST_VERSION,
  pluginCapabilityCatalog,
} from "../platform/plugins/PluginManifest";
export type {
  PluginActivationContext,
  PluginJobsCapability,
  PluginLoadResult,
  PluginMachineJogCapability,
  PluginMachineCoordinatesCapability,
  PluginMachineReadCapability,
  PluginToolsCapability,
  PluginUiCapability,
  PluginUiContribution,
  InMemoryPluginModule,
} from "../platform/plugins/InMemoryPluginLoader";
export type {
  PluginCapability,
  PluginManifestV1,
} from "../platform/plugins/PluginManifest";
export type { UiSlotId } from "../platform/extensions/UiExtensionRegistry";

export type PluginDefinition = InMemoryPluginModule;

export function definePluginManifest(
  manifest: PluginManifestV1,
): PluginManifestV1 {
  return validatePluginManifest(manifest);
}

export function definePlugin(
  plugin: InMemoryPluginModule,
): InMemoryPluginModule {
  validatePluginManifest(plugin.manifest);
  return Object.freeze(plugin);
}

export function createPluginManifest(
  manifest: Omit<PluginManifestV1, "manifestVersion" | "apiVersion">,
): PluginManifestV1 {
  return definePluginManifest({
    ...manifest,
    manifestVersion: PLUGIN_MANIFEST_VERSION,
    apiVersion: PLUGIN_API_VERSION,
  });
}
