import type { ControllerSnapshot } from "../shared/machine";
import {
  createUiExtensionRegistry,
  type UiExtensionRegistry,
} from "../platform/extensions/UiExtensionRegistry";
import type { MachineCommandGateway } from "../platform/machine/MachineCommandGateway";
import { MachineSnapshotStore } from "../platform/machine/MachineStateSource";
import { CapabilityGrantStore } from "../platform/plugins/CapabilityGrantStore";
import { InMemoryPluginLoader } from "../platform/plugins/InMemoryPluginLoader";
import { registerCoreUiExtensions } from "./registerCoreUiExtensions";

export interface PluginHost {
  readonly uiRegistry: UiExtensionRegistry;
  readonly machineState: MachineSnapshotStore;
  readonly plugins: InMemoryPluginLoader;
}

export interface PluginHostOptions {
  readonly initialSnapshot: ControllerSnapshot;
  readonly machineCommands: MachineCommandGateway;
  readonly grants?: CapabilityGrantStore;
  readonly onPluginError?: (pluginId: string, error: unknown) => void;
}

export function bootstrapPluginHost(options: PluginHostOptions): PluginHost {
  const uiRegistry = createUiExtensionRegistry();
  registerCoreUiExtensions(uiRegistry);
  const machineState = new MachineSnapshotStore(options.initialSnapshot);
  const plugins = new InMemoryPluginLoader({
    uiRegistry,
    machineState,
    machineCommands: options.machineCommands,
    grants: options.grants,
    onPluginError: options.onPluginError,
  });

  return Object.freeze({ uiRegistry, machineState, plugins });
}
