import { describe, expect, it } from "vitest";

import { defaultZProbeSettings } from "../../shared/profile";
import {
  validateZProbeRunSettings,
  validateZProbeSettings,
  zProbeFinalWorkZ,
} from "./zProbeModel";

describe("zProbeModel", () => {
  it("requires a measured positive plate thickness", () => {
    const settings = defaultZProbeSettings();

    expect(validateZProbeSettings(settings)).toBeUndefined();
    expect(validateZProbeRunSettings(settings)).toContain("толщину");
    expect(validateZProbeSettings({ ...settings, plateThicknessMm: 19.1 })).toBeUndefined();
    expect(validateZProbeRunSettings({ ...settings, plateThicknessMm: 19.1 })).toBeUndefined();
  });

  it("calculates the visible final work position after retract", () => {
    expect(
      zProbeFinalWorkZ({
        ...defaultZProbeSettings(),
        mode: "workZero",
        plateThicknessMm: 19.1,
        retractMm: 3,
      }),
    ).toBeCloseTo(22.1);
  });
});
