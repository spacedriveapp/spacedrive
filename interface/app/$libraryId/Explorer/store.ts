import { proxy } from 'valtio';
import { proxySet } from 'valtio/utils';
import { z } from 'zod';
import {
	ThumbKey,
	resetStore,
	type DoubleClickAction,
	type ExplorerItem,
	type ExplorerLayout,
	type ExplorerSettings,
	type Ordering
} from '@sd/client';
import i18n from '~/app/I18n';

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
	showInspector: false,
	showMoreInfo: false,
	newLocationToRedirect: null as null | number,
	mediaPlayerVolume: 0.7,
	newThumbnails: proxySet() as Set<string>,
	cutCopyState: { type: 'Idle' } as CutCopyState,
	drag: null as null | DragState,
	isTagAssignModeActive: false,
	isDragSelecting: false,
	isRenaming: false,
	// Used for disabling certain keyboard shortcuts when command palette is open
	isCMDPOpen: false,
	isContextMenuOpen: false,
	quickRescanLastRun: Date.now() - 200,
	// Map = { hotkey: '0'...'9', tagId: 1234 }
	tagBulkAssignHotkeys: [] as Array<{ hotkey: string; tagId: number }>
};

export function flattenThumbnailKey(thumbKey: ThumbKey) {
	return `${thumbKey.base_directory_str}/${thumbKey.shard_hex}/${thumbKey.cas_id}`;
}

export const explorerStore = proxy({
	...state,
	reset: (_state?: typeof state) => resetStore(explorerStore, _state || state),
	addNewThumbnail: (thumbKey: ThumbKey) => {
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
	z.literal('name').describe(i18n.t('name')),
	z.literal('sizeInBytes').describe(i18n.t('size')),
	z.literal('dateModified').describe(i18n.t('date_modified')),
	z.literal('dateIndexed').describe(i18n.t('date_indexed')),
	z.literal('dateCreated').describe(i18n.t('date_created')),
	z.literal('object.dateAccessed').describe(i18n.t('date_accessed')),
	z.literal('object.mediaData.epochTime').describe(i18n.t('date_taken'))
]);

export const objectOrderingKeysSchema = z.union([
	z.literal('dateAccessed').describe(i18n.t('date_accessed')),
	z.literal('kind').describe(i18n.t('kind')),
	z.literal('mediaData.epochTime').describe(i18n.t('date_taken'))
]);

export const nonIndexedPathOrderingSchema = z.union([
	z.literal('name').describe(i18n.t('name')),
	z.literal('sizeInBytes').describe(i18n.t('size')),
	z.literal('dateCreated').describe(i18n.t('date_created')),
	z.literal('dateModified').describe(i18n.t('date_modified'))
]);
