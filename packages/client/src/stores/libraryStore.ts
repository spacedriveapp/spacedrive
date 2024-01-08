import { createMutable } from 'solid-js/store';

import { useSolidStore } from '../solid';

export const libraryStore = createMutable({
	onlineLocations: [] as number[][]
});

export function useLibraryStore() {
	return useSolidStore(libraryStore);
}
