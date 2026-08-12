import { useEffect } from "react";

import type { ExtensionRegistration } from "../../platform/extensions/ExtensionRegistry";
import type { UiExtensionRegistry } from "../../platform/extensions/UiExtensionRegistry";
import { uiSlots } from "../../platform/extensions/UiExtensionRegistry";
import type { GeneratedJobStore } from "../../platform/jobs/GeneratedJobStore";
import type { MachineSnapshotStore } from "../../platform/machine/MachineStateSource";
import type { ScriptPluginGateway } from "../../platform/plugins/ScriptPluginGateway";
import type { InstalledScriptPlugin } from "../../shared/scriptPlugins";
import { ScriptCommandLauncher } from "./ScriptCommandLauncher";

export function ScriptPluginContributions({
  gateway,
  jobs,
  machine,
  onError,
  plugins,
  registry,
}: {
  readonly gateway: ScriptPluginGateway;
  readonly jobs: GeneratedJobStore;
  readonly machine: MachineSnapshotStore;
  readonly onError: (error?: string) => void;
  readonly plugins: readonly InstalledScriptPlugin[];
  readonly registry: UiExtensionRegistry;
}) {
  useEffect(() => {
    const registrations = registerScriptPluginContributions({
      gateway,
      jobs,
      machine,
      onError,
      plugins,
      registry,
    });
    return () => {
      for (const registration of registrations) registration.dispose();
    };
  }, [gateway, jobs, machine, onError, plugins, registry]);

  return null;
}

export function registerScriptPluginContributions({
  gateway,
  jobs,
  machine,
  onError,
  plugins,
  registry,
}: {
  readonly gateway: ScriptPluginGateway;
  readonly jobs: GeneratedJobStore;
  readonly machine: MachineSnapshotStore;
  readonly onError: (error?: string) => void;
  readonly plugins: readonly InstalledScriptPlugin[];
  readonly registry: UiExtensionRegistry;
}): ExtensionRegistration[] {
  return plugins
      .filter((plugin) => plugin.enabled)
      .flatMap((plugin, pluginIndex) => {
        const commands = plugin.package.commands.filter((command) =>
          command.requiredCapabilities.every((capability) =>
            plugin.grantedCapabilities.includes(capability),
          ),
        );
        const workspace = commands
          .filter((command) => command.surface === "workspaceTools")
          .map((command, commandIndex) =>
            registry.register({
              id: `script.${plugin.package.manifest.id}.${command.id}`,
              owner: `script.${plugin.package.manifest.id}`,
              slot: uiSlots.workspaceTools,
              order: 500 + pluginIndex * 100 + commandIndex,
              extension: {
                kind: "global",
                render: () => (
                  <ScriptCommandLauncher
                    command={command}
                    gateway={gateway}
                    jobs={jobs}
                    machine={machine}
                    onError={onError}
                    plugin={plugin}
                  />
                ),
              },
            }),
          );
        const machineCommands = commands.filter(
          (command) => command.surface === "machinePanel",
        );
        if (machineCommands.length === 0) return workspace;
        const machineRegistration = registry.register({
          id: `script.${plugin.package.manifest.id}.machine-panel`,
          owner: `script.${plugin.package.manifest.id}`,
          slot: uiSlots.controlMachine,
          order: 500 + pluginIndex * 100,
          extension: {
            kind: "global",
            render: () => (
              <div className="script-machine-panel">
                {machineCommands.map((command) => (
                  <ScriptCommandLauncher
                    command={command}
                    compact
                    gateway={gateway}
                    jobs={jobs}
                    key={command.id}
                    machine={machine}
                    onError={onError}
                    plugin={plugin}
                  />
                ))}
              </div>
            ),
          },
        });
        return [...workspace, machineRegistration];
      });
}
