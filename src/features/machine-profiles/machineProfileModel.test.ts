import { describe, expect, it } from "vitest";

import {
  emptyMachineProfileDraft,
  formatMachineTravel,
  validateMachineProfileDraft,
} from "./machineProfileModel";

describe("machine profile model", () => {
  it("defaults every unverified hardware feature to absent", () => {
    const draft = emptyMachineProfileDraft();

    expect(draft.spindleControl).toBe("manual");
    expect(draft.homingInstalled).toBe(false);
    expect(draft.limitSwitchesInstalled).toBe(false);
    expect(draft.probeInstalled).toBe(false);
    expect(draft.emergencyStopInstalled).toBe(false);
    expect(draft.maxJogDistanceMm).toBe(50);
  });

  it("requires only a name and finite positive XYZ travel", () => {
    const draft = emptyMachineProfileDraft();
    expect(validateMachineProfileDraft(draft)).toBe("Укажите название станка");

    draft.name = "Bench CNC";
    draft.travelMm = { x: 300, y: 180, z: 0 };
    expect(validateMachineProfileDraft(draft)).toBe("Укажите положительный ход Z");

    draft.travelMm.z = 45;
    expect(validateMachineProfileDraft(draft)).toBeUndefined();
    expect(formatMachineTravel(draft)).toBe("300 × 180 × 45 mm");

    draft.maxJogDistanceMm = 301;
    expect(validateMachineProfileDraft(draft)).toContain("Максимальный jog");

    draft.travelMm = { x: 2_000, y: 3_000, z: 250 };
    draft.maxJogDistanceMm = 3_000;
    expect(validateMachineProfileDraft(draft)).toBeUndefined();
  });
});
