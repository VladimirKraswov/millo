import { CORE_JOG_PAD_CONTRIBUTION } from "../../app/registerCoreUiExtensions";
import { uiSlots } from "../../platform/extensions/UiExtensionRegistry";
import type {
  InMemoryPluginModule,
  PluginActivationContext,
} from "../../platform/plugins/InMemoryPluginLoader";
import {
  PLUGIN_API_VERSION,
  PLUGIN_MANIFEST_VERSION,
} from "../../platform/plugins/PluginManifest";

export const TEST_PLUGIN_ID = "dev.millo.fixture";

export interface TestPluginObservations {
  activations: number;
  deactivations: number;
  machineJogGranted: boolean;
}

export function createTestUiPlugin(): {
  plugin: InMemoryPluginModule;
  observations: TestPluginObservations;
} {
  const observations: TestPluginObservations = {
    activations: 0,
    deactivations: 0,
    machineJogGranted: false,
  };
  const plugin: InMemoryPluginModule = {
    manifest: {
      manifestVersion: PLUGIN_MANIFEST_VERSION,
      apiVersion: PLUGIN_API_VERSION,
      id: TEST_PLUGIN_ID,
      name: "Millo fixture UI",
      version: "0.1.0",
      capabilities: {
        required: ["ui.contribute"],
        optional: ["machine.jog"],
      },
    },
    activate(context: PluginActivationContext) {
      observations.activations += 1;
      observations.machineJogGranted = context.hasCapability("machine.jog");
      context.ui?.register({
        id: `${TEST_PLUGIN_ID}.jog-panel`,
        slot: uiSlots.controlMachine,
        order: 50,
        replaces: [CORE_JOG_PAD_CONTRIBUTION],
        render: () => (
          <section aria-label="Fixture machine controls">
            Fixture plugin active
          </section>
        ),
      });
      return () => {
        observations.deactivations += 1;
      };
    },
  };

  return { plugin, observations };
}
