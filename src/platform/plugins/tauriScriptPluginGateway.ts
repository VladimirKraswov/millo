import { invoke } from "@tauri-apps/api/core";

import type { ScriptPluginGateway } from "./ScriptPluginGateway";
import type {
  InstalledScriptPlugin,
  ScriptPluginExecutionOutcome,
  ScriptPluginExecutionRequest,
  ScriptPluginPackage,
} from "../../shared/scriptPlugins";

export const tauriScriptPluginGateway: ScriptPluginGateway = {
  list: () => invoke<InstalledScriptPlugin[]>("script_plugins"),
  importPackage: () =>
    invoke<InstalledScriptPlugin | null>("import_script_plugin").then(
      (plugin) => plugin ?? undefined,
    ),
  savePackage: (pluginPackage: ScriptPluginPackage) =>
    invoke<InstalledScriptPlugin>("save_script_plugin", {
      request: { packageJson: JSON.stringify(pluginPackage) },
    }),
  exportPackage: (pluginId, digest) =>
    invoke<string | null>("export_script_plugin", {
      request: { pluginId, digest },
    }).then((path) => path ?? undefined),
  configure: (pluginId, digest, enabled, grantedCapabilities) =>
    invoke<InstalledScriptPlugin>("configure_script_plugin", {
      request: { pluginId, digest, enabled, grantedCapabilities },
    }),
  delete: (pluginId) =>
    invoke<boolean>("delete_script_plugin", { request: { pluginId } }),
  execute: (request: ScriptPluginExecutionRequest) =>
    invoke<ScriptPluginExecutionOutcome>("execute_script_plugin", { request }),
};
