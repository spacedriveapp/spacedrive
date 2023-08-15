import { proxy, useSnapshot } from 'valtio';
import { proxySet } from 'valtio/utils';
import {
	DoubleClickAction,
	ExplorerItem,
	ExplorerLayout,
	ExplorerSettings,
	FilePathSearchOrdering,
	ObjectSearchOrdering,
	resetStore
} from '@sd/client';
import { SortOrder } from '~/app/route-schemas';

type Join<K, P> = K extends string | number
	? P extends string | number
		? `${K}${'' extends P ? '' : '.'}${P}`
		: never
	: never;

type Leaves<T> = T extends object ? { [K in keyof T]-?: Join<K, Leaves<T[K]>> }[keyof T] : '';

type UnionKeys<T> = T extends any ? Leaves<T> : never;

export enum ExplorerKind {
	Location,
	Tag,
	Space
}

export type CutCopyType = 'Cut' | 'Copy';

export type FilePathSearchOrderingKeys = UnionKeys<FilePathSearchOrdering> | 'none';
export type ObjectSearchOrderingKeys = UnionKeys<ObjectSearchOrdering> | 'none';

export const nullValuesHandler = (obj: ExplorerSettings) => {
	const newObj: any = { ...defaultExplorerSettings };
	Object.entries(obj).forEach(([key, value]) => {
		if (value !== null) {
			newObj[key] = value;
		}
	});
	return newObj;
};

export const defaultExplorerSettings = {
	layoutMode: 'grid' as ExplorerLayout,
	gridItemSize: 110 as number,
	showBytesInGridView: true as boolean,
	mediaColumns: 8 as number,
	mediaAspectSquare: false as boolean,
	orderBy: 'dateCreated' as FilePathSearchOrderingKeys,
	orderByDirection: 'Desc' as SortOrder,
	openOnDoubleClick: 'openFile' as DoubleClickAction,
	colSizes: {
		kind: 150,
		name: 350,
		sizeInBytes: 100,
		dateModified: 150,
		dateIndexed: 150,
		dateCreated: 150,
		dateAccessed: 150,
		contentId: 180,
		objectId: 180
	}
} as const;

const state = {
	tagAssignMode: false,
	showInspector: false,
	mediaPlayerVolume: 0.7,
	multiSelectIndexes: [] as number[],
	newThumbnails: proxySet() as Set<string>,
	cutCopyState: {
		sourceParentPath: '', // this is used solely for preventing copy/cutting to the same path (as that will truncate the file)
		sourceLocationId: 0,
		sourcePathId: 0,
		actionType: 'Cut',
		active: false
	},
	quickViewObject: null as ExplorerItem | null,
	groupBy: 'none',
	...defaultExplorerSettings
} as const;

export function flattenThumbnailKey(thumbKey: string[]) {
	return thumbKey.join('/');
}

// Keep the private and use `useExplorerState` or `getExplorerStore` or you will get production build issues.
const explorerStore = proxy({
	...state,
	reset: (_state?: typeof state) => resetStore(explorerStore, _state || state),
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

export function getExplorerSettings(): ExplorerSettings {
	return {
		layoutMode: explorerStore.layoutMode,
		gridItemSize: explorerStore.gridItemSize,
		showBytesInGridView: explorerStore.showBytesInGridView,
		mediaColumns: explorerStore.mediaColumns,
		mediaAspectSquare: explorerStore.mediaAspectSquare,
		orderBy: explorerStore.orderBy,
		orderByDirection: explorerStore.orderByDirection.toLowerCase() as SortOrder,
		openOnDoubleClick: explorerStore.openOnDoubleClick,
		colSizes: {
			...explorerStore.colSizes
		}
	};
}

export function isCut(id: number) {
	return (
		explorerStore.cutCopyState.active &&
		explorerStore.cutCopyState.actionType === 'Cut' &&
		explorerStore.cutCopyState.sourcePathId === id
	);
}
