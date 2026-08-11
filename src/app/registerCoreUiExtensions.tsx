import { JogPad } from "../features/jog-pad/JogPad";
import {
  uiSlots,
  type UiExtensionRegistry,
} from "../platform/extensions/UiExtensionRegistry";

export const CORE_EXTENSION_OWNER = "core";
export const CORE_JOG_PAD_CONTRIBUTION = "core.jog-pad";

export function registerCoreUiExtensions(registry: UiExtensionRegistry): void {
  registry.register({
    id: CORE_JOG_PAD_CONTRIBUTION,
    owner: CORE_EXTENSION_OWNER,
    slot: uiSlots.controlMachine,
    order: 100,
    extension: (context) => (
      <JogPad
        desktopRuntime={context.desktopRuntime}
        disabled={context.controlsDisabled}
        gateway={context.machineCommands}
        onError={context.reportError}
        onInspection={context.updateInspection}
        snapshot={context.snapshot}
      />
    ),
  });
}
