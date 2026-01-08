import { create } from "zustand";
import { persist } from "zustand/middleware";

type SortBy = string;

interface SortPreferencesStore {
  preferences: Record<string, SortBy>;
  getPreferences: (pathKey: string) => SortBy | undefined;
  setPreferences: (pathKey: string, sortBy: SortBy) => void;
}

export const useSortPreferencesStore = create<SortPreferencesStore>()(
  persist(
    (set, get) => ({
      preferences: {},
      getPreferences: (pathKey) => {
        return get().preferences[pathKey];
      },
      setPreferences: (pathKey, sortBy) =>
        set((state) => ({
          preferences: {
            ...state.preferences,
            [pathKey]: sortBy,
          },
        })),
    }),
    {
      name: "spacedrive-sort-preferences",
    }
  )
);
