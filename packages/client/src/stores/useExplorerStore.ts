import create from 'zustand';

type ExplorerStore = {
	selectedRowIndex: number;
	setSelectedRowIndex: (index: number) => void;
	locationId: number;
	setLocationId: (index: number) => void;
	newThumbnails: Record<string, boolean>;
	addNewThumbnail: (cas_id: string) => void;
};

export const useExplorerStore = create<ExplorerStore>((set) => ({
	selectedRowIndex: 1,
	setSelectedRowIndex: (index) => set((state) => ({ ...state, selectedRowIndex: index })),
	locationId: -1,
	setLocationId: (id: number) => set((state) => ({ ...state, locationId: id })),
	newThumbnails: {},
	addNewThumbnail: (cas_id: string) =>
		set((state) => ({
			...state,
			newThumbnails: { ...state.newThumbnails, [cas_id]: true }
		}))
}));
