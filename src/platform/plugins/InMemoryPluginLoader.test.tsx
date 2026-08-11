import { describe, expect, it, vi } from "vitest";

import {
  CORE_JOG_PAD_CONTRIBUTION,
  registerCoreUiExtensions,
} from "../../app/registerCoreUiExtensions";
import {
  createTestUiPlugin,
  TEST_PLUGIN_ID,
} from "../../plugins/testing/createTestUiPlugin";
import {
  createUiExtensionRegistry,
  uiSlots,
} from "../extensions/UiExtensionRegistry";
import type { MachineCommandGateway } from "../machine/MachineCommandGateway";
import type {
  JogPadStepOutcome,
  JogPadStepRequest,
} from "../../shared/machine";
import { CapabilityGrantStore } from "./CapabilityGrantStore";
import {
  InMemoryPluginLoader,
  type InMemoryPluginModule,
} from "./InMemoryPluginLoader";
import {
  PLUGIN_API_VERSION,
  PLUGIN_MANIFEST_VERSION,
  type PluginCapability,
} from "./PluginManifest";

const machineCommands: MachineCommandGateway = {
  jogPadStep: vi.fn(),
};

function createLoader(capabilities: readonly PluginCapability[]) {
  const uiRegistry = createUiExtensionRegistry();
  registerCoreUiExtensions(uiRegistry);
  const loader = new InMemoryPluginLoader({
    uiRegistry,
    machineCommands,
    grants: new CapabilityGrantStore([
      { pluginId: TEST_PLUGIN_ID, capabilities },
    ]),
  });
  return { loader, uiRegistry };
}

function moduleWith(
  id: string,
  required: readonly PluginCapability[],
  activate: InMemoryPluginModule["activate"],
): InMemoryPluginModule {
  return {
    manifest: {
      manifestVersion: PLUGIN_MANIFEST_VERSION,
      apiVersion: PLUGIN_API_VERSION,
      id,
      name: "Loader fixture",
      version: "0.1.0",
      capabilities: { required },
    },
    activate,
  };
}

describe("InMemoryPluginLoader", () => {
  it("loads and unloads the fixture while restoring the core contribution", async () => {
    const { loader, uiRegistry } = createLoader(["ui.contribute"]);
    const { plugin, observations } = createTestUiPlugin();

    const result = await loader.load(plugin);

    expect(result.grantedCapabilities).toEqual(["ui.contribute"]);
    expect(result.deniedOptionalCapabilities).toEqual(["machine.jog"]);
    expect(observations).toEqual({
      activations: 1,
      deactivations: 0,
      machineJogGranted: false,
    });
    expect(uiRegistry.list(uiSlots.controlMachine).map(({ id }) => id)).toEqual([
      `${TEST_PLUGIN_ID}.jog-panel`,
    ]);

    await expect(loader.unload(TEST_PLUGIN_ID)).resolves.toBe(true);
    expect(observations.deactivations).toBe(1);
    expect(uiRegistry.list(uiSlots.controlMachine).map(({ id }) => id)).toEqual([
      CORE_JOG_PAD_CONTRIBUTION,
    ]);
  });

  it("exposes an optional machine proxy only when explicitly granted", async () => {
    const { loader } = createLoader(["ui.contribute", "machine.jog"]);
    const { plugin, observations } = createTestUiPlugin();

    const result = await loader.load(plugin);

    expect(result.grantedCapabilities).toEqual([
      "ui.contribute",
      "machine.jog",
    ]);
    expect(observations.machineJogGranted).toBe(true);
  });

  it("delegates granted machine jog through the typed gateway", async () => {
    const pluginId = "dev.millo.motion-fixture";
    const request: JogPadStepRequest = {
      confirmation: {
        spindleOff: true,
        toolClear: true,
        powerControlReachable: true,
      },
      axis: "x",
      distanceMm: 0.01,
    };
    const outcome = {} as JogPadStepOutcome;
    const jogPadStep = vi.fn(async () => outcome);
    const gateway: MachineCommandGateway = { jogPadStep };
    const loader = new InMemoryPluginLoader({
      uiRegistry: createUiExtensionRegistry(),
      machineCommands: gateway,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["machine.jog"] },
      ]),
    });
    let jog = undefined as
      | Parameters<InMemoryPluginModule["activate"]>[0]["machineJog"]
      | undefined;
    const plugin = moduleWith(pluginId, ["machine.jog"], (context) => {
      jog = context.machineJog;
    });

    await loader.load(plugin);

    await expect(jog?.step(request)).resolves.toBe(outcome);
    expect(jogPadStep).toHaveBeenCalledWith(request);
  });

  it("fails before activation when a required capability is unavailable", async () => {
    const uiRegistry = createUiExtensionRegistry();
    const activate = vi.fn();
    const loader = new InMemoryPluginLoader({
      uiRegistry,
      grants: new CapabilityGrantStore([
        {
          pluginId: "dev.millo.jobs-fixture",
          capabilities: ["jobs.create"],
        },
      ]),
    });
    const plugin = moduleWith(
      "dev.millo.jobs-fixture",
      ["jobs.create"],
      activate,
    );

    await expect(loader.load(plugin)).rejects.toThrow(
      "missing required capabilities: jobs.create",
    );
    expect(activate).not.toHaveBeenCalled();
  });

  it("rejects API mismatch before activation", async () => {
    const uiRegistry = createUiExtensionRegistry();
    const activate = vi.fn();
    const loader = new InMemoryPluginLoader({ uiRegistry });
    const basePlugin = moduleWith("dev.millo.future", [], activate);
    const plugin: InMemoryPluginModule = {
      ...basePlugin,
      manifest: {
        ...(basePlugin.manifest as Record<string, unknown>),
        apiVersion: PLUGIN_API_VERSION + 1,
      },
    };

    await expect(loader.load(plugin)).rejects.toThrow("requires API 2");
    expect(activate).not.toHaveBeenCalled();
  });

  it("removes partial UI when activation fails", async () => {
    const uiRegistry = createUiExtensionRegistry();
    registerCoreUiExtensions(uiRegistry);
    const pluginId = "dev.millo.broken";
    const loader = new InMemoryPluginLoader({
      uiRegistry,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["ui.contribute"] },
      ]),
    });
    const plugin = moduleWith(pluginId, ["ui.contribute"], (context) => {
      context.ui?.register({
        id: `${pluginId}.panel`,
        slot: uiSlots.controlMachine,
        replaces: [CORE_JOG_PAD_CONTRIBUTION],
        render: () => null,
      });
      throw new Error("activation failed");
    });

    await expect(loader.load(plugin)).rejects.toThrow("activation failed");
    expect(uiRegistry.list(uiSlots.controlMachine).map(({ id }) => id)).toEqual([
      CORE_JOG_PAD_CONTRIBUTION,
    ]);
  });

  it("binds every UI contribution to the plugin namespace", async () => {
    const pluginId = "dev.millo.spoof";
    const uiRegistry = createUiExtensionRegistry();
    const loader = new InMemoryPluginLoader({
      uiRegistry,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["ui.contribute"] },
      ]),
    });
    const plugin = moduleWith(pluginId, ["ui.contribute"], (context) => {
      context.ui?.register({
        id: "core.spoofed",
        slot: uiSlots.controlMachine,
        render: () => null,
      });
    });

    await expect(loader.load(plugin)).rejects.toThrow(
      `must be namespaced with ${pluginId}.`,
    );
    expect(uiRegistry.list(uiSlots.controlMachine)).toEqual([]);
  });

  it("removes owned UI even when plugin deactivation fails", async () => {
    const pluginId = "dev.millo.unload-failure";
    const uiRegistry = createUiExtensionRegistry();
    registerCoreUiExtensions(uiRegistry);
    const loader = new InMemoryPluginLoader({
      uiRegistry,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["ui.contribute"] },
      ]),
    });
    const plugin = moduleWith(pluginId, ["ui.contribute"], (context) => {
      context.ui?.register({
        id: `${pluginId}.panel`,
        slot: uiSlots.controlMachine,
        replaces: [CORE_JOG_PAD_CONTRIBUTION],
        render: () => null,
      });
      return () => {
        throw new Error("deactivation failed");
      };
    });
    await loader.load(plugin);

    await expect(loader.unload(pluginId)).rejects.toThrow("deactivation failed");
    expect(loader.list()).toEqual([]);
    expect(uiRegistry.list(uiSlots.controlMachine).map(({ id }) => id)).toEqual([
      CORE_JOG_PAD_CONTRIBUTION,
    ]);
  });
});
