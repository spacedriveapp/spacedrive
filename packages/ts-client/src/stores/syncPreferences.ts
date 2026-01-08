import { create } from "zustand";
import { persist } from "zustand/middleware";

interface SyncPreferencesStore {
  /**
   * Automatically switch to a library when it's received via sync from another device.
   * Default: true
   */
  autoSwitchOnSync: boolean;
  setAutoSwitchOnSync: (enabled: boolean) => void;
}

export const useSyncPreferencesStore = create<SyncPreferencesStore>()(
  persist(
    (set) => ({
      autoSwitchOnSync: true,
      setAutoSwitchOnSync: (enabled) => set({ autoSwitchOnSync: enabled }),
    }),
    {
      name: "spacedrive-sync-preferences",
    }
  )
);
