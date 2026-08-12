import {
  pluginCapabilityCatalog,
  type PluginCapability,
} from "./PluginManifest";

export interface PluginCapabilityGrant {
  readonly pluginId: string;
  readonly capabilities: readonly PluginCapability[];
}

export class CapabilityGrantStore {
  private readonly grants = new Map<string, ReadonlySet<PluginCapability>>();

  constructor(initial: readonly PluginCapabilityGrant[] = []) {
    for (const grant of initial) {
      if (!/^[a-z0-9]+(?:[.-][a-z0-9]+)+$/.test(grant.pluginId)) {
        throw new Error("capability grant pluginId must be a valid plugin id");
      }
      const current = new Set(this.grants.get(grant.pluginId) ?? []);
      for (const capability of grant.capabilities) {
        if (!pluginCapabilityCatalog.includes(capability)) {
          throw new Error(`unknown plugin capability grant: ${String(capability)}`);
        }
        current.add(capability);
      }
      this.grants.set(grant.pluginId, current);
    }
  }

  has(pluginId: string, capability: PluginCapability): boolean {
    return this.grants.get(pluginId)?.has(capability) ?? false;
  }

  list(pluginId: string): readonly PluginCapability[] {
    const granted = this.grants.get(pluginId);
    return pluginCapabilityCatalog.filter((capability) => granted?.has(capability));
  }
}
