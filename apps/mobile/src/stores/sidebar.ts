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

interface SidebarStore {
  // Current library selection
  currentLibraryId: string | null;
  setCurrentLibrary: (id: string | null) => void;

  // Collapsed section groups
  collapsedGroups: string[];
  isGroupCollapsed: (groupId: string) => boolean;
  toggleGroup: (groupId: string) => void;

  // Drawer state
  isDrawerOpen: boolean;
  setDrawerOpen: (open: boolean) => void;
}

export const useSidebarStore = create<SidebarStore>()(
  persist(
    (set, get) => ({
      currentLibraryId: null,
      setCurrentLibrary: (id) => set({ currentLibraryId: id }),

      collapsedGroups: [],
      isGroupCollapsed: (groupId) => get().collapsedGroups.includes(groupId),
      toggleGroup: (groupId) =>
        set((state) => {
          const isCollapsed = state.collapsedGroups.includes(groupId);
          return {
            collapsedGroups: isCollapsed
              ? state.collapsedGroups.filter((id) => id !== groupId)
              : [...state.collapsedGroups, groupId],
          };
        }),

      isDrawerOpen: false,
      setDrawerOpen: (open) => set({ isDrawerOpen: open }),
    }),
    {
      name: "spacedrive-sidebar",
      storage: createJSONStorage(() => asyncStorageAdapter),
      partialize: (state) => ({
        currentLibraryId: state.currentLibraryId,
        collapsedGroups: state.collapsedGroups,
      }),
    }
  )
);
