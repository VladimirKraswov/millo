import type { ZProbeSettings } from "../../shared/machine";

export const validateZProbeSettings = (
  settings: ZProbeSettings,
): string | undefined => {
  if (
    !Number.isFinite(settings.plateThicknessMm) ||
    settings.plateThicknessMm < 0 ||
    settings.plateThicknessMm > 100
  ) {
    return "Толщина пластины должна быть от 0 до 100 mm";
  }
  if (
    !Number.isFinite(settings.maxTravelMm) ||
    settings.maxTravelMm < 0.1 ||
    settings.maxTravelMm > 100
  ) {
    return "Ход поиска должен быть от 0.1 до 100 mm";
  }
  if (
    !Number.isFinite(settings.probeFeedMmPerMin) ||
    settings.probeFeedMmPerMin < 1 ||
    settings.probeFeedMmPerMin > 500
  ) {
    return "Подача касания должна быть от 1 до 500 mm/min";
  }
  if (
    !Number.isFinite(settings.retractMm) ||
    settings.retractMm < 0.1 ||
    settings.retractMm > 100
  ) {
    return "Отвод должен быть от 0.1 до 100 mm";
  }
  if (
    !Number.isFinite(settings.retractFeedMmPerMin) ||
    settings.retractFeedMmPerMin < 1 ||
    settings.retractFeedMmPerMin > 2_000
  ) {
    return "Подача отвода должна быть от 1 до 2000 mm/min";
  }
  return undefined;
};

export const validateZProbeRunSettings = (
  settings: ZProbeSettings,
): string | undefined => {
  const settingsError = validateZProbeSettings(settings);
  if (settingsError) return settingsError;
  if (settings.plateThicknessMm < 0.01) {
    return "Перед касанием введите измеренную толщину пластины";
  }
  return undefined;
};

export const zProbeFinalWorkZ = (settings: ZProbeSettings): number =>
  settings.plateThicknessMm + settings.retractMm;
