import { useEffect } from 'react';
import { proxy, useSnapshot } from 'valtio';
import { useLibraryContext } from '@sd/client';
import { resetStore } from '@sd/client/src/stores/util';

export type ExplorerLayoutMode = 'list' | 'grid' | 'columns' | 'media';

export enum ExplorerKind {
	Location,
	Tag,
	Space
}

export type CutCopyType = 'Cut' | 'Copy';

const state = {
	locationId: null as number | null,
	layoutMode: 'grid' as ExplorerLayoutMode,
	gridItemSize: 100,
	listItemSize: 40,
	selectedRowIndex: 1,
	tagAssignMode: false,
	showInspector: true,
	multiSelectIndexes: [] as number[],
	contextMenuObjectId: null as number | null,
	contextMenuActiveObject: null as object | null,
	newThumbnails: {} as Record<string, boolean>,
	cutCopyState: {
		sourceLocationId: 0,
		sourcePathId: 0,
		actionType: 'Cut',
		active: false
	}
};

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
	const { library } = useLibraryContext();

	useEffect(() => {
		explorerStore.reset();
	}, [library.uuid]);

	return useSnapshot(explorerStore);
}

export function getExplorerStore() {
	return explorerStore;
}
