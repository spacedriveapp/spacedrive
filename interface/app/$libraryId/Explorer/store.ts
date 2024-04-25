import { proxy } from 'valtio';
import { proxySet } from 'valtio/utils';
import { z } from 'zod';
import {
	resetStore,
	type DoubleClickAction,
	type ExplorerItem,
	type ExplorerLayout,
	type ExplorerSettings,
	type Ordering
} from '@sd/client';

import {
	DEFAULT_LIST_VIEW_ICON_SIZE,
	DEFAULT_LIST_VIEW_TEXT_SIZE,
	LIST_VIEW_ICON_SIZES,
	LIST_VIEW_TEXT_SIZES
} from './View/ListView/useTable';

export enum ExplorerKind {
	Location,
	Tag,
	Space
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
		listViewIconSize: DEFAULT_LIST_VIEW_ICON_SIZE as keyof typeof LIST_VIEW_ICON_SIZES,
		listViewTextSize: DEFAULT_LIST_VIEW_TEXT_SIZE as keyof typeof LIST_VIEW_TEXT_SIZES,
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

export type CutCopyState =
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
			sourcePath?: string;
			sourceLocationId?: number;
			sourceTagId?: number;
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
	// Used for disabling certain keyboard shortcuts when command palette is open
	isCMDPOpen: false,
	isContextMenuOpen: false,
	quickRescanLastRun: Date.now() - 200
};

export function flattenThumbnailKey(thumbKey: string[]) {
	return thumbKey.join('/');
}

export const explorerStore = proxy({
	...state,
	reset: (_state?: typeof state) => resetStore(explorerStore, _state || state),
	addNewThumbnail: (thumbKey: string[]) => {
		explorerStore.newThumbnails.add(flattenThumbnailKey(thumbKey));
	},
	resetCache: () => {
		explorerStore.newThumbnails.clear();
		// explorerStore.newFilePathsIdentified.clear();
	}
});

export function isCut(item: ExplorerItem, cutCopyState: CutCopyState) {
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
	z.literal('sizeInBytes').describe('Size'),
	z.literal('dateCreated').describe('Date Created'),
	z.literal('dateModified').describe('Date Modified')
]);
