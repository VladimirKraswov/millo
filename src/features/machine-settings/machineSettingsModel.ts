import type {
  ControllerSettingsState,
  ControllerSettingValue,
  SettingGroup,
} from "../../shared/settings";

export const settingGroupOrder: readonly SettingGroup[] = [
  "safety",
  "homing",
  "travel",
  "calibration",
  "motion",
  "spindle",
  "pins",
  "interface",
  "advanced",
];

export const settingGroupLabels: Record<SettingGroup, string> = {
  safety: "Безопасность",
  homing: "Homing",
  travel: "Рабочая область",
  calibration: "Калибровка осей",
  motion: "Динамика",
  spindle: "Шпиндель и laser",
  pins: "Сигналы и инверсия",
  interface: "Протокол",
  advanced: "Параметры прошивки",
};

export const settingValuesEqual = (left: string, right: string): boolean => {
  const leftNumber = Number(left);
  const rightNumber = Number(right);
  return Number.isFinite(leftNumber) && Number.isFinite(rightNumber)
    ? Math.abs(leftNumber - rightNumber) <= 0.000_001
    : left === right;
};

export const controllerSettingsIdentity = (
  state?: ControllerSettingsState,
): string | undefined =>
  state ? `${state.fingerprint.key}\u0000${state.profileId ?? "unbound"}` : undefined;

export interface SettingsWriteToken {
  readonly lifecycle: number;
  readonly settingsIdentity?: string;
}

export const createSettingsWriteToken = (
  lifecycle: number,
  state?: ControllerSettingsState,
): SettingsWriteToken => ({
  lifecycle,
  settingsIdentity: controllerSettingsIdentity(state),
});

export const isSettingsWriteTokenCurrent = (
  token: SettingsWriteToken,
  lifecycle: number,
  state: ControllerSettingsState | undefined,
  dialogOpen: boolean,
): boolean =>
  dialogOpen &&
  token.lifecycle === lifecycle &&
  token.settingsIdentity === controllerSettingsIdentity(state);

export const filterSettings = (
  values: readonly ControllerSettingValue[],
  query: string,
): ControllerSettingValue[] => {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return [...values];
  return values.filter((setting) =>
    [setting.key, setting.title, setting.unit]
      .filter(Boolean)
      .some((value) => value!.toLocaleLowerCase().includes(needle)),
  );
};
