import { create } from "zustand";
import { persist } from "zustand/middleware";

interface ViewSettings {
  gridSize: number;
  gapSize: number;
  showFileSize: boolean;
  columnWidth: number;
}

interface SpaceItemViewPreferences {
  viewMode: "grid" | "list" | "media" | "column" | "size" | "knowledge";
  viewSettings: ViewSettings;
}

interface ViewPreferencesStore {
  preferences: Record<string, SpaceItemViewPreferences>;
  getPreferences: (spaceItemId: string) => SpaceItemViewPreferences | undefined;
  setPreferences: (
    spaceItemId: string,
    prefs: Partial<SpaceItemViewPreferences>
  ) => void;
}

const DEFAULT_VIEW_SETTINGS: ViewSettings = {
  gridSize: 120,
  gapSize: 16,
  showFileSize: true,
  columnWidth: 256,
};

const DEFAULT_PREFERENCES: SpaceItemViewPreferences = {
  viewMode: "grid",
  viewSettings: DEFAULT_VIEW_SETTINGS,
};

export const useViewPreferencesStore = create<ViewPreferencesStore>()(
  persist(
    (set, get) => ({
      preferences: {},
      getPreferences: (spaceItemId) => {
        return get().preferences[spaceItemId];
      },
      setPreferences: (spaceItemId, prefs) =>
        set((state) => ({
          preferences: {
            ...state.preferences,
            [spaceItemId]: {
              ...DEFAULT_PREFERENCES,
              ...state.preferences[spaceItemId],
              ...prefs,
              viewSettings: {
                ...DEFAULT_VIEW_SETTINGS,
                ...state.preferences[spaceItemId]?.viewSettings,
                ...(prefs.viewSettings || {}),
              },
            },
          },
        })),
    }),
    {
      name: "spacedrive-view-preferences",
    }
  )
);
