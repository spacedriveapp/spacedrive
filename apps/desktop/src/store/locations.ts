import create from 'zustand';
import immer, { produce } from 'immer';

export interface Location {
  id: string;
  name: string;
  path: string;
  total_capacity: number;
  available_capacity: number;
  is_removable: boolean;
  is_ejectable: boolean;
  is_root_filesystem: boolean;
}

interface LocationStore {
  locations: Record<string, Location>;
  setLocations: (locations: Location[]) => void;
}

export const useLocationStore = create<LocationStore>((set, get) => ({
  locations: {},
  setLocations: (locations) =>
    set((state) =>
      produce(state, (draft) => {
        for (let location of locations) {
          draft.locations[location.path] = location;
        }
      })
    )
}));

export const useLocations = () => {
  return useLocationStore((store) => Object.values(store.locations));
};
