import type { MachineTravel, SpindleControl } from "./machine";

export interface MachineConnectionPreset {
  transportId: string;
  baudRate: number;
  fingerprint?: MachineFingerprint;
}

export type IdentityConfidence = "strong" | "portBound" | "synthetic";

export interface MachineFingerprint {
  key: string;
  confidence: IdentityConfidence;
  label: string;
}

export interface DetectedController {
  firmwareVersion?: string;
  firmwareBuildInfo?: string;
}

export interface MachineProfileDraft {
  name: string;
  travelMm: MachineTravel;
  spindleControl: SpindleControl;
  homingInstalled: boolean;
  limitSwitchesInstalled: boolean;
  probeInstalled: boolean;
  emergencyStopInstalled: boolean;
  connection?: MachineConnectionPreset;
  detectedController?: DetectedController;
}

export interface MachineProfile extends MachineProfileDraft {
  id: string;
}

export interface MachineProfileState {
  profiles: MachineProfile[];
  selectedProfileId?: string;
}

export interface MachineLocalSettingsUpdate {
  name: string;
  spindleControl: SpindleControl;
  homingInstalled: boolean;
  limitSwitchesInstalled: boolean;
  probeInstalled: boolean;
  emergencyStopInstalled: boolean;
}

export const selectedMachineProfile = (
  state: MachineProfileState,
): MachineProfile | undefined =>
  state.profiles.find((profile) => profile.id === state.selectedProfileId);
