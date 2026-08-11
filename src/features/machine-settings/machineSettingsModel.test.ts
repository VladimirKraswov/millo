import { describe, expect, it } from "vitest";

import type {
  ControllerSettingsState,
  ControllerSettingValue,
} from "../../shared/settings";
import {
  controllerSettingsIdentity,
  createSettingsWriteToken,
  filterSettings,
  isSettingsWriteTokenCurrent,
  settingValuesEqual,
} from "./machineSettingsModel";

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

  it("binds queued writes to the controller fingerprint and profile", () => {
    const state = {
      snapshot: { revision: 1, values: [] },
      sessionBaseline: {},
      revisionCount: 0,
      profileId: "machine-0001",
      fingerprint: {
        key: "usb:0483:5740:abc",
        confidence: "strong",
        label: "Controller",
      },
    } satisfies ControllerSettingsState;

    expect(controllerSettingsIdentity(state)).toBe(
      "usb:0483:5740:abc\u0000machine-0001",
    );
    expect(
      controllerSettingsIdentity({ ...state, profileId: "machine-0002" }),
    ).not.toBe(controllerSettingsIdentity(state));
    expect(
      controllerSettingsIdentity({
        ...state,
        snapshot: { revision: 99, values: [] },
      }),
    ).toBe(controllerSettingsIdentity(state));

    const token = createSettingsWriteToken(3, state);
    expect(isSettingsWriteTokenCurrent(token, 3, state, true)).toBe(true);
    expect(isSettingsWriteTokenCurrent(token, 4, state, true)).toBe(false);
    expect(isSettingsWriteTokenCurrent(token, 3, state, false)).toBe(false);
    expect(
      isSettingsWriteTokenCurrent(
        token,
        3,
        { ...state, profileId: "machine-0002" },
        true,
      ),
    ).toBe(false);
  });
});
