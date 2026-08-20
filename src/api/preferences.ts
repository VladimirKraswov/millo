import { invoke } from "@tauri-apps/api/core";

import type {
  ApplicationPreferences,
  ApplicationPreferencesUpdate,
} from "../shared/preferences";

export const getApplicationPreferences = (): Promise<ApplicationPreferences> =>
  invoke<ApplicationPreferences>("application_preferences");

export const updateApplicationPreferences = (
  update: ApplicationPreferencesUpdate,
): Promise<ApplicationPreferences> =>
  invoke<ApplicationPreferences>("update_application_preferences", { update });
