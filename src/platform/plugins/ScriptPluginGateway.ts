import type {
  InstalledScriptPlugin,
  ScriptCapability,
  ScriptPluginExecutionOutcome,
  ScriptPluginExecutionRequest,
  ScriptPluginPackage,
} from "../../shared/scriptPlugins";

export interface ScriptPluginGateway {
  list(): Promise<readonly InstalledScriptPlugin[]>;
  importPackage(): Promise<InstalledScriptPlugin | undefined>;
  savePackage(pluginPackage: ScriptPluginPackage): Promise<InstalledScriptPlugin>;
  exportPackage(pluginId: string, digest: string): Promise<string | undefined>;
  configure(
    pluginId: string,
    digest: string,
    enabled: boolean,
    grantedCapabilities: readonly ScriptCapability[],
  ): Promise<InstalledScriptPlugin>;
  delete(pluginId: string): Promise<boolean>;
  execute(
    request: ScriptPluginExecutionRequest,
  ): Promise<ScriptPluginExecutionOutcome>;
}
