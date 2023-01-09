import { resetStore } from '@sd/client';
import { proxy, useSnapshot } from 'valtio';

// TODO: Add "media"
export type ExplorerLayoutMode = 'list' | 'grid';

export enum ExplorerKind {
	Location,
	Tag,
	Space
}

const state = {
	locationId: null as number | null,
	layoutMode: 'grid' as ExplorerLayoutMode,
	// Using gridNumColumns instead of fixed size. We dynamically calculate the item size.
	gridNumColumns: 3,
	listItemSize: 40,
	newThumbnails: {} as Record<string, boolean>
};

const explorerStore = proxy({
	...state,
	reset: () => resetStore(explorerStore, state),
	addNewThumbnail: (cas_id: string) => {
		explorerStore.newThumbnails[cas_id] = true;
	}
});

export function useExplorerStore() {
	return useSnapshot(explorerStore);
}

export function getExplorerStore() {
	return explorerStore;
}
