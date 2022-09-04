import { proxy, ref } from 'valtio';

export type ExplorerLayoutMode = 'list' | 'grid';

export enum ExplorerKind {
	Location,
	Tag,
	Space
}

const state = {
	locationId: null as number | null,
	layoutMode: 'grid' as ExplorerLayoutMode,
	gridItemSize: 100,
	listItemSize: 40,
	selectedRowIndex: 1,
	showInspector: true,
	multiSelectIndexes: [] as number[],
	contextMenuObjectId: null as number | null,
	newThumbnails: {} as Record<string, boolean>
};

export const explorerStore = proxy({
	...state,
	reset: () => resetStore(explorerStore, state),
	addNewThumbnail: (cas_id: string) => {
		explorerStore.newThumbnails[cas_id] = true;
	},
	selectMore: (indexes: number[]) => {
		if (!explorerStore.multiSelectIndexes.length && indexes.length) {
			explorerStore.multiSelectIndexes = [explorerStore.selectedRowIndex, ...indexes];
		} else {
			explorerStore.multiSelectIndexes = [
				...new Set([...explorerStore.multiSelectIndexes, ...indexes])
			];
		}
	}
});

export function resetStore<T extends Record<string, any>, E extends Record<string, any>>(
	store: T,
	defaults: E
) {
	for (const key in defaults) {
		// @ts-ignore
		store[key] = defaults[key];
	}
}
