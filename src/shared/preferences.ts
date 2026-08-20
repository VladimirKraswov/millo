export interface ApplicationPreferences {
  readonly safeCommandMode: boolean;
}

export interface ApplicationPreferencesUpdate {
  readonly safeCommandMode: boolean;
}

export const defaultApplicationPreferences: ApplicationPreferences = {
  safeCommandMode: true,
};
