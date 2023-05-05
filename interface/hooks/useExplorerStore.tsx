import { proxy, useSnapshot } from 'valtio';
import { ExplorerItem } from '@sd/client';
import { resetStore } from '@sd/client/src/stores/util';

export type ExplorerLayoutMode = 'rows' | 'grid' | 'columns' | 'media';

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
	selectedRowIndex: 1 as number | null,
	showBytesInGridView: true,
	tagAssignMode: false,
	showInspector: false,
	multiSelectIndexes: [] as number[],
	contextMenuObjectId: null as number | null,
	contextMenuActiveObject: null as object | null,
	newThumbnails: {} as Record<string, boolean>,
	cutCopyState: {
		sourcePath: '', // this is used solely for preventing copy/cutting to the same path (as that will truncate the file)
		sourceLocationId: 0,
		sourcePathId: 0,
		actionType: 'Cut',
		active: false
	},
	quickViewObject: null as ExplorerItem | null,
	isRenaming: false,
	mediaColumns: 8,
	mediaAspectSquare: true
};

// Keep the private and use `useExplorerState` or `getExplorerStore` or you will get production build issues.
const explorerStore = proxy({
	...state,
	reset: () => resetStore(explorerStore, state),
	addNewThumbnail: (cas_id: string) => {
		explorerStore.newThumbnails[cas_id] = true;
	}
	// selectMore: (indexes: number[]) => {
	// 	if (!explorerStore.multiSelectIndexes.length && indexes.length) {
	// 		explorerStore.multiSelectIndexes = [explorerStore.selectedRowIndex, ...indexes];
	// 	} else {
	// 		explorerStore.multiSelectIndexes = [
	// 			...new Set([...explorerStore.multiSelectIndexes, ...indexes])
	// 		];
	// 	}
	// }
});

export function useExplorerStore() {
	// const { library } = useLibraryContext();

	// useEffect(() => {
	// 	explorerStore.reset();
	// }, [library.uuid]);

	return useSnapshot(explorerStore);
}

export function getExplorerStore() {
	return explorerStore;
}
