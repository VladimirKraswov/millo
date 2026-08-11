import { invoke } from "@tauri-apps/api/core";

import type {
  MachineLocalSettingsUpdate,
  MachineProfileDraft,
  MachineProfileState,
} from "../shared/profile";

export const getMachineProfiles = (): Promise<MachineProfileState> =>
  invoke<MachineProfileState>("machine_profiles");

export const createMachineProfile = (
  draft: MachineProfileDraft,
): Promise<MachineProfileState> =>
  invoke<MachineProfileState>("create_machine_profile", { draft });

export const selectMachineProfile = (
  profileId: string,
): Promise<MachineProfileState> =>
  invoke<MachineProfileState>("select_machine_profile", { profileId });

export const updateMachineLocalSettings = (
  profileId: string,
  update: MachineLocalSettingsUpdate,
): Promise<MachineProfileState> =>
  invoke<MachineProfileState>("update_machine_local_settings", {
    profileId,
    update,
  });

export const detectMachineProfile = (
  transportId: string,
  baudRate: number,
): Promise<MachineProfileDraft> =>
  invoke<MachineProfileDraft>("detect_machine_profile", {
    transportId,
    baudRate,
  });
