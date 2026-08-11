import { describe, expect, it } from "vitest";

import type { ControllerSettingValue } from "../../shared/settings";
import { filterSettings, settingValuesEqual } from "./machineSettingsModel";

const values: ControllerSettingValue[] = [
  {
    key: "$120",
    value: "500.000",
    title: "X acceleration",
    group: "motion",
    kind: "decimal",
    unit: "mm/s^2",
    known: true,
  },
  {
    key: "$200",
    value: "7.5",
    title: "Firmware setting 200",
    group: "advanced",
    kind: "decimal",
    known: false,
  },
];

describe("machine settings model", () => {
  it("compares the controller's numeric formatting semantically", () => {
    expect(settingValuesEqual("500", "500.000")).toBe(true);
    expect(settingValuesEqual("500.1", "500.000")).toBe(false);
  });

  it("searches known and unknown settings without dropping firmware values", () => {
    expect(filterSettings(values, "acceleration").map(({ key }) => key)).toEqual([
      "$120",
    ]);
    expect(filterSettings(values, "$200").map(({ key }) => key)).toEqual([
      "$200",
    ]);
    expect(filterSettings(values, "")).toHaveLength(2);
  });
});

