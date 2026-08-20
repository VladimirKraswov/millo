import { describe, expect, it } from "vitest";

import {
  capabilityLabels,
  commandNeedsMachineConfirmation,
  scriptCapabilityCatalog,
} from "./scriptPlugins";

describe("script plugin command capabilities", () => {
  it("treats expert controller commands as explicit confirmed authority", () => {
    expect(scriptCapabilityCatalog).toContain("machine.commands");
    expect(capabilityLabels["machine.commands"]).toContain("GRBL");
    expect(
      commandNeedsMachineConfirmation({
        id: "expert",
        title: "Expert",
        description: "Fixture",
        icon: "terminal",
        surface: "machinePanel",
        fields: [],
        requiredCapabilities: ["machine.commands"],
      }),
    ).toBe(true);
  });
});
