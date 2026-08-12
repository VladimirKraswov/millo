import type { ControllerSnapshot } from "../shared/machine";
import {
  createUiExtensionRegistry,
  type UiExtensionRegistry,
} from "../platform/extensions/UiExtensionRegistry";
import type { MachineCommandGateway } from "../platform/machine/MachineCommandGateway";
import { MachineSnapshotStore } from "../platform/machine/MachineStateSource";
import { CapabilityGrantStore } from "../platform/plugins/CapabilityGrantStore";
import { InMemoryPluginLoader } from "../platform/plugins/InMemoryPluginLoader";
import type { InMemoryPluginModule } from "../platform/plugins/InMemoryPluginLoader";
import { GeneratedJobStore } from "../platform/jobs/GeneratedJobStore";
import type { ImageJobGateway } from "../platform/jobs/ImageJobGateway";
import { JobCreationService } from "../platform/jobs/JobCreationService";
import type { ToolLibraryGateway } from "../platform/tooling/ToolLibraryGateway";
import { ToolLibraryService } from "../platform/tooling/ToolLibraryService";
import { registerCoreUiExtensions } from "./registerCoreUiExtensions";

export interface PluginHost {
  readonly uiRegistry: UiExtensionRegistry;
  readonly machineState: MachineSnapshotStore;
  readonly plugins: InMemoryPluginLoader;
  readonly generatedJobs: GeneratedJobStore;
  readonly tools?: ToolLibraryService;
  readonly ready: Promise<void>;
}

export interface PluginHostOptions {
  readonly initialSnapshot: ControllerSnapshot;
  readonly machineCommands: MachineCommandGateway;
  readonly grants?: CapabilityGrantStore;
  readonly imageJobs?: ImageJobGateway;
  readonly toolLibrary?: ToolLibraryGateway;
  readonly bundledPlugins?: readonly InMemoryPluginModule[];
  readonly onPluginError?: (pluginId: string, error: unknown) => void;
}

export function bootstrapPluginHost(options: PluginHostOptions): PluginHost {
  const uiRegistry = createUiExtensionRegistry();
  registerCoreUiExtensions(uiRegistry);
  const machineState = new MachineSnapshotStore(options.initialSnapshot);
  const generatedJobs = new GeneratedJobStore();
  const tools = options.toolLibrary
    ? new ToolLibraryService(options.toolLibrary)
    : undefined;
  const jobs = options.imageJobs
    ? new JobCreationService(options.imageJobs, generatedJobs)
    : undefined;
  const plugins = new InMemoryPluginLoader({
    uiRegistry,
    machineState,
    machineCommands: options.machineCommands,
    jobs,
    tools,
    grants: options.grants,
    onPluginError: options.onPluginError,
  });
  const ready = (async () => {
    await tools?.initialize();
    for (const plugin of options.bundledPlugins ?? []) {
      await plugins.load(plugin);
    }
  })();
  void ready.catch(() => {
    // The caller still owns reporting; attach immediately to avoid an unhandled rejection.
  });

  return Object.freeze({
    uiRegistry,
    machineState,
    plugins,
    generatedJobs,
    tools,
    ready,
  });
}
