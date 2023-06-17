import { proxy, useSnapshot } from 'valtio';
import { proxySet } from 'valtio/utils';
import { z } from 'zod';
import { ExplorerItem, FilePathSearchOrdering, ObjectSearchOrdering, resetStore } from '@sd/client';

type Join<K, P> = K extends string | number
	? P extends string | number
	? `${K}${'' extends P ? '' : '.'}${P}`
	: never
	: never;

type Leaves<T> = T extends object ? { [K in keyof T]-?: Join<K, Leaves<T[K]>> }[keyof T] : '';

type UnionKeys<T> = T extends any ? Leaves<T> : never;

export type ExplorerLayoutMode = 'rows' | 'grid' | 'columns' | 'media';

export enum ExplorerKind {
	Location,
	Tag,
	Space
}

export type CutCopyType = 'Cut' | 'Copy';

export type FilePathSearchOrderingKeys = UnionKeys<FilePathSearchOrdering> | 'none';
export type ObjectSearchOrderingKeys = UnionKeys<ObjectSearchOrdering> | 'none';

export const SortOrder = z.union([z.literal('Asc'), z.literal('Desc')]);

const state = {
	locationId: null as number | null,
	layoutMode: 'grid' as ExplorerLayoutMode,
	gridItemSize: 100,
	listItemSize: 40,
	selectedRowIndex: 1 as number | null,
	showBytesInGridView: true,
	tagAssignMode: false,
	showInspector: false,
	mediaPlayerVolume: 0.7,
	multiSelectIndexes: [] as number[],
	newThumbnails: proxySet() as Set<string>,
	cutCopyState: {
		sourcePath: '', // this is used solely for preventing copy/cutting to the same path (as that will truncate the file)
		sourceLocationId: 0,
		sourcePathId: 0,
		actionType: 'Cut',
		active: false
	},
	quickViewObject: null as ExplorerItem | null,
	mediaColumns: 8,
	mediaAspectSquare: true,
	orderBy: 'dateCreated' as FilePathSearchOrderingKeys,
	orderByDirection: 'Desc' as z.infer<typeof SortOrder>,
	groupBy: 'none'
};

export function flattenThumbnailKey(thumbKey: string[]) {
	return thumbKey.join('/');
}

// Keep the private and use `useExplorerState` or `getExplorerStore` or you will get production build issues.
const explorerStore = proxy({
	...state,
	reset: () => resetStore(explorerStore, state),
	addNewThumbnail: (thumbKey: string[]) => {
		explorerStore.newThumbnails.add(flattenThumbnailKey(thumbKey));
	},
	// this should be done when the explorer query is refreshed
	// prevents memory leak
	resetNewThumbnails: () => {
		explorerStore.newThumbnails.clear();
	}
});

export function useExplorerStore() {
	return useSnapshot(explorerStore);
}

export function getExplorerStore() {
	return explorerStore;
}
