import { describe, expect, it } from "vitest";

import {
  createUiExtensionRegistry,
  uiSlots,
} from "../platform/extensions/UiExtensionRegistry";
import {
  CORE_JOG_PAD_CONTRIBUTION,
  registerCoreUiExtensions,
} from "./registerCoreUiExtensions";

describe("core UI extensions", () => {
  it("mounts Jog Pad as a replaceable control.machine contribution", () => {
    const registry = createUiExtensionRegistry();
    registerCoreUiExtensions(registry);
    const replacement = registry.register({
      id: "plugin.custom-jog",
      owner: "plugin.example",
      slot: uiSlots.controlMachine,
      replaces: [CORE_JOG_PAD_CONTRIBUTION],
      extension: () => null,
    });

    expect(registry.list(uiSlots.controlMachine).map(({ id }) => id)).toEqual([
      "plugin.custom-jog",
    ]);
    replacement.dispose();
    expect(registry.list(uiSlots.controlMachine).map(({ id }) => id)).toEqual([
      CORE_JOG_PAD_CONTRIBUTION,
    ]);
  });
});
