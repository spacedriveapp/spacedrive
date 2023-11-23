import { proxy, useSnapshot } from 'valtio';
import { proxySet } from 'valtio/utils';
import { z } from 'zod';
import {
	resetStore,
	type DoubleClickAction,
	type ExplorerItem,
	type ExplorerLayout,
	type ExplorerSettings,
	type SortOrder
} from '@sd/client';

export enum ExplorerKind {
	Location,
	Tag,
	Space
}

export type Ordering = { field: string; value: SortOrder | Ordering };
// branded type for added type-safety
export type OrderingKey = string & {};

type OrderingValue<T extends Ordering, K extends string> = Extract<T, { field: K }>['value'];

export type OrderingKeys<T extends Ordering> = T extends Ordering
	? {
			[K in T['field']]: OrderingValue<T, K> extends SortOrder
				? K
				: OrderingValue<T, K> extends Ordering
				? `${K}.${OrderingKeys<OrderingValue<T, K>>}`
				: never;
	  }[T['field']]
	: never;

export function orderingKey(ordering: Ordering): OrderingKey {
	let base = ordering.field;

	if (typeof ordering.value === 'object') {
		base += `.${orderingKey(ordering.value)}`;
	}

	return base;
}

export function createOrdering<TOrdering extends Ordering = Ordering>(
	key: OrderingKey,
	value: SortOrder
): TOrdering {
	return key
		.split('.')
		.reverse()
		.reduce((acc, field, i) => {
			if (i === 0)
				return {
					field,
					value
				};
			else return { field, value: acc };
		}, {} as any);
}

export function getOrderingDirection(ordering: Ordering): SortOrder {
	if (typeof ordering.value === 'object') return getOrderingDirection(ordering.value);
	else return ordering.value;
}

export const createDefaultExplorerSettings = <TOrder extends Ordering>(args?: {
	order?: TOrder | null;
}) =>
	({
		order: args?.order ?? null,
		layoutMode: 'grid' as ExplorerLayout,
		gridItemSize: 110 as number,
		gridGap: 8 as number,
		showBytesInGridView: true as boolean,
		showHiddenFiles: false as boolean,
		mediaColumns: 8 as number,
		mediaAspectSquare: false as boolean,
		mediaViewWithDescendants: true as boolean,
		openOnDoubleClick: 'openFile' as DoubleClickAction,
		colVisibility: {
			name: true,
			kind: true,
			sizeInBytes: true,
			dateCreated: true,
			dateModified: true,
			dateImageTaken: true,
			dateAccessed: false,
			dateIndexed: false,
			imageResolution: true,
			contentId: false,
			objectId: false
		},
		colSizes: {
			name: 350,
			kind: 150,
			sizeInBytes: 100,
			dateCreated: 150,
			dateModified: 150,
			dateImageTaken: 150,
			dateAccessed: 150,
			dateIndexed: 150,
			imageResolution: 180,
			contentId: 180,
			objectId: 180
		}
	}) satisfies ExplorerSettings<TOrder>;

type CutCopyState =
	| {
			type: 'Idle';
	  }
	| {
			type: 'Cut' | 'Copy';
			sourceParentPath: string; // this is used solely for preventing copy/cutting to the same path (as that will truncate the file)
			indexedArgs?: {
				sourceLocationId: number;
				sourcePathIds: number[];
			};
			ephemeralArgs?: {
				sourcePaths: string[];
			};
	  };

type DragState =
	| {
			type: 'touched';
	  }
	| {
			type: 'dragging';
			items: ExplorerItem[];
			sourceParentPath: string;
			sourceLocationId?: number;
	  };

const state = {
	tagAssignMode: false,
	showInspector: false,
	showMoreInfo: false,
	newLocationToRedirect: null as null | number,
	mediaPlayerVolume: 0.7,
	newThumbnails: proxySet() as Set<string>,
	cutCopyState: { type: 'Idle' } as CutCopyState,
	drag: null as null | DragState,
	isDragSelecting: false,
	isRenaming: false,
	isContextMenuOpen: false
};

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

export function isCut(item: ExplorerItem) {
	const { cutCopyState } = getExplorerStore();

	switch (item.type) {
		case 'NonIndexedPath':
			return (
				cutCopyState.type === 'Cut' &&
				cutCopyState.ephemeralArgs != undefined &&
				cutCopyState.ephemeralArgs.sourcePaths.includes(item.item.path)
			);

		case 'Path':
			return (
				cutCopyState.type === 'Cut' &&
				cutCopyState.indexedArgs != undefined &&
				cutCopyState.indexedArgs.sourcePathIds.includes(item.item.id)
			);

		default:
			return false;
	}
}

export const filePathOrderingKeysSchema = z.union([
	z.literal('name').describe('Name'),
	z.literal('sizeInBytes').describe('Size'),
	z.literal('dateModified').describe('Date Modified'),
	z.literal('dateIndexed').describe('Date Indexed'),
	z.literal('dateCreated').describe('Date Created'),
	z.literal('object.dateAccessed').describe('Date Accessed'),
	z.literal('object.mediaData.epochTime').describe('Date Taken')
]);

export const objectOrderingKeysSchema = z.union([
	z.literal('dateAccessed').describe('Date Accessed'),
	z.literal('kind').describe('Kind'),
	z.literal('mediaData.epochTime').describe('Date Taken')
]);

export const nonIndexedPathOrderingSchema = z.union([
	z.literal('name').describe('Name'),
	// z.literal('sizeInBytes').describe('Size'),
	z.literal('dateCreated').describe('Date Created'),
	z.literal('dateModified').describe('Date Modified')
]);
