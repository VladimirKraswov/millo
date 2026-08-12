import type { MachineProfileDraft } from "../../shared/profile";
import { defaultZProbeSettings } from "../../shared/profile";

export const emptyMachineProfileDraft = (): MachineProfileDraft => ({
  name: "",
  travelMm: { x: 0, y: 0, z: 0 },
  maxJogDistanceMm: 50,
  spindleControl: "manual",
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
  return undefined;
};

export const formatMachineTravel = (draft: MachineProfileDraft): string =>
  `${draft.travelMm.x} × ${draft.travelMm.y} × ${draft.travelMm.z} mm`;
