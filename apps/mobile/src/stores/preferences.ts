import AsyncStorage from "@react-native-async-storage/async-storage";
import { create } from "zustand";
import {
  createJSONStorage,
  persist,
  type StateStorage,
} from "zustand/middleware";

// AsyncStorage adapter for Zustand
const asyncStorageAdapter: StateStorage = {
  getItem: async (name: string) => {
    return await AsyncStorage.getItem(name);
  },
  setItem: async (name: string, value: string) => {
    await AsyncStorage.setItem(name, value);
  },
  removeItem: async (name: string) => {
    await AsyncStorage.removeItem(name);
  },
};

export type ThemeMode = "dark" | "light" | "system";

interface ViewPreferences {
  viewMode: "grid" | "list" | "media";
  gridSize: number;
  showHiddenFiles: boolean;
}

interface PreferencesStore {
  // Theme
  themeMode: ThemeMode;
  setThemeMode: (mode: ThemeMode) => void;

  // Haptics
  hapticsEnabled: boolean;
  setHapticsEnabled: (enabled: boolean) => void;

  // View preferences per location/path
  viewPreferences: Record<string, ViewPreferences>;
  getViewPreferences: (key: string) => ViewPreferences;
  setViewPreferences: (key: string, prefs: Partial<ViewPreferences>) => void;

  // Onboarding
  hasCompletedOnboarding: boolean;
  setHasCompletedOnboarding: (completed: boolean) => void;

  // Sync preferences
  autoSwitchOnSync: boolean;
  setAutoSwitchOnSync: (enabled: boolean) => void;
}

const defaultViewPreferences: ViewPreferences = {
  viewMode: "grid",
  gridSize: 120,
  showHiddenFiles: false,
};

export const usePreferencesStore = create<PreferencesStore>()(
  persist(
    (set, get) => ({
      // Theme
      themeMode: "dark",
      setThemeMode: (mode) => set({ themeMode: mode }),

      // Haptics
      hapticsEnabled: true,
      setHapticsEnabled: (enabled) => set({ hapticsEnabled: enabled }),

      // View preferences
      viewPreferences: {},
      getViewPreferences: (key) => {
        return get().viewPreferences[key] ?? defaultViewPreferences;
      },
      setViewPreferences: (key, prefs) =>
        set((state) => ({
          viewPreferences: {
            ...state.viewPreferences,
            [key]: {
              ...(state.viewPreferences[key] ?? defaultViewPreferences),
              ...prefs,
            },
          },
        })),

      // Onboarding
      hasCompletedOnboarding: false,
      setHasCompletedOnboarding: (completed) =>
        set({ hasCompletedOnboarding: completed }),

      // Sync preferences
      autoSwitchOnSync: true,
      setAutoSwitchOnSync: (enabled) => set({ autoSwitchOnSync: enabled }),
    }),
    {
      name: "spacedrive-preferences",
      storage: createJSONStorage(() => asyncStorageAdapter),
    }
  )
);
