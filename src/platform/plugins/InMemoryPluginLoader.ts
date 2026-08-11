import type { ReactNode } from "react";

import type { ExtensionRegistration } from "../extensions/ExtensionRegistry";
import type {
  UiExtensionRegistry,
  UiSlotId,
} from "../extensions/UiExtensionRegistry";
import type { MachineCommandGateway } from "../machine/MachineCommandGateway";
import type {
  JogPadStepOutcome,
  JogPadStepRequest,
} from "../../shared/machine";
import { CapabilityGrantStore } from "./CapabilityGrantStore";
import {
  PLUGIN_API_VERSION,
  type PluginCapability,
  type PluginManifestV1,
  validatePluginManifest,
} from "./PluginManifest";

export interface PluginUiContribution {
  readonly id: string;
  readonly slot: UiSlotId;
  readonly order?: number;
  readonly replaces?: readonly string[];
  readonly render: () => ReactNode;
}

export interface PluginUiCapability {
  register(contribution: PluginUiContribution): ExtensionRegistration;
}

export interface PluginMachineJogCapability {
  step(request: JogPadStepRequest): Promise<JogPadStepOutcome>;
}

export interface PluginActivationContext {
  readonly manifest: PluginManifestV1;
  readonly grantedCapabilities: readonly PluginCapability[];
  readonly hasCapability: (capability: PluginCapability) => boolean;
  readonly ui?: PluginUiCapability;
  readonly machineJog?: PluginMachineJogCapability;
}

export interface InMemoryPluginModule {
  readonly manifest: unknown;
  activate(
    context: PluginActivationContext,
  ):
    | void
    | (() => void | Promise<void>)
    | Promise<void | (() => void | Promise<void>)>;
}

export interface PluginLoadResult {
  readonly manifest: PluginManifestV1;
  readonly grantedCapabilities: readonly PluginCapability[];
  readonly deniedOptionalCapabilities: readonly PluginCapability[];
}

interface ActivePlugin extends PluginLoadResult {
  readonly deactivate?: () => void | Promise<void>;
}

interface InMemoryPluginLoaderOptions {
  readonly uiRegistry: UiExtensionRegistry;
  readonly machineCommands?: MachineCommandGateway;
  readonly grants?: CapabilityGrantStore;
}

export class PluginLoadError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PluginLoadError";
  }
}

export class InMemoryPluginLoader {
  private readonly uiRegistry: UiExtensionRegistry;
  private readonly machineCommands?: MachineCommandGateway;
  private readonly grants: CapabilityGrantStore;
  private readonly active = new Map<string, ActivePlugin>();
  private readonly loading = new Set<string>();

  constructor(options: InMemoryPluginLoaderOptions) {
    this.uiRegistry = options.uiRegistry;
    this.machineCommands = options.machineCommands;
    this.grants = options.grants ?? new CapabilityGrantStore();
  }

  async load(plugin: InMemoryPluginModule): Promise<PluginLoadResult> {
    const manifest = validatePluginManifest(plugin.manifest);
    if (manifest.apiVersion !== PLUGIN_API_VERSION) {
      throw new PluginLoadError(
        `plugin ${manifest.id} requires API ${manifest.apiVersion}, host provides ${PLUGIN_API_VERSION}`,
      );
    }
    if (this.active.has(manifest.id) || this.loading.has(manifest.id)) {
      throw new PluginLoadError(`plugin is already loaded: ${manifest.id}`);
    }

    const grantedCapabilities = [
      ...manifest.capabilities.required,
      ...manifest.capabilities.optional,
    ].filter(
      (capability) =>
        this.grants.has(manifest.id, capability) && this.supports(capability),
    );
    const missingRequired = manifest.capabilities.required.filter(
      (capability) => !grantedCapabilities.includes(capability),
    );
    if (missingRequired.length > 0) {
      throw new PluginLoadError(
        `plugin ${manifest.id} is missing required capabilities: ${missingRequired.join(", ")}`,
      );
    }
    const deniedOptionalCapabilities = manifest.capabilities.optional.filter(
      (capability) => !grantedCapabilities.includes(capability),
    );
    const result: PluginLoadResult = Object.freeze({
      manifest,
      grantedCapabilities: Object.freeze([...grantedCapabilities]),
      deniedOptionalCapabilities: Object.freeze([
        ...deniedOptionalCapabilities,
      ]),
    });

    this.loading.add(manifest.id);
    try {
      const deactivate = await plugin.activate(
        this.activationContext(manifest, grantedCapabilities),
      );
      if (deactivate !== undefined && typeof deactivate !== "function") {
        throw new PluginLoadError(
          `plugin ${manifest.id} returned an invalid deactivate handler`,
        );
      }
      const deactivateHandler =
        typeof deactivate === "function" ? deactivate : undefined;
      this.active.set(manifest.id, {
        ...result,
        deactivate: deactivateHandler,
      });
      return result;
    } catch (error) {
      this.uiRegistry.unregisterOwner(manifest.id);
      throw error;
    } finally {
      this.loading.delete(manifest.id);
    }
  }

  async unload(pluginId: string): Promise<boolean> {
    const plugin = this.active.get(pluginId);
    if (!plugin) return false;
    this.active.delete(pluginId);
    try {
      await plugin.deactivate?.();
    } finally {
      this.uiRegistry.unregisterOwner(pluginId);
    }
    return true;
  }

  list(): readonly PluginLoadResult[] {
    return [...this.active.values()].map(
      ({ manifest, grantedCapabilities, deniedOptionalCapabilities }) => ({
        manifest,
        grantedCapabilities,
        deniedOptionalCapabilities,
      }),
    );
  }

  private supports(capability: PluginCapability): boolean {
    switch (capability) {
      case "ui.contribute":
        return true;
      case "machine.jog":
        return this.machineCommands !== undefined;
      case "machine.read":
      case "jobs.create":
        return false;
    }
  }

  private activationContext(
    manifest: PluginManifestV1,
    grantedCapabilities: readonly PluginCapability[],
  ): PluginActivationContext {
    const hasCapability = (capability: PluginCapability) =>
      grantedCapabilities.includes(capability);
    const ui = hasCapability("ui.contribute")
      ? this.uiCapability(manifest.id)
      : undefined;
    const machineJog =
      hasCapability("machine.jog") && this.machineCommands
        ? Object.freeze({
            step: (request: JogPadStepRequest) =>
              this.machineCommands!.jogPadStep(request),
          })
        : undefined;

    return Object.freeze({
      manifest,
      grantedCapabilities: Object.freeze([...grantedCapabilities]),
      hasCapability,
      ui,
      machineJog,
    });
  }

  private uiCapability(pluginId: string): PluginUiCapability {
    return Object.freeze({
      register: (contribution: PluginUiContribution) => {
        if (!contribution.id.startsWith(`${pluginId}.`)) {
          throw new PluginLoadError(
            `plugin contribution must be namespaced with ${pluginId}.`,
          );
        }
        return this.uiRegistry.register({
          id: contribution.id,
          owner: pluginId,
          slot: contribution.slot,
          order: contribution.order,
          replaces: contribution.replaces,
          extension: () => contribution.render(),
        });
      },
    });
  }
}
