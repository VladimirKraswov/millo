import type { MachineProfileDraft } from "../../shared/profile";

export const emptyMachineProfileDraft = (): MachineProfileDraft => ({
  name: "",
  travelMm: { x: 0, y: 0, z: 0 },
  spindleControl: "manual",
  homingInstalled: false,
  limitSwitchesInstalled: false,
  probeInstalled: false,
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
  return undefined;
};

export const formatMachineTravel = (draft: MachineProfileDraft): string =>
  `${draft.travelMm.x} × ${draft.travelMm.y} × ${draft.travelMm.z} mm`;
