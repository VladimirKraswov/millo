import type { ReactNode } from "react";

import type { ExtensionRegistration } from "../extensions/ExtensionRegistry";
import type {
  UiExtensionRegistry,
  UiSlotId,
} from "../extensions/UiExtensionRegistry";
import type { MachineCommandGateway } from "../machine/MachineCommandGateway";
import type { JobCreationCapability } from "../jobs/JobCreationService";
import type {
  GeneratedJob,
  GeneratedGcodeSaveOutcome,
  GeneratedImageJob,
  GeneratedSurfacingJob,
  ImageJobRequest,
  SurfacingJobRequest,
} from "../../shared/jobs";
import type { ToolLibraryState } from "../../shared/tooling";
import type { ToolLibraryService } from "../tooling/ToolLibraryService";
import type {
  MachineStateListener,
  MachineStateSource,
  ReadonlyControllerSnapshot,
} from "../machine/MachineStateSource";
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

export interface PluginMachineReadCapability {
  current(): ReadonlyControllerSnapshot;
  subscribe(listener: MachineStateListener): () => void;
}

export interface PluginJobsCapability {
  generateImage(request: ImageJobRequest): Promise<GeneratedImageJob>;
  generateSurfacing(request: SurfacingJobRequest): Promise<GeneratedSurfacingJob>;
  open(job: GeneratedJob): void;
  save(job: GeneratedJob): Promise<GeneratedGcodeSaveOutcome | undefined>;
}

export interface PluginToolsCapability {
  current(): ToolLibraryState;
  subscribe(listener: (state: ToolLibraryState) => void): () => void;
}

export interface PluginActivationContext {
  readonly manifest: PluginManifestV1;
  readonly grantedCapabilities: readonly PluginCapability[];
  readonly hasCapability: (capability: PluginCapability) => boolean;
  readonly ui?: PluginUiCapability;
  readonly machineRead?: PluginMachineReadCapability;
  readonly machineJog?: PluginMachineJogCapability;
  readonly jobs?: PluginJobsCapability;
  readonly tools?: PluginToolsCapability;
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
  readonly resources: PluginResourceScope;
}

interface LoadingPlugin {
  readonly resources: PluginResourceScope;
  cancelled: boolean;
}

interface InMemoryPluginLoaderOptions {
  readonly uiRegistry: UiExtensionRegistry;
  readonly machineCommands?: MachineCommandGateway;
  readonly machineState?: MachineStateSource;
  readonly jobs?: JobCreationCapability;
  readonly tools?: ToolLibraryService;
  readonly grants?: CapabilityGrantStore;
  readonly onPluginError?: (pluginId: string, error: unknown) => void;
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
  private readonly machineState?: MachineStateSource;
  private readonly jobs?: JobCreationCapability;
  private readonly tools?: ToolLibraryService;
  private readonly grants: CapabilityGrantStore;
  private readonly onPluginError?: (pluginId: string, error: unknown) => void;
  private readonly active = new Map<string, ActivePlugin>();
  private readonly loading = new Map<string, LoadingPlugin>();

  constructor(options: InMemoryPluginLoaderOptions) {
    this.uiRegistry = options.uiRegistry;
    this.machineCommands = options.machineCommands;
    this.machineState = options.machineState;
    this.jobs = options.jobs;
    this.tools = options.tools;
    this.grants = options.grants ?? new CapabilityGrantStore();
    this.onPluginError = options.onPluginError;
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

    const resources = new PluginResourceScope(manifest.id);
    const loading = { resources, cancelled: false };
    this.loading.set(manifest.id, loading);
    try {
      const deactivate = await plugin.activate(
        this.activationContext(manifest, grantedCapabilities, resources),
      );
      if (deactivate !== undefined && typeof deactivate !== "function") {
        throw new PluginLoadError(
          `plugin ${manifest.id} returned an invalid deactivate handler`,
        );
      }
      const deactivateHandler =
        typeof deactivate === "function" ? deactivate : undefined;
      if (loading.cancelled) {
        try {
          await deactivateHandler?.();
        } finally {
          resources.dispose();
          this.uiRegistry.unregisterOwner(manifest.id);
        }
        throw new PluginLoadError(
          `plugin was unloaded during activation: ${manifest.id}`,
        );
      }
      this.active.set(manifest.id, {
        ...result,
        deactivate: deactivateHandler,
        resources,
      });
      return result;
    } catch (error) {
      try {
        resources.dispose();
      } catch (cleanupError) {
        this.reportPluginError(manifest.id, cleanupError);
      } finally {
        this.uiRegistry.unregisterOwner(manifest.id);
      }
      throw error;
    } finally {
      if (this.loading.get(manifest.id) === loading) {
        this.loading.delete(manifest.id);
      }
    }
  }

  async unload(pluginId: string): Promise<boolean> {
    const loading = this.loading.get(pluginId);
    if (loading) {
      loading.cancelled = true;
      try {
        loading.resources.dispose();
      } finally {
        this.uiRegistry.unregisterOwner(pluginId);
      }
      return true;
    }
    const plugin = this.active.get(pluginId);
    if (!plugin) return false;
    this.active.delete(pluginId);
    let cleanupError: unknown;
    try {
      plugin.resources.dispose();
    } catch (error) {
      cleanupError = error;
    }
    let deactivationError: unknown;
    try {
      await plugin.deactivate?.();
    } catch (error) {
      deactivationError = error;
    } finally {
      this.uiRegistry.unregisterOwner(pluginId);
    }
    if (deactivationError !== undefined) throw deactivationError;
    if (cleanupError !== undefined) throw cleanupError;
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
        return this.machineState !== undefined;
      case "jobs.create":
        return this.jobs !== undefined;
      case "tools.read":
        return this.tools !== undefined;
    }
  }

  private activationContext(
    manifest: PluginManifestV1,
    grantedCapabilities: readonly PluginCapability[],
    resources: PluginResourceScope,
  ): PluginActivationContext {
    const hasCapability = (capability: PluginCapability) =>
      grantedCapabilities.includes(capability);
    const ui = hasCapability("ui.contribute")
      ? this.uiCapability(manifest.id, resources)
      : undefined;
    const machineRead =
      hasCapability("machine.read") && this.machineState
        ? this.machineReadCapability(manifest.id, resources)
        : undefined;
    const machineJog =
      hasCapability("machine.jog") && this.machineCommands
        ? Object.freeze({
            step: (request: JogPadStepRequest) => {
              resources.assertOpen();
              return this.machineCommands!.jogPadStep(request);
            },
          })
        : undefined;
    const jobs =
      hasCapability("jobs.create") && this.jobs
        ? Object.freeze({
            generateImage: async (request: ImageJobRequest) => {
              resources.assertOpen();
              const job = await this.jobs!.generateImage(request);
              resources.assertOpen();
              return job;
            },
            generateSurfacing: async (request: SurfacingJobRequest) => {
              resources.assertOpen();
              const job = await this.jobs!.generateSurfacing(request);
              resources.assertOpen();
              return job;
            },
            open: (job: GeneratedImageJob) => {
              resources.assertOpen();
              this.jobs!.open(job);
            },
            save: async (job: GeneratedImageJob) => {
              resources.assertOpen();
              const outcome = await this.jobs!.save(job);
              resources.assertOpen();
              return outcome;
            },
          })
        : undefined;
    const tools =
      hasCapability("tools.read") && this.tools
        ? Object.freeze({
            current: () => {
              resources.assertOpen();
              return this.tools!.current();
            },
            subscribe: (listener: (state: ToolLibraryState) => void) => {
              resources.assertOpen();
              if (typeof listener !== "function") {
                throw new PluginLoadError("tools.read listener must be a function");
              }
              return resources.track(this.tools!.subscribe(listener));
            },
          })
        : undefined;

    return Object.freeze({
      manifest,
      grantedCapabilities: Object.freeze([...grantedCapabilities]),
      hasCapability,
      ui,
      machineRead,
      machineJog,
      jobs,
      tools,
    });
  }

  private machineReadCapability(
    pluginId: string,
    resources: PluginResourceScope,
  ): PluginMachineReadCapability {
    return Object.freeze({
      current: () => {
        resources.assertOpen();
        return this.machineState!.current();
      },
      subscribe: (listener: MachineStateListener) => {
        resources.assertOpen();
        if (typeof listener !== "function") {
          throw new PluginLoadError("machine.read listener must be a function");
        }
        const unsubscribe = this.machineState!.subscribe((snapshot) => {
          try {
            listener(snapshot);
          } catch (error) {
            this.reportPluginError(pluginId, error);
          }
        });
        return resources.track(unsubscribe);
      },
    });
  }

  private uiCapability(
    pluginId: string,
    resources: PluginResourceScope,
  ): PluginUiCapability {
    return Object.freeze({
      register: (contribution: PluginUiContribution) => {
        resources.assertOpen();
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
          extension: { kind: "global", render: contribution.render },
        });
      },
    });
  }

  private reportPluginError(pluginId: string, error: unknown): void {
    try {
      this.onPluginError?.(pluginId, error);
    } catch {
      // Diagnostics must never interrupt host cleanup or state publication.
    }
  }
}

class PluginResourceScope {
  private open = true;
  private readonly disposers = new Set<() => void>();

  constructor(private readonly pluginId: string) {}

  assertOpen(): void {
    if (!this.open) {
      throw new PluginLoadError(`plugin is no longer active: ${this.pluginId}`);
    }
  }

  track(disposer: () => void): () => void {
    this.assertOpen();
    let active = true;
    const trackedDisposer = () => {
      if (!active) return;
      active = false;
      this.disposers.delete(trackedDisposer);
      disposer();
    };
    this.disposers.add(trackedDisposer);
    return trackedDisposer;
  }

  dispose(): void {
    if (!this.open) return;
    this.open = false;
    const errors: unknown[] = [];
    for (const disposer of [...this.disposers].reverse()) {
      try {
        disposer();
      } catch (error) {
        errors.push(error);
      }
    }
    this.disposers.clear();
    if (errors.length > 0) {
      throw new PluginLoadError(
        `plugin ${this.pluginId} resource cleanup failed: ${String(errors[0])}`,
      );
    }
  }
}
