import type { MachineTravel, RotaryAxisProfile, SpindleControl, ZProbeSettings } from "./machine";

export const defaultZProbeSettings = (): ZProbeSettings => ({
  mode: "off",
  plateThicknessMm: 0,
  maxTravelMm: 10,
  probeFeedMmPerMin: 25,
  retractMm: 3,
  retractFeedMmPerMin: 100,
});

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
  rotaryAxis?: RotaryAxisProfile;
  maxJogDistanceMm: number;
  spindleControl: SpindleControl;
  floodCoolantControl: boolean;
  mistCoolantControl: boolean;
  homingInstalled: boolean;
  limitSwitchesInstalled: boolean;
  probeInstalled: boolean;
  probeSettings: ZProbeSettings;
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
  maxJogDistanceMm: number;
  rotaryAxis?: RotaryAxisProfile;
  spindleControl: SpindleControl;
  floodCoolantControl: boolean;
  mistCoolantControl: boolean;
  homingInstalled: boolean;
  limitSwitchesInstalled: boolean;
  probeInstalled: boolean;
  probeSettings: ZProbeSettings;
  emergencyStopInstalled: boolean;
}

export const selectedMachineProfile = (
  state: MachineProfileState,
): MachineProfile | undefined =>
  state.profiles.find((profile) => profile.id === state.selectedProfileId);
