import { describe, expect, it, vi } from "vitest";

import type { MachineCommandGateway } from "../platform/machine/MachineCommandGateway";
import { CapabilityGrantStore } from "../platform/plugins/CapabilityGrantStore";
import type { InMemoryPluginModule } from "../platform/plugins/InMemoryPluginLoader";
import {
  PLUGIN_API_VERSION,
  PLUGIN_MANIFEST_VERSION,
} from "../platform/plugins/PluginManifest";
import type { ControllerSnapshot } from "../shared/machine";
import { emptySnapshot } from "../shared/machine";
import { uiSlots } from "../platform/extensions/UiExtensionRegistry";
import { CORE_JOG_PAD_CONTRIBUTION } from "./registerCoreUiExtensions";
import { bootstrapPluginHost } from "./bootstrapPluginHost";

const machineCommands: MachineCommandGateway = {
  jogPadStep: vi.fn(),
};

function snapshot(pollSequence: number): ControllerSnapshot {
  return {
    ...emptySnapshot,
    machine: { ...emptySnapshot.machine },
    pollSequence,
  };
}

describe("bootstrapPluginHost", () => {
  it("starts one shared host with core UI and no activated plugins", () => {
    const host = bootstrapPluginHost({
      initialSnapshot: snapshot(1),
      machineCommands,
    });

    expect(Object.isFrozen(host)).toBe(true);
    expect(host.machineState.current().pollSequence).toBe(1);
    expect(host.plugins.list()).toEqual([]);
    expect(host.uiRegistry.list(uiSlots.controlMachine).map(({ id }) => id)).toEqual([
      CORE_JOG_PAD_CONTRIBUTION,
    ]);
  });

  it("gives an explicitly loaded plugin the shared machine state source", async () => {
    const pluginId = "dev.millo.bootstrap-observer";
    const listener = vi.fn();
    const host = bootstrapPluginHost({
      initialSnapshot: snapshot(0),
      machineCommands,
      grants: new CapabilityGrantStore([
        { pluginId, capabilities: ["machine.read"] },
      ]),
    });
    const plugin: InMemoryPluginModule = {
      manifest: {
        manifestVersion: PLUGIN_MANIFEST_VERSION,
        apiVersion: PLUGIN_API_VERSION,
        id: pluginId,
        name: "Bootstrap observer",
        version: "0.1.0",
        capabilities: { required: ["machine.read"] },
      },
      activate(context) {
        context.machineRead?.subscribe(listener);
      },
    };

    await host.plugins.load(plugin);
    host.machineState.publish(snapshot(3));

    expect(listener).toHaveBeenCalledOnce();
    expect(listener.mock.calls[0]?.[0].pollSequence).toBe(3);
    await host.plugins.unload(pluginId);
  });
});
