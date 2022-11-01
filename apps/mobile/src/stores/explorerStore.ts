import { resetStore } from '@sd/client';
import { proxy, useSnapshot } from 'valtio';

export type ExplorerLayoutMode = 'list' | 'grid' | 'media';

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
	newThumbnails: {} as Record<string, boolean>
};

// Keep the private and use `useExplorerState` or `getExplorerStore` or you will get production build issues. (Was the case for desktop. Not sure if it's still the case for mobile)
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
