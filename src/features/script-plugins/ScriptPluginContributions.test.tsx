import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import { createUiExtensionRegistry, uiSlots } from "../../platform/extensions/UiExtensionRegistry";
import { GeneratedJobStore } from "../../platform/jobs/GeneratedJobStore";
import { MachineSnapshotStore } from "../../platform/machine/MachineStateSource";
import type { ScriptPluginGateway } from "../../platform/plugins/ScriptPluginGateway";
import { emptySnapshot } from "../../shared/machine";
import type { InstalledScriptPlugin } from "../../shared/scriptPlugins";
import { registerScriptPluginContributions } from "./ScriptPluginContributions";

const gateway: ScriptPluginGateway = {
  list: vi.fn(),
  importPackage: vi.fn(),
  savePackage: vi.fn(),
  exportPackage: vi.fn(),
  configure: vi.fn(),
  delete: vi.fn(),
  execute: vi.fn(),
};

const plugin: InstalledScriptPlugin = {
  digest: "abc123",
  enabled: true,
  bundled: false,
  grantedCapabilities: ["ui.contribute", "jobs.create", "machine.jog"],
  package: {
    packageVersion: 1,
    manifest: {
      manifestVersion: 1,
      apiVersion: 1,
      id: "community.fixture",
      name: "Fixture plugin",
      version: "1.0.0",
      description: "Fixture",
      capabilities: {
        required: ["ui.contribute"],
        optional: ["jobs.create", "machine.jog"],
      },
    },
    commands: [
      {
        id: "create",
        title: "Создать fixture",
        description: "Creates a job",
        icon: "braces",
        surface: "workspaceTools",
        fields: [],
        requiredCapabilities: ["jobs.create"],
      },
      {
        id: "raise",
        title: "Поднять fixture",
        description: "Raises Z",
        icon: "arrow-up-from-line",
        surface: "machinePanel",
        fields: [],
        requiredCapabilities: ["machine.jog"],
      },
    ],
    source: "fn run(command, input, machine) { #{} }",
  },
};

describe("external script plugin contributions", () => {
  it("mounts declared workspace and machine UI and fully unloads it", () => {
    const registry = createUiExtensionRegistry();
    const registrations = registerScriptPluginContributions({
      gateway,
      jobs: new GeneratedJobStore(),
      machine: new MachineSnapshotStore(emptySnapshot),
      onError: vi.fn(),
      plugins: [plugin],
      registry,
    });

    const workspace = registry.list(uiSlots.workspaceTools)[0];
    const machine = registry.list(uiSlots.controlMachine)[0];
    expect(renderToStaticMarkup(workspace.extension.kind === "global" ? workspace.extension.render() : null)).toContain("Создать fixture");
    expect(renderToStaticMarkup(machine.extension.kind === "global" ? machine.extension.render() : null)).toContain("Поднять fixture");

    for (const registration of registrations) registration.dispose();
    expect(registry.list(uiSlots.workspaceTools)).toEqual([]);
    expect(registry.list(uiSlots.controlMachine)).toEqual([]);
  });

  it("does not mount commands whose capability was not granted", () => {
    const registry = createUiExtensionRegistry();
    registerScriptPluginContributions({
      gateway,
      jobs: new GeneratedJobStore(),
      machine: new MachineSnapshotStore(emptySnapshot),
      onError: vi.fn(),
      plugins: [{ ...plugin, grantedCapabilities: ["ui.contribute"] }],
      registry,
    });

    expect(registry.list(uiSlots.workspaceTools)).toEqual([]);
    expect(registry.list(uiSlots.controlMachine)).toEqual([]);
  });
});
