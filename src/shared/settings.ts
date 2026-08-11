import type { HardwareInspection } from "./machine";
import type { MachineFingerprint, MachineProfileDraft, MachineProfileState } from "./profile";
import type { ControllerSnapshot } from "./machine";

export type SettingGroup =
  | "interface"
  | "pins"
  | "safety"
  | "homing"
  | "spindle"
  | "calibration"
  | "motion"
  | "travel"
  | "advanced";

export type SettingKind = "boolean" | "integer" | "decimal" | "mask";

export interface ControllerSettingValue {
  key: string;
  value: string;
  title: string;
  group: SettingGroup;
  kind: SettingKind;
  unit?: string;
  known: boolean;
}

export interface ControllerSettingsSnapshot {
  revision: number;
  firmwareVersion?: string;
  firmwareBuildInfo?: string;
  values: ControllerSettingValue[];
}

export interface ControllerSettingsState {
  snapshot: ControllerSettingsSnapshot;
  sessionBaseline: Record<string, string>;
  previousBaseline?: Record<string, string>;
  revisionCount: number;
  profileId?: string;
  fingerprint: MachineFingerprint;
}

export interface ControllerSettingEditRequest {
  key: string;
  value: string;
  confirmed: boolean;
  expectedValue: string;
  expectedRevision: number;
}

export interface ConnectOutcome {
  snapshot: ControllerSnapshot;
  inspection: HardwareInspection;
  settings: ControllerSettingsState;
  profiles: MachineProfileState;
  onboardingDraft?: MachineProfileDraft;
}
