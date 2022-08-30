import produce from 'immer';
import create from 'zustand';

type LayoutMode = 'list' | 'grid';

export enum ExplorerKind {
	Location,
	Tag,
	Space
}

type ExplorerStore = {
	layoutMode: LayoutMode;
	selectedRowIndex: number;
	multiSelectIndexes: number[];
	contextMenuObjectId: number | null;
	locationId: number; // todo: check if even needed
	path: string;
	limit: number;
	newThumbnails: Record<string, boolean>;
	addNewThumbnail: (cas_id: string) => void;
	selectMore: (indexes: number[]) => void;
	reset: () => void;
	set: (changes: Partial<ExplorerStore>) => void;
};

export const useExplorerStore = create<ExplorerStore>((set) => ({
	layoutMode: 'grid',
	selectedRowIndex: 1,
	multiSelectIndexes: [],
	contextMenuObjectId: -1,
	locationId: -1,
	newThumbnails: {},
	path: '',
	limit: 100,
	addNewThumbnail: (cas_id) =>
		set((state) =>
			produce(state, (draft) => {
				draft.newThumbnails[cas_id] = true;
			})
		),
	selectMore: (indexes) => {
		set((state) =>
			produce(state, (draft) => {
				if (!draft.multiSelectIndexes.length && indexes.length) {
					draft.multiSelectIndexes = [draft.selectedRowIndex, ...indexes];
				} else {
					draft.multiSelectIndexes = [...new Set([...draft.multiSelectIndexes, ...indexes])];
				}
			})
		);
	},
	reset: () => set(() => ({})),
	set: (changes) => set((state) => ({ ...state, ...changes }))
}));
