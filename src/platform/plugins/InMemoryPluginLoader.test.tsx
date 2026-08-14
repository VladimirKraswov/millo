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
import type { WorkCoordinateGateway } from "../machine/WorkCoordinateGateway";
import { MachineSnapshotStore } from "../machine/MachineStateSource";
import type { JobCreationCapability } from "../jobs/JobCreationService";
import type { ToolLibraryGateway } from "../tooling/ToolLibraryGateway";
import { ToolLibraryService } from "../tooling/ToolLibraryService";
import type {
  PluginActivationContext,
  PluginMachineJogCapability,
  PluginMachineCoordinatesCapability,
  PluginMachineReadCapability,
  PluginToolsCapability,
} from "./InMemoryPluginLoader";
import type {
  ControllerSnapshot,
  JogPadStepOutcome,
  JogPadStepRequest,
} from "../../shared/machine";
import { emptySnapshot } from "../../shared/machine";
import type { GeneratedImageJob, ImageJobRequest } from "../../shared/jobs";
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

function controllerSnapshot(pollSequence: number): ControllerSnapshot {
  return {
    ...emptySnapshot,
    connection: "connected",
    machine: {
      ...emptySnapshot.machine,
      mode: "idle",
      reportedMode: "Idle",
      machinePosition: { x: pollSequence, y: 0, z: 0 },
    },
    pollSequence,
  };
}

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
      feedMmPerMin: 100,
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
    let jog: PluginMachineJogCapability | undefined;
    const plugin = moduleWith(pluginId, ["machine.jog"], (context) => {
      jog = context.machineJog;
    });

    await loader.load(plugin);

    await expect(jog?.step(request)).resolves.toBe(outcome);
    expect(jogPadStep).toHaveBeenCalledWith(request);
  });

  it("delegates granted work-coordinate operations and closes them on unload", async () => {
    const pluginId = "dev.millo.coordinates-fixture";
    const setZero = vi.fn(async () => ({}) as never);
    const returnToZero = vi.fn(async () => ({}) as never);
    const workCoordinates: WorkCoordinateGateway = { setZero, returnToZero };
    const loader = new InMemoryPluginLoader({
      uiRegistry: createUiExtensionRegistry(),
      workCoordinates,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["machine.coordinates"] },
      ]),
    });
    let coordinates: PluginMachineCoordinatesCapability | undefined;
    const plugin = moduleWith(pluginId, ["machine.coordinates"], (context) => {
      coordinates = context.machineCoordinates;
    });
    const zeroRequest = { axis: "z", positionConfirmed: true } as const;
    const returnRequest = { axis: "z", feedMmPerMin: 100 } as const;

    await loader.load(plugin);
    await coordinates?.setZero(zeroRequest);
    await coordinates?.returnToZero(returnRequest);
    expect(setZero).toHaveBeenCalledWith(zeroRequest);
    expect(returnToZero).toHaveBeenCalledWith(returnRequest);

    await loader.unload(pluginId);
    await expect(coordinates?.setZero(zeroRequest)).rejects.toThrow("no longer active");
  });

  it("exposes immutable current state and future updates when granted", async () => {
    const pluginId = "dev.millo.observer";
    const machineState = new MachineSnapshotStore(controllerSnapshot(1));
    const listener = vi.fn();
    let machineRead: PluginMachineReadCapability | undefined;
    const loader = new InMemoryPluginLoader({
      uiRegistry: createUiExtensionRegistry(),
      machineState,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["machine.read"] },
      ]),
    });
    const plugin = moduleWith(pluginId, ["machine.read"], (context) => {
      machineRead = context.machineRead;
      context.machineRead?.subscribe(listener);
    });

    await loader.load(plugin);
    const current = machineRead?.current();
    machineState.publish(controllerSnapshot(2));

    expect(current?.pollSequence).toBe(1);
    expect(Object.isFrozen(current)).toBe(true);
    expect(Object.isFrozen(current?.machine.machinePosition)).toBe(true);
    expect(listener).toHaveBeenCalledOnce();
    expect(listener.mock.calls[0]?.[0].pollSequence).toBe(2);
  });

  it("does not grant machine state merely because a source is available", async () => {
    const pluginId = "dev.millo.ungranted-observer";
    const activate = vi.fn();
    const loader = new InMemoryPluginLoader({
      uiRegistry: createUiExtensionRegistry(),
      machineState: new MachineSnapshotStore(controllerSnapshot(0)),
    });
    const plugin = moduleWith(pluginId, ["machine.read"], activate);

    await expect(loader.load(plugin)).rejects.toThrow(
      "missing required capabilities: machine.read",
    );
    expect(activate).not.toHaveBeenCalled();
  });

  it("removes machine subscriptions and closes retained proxies on unload", async () => {
    const pluginId = "dev.millo.lifecycle-observer";
    const machineState = new MachineSnapshotStore(controllerSnapshot(0));
    const listener = vi.fn();
    let machineRead: PluginMachineReadCapability | undefined;
    const loader = new InMemoryPluginLoader({
      uiRegistry: createUiExtensionRegistry(),
      machineState,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["machine.read"] },
      ]),
    });
    const plugin = moduleWith(pluginId, ["machine.read"], (context) => {
      machineRead = context.machineRead;
      context.machineRead?.subscribe(listener);
    });
    await loader.load(plugin);

    await loader.unload(pluginId);
    machineState.publish(controllerSnapshot(1));

    expect(listener).not.toHaveBeenCalled();
    expect(() => machineRead?.current()).toThrow(
      `plugin is no longer active: ${pluginId}`,
    );
    expect(() => machineRead?.subscribe(listener)).toThrow(
      `plugin is no longer active: ${pluginId}`,
    );
  });

  it("stops observations before asynchronous deactivation completes", async () => {
    const pluginId = "dev.millo.slow-deactivation";
    const machineState = new MachineSnapshotStore(controllerSnapshot(0));
    const listener = vi.fn();
    let finishDeactivation: (() => void) | undefined;
    const loader = new InMemoryPluginLoader({
      uiRegistry: createUiExtensionRegistry(),
      machineState,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["machine.read"] },
      ]),
    });
    const plugin = moduleWith(pluginId, ["machine.read"], (context) => {
      context.machineRead?.subscribe(listener);
      return () =>
        new Promise<void>((resolve) => {
          finishDeactivation = resolve;
        });
    });
    await loader.load(plugin);

    const unloading = loader.unload(pluginId);
    machineState.publish(controllerSnapshot(1));

    expect(listener).not.toHaveBeenCalled();
    finishDeactivation?.();
    await expect(unloading).resolves.toBe(true);
  });

  it("cancels capabilities and UI when unloaded during activation", async () => {
    const pluginId = "dev.millo.slow-activation";
    const uiRegistry = createUiExtensionRegistry();
    let finishActivation: (() => void) | undefined;
    const deactivate = vi.fn();
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
        render: () => "pending",
      });
      return new Promise<() => void>((resolve) => {
        finishActivation = () => resolve(deactivate);
      });
    });

    const loading = loader.load(plugin);
    await expect(loader.unload(pluginId)).resolves.toBe(true);
    expect(uiRegistry.list(uiSlots.controlMachine)).toEqual([]);

    finishActivation?.();
    await expect(loading).rejects.toThrow(
      `plugin was unloaded during activation: ${pluginId}`,
    );
    expect(deactivate).toHaveBeenCalledOnce();
    expect(loader.list()).toEqual([]);
  });

  it("unloadAll closes both active and still-loading plugins", async () => {
    const activeId = "dev.millo.active-for-shutdown";
    const loadingId = "dev.millo.loading-for-shutdown";
    const activeDeactivate = vi.fn();
    const loadingDeactivate = vi.fn();
    let finishActivation: (() => void) | undefined;
    const loader = new InMemoryPluginLoader({
      uiRegistry: createUiExtensionRegistry(),
    });
    await loader.load(moduleWith(activeId, [], () => activeDeactivate));
    const loading = loader.load(
      moduleWith(
        loadingId,
        [],
        () =>
          new Promise<() => void>((resolve) => {
            finishActivation = () => resolve(loadingDeactivate);
          }),
      ),
    );

    await loader.unloadAll();
    expect(activeDeactivate).toHaveBeenCalledOnce();
    finishActivation?.();
    await expect(loading).rejects.toThrow("unloaded during activation");
    expect(loadingDeactivate).toHaveBeenCalledOnce();
    expect(loader.list()).toEqual([]);
  });

  it("removes subscriptions registered before activation fails", async () => {
    const pluginId = "dev.millo.failed-observer";
    const machineState = new MachineSnapshotStore(controllerSnapshot(0));
    const listener = vi.fn();
    const loader = new InMemoryPluginLoader({
      uiRegistry: createUiExtensionRegistry(),
      machineState,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["machine.read"] },
      ]),
    });
    const plugin = moduleWith(pluginId, ["machine.read"], (context) => {
      context.machineRead?.subscribe(listener);
      throw new Error("observer activation failed");
    });

    await expect(loader.load(plugin)).rejects.toThrow(
      "observer activation failed",
    );
    machineState.publish(controllerSnapshot(1));

    expect(listener).not.toHaveBeenCalled();
  });

  it("isolates subscriber errors and reports them to the host", async () => {
    const pluginId = "dev.millo.throwing-observer";
    const machineState = new MachineSnapshotStore(controllerSnapshot(0));
    const onPluginError = vi.fn(() => {
      throw new Error("diagnostics failed");
    });
    const survivingListener = vi.fn();
    const loader = new InMemoryPluginLoader({
      uiRegistry: createUiExtensionRegistry(),
      machineState,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["machine.read"] },
      ]),
      onPluginError,
    });
    const plugin = moduleWith(pluginId, ["machine.read"], (context) => {
      context.machineRead?.subscribe(() => {
        throw new Error("subscriber failed");
      });
      context.machineRead?.subscribe(survivingListener);
    });
    await loader.load(plugin);

    machineState.publish(controllerSnapshot(1));

    expect(onPluginError).toHaveBeenCalledWith(pluginId, expect.any(Error));
    expect(survivingListener).toHaveBeenCalledOnce();
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

  it("scopes jobs.create to the host generation service and closes it on unload", async () => {
    const pluginId = "dev.millo.jobs-service";
    const generated = Object.freeze({}) as GeneratedImageJob;
    const generateImage = vi.fn(async () => generated);
    const generateSurfacing = vi.fn();
    const open = vi.fn();
    const save = vi.fn(async () => undefined);
    const jobs: JobCreationCapability = {
      generateImage,
      generateSurfacing,
      inspectPcb: vi.fn(),
      generatePcb: vi.fn(),
      open,
      save,
    };
    const loader = new InMemoryPluginLoader({
      uiRegistry: createUiExtensionRegistry(),
      jobs,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["jobs.create"] },
      ]),
    });
    let capability: PluginActivationContext["jobs"];
    const plugin = moduleWith(pluginId, ["jobs.create"], (context) => {
      capability = context.jobs;
    });
    const request = {} as ImageJobRequest;

    await loader.load(plugin);
    await expect(capability?.generateImage(request)).resolves.toBe(generated);
    capability?.open(generated);
    await expect(capability?.save(generated)).resolves.toBeUndefined();

    expect(generateImage).toHaveBeenCalledWith(request);
    expect(open).toHaveBeenCalledWith(generated);
    expect(save).toHaveBeenCalledWith(generated);
    await loader.unload(pluginId);
    expect(() => capability?.open(generated)).toThrow("no longer active");
  });

  it("scopes tools.read subscriptions and closes the proxy on unload", async () => {
    const pluginId = "dev.millo.tool-observer";
    const gateway = {
      load: vi.fn(),
      create: vi.fn(),
      update: vi.fn(),
      delete: vi.fn(),
      restorePresets: vi.fn(),
    } as unknown as ToolLibraryGateway;
    const tools = new ToolLibraryService(gateway);
    const unsubscribe = vi.fn();
    vi.spyOn(tools, "subscribe").mockReturnValue(unsubscribe);
    let capability: PluginToolsCapability | undefined;
    const loader = new InMemoryPluginLoader({
      uiRegistry: createUiExtensionRegistry(),
      tools,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["tools.read"] },
      ]),
    });
    const plugin = moduleWith(pluginId, ["tools.read"], (context) => {
      capability = context.tools;
      context.tools?.subscribe(vi.fn());
    });

    await loader.load(plugin);
    expect(Object.isFrozen(capability?.current())).toBe(true);
    await loader.unload(pluginId);

    expect(unsubscribe).toHaveBeenCalledOnce();
    expect(() => capability?.current()).toThrow("no longer active");
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
