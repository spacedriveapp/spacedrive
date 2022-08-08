import create from 'zustand';

type LayoutMode = 'list' | 'grid';

interface ExplorerStore {
	selectedRowIndex: number;
	layoutMode: LayoutMode;
	setSelectedRowIndex: (index: number) => void;
	locationId: number;
	setLocationId: (index: number) => void;
	newThumbnails: Record<string, boolean>;
	addNewThumbnail: (cas_id: string) => void;
	setLayoutMode: (mode: LayoutMode) => void;
	reset: () => void;
}

export const useExplorerStore = create<ExplorerStore>((set) => ({
	layoutMode: 'grid',
	selectedRowIndex: 1,
	setSelectedRowIndex: (index) => set((state) => ({ ...state, selectedRowIndex: index })),
	locationId: -1,
	setLocationId: (id: number) => set((state) => ({ ...state, locationId: id })),
	newThumbnails: {},
	addNewThumbnail: (casId: string) =>
		set((state) => ({
			...state,
			newThumbnails: { ...state.newThumbnails, [casId]: true }
		})),
	setLayoutMode: (mode: LayoutMode) => set((state) => ({ ...state, layoutMode: mode })),
	reset: () => set(() => ({}))
}));
