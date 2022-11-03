import { ExplorerItem, onLibraryChange } from '@sd/client';
import { proxy, useSnapshot } from 'valtio';

import { resetStore } from '@sd/client/src/stores/util';

export type ExplorerLayoutMode = 'list' | 'grid' | 'media' | 'columns' | 'timeline';

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
	tagAssignMode: false,
	showInspector: true,
	multiSelectIndexes: [] as number[],
	contextMenuActiveItem: null as ExplorerItem | null,
	newThumbnails: {} as Record<string, boolean>
};

onLibraryChange(() => getExplorerStore().reset());

// Keep the private and use `useExplorerState` or `getExplorerStore` or you will get production build issues.
const explorerStore = proxy({
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

export function useExplorerStore() {
	return useSnapshot(explorerStore);
}

export function getExplorerStore() {
	return explorerStore;
}
