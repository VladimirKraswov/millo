import type { MachineProfileDraft } from "../../shared/profile";
import { defaultZProbeSettings } from "../../shared/profile";

export const emptyMachineProfileDraft = (): MachineProfileDraft => ({
  name: "",
  travelMm: { x: 0, y: 0, z: 0 },
  maxJogDistanceMm: 50,
  spindleControl: "manual",
  floodCoolantControl: false,
  mistCoolantControl: false,
  homingInstalled: false,
  limitSwitchesInstalled: false,
  probeInstalled: false,
  probeSettings: defaultZProbeSettings(),
  emergencyStopInstalled: false,
});

export const validateMachineProfileDraft = (
  draft: MachineProfileDraft,
): string | undefined => {
  if (!draft.name.trim()) return "Укажите название станка";
  for (const axis of ["x", "y", "z"] as const) {
    const value = draft.travelMm[axis];
    if (!Number.isFinite(value) || value <= 0 || value > 100_000) {
      return `Укажите положительный ход ${axis.toUpperCase()}`;
    }
  }
  const maximumTravel = Math.max(draft.travelMm.x, draft.travelMm.y, draft.travelMm.z);
  if (
    !Number.isFinite(draft.maxJogDistanceMm) ||
    draft.maxJogDistanceMm < 0.01 ||
    draft.maxJogDistanceMm > maximumTravel
  ) {
    return "Максимальный jog должен быть от 0.01 mm до наибольшего хода оси";
  }
  if (draft.rotaryAxis) {
    const { travelDegrees, maxJogDegrees, maxFeedDegreesPerMin } = draft.rotaryAxis;
    if (!Number.isFinite(travelDegrees) || travelDegrees < 1) return "Укажите ход оси A";
    if (!Number.isFinite(maxJogDegrees) || maxJogDegrees < 0.01 || maxJogDegrees > travelDegrees) {
      return "Максимальный jog A должен быть в пределах её хода";
    }
    if (!Number.isFinite(maxFeedDegreesPerMin) || maxFeedDegreesPerMin < 1) {
      return "Укажите максимальную скорость оси A";
    }
  }
  return undefined;
};

export const formatMachineTravel = (draft: MachineProfileDraft): string =>
  `${draft.travelMm.x} × ${draft.travelMm.y} × ${draft.travelMm.z} mm`;
