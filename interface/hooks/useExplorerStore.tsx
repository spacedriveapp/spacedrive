import { proxy, useSnapshot } from 'valtio';
import { ExplorerItem, FilePathSearchOrdering, ObjectSearchOrdering } from '@sd/client';
import { resetStore } from '@sd/client';
import { z } from "zod"

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
export type ObjectSearchOrderingKyes = UnionKeys<ObjectSearchOrdering> | 'none';

export const SortOrder = z.union([z.literal("Asc"), z.literal("Desc")])

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
	newThumbnails: {} as Record<string, boolean | undefined>,
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
	mediaAspectSquare: true,
	orderBy: 'dateCreated' as FilePathSearchOrderingKeys,
	orderByDirection: 'Desc' as z.infer<typeof SortOrder>,
	groupBy: 'none'
};

// Keep the private and use `useExplorerState` or `getExplorerStore` or you will get production build issues.
const explorerStore = proxy({
	...state,
	reset: () => resetStore(explorerStore, state),
	addNewThumbnail: (casId: string) => {
		explorerStore.newThumbnails[casId] = true;
	}
});

export function useExplorerStore() {
	return useSnapshot(explorerStore);
}

export function getExplorerStore() {
	return explorerStore;
}
