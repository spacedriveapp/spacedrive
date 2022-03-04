import create from 'zustand';

interface AppState {
  bears: number;
}

const useAppStateStore = create<AppState>((set) => ({
  bears: 0,
  increasePopulation: () => set((state) => ({ bears: state.bears + 1 })),
  removeAllBears: () => set({ bears: 0 })
}));
